//! Leitura de controle por `/dev/input/event*`.
//!
//! ## Por que não a API de gamepad do SDL
//!
//! Os botões dos handhelds vêm de driver próprio (`rocknix-joypad`). O SDL só
//! reconhece um controle se houver entrada no `SDL_GameControllerDB`, que não
//! existe para aparelho custom — então no PC funciona e no aparelho não.
//!
//! Além disso, `buttons.map` mapeia por **código evdev** (`BTN_EAST`), enquanto o
//! SDL entrega **índice** de botão. Ler evdev usa o mapa do sistema como ele já é.
//!
//! ## Múltiplos controles
//!
//! Uma thread de leitura por dispositivo, todas alimentando um canal. O estado é a
//! soma de todos: se qualquer controle manda "A", a ação dispara.
//!
//! ## Limite conhecido
//!
//! Só botões e hat digital. Analógico (`ABS_X`/`ABS_Y`) precisa de `absinfo` via
//! ioctl para normalizar a faixa, o que exigiria `libc`.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};

use giochi_launcher_core::GamepadFrame;

/// Tamanho de `struct input_event` em aarch64/x86_64:
/// `timeval` (16) + type (2) + code (2) + value (4).
const EVENT_SIZE: usize = 24;

const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

/// Caminho padrão do mapa, instalado pelo Giochi System.
pub const DEFAULT_MAP_PATH: &str = "/etc/giochi/buttons.map";

/// Evento normalizado que sai das threads.
enum Ev {
    Key(u16, i32),
    Hat(u16, i32),
}

/// Tradução entre nome lógico e código evdev.
#[derive(Clone, Debug)]
pub struct ButtonsMap {
    by_name: HashMap<String, u16>,
}

impl ButtonsMap {
    /// Carrega `buttons.map`, ou usa o padrão embutido se o arquivo não existir.
    ///
    /// Aceita tanto mapa plano (resolvido pelo sistema para o aparelho) quanto
    /// arquivo com seções `[default]` / `[x35s]` — as seções são ignoradas e a
    /// última definição de cada nome vence.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => {
                let map = Self::parse(&text);
                if map.by_name.is_empty() {
                    Self::default_map()
                } else {
                    map
                }
            }
            Err(_) => Self::default_map(),
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut by_name = HashMap::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let name = name.trim().to_lowercase();
            // Comentário no fim da linha é o formato do `/etc/giochi/buttons.map`
            // (cada botão anota o GPIO ao lado). Sem cortar aqui, `code_for`
            // recebia `BTN_DPAD_UP        # gpio4 PA0` e descartava a entrada em
            // silêncio — o mapa do aparelho parseava 1 de 13 botões.
            let value = value.split('#').next().unwrap_or("").trim();
            if let Some(code) = code_for(value) {
                by_name.insert(name, code);
            }
        }
        Self { by_name }
    }

    /// Mapa padrão, igual ao `[default]` do antecessor.
    pub fn default_map() -> Self {
        let mut by_name = HashMap::new();
        for (name, code) in [
            ("up", 0x220u16),
            ("down", 0x221),
            ("left", 0x222),
            ("right", 0x223),
            ("a", 0x130),
            ("b", 0x131),
            ("x", 0x133),
            ("y", 0x134),
            ("l", 0x136),
            ("r", 0x137),
            ("select", 0x13a),
            ("start", 0x13b),
            ("home", 0x13c),
        ] {
            by_name.insert(name.to_string(), code);
        }
        Self { by_name }
    }

    pub fn code(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    pub fn is_complete(&self) -> bool {
        ["up", "down", "left", "right", "a", "b", "x", "y", "l", "r", "start", "select", "home"]
            .iter()
            .all(|name| self.by_name.contains_key(*name))
    }
}

/// Nome de símbolo do kernel → código. Só o que o mapa usa.
fn code_for(symbol: &str) -> Option<u16> {
    Some(match symbol {
        "BTN_SOUTH" | "BTN_A" | "BTN_GAMEPAD" => 0x130,
        "BTN_EAST" | "BTN_B" => 0x131,
        "BTN_NORTH" | "BTN_X" => 0x133,
        "BTN_WEST" | "BTN_Y" => 0x134,
        "BTN_TL" => 0x136,
        "BTN_TR" => 0x137,
        "BTN_TL2" => 0x138,
        "BTN_TR2" => 0x139,
        "BTN_SELECT" => 0x13a,
        "BTN_START" => 0x13b,
        "BTN_MODE" => 0x13c,
        "BTN_THUMBL" => 0x13d,
        "BTN_THUMBR" => 0x13e,
        "BTN_DPAD_UP" => 0x220,
        "BTN_DPAD_DOWN" => 0x221,
        "BTN_DPAD_LEFT" => 0x222,
        "BTN_DPAD_RIGHT" => 0x223,
        _ => return None,
    })
}

/// Estado de todos os controles abertos.
pub struct EvdevInput {
    rx: Receiver<Ev>,
    map: ButtonsMap,
    pressed: HashMap<u16, bool>,
    hat: (i32, i32),
    devices: usize,
    names: Vec<String>,
    _threads: Vec<JoinHandle<()>>,
}

impl EvdevInput {
    /// Abre todos os dispositivos de `/dev/input/event*`.
    ///
    /// Não filtra por capacidade: um teclado também é aberto, e os códigos que não
    /// interessam simplesmente nunca aparecem no mapa.
    pub fn open(input_dir: &Path, map: ButtonsMap) -> Self {
        let (tx, rx) = channel();
        let mut threads = Vec::new();
        let mut names = Vec::new();

        if let Ok(entries) = fs::read_dir(input_dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.starts_with("event"))
                        .unwrap_or(false)
                })
                .collect();
            paths.sort();

            for path in paths {
                if let Some(name) = device_name(&path) {
                    names.push(name);
                }
                if let Some(handle) = spawn_reader(path, tx.clone()) {
                    threads.push(handle);
                }
            }
        }

        Self {
            rx,
            map,
            pressed: HashMap::new(),
            hat: (0, 0),
            devices: threads.len(),
            names,
            _threads: threads,
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices
    }

    pub fn device_names(&self) -> &[String] {
        &self.names
    }

    pub fn map(&self) -> &ButtonsMap {
        &self.map
    }

    /// Drena o canal e atualiza o estado. Chamar uma vez por quadro.
    pub fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                // value 2 é autorepeat; para botão de controle não interessa.
                Ev::Key(code, value) if value != 2 => {
                    self.pressed.insert(code, value == 1);
                }
                Ev::Key(_, _) => {}
                Ev::Hat(code, value) => match code {
                    ABS_HAT0X => self.hat.0 = value,
                    ABS_HAT0Y => self.hat.1 = value,
                    _ => {}
                },
            }
        }
    }

    /// Monta o quadro do núcleo a partir do estado mesclado.
    pub fn frame(&self) -> GamepadFrame {
        let down = |name: &str| -> bool {
            self.map
                .code(name)
                .and_then(|code| self.pressed.get(&code).copied())
                .unwrap_or(false)
        };

        GamepadFrame {
            dpad_up: down("up") || self.hat.1 < 0,
            dpad_down: down("down") || self.hat.1 > 0,
            dpad_left: down("left") || self.hat.0 < 0,
            dpad_right: down("right") || self.hat.0 > 0,
            // Analógico ainda não lido: ver limite no topo do arquivo.
            stick_x: 0.0,
            stick_y: 0.0,
            confirm: down("a"),
            back: down("b"),
            options: down("x"),
            search: down("y"),
            tab_prev: down("l"),
            tab_next: down("r"),
            quick_settings: down("select"),
        }
    }

    /// `start` abre o menu de energia; o núcleo trata fora do `Mapper`.
    pub fn start_pressed(&self) -> bool {
        self.map
            .code("start")
            .and_then(|code| self.pressed.get(&code).copied())
            .unwrap_or(false)
    }

    pub fn home_pressed(&self) -> bool {
        self.map
            .code("home")
            .and_then(|code| self.pressed.get(&code).copied())
            .unwrap_or(false)
    }
}

fn device_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let sys = format!("/sys/class/input/{name}/device/name");
    fs::read_to_string(sys)
        .ok()
        .map(|text| text.trim().to_string())
}

fn spawn_reader(path: PathBuf, tx: Sender<Ev>) -> Option<JoinHandle<()>> {
    let mut file = File::open(&path).ok()?;

    let handle = thread::spawn(move || {
        let mut buffer = [0u8; EVENT_SIZE];
        loop {
            if file.read_exact(&mut buffer).is_err() {
                // Dispositivo removido ou erro de leitura: encerra a thread.
                break;
            }

            let event_type = u16::from_ne_bytes([buffer[16], buffer[17]]);
            let code = u16::from_ne_bytes([buffer[18], buffer[19]]);
            let value = i32::from_ne_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]);

            let event = match event_type {
                EV_KEY => Ev::Key(code, value),
                EV_ABS => Ev::Hat(code, value),
                _ => continue,
            };

            if tx.send(event).is_err() {
                break;
            }
        }
    });

    Some(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_le_mapa_plano() {
        let map = ButtonsMap::parse("up = BTN_DPAD_UP\na = BTN_SOUTH\n");
        assert_eq!(map.code("up"), Some(0x220));
        assert_eq!(map.code("a"), Some(0x130));
    }

    #[test]
    fn parse_ignora_comentario_e_secao() {
        let map = ButtonsMap::parse(
            "# comentário\n[default]\nup = BTN_DPAD_UP\n[x35s]\na = BTN_SOUTH\n",
        );
        assert_eq!(map.code("up"), Some(0x220));
        assert_eq!(map.code("a"), Some(0x130));
    }

    #[test]
    fn parse_aceita_comentario_no_fim_da_linha() {
        // Formato real do /etc/giochi/buttons.map: o GPIO vem depois do `#`.
        let map = ButtonsMap::parse(
            "up      = BTN_DPAD_UP        # gpio4 PA0\n\
             a       = BTN_EAST           # gpio3 PD3\n\
             home    = BTN_MODE\n",
        );
        assert_eq!(map.code("up"), Some(0x220));
        assert_eq!(map.code("a"), Some(0x131));
        assert_eq!(map.code("home"), Some(0x13c));
        assert!(map.is_complete() == false, "faltam botões de propósito");
    }

    #[test]
    fn mapa_do_aparelho_fica_completo() {
        // Conteúdo de /etc/giochi/buttons.map reduzido às 13 teclas exigidas.
        let map = ButtonsMap::parse(
            "up = BTN_DPAD_UP # gpio4 PA0\n\
             down = BTN_DPAD_DOWN # gpio4 PA1\n\
             left = BTN_DPAD_LEFT # gpio3 PD6\n\
             right = BTN_DPAD_RIGHT # gpio3 PD7\n\
             a = BTN_EAST # gpio3 PD3\n\
             b = BTN_SOUTH # gpio3 PD2\n\
             x = BTN_NORTH # gpio3 PD5\n\
             y = BTN_WEST # gpio3 PD4\n\
             l = BTN_TL # gpio3 PD0\n\
             r = BTN_TR # gpio3 PC6\n\
             start = BTN_START # gpio4 PA2\n\
             select = BTN_SELECT # gpio4 PA4\n\
             home = BTN_MODE\n",
        );
        assert!(map.is_complete(), "o mapa do aparelho precisa fechar");
    }

    #[test]
    fn ultima_definicao_vence() {
        let map = ButtonsMap::parse("[a]\na = BTN_EAST\n[b]\na = BTN_SOUTH\n");
        assert_eq!(map.code("a"), Some(0x130));
    }

    #[test]
    fn simbolo_desconhecido_e_ignorado() {
        let map = ButtonsMap::parse("a = BTN_INVENTADO\n");
        assert_eq!(map.code("a"), None);
    }

    #[test]
    fn mapa_padrao_tem_os_treze_nomes() {
        assert!(ButtonsMap::default_map().is_complete());
    }

    #[test]
    fn frame_mescla_dpad_e_hat() {
        let mut input = EvdevInput {
            rx: channel().1,
            map: ButtonsMap::default_map(),
            pressed: HashMap::new(),
            hat: (0, -1),
            devices: 0,
            names: Vec::new(),
            _threads: Vec::new(),
        };
        let frame = input.frame();
        assert!(frame.dpad_up, "hat para cima deve virar dpad_up");

        input.hat = (0, 0);
        input.pressed.insert(0x220, true);
        assert!(input.frame().dpad_up, "botão de dpad também vira dpad_up");
        assert!(!input.frame().dpad_down);
    }
}
