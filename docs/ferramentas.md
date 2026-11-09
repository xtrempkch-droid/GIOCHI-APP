# Ferramentas

Os scripts deste repositório existem porque o aparelho é a única fonte de
verdade e testar nele à mão custa tempo. Cada um nasceu de um erro concreto —
nenhum foi escrito "por completude".

## Resumo

| Ferramenta | Roda onde | Para quê |
| --- | --- | --- |
| `scripts/aparelho.sh` | PC | operação e diagnóstico do aparelho por SSH |
| `scripts/verificar-frame.mjs` | PC | decide se o quadro renderizado é a UI ou lixo |
| `scripts/verificar-telas.mjs` | PC | as 12 telas em 3 tamanhos: cabe? ocupa quanto? |
| `scripts/verificar-mapa.mjs` | PC | um `buttons.map` contra os símbolos que o launcher entende |
| `scripts/empacotar.mjs` | PC | pasta de app → `.giochi` assinado |
| `scripts/verificar-pacote.mjs` | PC | estrutura + SHA-256 + Ed25519 de um pacote |
| `scripts/verificar-jogos.mjs` | PC | geometria dos jogos: a célula é legível? |
| `scripts/capacidades-input.mjs` | PC | quais códigos de tecla cada `/dev/input` declara |
| `scripts/compilar-launcher.sh` | PC | compila o launcher para x86-64, sem root |
| `scripts/compilar-launcher-arm64.sh` | PC | compila para aarch64 e instala no aparelho |
| `scripts/compilar-jogo-arm64.sh` | PC | idem para os jogos em Rust (`cobrinha`), reusando o cache |
| `scripts/compilar-jogo-cpp-arm64.sh` | PC | idem para o jogo em C++ (`arcade-volleyball`), montando o compilador cruzado sem root |
| `scripts/validate-app.mjs` | PC / CI | valida `apps.json`, `metadata.json`, `forge.json` |
| `scripts/migrar-do-antecessor.sh` | PC | traz as fontes do repositório antecessor |

## `aparelho.sh`

```bash
bash scripts/aparelho.sh procurar        # varre a /24 e acha o aparelho
bash scripts/aparelho.sh diagnostico     # sistema, tela, áudio, entrada, libs, processo
bash scripts/aparelho.sh instalar        # cross-compila e instala, conferindo sha256
bash scripts/aparelho.sh subir           # inicia o launcher e mostra pid + log
bash scripts/aparelho.sh parar           # encerra pelo pid
bash scripts/aparelho.sh verificar       # processo + DRM + entrada + frame comparado
bash scripts/aparelho.sh verificar-jogo cobrinha        # roda o jogo e analisa o quadro
bash scripts/aparelho.sh verificar-teste                # launcher da engine × launcher de produção
bash scripts/aparelho.sh log             # últimas 40 linhas de /tmp/giochi.log
bash scripts/aparelho.sh input 15        # eventos crus de /dev/input por 15 s
bash scripts/aparelho.sh capacidades     # códigos que cada dispositivo declara
bash scripts/aparelho.sh opcoes          # qual rota e autenticação seriam usadas
bash scripts/aparelho.sh boot-log       # salva /root/giochi-boot.log ANTES de regravar
```

Variáveis:

| Variável | Para quê |
| --- | --- |
| `GIOCHI_DEVICE` | fixa o host, pulando a descoberta |
| `GIOCHI_SUBREDE` | prefixo da varredura (padrão `192.168.1`) |
| `GIOCHI_CANDIDATOS` | endereços tentados antes da varredura (padrão `10.55.0.2`) |
| `GIOCHI_SENHA` | senha; sem ela, autenticação por chave |

### Por que cada subcomando existe

**O IP muda a cada boot.** É DHCP. `procurar` varre a `/24` em paralelo e
identifica o aparelho pelo hostname (`*giochi*`), não pelo endereço antigo nem
pelo MAC. Fixar o IP na memória sempre custa uma rodada de tentativas erradas.

**São dois caminhos físicos.** Pela rede o endereço é imprevisível; pelo cabo
`usb0` é fixo em `10.55.0.2` e autentica por **senha**. `procurar` tenta os
candidatos fixos **antes** de varrer 254 endereços — um segundo em vez de
minutos, e é a única forma de achar o aparelho quando ele está no USB, já que
esse endereço não pertence à faixa varrida.

`GIOCHI_SENHA` desliga o `BatchMode` do ssh. Sem isso a rota `usb0` nunca
funcionaria: `BatchMode=yes` manda o ssh **falhar** em vez de pedir senha. Com
`sshpass` instalado a senha é passada direto; sem ele, o ssh pede no terminal —
uma vez por comando, o que funciona mas incomoda.

`opcoes` mostra a decisão antes da tentativa. Existe porque "não conecta" tem
causas muito diferentes — chave, senha, IP mudou, `BatchMode` — e todas dão a
mesma mensagem do ssh:

```text
$ bash scripts/aparelho.sh opcoes

rota de acesso
  host:        10.55.0.2
  candidatos:  10.55.0.2

autenticação
aviso senha sem sshpass — o ssh vai PEDIR a senha em cada comando

comando que seria executado
  ssh -o StrictHostKeyChecking=no -o ConnectTimeout=8 root@10.55.0.2 uname -n
```

**O `inittab` não inicia o `giochi-init`.** A interface não sobe sozinha hoje;
`subir` faz o que o init deveria fazer. Isto é dívida do **Giochi System**, não
do app.

**"Processo vivo" não prova nada.** Um launcher pode estar rodando e não desenhar
nada. Por isso `verificar` exige descritores do DRM abertos e, mais importante,
compara o quadro do aparelho com o mesmo código rodando no PC.

**`pkill -f` mata a própria sessão SSH.** A linha de comando do shell contém o
padrão procurado, então o shell casa consigo mesmo. O script usa `pidof` e depois
`kill`.

### `boot-log`: a evidência que a regravação apaga

O mesmo cartão serve os dois lados da bancada. **Regravar apaga o
`/root/giochi-boot.log`** — e é exatamente no momento de regravar que a memória
falha, porque a vontade é "resolver logo". Um comando reduz isso a um passo:

```bash
bash scripts/aparelho.sh boot-log
# salvo em dist/boot-log-20261103-191530.txt (48 linhas)
```

Guarda com a data no nome em `dist/` (ignorado pelo git), então dois boots ruins
não se sobrescrevem. Se o arquivo não existir, isso **já é informação**: ou o init
não rodou, ou o caminho mudou — e a mensagem diz isso em vez de falhar em
silêncio.

### `verificar`, em detalhe

```bash
bash scripts/aparelho.sh verificar
```

1. confere processo, descritores de DRM e de `/dev/input`;
2. **para o launcher** — o KMSDRM é exclusivo, e sem parar o segundo processo
   recebe `kmsdrm not available` e não gera quadro nenhum;
3. captura `--dump-frame` no aparelho e traz por `scp`;
4. desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`;
5. des-rotaciona a captura conforme `/sys/class/graphics/fbcon/rotate` e compara
   pixel a pixel com a referência;
6. sobe o launcher de novo, se ele estava rodando antes — mas **quem o sobe é o
   `giochi-init`**, quando ele existe (ver abaixo).

No X55 o resultado é **98,9 %** de acerto (99,9 % no X35H e **99,2 % no R35S**);
o que difere é o relógio da barra superior, que muda entre as duas capturas.
Rodar com a rotação oposta cai para **10,9 %** — é essa diferença que separa
"desenhou certo" de "desenhou de ponta a cabeça", e foi o que resolveu o bug de
orientação.

Os quadros ficam em `dist/testes/<data>/frame.png` (ignorado pelo git), para
inspeção visual depois.

### Quem manda no launcher é o `giochi-init`

`subir`, `parar` e o passo 6 do `verificar` **perguntam antes** se o
`giochi-init` está rodando, e mudam de comportamento por causa disso:

| Sem `giochi-init` | Com `giochi-init` |
| --- | --- |
| o script sobe com `setsid nohup` | espera ele subir, e não inicia concorrente |
| `parar` para o app de verdade | avisa que o supervisor o reinicia |
| falha se o binário não abrir | se não vier em 30 s, reinicia o supervisor e espera mais 60 s |

**A segunda linha da última coluna não é zelo: é o `RESTART_MAX`.** O
`giochi-init` desiste depois de 5 quedas dentro da janela, e cada `verificar`
derruba o launcher uma vez. Seis rodadas seguidas deixam o aparelho **sem
interface**, com o supervisor vivo e dormindo. O script detecta e reinicia o
supervisor pelo `::respawn:` do inittab, que zera a cota.

A razão é medida: em 06/11/2026, no R35S, o `verificar` parava o launcher,
capturava o quadro e tentava retomar com `nohup`. O `giochi-init` reiniciava o
dele mais rápido, o nosso perdia a corrida por causa do DRM exclusivo e
imprimia

```text
Error: "kmsdrm not available"
```

— que parece falha do retomar, quando o retomar **tinha** acontecido, só que por
outra mão. A prova de quem é o pai:

```text
pid=438 ppid=159 drm=3        # 159 = /bin/sh /usr/bin/giochi-init
```

Depois da correção, o mesmo passo imprime `pid=957 ppid=159 drm=3` e o número
muda a cada rodada — que é exatamente o respawn funcionando.

O `drm=` no final não é enfeite: **processo vivo não é hardware aberto**. Se o
launcher sobe mas não consegue o DRM, ele existe e não desenha — e é isso que o
`drm=0` denuncia.

## `verificar-frame.mjs`

```bash
node scripts/verificar-frame.mjs --captura frame.bmp
node scripts/verificar-frame.mjs --captura frame.bmp --referencia ref.bmp --rotacao ccw
node scripts/verificar-frame.mjs --captura frame.bmp --png saida.png
```

Sem dependência externa: lê BMP (24 ou 32 bpp, de cima ou de baixo para cima)
com o `Buffer` do node e escreve PNG com `zlib`. Não há ImageMagick no PC nem no
aparelho — e não precisa haver.

### `--min-cores`: o piso de cores é do app, não do verificador

O limiar de cores já foi 8 fixo, e isso dava **falso positivo em jogo**. O
`arcade-volleyball` desenha exatamente as 6 cores da sua paleta (fundo, chão,
rede, dois jogadores, bola) — um quadro perfeito tem 6 cores, e a ferramenta o
reprovava dizendo "o render provavelmente parou cedo".

O mesmo número tem significado oposto nos dois casos:

| App | Cores de um quadro bom | 6 cores significa |
| --- | --- | --- |
| launcher | 350–400 (texto com antialiasing) | parou cedo |
| arcade-volleyball | 6 (retângulos, paleta fixa) | desenhou tudo |

Por isso o piso é do chamador: `--min-cores 8` para o launcher, o padrão `3`
para os jogos. Abaixo do piso continua sendo suspeito; entre o piso e 8 a
ferramenta imprime `paleta: plana` e explica, sem reprovar.

Sai com código 1 quando desconfia do quadro:

| Sintoma | Leitura |
| --- | --- |
| menos de 8 cores distintas | o render parou cedo |
| mais de 90 % de preto puro | tela apagada ou letterbox |
| menos de 90 % de acerto na comparação | rotação errada |

Quando a comparação falha com as dimensões certas, o script sugere o **outro**
sentido, porque `cw` e `ccw` diferem exatamente 180° e essa é a única
alternativa possível.

### Testar um painel que não temos em mãos

`--tamanho LxA` cria o output no tamanho pedido. Com `SDL_VIDEODRIVER=dummy`, o
resultado é o `set_logical_size` letterboxando **exatamente como faria no
aparelho** — dá para conferir uma resolução de hardware que não está aqui.

```bash
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  ./target/debug/giochi-launcher --tamanho 640x480 --dump-frame /tmp/x.bmp
node ../../scripts/verificar-frame.mjs --captura /tmp/x.bmp
```

Medido em 03/11/2026 no código real (a UI é projetada em 1280×720):

| `--tamanho` | Proporção | Preto puro | Leitura |
| --- | --- | --- | --- |
| 1280×720 | 16:9 | **0,00 %** | nativo, sem faixa |
| 640×480 | 4:3 | **25,00 %** | letterbox de 60 px por lado |
| 1024×768 | 4:3 | **25,00 %** | idem, outra escala |
| 800×480 | 5:3 | **5,83 %** | quase cheio |
| 720×1280 | 9:16 | **68,13 %** | retrato **sem** giro — não é defeito |

A coluna de preto é o que o `verificar-frame` mede. **`25 %` num painel 4:3 é o
comportamento correto, não regressão** — é a previsão que o X35H vai produzir.

A linha de 720×1280 parece errada e não é: no PC não existe
`/sys/class/graphics/fbcon/rotate`, então o app não gira. No aparelho ele gira e
o preto cai a 0 %. Isso serve de contraprova: o letterbox é consequência da
**proporção**, e o giro é consequência da **declaração do sistema** — dois
mecanismos independentes.

### A armadilha do viewport: o BMP não mostra a posição da faixa

`--dump-frame` usa `read_pixels`, que **não respeita o deslocamento do
viewport**. O SDL calcula o letterbox centrado e a tela mostra centrado, mas a
leitura devolve o conteúdo a partir de (0,0).

Consequência medida num painel 640×480 com projeto 1280×720:

```text
viewport que o SDL reporta   Rect { x: 0, y: 120, w: 1280, h: 720 }   <- centrado
faixa no BMP                 0 px no topo, 120 px na base             <- artefato
faixa na tela                60 px no topo, 60 px na base              <- correto
```

Isso já produziu um diagnóstico errado: medi "tudo embaixo" no arquivo e concluí
que o conteúdo estava pendurado no topo. O usuário olhou a tela e disse "em cima
**e** embaixo" — estava certo.

`verificar-frame.mjs` agora **mede** a faixa e avisa quando ela parece
assimétrica no arquivo, sem reprovar o quadro. O que continua válido:

| Medida | Vale? |
| --- | --- |
| percentual de preto | **sim** — o total independe do offset |
| comparação pixel a pixel com o PC | **sim** — o PC tem o mesmo artefato |
| posição da faixa (topo × base) | **não** — confira no aparelho |

Regra na mesma família de "biblioteca presente não prova uso": **o BMP prova o
conteúdo, não a posição.**

### Duas coisas que a referência do PC precisa ter

A comparação de `verificar` só vale se os dois lados desenharem **a mesma
entrada no mesmo espaço**. As duas já estiveram erradas, e nos dois casos o
launcher levou a culpa:

1. **O mesmo catálogo.** Sem isto o aparelho desenhava `/opt/giochi/apps/index.json`
   e o PC o catálogo de demonstração. Medido no X35H: 99,4% → **97,8%** sem
   nenhuma regressão de desenho, só porque a Home tinha um jogo de um lado e
   quatro do outro. `gerar_referencia` copia o `index.json` do aparelho e passa
   por `GIOCHI_CATALOGO`.

2. **O mesmo espaço — paisagem.** `comparar` des-rotaciona a captura e compara no
   **espaço de projeto**. Num painel retrato (X55: 720×1280, `rotate=3`) a
   captura é retrato e, des-rotacionada, fica 1280×720; gerar a referência no
   modo do painel dá 720×1280 e o resultado é
   `dimensões incompatíveis: captura 1280x720 vs referência 720x1280`.
   `gerar_referencia` agora troca as medidas quando o tamanho é retrato, porque o
   launcher desenha em paisagem nos dois casos (`projecao()` faz o mesmo).

Depois das duas correções o X55 fecha em **98,9%** — o que falta é o relógio da
barra superior e a bateria, que mudam entre as duas capturas.

### A armadilha do driver de vídeo

`--dump-frame` só produz conteúdo sob `SDL_VIDEODRIVER=dummy` no PC. No driver
`x11` o SDL troca o buffer ao apresentar, e a leitura de volta vem **preta** com
a UI desenhada — já aconteceu, e o quadro 100 % preto parecia bug do app. No
aparelho, `kmsdrm` com `SDL_RENDER_DRIVER=software` funciona. Com `opengles2` a
leitura também vem preta: o buffer fica na GPU.

### O backend se anuncia na abertura

`SDL_RENDER_DRIVER` **inválido cai em silêncio** para outro driver — não há erro.
Por isso o launcher imprime o que o SDL abriu de fato:

```text
renderer: software (software, vsync; textura máx 0x0)          # GIOCHI_GLES=0
renderer: opengles2 (acelerado, vsync; textura máx 8192x8192)  # ausente, ou =1
```

O `textura máx 0x0` no software é do próprio backend e não é erro. A linha vem
de `canvas.info()`, então responde "qual backend rodou", que é diferente de
"qual foi pedido" — a mesma distinção entre existir, declarar e ser usado.

**Esta tabela já esteve errada, e por dois motivos.** Medido no X55 em
06/11/2026: `GIOCHI_GLES=0` *ligava* o GLES (o código testava
`env::var(..).is_ok()`, que é presença e não valor), e sem a variável o SDL já
abria `opengles2` — então "defina `GIOCHI_GLES=1` no aparelho" não mudava nada.

Hoje:

| valor | efeito |
| --- | --- |
| ausente | deixa o SDL escolher (no X55: `opengles2`) |
| `1` / qualquer outro | pede `opengles2` |
| `0` / `false` | pede `software` |

O caso útil é o `0`: forçar software num aparelho onde o acelerado funciona, para
comparar os dois backends — que é o que `SDL_RENDER_DRIVER=software` já fazia e a
variável não.

### `verificar-teste`: a engine desenha igual no aparelho?

```bash
bash scripts/aparelho.sh verificar-teste                 # 6 telas
bash scripts/aparelho.sh verificar-teste home busca      # só as pedidas
```

Responde a pergunta do porte com a única medida que vale: **pixel a pixel**. Gera
a referência no PC com o *launcher de produção* e compara com o quadro que o
`launcher-teste` desenhou no aparelho, tela por tela.

```text
painel 640x480, 6 tela(s)
  ok   home: 100,0% igual ao launcher de produção
  ok   busca: 100,0%     ← a única tela que usa recorte
  ...
  ok   pid=1077 ppid=881 drm=6
```

Três decisões que custaram medição:

- **`GIOCHI_GLES=0`.** Com backend acelerado o `--dump-frame` sai preto: o swap
  esvazia o buffer antes da leitura. Quem mede o backend é o log, não a captura.
- **Uma sessão ssh e um kill só.** A primeira versão matava o launcher por tela;
  12 mortes em sequência fizeram o `giochi-init` desistir (`RESTART_MAX`) e o
  aparelho ficou sem interface até o `subir` reiniciar o supervisor.
- **`env A=1 B=2 cmd`.** `A=1 "B=2" cmd` não vira ambiente: o binário roda sem
  `LD_LIBRARY_PATH`, não acha `libSDL2` e não grava BMP — e o sintoma é
  `comparação ?%`, que manda procurar defeito no aparelho.

### Pacote: `empacotar.mjs` → `instalar-pacote`

```bash
node scripts/empacotar.mjs app/jogos/pacman            # → dist/pacotes/pacman.giochi
node scripts/verificar-pacote.mjs dist/pacotes/pacman.giochi
bash scripts/aparelho.sh instalar-pacote dist/pacotes/pacman.giochi
```

O pacote é **reprodutível**: dois empacotamentos do mesmo conteúdo dão o mesmo
SHA-256, porque `mtime`, `uid` e `gid` são fixos no cabeçalho ustar. Sem isso a
assinatura deixaria de identificar o conteúdo.

`instalar-pacote` verifica no PC (estrutura, hash, assinatura), envia, extrai com
`scripts/lib/extrair-pacote.sh` **no aparelho** e republica o catálogo — que é
gerado incluindo o que está instalado em `/opt/giochi/apps/`, não só o
`apps.json`. Sem essa colheita, um app vindo da loja ficaria invisível sem erro.

O extrator é um script copiado, não um `sh -c` com aspas aninhadas: a primeira
versão fazia isso e o shell comeu o `$(...)`, produzindo
`sh: "bin/giochi-pacman": unknown operand`. Aninhar três níveis de aspas para
falar com outra máquina não é difícil de escrever — é difícil de **ler quando
falha**.

### Cartão de boot: `instalar-do-cartao`

O X55 **não tem eMMC**: o sistema roda do próprio cartão SD. A partição
`GIOCHI_BOOT` (256 MiB, FAT32) é a única que um PC lê, então é por ela que se
levam jogos sem SSH.

```bash
bash scripts/aparelho.sh instalar-do-cartao
```

Descobre **todos** os cartões sem nome de dispositivo (`scripts/lib/descobrir-cartoes.sh`),
monta read-only, e instala de cada um — jogos podem estar nos dois. O disco que
sustenta `/` é ignorado inteiro, senão o app montaria o próprio rootfs como mídia.

Confere o SHA-256 de cada pacote contra o manifesto, extrai em `/user/apps/<slug>`
e põe no catálogo pelo próprio launcher (`--catalogo-adicionar`) — sem Node e sem
PC no meio.

Um pacote corrompido é **recusado** e o comando sai com 1. Ver
[`cartao-sd.md`](cartao-sd.md) para o que a verificação cobre (integridade) e o
que não cobre (assinatura — falta crate de Ed25519 offline).

### Antes do hardware: `verificar-mapa.mjs`

Um símbolo que o launcher não conhece no `buttons.map` vira **botão morto em
silêncio**. Já aconteceu duas vezes (comentário no fim da linha; `KEY_VOLUMEUP`
fora do tradutor), e as duas só apareceram com o aparelho na mão.

```bash
node scripts/verificar-mapa.mjs --mapa ../GIOCHI-SYSTEM/device/r35s/buttons.map
```

```text
  19 botão(ões), 0 entrada(s) de calibragem
  símbolos conhecidos pelo launcher: 28
  todos os símbolos são reconhecidos
```

A lista de símbolos vem do **binário** (`--simbolos`), não de uma cópia no
script: uma lista paralela envelheceria separada do `code_for`, que é o erro que
o `releaseYear` já custou. Há um teste Rust (`lista_de_simbolos_bate_com_o_
tradutor`) que impede a lista publicada de divergir do tradutor.

É o que permite verificar um aparelho **que não está na mão** — foi assim que o
mapa do R35S foi conferido. Ver [`r35s.md`](r35s.md).

### Um app por tela: `--tela` + `verificar-telas.mjs`

`--dump-frame` sozinho só fotografa a Home, porque abre um **processo novo** que
sempre desenha o estado inicial. `--tela <nome>` escolhe outra:

```bash
./giochi-launcher --tamanho 640x480 --tela busca --dump-frame busca.bmp
# nomes: home, biblioteca, apps, ajustes, busca, rapido, detalhe, energia, sono,
#        wifi, senha, bluetooth
```

`verificar-telas.mjs` percorre as doze nos tamanhos que existem de fato e mede
margem e ocupação de cada uma, reprovando quem toca a borda:

```bash
node scripts/verificar-telas.mjs          # 640x480, 800x600, 1280x720
```

É o que prova layout adaptativo sem aparelho na mão. Não substitui o olho: mede
**se cabe**, não se está legível.

#### Duas chaves que a foto de tela parada exige

```bash
--foco zona:indice          # põe o anel de foco onde o teste precisa vê-lo
--teclas right,left         # despacha uma sequência e mostra onde o foco parou
--categoria sound           # escolhe a categoria das configurações
--esperar-ms <n>            # espera antes de gravar o quadro
```

`--foco` existe porque um quadro no estado inicial **nunca** mostra o anel de
foco da tela fotografada, e foi justamente a falta de foco no painel de
configurações a queixa do usuário: sem ele, "tem anel" e "não tem anel" produzem
o mesmo BMP. Zonas: `home`, `filtros`, `biblioteca`, `apps`, `categorias`,
`conteudo`, `wifi`, `senha`, `bluetooth`, `teclado`, `resultados`, `rapido`,
`detalhe`, `energia`.

`--teclas` existe porque `--foco` posiciona o destaque mas **não mostra o
resultado de andar** — e, como a tecla é despachada de verdade pelo reducer,
também exercita os efeitos: `--teclas a` na Home **inicia o jogo**, o mesmo
caminho que o botão. Foi assim que se provou no aparelho que o cartão da Home
passou a abrir o jogo (o filho aparece com `ppid` do launcher). Uma seta que não voltava passou por 169 testes verdes: o
teste olhava o estado, não o caminho. O log diz onde o foco parou, e o quadro
mostra o anel no lugar certo:

```text
--teclas: right,left -> foco SettingsSidebar 0 | filtro Todos | categoria … | grade 2x4
```

**A sequência roda depois de um quadro desenhado, e um quadro por comando.** Não
é detalhe de implementação: o arranjo da grade (colunas e itens por página) é
publicado **pelo desenho** e fica no estado. Aplicando as teclas antes do
primeiro quadro, a navegação usaria o padrão — 4 colunas — que é justamente o
número errado que causou o bug da aba Jogos, e a ferramenta mostraria o
comportamento antigo como se fosse o atual. A linha `grade 2x4` no log existe
para que isso seja visível: se ela mostrar `4x8`, o desenho não rodou.

Duas armadilhas que já custaram tempo:

- **o estado inicial é `Screen::Boot`**, e `TabNext`/`GoTab` são ignorados nele.
  Sem `--tela home`, `--teclas r` não troca de aba e o log mostra o foco
  original — parece que a navegação está quebrada quando o problema é o
  diagnóstico. Use sempre `--tela` junto.
- **`--teclas` agora sai depois de imprimir.** Antes ficava no laço, o que no
  aparelho dependia de um `timeout` que **não existe** (`sh: timeout: not
  found`) — a ferramenta travava em vez de responder. Com `--dump-frame`,
  `--lancar` ou `--conectar`, o laço continua, porque nesses casos o resultado é
  o quadro ou o processo.

`--categoria` existe porque `--tela ajustes` só troca a aba e deixa a primeira
categoria — sem ela não há como fotografar "Som" nem exercitar uma linha
ajustável. E `--foco categorias:N` **sozinho era enganoso**: punha o destaque na
coluna sem mudar `settings_category`, então o `A` abria a linha da categoria
anterior. O diagnóstico media outro estado e chamava de ok; agora os dois andam
juntos, como no reducer.

`--esperar-ms` existe porque `--dump-frame` grava no **primeiro** quadro: uma
tela que depende de trabalho assíncrono sai antes de ter conteúdo. A lista de
Wi-Fi chega do `wpa_cli` uns 5 s depois de abrir.

#### `--rede-real`: exemplo × verdade

As telas de rede abrem com dados de exemplo, para fotografar o layout sem
hardware. Isso é bom para olhar o desenho e **péssimo** para provar que o
`wpa_cli` funciona: a foto mostraria números inventados e chamaria de sucesso.

```bash
# desenho (exemplo)
SDL_VIDEODRIVER=dummy ./giochi-launcher --tamanho 640x480 --tela wifi --dump-frame /tmp/w.bmp

# verdade (varredura de verdade; no aparelho, kmsdrm em vez de dummy)
SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher \
  --tela wifi --rede-real --esperar-ms 12000 --dump-frame /tmp/w.bmp
```

`--conectar SSID:senha` e `--parear MAC` exercitam a rede sem dedo na tela e
imprimem o veredito. Foi com `--conectar` e uma senha errada que a corrente de
comandos do `wpa_cli` foi provada, e com `--parear` num MAC inexistente que a
mensagem real do `bluetoothctl` chegou à tela. Detalhes em
[`rede.md`](rede.md).

### Lançar um app sem apertar botão: `--lancar`

O caminho "abrir display → rodar app → reabrir display" não tem como ser
exercitado de fora: `/dev/fb0` devolve lixo sob KMSDRM e `--dump-frame` abre um
processo novo. Sem uma chave, verificar o lançamento exigiria um dedo no
aparelho — e verificação que depende de dedo não entra em rodada automatizada.

```bash
# no PC, com um catálogo de mentira e um filho de mentira
GIOCHI_CATALOGO=/tmp/cat/index.json \
  SDL_VIDEODRIVER=dummy ./giochi-launcher --tamanho 640x480 \
  --lancar volley --dump-frame /tmp/depois.bmp

# no aparelho, de verdade
SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher --lancar volley --dump-frame /tmp/depois.bmp
```

Escreve em `state.lancar`, o **mesmo** campo que o botão "Jogar" preenche — então
exercita o mesmo código, não uma cópia dele. O quadro gravado é o da **volta**,
o que prova que a janela foi recriada e que o texto continua aparecendo.

`GIOCHI_CATALOGO=<arquivo>` aponta outro catálogo; existe porque no PC não há
`/opt/giochi/apps/` e testar exigiria root.

**Cuidado com `| head` no `--dump-frame`.** O `head` fecha o pipe, o `SIGPIPE`
mata o launcher antes de gravar, e o arquivo não aparece — o sintoma é "não
gerou o quadro" com o programa retornando 0. Já aconteceu ao investigar.

## `verificar-jogos.mjs`

```bash
node scripts/verificar-jogos.mjs                    # 640x480, 800x600, 1280x720
node scripts/verificar-jogos.mjs --tamanhos 800x600
node scripts/verificar-jogos.mjs --png              # uma imagem por caso
```

Irmão do `verificar-telas.mjs`, com uma diferença de fundo: aquele mede se o
desenho **cabe**; este mede se a grade **serve**.

Um jogo de grade não tem referência de pixels para comparar consigo mesma em
outro tamanho — o desenho é proporcional por construção. O que pode dar errado é
a célula ficar pequena demais para ler num painel de 3,5". Por isso a medida é
geométrica:

| Jogo | O que se mede | Piso |
| --- | --- | --- |
| `cobrinha` | passo da grade (24 colunas) | 14 px |
| `pacman` | passo do labirinto (21 colunas) | 14 px |
| `volley` | altura da raquete | 28 px |

O piso de 14 px é 25% de folga sobre o menor aparelho real (640×480 dá 18,4 px no
`cobrinha`). Abaixo disso o vão entre células, o corpo e a comida viram o mesmo
borrão.

Medido nos três tamanhos: tudo acima do piso, tudo proporcional. **600p não é um
caso especial** — é o mesmo desenho num terceiro tamanho, porque o layout é
proporcional desde `Moldura` (launcher) e desde a primeira linha (jogos).

### O `--tamanho` dos jogos

Sem ele, o `SDL_VIDEODRIVER=dummy` devolve o **desktop** (1024×768), e não havia
como pedir outro painel — testar 480p ou 600p exigia o hardware na mão:

```bash
giochi-pacman --tamanho 800x600 --quadro /tmp/p.bmp
```

Mesma flag do launcher, mesma semântica. No aparelho, sem a flag, o
`fullscreen_desktop` usa o modo do conector, que é o certo.

## `capacidades-input.mjs`

```bash
node scripts/capacidades-input.mjs                      # lê o /proc local
node scripts/capacidades-input.mjs --arquivo dump.txt   # lê um dump
node scripts/capacidades-input.mjs --codigo 116         # quem emite KEY_POWER
node scripts/capacidades-input.mjs --dispositivo gpio   # um dispositivo só
```

Lê `B: KEY=` de `/proc/bus/input/devices`. É o único caminho: `evtest --query`
devolve **0 para qualquer código** nesta imagem, então não serve para descobrir
o que um botão emite.

### O formato engana

O kernel imprime as palavras de 64 bits em ordem **decrescente** — a **última**
coluna é a palavra 0. Ler da esquerda para a direita desloca todos os códigos em
múltiplos de 64 e produz um mapa plausível e errado.

```text
B: KEY=10000000000000 0     → palavra 1, bit 52 → 64+52 = 116 = KEY_POWER
B: KEY=c000000000000 0      → palavra 1, bits 50-51 → 114 e 115 = VOLUMEDOWN/UP
```

### O que ele revelou no X55

```text
rk805 pwrkey        [event0]  116 KEY_POWER
gpio-keys-control   [event3]  304-305   BTN_SOUTH (A), BTN_EAST (B)
                              307-308   BTN_NORTH (X), BTN_WEST (Y)
                              310-318   BTN_TL … BTN_THUMBR
                              544-547   BTN_DPAD_UP/DOWN/LEFT/RIGHT
gpio-keys-vol       [event4]  114-115   KEY_VOLUMEDOWN, KEY_VOLUMEUP
```

**17 códigos** no controle — que são exatamente os 17 botões citados no cabeçalho
do `buttons.map` do sistema. Os 4 do direcional ficam na faixa `BTN_` (0x220), não
na faixa `KEY_`, e é por isso que parecem códigos sem nome.

Cruzando com o app: todos os 20 códigos declarados pelo kernel têm entrada em
`code_for` e no `buttons.map`. A cobertura está completa — e agora há como provar
isso em segundos, em vez de conferir 20 linhas à mão.

### Como usar quando um botão não reage

| `capacidades` declara? | `input` mostra ao apertar? | Conclusão |
| --- | --- | --- |
| sim | sim | kernel entrega; o problema é `buttons.map` ou `code_for` |
| sim | não | driver ou DTS não registra o evento |
| não | sim | bitmask incompleto; vale como aviso, não como veredito |
| não | não | o botão não existe neste hardware |

É essa separação que evita depurar no escuro. `KEY_POWER` e as teclas de volume
ficaram meses sem função porque a pergunta certa nunca era feita.


```bash
bash scripts/compilar-launcher.sh
```

Compila para x86-64 sem root. Extrai os `.deb` do SDL2 em `vendor/sdl2` e cria o
symlink de `libasound.so`, que o linker exige mas o pacote de runtime não traz.

## `compilar-launcher-arm64.sh`

```bash
bash scripts/compilar-launcher-arm64.sh
bash scripts/compilar-launcher-arm64.sh --instalar root@192.168.1.4
```

Cross-compila para `aarch64-unknown-linux-gnu` e copia para
`app/launcher/bin/`. Detalhes das quatro armadilhas de link em
[compilar-launcher.md](./compilar-launcher.md).

## `compilar-jogo-arm64.sh`

```bash
bash scripts/compilar-jogo-arm64.sh
bash scripts/compilar-jogo-arm64.sh --instalar 10.55.0.2
```

Mesmo alvo, mesmo binário de saída em `app/jogos/volley/bin/`. **Não redescobre nada:**
exige o cache de `~/.cache/giochi-arm64` que o script do launcher prepara e reusa
o wrapper de `rust-lld` e as bibliotecas do aparelho. Se o cache não existir, ele
para e diz qual comando rodar antes.

É a diferença entre um segundo lugar para envelhecer e um segundo consumidor do
mesmo lugar. O jogo linka só contra `libSDL2` (sem `ttf`, sem `asound`), então é
um teste mais simples do mesmo caminho.

Ele também confere `PT_INTERP` com `readelf` **antes** de instalar: sem essa
entrada o kernel mata o processo na largada e `file` ainda diz que o ELF é
válido.

## `validate-app.mjs`

```bash
node scripts/validate-app.mjs --all
node scripts/validate-app.mjs app/store
node scripts/validate-app.mjs --all --json
node tests/run-all.mjs
```

Regras de `apps.json`, `metadata.json` e `forge.json`, incluindo a recusa
mecânica de GPL declarada em `distribution: "image"`. Ver
[metadata.md](./metadata.md) e [licenciamento.md](./licenciamento.md).

## O que ainda falta

- **`aparelho.sh` depende de `ssh` e `scp` no PC.** Não há plano B.
- **A rota `usb0` funciona** (exercitada em 04/11/2026: `10.55.0.2`, senha
  `giochi-dev`). A chave pública foi instalada em `/root/.ssh/authorized_keys`,
  então `BatchMode=yes` funciona e `sshpass` deixou de ser necessário. O host key
  muda a cada regravação — `ssh-keygen -R 10.55.0.2` antes.
- **Nenhuma ferramenta cobre áudio além de listar o mixer.** Testar volume fim a
  fim ainda é manual (`alsactl` + arquivo de persistência).
- **Não há teste de toque.** Não existe `/dev/uinput` nem `evemu` no aparelho,
  então não dá para injetar evento: toque só se testa com o dedo.
- **Injetar entrada é a lacuna que mais custa.** Sem `uinput`, cada verificação
  interativa depende de alguém apertar botão. `--lancar` cobre o caminho de
  lançamento, mas **navegar** (dpad, confirmar, abrir menu) continua manual. Um
  módulo `uinput` na imagem do `Giochi System` transformaria isso em teste.
- **O bitmask `ABS=` não é decodificado.** `capacidades-input.mjs` lê só `KEY=`.
  Os eixos analógicos declaram faixa em `ABS`, e os `axis_*_min/max` do
  `buttons.map` só podem ser conferidos contra isso — hoje é olho.
- **`boot-log` não foi exercitado.** O arquivo não existe na imagem anterior, e o
  aparelho estava em uso pela equipe do System durante esta rodada — então o
  caminho "arquivo presente" nunca rodou. O caminho "arquivo ausente" é o
  esperado hoje e está implementado com mensagem própria.

## Verificar o que a imagem tem, sem o aparelho

O `rootfs.ext4` de `GIOCHI-SYSTEM/out/<aparelho>/` responde "esta ferramenta
existe na imagem?" sem root, sem montar e sem aparelho ligado:

```bash
IMG=../GIOCHI-SYSTEM/out/powkiddy-x55/rootfs.ext4
debugfs -R "ls -l /usr/bin" "$IMG" | grep -iE "sha|ssl"
debugfs -R "stat /usr/bin/sha256sum" "$IMG" | tail -1   # Fast link dest
debugfs -R "stat /usr/bin/openssl" "$IMG"               # File not found
```

Três armadilhas, todas encontradas na prática:

1. **`cat` num symlink não mostra o alvo** — o `debugfs` devolve
   `File not found by ext2_lookup`, que parece "não existe" e é o contrário. O
   alvo está na última linha do `stat`, em `Fast link dest`.
2. **`stat` de caminho inexistente imprime o inode da consulta anterior** se o
   stderr estiver suprimido: dois arquivos parecem o mesmo. Não suprima o
   stderr, ou confira o `not found`.
3. **A mesma ferramenta pode estar em `/bin` e não em `/usr/bin`** (busybox
   mora em `/bin`). Procure nos dois antes de concluir que falta.

Medido assim: `sha256sum`, `awk`, `sed`, `tar`, `head`, `cmp` e `od` existem
(todos busybox); `openssl` e `python3` **não**.

## R35S: console serial, porque não há outra porta

O **R35S não tem rádio** e o `dwc2` nasce como *host*. Não há SSH nem `usb0`,
então `aparelho.sh` — que é todo `ssh`/`scp` — **não alcança** esse aparelho. A
console serial é o único caminho.

```bash
bash scripts/console-r35s.sh "uname -r"
bash scripts/console-r35s.sh "cat /sys/class/graphics/fb0/virtual_size" "date -u"
bash scripts/console-r35s.sh --escutar 12     # só ouve: boot, panic, prompt
```

### Como funciona, e as três armadilhas

Roda dentro de um container (`--privileged -v /dev:/dev`) com a imagem do
Giochi System, por dois motivos:

1. o nó é `root:dialout`, e quem não está no grupo não abre;
2. o **CH341 cai do barramento** de vez em quando — com `-v /dev:/dev` o
   container sobrevive à queda; com `--device=/dev/ttyUSB0` fixado no `start`,
   ele aborta quando o nó some.

`--privileged` é necessário: montar `/dev` não basta, o cgroup de devices
bloqueia a abertura com "Operation not permitted".

**A terceira armadilha é a que engana:** um `docker run --rm` cujo script do host
foi morto **continua rodando e segurando a porta**. O leitor seguinte recebe
`returned no data` — a mesma mensagem de "adaptador desconectado", que manda
procurar defeito no cabo. O script encerra leitores órfãos antes de abrir.

### Por que marcador, e não `sleep` + ler

Porque o shell **ecoa** o comando. Sem marcador, a saída se mistura com o eco e
"achou" e "não achou" viram a mesma leitura. O comando é enviado entre dois
`printf` com etiqueta aleatória, e a leitura termina no marcador final.

E o marcador é montado **no aparelho**, com `%s`:

```sh
printf '\n@@%s-%s@@\n' INI <etiqueta>; <comando>; printf '@@%s-%s:%d@@\n' FIM <etiqueta> $?
```

Escrito literalmente, o marcador apareceria **no eco** antes da hora, e a
leitura pararia ali — devolvendo vazio. Custou duas tentativas.

O `$?` viaja no marcador porque o código de saída **é** o dado: texto vazio com
status 0 e com status 1 significam coisas diferentes.

### Baud

`1500000` — é o que o `device.json` do R35S declara (`console=ttyS2,1500000`) e
o que o aparelho responde. Outros aparelhos Rockchip usam o mesmo.

## `aparelho.sh verificar-jogo`

```bash
bash scripts/aparelho.sh verificar-jogo cobrinha        # roda e analisa o quadro
bash scripts/aparelho.sh verificar-jogo cobrinha 300    # 300 quadros antes de capturar
```

Prova que o jogo desenha no aparelho **sem ninguém olhar a tela**: roda com
captura de quadro, traz o BMP e analisa.

Duas coisas que ele passou a descobrir sozinho em 06/11/2026, por ter falhado
com o `cobrinha`:

- **onde o app está instalado** — `/opt/giochi/apps` (distribution `image`) ou
  `/user/apps` (distribution `store`). Fixar uma das duas fazia o app certo
  parecer ausente;
- **o nome do binário e a flag de captura** — vinham do `metadata.json` e de
  `--ajuda` do próprio binário, não de uma convenção `<pasta>/bin/<pasta>` nem
  de uma tabela de qual jogo usa `--quadro` ou `--dump-frame`.

E ele força `SDL_RENDER_DRIVER=software`: com GLES o swap esvazia o buffer e o
BMP sai preto, o que faria a função reprovar um jogo que desenha.

## Duas armadilhas do KMSDRM (relatadas pela equipe da engine)

A equipe do Giochi System documentou as duas em `KMSDRM.md` §8, e as duas já
tinham custado uma sessão aqui antes de eles escreverem. Ficam registradas do
nosso lado porque **quem opera o aparelho pelo toolkit é afetado primeiro**.

### 1. O DRM master é do launcher, e a mensagem mente

O KMSDRM é **exclusivo**: um segundo processo não abre o mesmo conector. Quando o
launcher está rodando (e ele está sempre, supervisionado pelo `giochi-init`), o
SDL/Mesa responde:

```text
kmsdrm not available
```

ou, pior:

```text
Is a DRM device that is a GPU and is being used by an X server
```

A segunda manda procurar um X server que **não existe** neste aparelho — não há
X11 nem Wayland no rootfs. O que segura o dispositivo é o launcher do próprio
console (PID do processo, não um servidor gráfico).

Quem resolve é o toolkit, matando o launcher duas vezes na mesma sessão SSH (o
`inittab` o ressuscita em ~1 s; matar uma vez só perde a corrida). O `cobrinha`
agora também **detecta** e nomeia o dono:

```text
erro: não consegui abrir o vídeo (kmsdrm not available).
  O launcher está rodando e detém o DRM master: pid 5426: /opt/giochi/apps/launcher/bin/giochi-launcher
  O KMSDRM é exclusivo — um segundo processo não abre o conector.
  "pare seu X server" não se aplica: não há X nem Wayland neste aparelho.
```

Três causas separadas na mensagem, porque exigem três ações diferentes:

| causa detectada | o que fazer |
| --- | --- |
| `/dev/dri/card0` ausente | driver/imagem — é do **Giochi System** |
| launcher rodando | liberar o console (`aparelho.sh verificar-jogo`) |
| só o `giochi-init` vivo | `RESTART_MAX` esgotado — `aparelho.sh subir` |

### 2. O supervisor desiste em silêncio e estaciona

`giochi-init` com `RESTART_MAX=5` dentro de `RESTART_WINDOW` para de reiniciar o
launcher e fica em `sleep 3600`: **vivo, para o kernel não ficar sem init, e com
a tela preta**. Só sai sinalizando `giochi-init` — o `::respawn:` do `inittab` o
reinicia com `restarts=0`.

É o que aconteceu ao liberar o console para testar: cada `verificar` derruba o
launcher uma vez, e seis rodadas seguidas deixam o aparelho sem interface. O
efeito colateral aparece no **próximo** comando, não no que causou — o sintoma é
"o launcher não volta", sem nenhum erro.

`aparelho.sh subir` e `verificar` tratam: se o launcher não aparece em 30 s,
matam o `giochi-init` e esperam mais 60 s. Medido:

```text
aviso o giochi-init (pid 4139) parou de reiniciar — provável RESTART_MAX
  reiniciando o supervisor (o inittab o respawna com a cota zerada)
ok   pid=5426 ppid=5422 drm=6
```

**Nada disso é contornável pelo app**: a cota é do supervisor. O que o app pode
fazer é não piorar — e é por isso que o `verificar-jogo` faz as duas mortes na
mesma sessão, em vez de uma por comando.

## Catálogo ausente não vira catálogo inventado

O Giochi System apontou (canal, 19:45) que `/opt/giochi/apps/index.json` **não
existe na imagem** e o selftest avisa `[WARN] sem ... — a grade ficaria vazia`.

A grade não ficava vazia: ficava **cheia de jogos que não existem**. O fallback
desenhava quatro jogos de demonstração (Arcade Volleyball, Colossal Cave,
Spacewar, Shattered Pixel Dungeon) e oito apps. Apertar A dava
`este app não está no catálogo do sistema` — exatamente o cartão que aparece e
não abre que `podar_sem_binario` existe para impedir.

A regra agora:

| situação | tela |
| --- | --- |
| catálogo válido | os apps do índice |
| catálogo ausente ou vazio | **"Nenhum jogo instalado."** + onde ele é lido |
| `--demo` | catálogo de demonstração (foto, protótipo) |

Medido: catálogo ausente → **122 cores** (só a moldura); `--demo` → **318 cores**
com os cartões. O CI ganhou um passo que reprova se o log disser `demonstração`
sem `--demo`.

Quem decide o conteúdo da imagem é o **Giochi System**: se `index.json` deve ir
junto, é lá. Do lado do app, a obrigação é não inventar.
