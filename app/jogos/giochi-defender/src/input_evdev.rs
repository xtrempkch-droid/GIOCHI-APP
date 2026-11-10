//! Leitura do controle por `/dev/input/event*`.
//!
//! ## Por que não a API de gamepad do SDL — e o que isso custou
//!
//! Os botões dos handhelds vêm de driver próprio (`gpio-keys`,
//! `r36s_Gamepad`). O SDL só reconhece um controle que tenha entrada no
//! `SDL_GameControllerDB`, e não existe entrada para aparelho custom: no PC
//! funciona com qualquer controle USB, **no aparelho não funciona com nada**.
//!
//! A primeira versão deste jogo usou `entrada.gamepad()` do SDL. Consequência
//! medida no R35S: o jogo abria, desenhava o menu e **ficava ali para sempre** —
//! nenhum botão chegava, o launcher esperava em `fork`+`wait` e o aparelho
//! parecia travado. O `neon-drift`, que lê evdev, loga `3 dispositivo(s) de
//! entrada` no mesmo aparelho.
//!
//! O `AGENTS.md` §12 já tinha a decisão escrita ("Entrada por evdev, não por SDL
//! gamepad") e o launcher já a aplicava. Este módulo a traz para cá.
//!
//! ## É uma cópia, não uma dependência
//!
//! Como o `rotate_rect` do `volley`: o launcher tem o seu `input_evdev.rs`, e este
//! jogo tem o seu. São apps diferentes; um não deve poder reclamar do outro
//! quando quebrar. O que se copia é a **decisão** (abrir os eventos crus, usar o
//! `buttons.map` do sistema), não o arquivo.
//!
//! ## O mapa é do sistema, o tradutor é do app
//!
//! `a = BTN_EAST` no R35S e `a = BTN_SOUTH` no X55: o símbolo lógico é o mesmo e
//! o código muda com o gabarito. Fixar `BTN_SOUTH` como "confirmar" funcionaria
//! num aparelho e no outro trocaria A por B — em silêncio.
//!
//! ## Limite conhecido, herdado do launcher
//!
//! Só botões e hat digital. O analógico (`ABS_X`/`ABS_Y`) precisa de `absinfo` por
//! ioctl para normalizar a faixa do ADC, o que exige `libc`. A câmera aqui anda
//! pelo direcional, que é o que o mapa declara.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};

/// Tamanho de `struct input_event` em aarch64/x86_64:
/// `timeval` (16) + type (2) + code (2) + value (4).
const TAMANHO_DO_EVENTO: usize = 24;

const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

/// Caminho padrão do mapa, instalado pelo Giochi System.
pub const MAPA_PADRAO: &str = "/etc/giochi/buttons.map";
/// Onde os dispositivos de entrada aparecem.
pub const DIRETORIO_DE_ENTRADA: &str = "/dev/input";

/// Um evento lido de um dispositivo.
#[derive(Debug, Clone, Copy)]
enum Ev {
    Key(u16, i32),
    Hat(u16, i32),
}

/// Tradução entre nome lógico e código evdev.
#[derive(Debug, Default, Clone)]
pub struct ButtonsMap {
    por_nome: HashMap<String, u16>,
    /// Símbolos que o arquivo declarou e este app não conhece.
    ///
    /// Dizer que não sabe é obrigatório: um símbolo descartado em silêncio vira
    /// botão morto sem nenhuma pista. Aconteceu duas vezes no launcher.
    desconhecidos: Vec<String>,
}

impl ButtonsMap {
    /// Carrega o mapa do sistema, ou devolve um padrão se o arquivo não existir.
    pub fn carregar(caminho: &Path) -> Self {
        match fs::read_to_string(caminho) {
            Ok(texto) => Self::parse(&texto),
            Err(erro) => {
                log::warn!("controle: não consegui ler {caminho:?} ({erro}); usando o mapa padrão");
                Self::padrao()
            }
        }
    }

    /// Interpreta o texto de um `buttons.map`.
    ///
    /// O formato é `nome = SIMBOLO  # comentário`, e o `#` **precisa** ser
    /// cortado: o mapa real traz um comentário no fim de cada linha, e sem cortar
    /// o símbolo vira `"BTN_DPAD_UP  # gpio4 PA0"` — que não casa com nada. Foi
    /// assim que o launcher ficou com 1 de 13 botões.
    pub fn parse(texto: &str) -> Self {
        let mut mapa = Self::default();
        for linha in texto.lines() {
            let linha = linha.split('#').next().unwrap_or("").trim();
            if linha.is_empty() {
                continue;
            }
            let Some((nome, simbolo)) = linha.split_once('=') else {
                continue;
            };
            let nome = nome.trim().to_ascii_lowercase();
            let simbolo = simbolo.trim();
            // As chaves `axis_*` são calibração do analógico, não botão.
            if nome.starts_with("axis_") {
                continue;
            }
            match code_for(simbolo) {
                Some(codigo) => {
                    mapa.por_nome.insert(nome, codigo);
                }
                None => mapa.desconhecidos.push(simbolo.to_string()),
            }
        }
        mapa
    }

    /// Mapa mínimo, para quando não há arquivo nenhum.
    ///
    /// Cobre o layout mais comum (X55, X35H, R35S): A no leste, B no sul, o
    /// direcional em `BTN_DPAD_*`.
    pub fn padrao() -> Self {
        let texto = "\
up = BTN_DPAD_UP
down = BTN_DPAD_DOWN
left = BTN_DPAD_LEFT
right = BTN_DPAD_RIGHT
a = BTN_EAST
b = BTN_SOUTH
x = BTN_NORTH
y = BTN_WEST
l = BTN_TL
r = BTN_TR
l2 = BTN_TL2
r2 = BTN_TR2
start = BTN_START
select = BTN_SELECT
volup = KEY_VOLUMEUP
voldown = KEY_VOLUMEDOWN
";
        let mut mapa = Self::parse(texto);
        mapa.desconhecidos.clear();
        mapa
    }

    /// Código evdev de um nome lógico.
    pub fn code(&self, nome: &str) -> Option<u16> {
        self.por_nome.get(nome).copied()
    }

    /// Símbolos que o arquivo declarou e este app não traduz.
    pub fn desconhecidos(&self) -> &[String] {
        &self.desconhecidos
    }

    /// Todos os pares (nome, código), em ordem de nome.
    pub fn pares(&self) -> Vec<(String, u16)> {
        let mut pares: Vec<(String, u16)> = self
            .por_nome
            .iter()
            .map(|(nome, codigo)| (nome.clone(), *codigo))
            .collect();
        pares.sort();
        pares
    }

    /// Quantos nomes o mapa conhece.
    pub fn len(&self) -> usize {
        self.por_nome.len()
    }

    /// O mapa está vazio?
    pub fn is_empty(&self) -> bool {
        self.por_nome.is_empty()
    }
}

/// Tradução de símbolo evdev (`BTN_EAST`) para código.
///
/// Escrito à mão em vez de gerado: são 30 símbolos, e uma tabela explícita é
/// auditável. O teste `todo_simbolo_da_tabela_e_um_codigo_btN_valido` confere a
/// faixa, e `o_mapa_real_do_r35s_e_traduzido_inteiro` roda com o arquivo do
/// aparelho de verdade.
pub fn code_for(simbolo: &str) -> Option<u16> {
    Some(match simbolo {
        // Gamepad: faixa BTN_ 0x130..=0x13f
        "BTN_SOUTH" => 0x130,
        "BTN_EAST" => 0x131,
        "BTN_NORTH" => 0x133,
        "BTN_WEST" => 0x134,
        "BTN_TL" => 0x136,
        "BTN_TR" => 0x137,
        "BTN_TL2" => 0x138,
        "BTN_TR2" => 0x139,
        "BTN_SELECT" => 0x13a,
        "BTN_START" => 0x13b,
        "BTN_MODE" => 0x13c,
        "BTN_THUMBL" => 0x13d,
        "BTN_THUMBR" => 0x13e,
        // Direcional: também faixa BTN_, e é por isso que parece "código sem
        // nome" quando alguém procura em KEY_*.
        "BTN_DPAD_UP" => 0x220,
        "BTN_DPAD_DOWN" => 0x221,
        "BTN_DPAD_LEFT" => 0x222,
        "BTN_DPAD_RIGHT" => 0x223,
        // Teclas do sistema
        "KEY_POWER" => 116,
        "KEY_VOLUMEDOWN" => 114,
        "KEY_VOLUMEUP" => 115,
        "KEY_ENTER" => 28,
        "KEY_ESC" => 1,
        "KEY_BACK" => 158,
        "KEY_HOME" => 102,
        _ => return None,
    })
}

/// Estado do controle num quadro.
///
/// Duas leituras do mesmo botão, porque a UI precisa das duas: **nível** para a
/// câmera (segurar gira) e **borda** para o menu (apertar move um item). Misturar
/// as duas é o que faz um menu rolar sozinho enquanto o botão está apertado.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Controles {
    /// Direcional pressionado agora.
    pub cima: bool,
    /// Ver [`Controles::cima`].
    pub baixo: bool,
    /// Ver [`Controles::cima`].
    pub esquerda: bool,
    /// Ver [`Controles::cima`].
    pub direita: bool,
    /// `a` do mapa (confirmar / atirar).
    pub confirmar: bool,
    /// `b` do mapa (voltar).
    pub voltar: bool,
    /// `r` (ombro direito) — atirar sem soltar o direcional.
    pub atirar: bool,
    /// `start`.
    pub menu: bool,
    /// Borda de descida: o botão passou de solto a apertado neste quadro.
    pub cima_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub baixo_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub esquerda_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub direita_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub confirmar_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub voltar_apertou: bool,
    /// Ver [`Controles::cima_apertou`].
    pub menu_apertou: bool,
    /// Tecla de volume para cima, neste quadro.
    pub volume_mais: bool,
    /// Tecla de volume para baixo, neste quadro.
    pub volume_menos: bool,
}

impl Controles {
    /// Recalcula as bordas comparando com o quadro anterior.
    ///
    /// O leitor evdev chama isto por quadro. A **fonte de reserva** (o gamepad do
    /// SDL) também precisa, senão o menu rola sozinho enquanto o botão fica
    /// apertado: o SDL só entrega nível.
    pub fn com_bordas(&mut self, anterior: &Controles) {
        self.cima_apertou = self.cima && !anterior.cima;
        self.baixo_apertou = self.baixo && !anterior.baixo;
        self.esquerda_apertou = self.esquerda && !anterior.esquerda;
        self.direita_apertou = self.direita && !anterior.direita;
        self.confirmar_apertou = self.confirmar && !anterior.confirmar;
        self.voltar_apertou = self.voltar && !anterior.voltar;
        self.menu_apertou = self.menu && !anterior.menu;
    }
}

/// Leitor de todos os `/dev/input/event*`.
pub struct EvdevInput {
    rx: Receiver<Ev>,
    mapa: ButtonsMap,
    pressionado: HashMap<u16, bool>,
    anterior: Controles,
    atual: Controles,
    hat: (i32, i32),
    dispositivos: usize,
    nomes: Vec<String>,
    _threads: Vec<JoinHandle<()>>,
}

impl EvdevInput {
    /// Abre todos os dispositivos de entrada do diretório.
    ///
    /// Não filtra por capacidade: um teclado também é aberto, e os códigos que
    /// não interessam nunca aparecem no mapa. Se nenhum dispositivo abrir, o
    /// leitor fica vazio em vez de falhar — o jogo ainda abre (só não recebe
    /// botão), e o log diz isso em vez de morrer.
    pub fn abrir(diretorio: &Path, mapa: ButtonsMap) -> Self {
        let (tx, rx) = channel();
        let mut threads = Vec::new();
        let mut nomes = Vec::new();

        if let Ok(entradas) = fs::read_dir(diretorio) {
            let mut caminhos: Vec<PathBuf> = entradas
                .filter_map(|entrada| entrada.ok())
                .map(|entrada| entrada.path())
                .filter(|caminho| {
                    caminho
                        .file_name()
                        .and_then(|nome| nome.to_str())
                        .map(|nome| nome.starts_with("event"))
                        .unwrap_or(false)
                })
                .collect();
            caminhos.sort();

            for caminho in caminhos {
                if let Some(nome) = nome_do_dispositivo(&caminho) {
                    nomes.push(nome);
                }
                if let Some(thread) = spawn_leitor(caminho, tx.clone()) {
                    threads.push(thread);
                }
            }
        }

        if threads.is_empty() {
            log::warn!(
                "controle: nenhum dispositivo em {diretorio:?} pôde ser aberto; \
                 os botões não vão responder"
            );
        } else {
            log::info!(
                "controle: {} dispositivo(s) de entrada, {} símbolo(s) no mapa",
                threads.len(),
                mapa.len()
            );
        }
        if !mapa.desconhecidos.is_empty() {
            log::warn!(
                "controle: {} símbolo(s) do mapa não reconhecido(s): {}",
                mapa.desconhecidos.len(),
                mapa.desconhecidos.join(", ")
            );
        }

        Self {
            rx,
            mapa,
            pressionado: HashMap::new(),
            anterior: Controles::default(),
            atual: Controles::default(),
            hat: (0, 0),
            dispositivos: threads.len(),
            nomes,
            _threads: threads,
        }
    }

    /// Quantos dispositivos foram abertos.
    /// O evdev achou algum controle.
    ///
    /// Quando **não** acha — num PC sem controle, ou com `/dev/input`
    /// inacessível — quem responde é a fonte de reserva (o gamepad do SDL).
    /// Quando acha, o mapa do **sistema** é autoritativo: é ele que sabe que
    /// `a` é `BTN_EAST` neste aparelho e `BTN_SOUTH` no outro.
    pub fn ativo(&self) -> bool {
        self.dispositivos > 0
    }

    pub fn dispositivos(&self) -> usize {
        self.dispositivos
    }

    /// Nomes dos dispositivos abertos.
    pub fn nomes(&self) -> &[String] {
        &self.nomes
    }

    /// O mapa em uso.
    pub fn mapa(&self) -> &ButtonsMap {
        &self.mapa
    }

    /// Drena o canal e recalcula o estado. Uma vez por quadro.
    pub fn poll(&mut self) {
        while let Ok(evento) = self.rx.try_recv() {
            match evento {
                // `value == 2` é autorepeat: para botão de controle não existe,
                // mas o teclado do console manda, e tratar como "apertou"
                // encheria o menu de repetições.
                Ev::Key(codigo, valor) if valor != 2 => {
                    self.pressionado.insert(codigo, valor == 1);
                }
                Ev::Key(_, _) => {}
                Ev::Hat(codigo, valor) => match codigo {
                    ABS_HAT0X => self.hat.0 = valor,
                    ABS_HAT0Y => self.hat.1 = valor,
                    _ => {}
                },
            }
        }

        let nivel = |nome: &str| -> bool {
            self.mapa
                .code(nome)
                .and_then(|codigo| self.pressionado.get(&codigo).copied())
                .unwrap_or(false)
        };

        self.anterior = self.atual;
        self.atual = Controles {
            cima: nivel("up") || self.hat.1 < 0,
            baixo: nivel("down") || self.hat.1 > 0,
            esquerda: nivel("left") || self.hat.0 < 0,
            direita: nivel("right") || self.hat.0 > 0,
            confirmar: nivel("a"),
            voltar: nivel("b"),
            atirar: nivel("r"),
            menu: nivel("start"),
            volume_mais: nivel("volup"),
            volume_menos: nivel("voldown"),
            // Bordas preenchidas abaixo.
            cima_apertou: false,
            baixo_apertou: false,
            esquerda_apertou: false,
            direita_apertou: false,
            confirmar_apertou: false,
            voltar_apertou: false,
            menu_apertou: false,
        };
        self.atual.com_bordas(&self.anterior);
    }

    /// Estado do quadro.
    pub fn controles(&self) -> Controles {
        self.atual
    }
}

/// Nome legível do dispositivo, de `/sys/class/input/eventN/device/name`.
fn nome_do_dispositivo(caminho: &Path) -> Option<String> {
    let nome = caminho.file_name()?.to_str()?;
    let sys = format!("/sys/class/input/{nome}/device/name");
    fs::read_to_string(sys)
        .ok()
        .map(|texto| texto.trim().to_string())
}

/// Uma thread por dispositivo, todas mandando para o mesmo canal.
fn spawn_leitor(caminho: PathBuf, tx: Sender<Ev>) -> Option<JoinHandle<()>> {
    let mut arquivo = File::open(&caminho).ok()?;

    let thread = thread::spawn(move || {
        let mut buffer = [0u8; TAMANHO_DO_EVENTO];
        loop {
            if arquivo.read_exact(&mut buffer).is_err() {
                // Dispositivo removido ou erro de leitura.
                break;
            }
            let tipo = u16::from_ne_bytes([buffer[16], buffer[17]]);
            let codigo = u16::from_ne_bytes([buffer[18], buffer[19]]);
            let valor = i32::from_ne_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]);
            let evento = match tipo {
                EV_KEY => Ev::Key(codigo, valor),
                EV_ABS => Ev::Hat(codigo, valor),
                _ => continue,
            };
            if tx.send(evento).is_err() {
                // O receptor sumiu: o jogo fechou.
                break;
            }
        }
    });
    Some(thread)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O mapa real do R35S, copiado do aparelho.
    const MAPA_R35S: &str = "\
#
# Gerado por scripts/dtb-buttons.mjs a partir do device tree.
# O DTS é a fonte da verdade: se o hardware divergir, corrigir aqui.

# ── botões ──────────────────────────────────────────────────
up      = BTN_DPAD_UP      # GPIO DPAD-UP
down    = BTN_DPAD_DOWN    # GPIO DPAD-DOWN
left    = BTN_DPAD_LEFT    # GPIO DPAD-LEFT
right   = BTN_DPAD_RIGHT   # GPIO DPAD-RIGHT
a       = BTN_EAST         # GPIO KEY BTN-A
b       = BTN_SOUTH        # GPIO BTN-B
x       = BTN_NORTH        # GPIO BTN-X
y       = BTN_WEST         # GPIO BTN-Y
l       = BTN_TL           # GPIO TOP-LEFT
r       = BTN_TR           # GPIO TOP-RIGHT
l2      = BTN_TL2          # GPIO TOP-LEFT2
r2      = BTN_TR2          # GPIO TOP-RIGHT2
start   = BTN_START        # GPIO BTN_START
select  = BTN_SELECT       # GPIO BTN_SELECT
home    = BTN_MODE         # GPIO BTN_FN
thumbl  = BTN_THUMBL       # GPIO BTN_THUMBL
thumbr  = BTN_THUMBR       # GPIO BTN_THUMBR
volup   = KEY_VOLUMEUP     # VOLUMEUP
voldown = KEY_VOLUMEDOWN   # VOLUMEDOWN
axis_x_min = 15
axis_x_max = 1023
";

    #[test]
    fn o_mapa_real_do_r35s_e_traduzido_inteiro() {
        // O arquivo do aparelho, com comentário no fim de cada linha e as chaves
        // `axis_*` no fim. Nenhum símbolo pode ficar de fora: desconhecido é
        // botão morto em silêncio.
        let mapa = ButtonsMap::parse(MAPA_R35S);
        assert!(
            mapa.desconhecidos().is_empty(),
            "símbolos não traduzidos: {:?}",
            mapa.desconhecidos()
        );
        assert!(mapa.len() >= 19, "só {} símbolos no mapa", mapa.len());
        for nome in [
            "up", "down", "left", "right", "a", "b", "x", "y", "l", "r", "l2", "r2", "start",
            "select", "home", "thumbl", "thumbr", "volup", "voldown",
        ] {
            assert!(mapa.code(nome).is_some(), "{nome} ficou de fora");
        }
    }

    #[test]
    fn o_comentario_do_fim_da_linha_e_cortado() {
        // A armadilha que deixou o launcher com 1 de 13 botões: sem cortar o
        // `#`, o símbolo vira "BTN_DPAD_UP      # GPIO DPAD-UP" e não casa.
        let mapa = ButtonsMap::parse("up = BTN_DPAD_UP      # GPIO DPAD-UP");
        assert_eq!(mapa.code("up"), Some(0x220));
    }

    #[test]
    fn a_calibragem_do_analogico_nao_vira_simbolo_desconhecido() {
        let mapa = ButtonsMap::parse(MAPA_R35S);
        assert!(
            mapa.desconhecidos().is_empty(),
            "as chaves axis_* foram contadas como desconhecidas"
        );
        assert_eq!(mapa.code("axis_x_min"), None);
    }

    #[test]
    fn simbolo_desconhecido_e_reportado_e_nao_engolido() {
        let mapa = ButtonsMap::parse("a = BTN_EAST\nz = BTN_INEXISTENTE\n");
        assert_eq!(mapa.code("a"), Some(0x131));
        assert_eq!(mapa.desconhecidos(), &["BTN_INEXISTENTE".to_string()]);
    }

    #[test]
    fn o_mesmo_nome_logico_aponta_para_codigos_diferentes_conforme_o_aparelho() {
        // R35S: `a = BTN_EAST` (0x131). X55: `a = BTN_SOUTH` (0x130). Fixar
        // BTN_SOUTH como "confirmar" trocaria A por B num dos aparelhos, em
        // silêncio — é a razão de existir o mapa.
        let r35s = ButtonsMap::parse("a = BTN_EAST");
        let x55 = ButtonsMap::parse("a = BTN_SOUTH");
        assert_eq!(r35s.code("a"), Some(0x131));
        assert_eq!(x55.code("a"), Some(0x130));
        assert_ne!(r35s.code("a"), x55.code("a"));
    }

    #[test]
    fn todo_simbolo_da_tabela_tem_codigo_plausivel() {
        // `BTN_*` fica em 0x100..=0x2ff e `KEY_*` abaixo de 0x100 (as duas
        // faixas que o kernel usa para botão de controle e tecla de sistema).
        for simbolo in [
            "BTN_SOUTH",
            "BTN_EAST",
            "BTN_NORTH",
            "BTN_WEST",
            "BTN_TL",
            "BTN_TR",
            "BTN_TL2",
            "BTN_TR2",
            "BTN_SELECT",
            "BTN_START",
            "BTN_MODE",
            "BTN_THUMBL",
            "BTN_THUMBR",
            "BTN_DPAD_UP",
            "BTN_DPAD_DOWN",
            "BTN_DPAD_LEFT",
            "BTN_DPAD_RIGHT",
        ] {
            let codigo = code_for(simbolo).unwrap_or_else(|| panic!("{simbolo} ausente"));
            assert!(
                (0x100..=0x2ff).contains(&codigo),
                "{simbolo} = {codigo:#x}, fora da faixa BTN_"
            );
        }
        for simbolo in ["KEY_POWER", "KEY_VOLUMEUP", "KEY_VOLUMEDOWN"] {
            let codigo = code_for(simbolo).unwrap();
            assert!(
                codigo < 0x100,
                "{simbolo} = {codigo:#x}, fora da faixa KEY_"
            );
        }
        // E os valores que o AGENTS.md já decodificou do kernel, um a um.
        assert_eq!(code_for("KEY_POWER"), Some(116));
        assert_eq!(code_for("KEY_VOLUMEDOWN"), Some(114));
        assert_eq!(code_for("KEY_VOLUMEUP"), Some(115));
        assert_eq!(code_for("BTN_DPAD_UP"), Some(0x220));
        assert_eq!(code_for("BTN_DPAD_RIGHT"), Some(0x223));
        assert_eq!(code_for("BTN_SOUTH"), Some(0x130));
        assert_eq!(code_for("BTN_EAST"), Some(0x131));
    }

    #[test]
    fn o_mapa_padrao_cobre_o_que_o_jogo_usa() {
        let mapa = ButtonsMap::padrao();
        assert!(mapa.desconhecidos().is_empty());
        for nome in [
            "up", "down", "left", "right", "a", "b", "r", "start", "volup", "voldown",
        ] {
            assert!(mapa.code(nome).is_some(), "o padrão não tem {nome}");
        }
    }

    #[test]
    fn linha_sem_igual_ou_vazia_e_ignorada() {
        let mapa = ButtonsMap::parse("\n# só comentário\nlixo\n=\nx = BTN_TL\n");
        assert_eq!(mapa.len(), 1);
        assert_eq!(mapa.code("x"), Some(0x136));
    }

    #[test]
    fn sem_diretorio_nenhum_dispositivo_e_aberto_sem_panico() {
        // O jogo tem de abrir mesmo sem /dev/input (PC em contêiner, R35S sem
        // permissão). O que não pode é morrer.
        let leitor = EvdevInput::abrir(Path::new("/nao/existe"), ButtonsMap::padrao());
        assert_eq!(leitor.dispositivos(), 0);
        assert!(leitor.nomes().is_empty());
    }

    #[test]
    fn sem_evento_o_estado_e_tudo_solto() {
        let mut leitor = EvdevInput::abrir(Path::new("/nao/existe"), ButtonsMap::padrao());
        leitor.poll();
        let c = leitor.controles();
        assert!(!c.confirmar);
        assert!(!c.cima);
        assert!(!c.atirar);
        assert!(!c.confirmar_apertou, "borda não pode nascer apertada");
    }

    /// A reserva é o gamepad do SDL, que **só entrega nível**. Sem derivar a
    /// borda, o menu rolaria sozinho enquanto o direcional estivesse apertado —
    /// e no PC, onde é a única fonte, isso seria o comportamento normal.
    #[test]
    fn a_fonte_de_reserva_deriva_a_borda_do_nivel() {
        let solto = Controles::default();
        let mut apertado = Controles {
            cima: true,
            confirmar: true,
            ..Controles::default()
        };
        apertado.com_bordas(&solto);
        assert!(apertado.cima_apertou, "0 -> 1 é borda");
        assert!(apertado.confirmar_apertou);

        // Segundo quadro com o botão ainda apertado: o nível continua, a borda
        // não. É o que impede o menu de rolar a 60 passos por segundo.
        let mut segurando = apertado;
        segurando.com_bordas(&apertado);
        assert!(segurando.cima, "o nível tem de continuar");
        assert!(!segurando.cima_apertou, "segurar não é apertar de novo");
        assert!(!segurando.confirmar_apertou);
    }

    /// O `ativo()` decide **quem manda**. Ele não pode responder "sim" só porque
    /// o módulo existe: num PC sem controle aberto, o evdev fica vazio e a
    /// reserva é a única fonte. Errar isso é o jogo não responder no PC ou
    /// responder duas vezes no aparelho.
    #[test]
    fn sem_dispositivo_o_evdev_se_declara_inativo() {
        let leitor = EvdevInput::abrir(Path::new("/nao/existe"), ButtonsMap::padrao());
        assert!(!leitor.ativo());
        assert_eq!(leitor.dispositivos(), 0);
    }
}
