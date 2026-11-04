//! Entrada do launcher.
//!
//! Uso:
//!   giochi-launcher                 janela 1280×720
//!   giochi-launcher --fullscreen    tela cheia
//!   giochi-launcher --font <ttf>    fonte alternativa
//!   giochi-launcher --dump-frame <bmp>
//!                                   desenha a Home num BMP e sai
//!   giochi-launcher --testar-backlight
//!                                   apaga o painel 2 s e restaura
//!   giochi-launcher --brilho 40     aplica 40% pelo mesmo caminho do slider
//!   giochi-launcher --volume 30     aplica 30% no mixer, pelo mesmo caminho da UI
//!   giochi-launcher --mostrar-volume
//!                                   imprime o volume atual do mixer e sai
//!   giochi-launcher --espiar-input [segundos]
//!                                   mostra cada tecla que chega do kernel
//!   giochi-launcher --checar-energia
//!                                   mostra o que o device tree declara sobre energia
//!
//! Para o aparelho (GLES2 sobre KMSDRM), defina `GIOCHI_GLES=1`. No PC deixe
//! vazio: forçar `opengles2` no desktop costuma falhar.

mod app;
mod backlight;
mod display;
mod draw;
mod icons;
mod input_evdev;
mod power;
mod system;
mod theme;
mod views;
mod volume;

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

    // Diagnóstico de campo: prova que o caminho do backlight funciona neste
    // painel, sem precisar passar pelo menu de energia. A tela pisca de
    // propósito — é o sinal de que escreveu no sysfs.
    if args.iter().any(|a| a == "--testar-backlight") {
        let mut bl = backlight::Backlight::discover();
        if !bl.disponivel() {
            eprintln!("nenhum /sys/class/backlight/*/brightness gravável");
            return Err("sem backlight".into());
        }
        println!("apagando o painel por 2 s...");
        bl.apagar();
        std::thread::sleep(Duration::from_secs(2));
        println!("restaurando...");
        bl.restaurar();
        println!("ok");
        return Ok(());
    }

    // Exercita exatamente a função que o slider usa, para provar que o caminho
    // chega ao painel — e que a escolha fica gravada para o `giochi-init`.
    if let Some(i) = args.iter().position(|a| a == "--brilho") {
        let pct: u8 = args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .ok_or("--brilho precisa de um valor de 0 a 100")?;
        let mut bl = backlight::Backlight::discover();
        if !bl.disponivel() {
            return Err("nenhum /sys/class/backlight/*/brightness gravável".into());
        }
        let antes = bl.fracao_atual();
        bl.definir_fracao(pct);
        println!(
            "brilho: {antes:?}% -> {:?}% (pedido {pct}%)",
            bl.fracao_atual()
        );
        return Ok(());
    }

    // Diagnóstico de volume: usa o mesmo `Volume` que a UI, então prova o
    // caminho inteiro (descoberta do elemento, faixa do hardware, persistência).
    if let Some(i) = args.iter().position(|a| a == "--volume") {
        let pct: u8 = args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .ok_or("--volume precisa de um valor de 0 a 100")?;
        let mut v = volume::Volume::novo(volume::abrir_mixer());
        if !v.disponivel() {
            return Err("nenhum mixer ALSA com volume de reprodução".into());
        }
        let antes = v.atual();
        v.definir(pct);
        println!("volume: {antes}% -> {}% (pedido {pct}%)", v.atual());
        return Ok(());
    }

    if args.iter().any(|a| a == "--mostrar-volume") {
        let v = volume::Volume::novo(volume::abrir_mixer());
        match v.disponivel() {
            true => println!("volume do mixer: {}%", v.atual()),
            false => println!("sem mixer com volume de reprodução"),
        }
        return Ok(());
    }

    // Espião de entrada: responde "o evento chega ao sistema?" separado de
    // "o app faz algo com ele?". Sem isso, botão mudo não é diagnosticável.
    if let Some(i) = args.iter().position(|a| a == "--espiar-input") {
        let segundos: u64 = args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(15);
        return espiar_input(segundos);
    }

    // Diagnóstico que NÃO desliga: mostra o que o device tree declara.
    //
    // Não é veredito sobre o `poweroff`: no X55 a propriedade não existe e o
    // aparelho desliga normalmente (medido). Serve para comparar aparelhos e
    // perceber mudança no device tree.
    if args.iter().any(|a| a == "--checar-energia") {
        let declarado = system::declaracao_de_energia();
        println!(
            "system-power-controller no device tree: {}",
            if declarado { "declarado" } else { "ausente" }
        );
        println!();
        println!("isto é informação, não veredito: no X55 a propriedade é ausente");
        println!("e `poweroff` desliga o aparelho de verdade (medido em 03/11/2026).");
        println!("para saber se desliga, o caminho é desligar.");
        return Ok(());
    }

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    // Tamanho do output. Existe para testar um painel que não temos em mãos:
    // com `SDL_VIDEODRIVER=dummy` e `--tamanho 640x480`, o `set_logical_size`
    // letterboxa exatamente como faria no X35H, e o quadro mostra o resultado.
    let (mut largura, mut altura) = (1280u32, 720u32);
    if let Some(i) = args.iter().position(|a| a == "--tamanho") {
        let pedido = args.get(i + 1).ok_or("--tamanho precisa de LARGURAxALTURA")?;
        let (l, a) = pedido
            .split_once(['x', 'X'])
            .ok_or("--tamanho espera LARGURAxALTURA, ex.: 640x480")?;
        largura = l.trim().parse().map_err(|_| format!("largura inválida: {l}"))?;
        altura = a.trim().parse().map_err(|_| format!("altura inválida: {a}"))?;
        if largura == 0 || altura == 0 {
            return Err("--tamanho não aceita zero".into());
        }
    }

    let mut builder = video.window("Giochi Launcher", largura, altura);
    if fullscreen {
        builder.fullscreen_desktop();
    }
    let window = builder
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| format!("janela: {e}"))?;

    let painter = draw::Painter::new(window, &font_path)?;
    // O projeto é sempre 1280x720; o painel só muda o encaixe. Registrar a
    // diferença evita confundir letterbox esperado com bug de desenho.
    if (largura, altura) != (draw::DESIGN_W as u32, draw::DESIGN_H as u32) {
        eprintln!(
            "output {largura}x{altura} com projeto {}x{} — letterbox esperado",
            draw::DESIGN_W,
            draw::DESIGN_H
        );
    }
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

/// Lê `/dev/input/event*` cru e imprime cada `EV_KEY`, com nome do dispositivo.
///
/// Não usa o mapa nem o `EvdevInput`: o objetivo é justamente ver o que o kernel
/// entrega, antes de qualquer tradução do app.
fn espiar_input(segundos: u64) -> Result<(), String> {
    use std::time::{Duration, Instant};

    let dir = std::path::Path::new("/dev/input");
    let entradas = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;

    let mut dispositivos = Vec::new();
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        let nome_arquivo = caminho.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !nome_arquivo.starts_with("event") {
            continue;
        }
        let nome = std::fs::read_to_string(
            std::path::Path::new("/sys/class/input")
                .join(nome_arquivo)
                .join("device/name"),
        )
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|_| "?".into());

        match std::fs::File::open(&caminho) {
            Ok(f) => dispositivos.push((caminho, nome, f)),
            Err(e) => eprintln!("nao consegui abrir {}: {e}", caminho.display()),
        }
    }

    if dispositivos.is_empty() {
        return Err("nenhum /dev/input/event* legível".into());
    }

    println!(
        "espiando {} dispositivo(s) por {segundos}s — pressione os botões agora",
        dispositivos.len()
    );
    for (caminho, nome, _) in &dispositivos {
        println!("  {} [{nome}]", caminho.display());
    }

    // `read` bloqueante numa thread por dispositivo, com timeout por quadro.
    let (tx, rx) = std::sync::mpsc::channel();
    for (_caminho, nome, arquivo) in dispositivos {
        let tx = tx.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut arquivo = arquivo;
            // 24 bytes = `struct input_event` em 64 bits (timeval 16 + 3 u16 + pad).
            let mut buf = [0u8; 24];
            loop {
                match arquivo.read_exact(&mut buf) {
                    Ok(()) => {
                        let tipo = u16::from_ne_bytes([buf[16], buf[17]]);
                        let codigo = u16::from_ne_bytes([buf[18], buf[19]]);
                        let valor = i32::from_ne_bytes([buf[20], buf[21], buf[22], buf[23]]);
                        // EV_KEY = 1. Ignora autorepeat (2) para não inundar.
                        if tipo == 1 && valor != 2 {
                            let _ = tx.send((nome.clone(), codigo, valor));
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let limite = Instant::now() + Duration::from_secs(segundos);
    let mut vistos = 0u32;
    while Instant::now() < limite {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((nome, codigo, valor)) => {
                let acao = if valor == 1 { "DOWN" } else { "UP  " };
                let apelido = nome_de_tecla(codigo);
                println!("{acao} codigo={codigo:<4} {apelido:<16} [{nome}]");
                vistos += 1;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }

    println!("--- {vistos} evento(s) ---");
    if vistos == 0 {
        println!("nada chegou: o problema é antes do app (driver, DTS ou botão)");
    }
    Ok(())
}

/// Nome legível das teclas que interessam ao diagnóstico.
fn nome_de_tecla(codigo: u16) -> &'static str {
    match codigo {
        114 => "KEY_VOLUMEDOWN",
        115 => "KEY_VOLUMEUP",
        116 => "KEY_POWER",
        102 => "KEY_HOME",
        0x220 => "BTN_DPAD_UP",
        0x221 => "BTN_DPAD_DOWN",
        0x222 => "BTN_DPAD_LEFT",
        0x223 => "BTN_DPAD_RIGHT",
        0x130 => "BTN_SOUTH(A)",
        0x131 => "BTN_EAST(B)",
        0x133 => "BTN_NORTH(X)",
        0x134 => "BTN_WEST(Y)",
        0x136 => "BTN_TL(L)",
        0x137 => "BTN_TR(R)",
        0x13a => "BTN_SELECT",
        0x13b => "BTN_START",
        0x13c => "BTN_MODE",
        _ => "?",
    }
}

fn default_font() -> String {
    // A lista vive em `draw::CANDIDATOS_FONTE` porque o aviso de fonte não
    // carregada precisa imprimi-la. Duas listas divergiriam na primeira
    // correção — e a divergência apareceria como "fonte não encontrada" com o
    // arquivo no lugar certo.
    for candidate in draw::CANDIDATOS_FONTE {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    // Caminho que provavelmente não existe: o desenho continua, só sem texto,
    // e o aviso aparece uma vez no console, com a lista dos caminhos tentados.
    draw::CANDIDATOS_FONTE[0].to_string()
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
