# STATUS — GIOCHI Tower Defense

Atualizado ao fim do bloco M4 (parcial). Este arquivo é o resumo de uma página; o
raciocínio de cada decisão está em [`DECISIONS.md`](DECISIONS.md) e o plano do M4
em [`M4-PLANO.md`](M4-PLANO.md).

## Onde o projeto está

| Bloco | Estado | O que existe |
|---|---|---|
| M1 — fundação | **entregue** | engine vendorizada (main `49c5d63`), janela GLES/EGL, câmera isométrica, loop fixo 60 Hz, 10 configs RON validadas, entrada (gamepad SDL, teclado/mouse, touch emulado) |
| M2 — rede e combate | **entregue** | protocolo `postcard`, AOI, stub loopback, 5 inimigos com IA, spawner por onda com teto de vivos, torre, placar, fases |
| M3 — bosses, nemesis, loja | **entregue (sem desenho de UI)** | 4 bosses com fases, Coronel por tier com item exclusivo, loja validada no host, `src/ui/` em modelos (HUD, minimapa, loja) |
| M4 — multiplayer real | **1 de 6 itens** | `net/host.rs` e `net/client.rs` sobre `giochi-net` (feature `real-net`) |
| M5 — handheld | não iniciado | — |
| M6 — polimento | não iniciado | — |

Números: **27 arquivos Rust, 8.590 linhas**, 4 suítes de teste
(`net_stub`, `wave_scaling`, `shop_validation`, `nemesis_evolution`),
`node scripts/validate-app.mjs --all` → 11 apps válidos.

## O que NÃO foi feito, e por quê

1. **Nada compilou.** Não há `rustc`/`cargo` na máquina de geração. Todo o código
   foi escrito contra assinaturas **lidas** na engine e no `giochi-net`, com
   arquivo e linha citados nos comentários — não adivinhadas. A compilação fica
   para a máquina com toolchain (ver `DECISIONS.md`, D-020).
2. **`src/ui/` não desenha.** A engine 0.3.1 não tem texto nem 2D; o HUD em tela
   exige a câmera falsa de 90° sobre o 3D (D-009). Existe o modelo pronto
   (`ui::build` devolve `UiFrame`), falta a camada de desenho (~80 linhas em
   `engine_bridge/render.rs`).
3. **M4 incompleto:** falta a fábrica de sessão (stub × real), mDNS, autoridade
   real do host (D-028), predição/smoothing e o teste de 4 clientes.

## Ordem recomendada ao abrir numa máquina com Rust

```bash
cd "app/jogos/GIOCHI TOWER DEFENSE/giochi-tower"
cargo check --all-targets          # 1º: a fila de erros de assinatura
cargo test                         # 2º: as 4 suítes
cargo clippy --all-targets -- -D warnings
```

Onde é mais provável quebrar, em ordem: borrows em `game/state.rs`
(`step_player`, `step_shop`), caminho de `ron::error::SpannedError`, alias
`ClienteId` em `net/host.rs`, e as features do `sdl2`.

## O que NÃO fazer sem medir

- Não portar para a ABI C (`libgiochi.so`) "porque o prompt pede": o jogo precisa
  de `giochi-net`, que **não tem superfície C**. Ver D-002.
- Não reativar `rand` antes do M5: o spawn determinístico é o que torna
  `--frames` comparável (D-024).
- Não tratar "compila" como "roda": a regra do repositório vale aqui.
