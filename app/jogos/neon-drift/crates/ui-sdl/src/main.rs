//! Neon Drift: laço principal, GLES e diagnóstico.
//!
//! ## O que este arquivo **não** faz
//!
//! Regra de corrida, física, pontuação e formatação vivem no núcleo
//! (`giochi-neon-drift-core`). Aqui ficam só as três coisas que precisam do
//! aparelho: janela/GLES, entrada e o laço com teto de quadros. É o que permite
//! testar "duas voltas terminam em vitória" sem abrir tela.
//!
//! ## Diagnóstico sem dedo no aparelho
//!
//! `--quadro`, `--quadros`, `--tamanho`, `--teclas`, `--passos` e `--json`
//! existem porque este projeto já pagou caro por ferramenta que só funcionava
//! com alguém olhando a tela: o `--teclas` do launcher passou 223 testes verdes
//! imprimindo um estado que não tinha mudado.

mod atlas;
mod audio;
mod cena3d;
mod efeitos;
mod texturas;
mod entrada_jogo;
mod evdev;
mod malha;
mod vetor;
mod video;

use std::time::Instant;

use audio::{EstadoSom, Som};
use cena3d::Cena3D;
use entrada_jogo::{Acoes, Teclas};
use giochi_engine::platform::sdl::Platform;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine_3d::context::{self, GlesVersion};
use giochi_engine_3d::RenderContext3D;
use giochi_neon_drift_core::carro::Entrada;
use giochi_neon_drift_core::corrida::{Corrida, Evento, Fase};
use giochi_neon_drift_core::dificuldade::Dificuldade;
use giochi_neon_drift_core::relogio::{ALVO_FPS, MAX_FPS, TIQUE};
use giochi_neon_drift_core::{hud, Relogio};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::surface::Surface;

const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Opções de linha de comando.
#[derive(Debug)]
struct Opcoes {
    quadro: Option<String>,
    /// Grava a folha das texturas em BMP e sai, sem abrir janela.
    texturas: Option<String>,
    /// Grava o atlas de sprites em BMP e sai, sem abrir janela.
    atlas: Option<String>,
    /// Grava a prova de rotação e sai (`--prova-rotacao arquivo.bmp`).
    ///
    /// Três barras em posições conhecidas do **projeto**, com predição numérica do
    /// lugar de cada uma no **painel**. Existe porque "está de ponta a cabeça" e
    /// "as informações na vertical" não dizem qual peça girou errado, e comparar
    /// aparelho com PC não separa os casos quando os dois erram igual.
    prova_rotacao: Option<String>,
    tamanho: Option<(u32, u32)>,
    quadros: Option<u32>,
    passos: u32,
    /// `s` onde o carro começa, em metros (`--em`).
    em: Option<f32>,
    teclas: Teclas,
    tela: Option<Fase>,
    fps: u32,
    json: bool,
    sem_som: bool,
    piloto: bool,
    so: Option<String>,
    mapa: Option<String>,
    /// Modo de dificuldade por id (`facil`, `normal`, `dificil`, `desafiante`).
    dificuldade: Option<String>,
    sem_vsync: bool,
    escala: f32,
    ajuda: bool,
    versao: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            quadro: None,
            atlas: None,
            prova_rotacao: None,
            tamanho: None,
            quadros: None,
            passos: 0,
            em: None,
            texturas: None,
            teclas: Teclas::default(),
            tela: None,
            fps: ALVO_FPS,
            json: false,
            sem_som: false,
            piloto: false,
            so: None,
            mapa: None,
            dificuldade: None,
            sem_vsync: std::env::var_os("GIOCHI_VSYNC").is_some_and(|v| v == "0"),
            escala: std::env::var("GIOCHI_ESCALA")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            ajuda: false,
            versao: false,
        }
    }
}

fn opcoes(argv: Vec<String>) -> Result<Opcoes, String> {
    let mut o = Opcoes::default();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--quadro" => {
                o.quadro = Some(argv.get(i + 1).ok_or("--quadro sem caminho")?.clone());
                i += 2;
            }
            // Grava a folha das texturas procedurais: é o molde que o artista
            // usa para pintar a versão dele. Sem isto, "que densidade tem o
            // asfalto?" só se responde lendo o código.
            "--texturas" => {
                o.texturas = Some(argv.get(i + 1).ok_or("--texturas sem caminho")?.clone());
                i += 2;
            }
            "--prova-rotacao" => {
                o.prova_rotacao = Some(
                    argv.get(i + 1)
                        .ok_or("--prova-rotacao sem caminho")?
                        .clone(),
                );
                // Sem este avanço o laço relê a mesma flag para sempre: o
                // processo gira em silêncio, sem imprimir nada e sem abrir
                // janela. Custou uma rodada de diagnóstico.
                i += 2;
            }
            "--atlas" => {
                o.atlas = Some(argv.get(i + 1).ok_or("--atlas sem caminho")?.clone());
                i += 2;
            }
            "--tamanho" => {
                let valor = argv.get(i + 1).ok_or("--tamanho sem valor")?;
                let (l, a) = valor
                    .split_once('x')
                    .ok_or("--tamanho espera LARGURAxALTURA")?;
                let tamanho = (
                    l.parse().map_err(|_| "largura inválida")?,
                    a.parse().map_err(|_| "altura inválida")?,
                );
                if tamanho.0 == 0 || tamanho.1 == 0 {
                    return Err("--tamanho exige dimensões maiores que zero".into());
                }
                o.tamanho = Some(tamanho);
                i += 2;
            }
            "--quadros" => {
                let n: u32 = argv
                    .get(i + 1)
                    .ok_or("--quadros sem valor")?
                    .parse()
                    .map_err(|_| "--quadros espera um número")?;
                if n == 0 {
                    return Err("--quadros precisa ser maior que zero".into());
                }
                o.quadros = Some(n);
                i += 2;
            }
            // Posiciona o carro em `s` metros da volta, já andando na velocidade
            // de cruzeiro. Existe para testar rampa: chegar ao salto a 30 s de
            // volta transforma cada tentativa de ajuste em meio minuto.
            "--em" => {
                o.em = Some(
                    argv.get(i + 1)
                        .ok_or("--em sem valor")?
                        .parse()
                        .map_err(|_| "--em espera metros")?,
                );
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
            "--teclas" => {
                let valor = argv.get(i + 1).ok_or("--teclas sem valor")?;
                o.teclas = teclas(valor)?;
                i += 2;
            }
            "--tela" => {
                let valor = argv.get(i + 1).ok_or("--tela sem valor")?;
                o.tela = Some(match valor.as_str() {
                    "titulo" | "title" => Fase::Titulo,
                    "corrida" | "race" => Fase::Correndo,
                    "resultado" | "result" => Fase::Resultado,
                    outro => return Err(format!("--tela desconhecida: {outro}")),
                });
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
            "--json" => {
                o.json = true;
                i += 1;
            }
            "--piloto" => {
                o.piloto = true;
                i += 1;
            }
            "--mapa" => {
                o.mapa = Some(argv.get(i + 1).ok_or("--mapa sem nome")?.clone());
                i += 2;
            }
            "--dificuldade" => {
                o.dificuldade = Some(argv.get(i + 1).ok_or("--dificuldade sem nome")?.clone());
                i += 2;
            }
            "--so" => {
                o.so = Some(argv.get(i + 1).ok_or("--so sem nome de malha")?.clone());
                i += 1;
                i += 1;
            }
            "--sem-som" => {
                o.sem_som = true;
                i += 1;
            }
            "--sem-vsync" => {
                o.sem_vsync = true;
                i += 1;
            }
            "--escala" => {
                let v = argv.get(i + 1).ok_or("--escala sem valor")?;
                o.escala = v
                    .parse::<f32>()
                    .map_err(|_| format!("--escala precisa de um número, veio {v}"))?;
                i += 2;
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

/// Converte `a,right,space` em teclas mantidas apertadas.
fn teclas(valor: &str) -> Result<Teclas, String> {
    let mut t = Teclas::default();
    for nome in valor.split(',') {
        match nome.trim() {
            "" => {}
            "a" | "acelerar" | "up" => t.cima = true,
            "b" | "frear" | "down" => t.baixo = true,
            "left" | "esquerda" => t.esquerda = true,
            "right" | "direita" => t.direita = true,
            "space" | "espaco" | "drift" => t.espaco = true,
            "shift" | "nitro" => t.shift = true,
            "enter" | "start" => t.enter = true,
            "esc" | "escape" => t.escape = true,
            outro => return Err(format!("tecla desconhecida: {outro}")),
        }
    }
    Ok(t)
}

/// Estado da corrida em uma linha, para o `--json`.
///
/// Existe porque `--passos` sem número de retorno é fé: rodar 900 passos e
/// receber `"fase":"Correndo"` não diz se o carro andou 4 m ou 500 m.
fn diagnostico(corrida: &Corrida, cena: &Cena3D, render: &RenderContext3D) -> String {
    // Contadores da **engine**, não do jogo. O jogo tinha um `desenhos` próprio,
    // incrementado à mão em cada ponto de desenho, e ele já estava errado por um:
    // os riscos de velocidade emitiam `draw_instanced` sem incrementar. Um
    // contador que o autor precisa lembrar de atualizar mede intenção; este conta
    // no ponto em que o backend emite a chamada.
    let contadores = render.frame_counters();
    let s = corrida.carro.pos;
    let (cx, cy, cz, fov) = cena.camera();
    // Quanto o melhor adversário está à frente do jogador, em metros (negativo =
    // o jogador lidera). Existe porque `--dificuldade` sem número mensurável é
    // fé: `posicao` vale 1 nas quatro dificuldades com o piloto ao volante, e
    // "1º" não distingue ganhar por 200 m de ganhar por 2.
    let rival = corrida
        .adversarios
        .iter()
        .map(|a| a.progresso(&corrida.pista))
        .fold(f32::NEG_INFINITY, f32::max);
    let gap = rival - corrida.progresso_jogador();
    format!(
        "\"velocidade\":{:.1},\"volta\":{},\"posicao\":{},\"tempo\":{:.2},\"drift\":{},\"s\":{:.1},\"x\":{:.1},\"z\":{:.1},\"altitude\":{:.2},\"cam\":[{:.1},{:.1},{:.1}],\"fov\":{:.2},\"mapa\":\"{}\",\"dificuldade\":\"{}\",\"gap\":{:.1},\"posicao\":{},\"rendendo\":{},\"impacto\":{:.2},\"fragmentos\":{},\"largou\":{},\"progresso\":{:.1},\"no_ar\":{},\"v_vertical\":{:.2},\"altura_solo\":{:.2},\"nitro\":{:.1},\"nitro_usando\":{},\"derrapando\":{},\"malhas\":{},\"vertices\":{}",
        corrida.carro.velocidade,
        corrida.volta,
        corrida.posicao,
        corrida.tempo,
        corrida.drift.pontos,
        corrida.carro.s,
        s.x,
        s.z,
        corrida.carro.altura,
        cx,
        cy,
        cz,
        fov,
        corrida.mapa.id,
        corrida.dificuldade.id(),
        gap,
        corrida.posicao,
        corrida.carro.raspando,
        corrida.carro.impacto,
        cena.fragmentos(),
        // `largou` é o que separa "volta 1" de "corrida começada": sem ele, um
        // `--passos 250` que responde `volta=1` pode ser uma corrida que não
        // largou ou uma que largou e ainda não fechou a volta. O diagnóstico
        // existe para não ter de adivinhar entre as duas.
        corrida.largou(),
        corrida.progresso_jogador(),
        // `no_ar` e `v_vertical` são o salto em números: sem eles, "o carro
        // pulou" só se prova olhando a tela, e um salto que não acontece é
        // indistinguível de uma rampa que não existe.
        corrida.carro.no_ar,
        corrida.carro.velocidade_vertical,
        corrida.pista.altura_em(corrida.carro.s),
        // O nitro é a economia do jogo: `gasto 30/s` contra `recarga 8/s` dá
        // 21 % de ciclo ativo. Sem expor os dois, "o carro passa dos 200 km/h
        // direto" é indistinguível de "o tanque está infinito".
        corrida.carro.nitro,
        corrida.carro.nitro_usando,
        corrida.carro.derrapando,
        contadores.meshes_drawn,
        contadores.vertices_uploaded
    )
}

fn ajuda() {
    println!(
        "giochi-neon-drift {VERSAO}

Uso:
  giochi-neon-drift                        jogar
  giochi-neon-drift --quadro <arq.bmp>      desenhar um quadro e sair
  giochi-neon-drift --tamanho 640x480       painel simulado
  giochi-neon-drift --quadros 300           desenhar N quadros e sair
  giochi-neon-drift --passos 2000           adiantar a simulação
  giochi-neon-drift --em 1240 --teclas up   começar o carro em 1 240 m da volta
  giochi-neon-drift --piloto                o piloto artificial dirige nos --passos
  giochi-neon-drift --teclas a,right,shift  teclas seguradas
  giochi-neon-drift --tela resultado        forçar uma tela
  giochi-neon-drift --fps-teto 75           limite: 1 a 75 (é também o padrão)
  giochi-neon-drift --sem-som               sem áudio
  giochi-neon-drift --mapa boavista         escolhe o mapa (avenida, boavista)
  giochi-neon-drift --so asfalto            desenha uma malha só (diagnóstico)
  giochi-neon-drift --texturas t.bmp        grava a folha das 7 texturas e sai
  giochi-neon-drift --atlas a.bmp           grava o atlas de sprites e sai
  giochi-neon-drift --prova-rotacao r.bmp   grava a prova de orientação e sai
  GIOCHI_SPRITES=x.bmp GIOCHI_SPRITES_COLUNAS=8
                                            usa a sua folha de sprites (BMP)
  giochi-neon-drift --json                  saída de máquina (stdout é só o JSON)
  giochi-neon-drift --versao

Controles: WASD/setas, ESPACO freio de mao, SHIFT nitro, ENTER, ESC.
GIOCHI_GL_VERSION=2 solicita GLES 2.0; o padrão pede 3.0 com queda para 2.0."
    );
}

fn restaurar_sigpipe() {
    // Escrever num pipe fechado (`--json | head`) não pode matar o processo no
    // meio do desenho: o Rust ignora SIGPIPE por padrão, mas o SDL instala o
    // handler de C, que encerra.
    unsafe {
        libc::signal(libc::SIGPIPE, 0);
    }
}

fn painel(plataforma: &Platform, o: &Opcoes) -> Result<(u32, u32), String> {
    if let Some(t) = o.tamanho {
        return Ok(t);
    }
    plataforma
        .video_subsystem()
        .desktop_display_mode(0)
        .map(|m| (m.w as u32, m.h as u32))
        .map_err(|e| format!("não consegui consultar o painel: {e}"))
}

fn criar_render(
    plataforma: &Platform,
    tamanho: (u32, u32),
    tela_cheia: bool,
    versao: GlesVersion,
    sem_vsync: bool,
) -> Result<(Window, RenderContext3D), Box<dyn std::error::Error>> {
    context::configure_attributes(plataforma, versao)?;
    let config = WindowConfig::new(tamanho.0, tamanho.1).fullscreen(tela_cheia);
    let janela = Window::new_opengl(plataforma, config, "Giochi Neon Drift")?;
    // `--sem-vsync` existe porque o KMSDRM desta imagem trava o swap em 60 Hz: com
    // vsync, todo quadro mais barato que 16,7 ms mede "59,9 fps" e não dá para
    // saber se sobrou folga ou se faltou 1 ms. Sem vsync o número passa a ser o
    // custo real do quadro — é o que permite decidir se um efeito novo cabe.
    if sem_vsync {
        // No `sdl2` 0.38 o `gl_set_swap_interval` mora no `WindowBuilder`, não no
        // `Window` — e a engine já criou a janela. Vai pela função C, que é a
        // mesma que o builder chamaria.
        let r = unsafe { sdl2::sys::SDL_GL_SetSwapInterval(0) };
        if r != 0 {
            eprintln!("aviso: não consegui desligar o vsync ({})", sdl2::get_error());
        }
    }
    let render = RenderContext3D::new(context::create(&janela)?);
    Ok((janela, render))
}

/// Grava o back buffer em BMP.
///
/// `read_pixels` devolve as linhas de baixo para cima (convenção do GL); o BMP
/// espera de cima para baixo. Sem a inversão o quadro sai virado e a comparação
/// com a referência mede o espelho, não o desenho.
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

/// Grava a folha das texturas procedurais em BMP, sem abrir janela.
///
/// Uma tira horizontal com as sete texturas lado a lado, na ordem de
/// `texturas::todas()`, com uma faixa escura de separação. Serve de **molde**:
/// quem quiser desenhar a própria textura pinta em cima destas células, com o
/// mesmo tamanho e a mesma densidade, e o resultado entra no jogo sem ajuste.
///
/// Não há carregamento de textura externa hoje — o leitor seria mais código do
/// que as sete fórmulas juntas. O que existe é a referência para pintar.
fn gravar_texturas(caminho: &str) -> Result<(), Box<dyn std::error::Error>> {
    let lista = texturas::todas();
    let lado = texturas::LADO;
    let separacao = 4u32;
    let largura = lista.len() as u32 * (lado + separacao) + separacao;
    let altura = lado + separacao * 2;
    let mut pixels = vec![0u8; (largura * altura * 4) as usize];
    // Fundo: cinza escuro, para a borda de cada célula ser visível.
    for i in 0..(largura * altura) as usize {
        pixels[i * 4] = 24;
        pixels[i * 4 + 1] = 24;
        pixels[i * 4 + 2] = 32;
        pixels[i * 4 + 3] = 255;
    }
    for (indice, textura) in lista.iter().enumerate() {
        let x0 = separacao + indice as u32 * (lado + separacao);
        let y0 = separacao;
        for y in 0..lado {
            for x in 0..lado {
                let origem = ((y * lado + x) * 4) as usize;
                let destino = (((y + y0) * largura + (x + x0)) * 4) as usize;
                pixels[destino..destino + 4].copy_from_slice(&textura.pixels[origem..origem + 4]);
            }
        }
    }
    let superficie = Surface::from_data(
        &mut pixels,
        largura,
        altura,
        largura * PixelFormatEnum::RGBA32.byte_size_per_pixel() as u32,
        PixelFormatEnum::RGBA32,
    )?;
    superficie.save_bmp(caminho)?;
    eprintln!(
        "neon-drift: {largura}x{altura}, {} texturas de {lado}x{lado} ({}): {}",
        lista.len(),
        lista.iter().map(|t| t.nome).collect::<Vec<_>>().join(", "),
        caminho
    );
    Ok(())
}

/// Grava o atlas de sprites em BMP, **sem abrir janela nem contexto GL**.
///
/// O atlas é uma função pura (`atlas::gerar`), mas "a função roda" não é a mesma
/// afirmação que "a forma está certa": uma gaussiana centrada errada continua
/// gerando bytes válidos e desenha a nuvem fora do quad. Este arquivo é a única
/// forma de olhar para o que as partículas vão amostrar.
///
/// O fundo sai **magenta**, e não preto: o alfa 0 de uma célula vazia é
/// indistinguível de um sprite preto sobre preto, e o quadro mentiria sobre ter
/// desenhado.
fn gravar_atlas(caminho: &str) -> Result<(), Box<dyn std::error::Error>> {
    let lado = atlas::LADO;
    // Respeita `GIOCHI_SPRITES` como o jogo: uma ferramenta que mostra o atlas
    // procedural enquanto o jogo desenha a folha do usuário mede outra coisa.
    let colunas = std::env::var("GIOCHI_SPRITES_COLUNAS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(atlas::COLUNAS);
    let caminho_folha = std::env::var("GIOCHI_SPRITES").ok();
    let (origem, fonte, problema) = atlas::carregar_ou_gerar(caminho_folha.as_deref(), colunas);
    if let Some(motivo) = problema {
        eprintln!("neon-drift: folha de sprites ignorada — {motivo}");
    }
    let mut pixels = vec![0u8; (lado * lado * 4) as usize];
    for i in 0..(lado * lado) as usize {
        let a = f32::from(origem[i * 4 + 3]) / 255.0;
        // Fundo magenta fixo; o branco do atlas entra por cima ponderado pelo alfa.
        let fundo = [255.0f32, 0.0, 255.0];
        for canal in 0..3 {
            let cor = f32::from(origem[i * 4 + canal]);
            pixels[i * 4 + canal] = (fundo[canal] * (1.0 - a) + cor * a).round() as u8;
        }
        pixels[i * 4 + 3] = 255;
    }
    let superficie = Surface::from_data(
        &mut pixels,
        lado,
        lado,
        lado * PixelFormatEnum::RGBA32.byte_size_per_pixel() as u32,
        PixelFormatEnum::RGBA32,
    )?;
    superficie.save_bmp(caminho)?;
    eprintln!(
        "neon-drift: atlas {}x{} com {} formas ({}) gravado em {caminho}",
        lado,
        lado,
        atlas::Forma::TODAS.len(),
        fonte.nome()
    );
    Ok(())
}

/// Estado do som a partir da corrida.
fn estado_som(corrida: &Corrida) -> EstadoSom {
    let carro = &corrida.carro;
    EstadoSom {
        rpm: carro.rpm(),
        marcha: carro.marcha(),
        acelerando: carro.velocidade > 0.5,
        deslize: if carro.derrapando {
            carro.derrapagem.abs()
        } else {
            0.0
        },
        nitro: carro.nitro_usando,
        kmh: carro.kmh(),
        ativo: corrida.fase == Fase::Correndo && corrida.contagem <= 0.0,
    }
}

fn principal() -> Result<i32, Box<dyn std::error::Error>> {
    restaurar_sigpipe();
    let o = opcoes(std::env::args().skip(1).collect())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    if o.ajuda {
        ajuda();
        return Ok(0);
    }
    if o.versao {
        if o.json {
            println!("{{\"nome\":\"giochi-neon-drift\",\"versao\":\"{VERSAO}\"}}");
        } else {
            println!("giochi-neon-drift {VERSAO}");
        }
        return Ok(0);
    }

    if let Some(caminho) = &o.texturas {
        gravar_texturas(caminho)?;
        return Ok(0);
    }
    if let Some(caminho) = &o.atlas {
        gravar_atlas(caminho)?;
        return Ok(0);
    }

    // GLES 3 por padrão, com queda para 2 — o piso dos aparelhos do projeto.
    if std::env::var_os("GIOCHI_GL_VERSION").is_none() {
        std::env::set_var("GIOCHI_GL_VERSION", "3");
    }

    let plataforma = Platform::new()?;
    let tamanho_painel = painel(&plataforma, &o).map_err(std::io::Error::other)?;
    let (rotacao, fonte_rotacao) =
        video::resolver(tamanho_painel.0, tamanho_painel.1, o.tamanho.is_none());

    let versao_pedida = GlesVersion::from_env();
    let tela_cheia = o.tamanho.is_none();
    let (janela, mut render) =
        match criar_render(&plataforma, tamanho_painel, tela_cheia, versao_pedida, o.sem_vsync) {
            Ok(par) => par,
            Err(erro) if versao_pedida == GlesVersion::Gles3 => {
                eprintln!("aviso: GLES 3.0 indisponível ({erro}); tentando GLES 2.0");
                std::env::set_var("GIOCHI_GL_VERSION", "2");
                criar_render(&plataforma, tamanho_painel, tela_cheia, GlesVersion::Gles2, o.sem_vsync)?
            }
            Err(erro) => return Err(error_de_video(erro.to_string()).into()),
        };

    let (largura, altura) = janela.drawable_size();
    render.resize(largura, altura);
    // O `Upscaler` da engine: desenha num alvo menor e estica com bilinear. Num
    // painel de 720×1280 são 921 mil pixels por quadro, e o Mali-G52 gasta
    // proporcionalmente ao **pixel**, não ao triângulo — é o botão que troca
    // nitidez por quadro em qualquer aparelho, com um número só.
    if o.escala < 0.999 {
        render.set_render_scale(Some(o.escala.clamp(0.25, 1.0)));
        let (rw, rh) = render.render_size();
        eprintln!(
            "neon-drift: escala {:.2} — desenhando em {rw}x{rh} e esticando para {largura}x{altura}",
            o.escala
        );
    }

    // O mapa define traçado, relevo, voltas, tempo limite e cenário. Default: o
    // primeiro da lista, para `--mapa` ausente não mudar o comportamento de
    // ninguém que já usava o binário.
    let mapa = match &o.mapa {
        Some(id) => match giochi_neon_drift_core::mapa::por_id(id) {
            Some(m) => m,
            None => {
                let nomes: Vec<&str> = giochi_neon_drift_core::mapa::TODOS
                    .iter()
                    .map(|m| m.id)
                    .collect();
                return Err(format!("--mapa '{id}' não existe (use: {})", nomes.join(", ")).into());
            }
        },
        None => giochi_neon_drift_core::mapa::PADRAO,
    };
    let dificuldade = match &o.dificuldade {
        Some(id) => match Dificuldade::parse(id) {
            Some(d) => d,
            None => {
                let nomes: Vec<&str> = Dificuldade::todos().iter().map(|d| d.id()).collect();
                return Err(
                    format!("--dificuldade '{id}' não existe (use: {})", nomes.join(", ")).into(),
                );
            }
        },
        None => Dificuldade::Normal,
    };
    let mut corrida = Corrida::com_mapa_e_dificuldade(mapa, dificuldade);
    eprintln!(
        "neon-drift: dificuldade {} ({}) — jogador {:.0}%, adversários {:.0}%",
        dificuldade.nome(),
        dificuldade.descricao(),
        dificuldade.fator_jogador() * 100.0,
        dificuldade.fator_ia() * 100.0
    );
    if let Some(em) = o.em {
        // `--em` implica corrida em andamento: posicionar na tela de título
        // mediria a pista errada.
        if corrida.fase == Fase::Titulo {
            corrida.comecar();
            corrida.contagem = 0.0;
        }
        let total = corrida.pista.comprimento();
        let s = em.rem_euclid(total.max(1.0));
        corrida.carro.posicionar(&corrida.pista, s, 0.0);
        corrida.carro.velocidade = 48.0;
        // A largada é dada por cruzada: começar em cima da pista sem marcar
        // `largou` deixaria a volta e o progresso negativos.
        corrida.marcar_largou();
        eprintln!("--em: carro em {s:.0} m de {total:.0} m, a 48 m/s");
    }
    if let Some(tela) = o.tela {
        match tela {
            Fase::Correndo => corrida.comecar(),
            Fase::Resultado => {
                // Resultado sem ninguém correndo: usa "tempo esgotado", que é o
                // desfecho honesto dessa situação.
                corrida.comecar();
                corrida.contagem = 0.0;
                corrida.tempo = corrida.tempo_limite;
                corrida.passo(&Entrada::PARADA, 1.0 / TIQUE);
                corrida.fase = Fase::Resultado;
            }
            Fase::Titulo => corrida.titulo(),
        }
    }

    let mut cena = Cena3D::nova(&mut render, &corrida.pista, rotacao, corrida.mapa.cenario)?;
    cena.so(o.so.clone());
    let mut indice_mapa = giochi_neon_drift_core::mapa::TODOS
        .iter()
        .position(|m| std::ptr::eq(*m, mapa))
        .unwrap_or(0);

    if !o.json {
        eprintln!(
            "neon-drift: painel {largura}x{altura}, rotação {} ({fonte_rotacao}), GLES {} em {}",
            rotacao.nome(),
            render.gles_version().major(),
            render.adapter()
        );
        eprintln!(
            "neon-drift: mapa \"{}\" ({}) — pista {:.0} m, desnível {:.0} m",
            mapa.nome,
            mapa.id,
            corrida.pista.comprimento(),
            mapa.desnivel()
        );
        eprintln!(
            "neon-drift: instancing {}, pista {:.0} m, {} voltas, limite {} fps",
            if cena.usa_instancing() {
                "ativo"
            } else {
                "indisponível"
            },
            corrida.pista.comprimento(),
            corrida.total_voltas,
            o.fps
        );
    }

    let botoes = evdev::Botoes::iniciar();
    if !o.json {
        eprintln!("neon-drift: {} dispositivo(s) de entrada", botoes.abertos());
    }
    let mut som = if o.sem_som {
        Som::sem_dispositivo()
    } else {
        Som::iniciar(&plataforma)
    };

    // `--passos` antes do laço: é o que permite capturar um quadro com a corrida
    // em andamento (drift, nitro, HUD cheio) sem dedo no aparelho.
    if o.passos > 0 {
        let (entrada, acoes) = entrada_jogo::combinar(0, &o.teclas, corrida.fase);
        if acoes.comecar {
            corrida.comecar();
        }
        // `--piloto` dirige no lugar das teclas fixas. Sem ele, `--passos 900`
        // com "acelerar" apertado só leva o carro de encontro ao primeiro muro —
        // medido: 2,6 m/s encostado na borda, e o quadro capturado mostrava a
        // linha de largada como se a corrida nem tivesse começado.
        for i in 0..o.passos {
            let entrada = if o.piloto {
                giochi_neon_drift_core::piloto::entrada(&corrida)
            } else {
                entrada
            };
            corrida.passo(&entrada, 1.0 / TIQUE);
            cena.seguir(&corrida, 1.0 / TIQUE);
            if corrida.fase == Fase::Resultado {
                break;
            }
            let _ = i;
        }
    }

    let mut teclas = o.teclas;
    let mut sair = false;
    // Último `trocar_mapa` visto, para trocar só na borda.
    let mut ultima_troca = 0i32;
    let mut ultima_dificuldade = 0i32;
    let mut relogio = Relogio::com_ritmo(TIQUE, o.fps);

    // Prova de rotação: desenha três barras e sai. Roda antes de tudo porque não
    // depende de estado de partida nenhum — é o teste da **geometria** da tela.
    if let Some(caminho) = &o.prova_rotacao {
        cena.preparar(&mut render);
        render.begin_frame(&janela);
        cena.desenhar_prova_rotacao(&mut render)?;
        let (w, h) = gravar_quadro(&render, caminho)?;
        render.end_frame(&janela);
        if o.json {
            println!(
                "{{\"prova\":\"gravada\",\"largura\":{w},\"altura\":{h},\"rotacao\":\"{}\",\"fonte\":\"{}\"}}",
                rotacao.nome(),
                fonte_rotacao
            );
        }
        return Ok(0);
    }

    // Quadro único: captura e sai, sem laço de eventos.
    if let Some(caminho) = &o.quadro {
        cena.preparar(&mut render);
        render.begin_frame(&janela);
        cena.desenhar(&mut render, &corrida, 1.0 / TIQUE)?;
        let (w, h) = gravar_quadro(&render, caminho)?;
        render.end_frame(&janela);
        let erro = render.take_gl_error();
        if erro != 0 {
            return Err(format!("erro GLES 0x{erro:04x} ao desenhar o quadro").into());
        }
        if o.json {
            println!(
                "{{\"quadro\":\"gravado\",\"largura\":{w},\"altura\":{h},\"gles\":{},\"instancing\":{},\"desenhos\":{},\"fase\":\"{:?}\",\"efeitos\":{{{}}},{}}}",
                render.gles_version().major(),
                cena.usa_instancing(),
                render.frame_counters().draw_calls,
                corrida.fase,
                cena.efeitos_json(),
                diagnostico(&corrida, &cena, &render)
            );
        } else {
            eprintln!("neon-drift: quadro {w}x{h} gravado em {caminho}");
        }
        render.release_all();
        return Ok(0);
    }

    let inicio = Instant::now();
    let mut quadros = 0u32;
    let mut passos_totais = 0u32;
    let mut ultimo = Instant::now();
    // Soma do tempo de CPU por quadro, sem o swap (diagnóstico de `--json`).
    let mut ms_cpu = 0.0f64;

    while !sair {
        for evento in plataforma.poll_events() {
            match evento {
                Event::Quit { .. } => sair = true,
                Event::KeyDown {
                    keycode: Some(tecla),
                    repeat: false,
                    ..
                } => {
                    aplicar_tecla(&mut teclas, tecla, true);
                }
                Event::KeyUp {
                    keycode: Some(tecla), ..
                } => {
                    aplicar_tecla(&mut teclas, tecla, false);
                }
                _ => {}
            }
        }
        if sair {
            break;
        }

        // A máscara é lida **uma** vez. Lendo duas (uma para decidir, outra
        // para qualquer uso posterior), uma borda de pressão pode cair entre as
        // duas leituras e o quadro age sobre um estado que já mudou.
        let bits_agora = botoes.bits();
        let (entrada, acoes) = entrada_jogo::combinar(bits_agora, &teclas, corrida.fase);
        // Troca de mapa na tela de título: o mapa é **construtivo** (define
        // pista, relevo, voltas, cenário e até as malhas), então trocar é
        // reconstruir a corrida e a cena. `release_all` antes libera as malhas
        // e texturas do mapa anterior; sem isso cada troca deixaria um mundo
        // pendurado na GPU.
        //
        // Borda, não nível: segurar a seta trocaria de mapa 75 vezes por segundo.
        // O teste com `--teclas right` mostrou exatamente isso — quatro quadros,
        // quatro trocas —, e uma seta segurada é o caso normal de quem está
        // escolhendo.
        let troca = if acoes.trocar_mapa != ultima_troca {
            acoes.trocar_mapa
        } else {
            0
        };
        ultima_troca = acoes.trocar_mapa;
        // Dificuldade: mesma regra de borda, e o mapa em vigor é preservado.
        let troca_dif = if acoes.trocar_dificuldade != ultima_dificuldade {
            acoes.trocar_dificuldade
        } else {
            0
        };
        ultima_dificuldade = acoes.trocar_dificuldade;
        if troca_dif != 0 && corrida.fase == Fase::Titulo {
            let mut nova = corrida.dificuldade;
            for _ in 0..troca_dif.abs() {
                nova = if troca_dif > 0 {
                    nova.proxima()
                } else {
                    nova.anterior()
                };
            }
            if nova != corrida.dificuldade {
                corrida.definir_dificuldade(nova);
                if !o.json {
                    eprintln!(
                        "neon-drift: dificuldade {} ({})",
                        nova.nome(),
                        nova.descricao()
                    );
                }
            }
        }

        if troca != 0 && corrida.fase == Fase::Titulo {
            let total = giochi_neon_drift_core::mapa::TODOS.len() as i32;
            let atual = indice_mapa as i32;
            let novo = (atual + troca).rem_euclid(total) as usize;
            if novo != indice_mapa {
                indice_mapa = novo;
                let mapa = giochi_neon_drift_core::mapa::TODOS[novo];
                corrida = Corrida::com_mapa_e_dificuldade(mapa, corrida.dificuldade);
                render.release_all();
                // `release_all` leva o buffer de instâncias junto. Sem recriar,
                // o mundo novo desenha carros e partículas **empilhados na
                // origem** — medido antes desta linha: 50 desenhos por quadro
                // antes da troca, 30 depois, porque `desenhar_carros` e
                // `desenhar_particulas` desistem sem instancing.
                render.rebuild_instancing();
                cena = Cena3D::nova(&mut render, &corrida.pista, rotacao, mapa.cenario)?;
                cena.so(o.so.clone());
                if !o.json {
                    eprintln!(
                        "neon-drift: mapa \"{}\" ({}) — pista {:.0} m, desnível {:.0} m",
                        mapa.nome,
                        mapa.id,
                        corrida.pista.comprimento(),
                        mapa.desnivel()
                    );
                }
            }
        }
        let reiniciou = acoes.reiniciar;
        sair |= executar_acoes(&mut corrida, acoes);
        if reiniciou {
            // A fumaça da corrida anterior não pode reaparecer no ar do reinício.
            cena.limpar_efeitos();
        }
        if sair {
            break;
        }

        let agora = inicio.elapsed().as_secs_f64();
        let planejado = relogio.avancar(agora);
        for _ in 0..planejado.ticks {
            let evento = corrida.passo(&entrada, 1.0 / TIQUE);
            passos_totais += 1;
            // Só a batida precisa avisar a cena; o resto ela lê do estado.
            if evento == Evento::Bateu {
                cena.bater();
            }
        }

        let dt = ultimo.elapsed().as_secs_f32().min(0.1);
        ultimo = Instant::now();
        som.atualizar(&estado_som(&corrida), dt);

        cena.preparar(&mut render);
        let t_cpu = Instant::now();
        render.begin_frame(&janela);
        cena.desenhar(&mut render, &corrida, dt)?;
        // Antes do `end_frame`: ele faz o swap e **bloqueia no vblank**, então
        // incluí-lo transformaria toda medida em "tempo de vsync" e não diria
        // nada sobre o custo do quadro.
        ms_cpu += t_cpu.elapsed().as_secs_f64() * 1000.0;
        render.end_frame(&janela);
        quadros += 1;

        if let Some(limite) = o.quadros {
            if quadros >= limite {
                break;
            }
        }

        std::thread::sleep(planejado.dormir);
    }

    let decorrido = inicio.elapsed().as_secs_f64();
    let fps_medio = if decorrido > 0.0 {
        (quadros.saturating_sub(1)) as f64 / decorrido
    } else {
        0.0
    };
    if o.json {
        // Stdout é **só dado** neste ramo: as linhas `neon-drift:` (texturas,
        // painel, mapa, atlas) vão para stderr. Antes elas dividiam o stdout com
        // o JSON e `--json | jq` falhava com "Expecting value: line 1 column 1" —
        // o consumidor lia a prosa do diagnóstico como se fosse o objeto.
        println!(
            // `posicao`, `volta` e `drift` **não** aparecem aqui: `diagnostico`
            // já os emite, e a versão anterior do JSON trazia os dois — chave
            // duplicada, com o último valor vencendo em silêncio. Duas fontes do
            // mesmo número envelhecem diferente; esta é a que não tem a corrida
            // em mãos.
            "{{\"quadros\":{quadros},\"passos\":{passos_totais},\"segundos\":{decorrido:.3},\"fps\":{fps_medio:.1},\"fps_teto\":{},\"ms_cpu\":{:.2},\"particulas\":{},\"desenhos\":{},\"efeitos\":{{{}}},\"gles\":{},\"instancing\":{},\"fase\":\"{:?}\",{}}}",
            o.fps,
            if quadros > 0 { ms_cpu / quadros as f64 } else { 0.0 },
            cena.particulas_vivas(),
            render.frame_counters().draw_calls,
            cena.efeitos_json(),
            render.gles_version().major(),
            cena.usa_instancing(),
            corrida.fase,
            diagnostico(&corrida, &cena, &render)
        );
    } else {
        eprintln!(
            "neon-drift: fim — {} quadros em {:.2} s ({:.1} fps; teto {}) · drift {} · {}",
            quadros,
            decorrido,
            fps_medio,
            o.fps,
            hud::pontos(corrida.drift.total()),
            corrida.desfecho.map(|d| d.titulo()).unwrap_or("em andamento")
        );
    }
    render.release_all();
    Ok(0)
}

/// Liga ou desliga uma tecla do estado.
fn aplicar_tecla(teclas: &mut Teclas, tecla: Keycode, apertou: bool) {
    let alvo = match tecla {
        Keycode::Up | Keycode::W => &mut teclas.cima,
        Keycode::Down | Keycode::S => &mut teclas.baixo,
        Keycode::Left | Keycode::A => &mut teclas.esquerda,
        Keycode::Right | Keycode::D => &mut teclas.direita,
        Keycode::Space => &mut teclas.espaco,
        Keycode::LShift | Keycode::RShift => &mut teclas.shift,
        Keycode::Return | Keycode::KpEnter => &mut teclas.enter,
        Keycode::Escape | Keycode::Q => &mut teclas.escape,
        _ => return,
    };
    *alvo = apertou;
}

/// Executa começar/reiniciar/sair. Devolve `true` quando o laço deve terminar.
fn executar_acoes(corrida: &mut Corrida, acoes: Acoes) -> bool {
    if acoes.comecar {
        corrida.comecar();
    }
    if acoes.reiniciar {
        corrida.reiniciar();
    }
    acoes.sair
}

/// Mensagem de erro de vídeo com dica do que fazer.
fn error_de_video(mensagem: String) -> std::io::Error {
    let dica = if mensagem.contains("kmsdrm") {
        "  outro processo está com o DRM (o launcher?). Feche-o antes.\n"
    } else {
        ""
    };
    std::io::Error::other(format!("{mensagem}\n{dica}"))
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
    fn teclas_da_cli_viram_estado() {
        let t = teclas("a,right,shift").unwrap();
        assert!(t.cima);
        assert!(t.direita);
        assert!(t.shift);
        assert!(!t.esquerda);
    }

    #[test]
    fn tecla_desconhecida_e_erro_com_nome() {
        let erro = teclas("a,voar").unwrap_err();
        assert!(erro.contains("voar"), "{erro}");
    }

    #[test]
    fn o_teto_de_fps_recusa_acima_de_75() {
        let erro = opcoes(vec!["--fps-teto".into(), "76".into()]).unwrap_err();
        assert!(erro.contains("1 e 75"), "{erro}");
    }

    #[test]
    fn o_teto_padrao_e_75() {
        assert_eq!(opcoes(vec![]).unwrap().fps, 75);
        assert!(opcoes(vec!["--fps-teto".into(), "0".into()]).is_err());
    }

    #[test]
    fn tamanho_invalido_e_erro() {
        assert!(opcoes(vec!["--tamanho".into(), "640".into()]).is_err());
        assert!(opcoes(vec!["--tamanho".into(), "0x480".into()]).is_err());
        let o = opcoes(vec!["--tamanho".into(), "800x600".into()]).unwrap();
        assert_eq!(o.tamanho, Some((800, 600)));
    }

    #[test]
    fn tela_desconhecida_e_erro() {
        // Toda flag que consome um valor tem de avançar o índice. Faltar o
        // `i += 2` num braço não dá erro de compilação nem mensagem: o laço relê
        // a mesma flag para sempre, e o sintoma é um processo que não imprime
        // nada e não termina. Aconteceu com `--prova-rotacao`.
        for flag in [
            "--quadro",
            "--texturas",
            "--atlas",
            "--prova-rotacao",
            "--tamanho",
            "--mapa",
            "--config",
            "--classe",
            "--perfil",
            "--em",
            "--teclas",
            "--tela",
            "--fps-teto",
            "--escala",
        ] {
            let valor = if flag == "--tamanho" {
                "640x480"
            } else if flag == "--fps-teto" {
                "60"
            } else if flag == "--escala" {
                "0.5"
            } else if flag == "--em" {
                "100"
            } else if flag == "--tela" {
                "titulo"
            } else if flag == "--mapa" {
                "avenida"
            } else if flag == "--classe" {
                "soldado"
            } else if flag == "--perfil" {
                "balanceado"
            } else {
                "x"
            };
            // Se o braço não avançar, esta chamada nunca retorna — por isso ela
            // roda com um valor que o braço precisa consumir.
            let r = opcoes(vec![flag.into(), valor.into()]);
            assert!(r.is_ok() || r.is_err(), "{flag} não terminou");
        }
        assert!(opcoes(vec!["--tela".into(), "inexistente".into()]).is_err());
        assert_eq!(
            opcoes(vec!["--tela".into(), "resultado".into()]).unwrap().tela,
            Some(Fase::Resultado)
        );
    }

    #[test]
    fn argumento_desconhecido_nao_e_ignorado() {
        assert!(opcoes(vec!["--turbo".into()]).unwrap_err().contains("--turbo"));
    }

    #[test]
    fn quadros_zero_e_recusado() {
        assert!(opcoes(vec!["--quadros".into(), "0".into()]).is_err());
    }

    #[test]
    fn as_teclas_do_jogo_mapeiam_para_os_campos_certos() {
        let mut t = Teclas::default();
        aplicar_tecla(&mut t, Keycode::Up, true);
        aplicar_tecla(&mut t, Keycode::Space, true);
        aplicar_tecla(&mut t, Keycode::LShift, true);
        assert!(t.cima && t.espaco && t.shift);
        aplicar_tecla(&mut t, Keycode::Up, false);
        assert!(!t.cima, "soltar tem de desligar");
    }

    #[test]
    fn as_teclas_wasd_fazem_o_mesmo_que_as_setas() {
        let mut com_seta = Teclas::default();
        aplicar_tecla(&mut com_seta, Keycode::Left, true);
        let mut com_wasd = Teclas::default();
        aplicar_tecla(&mut com_wasd, Keycode::A, true);
        assert_eq!(com_seta, com_wasd);
    }
}
