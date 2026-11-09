# Estado — GIOCHI DEFENDER

## Último bloco concluído: 2a — Arma: raycast + mira + abate
Data: 2026-11-09

Blocos fechados: **1a** (arena e câmera), **1b** (inimigos e colisão),
**1c** (polimento e performance) e **2a** (arma e mira). Dia 1 fechado, dia 2
começado.

## Gates validados

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test` — **159/159** em x86_64
- [x] `cargo build` (debug e release)
- [x] `cargo doc --no-deps`
- [x] `cargo build --release --target aarch64-unknown-linux-gnu` — **feito** com
      `scripts/compilar-jogo-arm64.sh` (PT_INTERP conferido) e **instalado no
      X35H** com sha256 idêntico nas duas pontas.
- [x] `cargo bench --bench frame_time` — custo de simulação medido sem GL.

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

### Bloco 1b

| Verificação | Resultado medido |
| --- | --- |
| Nasce um cubo a cada 2 s | `--simular 6` → 2 no ar (o terceiro nasce durante os 3 quadros da captura) |
| Os cubos aparecem na tela | 6 s: 3 manchas vermelhas com 12.019, 1.098 e 829 px; lado projetado 121/35/35 px ⇒ distâncias ~4, ~14 e ~14 unidades para um cubo de 0,8 (`499/d`) |
| O cubo que chega dói | 12 s → `player damaged` três vezes, vidas 5 → **2**, 3 no ar |
| O jogo termina no zero | 40 s → 5 danos, vidas **0**, `GAME OVER` **uma** vez, e o processo segue rodando (não derruba) |
| A simulação é determinística | mesma semente ⇒ mesmo md5 (`f28d1a27…`) em duas execuções |
| A semente manda na cena | semente 7 ⇒ md5 diferente (`e426c032…`) — o contrário do teste que passa dos dois lados |
| Um passo longo vale o mesmo | `atualizar(6,0)` × 360 × `atualizar(1/60)`: mesmas vidas, mesmas derrotas, posições dentro de 1e-3 |

Comando da captura, para repetir:

```bash
SDL_VIDEODRIVER=offscreen cargo run -- --frames 3 --quadro /tmp/defender.bmp
```

### Bloco 1c — medido no X35H (não no PC)

Aparelho: **Powkiddy X35H**, RK3566, Mali-G52 r1 (Panfrost/Mesa), painel
640×480, imagem `commit 5e81871`, kernel 6.12.0, 964 MB.
Binário instalado em `/opt/giochi/apps/giochi-defender/bin/giochi-defender`,
sha256 `5e6817eb…` **idêntico** local e remoto.

| Verificação | Resultado medido |
| --- | --- |
| O jogo abre no aparelho | `GLES 2 em 'Mali-G52 r1 (Panfrost)'`, drawable **640×480** |
| **15 cubos rodam** | **49,3 fps** de mediana (p50 20,28 ms, p95 20,39 ms, p99 20,63 ms) |
| O gargalo é o painel, não o jogo | trabalho de submissão **3,09 ms = 15 %** do intervalo. Com **0** cubos o mesmo painel dá os mesmos **49,3 fps** com 2,03 ms — ou seja, o teto é o vblank |
| Folga | 3,09 ms de 20,28 ms ⇒ **~6,5× de margem**; a exigência de 30 fps é atendida com 1,64× |
| Custo por cubo | 15 cubos custam **+1,06 ms** sobre a arena vazia (~0,07 ms por cubo) |
| GLES 3 é mais leve na CPU | 2,40 ms de trabalho (12 %) contra 3,09 ms em GLES 2 — a 49,3 fps os dois desenham o mesmo |
| A rocha mantém o facetado | **27.443 px** de rocha com **69 tons** distintos (no PC, 1280×720, 73 tons) |
| Variação de cor por cubo | 4 cubos visíveis com médias `(142,31,16)`, `(177,97,27)`, `(183,86,26)`, `(173,62,21)` — vermelho e laranja distinguíveis |
| Nada de preto puro | **0,00 %**; 84 cores distintas no quadro |
| Game over | `--simular 40` → **5** `player damaged`, vidas **0**, `GAME OVER` **uma** vez, saída 0 |

Quadros em [`docs/imagens/`](../../../docs/imagens/):
`giochi-defender-x35h.png` (o aparelho) e `giochi-defender-pc.png` (o PC).

### O que a medição obrigou a mudar no diagnóstico

`--benchmark N` **não bastava**. Ele mede o intervalo entre quadros, e no
aparelho o intervalo é 20,28 ms com 0 cubos e com 15 — a conclusão sairia
"49 fps" sem dizer de quem é o limite. Entrou a segunda série: o tempo de
**submissão**, do começo do desenho até antes de `end_frame` (a troca de buffers
é onde o KMSDRM espera o vblank, e incluí-la faria o número ser sempre igual ao
intervalo). Com as duas, `Benchmark::fracao_de_trabalho` responde a pergunta:
perto de 1 o gargalo é o jogo, perto de 0 é o painel. Aqui: **0,15**.

### Duas iterações que a imagem derrubou

1. **A luz vinha do lado errado.** A primeira versão tinha a luz viajando para
   `+z`; a câmera inicial fica em `+z`, então a face que o jogador vê era a que
   só recebia ambiente. Medido no PC: a maior área da tela ficava em
   `(38,40,44)` e a rocha lia como silhueta escura com o topo claro. Com a luz
   viajando para `-z`, a luminância média da rocha subiu de **67,1** para
   **123,6** (p10 de 39,9 para 63,9).
2. **A rampa de cor tinha duas fontes.** A primeira paleta aplicava um fator de
   brilho por cima da interpolação, e `cor_do_cubo(0.0)` deixou de ser
   `COR_VERMELHA` — a constante e a função discordavam no mesmo ponto. Agora a
   rampa é uma interpolação só, com os extremos iguais às constantes, e a
   luminância sobe de forma monotônica (é ela que separa dois cubos sobrepostos;
   só matiz não basta).

### Entregue no bloco 1c

- Iluminação direcional de verdade: rocha e cubos no shader `phong`, com duas
  luzes distintas (`arena::luz_do_asteroide`, `arena::luz_dos_inimigos`). O
  `flat` tinha a direção fixa dentro do shader; agora ela é do jogo.
- Variação de cor por cubo (`enemy::cor_do_cubo`, `COR_VERMELHA`→`COR_LARANJA`),
  fixada no nascimento e guardada no cubo.
- `Benchmark` (`src/bench.rs`) com intervalo e trabalho, p50/p95/p99 e a fração
  que diz quem é o gargalo.
- `--benchmark N`, `--inimigos N` e o sinônimo `--quadros N` no diagnóstico.
- `benches/frame_time.rs` (sem `criterion`): custo de simulação por quadro —
  **0,28 µs** com 15 cubos, 0,00 % do orçamento de 16,67 ms.
- `scripts/compilar-jogo-arm64.sh` aceita pasta que já começa com `giochi-`
  (antes produzia `giochi-giochi-defender` e o erro falava de binário ausente).

## Entregue no bloco 1b

- `src/gameplay/enemy.rs` — cubo hostil: direção ao alvo, movimento que **não
  atravessa** o alvo em passo grande, AABB exata.
- `src/gameplay/enemy_pool.rs` — 15 slots fixos reutilizáveis, `spawn` devolve
  `None` quando lota, `despawn` idempotente (o duplo desconto faria o pool
  "lotar" com slots livres).
- `src/gameplay/collision.rs` — esfera × AABB (ponto mais próximo) e raio × AABB
  (slab), com os casos que quebram na prática: encostar exato, raio paralelo a um
  eixo, raio nascendo dentro, caixa atrás da origem.
- `src/gameplay/partida.rs` — a simulação (`Partida`) separada do desenho
  (`Inimigos`): spawn a cada 2 s numa faixa de ±4 em torno do horizonte, dano ao
  encostar na esfera de 2,5, `GAME OVER` logado uma vez.
- `src/utils/rng.rs` — sorteio determinístico extraído do gerador de malha, para
  o mesmo gerador servir ao céu e às posições de spawn.
- `--simular SEGUNDOS` e `--semente N` no diagnóstico.

### Desvios do plano, e por quê

1. **`gameplay/gameplay.rs` virou `gameplay/partida.rs`.** O nome do módulo igual
   ao do pai é `clippy::module_inception` (`-D warnings`), e o tipo é `Partida` e
   não `GameplayScene` — a cena de verdade chega no 3a. O nome do arquivo passou
   a dizer o que está dentro.
2. **`atualizar(dt)` subdivide o passo em 1/60.** Sem isso, um `dt` grande move
   por inteiro até os cubos que nasceram no meio do intervalo: medido, um passo
   de 6 s tirava **4 vidas** de uma vez. Com a subdivisão, `--simular 6` mede a
   mesma partida que o laço produz.

### Três testes que a primeira versão tinha errado

Nenhum era bug do jogo; os três eram a expectativa errada — e é o que os testes
existem para descobrir.

1. `raio_que_raspa_o_canto_acerta` esperava `2·√2` e recebeu `2`. A direção
   passada não era unitária, então `t` era o parâmetro da reta, não uma
   distância. Normalizada, dá `2·√3` — e o teste passou a dizer isso.
2. `um_passo_longo_e_igual_a_muitos_curtos` comparava `Partida` por igualdade
   exata. 360 somas de `1/60` em `f32` não dão o mesmo que a subdivisão interna do
   mesmo total: agora compara vidas, derrotas e posições dentro de 1e-3 — o que
   se quer provar, não o arredondamento.
3. `o_cubo_que_chega_tira_uma_vida_e_some` exigia o pool vazio. Falso: outros
   cubos já nasceram e estão a caminho. A asserção virou "nenhum cubo colidindo
   ficou vivo", que é a regra de verdade.

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

### Bloco 2a — arma, mira e abate, medido no X35H

| Verificação | Resultado medido |
| --- | --- |
| O tiro acerta o cubo da frente | `--alvo 3 --atirar` → **3 abates**, distâncias **12,4 / 14,9 / 17,4 m** |
| O placar sobe | score 100, 200, 300 — `+100` por abate, no log e no estado |
| O cooldown dá a cadência | 0,15 s entre tiros: 3 abates em 0,30 s de simulação |
| O tiro não atravessa o asteroide | alvos plantados atrás dele não são atingidos (raycast para no primeiro obstáculo) |
| A mira é o ícone do usuário | anel duplo + 4 traços + ponto central, decodificado do `favicon` recebido |
| O tamanho da mira não depende do painel | **220 px** e extensão **±21 px** idênticas em 640×480, 800×600 e 1280×720 |
| A mira aparece no aparelho | **231 px** de mira no quadro do X35H, cor de repouso |
| O custo da mira | **0,19 ms** — trabalho de submissão 3,04 → 3,23 ms (12 % → 13 %) |
| FPS com a mira | **49,3 fps**, o mesmo de antes: o painel continua sendo o teto |
| O flash de acerto funciona | 50 ms, escala 1,35×, cor de acerto — `Mira::acertou()` no instante do abate |
| A malha da mira é fechada | todos os triângulos no mesmo sentido (teste permanente: sem isso metade some calada) |

A mira foi reconstruída uma vez durante a partida, e o log confirma que o
caminho de produção (não só o `--sem-mira`) a reconstrói:

```text
mira: malha reconstruida (220 px, extensao +-21 px)
```

### Duas armadilhas que a mira expôs

- **O shader `flat` ignora `set_light`.** Ele tem `0.35 + 0.65·lambert` fixo no
  fragmento, então a mira emissiva chegava a **54 %** do brilho pedido e ficava
  invisível (variação de +1/−4 por canal no quadro). Trocado para `Phong` com
  `ambient = 1.0` e luz preta, que é o caminho emissivo de verdade.
- **Sentido do triângulo é silencioso.** O anel era emitido
  `inner→outer→outer→inner` = horário = descartado por
  `FrontFace::CounterClockwise`. Só os 4 traços (retângulos) apareciam. Há teste
  fixando que todos os triângulos estão no mesmo sentido.

Comando da captura da capa (320×320, o instante do abate):

```bash
SDL_VIDEODRIVER=offscreen ./target/release/giochi-defender \
  --tela 320x320 --distancia 4.8 --pitch 21 --yaw 8 \
  --inimigos 15 --simular 3.0 --alvo 1 --atirar --frames 2 --quadro /tmp/capa.bmp
```

Medido: 122 cores, luminância média 70,8, **0,00 %** de quase preto, 997 px de
cubos, 220 px de mira avermelhada. Duas execuções dão o **mesmo md5** — a capa
é reproduzível.

## Próximo bloco: 2b — HUD 2D: vida, score, wave, efeitos

O jogo hoje não mostra **nada** além da cena: vida, placar e wave só existem no
log. O 2b desenha a HUD na câmera de 90° que o 2a já montou (`ui::camera_hud`),
com texto do `SDL2_ttf` da engine, e anima o `+100` de cada abate.


## Bloqueios / pendências

- **ARM64 testado** desde o 1c: o binário roda no X35H por KMSDRM e a medição
  está na seção do 1c. O que continua sem número no aparelho é só o custo de
  GPU sem vblank, e isso é limitação do driver (ver abaixo).
- **O tiro existe desde o 2a**, mas ainda não há **feedback na tela**: o abate
  aparece no log e no estado, não na HUD. É a entrega do 2b, junto com vida e
  wave.
- **`libSDL2_mixer` era o risco, e não é.** O `NEEDED` do binário lista
  `libSDL2_mixer-2.0.so.0` porque o `AudioSystem` da engine é usado, e
  biblioteca ausente é falha de **carga** (o processo não sobe), não de áudio.
  Medido no X35H: `libSDL2_mixer`, `libSDL2_image` e `libSDL2_ttf` **presentes**.
  O `SDL_mixer` avisa `MP3 support not available` e segue — degradação, não
  falha. Falta conferir no R35S, cuja imagem é outra.
- **Sem `offscreen` no aparelho.** `SDL_VIDEODRIVER=offscreen` responde
  `offscreen not available`; lá só existe KMSDRM. Consequência: não há como
  medir o custo puro de GPU sem vblank no console — é por isso que o
  `--benchmark` separa submissão de intervalo em vez de tentar enganar o vsync.
- **`--atirar` mantém o gatilho apertado.** O cooldown é que dá a cadência, então
  o diagnóstico mede o mesmo que um jogador segurando o botão; não há caminho de
  "um tiro por comando" porque o alvo é a cadência de 0,15 s.
- **A entrada local não é scriptável.** Não existe `--teclas` neste jogo (ao
  contrário do neon-drift), então a arma é exercitada por `--alvo`/`--atirar`,
  que entram pelo **mesmo** `Partida::atirar` do laço, não por uma cópia.

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

- **X35H, 15 cubos**: 49,3 fps de mediana; trabalho de submissão 3,09 ms (15 % do
  intervalo). O teto é o painel — com 0 cubos o número é o mesmo.
- **X35H, 0 cubos**: 49,3 fps, trabalho 2,03 ms (10 %).
- **X35H, 15 cubos em GLES 3**: 49,3 fps, trabalho 2,40 ms (12 %).
- **X35H, 15 cubos + mira (2a)**: 49,3 fps, trabalho 3,04 → **3,23 ms** (12 % →
  13 %). A mira custa **0,19 ms**, ou 0,9 % do intervalo.
- **PC (Radeon RX 580, 1280×720, `offscreen`)**: 15 cubos, 0,05 ms de intervalo —
  sem vblank o número é limite superior de CPU e não serve como FPS do jogo.
- **Simulação (sem GL)**: 0,28 µs por quadro com 15 cubos — 0,00 % do orçamento.
- Geração de malha, uma vez na abertura: asteroide 82,8 µs (80 triângulos), céu
  de 500 estrelas 535,1 µs (6.000 triângulos), mira ~17 µs (86 triângulos,
  reconstruída uma vez na partida).
- Chamadas de desenho por quadro: 4 da arena + até 15 dos cubos + 1 da mira =
  **20**, dentro do envelope do R5.

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
7. **A arma não conhece o pool de inimigos.** `Partida::atirar` resolve o alvo e
   entrega ao `Inimigos`; `Weapon` só mede cadência e monta o raio. Sem essa
   fronteira o `Weapon` teria de receber `&mut` do pool e os testes dele
   passariam a depender do spawn.
8. **O alvo é o mais próximo no raio, com folga de 0,10.** O raycast devolve
   todas as interseções e a folga descarta o que está muito fora do eixo — sem
   ela, um cubo a 3 m raspando a borda do raio "acertaria" e o jogador veria o
   abate sem mira em cima.
9. **A HUD usa câmera própria de 90° com planos 9..4000.** É a mesma do
   `neon-drift`, e o plano próximo 9 é essencial: com 0,5..60 a HUD cai em
   profundidade ~0,96 e fica **atrás** da cena.
10. **A mira é reconstruída uma vez, não por quadro.** A geometria depende só do
    tamanho da tela; refazer a cada quadro custaria a malha inteira por nada.

