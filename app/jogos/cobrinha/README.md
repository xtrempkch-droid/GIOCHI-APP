# Cobrinha 3D

Jogo para os consoles Giochi, renderizado pela GIOCHI-ENGINE-3D com SDL2 e
OpenGL ES. O tabuleiro, as paredes, a cobra, a fruta e o placar são geometria
3D. A cópia da engine usada pelo jogo fica em [`vendor/`](./vendor/), isolada
do repositório compartilhado `GIOCHI-ENGINE-3D`.
Esse snapshot corresponde ao commit
`50f6857dedb8eab00f15dee5e759e530f9bf518a`; atualizações da equipe da engine não
alteram esta cópia automaticamente.

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
