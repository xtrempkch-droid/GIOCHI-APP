//! Núcleo da Cobrinha: regras, ritmo e layout, sem SDL.
//!
//! A separação existe por um motivo prático: o CI e o PC de desenvolvimento não
//! têm SDL2 nem aparelho. Tirando o SDL daqui, a lógica inteira — colisão,
//! crescimento, ritmo, geometria — é testável em milissegundos e num contêiner
//! sem biblioteca gráfica.
//!
//! A camada `giochi-cobrinha` (SDL) é fina de propósito: transformar estado em
//! retângulo e tecla em ação. Quanto menos código nela, menos código sem teste.

pub mod entrada;
pub mod geometria;
pub mod jogo;
pub mod relogio;

pub use entrada::{de_evdev, Acao};
pub use geometria::{Geometria, COLUNAS, LINHAS};
pub use jogo::{Direcao, Estado, Evento, Jogo, Motivo, Posicao};
pub use relogio::{Passos, Relogio, ACELERACAO, ALVO_FPS, MAX_FPS, TIQUE_BASE, TIQUE_MAX};
