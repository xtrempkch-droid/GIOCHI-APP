# GIOCHI Tower Defense

Tower Defense 3D co-op PvE (2–4 jogadores) sobre a **GIOCHI-ENGINE-3D**, alvo
`aarch64-unknown-linux-gnu`, mínimo **RK3326 + 1 GB** como cliente.

> **Estado: M1, M2 e M3 entregues; M4 em 3 de 6.** A engine vem do repositório
> da engine (`main` = `33a3b92`, com texturas e OBJ), vendorizada em `vendor/` —
> três crates: `giochi-engine`, `giochi-engine-3d` e `giochi-net`. O jogo abre,
> tem menu, desenha personagens com modelo e textura, joga em rede real e roda no
> R35S a 59,9 fps. O que foi decidido fora do prompt está em
> [`DECISIONS.md`](DECISIONS.md) — inclusive a correção sobre a ABI C (D-002).

O design travado está em
[`docs/GDD.md`](docs/GDD.md) e o prompt
mestre em [`../prompt.md`](../prompt.md).

## O que já roda

**M1 — fundação**

- Abre janela OpenGL ES 2.0 (EGL) via engine; em KMSDRM pede fullscreen sozinho.
- Câmera top-down isométrica (yaw 45°, pitch 35,264°) que segue o jogador.
- Jogador da classe escolhida anda em 8 direções relativo à câmera e mira.
- Loop de simulação **fixo em 60 Hz**, desenho no ritmo do aparelho.
- Carrega e **valida** os 10 arquivos de `config/` antes de abrir a janela.

**M2 — rede e combate**

- 5 tipos de inimigo com escala por onda (HP ×1,15·N, dano ×1,10·N, velocidade
  ×1,05·N), steering com aggro e o "para e atira" do Ranged.
- Spawner lendo `waves_campaign.ron`: composição por onda, intervalo por grupo e
  **teto de vivos** do perfil (segura o spawn em vez de descartar inimigo).
- Combate: hitscan com cone de 70°, cadência por classe, gold e pontos por kill.
- Torre com HP 5000, armadura 5 e regeneração, dano mitigado por curva.
- Fases do GDD §4 de ponta a ponta: preparação → combate → loja, com **vitória**
  na onda 6 (só com o campo limpo) e **derrota** quando a torre cai.
- Rede: protocolo em `postcard`, AOI derivada do frustum, stub de loopback
  host+cliente — o snapshot que a tela desenha é o que o **cliente** recebeu,
  filtrado por AOI.

A interface **desenha**: HUD (onda, relógio, gold, placar, HP do jogador e da
torre), minimapa e tela da loja. É fonte de 5×7 em dados + câmera de 90° sobre o
3D, com uma chamada de desenho por cor — ver `DECISIONS.md` D-029 e D-030.

**M3.5 — arte, menu e rede de verdade**

- **15 modelos OBJ e 15 texturas BMP**, gerados por
  `scripts/gerar-modelos.mjs` (determinístico, `--conferir`): 4 classes, 5
  inimigos, 4 bosses, o Coronel e a torre. A horda sai em **um lote instanciado
  por modelo** — 12 inimigos custam 0,47 chamada de desenho por quadro.
- **Menu** com cinco telas (Principal, Multijogador, Configurações, Pausa, Fim),
  estado puro em `src/menu.rs`, `settings.json` persistido em `--config`.
- **Rede real sempre compilada** (`net/sessao.rs`, `net/descoberta.rs`): host
  escuta e anuncia por UDP broadcast, cliente procura e entra. O id do jogador
  vem do handshake e vira o `local_id` do mundo.

Não faz (ainda): animação nos modelos, áudio por evento, predição de rede e a
entrada por evdev do aparelho (M5). O estado atual, com números medidos, está em
[`STATUS.md`](STATUS.md).

## Estrutura

```text
app/jogos/giochi-tower/          ← o app (tem metadata.json)
├── docs/
│   ├── GDD.md                   design oficial: ondas, classes, bosses, loja
│   └── prompt.md                brief que originou o GDD
├── scripts/
│   └── gerar-modelos.mjs        gera assets/modelos/*.obj e assets/texturas/*.bmp
├── Cargo.toml                   workspace; vendor/ é excluído e consumido por caminho
├── build.rs                     carimba alvo/perfil e falha cedo se o snapshot faltar
├── DECISIONS.md                 toda decisão fora do prompt + o que não foi verificado
├── STATUS.md                    onde o projeto está e o que falta
├── config/                      10 arquivos RON — todo número de balanceamento mora aqui
├── bin/                         binário aarch64; vai para o aparelho junto com config/
├── capa.png                     320x320, capa da loja
├── forge.json                   requisitos: gles2, 96 MB, 480x320
├── assets/modelos/              15 .obj low-poly, gerados (não editar à mão)
├── assets/texturas/             15 .bmp 24 bits, gerados (sem PNG: §24 do AGENTS)
├── src/
│   ├── main.rs                  bootstrap, argumentos, loop fixo
│   ├── lib.rs                   módulos públicos
│   ├── config.rs                structs serde + validação mecânica do GDD
│   ├── errors.rs                GameError
│   ├── engine_bridge/           única camada que fala com a engine
│   │   ├── mod.rs               fachada
│   │   ├── input.rs             gamepad (SDL), teclado/mouse, bordas, zona morta
│   │   ├── render.rs            contexto GLES, malhas, desenho, captura BMP
│   │   ├── modelos.rs           carrega os .obj e as texturas (GIOCHI_TOWER_ASSETS)
│   │   ├── cena.rs              tabela de aparência + agrupamento em lotes
│   │   ├── audio.rs             mixer (sem uso no M1)
│   │   └── touch.rs             touch emulado por mouse (joystick + 4 botões)
│   ├── menu.rs                  máquina de estados pura do menu (5 telas, 21 testes)
│   ├── net/
│   │   ├── mod.rs               fachada: traits ClientBackend/HostBackend
│   │   ├── protocol.rs          GameMsg em postcard (input, onda, torre, loja)
│   │   ├── stub.rs              loopback host+cliente no mesmo processo (banco de prova)
│   │   ├── sessao.rs            SessaoJogo::Host|Cliente — a única porta de rede do laço
│   │   ├── descoberta.rs        anúncio e procura de salas por UDP broadcast
│   │   └── aoi.rs               Interest Management sobre o AOI do giochi-net
│   ├── ui/                      HUD, loja, minimapa (desenho por retângulo)
│   └── game/
│       ├── camera.rs            mapa 4x4 telas, câmera isométrica, raio de AOI
│       ├── player.rs            classes, dano, respawn, predição local
│       ├── enemy.rs             5 tipos, escala por onda, steering, ataque
│       ├── waves.rs             spawner por onda, teto de vivos, Endless
│       ├── tower.rs             HP, armadura, regeneração, tetos de loja
│       ├── timers.rs            recarga com tolerância de f32 (60 ticks ≠ 1 s)
│       ├── boss.rs              Tanker, Plasma, Brain, Queen e fases
│       ├── nemesis.rs           6 tiers, HP efetivo, item exclusivo
│       ├── score.rs             pontuação do GDD §13
│       └── state.rs             host: orquestra tudo, tick fixo, fases
├── tests/
│   ├── nemesis_evolution.rs     tiers crescentes e item exclusivo
│   ├── net_stub.rs              host+cliente trocam input/snapshot sem deadlock
│   └── modelos.rs               os 15 OBJ + 15 BMP carregados pelo loader da engine
└── vendor/                      engine (main 33a3b92) — NÃO EDITAR (regra R1)
```

## Configuração

Nenhum valor de balanceamento está no código (prompt §10). Tudo em `config/`:

| Arquivo | Conteúdo | Fonte |
|---|---|---|
| `classes.ron` | 4 classes, HP e 8 habilidades | GDD §7 |
| `enemies.ron` | 5 tipos, escala por onda, teto por perfil | GDD §8, §16 |
| `waves_campaign.ron` | 6 ondas, 6 min + 1 min | GDD §5 |
| `waves_endless.ron` | onda 7+, boss em toda onda | GDD §4 |
| `bosses.ron` | Tanker, Plasma, Brain, Queen e fases | GDD §9 |
| `nemesis.ron` | 6 tiers, item exclusivo, atraso do minimapa | GDD §10 |
| `items.ron` | 9 itens de loja + 1 exclusivo | GDD §12 |
| `host_profiles.ron` | Full / Balanced / Rk3326 | GDD §16 |
| `tower.ron` | 5000 HP → 8000, armadura 5 → 15, regen 0 → 10/s | GDD §11 |
| `score.ron` | kill 10, boss 500, onda 100×N | GDD §13 |

`tower.ron` e `score.ron` não estavam na lista do prompt e foram acrescentados —
o motivo é que o GDD §11 e §13 também são tabelas de balanceamento
(`DECISIONS.md`, D-014).

A validação é **mecânica**: número de classes, de inimigos, de ondas, de tiers,
faixa de preço, coerência torre base × teto de loja, e a regra do GDD §10 de que o
tier 5 do nemesis tem de ser mais forte que a Queen. Um `classes.ron` com três
classes não abre o jogo — falha antes, dizendo qual arquivo e qual regra.

Tunar o jogo = editar o RON. Não precisa recompilar.

## Compilar

O caminho tem espaços: use aspas em todo comando de shell.

```bash
cd app/jogos/giochi-tower

rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

O `vendor/` já traz o SDL2 por `pkg-config` (`sdl2-system`, o default). Em máquina
sem SDL2 de sistema:

```bash
cargo build --release --target aarch64-unknown-linux-gnu --features sdl2-bundled
```

## Rodar

```bash
# PC (usa o SDL2 do sistema)
cargo run --release

# aparelho, com o catálogo do sistema
GIOCHI_TOWER_CONFIG=/opt/giochi/apps/giochi-tower/config ./giochi-tower
```

Flags:

```text
--config <dir>      diretório dos .ron e do settings.json (padrão: config)
--tamanho <LxA>     1280x720, 800x600, 640x480 (no KMSDRM o painel manda)
--classe <nome>     soldado | guerreiro | sniper | medico
--perfil <nome>     full | balanced | rk3326
--menu              abre no menu em vez de entrar direto na partida
--hospedar [porta]  hospeda uma partida em rede (padrão 7777)
--entrar <ip:porta> entra numa partida em rede
--teclas <lista>    injeta teclas uma por quadro (ver abaixo)
--quadro <arquivo>  grava o último quadro em BMP 24 bits (lido antes do swap)
--frames <n>        roda n quadros com passo fixo, sem relógio real
--benchmark <n>     roda n quadros sem espera e reporta FPS e custo por quadro
--sem-ui            sem HUD (para isolar o custo da UI)
--demo-loja         abre a loja na abertura (para capturar a tela)
--ajuda
```

`--teclas` aceita `cima`, `baixo`, `esq`, `dir`, `a`, `b` e **`start`**. Cada
tecla entra pela fila de eventos e é aplicada pelo laço, uma por quadro com um
quadro de soltura entre duas — é o caminho de produção, não um atalho:

```bash
giochi-tower --menu --teclas "a"                    # começa a partida
giochi-tower --frames 90 --teclas "start,baixo,a"   # pausa e reinicia
giochi-tower --menu --teclas "cima,a"               # sai
giochi-tower --menu --teclas "baixo,baixo,a,esq,esq" # volume 60% e grava
```

## Controles

Gamepad tem prioridade sobre teclado (GDD §14).

| Ação | Gamepad | Teclado |
|---|---|---|
| Mover | analógico esquerdo | WASD |
| Mirar | analógico direito | setas |
| Atacar | A / ✕ | espaço |
| Habilidade 1 | B / ○ | Q |
| Habilidade 2 | X / □ | E |
| Item rápido | Y / △ | F |
| Loja (no intervalo) | D-pad ↑ | Tab |
| Pausar | Start | Esc |

Touch é a terceira opção: joystick virtual no canto inferior esquerdo e quatro
botões à direita, emulados por mouse (o prompt proíbe `SDL_Finger`).

## Verificar

```bash
cd app/jogos/giochi-tower

cargo test                                  # unidade (config, câmera, estado, input, touch, BMP)
cargo clippy --all-targets -- -D warnings
cargo build --release --target aarch64-unknown-linux-gnu

# cena sem interação, para a bancada conferir o quadro
cargo run --release -- --tamanho 1280x720 --frames 30 --quadro /tmp/tower.bmp
```

No repositório, o validador de app confere a estrutura:

```bash
node scripts/validate-app.mjs app/jogos/giochi-tower
```

### Limite honesto desta entrega

O código do M1 foi escrito **sem compilador Rust na máquina de geração**
(ver `DECISIONS.md`, D-020). As assinaturas da engine foram conferidas símbolo a
símbolo no `main` do repositório da engine — `new_opengl`, `draw_mesh_colored`,
`create_*_mesh`, `begin/end_frame`, `read_pixels`, `release_all`,
`Camera3D::from_parts`/`with_fov_y`/`with_clip_planes`, `Input::{new, update,
gamepad, key_down, mouse_down}`, `GameLoop`, `AudioSystem`, `DirectionalLight::sun`
— com arquivo e linha citados nos comentários. Esse método **custou caro**: o
primeiro `cargo test` abriu 86 falhas e o vendor nem compilava. O episódio e as
correções estão em [`STATUS.md`](STATUS.md); a regra que fica é que ler a
assinatura funciona, mas **não dispensa compilar antes de seguir.

## Próximos blocos

| Bloco | Entrega | Estado |
|---|---|---|
| M1 | fundação, câmera, configs, loop fixo | **entregue** |
| M2 | rede (protocolo/AOI/stub), inimigos, spawner, torre, score | **entregue** |
| M3 | 4 bosses com fases, nemesis por tier, loja, HUD, minimapa, tela da loja | **entregue** |
| M4 | host/cliente reais sobre `giochi-net`, mDNS, predição e *smoothing*, AOI unicast | **3 de 6** — host/cliente, descoberta UDP e `local_id` |
| M5 | perfis de host, evdev no aparelho (inclui eixos analógicos), `package.sh` | parcial — roda no R35S por KMSDRM com modelos, falta evdev |
| M6 | balanceamento em RON, Endless, áudio, arte low-poly, README final | iniciado — modelos, texturas, menu e volume |

`--benchmark` é o que mede M5 de verdade: com ele o FPS do aparelho é um número,
não uma impressão. Ver [`STATUS.md`](STATUS.md).

## Licença

Proprietary — ver `metadata.json`. O snapshot da engine em `vendor/` é MIT
(`vendor/Cargo.toml`) e não é modificado aqui.
