//! Touch emulado por eventos de mouse (GDD §14, terceira prioridade).
//!
//! O prompt proíbe `SDL_Finger`: a ABI/snapshot não a expõe, e o GDD manda emular
//! via mouse até a extensão existir. Na prática, um touchscreen e um mouse
//! entregam o mesmo evento (`FingerDown`/`MouseButtonDown`), então esta camada
//! funciona nos dois casos e não precisa de duas implementações.
//!
//! O que este módulo faz é só geometria: dado um ponto e um botão pressionado,
//! decide "isto é o joystick virtual" ou "isto é um botão de ação", e devolve um
//! eixo normalizado igual ao do gamepad. Nada de estado global, nada de SDL —
//! por isso tem teste.

use super::input::{Axis, Buttons};

/// Botões de ação desenhados na metade direita da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchButton {
    /// Atacar.
    Attack,
    /// Habilidade 1.
    Ability1,
    /// Habilidade 2.
    Ability2,
    /// Item rápido.
    QuickItem,
}

impl TouchButton {
    /// Os quatro botões, na ordem do layout.
    pub const ALL: [TouchButton; 4] = [
        TouchButton::Attack,
        TouchButton::Ability1,
        TouchButton::Ability2,
        TouchButton::QuickItem,
    ];

    /// Converte para o vocabulário de ações do jogo.
    pub fn to_buttons(self) -> Buttons {
        let mut buttons = Buttons::default();
        match self {
            TouchButton::Attack => buttons.attack = true,
            TouchButton::Ability1 => buttons.ability1 = true,
            TouchButton::Ability2 => buttons.ability2 = true,
            TouchButton::QuickItem => buttons.quick_item = true,
        }
        buttons
    }

    /// Nome curto para log.
    pub fn as_str(self) -> &'static str {
        match self {
            TouchButton::Attack => "atacar",
            TouchButton::Ability1 => "habilidade 1",
            TouchButton::Ability2 => "habilidade 2",
            TouchButton::QuickItem => "item rápido",
        }
    }
}

/// Resultado de um quadro de toque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchFrame {
    /// Eixo do joystick virtual (já com Y invertido para "frente = positivo").
    pub stick: Axis,
    /// Ações pressionadas neste quadro.
    pub buttons: Buttons,
    /// Botão que começou a ser pressionado neste quadro (borda).
    pub pressed_button: Option<TouchButton>,
    /// Houve interação de toque neste quadro.
    pub active: bool,
}

impl Default for TouchFrame {
    fn default() -> Self {
        Self {
            stick: Axis::ZERO,
            buttons: Buttons::default(),
            pressed_button: None,
            active: false,
        }
    }
}

/// Posição e tamanho da zona do joystick e dos botões, em pixels do drawable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchLayout {
    /// Largura do drawable.
    pub width: f32,
    /// Altura do drawable.
    pub height: f32,
}

impl TouchLayout {
    /// Centro do joystick virtual: canto inferior esquerdo, a 18% da borda.
    pub fn stick_center(self) -> (f32, f32) {
        (self.width * 0.18, self.height * 0.78)
    }

    /// Raio do joystick virtual.
    pub fn stick_radius(self) -> f32 {
        self.width.min(self.height) * 0.18
    }

    /// Raio de um botão de ação.
    pub fn button_radius(self) -> f32 {
        self.width.min(self.height) * 0.075
    }

    /// Centro de cada botão de ação, num losango à direita.
    pub fn button_center(self, button: TouchButton) -> (f32, f32) {
        let radius = self.width.min(self.height) * 0.22;
        let center = (self.width * 0.82, self.height * 0.72);
        let (dx, dy) = match button {
            TouchButton::QuickItem => (0.0, -radius),
            TouchButton::Attack => (radius, 0.0),
            TouchButton::Ability1 => (0.0, radius),
            TouchButton::Ability2 => (-radius, 0.0),
        };
        (center.0 + dx, center.1 + dy)
    }
}

/// Emulação de touch: guarda a borda do último botão tocado.
#[derive(Debug, Clone)]
pub struct TouchEmulation {
    layout: TouchLayout,
    last_button: Option<TouchButton>,
}

impl Default for TouchEmulation {
    /// 1280×720 é o tamanho de referência do projeto (GDD §17); sem um drawable
    /// real, é o palpite menos errado para testes e para o primeiro quadro.
    fn default() -> Self {
        Self::new(1280.0, 720.0)
    }
}

impl TouchEmulation {
    /// Cria a emulação para um drawable do tamanho dado.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            layout: TouchLayout {
                width: width.max(1.0),
                height: height.max(1.0),
            },
            last_button: None,
        }
    }

    /// Layout em uso.
    pub fn layout(&self) -> TouchLayout {
        self.layout
    }

    /// Converte o estado do mouse/touch deste quadro.
    ///
    /// `position` é o ponteiro em coordenadas de janela (origem no canto superior
    /// esquerdo, Y para baixo), como `MouseState::position()` entrega.
    pub fn sample(&mut self, position: Option<(f32, f32)>, pressed: bool) -> TouchFrame {
        if !pressed {
            self.last_button = None;
            return TouchFrame::default();
        }
        let Some((x, y)) = position else {
            return TouchFrame::default();
        };

        // Botões primeiro: eles ficam por cima da zona morta do joystick no
        // layout, e um toque no botão não pode virar meia-volta do stick.
        for button in TouchButton::ALL {
            let (cx, cy) = self.layout.button_center(button);
            let reach = self.layout.button_radius();
            if (x - cx).powi(2) + (y - cy).powi(2) <= reach * reach {
                let pressed_button = if self.last_button == Some(button) {
                    None
                } else {
                    self.last_button = Some(button);
                    Some(button)
                };
                return TouchFrame {
                    stick: Axis::ZERO,
                    buttons: button.to_buttons(),
                    pressed_button,
                    active: true,
                };
            }
        }
        self.last_button = None;

        let (cx, cy) = self.layout.stick_center();
        let radius = self.layout.stick_radius().max(1.0);
        let dx = (x - cx) / radius;
        // Y da tela cresce para baixo; "para frente" é para cima.
        let dy = (cy - y) / radius;
        let axis = Axis::new(dx, dy).clamped();
        let active = !axis.is_neutral();
        TouchFrame {
            stick: if active { axis } else { Axis::ZERO },
            buttons: Buttons::default(),
            pressed_button: None,
            active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emulation() -> TouchEmulation {
        TouchEmulation::new(1280.0, 720.0)
    }

    #[test]
    fn released_pointer_is_neutral() {
        let mut touch = emulation();
        let frame = touch.sample(Some((10.0, 10.0)), false);
        assert!(!frame.active);
        assert!(frame.stick.is_neutral());
        assert_eq!(frame.buttons, Buttons::default());
    }

    #[test]
    fn dragging_the_stick_up_means_forward() {
        let mut touch = emulation();
        let (cx, cy) = touch.layout().stick_center();
        let radius = touch.layout().stick_radius();

        let frame = touch.sample(Some((cx, cy - radius)), true);
        assert!(frame.active);
        assert!(frame.stick.y > 0.95, "para cima na tela = frente: {:?}", frame.stick);
        assert!(frame.stick.x.abs() < 1.0e-5);
    }

    #[test]
    fn the_stick_is_clamped_at_the_unit_circle() {
        let mut touch = emulation();
        let (cx, cy) = touch.layout().stick_center();
        let frame = touch.sample(Some((cx - 5000.0, cy)), true);
        assert!(frame.stick.length() <= 1.0 + 1.0e-5, "{:?}", frame.stick);
        assert!(frame.stick.x < -0.99);
    }

    #[test]
    fn a_tap_outside_every_zone_is_not_active() {
        let mut touch = emulation();
        let frame = touch.sample(Some((1280.0 * 0.5, 20.0)), true);
        assert!(!frame.active, "topo da tela não é stick nem botão");
    }

    #[test]
    fn each_button_is_hit_in_its_own_place() {
        for button in TouchButton::ALL {
            let mut touch = emulation();
            let (cx, cy) = touch.layout().button_center(button);
            let frame = touch.sample(Some((cx, cy)), true);
            assert!(frame.active, "{:?} devia estar ativo", button);
            assert_eq!(frame.pressed_button, Some(button));
            assert_eq!(frame.buttons, button.to_buttons());
        }
    }

    #[test]
    fn holding_a_button_only_edges_once() {
        let mut touch = emulation();
        let (cx, cy) = touch.layout().button_center(TouchButton::Attack);

        let first = touch.sample(Some((cx, cy)), true);
        assert_eq!(first.pressed_button, Some(TouchButton::Attack));

        let second = touch.sample(Some((cx, cy)), true);
        assert_eq!(second.pressed_button, None, "segurar não re-aperta");
        assert!(second.buttons.attack, "mas continua pressionado");

        let _ = touch.sample(Some((cx, cy)), false);
        let third = touch.sample(Some((cx, cy)), true);
        assert_eq!(third.pressed_button, Some(TouchButton::Attack), "soltar e apertar volta a dar borda");
    }

    #[test]
    fn the_four_buttons_do_not_overlap() {
        let touch = emulation();
        let layout = touch.layout();
        let reach = layout.button_radius();
        for a in TouchButton::ALL {
            for b in TouchButton::ALL {
                if a == b {
                    continue;
                }
                let (ax, ay) = layout.button_center(a);
                let (bx, by) = layout.button_center(b);
                let distance = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                assert!(
                    distance > reach * 1.9,
                    "{a:?} e {b:?} a {distance} px, raio {reach} px"
                );
            }
        }
    }
}
