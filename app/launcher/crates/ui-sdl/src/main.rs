//! Entrada do launcher.
//!
//! Uso:
//!   giochi-launcher                 janela 1280×720
//!   giochi-launcher --fullscreen    tela cheia
//!   giochi-launcher --font <ttf>    fonte alternativa
//!   giochi-launcher --dump-frame <bmp>
//!                                   desenha a Home num BMP e sai
//!
//! Para o aparelho (GLES2 sobre KMSDRM), defina `GIOCHI_GLES=1`. No PC deixe
//! vazio: forçar `opengles2` no desktop costuma falhar.

mod app;
mod draw;
mod icons;
mod input_evdev;
mod theme;
mod views;

use std::time::{Duration, Instant};

use giochi_launcher_core::{App as AppEntry, Game, State};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let fullscreen = args.iter().any(|a| a == "--fullscreen");
    // Diagnóstico: desenha um quadro e sai. Serve para conferir a UI com
    // `SDL_VIDEODRIVER=dummy`, numa máquina sem tela.
    let dump_frame = args
        .iter()
        .position(|a| a == "--dump-frame")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let font_path = args
        .iter()
        .position(|a| a == "--font")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(default_font);

    if std::env::var("GIOCHI_GLES").is_ok() {
        // No aparelho o backend é GLES2; no desktop o padrão do SDL é melhor.
        sdl2::hint::set("SDL_RENDER_DRIVER", "opengles2");
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    let mut builder = video.window("Giochi Launcher", 1280, 720);
    if fullscreen {
        builder.fullscreen_desktop();
    }
    let window = builder
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| format!("janela: {e}"))?;

    let painter = draw::Painter::new(window, &font_path)?;
    let state = State::new(demo_games(), demo_apps());

    // Entrada por evdev: o SDL fica só com a janela e o desenho.
    let map = input_evdev::ButtonsMap::load(std::path::Path::new(
        input_evdev::DEFAULT_MAP_PATH,
    ));
    if !map.is_complete() {
        eprintln!(
            "aviso: mapa de botões incompleto; usando o padrão embutido onde faltar"
        );
    }
    let evdev = input_evdev::EvdevInput::open(std::path::Path::new("/dev/input"), map);
    if evdev.device_count() == 0 {
        eprintln!(
            "aviso: nenhum dispositivo em /dev/input — o controle não responderá.\n\
             No PC, isso é esperado se você não tem controle conectado."
        );
    }

    let mut app = app::App::new(painter, state, evdev);

    let mut events = sdl.event_pump()?;
    let mut last = Instant::now();

    'outer: loop {
        for event in events.poll_iter() {
            if !app.handle_event(&event) {
                break 'outer;
            }
        }

        let now = Instant::now();
        let delta = now.duration_since(last).as_secs_f32();
        last = now;

        app.tick(delta.min(0.1));
        app.render();

        if let Some(path) = dump_frame.as_deref() {
            if app.state.screen == giochi_launcher_core::Screen::Running {
                app.save_frame(path)?;
                break;
            }
        }

        // Trava em ~60 Hz sem depender de vsync do backend.
        let frame = Duration::from_micros(16_666);
        let elapsed = now.elapsed();
        if elapsed < frame {
            std::thread::sleep(frame - elapsed);
        }
    }

    Ok(())
}

fn default_font() -> String {
    // DejaVu Sans. Procura primeiro no pacote do app (o que vai para o
    // aparelho), depois no sistema — em distro Linux ele já está instalado.
    const CANDIDATES: [&str; 4] = [
        "assets/fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "app/launcher/assets/fonts/DejaVuSans.ttf",
    ];
    for candidate in CANDIDATES {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    // Caminho que provavelmente não existe: o desenho continua, só sem texto,
    // e o aviso aparece uma vez no console.
    CANDIDATES[0].to_string()
}

/// Catálogo de demonstração.
///
/// São os quatro jogos que existem no projeto — o protótipo listava títulos
/// comerciais que não estão no catálogo. A leitura real virá de
/// `apps.json` + `/apps` no aparelho.
fn demo_games() -> Vec<Game> {
    vec![
        Game::new(0, "Arcade Volleyball", "Esporte", 0.0, "nunca", true),
        Game::new(1, "Colossal Cave Adventure", "Aventura", 0.0, "nunca", false),
        Game::new(2, "Spacewar", "Ação", 0.0, "nunca", true),
        Game::new(3, "Shattered Pixel Dungeon", "Roguelike", 0.0, "nunca", false),
    ]
}

fn demo_apps() -> Vec<AppEntry> {
    vec![
        AppEntry::new("music", "Música", "music", "Mídia"),
        AppEntry::new("retro", "RetroArch", "retro", "Emulador"),
        AppEntry::new("files", "Arquivos", "files", "Sistema"),
        AppEntry::new("term", "Terminal", "term", "Avançado"),
        AppEntry::new("browse", "Navegador", "browse", "Internet"),
        AppEntry::new("shots", "Capturas", "camera", "Mídia"),
        AppEntry::new("store", "Loja", "cart", "Sistema"),
        AppEntry::new("sync", "Sincronizar", "sync", "Nuvem"),
    ]
}
