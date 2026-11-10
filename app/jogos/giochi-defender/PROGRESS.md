# Estado — GIOCHI DEFENDER

## Último bloco concluído: 2c — áudio sintetizado e tremor de câmera
Data: 2026-11-10

Blocos fechados: **1a** (arena e câmera), **1b** (inimigos e colisão),
**1c** (polimento e performance), **2a** (arma e mira), **2b** (HUD) e **2c**
(áudio e resposta ao dano). **Dia 2 do plano fechado.**

## Gates validados

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets -- -D warnings`
- [x] `cargo test` — **220/220** em x86_64
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

## Desvios do plano no 2b

- **Texto por bitmap, não `SDL2_ttf`.** O plano pedia o `SDL2_ttf` da engine. Ele
  existe na imagem dos dois aparelhos, mas: (a) o `ttf::init` do SDL não pode ser
  chamado duas vezes — o launcher já pagou caro por isso (§12 do AGENTS.md); (b)
  exige uma fonte em arquivo no pacote, com a licença dela; (c) desenhar glifo
  pelo caminho 2D do SDL exigiria um segundo renderer na mesma janela, que o
  ADR-037 proíbe. A fonte de 5×7 em dados custa 300 linhas, não pede arquivo,
  não pede licença e é nítida em qualquer DPI. O custo é não ter acento nem
  minúscula — e a HUD deste jogo é toda em caixa alta.
- **O fim de jogo é um aviso sobre a cena, não uma cena.** A cena de derrota com
  "R para reiniciar" é o 3a. Aqui entra só o texto, porque sem ele o jogador
  perdia e a tela ficava igual.


## Bloco 2b — a HUD, medida no X35H

A HUD é uma fonte de 5×7 **em dados** (`ui/font.rs`) desenhada como
quadriláteros instanciados sobre a câmera de 90° do 2a. Nenhuma dependência nova:
sem `SDL2_ttf`, sem arquivo de fonte, sem licença de fonte — o mesmo motivo que
fez o `neon-drift` e o `cobrinha` desenharem o placar em pontos.

| Verificação | Resultado medido |
| --- | --- |
| A HUD aparece no aparelho | **83 cores**, 0,00 % de quase preto, texto ciano legível em 640×480 |
| Custo no X35H | 3,28 → **3,82 ms** (16 % → 19 % do intervalo): **0,54 ms** de HUD |
| FPS | **49,3**, igual a antes: o teto segue sendo o painel |
| Chamadas de desenho | **2** com a HUD normal, 4 com clarão, 7 com o fim de jogo |
| Os três blocos do topo | medidos a 640×480, 800×600 e 1280×720 sem se tocarem (folga ≥ uma letra) |
| Marcadores de vida | **14×14 px** em 640×480, um por vida, apagados conforme se perde |
| O `+100` nasce no abate | projetado com a mesma câmera do quadro; sobe e desbota em 0,9 s |
| Clarão de dano | quatro faixas de borda, afinando em 0,45 s — medido no BMP do PC |
| Fim de jogo | `FIM DE JOGO` + placar, medido num quadro de 45 s de simulação |
| Fonte completa | todo texto da HUD passa por `font::glifo` em teste: letra ausente é falha de build, não buraco na tela |

A capa continua sendo um quadro do jogo — agora **com a HUD dentro**, que é o
estado real a partir deste bloco: 133 cores, luminância média 73,7, 0,00 % de
quase preto. O comando está no README e `--frames 45` (em vez de 2) é o que
deixa o `+100` do primeiro abate expirar e mostra a cena com placar em vez de
com o texto flutuante por cima do centro.

### Três defeitos que só a medição pegou

1. **O quadrilátero saía com metade do tamanho.** `matriz_de_pixels` usava
   `PROFUNDIDADE` como escala, mas com `fov` de 90° a altura visível é
   `2·PROFUNDIDADE` — medido: marcadores pedidos com 14 px saíam com **7**, e as
   células dos glifos com metade do lado (o texto continuava legível porque as
   *posições* estavam certas, só a célula encolhia). O teste
   `um_retangulo_de_pixels_ocupa_os_mesmos_pixels` projeta os cantos do quad com
   a câmera real e exige o retângulo pedido; revertendo o fator 2, ele reprova.
2. **O aviso de fim era invisível.** O laço chamava `anunciar_fim` a cada quadro
   enquanto a partida estava acabada, e recriar o aviso zerava a idade: a
   animação de entrada nunca passava de alfa 0. O jogador perdia e a tela não
   dizia nada. Agora é idempotente, com teste que chama três vezes e exige o
   texto opaco.
3. **Escala pela altura quebrava o retrato.** A 720×1280 a letra ia a 37 px e o
   bloco das vidas terminava em x=448 enquanto o `ONDA` começava em 237. A HUD
   passou a escalar pelo **menor lado**; o teste de sobreposição roda nos quatro
   tamanhos e exige folga de uma letra, não só ausência de cruzamento.

### O que a HUD não faz (de propósito)

- **Não guarda placar nem vidas.** Os números vêm do `GameState` a cada quadro.
  Duplicá-los criaria dois lugares para envelhecer diferente.
- **Não desenha glifo por chamada.** Cada célula acesa é uma instância; a HUD
  inteira sai em 2 chamadas porque a cor é uniforme por desenho e o alfa é
  quantizado em 8 degraus.
- **Não inventa sons nem números flutuantes de dano.** Áudio é o 2c.

## Bloco 2c — som sintetizado e tremor, medidos no X35H

Cinco efeitos gerados em código (`src/audio.rs`): tiro, abate, dano, onda e fim.
Nenhum `.wav` no pacote, nenhuma biblioteca nova no `NEEDED`.

| Verificação | Resultado medido |
| --- | --- |
| Os cinco efeitos carregam no aparelho | `som: 5 efeito(s) carregado(s) de código` |
| Os cinco tocam | `--testar-som` → **5 efeito(s) tocado(s)**, `play` sem erro |
| O caminho até o alto-falante está vivo | `amixer -c 0 sget Master` = **255/255 (100 %, 0,00 dB)**, `Internal Speakers` = **[on]**, codec `rk817` em `card 0` |
| Custo por quadro | 4,52 → **4,58 ms** (22 % → 23 % do intervalo): **0,06 ms**, dentro do ruído |
| FPS | **49,3**, igual: o teto segue sendo o painel |
| Tremor medido em pixel | **10,3 px** de deslocamento do quadro em 720p, 2,8 % dos pixels mudando |
| Tremor não acumula | olho e alvo transladam juntos; a direção de visão não muda (teste) |
| Tremor não quebra o limite de pitch | matriz de visão válida nos dois extremos, com tremor de ±0,22 (teste) |

### O que o som tem de diferente dos outros assets

`Mix_QuickLoad_RAW` exige as amostras **no formato que o mixer abriu** —
`AUDIO_S16LSB`, 2 canais, 44 100 Hz, que é o que a engine pede. Não há conversão
depois, então o gerador produz estéreo entrelaçado e o teste
`o_formato_bate_com_o_que_o_mixer_abriu` amarra as três constantes.

`AudioSystem::load_sound` só aceita **caminho de arquivo**. Para tocar bytes em
memória as saídas eram gravar um temporário em disco ou falar com o
`sdl2::mixer` direto; a segunda não abre um segundo contexto, porque a engine
**leaka** o `Sdl2MixerContext` de propósito. **É uma lacuna da engine, e vale
reportar:** falta um `load_sound_from_bytes`. O `Cargo.toml` já depende do
`sdl2` direto pelo mesmo motivo no mouse relativo.

### O defeito que o `--simular` revelou: o mesmo evento em dois caminhos

A primeira versão pôs as três consequências do dano — HUD, som e tremor — dentro
do laço, e o `--simular` ficou com uma cópia só do clarão. Medido no aparelho:

```text
--simular 12  → 3 danos, "player damaged" três vezes, "som: 0 efeito(s)"
```

O diagnóstico mostrava o dano e o jogo ficava em silêncio. É a mesma família do
`A — Jogar` do launcher: **a consequência estava escrita em dois lugares**, e um
deles envelheceu. Agora `reagir_ao_dano` e `reagir_ao_abate` são as únicas portas,
e os dois caminhos chamam a mesma função. Medido depois: `--simular 12` →
**3 efeito(s) tocado(s)**.

### O tremor: a primeira versão não aparecia

Ele gemia `yaw`/`pitch`. Como a câmera orbita o alvo, girar em volta dele
**mantém o asteroide centrado** e o quadro quase não muda — medido: o centroide
da rocha andou **1,14 px** em 720p com 1,2° de tremor.

Trocado por translação do olho **e** do alvo juntos: o quadro inteiro desliza, a
direção de visão não muda e o efeito mede **10,3 px**, nove vezes mais. A
amplitude passou a ser em unidades de mundo (0,22), e o teste
`a_amplitude_virá_poucos_pixels_na_tela` converte para pixel nos dois painéis
para o número não voltar a ser arbitrário.

### Volume do sistema: acompanhava, mas as teclas não chegavam

Pergunta do dono: *"som funcionando mas não sei se o volume está acompanhando o
do sistema"*. **Acompanha, e sempre acompanhou** — o jogo nunca chamou
`AudioSystem::set_volume`, então o mixer do SDL fica no máximo e quem escala a
saída é o `Master` do ALSA, que **é** o volume do sistema.

O que faltava era outra coisa, e é a que se sente jogando: **as teclas de volume
não faziam nada durante a partida.** Quem as lê é o launcher, e ele fica em
`fork`+`wait` enquanto o jogo roda — bloqueado, sem bombear eventos. O jogador
não conseguia abaixar o som justamente quando estava jogando.

Agora o jogo lê `SDLK_VOLUMEUP`/`VOLUMEDOWN` e mexe no **mesmo** `Master`, com
passo de 5 % e persistência em `/user/.config/giochi/volume` — o mesmo arquivo
que o launcher restaura no boot. Não há dois volumes para divergir.

| Verificação | Resultado medido |
| --- | --- |
| O jogo mexe no Master de verdade | `--volume 40` → ALSA `102 [40%] [-57.00 dB]`; `--volume 100` → `255 [100%] [0,00 dB]` |
| Persiste para o próximo boot | `/user/.config/giochi/volume` = `40` depois do comando |
| Os limites saturam | de 98 % sobe para 100 %, de 3 % desce para 0 % (teste) |
| Mixer que recusa não mente | o nível local fica no anterior e o controle desliga, com **um** aviso (teste) |
| As constantes são as do SDL | teste compara com `sdl2::keyboard::Keycode::VOLUMEUP` |

Duas medições que **falharam** e ficam registradas:

- **No aparelho não há como pressionar a tecla.** O X35H não tem `/dev/uinput`
  e não tem `python3`, então não há como sintetizar entrada. O caminho do volume
  está medido ponta a ponta pelo `--volume`, que usa a **mesma** `Volume`; o que
  falta provar é só "o SDL entrega `SDLK_VOLUMEUP` quando o botão é apertado", e
  isso é do SDL.
- **No PC o teste injetou a tecla e quem respondeu foi o desktop.** Com
  `/dev/uinput` disponível, injetei `KEY_VOLUMEUP` três vezes: o `Master` do PC
  foi de 57 % para **75 %** (o daemon do ambiente subiu o volume), e o jogo não
  registrou nada — a janela não tinha foco, sem gerenciador de janelas. A
  injeção funcionou; o destinatário era outro.

### Por que `amixer` e não o FFI do launcher

O launcher fala com a `libasound` por `dlopen` (643 linhas de FFI manual). Copiar
aquilo para cá traria o mesmo poder e o mesmo custo de manutenção para **uma**
função: subir e descer 5 %. O `amixer` existe na imagem, é chamado no **evento**
(não por quadro) e a lógica de passo/limite/persistência fica atrás de um trait,
testável sem placa de som.

Limite declarado: onde não há `amixer` — o R35S não tem `libasound` e portanto
não tem `amixer` — o controle degrada para "indisponível", avisa **uma vez** e o
jogo segue. O som continua seguindo o volume do sistema; o que não funciona ali é
a tecla dentro do jogo.

## Próximo bloco: 3a — telas de menu, vitória e derrota

Hoje o fim de partida é um aviso sobre a cena; o `R` para reiniciar não existe e
não há menu inicial. O 3a transforma isso em cenas (`Screen::Menu`, `Play`,
`GameOver`, `Victory`) com transição e reinício de verdade.

## Bloqueios / pendências

- **ARM64 testado** desde o 1c: o binário roda no X35H por KMSDRM e a medição
  está na seção do 1c. O que continua sem número no aparelho é só o custo de
  GPU sem vblank, e isso é limitação do driver (ver abaixo).
- **Som e resposta ao dano existem desde o 2c**, mas **não foram ouvidos por
  ninguém**: o que está provado é que os cinco `Chunk` carregam no `rk817`, que
  `play` responde OK e que o mixer está a 100 % com o alto-falante ligado. Se
  sai som do alto-falante é a única coisa que o software não mede aqui.
- **O fim de partida ainda é um aviso sobre a cena**, não uma cena: não há `R`
  para reiniciar nem menu inicial. É o 3a.
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
- **X35H, 15 cubos + HUD (2b)**: 49,3 fps, trabalho 3,28 → **3,82 ms** (16 % →
  19 %). A HUD custa **0,54 ms** e sai em **2 chamadas** de desenho — 4 com o
  clarão de dano, 7 com o aviso de fim.
- **X35H, 15 cubos + tiro contínuo + som (2c)**: 49,3 fps, trabalho **4,58 ms**
  (23 %); com `--sem-som`, 4,52 ms (22 %). O áudio custa **0,06 ms** — dentro do
  ruído, porque só é chamado no evento. O `p99` sobe de 21,1 para 24,1 ms: a
  thread do mixer do `SDL_mixer` concorre com o quadro, e isso é visível no pior
  caso mesmo com a mediana intacta.
- **PC (Radeon RX 580, 1280×720, `offscreen`)**: 15 cubos, 0,05 ms de intervalo —
  sem vblank o número é limite superior de CPU e não serve como FPS do jogo.
- **Simulação (sem GL)**: 0,28 µs por quadro com 15 cubos — 0,00 % do orçamento.
- Geração de malha, uma vez na abertura: asteroide 82,8 µs (80 triângulos), céu
  de 500 estrelas 535,1 µs (6.000 triângulos), mira ~17 µs (86 triângulos,
  reconstruída uma vez na partida).
- Chamadas de desenho por quadro: 4 da arena + até 15 dos cubos + 1 da mira +
  até 7 da HUD = **27**, dentro do envelope do R5.

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
11. **A HUD é instanciada, não reconstruída.** Uma malha de 2 triângulos e uma
    matriz por quadrilátero: o texto inteiro de um quadro são centenas de
    instâncias em uma chamada por cor. Refazer a malha por quadro subiria
    centenas de KB de vértice para desenhar a mesma coisa.
12. **O alfa é quantizado em 8 degraus.** É o que impede oito `+100` desbotando
    em instantes diferentes de virarem oito chamadas de desenho. A diferença de
    alfa entre degraus (12 %) é invisível num efeito de 0,9 s a 60 Hz.
13. **A escala da HUD vem do menor lado da tela**, não da altura. Num painel
    retrato (720×1280) escalar pela altura dava letra de 37 px e os três blocos
    do topo se cruzavam.
14. **O som é sintetizado e carregado por `from_raw_buffer`.** Nenhum arquivo,
    nenhuma licença, e nenhuma biblioteca nova no `NEEDED`. O preço é o formato
    do mixer virar contrato — amarrado por teste.
15. **O tremor de câmera é translação, não giro.** Girar em volta do alvo
    mantém o alvo centrado e o efeito não aparece (medido: 1,14 px contra
    10,3 px). Olho e alvo andam juntos, então a direção de visão não muda.
16. **A novidade do anúncio é decidida em um lugar só.** `anunciar_onda` e
    `anunciar_fim` devolvem `bool`, e o laço toca o som por esse retorno em vez
    de repetir a condição — duas cópias da regra envelheceriam diferente.

