//! Cobrinha — o jogo do console Giochi.
//!
//! ## Como roda no aparelho
//!
//! SDL2 sobre **KMSDRM**, sem X11, sem Wayland, sem compositor. O launcher já
//! provou esse caminho em três aparelhos; aqui ele é repetido com as mesmas
//! regras: orientação lida do sistema, escala 1,0, entrada por evdev.
//!
//! ## Uso
//!
//! ```text
//! giochi-cobrinha                        # joga
//! giochi-cobrinha --versao
//! giochi-cobrinha --quadro /tmp/q.bmp    # desenha um quadro e sai (CI)
//! giochi-cobrinha --tamanho 640x480 --quadro q.bmp
//! giochi-cobrinha --passos 40 --quadro q.bmp   # adianta a simulação e fotografa
//! giochi-cobrinha --fps-teto 45
//! ```
//!
//! `GIOCHI_ROTATE` sobrepõe a orientação, para testar no PC. Mesma precedência
//! do launcher, de propósito: um comando que gira um app tem de girar o outro,
//! senão o teste mente sobre um dos dois.

mod desenho;
mod evdev;
mod video;

use std::time::{Duration, Instant};

use giochi_cobrinha_core::{
    Acao, Direcao, Estado, Evento, Geometria, Jogo, Relogio, ALVO_FPS, COLUNAS,
};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::surface::Surface;

use desenho::{Tela, APAGADO, CABECA, COMIDA, CORPO, TEXTO};

const VERSAO: &str = env!("CARGO_PKG_VERSION");

struct Opcoes {
    quadro: Option<String>,
    tamanho: Option<(u32, u32)>,
    passos: u32,
    teclas: Vec<Acao>,
    quadros: Option<u32>,
    semente: u64,
    fps: u32,
    json: bool,
    ajuda: bool,
    versao: bool,
}

fn opcoes(argv: Vec<String>) -> Result<Opcoes, String> {
    let mut o = Opcoes {
        quadro: None,
        tamanho: None,
        passos: 0,
        teclas: Vec::new(),
        quadros: None,
        semente: 20261106,
        fps: ALVO_FPS,
        json: false,
        ajuda: false,
        versao: false,
    };
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        match arg {
            "--quadro" => {
                o.quadro = Some(argv.get(i + 1).ok_or("--quadro sem caminho")?.clone());
                i += 2;
            }
            "--tamanho" => {
                let v = argv.get(i + 1).ok_or("--tamanho sem valor")?;
                let (l, a) = v.split_once('x').ok_or("--tamanho espera LARGURAxALTURA")?;
                o.tamanho = Some((
                    l.parse().map_err(|_| "largura inválida")?,
                    a.parse().map_err(|_| "altura inválida")?,
                ));
                i += 2;
            }
            "--passos" => {
                o.passos = argv
                    .get(i + 1)
                    .ok_or("--passos sem valor")?
                    .parse()
                    .map_err(|_| "--passos espera um número")?;
                i += 2;
            }
            "--quadros" => {
                o.quadros = Some(
                    argv.get(i + 1)
                        .ok_or("--quadros sem valor")?
                        .parse()
                        .map_err(|_| "--quadros espera um número")?,
                );
                i += 2;
            }
            "--semente" => {
                o.semente = argv
                    .get(i + 1)
                    .ok_or("--semente sem valor")?
                    .parse()
                    .map_err(|_| "--semente espera um número")?;
                i += 2;
            }
            "--fps-teto" => {
                o.fps = argv
                    .get(i + 1)
                    .ok_or("--fps-teto sem valor")?
                    .parse()
                    .map_err(|_| "--fps-teto espera um número")?;
                i += 2;
            }
            "--teclas" => {
                let v = argv.get(i + 1).ok_or("--teclas sem valor")?;
                for nome in v.split(',') {
                    o.teclas.push(tecla(nome.trim())?);
                }
                i += 2;
            }
            "--json" => {
                o.json = true;
                i += 1;
            }
            "-h" | "--ajuda" => {
                o.ajuda = true;
                i += 1;
            }
            "--versao" => {
                o.versao = true;
                i += 1;
            }
            outro => return Err(format!("argumento desconhecido: {outro}")),
        }
    }
    Ok(o)
}

fn tecla(nome: &str) -> Result<Acao, String> {
    Ok(match nome {
        "up" | "cima" => Acao::Dir(Direcao::Cima),
        "down" | "baixo" => Acao::Dir(Direcao::Baixo),
        "left" | "esquerda" => Acao::Dir(Direcao::Esquerda),
        "right" | "direita" => Acao::Dir(Direcao::Direita),
        "a" | "start" | "confirmar" => Acao::Confirmar,
        "b" | "select" | "sair" => Acao::Sair,
        outro => return Err(format!("tecla desconhecida: {outro}")),
    })
}

fn ajuda() {
    println!(
        "giochi-cobrinha {VERSAO}

Uso:
  giochi-cobrinha                        joga
  giochi-cobrinha --quadro <bmp>         desenha um quadro e sai
  giochi-cobrinha --tamanho 640x480      painel simulado (SDL_VIDEODRIVER=dummy)
  giochi-cobrinha --passos 40            adianta a simulação antes de fotografar
  giochi-cobrinha --teclas a,down,left   entrada simulada
  giochi-cobrinha --quadros 90           desenha N quadros e sai
  giochi-cobrinha --fps-teto 45          teto de desenho (padrão 45)
  giochi-cobrinha --semente N            semente determinística
  giochi-cobrinha --json                 saída de máquina (ASCII)
  giochi-cobrinha --versao"
    );
}

fn aplicar(jogo: &mut Jogo, acao: Acao) -> bool {
    match acao {
        Acao::Dir(d) => {
            jogo.pedir_direcao(d);
            false
        }
        Acao::Confirmar => {
            jogo.comecar();
            false
        }
        Acao::Sair => true,
    }
}

/// Adianta a simulação com um **bot** que persegue a comida.
///
/// Serve para o quadro de verificação mostrar o jogo em movimento — cobra longa,
/// curva e placar com mais de um dígito — em vez da posição inicial. Um bot
/// guloso também exercita o que o quadro precisa provar: que a colisão funciona
/// (ele desvia da parede e do corpo) e que comer cresce.
///
/// É determinístico: mesma semente, mesmo quadro.
fn adiantar(jogo: &mut Jogo, passos: u32) {
    jogo.comecar();
    for _ in 0..passos {
        if jogo.estado != Estado::Jogando {
            jogo.comecar();
        }
        if let Some(d) = direcao_gulosa(jogo) {
            jogo.pedir_direcao(d);
        }
        jogo.passo();
    }
}

/// Escolhe a direção que reduz a distância até a comida sem morrer.
fn direcao_gulosa(jogo: &Jogo) -> Option<Direcao> {
    use giochi_cobrinha_core::{COLUNAS, LINHAS};
    let cabeca = jogo.cabeca_pos();
    let comida = jogo.comida;
    let mut candidatos = Vec::with_capacity(2);
    if comida.x != cabeca.x {
        candidatos.push(if comida.x > cabeca.x {
            Direcao::Direita
        } else {
            Direcao::Esquerda
        });
    }
    if comida.y != cabeca.y {
        candidatos.push(if comida.y > cabeca.y {
            Direcao::Baixo
        } else {
            Direcao::Cima
        });
    }
    for d in candidatos {
        let (dx, dy) = d.delta();
        let (nx, ny) = (cabeca.x + dx, cabeca.y + dy);
        if nx < 0 || nx >= COLUNAS || ny < 0 || ny >= LINHAS {
            continue;
        }
        let ocupada = (1..jogo.tamanho())
            .any(|k| jogo.segmento(k).x == nx && jogo.segmento(k).y == ny);
        if !ocupada {
            return Some(d);
        }
    }
    // Sem saída segura: devolve qualquer direção válida e deixa o passo
    // decidir. Não chama `pedir_direcao` porque o bot só lê o estado.
    [Direcao::Cima, Direcao::Baixo, Direcao::Esquerda, Direcao::Direita]
        .into_iter()
        .find(|d| *d != jogo.direcao.oposta())
}

fn desenhar(tela: &mut Tela, jogo: &Jogo, alpha: f32, recorde: u32) {
    tela.comecar_quadro();
    let geom = tela.geom;

    // Comida
    let (fx, fy) = geom.celula_em(jogo.comida.x, jogo.comida.y);
    let margem = (geom.celula / 4).max(1);
    tela.preencher(
        fx + margem,
        fy + margem,
        geom.celula - 2 * margem,
        geom.celula - 2 * margem,
        COMIDA,
    );

    // Corpo: do rabo para a cabeça, para a cabeça ficar por cima.
    for k in (0..jogo.tamanho()).rev() {
        let p = jogo.segmento(k);
        let (x, y) = geom.celula_em(p.x, p.y);
        let vao = (geom.celula / 8).max(1);
        let cor = if k == 0 { CABECA } else { CORPO };
        tela.preencher(
            x + vao,
            y + vao,
            geom.celula - 2 * vao,
            geom.celula - 2 * vao,
            cor,
        );
    }

    // Cabeça interpolada: um retângulo pequeno entre as duas células. É o que
    // faz 8 passos/s parecerem 45 quadros/s.
    if jogo.estado == Estado::Jogando && alpha > 0.02 {
        let (gx, gy) = jogo.cabeca_interpolada(alpha);
        let x = geom.campo_x + (gx * geom.celula as f32) as i32;
        let y = geom.campo_y + (gy * geom.celula as f32) as i32;
        let tam = (geom.celula * 2 / 3).max(3);
        let desloc = (geom.celula - tam) / 2;
        tela.preencher(x + desloc, y + desloc, tam, tam, CABECA);
    }

    // HUD
    let alt = (geom.hud * 45 / 100).max(6);
    let topo = (geom.campo_y - geom.hud / 2 - alt / 2).max(2);
    tela.numero(jogo.pontos, geom.campo_x + geom.campo_w(), topo, alt, TEXTO);
    let rec = recorde.max(jogo.pontos);
    tela.numero(rec, geom.campo_x, topo, alt, APAGADO);

    match jogo.estado {
        Estado::Pronto => {
            triangulo(tela, geom.campo_x + geom.campo_w() / 2, geom.campo_y + geom.campo_h() / 2, geom.celula * 3, TEXTO);
        }
        Estado::Perdido => {
            let (x, y) = (geom.campo_x, geom.campo_y);
            tela.preencher(x, y, geom.campo_w(), geom.campo_h(), desenho::FUNDO);
            let alt = (geom.celula * 2).max(10);
            tela.numero(
                jogo.pontos,
                geom.campo_x + geom.campo_w() / 2 + desenho::largura_do_numero(jogo.pontos, alt) / 2,
                geom.campo_y + geom.campo_h() / 2 - alt,
                alt,
                TEXTO,
            );
            triangulo(
                tela,
                geom.campo_x + geom.campo_w() / 2,
                geom.campo_y + geom.campo_h() / 2 + alt,
                geom.celula * 2,
                CORPO,
            );
        }
        Estado::Jogando => {}
    }
}

/// Triângulo "play" feito de retângulos — o ícone sem fonte.
fn triangulo(tela: &mut Tela, cx: i32, cy: i32, tamanho: i32, cor: sdl2::pixels::Color) {
    let passos = (tamanho / 2).max(4);
    let largura_max = tamanho * 3 / 4;
    for i in 0..passos {
        let t = i as f32 / passos as f32;
        let w = ((1.0 - (2.0 * t - 1.0).abs()) * largura_max as f32) as i32;
        if w <= 0 {
            continue;
        }
        let y = cy - tamanho / 2 + i * (tamanho / passos).max(1);
        tela.preencher(cx - largura_max / 2 + (largura_max - w) / 2, y, w, (tamanho / passos).max(1), cor);
    }
}

fn gravar_quadro(tela: &mut Tela, caminho: &str) -> Result<(u32, u32), String> {
    let (w, h) = tela.canvas.output_size()?;
    let formato = PixelFormatEnum::ABGR8888;
    let mut pixels = tela.canvas.read_pixels(None, formato)?;
    let pitch = w * formato.byte_size_per_pixel() as u32;
    let superficie = Surface::from_data(&mut pixels, w, h, pitch, formato)?;
    superficie.save_bmp(caminho)?;
    Ok((w, h))
}

fn principal() -> Result<i32, String> {
    let o = opcoes(std::env::args().skip(1).collect())?;
    if o.ajuda {
        ajuda();
        return Ok(0);
    }
    if o.versao {
        if o.json {
            println!("{{\"nome\":\"giochi-cobrinha\",\"versao\":\"{VERSAO}\"}}");
        } else {
            println!("giochi-cobrinha {VERSAO}");
        }
        return Ok(0);
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let _ttf = (); // sem SDL2_ttf: três `NEEDED`, nada de fonte.

    let painel = match o.tamanho {
        Some(t) => t,
        None => video
            .desktop_display_mode(0)
            .map(|m| (m.w as u32, m.h as u32))
            .unwrap_or((640, 480)),
    };

    // Painel explícito é painel simulado: o `fbcon` da máquina de teste não
    // descreve o painel pedido.
    let (rot, fonte) = video::resolver(painel.0, painel.1, o.tamanho.is_none());
    let design = video::projetar(rot, painel);

    let janela = video
        .window("Giochi Cobrinha", painel.0, painel.1)
        .position_centered()
        .build()
        .map_err(|e| format!("janela: {e}"))?;
    let canvas = janela
        .into_canvas()
        .build()
        .map_err(|e| format!("renderer: {e}"))?;

    let geom = Geometria::calcular(design.0 as i32, design.1 as i32, 0.10);
    let mut tela = Tela::nova(canvas, geom, rot, (design.0 as i32, design.1 as i32), painel);

    let (rw, rh) = tela.canvas.output_size().unwrap_or(painel);
    let nome_renderer = tela.canvas.info().name;

    if !o.json {
        println!(
            "cobrinha: painel {}x{}, projeto {}x{} ({}), celula {} px, escala 1.00, renderer {}",
            painel.0, painel.1, design.0, design.1, rot.nome(), geom.celula, nome_renderer
        );
        println!("cobrinha: rotacao de {fonte}; campo {}x{} em ({},{}), sobra {}x{}",
            geom.campo_w(), geom.campo_h(), geom.campo_x, geom.campo_y, geom.sobra_x, geom.sobra_y);
    }

    let entrada = evdev::Entrada::iniciar();
    if !o.json {
        println!(
            "cobrinha: {} dispositivo(s) de entrada{}",
            entrada.abertos(),
            if entrada.abertos() == 0 {
                " (teclado do PC)"
            } else if entrada.ativa() {
                ""
            } else {
                " (abertos, ainda sem evento)"
            }
        );
        println!("cobrinha: teto {} fps, simulacao {} passos/s", o.fps, giochi_cobrinha_core::TIQUE_BASE);
    }

    let mut jogo = Jogo::novo(o.semente);
    let mut relogio = Relogio::com_ritmo(giochi_cobrinha_core::TIQUE_BASE, o.fps);
    let mut recorde = 0u32;

    if o.passos > 0 {
        adiantar(&mut jogo, o.passos);
    }
    for acao in &o.teclas {
        aplicar(&mut jogo, *acao);
    }

    if let Some(caminho) = &o.quadro {
        desenhar(&mut tela, &jogo, 0.5, recorde);
        tela.terminar_quadro();
        let (w, h) = gravar_quadro(&mut tela, caminho)?;
        if o.json {
            println!(
                "{{\"quadro\":\"{}\",\"largura\":{},\"altura\":{},\"pontos\":{},\"celula\":{}}}",
                caminho, w, h, jogo.pontos, geom.celula
            );
        } else {
            println!("cobrinha: quadro {w}x{h} gravado em {caminho} ({} pontos)", jogo.pontos);
        }
        return Ok(0);
    }

    // ── laço principal ────────────────────────────────────────
    let mut eventos = sdl.event_pump()?;
    let inicio = Instant::now();
    let mut quadros = 0u32;
    let mut total_passos = 0u32;
    let mut sair = false;

    while !sair {
        for evento in eventos.poll_iter() {
            match evento {
                Event::Quit { .. } => sair = true,
                Event::KeyDown { keycode, repeat: false, .. } => {
                    let acao = match keycode {
                        Some(Keycode::Up) => Some(Acao::Dir(Direcao::Cima)),
                        Some(Keycode::Down) => Some(Acao::Dir(Direcao::Baixo)),
                        Some(Keycode::Left) => Some(Acao::Dir(Direcao::Esquerda)),
                        Some(Keycode::Right) => Some(Acao::Dir(Direcao::Direita)),
                        Some(Keycode::Space) | Some(Keycode::Return) => Some(Acao::Confirmar),
                        Some(Keycode::Escape) | Some(Keycode::Q) => Some(Acao::Sair),
                        _ => None,
                    };
                    if let Some(a) = acao {
                        if aplicar(&mut jogo, a) {
                            sair = true;
                        }
                    }
                }
                _ => {}
            }
        }

        while let Some(acao) = entrada.fila().puxar() {
            if aplicar(&mut jogo, acao) {
                sair = true;
            }
        }

        let agora = inicio.elapsed().as_secs_f64();
        let passos = relogio.avancar(agora);

        for _ in 0..passos.ticks {
            match jogo.passo() {
                Evento::Comeu => {
                    let novo = giochi_cobrinha_core::TIQUE_BASE
                        + giochi_cobrinha_core::ACELERACAO * jogo.pontos as f32;
                    relogio.definir_tiques(novo.min(giochi_cobrinha_core::TIQUE_MAX));
                }
                Evento::Morreu(_) => {
                    recorde = recorde.max(jogo.pontos);
                }
                _ => {}
            }
            total_passos += 1;
        }

        desenhar(&mut tela, &jogo, passos.alpha, recorde);
        tela.terminar_quadro();
        quadros += 1;

        if let Some(limite) = o.quadros {
            if quadros >= limite {
                break;
            }
        }

        if passos.dormir > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(passos.dormir));
        }
    }

    let decorrido = inicio.elapsed().as_secs_f64();
    if o.json {
        println!(
            "{{\"quadros\":{},\"passos\":{},\"segundos\":{:.3},\"fps\":{:.1},\"pontos\":{},\"atrasados\":{},\"renderer\":\"{}\",\"painel\":\"{}x{}\",\"celula\":{}}}",
            quadros,
            total_passos,
            decorrido,
            if decorrido > 0.0 { quadros as f64 / decorrido } else { 0.0 },
            jogo.pontos.max(recorde),
            relogio.atrasados(),
            nome_renderer,
            rw,
            rh,
            geom.celula
        );
    } else {
        println!(
            "cobrinha: fim — {} pontos, {} quadros em {:.2} s ({:.0} fps)",
            jogo.pontos.max(recorde),
            quadros,
            decorrido,
            if decorrido > 0.0 { quadros as f64 / decorrido } else { 0.0 }
        );
    }
    let _ = COLUNAS;
    Ok(0)
}

fn main() {
    match principal() {
        Ok(codigo) => std::process::exit(codigo),
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(2);
        }
    }
}
