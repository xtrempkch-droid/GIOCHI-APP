//! Bootstrap: argumentos, engine, loop de timestep fixo de 60 Hz.
//!
//! # O loop
//!
//! A simulação roda em passos fixos de 1/60 s (`giochi_engine::GameLoop`), e o
//! desenho roda quantas vezes o aparelho conseguir. As três garantias que isso dá
//! e que o prompt exige (§9):
//!
//! - o mundo não muda de velocidade quando o FPS cai — num RK3326 a 30 FPS o jogo
//!   simula 60 Hz e desenha 30;
//! - `GameLoop::advance` limita a recuperação a 15 passos por quadro
//!   (`MAX_FRAME_TIME = 0.25 s`), então uma pausa longa do SO não faz o jogo
//!   "teleportar" para frente;
//! - **nada de rede toca este loop** (§10): o stub de rede do M2 é drenado dentro
//!   do passo, sem `recv` bloqueante.
//!
//! # Modo de verificação sem interação
//!
//! `--frames N --quadro arquivo.bmp` roda N quadros e grava o último, com passo
//! fixo (sem relógio real). É o que permite a bancada conferir a cena — a mesma
//! ideia do `--dump-frame` do launcher e do `--quadro` dos jogos do repositório.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use giochi_engine::core::game_loop::GameLoop;
use giochi_engine::core::time::FrameClock;
use giochi_engine::platform::{quit_signal, Platform, QuitSignal};
use giochi_engine_3d::math3d::Color3D;

use giochi_tower::config::{ClassId, EnemyKind, GameConfig, ProfileId};
use giochi_tower::engine_bridge::render::{AXIS_X_COLOR, AXIS_Z_COLOR, PLAYER_COLOR, TOWER_COLOR};
use giochi_tower::engine_bridge::{AudioBridge, InputBridge, RenderBridge};
use giochi_tower::errors::{GameError, Result};
use giochi_tower::game::state::{GameState, Phase};
use giochi_tower::net::GameMsg;
use giochi_tower::ui::{self, layout};
use giochi_tower::{BUILD_PROFILE, BUILD_TARGET, CONFIG_DIR, CONFIG_DIR_ENV, TITLE, VERSION};

/// Tamanho de janela padrão quando não há `--tamanho`.
const DEFAULT_WIDTH: u32 = 1280;

/// Altura padrão quando não há `--tamanho`.
const DEFAULT_HEIGHT: u32 = 720;

/// Intervalo entre quadros no caminho normal, em milissegundos.
const FRAME_MS: u32 = 1000 / 60;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // A mensagem diz de quem é o problema: config, engine ou render.
            eprintln!("giochi-tower: erro: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Opções de linha de comando.
#[derive(Debug, Clone)]
struct Options {
    width: u32,
    height: u32,
    config_dir: PathBuf,
    class: ClassId,
    profile: ProfileId,
    capture: Option<PathBuf>,
    frames: Option<u64>,
    /// Desenhar o HUD e a loja. Desligado com `--sem-ui`, para medir o custo das
    /// duas camadas separadamente.
    ui: bool,
    /// Abre a loja já na abertura. Existe porque a loja só aparece 6 minutos
    /// depois do início (e a partida costuma terminar antes disso sem alguém
    /// atirando), então capturá-la exigiria jogar — e verificação que depende de
    /// dedo não entra em rodada automatizada.
    demo_loja: bool,
    /// Roda N quadros **sem espera** e reporta FPS. É o que separa "abriu" de
    /// "roda a 40 fps": com o teto de 16 ms do caminho normal todo jogo mede 60.
    benchmark: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            config_dir: PathBuf::from(CONFIG_DIR),
            class: ClassId::Soldier,
            profile: ProfileId::Balanced,
            capture: None,
            frames: None,
            ui: true,
            demo_loja: false,
            benchmark: None,
        }
    }
}

const USAGE: &str = "\
GIOCHI Tower Defense — Tower Defense 3D co-op PvE (GDD: 6 ondas, 4 bosses, nemesis)

USO:
  giochi-tower [opções]

OPÇÕES:
  --config <dir>       diretório dos .ron (padrão: config, ou $GIOCHI_TOWER_CONFIG)
  --tamanho <LxA>      tamanho da janela, ex.: 1280x720 (no KMSDRM o painel manda)
  --classe <nome>      soldado | guerreiro | sniper | medico
  --perfil <nome>      full | balanced | rk3326
  --quadro <arquivo>   grava o último quadro em BMP de 24 bits
  --frames <n>         roda só n quadros, com passo fixo (verificação)
                       (aceita também --quadros, o nome usado no resto do repo)
  --sem-ui             não desenha HUD, minimapa nem loja (mede o custo da UI)
  --demo-loja          abre a loja na abertura (para capturar a tela)
  --benchmark <n>      roda n quadros sem espera e reporta FPS (combine com --sem-ui)
  --ajuda              esta mensagem

CONTROLES (gamepad tem prioridade sobre teclado):
  analógico esq. / WASD     mover
  analógico dir. / setas    mirar
  A / espaço                atacar
  B / Q                     habilidade 1
  X / E                     habilidade 2
  Y / F                     item rápido
  D-pad ↑ / Tab             abrir loja (no intervalo)
  Start / Esc               pausar
";

fn run() -> Result<()> {
    let options = parse_args(std::env::args().skip(1))?;

    let config = GameConfig::load(&options.config_dir)?;
    log::info!(
        "config: {} classes, {} inimigos, {} ondas, {} bosses, {} tiers de nemesis, {} itens",
        config.classes.classes.len(),
        config.enemies.enemies.len(),
        config.campaign.waves.len(),
        config.bosses.bosses.len(),
        config.nemesis.tiers.len(),
        config.items.items.len()
    );

    let platform = Platform::new()?;
    let mut render = RenderBridge::new(&platform, options.width, options.height, TITLE)?;
    let mut input = InputBridge::new(&platform);
    let mut audio = AudioBridge::new(&platform);
    let mut state = GameState::new(&config, options.class, options.profile);
    if options.demo_loja {
        // Loja aberta e gold inicial para o menu ter itens compráveis e outros
        // não: uma tela só com "sem gold" não mostra o marcador de escolha.
        state.phase = Phase::Shop;
        state.local_mut().gold = 120;
        log::info!("demo: loja aberta na abertura (--demo-loja)");
    }

    banner(&render, &input, &config, &state, options.frames.is_some());

    let mut game_loop = GameLoop::new();
    let mut clock = FrameClock::new();
    let fixed_step = game_loop.fixed_timestep();
    let deterministic = options.frames.is_some() || options.benchmark.is_some();
    // No benchmark o laço não dorme: o intervalo medido é o do quadro, não o do
    // quadro mais uma espera escolhida por nós.
    let sem_espera = options.benchmark.is_some();
    let inicio_benchmark = Instant::now();
    let mut cpu_total = std::time::Duration::ZERO;
    // O custo da UI é dividido em duas metades que se otimizam de formas
    // opostas: montar o plano é CPU pura (Strings, `format!`), desenhar é
    // submissão de GPU. Somadas elas não dizem qual das duas atacar.
    let mut cpu_ui = std::time::Duration::ZERO;
    let mut cpu_ui_desenho = std::time::Duration::ZERO;

    // Sessão de rede local (M2). O stub carrega o mesmo input do jogador local e
    // devolve o snapshot já filtrado por AOI — é o dado que vai para a tela, então
    // um erro de transporte aparece como inimigo faltando, não como silêncio.
    #[cfg(feature = "stub-net")]
    let mut session = giochi_tower::net::local_session();
    #[cfg(feature = "stub-net")]
    let mut net_inputs: u64 = 0;

    let mut frames: u64 = 0;
    let mut announced_source = false;
    // Último HUD montado, para o log de encerramento: é o que prova que a
    // interface foi decidida, e não só desenhada.
    let mut last_ui: Option<giochi_tower::ui::Hud> = None;
    loop {
        let started = Instant::now();

        // --- eventos do SO ----------------------------------------------
        let events = platform.poll_events();
        let mut should_quit = false;
        for event in &events {
            if quit_signal(event) != QuitSignal::None {
                should_quit = true;
            }
        }
        if should_quit {
            log::info!("janela fechada");
            break;
        }

        // --- entrada ----------------------------------------------------
        let (drawable_w, drawable_h) = render.drawable_size();
        let frame_input = input.update(&events, (drawable_w as f32, drawable_h as f32));
        if !announced_source && frame_input.source != giochi_tower::engine_bridge::InputSource::None {
            log::info!(
                "entrada: {:?}{}",
                frame_input.source,
                input
                    .pad_name()
                    .map(|name| format!(" — gamepad '{name}'"))
                    .unwrap_or_default()
            );
            announced_source = true;
        }

        // --- simulação: passos fixos, nunca ditados pelo framerate -------
        let delta = if deterministic {
            fixed_step
        } else {
            clock.tick()
        };
        let steps = game_loop.advance(delta);
        for _ in 0..steps {
            state.step(fixed_step, &frame_input, &config);
        }

        // --- rede: input sobe, snapshot desce filtrado por AOI -----------
        let mut visible = state.snapshot_enemies();
        #[cfg(feature = "stub-net")]
        if let Some((host, client)) = session.as_mut() {
            // Os dois traits, e não só o do cliente: `poll_input`,
            // `publish_snapshot` e `broadcast` são do `HostBackend`. Faltando
            // ele no escopo, o erro é `no method named poll_input found`, que
            // manda procurar defeito no `StubHost` em vez de no `use`.
            use giochi_tower::net::{ClientBackend, HostBackend};
            let camera = [state.camera.focus.x, state.camera.focus.z];
            client.set_camera_pos(camera);
            client.send_input(GameMsg::input(
                state.tick,
                client.player_id(),
                &frame_input,
                camera,
            ));
            net_inputs += host.poll_input(16).len() as u64;
            host.publish_snapshot(&state.snapshot_enemies());
            host.broadcast(GameMsg::TowerState {
                hp: state.tower.hp.max(0.0) as u16,
                max_hp: state.tower.max_hp as u16,
                armor: state.tower.armor as u16,
            });
            let _ = client.poll(64);
            visible = client.take_snapshot().unwrap_or_default();
        }

        if state.phase.is_over() {
            log::info!("fim: {}", state.summary());
            break;
        }

        // --- desenho ----------------------------------------------------
        let camera = state.camera.engine_camera(drawable_w, drawable_h);
        render.begin();
        render.draw_ground(&camera)?;

        // Torre provisória no centro do mapa (o alvo a defender, GDD §11).
        render.draw_cube(&camera, glam::Vec3::new(0.0, 3.0, 0.0), 6.0, TOWER_COLOR)?;

        // Jogador: um cubo, e um segundo cubo à frente marcando a mira.
        let player = state.local().position;
        render.draw_cube(&camera, glam::Vec3::new(player.x, 0.9, player.z), 1.6, PLAYER_COLOR)?;
        let aim = state.local().aim;
        render.draw_cube(
            &camera,
            glam::Vec3::new(player.x + aim.x * 2.4, 0.45, player.z + aim.z * 2.4),
            0.5,
            AXIS_Z_COLOR,
        )?;

        // Dois marcadores de orientação no chão: sem eles a câmera isométrica não
        // tem como provar que está girada 45° e não 0°.
        render.draw_cube(&camera, glam::Vec3::new(12.0, 0.2, 0.0), 0.4, AXIS_X_COLOR)?;
        render.draw_cube(&camera, glam::Vec3::new(0.0, 0.2, 12.0), 0.4, AXIS_Z_COLOR)?;

        // Inimigos: desenhados do **snapshot**, não do mundo do host. Se a AOI
        // filtrar demais, a tela mostra menos inimigos — que é a falha que se
        // quer ver acontecendo, e não um silêncio.
        for enemy in &visible {
            // `tipo` 9 é boss e 10 é o Coronel; o resto é inimigo comum.
            let (height, size, color) = match enemy.tipo {
                9 => (3.5, 4.5, Color3D::new(0.95, 0.15, 0.35, 1.0)),
                10 => (2.5, 3.0, Color3D::new(0.85, 0.15, 0.85, 1.0)),
                _ => {
                    let kind = GameState::enemy_kind_of_byte(enemy.tipo);
                    let (height, size) = match kind {
                        EnemyKind::Flying => (2.6, 1.3),
                        EnemyKind::Tank => (1.1, 2.2),
                        EnemyKind::Swarm => (0.6, 0.9),
                        EnemyKind::Ranged => (0.9, 1.4),
                        EnemyKind::Grunt => (0.9, 1.5),
                    };
                    (height, size, enemy_color(kind))
                }
            };
            render.draw_cube(
                &camera,
                glam::Vec3::new(enemy.x, height, enemy.z),
                size,
                color,
            )?;
        }

        // --- UI: HUD, minimapa e loja -----------------------------------
        //
        // Montada a partir do estado **deste** quadro e desenhada por cima do
        // mundo. As duas camadas são separadas de propósito: `ui::build` produz
        // números, `layout::montar` produz pixels e só `draw_ui` toca o GL — o
        // que permite testar "a loja cabe em 640x480?" sem abrir janela.
        if options.ui {
            let antes = Instant::now();
            let frame_ui = ui::build(&state, &config);
            let plano = layout::montar(&frame_ui, layout::Tela::new(drawable_w, drawable_h));
            if sem_espera {
                cpu_ui += antes.elapsed();
            }
            let antes = Instant::now();
            render.draw_ui(&plano)?;
            if sem_espera {
                cpu_ui_desenho += antes.elapsed();
            }
            last_ui = Some(frame_ui.hud.clone());
        }

        // Trabalho de submissão: do começo do quadro até antes da troca de
        // buffers. A troca é onde o KMSDRM espera o vblank, e incluí-la aqui
        // faria todo jogo medir exatamente o intervalo do painel — foi o que
        // aconteceu na primeira versão deste número (16,72 ms "de trabalho" com
        // e sem UI, ou seja, medindo o vblank).
        let trabalho = started.elapsed();
        if sem_espera {
            cpu_total += trabalho;
        }

        render.end()?;

        frames += 1;
        let ultimo_quadro = options.frames.is_some_and(|limite| frames >= limite);
        // A captura é do **último** quadro, não de todos: gravar dentro do laço
        // escrevia o BMP 60 vezes em `--frames 60` e enchia o log com 60 linhas
        // idênticas, escondendo o resto do diagnóstico.
        if ultimo_quadro {
            if let Some(path) = options.capture.as_deref() {
                render.capture_bmp(path)?;
            }
        }
        if ultimo_quadro {
            break;
        }

        // --- limite de quadros (a engine não limita sozinha) -------------
        if !sem_espera {
            let spent = started.elapsed().as_millis() as u32;
            if spent < FRAME_MS {
                platform.delay_ms(FRAME_MS - spent);
            }
        }

        if options.benchmark.is_some_and(|limite| frames >= limite) {
            break;
        }
    }

    if let Some(limite) = options.benchmark {
        let total = inicio_benchmark.elapsed().as_secs_f64();
        let fps = if total > 0.0 { frames as f64 / total } else { 0.0 };
        let por_quadro = |d: std::time::Duration| {
            if frames > 0 {
                d.as_secs_f64() * 1000.0 / frames as f64
            } else {
                0.0
            }
        };
        let cpu_ms = por_quadro(cpu_total);
        let montar_ms = por_quadro(cpu_ui);
        let desenho_ms = por_quadro(cpu_ui_desenho);
        log::info!(
            "benchmark: {frames}/{limite} quadro(s) em {total:.2} s = {fps:.1} fps · trabalho por quadro: {cpu_ms:.2} ms · ui: {} lote(s) → {} chamada(s) reais, {} instância(s) · montar {montar_ms:.2} ms + desenhar {desenho_ms:.2} ms",
            render.hud_draw_calls(),
            render.hud_gl_draws(),
            render.hud_instances()
        );
    }

    log::info!(
        "encerrando: {frames} quadro(s), {} passo(s) de simulação, onda {} ({})",
        state.tick,
        state.wave,
        state.phase.as_str()
    );
    log::info!("resultado: {}", state.summary());
    #[cfg(feature = "stub-net")]
    log::info!("rede: {net_inputs} input(s) entregues ao host pelo stub");
    if state.phase == Phase::Victory {
        log::info!("campanha vencida — {}", state.hud_line(&config));
    }

    // O que a interface estava mostrando no último quadro. Sem isto, "o HUD
    // desenhou" e "o HUD desenhou o número certo" ficam iguais no log.
    if let Some(hud) = &last_ui {
        for linha in hud.lines() {
            log::info!("hud: {linha}");
        }
    }
    if options.ui {
        log::info!(
            "ui: {} chamada(s) de desenho, {} instância(s)",
            render.hud_draw_calls(),
            render.hud_instances()
        );
    }

    audio.stop_all();
    render.destroy_all();
    Ok(())
}

/// Cor de cada tipo de inimigo.
///
/// A silhueta já diferencia pelo tamanho; a cor é a segunda pista, e é o que o
/// GDD §18 pede ("destaques em vermelho para inimigos").
fn enemy_color(kind: EnemyKind) -> Color3D {
    match kind {
        EnemyKind::Grunt => Color3D::new(0.86, 0.27, 0.22, 1.0),
        EnemyKind::Tank => Color3D::new(0.55, 0.14, 0.14, 1.0),
        EnemyKind::Ranged => Color3D::new(0.92, 0.48, 0.12, 1.0),
        EnemyKind::Swarm => Color3D::new(0.96, 0.72, 0.22, 1.0),
        EnemyKind::Flying => Color3D::new(0.70, 0.32, 0.78, 1.0),
    }
}

/// Imprime o que está de fato aberto, não o que foi pedido.
///
/// A lição de `docs/ferramentas.md`: pedir um backend não prova que ele abriu —
/// `SDL_RENDER_DRIVER` inválido cai em silêncio para outro. Aqui, driver, versão
/// de GLES e adaptador vêm do objeto, e o pedido aparece separado do obtido.
fn banner(
    render: &RenderBridge,
    input: &InputBridge,
    config: &GameConfig,
    state: &GameState,
    deterministic: bool,
) {
    log::info!("GIOCHI Tower Defense {VERSION} ({BUILD_PROFILE}, {BUILD_TARGET})");
    log::info!(
        "vídeo: driver '{}', GLES {} obtido (pedido {}), adaptador '{}'",
        render.driver(),
        render.gles_major(),
        render.requested_gles().major(),
        render.adapter()
    );
    log::info!(
        "drawable: {}x{}",
        render.drawable_size().0,
        render.drawable_size().1
    );
    log::info!(
        "gamepad: {}",
        input.pad_name().unwrap_or("nenhum (teclado/mouse ou touch emulado)")
    );
    log::info!(
        "perfil de host: {} — teto de {} inimigos, IA {} Hz, AOI ×{}",
        state.profile.as_str(),
        config
            .profile(state.profile)
            .map(|p| p.max_enemies)
            .unwrap_or(0),
        config.profile(state.profile).map(|p| p.ai_hz).unwrap_or(0),
        config
            .profile(state.profile)
            .map(|p| p.aoi_scale)
            .unwrap_or(0.0)
    );
    log::info!(
        "modo: {} — {} onda(s) de {} min + {} min de loja",
        state.mode.as_str(),
        config.campaign.wave_count,
        config.campaign.combat_s as u32 / 60,
        config.campaign.shop_s as u32 / 60
    );
    log::info!(
        "sala vazia (M1): câmera isométrica e jogador {}. Inimigos, torre e loja chegam em M2/M3.",
        state.local().class.as_str()
    );
    if deterministic {
        log::info!("modo de verificação: passo fixo, sem relógio real");
    }
}

/// Onde procurar os `.ron`, do mais específico para o mais genérico.
///
/// O padrão era `config` relativo ao **diretório de execução**, e isso quebra
/// no aparelho de duas maneiras: o launcher inicia o app com `cwd` na pasta do
/// app (funciona), mas rodar à mão de dentro de `bin/` não acha nada — e o
/// sintoma no aparelho foi `não consegui ler config/classes.ron`, que parece
/// instalação incompleta e é resolução de caminho.
///
/// A ordem cobre os três casos reais: instalado (`<app>/config`), rodando do
/// `bin/` (`<app>/config` via `..`), e a árvore do repositório no PC (onde o
/// `CARGO_MANIFEST_DIR` é o único caminho que existe sempre).
fn candidatos_de_config() -> Vec<PathBuf> {
    let mut lista = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // `<app>/bin/giochi-tower` → `<app>/config`
            lista.push(dir.join("..").join(CONFIG_DIR));
            lista.push(dir.join(CONFIG_DIR));
        }
    }
    lista.push(PathBuf::from(CONFIG_DIR));
    lista.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(CONFIG_DIR));
    lista
}

/// Primeiro candidato que tem os `.ron` de fato.
///
/// Testa a presença de um arquivo em vez de a pasta existir: um diretório
/// `config/` vazio passaria e o erro apareceria lá na frente, na leitura do
/// primeiro `.ron`.
fn resolver_config_dir() -> PathBuf {
    for candidato in candidatos_de_config() {
        if candidato.join("classes.ron").is_file() {
            return candidato;
        }
    }
    // Nenhum serve: devolve o padrão e deixa `GameConfig::load` dar a mensagem
    // com o caminho — adivinhar aqui esconderia qual foi tentado.
    PathBuf::from(CONFIG_DIR)
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options> {
    let mut options = Options {
        config_dir: resolver_config_dir(),
        ..Options::default()
    };
    if let Ok(dir) = std::env::var(CONFIG_DIR_ENV) {
        if !dir.is_empty() {
            options.config_dir = PathBuf::from(dir);
        }
    }

    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ajuda" | "--help" | "-h" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--config precisa de um diretório"))?;
                options.config_dir = PathBuf::from(value);
            }
            "--tamanho" | "--size" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--tamanho precisa de LxA, ex.: 1280x720"))?;
                let (width, height) = parse_size(&value)?;
                options.width = width;
                options.height = height;
            }
            "--classe" | "--class" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--classe precisa de um nome"))?;
                options.class = ClassId::from_arg(&value).ok_or_else(|| {
                    GameError::message(format!(
                        "classe '{value}' desconhecida (soldado, guerreiro, sniper, medico)"
                    ))
                })?;
            }
            "--perfil" | "--profile" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--perfil precisa de um nome"))?;
                options.profile = ProfileId::from_arg(&value).ok_or_else(|| {
                    GameError::message(format!(
                        "perfil '{value}' desconhecido (full, balanced, rk3326)"
                    ))
                })?;
            }
            "--quadro" | "--frame" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--quadro precisa de um caminho"))?;
                options.capture = Some(PathBuf::from(value));
            }
            // `--quadros` é o nome que o resto do repositório usa (cobrinha,
            // arcade-volleyball) e o que `aparelho.sh verificar-jogo` passa.
            // Aceitar os dois evita "argumento desconhecido" na ferramenta que
            // existe justamente para provar que o jogo desenha no aparelho.
            "--sem-ui" => options.ui = false,
            "--demo-loja" => options.demo_loja = true,
            "--benchmark" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--benchmark precisa de um número"))?;
                let quadros = value.parse::<u64>().map_err(|_| {
                    GameError::message(format!("--benchmark inválido: '{value}'"))
                })?;
                if quadros == 0 {
                    return Err(GameError::message("--benchmark 0 não mede nada"));
                }
                options.benchmark = Some(quadros);
            }
            "--frames" | "--quadros" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--frames/--quadros precisa de um número"))?;
                let frames = value.parse::<u64>().map_err(|_| {
                    GameError::message(format!("--frames/--quadros inválido: '{value}'"))
                })?;
                if frames == 0 {
                    return Err(GameError::message("--frames 0 não roda nada"));
                }
                options.frames = Some(frames);
            }
            other => {
                return Err(GameError::message(format!(
                    "argumento desconhecido: '{other}' (use --ajuda)"
                )));
            }
        }
    }

    Ok(options)
}

fn parse_size(value: &str) -> Result<(u32, u32)> {
    let (width, height) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| GameError::message(format!("tamanho inválido: '{value}' (esperava LxA)")))?;
    let width = width
        .trim()
        .parse::<u32>()
        .map_err(|_| GameError::message(format!("largura inválida em '{value}'")))?;
    let height = height
        .trim()
        .parse::<u32>()
        .map_err(|_| GameError::message(format!("altura inválida em '{value}'")))?;
    if width == 0 || height == 0 {
        return Err(GameError::message("largura e altura precisam ser ≥ 1"));
    }
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Options> {
        parse_args(values.iter().map(|value| value.to_string()))
    }

    #[test]
    fn the_defaults_are_a_720p_window_with_the_balanced_profile() {
        let options = args(&[]).expect("sem argumentos é válido");
        assert_eq!((options.width, options.height), (1280, 720));
        assert_eq!(options.class, ClassId::Soldier);
        assert_eq!(options.profile, ProfileId::Balanced);
        assert!(options.capture.is_none() && options.frames.is_none());
    }

    #[test]
    fn the_size_flag_accepts_both_cases_of_the_separator() {
        assert_eq!(args(&["--tamanho", "640x480"]).unwrap().width, 640);
        assert_eq!(args(&["--tamanho", "800X600"]).unwrap().height, 600);
    }

    #[test]
    fn a_bad_size_is_refused_with_the_offending_value() {
        let error = args(&["--tamanho", "1280"]).unwrap_err().to_string();
        assert!(error.contains("1280"), "{error}");
        assert!(args(&["--tamanho", "0x720"]).is_err());
    }

    #[test]
    fn classes_and_profiles_are_accepted_in_portuguese_and_english() {
        assert_eq!(args(&["--classe", "guerreiro"]).unwrap().class, ClassId::Warrior);
        assert_eq!(args(&["--classe", "medic"]).unwrap().class, ClassId::Medic);
        assert_eq!(args(&["--perfil", "rk3326"]).unwrap().profile, ProfileId::Rk3326);
        assert!(args(&["--classe", "ninja"]).is_err());
    }

    #[test]
    fn an_unknown_argument_fails_instead_of_being_ignored() {
        assert!(args(&["--turbinar"]).is_err());
    }

    #[test]
    fn frames_zero_is_refused() {
        assert!(args(&["--frames", "0"]).is_err());
        assert_eq!(args(&["--frames", "3"]).unwrap().frames, Some(3));
    }
}
