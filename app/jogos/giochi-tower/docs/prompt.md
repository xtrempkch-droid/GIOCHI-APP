# PROMPT MESTRE — GIOCHI TOWER DEFENSE

## 1. Persona e objetivo

Atue como **Engenheiro Sênior de Redes + Game Designer + Arquiteto Rust**, especializado em jogos multiplayer P2P, otimização para hardware ARM64 de baixo consumo e integração com engines customizadas.

Sua tarefa é **gerar o projeto completo e compilável** de um jogo chamado **GIOCHI Tower Defense** — um Tower Defense 3D co-op PvE (2–4 jogadores) — para a stack GIOCHI, conforme a especificação abaixo. Você tem liberdade para tomar decisões de implementação desde que respeite **todas as restrições e decisões travadas** neste documento.

**Regra de ouro:** este documento contém decisões **já tomadas**. Não as questione nem as substitua. Se algo não estiver especificado, você pode decidir — mas registre a decisão em um arquivo `DECISIONS.md` na raiz do projeto.

---

## 2. Stack tecnológica (não negociável)

| Camada | Tecnologia |
|---|---|
| **Engine** | **GIOCHI-ENGINE-3D** — https://github.com/xtrempkch-droid/GIOCHI-ENGINE-3D |
| **Engine detalhes** | Rust + SDL2 + OpenGL ES 2.0. ABI C estável (`libgiochi.so`). Handles opacos `u64`. Tipos `#[repr(C)]`. Contrato em `docs/C_ABI_CONTRACT.md` do repositório. |
| **Binding de script** | Lua 5.4 (crate `giochi-engine-lua`). Opcional — o jogo pode ser Rust puro. |
| **Sistema operacional** | **GIOCHI-OS** — https://github.com/xtrempkch-droid/GIOCHI-OS (Linux ARM64) |
| **Build system** | **GIOCHI-SYSTEM** — https://github.com/xtrempkch-droid/GIOCHI-SYSTEM (Buildroot 2024.02) |
| **Linguagem do jogo** | Rust (edition 2021) |
| **Alvo de compilação** | `aarch64-unknown-linux-gnu` |
| **Hardware mínimo** | **RK3326 + 1 GB RAM** (Cortex-A35 @ 1.5 GHz, Mali-G31 MP2, DDR3) |
| **Empacotamento** | `/opt/giochi/apps/giochi-tower/` com manifesto de permissão de rede |

**Restrições de dependências Rust:**
- **Permitido:** `serde`, `postcard` (ou `bincode`), `mdns-sd`, `libc`, `thiserror`, `log`/`env_logger`, `ron` (para configs), `rand`, `glam` (matemática 3D), `bytemuck`.
- **Proibido:** `tokio`, `async-std`, `quinn`, qualquer runtime async multi-thread, qualquer dependência que não compile limpo em `aarch64-unknown-linux-gnu` com Buildroot. Rede é síncrona com `std::net::UdpSocket`.
- **Justificativa:** cross-compile para ARM64 minimalista + loop de timestep fixo da engine não pode ser bloqueado por I/O.

---

## 3. Estado atual dos bloqueios (contexto)

| Item | Status | O que fazer no código |
|---|---|---|
| **Gamepad** | ✅ Já exposto na ABI C (`giochi_gamepad_connected`, `giochi_gamepad_axis`, `giochi_gamepad_button_down`) | Usar diretamente. Nada a implementar. |
| **Touch** | 🟡 3ª opção. `SDL_Finger` **não** está na ABI C. | Emular via eventos de mouse da SDL (já na ABI). Prioridade baixa. |
| **Rede P2P** | 🔴 **Ausente na engine**. Issue aberta: `feat(net): giochi-net com AOI`. | **Implementar stub local** com a mesma assinatura planejada da ABI C (`net_*`). Quando a issue for mergeada, troca-se o stub pela implementação real **sem tocar no jogo**. |
| **Mapa 4×4** | ✅ Definido | 16 telas totais, torre central. |
| **Dois modos** | ✅ Definido | Campanha (6 ondas, ~42 min) e Endless (pós-onda 6). |

**O stub de rede é obrigatório.** Ele deve:
- Ter a **mesma assinatura** das funções planejadas: `net_host_create`, `net_client_join`, `net_send_input`, `net_set_camera_pos`, `net_poll_snapshot`, `net_poll_snapshot_filtered`.
- Funcionar como **host + client no mesmo processo** (loopback interno), permitindo partida single-player local.
- Ser isolado em um módulo (`src/net/stub.rs`) com feature flag `stub-net` (ativada por padrão). Quando a implementação real existir, basta desativar a flag.

---

## 4. Especificação do jogo (GDD resumido e travado)

### 4.1 Visão geral

- **Gênero:** Tower Defense 3D co-op PvE, top-down isométrico.
- **Jogadores:** 2 a 4, todos aliados. Host-autoritativo.
- **Objetivo:** defender a **Torre central** (o "refém/vítima") o máximo de tempo, acumular pontos.
- **Tema:** Starship Troopers (bugs aracnídeos). Alternativa medieval = reskin puro (mesma mecânica).
- **Duração Campanha:** 6 ondas × 7 min (6 min combate + 1 min loja) = **42 min**.
- **Endless:** após onda 6, ciclos continuam até derrota; toda onda ≥ 7 tem boss escalado.

### 4.2 Mapa e câmera

- **Mapa:** 4×4 telas (16 totais). Torre nas 4 telas centrais. Spawns nas bordas externas.
- **Câmera:** top-down isométrica **por jogador**, independente. Zoom fixo.
- **AOI por jogador:** raio ~1,5 × meia-diagonal da tela.
- **Minimapa:** torre, aliados em tempo real, último avistamento do nemesis defasado ~5 s. Inimigos comuns só via visão direta.

### 4.3 Classes jogáveis

| Classe | HP | Dano | Habilidade 1 | Habilidade 2 |
|---|---|---|---|---|
| Soldado | 100 | Médio | Dash | Granada |
| Guerreiro | 180 | Alto melee | Escudo (50% 3 s) | Provocar (aggro raio) |
| Sniper | 80 | Alto distância | Mira precisa | Tiro perfurante |
| Médico | 90 | Baixo | Cura em área | Reanimar aliado |

Respawn: 10 s na torre. Torre cair = game over.

### 4.4 Inimigos comuns

| Tipo | Comportamento | HP base | Dano base | Gold |
|---|---|---|---|---|
| Grunt | Caminha reto | 30 | 5 | 1 |
| Tank | Lento, alto HP | 150 | 15 | 3 |
| Ranged | Para e atira | 60 | 8 | 2 |
| Swarm | Rápido, frágil, grupo | 15 | 3 | 1 |
| Flying | Ignora obstáculos | 40 | 6 | 2 |

**Escala por onda N:** HP ×(1 + 0.15·N), dano ×(1 + 0.10·N), velocidade ×(1 + 0.05·N).

### 4.5 Estrutura de ondas — Campanha

| Onda | Pico vivo | Boss |
|---|---|---|
| 1 | ~30 | — |
| 2 | ~50 | — |
| 3 | ~60 | **Tanker Bug** |
| 4 | ~80 | **Plasma Bug** |
| 5 | ~100 | **Brain Bug** |
| 6 | ~120 | **Queen** (multi-fase, final) |

Intervalo entre ondas: 1 min de loja + fortificação. Jogadores mortos ressuscitam.

### 4.6 Bosses

**Tanker Bug (Onda 3)** — HP 2500, dano 40 (stomp AoE raio 3 m a cada 15 s), imune a knockback, lento. Recompensa 50 gold / 500 score.

**Plasma Bug (Onda 4)** — HP 2000, dano 30 (projétil em arco), teleporta 5 m quando jogador se aproxima, mantém distância. Recompensa 50/500.

**Brain Bug (Onda 5)** — HP 3000, dano 20 melee fraco, invoca 4 adds a cada 20 s, buff +30% dano em aliados próximos, foge. Recompensa 50/500.

**Queen (Onda 6, final)** — HP 6000, dano 60 slam / 25 projétil.
- Fase 1 (100–60%): spawna filhotes a cada 10 s, slam AoE.
- Fase 2 (60–30%): projéteis em leque, filhotes mais rápidos.
- Fase 3 (30–0%): enrage, velocidade +50%, slam duplo.
Recompensa 200 gold / 2000 score, vitória da Campanha.

### 4.7 Nemesis — "O Coronel"

Inimigo único. Spawn na onda 1 em borda aleatória. **Se morrer, não respawna.**

| Tier | Sobreviveu até | Ganho |
|---|---|---|
| 0 | Spawn (onda 1) | HP 300, dano 20 |
| 1 | Fim onda 1 | + Dash |
| 2 | Fim onda 2 | + Invoca 2 adds a cada 30 s |
| 3 | Fim onda 3 | + 30% redução de dano |
| 4 | Fim onda 4 | + Ataque à distância |
| 5 | Fim onda 5 | **HP 5000, dano 80 — mais forte que a Queen, mas matável** |

**Recompensa por matar:** score = tier × 200 + **item exclusivo** (não vendido na loja).
**Penalidade por ignorar:** dano massivo na torre se alcançá-la; pressão em jogadores isolados.

**Rede:** posição em broadcast **defasada ~5 s** para o minimapa; posição real só para quem está na AOI dele.

### 4.8 Torre

| Atributo | Base | Máx. loja |
|---|---|---|
| HP | 5000 | 8000 |
| Armadura | 5 | 15 |
| Regen | 0/s | 10/s |

### 4.9 Loja e economia

**Fonte de gold:** Grunt 1, Tank 3, Ranged 2, Swarm 1, Flying 2, Boss 50, Nemesis tier×200.

| Item | Efeito | Preço |
|---|---|---|
| Rifle melhorado | +20% dano | 80 |
| Scope | +30% alcance | 60 |
| Colete | +30 HP máx. | 70 |
| Poção de cura | +50 HP | 20 |
| Dash extra | +1 carga | 100 |
| Reparo torre | +500 HP | 50 |
| Upgrade HP torre | +500 máx. | 150 |
| Upgrade armadura torre | +2 | 100 |
| Regen torre | +2/s | 200 |

**Validação de compra sempre no host.**

### 4.10 Pontuação

Kill comum 10 · Boss 500 · Nemesis tier×200 · Onda limpa 100×N · HP torre preservado 1 pt/10 HP.

### 4.11 Controles

**Prioridade:** Gamepad (obrigatório) > Teclado/mouse (dev) > Touch (plus).

**Gamepad twin-stick:**
- Analógico esq. = movimento
- Analógico dir. = mira/tiro
- A/✕ = atacar
- B/○ = habilidade 1
- X/□ = habilidade 2
- Y/△ = item rápido
- D-pad ↑ = abrir loja (só no intervalo)
- Start = pausar

**Touch (3ª opção):** joystick virtual + botões + tap-to-aim. Emular via eventos de mouse SDL até extensão `SDL_Finger`.

**Hot-plug:** entrada no meio da partida; pausa local se desconectar.

---

## 5. Arquitetura de rede (obrigatória)

**Host-autoritativo.** Host simula tudo. Clientes enviam input, recebem snapshots filtrados.

**Descoberta:** `mdns-sd` na LAN.

**Transporte:** UDP síncrono (`std::net::UdpSocket`). Serialização `postcard`.

**Protocolo (enum binário):**

| Variante | Direção | Campos |
|---|---|---|
| `Handshake` | C→H | player_id, version, controller_type |
| `Input` | C→H | tick, player_id, move_dir, aim_dir, action_flags, camera_pos |
| `PlayerState` | H→C broadcast | player_id, pos, hp, abilities, score |
| `EnemySnapshot` | H→C **unicast AOI** | tick, enemies: Vec<{id, pos, hp, kind, boss_phase}> |
| `WaveState` | H→C broadcast | number, timer, is_boss, boss_id, boss_hp_pct, mode (Campaign/Endless) |
| `TowerState` | H→C broadcast | hp, max_hp, armor |
| `NemesisState` | H→C broadcast | id, last_seen_pos (defasado ~5s), wave_survived, tier |
| `ShopOpen` / `ShopBuy` | H→C / C→H | items, gold / player_id, item_id |
| `Ping` / `Pong` | ambos | timestamp |

**Interest Management (AOI):**
- Cliente envia `camera_pos` junto ao `Input`.
- Host filtra inimigos por AOI (~1,5 × meia-diagonal da tela) e envia **unicast**.
- Estado global (torre, onda, boss, nemesis defasado) em broadcast.

**Requisitos de performance do host:**
- IA de inimigos a 20 Hz (desacoplada do render 60 Hz).
- Limite duro de inimigos vivos configurável.
- Pathfinding: A* em grid com cache **ou** steering behavior (config por perfil).
- Rede **nunca** bloqueia o loop de timestep fixo da engine.

---

## 6. Perfis de host

| Perfil | Alvo | Configuração |
|---|---|---|
| `Full` | RK3566/RK3588/PC ARM64 | ~150 inimigos, IA 20 Hz, A* grid, AOI padrão |
| `Balanced` | Intermediário | ~100 inimigos, IA 15 Hz, A* cache, AOI padrão |
| `Rk3326` (emergência) | RK3326 1 GB | **~40–50 inimigos**, steering, IA 10–15 Hz, AOI agressiva, VFX mínimos |

Selecionável no menu por variável de config. **RK3326 é cliente por padrão.**

---

## 7. Estrutura de projeto (obrigatória)

```
giochi-tower/
├── Cargo.toml
├── build.rs
├── DECISIONS.md                 # decisões tomadas por você durante a geração
├── README.md
├── config/
│   ├── waves_campaign.ron
│   ├── waves_endless.ron
│   ├── items.ron
│   ├── bosses.ron
│   ├── nemesis.ron
│   ├── enemies.ron
│   ├── classes.ron
│   └── host_profiles.ron
├── assets/
│   ├── models/                  # placeholders low-poly (.obj ou .gltf)
│   ├── textures/
│   ├── audio/
│   └── fonts/
├── src/
│   ├── main.rs                  # bootstrap, init engine, loop
│   ├── lib.rs
│   ├── engine_bridge/
│   │   ├── mod.rs               # wrappers FFI seguros sobre ABI C
│   │   ├── input.rs             # gamepad via giochi_gamepad_*
│   │   ├── render.rs            # cena 3D, câmera
│   │   ├── audio.rs
│   │   └── touch.rs             # emulação via mouse (stub)
│   ├── net/
│   │   ├── mod.rs               # trait NetBackend + fachada
│   │   ├── stub.rs              # implementação loopback single-player
│   │   ├── protocol.rs          # enum de mensagens (postcard)
│   │   ├── host.rs              # lógica host (quando net real existir)
│   │   ├── client.rs            # lógica client
│   │   └── aoi.rs               # cálculo de Interest Management
│   ├── game/
│   │   ├── mod.rs
│   │   ├── state.rs             # GameState, tick fixo 60 Hz
│   │   ├── player.rs            # classes, input, prediction
│   │   ├── enemy.rs             # tipos, spawn, IA
│   │   ├── boss.rs              # 4 bosses com fases
│   │   ├── nemesis.rs           # evolução por tier
│   │   ├── tower.rs             # HP, armadura, regen
│   │   ├── waves.rs             # spawner, timer, transição
│   │   ├── shop.rs              # itens, compra, validação host
│   │   ├── score.rs
│   │   └── camera.rs            # câmera isométrica por jogador
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── hud.rs               # barras, gold, timer de onda
│   │   ├── minimap.rs           # torre, aliados, nemesis defasado
│   │   └── shop_ui.rs
│   ├── config.rs                # loaders RON (serde)
│   └── errors.rs
└── tests/
    ├── net_stub.rs              # loopback host+client
    ├── wave_scaling.rs
    ├── nemesis_evolution.rs
    └── shop_validation.rs
```

---

## 8. Ordem de geração (siga nesta sequência, entregando em blocos)

Gere o projeto em **6 blocos**, um por vez, e **pause para confirmação** após cada bloco. Em cada bloco, entregue todos os arquivos completos (não omita código com `// ...` — use implementações reais mesmo que simples).

### Bloco M1 — Fundação
1. `Cargo.toml` com dependências permitidas, feature `stub-net` default.
2. `build.rs` (se necessário para linkar `libgiochi`).
3. `src/main.rs` + `src/lib.rs` — bootstrap, init da engine, loop de timestep fixo 60 Hz.
4. Todos os arquivos em `config/` com valores **exatamente** conforme o GDD.
5. `src/config.rs` com structs `serde` que refletem os RONs.
6. `src/engine_bridge/input.rs` — leitura de gamepad via ABI C (`giochi_gamepad_*`), teclado e mouse como fallback; touch via mouse stub.
7. `src/game/state.rs` + `src/game/camera.rs` — GameState vazio, câmera isométrica funcional.
8. `DECISIONS.md` com decisões tomadas.

**Aceitação M1:** compila para `aarch64-unknown-linux-gnu`, roda em modo "sala vazia" com câmera isométrica e input lido do gamepad.

### Bloco M2 — Rede stub + combate básico
1. `src/net/protocol.rs` — enum completo com serde/postcard.
2. `src/net/stub.rs` — loopback host+client no mesmo processo, feature `stub-net`.
3. `src/net/mod.rs` + `src/net/aoi.rs` — fachada + cálculo de AOI.
4. `src/game/player.rs` — 4 classes, movimento, tiro/melee, prediction local.
5. `src/game/enemy.rs` — 5 tipos, spawn, steering simples, pathfinding plugável.
6. `src/game/waves.rs` — spawner lendo `waves_campaign.ron`, escala por onda.
7. `src/game/tower.rs` — HP/armadura/regen, dano recebido.
8. `src/game/score.rs`.
9. `tests/net_stub.rs` — host+client trocam Input/EnemySnapshot sem deadlock.

**Aceitação M2:** single-player local funcional — soldado se move, inimigos spawnam e atacam a torre, contagem de onda avança, derrota quando torre cai.

### Bloco M3 — Bosses, nemesis, loja
1. `src/game/boss.rs` — Tanker, Plasma, Brain, Queen com fases.
2. `src/game/nemesis.rs` — evolução por tier, item exclusivo ao matar.
3. `src/game/shop.rs` — validação host, aplica efeitos.
4. `src/ui/shop_ui.rs` + `src/ui/hud.rs` + `src/ui/minimap.rs`.
5. `tests/nemesis_evolution.rs`, `tests/shop_validation.rs`, `tests/wave_scaling.rs`.

**Aceitação M3:** campanha completa jogável em single-player (6 ondas, 4 bosses, nemesis, loja funcional, vitória/derrota).

### Bloco M4 — Multiplayer real
1. Ativar `giochi-net` real (quando issue mergeada). **Antes disso**, escrever a camada `src/net/host.rs` e `src/net/client.rs` já pronta para a ABI real, mas usando o stub por padrão.
2. `mdns-sd` para descoberta LAN.
3. Reconciliação cliente (predição + smoothing).
4. AOI ativa — host envia `EnemySnapshot` unicast.
5. Teste de integração: 4 clientes LAN (simulados com `UdpSocket` local).

**Aceitação M4:** 4 jogadores em rede local com RTT < 100 ms, banda ≤ 5 KB/s por cliente.

### Bloco M5 — Handheld / perfis
1. `config/host_profiles.ron` + seleção no menu.
2. Perfil `Rk3326`: teto de inimigos 40–50, steering, IA 10–15 Hz, AOI agressiva, VFX mínimos.
3. Benchmark embutido: relatório de FPS vs. inimigos vivos, salvo em `/tmp/giochi-tower-bench.txt`.
4. Empacotamento: script `package.sh` que copia binário + assets + manifesto para `apps/giochi-tower/`.

**Aceitação M5:** roda em RK3326 como cliente com 30 FPS estáveis; benchmark de host gera relatório.

### Bloco M6 — Polimento
1. Balanceamento fino (tuning em RON, sem recompilar).
2. Modo Endless.
3. Áudio: trilha, stingers de boss/nemesis.
4. Arte: placeholders low-poly com silhuetas distintas.
5. README com instruções de build, deploy e benchmark.

**Aceitação M6:** campanha e Endless completos, arte/áudio funcionais, README finalizado.

---

## 9. Critérios de aceitação globais

- Compila sem warnings para `aarch64-unknown-linux-gnu`.
- Nenhuma dependência proibida (seção 2).
- Loop de timestep fixo 60 Hz **nunca** bloqueado por rede.
- Todas as configs em RON — jogador pode tunar sem recompilar.
- Stub de rede substituível pela implementação real sem mudar o jogo.
- Testes das 4 suítes (`tests/`) passam.
- `DECISIONS.md` documenta toda decisão não coberta por este prompt.

---

## 10. O que NÃO fazer

- ❌ Não usar `tokio`, async/await ou runtime async.
- ❌ Não usar engine genérica (Bevy, Fyrox, Godot). Use **apenas** a ABI C da GIOCHI-ENGINE-3D.
- ❌ Não implementar PvP, anti-cheat complexo ou servidor dedicado.
- ❌ Não adicionar `SDL_Finger` — touch é emulado via mouse até extensão futura.
- ❌ Não tornar `giochi-net` obrigatório no build — feature `stub-net` deve ser default.
- ❌ Não hardcodar valores de balanceamento — todos em RON.
- ❌ Não bloquear o loop principal com I/O de rede.
- ❌ Não alterar decisões travadas (seções 4, 5, 6). Se algo parecer errado, registre em `DECISIONS.md` mas **implemente como especificado**.

---

## 11. Formato de entrega

Para cada bloco (M1 a M6):

1. **Lista de arquivos** a criar/modificar.
2. **Conteúdo completo** de cada arquivo em blocos de código (sem omitir código).
3. **Comandos de build/teste** para validar o bloco.
4. **Notas** — decisões tomadas, riscos, pontos abertos.
5. **Aguardar confirmação** antes do próximo bloco.

Comece pelo **Bloco M1** agora. Antes de gerar código, faça um breve resumo (≤ 15 linhas) do que vai entregar em M1 e confirme que entendeu as restrições. Depois entregue M1 completo.