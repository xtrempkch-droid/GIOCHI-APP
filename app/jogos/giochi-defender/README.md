# GIOCHI DEFENDER

Tower defense espacial 3D *low-poly*: um asteroide no centro, cubos hostis
chegando de todas as direções, e uma câmera orbital para mirar.

Roda sobre a **GIOCHI-ENGINE** (SDL2/KMSDRM) e a **GIOCHI-ENGINE-3D** (GLES 2.0
sobre EGL), ambas consumidas de um snapshot local em `vendor/`.

## Estado

**Dia 1 fechado (blocos 1a, 1b e 1c)** — arena com iluminação direcional,
câmera orbital, inimigos voando para o centro e dano no asteroide. Ainda **sem
tiro e sem HUD**: o cubo não pode ser destruído, ele só chega e dói (o tiro é o
2a). O plano dos nove blocos está em [`PROGRESS.md`](PROGRESS.md); ideias fora de
escopo, em [`IDEAS.md`](IDEAS.md).

Medido no aparelho (X35H, 640×480, Mali-G52): **49,3 fps com 15 cubos**, com
3,09 ms de trabalho por quadro — o teto é o painel, não o jogo.

Quadro do aparelho em [`docs/imagens/giochi-defender-x35h.png`](../../../docs/imagens/giochi-defender-x35h.png).

## Como rodar

```bash
cargo run --release
```

- **mouse** — orbita a câmera
- **roda do mouse** — aproxima/afasta (5 a 15 unidades)
- **setas / WASD** — orbita sem mouse
- **analógico direito** — orbita no controle
- **ESC** — sai

## Como medir sem tela

As duas opções existem porque "abriu a janela" e "desenhou o asteroide" são
afirmações diferentes, e num console por SSH não há como olhar a tela:

```bash
# 3 quadros, sem janela, gravando o último em BMP
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --quadro /tmp/defender.bmp

# a mesma cena de outro ângulo (dois quadros têm de DIFERIR)
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --yaw 90 --quadro /tmp/y90.bmp

# adianta a partida para capturar cubos no ar (sem esperar na tela)
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --simular 6 --quadro /tmp/cubos.bmp

# mede o intervalo entre quadros e o trabalho de submissão, com o pool cheio
SDL_VIDEODRIVER=offscreen cargo run --release -- --inimigos 15 --benchmark 300
```

`--benchmark` imprime mediana, p50/p95/p99 e o pior quadro, além do **trabalho
de submissão** e a fração que ele ocupa do intervalo — é essa fração que diz se
o gargalo é o jogo (perto de 1) ou o painel (perto de 0). No aparelho ela é 0,15.

Custo de simulação puro, sem GL:

```bash
cargo bench --bench frame_time
```

`--semente N` troca o sorteio das posições; o padrão é fixo para a captura ser
reproduzível.

O BMP é de 32 bits sem compressão, escrito pelo próprio jogo: o `SDL2_image` da
imagem do aparelho é compilado sem `libpng` (AGENTS.md §24), e um codificador de
PNG para uma captura que o PC converte em um segundo seria peso sem retorno.

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
│   └── partida.rs       simulação (spawn, dano) e o desenho dos cubos
├── utils/
│   ├── math.rs          clamp, lerp, ângulos
│   ├── procedural_mesh.rs  icosphere distorcida, cubo, campo de estrelas
│   └── rng.rs           sorteio determinístico (céu e posições de spawn)
└── benches/frame_time.rs  custo de simulação por quadro, sem GL
vendor/                  snapshot da engine (NÃO editar)
```

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

118 testes. A geometria, a câmera, a colisão e a simulação são testadas sem
abrir janela, e a captura headless (`--quadro`) é o que prova o desenho.

## Licença

Proprietário — ver `metadata.json` quando o app for registrado em `apps.json`.
