//! Pac-Man — terceiro jogo embarcado do Giochi.
//!
//! ## Por que um terceiro jogo
//!
//! `volley` provou o caminho launcher→app→launcher. `cobrinha` provou a lista
//! com dois itens. Este existe por um motivo diferente: é o primeiro que
//! exercita **ciclo de vida com estado** — pastilhas que somem, uma pausa quando
//! o jogador perde, recomeço, e quatro agentes com comportamento distinto. Se
//! algo no launcher mexe com tempo, foco ou reentrada, quebra aqui antes de
//! quebrar em jogo de verdade.
//!
//! ## O que é original
//!
//! O labirinto, a arte e o desenho são deste repositório. Nada do Pac-Man
//! original foi copiado: nem mapa, nem sprite, nem som. O que se reconhece é o
//! gênero — comer pastilhas, fugir de fantasmas — que não é de ninguém.
//!
//! ## Controles
//!
//! | Entrada | Ação |
//! | --- | --- |
//! | direcional | vira na próxima esquina possível |
//! | A / START | começa ou recomeça |
//! | B / SELECT | sai |
//! | setas, Espaço, Esc (PC) | o mesmo |

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

const FUNDO: Color = Color::RGB(6, 7, 14);
const PAREDE: Color = Color::RGB(37, 62, 160);
const PAREDE_LUZ: Color = Color::RGB(70, 108, 226);
const PASTILHA: Color = Color::RGB(232, 216, 168);
const HEROI: Color = Color::RGB(250, 204, 21);
const ASSUSTADO: Color = Color::RGB(80, 120, 255);

/// O labirinto.
///
/// `#` parede, `.` pastilha, `o` pastilha grande, ` ` corredor vazio, `-` a
/// porta da casa dos fantasmas.
///
/// 21×21 de propósito: ímpar para haver simetria em torno da coluna central, e
/// pequeno o bastante para caber legível em 640×480. Um mapa maior ficaria
/// bonito no X55 e ilegível no X35H — e o alvo é o aparelho pequeno.
const MAPA: [&str; 21] = [
    "#####################",
    "#.........#.........#",
    "#o##.####.#.####.##o#",
    "#.##.####.#.####.##.#",
    "#...................#",
    "#.##.#.#######.#.##.#",
    "#....#....#....#....#",
    "####.#### # ####.####",
    "   #.#    -    #.#   ",
    "####.# ##   ## #.####",
    "    .  #     #  .    ",
    "####.# ####### #.####",
    "   #.#         #.#   ",
    "####.# ####### #.####",
    "#.........#.........#",
    "#.##.####.#.####.##.#",
    "#o.#.....   .....#.o#",
    "##.#.#.#######.#.#.##",
    "#....#....#....#....#",
    "#.########.########.#",
    "#####################",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direcao {
    Cima,
    Baixo,
    Esquerda,
    Direita,
    Parado,
}

impl Direcao {
    fn delta(self) -> (i32, i32) {
        match self {
            Direcao::Cima => (0, -1),
            Direcao::Baixo => (0, 1),
            Direcao::Esquerda => (-1, 0),
            Direcao::Direita => (1, 0),
            Direcao::Parado => (0, 0),
        }
    }

    fn oposta(self) -> Self {
        match self {
            Direcao::Cima => Direcao::Baixo,
            Direcao::Baixo => Direcao::Cima,
            Direcao::Esquerda => Direcao::Direita,
            Direcao::Direita => Direcao::Esquerda,
            Direcao::Parado => Direcao::Parado,
        }
    }

    fn todas() -> [Self; 4] {
        [
            Direcao::Cima,
            Direcao::Esquerda,
            Direcao::Baixo,
            Direcao::Direita,
        ]
    }
}

/// Rotação do painel. Mesma convenção e precedência do launcher.
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

/// Tamanho de painel pedido em `--tamanho LxA`.
///
/// Existe pelo mesmo motivo que no launcher, e um a mais: os três jogos só
/// tinham o painel do aparelho ou o desktop do PC (1024×768 no `dummy`). Sem
/// isto, conferir 640×480 ou 800×600 exigiria o hardware na mão — e as contas de
/// grade do `cobrinha` e do `pacman` dependem do tamanho.
///
/// Com `SDL_VIDEODRIVER=dummy` o tamanho pedido é respeitado, então dá para
/// olhar qualquer painel.
fn tamanho_pedido(args: &[String]) -> Option<(u32, u32)> {
    let i = args.iter().position(|a| a == "--tamanho")?;
    let texto = args.get(i + 1)?;
    let (l, a) = texto.split_once(['x', 'X'])?;
    let l: u32 = l.trim().parse().ok()?;
    let a: u32 = a.trim().parse().ok()?;
    if l == 0 || a == 0 {
        return None;
    }
    Some((l, a))
}

fn main() -> Result<(), String> {
    if std::env::args().any(|a| a == "--versao" || a == "--version") {
        println!("giochi-pacman 0.1.0");
        return Ok(());
    }

    let teclas = Arc::new(Mutex::new(HashSet::new()));
    iniciar_evdev(Arc::clone(&teclas));

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    // `--tamanho` só faz sentido com driver que respeite o pedido (`dummy`).
    // No aparelho, `fullscreen_desktop` usa o modo do conector, que é o certo.
    let args: Vec<String> = std::env::args().collect();
    let pedido = tamanho_pedido(&args);

    let mut construtor = video.window(
        "Pac-Man",
        pedido.map(|(l, _)| l).unwrap_or(640),
        pedido.map(|(_, a)| a).unwrap_or(480),
    );
    if pedido.is_none() {
        construtor.fullscreen_desktop();
    }
    let janela = construtor
        .position_centered()
        .build()
        .map_err(|e| format!("janela: {e}"))?;

    let mut canvas = janela
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| format!("canvas: {e}"))?;

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
        "pacman: painel {pw}x{ph}, projeto {dw}x{dh} ({rot:?}), renderer {}",
        info.name
    );

    let mut jogo = Jogo::novo(dw, dh);
    let mut events = sdl.event_pump()?;
    let mut ultimo = Instant::now();

    if let Some(i) = std::env::args().position(|a| a == "--quadro") {
        let destino = std::env::args()
            .nth(i + 1)
            .ok_or("--quadro precisa de um caminho")?;
        for _ in 0..50 {
            jogo.atualizar(1.0 / 30.0);
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

        // A direção pedida é guardada, não aplicada: só vale quando houver
        // corredor. É o que faz virar esquina sem parar no meio.
        if apertado(BTN_DPAD_UP) || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Up) {
            jogo.pedido = Direcao::Cima;
        } else if apertado(BTN_DPAD_DOWN)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Down)
        {
            jogo.pedido = Direcao::Baixo;
        } else if apertado(BTN_DPAD_LEFT)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Left)
        {
            jogo.pedido = Direcao::Esquerda;
        } else if apertado(BTN_DPAD_RIGHT)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Right)
        {
            jogo.pedido = Direcao::Direita;
        }

        if apertado(BTN_EAST)
            || apertado(BTN_START)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Space)
        {
            if !jogo.vivo {
                jogo.reiniciar();
            }
        }

        if apertado(BTN_SOUTH) || apertado(BTN_SELECT) {
            break 'laco;
        }

        jogo.atualizar(dt);
        jogo.desenhar(&mut canvas, &tela);
        canvas.present();
    }

    println!(
        "pacman: fim — {} ponto(s), {} vida(s)",
        jogo.pontos, jogo.vidas
    );
    Ok(())
}

fn iniciar_evdev(teclas: Arc<Mutex<HashSet<u16>>>) {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        eprintln!("pacman: sem /dev/input — use o teclado");
        return;
    };
    let mut abertos = 0;
    for entrada in dir.flatten() {
        if !entrada.file_name().to_string_lossy().starts_with("event") {
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
        eprintln!("pacman: nenhum /dev/input/event* abriu — use o teclado");
    } else {
        eprintln!("pacman: {abertos} dispositivo(s) de entrada abertos");
    }
}

/// Um fantasma: posição, direção e personalidade.
struct Fantasma {
    x: i32,
    y: i32,
    dir: Direcao,
    /// Como escolhe a esquina. Quatro regras diferentes fazem quatro
    /// perseguições diferentes com o mesmo código.
    humor: Humor,
    /// Quadros até poder virar de novo. Sem isso ele oscila numa esquina.
    trava: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Humor {
    /// Vai direto atrás do herói.
    Cacador,
    /// Mira quatro casas à frente, como quem tenta cortar o caminho.
    Emboscada,
    /// Persegue a posição que o herói tinha há alguns quadros.
    Lento,
    /// Anda ao acaso nas bifurcações.
    Aleatorio,
}

struct Jogo {
    cols: i32,
    linhas: i32,
    celula: f32,
    origem: (f32, f32),
    /// `true` = parede. Índice `y * cols + x`.
    parede: Vec<bool>,
    /// Pastilhas que ainda existem.
    pastilha: Vec<bool>,
    /// Pastilha grande (assusta fantasmas).
    grande: Vec<bool>,
    heroi: (i32, i32),
    dir: Direcao,
    pedido: Direcao,
    fantasmas: Vec<Fantasma>,
    /// Segundos por casa. Menor = mais rápido.
    passo: f32,
    acumulado: f32,
    pontos: u32,
    vidas: u32,
    vivo: bool,
    /// Segundos restantes de susto. Zero = fantasmas perigosos.
    susto: f32,
    semente: u32,
    /// Últimos quadros do herói, para o fantasma `Lento` perseguir o passado.
    rastro: Vec<(i32, i32)>,
}

impl Jogo {
    fn novo(largura: f32, altura: f32) -> Self {
        let linhas = MAPA.len() as i32;
        let cols = MAPA[0].len() as i32;

        let mut parede = Vec::with_capacity((cols * linhas) as usize);
        let mut pastilha = Vec::with_capacity((cols * linhas) as usize);
        let mut grande = Vec::with_capacity((cols * linhas) as usize);
        for (y, linha) in MAPA.iter().enumerate() {
            for (x, c) in linha.chars().enumerate() {
                parede.push(c == '#');
                pastilha.push(c == '.');
                grande.push(c == 'o');
                let _ = (x, y);
            }
        }

        // A célula é o menor dos dois eixos, para o labirinto caber inteiro sem
        // deformar quadrado em retângulo.
        let margem = 6.0;
        let celula = ((largura - margem * 2.0) / cols as f32)
            .min((altura - margem * 2.0) / linhas as f32);
        let origem = (
            (largura - celula * cols as f32) / 2.0,
            (altura - celula * linhas as f32) / 2.0,
        );

        let mut jogo = Self {
            cols,
            linhas,
            celula,
            origem,
            parede,
            pastilha,
            grande,
            heroi: (10, 15),
            dir: Direcao::Parado,
            pedido: Direcao::Parado,
            fantasmas: Vec::new(),
            passo: 0.15,
            acumulado: 0.0,
            pontos: 0,
            vidas: 3,
            vivo: true,
            susto: 0.0,
            semente: 0x9e37_79b9,
            rastro: Vec::new(),
        };
        jogo.reiniciar();
        jogo
    }

    fn reiniciar(&mut self) {
        self.heroi = (10, 15);
        self.dir = Direcao::Parado;
        self.pedido = Direcao::Parado;
        self.vivo = true;
        self.susto = 0.0;
        self.acumulado = 0.0;
        self.rastro.clear();
        self.fantasmas = vec![
            Fantasma {
                x: 9,
                y: 9,
                dir: Direcao::Esquerda,
                humor: Humor::Cacador,
                trava: 0.0,
            },
            Fantasma {
                x: 10,
                y: 9,
                dir: Direcao::Direita,
                humor: Humor::Emboscada,
                trava: 0.0,
            },
            Fantasma {
                x: 11,
                y: 9,
                dir: Direcao::Cima,
                humor: Humor::Lento,
                trava: 0.0,
            },
            Fantasma {
                x: 10,
                y: 10,
                dir: Direcao::Baixo,
                humor: Humor::Aleatorio,
                trava: 0.0,
            },
        ];
    }

    fn aleatorio(&mut self) -> u32 {
        self.semente ^= self.semente << 13;
        self.semente ^= self.semente >> 17;
        self.semente ^= self.semente << 5;
        self.semente
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.cols || y >= self.linhas {
            None
        } else {
            Some((y * self.cols + x) as usize)
        }
    }

    fn eh_parede(&self, x: i32, y: i32) -> bool {
        // Fora do mapa conta como parede: evita ter que tratar borda em cada
        // movimento.
        match self.idx(x, y) {
            Some(i) => self.parede[i],
            None => true,
        }
    }

    fn atualizar(&mut self, dt: f32) {
        if !self.vivo {
            return;
        }
        if self.susto > 0.0 {
            self.susto = (self.susto - dt).max(0.0);
        }

        self.acumulado += dt;
        if self.acumulado < self.passo {
            return;
        }
        self.acumulado = 0.0;

        // Herói: tenta o pedido; se não der, segue; se não der, para.
        for candidata in [self.pedido, self.dir] {
            if candidata == Direcao::Parado {
                continue;
            }
            let (dx, dy) = candidata.delta();
            if !self.eh_parede(self.heroi.0 + dx, self.heroi.1 + dy) {
                self.dir = candidata;
                break;
            }
        }
        let (dx, dy) = self.dir.delta();
        if self.dir != Direcao::Parado && !self.eh_parede(self.heroi.0 + dx, self.heroi.1 + dy) {
            self.heroi = (self.heroi.0 + dx, self.heroi.1 + dy);
        }

        self.rastro.push(self.heroi);
        if self.rastro.len() > 12 {
            self.rastro.remove(0);
        }

        // Comer.
        if let Some(i) = self.idx(self.heroi.0, self.heroi.1) {
            if self.pastilha[i] {
                self.pastilha[i] = false;
                self.pontos += 10;
            } else if self.grande[i] {
                self.grande[i] = false;
                self.pontos += 50;
                // Tempo suficiente para atravessar o labirinto e voltar, e
                // curto o bastante para não virar passeio.
                self.susto = 6.0;
            }
        }

        // Fantasmas. Cada um escolhe direção na sua vez.
        let alvo_heroi = self.heroi;
        let alvo_emboscada = (
            self.heroi.0 + self.dir.delta().0 * 4,
            self.heroi.1 + self.dir.delta().1 * 4,
        );
        let alvo_lento = self
            .rastro
            .first()
            .copied()
            .unwrap_or(self.heroi);
        let assustados = self.susto > 0.0;

        for i in 0..self.fantasmas.len() {
            let (fx, fy, humor, dir_atual, trava) = {
                let f = &self.fantasmas[i];
                (f.x, f.y, f.humor, f.dir, f.trava)
            };

            let mut nova_dir = dir_atual;
            let mut nova_trava = (trava - self.passo).max(0.0);

            if nova_trava <= 0.0 {
                let alvo = if assustados {
                    // Assustado foge: escolhe a direção que mais afasta.
                    (fx - (alvo_heroi.0 - fx), fy - (alvo_heroi.1 - fy))
                } else {
                    match humor {
                        Humor::Cacador => alvo_heroi,
                        Humor::Emboscada => alvo_emboscada,
                        Humor::Lento => alvo_lento,
                        Humor::Aleatorio => (
                            (self.aleatorio() % self.cols as u32) as i32,
                            (self.aleatorio() % self.linhas as u32) as i32,
                        ),
                    }
                };

                // Avalia as quatro direções menos a oposta (não dá meia-volta),
                // e fica com a que chega mais perto do alvo. Empatou, mantém.
                let mut melhor = None;
                let mut melhor_d = i32::MAX;
                for d in Direcao::todas() {
                    if d == dir_atual.oposta() {
                        continue;
                    }
                    let (ddx, ddy) = d.delta();
                    if self.eh_parede(fx + ddx, fy + ddy) {
                        continue;
                    }
                    let dist = (alvo.0 - (fx + ddx)).abs() + (alvo.1 - (fy + ddy)).abs();
                    if dist < melhor_d {
                        melhor_d = dist;
                        melhor = Some(d);
                    }
                }
                // Sem saída à frente (beco): aceita a meia-volta.
                if melhor.is_none() {
                    for d in Direcao::todas() {
                        let (ddx, ddy) = d.delta();
                        if !self.eh_parede(fx + ddx, fy + ddy) {
                            melhor = Some(d);
                            break;
                        }
                    }
                }
                if let Some(d) = melhor {
                    if d != dir_atual {
                        nova_dir = d;
                        // Trava curta ao virar: impede tremer na esquina entre
                        // duas opções de mesma distância.
                        nova_trava = self.passo * 2.0;
                    }
                }
            }

            let (dx, dy) = nova_dir.delta();
            let nx = fx + dx;
            let ny = fy + dy;
            let pode = !self.eh_parede(nx, ny);

            // Encontro: mesma casa ou troca de casa (um passa pelo outro).
            let encostou = pode
                && ((nx, ny) == self.heroi
                    || ((nx, ny) == (self.heroi.0 - self.dir.delta().0, self.heroi.1 - self.dir.delta().1)
                        && (fx, fy) == self.heroi));

            if encostou {
                if assustados {
                    // Comido: volta para a casa e continua.
                    let f = &mut self.fantasmas[i];
                    f.x = 10;
                    f.y = 9;
                    f.dir = Direcao::Baixo;
                    f.trava = self.passo * 4.0;
                    self.pontos += 200;
                    continue;
                }
                // Perdeu uma vida. Recomeça o mapa: mais simples que reposicionar
                // tudo, e o jogador entende.
                self.vidas -= 1;
                if self.vidas == 0 {
                    self.vivo = false;
                } else {
                    let pontos = self.pontos;
                    let vidas = self.vidas;
                    let passo = self.passo;
                    let semente = self.semente;
                    self.reiniciar();
                    self.pontos = pontos;
                    self.vidas = vidas;
                    self.passo = passo;
                    self.semente = semente;
                }
                return;
            }

            let f = &mut self.fantasmas[i];
            f.dir = nova_dir;
            f.trava = nova_trava;
            if pode {
                f.x = nx;
                f.y = ny;
            }
        }

        if !self.pastilha.iter().any(|p| *p) {
            // Limpou o labirinto: recomeça com tudo, mantendo a pontuação.
            let pontos = self.pontos;
            let vidas = self.vidas;
            self.pastilha = MAPA
                .iter()
                .flat_map(|l| l.chars().map(|c| c == '.'))
                .collect();
            self.grande = MAPA
                .iter()
                .flat_map(|l| l.chars().map(|c| c == 'o'))
                .collect();
            self.pontos = pontos + 1000;
            self.vidas = vidas;
            self.reiniciar();
            self.pontos = pontos + 1000;
            self.vidas = vidas;
        }
    }

    fn desenhar(&mut self, canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, tela: &Tela) {
        canvas.set_draw_color(FUNDO);
        canvas.clear();

        let c = self.celula;
        let (ox, oy) = self.origem;

        for y in 0..self.linhas {
            for x in 0..self.cols {
                let i = (y * self.cols + x) as usize;
                let px = ox + x as f32 * c;
                let py = oy + y as f32 * c;

                if self.parede[i] {
                    canvas.set_draw_color(PAREDE);
                    let _ = canvas.fill_rect(tela.retalho(px, py, c - 1.0, c - 1.0));
                    // Brilho no topo da parede: dá relevo sem sprite.
                    canvas.set_draw_color(PAREDE_LUZ);
                    let _ = canvas.fill_rect(tela.retalho(px, py, c - 1.0, (c * 0.18).max(1.0)));
                    continue;
                }

                if self.pastilha[i] {
                    canvas.set_draw_color(PASTILHA);
                    let m = c * 0.34;
                    let _ = canvas.fill_rect(tela.retalho(
                        px + m,
                        py + m,
                        (c * 0.32).max(1.0),
                        (c * 0.32).max(1.0),
                    ));
                }
                if self.grande[i] {
                    canvas.set_draw_color(PASTILHA);
                    let m = c * 0.22;
                    let _ = canvas.fill_rect(tela.retalho(px + m, py + m, c - m * 2.0, c - m * 2.0));
                }
            }
        }

        // Fantasmas: corpo quadrado com um "saiote" embaixo, que é o que os
        // torna reconhecíveis sem sprite.
        for f in &self.fantasmas {
            let px = ox + f.x as f32 * c;
            let py = oy + f.y as f32 * c;
            let m = c * 0.12;
            let mut cor = match f.humor {
                Humor::Cacador => Color::RGB(239, 68, 68),
                Humor::Emboscada => Color::RGB(236, 145, 210),
                Humor::Lento => Color::RGB(249, 168, 87),
                Humor::Aleatorio => Color::RGB(94, 234, 212),
            };
            if self.susto > 0.0 {
                // Pisca no fim do susto: avisa que a proteção está acabando.
                cor = if self.susto < 2.0 && ((self.susto * 6.0) as i32) % 2 == 0 {
                    Color::RGB(240, 240, 240)
                } else {
                    ASSUSTADO
                };
            }
            canvas.set_draw_color(cor);
            let _ = canvas.fill_rect(tela.retalho(px + m, py + m, c - m * 2.0, c - m * 2.0));
        }

        // Herói: losango (dois retângulos girados de 45° não cabem sem
        // `copy_ex`; um quadrado menor e um triângulo achatado já lê como boca).
        {
            let px = ox + self.heroi.0 as f32 * c;
            let py = oy + self.heroi.1 as f32 * c;
            let m = c * 0.12;
            canvas.set_draw_color(HEROI);
            let _ = canvas.fill_rect(tela.retalho(px + m, py + m, c - m * 2.0, c - m * 2.0));
            // "Boca": um entalhe escuro na direção do movimento.
            let (dx, dy) = self.dir.delta();
            canvas.set_draw_color(FUNDO);
            let boca = c * 0.28;
            let bx = px + c / 2.0 - boca / 2.0 + dx as f32 * c * 0.24;
            let by = py + c / 2.0 - boca / 2.0 + dy as f32 * c * 0.24;
            let _ = canvas.fill_rect(tela.retalho(bx, by, boca, boca));
        }

        // Pontos e vidas na linha acima do labirinto, em sete segmentos.
        let digito = (c * 0.62) as i32;
        let ty = (oy - digito as f32 - 3.0).max(0.0) as i32;
        numero(canvas, tela, self.pontos, (ox + c * 0.5) as i32, ty, digito, PASTILHA);
        for v in 0..self.vidas {
            canvas.set_draw_color(HEROI);
            let r = c * 0.3;
            let _ = canvas.fill_rect(tela.retalho(
                ox + c * (self.cols as f32 - 1.2) - v as f32 * c * 0.9,
                (oy - r * 2.0 - 2.0).max(0.0),
                r,
                r,
            ));
        }

        if !self.vivo {
            let w = c * 7.0;
            let h = c * 2.0;
            let x = ox + (c * self.cols as f32 - w) / 2.0;
            let y = oy + (c * self.linhas as f32 - h) / 2.0;
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            let _ = canvas.fill_rect(tela.retalho(x - 4.0, y - 4.0, w + 8.0, h + 8.0));
            canvas.set_draw_color(Color::RGB(239, 68, 68));
            let _ = canvas.fill_rect(tela.retalho(x, y + h / 2.0 - 2.0, w, 4.0));
        }
    }
}

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
    let t = (tamanho / 6).max(1);
    let l = tamanho;
    let h = tamanho / 2;
    let mut casas: Vec<u32> = Vec::new();
    let mut v = valor;
    loop {
        casas.insert(0, v % 10);
        v /= 10;
        if v == 0 {
            break;
        }
    }
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
        deslocamento = h + t * 2;
    }
}

fn segmento_aceso(digito: u32, segmento: usize) -> bool {
    const MAPA: [u8; 10] = [
        0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101,
        0b000_0111, 0b111_1111, 0b110_1111,
    ];
    MAPA[digito as usize % 10] & (1 << segmento) != 0
}

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
