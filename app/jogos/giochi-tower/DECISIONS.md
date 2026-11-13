# DECISIONS — GIOCHI Tower Defense

Decisões tomadas durante a geração do projeto que **não estão no prompt mestre**
ou que **divergem** dele. O prompt manda: se algo não estiver especificado, decida
e registre aqui; se algo parecer errado, registre e **implemente como
especificado** — quando isso foi possível, a seção diz o que foi feito.

Estado: **M1 (Fundação) entregue.** M2–M6 pendentes.

---

## Índice

| # | Decisão | Impacto |
|---|---|---|
| D-001 | Projeto em `app/jogos/GIOCHI TOWER DEFENSE/giochi-tower/` | estrutura do repo |
| D-002 | Engine vendorizada do repositório da engine (`main`), como crates Rust | **corrigido** — ver a seção |
| D-003 | Câmera isométrica é perspectiva, não ortográfica | GDD §6 |
| D-004 | Uma "tela" do mapa = 40 m (mapa 160 × 160 m) | GDD §6 |
| D-005 | AOI derivada do frustum, não da diagonal de um retângulo | GDD §6 |
| D-006 | Gamepad por SDL no PC; evdev é trabalho do M5 | GDD §14 |
| D-007 | Zona morta radial e bordas derivadas por quadro | GDD §14 |
| D-008 | Loop fixo via `GameLoop` da engine + modo determinístico | prompt §9 |
| D-009 | Só cubo/plano/esfera; sem textura, sem HUD em M1 | arte |
| D-010 | Números de classe (dano, alcance, recarga, velocidade) | GDD §7 |
| D-011 | Velocidade e alcance dos inimigos | GDD §8 |
| D-012 | Raios, períodos e distâncias dos bosses | GDD §9 |
| D-013 | `stack_limit` (0 = sem limite) e item exclusivo | GDD §12 |
| D-014 | Dois arquivos de config a mais: `tower.ron`, `score.ron` | prompt §7 |
| D-015 | Composição das ondas (o GDD só fixa o pico vivo) | GDD §5 |
| D-016 | `metadata.json` em `scaffold` e entrada no `apps.json` | repo |
| D-017 | Dependências: `postcard`/`mdns-sd` só quando forem usadas | prompt §2 |
| D-018 | `sdl2` como dependência direta só para nomear `Event` | build |
| D-019 | `glam` como camada de matemática, na versão da engine (0.34) | build |
| D-021 | Vendor: três crates, sem `capi` e sem `lua` | estrutura |
| D-022 | `giochi-net` existe: a rede real substitui o stub no M4 | prompt §3 |
| D-020 | O que **não** pôde ser verificado nesta sessão | honestidade |
| D-029 | O HUD usa fonte de 5×7 em dados e câmera de 90° sobre o 3D | UI |
| D-030 | A decomposição do glifo é em retângulos, não em pontos | performance |
| D-031 | O estado do front buffer é medido até antes da troca de buffers | medição |

---

## D-001 — Onde o projeto mora

**Decisão:** `app/jogos/giochi-tower/` — **corrigida em 13/11/2026**.

A decisão original era `app/jogos/GIOCHI TOWER DEFENSE/giochi-tower/`, com uma
pasta de grupo carregando o design. Três níveis e um nome com espaços **passavam**
no validador (conferido), mas violavam duas convenções do repositório: nome em
kebab-case e profundidade máxima 2 (`AGENTS.md` §5). A pasta foi renomeada, e
`docs/GDD.md` e `docs/prompt.md` foram para dentro do app — o design não é um
app, então não precisa de pasta própria para existir.

O validador continua distinguindo grupo de app pelo `metadata.json`
(`scripts/lib/apps.mjs`); a convenção é que os jogos moram em `app/jogos/<slug>`
e pronto.

**Verificado, não suposto.** A dúvida era se três níveis
(`app/jogos/<grupo>/<app>`) e um nome de grupo com espaços passariam no
validador. Testado:

```text
$ node scripts/validate-app.mjs --all
OK    giochi-tower  [scaffold]
OK: 11 app(s) válido(s) (2 em scaffold).
```

Passa. O `apps.json` registra `folder: "jogos/GIOCHI TOWER DEFENSE/giochi-tower"`.

**Alternativa descartada:** `app/jogos/giochi-tower/` como irmão dos outros jogos.
Funciona, mas separa o design do código — e o dono do projeto apontou esta pasta
como o lugar do jogo.

**Efeito colateral aceito:** o caminho tem espaços. Scripts de shell precisam de
aspas; o `Cargo.toml` e o `cargo` não se importam.

O prompt desenha a árvore a partir de `giochi-tower/`; dentro da pasta do app, a
árvore é exatamente a pedida (`Cargo.toml`, `build.rs`, `config/`, `assets/`,
`src/`, `tests/`).

## D-002 — A engine vem do repositório da engine (`main`), como crates Rust

**Correção de registro.** A primeira versão deste documento afirmava que a ABI C
"não existe". Estava **errado**, e o erro foi de fonte: eu vendorizei o snapshot
que mora dentro de `app/jogos/giochi-defender/vendor/` — engine 0.3.1 no commit
`cd94ce6`, que **não inclui** o crate `giochi-engine-capi` — e concluí a partir
dele. Era o snapshot que estava velho, não a engine.

A engine de verdade está em
`C:\Users\Jussara dias\Desktop\plataforma sistema\Giochi engine 3d`, `main` =
`49c5d63`, e tem:

| Item | Estado real |
|---|---|
| `giochi-engine-capi/` | existe, com `include/giochi.h`, **ABI v3** |
| `giochi_gamepad_connected` / `_axis` / `_button_down` | **declarados** no `giochi.h` (linhas 985, 997, 1006) |
| `docs/C_ABI_CONTRACT.md`, `docs/C_ABI.md` | existem |
| `giochi-net/` | **existe**: "feat(net): crate giochi-net com protocolo, AOI e host/cliente P2P" |
| `glam` | **0.34**, não 0.29 |

**Decisão:** vendorizar `giochi-engine`, `giochi-engine-3d` e `giochi-net` **do
repositório da engine** (`main`) e consumi-los como crates por caminho.

**Por quê, e não FFI sobre `libgiochi.so`:**

1. **`giochi-net` não tem superfície C.** A `capi` cobre render, input, áudio,
   cena e assets — não cobre rede. Como o jogo é Rust e a rede dele é o
   `giochi-net`, o binário vai linkar crate Rust de qualquer jeito; usar FFI para
   a metade gráfica e caminho Rust para a rede significaria duas formas de falar
   com a mesma engine no mesmo processo.
2. **Os três jogos que compilam hoje** (`cobrinha`, `neon-drift`,
   `giochi-defender`) fazem exatamente isto (AGENTS.md §25, regra R1).
3. **`libgiochi.so` teria de ser construída e distribuída por alvo** (x86_64 para
   bancada, aarch64 para o aparelho) antes de o jogo compilar. Nesta máquina não
   há toolchain Rust; um `build.rs` que linka um `.so` inexistente transformaria
   "não compila" em "não acha `-lgiochi`", que é o diagnóstico errado.
4. **Tipagem cheia** num jogo com 4 classes, 5 inimigos, 4 bosses, 6 tiers de
   nemesis e 11 variantes de protocolo.

**Se a decisão mudar:** o contato com a engine está todo em `src/engine_bridge/`
e nada mais no jogo a conhece. A troca para a ABI C é reescrever esse módulo
(declarações `extern "C"` a partir do `giochi.h` + `build.rs` com `-lgiochi` +
`-Wl,-rpath`), sem tocar em `game/`, `ui/` nem `net/`. A rede continuaria em Rust
enquanto a `capi` não expuser `giochi_net_*`.

**O que continua valendo do prompt:** proibido engine genérica, `stub-net` como
default, e separação jogo × engine.

## D-003 — Câmera isométrica é perspectiva

O GDD §6 pede "top-down isométrica". A engine **não tem** câmera ortográfica:

- `Mat4::orthographic` existe (`math3d/mat4.rs:118`), mas
- `Camera3D` só constrói `Mat4::perspective` (`math3d/camera.rs:118`), e
- `draw_mesh*` recebe apenas `&Camera3D` — não há como injetar matriz própria.

**Decisão:** perspectiva com yaw 45° e pitch `atan(1/√2) ≈ 35,264°`, FOV vertical
32° e distância 52 m. É o ângulo isométrico clássico; sem visão a pino, um cubo
mostra três faces, que é a assinatura visual de "isométrico".

**Custo:** há perspectiva (objetos ao fundo menores). Num TD top-down isso é
imperceptível no mapa de 160 m e não afeta mira, porque a mira é feita no plano do
chão pela própria câmera (`ground_direction`).

## D-004 — Uma "tela" = 40 m

O GDD fala em mapa "4 × 4 telas" sem definir tela. Adotado **40 m por tela**:
mapa de **160 × 160 m**, meia-extensão 80 m, torre na origem. É a escala em que o
Soldado (7 m/s) atravessa o mapa em ~23 s — rápido o bastante para o jogador
circular, lento o bastante para o mapa parecer grande (pilar "defesa em
movimento", GDD §2).

## D-005 — AOI derivada do frustum

O GDD pede "~1,5 × meia-diagonal da tela". Não há tela plana: há um frustum
projetado no chão. A conta usada (`camera.rs::aoi_radius`):

```text
meia_altura (no plano do foco) = distância · tan(fov/2)
meia_largura                   = meia_altura · aspect
meia_profundidade (no chão)    = meia_altura / sin(pitch)
AOI                            = aoi_scale · √(meia_largura² + meia_profundidade²)
```

`aoi_scale` vem do perfil de host: 1,5 (Full/Balanced) e 1,0 (Rk3326, "AOI
agressiva" = menos inimigos por cliente). Há teste para as duas pontas.

## D-006 — Gamepad: SDL agora, evdev no M5

O prompt diz que o gamepad "já está exposto na ABI C" e que não há nada a
implementar. Neste repositório, a leitura de gamepad existe no crate Rust
(`giochi-engine/src/input/gamepad.rs`) e é o que o M1 usa.

**Ressalva medida, não suposta:** no aparelho real o caminho SDL **não
funciona**. `giochi-defender/src/input_evdev.rs:3-17` registra a medição ("no PC
qualquer controle funciona, no aparelho nenhum funciona") e os dois jogos que
rodam no hardware leem `/dev/input` diretamente.

**Consequência para o M1:** o critério "input lido do gamepad" é verificável **no
PC** (caminho SDL). No aparelho, o M1 só responde a teclado. Trazer o caminho
evdev — e, junto com ele, os **eixos analógicos**, que hoje **ninguém** no
repositório lê (normalizar exige `EVIOCGABS` por ioctl, e `ButtonsMap::parse`
descarta as chaves `axis_*`) — é trabalho do **M5**, onde já estão os perfis e o
RK3326. Um jogo twin-stick sem analógico no aparelho não é jogável; entregar
esse caminho pela metade seria pior que declarar a lacuna.

## D-007 — Zona morta radial e bordas derivadas

- `GamepadState` só tem `is_down` (nível). "Apertou neste quadro" é derivado
  comparando com o quadro anterior (`input::edges`, com teste).
- A engine **não tem** zona morta implementada (o comentário do módulo diz que a
  SDL aplica a do mapeamento; não há código). Adotada zona **radial** de 0,18 com
  reescala: zona por eixo deixaria passar o canto da diagonal morta e faria a
  direção saltar na saída.
- Y da SDL é invertido (para cima = −1); a inversão acontece uma vez, no bridge.

## D-008 — Loop fixo e modo determinístico

Simulação em passos de 1/60 s via `giochi_engine::GameLoop` (o mesmo `FIXED_TIMESTEP`
do launcher e dos jogos). O desenho roda no ritmo do aparelho.

`--frames N` usa passo fixo e **não** lê o relógio real: dois `--frames 30` na
mesma máquina produzem o mesmo tick e o mesmo quadro. É o que torna a captura
comparável — a mesma razão pela qual o launcher tem `--dump-frame`.

## D-009 — Primitivas disponíveis

A engine 0.3.1 só cria **cubo, plano e esfera** (`mesh/primitives.rs`), e o
renderer **não tem upload de textura** (`create_texture` não existe). Por isso a
sala vazia usa cubos coloridos, e o HUD do M3 será desenhado com cubos e/ou com a
câmera 2D falsa (perspectiva de 90°, como `giochi-defender/src/ui/mod.rs` faz).

## D-010 — Números das classes

O GDD dá HP e adjetivos ("dano médio", "alto"). Adotado:

| Classe | Dano | Alcance | Recarga | Velocidade | Ab1 | Ab2 |
|---|---|---|---|---|---|---|
| Soldado | 12 | 14 m | 0,28 s | 7,0 m/s | Dash 8 m / 6 s | Granada 60 dmg r3 / 12 s |
| Guerreiro | 30 | 2,2 m | 0,55 s | 5,5 m/s | Escudo 50% / 3 s / 10 s | Provocar r6 / 2 s / 14 s |
| Sniper | 45 | 24 m | 1,10 s | 6,0 m/s | Mira +25% / 3 s / 8 s | Perfurante (3 alvos) / 12 s |
| Médico | 6 | 10 m | 0,50 s | 6,5 m/s | Cura 30 r4 / 9 s | Reanimar / 20 s |

DPS resultante: Soldado ~43, Guerreiro ~55 (melee), Sniper ~41 (à distância),
Médico ~12. O guerreiro é o maior DPS **e** o maior HP, mas paga em alcance (2,2 m)
e velocidade — é o fronteiriço do GDD §7, não um DPS puro.

## D-011 — Velocidade e alcance dos inimigos

HP/dano/gold são do GDD. `speed_mps` e `attack_range_m` não estão no documento:

| Inimigo | velocidade | alcance |
|---|---|---|
| Grunt | 2,2 m/s | 1,2 m |
| Tank | 1,2 m/s | 1,4 m |
| Ranged | 2,0 m/s | 9,0 m (para e atira) |
| Swarm | 3,6 m/s | 1,0 m |
| Flying | 2,8 m/s | 1,2 m |

Referência: o Grunt atravessa 160 m em ~73 s, então uma onda de 6 min dá tempo de
a torre ser alcançada várias vezes — o combate tem de ser jogado, não esperado.

## D-012 — Parâmetros dos bosses

Do GDD: HP, dano, gold e score. Acrescentados: raio/período de stomp (Tanker 3 m /
15 s; Queen 4 m / 8 s), teleporte do Plasma (5 m a 6 m de distância, mantém 12 m),
invocação e buff do Brain (4 adds / 20 s; +30% em 8 m), e as três fases da Queen
com multiplicadores de velocidade 1,0 / 1,15 / 1,5 e leques de 0 / 5 / 7
projéteis.

A fase 3 da Queen tem `speed_multiplier: 1.5`, que é o "velocidade +50%" do GDD
§9.4 — o número está no RON, não no código.

## D-013 — `stack_limit` e item exclusivo

O GDD não diz quantas vezes cada item pode ser comprado. Convenção: **0 = sem
limite**. Coleto/rifle/scope limitados a 3/3/2 para não transformar a loja em
"compre 40 rifles"; consumíveis e reparo de torre ilimitados.

O item exclusivo do nemesis (`ColonelTrophy`) está em `items.ron` com
`exclusive: true` e `price: 0`, e a validação **recusa** um item de loja marcado
exclusivo (e vice-versa). Ele existe no arquivo — e não num `enum` paralelo —
porque o GDD §10 diz "item de torre/arma que não existe na loja normal", e o
lugar de um item é `items.ron`.

## D-014 — Dois arquivos de configuração a mais

O prompt lista 8 arquivos em `config/`. O GDD §11 (torre) e §13 (pontuação)
também são tabelas de balanceamento, e a regra do prompt é "não hardcodar valores
de balanceamento". Adicionados `tower.ron` e `score.ron` — 6 e 5 campos.

## D-015 — Composição das ondas

O GDD fixa **pico vivo** e duração, não o total de spawns. A composição em
`waves_campaign.ron` foi escolhida para que o pico seja atingível sem estourar o
teto do perfil (o menor teto, Rk3326, é 45 — e a onda 6 pede pico de 120, então
o Rk3326 **não** pode hospedar a onda 6; é coerente com "RK3326 é cliente por
padrão", GDD §16).

Total de spawns por onda: 60, 110, 112, 144, 186, 230. O spawner (M2) é quem
garante o pico, atrasando spawns quando o teto é atingido.

## D-016 — Integração com o repositório

- `metadata.json` com `status: scaffold` no `apps.json`: não há `bin/` ainda, e o
  validador rebaixa `entrypoint_ausente` para aviso nesse estado
  (`scripts/lib/apps.mjs:607`).
- `entrypoint: bin/giochi-tower`, `arch: aarch64`, `graphics_api: gles2`,
  `license: Proprietary`, `distribution: image` — coerente com os outros três
  jogos.
- Passar para `ready` quando o M5 gerar o binário aarch64 em `bin/`.

## D-017 — Dependências que só entram quando forem usadas

O prompt permite `postcard`, `mdns-sd`, `rand`, `libc`, `bytemuck`. **Não**
foram adicionadas no M1, porque nada as usa ainda:

- `postcard` → M2 (protocolo de rede);
- `mdns-sd` → M4 (descoberta);
- `libc` → M5 (ioctl `EVIOCGABS` para os eixos do evdev);
- `rand` → M2 (spawn em borda aleatória);
- `bytemuck` → só se o renderer precisar de *cast* de buffer.

Dependência declarada e não usada é dependência que ninguém audita e que pode
quebrar o cross-compile sem dar sintoma. As features `stub-net` e `real-net` já
existem (o default é `stub-net`, como o prompt exige).

## D-018 — `sdl2` como dependência direta

`Input::update(&[sdl2::event::Event])` obriga a nomear `Event` na assinatura do
bridge. Sem `sdl2` nas dependências, o crate sequer compila — não é preferência,
é requisito da API. Mesma escolha do `giochi-defender` (`Cargo.toml:21`).

## D-019 — `glam` como camada de matemática

O `Vec3` da engine **não implementa `Add`/`Sub`/`Mul`** — só `new`, `length`,
`normalized`, `is_finite` e conversões para o glam. Toda a matemática do jogo vive
em `glam`, com conversão numa fronteira única (`engine_bridge/render.rs`).
Alternativa descartada: aritmética manual em `x/y/z`, que espalharia 4 linhas de
conta por ponto de uso.

A versão é **0.29**, a do snapshot da engine — **corrigida em 13/11/2026**.

A decisão dizia 0.34, e `vendor/Cargo.toml` fixava `glam = "0.34"` e
`glow = "0.18"`. Só que o código da engine usa a API de **0.29/0.14**
(`Mat4::look_at_rh`, `perspective_rh_gl`, `PixelPackData` com `&mut [u8]`): o
vendor não compilava, e o erro aparecia como "método não existe", longe da causa.
Os pins foram alinhados a 0.14/0.29 — as mesmas versões que `cobrinha`,
`neon-drift` e `giochi-defender` usam.

A regra que fica: **o pin tem de ser o da API que o código chama**, não o do
número que o manifesto da engine declara.

## D-021 — O vendor tem três crates, e não os cinco do repositório da engine

`vendor/Cargo.toml` declara um workspace com `giochi-engine`, `giochi-engine-3d`
e `giochi-net`, copiados de `Giochi engine 3d` @ `49c5d63`.

**Fora:** `giochi-engine-capi` (ABI C — servida a apps C/Python/Lua, não a este
jogo) e `giochi-engine-lua` (binding Lua; o prompt o chama de opcional e o jogo é
Rust puro).

**Dentro:** `giochi-net`, mesmo sem dependência declarada ainda. Ela entra no
vendor agora porque é ela que o M4 vai usar, e porque vendorizar depois seria
mexer no manifesto duas vezes.

`[workspace.dependencies]` é cópia fiel do manifesto da engine, incluindo
`mlua`/`cbindgen` por fidelidade — nenhuma entrada não usada é baixada pelo
`cargo`.

## D-022 — A rede deixou de ser stub obrigatório: `giochi-net` existe

O prompt §3 tratava `giochi-net` como issue aberta e exigia um stub local "com a
mesma assinatura planejada". No snapshot atual (a issue está mergeada) o crate
existe e tem `aoi.rs`, `host.rs`, `client.rs`, `wire.rs`, `peer.rs` — **zero
dependência externa**, serialização escrita à mão.

**Decisão:** manter a fachada `NetBackend` e a feature `stub-net` como default
(loopback host+cliente no mesmo processo, que é o que permite jogar sozinho e
testar sem rede), mas o backend **real** do M4 é o `giochi-net`, não uma
reimplementação do protocolo. O stub passa a ser o que o nome diz: um stub para
single-player e para teste, não uma antecipação do crate.

Isso não contradiz o prompt — ele manda não bloquear o desenvolvimento na issue.
Ele deixa de valer na parte em que a issue era o bloqueio, porque não é mais.

## D-023 — Endless reaproveita a composição da onda 6

O GDD §4 diz "onda ≥ 7 tem boss escalado" mas não define composição para o
Endless. Decisão: a onda 7+ usa **os grupos da onda 6**, com a contagem crescida
por `endless.spawn_count_growth` (12% por onda acima da campanha) e o pico por
`peak_alive_growth` (+8). A escala de HP/dano/velocidade soma
`extra_scaling_per_wave` (5%) por onda acima da 6 — implementado em
`config::scaling_for_endless_wave`.

Motivo: inventar cinco composições novas sem dados de balanceamento produziria
curva aleatória. Partir da onda 6 mantém a curva monotônica e deixa o ajuste fino
para o M6, onde há partidas medidas.

## D-024 — Spawn determinístico, sem `rand` no M2

A borda e a posição de spawn saem de um hash do `id` do inimigo, do número da onda
e do tick (`GameState::spawn_position`), não de `rand`.

Motivo: um teste pode afirmar "a onda 1 gera 60 inimigos e todos nascem numa
borda" sem semente mágica, e duas execuções com o mesmo número de ticks produzem
a mesma partida — o que é o pré-requisito para balancear (M6) e para comparar
quadros (`--frames`). `rand` entra no M5, quando o perfil `Rk3326` precisar de
variedade de patrulha; a assinatura de `spawn_position` não muda.

## D-025 — Fórmulas de combate que o GDD não dá

O GDD dá os números de entrada, não as fórmulas de saída. Adotado, e testado:

| Regra | Fórmula | Onde |
|---|---|---|
| Armadura da torre | `dano × 100 / (100 + armadura × 10)` → 5 = 66,7%, 15 = 40% | `tower.rs` |
| Cadência de inimigo | 1 golpe/s em contato | `enemy::ATTACK_INTERVAL_S` |
| Aggro | jogador a ≤ 12 m desvia o alvo (menos o Grunt/Tank, que seguem a torre) | `enemy::AGGRO_RADIUS_M` |
| Cone de tiro | `dot(mira, direção do alvo) ≥ 0,35` (~70°) | `state::target_in_aim` |
| Raio da torre | 8 m — é "chegou na torre" | `tower::TOWER_RADIUS_M` |

O cone é o que impede o Soldado de atirar pelas costas; sem ele o twin-stick do
GDD §14 perde o sentido.

## D-026 — Regras de transição de fase no M2

O GDD §4 dá o ciclo (6 min combate + 1 min loja) mas não diz o que acontece com
inimigos vivos quando o relógio zera.

- **Fim de onda comum:** o relógio zera → o campo é **limpo** e a onda conta como
  limpa (score `100 × N`). A próxima leva traz a sua própria composição.
- **Última onda da campanha:** só termina com **nenhum inimigo vivo** — senão a
  "vitória" seria anunciada com a torre ainda cercada. O relógio zerado não
  encerra; ele continua até o campo limpar.
- **Intervalo:** todos os mortos voltam com HP cheio na torre (GDD §5).
- **Derrota:** verificada **depois** de tudo no tick, para que o dano que derruba
  a torre apareça no resumo.

## D-027 — O protocolo tem duas camadas porque o crate tem

`giochi-net` transporta (`Mensagem`, com payload opaco de regra) e o jogo
interpreta (`GameMsg`, `postcard`). O snapshot de inimigos **não** foi duplicado
em `GameMsg`: é `giochi_net::wire::SnapshotInimigo`, o mesmo tipo que
`Host::tick_snapshot` consome e que `aoi::AreaDeInteresse::contem` filtra.

Consequência prática: o M4 não reescreve o jogo, só implementa
`ClientBackend`/`HostBackend` sobre `Cliente`/`Host`.

## D-028 — O stub carrega o input de verdade, mas em single-player o host já o tem

Em single-player, `main.rs` passa o `InputFrame` direto para `GameState::step` e
**também** o envia pelo stub. O snapshot que a tela desenha vem do cliente
(filtrado por AOI), então a rede é exercitada de ponta a ponta no caminho de
descida; o de subida é exercitado (contagem conferida no log de encerramento),
mas **não é a autoridade ainda**.

Isso é uma simplificação declarada do M2, não a arquitetura final: no M4 o host
passa a aplicar **somente** o input que chega pelo fio, e `GameState::step` deixa
de receber `&InputFrame`.

## D-020 — O que NÃO foi verificado nesta sessão

Regra de honestidade do repositório: nada é "funciona" sem ter sido executado.
Nesta sessão **não foi possível compilar**:

| Item | Estado |
|---|---|
| `cargo`/`rustc` | **ausentes** na máquina (nenhum toolchain Rust instalado) |
| `cargo build --target aarch64-unknown-linux-gnu` | **não executado** |
| `cargo test` | **não executado** |
| Rodar a sala vazia | **não executado** |
| Sintaxe dos 10 `config/*.ron` | **verificada** por parser RON próprio (gramática, chaves, contagem de ondas, 42 min, tiers 0–5) |
| Números do GDD nos RONs | **conferidos** contra o documento e por teste em `config.rs` (que ainda não rodou) |
| API da engine usada pelo bridge | **conferida símbolo a símbolo** no `main` da engine (`new_opengl`, `draw_mesh_colored`, `create_*_mesh`, `begin/end_frame`, `read_pixels`, `release_all`, `Camera3D::from_parts`/`with_fov_y`/`with_clip_planes`, `Input::{new,update,gamepad,key_down,mouse_down}`, `GameLoop`, `AudioSystem`, `DirectionalLight::sun`) — todas presentes com a mesma assinatura |
| `vendor/` | **conferido**: 1,04 MB, 122 arquivos, dos quais 3 crates (`giochi-engine`, `giochi-engine-3d`, `giochi-net`) |

Consequência: o código foi escrito contra assinaturas **lidas no código da
engine** (arquivo e linha citados nos comentários), não contra saída de
compilador. O primeiro `cargo test` numa máquina com toolchain é o próximo passo
obrigatório, e é provável que apareçam ajustes — especialmente em `render.rs`
(assinaturas de `draw_mesh_colored` e `read_pixels`) e `input.rs` (borrows do
`Input`).

Ver `README.md` § "Verificar" para os comandos exatos.

## D-029 — O HUD é geometria, com fonte em dados

**Decisão:** a interface vira retângulos em espaço de tela, desenhados com uma
câmera de `fov` 90° a `PROFUNDIDADE = 10` unidades, e o texto vem de uma tabela
de glifos 5×7 **em código**.

Três alternativas foram descartadas, todas por motivo medido:

| Alternativa | Por que não |
| --- | --- |
| `SDL2_ttf` | mais uma biblioteca na imagem (o R35S já apareceu sem ela), mais uma licença, e nenhum outro jogo deste repositório a usa |
| textura de atlas de glifos | a engine 0.3.1 não tem `create_texture_rgba`, e o shader `Textured` não lê matriz por instância — seria preciso um `TexturedInstanced` que não existe |
| desenhar a HUD em 2D no SDL, depois do GL | o SDL não desenha sobre um contexto GL aberto pelo EGL do KMSDRM sem `SDL_Renderer` no mesmo backbuffer; a tentativa não é sequer possível nesta configuração |

A fonte em dados tem a propriedade que interessa: **nítida em qualquer
resolução** (cada ponto é um retângulo do tamanho que se quiser) e sem nenhuma
dependência. Um texto de HUD é curto e sempre em caixa alta; a tabela tem 55
glifos e `normalizar()` resolve acento, travessão e ponto médio.

O plano próximo em **9,0** não é folga: com os planos padrão (0,1..1000) o HUD
cairia em profundidade normalizada ~0,99 e sumiria atrás do chão. A 10 unidades
com planos 9..4000 ele fica em ~0,10.

## D-030 — Glifo decomposto em retângulos máximos, não em pontos

**Decisão:** `font::retangulos()` devolve retângulos máximos (guloso por glifo),
e não um retângulo por ponto aceso.

Não é estilo: no Cortex-A35 do R35S cada instância do HUD custa **1,47 µs** de
submissão, medido em duas corridas (944 instâncias → 4,32 ms; 4 649 → 9,75 ms). A
média de um glifo 5×7 é **14,9 pontos** e **5,3 retângulos** — 2,8× menos
instâncias.

| | instâncias | desenho | FPS |
| --- | --- | --- | --- |
| HUD, um por ponto | 944 | 4,32 ms | 59,5 |
| HUD, retângulos | 406 | **3,31 ms** | 59,5 |
| loja, um por ponto | 4 649 | 9,75 ms | **29,9** |
| loja, retângulos | 1 463 | **5,64 ms** | **59,4** |

A cobertura é idêntica e está testada (`os_retangulos_cobrem_exatamente_os_pontos`):
nenhuma célula faltando, nenhuma coberta duas vezes. Os dois quadros comparados
dão **0 pixels diferentes** — a otimização não muda o desenho, muda o custo.

O guloso é **por glifo**, não por linha de texto: retângulos que atravessassem a
fresta entre letras mudariam a largura percebida de um caractere e um
`RETANGULOS` de 5 pontos poderia inchar 30 % e borrar a letra.

## D-031 — O benchmark mede submissão, não o intervalo do quadro

**Decisão:** `--benchmark` acumula o tempo do começo do quadro até **antes** de
`render.end()`, e reporta três números: FPS, trabalho por quadro, e o custo da UI
dividido em montar (CPU) e desenhar (submissão).

A primeira versão acumulava até depois da troca de buffers e reportava
**16,72 ms "de trabalho" com e sem UI** — exatamente o vblank do painel. O número
estava certo e a conclusão que ele permitia era falsa: parecia que o HUD era
grátis. Separando as duas metades, o custo aparece (4,32 → 3,31 ms depois de
D-030) e a diferença entre "otimizei" e "não mexi" fica visível.

A divisão montar/desenhar existe pelo mesmo motivo: são otimizações opostas
(alocação de `String` contra número de instâncias), e somadas elas não dizem
qual das duas atacar. Medido no R35S: montar 0,24 ms, desenhar 3,31 ms.

## D-032 — Modelos e texturas vêm de um gerador, e a prova é a contagem de cores

**Decisão:** os 15 modelos (`.obj`) e as 15 texturas (`.bmp`) são **gerados** por
`scripts/gerar-modelos.mjs`, determinístico e com modo `--conferir`. Nenhum
arquivo binário entra no repositório sem poder ser reproduzido.

Três razões, todas medidas:

1. **BMP e não PNG.** O `SDL2_image` do R35S já apareceu compilado sem
   `--enable-png` e `IMG_Load` responde *"PNG images are not supported"*
   (AGENTS.md §24). Um jogo com PNG no asset simplesmente não abriria lá.
2. **Reprodutibilidade.** `--conferir` reprova se qualquer arquivo divergir do
   gerador. Um OBJ editado à mão e perdido no histórico não tem como voltar.
3. **Sem ferramenta de modelagem.** Não há Blender nesta máquina, e uma figura
   de 40 vértices cabe em 20 linhas de laço.

**`load_obj` sem `vn` de propósito:** a engine deriva a normal da face em
`prepare_normals`, e é isso que dá o relevo que o shader `textured` desenha.
Emitir normais suavizadas esconderia as arestas que fazem o modelo ler como
sólido a 640×480.

**A verificação não é "o arquivo existe":**

| | cores distintas | pixels diferentes |
| --- | --- | --- |
| com modelos | **2 795** | **13,63 %** |
| sem modelos (cubos) | 113 | — |

E `tests/modelos.rs` carrega os 15 arquivos pelo **loader da engine** e mede
altura, largura e número de vértices. Um OBJ vazio ou com face invertida reprova
sem abrir janela — `load_obj` é puro, não toca GL.

**A horda sai instanciada por modelo** (`cena::agrupar`), não uma por inimigo:
12 inimigos custam **0,47 chamada/quadro** e 0,32 ms.

## D-033 — O `glam` do jogo segue o que o código da engine chama

**Decisão:** `glam = "0.34"` no jogo, e o `vendor/Cargo.toml` também re-pinado,
porque a engine usa `glam::camera::rh::proj::opengl::perspective` — API que só
existe a partir da 0.34.

O snapshot anterior estava em `0.29` e o código compilava **porque as duas
pontas estavam velhas juntas**: o vendor tinha a mesma versão do jogo. Atualizar
o vendor para o upstream e deixar o jogo em 0.29 falha com
`no function or associated item named 'perspective' found`, que manda procurar
defeito no jogo.

A regra é de mão única: **o pin segue o código que a engine chama**, nunca o
contrário. Um `glam` mais novo no jogo com uma engine antiga quebraria na
primeira chamada de matriz.

## D-034 — A `real-net` deixou de ser uma feature

**Decisão:** `net/host.rs`, `net/client.rs`, `net/sessao.rs` e `net/descoberta.rs`
compilam **sempre**. A feature `real-net` foi removida.

Rede atrás de `--features` é rede que o jogador não tem: o binário instalado no
aparelho foi compilado sem ela, e as duas funções `RealHost::novo` e
`RealClient::entrar` tinham erros de compilação **latentes que ninguém viu**
(`MAX_CLIENTS` em vez de `MAX_CLIENTES`, e o campo `tamanho` do
`HandshakeRecusado` ignorado). Um caminho que nunca é compilado não é código: é
texto.

O `stub-net` continua como feature porque ele é banco de prova (rede de
loopback determinística) e não caminho de produção.

## D-035 — O id do jogador vem de `cliente.jogador()`, nunca de uma mensagem

**Decisão:** o cliente lê o próprio id de `giochi_net::client::Cliente::jogador()`
e o copia para `state.local_id`; o braço `Mensagem::HandshakeOk { .. }` do jogo
foi **removido**.

O crate **consome** o `HandshakeOk` dentro de `processar()` (guarda o id e faz
`continue`), então aquela mensagem nunca chega ao jogo. O braço existia, o log
`handshake aceito` nunca saiu, e a consequência real foi medida com dois
processos: o cliente ficava com `local_id = 0`, que é **o id do host** — ele
dirigia o avatar do outro e via o próprio como remoto.

```text
antes    cliente: rede: jogador 0        (nunca chegava o handshake)
depois   cliente: rede: assumindo o jogador 1
         cliente: net: handshake aceito, jogador 1
```

O teste `id_zero_nunca_e_adotado_como_proprio` fixa a regra: 0 não é id de
cliente. É a mesma família do `year_label()` do launcher — **um valor plausível
no lugar do dado ausente esconde o defeito**.

## D-036 — `--teclas` injeta entrada; não chama o menu

**Decisão:** `--teclas` transforma a lista em `Tecla` e injeta **uma por quadro**
na entrada, com um quadro de soltura entre duas. Quem executa a ação é o laço.

A versão anterior chamava `Menu::entrada` antes do laço e descartava a `Acao`
devolvida. O sintoma medido:

```text
--teclas "a" -> tela partida · 60 quadros · 0 passo(s) de simulação
```

A tela mudava e nada começava — ferramenta que imprime um estado plausível sem
ter feito nada, a mesma classe do `--teclas` do launcher (AGENTS.md §24).

O **quadro de soltura** existe porque a repetição do menu é de `REPETICAO_S`
(0,22 s): a tecla seguinte, chegando 1/60 s depois, caía dentro da janela e era
engolida. Medido: `baixo,baixo` parava em "Multijogador" em vez de chegar a
"Configurações". Um dedo levanta o botão; a fila agora também.

`Tecla::Pausa` foi acrescentada porque `menu::Direcao` é o vocabulário **do
menu** e o START não está nele — sem ela, o caminho pausa → reiniciar → sair não
podia ser exercitado por linha de comando.

## D-037 — A captura lê os pixels antes da troca de buffers

**Decisão:** `--quadro` lê o frame **antes** de `render.end()`, nos dois caminhos
(menu e partida).

Com GLES/KMSDRM o `gl_swap_window` esvazia o back buffer, e ler depois grava um
BMP **preto**. O repositório contornava isso obrigando `SDL_RENDER_DRIVER=software`
na captura — um paliativo que custa a fidelidade do backend que roda de verdade.
Ler antes resolve na origem e vale para os dois backends.

É o mesmo motivo que fez `--benchmark` parar antes de `end()` (D-031): a troca de
buffers é o limite entre "o que o jogo fez" e "o que o painel mostrou".

## D-038 — A contagem de chamadas da horda é por quadro

**Decisão:** o relatório de `--benchmark` publica `horda … chamada(s)/quadro`,
média por quadro, e não o total acumulado.

A primeira versão imprimia `horda 0.32 ms em 1041 chamada(s)` — um total de 1800
quadros ao lado de um custo por quadro. Lê como "1041 chamadas num quadro", que é
**plausível e falso**: 12 inimigos desenham no máximo 3 lotes. É a mesma classe
do contador de efeitos do `launcher-teste` (AGENTS.md §30) e do contador de
desenhos do `neon-drift` (§38): um número que não diz o que mede.

Números por quadro misturados com totais são a versão numérica de "duas listas
que envelhecem diferente".
