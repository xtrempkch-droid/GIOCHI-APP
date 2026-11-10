//! Launcher do Giochi **portado para a GIOCHI-ENGINE** — versão de teste.
//!
//! ## Por que um app separado, e não uma flag no launcher
//!
//! O launcher de produção é o que roda no aparelho e tem 223 testes e um
//! histórico de defeitos medidos em campo. Trocar a camada de desenho dele para
//! "experimentar" colocaria em risco tudo o que já foi provado. Aqui a engine é
//! exercitada num binário próprio: se ela falhar, o launcher real continua
//! intacto.
//!
//! A engine é o crate 2D do workspace **GIOCHI-ENGINE-3D** (2D + 3D + ABI em C +
//! Lua), versão 0.2.0, de superfície C-shaped: textura é `u64`, sem tempo de
//! vida em lugar nenhum.
//!
//! ## O que é reusado, e o que é novo
//!
//! | Peça | Origem |
//! | --- | --- |
//! | estado, foco, reducer, grade | `giochi-launcher-core` (**o mesmo crate**) |
//! | telas, tema, ícones, dica do rodapé | `launcher/crates/ui-sdl/src/*` (**os mesmos arquivos**, via `#[path]`) |
//! | janela, textura, apresentação, blend | **GIOCHI-ENGINE** |
//! | `Painter` | novo, em `src/draw.rs` |
//!
//! Reusar as telas por `#[path]` é a decisão que dá sentido ao teste: se o
//! desenho diferir do de produção, a diferença está na engine — nunca em uma
//! tela reescrita. Copiar `views.rs` produziria duas UIs que começam iguais e
//! divergem na primeira correção.

// As telas são literalmente os arquivos do launcher de produção. `#[path]` faz o
// módulo apontar para fora da árvore deste crate sem duplicar o código.
#[path = "../../launcher/crates/ui-sdl/src/theme.rs"]
mod theme;
#[path = "../../launcher/crates/ui-sdl/src/icons.rs"]
mod icons;
#[path = "../../launcher/crates/ui-sdl/src/views.rs"]
mod views;

// O catálogo, o leitor de `/dev/input` e o lançador de jogo são **os mesmos
// arquivos** do launcher de produção, pelo mesmo motivo das telas: uma segunda
// leitura do `index.json` (ou um segundo mapa de botões) começaria igual e
// divergiria na primeira correção. O que este app troca é a **engine de
// desenho**, não o contrato com o sistema.
#[path = "../../launcher/crates/ui-sdl/src/catalogo.rs"]
mod catalogo;
#[path = "../../launcher/crates/ui-sdl/src/input_evdev.rs"]
mod input_evdev;
#[path = "../../launcher/crates/ui-sdl/src/system.rs"]
mod system;

mod draw;
mod efeitos;

use std::path::Path;
use std::time::Instant;


use giochi_engine::{Platform, RenderContext, WindowConfig};
use giochi_launcher_core::state::{
    App as AppEntry, Game, Screen, State, Tab, SETTINGS_CATEGORIES,
};
use giochi_launcher_core::{update, Cmd, Dir};
use giochi_launcher_core::gamepad::Mapper;

use input_evdev::{ButtonsMap, EvdevInput};

use draw::{CANDIDATOS_FONTE, Painter};
use efeitos::Efeitos;

fn main() {
    if let Err(e) = principal() {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

fn argumentos() -> Opcoes {
    let mut o = Opcoes::default();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        let mut proximo = || {
            i += 1;
            argv.get(i).cloned()
        };
        match a.as_str() {
            "--tamanho" => {
                if let Some(v) = proximo() {
                    if let Some((l, alt)) = v.split_once('x') {
                        if let (Ok(l), Ok(alt)) = (l.parse::<u32>(), alt.parse::<u32>()) {
                            o.tamanho = Some((l, alt));
                        }
                    }
                }
            }
            "--dump-frame" => o.dump_frame = proximo(),
            "--font" | "--fonte" => o.fonte = proximo(),
            "--teclas" => o.teclas = proximo(),
            "--tela" => o.tela = proximo(),
            "--demo" => o.demo = true,
            // Pedido de lançamento na abertura. É o mesmo campo que o `A` no
            // cartão preenche (`state.lancar`), então exercita o caminho de
            // verdade — e é o que permite testar o ciclo launcher → jogo →
            // launcher por SSH, sem depender de apertar um botão.
            "--lancar" => o.lancar = proximo(),
            "--efeitos" => o.efeitos = true,
            "--bench" => {
                if let Some(v) = proximo() {
                    o.bench = v.parse().unwrap_or(600);
                }
            }
            "--efeitos-quadros" => {
                if let Some(v) = proximo() {
                    o.efeitos_quadros = v.parse().unwrap_or(90);
                }
            }
            "--rotacao" => o.rotacao = proximo(),
            "-h" | "--ajuda" => {
                ajuda();
                std::process::exit(0);
            }
            outro => {
                eprintln!("argumento desconhecido: {outro}");
                ajuda();
                std::process::exit(2);
            }
        }
        i += 1;
    }
    o
}

struct Opcoes {
    tamanho: Option<(u32, u32)>,
    dump_frame: Option<String>,
    fonte: Option<String>,
    teclas: Option<String>,
    tela: Option<String>,
    demo: bool,
    lancar: Option<String>,
    rotacao: Option<String>,
    efeitos: bool,
    efeitos_quadros: u32,
    bench: u32,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            tamanho: None,
            dump_frame: None,
            fonte: None,
            teclas: None,
            tela: None,
            demo: false,
            lancar: None,
            rotacao: None,
            // `GIOCHI_EFEITOS=1` é o caminho do aparelho: quem instala a versão
            // de teste não quer digitar flag por SSH, e a variável de ambiente
            // é o mesmo padrão que `GIOCHI_GLES` e `GIOCHI_ROTATE` já usam.
            efeitos: std::env::var("GIOCHI_EFEITOS").is_ok_and(|v| v != "0"),
            efeitos_quadros: 90,
            bench: 0,
        }
    }
}

fn ajuda() {
    println!(
        "giochi-launcher-teste — launcher do Giochi sobre a GIOCHI-ENGINE

uso:
  giochi-launcher-teste [opções]

opções:
  --tamanho LxA        tamanho do painel simulado (padrão: o do sistema)
  --dump-frame <arq>   grava um quadro em BMP e sai
  --teclas a,b,right   aplica comandos antes de desenhar (vírgula)
  --tela <nome>        boot | home | biblioteca | apps | ajustes
  --demo               usa o catálogo de demonstração
  --lancar <id|slug>   pede o lançamento já na abertura
  --efeitos            liga a camada de efeitos (GIOCHI_EFEITOS=1)
  --efeitos-quadros N  quadros adiantados antes do --dump-frame (padrão 90)
  --bench N            mede N quadros sem dormir e imprime ms por quadro
  --rotacao 0|90|270   sobrepõe a rotação do sistema
  --fonte <caminho>    caminho do .ttf
  -h, --ajuda          esta ajuda"
    );
}

fn principal() -> Result<(), String> {
    let opcoes = argumentos();

    if let Some(rot) = &opcoes.rotacao {
        std::env::set_var("GIOCHI_ROTATE", rot);
    }

    // Mesma variável do launcher de produção, com os mesmos três estados —
    // `ausente` deixa o SDL escolher, `1` pede GLES, `0`/`false` pede software.
    //
    // Existe porque `--dump-frame` com backend acelerado grava BMP **preto**: o
    // swap esvazia o buffer antes da leitura. Medido nos três aparelhos. Aqui a
    // variável evita ter de saber o nome do driver do SDL (`SDL_RENDER_DRIVER`),
    // que é detalhe de implementação e não de produto.
    match std::env::var("GIOCHI_GLES").ok().as_deref() {
        Some("0") | Some("false") => {
            std::env::set_var("SDL_RENDER_DRIVER", "software");
            eprintln!("GIOCHI_GLES: pedindo o backend por software");
        }
        Some(_) => {
            std::env::set_var("SDL_RENDER_DRIVER", "opengles2");
            eprintln!("GIOCHI_GLES: pedindo o backend acelerado");
        }
        None => {}
    }

    // O painel do aparelho não vem do `--tamanho`: num console o SDL/KMSDRM usa
    // o modo do conector. Ler o `virtual_size` do framebuffer é o que faz este
    // binário abrir no tamanho certo sem uma constante por aparelho.
    let (largura, altura) = opcoes.tamanho.unwrap_or_else(tamanho_do_painel);
    eprintln!("launcher-teste: painel pedido {largura}x{altura}");

    // A `Platform` inicializa o SDL e cria a janela; o `RenderContext` consome a
    // janela, porque o SDL amarra o renderer a ela.
    let platform = Platform::new().map_err(|e| format!("SDL: {e}"))?;

    let fonte = opcoes
        .fonte
        .clone()
        .or_else(|| CANDIDATOS_FONTE.iter().find(|c| std::path::Path::new(c).exists()).map(|s| s.to_string()))
        .ok_or_else(|| format!("nenhuma fonte encontrada em: {}", CANDIDATOS_FONTE.join(", ")))?;

    let mut painter = criar_painter(&platform, largura, altura, &fonte)?;
    let (catalogo, aviso_catalogo) = catalogo_do_aparelho();
    let mut state = estado_inicial(opcoes.demo, &catalogo);

    if let Some(tela) = &opcoes.tela {
        state = aplicar_tela(state, tela);
    }

    // Resolução do `--lancar`: por índice ou por slug, como no launcher de
    // produção. O `slug` está em `alvos`, na mesma ordem dos `games`, então a
    // posição serve de id — e o erro nomeia o pedido em vez de dizer "inválido".
    if let Some(pedido) = &opcoes.lancar {
        let id = pedido
            .parse::<u32>()
            .ok()
            .filter(|id| (*id as usize) < state.games.len())
            .or_else(|| {
                catalogo
                    .alvos
                    .iter()
                    .position(|a| &a.slug == pedido)
                    .map(|p| p as u32)
            })
            .ok_or_else(|| format!("--lancar: app desconhecido: {pedido}"))?;
        state.lancar = Some(id);
        eprintln!("--lancar: pedido de lançamento para o app {id} ({pedido})");
    }

    // ── camada de efeitos (opcional) ─────────────────────────
    //
    // Ligada por `--efeitos` / `GIOCHI_EFEITOS=1`. Ligada, o desenho das telas é
    // **o mesmo**; o que entra é uma camada por cima. Desligada, o quadro tem de
    // sair idêntico ao do launcher de produção — é o que os 100,0 % medidos
    // provam, e é por isso que os efeitos não podem ser o padrão.
    let mut camada = Efeitos::novo(opcoes.efeitos);
    if camada.ativo() {
        let (l, a) = painter.logical_size();
        eprintln!(
            "efeitos: {} — {l}x{a}, {} quadro(s) adiantados",
            Efeitos::lista(),
            opcoes.efeitos_quadros
        );
        // Adiantar antes do primeiro desenho é o que torna `--dump-frame`
        // reprodutível: o BMP não depende de quanto o processo demorou.
        let cor = theme::ACCENT;
        camada.semear(l as i32, a as i32);
        camada.avancar(opcoes.efeitos_quadros, l as i32, a as i32, cor);
    }

    // `--teclas` é aplicado **depois do primeiro desenho**: a navegação usa a
    // grade publicada pelo layout, e antes de desenhar ela seria o padrão. No
    // launcher de produção esta ordem foi um bug medido — a ferramenta
    // imprimia `HomeCarousel` em vez de `LibraryGrid` e "provava" que a
    // navegação não funcionava.
    desenhar_com(&mut painter, &mut state, Some(&mut camada));

    if let Some(lista) = &opcoes.teclas {
        for nome in lista.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match comando(nome) {
                Some(cmd) => state = update(state, cmd),
                None => return Err(format!("--teclas: tecla desconhecida: {nome}")),
            }
            // A rajada é a resposta visual ao comando — e chamá-la aqui, de quem
            // **sabe** que uma tecla chegou, evita adivinhar comparando estados
            // (que erraria em `Back` sobre overlay).
            let (l, a) = painter.logical_size();
            camada.rajada(l as i32, a as i32);
            desenhar_com(&mut painter, &mut state, Some(&mut camada));
        }
        eprintln!(
            "--teclas: {lista} -> foco {:?} {} | filtro {:?} | categoria {:?} | grade {}x{}",
            state.focus.zone,
            state.focus.index,
            state.library_filter,
            SETTINGS_CATEGORIES[state.settings_category.min(SETTINGS_CATEGORIES.len() - 1)],
            state.grade_biblioteca.cols,
            state.grade_biblioteca.por_pagina
        );
    }

    // ── medição ──────────────────────────────────────────────
    //
    // Sem `--dump-frame` e sem tela, o laço normal dorme 16 ms por quadro e todo
    // número de tempo mede o `sleep`, não o desenho. Aqui o laço não dorme: o que
    // sai é o **custo do quadro**, que é o que decide se um efeito cabe.
    //
    // `present()` entra na conta de propósito. O desenho e a apresentação são
    // inseparáveis num aparelho onde o swap espera o vblank: medir só o desenho
    // daria "0,4 ms" e o usuário veria 30 fps.
    if opcoes.bench > 0 {
        let (l, a) = painter.logical_size();
        camada.semear(l as i32, a as i32);
        // Aquece: a primeira passada paga cache frio de fonte e de textura e
        // não representa o regime.
        for _ in 0..20 {
            state = update(state, Cmd::TickHud);
            desenhar_com(&mut painter, &mut state, Some(&mut camada));
        }
        let inicio = Instant::now();
        for _ in 0..opcoes.bench {
            state = update(state, Cmd::TickHud);
            desenhar_com(&mut painter, &mut state, Some(&mut camada));
        }
        let decorrido = inicio.elapsed().as_secs_f64();
        let ms = decorrido * 1000.0 / opcoes.bench as f64;
        let (pl, pa) = painter.painel();
        if std::env::var("GIOCHI_JSON").is_ok() {
            println!(
                "{{\"quadros\":{},\"ms\":{ms:.2},\"fps\":{:.1},\"desenhos\":{},\"brasas\":{},\"brilhos\":{},\"painel\":\"{pl}x{pa}\",\"efeitos\":{}}}",
                opcoes.bench,
                1000.0 / ms,
                camada.desenhos(),
                camada.brasas(),
                camada.brilhos(),
                camada.ativo()
            );
        } else {
            println!(
                "bench: {} quadros em {:.3} s — {ms:.2} ms/quadro ({:.1} fps) · {} desenho(s) · {} brasa(s), {} brilho(s) · efeitos {} · painel {pl}x{pa}",
                opcoes.bench,
                decorrido,
                1000.0 / ms,
                camada.desenhos(),
                camada.brasas(),
                camada.brilhos(),
                if camada.ativo() { "ligados" } else { "desligados" }
            );
        }
        return Ok(());
    }

    if let Some(destino) = &opcoes.dump_frame {
        painter.save_frame(destino)?;
        let (l, a) = painter.painel();
        eprintln!("quadro {l}x{a} gravado em {destino}");
        return Ok(());
    }

    // ── laço de uso ──────────────────────────────────────────
    //
    // Este é o modo que faz deste app **um launcher** e não só um banco de
    // prova de desenho: lê o controle por evdev, passa os comandos pelo
    // reducer do núcleo (o mesmo da produção) e, quando o núcleo pede para
    // lançar, fecha a janela, roda o jogo e reabre.
    //
    // Entrada por **evdev** e não por `SDL_GameController`, pela mesma razão do
    // launcher de produção: os botões dos handhelds vêm de `gpio-keys`, e a API
    // de gamepad do SDL devolve `nullptr` para eles — o app abriria, desenharia
    // e nenhum botão responderia, sem erro nenhum no log.
    let mapa = ButtonsMap::load(Path::new(input_evdev::DEFAULT_MAP_PATH));
    if !mapa.is_complete() {
        eprintln!("launcher-teste: aviso — o mapa de botões não está completo");
    }
    let mut evdev = EvdevInput::open(Path::new("/dev/input"), mapa.clone());
    eprintln!(
        "launcher-teste: {} dispositivo(s) de entrada",
        evdev.device_count()
    );
    let mut mapper = Mapper::new();
    if let Some(aviso) = &aviso_catalogo {
        eprintln!("launcher-teste: {aviso}");
        state.toast = giochi_launcher_core::Toast::show(aviso);
    }

    loop {
        // `poll()` move o estado atual dos dispositivos; `frame()` fotografa.
        // Os dois são necessários **por quadro**, e é o `mapper` que transforma
        // o quadro em comandos com borda e repetição — a mesma máquina de
        // estados do launcher de produção.
        evdev.poll();
        for cmd in mapper.update(evdev.frame()) {
            state = update(state, cmd);
        }

        // Pedido de lançamento: o núcleo decidiu *o que* abrir, e aqui é o
        // único lugar que sabe *como*.
        if let Some(id) = state.lancar.take() {
            let alvo = catalogo.alvo(id).cloned();
            let mut recado: Option<String> = None;

            // Destruir o pintor **antes** do filho é o que libera o DRM master.
            // Sem isto o jogo responde `kmsdrm not available` e o launcher
            // aparece "funcionando" enquanto nada abre.
            drop(painter);
            match alvo {
                Some(alvo) => match system::lancar_com_saida(&alvo, Some(&mapa)) {
                    Ok(system::Lancamento::EncerradoAMao) => {
                        recado =
                            Some("jogo encerrado à mão (START+B por 1,5 s)".to_string());
                    }
                    Ok(system::Lancamento::Terminou(_)) => {}
                    Err(e) => recado = Some(e),
                },
                None => {
                    recado = Some(
                        "este app não está no catálogo: sem caminho para executar".to_string(),
                    );
                }
            }
            painter = criar_painter(&platform, largura, altura, &fonte)?;
            if let Some(msg) = recado {
                eprintln!("launcher-teste: {msg}");
                state.toast = giochi_launcher_core::Toast::show(&msg);
            }
            // A camada de efeitos tem tamanho de tela dentro dela; a janela nova
            // pode ter outro tamanho (o jogo muda o modo do conector).
            let (l, a) = painter.logical_size();
            camada = Efeitos::novo(camada.ativo());
            if camada.ativo() {
                camada.semear(l as i32, a as i32);
            }
        }

        state = update(state, Cmd::TickHud);
        desenhar_com(&mut painter, &mut state, Some(&mut camada));
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}

/// Abre janela, renderer e pintor.
///
/// Em função porque **é chamada de novo** depois de cada jogo: o `Painter`
/// segura o DRM master, e ele tem de morrer antes de o filho abrir o KMSDRM.
/// Medido no X35H: um segundo processo com o launcher vivo recebe
/// `kmsdrm not available`. Reabrir a janela é o que o launcher de produção
/// também faz, e pelo mesmo motivo.
fn criar_painter(
    platform: &Platform,
    largura: u32,
    altura: u32,
    fonte: &str,
) -> Result<Painter, String> {
    let janela = platform
        .create_window(WindowConfig::new(largura, altura), "Giochi (engine 3D)")
        .map_err(|e| format!("janela: {e}"))?;
    let render = RenderContext::new(janela).map_err(|e| format!("renderer: {e}"))?;
    Painter::new(render, fonte)
}

/// A mesma sequência, com a camada de efeitos opcional por cima.
///
/// `None` é o caminho que a comparação pixel a pixel usa: sem camada, o quadro
/// tem de sair **idêntico** ao do launcher de produção — é o que prova que o
/// porte está fiel. `Some` entra depois do `hud()` e antes do `present()`, então
/// nada do desenho das telas muda.
fn desenhar_com(painter: &mut Painter, state: &mut State, mut efeitos: Option<&mut Efeitos>) {
    painter.clear(theme::BG);

    // Brilho ambiente no topo, na cor do jogo em foco, em faixas de alfa
    // decrescente. As medidas vêm de `Moldura`, não de 1280×720 fixo.
    if let Some(game) = state.focused_game() {
        let glow = theme::game_accent(game.id);
        let m = views::Moldura::de(painter);
        let faixa = m.e(22);
        let mut y = 0;
        let mut alpha = 46;
        while y < m.e(260) && alpha > 0 {
            painter.fill_rect(0, y, m.w, faixa, theme::with_alpha(glow, alpha as u8));
            y += faixa;
            alpha -= 4;
        }
    }

    let mut layout = views::Layout::default();
    views::chrome_top(painter, state, &mut layout);

    match state.tab {
        Tab::Home => views::home(painter, state, &mut layout),
        Tab::Library => views::library(painter, state, &mut layout),
        Tab::Apps => views::apps(painter, state, &mut layout),
        Tab::Settings => views::settings(painter, state, &mut layout),
    }

    views::chrome_foot(painter, state);
    views::overlays(painter, state, &mut layout);
    views::toast(painter, state);
    views::hud(painter, state);

    // O desenho é quem sabe quantas colunas cabem: publicar no estado é o que
    // faz o d-pad seguir a grade que está na tela, e não uma suposição.
    if let Some(grade) = layout.grade_biblioteca {
        state.grade_biblioteca = grade;
    }
    if let Some(grade) = layout.grade_apps {
        state.grade_apps = grade;
    }

    if let Some(camada) = efeitos.as_mut() {
        camada.desenhar(painter, state, &layout);
    }

    painter.present();
}

/// Catálogo lido do aparelho, ou vazio quando não há.
///
/// `GIOCHI_CATALOGO` aponta outro arquivo, como no launcher de produção — no PC
/// não existe `/opt/giochi/apps/` e testar exigiria root.
///
/// Devolve o catálogo **e** o aviso da falha, para o chamador poder dizer na
/// tela: um catálogo vazio por arquivo ausente é indistinguível de "nenhum jogo
/// instalado", e foram dois diagnósticos errados seguidos nesse mesmo caminho.
fn catalogo_do_aparelho() -> (catalogo::Catalogo, Option<String>) {
    let caminho = std::env::var_os("GIOCHI_CATALOGO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(catalogo::caminho);
    match catalogo::Catalogo::carregar(&caminho) {
        Ok(c) => (c, None),
        Err(e) => (
            catalogo::Catalogo::default(),
            Some(format!("catálogo: {e}")),
        ),
    }
}

fn estado_inicial(demo: bool, catalogo: &catalogo::Catalogo) -> State {
    let games = if demo {
        vec![
            Game::new(0, "Arcade Volleyball", "Esporte", 0.0, "nunca", true).com_ano(1988),
            Game::new(1, "Colossal Cave Adventure", "Aventura", 0.0, "nunca", false).com_ano(1976),
            Game::new(2, "Spacewar", "Ação", 0.0, "nunca", true).com_ano(1962),
            Game::new(3, "Shattered Pixel Dungeon", "Roguelike", 0.0, "nunca", false).com_ano(2014),
        ]
    } else {
        // Uma leitura só: o mesmo `catalogo` que monta os cartões resolve o
        // `alvo` no lançamento. Ler de novo no laço criaria duas listas que
        // podem discordar — o cartão na tela e o executável aberto sairiam de
        // fontes diferentes.
        eprintln!("launcher-teste: catálogo com {} jogo(s)", catalogo.games.len());
        catalogo.games.clone()
    };
    let apps = vec![
        AppEntry::new("music", "Música", "music", "Mídia"),
        AppEntry::new("retro", "RetroArch", "retro", "Emulador"),
        AppEntry::new("files", "Arquivos", "files", "Sistema"),
        AppEntry::new("term", "Terminal", "term", "Avançado"),
        AppEntry::new("browse", "Navegador", "browse", "Internet"),
        AppEntry::new("shots", "Capturas", "camera", "Mídia"),
        AppEntry::new("store", "Loja", "cart", "Sistema"),
        AppEntry::new("sync", "Sincronizar", "sync", "Nuvem"),
    ];
    let mut state = State::new(games, apps);
    state.screen = Screen::Running;

    // Os indicadores da barra superior vêm do hardware, e as duas máquinas da
    // comparação não têm o mesmo hardware: o X35H tem célula (mostra `78%`) e o
    // PC não (`—`). Congelar do **mesmo lado dos dois** é o que faz a comparação
    // medir desenho em vez de bateria — `aparelho.sh verificar-teste` exporta
    // estas variáveis para a captura e para a referência.
    if let Some(n) = env_num::<u8>("GIOCHI_BATERIA_FIXA") {
        state.battery = Some(n);
    }
    if let Some(t) = env_num::<u16>("GIOCHI_TEMPERATURA_FIXA") {
        state.hud.temp_c = t;
    }
    state
}

/// Lê `NOME=valor` do ambiente como número, ignorando texto inválido.
fn env_num<T: std::str::FromStr>(nome: &str) -> Option<T> {
    std::env::var(nome).ok()?.trim().parse().ok()
}

fn aplicar_tela(mut state: State, nome: &str) -> State {
    use giochi_launcher_core::Overlay;
    state.screen = Screen::Running;
    match nome {
        "home" => state.tab = Tab::Home,
        "biblioteca" | "library" => state.tab = Tab::Library,
        "apps" => state.tab = Tab::Apps,
        "ajustes" | "settings" => state.tab = Tab::Settings,
        // A busca é a única tela que usa recorte (`push_clip`): é ela que prova
        // que o recorte em software do porte se comporta como o `set_clip_rect`
        // do launcher de produção.
        "busca" | "search" => {
            state.tab = Tab::Home;
            state.search_query = "spa".into();
            state.overlays.push(Overlay::Search);
        }
        "rapido" | "quicksettings" => {
            state.tab = Tab::Home;
            state.overlays.push(Overlay::QuickSettings);
        }
        "detalhe" | "gamedetail" => {
            state.tab = Tab::Home;
            state.selected_game = Some(0);
            state.overlays.push(Overlay::GameDetail);
        }
        "energia" | "power" => {
            state.tab = Tab::Home;
            state.overlays.push(Overlay::PowerPopup);
        }
        "boot" => state.screen = Screen::Boot,
        outro => eprintln!("tela desconhecida: {outro} (use home|biblioteca|apps|ajustes|busca|rapido|detalhe|energia)"),
    }
    state
}

fn comando(nome: &str) -> Option<Cmd> {
    Some(match nome {
        "up" | "cima" => Cmd::Nav(Dir::Up),
        "down" | "baixo" => Cmd::Nav(Dir::Down),
        "left" | "esquerda" => Cmd::Nav(Dir::Left),
        "right" | "direita" => Cmd::Nav(Dir::Right),
        "a" | "confirm" | "enter" => Cmd::Confirm,
        "b" | "back" | "escape" | "voltar" => Cmd::Back,
        "x" | "opcoes" => Cmd::Options,
        "y" | "busca" => Cmd::Search,
        "l" | "l1" | "tabprev" => Cmd::TabPrev,
        "r" | "r1" | "tabnext" => Cmd::TabNext,
        "l2" | "subprev" => Cmd::SubPrev,
        "r2" | "subnext" => Cmd::SubNext,
        _ => return None,
    })
}

/// Tamanho do painel segundo o sistema, em paisagem.
///
/// O `virtual_size` do `fb0` é a fonte que o launcher de produção usa; num PC
/// sem `fb0` o padrão é 640×480, o mesmo do aparelho de menor painel.
fn tamanho_do_painel() -> (u32, u32) {
    let texto = std::fs::read_to_string("/sys/class/graphics/fb0/virtual_size").unwrap_or_default();
    if let Some((l, a)) = texto.trim().split_once(',') {
        if let (Ok(l), Ok(a)) = (l.trim().parse::<u32>(), a.trim().parse::<u32>()) {
            return (l, a);
        }
    }
    (640, 480)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn catalogo_com(slugs: &[&str]) -> catalogo::Catalogo {
        let mut c = catalogo::Catalogo::default();
        for (i, slug) in slugs.iter().enumerate() {
            c.games
                .push(Game::new(i as u32, slug, "Teste", 0.0, "nunca", false));
            c.alvos.push(catalogo::Alvo {
                slug: (*slug).to_string(),
                dir: format!("/tmp/{slug}"),
                entrypoint: "bin/x".to_string(),
            });
        }
        c
    }

    #[test]
    fn sem_demo_a_home_mostra_o_catalogo_do_aparelho() {
        // A regressão que este teste existe para pegar é o laço antigo, que
        // abria com `Estado::new(Vec::new(), apps)` — a tela desenhada é
        // idêntica, e a única diferença é que **nenhum jogo aparece**. Foi
        // assim que o launcher-teste passou a ser um banco de prova bonito e
        // inútil para ser o launcher.
        let cat = catalogo_com(&["neon-drift", "cobrinha"]);
        let s = estado_inicial(false, &cat);
        assert_eq!(s.games.len(), 2);
        assert_eq!(s.games[0].title, "neon-drift");
        assert_eq!(s.games[1].title, "cobrinha");
    }

    #[test]
    fn demo_nao_usa_o_catalogo() {
        // `--demo` é o caminho dos quatro jogos de demonstração; se ele
        // passasse a ler o catálogo, uma captura de tela no PC com um aparelho
        // na rede mediria coisas diferentes em cada máquina.
        let cat = catalogo_com(&["neon-drift"]);
        let s = estado_inicial(true, &cat);
        assert_eq!(s.games.len(), 4);
        assert_ne!(s.games[0].title, "neon-drift");
    }

    #[test]
    fn catalogo_vazio_nao_e_erro_de_tela() {
        // Aparelho sem `index.json` é o caso normal de uma imagem recém-gravada.
        // A Home desenha "Nenhum jogo instalado"; o que não pode acontecer é o
        // app entrar em pânico ou mostrar quatro jogos que não existem.
        let s = estado_inicial(false, &catalogo::Catalogo::default());
        assert!(s.games.is_empty());
        assert_eq!(s.screen, Screen::Running);
    }
}
