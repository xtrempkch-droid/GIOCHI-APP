# Cobrinha 3D

Jogo para os consoles Giochi, renderizado pela GIOCHI-ENGINE-3D com SDL2 e
OpenGL ES. O tabuleiro, as paredes, a cobra, a fruta e o placar são geometria
3D. A cópia da engine usada pelo jogo fica em [`vendor/`](./vendor/), isolada
do repositório compartilhado `GIOCHI-ENGINE-3D`.
Esse snapshot corresponde ao commit
`50f6857dedb8eab00f15dee5e759e530f9bf518a`; atualizações da equipe da engine não
alteram esta cópia automaticamente.

**Esta cópia tem uma alteração local, e está declarada aqui para não se perder
numa resincronização:** `render3d/upscale.rs` e `render3d/renderer.rs` foram
estendidos para permitir **supersampling** (`MAX_SCALE` 1,0 → 2,0, com
`UNITY_SCALE` como identidade). O motivo e as medidas estão em
[Resolução](#resolução-800600-de-alvo-num-painel-de-640480).

O desenho tem limite rígido configurável de **1 a 75 FPS** (padrão: 75); o
núcleo também limita o ritmo a 75. A simulação usa passo fixo, independente da
frequência de desenho. O jogo solicita GLES 3.0 por padrão e tenta GLES 2.0 se
não conseguir criar o contexto. GLES 2.0 é o requisito mínimo declarado para
os aparelhos Giochi.

> **Desempenho de hardware (medido no R35S/RK3326, 09/11/2026):** GLES 3 em
> Mali-G31 (Panfrost) com instancing ativo, 640×480, **~60 FPS estáveis** em
> 600 quadros. O teto de 75 **não é atingido nem ultrapassado** nesse aparelho:
> o painel entrega 60 Hz e o `gl_swap_window` do KMSDRM espera o vblank —
> medido igual com `vblank_mode=0` (59,7) e com o renderer por software (59,2),
> ou seja, o limite é do display, não do jogo. A primeira execução depois de
> matar a interface mede ~59,4 porque os shaders ainda não estão compilados.
>
> **Medido também no X35H/RK3566 (09/11/2026, imagem `5e81871`):** GLES 3 em
> **Mali-G52 r1** com instancing ativo, 640×480, **74,2 FPS** em 600 quadros —
> praticamente no teto de 75. Aqui o display entrega mais que 60 Hz, então o
> limite deixa de ser o vblank e passa a ser o próprio laço, que é o desejado.
> Ou seja: **o teto de 75 é real e alcançável**, e não um número de fachada.
> No X55 o port **não foi medido**.

## Resolução: 800×600 de alvo num painel de 640×480

O jogo **supersampleia** por padrão: o quadro é desenhado em **800×600** e
apresentado em 640×480. Foi pedido do dono do projeto — *"é um jogo leve, acho
que por ele em 800×600 daria para manter acima dos 60 fps ou 45 pelo menos"* —
e o número caiu exatamente no fator 1,25 (640 × 1,25 = 800).

```bash
giochi-cobrinha                 # padrão: alvo 800x600, apresenta no painel
giochi-cobrinha --escala 1      # desenha na resolução da tela
giochi-cobrinha --escala 2      # 1280x960 de alvo (4x o custo de fill)
```

`--escala` aceita **0,25 a 2,0**; abaixo de 1 reduz (mais FPS), acima
supersampleia (mais nitidez). O padrão é `1.25`.

### O que mudou na engine vendorizada

`Upscaler` só sabia **reduzir**: `MAX_SCALE` era `1.0` e `set_render_scale`
filtrava por `< MAX_SCALE`. Agora são `2.0`, existe `UNITY_SCALE = 1.0` como "sem
escala" (o filtro passou a ser `!= 1.0`) e `sanitise_scale` devolve **1,0** para
valor inválido — antes devolvia `MAX_SCALE`, que com o teto novo significaria
supersampling dobrado a partir de um `NaN`.

O caminho de desenho é o **mesmo** nos dois sentidos: alvo `800x600` → passe
final com filtro linear → `640x480`. Não há um segundo caminho de desenho para
envelhecer diferente.

### Medido no X35H (640×480, Mali-G52, imagem de 09/11)

```text
--escala 1.0   49,3 fps    521 cores distintas no quadro
--escala 1.25  49,2 fps  4 604 cores distintas no quadro
```

**Custo: nenhum mensurável** (−0,2 %, dentro do ruído), e o ganho é real: os
dois quadros diferem em **13,41 %** dos pixels, e as cores distintas vão de 521
para 4 604 — é a antialiasing das bordas, que antes eram degrau puro. O mesmo
par de quadros no PC dá 517 → 4 552 cores, com os mesmos 13,46 % de pixels
diferentes: **o aparelho e o PC desenham o mesmo**.

A queda de 74,2 fps (medida em 09/11 num outro estado do painel) para 49,3 aqui
é do **display**, não do jogo: `atrasados` marca ~99 % dos quadros contra o
orçamento de 13,3 ms e as duas escalas empatam. Foi o mesmo efeito já registrado
no neon-drift (`cobrinha` e `neon-drift` medindo 49,3 no mesmo instante).

### `--quadro` lia o alvo intermediário

Com escala de render o `drawable_size()` do renderer é o **alvo** (800×600), e
`--quadro` gravava um BMP de 800×600 chamando `read_pixels` de uma janela de
640×480 — região fora da tela. Agora a captura usa o tamanho da **janela**: o
BMP sai sempre do tamanho do que é apresentado, e é isso que permite comparar
duas escalas pixel a pixel (antes a comparação virava comparação de tamanhos).

## Construir e testar

```bash
# Na raiz do repositório
bash scripts/compilar-jogo-arm64.sh --jogo cobrinha

# Testes de núcleo e interface
cd app/jogos/cobrinha
cargo test -p giochi-cobrinha-core -p giochi-cobrinha
```

O script de cross-compile resolve o SDL2 do aparelho. Não compile nem altere
arquivos no repositório separado da engine; a dependência local está em
`vendor/`.

## Diagnóstico

```bash
giochi-cobrinha --versao
giochi-cobrinha --fps-teto 75 --quadros 150 --json
giochi-cobrinha --escala 1.25 --quadro /tmp/cobrinha.bmp   # alvo 800x600
giochi-cobrinha --tamanho 640x480 --quadro /tmp/cobrinha.bmp
giochi-cobrinha --tamanho 720x1280 --quadro /tmp/cobrinha-retrato.bmp
GIOCHI_GL_VERSION=2 giochi-cobrinha
GIOCHI_ROTATE=ccw giochi-cobrinha
```

`--fps-teto` aceita apenas valores entre 1 e 75. `--quadros` termina após o
número solicitado; `--json` relata quadros, FPS médio observado, GLES ativo e
se instancing foi habilitado. `--quadro` grava o back buffer antes do swap.
`--tamanho` simula a dimensão do painel e evita usar a orientação do sistema de
teste.

## Arquitetura

```text
crates/core
├── jogo.rs       regras, colisão, crescimento e RNG determinístico
├── relogio.rs    simulação em passo fixo e limite de 75 FPS
└── entrada.rs    códigos evdev → ações

crates/ui-sdl
├── main.rs       SDL, contexto GLES, laço limitado e CLI
├── cena3d.rs     tabuleiro em static batches, meshes e instancing
├── video.rs      orientação declarada pelo sistema
└── evdev.rs      leitura em thread separada e fila SPSC

vendor
├── giochi-engine
└── giochi-engine-3d
```

O tabuleiro imutável é enviado em dois lotes estáticos; segmentos da cobra e
segmentos do placar usam instancing quando o contexto oferece suporte. Sem
instancing, o jogo desenha cada mesh individualmente. As regras e a entrada
evdev permanecem independentes da renderização.

## Controles

| Entrada | Ação |
| --- | --- |
| direcional (544–547) | muda a direção |
| A (304) / START (315) | começa ou recomeça |
| B (305) / SELECT (314) | sai |
| setas, Espaço, Esc, Q (PC) | controles equivalentes |

Até três mudanças de direção podem ficar enfileiradas para que viradas rápidas
sejam aplicadas em passos sucessivos, sem aceitar uma inversão de 180°.
