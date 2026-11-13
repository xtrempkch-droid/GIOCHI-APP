//! Entrada: gamepad (GDD §14), teclado/mouse de desenvolvimento e touch emulado.
//!
//! # O que a engine entrega
//!
//! `giochi_engine::input::Input` já expõe `gamepad()`, `keyboard()` e `mouse()` e
//! trata hot-plug por dentro: `Input::update` abre o controle em
//! `ControllerDeviceAdded`, fecha em `ControllerDeviceRemoved` e tenta
//! `open_first` enquanto nada está aberto (`input/mod.rs:115-195`). Não há API de
//! hot-plug a chamar daqui — o que existe é a obrigação de chamar `update`.
//!
//! # O que a engine não entrega, e este módulo cobre
//!
//! 1. **Borda de botão do gamepad.** `GamepadState` só tem `is_down` (nível). O
//!    jogo precisa de "apertou neste quadro" para abrir loja, pausar e soltar
//!    habilidade uma vez só. [`edges`] deriva isso comparando com o quadro
//!    anterior.
//! 2. **Zona morta.** O documento do módulo diz que a SDL já aplica a zona morta
//!    do mapeamento, mas o crate não tem nenhum código de zona morta. Aqui há uma
//!    zona **radial** de [`radial_deadzone`], com reescala — sem ela o
//!    personagem anda sozinho com o stick em repouso.
//! 3. **Inversão de Y.** SDL devolve Y negativo para "cima"; o jogo trabalha com
//!    Y positivo = frente. A inversão acontece uma vez, aqui.
//!
//! # Prioridade (GDD §14)
//!
//! Gamepad > teclado/mouse > touch. O gamepad conectado ganha mesmo que o
//! teclado também esteja sendo usado; o teclado só dirige o jogador quando não há
//! controle — assim um controle de bancada mexido não briga com o teclado.
//!
//! # Limite conhecido (medido no repositório, não suposto)
//!
//! No aparelho (X55/X35H) o caminho SDL de gamepad **não funciona**: os dois
//! jogos que rodam hoje leem `/dev/input` (`giochi-defender/src/input_evdev.rs`).
//! Este módulo cobre o PC. O caminho evdev — inclusive os eixos analógicos, que
//! hoje ninguém no repositório lê — é trabalho do bloco M5, e está registrado em
//! `DECISIONS.md` (D-006) e em `README.md`.

use giochi_engine::input::gamepad::{
    AXIS_LEFT_X, AXIS_LEFT_Y, AXIS_RIGHT_X, AXIS_RIGHT_Y, BUTTON_DPAD_UP, BUTTON_EAST,
    BUTTON_NORTH, BUTTON_SOUTH, BUTTON_START, BUTTON_WEST,
};
use giochi_engine::input::keyboard::KeyCode;
use giochi_engine::input::{keyboard, Input};
use giochi_engine::platform::Platform;
use sdl2::event::Event;

use crate::engine_bridge::touch::{TouchButton, TouchEmulation};

/// Zona morta radial do analógico.
pub const DEADZONE: f32 = 0.18;

/// Teclas que a engine não declara. Os valores são os `SDLK_*` da SDL2, que é o
/// que `KeyboardState` guarda (a engine só exporta 15 constantes).
pub mod keys {
    use giochi_engine::input::keyboard::KeyCode;

    /// Tab — abre a loja no intervalo.
    pub const TAB: KeyCode = 9;
    /// Enter — confirma.
    pub const ENTER: KeyCode = 13;
    /// F — item rápido.
    pub const F: KeyCode = 102;
    /// R — recarrega a configuração (desenvolvimento).
    pub const R: KeyCode = 114;
}

/// Direção de stick ou de teclado, em `[-1, 1]` por eixo.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Axis {
    /// Positivo é direita.
    pub x: f32,
    /// Positivo é frente.
    pub y: f32,
}

impl Axis {
    /// O eixo neutro.
    pub const ZERO: Axis = Axis { x: 0.0, y: 0.0 };

    /// Cria um eixo.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Comprimento euclidiano, limitado a 1.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt().min(1.0)
    }

    /// `true` quando não há entrada além da zona morta.
    pub fn is_neutral(self) -> bool {
        self.length() <= 1.0e-6
    }

    /// Limita cada componente a `[-1, 1]`.
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(-1.0, 1.0),
            y: self.y.clamp(-1.0, 1.0),
        }
    }
}

/// Botões/ações que o jogo consome, no vocabulário do GDD §14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons {
    /// A / ✕ — atacar.
    pub attack: bool,
    /// B / ○ — habilidade 1.
    pub ability1: bool,
    /// X / □ — habilidade 2.
    pub ability2: bool,
    /// Y / △ — item rápido.
    pub quick_item: bool,
    /// D-pad ↑ — abrir loja (só no intervalo).
    pub open_shop: bool,
    /// Start — pausar.
    pub pause: bool,
}

/// Transições de "apertou neste quadro".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edges {
    /// Atacar foi apertado agora.
    pub attack: bool,
    /// Habilidade 1 foi apertada agora.
    pub ability1: bool,
    /// Habilidade 2 foi apertada agora.
    pub ability2: bool,
    /// Item rápido foi apertado agora.
    pub quick_item: bool,
    /// Loja foi aberta agora.
    pub open_shop: bool,
    /// Pausa foi apertada agora.
    pub pause: bool,
}

/// Deriva as bordas de subida comparando o quadro atual com o anterior.
///
/// Função livre e pura porque é a parte que erra em silêncio: um `is_down` usado
/// como "apertou" abre a loja todo quadro, ou pausa e despausa no mesmo
/// instante. Assim ela tem teste sem precisar de SDL.
pub fn edges(now: Buttons, previous: Buttons) -> Edges {
    Edges {
        attack: now.attack && !previous.attack,
        ability1: now.ability1 && !previous.ability1,
        ability2: now.ability2 && !previous.ability2,
        quick_item: now.quick_item && !previous.quick_item,
        open_shop: now.open_shop && !previous.open_shop,
        pause: now.pause && !previous.pause,
    }
}

/// Aplica zona morta radial e reescala o restante para `0..=1`.
///
/// Radial e não por eixo: com zona morta por eixo, empurrar o stick na diagonal
/// morta produz um vetor que anda mesmo assim, e a direção "salta" ao sair da
/// zona.
pub fn radial_deadzone(x: f32, y: f32) -> Axis {
    let magnitude = (x * x + y * y).sqrt();
    if !magnitude.is_finite() || magnitude <= DEADZONE {
        return Axis::ZERO;
    }
    let scaled = ((magnitude - DEADZONE) / (1.0 - DEADZONE)).min(1.0) / magnitude;
    Axis::new(x * scaled, y * scaled).clamped()
}

/// De onde veio a entrada deste quadro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputSource {
    /// Nenhuma entrada.
    #[default]
    None,
    /// Gamepad (prioridade 1).
    Gamepad,
    /// Teclado/mouse (prioridade 2, desenvolvimento).
    Keyboard,
    /// Touch emulado por mouse (prioridade 3).
    Touch,
}

/// Entrada já normalizada para um quadro.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InputFrame {
    /// Origem que ganhou a prioridade.
    pub source: InputSource,
    /// Movimento pedido (x direita, y frente).
    pub move_dir: Axis,
    /// Mira pedida (x direita, y frente).
    pub aim_dir: Axis,
    /// Ações seguradas.
    pub buttons: Buttons,
    /// Ações que começaram neste quadro.
    pub edges: Edges,
    /// Deslocamento de câmera pedido pelo teclado (setas), separado da mira.
    pub camera_pan: Axis,
    /// Há gamepad conectado.
    pub pad_connected: bool,
    /// Botão de touch tocado neste quadro, se houver.
    pub touch_button: Option<TouchButton>,
}

/// Dono do estado de entrada e da detecção de bordas.
pub struct InputBridge {
    input: Input,
    previous: Buttons,
    touch: TouchEmulation,
    last_pad: Option<String>,
    frame: InputFrame,
}

impl InputBridge {
    /// Cria o estado de entrada a partir da plataforma (o gamepad é aberto aqui).
    pub fn new(platform: &Platform) -> Self {
        let input = Input::new(platform);
        let last_pad = input.gamepad().name().map(|name| name.to_string());
        Self {
            input,
            previous: Buttons::default(),
            touch: TouchEmulation::default(),
            last_pad,
            frame: InputFrame::default(),
        }
    }

    /// Nome do gamepad aberto, se houver.
    pub fn pad_name(&self) -> Option<&str> {
        self.input.gamepad().name()
    }

    /// Ajusta a área de toque ao tamanho do drawable.
    pub fn set_screen(&mut self, width: f32, height: f32) {
        self.touch = TouchEmulation::new(width, height);
    }

    /// Consome os eventos do quadro e devolve a entrada normalizada.
    ///
    /// Chamar `update` é também o que habilita hot-plug: é dentro dele que a
    /// engine abre e fecha o controle.
    pub fn update(&mut self, events: &[Event], drawable: (f32, f32)) -> InputFrame {
        self.input.update(events);
        self.set_screen(drawable.0, drawable.1);

        let pad = self.input.gamepad();
        let pad_connected = pad.is_connected();
        if let Some(name) = pad.name() {
            if self.last_pad.as_deref() != Some(name) {
                log::info!("gamepad: '{name}'");
                self.last_pad = Some(name.to_string());
            }
        } else if self.last_pad.is_some() {
            log::warn!("gamepad desconectado — a entrada volta para teclado/touch");
            self.last_pad = None;
        }

        // --- gamepad (prioridade 1) -------------------------------------
        let pad_move = radial_deadzone(pad.axis(AXIS_LEFT_X), -pad.axis(AXIS_LEFT_Y));
        let pad_aim = radial_deadzone(pad.axis(AXIS_RIGHT_X), -pad.axis(AXIS_RIGHT_Y));
        let pad_buttons = Buttons {
            attack: pad.is_down(BUTTON_SOUTH),
            ability1: pad.is_down(BUTTON_EAST),
            ability2: pad.is_down(BUTTON_WEST),
            quick_item: pad.is_down(BUTTON_NORTH),
            open_shop: pad.is_down(BUTTON_DPAD_UP),
            pause: pad.is_down(BUTTON_START),
        };
        let pad_used = pad_connected
            && (!pad_move.is_neutral()
                || !pad_aim.is_neutral()
                || pad_buttons != Buttons::default());

        // --- teclado/mouse (prioridade 2) -------------------------------
        let key = |code: KeyCode| self.input.key_down(code);
        let mut key_move = Axis::new(
            (key(keyboard::KEY_D) as i32 - key(keyboard::KEY_A) as i32) as f32,
            (key(keyboard::KEY_W) as i32 - key(keyboard::KEY_S) as i32) as f32,
        );
        if !key_move.is_neutral() {
            key_move = key_move.clamped();
        }
        let key_aim = Axis::new(
            (key(keyboard::KEY_RIGHT) as i32 - key(keyboard::KEY_LEFT) as i32) as f32,
            (key(keyboard::KEY_UP) as i32 - key(keyboard::KEY_DOWN) as i32) as f32,
        );
        let key_buttons = Buttons {
            attack: key(keyboard::KEY_SPACE),
            ability1: key(keyboard::KEY_Q),
            ability2: key(keyboard::KEY_E),
            quick_item: key(keys::F),
            open_shop: key(keys::TAB),
            pause: key(keyboard::KEY_ESCAPE),
        };
        let key_used = !key_move.is_neutral()
            || !key_aim.is_neutral()
            || key_buttons != Buttons::default();

        // --- touch emulado (prioridade 3) -------------------------------
        let (mouse_x, mouse_y) = self.input.mouse().position();
        let mouse_down = self.input.mouse_down(giochi_engine::input::mouse::MOUSE_LEFT);
        let touch_frame = self.touch.sample(Some((mouse_x, mouse_y)), mouse_down);
        let touch_used = !pad_connected && !key_used && touch_frame.active;

        let (source, move_dir, aim_dir, buttons) = if pad_used || (pad_connected && !key_used) {
            (InputSource::Gamepad, pad_move, pad_aim, pad_buttons)
        } else if key_used {
            (InputSource::Keyboard, key_move, key_aim, key_buttons)
        } else if touch_used {
            (
                InputSource::Touch,
                touch_frame.stick,
                touch_frame.stick,
                touch_frame.buttons,
            )
        } else if pad_connected {
            (InputSource::Gamepad, Axis::ZERO, Axis::ZERO, Buttons::default())
        } else {
            (InputSource::None, Axis::ZERO, Axis::ZERO, Buttons::default())
        };

        self.frame = InputFrame {
            source,
            move_dir,
            aim_dir,
            buttons,
            edges: edges(buttons, self.previous),
            // As setas movem a câmera quando não há mira pedida: no teclado,
            // mirar e olhar não podem ser a mesma tecla.
            camera_pan: if aim_dir.is_neutral() { key_aim } else { Axis::ZERO },
            pad_connected,
            touch_button: touch_frame.pressed_button,
        };
        self.previous = buttons;
        self.frame.clone()
    }

    /// Acesso cru à entrada da engine, para o que este bridge não cobre.
    pub fn raw(&self) -> &Input {
        &self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_fires_once_and_only_on_the_transition() {
        let idle = Buttons::default();
        let pressing = Buttons {
            attack: true,
            ..Buttons::default()
        };

        assert!(edges(pressing, idle).attack, "primeiro quadro aperta");
        assert!(!edges(pressing, pressing).attack, "segurar não re-aperta");
        assert!(!edges(idle, pressing).attack, "soltar não é apertar");
    }

    #[test]
    fn each_action_has_its_own_edge() {
        let e = edges(
            Buttons {
                open_shop: true,
                pause: true,
                ..Buttons::default()
            },
            Buttons::default(),
        );
        assert!(e.open_shop && e.pause);
        assert!(!e.attack && !e.ability1 && !e.ability2 && !e.quick_item);
    }

    #[test]
    fn the_deadzone_is_radial_and_neutralises_the_rest() {
        assert!(radial_deadzone(0.0, 0.0).is_neutral());
        assert!(radial_deadzone(DEADZONE * 0.5, 0.0).is_neutral());
        // Diagonal: `(0,13, 0,13)` tem magnitude 0,184, **acima** da zona de
        // 0,18. Por eixo ele passaria (0,13 < 0,18 nos dois), então é
        // exatamente o caso que a zona radial pega e a zona por eixo não.
        // O teste afirmava `is_neutral()`, que é o oposto do que ele descreve.
        assert!(
            !radial_deadzone(0.13, 0.13).is_neutral(),
            "diagonal de 0,184 está fora da zona radial de {DEADZONE}"
        );
        // E um ponto genuinamente dentro continua neutro.
        assert!(radial_deadzone(0.10, 0.10).is_neutral());
    }

    #[test]
    fn the_deadzone_rescales_the_remaining_range() {
        let full = radial_deadzone(0.0, 1.0);
        assert!((full.y - 1.0).abs() < 1.0e-5, "fundo do stick ainda é 1.0: {full:?}");
        assert!(full.x.abs() < 1.0e-6);

        let just_out = radial_deadzone(0.0, DEADZONE + 0.02);
        assert!(just_out.y > 0.0 && just_out.y < 0.1, "saída da zona é suave: {just_out:?}");
    }

    #[test]
    fn the_deadzone_never_exceeds_the_unit_circle() {
        let axis = radial_deadzone(0.9, 0.9);
        assert!(axis.length() <= 1.0 + 1.0e-5, "{axis:?}");
    }

    #[test]
    fn a_neutral_frame_reports_no_source() {
        let frame = InputFrame::default();
        assert_eq!(frame.source, InputSource::None);
        assert!(frame.move_dir.is_neutral());
    }
}
