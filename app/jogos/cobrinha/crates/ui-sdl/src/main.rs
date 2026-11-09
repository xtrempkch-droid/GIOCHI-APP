//! Cobrinha 3D: regras puras no núcleo, renderização GLES pela GIOCHI-ENGINE 3D.

mod cena3d;
mod evdev;
mod video;

use std::time::{Duration, Instant};

use cena3d::Cena3D;
use giochi_cobrinha_core::{Acao, Direcao, Estado, Evento, Jogo, Relogio, ALVO_FPS, MAX_FPS};
use giochi_engine::platform::sdl::Platform;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine_3d::context::{self, GlesVersion};
use giochi_engine_3d::DirectionalLight;
use giochi_engine_3d::RenderContext3D;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::surface::Surface;

const VERSAO: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
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
        match argv[i].as_str() {
            "--quadro" => {
                o.quadro = Some(argv.get(i + 1).ok_or("--quadro sem caminho")?.clone());
                i += 2;
            }
            "--tamanho" => {
                let valor = argv.get(i + 1).ok_or("--tamanho sem valor")?;
                let (largura, altura) = valor
                    .split_once('x')
                    .ok_or("--tamanho espera LARGURAxALTURA")?;
                let tamanho = (
                    largura.parse().map_err(|_| "largura inválida")?,
                    altura.parse().map_err(|_| "altura inválida")?,
                );
                if tamanho.0 == 0 || tamanho.1 == 0 {
                    return Err("--tamanho exige dimensões maiores que zero".into());
                }
                o.tamanho = Some(tamanho);
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
                let quadros: u32 = argv
                    .get(i + 1)
                    .ok_or("--quadros sem valor")?
                    .parse()
                    .map_err(|_| "--quadros espera um número")?;
                if quadros == 0 {
                    return Err("--quadros precisa ser maior que zero".into());
                }
                o.quadros = Some(quadros);
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
                let fps: u32 = argv
                    .get(i + 1)
                    .ok_or("--fps-teto sem valor")?
                    .parse()
                    .map_err(|_| "--fps-teto espera um número")?;
                if !(1..=MAX_FPS).contains(&fps) {
                    return Err(format!("--fps-teto precisa ficar entre 1 e {MAX_FPS}"));
                }
                o.fps = fps;
                i += 2;
            }
            "--teclas" => {
                let valor = argv.get(i + 1).ok_or("--teclas sem valor")?;
                for nome in valor.split(',') {
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
  giochi-cobrinha                         jogar
  giochi-cobrinha --quadro <arquivo.bmp>  desenhar um quadro e sair
  giochi-cobrinha --tamanho 640x480       painel simulado
  giochi-cobrinha --passos 40             adiantar a simulação
  giochi-cobrinha --teclas a,down,left    entrada simulada
  giochi-cobrinha --quadros 150           desenhar N quadros e sair
  giochi-cobrinha --fps-teto 75           limite configurável: 1 a 75
  giochi-cobrinha --semente N             semente determinística
  giochi-cobrinha --json                  saída de máquina
  giochi-cobrinha --versao

GIOCHI_GL_VERSION=2 solicita GLES 2.0; o padrão solicita GLES 3.0 com fallback."
    );
}

fn aplicar(jogo: &mut Jogo, acao: Acao) -> bool {
    match acao {
        Acao::Dir(direcao) => {
            jogo.pedir_direcao(direcao);
            false
        }
        Acao::Confirmar => {
            jogo.comecar();
            false
        }
        Acao::Sair => true,
    }
}

fn adiantar(jogo: &mut Jogo, passos: u32) {
    jogo.comecar();
    for _ in 0..passos {
        if jogo.estado != Estado::Jogando {
            jogo.comecar();
        }
        if let Some(direcao) = direcao_gulosa(jogo) {
            jogo.pedir_direcao(direcao);
        }
        jogo.passo();
    }
}

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
    for direcao in candidatos {
        let (dx, dy) = direcao.delta();
        let (x, y) = (cabeca.x + dx, cabeca.y + dy);
        if x < 0 || x >= COLUNAS || y < 0 || y >= LINHAS {
            continue;
        }
        let ocupada =
            (1..jogo.tamanho()).any(|k| jogo.segmento(k).x == x && jogo.segmento(k).y == y);
        if !ocupada {
            return Some(direcao);
        }
    }
    [
        Direcao::Cima,
        Direcao::Baixo,
        Direcao::Esquerda,
        Direcao::Direita,
    ]
    .into_iter()
    .find(|direcao| *direcao != jogo.direcao.oposta())
}

fn gravar_quadro(
    render: &RenderContext3D,
    caminho: &str,
) -> Result<(u32, u32), Box<dyn std::error::Error>> {
    let (largura, altura) = render.drawable_size();
    let mut pixels = render.read_pixels(largura, altura)?;
    let pitch = largura as usize * 4;
    for y in 0..altura as usize / 2 {
        let outro = altura as usize - 1 - y;
        let (inicio, fim) = pixels.split_at_mut(outro * pitch);
        inicio[y * pitch..(y + 1) * pitch].swap_with_slice(&mut fim[..pitch]);
    }
    let superficie = Surface::from_data(
        &mut pixels,
        largura,
        altura,
        largura * PixelFormatEnum::RGBA32.byte_size_per_pixel() as u32,
        PixelFormatEnum::RGBA32,
    )?;
    superficie.save_bmp(caminho)?;
    Ok((largura, altura))
}

fn restaurar_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, 0);
    }
}

fn painel(platform: &Platform, opcoes: &Opcoes) -> Result<(u32, u32), String> {
    if let Some(tamanho) = opcoes.tamanho {
        return Ok(tamanho);
    }
    platform
        .video_subsystem()
        .desktop_display_mode(0)
        .map(|modo| (modo.w as u32, modo.h as u32))
        .map_err(|erro| format!("não consegui consultar o painel: {erro}"))
}

fn criar_render(
    plataforma: &Platform,
    tamanho: (u32, u32),
    fullscreen: bool,
    versao: GlesVersion,
) -> Result<(Window, RenderContext3D), Box<dyn std::error::Error>> {
    context::configure_attributes(plataforma, versao)?;
    let config = WindowConfig::new(tamanho.0, tamanho.1).fullscreen(fullscreen);
    let janela = Window::new_opengl(plataforma, config, "Giochi Cobrinha 3D")?;
    let render = RenderContext3D::new(context::create(&janela)?);
    Ok((janela, render))
}

fn desenhar_quadro(
    render: &mut RenderContext3D,
    janela: &Window,
    cena: &mut Cena3D,
    jogo: &Jogo,
    alpha: f32,
    tempo: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    cena.preparar_quadro(render);
    render.begin_frame(janela);
    cena.desenhar(render, jogo, alpha, tempo)?;
    Ok(())
}

fn principal() -> Result<i32, Box<dyn std::error::Error>> {
    restaurar_sigpipe();
    let opcoes = opcoes(std::env::args().skip(1).collect())
        .map_err(|erro| std::io::Error::new(std::io::ErrorKind::InvalidInput, erro))?;
    if opcoes.ajuda {
        ajuda();
        return Ok(0);
    }
    if opcoes.versao {
        if opcoes.json {
            println!("{{\"nome\":\"giochi-cobrinha\",\"versao\":\"{VERSAO}\"}}");
        } else {
            println!("giochi-cobrinha {VERSAO}");
        }
        return Ok(0);
    }

    if std::env::var_os("GIOCHI_GL_VERSION").is_none() {
        std::env::set_var("GIOCHI_GL_VERSION", "3");
    }

    let plataforma = Platform::new()?;
    let tamanho_painel =
        painel(&plataforma, &opcoes).map_err(|erro| std::io::Error::other(erro))?;
    let (rotacao, fonte_rotacao) =
        video::resolver(tamanho_painel.0, tamanho_painel.1, opcoes.tamanho.is_none());

    let versao_solicitada = GlesVersion::from_env();
    let fullscreen = opcoes.tamanho.is_none();
    let (janela, mut render) =
        match criar_render(&plataforma, tamanho_painel, fullscreen, versao_solicitada) {
            Ok(par) => par,
            Err(erro) if versao_solicitada == GlesVersion::Gles3 => {
                let mensagem = video::erro_de_video(&erro.to_string());
                eprintln!("aviso: GLES 3.0 indisponível ({mensagem}); tentando GLES 2.0");
                std::env::set_var("GIOCHI_GL_VERSION", "2");
                criar_render(&plataforma, tamanho_painel, fullscreen, GlesVersion::Gles2)?
            }
            Err(erro) => {
                return Err(std::io::Error::other(video::erro_de_video(&erro.to_string())).into())
            }
        };
    render.set_light(DirectionalLight::sun());
    let (largura, altura) = janela.drawable_size();
    render.resize(largura, altura);
    let mut cena = Cena3D::nova(&mut render, rotacao)?;

    if !opcoes.json {
        let projeto = video::projetar(rotacao, (largura, altura));
        println!(
            "cobrinha: painel {largura}x{altura}, projeto {}x{}, rotação {} ({fonte_rotacao}), GLES {} em {}",
            projeto.0,
            projeto.1,
            rotacao.nome(),
            render.gles_version().major(),
            render.adapter()
        );
        println!(
            "cobrinha: instancing {}, GLES solicitado {}, limite {} fps",
            if cena.usa_instancing() {
                "ativo"
            } else {
                "indisponível"
            },
            versao_solicitada.major(),
            opcoes.fps
        );
    }

    let entrada = evdev::Entrada::iniciar();
    if !opcoes.json {
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
    }

    let mut jogo = Jogo::novo(opcoes.semente);
    let mut relogio = Relogio::com_ritmo(giochi_cobrinha_core::TIQUE_BASE, opcoes.fps);
    let mut recorde = 0u32;
    if opcoes.passos > 0 {
        adiantar(&mut jogo, opcoes.passos);
    }
    let mut sair = false;
    for acao in &opcoes.teclas {
        sair |= aplicar(&mut jogo, *acao);
    }

    if let Some(caminho) = &opcoes.quadro {
        desenhar_quadro(&mut render, &janela, &mut cena, &jogo, 0.5, 0.4)?;
        let (w, h) = gravar_quadro(&render, caminho)?;
        render.end_frame(&janela);
        let erro_gl = render.take_gl_error();
        if erro_gl != 0 {
            return Err(format!("erro GLES 0x{erro_gl:04x} ao desenhar o quadro").into());
        }
        if opcoes.json {
            println!(
                "{{\"quadro\":\"gravado\",\"largura\":{w},\"altura\":{h},\"pontos\":{},\"gles\":{},\"fps_teto\":{}}}",
                jogo.pontos,
                render.gles_version().major(),
                opcoes.fps
            );
        } else {
            println!("cobrinha: quadro 3D {w}x{h} gravado em {caminho}");
        }
        render.release_all();
        return Ok(0);
    }

    let intervalo = Duration::from_secs_f64(1.0 / f64::from(opcoes.fps));
    let inicio = Instant::now();
    let mut quadros = 0u32;
    let mut total_passos = 0u32;

    while !sair {
        let inicio_quadro = Instant::now();
        for evento in plataforma.poll_events() {
            if let Event::Quit { .. } = evento {
                sair = true;
                continue;
            }
            if let Event::KeyDown {
                keycode,
                repeat: false,
                ..
            } = evento
            {
                let acao = match keycode {
                    Some(Keycode::Up) => Some(Acao::Dir(Direcao::Cima)),
                    Some(Keycode::Down) => Some(Acao::Dir(Direcao::Baixo)),
                    Some(Keycode::Left) => Some(Acao::Dir(Direcao::Esquerda)),
                    Some(Keycode::Right) => Some(Acao::Dir(Direcao::Direita)),
                    Some(Keycode::Space) | Some(Keycode::Return) => Some(Acao::Confirmar),
                    Some(Keycode::Escape) | Some(Keycode::Q) => Some(Acao::Sair),
                    _ => None,
                };
                if let Some(acao) = acao {
                    sair |= aplicar(&mut jogo, acao);
                }
            }
        }
        while let Some(acao) = entrada.fila().puxar() {
            sair |= aplicar(&mut jogo, acao);
        }
        if sair {
            break;
        }

        let tempo = inicio.elapsed().as_secs_f64();
        let passos = relogio.avancar(tempo);
        for _ in 0..passos.ticks {
            match jogo.passo() {
                Evento::Comeu => {
                    let novo = giochi_cobrinha_core::TIQUE_BASE
                        + giochi_cobrinha_core::ACELERACAO * jogo.pontos as f32;
                    relogio.definir_tiques(novo.min(giochi_cobrinha_core::TIQUE_MAX));
                }
                Evento::Morreu(_) => recorde = recorde.max(jogo.pontos),
                _ => {}
            }
            total_passos += 1;
        }

        desenhar_quadro(
            &mut render,
            &janela,
            &mut cena,
            &jogo,
            passos.alpha,
            tempo as f32,
        )?;
        render.end_frame(&janela);
        quadros += 1;

        if let Some(limite) = opcoes.quadros {
            if quadros >= limite {
                break;
            }
        }

        std::thread::sleep(intervalo.saturating_sub(inicio_quadro.elapsed()));
    }

    let decorrido = inicio.elapsed().as_secs_f64();
    let fps_medio = if decorrido > 0.0 {
        quadros.saturating_sub(1) as f64 / decorrido
    } else {
        0.0
    };
    if opcoes.json {
        println!(
            "{{\"quadros\":{quadros},\"passos\":{total_passos},\"segundos\":{decorrido:.3},\"fps\":{fps_medio:.1},\"fps_teto\":{},\"pontos\":{},\"atrasados\":{},\"gles\":{},\"instancing\":{}}}",
            opcoes.fps,
            jogo.pontos.max(recorde),
            relogio.atrasados(),
            render.gles_version().major(),
            cena.usa_instancing()
        );
    } else {
        println!(
            "cobrinha: fim — {} pontos, {} quadros em {:.2} s ({:.1} fps; teto {})",
            jogo.pontos.max(recorde),
            quadros,
            decorrido,
            fps_medio,
            opcoes.fps
        );
    }
    render.release_all();
    Ok(0)
}

fn main() {
    match principal() {
        Ok(codigo) => std::process::exit(codigo),
        Err(erro) => {
            eprintln!("erro: {erro}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn teclas_de_cli_mapeiam_para_acoes() {
        assert_eq!(tecla("up").unwrap(), Acao::Dir(Direcao::Cima));
        assert_eq!(tecla("a").unwrap(), Acao::Confirmar);
        assert_eq!(tecla("b").unwrap(), Acao::Sair);
        assert!(tecla("l2").is_err());
    }

    #[test]
    fn cli_rejeita_teto_acima_de_75_fps() {
        let resultado = opcoes(vec!["--fps-teto".into(), "76".into()]);
        assert!(resultado.unwrap_err().contains("1 e 75"));
    }

    #[test]
    fn cli_aceita_o_teto_de_75_fps() {
        let opcoes = opcoes(vec!["--fps-teto".into(), "75".into()]).unwrap();
        assert_eq!(opcoes.fps, 75);
    }

    #[test]
    fn teto_padrao_e_75_e_teto_zero_e_recusado() {
        assert_eq!(opcoes(vec![]).unwrap().fps, 75);
        assert!(opcoes(vec!["--fps-teto".into(), "0".into()]).is_err());
    }

    #[test]
    fn tamanho_de_painel_invalido_e_erro_com_mensagem() {
        assert!(opcoes(vec!["--tamanho".into(), "640".into()]).is_err());
        assert!(opcoes(vec!["--tamanho".into(), "axb".into()]).is_err());
        assert!(opcoes(vec!["--tamanho".into(), "0x480".into()]).is_err());
        let opcoes = opcoes(vec!["--tamanho".into(), "800x600".into()]).unwrap();
        assert_eq!(opcoes.tamanho, Some((800, 600)));
    }

    #[test]
    fn argumento_desconhecido_nao_e_ignorado() {
        let resultado = opcoes(vec!["--nao-existe".into()]);
        assert!(resultado.unwrap_err().contains("--nao-existe"));
    }

    #[test]
    fn bot_avanca_a_simulacao_sem_morrer() {
        let mut jogo = Jogo::novo(7);
        adiantar(&mut jogo, 120);
        assert!(jogo.pontos >= 1);
        assert!(jogo.tamanho() > 3);
    }
}
