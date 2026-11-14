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

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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

/// Códigos `KEY_*` de `linux/input-event-codes.h` que o aparelho declara.
pub const KEY_POWER: u16 = 116;
pub const KEY_VOLUMEDOWN: u16 = 114;
pub const KEY_VOLUMEUP: u16 = 115;

/// Evento normalizado que sai das threads.
enum Ev {
    Key(u16, i32),
    Hat(u16, i32),
}

/// Tradução entre nome lógico e código evdev.
#[derive(Clone, Debug)]
pub struct ButtonsMap {
    by_name: HashMap<String, u16>,
    /// Calibração dos analógicos, declarada pelo sistema com chaves `axis_*`.
    ///
    /// Não são símbolos evdev: são a faixa real de cada eixo, que varia com o
    /// ADC do aparelho (`adc-joystick`, faixa 15..1023 aqui). O laço ainda não
    /// lê o analógico, mas **guardar a declaração** evita o descarte silencioso
    /// e deixa o dado pronto para quando a leitura entrar.
    calibragem: HashMap<String, i32>,
    /// Símbolos que o arquivo declarou e esta build não conhece.
    ///
    /// O mapa é do **sistema**; o tradutor é do app. Quando o sistema declara
    /// algo novo, o app precisa dizer que não sabe — antes disto o símbolo era
    /// descartado em silêncio e o botão simplesmente não funcionava, sem
    /// nenhuma pista. Aconteceu duas vezes: com comentário no fim da linha e com
    /// `KEY_VOLUMEUP`.
    desconhecidos: Vec<String>,
    /// Nomes marcados `OPCIONAL` pelo sistema: podem não existir no gabarito.
    ///
    /// Caso real: o botão `F` (`BTN_MODE`) da X55 está no DTS — revisões que o
    /// têm funcionam sem rebuild — mas **não existe fisicamente** no aparelho
    /// testado. Exigi-lo deixaria o mapa "incompleto" para sempre, e um aviso
    /// permanente é pior que nenhum: esconde o dia em que um botão de verdade
    /// quebrar.
    ///
    /// O marcador vem no comentário da linha, junto do GPIO:
    /// `home = BTN_MODE  # gpio3 PB7 — OPCIONAL: nao existe fisicamente`
    opcionais: HashSet<String>,
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
                if !map.desconhecidos.is_empty() {
                    eprintln!(
                        "aviso: {} símbolo(s) do mapa não reconhecido(s): {}",
                        map.desconhecidos.len(),
                        map.desconhecidos.join(", ")
                    );
                }
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
        let mut calibragem = HashMap::new();
        let mut desconhecidos = Vec::new();
        let mut opcionais = HashSet::new();
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
            //
            // O comentário é lido **antes** de ser descartado: é nele que o
            // sistema escreve o marcador `OPCIONAL`.
            let comentario = value.split_once('#').map(|(_, c)| c).unwrap_or("");
            if comentario.to_uppercase().contains("OPCIONAL") {
                opcionais.insert(name.clone());
            }
            let value = value.split('#').next().unwrap_or("").trim();
            if name.starts_with("axis_") {
                match value.parse::<i32>() {
                    Ok(v) => {
                        // Sem o prefixo: `eixo("x_min")`, não `eixo("axis_x_min")`.
                        let chave = name.trim_start_matches("axis_").to_string();
                        calibragem.insert(chave, v);
                    }
                    Err(_) => desconhecidos.push(format!("{name} = {value}")),
                }
                continue;
            }
            match code_for(value) {
                Some(code) => {
                    by_name.insert(name, code);
                }
                None => desconhecidos.push(format!("{name} = {value}")),
            }
        }
        Self {
            by_name,
            calibragem,
            desconhecidos,
            opcionais,
        }
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
        Self {
            by_name,
            calibragem: HashMap::new(),
            desconhecidos: Vec::new(),
            opcionais: HashSet::new(),
        }
    }

    /// Faixa declarada de um eixo, ex.: `("x_min", 15)`.
    ///
    /// Fica público porque é o dado que a leitura do analógico vai precisar
    /// para normalizar; hoje só o teste usa.
    pub fn eixo(&self, nome: &str) -> Option<i32> {
        self.calibragem.get(nome).copied()
    }

    pub fn code(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    /// O sistema marcou este botão como `OPCIONAL`?
    pub fn eh_opcional(&self, name: &str) -> bool {
        self.opcionais.contains(name)
    }

    /// Os botões essenciais estão todos mapeados?
    ///
    /// `home` **não** é exigido, e isso não é tolerância: é o botão que varia
    /// por revisão de hardware.
    ///
    /// - **X55**: o `BTN_MODE` está no DTS (via patch), mas o gabarito físico
    ///   testado não tem o botão `F` — o sistema o marca `OPCIONAL`.
    /// - **X35H**: não declara `home` de forma alguma; 16 botões, sem `BTN_MODE`.
    ///
    /// Exigi-lo geraria aviso permanente de "mapa incompleto" nos dois aparelhos,
    /// e aviso permanente é pior que nenhum: esconde o dia em que um botão de
    /// verdade quebrar — que foi exatamente o caso que o aviso existe para pegar.
    ///
    /// Botão marcado `OPCIONAL` pelo sistema também sai da cobrança, para
    /// proteger qualquer outro que venha a ser variável.
    pub fn is_complete(&self) -> bool {
        ["up", "down", "left", "right", "a", "b", "x", "y", "l", "r", "start", "select"]
            .iter()
            .filter(|name| !self.eh_opcional(name))
            .all(|name| self.by_name.contains_key(*name))
    }
}

/// Um botão do mapa está pressionado?
///
/// Livre (não método) para poder ser testado sem abrir `/dev/input`.
fn pressionado(map: &ButtonsMap, estado: &HashMap<u16, bool>, nome: &str) -> bool {
    map.code(nome)
        .and_then(|c| estado.get(&c).copied())
        .unwrap_or(false)
}

/// Nome de símbolo do kernel → código. Só o que o mapa usa.
pub fn code_for(symbol: &str) -> Option<u16> {
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
        // Teclas KEY_*, usadas pelos botões de sistema do aparelho
        // (`gpio-keys-vol`, `rk805 pwrkey`), não pelo controle.
        "KEY_VOLUMEDOWN" => KEY_VOLUMEDOWN,
        "KEY_VOLUMEUP" => KEY_VOLUMEUP,
        "KEY_POWER" => KEY_POWER,
        "KEY_HOME" => 102,
        "KEY_MENU" => 139,
        "KEY_BACK" => 158,
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
    /// Transmissor guardado para os leitores abertos **depois** da abertura
    /// inicial (controle Bluetooth que conectou com o launcher já rodando).
    tx: Sender<Ev>,
    dir: PathBuf,
    abertos: HashSet<PathBuf>,
    proxima_revisao: Instant,
}

/// De quanto em quanto tempo `/dev/input` é reexaminado.
///
/// Um controle Bluetooth aparece como um `event*` novo **só quando conecta**, e
/// nunca no boot. Abrir a lista uma vez — como era — deixava o controle recém-
/// pareado invisível para sempre, e o sintoma era "pareou mas não funciona":
/// indistinguível de "não pareou".
const REVISAO_MS: u64 = 2_000;

impl EvdevInput {
    /// Abre todos os dispositivos de `/dev/input/event*`.
    ///
    /// Não filtra por capacidade: um teclado também é aberto, e os códigos que não
    /// interessam simplesmente nunca aparecem no mapa.
    pub fn open(input_dir: &Path, map: ButtonsMap) -> Self {
        let (tx, rx) = channel();
        let mut this = Self {
            rx,
            map,
            pressed: HashMap::new(),
            hat: (0, 0),
            devices: 0,
            names: Vec::new(),
            _threads: Vec::new(),
            tx,
            dir: input_dir.to_path_buf(),
            abertos: HashSet::new(),
            proxima_revisao: Instant::now() + Duration::from_millis(REVISAO_MS),
        };
        this.abrir_novos();
        this
    }

    /// Abre os `event*` que ainda não têm leitor.
    ///
    /// Idempotente e barato: relê o diretório e ignora o que já está em
    /// `abertos`. Devolve quantos abriu agora, o que é o que o log reporta.
    fn abrir_novos(&mut self) -> usize {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return 0;
        };
        let mut caminhos: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("event"))
                    .unwrap_or(false)
            })
            .collect();
        caminhos.sort();

        let mut novos = 0;
        for path in caminhos {
            if self.abertos.contains(&path) {
                continue;
            }
            if let Some(handle) = spawn_reader(path.clone(), self.tx.clone()) {
                self._threads.push(handle);
                self.abertos.insert(path.clone());
                self.devices = self.abertos.len();
                if let Some(name) = device_name(&path) {
                    self.names.push(name);
                }
                novos += 1;
            }
        }
        novos
    }

    /// Reexamina `/dev/input` de tempos em tempos. Chamado por `poll`.
    ///
    /// O intervalo existe para não reler o diretório 60 vezes por segundo: um
    /// controle Bluetooth leva segundos para conectar, e 2 s de atraso é
    /// invisível para quem acabou de apertar "parear".
    fn revisar(&mut self) {
        if Instant::now() < self.proxima_revisao {
            return;
        }
        self.proxima_revisao = Instant::now() + Duration::from_millis(REVISAO_MS);
        let novos = self.abrir_novos();
        if novos > 0 {
            // O log importa: sem ele, "o controle conectou" e "o controle foi
            // lido" continuam indistinguíveis de fora.
            eprintln!(
                "entrada: {novos} dispositivo(s) novo(s) — {} no total",
                self.devices
            );
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
        self.revisar();
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
            // L2/R2 vêm do mesmo mapa (`l2 = BTN_TL2`, `r2 = BTN_TR2`). Estavam
            // traduzidos por `code_for` e **descartados** aqui: o botão existia,
            // o evento chegava, e nada acontecia.
            sub_prev: down("l2"),
            sub_next: down("r2"),
            quick_settings: down("select"),
        }
    }

    /// Botão de energia físico (`rk805 pwrkey`, `KEY_POWER`).
    ///
    /// Não passa pelo `buttons.map` de propósito: é um botão do **sistema**, não
    /// do controle. Um mapa por aparelho não deveria precisar declarar que o
    /// botão de energia existe — isso é fato do hardware, como o próprio
    /// dispositivo de entrada.
    ///
    /// Medido no X55: `/sys/class/input/event0/device/capabilities/key` =
    /// `10000000000000 0`, que decodificado dá o bit 52 da segunda palavra =
    /// código 116 (`KEY_POWER`). O mesmo arquivo do `gpio-keys-vol` traz
    /// `c000000000000` = 114/115 (`KEY_VOLUMEDOWN`/`KEY_VOLUMEUP`), o que
    /// confirma a leitura.
    pub fn power_pressed(&self) -> bool {
        self.pressed.get(&KEY_POWER).copied().unwrap_or(false)
    }

    /// Estado dos botões de volume, lidos **pelo mapa**.
    ///
    /// Diferente do power (que é do PMIC e não passa pelo mapa), `volup` e
    /// `voldown` estão declarados no `/etc/giochi/buttons.map` — então é o
    /// sistema decidindo, e o app só consulta.
    ///
    /// Devolve `(cima, baixo)`.
    pub fn volume_pressed(&self) -> (bool, bool) {
        (
            pressionado(&self.map, &self.pressed, "volup"),
            pressionado(&self.map, &self.pressed, "voldown"),
        )
    }

    /// `start` abre o menu de energia; o núcleo trata fora do `Mapper`.
    pub fn start_pressed(&self) -> bool {
        pressionado(&self.map, &self.pressed, "start")
    }

    pub fn home_pressed(&self) -> bool {
        pressionado(&self.map, &self.pressed, "home")
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

/// Todos os símbolos que o tradutor entende.
///
/// Esta lista existe para ser **consultada de fora** (`--simbolos`), não para
/// ser mantida à mão: o teste `lista_de_simbolos_bate_com_o_tradutor` percorre
/// cada entrada e confirma que `code_for` a aceita. Assim a lista não pode
/// envelhecer separada do tradutor — que é como o `releaseYear` precisou entrar
/// em três lugares e o carrossel ficou dessincronizado.
pub const SIMBOLOS_CONHECIDOS: &[&str] = &[
    "BTN_SOUTH", "BTN_A", "BTN_GAMEPAD",
    "BTN_EAST", "BTN_B",
    "BTN_NORTH", "BTN_X",
    "BTN_WEST", "BTN_Y",
    "BTN_TL", "BTN_TR", "BTN_TL2", "BTN_TR2",
    "BTN_SELECT", "BTN_START", "BTN_MODE",
    "BTN_THUMBL", "BTN_THUMBR",
    "BTN_DPAD_UP", "BTN_DPAD_DOWN", "BTN_DPAD_LEFT", "BTN_DPAD_RIGHT",
    "KEY_VOLUMEDOWN", "KEY_VOLUMEUP", "KEY_POWER",
    "KEY_HOME", "KEY_MENU", "KEY_BACK",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Um `event*` que aparece **depois** da abertura tem de ser lido.
    ///
    /// É o caso do controle Bluetooth: ele só cria o nó quando conecta, e a
    /// abertura única deixava o controle recém-pareado invisível para sempre —
    /// o sintoma era "pareou mas não funciona", indistinguível de "não pareou".
    #[test]
    fn dispositivo_novo_e_aberto_na_revisao_seguinte() {
        let dir = std::env::temp_dir().join(format!("giochi-evdev-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("event0"), []).expect("event0");

        let mut entrada = EvdevInput::open(&dir, ButtonsMap::parse("a = BTN_EAST\n"));
        assert_eq!(entrada.device_count(), 1, "o event0 existente tem de ser aberto");

        // O controle conectou: o nó apareceu com o launcher já rodando.
        fs::write(dir.join("event1"), []).expect("event1");

        // `revisar` tem uma janela de 2 s; aqui ela é vencida à mão.
        entrada.proxima_revisao = Instant::now() - Duration::from_millis(1);
        entrada.revisar();
        assert_eq!(
            entrada.device_count(),
            2,
            "o event1 apareceu depois da abertura e não foi aberto"
        );

        // E a revisão é idempotente: nada novo, nada duplicado.
        entrada.proxima_revisao = Instant::now() - Duration::from_millis(1);
        entrada.revisar();
        assert_eq!(entrada.device_count(), 2, "a segunda revisão duplicou o leitor");

        let _ = fs::remove_dir_all(&dir);
    }

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
    fn mapa_real_do_x35h_fica_completo() {
        // Conteúdo real de `device/powkiddy x35h/buttons.map`: 16 botões, SEM
        // `home`. Exigir `home` aqui daria aviso falso de mapa incompleto.
        let map = ButtonsMap::parse(
            "up      = BTN_DPAD_UP        # gpio4 PA0\n\
             down    = BTN_DPAD_DOWN      # gpio4 PA1\n\
             left    = BTN_DPAD_LEFT      # gpio3 PD6\n\
             right   = BTN_DPAD_RIGHT     # gpio3 PD7\n\
             a       = BTN_EAST           # gpio3 PD3\n\
             b       = BTN_SOUTH          # gpio3 PD2\n\
             x       = BTN_NORTH          # gpio3 PD5\n\
             y       = BTN_WEST           # gpio3 PD4\n\
             l       = BTN_TL             # gpio3 PD0\n\
             r       = BTN_TR             # gpio3 PC6\n\
             l2      = BTN_TL2            # gpio3 PD1\n\
             r2      = BTN_TR2            # gpio3 PC7\n\
             start   = BTN_START          # gpio4 PA2\n\
             select  = BTN_SELECT         # gpio4 PA4\n\
             thumbl  = BTN_THUMBL         # gpio4 PA7\n\
             thumbr  = BTN_THUMBR         # gpio4 PB0\n\
             volup   = KEY_VOLUMEUP       # gpio4 PA5\n\
             voldown = KEY_VOLUMEDOWN     # gpio4 PA3\n\
             axis_x_min = 15\n\
             axis_x_max = 1023\n\
             axis_y_min = 1023\n\
             axis_y_max = 15\n\
             axis_y_deadzone = 30\n",
        );
        assert!(map.is_complete(), "X35H não tem `home` e isso é normal");
        assert!(map.desconhecidos.is_empty(), "{:?}", map.desconhecidos);
        assert_eq!(map.code("home"), None);
        assert_eq!(map.code("thumbl"), Some(0x13d));
        // A calibração vem declarada; o kernel já entrega Y invertido (min > max),
        // então quem ler o eixo NÃO deve inverter de novo.
        assert_eq!(map.eixo("y_min"), Some(1023));
        assert_eq!(map.eixo("y_max"), Some(15));
        assert_eq!(map.eixo("y_deadzone"), Some(30));
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
    fn opcional_nao_e_exigido() {
        // `home` marcado OPCIONAL pelo sistema: o botão `F` existe no DTS mas não
        // no gabarito físico desta revisão. Sem tratar o marcador, o mapa fica
        // "incompleto" para sempre e o aviso esconde um botão realmente quebrado.
        let map = ButtonsMap::parse(
            "up = BTN_DPAD_UP\n\
             down = BTN_DPAD_DOWN\n\
             left = BTN_DPAD_LEFT\n\
             right = BTN_DPAD_RIGHT\n\
             a = BTN_EAST\n\
             b = BTN_SOUTH\n\
             x = BTN_NORTH\n\
             y = BTN_WEST\n\
             l = BTN_TL\n\
             r = BTN_TR\n\
             start = BTN_START\n\
             select = BTN_SELECT\n\
             home = BTN_MODE  # gpio3 PB7 — OPCIONAL: nao existe fisicamente\n",
        );
        assert!(map.is_complete(), "OPCIONAL não pode ser exigido");
        // Continua mapeado: em revisões que têm o botão, ele funciona.
        assert_eq!(map.code("home"), Some(0x13c));
        assert!(map.eh_opcional("home"));
    }

    #[test]
    fn opcional_ausente_do_mapa_nao_reprova() {
        // X35H: 16 botões, sem BTN_MODE declarado. O mapa não tem `home`.
        let map = ButtonsMap::parse(
            "up = BTN_DPAD_UP\n\
             down = BTN_DPAD_DOWN\n\
             left = BTN_DPAD_LEFT\n\
             right = BTN_DPAD_RIGHT\n\
             a = BTN_EAST\n\
             b = BTN_SOUTH\n\
             x = BTN_NORTH\n\
             y = BTN_WEST\n\
             l = BTN_TL\n\
             r = BTN_TR\n\
             start = BTN_START\n\
             select = BTN_SELECT\n\
             home = BTN_MODE  # OPCIONAL\n",
        );
        // Mesmo sem o código presente, o marcador tira a exigência.
        assert!(map.is_complete());
    }

    #[test]
    fn botao_normal_ausente_ainda_reprova() {
        // A contraprova: sem OPCIONAL, faltar `a` é defeito de verdade.
        let map = ButtonsMap::parse(
            "up = BTN_DPAD_UP\n\
             down = BTN_DPAD_DOWN\n\
             left = BTN_DPAD_LEFT\n\
             right = BTN_DPAD_RIGHT\n\
             x = BTN_NORTH\n\
             y = BTN_WEST\n\
             l = BTN_TL\n\
             r = BTN_TR\n\
             start = BTN_START\n\
             select = BTN_SELECT\n\
             home = BTN_MODE\n",
        );
        assert!(!map.is_complete(), "faltar `a` sem OPCIONAL é erro");
    }

    #[test]
    fn opcional_nao_suja_a_lista_de_desconhecidos() {
        let map = ButtonsMap::parse("home = BTN_MODE  # OPCIONAL\n");
        assert!(
            map.desconhecidos.is_empty(),
            "o marcador é conhecido: {:?}",
            map.desconhecidos
        );
        assert!(map.eh_opcional("home"));
    }

    #[test]
    fn ultima_definicao_vence() {
        let map = ButtonsMap::parse("[a]\na = BTN_EAST\n[b]\na = BTN_SOUTH\n");
        assert_eq!(map.code("a"), Some(0x130));
    }

    #[test]
    fn simbolo_desconhecido_e_ignorado_mas_reportado() {
        let map = ButtonsMap::parse("a = BTN_INVENTADO\n");
        assert_eq!(map.code("a"), None);
        // O silêncio foi a causa de dois bugs reais. Agora tem que aparecer.
        assert_eq!(map.desconhecidos, vec!["a = BTN_INVENTADO".to_string()]);
    }

    #[test]
    fn tecla_de_volume_do_mapa_vira_estado_pressionado() {
        // Cadeia completa do lado do app: linha do mapa → código evdev →
        // consulta de estado. Sem isto, "o botão de volume não faz nada" fica
        // sem saber se o problema é o mapa, o código ou o kernel.
        let map = ButtonsMap::parse("volup = KEY_VOLUMEUP\nvoldown = KEY_VOLUMEDOWN\n");
        let mut estado: HashMap<u16, bool> = HashMap::new();

        assert!(!pressionado(&map, &estado, "volup"), "solto");

        estado.insert(115, true);
        assert!(pressionado(&map, &estado, "volup"), "KEY_VOLUMEUP apertado");
        assert!(!pressionado(&map, &estado, "voldown"));

        estado.insert(115, false);
        estado.insert(114, true);
        assert!(!pressionado(&map, &estado, "volup"), "soltou");
        assert!(pressionado(&map, &estado, "voldown"));
    }

    #[test]
    fn botao_fora_do_mapa_nunca_esta_pressionado() {
        let map = ButtonsMap::parse("a = BTN_SOUTH\n");
        let mut estado = HashMap::new();
        estado.insert(0x130, true);
        assert!(!pressionado(&map, &estado, "volup"), "não declarado no mapa");
    }

    #[test]
    fn calibragem_dos_eixos_e_guardada_nao_descartada() {
        // `axis_*` não é símbolo evdev: é a faixa real do ADC do aparelho.
        let map = ButtonsMap::parse(
            "axis_x_min = 15\naxis_x_max = 1023\naxis_y_deadzone = 30\n",
        );
        assert_eq!(map.eixo("x_min"), Some(15));
        assert_eq!(map.eixo("x_max"), Some(1023));
        assert_eq!(map.eixo("y_deadzone"), Some(30));
        assert!(
            map.desconhecidos.is_empty(),
            "calibragem não é símbolo desconhecido: {:?}",
            map.desconhecidos
        );
    }

    #[test]
    fn eixo_com_valor_invalido_e_reportado() {
        let map = ButtonsMap::parse("axis_x_min = abc
");
        assert_eq!(map.eixo("x_min"), None);
        assert_eq!(map.desconhecidos.len(), 1);
    }

    #[test]
    fn teclas_de_sistema_do_mapa_sao_reconhecidas() {
        // Linhas reais do /etc/giochi/buttons.map do aparelho.
        let map = ButtonsMap::parse(
            "volup   = KEY_VOLUMEUP       # gpio4 PA5\n\
             voldown = KEY_VOLUMEDOWN     # gpio4 PA3\n",
        );
        assert_eq!(map.code("volup"), Some(115));
        assert_eq!(map.code("voldown"), Some(114));
        assert!(map.desconhecidos.is_empty(), "nada pode sobrar sem tradução");
    }

    #[test]
    fn mapa_completo_do_aparelho_nao_tem_simbolo_orfao() {
        // O mapa inteiro, como está no aparelho. Se o sistema acrescentar uma
        // tecla que o app não conhece, este teste é o que avisa.
        let mapa = "up = BTN_DPAD_UP # gpio4 PA0\n\
             down = BTN_DPAD_DOWN # gpio4 PA1\n\
             left = BTN_DPAD_LEFT # gpio3 PD6\n\
             right = BTN_DPAD_RIGHT # gpio3 PD7\n\
             a = BTN_EAST # gpio3 PD3\n\
             b = BTN_SOUTH # gpio3 PD2\n\
             x = BTN_NORTH # gpio3 PD5\n\
             y = BTN_WEST # gpio3 PD4\n\
             l = BTN_TL # gpio3 PD0\n\
             r = BTN_TR # gpio3 PC6\n\
             l2 = BTN_TL2 # gpio3 PD1\n\
             r2 = BTN_TR2 # gpio3 PC7\n\
             start = BTN_START # gpio4 PA2\n\
             select = BTN_SELECT # gpio4 PA4\n\
             home = BTN_MODE\n\
             thumbl = BTN_THUMBL # gpio4 PA7\n\
             thumbr = BTN_THUMBR # gpio4 PB0\n\
             volup = KEY_VOLUMEUP # gpio4 PA5\n\
             voldown = KEY_VOLUMEDOWN # gpio4 PA3\n";
        let map = ButtonsMap::parse(mapa);
        assert!(
            map.desconhecidos.is_empty(),
            "símbolos sem tradução: {:?}",
            map.desconhecidos
        );
        assert!(map.is_complete());
        assert_eq!(map.code("volup"), Some(115));
        assert_eq!(map.code("voldown"), Some(114));
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
            tx: channel().0,
            dir: PathBuf::from("/dev/input"),
            abertos: HashSet::new(),
            proxima_revisao: Instant::now() + Duration::from_secs(3600),
        };
        let frame = input.frame();
        assert!(frame.dpad_up, "hat para cima deve virar dpad_up");

        input.hat = (0, 0);
        input.pressed.insert(0x220, true);
        assert!(input.frame().dpad_up, "botão de dpad também vira dpad_up");
        assert!(!input.frame().dpad_down);
    }
}
