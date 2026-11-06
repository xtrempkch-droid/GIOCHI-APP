//! # giochi-launcher-core
//!
//! Núcleo do launcher do Giochi: estado, foco, navegação e mapeamento de input.
//!
//! ## Por que este crate existe separado da UI
//!
//! O protótipo original era um único arquivo HTML com estado, reducer, foco e
//! navegação misturados ao DOM. Isso torna impossível testar a lógica sem
//! navegador. Aqui a regra é: **este crate não conhece pixel, janela nem GPU.**
//! Ele recebe comandos e devolve estado.
//!
//! Consequência prática: `cargo test` roda no PC de desenvolvimento, sem
//! hardware, sem SDL, sem painel. A UI vira uma função de `State` para tela.
//!
//! ## A diferença estrutural em relação ao protótipo
//!
//! No HTML havia uma pilha única (`stack`) misturando abas e overlays, e o
//! código assumia `stack[0]` como sendo sempre uma aba. Aqui isso é impossível
//! por construção: a aba atual é um campo (`tab`) e os overlays são uma pilha
//! separada (`overlays`). Não existe estado representável onde o primeiro item
//! da pilha não seja uma aba.

pub mod cmd;
pub mod focus;
pub mod gamepad;
pub mod reduce;
pub mod state;
pub mod viewport;

pub use cmd::{Cmd, Dir};
pub use focus::{Focus, Zone};
pub use gamepad::{GamepadFrame, Mapper};
pub use state::{
    settings_ajustavel, settings_rows, App, DispositivoBt, Game, Overlay, Perf, Rede, Screen, SemFio, Settings, State,
    Tab, Toast, HUD, KEYBOARD, KEYBOARD_COLS, SETTINGS_CATEGORIES,
};
pub use viewport::{grade_de, grade_para, Grade, Viewport};

/// Resolve um comando sobre um estado, devolvendo o novo estado.
///
/// Função livre em vez de método: deixa explícito que o estado é valor, não
/// objeto mutável. A UI guarda `State` e troca pelo retorno.
pub fn update(state: State, cmd: Cmd) -> State {
    reduce::reduce(state, cmd)
}
