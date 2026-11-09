//! GIOCHI DEFENDER — tower defense espacial 3D *low-poly*.
//!
//! Este crate é o **jogo**; a engine é o snapshot em `vendor/`, consumido por
//! caminho e nunca editado.
//!
//! A separação entre o crate e o binário é o que deixa a lógica testável sem
//! janela: [`state`] e [`gameplay`] não importam `sdl2` nem `glow`. O binário
//! (`src/main.rs`) é o único lugar que abre dispositivo e roda o laço.

pub mod diag;
pub mod gameplay;
pub mod state;
pub mod utils;

/// Versão do jogo, como declarada no `Cargo.toml`.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");
