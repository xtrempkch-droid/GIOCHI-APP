//! GIOCHI Tower Defense — regras, estado e ponte com a engine.

pub mod config;
pub mod engine_bridge;
pub mod errors;
pub mod game;
pub mod menu;
pub mod net;
pub mod ui;

/// Versão do crate, como declarada em `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Alvo para o qual este binário foi compilado (carimbado pelo `build.rs`).
pub const BUILD_TARGET: &str = env!("GIOCHI_TOWER_TARGET");

/// Perfil de compilação (`debug`/`release`), carimbado pelo `build.rs`.
pub const BUILD_PROFILE: &str = env!("GIOCHI_TOWER_PROFILE");

/// Nome exibido na janela e no catálogo.
pub const TITLE: &str = "GIOCHI Tower Defense";

/// Frequência da simulação, em Hz.
pub use game::state::TICK_HZ;

/// Diretório de configuração padrão, relativo ao diretório de execução.
pub const CONFIG_DIR: &str = "config";

/// Variável de ambiente que sobrepõe [`CONFIG_DIR`].
pub const CONFIG_DIR_ENV: &str = "GIOCHI_TOWER_CONFIG";

/// Diretório de assets padrão (modelos e texturas), relativo ao executável.
pub const ASSETS_DIR: &str = "assets";

/// Variável de ambiente que sobrepõe [`ASSETS_DIR`].
///
/// Existe pelo mesmo motivo do `GIOCHI_TOWER_CONFIG`: sem ela, testar um pacote
/// de arte em outro diretório exigiria reinstalar o jogo.
pub const ASSETS_DIR_ENV: &str = "GIOCHI_TOWER_ASSETS";
