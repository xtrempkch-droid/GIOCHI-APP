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

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use giochi_engine::core::game_loop::GameLoop;
use giochi_engine::core::time::FrameClock;
use giochi_engine::platform::{quit_signal, Platform, QuitSignal};
use giochi_engine_3d::math3d::Color3D;

use giochi_tower::config::{ClassId, GameConfig, ProfileId};
use giochi_tower::engine_bridge::cena;
use giochi_tower::engine_bridge::render::{
    AIM_COLOR, LINHA_ALFA, PLAYER_COLOR, TARGET_COLOR,
};
use giochi_tower::engine_bridge::{AudioBridge, InputBridge, InputFrame, RenderBridge};
use giochi_tower::errors::{GameError, Result};
use giochi_tower::game::cenario;
use giochi_tower::game::state::{GameState, Phase};
use giochi_tower::menu::{self, Menu};
use giochi_tower::net::GameMsg;
use giochi_tower::ui::{self, layout};
use giochi_tower::{BUILD_PROFILE, BUILD_TARGET, CONFIG_DIR, CONFIG_DIR_ENV, TITLE, VERSION};

/// Tamanho de janela padrão quando não há `--tamanho`.
const DEFAULT_WIDTH: u32 = 1280;

/// Altura padrão quando não há `--tamanho`.
const DEFAULT_HEIGHT: u32 = 720;

/// Intervalo entre quadros no caminho normal, em milissegundos.
const FRAME_MS: u32 = 1000 / 60;

/// Porta UDP padrão do hospedeiro (a mesma da descoberta na rede local).
const PORTA_PADRAO: u16 = 7777;

/// Onde o traço de mira começa, em metros à frente do soldado.
///
/// Não em cima dele: o modelo ocupa ~1 m e o traço saía metade escondido atrás
/// do corpo e da base da torre. Medido em 640×480, dos ~400 px que o traço
/// deveria pintar só ~110 apareciam.
const MIRA_INICIO_M: f32 = 1.2;

/// Comprimento mínimo do traço de mira, em metros.
///
/// O traço normal vai até o **alcance da classe** (`attack_range`): ele é uma
/// régua, não um enfeite. Foi o que respondeu à segunda metade do relato — o
/// jogador via o bug a 15 m, atirava, e nada acontecia, porque o Soldado alcança
/// 14 m e não havia nada na tela dizendo isso. Agora o inimigo fora do traço
/// está fora de alcance, e a linha vermelha só aparece quando ele entra.
const MIRA_CURTA_MIN_M: f32 = 3.0;

/// Lado do losango na ponta do traço, em metros.
///
/// Uma ponta é o que transforma um risco em vetor. Sem ela o traço some entre as
/// pedras do cenário e o jogador não sabe onde ele termina.
const MIRA_PONTA_M: f32 = 0.8;

/// Largura do traço de mira e da linha do alvo, em metros.
///
/// A 52 m da câmera e 32° de FOV vertical, 1 m ≈ 16 px em 640×480; 0,30 m dá
/// ~5 px, que é a espessura em que uma linha de mira lê como linha e não como
/// risco de formiga. Abaixo de 0,2 m ela desaparece no quadro do aparelho.
const MIRA_LARGURA_M: f32 = 0.30;

/// Raio do retículo, em múltiplos da altura do inimigo.
///
/// 1,05 dá um diâmetro de ~2× a altura: fecha em volta do corpo inteiro e ainda
/// deixa ver o próximo bug da fila, que é quem o jogador quer escolher depois.
/// Abaixo de 0,8 o anel encosta no corpo e as quatro aberturas somem no meio da
/// silhueta — medido em 640×480, onde o retículo tem ~36 px.
const RAIO_DO_ALVO: f32 = 1.05;

/// Modo do laço: menu na frente, ou mundo simulado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// O menu está na frente; o mundo, se houver, fica congelado atrás.
    Menu,
    /// Jogando (o menu de pausa é `Modo::Menu` com `Tela::Pausa`).
    Jogando,
}

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
    /// Desenhar o cenário (pedras, crateras, mesas, destroços). Desligado com
    /// `--sem-cenario`: é o A/B que responde "quanto custa a paisagem" sem
    /// comparar com uma versão antiga do binário.
    cenario: bool,
    /// Abre a loja já na abertura. Existe porque a loja só aparece 6 minutos
    /// depois do início (e a partida costuma terminar antes disso sem alguém
    /// atirando), então capturá-la exigiria jogar — e verificação que depende de
    /// dedo não entra em rodada automatizada.
    demo_loja: bool,
    /// Roda N quadros **sem espera** e reporta FPS. É o que separa "abriu" de
    /// "roda a 40 fps": com o teto de 16 ms do caminho normal todo jogo mede 60.
    benchmark: Option<u64>,
    /// Começar no menu principal em vez de direto na partida.
    ///
    /// Ligado por padrão quando **nada** deterministico foi pedido. `--frames` e
    /// `--benchmark` continuam entrando direto na partida, senão toda a bancada
    /// (e o CI) mediria a tela de menu.
    menu: bool,
    /// Hospeda uma partida em rede na porta dada.
    hospedar: Option<u16>,
    /// Entra numa partida no endereço dado (`ip:porta`).
    entrar: Option<String>,
    /// Vitrine de modelos: desenha os 20 arquivos lado a lado, sem mundo, sem
    /// HUD e com câmera fixa.
    ///
    /// Existe porque "o modelo carregou" e "o modelo parece um soldado" são
    /// afirmações diferentes, e a segunda não tinha ferramenta: para ver um bug
    /// seria preciso jogar até a onda dele, e o Coronel só aparece no meio da
    /// horda. Aqui os vinte cabem num quadro.
    vitrine: bool,
    /// Posição inicial do jogador local, em metros (`--posicionar x,z`).
    ///
    /// Ferramenta de bancada, como o `--em` do `neon-drift`: sem ela, capturar um
    /// quadro **com alvo marcado** exigiria jogar até a horda encostar — e os
    /// inimigos de longo alcance param a 17 m da torre, fora dos 14 m do Soldado.
    /// A posição vale só no primeiro quadro: o jogador anda a partir dali.
    posicionar: Option<(f32, f32)>,
    /// Desenhar a mira (traço de alcance, linha do alvo e retículo).
    ///
    /// Desligado com `--sem-mira`, pela mesma razão de `--sem-cenario`: é o A/B
    /// que responde "quanto custa" e que permite mostrar o antes e o depois no
    /// mesmo comando, sem guardar um binário antigo.
    mira: bool,
    /// Ignorar controle, teclado e mouse: só `--teclas` move alguma coisa.
    ///
    /// No aparelho há **controle de verdade conectado**, e o analógico parado
    /// ainda emite ruído. Esse ruído muda o `aim` do soldado e, com ele, o alvo
    /// da mira — então o mesmo `--frames` capturado no PC e no aparelho diverge
    /// por inteiro (medido: 273 mil pixels de diferença, 68 % do quadro) e a
    /// comparação de quadro deixa de significar qualquer coisa.
    ///
    /// Desligado por padrão: quem joga quer o controle.
    sem_entrada: bool,
    /// Sequência de teclas aplicada **antes** do primeiro desenho.
    ///
    /// Existe pela mesma razão do `--teclas` do launcher e do `neon-drift`:
    /// verificação que depende de dedo não entra em rodada automatizada. Cada
    /// tecla é entregue ao menu pelo caminho de produção (`Menu::entrada`), não
    /// por um atalho que mexa no cursor diretamente.
    teclas: Vec<String>,
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
            cenario: true,
            demo_loja: false,
            benchmark: None,
            menu: false,
            hospedar: None,
            entrar: None,
            vitrine: false,
            posicionar: None,
            mira: true,
            sem_entrada: false,
            teclas: Vec::new(),
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
  --sem-cenario        não desenha pedras, crateras, mesas nem destroços
  --vitrine            desenha os 20 modelos lado a lado (confere a arte num quadro)
  --demo-loja          abre a loja na abertura (para capturar a tela)
  --benchmark <n>      roda n quadros sem espera e reporta FPS (combine com --sem-ui)
  --menu               abre no menu principal (o padrão é entrar direto na partida)
  --hospedar [porta]   hospeda uma partida em rede (padrão 7777)
  --entrar <ip:porta>  entra numa partida em rede
  --posicionar <x,z>   começa com o soldado em x,z metros (ferramenta de captura)
  --sem-mira           não desenha traço de alcance, linha do alvo nem retículo
  --sem-entrada        ignora controle/teclado/mouse (captura determinística)
  --teclas <lista>     aplica teclas antes do primeiro quadro, separadas por vírgula
                       (cima, baixo, esq, dir, a, b) — para verificar o menu sem dedo
  --ajuda              esta mensagem

MENU (gamepad ou teclado):
  cima/baixo           escolhe a linha
  esquerda/direita     ajusta volume, câmera e FPS
  A / espaço           confirma
  B / Q                volta um nível (nunca encerra o jogo)
  Start / Esc          pausa durante a partida

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
    if let Some((x, z)) = options.posicionar {
        let mapa = state.map;
        state.local_mut().position = mapa.clamp(glam::Vec3::new(x, 0.0, z));
        state.camera.focus = state.local_mut().position;
        log::info!(
            "bancada: soldado posicionado em ({:.1}, {:.1})",
            state.local().position.x,
            state.local().position.z
        );
    }

    banner(&render, &input, &config, &state, options.frames.is_some());

    // ## Vitrine de modelos
    //
    // `--vitrine` não é um modo de jogo: desenha os 20 modelos em duas fileiras,
    // grava o quadro e sai. Roda **antes** do laço para não precisar de estado de
    // partida nem de menu, e para o tempo de vida do BMP ser o de um quadro só.
    if options.vitrine {
        render.begin();
        vitrine(&mut render)?;
        // A captura lê os pixels **antes** da troca de buffers: com GLES o swap
        // esvazia o back buffer e o BMP sai preto (medido nos três aparelhos).
        if let Some(caminho) = options.capture.as_ref() {
            render.capture_bmp(caminho)?;
        }
        render.end()?;
        log::info!(
            "vitrine: {} modelo(s) desenhado(s) — duas fileiras de personagens e uma de cenário",
            giochi_tower::engine_bridge::modelos::todos_os_nomes().count()
        );
        return Ok(());
    }

    // O cenário é montado **uma vez**: a paisagem não muda durante a partida, e o
    // `RenderBridge` já a fundiu em uma malha por modelo na abertura.
    let cenario_copias = cenario::montar();
    let contar = |nome: &str| cenario_copias.iter().filter(|c| c.modelo == nome).count();
    log::info!(
        "cenário: {} peça(s) em {} malha(s) — {} rocha(s), {} espigão(ões), {} cratera(s), {} mesa(s), {} destroço(s)",
        cenario_copias.len(),
        render.cenario_malhas(),
        contar("rocha"),
        contar("espigao"),
        contar("cratera"),
        contar("mesa"),
        contar("destroco"),
    );

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
    // A horda é o custo que cresce com o número de inimigos: medido à parte, ele
    // responde "quantos inimigos este aparelho aguenta" sem misturar com o HUD.
    let mut cpu_horda = std::time::Duration::ZERO;
    let mut chamadas_horda: u64 = 0;

    // --- menu e configurações --------------------------------------
    //
    // As configurações vivem num arquivo ao lado dos `.ron`, e o menu é o editor
    // delas. Ler antes de desenhar o primeiro quadro é o que faz o volume
    // escolhido na sessão anterior valer já na abertura.
    let caminho_settings = options.config_dir.join("settings.json");
    let mut config_jogo = carregar_config(&caminho_settings);
    log::info!(
        "configurações: volume {}%, câmera {:.0} m, fps {}",
        config_jogo.volume,
        config_jogo.camera_dist_m,
        if config_jogo.mostrar_fps {
            "sim"
        } else {
            "não"
        }
    );
    audio.definir_volume(config_jogo.volume_fracao());
    state.camera.distance_m = config_jogo.camera_dist_m;

    // A bancada (`--frames`, `--benchmark`, `--demo-loja`, rede) entra direto na
    // partida: um teste que abrisse no menu mediria a tela de menu.
    let comeca_no_menu = options.menu
        || (!deterministic
            && !options.demo_loja
            && options.hospedar.is_none()
            && options.entrar.is_none());
    let mut menu = Menu::novo(false);
    let mut modo = if comeca_no_menu {
        Modo::Menu
    } else {
        Modo::Jogando
    };
    menu.config = config_jogo;
    if !comeca_no_menu {
        menu.marcar_partida(true);
    }

    // --- rede ------------------------------------------------------
    //
    // Uma sessão por vez, e o papel decide o que fazer com ela: o host simula e
    // publica; o cliente manda input e desenha o que recebe. Single-player
    // continua pelo stub de loopback, que é o caminho exercitado desde o M2.
    let mut sessao: Option<giochi_tower::net::SessaoJogo> = None;
    #[cfg(feature = "stub-net")]
    let mut session = giochi_tower::net::local_session();
    #[cfg(feature = "stub-net")]
    let mut net_inputs: u64 = 0;

    if let Some(porta) = options.hospedar {
        sessao = Some(giochi_tower::net::SessaoJogo::hospedar(porta)?);
        log::info!("rede: hospedando na porta {porta}");
    }
    if let Some(endereco) = options.entrar.as_deref() {
        sessao = Some(giochi_tower::net::SessaoJogo::entrar(endereco)?);
        log::info!("rede: entrando em {endereco}");
    }
    // `--teclas`: uma tecla por quadro, injetada como **entrada** e não como
    // chamada direta ao menu.
    //
    // A versão anterior chamava `Menu::entrada` aqui, antes do laço, e
    // descartava a `Acao` devolvida: o menu trocava de tela, o log dizia
    // "--teclas: a -> tela partida" e a partida **não começava** — 60 quadros
    // depois, `0 passo(s) de simulação`. É a mesma classe do `--teclas` do
    // launcher (AGENTS.md §24): ferramenta que imprime um estado plausível sem
    // ter feito nada. Agora a tecla entra pela fila de eventos e quem executa a
    // ação é o laço, no caminho de produção.
    let mut fila_teclas = parse_teclas(&options.teclas);
    let tinha_teclas = !fila_teclas.is_empty();
    if tinha_teclas {
        log::info!("--teclas: {} tecla(s) na fila", fila_teclas.len());
    }
    // Um quadro de soltura entre duas teclas: a navegação tem repetição de
    // `REPETICAO_S` (0,22 s) e a tecla seguinte, chegando 1/60 s depois, cairia
    // dentro dela e seria **engolida**. Foi o que aconteceu com
    // `baixo,baixo,a` — parava em "Multijogador" e o resto ia para outra tela.
    // Um dedo levanta o botão; a fila agora também.
    let mut soltar_tecla = false;
    let mut teclas_logadas = false;

    let mut fps_medido = 0.0_f32;
    let mut anunciou_rede = false;
    let mut quadros_para_fps = 0u32;
    let mut inicio_fps = Instant::now();
    let mut jogadores_rede = 1usize;

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
        // `--sem-entrada`: o aparelho tem controle conectado e o analógico parado
        // emite ruído; ler isso faria o mesmo `--frames` divergir do PC (medido:
        // 273 mil pixels, 68 % do quadro) e a comparação de quadro não mediria
        // nada. A fila de `--teclas` continua valendo — é entrada sintética.
        let mut frame_input = if options.sem_entrada {
            InputFrame::default()
        } else {
            input.update(&events, (drawable_w as f32, drawable_h as f32))
        };
        // Uma tecla da fila por quadro: dá tempo de a borda ser consumida e
        // reproduz o que o dedo faz, em vez de aplicar três comandos no mesmo
        // instante (o que o menu leria como uma tecla só, por causa da borda).
        if soltar_tecla {
            frame_input = InputFrame::default();
            soltar_tecla = false;
        } else if !fila_teclas.is_empty() {
            frame_input = fila_teclas.remove(0).quadro();
            soltar_tecla = true;
        }
        // O estado do menu é o resultado que o `--teclas` promete. Imprimi-lo no
        // fim da fila (e não antes de aplicar) é o que separa "mudei de tela" de
        // "o comando foi executado".
        if tinha_teclas && fila_teclas.is_empty() && !soltar_tecla && !teclas_logadas {
            log::info!(
                "--teclas: {} -> tela {} | cursor {} | {}",
                options.teclas.join(","),
                menu.tela.nome(),
                menu.cursor,
                menu.item_atual().map(|i| i.rotulo).unwrap_or_default()
            );
            teclas_logadas = true;
        }
        if !announced_source && frame_input.source != giochi_tower::engine_bridge::InputSource::None
        {
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

        let delta = if deterministic {
            fixed_step
        } else {
            clock.tick()
        };

        // --- menu na frente ---------------------------------------------
        //
        // O menu consome a entrada **antes** da simulação: com o mundo
        // congelado, um "reiniciar" não pode deixar meio passo de simulação
        // entrar no estado novo.
        if modo == Modo::Menu {
            let acao = menu.entrada(&frame_input, delta);
            match acao {
                menu::Acao::Nada => {}
                menu::Acao::Voltar => {
                    // "Continuar" e o B na pausa: volta ao jogo, com o relógio
                    // reiniciado para a pausa não virar um salto de tempo.
                    if menu.em_partida {
                        clock = FrameClock::new();
                        modo = Modo::Jogando;
                    }
                }
                menu::Acao::Jogar | menu::Acao::Reiniciar => {
                    state = GameState::new(&config, options.class, options.profile);
                    state.camera.distance_m = config_jogo.camera_dist_m;
                    aplicar_demo(&mut state, &options);
                    clock = FrameClock::new();
                    game_loop = GameLoop::new();
                    menu.marcar_partida(true);
                    modo = Modo::Jogando;
                    log::info!(
                        "partida: {}",
                        if acao == menu::Acao::Jogar {
                            "nova"
                        } else {
                            "reiniciada"
                        }
                    );
                }
                menu::Acao::MenuPrincipal => {
                    // A sessão de rede morre com a partida: manter o socket aberto
                    // deixaria o host publicando um mundo que ninguém vê.
                    //
                    // A **tela** quem troca é o menu (`Menu::sair_da_partida`):
                    // antes isso era feito só aqui, zerando o `modo`, e o painel
                    // continuava desenhando a pausa — "Sair para o menu" não fazia
                    // nada visível (relato do dono, 11/11/2026).
                    sessao = None;
                    menu.marcar_partida(false);
                    modo = Modo::Menu;
                    log::info!("menu: partida encerrada — de volta ao menu principal");
                }
                menu::Acao::Sair => {
                    log::info!("sair pedido pelo menu");
                    break;
                }
                menu::Acao::Hospedar => {
                    iniciar_rede(
                        &mut menu,
                        &mut sessao,
                        Papel::Host,
                        options.hospedar.unwrap_or(PORTA_PADRAO),
                    );
                }
                menu::Acao::ProcurarSalas => {
                    procurar_salas(&mut menu, options.hospedar.unwrap_or(PORTA_PADRAO));
                }
                menu::Acao::EntrarSala(indice) => entrar_em_sala(
                    &mut menu,
                    &mut sessao,
                    &mut state,
                    &options,
                    &config,
                    indice,
                ),
                menu::Acao::Abrir(_) | menu::Acao::Pausar => {}
            }

            // Mudança de configuração vale no quadro seguinte: volume no mixer e
            // distância na câmera. Aplicar no ato evita "só vale depois de
            // reiniciar", que é indistinguível de não funcionar.
            if menu.config != config_jogo {
                config_jogo = menu.config.saneada();
                audio.definir_volume(config_jogo.volume_fracao());
                state.camera.distance_m = config_jogo.camera_dist_m;
                if let Err(erro) = salvar_config(&caminho_settings, config_jogo) {
                    // Não salvar não impede jogar; engolir o erro faria a escolha
                    // sumir no próximo boot sem ninguém saber por quê.
                    log::warn!("não consegui salvar {}: {erro}", caminho_settings.display());
                }
            }
        }

        // --- simulação: passos fixos, nunca ditados pelo framerate -------
        let steps = if modo == Modo::Jogando {
            game_loop.advance(delta)
        } else {
            0
        };
        for _ in 0..steps {
            state.step(fixed_step, &frame_input, &config);
        }

        // Pausa: o botão Start abre o menu de pausa em vez de alternar um
        // booleano dentro do estado. Um `paused` no estado deixava dois lugares
        // decidindo se o mundo anda, e o jogador via a pausa "não pegar" quando um
        // deles discordava.
        if modo == Modo::Jogando && frame_input.edges.pause {
            menu.abrir(menu::Tela::Pausa);
            modo = Modo::Menu;
            continue;
        }

        // --- rede: input sobe, snapshot desce filtrado por AOI -----------
        let mut visible = state.snapshot_enemies();
        // Sessão real tem prioridade: com duas ativas, o stub entregaria um
        // snapshot de um mundo que ninguém está jogando.
        if let Some(rede) = sessao.as_mut() {
            visible = rede.passo(&mut state, &frame_input, &config, fixed_step);
            if !anunciou_rede {
                log::info!(
                    "rede: {:?} com {} jogador(es)",
                    rede.papel(),
                    rede.jogadores()
                );
                anunciou_rede = true;
            }
            jogadores_rede = rede.jogadores();
        }
        // `if let` aninhado em vez de let-chain: o crate é edição 2021, onde
        // `&& let` não compila (o erro fala de edição, não de sintaxe).
        #[cfg(feature = "stub-net")]
        if let (None, Some((host, client))) = (sessao.as_ref(), session.as_mut()) {
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

        // Fim de partida: abre o menu de fim em vez de encerrar o processo. Antes
        // disto a janela simplesmente fechava ao perder a torre, e o jogador não
        // tinha como reiniciar sem reabrir o jogo.
        if state.phase.is_over() && modo == Modo::Jogando {
            let venceu = state.phase == Phase::Victory;
            log::info!("fim: {}", state.summary());
            menu.marcar_partida(false);
            menu.abrir(menu::Tela::Fim { venceu });
            modo = Modo::Menu;
        }

        // --- desenho ----------------------------------------------------
        let camera = state.camera.engine_camera(drawable_w, drawable_h);
        render.begin();
        render.draw_ground(&camera)?;

        // O planeta morto: pedras, crateras, mesas no horizonte e destroços.
        // Uma chamada por modelo — as cópias já foram fundidas na abertura, então
        // não há matriz de instância para reenviar a cada quadro.
        if options.cenario {
            render.draw_cenario(&camera)?;
        }

        // A torre, o alvo a defender (GDD §11). O modelo cresce com o upgrade de
        // HP, então a silhueta diz o quanto ela já foi reforçada.
        let torre = cena::da_torre(state.tower.max_hp);
        render.draw_modelo(
            &camera,
            torre.modelo,
            glam::Vec3::new(0.0, 0.0, 0.0),
            torre.altura,
            0.0,
            Color3D::new(torre.cor[0], torre.cor[1], torre.cor[2], 1.0),
        )?;

        // Jogadores: um modelo por classe, com a cor da classe. O jogador local
        // ganha um marcador no chão para se achar no meio de quatro.
        for jogador in &state.players {
            if !jogador.can_act() {
                continue;
            }
            let aparencia = cena::da_classe(jogador.class);
            render.draw_modelo(
                &camera,
                aparencia.modelo,
                glam::Vec3::new(jogador.position.x, 0.0, jogador.position.z),
                aparencia.altura,
                jogador.aim.x.atan2(jogador.aim.z),
                Color3D::new(aparencia.cor[0], aparencia.cor[1], aparencia.cor[2], 1.0),
            )?;
            if jogador.id == state.local_id {
                render.draw_cube(
                    &camera,
                    glam::Vec3::new(jogador.position.x, 0.03, jogador.position.z),
                    1.3,
                    PLAYER_COLOR,
                )?;
            }
        }

        // --- de onde se está atirando ----------------------------------
        //
        // Relato do dono (10/11/2026): *"a visão de onde se está atirando está
        // impossível"*. O jogo não atira para onde o analógico aponta: atira no
        // inimigo **mais próximo** dentro de um cone de ~70° (`GameState::
        // target_in_aim_de`). Sem marcar quem o cone pegou, o gatilho parece
        // aleatório — e "parece" é o problema, porque a regra é determinística.
        //
        // Três peças, e cada uma responde uma pergunta diferente:
        //   1. com alvo: linha vermelha do soldado até ele e retículo em volta;
        //   2. sem alvo: traço curto e claro na direção da mira — "é para cá que
        //      eu aponto", que é a única informação disponível quando nada está
        //      ao alcance;
        //   3. o retículo acompanha o **tamanho do corpo**, então o mesmo código
        //      marca bug pequeno e boss.
        //
        // Custo: 1 chamada sem alvo, 2 com. O cenário inteiro custa 5.
        if options.mira {
            if let Some(local) = state.players.iter().find(|p| p.id == state.local_id) {
                if local.can_act() {
                    let alvo = state.alvo_em_mira(state.local_id, &config);
                    match alvo {
                        Some(alvo) => {
                            let altura = cena::do_byte_de_inimigo(
                                alvo.tipo,
                                state.boss.as_ref().map(|b| b.kind),
                            )
                            .altura;
                            let origem = glam::Vec3::new(local.position.x, 0.06, local.position.z);
                            let destino = glam::Vec3::new(alvo.x, 0.06, alvo.z);
                            let delta = destino - origem;
                            render.draw_mira(
                                &camera,
                                origem,
                                delta,
                                delta.length(),
                                MIRA_LARGURA_M,
                                TARGET_COLOR.with_alpha(LINHA_ALFA),
                            )?;
                            render.draw_alvo(
                                &camera,
                                glam::Vec3::new(alvo.x, 0.07, alvo.z),
                                altura * RAIO_DO_ALVO,
                                TARGET_COLOR,
                            )?;
                        }
                        None => {
                            // Traço + ponta: um risco sozinho some no meio das pedras,
                            // e a ponta diz onde ele acaba. Duas chamadas, como o par
                            // linha+retículo do caso com alvo.
                            let frente = glam::Vec3::new(local.aim.x, 0.0, local.aim.z);
                            let inicio = glam::Vec3::new(
                                local.position.x + frente.x * MIRA_INICIO_M,
                                0.05,
                                local.position.z + frente.z * MIRA_INICIO_M,
                            );
                            let alcance =
                                (local.attack_range(&config) - MIRA_INICIO_M).max(MIRA_CURTA_MIN_M);
                            render.draw_mira(
                                &camera,
                                inicio,
                                frente,
                                alcance,
                                MIRA_LARGURA_M,
                                AIM_COLOR.with_alpha(LINHA_ALFA),
                            )?;
                            render.draw_cube(
                                &camera,
                                glam::Vec3::new(
                                    inicio.x + frente.x * alcance,
                                    0.05,
                                    inicio.z + frente.z * alcance,
                                ),
                                MIRA_PONTA_M,
                                AIM_COLOR,
                            )?;
                        }
                    }
                }
            }
        }

        // Inimigos: desenhados do **snapshot**, não do mundo do host. Se a AOI
        // filtrar demais, a tela mostra menos inimigos — que é a falha que se
        // quer ver acontecendo, e não um silêncio.
        //
        // Boss e Coronel desenham sozinhos (texturizado, um por vez: são poucos e
        // são o que o jogador precisa reconhecer). A horda sai em **um lote por
        // tipo** — 60 inimigos em 60 chamadas custariam ~90 µs de CPU por quadro
        // só para submeter no Cortex-A35.
        let mut horda: Vec<cena::Copia> = Vec::with_capacity(visible.len());
        for enemy in &visible {
            let aparencia =
                cena::do_byte_de_inimigo(enemy.tipo, state.boss.as_ref().map(|b| b.kind));
            let pos = glam::Vec3::new(enemy.x, 0.0, enemy.z);
            // O giro é **derivado**: o snapshot não carrega orientação, e todo
            // inimigo caminha para a torre. Apontar para o centro é exatamente o
            // que ele está fazendo, e custa uma conta em vez de um campo novo no
            // protocolo (que exigiria subir a versão e recusar cliente antigo).
            let giro = (-enemy.x).atan2(-enemy.z);
            if enemy.tipo == 9 || enemy.tipo == 10 {
                render.draw_modelo(
                    &camera,
                    aparencia.modelo,
                    pos,
                    aparencia.altura,
                    giro,
                    Color3D::new(aparencia.cor[0], aparencia.cor[1], aparencia.cor[2], 1.0),
                )?;
                continue;
            }
            horda.push(cena::Copia {
                modelo: aparencia.modelo,
                pos,
                altura: aparencia.altura,
                giro,
                cor: aparencia.cor,
            });
        }
        for lote in cena::agrupar(&horda) {
            let antes = Instant::now();
            let chamadas = render.draw_modelo_lote(
                &camera,
                lote.modelo,
                lote.matrizes.as_slice(),
                Color3D::new(lote.cor[0], lote.cor[1], lote.cor[2], 1.0),
            )?;
            if sem_espera {
                cpu_horda += antes.elapsed();
                chamadas_horda += u64::from(chamadas);
            }
        }

        // --- UI: HUD, minimapa e loja -----------------------------------
        //
        // No menu, o mundo fica congelado atrás e o painel entra por cima. É o
        // que dá contexto à pausa: o jogador vê onde estava.

        if modo == Modo::Menu {
            if options.ui {
                let plano = layout::menu(&menu, layout::Tela::new(drawable_w, drawable_h));
                render.draw_ui(&plano)?;
            }
            // A captura tem de acontecer **antes** da troca de buffers. Com
            // GLES/KMSDRM o `gl_swap_window` esvazia o back buffer, e ler depois
            // dele grava um BMP preto — a armadilha que o launcher só contornava
            // forçando o backend por software (AGENTS.md §23). Ler antes resolve
            // na origem e vale para os dois backends.
            let ultimo_quadro = options.frames.is_some_and(|limite| frames + 1 >= limite);
            if ultimo_quadro {
                if let Some(path) = options.capture.as_deref() {
                    render.capture_bmp(path)?;
                }
            }
            render.end()?;
            frames += 1;
            if ultimo_quadro {
                break;
            }
            if !sem_espera {
                let spent = started.elapsed().as_millis() as u32;
                if spent < FRAME_MS {
                    platform.delay_ms(FRAME_MS - spent);
                }
            }
            continue;
        }

        //
        // Montada a partir do estado **deste** quadro e desenhada por cima do
        // mundo. As duas camadas são separadas de propósito: `ui::build` produz
        // números, `layout::montar` produz pixels e só `draw_ui` toca o GL — o
        // que permite testar "a loja cabe em 640x480?" sem abrir janela.
        if options.ui {
            let antes = Instant::now();
            let mut frame_ui = ui::build(&state, &config);
            if config_jogo.mostrar_fps {
                // O FPS é medida, não decoração: é o número que responde "este
                // aparelho aguenta esta onda" sem abrir o benchmark.
                frame_ui.hud.fps = Some((fps_medido, jogadores_rede));
            }
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

        // FPS por média móvel de 30 quadros: um número instantâneo pisca a cada
        // vblank perdido e não diz nada sobre o desempenho.
        quadros_para_fps += 1;
        if quadros_para_fps >= 30 {
            let decorrido = inicio_fps.elapsed().as_secs_f32();
            if decorrido > 0.0 {
                fps_medido = quadros_para_fps as f32 / decorrido;
            }
            quadros_para_fps = 0;
            inicio_fps = Instant::now();
        }

        // A captura é do **último** quadro, não de todos: gravar dentro do laço
        // escrevia o BMP 60 vezes em `--frames 60` e enchia o log com 60 linhas
        // idênticas, escondendo o resto do diagnóstico.
        //
        // Antes de `end()`, pelo mesmo motivo do caminho do menu: o swap esvazia
        // o back buffer e a captura sairia preta no aparelho.
        let ultimo_quadro = options.frames.is_some_and(|limite| frames + 1 >= limite);
        if ultimo_quadro {
            if let Some(path) = options.capture.as_deref() {
                render.capture_bmp(path)?;
            }
        }

        render.end()?;

        frames += 1;
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
        let fps = if total > 0.0 {
            frames as f64 / total
        } else {
            0.0
        };
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
        let horda_ms = por_quadro(cpu_horda);
        // A contagem da horda é a **única** grandeza que não é por quadro por
        // natureza (duas ondas de inimigos desenham números diferentes), então
        // ela sai como média por quadro. Publicá-la crua — 1041 num laço de
        // 1800 quadros — lia como "1041 chamadas num quadro", que é um número
        // plausível e falso: 12 inimigos desenham no máximo 3 lotes.
        let horda_chamadas = if frames > 0 {
            chamadas_horda as f64 / frames as f64
        } else {
            0.0
        };
        log::info!(
            "benchmark: {frames}/{limite} quadro(s) em {total:.2} s = {fps:.1} fps · trabalho por quadro: {cpu_ms:.2} ms · ui: {} lote(s) → {} chamada(s) reais, {} instância(s) · montar {montar_ms:.2} ms + desenhar {desenho_ms:.2} ms · horda {horda_ms:.2} ms em {horda_chamadas:.2} chamada(s)/quadro",
            render.hud_draw_calls(),
            render.hud_gl_draws(),
            render.hud_instances(),
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
        input
            .pad_name()
            .unwrap_or("nenhum (teclado/mouse ou touch emulado)")
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

/// Lê `settings.json`, caindo no padrão quando ele não existe ou está corrompido.
///
/// Não é erro abrir o jogo pela primeira vez: `NotFound` vira o padrão em
/// silêncio. JSON inválido, **sim**, vira aviso no log — um arquivo editado à mão
/// que o jogo ignora sem dizer nada é o defeito que o validador deste repositório
/// combate desde o `buttons.map`.
fn carregar_config(caminho: &Path) -> menu::Config {
    match std::fs::read_to_string(caminho) {
        Ok(texto) => match serde_json_lite(&texto) {
            Some(config) => config.saneada(),
            None => {
                log::warn!(
                    "{} não é um settings.json válido; usando o padrão",
                    caminho.display()
                );
                menu::Config::default()
            }
        },
        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => menu::Config::default(),
        Err(erro) => {
            log::warn!("não consegui ler {}: {erro}", caminho.display());
            menu::Config::default()
        }
    }
}

/// Grava `settings.json`.
///
/// Escrito à mão, sem dependência de serialização: são três campos, e o crate não
/// tem `serde_json`. O formato é o JSON mais chato possível, com uma chave por
/// linha, para um `diff` mostrar a mudança.
fn salvar_config(caminho: &Path, config: menu::Config) -> std::io::Result<()> {
    let config = config.saneada();
    let texto = format!(
        "{{\n  \"volume\": {},\n  \"camera_dist_m\": {:.1},\n  \"mostrar_fps\": {}\n}}\n",
        config.volume, config.camera_dist_m, config.mostrar_fps
    );
    std::fs::write(caminho, texto)
}

/// Lê os três campos de um JSON de configuração.
///
/// Parser mínimo e **tolerante**: procura `"chave"` e o valor depois dos dois
/// pontos. Não valida JSON — a versão estrita seria um parser inteiro para três
/// números, e o pior caso dele é cair no padrão, que é o que já acontece com
/// arquivo ausente.
fn serde_json_lite(texto: &str) -> Option<menu::Config> {
    let ler_numero = |chave: &str| -> Option<f32> {
        let inicio = texto.find(&format!("\"{chave}\""))? + chave.len() + 2;
        let resto = &texto[inicio..];
        let dois_pontos = resto.find(':')? + 1;
        let valor: String = resto[dois_pontos..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
            .collect();
        valor.parse().ok()
    };
    let ler_bool = |chave: &str| -> Option<bool> {
        let inicio = texto.find(&format!("\"{chave}\""))? + chave.len() + 2;
        let resto = &texto[inicio..];
        let dois_pontos = resto.find(':')? + 1;
        let valor = resto[dois_pontos..].trim_start();
        if valor.starts_with("true") {
            Some(true)
        } else if valor.starts_with("false") {
            Some(false)
        } else {
            None
        }
    };

    Some(menu::Config {
        volume: ler_numero("volume")?.round().clamp(0.0, 100.0) as u8,
        camera_dist_m: ler_numero("camera_dist_m")?,
        mostrar_fps: ler_bool("mostrar_fps").unwrap_or(false),
    })
}

/// Aplica uma lista de teclas ao menu, pelo caminho de produção.
///
/// "Produção" quer dizer [`Menu::entrada`] com um `InputFrame` sintético — o
/// mesmo que o analógico produz. Um atalho que mexesse no `cursor` testaria a
/// suposição de quem escreveu o teste, não o menu.
/// Uma tecla que o `--teclas` sabe apertar.
///
/// `menu::Direcao` é o vocabulário **do menu**; o START não está nele porque o
/// menu não o consome — quem trata é o laço, que abre a pausa. Faltava essa
/// tecla e a consequência era o caminho pausa → reiniciar → sair não poder ser
/// exercitado por linha de comando: só com o dedo no controle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tecla {
    Dir(menu::Direcao),
    Pausa,
}

impl Tecla {
    fn quadro(self) -> InputFrame {
        match self {
            Self::Dir(direcao) => menu::quadro(direcao),
            Self::Pausa => {
                let mut frame = InputFrame::default();
                frame.edges.pause = true;
                frame
            }
        }
    }
}

fn parse_teclas(teclas: &[String]) -> Vec<Tecla> {
    let mut fila = Vec::with_capacity(teclas.len());
    for tecla in teclas {
        match tecla.to_ascii_lowercase().as_str() {
            "cima" | "up" => fila.push(Tecla::Dir(menu::Direcao::Cima)),
            "baixo" | "down" => fila.push(Tecla::Dir(menu::Direcao::Baixo)),
            "esq" | "esquerda" | "left" => fila.push(Tecla::Dir(menu::Direcao::Esquerda)),
            "dir" | "direita" | "right" => fila.push(Tecla::Dir(menu::Direcao::Direita)),
            "a" | "confirma" | "enter" => fila.push(Tecla::Dir(menu::Direcao::Confirma)),
            "b" | "volta" | "back" => fila.push(Tecla::Dir(menu::Direcao::Volta)),
            "start" | "pausa" | "pause" => fila.push(Tecla::Pausa),
            // Nome errado é erro, não aviso: uma tecla descartada em silêncio
            // transforma o diagnóstico inteiro em ficção.
            outra => log::warn!("--teclas: '{outra}' não é uma tecla conhecida"),
        }
    }
    fila
}

/// Liga a demo da loja quando `--demo-loja` foi pedido.
fn aplicar_demo(state: &mut GameState, options: &Options) {
    if !options.demo_loja {
        return;
    }
    // Loja aberta e gold inicial para o menu ter itens compráveis e outros não:
    // uma tela só com "sem gold" não mostra o marcador de escolha.
    state.phase = Phase::Shop;
    state.local_mut().gold = 120;
    log::info!("demo: loja aberta na abertura (--demo-loja)");
}

/// Papel que o menu pode pedir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Papel {
    /// Hospedar.
    Host,
}

/// Sobe uma sessão de rede pedida pelo menu.
fn iniciar_rede(
    menu: &mut Menu,
    sessao: &mut Option<giochi_tower::net::SessaoJogo>,
    papel: Papel,
    porta: u16,
) {
    {
        match papel {
            Papel::Host => match giochi_tower::net::SessaoJogo::hospedar(porta) {
                Ok(nova) => {
                    *sessao = Some(nova);
                    menu.aviso = Some(format!("hospedando na porta {porta}"));
                    // Anunciar é o que faz a sala aparecer na procura dos outros.
                    if let Err(erro) =
                        giochi_tower::net::descoberta::Anunciante::novo(porta, TITLE).iniciar()
                    {
                        log::warn!("descoberta indisponível: {erro}");
                        menu.aviso = Some(format!("hospedando (sem anúncio: {erro})"));
                    }
                }
                Err(erro) => {
                    log::error!("{erro}");
                    menu.aviso = Some(format!("{erro}"));
                }
            },
        }
    }
}

/// Procura salas e põe o resultado no menu.
fn procurar_salas(menu: &mut Menu, porta: u16) {
    let _ = porta;
    log::info!("rede: procurando salas…");
    let salas = giochi_tower::net::procurar(
        giochi_tower::net::descoberta::PORTA_DESCOBERTA,
        giochi_tower::net::descoberta::ESPERA_PADRAO_MS,
    );
    if salas.is_empty() {
        menu.aviso = Some("nenhuma sala encontrada".into());
        log::info!("rede: nenhuma sala");
    } else {
        log::info!("rede: {} sala(s)", salas.len());
        menu.aviso = None;
    }
    menu.definir_salas(salas.iter().map(|s| s.rotulo()).collect());
    // Guarda os endereços para o "Entrar" — o rótulo é para o olho, o endereço é
    // para o socket, e derivar um do outro por `split('@')` quebraria num nome
    // com arroba.
    SALAS_ATUAIS.with(|celula| *celula.borrow_mut() = salas);
}

thread_local! {
    /// Última procura. `thread_local` porque o menu roda no laço principal e não
    /// há outro consumidor — um `static mut` exigiria `unsafe` por nada.
    static SALAS_ATUAIS: std::cell::RefCell<Vec<giochi_tower::net::Sala>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Entra na sala `indice` da última procura.
fn entrar_em_sala(
    menu: &mut Menu,
    sessao: &mut Option<giochi_tower::net::SessaoJogo>,
    state: &mut GameState,
    options: &Options,
    config: &GameConfig,
    indice: usize,
) {
    let sala = SALAS_ATUAIS.with(|celula| celula.borrow().get(indice).cloned());
    let Some(sala) = sala else {
        menu.aviso = Some("essa sala não está mais disponível".into());
        return;
    };
    {
        match giochi_tower::net::SessaoJogo::entrar(&sala.endereco.to_string()) {
            Ok(nova) => {
                *sessao = Some(nova);
                // Ao entrar, a partida antiga não serve: o mundo é o do host.
                *state = GameState::new(config, options.class, options.profile);
                // Não se adivinha o id: ele vem no handshake e o `passo()` da
                // sessão adota o valor do crate. Fixar 1 aqui funcionava só
                // enquanto o host atribuísse 1 ao primeiro cliente.
                menu.aviso = Some(format!("entrou em {}", sala.rotulo()));
                menu.abrir(menu::Tela::Pausa);
            }
            Err(erro) => {
                log::error!("{erro}");
                menu.aviso = Some(format!("{erro}"));
            }
        }
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
/// Onde procurar os `.ron`, do mais específico para o mais genérico.
///
/// O padrão era `config` relativo ao **diretório de execução**, e isso quebra no
/// aparelho de duas maneiras: o launcher inicia o app com `cwd` na pasta do app
/// (funciona), mas rodar à mão de dentro de `bin/` não acha nada — e o sintoma no
/// aparelho foi `não consegui ler config/classes.ron`, que parece instalação
/// incompleta e é resolução de caminho.
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

    // `peekable` porque `--hospedar` tem argumento **opcional**: consumi-lo com
    // `next` engoliria a flag seguinte quando a porta não fosse dada.
    let mut args = args.peekable();
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
            "--menu" => options.menu = true,
            "--hospedar" => {
                // Porta opcional: `--hospedar` sozinho usa a padrão.
                let porta = args.peek().and_then(|v| v.parse::<u16>().ok());
                if porta.is_some() {
                    let _ = args.next();
                }
                options.hospedar = Some(porta.unwrap_or(PORTA_PADRAO));
            }
            "--sem-mira" => options.mira = false,
            "--sem-entrada" => options.sem_entrada = true,
            "--posicionar" => {
                let par = args
                    .next()
                    .ok_or_else(|| GameError::message("--posicionar precisa de x,z"))?;
                let (x, z) = par.split_once(',').ok_or_else(|| {
                    GameError::message(format!("--posicionar espera x,z — veio '{par}'"))
                })?;
                let x = x
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| GameError::message(format!("x inválido: '{x}'")))?;
                let z = z
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| GameError::message(format!("z inválido: '{z}'")))?;
                if !x.is_finite() || !z.is_finite() {
                    return Err(GameError::message("--posicionar com valor não finito"));
                }
                options.posicionar = Some((x, z));
            }
            "--teclas" => {
                let lista = args
                    .next()
                    .ok_or_else(|| GameError::message("--teclas precisa de uma lista"))?;
                options.teclas = lista
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
                if options.teclas.is_empty() {
                    return Err(GameError::message("--teclas vazia"));
                }
            }
            "--entrar" => {
                let endereco = args.next().ok_or_else(|| {
                    GameError::message("--entrar precisa de um endereço ip:porta")
                })?;
                options.entrar = Some(endereco);
            }
            "--sem-ui" => options.ui = false,
            "--sem-cenario" => options.cenario = false,
            "--vitrine" => options.vitrine = true,
            "--demo-loja" => options.demo_loja = true,
            "--benchmark" => {
                let value = args
                    .next()
                    .ok_or_else(|| GameError::message("--benchmark precisa de um número"))?;
                let quadros = value
                    .parse::<u64>()
                    .map_err(|_| GameError::message(format!("--benchmark inválido: '{value}'")))?;
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

/// Desenha os 20 modelos em quatro fileiras, sob uma câmera própria.
///
/// # O que esta função responde
///
/// "O modelo carregou" o teste já responde sem GPU. O que nenhum teste responde é
/// **se ele parece o que devia** — e para isso é preciso olhar. Jogar até a onda do
/// boss para ver o Tanker Bug é caro; aqui os vinte cabem num quadro.
///
/// # A câmera é mais perto que a do jogo, e de propósito
///
/// A do jogo fica a 52 m para caber combate; nessa distância um soldado de 2,4 m
/// mede **34 px de altura** em 640×480 (medido), e não dá para julgar silhueta. A
/// da vitrine fica a 26 m: o dobro do tamanho na tela, que é o que se quer para
/// olhar arte.
///
/// # Quatro fileiras de cinco
///
/// Uma fileira só com os vinte daria 70 m de largura. Quatro fileiras de cinco dão
/// 17 m, e todas cabem na mesma vista. A ordem segue `modelos::todos_os_nomes()`:
/// classes, inimigos, bosses+torre, cenário — assim um modelo novo aparece no
/// grupo certo quando alguém acrescentar.
fn vitrine(render: &mut RenderBridge) -> Result<()> {
    let camera = giochi_tower::game::IsoCamera {
        focus: glam::Vec3::ZERO,
        distance_m: 26.0,
        ..giochi_tower::game::IsoCamera::new(glam::Vec3::ZERO)
    }
    .engine_camera(1280, 720);

    render.draw_ground(&camera)?;

    let fileiras: [(&[&str], f32, f32); 4] = [
        (&["soldado", "guerreiro", "sniper", "medico"], 6.0, 2.4),
        (&["grunt", "tank", "ranged", "swarm", "flying"], 2.0, 2.2),
        (
            &["tanker", "plasma", "brain", "queen", "coronel"],
            -2.0,
            3.4,
        ),
        (
            &["torre", "rocha", "espigao", "cratera", "destroco"],
            -7.0,
            4.0,
        ),
    ];

    for (nomes, z, altura) in fileiras {
        for (i, nome) in nomes.iter().enumerate() {
            let x = (i as f32 - (nomes.len() as f32 - 1.0) * 0.5) * 3.6;
            // O cenário tem escala própria no jogo (uma cratera é um buraco de
            // 12 m de diâmetro, uma mesa tem 20 m de altura) e na vitrine seria
            // ilegível. Aqui ele entra na altura da fileira, que é o que permite
            // comparar a silhueta.
            let no_mundo = if giochi_tower::engine_bridge::modelos::CENARIO.contains(nome) {
                altura * 0.55
            } else {
                altura
            };
            render.draw_modelo(
                &camera,
                nome,
                glam::Vec3::new(x, 0.0, z),
                no_mundo,
                0.0,
                Color3D::WHITE,
            )?;
        }
    }
    Ok(())
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
        assert_eq!(
            args(&["--classe", "guerreiro"]).unwrap().class,
            ClassId::Warrior
        );
        assert_eq!(args(&["--classe", "medic"]).unwrap().class, ClassId::Medic);
        assert_eq!(
            args(&["--perfil", "rk3326"]).unwrap().profile,
            ProfileId::Rk3326
        );
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
