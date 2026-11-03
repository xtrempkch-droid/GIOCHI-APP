# Checklist: chegou uma imagem nova do sistema

Protocolo para quando o **Giochi System** entrega uma imagem atualizada. Não
substitui o diagnóstico — é a ordem em que as perguntas devem ser feitas, para
que um problema de imagem não seja confundido com um problema de app.

Levantado em 03/11/2026, quando a primeira imagem foi validada a campo.

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

**`::respawn:` é o que importa**, não só a linha existir. Sem `respawn`, o
launcher não volta depois que um jogo termina — e o launcher usa `exec`, então
não sobra processo para redesenhar a interface. Teste: saia da UI (menu →
desligar, ou mate o processo com `aparelho.sh parar`) e veja se ela volta sozinha.

## 2. O texto aparece?

O bloqueio número um até aqui. Dependência de imagem, não de código.

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

Três coisas precisam existir juntas: **`libSDL2_ttf`**, **`freetype`** e **o
TTF**. Faltando qualquer uma, não há texto — e o `SDL2_ttf` sozinho não resolve,
porque ele carrega freetype em tempo de execução.

Com as três presentes, **recompilar sem `--no-default-features`**:

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

## Resumo em uma tela

```bash
bash scripts/aparelho.sh procurar          # IP muda a cada boot
bash scripts/aparelho.sh diagnostico | tee dist/imagem.txt
bash scripts/aparelho.sh verificar         # frame comparado
bash scripts/aparelho.sh capacidades       # botões declarados
bash scripts/aparelho.sh log               # orientação, brilho, volume, fonte
```

| Item | Se falhar, é do |
| --- | --- |
| UI não sobe no boot | **sistema** (`inittab`) |
| Sem texto | **sistema** (SDL2_ttf, freetype, fonte) |
| Orientação errada | **sistema** (`fbcon=rotate`) |
| Código de botão ausente | **sistema** (driver/DTS) |
| Botão chega e não age | **app** (`buttons.map`, `code_for`) |
| Frame preto ou girado | **app** (desenho, rotação) |
| Volume/brilho não persistem | **app** (dono do estado, por contrato) |

Essa última coluna é o objetivo deste checklist: dizer de quem é o problema
**antes** de abrir o editor.
