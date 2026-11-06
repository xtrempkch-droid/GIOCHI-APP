# Cobrinha

Jogo do console Giochi. Rust + SDL2 sobre **KMSDRM**, escala 1,0, teto de
**45 fps**, sem upscaling.

```bash
# no PC (sem tela)
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  ./target/debug/giochi-cobrinha --tamanho 640x480 --passos 60 --quadro /tmp/q.bmp

# no aparelho
bash ../../../scripts/compilar-jogo-arm64.sh --jogo cobrinha --instalar root@10.55.0.2
```

## Por que não Godot

Foi pedido "usar a plataforma Godot". **Não é possível neste aparelho, e o
motivo foi medido** (`docs/godot.md`): nenhum build oficial do Godot tem driver
KMSDRM, e o rootfs não tem X11 nem Wayland. Exportar para `linux_release.arm64`
produz um binário que tenta X11 → falha, tenta Wayland → falha, cai em
`Headless` → não desenha nada.

As três saídas, e o custo de cada uma:

| Saída | Custo | De quem é |
| --- | --- | --- |
| **Nativo Rust+SDL2** (esta) | escrito aqui, 356 KB, roda hoje | deste repositório |
| Recompilar o Godot com KMSDRM | horas de cross-compile de um motor; cada atualização repete | deste repositório, mas caro |
| X11/Wayland no rootfs | ~40 MB de imagem + compositor para manter | **Giochi System** |

"O máximo de tecnologia que o RK3326 executa" não é o motor mais pesado: é o que
sustenta 45 fps **sem perder quadro**. Foi o que se mediu.

## Arquitetura

```text
crates/core        regras puras — sem SDL, testável em milissegundos
├── jogo.rs        anel de corpo, colisão, crescimento, RNG determinístico
├── relogio.rs     passo fixo de simulação + teto de desenho + alpha
├── geometria.rs   célula inteira a partir do painel; escala 1,0
└── entrada.rs     código evdev → ação

crates/ui-sdl      a camada fina
├── main.rs        laço, CLI, gravação de quadro
├── video.rs       orientação do sistema, projeção, rotação
├── desenho.rs     paleta, fundo pré-rasterizado, dígitos 7-segmentos
└── evdev.rs       thread de /dev/input + fila SPSC com Acquire/Release
```

A divisão existe por um motivo prático: **o CI e o PC não têm SDL2 nem
aparelho**. Tirando o SDL do núcleo, colisão, crescimento, ritmo e geometria são
testáveis sem biblioteca gráfica. 27 testes no núcleo, 32 na interface.

## As quatro decisões que fazem 45 fps

### 1. Dois relógios: simulação fixa, desenho limitado

A cobra **anda em degraus** — 7 passos por segundo no início, até 16. Se a
simulação rodasse no ritmo do desenho, o jogo ficaria mais difícil num aparelho
lento e o recorde deixaria de significar algo.

O que liga os dois é o **alpha**: quanto falta para o próximo passo. A cabeça é
desenhada no meio do caminho entre duas células, então 7 passos/s aparecem lisos
a 45 quadros/s sem nenhum passo extra de simulação.

Medido no **R35S real** (RK3326), 640×480:

```text
software    {"fps":44.7,"atrasados":0,"celula":22}
opengles2   {"fps":45.3,"atrasados":0,"celula":22}
```

300 quadros seguidos: `45 fps`, sem queda. Os dois backends batem a meta —
o software serve de plano B quando o Mali não abre.

No PC a 640×480 dá `45,3 fps` com `20 passos` em 3 s, que é ~6,7/s: o passo base
de 7. **Os dois relógios são independentes de fato, não no papel.**

### 2. Sem upscaling: a célula é o pixel

Desenhar num espaço fixo e esticar é o caminho comum e o pior aqui: no Mali-G31
cada pixel é escrito duas vezes, e a célula de 26 px vira 29,25 px — com borda
serrilhada de tamanho diferente das vizinhas.

Aqui a célula vem do painel, em pixels **inteiros**, e o desenho é em coordenada
de dispositivo. Escala é sempre 1,0. Painel maior dá célula maior:

| Painel | célula | campo | escala |
| --- | --- | --- | --- |
| 640×480 (R35S/X35H) | **22 px** | 528×396 | 1,00 — **medido no R35S** |
| 800×600 | **28 px** | 672×504 | 1,00 |
| 1280×720 (X55 girado) | **34 px** | 816×612 | 1,00 |

### 3. O fundo é uma textura, não 432 retângulos

O tabuleiro são 24×18 = 432 células de xadrez. Desenhá-las por quadro seriam 19
mil `fill_rect` por segundo só para pintar o que não muda. O fundo é rasterizado
**uma vez** numa `Surface` (já girada), vira textura, e cada quadro custa **um**
`copy`. O resto são ~60 retângulos pequenos.

### 4. Sem `SDL2_ttf`

O placar é desenhado em **sete segmentos de retângulo**. Custa 30 linhas e evita
`freetype` + `harfbuzz` + arquivo de fonte. O `NEEDED` do binário aarch64 fica em
três:

```text
libSDL2-2.0.so.0
libgcc_s.so.1
libc.so.6
```

Um binário que exige biblioteca ausente **não sobe** — não é "roda sem som", é
falha de carga, e já aconteceu com `libasound` neste aparelho. Jogo que não pede
nada à imagem instala em qualquer aparelho.

### Memória: 2 MB no PC, 20 MB no aparelho

```text
PC, software       VmHWM  2,2 MB
R35S, opengles2    VmHWM 20,1 MB
```

A diferença **não é o jogo**: é o driver Mali e o Mesa carregados pelo GLES.
O `forge.json` declara `ram_min_mb: 24`, que é o pior caso medido com folga.

## Memória fraca do ARM: onde isso morde

A thread de `/dev/input` produz, o laço consome. A fila é SPSC e usa
`Acquire`/`Release` explícitos:

- `Release` no índice de escrita garante que o conteúdo do slot foi escrito
  **antes** do índice avançar;
- `Acquire` na leitura garante que o consumidor só lê o slot **depois** de ver o
  índice.

Sem isso o compilador e o Cortex-A35 reordenam, e o sintoma não é crash: é
**botão que às vezes não registra** — o pior tipo de bug para diagnosticar. Em
x86 o mesmo código passaria por acaso, porque TSO não expõe a reordenação.

## NEON: onde **não** usar

Este jogo não usa NEON, e isso é uma decisão, não um esquecimento. As operações
por quadro são dezenas de retângulos de 20 px e um blit de textura — o custo está
em chamadas de desenho e em banda de memória, não em aritmética vetorizável.
Vetorizar o desenho de 60 retângulos não pagaria o custo.

Onde NEON pagaria, e por que não existe aqui: **escalonamento de imagem** (não há,
porque não há upscaling), **mixagem de áudio** (não há som) e **partículas** (não
há). Se alguma dessas entrar, o lugar do NEON será a função de escala ou de
mistura — e será medida antes de escrita.

## O `Aborted` que não era crash

O perfil de release usa `panic = "abort"`. O Rust ignora `SIGPIPE`, então
escrever num pipe fechado vira `Err` e o `println!` entra em pânico — o processo
aborta e imprime **`Aborted`**.

Aconteceu de verdade: `giochi-cobrinha --quadro x | head -3` imprimiu `Aborted`,
e por um momento pareceu que o jogo quebrava no aparelho. Era o `head`. Agora
`restaurar_sigpipe()` devolve o comportamento Unix padrão: o processo morre em
silêncio por `SIGPIPE` (código 141), que é o que qualquer programa faz.

Regra que fica: **mensagem de erro que sugere a causa errada custa mais que
nenhuma mensagem.**

## Orientação

Lida do sistema, nunca fixa: `GIOCHI_ROTATE` → `/user/.config/giochi/rotation` →
`/sys/class/graphics/fbcon/rotate` → dedução pelo formato do painel.

`fbcon=rotate:3` é o X55 (retrato 720×1280, projeto 1280×720). `rotate:2`
(180°) é recusado com mensagem — melhor avisar que fingir que atendeu.

Medido com o painel simulado em 720×1280:

```text
cobrinha: painel 720x1280, projeto 1280x720 (Ccw90), celula 34 px, escala 1.00
campo girado: x 88..703 (w 616)  y 230..1049  ← geometria exata, centrada
```

Painel **simulado** (`--tamanho`) não obedece ao `fbcon` do PC: o `fbcon` da
máquina de teste não descreve o painel pedido, e obedecê-lo faria o teste de
720×1280 mostrar paisagem.

## Controles

| Entrada | Ação |
| --- | --- |
| direcional (544-547) | muda a direção |
| A (304) / START (315) | começa, ou recomeça |
| B (305) / SELECT (314) | sai |
| setas, Espaço, Esc, Q (PC) | o mesmo |

Três viradas ficam na fila. Apertar Cima e depois Esquerda dentro do mesmo passo
executa as duas, uma por passo — com um slot só, a segunda substituía a primeira
e o jogador batia na parede sem entender.

## Diagnóstico

```bash
giochi-cobrinha --versao
giochi-cobrinha --quadro /tmp/q.bmp            # prova que desenha, sem tela
giochi-cobrinha --tamanho 720x1280 --quadro q  # painel simulado, com giro
giochi-cobrinha --quadros 135 --json           # mede fps e quadros atrasados
GIOCHI_ROTATE=ccw giochi-cobrinha              # força orientação
SDL_RENDER_DRIVER=software giochi-cobrinha     # força backend software
```

No aparelho, `--quadro` com GLES grava BMP **preto** (o swap esvazia o buffer
antes da leitura). Para fotografar, force software — é a mesma armadilha do
launcher, documentada em `docs/ferramentas.md`. O app agora **avisa** quando
detecta o quadro preto, em vez de deixar o diagnóstico para quem olhar o BMP.

## GIOCHI_GLES

Mesma variável e mesmo significado do launcher — se um comando muda o backend de
um app, tem de mudar o do outro:

| valor | efeito |
| --- | --- |
| ausente | deixa o SDL escolher (no R35S: `opengles2`) |
| `1` / outro | pede `opengles2` |
| `0` / `false` / `off` | pede `software` |

Medido no R35S: ausente → `opengles2` 45,8 fps; `0` → `software` 44,1 fps.

## Instalado no R35S (06/11/2026)

```text
/opt/giochi/apps/cobrinha/
├── bin/giochi-cobrinha   360 KB, sha256 conferido na instalação
├── metadata.json
└── capa.png
```

O launcher do aparelho reporta `catálogo: 2 app(s)` e o ciclo completo funciona:

```text
3688 <- 3691 : ./bin/giochi-launcher --lancar cobrinha
3691 <- 3700 : /opt/giochi/apps/cobrinha/bin/giochi-cobrinha
```

O `ppid` prova: o jogo foi iniciado **pelo launcher** (`fork`+`wait`), não por
SSH.
