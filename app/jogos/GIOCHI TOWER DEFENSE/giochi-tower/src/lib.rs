//! GIOCHI Tower Defense â€” regras, estado e ponte com a engine.

pub mod config;
pub mod engine_bridge;
pub mod errors;
pub mod game;
pub mod net;
pub mod ui;

/// VersÃ£o do crate, como declarada em `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Alvo para o qual este binÃ¡rio foi compilado (carimbado pelo `build.rs`).
pub const BUILD_TARGET: &str = env!("GIOCHI_TOWER_TARGET");

/// Perfil de compilaÃ§Ã£o (`debug`/`release`), carimbado pelo `build.rs`.
pub const BUILD_PROFILE: &str = env!("GIOCHI_TOWER_PROFILE");

/// Nome exibido na janela e no catÃ¡logo.
pub const TITLE: &str = "GIOCHI Tower Defense";

/// FrequÃªncia da simulaÃ§Ã£o, em Hz.
pub use game::state::TICK_HZ;

/// DiretÃ³rio de configuraÃ§Ã£o padrÃ£o, relativo ao diretÃ³rio de execuÃ§Ã£o.
pub const CONFIG_DIR: &str = "config";

/// VariÃ¡vel de ambiente que sobrepÃµe [`CONFIG_DIR`].
pub const CONFIG_DIR_ENV: &str = "GIOCHI_TOWER_CONFIG";
