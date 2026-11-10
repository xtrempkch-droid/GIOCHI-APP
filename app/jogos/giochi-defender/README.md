# GIOCHI DEFENDER

Tower defense espacial 3D *low-poly*: um asteroide no centro, cubos hostis
chegando de todas as direções, e uma câmera orbital para mirar.

Roda sobre a **GIOCHI-ENGINE** (SDL2/KMSDRM) e a **GIOCHI-ENGINE-3D** (GLES 2.0
sobre EGL), ambas consumidas de um snapshot local em `vendor/`.

## Estado

**Blocos 1a, 1b, 1c, 2a e 2b fechados** — arena com iluminação direcional,
câmera orbital, inimigos voando para o centro, dano no asteroide, **tiro** com
mira no centro da tela, e **HUD**: vida, onda, pontos, `+100` flutuando no
abate, clarão de dano nas bordas e o aviso de fim de partida.

O que ainda **não** existe é som (2c) e as telas de menu/derrota como cena (3a) —
hoje o fim é um aviso sobre a cena. O plano dos nove blocos está em
[`PROGRESS.md`](PROGRESS.md); ideias fora de escopo, em
[`IDEAS.md`](IDEAS.md).

Medido no aparelho (X35H, 640×480, Mali-G52): **49,3 fps com 15 cubos**, com
3,82 ms de trabalho por quadro — o teto é o painel, não o jogo. A mira custa
**0,19 ms** e **220 px** nos três tamanhos testados; a HUD custa **0,54 ms** e
sai em **2 chamadas** de desenho.

Quadro do aparelho com a HUD em
[`docs/imagens/giochi-defender-hud-x35h.png`](../../../docs/imagens/giochi-defender-hud-x35h.png),
e sem ela em
[`docs/imagens/giochi-defender-x35h.png`](../../../docs/imagens/giochi-defender-x35h.png).

## Como rodar

```bash
cargo run --release
```

- **mouse** — orbita a câmera
- **botão esquerdo do mouse / `RB` / `A` do controle** — atira

A HUD mostra **VIDA** (cinco marcadores que se apagam), **ONDA n/3** e
**PONTOS** no topo. Cada abate nasce como `+100` no ponto do impacto, o dano no
asteroide acende as bordas de vermelho, e a partida termina com o aviso e o
placar no centro.
- **roda do mouse** — aproxima/afasta (5 a 15 unidades)
- **setas / WASD** — orbita sem mouse
- **analógico direito** — orbita no controle
- **ESC** — sai

O tiro é um raio do centro da tela na direção da câmera, e acerta o **primeiro**
cubo no caminho: um cubo atrás do asteroide não é atingido por acidente. A folga
de mira é de 10 % do cubo, para o abate não acontecer com a mira fora dele.

## Como medir sem tela

As opções existem porque "abriu a janela" e "desenhou o asteroide" são
afirmações diferentes, e num console por SSH não há como olhar a tela:

```bash
# 3 quadros, sem janela, gravando o último em BMP
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --quadro /tmp/defender.bmp

# a mesma cena de outro ângulo (dois quadros têm de DIFERIR)
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --yaw 90 --quadro /tmp/y90.bmp

# adianta a partida para capturar cubos no ar (sem esperar na tela)
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --simular 6 --quadro /tmp/cubos.bmp

# três abates, sem dedo no aparelho
SDL_VIDEODRIVER=offscreen cargo run -- --alvo 3 --atirar --frames 30

# mede o intervalo entre quadros e o trabalho de submissão, com o pool cheio
SDL_VIDEODRIVER=offscreen cargo run --release -- --inimigos 15 --benchmark 300

# outro tamanho de painel (o layout é proporcional, a mira NÃO muda de tamanho)
SDL_VIDEODRIVER=offscreen cargo run -- --tela 800x600 --quadro /tmp/q600.bmp

# os três efeitos da HUD acesos, sem precisar jogar
SDL_VIDEODRIVER=offscreen cargo run -- --demo-hud --frames 3 --quadro /tmp/hud.bmp

# a HUD de uma partida perdida, sem esperar 45 s na tela
SDL_VIDEODRIVER=offscreen cargo run -- --simular 45 --frames 2 --quadro /tmp/fim.bmp
```

`--alvo N` planta N cubos no eixo da câmera a distâncias seguras (fora da esfera
de colisão do asteroide — plantar dentro dele faria o cubo ser despawnado como
dano) e `--atirar` mantém o gatilho apertado, com o cooldown dando a cadência.
É o mesmo caminho `Partida::atirar` do laço de produção, não uma cópia.

`--sem-mira` e `--sem-hud` tiram as duas camadas do desenho, cada uma por si —
é o que permite medir o custo de cada uma (a HUD custa 0,54 ms, a mira 0,19 ms
no X35H). `--demo-hud` acende os três efeitos na abertura, porque provar que o
`+100` sobe exigiria acertar um cubo com o dedo no aparelho.

`--benchmark` imprime mediana, p50/p95/p99 e o pior quadro, além do **trabalho
de submissão** e a fração que ele ocupa do intervalo — é essa fração que diz se
o gargalo é o jogo (perto de 1) ou o painel (perto de 0). No aparelho ela é 0,13.

Custo de simulação puro, sem GL:

```bash
cargo bench --bench frame_time
```

`--semente N` troca o sorteio das posições; o padrão é fixo para a captura ser
reproduzível.

O BMP é de 32 bits sem compressão, escrito pelo próprio jogo: o `SDL2_image` da
imagem do aparelho é compilado sem `libpng` (AGENTS.md §24), e um codificador de
PNG para uma captura que o PC converte em um segundo seria peso sem retorno.

### A capa é um quadro do jogo

`capa.png` (320×320) não é arte à parte: é uma captura do próprio jogo no
instante de um abate, e o comando que a gera está abaixo. Duas execuções dão o
mesmo md5, então trocar a cena quebra a capa de forma visível em vez de silenciosa.

```bash
SDL_VIDEODRIVER=offscreen ./target/release/giochi-defender \
  --tela 320x320 --distancia 4.8 --pitch 21 --yaw 8 \
  --inimigos 15 --simular 3.0 --alvo 1 --atirar --frames 45 --quadro /tmp/capa.bmp
```

## Requisitos

| Item | Versão | Observação |
| --- | --- | --- |
| Rust | 1.82+ | piso da engine |
| SDL2 | 2.0.14+ | por `pkg-config` (`sdl2-system`, o padrão) |
| EGL / GLES 2.0 | — | Mesa ou o driver do SoC |
| `libSDL2_mixer` | — | a engine 2D a usa no `AudioSystem`; **é dependência de carga** (sem ela o processo não sobe) |

Sem `pkg-config` de SDL2, compile o SDL2 do código-fonte:

```bash
cargo build --no-default-features --features sdl2-bundled
```

## Estrutura

```text
src/
├── main.rs              host: SDL2 + EGL + laço de passo fixo
├── lib.rs               o jogo como biblioteca (testável sem janela)
├── state.rs             score, vidas, wave, pausa
├── diag.rs              argumentos de linha de comando e BMP
├── bench.rs             medição de intervalo e de trabalho por quadro
├── gameplay/
│   ├── arena.rs         asteroide + céu + luzes
│   ├── camera.rs        câmera orbital (yaw/pitch/distância)
│   ├── collision.rs     esfera × AABB e raio × AABB
│   ├── enemy.rs         o cubo hostil e a paleta
│   ├── enemy_pool.rs    pool fixo de 15
│   ├── partida.rs       simulação (spawn, dano, tiro) e o desenho dos cubos
│   └── weapon.rs        cadência, raio de tiro e escolha do alvo
├── ui/
│   ├── mod.rs           câmera da HUD, matriz de pixel e batelada por cor
│   ├── font.rs          fonte 5x7 em dados (sem arquivo, sem licença)
│   ├── hud.rs           layout em pixels e os efeitos (+100, dano, aviso)
│   └── reticle.rs       a mira: geometria do ícone e estado do flash
├── utils/
│   ├── math.rs          clamp, lerp, ângulos
│   ├── procedural_mesh.rs  icosphere distorcida, cubo, campo de estrelas
│   └── rng.rs           sorteio determinístico (céu e posições de spawn)
└── benches/frame_time.rs  custo de simulação por quadro, sem GL
vendor/                  snapshot da engine (NÃO editar)
```

## Por que a HUD não usa `SDL2_ttf`

O plano pedia o `SDL2_ttf` da engine, e ele existe nas duas imagens. Mesmo assim a
fonte é uma tabela de 5×7 bits em código:

- o `ttf::init` do SDL **não pode ser chamado duas vezes** — o launcher já pagou
  caro por isso ao voltar de um jogo;
- fonte em arquivo é mais um item no pacote, com licença para discutir;
- desenhar texto pelo caminho 2D do SDL exigiria um segundo renderer na mesma
  janela, e o ADR-037 fixa um renderer por jogo.

Cada célula acesa vira uma instância do quad unitário, então o texto inteiro sai
em uma chamada por cor. O preço é não ter acento nem minúscula — a HUD é toda em
caixa alta.

## Por que não há `Engine` no `main.rs`

`giochi_engine::Engine` é a fachada **2D**: ela cria um `RenderContext` sobre
`sdl2::render`. O ADR-037 da engine fixa que um jogo escolhe um renderer e não
troca em tempo de execução — este jogo é 3D, e as duas coisas não compartilham
janela.

O que se reusa é o resto: `Platform`, `Window` (com a flag OpenGL), `Input`,
`AudioSystem`, `GameLoop` e `FrameClock`. O laço em `main.rs` tem o mesmo desenho
do `Engine::run`: passo fixo de 1/60 s, acumulador com teto, um `render` por
quadro de tela.

## ARM64 (handheld)

```bash
bash ../../../scripts/compilar-jogo-arm64.sh --jogo giochi-defender
```

O script usa o sysroot do próprio aparelho e o wrapper de `rust-lld` do
`compilar-launcher-arm64.sh`. `Cross.toml` cobre o caminho reprodutível em CI.

Validado no **X35H** em 09/11/2026: GLES 2 sobre Mali-G52 r1, 640×480,
49,3 fps com 15 cubos, sha256 do binário conferido nas duas pontas. Pendente no
R35S, cuja imagem é outra (bloco 3c).

## Testes

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

193 testes. A geometria, a câmera, a colisão, a simulação, a arma, a mira e o
layout da HUD são testados sem abrir janela, e a captura headless (`--quadro`) é
o que prova o desenho.

Dois testes merecem nome, porque cada um pega um defeito que já aconteceu:
`um_retangulo_de_pixels_ocupa_os_mesmos_pixels` (a escala errada por um fator 2
fazia os marcadores saírem com metade do tamanho) e
`os_tres_blocos_do_topo_nao_se_sobrepoem` (vidas, onda e pontos crescendo até se
cruzarem num painel estreito — o `neon-drift` já pagou por esse).


## Licença

Proprietário — ver `metadata.json` quando o app for registrado em `apps.json`.
