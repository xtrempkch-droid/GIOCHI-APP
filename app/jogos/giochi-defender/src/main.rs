//! GIOCHI DEFENDER — host do jogo.
//!
//! ## Por que não existe um `Engine` aqui
//!
//! `giochi_engine::Engine` é a fachada **2D**: ela cria um `RenderContext` sobre
//! `sdl2::render`. O ADR-037 da engine diz que um jogo escolhe um renderer e não
//! troca em tempo de execução — e este jogo é 3D (GLES sobre EGL). As duas coisas
//! não compartilham janela.
//!
//! O que se reusa é o resto, que é a maior parte: `Platform` (SDL2, KMSDRM),
//! `Window` (com a flag OpenGL), `Input`, `AudioSystem`, `GameLoop` e `FrameClock`
//! — todos da engine 2D, que os cede sem exigir o renderer dela.
//!
//! O laço abaixo é o mesmo desenho do `Engine::run`: passo fixo de 1/60 s,
//! acumulador com teto, `render` uma vez por quadro de tela.

use std::process::ExitCode;
use std::time::Instant;

use anyhow::{bail, Result};
use giochi_engine::audio::AudioSystem;
use giochi_engine::core::game_loop::GameLoop;
use giochi_engine::core::time::FrameClock;
use giochi_engine::input::gamepad::{AXIS_RIGHT_X, AXIS_RIGHT_Y};
use giochi_engine::input::keyboard::{
    KEY_A, KEY_D, KEY_DOWN, KEY_ESCAPE, KEY_LEFT, KEY_RIGHT, KEY_S, KEY_UP, KEY_W,
};
use giochi_engine::input::Input;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine::platform::{quit_signal, Platform, QuitSignal};
use giochi_engine_3d::context::{self, GlesVersion};
use giochi_engine_3d::math3d::Camera3D;
use giochi_engine_3d::RenderContext3D;

use giochi_defender::diag::{self, Diag};
use giochi_defender::gameplay::arena::{self, Arena};
use giochi_defender::gameplay::camera::OrbitalCamera;
use giochi_defender::state::{GameState, WAVES_TOTAIS};
use giochi_defender::utils::math::{graus_para_radianos, radianos_para_graus};

/// Tamanho pedido da janela. No aparelho o painel manda (ADR-035).
const LARGURA: u32 = 1280;
/// Altura pedida da janela.
const ALTURA: u32 = 720;
/// Título da janela.
const TITULO: &str = "GIOCHI DEFENDER";

/// Giro do teclado, em radianos por segundo.
const TAXA_TECLADO: f32 = 1.8;
/// Giro do analógico direito na deflexão máxima, em radianos por segundo.
const TAXA_ANALOGICO: f32 = 2.6;
/// Zona morta do analógico: abaixo disto o valor é ruído do potenciômetro.
const ZONA_MORTA: f32 = 0.25;
/// Orçamento de um quadro a 60 Hz, em milissegundos.
const QUADRO_MS: u32 = 1000 / 60;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let diag = match Diag::parse(std::env::args().skip(1)) {
        Ok(diag) => diag,
        Err(erro) => {
            eprintln!("erro: {erro:#}");
            return ExitCode::FAILURE;
        }
    };

    if diag.ajuda {
        println!("{}", diag::AJUDA);
        return ExitCode::SUCCESS;
    }

    match executar(diag) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erro) => {
            log::error!("{erro:#}");
            ExitCode::FAILURE
        }
    }
}

/// Abre dispositivo, janela e contexto, e roda até o ESC ou até o limite de
/// quadros.
fn executar(diag: Diag) -> Result<()> {
    let plataforma = Platform::new()?;
    log::info!(
        "GIOCHI DEFENDER boot — engine {} · vídeo '{}'",
        giochi_engine_3d::VERSION,
        plataforma.video_driver_name()
    );

    let requisitada = GlesVersion::from_env();
    // Os atributos `SDL_GL_*` são globais no SDL2 e lidos na criação da JANELA:
    // configurá-los depois de abrir a janela deixa uma janela sem a flag OpenGL.
    context::configure_attributes(&plataforma, requisitada)?;

    let janela = Window::new_opengl(&plataforma, configuracao_janela(&plataforma), TITULO)?;
    let mut render = RenderContext3D::new(context::create(&janela)?);
    render.set_clear_color(arena::COR_ESPACO);

    // O drawable só é conhecido depois do primeiro quadro: no KMSDRM ele é o
    // painel, não o tamanho pedido (ADR-035). Aqui o que se sabe é o que foi
    // pedido.
    log::info!(
        "GLES {} em '{}' · janela pedida {}x{}",
        render.gles_version().major(),
        render.adapter(),
        janela.size().0,
        janela.size().1
    );
    if requisitada == GlesVersion::Gles3 && !render.supports_gles3() {
        log::warn!("GLES 3.0 foi pedido e o driver entregou 2.0; seguindo em 2.0");
    }

    let mut entrada = Input::new(&plataforma);
    // R6: áudio quebrado é silêncio com aviso, nunca `panic`.
    let audio = iniciar_audio(&plataforma);

    if diag.mouse_relativo {
        prender_cursor(&plataforma);
    }

    let arena = Arena::new(&mut render)?;
    let mut camera = OrbitalCamera::new();
    aplicar_preset(&mut camera, &diag);
    let estado = GameState::new();

    log::info!(
        "arena: asteroide raio {:.2} (externo {:.2}), {} estrelas, {} waves",
        arena.raio(),
        arena.raio_externo(),
        arena.estrelas(),
        WAVES_TOTAIS
    );
    log::info!(
        "câmera: yaw {:.1}°, pitch {:.1}°, distância {:.1}",
        radianos_para_graus(camera.yaw),
        radianos_para_graus(camera.pitch_seguro()),
        camera.distancia_segura()
    );
    log::info!(
        "partida nova: {} vidas, wave 1 de {}",
        estado.lives,
        WAVES_TOTAIS
    );

    let resultado = laco(
        &plataforma,
        &janela,
        &mut render,
        &mut entrada,
        &arena,
        &mut camera,
        &diag,
    );

    arena.destruir(&mut render);
    log::info!(
        "encerrando: {} malhas e {} shaders ainda na engine (devem ser 0)",
        render.mesh_count(),
        render.shader_count()
    );

    // O áudio é solto depois do laço para o mixer não segurar o dispositivo
    // enquanto o processo termina.
    drop(audio);

    resultado
}

/// Janela em modo janela no PC e em *fullscreen* no console.
///
/// O KMSDRM não tem gerenciador de janelas: pedir uma janela lá é pedir para
/// desenhar num canto da tela. Detectar pelo driver evita um `--fullscreen` que
/// alguém esquece de passar no aparelho.
fn configuracao_janela(plataforma: &Platform) -> WindowConfig {
    let config = WindowConfig::new(LARGURA, ALTURA);
    if plataforma
        .video_driver_name()
        .eq_ignore_ascii_case("kmsdrm")
    {
        config.with_fullscreen()
    } else {
        config
    }
}

/// Cria o sistema de áudio, caindo para o modo silencioso quando não há device.
fn iniciar_audio(plataforma: &Platform) -> AudioSystem {
    match AudioSystem::new(plataforma) {
        Ok(audio) => audio,
        Err(erro) => {
            log::warn!("áudio indisponível ({erro}); seguindo em silêncio");
            AudioSystem::silent()
        }
    }
}

/// Prende o cursor no centro da janela, para a câmera relativa.
fn prender_cursor(plataforma: &Platform) {
    match plataforma.sdl_mouse() {
        Some(mouse) => mouse.set_relative_mouse_mode(true),
        None => log::warn!("sem subsistema de mouse; a câmera fica no teclado/analógico"),
    }
}

/// Aplica `--yaw`, `--pitch` e `--distancia` sobre a câmera inicial.
fn aplicar_preset(camera: &mut OrbitalCamera, diag: &Diag) {
    if let Some(graus) = diag.yaw_graus {
        camera.yaw = graus_para_radianos(graus);
    }
    if let Some(graus) = diag.pitch_graus {
        camera.pitch = graus_para_radianos(graus);
    }
    if let Some(distancia) = diag.distancia {
        camera.distance = distancia;
    }
}

/// Gira a câmera com mouse, teclado e analógico direito.
///
/// Os três somam em vez de competir: no aparelho não há mouse, no PC não há
/// controle, e um único caminho de câmera evita que o teste no PC meça outro
/// comportamento.
fn atualizar_camera(camera: &mut OrbitalCamera, entrada: &Input, dt: f32) {
    let delta = entrada.mouse().delta();
    if delta.x != 0.0 || delta.y != 0.0 {
        camera.orbitar(delta.x, delta.y);
    }

    let roda = entrada.mouse().wheel_delta();
    if roda != 0.0 {
        camera.zoom(roda);
    }

    let mut yaw = 0.0;
    let mut pitch = 0.0;
    if entrada.key_down(KEY_LEFT) || entrada.key_down(KEY_A) {
        yaw -= 1.0;
    }
    if entrada.key_down(KEY_RIGHT) || entrada.key_down(KEY_D) {
        yaw += 1.0;
    }
    if entrada.key_down(KEY_UP) || entrada.key_down(KEY_W) {
        pitch += 1.0;
    }
    if entrada.key_down(KEY_DOWN) || entrada.key_down(KEY_S) {
        pitch -= 1.0;
    }

    let (analogico_x, analogico_y) = com_zona_morta(
        entrada.gamepad().axis(AXIS_RIGHT_X),
        entrada.gamepad().axis(AXIS_RIGHT_Y),
    );
    yaw += analogico_x * (TAXA_ANALOGICO / TAXA_TECLADO);
    pitch += analogico_y * (TAXA_ANALOGICO / TAXA_TECLADO);

    if yaw != 0.0 || pitch != 0.0 {
        camera.girar(yaw * TAXA_TECLADO * dt, pitch * TAXA_TECLADO * dt);
    }
}

/// Aplica a zona morta e reescala o resto para `0..=1`.
///
/// Sem a reescala, o analógico "começa" a 0.25 e o movimento tem um degrau; com
/// ela, sair da zona morta dá velocidade zero e a rampa é contínua.
fn com_zona_morta(x: f32, y: f32) -> (f32, f32) {
    let aplicar = |valor: f32| {
        let modulo = valor.abs();
        if modulo <= ZONA_MORTA {
            0.0
        } else {
            valor.signum() * ((modulo - ZONA_MORTA) / (1.0 - ZONA_MORTA)).min(1.0)
        }
    };
    (aplicar(x), aplicar(y))
}

/// O laço principal.
#[allow(clippy::too_many_arguments)]
fn laco(
    plataforma: &Platform,
    janela: &Window,
    render: &mut RenderContext3D,
    entrada: &mut Input,
    arena: &Arena,
    camera: &mut OrbitalCamera,
    diag: &Diag,
) -> Result<()> {
    let mut relogio = FrameClock::new();
    let mut laco = GameLoop::new();
    let mut quadros: u64 = 0;

    loop {
        let inicio = Instant::now();

        let eventos = plataforma.poll_events();
        let mut pediu_sair = false;
        for evento in &eventos {
            if quit_signal(evento) != QuitSignal::None {
                pediu_sair = true;
            }
        }
        entrada.update(&eventos);
        if pediu_sair || entrada.key_pressed(KEY_ESCAPE) {
            break;
        }

        // Em modo de captura o passo é fixo: dois processos têm de simular o
        // mesmo número de segundos, senão comparar quadros mede o relógio.
        let delta = if diag.frames.is_some() {
            laco.fixed_timestep()
        } else {
            relogio.tick()
        };

        let passos = laco.advance(delta);
        for _ in 0..passos {
            atualizar_camera(camera, entrada, laco.fixed_timestep());
            // O bloco 1b pendura aqui a atualização de inimigos e da wave; no 1a
            // não há nada a simular além da câmera.
        }

        let (largura, altura) = janela.drawable_size();
        let camera3d: Camera3D = camera.camera(largura, altura);

        render.begin_frame(janela);
        arena.desenhar(render, &camera3d)?;

        // A captura tem de acontecer ANTES da troca de buffers: depois do
        // `end_frame` o buffer de trás já tem o quadro seguinte (ou lixo).
        let capturar = diag
            .quadro
            .as_deref()
            .filter(|_| diag.frames.is_some_and(|limite| quadros + 1 >= limite));
        if let Some(caminho) = capturar {
            let (largura, altura) = render.drawable_size();
            let pixels = render.read_pixels(largura, altura)?;
            diag::escrever_bmp(std::path::Path::new(caminho), largura, altura, &pixels)?;
            log::info!("quadro gravado em '{caminho}' ({largura}x{altura})");
        }

        render.end_frame(janela);

        // Drenar a fila de erro a cada quadro é o que transforma "tela preta"
        // numa falha com nome.
        let erro = render.take_gl_error();
        if erro != 0 {
            bail!("erro de GL 0x{erro:04x} no quadro {quadros}");
        }

        quadros += 1;

        if let Some(limite) = diag.frames {
            if quadros >= limite {
                break;
            }
            continue;
        }

        // Teto de 60 quadros por segundo. O vsync já limita no aparelho; no PC,
        // sem esta espera, um quadro trivial roda a milhares de Hz e aquece a
        // máquina à toa.
        let gasto = inicio.elapsed().as_millis() as u32;
        if gasto < QUADRO_MS {
            plataforma.delay_ms(QUADRO_MS - gasto);
        }
    }

    let (largura, altura) = render.drawable_size();
    log::info!("{quadros} quadros renderizados · drawable {largura}x{altura}");
    Ok(())
}
