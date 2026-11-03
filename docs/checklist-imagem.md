# Checklist: chegou uma imagem nova do sistema

Protocolo para quando o **Giochi System** entrega uma imagem atualizada. Não
substitui o diagnóstico — é a ordem em que as perguntas devem ser feitas, para
que um problema de imagem não seja confundido com um problema de app.

Levantado em 03/11/2026, quando a primeira imagem foi validada a campo.

## Antes de tudo: como chegar no aparelho

Há dois caminhos físicos, e eles não se comportam igual:

| Caminho | Endereço | Autenticação |
| --- | --- | --- |
| rede local (Wi-Fi) | DHCP — **muda a cada boot** | chave |
| `usb0` (cabo) | **`10.55.0.2`**, fixo | **senha** |

Pelo `usb0` não é preciso tirar o cartão para nada:

```bash
ssh root@10.55.0.2        # senha: giochi-dev
```

O toolkit já tenta o `10.55.0.2` **antes** de varrer a `/24`, e aceita senha:

```bash
GIOCHI_SENHA=giochi-dev bash scripts/aparelho.sh procurar
GIOCHI_DEVICE=10.55.0.2 GIOCHI_SENHA=giochi-dev bash scripts/aparelho.sh diagnostico
```

Sem `sshpass` instalado, o `ssh` pede a senha a cada comando — funciona, mas é
desconfortável. `bash scripts/aparelho.sh opcoes` mostra qual rota e qual
autenticação seriam usadas, antes da tentativa. Instalar `sshpass` ou copiar a
chave (`ssh-copy-id`) resolve.

> **Regravar o cartão apaga o `/root/giochi-boot.log`.** O mesmo cartão serve os
> dois lados, então ao testar o app no aparelho: **se um boot der errado, leia o
> log antes de regravar.** É o que preserva o diagnóstico — depois do `dd` a
> evidência não existe mais.

## Antes de tudo: qual é a base

`bash scripts/aparelho.sh diagnostico` imprime tudo o que interessa. Guardar a
saída num arquivo é o que permite comparar depois:

```bash
bash scripts/aparelho.sh diagnostico | tee dist/imagem-$(date +%Y%m%d).txt
```

O bloco **sistema** diz se a imagem é a esperada:

```text
host:      giochi-system-x55
kernel:    6.12.0  aarch64
distro:    Buildroot 2024.02
memória:   1967 MB
rootfs:    772.3M livres de 974.7M
```

Se o kernel, a Buildroot ou o `hostname` mudaram sem aviso, **pare**: o resto da
validação mede outra coisa.

## O `giochi-selftest` roda primeiro

A imagem atual traz um autoteste no próprio aparelho. Ele responde, sem SSH e
sem o PC, se a base está sã:

```bash
giochi-selftest
```

O bloco **bibliotecas do launcher** é o que interessa para este repositório:

| O que ele verifica | Por que importa |
| --- | --- |
| as cinco `.so`: `asound`, `freetype`, `harfbuzz`, `SDL2`, `SDL2_ttf` | são as que o launcher precisa para som, desenho e texto |
| contagem de `.ttf` | sem fonte, `SDL2_ttf` presente não desenha nada |
| `readelf -d` no `SDL2_ttf` | **prova que ele usa o harfbuzz** |

Esse último é o item mais importante do bloco, e a razão é geral:

> **Biblioteca presente não prova uso.**

Um `libharfbuzz.so` instalado no rootfs não diz nada sobre o `SDL2_ttf` — ele
pode ter sido linkado sem harfbuzz e falhar em tempo de execução, com a
biblioteca ali do lado. `readelf -d` mostra as `DT_NEEDED` de verdade.

É a mesma distinção que este projeto já aprendeu do outro lado, com outro nome:

| Pergunta | Onde se responde |
| --- | --- |
| a biblioteca existe? | `ls` / `giochi-selftest` |
| o binário **usa** a biblioteca? | `readelf -d` |
| o dispositivo **declara** o código de tecla? | `B: KEY=` no `/proc` |
| o kernel **entrega** o evento? | `aparelho.sh input` |

Nos quatro casos, **existir** e **ser usado** são fatos diferentes, e confundir
os dois custa uma sessão de diagnóstico.

## 1. A interface sobe sozinha?

**É o teste mais importante desta rodada**, porque é a diferença entre um console
e um PC com SSH.

```bash
bash scripts/aparelho.sh procurar     # o IP muda a cada boot
bash scripts/aparelho.sh diagnostico | grep -A2 "boot e init"
```

O esperado depois da correção:

```text
inittab:    giochi-init presente
```

Se aparecer `AUSENTE (a UI não sobe no boot)`, a imagem **não** trouxe a correção,
e nada mais aqui depende disso — mas vale registrar antes de seguir. Não é
motivo para parar: os itens 2 a 6 são independentes.

Se estiver presente, a validação é simplesmente **não fazer nada**:

```text
ligar o aparelho → a UI deve aparecer sem nenhum comando
```

Sem isso, suba manualmente e siga:

```bash
bash scripts/aparelho.sh subir
```

**`::respawn:` é o que importa**, não só a linha existir. Duas diferenças:

- com `once`, a interface sobe no boot mas **não volta** se o launcher terminar;
- com `respawn`, o init a reinicia.

O launcher **não** usa `exec` hoje: cria processo filho com `Command::status()` e
espera, então sobrevive ao término de um comando. Ainda assim `respawn` é o que
queremos, porque o caso que importa é o launcher **morrer** — panic, memória, erro
ao abrir o DRM — e o aparelho ficar em tela preta sem saída além de SSH.

Vale também para a etapa seguinte: `docs/integracao.md` decide que o launcher
**cederá o processo** ao jogo (`exec`, por causa do DRM master), e aí quem o
reinicia é o init. Quando o lançamento de jogos entrar, `respawn` vira obrigatório.

Teste: mate o processo com `aparelho.sh parar` e veja se ele volta sozinho em
alguns segundos.

## 2. O texto aparece?

O bloqueio número um até aqui. Dependência de imagem, não de código.

**Pergunte ao aparelho primeiro**, sem SSH e sem o PC:

```bash
giochi-selftest          # bloco "bibliotecas do launcher"
```

Ele verifica as cinco `.so`, conta os `.ttf` e — o detalhe que muda o veredito —
mostra o `readelf -d` do `SDL2_ttf`, provando que ele **usa** o harfbuzz.
Biblioteca presente não prova uso.

Do lado do PC, a mesma pergunta:

```bash
bash scripts/aparelho.sh diagnostico | grep -A6 bibliotecas
```

Esperado:

```text
libSDL2-2.0.so.0         presente
libSDL2_ttf-2.0.so.0     presente
libasound.so.2           presente
libfreetype.so.6         presente
fonte TTF: /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

Quatro coisas precisam existir juntas: **`libSDL2_ttf`**, **`freetype`**,
**`harfbuzz`** e **o TTF**. Faltando qualquer uma, não há texto — e `SDL2_ttf`
sozinho não resolve, porque carrega freetype e harfbuzz em tempo de execução.

Com as quatro presentes, **recompilar sem `--no-default-features`**:

```bash
bash scripts/aparelho.sh instalar
```

O script detecta as libs e liga a feature `texto`. Depois:

```bash
bash scripts/aparelho.sh verificar
bash scripts/aparelho.sh log | grep -i fonte
```

O log deve parar de dizer `nenhuma fonte carregada`. **E a comparação de frame
passa a ter outro resultado**: com texto, o aparelho deixa de ser 99,9 % idêntico
à referência de PC sem texto — os dois precisam ser recompilados no mesmo modo.
Se a comparação cair de repente, o motivo mais provável é essa diferença de
build, não um bug de desenho.

Ver detalhe em [dependencias-aparelho.md](dependencias-aparelho.md).

## 3. A orientação continua correta?

```bash
bash scripts/aparelho.sh verificar
```

O log de abertura diz a fonte da verdade:

```text
rotação: Ccw90 (fonte: sistema (fbcon/rotate)) [output 720x1280]
```

| O que aparece | Leitura |
| --- | --- |
| `Ccw90 (fonte: sistema (fbcon/rotate))` | **correto** — o app obedeceu ao sistema |
| `Cw90 (fonte: sistema)` | o sistema passou a declarar outro sentido: **é do sistema** |
| `fonte: usuario` | há um `/user/.config/giochi/rotation` sobrescrevendo |
| `fonte: deduzido` | o sistema **parou** de declarar; regressão de imagem |
| `180°` não suportado | alguém trocou para `fbcon=rotate:2` |

Se o sentido mudou, **não mexa no app**. Corrija o `fbcon=rotate:` na imagem e
registre — é exatamente o que `docs/contrato-app-sistema.md` proíbe de remendar.

A comparação de frame é o juiz: 99,9 % em `ccw`, 10,9 % em `cw`, falha em `none`.

## 4. Os botões continuam chegando?

```bash
bash scripts/aparelho.sh capacidades
```

Comparar com a linha de base de 03/11/2026:

```text
rk805 pwrkey        [event0]  116 KEY_POWER
gpio-keys-control   [event3]  304,305,307,308,310-318 + 544-547 BTN_DPAD_*
gpio-keys-vol       [event4]  114,115 KEY_VOLUMEDOWN/UP
```

Menos códigos que isso é regressão de device tree ou de driver. Aumentar não é
problema — mas os novos precisam entrar no `buttons.map` **do sistema**, senão o
app avisa `símbolo do mapa não reconhecido`.

Depois, exercitar de verdade (`aparelho.sh input 20` e apertar tudo):

| Botão | Esperado |
| --- | --- |
| power, toque | dorme e apaga o painel |
| power, 1 s | abre o menu de energia |
| volume + e − | muda o mixer e grava `/user/.config/giochi/volume` |
| qualquer botão dormindo | acorda, e **não** age |

## 5. Brilho, volume e estado entre boots

O sistema **não guarda** estado de ALSA, e o `inittab` (até esta rodada) não
restaura brilho. Quem persiste é o app — por contrato, não por acidente.

```bash
bash scripts/aparelho.sh log | grep -E "brilho|volume"
```

Se a imagem nova passou a restaurar brilho **também**, os dois aplicam o mesmo
valor — é idempotente e não há conflito. O que seria problema é o init restaurar
um valor **diferente** do arquivo: aí a escolha do usuário some a cada boot.

## 6. Hardware e desempenho

Ainda não cobertos por ferramenta:

- **GLES2** — nunca exercitado. Vale medir antes de prometer desempenho:
  ```bash
  ssh root@<ip> 'SDL_VIDEODRIVER=kmsdrm GIOCHI_GLES=1 \
    /opt/giochi/apps/launcher/bin/giochi-launcher'
  ```
- **FPS no RK3326** — não há contador no launcher. É a lacuna mais concreta das
  ferramentas.
- **Analógico** — os eixos existem em `adc-joystick` (mainline) ou vêm
  normalizados do `rocknix-joypad` (DTS do projeto). Qual dos dois a imagem usa
  muda a normalização por completo — ver `docs/controles.md`.

## 7. Se precisarmos buildar a imagem

Não é o nosso repositório, mas o dia em que for necessário buildar, esta
armadilha aparece — relatada pela equipe do System em 03/11/2026:

**CMake 4.x quebra o Buildroot 2024.02.**

```text
CMake Error at CMakeLists.txt:1 (CMAKE_MINIMUM_REQUIRED):
  Compatibility with CMake < 3.5 has been removed from CMake.
```

O Buildroot 2024.02 é de fevereiro de 2024 e vários pacotes declaram mínimo 3.4;
o CMake 4.0 removeu a compatibilidade. A mensagem de erro fala de CMake, não de
versão do Buildroot — e **só aparece em build do zero**: com `work/` populado
ninguém vê. Ou seja, **CI verde não garante build limpo**.

Correção (a que o próprio CMake recomenda):

```bash
export CMAKE_POLICY_VERSION_MINIMUM=3.5
```

Já está no `build-device.sh` do Giochi System, com aviso no log quando o CMake do
host é 4.x.

## 8. Segundo aparelho: o teste da arquitetura

Toda a decisão de camadas deste repositório — orientação lida do sistema,
brilho por `sysfs`, volume descoberto no mixer, sem nome de aparelho na lógica —
tem uma justificativa que **ainda não foi provada**: que o mesmo binário serve em
hardware diferente sem remenda.

O X55 foi o único aparelho até aqui. Um segundo modelo, com **resolução menor**,
é o primeiro teste de verdade. Se o app precisar de ajuste de código para rodar
nele, a arquitetura falhou em algum ponto — e o ponto é o diagnóstico, não o
remendo.

### O que o app já deveria fazer sozinho

Os números do X35H já são conhecidos pela equipe do System (03/11/2026) e estão
no `device.json` deles: **painel 640×480, rotação `0`, `adc-joystick`, 16 botões
sem `BTN_MODE`**.

| Diferença esperada | Mecanismo que responde | Se falhar |
| --- | --- | --- |
| paisagem (`rotate:0`) em vez de retrato (`3`) | `fbcon/rotate` → sem giro | app ignorou o sistema |
| outro sentido de giro | `fbcon/rotate` = `1` → `cw` | app fixou sentido |
| 640×480 em vez de 720×1280 | `set_logical_size` escala | app fixou pixel |
| **4:3 em vez de 16:9** | letterbox do SDL | — **é esperado, não é bug** |
| outro SoC, mesmo `aarch64` | binário é o mesmo (RK3566 nos dois) | depende de libc do alvo |
| 16 botões, sem `home` | `is_complete()` não cobra `home` | app exigiu botão que não existe |

O caso **4:3** é o mais provável de gerar falso alarme: a UI é projetada em
1280×720 (16:9) e num painel 640×480 sobra faixa preta em cima e embaixo. O
`verificar` vai acusar mais preto que no X55 — **e estará certo**.

```text
escala      = 640 / 1280        = 0,5
altura usada = 720 × 0,5         = 360 px
letterbox   = 480 − 360          = 120 px  →  25 % da altura (60 px por lado)
```

Se a proporção do painel bater com a do projeto, aí sim desconfie de regressão.

**Já verificado no PC, sem o aparelho**, com `--tamanho 640x480`: o frame saiu com
**25,00 %** de preto puro e a UI íntegra. A conta acima é medição, não estimativa.
O comando está em [ferramentas.md](ferramentas.md).

### Ordem de teste, quando a equipe liberar

```bash
bash scripts/aparelho.sh opcoes        # qual rota e qual autenticação
bash scripts/aparelho.sh diagnostico   # resolução, rotação, PMIC, botões
bash scripts/aparelho.sh verificar     # frame comparado com a referência do PC
bash scripts/aparelho.sh capacidades   # códigos que este aparelho declara
```

Três coisas precisam ser lidas **antes** de qualquer conclusão:

1. **`painel: connected LxA`** — a resolução real. Toda expectativa depende dela.
2. **`rotação: fbcon/rotate=N`** — a declaração do sistema para este painel.
3. **`host:`** — o hostname identifica o modelo; sem ele, o IP de outro aparelho
   na bancada produz medição do aparelho errado.

### Como registrar o resultado

O valor de testar o segundo aparelho é a **comparação**, então guarde a saída:

```bash
bash scripts/aparelho.sh diagnostico | tee dist/imagem-x55.txt     # o antigo
bash scripts/aparelho.sh diagnostico | tee dist/imagem-x35h.txt   # o novo
diff dist/imagem-x55.txt dist/imagem-x35h.txt
```

O `diff` é o que mostra a diferença que o código teve (ou não) de absorver.
Se o app subiu sem uma linha alterada, a arquitetura se paga; se exigiu mudança,
o `diff` do código mostra exatamente qual premissa estava errada.

> **Estado:** o X35H **não foi testado**. A equipe do System está validando a
> imagem e o aparelho está em uso. Nada nesta seção foi executado — é previsão,
> escrita antes do teste para poder ser falsificada.

## Resumo em uma tela

```bash
bash scripts/aparelho.sh opcoes            # rota e autenticação
bash scripts/aparelho.sh procurar          # IP muda a cada boot
bash scripts/aparelho.sh diagnostico | tee dist/imagem.txt
bash scripts/aparelho.sh verificar         # frame comparado
bash scripts/aparelho.sh capacidades       # botões declarados
bash scripts/aparelho.sh log               # orientação, brilho, volume, fonte
```

| Item | Se falhar, é do |
| --- | --- |
| UI não sobe no boot | **sistema** (`inittab`) |
| Sem texto | **sistema** (SDL2_ttf, freetype, TTF) |
| `SDL2_ttf` presente e mesmo assim sem texto | **sistema** (`readelf -d`: harfbuzz ausente) |
| Orientação errada | **sistema** (`fbcon=rotate`) |
| Código de botão ausente | **sistema** (driver/DTS) |
| Botão chega e não age | **app** (`buttons.map`, `code_for`) |
| Frame preto ou girado | **app** (desenho, rotação) |
| Faixa preta com proporção diferente | **nenhum** — letterbox esperado |
| Volume/brilho não persistem | **app** (dono do estado, por contrato) |

Essa última linha é o objetivo deste checklist: dizer de quem é o problema
**antes** de abrir o editor.
