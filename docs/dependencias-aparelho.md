# Dependências que faltam no aparelho

Relatório do que o **Giochi System** precisa colocar na imagem para o launcher
funcionar completo. Levantado em 03/11/2026 no aparelho `giochi-system-x55`
(Buildroot 2024.02, aarch64, kernel 6.12.0) e reverificado no mesmo dia.

Este documento é para o responsável pela imagem. Ele não descreve código deste
repositório: descreve **o que a imagem precisa ter**. O que o app faz com cada
item está em [contrato-app-sistema.md](contrato-app-sistema.md); a ordem de
validação quando a imagem nova chegar está em
[checklist-imagem.md](checklist-imagem.md).

## Resumo

**O texto está resolvido; o `respawn` ainda não, e por um motivo de horário.** O
que resta é verificar em hardware — a verificação no artefato já foi feita, e
está abaixo.

| Item | Imagem antiga (X55 em uso) | Imagem nova (`out/`, 03/11 19h) |
| --- | --- | --- |
| `libSDL2-2.0.so.0` | tem (2.28.5) | tem |
| `libSDL2_ttf` | **falta** | **tem**, com `NEEDED libharfbuzz.so.0` |
| `libfreetype` / `libharfbuzz` | **falta** | **têm** |
| fonte TTF | **falta** | **`DejaVuSans.ttf`, 757076 bytes** |
| `::respawn:` no `inittab` | **falta** | **falta também** — ver abaixo |
| `/dev/input/event*` | 5 dispositivos | evdev |
| `/dev/dri/card0` | tem | KMSDRM |

> **Atualização de 06/11/2026: o X55 já roda a imagem de 04/11.** Medido pelo
> `/proc/version` (kernel compilado em `Wed Nov 4 21:04:03 UTC 2026`) e pelo
> `giochi-selftest`, que já tem o bloco "bibliotecas do launcher" — presente só
> na imagem nova. Ou seja: a coluna "imagem antiga" abaixo é **histórico**, não o
> estado do aparelho. As cinco bibliotecas de texto e o `DejaVuSans.ttf` estão na
> imagem; `readelf` continua ausente. Ver
> [o que o arcade-volleyball precisa](#o-que-o-arcade-volleyball-precisa-nada).

> **A imagem do X35H em `out/` foi construída 14 minutos ANTES da correção do
> `post-build.sh`.** Ela tem tudo de texto e **não** tem o `respawn`. Verificado
> no `inittab` do rootfs extraído, não por leitura de código. Rebuild resolve —
> e a imagem do X55 em `out/powkiddy-x55/` está vazia, então ela precisa ser
> construída de qualquer forma.

Evidência de que só falta o texto na imagem **antiga**: frames reais capturados
do aparelho em [`app/launcher/README.md`](../app/launcher/README.md) e a saída

```text
aviso: compilado sem a feature `texto`; nenhum texto será desenhado
```

## O que pedir, em ordem de prioridade

### 1. `SDL2_ttf` + `freetype` + `harfbuzz` (necessário para texto no launcher)

No Buildroot do **Giochi System**, o `Config.in` precisa de:

```text
BR2_PACKAGE_SDL2=y
BR2_PACKAGE_SDL2_TTF=y
BR2_PACKAGE_HARFBUZZ=y
BR2_PACKAGE_DEJAVU_SANS=y
```

A biblioteca resultante é `libSDL2_ttf-2.0.so.0`, instalada em `/usr/lib`.

**Três armadilhas, todas encontradas pela equipe do System em 03/11/2026** — cada
uma produz build verde e imagem quebrada:

| Armadilha | O que acontece |
| --- | --- |
| `sdl2_ttf` é **pacote à parte** | não é opção do `sdl2`; está em `package/sdl2_ttf/` |
| `SDL2_TTF` faz `select FREETYPE`, mas **não** seleciona harfbuzz | harfbuzz tem de ser explícito |
| `BR2_PACKAGE_DEJAVU=y` instala **zero** fontes | `dejavu.mk` só preenche dentro de `ifeq ($(BR2_PACKAGE_DEJAVU_SANS),y)` |

A quarta armadilha é de build incremental, e é a mais traiçoeira:

> **Acrescentar uma dependência não reconfigura um pacote já construído.**

O `sdl2_ttf` estava compilado com `--disable-harfbuzz`; pôr
`BR2_PACKAGE_HARFBUZZ=y` no `.config` não mudou nada. Só resolveu removendo
`build/sdl2_ttf-*` antes de recompilar. Medido:

```text
TTF_USE_HARFBUZZ='0'   →   TTF_USE_HARFBUZZ='1'
readelf -d libSDL2_ttf →   NEEDED libharfbuzz.so.0
```

**Estado em 03/11/2026: resolvido.** Verificado no artefato real (imagem do X35H
em `out/`), não só na documentação:

```text
/usr/lib/libSDL2_ttf-2.0.so.0.2200.0   NEEDED libharfbuzz.so.0  ✔
/usr/lib/libfreetype.so.6                                            ✔
/usr/share/fonts/dejavu/DejaVuSans.ttf   757076 bytes                ✔
```

A lição que fica, e que vale dos dois lados: **biblioteca presente não prova uso.**
Só `readelf -d` mostra as `DT_NEEDED`. Do nosso lado a mesma regra apareceu com
outro nome — o dispositivo *declarar* um código de tecla e o kernel *entregar* o
evento são fatos diferentes.

### 2. Uma fonte TTF com licença permissiva

**DejaVu Sans** é a escolha, e o motivo é prático: licença permissiva (Bitstream
Vera derivative), cobertura ampla de latim, e é o que já está instalado em
qualquer distro Linux — então o mesmo binário funciona no PC e no aparelho.

Caminho esperado pelo launcher, nesta ordem:

```text
assets/fonts/DejaVuSans.ttf                 (dentro do pacote do app)
/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
/usr/share/fonts/TTF/DejaVuSans.ttf
```

Tamanho: ~757 KB. Opções para a imagem:

- adicionar `BR2_PACKAGE_DEJAVU=y` (se a árvore tiver o pacote), ou
- embarcar o TTF dentro do próprio pacote do launcher (`assets/fonts/`), que é o
  caminho preferido — assim a imagem não depende de pacote de fonte e o app é
  autossuficiente.

O TTF **não está neste repositório** (arte de terceiro); copiar de
`/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` para
`app/launcher/assets/fonts/DejaVuSans.ttf`.

### 3. (Opcional) Cabeçalhos de desenvolvimento

**Não são necessários para a imagem final**, só para quem for compilar no próprio
aparelho. A compilação é feita no PC com cross-compile — ver
[compilar-launcher.md](compilar-launcher.md).

## Além de dependências: o `inittab` não inicia o launcher

Achado ao investigar por que a tela ficava no console: **`/etc/inittab` do
aparelho não tem entrada para o `giochi-init`.** O script existe em
`/usr/bin/giochi-init` e está correto, mas nada o chama — então o launcher
**nunca sobe no boot**. Hoje ele só aparece se alguém o executar por SSH.

O próprio `giochi-init` documenta a entrada esperada:

```text
::respawn:/usr/bin/giochi-init
```

Sem `::respawn:`, além de não subir, perde-se a recuperação: se o launcher
**terminar** — panic, falta de memória, erro ao abrir o DRM —, o aparelho fica
numa tela preta sem saída a não ser SSH. Com `once` ele sobe uma vez e não volta;
com `respawn` o init o reinicia.

O launcher **não** usa `exec` hoje: ele cria processo filho e espera
(`Command::status()` em `crates/ui-sdl/src/system.rs`), então sobrevive ao término
de um comando. O argumento imediato para `respawn` é a morte do próprio launcher,
não o fim de um jogo.

A ressalva é a etapa seguinte: `docs/integracao.md` decide que, ao lançar um jogo,
o launcher **cederá o processo** (`exec`) em vez de criar filho — só um processo
pode ter o DRM master. Quando isso entrar, o `respawn` deixa de ser recuperação e
passa a ser obrigatório para a interface voltar. Melhor já estar na imagem.

Consequência prática enquanto isso não entra: para ver a interface é preciso
subir o launcher à mão.

```bash
bash scripts/aparelho.sh subir     # faz o que o `::respawn:` faria
```

Ou, sem o script, no aparelho:

```sh
setsid nohup env SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher > /tmp/ui.log 2>&1 < /dev/null &
```

**Um aparelho que só mostra interface por SSH não é um console** — este é o item
de maior impacto da lista, acima até do texto.

## O que o `arcade-volleyball` precisa: **nada**

Levantado em 06/11/2026 no X55, e é a exceção que confirma a regra deste
documento: o primeiro app que não pede nada à imagem.

### Como foi medido

Não por lista de cabeçalho, e sim **do processo vivo**: `/proc/<pid>/maps` de um
`arcade-volleyball` rodando por KMSDRM. Isso separa "declara" de "usa" — a mesma
distinção que o `giochi-selftest` faz com `readelf -d`, só que no sentido
inverso: aqui a pergunta é o que o kernel de fato mapeou.

```bash
# no aparelho, com o jogo rodando
grep '\.so' /proc/$(pidof arcade-volleyball)/maps | tr -s ' ' | rev | cut -d' ' -f1 | rev | sort -u
```

### As 15 bibliotecas carregadas

**Diretas** (as 6 `DT_NEEDED` do ELF — conferidas com `readelf -d` no PC):

| Biblioteca | Já existe no rootfs |
| --- | --- |
| `libSDL2-2.0.so.0` | sim — 2.28.5, o mesmo do launcher |
| `libstdc++.so.6` | sim — 6.0.30 |
| `libm.so.6` | sim |
| `libgcc_s.so.1` | sim |
| `libc.so.6` | sim — glibc 2.38 |
| `ld-linux-aarch64.so.1` | sim |

**Transitivas** (puxadas pelo SDL2 e pelo driver GLES, 9):

| Biblioteca | Vem de |
| --- | --- |
| `libdrm.so.2.4.0` | backend KMSDRM do SDL2 |
| `libgbm.so.1.0.0` | alocação de buffer para o scanout |
| `libEGL.so.1.0.0` | contexto GLES |
| `libGLESv2.so.2.0.0` | desenho acelerado |
| `libglapi.so.0.0.0` | Mesa |
| `dri/rockchip_dri.so` | driver Mesa do RK3566 |
| `libz.so.1.3.1` / `libzstd.so.1.5.5` / `libexpat.so.1.9.1` | compressão e parsing, do Mesa |

**Diferença: zero.** As 15 já estavam na imagem. A prova mais forte é o que o
pacote instalou — três arquivos, nenhuma biblioteca:

```text
membros: metadata.json → capa.png → bin/ → bin/arcade-volleyball
$ find /user/apps/arcade-volleyball -type f
  /user/apps/arcade-volleyball/metadata.json
  /user/apps/arcade-volleyball/bin/arcade-volleyball
  /user/apps/arcade-volleyball/capa.png
```

### Por que ele não pede nada, e o launcher pede quatro coisas

É decisão de projeto do port, não sorte: **o jogo não usa fonte nem áudio**. O
placar é desenhado em sete segmentos de retângulo e todo o resto são retângulos,
linhas e um círculo feito de spans. Nenhuma chamada a `SDL_ttf`, nenhuma a
`libasound`.

| App | Precisa a mais | Por quê |
| --- | --- | --- |
| launcher | `SDL2_ttf`, `freetype`, `harfbuzz`, fonte TTF | a UI é feita de texto |
| `arcade-volleyball` | **nada** | desenha o placar em barras; sem áudio |

Ou seja: a regra "não peça à imagem o que o app pode desenhar" custa um placar
menos bonito e economiza quatro dependências, uma fonte e a discussão de licença
dela. Para os jogos, é o caminho — e é replicável: `volley`, `pacman` e
`cobrinha` também não pedem nada.

### Uma correção de estado: o X55 já roda a imagem de 04/11

Este documento foi escrito supondo a imagem de 03/11 no aparelho. Medido em
06/11/2026:

```text
$ cat /proc/version
Linux version 6.12.0 ... #2 SMP PREEMPT Wed Nov  4 21:04:03 UTC 2026

$ giochi-selftest  → bloco "bibliotecas do launcher": existe
```

O kernel foi compilado em **04/11 21:04 UTC** e o `selftest` já tem o bloco que
só a imagem nova possui. As cinco bibliotecas de texto e o `DejaVuSans.ttf`
estão na imagem — **não** são mais a cópia manual que este documento registrava
como paliativo. O `readelf` continua ausente, então o `selftest` segue pulando a
prova de que o `SDL2_ttf` usa harfbuzz.

## O que NÃO precisa ser pedido

- **`libSDL2` com backend KMSDRM**: já está lá e funciona.
- **Compositor / X11 / Wayland**: não é necessário; `kmsdrm` entrega a tela direto.
- **`libSDL2_image`, `libSDL2_mixer`, `libSDL2_gfx`**: o launcher desenha com
  primitivas e ícones vetoriais. `image` não é usada; `mixer` e `gfx` idem.

## Enquanto isso: o launcher funciona sem texto

A feature `texto` do crate `ui-sdl` é opcional de propósito:

```bash
cargo build -p giochi-launcher --no-default-features   # sem SDL2_ttf
cargo build -p giochi-launcher                          # com SDL2_ttf
```

O build sem texto desenha tudo (cartões, gradientes, ícones, barras, foco) e
omite os glifos. É o que está instalado no aparelho agora. `text_size` devolve
altura aproximada para o layout não colapsar.

**Isso é paliativo, não solução.** Um launcher sem rótulo não é usável.

## Alternativa se a dependência não puder entrar na imagem

Se `SDL2_ttf` não puder ir para o rootfs, o caminho é escrever uma **fonte de
bitmap embutida** no launcher (glifos como dados, do mesmo jeito que os ícones em
`icons.rs` já são). Custo: ~800 bytes para ASCII 5×7 e nenhuma dependência nova.
Desvantagem: ampliar para os tamanhos grandes da UI (`F_HERO` = 110 px) fica
serrilhado.

É uma decisão de produto, não técnica — por isso a recomendação é pedir
`SDL2_ttf` primeiro.

## Como conferir se a dependência entrou

**Pergunte ao próprio aparelho**, sem SSH e sem o PC:

```bash
giochi-selftest
```

O bloco **bibliotecas do launcher** lista as cinco `.so` que importam
(`asound`, `freetype`, `harfbuzz`, `SDL2`, `SDL2_ttf`), conta os `.ttf` e roda
`readelf -d` no `SDL2_ttf` para provar que ele **usa** o harfbuzz.

Esse último passo é o que evita o falso positivo mais provável deste item:

> **Biblioteca presente não prova uso.** Um `libharfbuzz.so` no rootfs não diz
> nada sobre o `SDL2_ttf` — ele pode ter sido linkado sem harfbuzz e falhar só
> em tempo de execução, com a biblioteca ali do lado. `readelf -d` mostra as
> `DT_NEEDED` de verdade.

**O caminho pelo PC**, com o aparelho na rede:

```bash
bash scripts/aparelho.sh diagnostico | grep -A6 bibliotecas
```

O esperado depois da correção:

```text
  libSDL2-2.0.so.0         presente
  libSDL2_ttf-2.0.so.0     presente
  libasound.so.2           presente
  libfreetype.so.6         presente
  fonte TTF: /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

Hoje a mesma saída diz `AUSENTE` nas duas linhas e a fonte vem vazia. As três
coisas precisam existir juntas: **`libSDL2_ttf`**, **`freetype`** e **o TTF**.
Faltando qualquer uma, não há texto — `SDL2_ttf` sozinho não resolve, porque ele
carrega freetype em tempo de execução.

Conferência manual, se preferir:

```sh
ls -la /usr/lib/libSDL2_ttf*
ls -la /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

Com as duas presentes, basta recompilar **sem** `--no-default-features`:

```bash
bash scripts/aparelho.sh instalar     # detecta as libs e liga a feature `texto`
bash scripts/aparelho.sh verificar
bash scripts/aparelho.sh log | grep -i fonte
```

O log deve parar de dizer `nenhuma fonte carregada`.

> **Atenção ao comparar frames:** com texto, o build do aparelho deixa de ser
> idêntico à referência de PC sem texto. Recompile os dois no mesmo modo antes de
> concluir que o desenho regrediu.

O roteiro completo de validação de uma imagem nova (incluindo o `inittab`, que
tem prioridade sobre este item) está em [checklist-imagem.md](checklist-imagem.md).
