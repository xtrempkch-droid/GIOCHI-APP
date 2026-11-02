//! Mapeamento de gamepad para comandos.
//!
//! O protótipo **não tinha nenhum código de gamepad** — apenas teclado e
//! ponteiro. Como o requisito é 100% controlável por controle, isso aqui é
//! núcleo, não acabamento.
//!
//! Duas decisões que evitam o comportamento frustrante clássico:
//!
//! - **Botões disparam na borda** (pressão), não enquanto segurados. Sem isso,
//!   segurar A abre dezenas de overlays por segundo.
//! - **Direção repete** com atraso inicial e intervalo fixo, como teclado. Sem
//!   isso, navegar 40 jogos exige 40 toques.
//!
//! O tempo vem em *ticks* de quadro, não em milissegundos, para o núcleo não
//! depender de relógio — o que também torna o teste determinístico.

use crate::cmd::{Cmd, Dir};

/// Zona morta do analógico. Abaixo disso é ruído de repouso.
pub const AXIS_DEADZONE: f32 = 0.35;

/// Ticks segurando a direção antes de começar a repetir (~230 ms a 60 Hz).
pub const REPEAT_DELAY_TICKS: u8 = 14;

/// Intervalo entre repetições depois do atraso (~100 ms a 60 Hz).
pub const REPEAT_INTERVAL_TICKS: u8 = 6;

/// Leitura de um quadro do controle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GamepadFrame {
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub stick_x: f32,
    pub stick_y: f32,
    pub confirm: bool,
    pub back: bool,
    pub options: bool,
    pub search: bool,
    pub tab_prev: bool,
    pub tab_next: bool,
    pub quick_settings: bool,
}

impl GamepadFrame {
    /// Direção dominante, considerando d-pad e analógico com zona morta.
    ///
    /// Se dois eixos passam do limite, vence o de maior magnitude — assim um
    /// analógico na diagonal não fica alternando entre duas direções.
    pub fn dominant_dir(&self) -> Option<Dir> {
        // D-pad tem prioridade: é preciso e não treme.
        if self.dpad_up {
            return Some(Dir::Up);
        }
        if self.dpad_down {
            return Some(Dir::Down);
        }
        if self.dpad_left {
            return Some(Dir::Left);
        }
        if self.dpad_right {
            return Some(Dir::Right);
        }

        let x = if self.stick_x.abs() >= AXIS_DEADZONE {
            self.stick_x
        } else {
            0.0
        };
        let y = if self.stick_y.abs() >= AXIS_DEADZONE {
            self.stick_y
        } else {
            0.0
        };

        if x == 0.0 && y == 0.0 {
            return None;
        }

        if x.abs() >= y.abs() {
            Some(if x > 0.0 { Dir::Right } else { Dir::Left })
        } else {
            Some(if y > 0.0 { Dir::Down } else { Dir::Up })
        }
    }
}

/// Converte quadros em comandos, com borda nos botões e repetição na direção.
#[derive(Clone, Debug, Default)]
pub struct Mapper {
    previous: GamepadFrame,
    last_dir: Option<Dir>,
    held_ticks: u8,
}

impl Mapper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processa um quadro e devolve os comandos que ele produziu.
    pub fn update(&mut self, frame: GamepadFrame) -> Vec<Cmd> {
        let mut commands = Vec::new();

        let dir = frame.dominant_dir();
        match dir {
            Some(d) if self.last_dir == Some(d) => {
                // Mesma direção: conta para a repetição.
                self.held_ticks = self.held_ticks.saturating_add(1);
                if self.held_ticks >= REPEAT_DELAY_TICKS {
                    let elapsed = self.held_ticks - REPEAT_DELAY_TICKS;
                    if elapsed % REPEAT_INTERVAL_TICKS == 0 {
                        commands.push(Cmd::Nav(d));
                    }
                }
            }
            Some(d) => {
                // Direção nova: dispara na hora e reinicia a contagem.
                self.held_ticks = 0;
                commands.push(Cmd::Nav(d));
            }
            None => {
                self.held_ticks = 0;
            }
        }
        self.last_dir = dir;

        // Borda de subida: só dispara quando passa de solto para apertado.
        let mut edge = |now: bool, before: bool, cmd: Cmd| {
            if now && !before {
                commands.push(cmd);
            }
        };

        edge(frame.confirm, self.previous.confirm, Cmd::Confirm);
        edge(frame.back, self.previous.back, Cmd::Back);
        edge(frame.options, self.previous.options, Cmd::Options);
        edge(frame.search, self.previous.search, Cmd::Search);
        edge(frame.tab_prev, self.previous.tab_prev, Cmd::TabPrev);
        edge(frame.tab_next, self.previous.tab_next, Cmd::TabNext);
        edge(
            frame.quick_settings,
            self.previous.quick_settings,
            Cmd::ToggleQuickSettings,
        );

        self.previous = frame;
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn botao_dispara_uma_vez_ao_apertar() {
        let mut mapper = Mapper::new();

        let pressed = GamepadFrame {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(mapper.update(pressed), vec![Cmd::Confirm]);

        // Segurando: não repete.
        assert!(mapper.update(pressed).is_empty());
        assert!(mapper.update(pressed).is_empty());
    }

    #[test]
    fn botao_dispara_de_novo_depois_de_soltar() {
        let mut mapper = Mapper::new();
        let pressed = GamepadFrame {
            confirm: true,
            ..Default::default()
        };
        let released = GamepadFrame::default();

        assert_eq!(mapper.update(pressed), vec![Cmd::Confirm]);
        assert!(mapper.update(released).is_empty());
        assert_eq!(mapper.update(pressed), vec![Cmd::Confirm]);
    }

    #[test]
    fn direcao_dispara_na_hora_e_depois_repte() {
        let mut mapper = Mapper::new();
        let right = GamepadFrame {
            dpad_right: true,
            ..Default::default()
        };

        assert_eq!(mapper.update(right), vec![Cmd::Nav(Dir::Right)]);

        // Antes do atraso, silêncio.
        for _ in 0..(REPEAT_DELAY_TICKS - 1) {
            assert!(mapper.update(right).is_empty());
        }

        // No tick do atraso, repete.
        assert_eq!(mapper.update(right), vec![Cmd::Nav(Dir::Right)]);
    }

    #[test]
    fn trocar_de_direcao_reinicia_a_contagem() {
        let mut mapper = Mapper::new();
        let right = GamepadFrame {
            dpad_right: true,
            ..Default::default()
        };
        let down = GamepadFrame {
            dpad_down: true,
            ..Default::default()
        };

        assert_eq!(mapper.update(right), vec![Cmd::Nav(Dir::Right)]);
        assert_eq!(mapper.update(down), vec![Cmd::Nav(Dir::Down)], "sem atraso ao trocar");
    }

    #[test]
    fn soltar_a_direcao_zera_a_contagem() {
        let mut mapper = Mapper::new();
        let right = GamepadFrame {
            dpad_right: true,
            ..Default::default()
        };

        assert_eq!(mapper.update(right), vec![Cmd::Nav(Dir::Right)]);
        for _ in 0..5 {
            mapper.update(right);
        }
        mapper.update(GamepadFrame::default());

        // Depois de soltar, o próximo toque é imediato.
        assert_eq!(mapper.update(right), vec![Cmd::Nav(Dir::Right)]);
    }

    #[test]
    fn zona_morta_descarta_ruido_do_analogico() {
        let frame = GamepadFrame {
            stick_x: 0.2,
            stick_y: 0.1,
            ..Default::default()
        };
        assert_eq!(frame.dominant_dir(), None);
    }

    #[test]
    fn analogico_alem_da_zona_morta_vira_direcao() {
        let frame = GamepadFrame {
            stick_x: 0.9,
            ..Default::default()
        };
        assert_eq!(frame.dominant_dir(), Some(Dir::Right));

        let frame = GamepadFrame {
            stick_y: -0.9,
            ..Default::default()
        };
        assert_eq!(frame.dominant_dir(), Some(Dir::Up));
    }

    #[test]
    fn diagonal_escolhe_o_eixo_mais_forte() {
        let frame = GamepadFrame {
            stick_x: 0.4,
            stick_y: 0.8,
            ..Default::default()
        };
        assert_eq!(frame.dominant_dir(), Some(Dir::Down));
    }

    #[test]
    fn dpad_tem_prioridade_sobre_analogico() {
        let frame = GamepadFrame {
            dpad_left: true,
            stick_x: 0.9,
            ..Default::default()
        };
        assert_eq!(frame.dominant_dir(), Some(Dir::Left));
    }

    #[test]
    fn botoes_de_aba_e_ajustes_mapeiam() {
        let mut mapper = Mapper::new();
        let frame = GamepadFrame {
            tab_next: true,
            quick_settings: true,
            ..Default::default()
        };
        let cmds = mapper.update(frame);
        assert!(cmds.contains(&Cmd::TabNext));
        assert!(cmds.contains(&Cmd::ToggleQuickSettings));
    }
}
