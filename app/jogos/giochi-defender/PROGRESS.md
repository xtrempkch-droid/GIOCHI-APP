# Estado — GIOCHI DEFENDER

## Último bloco concluído: 1a — Setup + janela + câmera orbital + arena
Data: 2026-11-09

## Gates validados

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test` — **51/51** em x86_64
- [x] `cargo build` (debug e release)
- [x] `cargo doc --no-deps`
- [x] `cargo check --target aarch64-unknown-linux-gnu` — **pendente**; o
      toolchain aarch64 deste repositório vem do script do launcher e é
      exercitado no bloco 3c.

## Testes manuais (o que foi validado)

Nada aqui é "deve funcionar": tudo foi executado.

| Verificação | Resultado medido |
| --- | --- |
| O jogo abre e roda | `SDL_VIDEODRIVER=offscreen`, `--frames 3`, sai com código 0 |
| Quadro gravado | BMP 1280x720, 3.686.454 bytes = 1280·720·4 + 54 |
| O asteroide está desenhado | 61.744 px de rocha no maior componente conexo |
| O tamanho projetado está certo | raio equivalente **140,2 px** contra **138,6 px** esperados para r=2,0 a d=9,0 com 60° de FOV — **+1,2 %** |
| A malha é fechada na tela | **0** pixels de fresta interna na silhueta (preenchimento a partir da borda) |
| O facetado aparece | **21** tons distintos dentro do asteroide |
| A distorção existe | raio por ângulo entre **122** e **170 px** (1,39×), dentro do intervalo teórico [108, 169] |
| O céu tem estrelas | 47 estrelas visíveis no quadro (de 500 na casca; o campo de visão de 60° cobre ~12 % da esfera) |
| O fundo é o espaço | canto do quadro = (4, 5, 8), exatamente `COR_ESPACO` |
| A câmera orbita | `--yaw 0` × `--yaw 90`: **7,6 %** dos pixels mudam |
| A câmera aproxima | `--distancia 5 --pitch -40` muda o enquadramento e o asteroide sai da sombra uniforme |
| Recursos liberados | `0 malhas e 0 shaders` na engine ao encerrar |
| A captura é reproduzível | 4 execuções (2 em release, 1 em debug, 1 com 600 quadros) dão o **mesmo md5** `94a838b7…` |

Comando da captura, para repetir:

```bash
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --quadro /tmp/defender.bmp
```

## Entregue no bloco 1a

- `Cargo.toml` com `vendor/` (snapshot da engine no commit `3785a33`) e as
  features `sdl2-system` (padrão) / `sdl2-bundled`.
- `src/main.rs` — host SDL2 + EGL + laço fixo de 1/60 s, `ESC` sai, mouse
  relativo, teclado e analógico direito.
- `src/lib.rs` — o jogo como biblioteca, o que permite testar sem janela.
- `src/state.rs` — `score`, `lives` (5), `wave`, `paused`, com as regras de
  fim de partida testadas.
- `src/gameplay/arena.rs` — asteroide (icosphere subdividida uma vez, 80
  triângulos, distorção determinística) e céu de 500 estrelas em 3 faixas.
- `src/gameplay/camera.rs` — câmera orbital com `pitch` preso em [-60°, 80°],
  distância em [5, 15] e yaw reduzido a (-π, π].
- `src/utils/procedural_mesh.rs` — icosphere, cubo, campo de estrelas e o
  sorteio determinístico; tudo com teste.
- `src/utils/math.rs`, `src/diag.rs` (CLI + BMP).
- `README.md`, `IDEAS.md`, `Cross.toml`, `.gitignore`.

## Próximo bloco: 1b — Inimigos: spawn, movimento, colisão com núcleo

Cubos hostis nascem num anel de 15–20 unidades, voam para o centro com direção
normalizada e tiram uma vida ao encostar no asteroide. Entram `enemy.rs`,
`enemy_pool.rs` (15 fixos, sem alocação por quadro) e `collision.rs` (esfera ×
AABB e raio × AABB, ambos com teste unitário). A cena passa a ter algo a
simular no passo fixo, que hoje só move a câmera.

## Bloqueios / pendências

- **ARM64 não testado.** Sem aparelho nesta rodada. O binário compila para
  x86_64 e o caminho de cross-compile está documentado; a validação é o 3c.
- **`libSDL2_mixer` é dependência de carga, não de áudio.** O `NEEDED` do
  binário lista `libSDL2_mixer-2.0.so.0` porque o `AudioSystem` da engine é
  usado. Isso é diferente de "o áudio falhou" (R6): biblioteca ausente é falha
  de carga, o processo não sobe. Na imagem do R35S de 09/11 §36 não há registro
  dessa biblioteca — **verificar no 3c**; se faltar, a saída é `dlopen` como o
  `volume.rs` do launcher faz com a `libasound`, ou remover o áudio.
- **Uma correção de manifesto no snapshot.** O `giochi-engine-3d` declara
  `giochi-engine` sem `default-features = false`, e a unificação de features
  liga o `sdl2-bundled` mesmo quando o jogo pede `sdl2-system` — o build cai no
  cmake. A correção de uma linha já existe nos snapshots do `cobrinha` e do
  `neon-drift`; está replicada e explicada em `vendor/Cargo.toml`. **Vale
  reportar à equipe da engine**, porque o feature documentado como caminho de
  cross-compile não funciona a partir de um consumidor 3D.
- **`ttf` e `image` no manifesto da engine.** São necessários para compilar a
  engine 2D (`load_texture_bytes`, `sdl2::mixer`), mas o link só registra
  `libSDL2_mixer` e `libSDL2` no `NEEDED` — `image` e `ttf` caem no
  `--as-needed`.

## Métricas de performance

- FPS com 15 cubos em x86_64: **não medido** (não há cubos no 1a; é entrega do
  1c, que também cria o `benches/frame_time.rs`).
- FPS com 15 cubos em ARM64: **pendente**.
- Custo de desenho do 1a: **4 chamadas** por quadro (3 faixas de estrelas +
  asteroide) e 19.800 vértices de céu + 240 do asteroide. Cabe folgado no
  envelope de ≤ 15 cubos e HUD leve.
- Teto superior medido no PC (Radeon RX 580, 1280x720, `offscreen`, sem vsync):
  **600 quadros em 0,27 s** — cerca de 0,1 ms de cena por quadro. É limite
  superior, não a métrica do aparelho: lá quem manda é o painel.

## Notas de arquitetura

1. **Não existe `Engine` no `main.rs`.** `giochi_engine::Engine` é a fachada
   2D e cria um `sdl2::render`; o ADR-037 fixa que um jogo escolhe um renderer.
   Foi reusado o resto (`Platform`, `Window::new_opengl`, `Input`,
   `AudioSystem`, `GameLoop`, `FrameClock`) e o laço tem o mesmo desenho do
   `Engine::run`.
2. **A malha volta como dados, não como `MeshHandle`.** `create_mesh` exige um
   contexto GL; devolver vértices e índices permite testar a geometria no CI, e
   o custo é uma linha no chamador.
3. **Normais por face, vértices duplicados.** Com normais compartilhadas a
   esfera fica lisa e o *low-poly* desaparece. A solda existe só na subdivisão,
   para a distorção não abrir fresta — medido: 0 px de fresta na silhueta.
4. **Estrelas em três malhas, não em 500 desenhos.** A cor é uniforme por
   desenho na engine atual, então três faixas de brilho dão o céu inteiro em
   três chamadas. As estrelas usam `phong` com `ambient = 1.0` e luz preta
   (emissivo): com o `flat`, o brilho dependeria da face e o céu cintilaria ao
   girar.
5. **`PITCH_MAXIMO` é 80°, não 90°.** A 90° o vetor `UP` fica paralelo à
   direção de visão e o `look_at` não tem solução; a tela ficaria preta sem
   erro. Há teste que exige matriz de visão válida nos dois limites.
6. **O campo `distance` é público e validado na leitura.** Os testes e o
   `--distancia` escrevem nele direto; `distancia_segura()` é quem garante o
   contrato.
