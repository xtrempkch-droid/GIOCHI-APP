//! Cobrinha — segundo jogo embarcado do Giochi.
//!
//! ## Por que um segundo jogo
//!
//! O `volley` provou que o launcher entrega a tela a um app e a recupera. Um
//! app só não testa o que interessa num console: **a lista**. Com dois no
//! catálogo, o launcher precisa desenhar dois cartões, navegar entre eles,
//! iniciar o escolhido e voltar para o cartão certo.
//!
//! Como o `volley`, este é original e pequeno — não é o Snake de nenhum
//! aparelho. Existe para exercitar o caminho com uma segunda entrada real.
//!
//! ## O que reusa, e o que repete
//!
//! Rotação, leitura de `/dev/input`, `--quadro` e `--versao` são **cópias** do
//! `volley`, não uma biblioteca comum. É deliberado: dois jogos independentes
//! não podem quebrar um ao outro, e uma crate compartilhada acoplaria o build
//! dos dois. O que se repete é pequeno e já tem teste do lado do launcher, que
//! é a implementação de referência.
//!
//! ## Controles
//!
//! | Entrada | Ação |
//! | --- | --- |
//! | direcional | muda a direção |
//! | A / START | começa (ou recomeça depois de perder) |
//! | B / SELECT | sai do jogo |
//! | setas, Espaço, Esc (PC) | o mesmo, por teclado |

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

const BTN_DPAD_UP: u16 = 544;
const BTN_DPAD_DOWN: u16 = 545;
const BTN_DPAD_LEFT: u16 = 546;
const BTN_DPAD_RIGHT: u16 = 547;
const BTN_EAST: u16 = 305;
const BTN_SOUTH: u16 = 304;
const BTN_START: u16 = 315;
const BTN_SELECT: u16 = 314;

const EVENT_SIZE: usize = 24;
const EV_KEY: u16 = 0x01;

// Cores do tema, as mesmas do launcher (`theme.rs`), para o jogo não parecer
// de outro produto.
const FUNDO: Color = Color::RGB(11, 12, 20);
const GRADE: Color = Color::RGB(30, 34, 50);
const CORPO: Color = Color::RGB(96, 165, 250);
const CABECA: Color = Color::RGB(147, 197, 253);
const COMIDA: Color = Color::RGB(248, 113, 113);
const TEXTO: Color = Color::RGB(203, 213, 225);

/// Rotação do painel. Mesma convenção e mesma precedência do launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotacao {
    Nenhuma,
    Cw90,
    Ccw90,
}

impl Rotacao {
    fn do_sistema() -> Self {
        if let Ok(texto) = std::env::var("GIOCHI_ROTATE") {
            match texto.trim().to_ascii_lowercase().as_str() {
                "0" | "none" => return Rotacao::Nenhuma,
                "90" | "cw" => return Rotacao::Cw90,
                "270" | "ccw" => return Rotacao::Ccw90,
                _ => {}
            }
        }
        if let Ok(texto) = std::fs::read_to_string("/sys/class/graphics/fbcon/rotate") {
            return match texto.trim() {
                "1" => Rotacao::Cw90,
                "3" => Rotacao::Ccw90,
                _ => Rotacao::Nenhuma,
            };
        }
        Rotacao::Nenhuma
    }

    fn ret(self, design: (i32, i32), x: i32, y: i32, w: i32, h: i32) -> Rect {
        match self {
            Rotacao::Nenhuma => Rect::new(x, y, w.max(0) as u32, h.max(0) as u32),
            Rotacao::Cw90 => Rect::new(design.1 - y - h, x, h.max(0) as u32, w.max(0) as u32),
            Rotacao::Ccw90 => Rect::new(y, design.0 - x - w, h.max(0) as u32, w.max(0) as u32),
        }
    }

    fn design(self, out: (u32, u32)) -> (f32, f32) {
        match self {
            Rotacao::Nenhuma => (out.0 as f32, out.1 as f32),
            _ => (out.1 as f32, out.0 as f32),
        }
    }
}

/// Pintor: guarda a rotação para nenhum `fill_rect` precisar saber dela.
struct Tela {
    rot: Rotacao,
    design: (i32, i32),
}

impl Tela {
    fn retalho(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        self.rot
            .ret(self.design, x as i32, y as i32, w as i32, h as i32)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direcao {
    Cima,
    Baixo,
    Esquerda,
    Direita,
}

impl Direcao {
    fn oposta(self) -> Self {
        match self {
            Direcao::Cima => Direcao::Baixo,
            Direcao::Baixo => Direcao::Cima,
            Direcao::Esquerda => Direcao::Direita,
            Direcao::Direita => Direcao::Esquerda,
        }
    }
}

fn main() -> Result<(), String> {
    if std::env::args().any(|a| a == "--versao" || a == "--version") {
        println!("giochi-cobrinha 0.1.0");
        return Ok(());
    }

    let teclas = Arc::new(Mutex::new(HashSet::new()));
    iniciar_evdev(Arc::clone(&teclas));

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    let janela = video
        .window("Cobrinha", 640, 480)
        .fullscreen_desktop()
        .position_centered()
        .build()
        .map_err(|e| format!("janela: {e}"))?;

    let mut canvas = janela
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| format!("canvas: {e}"))?;

    // Lógico = painel (escala 1,0); projeto = paisagem, com os retângulos
    // girados ao desenhar. Ver `Rotacao`.
    let (pw, ph) = canvas.output_size().map_err(|e| format!("output: {e}"))?;
    canvas
        .set_logical_size(pw, ph)
        .map_err(|e| format!("logical: {e}"))?;

    let rot = Rotacao::do_sistema();
    let (dw, dh) = rot.design((pw, ph));
    let tela = Tela {
        rot,
        design: (dw as i32, dh as i32),
    };

    let info = canvas.info();
    eprintln!(
        "cobrinha: painel {pw}x{ph}, projeto {dw}x{dh} ({rot:?}), renderer {}",
        info.name
    );

    let mut jogo = Jogo::novo(dw, dh);
    let mut events = sdl.event_pump()?;
    let mut ultimo = Instant::now();

    if let Some(i) = std::env::args().position(|a| a == "--quadro") {
        let destino = std::env::args()
            .nth(i + 1)
            .ok_or("--quadro precisa de um caminho")?;
        // Aquecimento: a cobrinha começa com três segmentos no meio; sem andar
        // o quadro não distingue "desenhou" de "limpou a tela".
        for _ in 0..40 {
            jogo.atualizar(1.0 / 8.0, None);
        }
        jogo.desenhar(&mut canvas, &tela);
        salvar_quadro(&mut canvas, &destino)?;
        println!("quadro: {destino} ({pw}x{ph})");
        return Ok(());
    }

    'laco: loop {
        for evento in events.poll_iter() {
            match evento {
                Event::Quit { .. } => break 'laco,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'laco,
                _ => {}
            }
        }

        let agora = Instant::now();
        let dt = agora.duration_since(ultimo).as_secs_f32().min(0.1);
        ultimo = agora;

        let apertado = |codigo: u16| teclas.lock().map(|t| t.contains(&codigo)).unwrap_or(false);
        let teclado = events.keyboard_state();

        // Direção: cada toque é uma intenção. A virada de 180° é recusada em
        // `atualizar`, não aqui — senão o jogador poderia "enganar" o passo
        // apertando duas teclas no mesmo quadro.
        let mut pedido = None;
        if apertado(BTN_DPAD_UP) || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Up) {
            pedido = Some(Direcao::Cima);
        } else if apertado(BTN_DPAD_DOWN)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Down)
        {
            pedido = Some(Direcao::Baixo);
        } else if apertado(BTN_DPAD_LEFT)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Left)
        {
            pedido = Some(Direcao::Esquerda);
        } else if apertado(BTN_DPAD_RIGHT)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Right)
        {
            pedido = Some(Direcao::Direita);
        }

        let acao = apertado(BTN_EAST)
            || apertado(BTN_START)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Space);

        if apertado(BTN_SOUTH) || apertado(BTN_SELECT) {
            break 'laco;
        }

        // `acao` só vale como "começar" quando não está em jogo; apertar A
        // durante a partida não pode reiniciar sem querer.
        if acao && !jogo.rodando {
            jogo.reiniciar();
        }

        jogo.atualizar(dt, pedido);
        jogo.desenhar(&mut canvas, &tela);
        canvas.present();
    }

    println!("cobrinha: fim — {} ponto(s)", jogo.pontos);
    Ok(())
}

/// Lê `/dev/input/event*` cru e mantém o conjunto de códigos pressionados.
fn iniciar_evdev(teclas: Arc<Mutex<HashSet<u16>>>) {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        eprintln!("cobrinha: sem /dev/input — use o teclado");
        return;
    };

    let mut abertos = 0;
    for entrada in dir.flatten() {
        let nome = entrada.file_name();
        if !nome.to_string_lossy().starts_with("event") {
            continue;
        }
        let Ok(arquivo) = File::open(entrada.path()) else {
            continue;
        };

        let teclas = Arc::clone(&teclas);
        abertos += 1;
        std::thread::spawn(move || {
            let mut arquivo = arquivo;
            let mut buf = [0u8; EVENT_SIZE];
            loop {
                if arquivo.read_exact(&mut buf).is_err() {
                    break;
                }
                if u16::from_le_bytes([buf[16], buf[17]]) != EV_KEY {
                    continue;
                }
                let codigo = u16::from_le_bytes([buf[18], buf[19]]);
                let valor = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
                if let Ok(mut conjunto) = teclas.lock() {
                    if valor == 0 {
                        conjunto.remove(&codigo);
                    } else {
                        conjunto.insert(codigo);
                    }
                }
            }
        });
    }

    if abertos == 0 {
        eprintln!("cobrinha: nenhum /dev/input/event* abriu — use o teclado");
    } else {
        eprintln!("cobrinha: {abertos} dispositivo(s) de entrada abertos");
    }
}

/// Estado do jogo, em células de grade.
struct Jogo {
    cols: i32,
    linhas: i32,
    celula: f32,
    origem: (f32, f32),
    corpo: Vec<(i32, i32)>,
    dir: Direcao,
    comida: (i32, i32),
    /// Segundos restantes até o próximo passo. A cobrinha anda em passos, não
    /// continuamente — é o que a torna jogável.
    espera: f32,
    pontos: u32,
    rodando: bool,
    /// Gerador pseudoaleatório de 32 bits. Não vale `rand`: uma dependência a
    /// mais no cross-compile por algo que cabe em três linhas.
    semente: u32,
}

impl Jogo {
    fn novo(largura: f32, altura: f32) -> Self {
        // A grade ocupa a área toda, menos uma margem proporcional. Célula
        // quadrada: o `min` evita células achatadas em painel 4:3.
        let margem = (largura.min(altura) * 0.04).max(8.0);
        let lado = (largura - margem * 2.0).min(altura - margem * 2.0);
        // 24 colunas cabe em 640 de largura e em 1280 sem virar grade de fio de
        // cabelo nem tabuleiro de 5 casas.
        let cols = 24.max(8) as i32;
        let celula = lado / cols as f32;
        let linhas = ((altura - margem * 2.0) / celula) as i32;
        let origem = (
            (largura - celula * cols as f32) / 2.0,
            (altura - celula * linhas as f32) / 2.0,
        );

        let mut jogo = Self {
            cols,
            linhas: linhas.max(6),
            celula,
            origem,
            corpo: Vec::new(),
            dir: Direcao::Direita,
            comida: (0, 0),
            espera: 0.0,
            pontos: 0,
            rodando: true,
            semente: 0x1234_5678,
        };
        jogo.reiniciar();
        jogo
    }

    fn reiniciar(&mut self) {
        let meio = (self.cols / 2, self.linhas / 2);
        self.corpo = vec![meio, (meio.0 - 1, meio.1), (meio.0 - 2, meio.1)];
        self.dir = Direcao::Direita;
        self.pontos = 0;
        self.rodando = true;
        self.espera = 0.0;
        self.sortear_comida();
    }

    fn aleatorio(&mut self) -> u32 {
        // xorshift32: determinístico, sem dependência, bom o bastante para
        // posicionar comida.
        self.semente ^= self.semente << 13;
        self.semente ^= self.semente >> 17;
        self.semente ^= self.semente << 5;
        self.semente
    }

    fn sortear_comida(&mut self) {
        // Tenta posições até achar uma livre. O limite evita laço infinito se a
        // cobrinha ocupar o tabuleiro inteiro (vitória, não bug).
        for _ in 0..200 {
            let x = (self.aleatorio() % self.cols as u32) as i32;
            let y = (self.aleatorio() % self.linhas as u32) as i32;
            if !self.corpo.contains(&(x, y)) {
                self.comida = (x, y);
                return;
            }
        }
        self.comida = (-1, -1);
    }

    fn atualizar(&mut self, dt: f32, pedido: Option<Direcao>) {
        if !self.rodando {
            return;
        }
        if let Some(d) = pedido {
            // Virar 180° mataria o jogador no passo seguinte sem que ele
            // entendesse por quê. Recusar é a regra de todo Snake.
            if d != self.dir.oposta() {
                self.dir = d;
            }
        }

        self.espera -= dt;
        if self.espera > 0.0 {
            return;
        }
        // Acelera com os pontos, com piso: a 0,05 s por passo o jogo fica
        // impossível antes de divertido.
        let passo = (0.16 - self.pontos as f32 * 0.004).max(0.055);
        self.espera = passo;

        let (mut x, mut y) = self.corpo[0];
        match self.dir {
            Direcao::Cima => y -= 1,
            Direcao::Baixo => y += 1,
            Direcao::Esquerda => x -= 1,
            Direcao::Direita => x += 1,
        }

        // Parede e próprio corpo encerram. Sem "atravessa a borda": num
        // tabuleiro pequeno, envolver torna a partida longa demais.
        let bateu = x < 0
            || y < 0
            || x >= self.cols
            || y >= self.linhas
            || self.corpo.contains(&(x, y));
        if bateu {
            self.rodando = false;
            return;
        }

        self.corpo.insert(0, (x, y));
        if (x, y) == self.comida {
            self.pontos += 1;
            self.sortear_comida();
        } else {
            self.corpo.pop();
        }
    }

    fn desenhar(&mut self, canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, tela: &Tela) {
        canvas.set_draw_color(FUNDO);
        canvas.clear();

        let c = self.celula;
        let (ox, oy) = self.origem;

        // Grade discreta: ajuda a julgar distância, e sem ela o campo parece
        // vazio em vez de tabuleiro.
        canvas.set_draw_color(GRADE);
        for i in 0..=self.cols {
            let _ = canvas.fill_rect(tela.retalho(ox + i as f32 * c, oy, 1.0, c * self.linhas as f32));
        }
        for j in 0..=self.linhas {
            let _ = canvas.fill_rect(tela.retalho(ox, oy + j as f32 * c, c * self.cols as f32, 1.0));
        }

        if self.comida.0 >= 0 {
            let m = c * 0.12;
            canvas.set_draw_color(COMIDA);
            let _ = canvas.fill_rect(tela.retalho(
                ox + self.comida.0 as f32 * c + m,
                oy + self.comida.1 as f32 * c + m,
                c - m * 2.0,
                c - m * 2.0,
            ));
        }

        for (i, (x, y)) in self.corpo.iter().enumerate() {
            // Cabeça mais clara: sem isso não se sabe para onde vai.
            canvas.set_draw_color(if i == 0 { CABECA } else { CORPO });
            let m = c * 0.08;
            let _ = canvas.fill_rect(tela.retalho(
                ox + *x as f32 * c + m,
                oy + *y as f32 * c + m,
                c - m * 2.0,
                c - m * 2.0,
            ));
        }

        // Pontos em barras de sete segmentos — sem fonte, como no `volley`.
        let digito = (self.linhas as f32 * c * 0.07).max(10.0) as i32;
        numero(
            canvas,
            tela,
            self.pontos,
            ox as i32 + (c * 0.4) as i32,
            oy as i32 - digito - 4,
            digito,
            TEXTO,
        );
    }
}

/// Desenha um número em sete segmentos.
fn numero(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    tela: &Tela,
    valor: u32,
    x: i32,
    y: i32,
    tamanho: i32,
    cor: Color,
) {
    canvas.set_draw_color(cor);
    let t = (tamanho / 6).max(2);
    let l = tamanho;
    let h = tamanho / 2;
    let casas: Vec<u32> = if valor >= 10 {
        vec![(valor / 10) % 10, valor % 10]
    } else {
        vec![valor % 10]
    };
    let mut deslocamento = 0;
    for casa in casas {
        let ox = x + deslocamento;
        let segs = [
            (0, 0, h + t, t),
            (h, 0, t, l / 2 + t),
            (h, l / 2, t, l / 2 + t),
            (0, l, h + t, t),
            (0, l / 2, t, l / 2 + t),
            (0, 0, t, l / 2 + t),
            (0, l / 2, h + t, t),
        ];
        for (i, s) in segs.into_iter().enumerate() {
            if segmento_aceso(casa, i) {
                let _ = canvas.fill_rect(tela.retalho(
                    (ox + s.0) as f32,
                    (y + s.1) as f32,
                    s.2 as f32,
                    s.3 as f32,
                ));
            }
        }
        deslocamento = h + t * 3;
    }
}

fn segmento_aceso(digito: u32, segmento: usize) -> bool {
    const MAPA: [u8; 10] = [
        0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101,
        0b000_0111, 0b111_1111, 0b110_1111,
    ];
    MAPA[digito as usize % 10] & (1 << segmento) != 0
}

/// Grava o canvas em BMP. Mesma limitação do launcher sobre `read_pixels`.
fn salvar_quadro(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    destino: &str,
) -> Result<(), String> {
    use sdl2::pixels::PixelFormatEnum;
    use sdl2::surface::Surface;

    let (w, h) = canvas.output_size().map_err(|e| format!("output: {e}"))?;
    let formato = PixelFormatEnum::ABGR8888;
    let mut pixels = canvas
        .read_pixels(None, formato)
        .map_err(|e| format!("read_pixels: {e}"))?;
    let pitch = w * formato.byte_size_per_pixel() as u32;
    let surface = Surface::from_data(&mut pixels, w, h, pitch, formato)
        .map_err(|e| format!("surface: {e}"))?;
    surface.save_bmp(destino).map_err(|e| format!("bmp: {e}"))
}
