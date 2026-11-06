# Godot no aparelho: o que foi medido, e por que o jogo não é Godot

> Medido em 06/11/2026 no X55 (RK3566, imagem de 04/11, glibc 2.38, kernel 6.12),
> com Godot **4.7.2-stable** oficial.

## Resumo

O projeto Godot do Arcade Volleyball existe, compila e passa nos próprios testes.
Ele **não roda no aparelho**, e o motivo não é empacotamento: **nenhum build
oficial do Godot tem driver de vídeo KMSDRM.** Sem X11 e sem Wayland no rootfs,
o Godot não tem onde desenhar.

O que roda hoje é o **port nativo em C++/SDL2**, que usa o mesmo SDL2/KMSDRM do
launcher. É ele que está em `bin/`.

## A medição

### 1. O aparelho não tem servidor gráfico

```text
$ ssh root@<ip> 'ls /run/wayland* /tmp/.X11-unix; command -v Xorg weston sway'
(nenhum servidor grafico rodando)
Xorg: nao    weston: nao    sway: nao

libX11:            AUSENTE
libwayland-client: AUSENTE
libwayland-server: AUSENTE
libxkbcommon:      AUSENTE

libGLESv2: libGLESv2.so     ← presentes: é o que o KMSDRM usa
libEGL:    libEGL.so
libgbm:    libgbm.so
libdrm:    libdrm.so
```

O aparelho desenha por DRM/GBM, direto no `/dev/dri/card0`. É o que o launcher
faz via `SDL_VIDEODRIVER=kmsdrm` e o que o `volley` faz.

### 2. O Godot não sabe desenhar assim

```text
$ strings Godot_v4.7.2-stable_linux.x86_64 | grep -oE 'DisplayServer[A-Za-z]+' | sort -u
DisplayServerEnums
DisplayServerHeadless
DisplayServerWayland
DisplayServerX

$ strings ... | grep -ic kmsdrm
0
```

Só `X11`, `Wayland` e `Headless`. E o **template de exportação aarch64** é o
mesmo binário com outro alvo:

```text
$ strings templates/linux_release.arm64 | grep -oE 'DisplayServer[A-Za-z]+'
DisplayServerHeadless  DisplayServerWayland  DisplayServerX
$ strings templates/linux_release.arm64 | grep -ic kmsdrm
0
```

Ou seja: exportar para aarch64 produz um executável que, no aparelho, tenta `X11`
→ falha, tenta `Wayland` → falha, cai em `Headless` → não desenha nada.

**Isto não é falta de configuração.** É uma plataforma compilada fora do build.

### 3. O projeto Godot em si está são

Os testes do próprio jogo, rodados com 4.7.2 (o projeto declara 4.3):

```text
== importacao (cache de classes) ==            ok
== invariantes da simulacao ==
  partida longa: 52 pontos, placar 7 x 0
  fuzz: 204 pontos, maior espera 292 passos (4.9 s)
TODOS OS TESTES PASSARAM
== cena principal (fumaca, 240 quadros) ==     ok
== orcamento (RAM/VRAM, tres presets) ==
  Baixo  RAM 255.2 MB  VRAM 23.79 MB   43 draws  OK
  Médio  RAM 256.5 MB  VRAM 23.81 MB   44 draws  OK
  Alto   RAM 256.6 MB  VRAM 23.82 MB   49 draws  OK
ORCAMENTO OK
```

`4.3` → `4.7.2` sem uma linha alterada, o que também resolve a dúvida de qual
versão do motor usar.

## As três saídas possíveis

| Saída | O que custa | De quem é |
| --- | --- | --- |
| **Port nativo C++/SDL2** (escolhida) | já existe, 76 KB, roda hoje | deste repositório |
| Recompilar o Godot com driver KMSDRM | horas de cross-compile de um motor inteiro; não é build oficial, então cada atualização repete o trabalho | deste repositório, mas caro |
| X11 ou Wayland no rootfs | ~40 MB de imagem, mais um compositor para manter | **Giochi System** |

A escolha foi o port nativo porque ele **já existia** no antecessor e porque o
launcher já provou que SDL2+KMSDRM funciona nos dois aparelhos. Trocar por Godot
compilado sob medida só faz sentido se o número de jogos Godot crescer ao ponto
de manter o motor valer a pena.

## O que foi medido do port nativo no aparelho

```text
arcade-volleyball: painel 720x1280, projeto 1280x720 (Ccw90), escala 1.00, renderer opengles2
arcade-volleyball: 5 dispositivo(s) de entrada abertos
arcade-volleyball: quadro 720x1280 gravado em /tmp/jogo.bmp
arcade-volleyball: 0 x 1
```

| Medida | Valor |
| --- | --- |
| painel detectado | 720x1280, rotação `Ccw90` de `fbcon/rotate=3` |
| espaço de projeto | 1280x720, escala **1,00** (nada encolhe) |
| renderer | **`opengles2`** — acelerado, não é o software |
| RAM de pico (`VmHWM`) | **19 MB** |
| comparação com o render do PC | **97,9 %** — a diferença é a bola e os jogadores, que se movem |
| geometria (chão, rede, jogador parado) | **pixels idênticos** aos do PC |
| ciclo launcher → jogo → launcher | pid do jogo com ppid = launcher; volta com 363 cores e 99,1 % de acerto |

O `forge.json` declarava `ram_min_mb: 512`. Medido: **19 MB**. Corrigido para 32.

## Três decisões que o porte exigiu

### 1. Rotação, não `SDL_RenderSetLogicalSize`

O port original usava `SDL_RenderSetLogicalSize(1280, 720)`. Ele **escala mas não
gira**: num painel retrato o jogo sairia deitado dentro de uma tela em pé. A
transformação agora é explícita (escala + rotação + centralização) em `struct
Tela`, e só dois pontos de desenho a consultam — `fill_rect` e `draw_ball`.

A matemática é a mesma de `app/jogos/volley/src/main.rs`, que veio de
`app/launcher/crates/ui-sdl/src/draw.rs`. Duplicar 40 linhas é melhor que
acoplar três apps: um não pode reclamar do outro quando quebrar.

Num painel 640x480 (X35H) a escala é 0,50 e sobra 25 % de faixa — o mesmo
percentual que o launcher já tinha documentado para 4:3.

### 2. Entrada por evdev, não por `SDL_GameController`

O port original usava a API de gamepad do SDL. No aparelho ela **não abre
controle nenhum**: os botões vêm de `gpio-keys`, que não está no
`SDL_GameControllerDB`, e `SDL_GameControllerOpen` devolve `nullptr` sem erro
visível. Resultado: o jogo instala, abre, desenha — e nenhum botão responde.

É a mesma razão pela qual o launcher usa evdev. Agora o port lê
`/dev/input/event*` (códigos 304/305/314/315/544–547) e o teclado continua
funcionando para testar no PC.

### 3. `runtime: "native"` no `metadata.json`

O campo descreve **como o app roda no aparelho**, não de que linguagem veio. O
binário é um ELF aarch64 que linka `libSDL2-2.0.so.0`; declarar `godot` seria
mentira útil só para quem lê o repositório. O projeto Godot fica em `src/` como
referência e está preservado.

## Como reconstruir

```bash
bash scripts/compilar-jogo-cpp-arm64.sh              # monta o cache e compila
bash scripts/aparelho.sh verificar-jogo arcade-volleyball
```

O Makefile em `app/jogos/arcade-volleyball/src/nativo/` é a **única** fonte das
bandeiras de compilação; o script só monta o cache cruzado e o invoca. O binário
que ele produz tem o mesmo sha256 do que rodou no aparelho:

```text
2dc8b70a5f4e4488507ad1632b9a9f47f397550ea5a2eaa0ffc3b5bd6f888920
```
