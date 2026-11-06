# Arcade Volleyball

Port do clássico *Arcade Volleyball* (1988) para o Giochi Konsol OS. O alvo
principal é **Godot 4.3 + renderizador Compatibility (GLES3)** — o runtime que o
[ADR-07](../../docs/architecture-v1.2.md) fixa para o RK3566 e que o
`device_profile.json` do aparelho declara (`engine.godot_max = 4.3`).

O port original em **C++17 + SDL2** continua aqui como referência de baixo
custo e para medir duas implementações das mesmas regras.

| Item | Godot (principal) | C++/SDL2 (referência) |
|---|---|---|
| Código | [godot/](./godot) | [arcade_volleyball.cpp](./arcade_volleyball.cpp) |
| Build | `godot --path godot` | [Makefile](./Makefile) |
| Dependência | motor Godot 4.3 (export template ARM64) | `pkg-config sdl2` |
| Regras | `godot/scripts/sim/match.gd` | mesma física, mesmas constantes |
| Testes | `godot/tests/run_tests.sh` | harness de fuzz (ver "Histórico") |
| Estado | jogável no host, medido | jogável no host, medido |

## Controles

| Ação | Teclado | Gamepad |
|---|---|---|
| Andar para a esquerda | `A` ou seta esquerda | dpad esquerda / analógico |
| Andar para a direita | `D` ou seta direita | dpad direita / analógico |
| Pular | `W`, seta para cima ou espaço | `A` |
| Pausar | `Esc` | `Start` |
| Painel de orçamento | `F3` | — |
| Gráficos (na pausa) | `←` `→` | — |
| Sair (na pausa) | `Q` | — |

O `P2` é uma **IA simples** (segue a bola e pula quando ela chega perto): o
console não tem segundo controle, então não há partida de dois jogadores.

Nenhum dos dois ports lê `buttons.map` do sistema: o Godot usa o mapeamento
interno do `SDL_GameControllerDB` que acompanha o motor, e o port C++ usa o
`SDL_GameController` do host.

## O que foi modernizado (e quanto custa)

Tudo abaixo usa **zero arquivo de imagem e zero arquivo de áudio**. A arte é
vetor (`_draw`) e um `GradientTexture2D` radial de 128×128 (~64 KB); o som é
sintetizado no boot em `AudioStreamWAV`.

| Recurso | Como | Custo |
|---|---|---|
| Quadra procedural | fragment shader (`shaders/court.gdshader`): piso em perspectiva, feixes de luz, halo, vinheta e grão | 1 draw call, 0 textura |
| Personagens | cabeça + tronco com `StyleBoxFlat` (cantos, contorno, sombra projetada), viseira na cor do time | vetor, 1 draw por jogador |
| Bola | círculo + brilho especular + anel de accent girando; halo aditivo acompanhando | 1 textura compartilhada |
| Rastro | `Line2D` com gradiente, descartando os pontos antigos | 12–24 pontos |
| Partículas | `CPUParticles2D` (poeira ao aterrissar, faísca no toque, explosão no ponto) | 0–36 por emissão |
| Anel de impacto | `fx_ring.gd`: círculo que expande e some | 1 `_draw` |
| Flash de tela | `ColorRect` com alpha decaindo | 1 draw |
| Camera shake | trauma² + zoom punch (cada impacto soma trauma) | sem custo |
| Squash & stretch | escala no pivô do pé / da bola | sem custo |
| Hit-stop | `Engine.time_scale = 0.05` por 45–60 ms em toque forte | sem custo |
| Slow motion | `0.30` por 260 ms no ponto; `0.35` por 900 ms na vitória | sem custo |
| Placar sem fonte | rótulos do tema com cor por lado, pop ao marcar | 1 atlas de fonte |
| Som procedural | bounce/spike/net/saque/ponto/vitória/UI, gerados por código | ~5 KB por som, em RAM |

O jogo tem **regras de rally**: quem chega a **15** fecha o set, com banner de
vitória e reinício automático.

## Orçamento (medido, não estimado)

Limite do RK3566 (`device_profile.json`): **700 MB de RAM** e **128 MB de
VRAM**. Medido no host com Mesa 26, GPU real (Radeon RX 580) — **no aparelho
nada disso foi medido ainda**:

| Preset | RAM (RSS) | VRAM total | Texturas | Buffers | Draw calls |
|---|---|---|---|---|---|
| Baixo | 190,5 MB | 27,1 MB | 16,9 MB | 10,2 MB | 41 |
| Médio (padrão) | 191,3 MB | 23,1 MB | 16,9 MB | 6,2 MB | 51 |
| Alto | 191,5 MB | 27,1 MB | 16,9 MB | 10,2 MB | 51 |

Entre execuções a VRAM oscila ~±3 MB (buffers transitórios das partículas e do
backbuffer); já foi medida em 23,1 MB e em 29,1 MB na mesma cena.

Leitura honesta desses números:

- **VRAM**: 23–29 MB é no máximo ~23% do teto. 16,9 MB são do próprio motor (atlas do tema
  padrão e da fonte); a arte do jogo soma menos de 1 MB. Por isso os presets
  quase não mudam a VRAM: eles trocam trabalho de CPU (partículas, rastro,
  sombra) e não memória de vídeo.
- **RAM**: ~190 MB no host. Com o rasterizador de software (llvmpipe) a mesma
  cena mede ~248 MB, porque o desenho acontece na CPU. O número do aparelho
  fica entre os dois e **só o aparelho decide** — o jogo tem o painel `F3`
  justamente para isso.
- Nos dois SoCs o jogo passa no classificador do repositório (ver
  "Integração").

## Como rodar

No host, com o motor Godot 4.3:

```sh
godot --path "Jogos/Arcade Volleyball/godot"
```

Sem monitor (CI/SSH), o motor roda a cena e o laço de física:

```sh
godot --headless --path "Jogos/Arcade Volleyball/godot" \
      res://scenes/Main.tscn --quit-after 240
```

O port C++ continua:

```sh
make -C "Jogos/Arcade Volleyball"
./build/host/bin/arcade-volleyball
```

## Testes

```sh
make godot-test                                   # usa godot do PATH
GODOT=/caminho/godot make godot-test              # ou aponte o binário
```

O runner ([godot/tests/run_tests.sh](./godot/tests/run_tests.sh)) faz quatro
coisas e sai com 0 sem o motor instalado (mesmo critério de
`tests/test_screenshots.py`, que pula sem Pillow):

| Etapa | O que cobre |
|---|---|
| importação | gera o cache de classes globais; sem ela o `--script` falha com *"Could not find type"* |
| `tests/invariants.gd` | movimento, limites, pulo de borda, saque, 6 000 passos de partida, fim de set e 20 000 passos de fuzz com entrada aleatória |
| cena principal | 240 quadros da cena real em headless; falha se o engine ou um script imprimir erro |
| `tests/budget_probe.gd` | mede RAM/VRAM nos três presets e **falha** acima de 700 MB / 128 MB (precisa de contexto OpenGL; pulado sem `DISPLAY`) |

Resultado atual dos invariantes: **204 pontos em 20 000 passos**, maior
intervalo sem ponto **292 passos (4,9 s)** — sem NaN, sem bola fora da tela,
sem bola dentro da rede, ninguém atravessa a rede.

### Por que o modelo é separado da cena

`scripts/sim/match.gd` não conhece engine gráfico: recebe entrada por um objeto
`Controls` e devolve eventos (`hit`, `net`, `point`, `win`). A cena
(`scripts/main.gd`) consome os eventos e faz o juice. Isso é o que torna o
teste de 20 000 passos possível e determinístico — e permite rodar as mesmas
regras no port C++.

O passo é fixo (`scripts/sim/fixed_step.gd`): acumula o delta escalado pelo
`Engine.time_scale` e gasta em passos de 1/60 s, com teto de 3 passos por
quadro. É isso que faz o slow motion funcionar sem quebrar a integração, e é
testado (60 quadros a 60 Hz = 60 passos; a 0,25× = 15 passos).

## Regras (iguais nos dois ports)

- Gravidade `0.6` no jogador e `0.42` na bola, por passo a 60 Hz.
- Jogadores `80x100`, velocidade `8`, impulso de pulo `-14`.
- Bola com raio `20` e velocidade máxima `15`.
- Rede é **sólida**: a bola só passa por cima do topo.
- Bola na cabeça (`normal.x` quase zero) sai **para o lado**: sem isso ela
  subia e caía na mesma vertical, o jogador se reposicionava e a partida
  parava sem ninguém pontuar.
- Quique praticamente vertical junto da parede recebe um empurrão lateral
  (`±3`), pela mesma razão.
- Depois das colisões a bola é presa de volta à área (um jogador na parede
  podia empurrá-la para fora por um passo).
- Ponto: a bola toca o chão de um lado e o ponto vai para o outro. O saque sai
  do lado de quem pontuou.

## Integração com o repositório

O projeto declara seus requisitos em [godot/forge.json](./godot/forge.json)
(schema: [schemas/forge.schema.json](../../schemas/forge.schema.json)) e o
`giochi-forge` classifica o jogo sem intervenção:

```console
$ apps/forge/giochi-forge check --device rk3566-2gb \
      --project "Jogos/Arcade Volleyball/godot"
dispositivo: manual-rk3566-2gb (rk3566, 2048 MB, Mali-G52)
requisitos : forge.json
Compatível
  ! GLES3 com fallback GLES2: qualidade reduzida possivel
```

O mesmo comando com `--device rk3326-1gb` também responde **Compatível**. Sem
`forge.json`, o forge infere os requisitos do `project.godot` e chega ao mesmo
resultado.

O cache do motor (`godot/.godot/`) é gerado: está no `.gitignore` e na lista de
exceções de [tools/check-tracked.sh](../../tools/check-tracked.sh). Removê-lo
faz o `make lint` acusar arquivos do projeto ignorados.

## O que falta

- **Nada foi rodado em aparelho.** As medições são de host (Mesa/AMD); o
  Mali-G52 com Panfrost tem outro driver e o número pode mudar.
- **Não há como publicar como `.giochi` ainda.** O schema aceita
  `runtime: "godot"`, mas o export template precisa ser recompilado contra musl
  ([ADR-03](../../docs/architecture-v1.2.md)) e o `giochi-forge` ainda não
  chama o export do Godot ([docs/APPS.md](../../docs/APPS.md)).
- **RK3326** (Mali-G31, 64 MB de VRAM, 1 GB de RAM): os números cabem até no
  preset baixo, mas o ADR-07 fixa **Godot 3.5 LTS** nesse SoC. Um projeto 4.3
  não abre no 3.5 — para o RK3326 há o port C++/SDL2 ou um segundo projeto.
- Sem áudio verificado: os sons são gerados e o pool toca, mas nunca foram
  ouvidos em ALSA de aparelho.
- Sem pausa com menu de opções completo (só troca de preset de gráficos), sem
  placar salvo e sem dois jogadores.
- O Launcher e a Loja não conhecem esta pasta: o jogo roda chamado à mão.

## Histórico

A primeira versão deste diretório tinha um `.cpp` que **não compilava** (novas
de linha colapsadas; o `//` de cada cabeçalho engolia declarações) e uma doc que
mandava publicar com `giochi-forge` definindo `runtime: "native"`, valor que o
schema não aceita. Reescrevemos o port C++, corrigimos dois vícios de física
(bola quicando para sempre na mesma vertical e bola saindo da tela) e
documentamos tudo.

Depois disso o projeto Godot foi criado com as mesmas regras e as mesmas
constantes — inclusive os dois empurrões laterais acima, que o port C++ também
recebeu para manter as duas físicas iguais (fuzz do C++ depois da mudança: 219
pontos em 20 000 quadros, maior intervalo 1,9 s).

Três defeitos encontrados **medindo**, não lendo:

1. `PlayerView` atualizava o estado mas nunca `position`, então os dois
   jogadores apareciam desenhados na origem (0,0).
2. A sombra no piso era desenhada no eixo trocado (`Vector2(y, 0)` em vez de
   `Vector2(0, y)`).
3. Banner e flash usavam `Tween`, que roda com delta escalado: durante o
   hit-stop o banner levava ~1,6 s para aparecer e ficava fora de sincronia com
   o som. Passaram a animar em tempo real medido por `Time.get_ticks_msec()`.
