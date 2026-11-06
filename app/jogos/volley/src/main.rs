//! Volley — jogo de exemplo do Giochi.
//!
//! ## Por que este jogo existe
//!
//! O catálogo declarava quatro jogos, todos projetos Godot que nunca foram
//! migrados (`bin/` vazio) e que o aparelho não teria como rodar: o X35H não tem
//! runtime Godot. Isso deixava uma pergunta sem resposta prática — *o launcher
//! consegue entregar o aparelho a um jogo e recebê-lo de volta?* — porque não
//! havia nenhum jogo para testar.
//!
//! Este é um jogo de verdade, pequeno e original, escrito para exercitar
//! exatamente esse caminho: abre o KMSDRM, lê o direcional por `/dev/input`,
//! desenha, e sai devolvendo a tela.
//!
//! ## O que ele deliberadamente não usa
//!
//! - **Fonte.** Não linka `SDL2_ttf` nem freetype. O placar é desenhado em
//!   segmentos, então o jogo roda em imagem sem biblioteca de texto.
//! - **`libasound`.** Não toca som. Uma dependência a menos para o link
//!   cross-compilado falhar.
//! - **Rotação.** Desenha em paisagem no tamanho real do painel. No X35H
//!   (`rotate:0`, 640×480) é o caso natural. Num painel retrato girado a imagem
//!   sairia deitada — limitação conhecida e registrada em `docs/jogo.md`.
//!
//! ## Controles
//!
//! | Entrada | Ação |
//! | --- | --- |
//! | direcional ↑ ↓ | move a raquete |
//! | A / START | saque |
//! | B / SELECT | sai do jogo |
//! | Esc, setas, Espaço (PC) | o mesmo, por teclado |

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

// ── códigos evdev usados ───────────────────────────────────
// Os mesmos que o `buttons.map` do aparelho declara. Não lemos o mapa: o mapa
// traduz *nome lógico* para código, e o jogo só precisa do código.
const BTN_DPAD_UP: u16 = 544;
const BTN_DPAD_DOWN: u16 = 545;
const BTN_EAST: u16 = 305;
const BTN_SOUTH: u16 = 304;
const BTN_START: u16 = 315;
const BTN_SELECT: u16 = 314;

/// `struct input_event` em aarch64/x86_64: timeval (16) + type(2) + code(2) + value(4).
const EVENT_SIZE: usize = 24;
const EV_KEY: u16 = 0x01;

/// Rotação do painel, na mesma convenção do launcher.
///
/// ## Por que o jogo precisa disto
///
/// O jogo desenha em **espaço de projeto**, que é paisagem. O X35H tem painel
/// 640×480 com `rotate=0` e serve a imagem como está; o X55 tem painel
/// **720×1280 retrato** com `fbcon=rotate:3`, e sem girar o desenho o jogo saía
/// espremido no canto superior do buffer — medido, 720×1280 com conteúdo nos
/// primeiros 480 px.
///
/// A matemática é a mesma de `app/launcher/crates/ui-sdl/src/draw.rs`, que já
/// tem teste de ida e volta. Duplicar 12 linhas é melhor que criar dependência
/// entre o launcher e o jogo: são apps diferentes, e um não pode reclamar do
/// outro quando quebrar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotacao {
    Nenhuma,
    Cw90,
    Ccw90,
}

impl Rotacao {
    /// Lê a orientação: `GIOCHI_ROTATE` primeiro, depois o que o sistema
    /// declara.
    ///
    /// A variável existe para testar num PC, onde não há
    /// `/sys/class/graphics/fbcon/rotate`. É a mesma precedência do launcher
    /// (`crates/ui-sdl/src/display.rs`), de propósito: um comando que gira um
    /// app tem de girar o outro, senão o teste mente sobre um dos dois.
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
                // `rotate=2` (180°) não é suportado nem no launcher: girar
                // meia volta exigiria desenhar de cabeça para baixo, e nenhum
                // painel do projeto precisa disso.
                _ => Rotacao::Nenhuma,
            };
        }
        Rotacao::Nenhuma
    }

    /// Retângulo do espaço de projeto → espaço de dispositivo.
    fn ret(self, design: (i32, i32), x: i32, y: i32, w: i32, h: i32) -> Rect {
        match self {
            Rotacao::Nenhuma => Rect::new(x, y, w.max(0) as u32, h.max(0) as u32),
            // Girar 90° troca largura por altura: um retângulo alinhado aos
            // eixos continua alinhado, então as medidas apenas invertem.
            Rotacao::Cw90 => {
                Rect::new(design.1 - y - h, x, h.max(0) as u32, w.max(0) as u32)
            }
            Rotacao::Ccw90 => {
                Rect::new(y, design.0 - x - w, h.max(0) as u32, w.max(0) as u32)
            }
        }
    }

    /// Tamanho do espaço de projeto para um painel de `out` pixels.
    fn design(self, out: (u32, u32)) -> (f32, f32) {
        match self {
            Rotacao::Nenhuma => (out.0 as f32, out.1 as f32),
            _ => (out.1 as f32, out.0 as f32),
        }
    }
}

/// Pintor do jogo: guarda a rotação para que **nenhum** `fill_rect` precise
/// saber dela.
///
/// Já esteve espalhado — cada chamada convertia à mão, e a primeira que
/// esquecesse sairia torta sem aviso.
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
        println!("giochi-volley 0.1.0");
        return Ok(());
    }

    let teclas = Arc::new(Mutex::new(HashSet::new()));
    iniciar_evdev(Arc::clone(&teclas));

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    // `fullscreen_desktop` deixa o `kmsdrm` usar o modo do conector. Pedir um
    // tamanho fixo num painel diferente do pedido produz letterbox ou recusa —
    // e o tamanho real só é conhecido depois de abrir.
    // `--tamanho` só faz sentido com driver que respeite o pedido (`dummy`).
    // No aparelho, `fullscreen_desktop` usa o modo do conector, que é o certo.
    let args: Vec<String> = std::env::args().collect();
    let pedido = tamanho_pedido(&args);

    let mut construtor = video.window(
        "Volley",
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

    // O espaço lógico é sempre o **do painel**, então a escala é 1,0 e nada
    // encolhe. Quem se adapta é o espaço de projeto: num painel retrato ele é a
    // paisagem de medidas trocadas, e cada retângulo é girado ao ser desenhado.
    //
    // Isto já esteve errado: o jogo desenhava paisagem num buffer retrato e o
    // resultado no X55 foi conteúdo espremido nos primeiros 480 px de 1280.
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
        "volley: painel {pw}x{ph}, projeto {dw}x{dh} ({rot:?}), renderer {}",
        info.name
    );

    let mut jogo = Jogo::novo(dw, dh);
    let mut events = sdl.event_pump()?;
    let mut ultimo = Instant::now();

    // Fotografa um quadro e sai. Existe pelo mesmo motivo que no launcher: numa
    // máquina sem tela é a única forma de olhar o desenho, e no PC o
    // `SDL_VIDEODRIVER=dummy` recria o painel do aparelho.
    if let Some(i) = std::env::args().position(|a| a == "--quadro") {
        let destino = std::env::args()
            .nth(i + 1)
            .ok_or("--quadro precisa de um caminho")?;
        // Alguns quadros de aquecimento: no primeiro a bola está no centro e o
        // placar zerado, que não distingue "desenhou" de "limpou a tela".
        for _ in 0..90 {
            jogo.atualizar(1.0 / 60.0, false, false, false);
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
        let dt = agora.duration_since(ultimo).as_secs_f32().min(0.05);
        ultimo = agora;

        let apertado = |codigo: u16| teclas.lock().map(|t| t.contains(&codigo)).unwrap_or(false);
        let teclado = events.keyboard_state();

        let cima = apertado(BTN_DPAD_UP) || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Up);
        let baixo =
            apertado(BTN_DPAD_DOWN) || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Down);
        let saque = apertado(BTN_EAST)
            || apertado(BTN_START)
            || teclado.is_scancode_pressed(sdl2::keyboard::Scancode::Space);
        let sair = apertado(BTN_SOUTH) || apertado(BTN_SELECT);

        if sair {
            break 'laco;
        }

        jogo.atualizar(dt, cima, baixo, saque);
        jogo.desenhar(&mut canvas, &tela);
        canvas.present();
    }

    // A pontuação vai para o log: é como se confere, por SSH, que o jogo rodou
    // de verdade e não só abriu uma janela preta.
    println!("volley: fim — jogador {} x {} cpu", jogo.pontos_jogador, jogo.pontos_cpu);
    Ok(())
}

/// Lê `/dev/input/event*` cru e mantém um conjunto de códigos pressionados.
///
/// Thread por dispositivo, todas escrevendo no mesmo conjunto. Mesma decisão do
/// launcher (`input_evdev.rs`): a API de gamepad do SDL depende de entrada no
/// `SDL_GameControllerDB`, que não existe para handheld custom.
fn iniciar_evdev(teclas: Arc<Mutex<HashSet<u16>>>) {
    let Ok(dir) = std::fs::read_dir("/dev/input") else {
        eprintln!("volley: sem /dev/input — use o teclado (setas + espaço)");
        return;
    };

    let mut abertos = 0;
    for entrada in dir.flatten() {
        let nome = entrada.file_name();
        let nome = nome.to_string_lossy();
        if !nome.starts_with("event") {
            continue;
        }
        let caminho = entrada.path();
        let Ok(arquivo) = File::open(&caminho) else {
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
                let tipo = u16::from_le_bytes([buf[16], buf[17]]);
                if tipo != EV_KEY {
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
        eprintln!("volley: nenhum /dev/input/event* abriu — use o teclado");
    } else {
        eprintln!("volley: {abertos} dispositivo(s) de entrada abertos");
    }
}

/// Estado do jogo. Coordenadas em pixels do painel.
struct Jogo {
    largura: f32,
    altura: f32,
    raquete_y: f32,
    raquete_h: f32,
    cpu_y: f32,
    bola_x: f32,
    bola_y: f32,
    vel_x: f32,
    vel_y: f32,
    raio: f32,
    em_jogo: bool,
    pontos_jogador: u32,
    pontos_cpu: u32,
    /// Margem lateral das raquetes, proporcional à largura.
    margem: f32,
}

impl Jogo {
    fn novo(largura: f32, altura: f32) -> Self {
        let mut jogo = Self {
            largura,
            altura,
            raquete_y: altura / 2.0,
            // Proporcional ao painel: 12% da altura cabe em 480 e em 1280 sem
            // virar uma barra que ocupa a tela inteira.
            raquete_h: (altura * 0.12).max(28.0),
            cpu_y: altura / 2.0,
            bola_x: largura / 2.0,
            bola_y: altura / 2.0,
            vel_x: 0.0,
            vel_y: 0.0,
            raio: (altura * 0.014).max(4.0),
            em_jogo: false,
            pontos_jogador: 0,
            pontos_cpu: 0,
            margem: (largura * 0.03).max(12.0),
        };
        jogo.sacar();
        jogo
    }

    fn sacar(&mut self) {
        self.bola_x = self.largura / 2.0;
        self.bola_y = self.altura / 2.0;
        // A bola sai devagar: 0,42 altura/s dá ~1,2 s para atravessar a tela no
        // X35H, tempo de o jogador achar a raquete.
        let velocidade = self.altura * 0.42;
        self.vel_x = if self.pontos_jogador > self.pontos_cpu {
            -velocidade
        } else {
            velocidade
        };
        self.vel_y = velocidade * 0.45;
        self.em_jogo = true;
    }

    fn atualizar(&mut self, dt: f32, cima: bool, baixo: bool, saque: bool) {
        // Raquete: 0,9 altura/s. Rápido o bastante para cruzar a tela em ~1 s,
        // e não tanto que um toque curto leve a raquete de ponta a ponta.
        let passo = self.altura * 0.9 * dt;
        if cima {
            self.raquete_y -= passo;
        }
        if baixo {
            self.raquete_y += passo;
        }
        let metade = self.raquete_h / 2.0;
        self.raquete_y = self.raquete_y.clamp(metade, self.altura - metade);

        if !self.em_jogo {
            if saque {
                self.sacar();
            }
            return;
        }

        self.bola_x += self.vel_x * dt;
        self.bola_y += self.vel_y * dt;

        // Teto e piso.
        if self.bola_y < self.raio {
            self.bola_y = self.raio;
            self.vel_y = self.vel_y.abs();
        }
        if self.bola_y > self.altura - self.raio {
            self.bola_y = self.altura - self.raio;
            self.vel_y = -self.vel_y.abs();
        }

        // CPU: persegue a bola com velocidade limitada, senão é invencível.
        let alvo = self.bola_y;
        let vel_cpu = self.altura * 0.55 * dt;
        if (alvo - self.cpu_y).abs() > vel_cpu {
            self.cpu_y += if alvo > self.cpu_y { vel_cpu } else { -vel_cpu };
        } else {
            self.cpu_y = alvo;
        }
        self.cpu_y = self.cpu_y.clamp(metade, self.altura - metade);

        let raquete_x = self.margem;
        let cpu_x = self.largura - self.margem - self.largura * 0.018;

        // Colisão por faixa: a bola está na coluna da raquete e dentro da altura
        // dela. Aproximação suficiente e sem canto mágico.
        if self.vel_x < 0.0
            && self.bola_x - self.raio <= raquete_x + self.largura * 0.018
            && self.bola_x > raquete_x
            && (self.bola_y - self.raquete_y).abs() <= metade + self.raio
        {
            self.bola_x = raquete_x + self.largura * 0.018 + self.raio;
            self.acelerar(1.0);
        }

        if self.vel_x > 0.0
            && self.bola_x + self.raio >= cpu_x
            && self.bola_x < cpu_x + self.largura * 0.018
            && (self.bola_y - self.cpu_y).abs() <= metade + self.raio
        {
            self.bola_x = cpu_x - self.raio;
            self.acelerar(-1.0);
        }

        // Ponto.
        if self.bola_x < -self.raio * 4.0 {
            self.pontos_cpu += 1;
            self.em_jogo = false;
            if self.pontos_cpu >= 7 {
                self.zera_placar();
            }
        } else if self.bola_x > self.largura + self.raio * 4.0 {
            self.pontos_jogador += 1;
            self.em_jogo = false;
            if self.pontos_jogador >= 7 {
                self.zera_placar();
            }
        }
    }

    /// Rebate somando velocidade, com teto: sem teto a bola atravessa a raquete
    /// entre dois quadros e a partida vira sorte.
    fn acelerar(&mut self, direcao: f32) {
        let max = self.altura * 0.95;
        self.vel_x = (self.vel_x.abs() * 1.06).min(max) * direcao;
        // Ângulo varia com o ponto de impacto, mas mantém componente vertical.
        let desvio = (self.bola_y - self.altura / 2.0) / (self.altura / 2.0);
        self.vel_y = (self.vel_x * 0.55 * desvio).clamp(-max * 0.7, max * 0.7);
    }

    fn zera_placar(&mut self) {
        self.pontos_jogador = 0;
        self.pontos_cpu = 0;
    }

    fn desenhar(&mut self, canvas: &mut sdl2::render::Canvas<sdl2::video::Window>, tela: &Tela) {
        canvas.set_draw_color(Color::RGB(11, 12, 20));
        canvas.clear();

        // Rede central: tracejado proporcional, para não virar linha cheia em
        // painel pequeno.
        canvas.set_draw_color(Color::RGB(44, 48, 70));
        let passo = (self.altura / 24.0).max(6.0);
        let mut y = 0.0;
        while y < self.altura {
            let _ = canvas.fill_rect(tela.retalho(
                self.largura / 2.0 - 1.0,
                y,
                2.0,
                passo * 0.55,
            ));
            y += passo;
        }

        let espessura = (self.largura * 0.018).max(6.0);

        canvas.set_draw_color(Color::RGB(96, 165, 250));
        let _ = canvas.fill_rect(tela.retalho(
            self.margem,
            self.raquete_y - self.raquete_h / 2.0,
            espessura,
            self.raquete_h,
        ));

        canvas.set_draw_color(Color::RGB(248, 113, 113));
        let _ = canvas.fill_rect(tela.retalho(
            self.largura - self.margem - espessura,
            self.cpu_y - self.raquete_h / 2.0,
            espessura,
            self.raquete_h,
        ));

        if self.em_jogo {
            canvas.set_draw_color(Color::RGB(241, 245, 249));
            let d = self.raio * 2.0;
            let _ = canvas.fill_rect(tela.retalho(
                self.bola_x - self.raio,
                self.bola_y - self.raio,
                d,
                d,
            ));
        }

        // Placar em sete segmentos — sem fonte, sem `SDL2_ttf`.
        let digito = (self.altura * 0.09) as i32;
        let topo = (self.altura * 0.06) as i32;
        let branco = Color::RGB(203, 213, 225);
        numero(
            canvas,
            tela,
            self.pontos_jogador,
            (self.largura * 0.30) as i32,
            topo,
            digito,
            branco,
        );
        numero(
            canvas,
            tela,
            self.pontos_cpu,
            (self.largura * 0.62) as i32,
            topo,
            digito,
            branco,
        );
    }
}

/// Desenha um número em sete segmentos.
///
/// Existe porque o jogo não linka biblioteca de fonte: num aparelho sem
/// `libSDL2_ttf` qualquer texto dependeria de uma biblioteca a mais, e o placar
/// é a única coisa que precisa ser lida.
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
    let t = (tamanho / 6).max(2); // espessura do segmento
    let h = tamanho / 2; // comprimento horizontal
    // Uma casa só quando o valor tem um dígito. Desenhar a dezena sempre fazia
    // o placar zerado sair como `00  00`: o dobro do espaço, e o jogador lê
    // como erro de desenho.
    let casas: Vec<u32> = if valor >= 10 {
        vec![(valor / 10) % 10, valor % 10]
    } else {
        vec![valor % 10]
    };
    let mut deslocamento = 0;
    for casa in casas {
        let ox = x + deslocamento;
        // Segmentos: a topo, b sup. dir., c inf. dir., d base,
        //            e inf. esq., f sup. esq., g meio.
        let a = (0, 0, h + t, t);
        let l = tamanho;
        let b = (h, 0, t, l / 2 + t);
        let c = (h, l / 2, t, l / 2 + t);
        let d = (0, l, h + t, t);
        let e = (0, l / 2, t, l / 2 + t);
        let f = (0, 0, t, l / 2 + t);
        let g = (0, l / 2, h + t, t);
        for (i, seg) in [a, b, c, d, e, f, g].into_iter().enumerate() {
            if segmento_aceso(casa, i) {
                let _ = canvas.fill_rect(tela.retalho(
                    (ox + seg.0) as f32,
                    (y + seg.1) as f32,
                    seg.2 as f32,
                    seg.3 as f32,
                ));
            }
        }
        deslocamento = h + t * 3;
    }
}

fn segmento_aceso(digito: u32, segmento: usize) -> bool {
    // Bit 0 = a ... bit 6 = g.
    const MAPA: [u8; 10] = [
        0b011_1111, // 0
        0b000_0110, // 1
        0b101_1011, // 2
        0b100_1111, // 3
        0b110_0110, // 4
        0b110_1101, // 5
        0b111_1101, // 6
        0b000_0111, // 7
        0b111_1111, // 8
        0b110_1111, // 9
    ];
    let padrao = MAPA[digito as usize % 10];
    padrao & (1 << segmento) != 0
}

/// Grava o canvas atual em BMP.
///
/// Mesma limitação conhecida do launcher: `read_pixels` **não respeita o
/// deslocamento do viewport**. Aqui a escala é sempre 1,0 (projeto = painel),
/// então o viewport não desloca e a imagem fica correta — mas a observação vale
/// para quem for comparar posições.
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
