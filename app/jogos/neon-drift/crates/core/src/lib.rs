//! Neon Drift — o núcleo puro.
//!
//! Nada aqui conhece SDL, GLES ou o aparelho: pista, física, drift, IA, corrida
//! e formatação do HUD são funções e tipos que rodam em qualquer lugar. É o que
//! permite testar "o carro chega ao topo", "a IA freia na curva" e "duas voltas
//! terminam em vitória" numa máquina sem tela — e o que impediu o defeito
//! clássico deste repositório, o de uma regra que só podia ser conferida
//! olhando o aparelho.

#![warn(missing_docs)]

pub mod carro;
pub mod corrida;
pub mod drift;
pub mod fonte;
pub mod hud;
pub mod ia;
pub mod mapa;
pub mod piloto;
pub mod pista;
pub mod relogio;

pub use carro::{Carro, Entrada, Evento as EventoCarro};
pub use corrida::{Corrida, Desfecho, Evento, Fase, CONTAGEM, TEMPO_LIMITE, VOLTAS};
pub use drift::Drift;
pub use ia::Adversario;
pub use mapa::{Cenario, Mapa};
pub use piloto::dirigir as dirigir_com_piloto;
pub use pista::{Pista, Ponto};
pub use relogio::{ALVO_FPS, MAX_FPS, Relogio, TIQUE};

/// Nome do jogo, como aparece no título e no `metadata.json`.
pub const NOME: &str = "NEON DRIFT";

/// Cores dos carros adversários, em RGB 0..1.
///
/// Ficam no núcleo porque o **desenho** e o **resultado** precisam concordar:
/// o nome na tela de fim tem de ser o carro da mesma cor que estava na pista.
/// Três cores distinguíveis também em painel pequeno e com daltonismo parcial:
/// magenta, ciano e âmbar.
pub const CORES_ADVERSARIOS: [[f32; 3]; 3] = [
    [1.00, 0.24, 0.72], // magenta
    [0.20, 0.86, 1.00], // ciano
    [1.00, 0.66, 0.16], // âmbar
];

/// Cor do carro do jogador: verde-limão de neon.
pub const COR_JOGADOR: [f32; 3] = [0.55, 1.00, 0.35];

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_cores_dos_carros_sao_distintas() {
        // Dois carros da mesma cor na pista, e o HUD de posição passa a mentir
        // sobre quem é quem. O launcher já teve duas capas com a mesma matiz.
        let mut todas = vec![COR_JOGADOR];
        todas.extend(CORES_ADVERSARIOS);
        for i in 0..todas.len() {
            for j in (i + 1)..todas.len() {
                let a = todas[i];
                let b = todas[j];
                let distancia =
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                assert!(
                    distancia > 0.35,
                    "cores {i} e {j} são parecidas demais (distância {distancia:.2})"
                );
            }
        }
    }

    #[test]
    fn ha_uma_cor_por_adversario() {
        assert_eq!(CORES_ADVERSARIOS.len(), 3, "o jogo tem três adversários");
    }
}
