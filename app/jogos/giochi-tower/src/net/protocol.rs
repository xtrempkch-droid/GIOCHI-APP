//! Protocolo do jogo.
//!
//! # Duas camadas, de propósito
//!
//! O `giochi-net` (crate da engine) transporta e filtra por AOI: as mensagens
//! dele têm **payload opaco** para tudo que é regra de jogo, e tipos `repr`-like
//! só para o que ele precisa para rotear (id, posição, vida, tipo, fase). O
//! comentário do crate é explícito: *"a engine transporta, o jogo interpreta"*.
//!
//! Este módulo é a metade que interpreta. Os snapshots de inimigo usam
//! `giochi_net::wire::SnapshotInimigo` — o mesmo tipo que o host entrega em
//! `Host::tick_snapshot`, e o mesmo que o AOI do crate filtra. Duplicar esse
//! formato aqui criaria duas verdades sobre o mesmo dado.
//!
//! # Serialização
//!
//! `postcard` nos payloads de regra: formato compacto, sem alocação no decode,
//! `no_std`-friendly. O prompt §5 pede exatamente isso, e o orçamento de banda
//! (≤ 5 KB/s por cliente, GDD §20) é o que justifica: um `Input` cabe em ~20
//! bytes, então 20 Hz de input custa ~400 B/s.
//!
//! # Versionamento
//!
//! [`PROTOCOL_VERSION`] é o do `giochi-net` (`VERSAO_PROTOCOLO`), não um número
//! próprio: quem recusa handshake por versão é o crate, e um segundo número aqui
//! só criaria a chance de os dois discordarem.

use serde::{Deserialize, Serialize};

/// Versão do protocolo, igual à do `giochi-net`.
pub const PROTOCOL_VERSION: u16 = giochi_net::VERSAO_PROTOCOLO;

/// Teto do datagrama, igual ao do `giochi-net` (MTU seguro).
pub const DATAGRAM_MAX: usize = giochi_net::DATAGRAMA_MAX;

/// Frequência de snapshot do host (GDD §5 / crate).
pub const SNAPSHOT_HZ: u32 = giochi_net::SNAPSHOT_HZ;

// Flags de ação no `Input`. Um u16 de bits, em vez de seis `bool`:
// seis bools ocupam 6 bytes, e este campo viaja 20 vezes por segundo por jogador.
/// Atacar / atirar.
pub const ACTION_ATTACK: u16 = 1 << 0;
/// Habilidade 1.
pub const ACTION_ABILITY1: u16 = 1 << 1;
/// Habilidade 2.
pub const ACTION_ABILITY2: u16 = 1 << 2;
/// Item rápido.
pub const ACTION_ITEM: u16 = 1 << 3;
/// Abrir loja.
pub const ACTION_SHOP: u16 = 1 << 4;
/// Pausar (só o host decide, mas o pedido viaja).
pub const ACTION_PAUSE: u16 = 1 << 5;

/// Modo da partida, no formato que atravessa a rede.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetMode {
    /// Campanha de 6 ondas.
    Campaign,
    /// Ondas infinitas.
    Endless,
}

/// Mensagem de regra de jogo.
///
/// As variantes seguem a tabela do prompt §5. `EnemySnapshot` **não** está aqui:
/// o snapshot de inimigos é `Vec<giochi_net::wire::SnapshotInimigo>`, que o crate
/// já sabe codificar e filtrar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GameMsg {
    /// C→H: apresentação do cliente.
    Handshake {
        /// Id proposto (o host pode reatribuir).
        player_id: u8,
        /// Versão do protocolo do cliente.
        version: u16,
        /// Tipo de controle: 0 desconhecido, 1 gamepad, 2 teclado, 3 touch.
        controller_type: u8,
    },
    /// C→H: a intenção do jogador para este tick.
    Input {
        /// Tick a que o input se refere.
        tick: u64,
        /// Quem mandou.
        player_id: u8,
        /// Direção de movimento, `[-1,1]` por eixo, relativa à câmera.
        move_dir: [f32; 2],
        /// Direção de mira, `[-1,1]` por eixo.
        aim_dir: [f32; 2],
        /// Bits de [`ACTION_ATTACK`] e companhia.
        action_flags: u16,
        /// Posição da câmera no chão — é o que define a AOI do cliente.
        camera_pos: [f32; 2],
    },
    /// H→C: resumo de um jogador.
    PlayerState {
        /// Quem.
        player_id: u8,
        /// Posição no chão.
        pos: [f32; 2],
        /// Vida atual (unidades cruas; o cliente divide pelo máximo).
        hp: u16,
        /// Vida máxima.
        max_hp: u16,
        /// Pontuação.
        score: u32,
        /// Vivo.
        alive: bool,
    },
    /// H→C: estado da onda.
    WaveState {
        /// Número da onda.
        number: u32,
        /// Tempo restante da fase, em milissegundos.
        timer_ms: u32,
        /// Há boss vivo.
        is_boss: bool,
        /// Tipo do boss (0 = nenhum; 1..4 na ordem do GDD §9).
        boss_id: u8,
        /// Vida do boss em porcentagem.
        boss_hp_pct: u8,
        /// Modo da partida.
        mode: NetMode,
    },
    /// H→C: estado da torre.
    TowerState {
        /// HP atual.
        hp: u16,
        /// HP máximo.
        max_hp: u16,
        /// Armadura.
        armor: u16,
    },
    /// H→C: nemesis. A posição é o **último avistamento**, defasado ~5 s
    /// (GDD §10) — nunca a posição real.
    NemesisState {
        /// Id do nemesis no mundo.
        id: u16,
        /// Último avistamento conhecido.
        last_seen: [f32; 2],
        /// Quantas ondas sobreviveu.
        wave_survived: u32,
        /// Tier atual.
        tier: u8,
    },
    /// H→C: a loja abriu.
    ShopOpen {
        /// Gold do jogador.
        gold: u32,
        /// Preços dos 9 itens, na ordem do GDD §12 (0 = indisponível).
        prices: [u16; 9],
    },
    /// C→H: pedido de compra. O host valida (GDD §12).
    ShopBuy {
        /// Quem está comprando.
        player_id: u8,
        /// Índice do item na tabela.
        item_id: u8,
    },
    /// Ambos: sonda de latência.
    Ping {
        /// Marca de tempo de quem mandou (ms).
        timestamp: u32,
    },
    /// Ambos: resposta da sonda.
    Pong {
        /// Marca de tempo devolvida.
        timestamp: u32,
    },
}

impl GameMsg {
    /// Codifica a mensagem para transporte.
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("GameMsg é serializável por construção")
    }

    /// Decodifica uma mensagem, rejeitando lixo e truncamento.
    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }

    /// Tamanho codificado, para conferir o orçamento de banda.
    pub fn encoded_len(&self) -> usize {
        self.encode().len()
    }

    /// Converte a entrada local num `Input` de rede.
    pub fn input(
        tick: u64,
        player_id: u8,
        frame: &crate::engine_bridge::InputFrame,
        camera_pos: [f32; 2],
    ) -> Self {
        GameMsg::Input {
            tick,
            player_id,
            move_dir: [frame.move_dir.x, frame.move_dir.y],
            aim_dir: [frame.aim_dir.x, frame.aim_dir.y],
            action_flags: action_flags(frame),
            camera_pos,
        }
    }
}

/// Traduz os botões segurados para o bitmap de ações.
///
/// Só o **estado segurado** vai na rede; borda ("apertou agora") é derivada no
/// host comparando com o input do tick anterior. Mandar bordas por UDP faria o
/// host perder a transição a cada pacote perdido.
pub fn action_flags(frame: &crate::engine_bridge::InputFrame) -> u16 {
    let mut flags = 0u16;
    let buttons = frame.buttons;
    if buttons.attack {
        flags |= ACTION_ATTACK;
    }
    if buttons.ability1 {
        flags |= ACTION_ABILITY1;
    }
    if buttons.ability2 {
        flags |= ACTION_ABILITY2;
    }
    if buttons.quick_item {
        flags |= ACTION_ITEM;
    }
    if buttons.open_shop {
        flags |= ACTION_SHOP;
    }
    if buttons.pause {
        flags |= ACTION_PAUSE;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_bridge::input::{Axis, Buttons, InputFrame};

    fn sample_messages() -> Vec<GameMsg> {
        vec![
            GameMsg::Handshake {
                player_id: 2,
                version: PROTOCOL_VERSION,
                controller_type: 1,
            },
            GameMsg::Input {
                tick: 12_345,
                player_id: 1,
                move_dir: [0.5, -0.5],
                aim_dir: [1.0, 0.0],
                action_flags: ACTION_ATTACK | ACTION_ABILITY1,
                camera_pos: [12.5, -30.25],
            },
            GameMsg::PlayerState {
                player_id: 3,
                pos: [1.0, 2.0],
                hp: 90,
                max_hp: 100,
                score: 1234,
                alive: true,
            },
            GameMsg::WaveState {
                number: 6,
                timer_ms: 240_000,
                is_boss: true,
                boss_id: 4,
                boss_hp_pct: 61,
                mode: NetMode::Campaign,
            },
            GameMsg::TowerState {
                hp: 5000,
                max_hp: 5000,
                armor: 5,
            },
            GameMsg::NemesisState {
                id: 7,
                last_seen: [-40.0, 12.0],
                wave_survived: 3,
                tier: 3,
            },
            GameMsg::ShopOpen {
                gold: 250,
                prices: [80, 60, 70, 20, 100, 50, 150, 100, 200],
            },
            GameMsg::ShopBuy {
                player_id: 1,
                item_id: 6,
            },
            GameMsg::Ping { timestamp: 999 },
            GameMsg::Pong { timestamp: 999 },
        ]
    }

    #[test]
    fn every_variant_survives_a_roundtrip() {
        for msg in sample_messages() {
            let bytes = msg.encode();
            let back = GameMsg::decode(&bytes).expect("decodifica");
            assert_eq!(back, msg, "ida e volta de {msg:?}");
        }
    }

    #[test]
    fn an_input_fits_in_a_tiny_datagram() {
        let msg = GameMsg::Input {
            tick: 60 * 60 * 42,
            player_id: 3,
            move_dir: [1.0, -1.0],
            aim_dir: [-0.25, 0.75],
            action_flags: ACTION_ATTACK,
            camera_pos: [-79.5, 79.5],
        };
        let len = msg.encoded_len();
        // 20 Hz × ~32 B = 640 B/s por cliente, muito abaixo dos 5 KB/s do GDD §20.
        assert!(len <= 32, "Input com {len} bytes");
        assert!(len < DATAGRAM_MAX);
    }

    #[test]
    fn garbage_and_truncation_are_errors_not_panics() {
        assert!(GameMsg::decode(&[]).is_err());
        assert!(GameMsg::decode(&[0xff; 64]).is_err());
        let full = GameMsg::Input {
            tick: 1,
            player_id: 0,
            move_dir: [0.0, 0.0],
            aim_dir: [0.0, 0.0],
            action_flags: 0,
            camera_pos: [0.0, 0.0],
        }
        .encode();
        assert!(GameMsg::decode(&full[..full.len() - 1]).is_err(), "truncado");
    }

    #[test]
    fn the_action_bitmap_matches_the_buttons() {
        let frame = InputFrame {
            buttons: Buttons {
                attack: true,
                ability2: true,
                pause: true,
                ..Buttons::default()
            },
            ..InputFrame::default()
        };
        let flags = action_flags(&frame);
        assert_eq!(
            flags & ACTION_ATTACK,
            ACTION_ATTACK,
            "atacar marcado"
        );
        assert_eq!(flags & ACTION_ABILITY2, ACTION_ABILITY2);
        assert_eq!(flags & ACTION_PAUSE, ACTION_PAUSE);
        assert_eq!(flags & ACTION_ABILITY1, 0);
        assert_eq!(flags & ACTION_SHOP, 0);
    }

    #[test]
    fn the_input_keeps_the_camera_position() {
        let frame = InputFrame {
            move_dir: Axis::new(0.5, 0.5),
            ..InputFrame::default()
        };
        match GameMsg::input(7, 1, &frame, [3.0, -4.0]) {
            GameMsg::Input {
                tick,
                player_id,
                move_dir,
                camera_pos,
                ..
            } => {
                assert_eq!(tick, 7);
                assert_eq!(player_id, 1);
                assert_eq!(move_dir, [0.5, 0.5]);
                assert_eq!(camera_pos, [3.0, -4.0]);
            }
            other => panic!("esperava Input, veio {other:?}"),
        }
    }

    #[test]
    fn the_version_tracks_the_engine_crate() {
        assert_eq!(PROTOCOL_VERSION, giochi_net::VERSAO_PROTOCOLO);
        assert_eq!(SNAPSHOT_HZ, giochi_net::SNAPSHOT_HZ);
    }
}
