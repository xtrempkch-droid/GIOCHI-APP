# GIOCHI DEFENDER

Tower defense espacial 3D *low-poly*: um asteroide no centro, cubos hostis
chegando de todas as direções, e uma câmera orbital para mirar.

Roda sobre a **GIOCHI-ENGINE** (SDL2/KMSDRM) e a **GIOCHI-ENGINE-3D** (GLES 2.0
sobre EGL), ambas consumidas de um snapshot local em `vendor/`.

## Estado

**Bloco 1a concluído** — projeto compilando, arena e câmera orbital. Sem
inimigos, sem tiro, sem HUD ainda. O plano dos nove blocos está em
[`PROGRESS.md`](PROGRESS.md); ideias fora de escopo, em [`IDEAS.md`](IDEAS.md).

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
```

O BMP é de 32 bits sem compressão, escrito pelo próprio jogo: o `SDL2_image` da
imagem do aparelho é compilado sem `libpng` (AGENTS.md §24), e um codificador de
PNG para uma captura que o PC converte em um segundo seria peso sem retorno.

## Requisitos

| Item | Versão | Observação |
| --- | --- | --- |
| Rust | 1.82+ | piso da engine |
| SDL2 | 2.0.14+ | por `pkg-config` (`sdl2-system`, o padrão) |
| EGL / GLES 2.0 | — | Mesa ou o driver do SoC |
| `libSDL2_mixer` | — | a engine 2D a usa no `AudioSystem` |

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
├── gameplay/
│   ├── arena.rs         asteroide + céu
│   └── camera.rs        câmera orbital (yaw/pitch/distância)
└── utils/
    ├── math.rs          clamp, lerp, ângulos
    └── procedural_mesh.rs  icosphere distorcida, cubo, campo de estrelas
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

Validado em ARM64: **pendente** (bloco 3c).

## Testes

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```

51 testes. A geometria e a câmera são testadas sem abrir janela, e a captura
headless (`--quadro`) é o que prova o desenho.

## Licença

Proprietário — ver `metadata.json` quando o app for registrado em `apps.json`.
