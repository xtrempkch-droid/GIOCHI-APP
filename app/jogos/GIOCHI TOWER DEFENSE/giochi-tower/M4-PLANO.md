# M4 — plano de execução (e o que falta conferir antes de escrever)

Bloco M4 do prompt: *"Multiplayer real"*. Este arquivo existe porque o M4 não pode
ser escrito às cegas: ele depende de campos do `giochi-net` que ainda não foram
lidos, e não há compilador nesta máquina. Escrever agora produziria código com
nomes inventados — exatamente o erro que o M1 cometeu ao vendorizar o snapshot
velho do `giochi-defender` (ver `DECISIONS.md`, D-002).

## O que já está confirmado (lido no código, não suposto)

Fonte: `Giochi engine 3d/giochi-net/src/` (`main` = `49c5d63`).

| Item | Assinatura confirmada |
|---|---|
| `host::Host::novo` | `novo(porta: Option<u16>) -> std::io::Result<Self>` |
| `host::Host::processar` | `processar(orcamento: usize) -> Vec<(ClienteId, Mensagem)>` |
| `host::Host::definir_area` | `definir_area(id: ClienteId, raio: f32)` |
| `host::Host::enviar_global` | `enviar_global(dados: &[u8])` |
| `host::Host::tick_snapshot` | `tick_snapshot(inimigos: &[SnapshotInimigo]) -> bool` |
| `host::Host::enviar_jogador` | `enviar_jogador(j: ResumoJogador)` |
| `host::Host::ultimos_envios` | `ultimos_envios() -> &[(ClienteId, usize)]` |
| `client::Cliente::entrar` | `entrar(endereco: impl ToSocketAddrs) -> std::io::Result<Self>` |
| `client::Cliente::enviar_input` | `enviar_input(input: Mensagem) -> std::io::Result<()>` |
| `client::Cliente::processar` | `processar(orcamento: usize) -> Vec<Mensagem>` |
| `client::Cliente::{jogador, rtt_ms, ultimo_tick, estado}` | já lidos |
| `wire::Mensagem` variantes | `Handshake`, `HandshakeOk`, `HandshakeRecusado`, `Input`, `Jogador(ResumoJogador)`, `Inimigos{..}`, `Global{..}`, `ShopBuy{..}`, `Ping`, `Pong` |
| constantes | `VERSAO_PROTOCOLO = 1`, `DATAGRAMA_MAX = 1200`, `SNAPSHOT_HZ = 15`, `PORTA_PADRAO = 47721` |
| `aoi::{AreaDeInteresse, FiltroAoi}` | `nova(x, z, raio)`, `da_tela(cx, cz, meia_diagonal)`, `contem`, `filtrar` |

## O que falta ler antes de tocar em código (4 campos)

1. campos de `Mensagem::Input` — o jogo precisa preencher tick, id, eixos e flags
   sem inventar nomes;
2. campos de `Mensagem::Inimigos { .. }` — provavelmente tick + `Vec<SnapshotInimigo>`;
3. campos de `Mensagem::Global { .. }` — é onde o `GameMsg` codificado viaja como
   payload opaco (é o encaixe previsto em `DECISIONS.md`, D-027);
4. campos de `Mensagem::ShopBuy { .. }`.

São quatro leituras de 10 linhas; com elas o M4 é mecânico.

## Ordem de execução proposta

| # | Entrega | Depende de |
|---|---|---|
| 1 | `net/host.rs` + `net/client.rs` implementando `HostBackend`/`ClientBackend` sobre `Host`/`Cliente`, atrás da feature `real-net` | os 4 campos acima |
| 2 | `net/mod.rs`: fábrica de sessão que escolhe `stub` (default) × `real-net` (feature) | 1 |
| 3 | mDNS com `mdns-sd` (descoberta de sala na LAN) | 1 |
| 4 | Cliente autoritativo de verdade: `GameState::step` deixa de receber `&InputFrame` e passa a consumir só o que chega pelo fio (hoje é a simplificação declarada em D-028) | 1 |
| 5 | Predição + smoothing, usando `rtt_ms()` do cliente para o orçamento de interpolação | 4 |
| 6 | Teste de 4 clientes com `UdpSocket` local; medir RTT e banda por cliente (meta: RTT < 100 ms, ≤ 5 KB/s — GDD §20) | 1–5 |

## Como o M4 será medido (e não "achado")

- **Banda:** `ultimos_envios()` dá bytes por cliente; `DATAGRAMA_MAX = 1200` é o
  teto por datagrama e `SNAPSHOT_HZ = 15` é o teto de frequência. 15 × 1200 =
  18 KB/s se o AOI não filtrar; a meta de 5 KB/s só se sustenta com AOI agressiva
  e snapshot pequeno. Isso é medível no teste 6, não estimável.
- **RTT:** `Cliente::rtt_ms()` + `INTERVALO_PING = 2 s`.

## Dívidas que o M4 herda (registradas para não sumirem)

1. **Nada compilou ainda.** Não há `rustc`/`cargo` nesta máquina; 8.015 linhas
   escritas contra assinaturas lidas, nunca verificadas por compilador.
2. **`src/ui/` do M3 não existe.** HUD, minimapa e tela da loja pendentes: a loja
   compra (cursor + ataque), mas sem seletor na tela. A câmera falsa de 90° sobre
   o 3D (D-009) é o caminho previsto.
3. **`GameState::step` ainda recebe `&InputFrame`.** É o item 4 acima.

Recomendação antes de M4: rodar `cargo test` numa máquina com toolchain. O M4
multiplexa os mesmos padrões do M2 (adaptadores, orçamento de drenagem, AOI) e
herdaria qualquer erro estrutural não visto.
