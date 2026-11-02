//! Laço do aplicativo: eventos → comandos → estado → desenho.
//!
//! A regra que mantém isso simples: **a UI não decide nada**. Ela traduz
//! entrada em `Cmd`, chama o reducer e desenha o resultado. Toda a lógica
//! (navegação, pilha, permissões) vive no crate `core`, que é testável sem
//! placa.

use giochi_launcher_core::{
    Cmd, Dir, GamepadFrame, Mapper, Screen, State, Tab, Zone,
};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::draw::Painter;
use crate::input_evdev::EvdevInput;
use crate::theme;
use crate::views::{self, Hit, Layout};

/// Quadros de boot antes de entrar na interface (~1 s a 60 Hz).
const BOOT_FRAMES: u32 = 60;

pub struct App {
    pub state: State,
    mapper: Mapper,
    painter: Painter,
    layout: Layout,
    /// Entrada por evdev. O SDL é usado só para desenhar — ver `input_evdev`.
    evdev: EvdevInput,
    window_size: (u32, u32),
    boot_frames: u32,
    start_was_down: bool,
    fps_frames: u32,
    fps_elapsed: f32,
    /// Uma vez que a fonte falha, não repete o aviso a cada quadro.
    font_warned: bool,
    /// Só redesenha quando algo mudou.
    ///
    /// A UI é praticamente estática: sem animação, dois quadros seguidos são
    /// idênticos. Redesenhar 60 vezes por segundo uma tela que não mudou é gasto
    /// puro de CPU e bateria num portátil.
    ///
    /// Escolhi isso em vez de cache de textura porque o `rust-sdl2` amarra
    /// `Texture` ao `TextureCreator`, que por sua vez empresta o `Canvas` — manter
    /// os dois na mesma struct exigiria `unsafe`. Não redesenhar resolve o mesmo
    /// problema sem tocar nessa parede.
    dirty: bool,
    /// Tempo restante do aviso na tela, em segundos.
    toast_timer: f32,
    /// Último minuto desenhado, para atualizar o relógio.
    last_minute: u64,
}

/// Duração do aviso, igual ao protótipo.
const TOAST_SECONDS: f32 = 2.2;

impl App {
    pub fn new(painter: Painter, state: State, evdev: EvdevInput) -> Self {
        Self {
            state,
            mapper: Mapper::new(),
            painter,
            layout: Layout::default(),
            evdev,
            window_size: (1280, 720),
            boot_frames: 0,
            start_was_down: false,
            fps_frames: 0,
            fps_elapsed: 0.0,
            font_warned: false,
            dirty: true,
            toast_timer: 0.0,
            last_minute: 0,
        }
    }

    fn dispatch(&mut self, cmd: Cmd) {
        let before = self.state.clone();
        self.state = giochi_launcher_core::update(self.state.clone(), cmd);
        if self.state != before {
            self.dirty = true;
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Quit { .. } => return false,

            Event::KeyDown {
                keycode: Some(key), ..
            } => {
                if let Some(cmd) = keyboard_command(*key) {
                    self.dispatch(cmd);
                }
                // No PC, o Enter também acorda e reinicia.
                if *key == Keycode::Return {
                    match self.state.screen {
                        Screen::Sleeping => self.dispatch(Cmd::Wake),
                        Screen::Shutdown => self.dispatch(Cmd::Reboot),
                        _ => {}
                    }
                }
            }

            Event::MouseButtonDown {
                x, y, mouse_btn, ..
            } if *mouse_btn == sdl2::mouse::MouseButton::Left => {
                self.pointer(*x, *y);
            }

            Event::TouchFingerDown { x, y, .. } => {
                // Coordenadas de toque vêm normalizadas (0..1).
                let px = (x * self.window_size.0 as f32) as i32;
                let py = (y * self.window_size.1 as f32) as i32;
                self.pointer(px, py);
            }

            Event::Window { win_event, .. } => {
                if let sdl2::event::WindowEvent::SizeChanged(w, h) = win_event {
                    self.window_size = (*w as u32, *h as u32);
                }
            }

            _ => {}
        }
        true
    }

    /// Um toque escolhe um alvo direto — não simula navegação.
    fn pointer(&mut self, window_x: i32, window_y: i32) {
        match self.state.screen {
            Screen::Sleeping => {
                self.dispatch(Cmd::Wake);
                return;
            }
            Screen::Shutdown => {
                self.dispatch(Cmd::Reboot);
                return;
            }
            Screen::Boot => return,
            Screen::Running => {}
        }

        let Some((x, y)) = self.painter.window_to_logical(window_x, window_y) else {
            return;
        };
        let Some(hit) = self.layout.hit(x, y) else {
            return;
        };

        match hit {
            Hit::Tab(tab) => self.dispatch(Cmd::GoTab(tab)),
            Hit::Filter(filter) => self.dispatch(Cmd::SetFilter(filter)),
            Hit::Carousel(index) => self.dispatch(Cmd::OpenGame(index)),
            Hit::Game(slot) => {
                // `slot` é a posição na lista filtrada, não no catálogo.
                if let Some(game_index) = self.state.filtered_games().get(slot).copied() {
                    self.dispatch(Cmd::OpenGame(game_index));
                }
            }
            Hit::App(index) => self.dispatch(Cmd::OpenApp(index)),
            Hit::SettingsCat(index) => self.dispatch(Cmd::GoSettingsCat(index)),
            Hit::QsRow(index) => {
                self.dispatch(Cmd::FocusAt(Zone::QuickSettings, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::QsButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::QuickSettings, index as usize));
                self.dispatch(Cmd::Confirm);
            }
            Hit::DetailButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::GameDetailActions, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::PowerButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::PowerActions, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::Key(index) => {
                self.dispatch(Cmd::FocusAt(Zone::SearchKeyboard, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::Result(index) => {
                self.dispatch(Cmd::FocusAt(Zone::SearchResults, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::PowerTap => self.dispatch(Cmd::PowerShort),
        }
    }

    /// Chamado a cada quadro: gamepad, relógios e boot.
    pub fn tick(&mut self, delta_seconds: f32) {
        if self.state.screen == Screen::Boot {
            self.boot_frames += 1;
            if self.boot_frames >= BOOT_FRAMES {
                self.dispatch(Cmd::BootDone);
            }
        }

        self.poll_gamepad();
        self.tick_toast(delta_seconds);

        self.fps_frames += 1;
        self.fps_elapsed += delta_seconds;
        if self.fps_elapsed >= 1.0 {
            self.state.hud.fps = (self.fps_frames as f32 / self.fps_elapsed).round() as u16;
            self.fps_frames = 0;
            self.fps_elapsed = 0.0;
            // O HUD mostra um número que muda a cada segundo; sem isso ele
            // congelaria com o redesenho condicional.
            if self.state.settings.hud {
                self.dirty = true;
            }
        }

        // O relógio da barra superior muda a cada minuto. É a única coisa que
        // obriga redesenho sem interação do usuário.
        let minute = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 60)
            .unwrap_or(0);
        if minute != self.last_minute {
            self.last_minute = minute;
            self.dirty = true;
        }
    }

    /// O aviso some sozinho.
    ///
    /// O núcleo expõe `Toast` e `HideToast`, mas não tem relógio — de propósito:
    /// tempo não pertence ao estado, pertence ao laço. Sem esta contagem, o aviso
    /// ficaria na tela para sempre, que era um bug do protótipo antes do
    /// `setTimeout` entrar.
    fn tick_toast(&mut self, delta_seconds: f32) {
        if self.state.toast.visible {
            self.toast_timer -= delta_seconds;
            if self.toast_timer <= 0.0 {
                self.toast_timer = 0.0;
                self.dispatch(Cmd::HideToast);
            }
        } else {
            self.toast_timer = TOAST_SECONDS;
        }
    }

    fn poll_gamepad(&mut self) {
        self.evdev.poll();

        let connected = self.evdev.device_count() > 0;
        if connected != self.state.pad_connected {
            self.dispatch(Cmd::SetPad(connected));
        }

        // `frame()` devolve valor, então o empréstimo de `evdev` termina antes de
        // `mapper` e `dispatch` precisarem de `&mut self`.
        let frame: GamepadFrame = self.evdev.frame();
        let commands = self.mapper.update(frame);
        for cmd in commands {
            self.dispatch(cmd);
        }

        // `start` abre o menu de energia: ação de borda, fora do `Mapper`.
        let start_down = self.evdev.start_pressed();
        if start_down && !self.start_was_down {
            self.dispatch(Cmd::PowerHold);
        }
        self.start_was_down = start_down;
    }

    pub fn render(&mut self) {
        // Nada mudou: não redesenha. Ver `dirty` na struct.
        if !self.dirty {
            return;
        }
        self.dirty = false;

        self.painter.clear(theme::BG);

        // Brilho ambiente no topo, na cor do jogo em foco. Faixas de alfa
        // decrescente: sem shader, e o custo é uma dúzia de retângulos.
        if let Some(game) = self.state.focused_game() {
            let glow = theme::game_accent(game.id);
            let mut y = 0;
            let mut alpha = 46;
            while y < 260 && alpha > 0 {
                self.painter
                    .fill_rect(0, y, 1280, 22, theme::with_alpha(glow, alpha as u8));
                y += 22;
                alpha -= 4;
            }
        }

        self.layout = Layout::default();

        views::chrome_top(&mut self.painter, &self.state, &mut self.layout);

        match self.state.tab {
            Tab::Home => views::home(&mut self.painter, &self.state, &mut self.layout),
            Tab::Library => views::library(&mut self.painter, &self.state, &mut self.layout),
            Tab::Apps => views::apps(&mut self.painter, &self.state, &mut self.layout),
            Tab::Settings => views::settings(&mut self.painter, &self.state, &mut self.layout),
        }

        views::chrome_foot(&mut self.painter, &self.state);
        views::overlays(&mut self.painter, &self.state, &mut self.layout);
        views::toast(&mut self.painter, &self.state);
        views::hud(&mut self.painter, &self.state);

        if !self.font_warned && self.painter.text_size("x", 12).0 == 0 {
            eprintln!(
                "aviso: nenhuma fonte carregada — o texto não será desenhado.\n\
                 Coloque um TTF em assets/fonts/ ou passe --font <caminho>."
            );
            self.font_warned = true;
        }

        self.painter.present();
    }
}

fn keyboard_command(key: Keycode) -> Option<Cmd> {
    match key {
        Keycode::Up => Some(Cmd::Nav(Dir::Up)),
        Keycode::Down => Some(Cmd::Nav(Dir::Down)),
        Keycode::Left => Some(Cmd::Nav(Dir::Left)),
        Keycode::Right => Some(Cmd::Nav(Dir::Right)),
        Keycode::Return | Keycode::Space => Some(Cmd::Confirm),
        Keycode::Escape => Some(Cmd::Back),
        Keycode::X => Some(Cmd::Options),
        Keycode::Y => Some(Cmd::Search),
        Keycode::Q => Some(Cmd::TabPrev),
        Keycode::E => Some(Cmd::TabNext),
        Keycode::Tab => Some(Cmd::ToggleQuickSettings),
        Keycode::P => Some(Cmd::PowerShort),
        _ => None,
    }
}
