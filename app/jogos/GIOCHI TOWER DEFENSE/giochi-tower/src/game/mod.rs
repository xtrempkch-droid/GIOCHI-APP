//! Regras e estado do jogo.
//!
//! M1 entregou a câmera e o laço; M2 acrescenta o combate: inimigos com IA,
//! spawner lendo `waves_campaign.ron`, torre com HP/armadura/regen, placar e a
//! rede (protocolo, AOI e stub de loopback). `boss`, `nemesis` e `shop` chegam
//! no M3, nos arquivos que a estrutura do prompt já nomeia.

pub mod boss;
pub mod camera;
pub mod enemy;
pub mod nemesis;
pub mod player;
pub mod score;
pub mod shop;
pub mod state;
pub mod tower;
pub mod waves;

pub use boss::Boss;
pub use camera::{IsoCamera, MapBounds};
pub use enemy::Enemy;
pub use nemesis::Nemesis;
pub use player::PlayerState;
pub use score::Score;
pub use shop::Shop;
pub use state::{GameState, Phase, RunMode};
pub use tower::Tower;
pub use waves::Spawner;
