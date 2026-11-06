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
mod catalogo;
mod display;
mod draw;
mod energia;
mod icons;
mod input_evdev;
mod power;
mod semfio;
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
    // Espera antes de gravar o quadro, em milissegundos.
    //
    // Existe porque algumas telas só ficam prontas depois de trabalho
    // assíncrono: a lista de redes chega do `wpa_cli` uns 5 s depois de abrir.
    // Sem esta espera o BMP sai em "Procurando…" e a foto não prova nada sobre
    // a lista.
    let espera_ms: u64 = args
        .iter()
        .position(|a| a == "--esperar-ms")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let font_path = args
        .iter()
        .position(|a| a == "--font")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(default_font);

    // `--catalogo-adicionar <dir>`: põe um app instalado no índice do aparelho.
    //
    // Existe para o aparelho se catalogar **sozinho**, sem Node e sem PC. O
    // caminho natural num portátil é copiar o `.giochi` para o cartão e instalar
    // ali; exigir SSH + Node para o cartão aparecer na tela tornaria o cartão
    // inútil.
    //
    // Grava em arquivo temporário e move: um corte de energia no meio da escrita
    // deixaria JSON truncado no lugar do catálogo, e o launcher leria lixo.
    if let Some(i) = args.iter().position(|a| a == "--catalogo-adicionar") {
        let dir = args
            .get(i + 1)
            .ok_or("--catalogo-adicionar precisa da pasta do app")?;
        let caminho_meta = std::path::Path::new(dir).join("metadata.json");
        let texto_meta = std::fs::read_to_string(&caminho_meta)
            .map_err(|e| format!("{}: {e}", caminho_meta.display()))?;
        let meta: serde_json::Value =
            serde_json::from_str(&texto_meta).map_err(|e| format!("metadata inválido: {e}"))?;

        let caminho_indice = catalogo::caminho();
        let atual = std::fs::read_to_string(&caminho_indice).unwrap_or_else(|_| {
            // Catálogo ausente não é erro: é o primeiro app do aparelho.
            r#"{"schemaVersion":1,"apps":[]}"#.to_string()
        });

        let novo = catalogo::adicionar_ao_indice(&atual, dir, &meta)?;
        let temporario = caminho_indice.with_extension("json.novo");
        std::fs::write(&temporario, novo)
            .map_err(|e| format!("{}: {e}", temporario.display()))?;
        std::fs::rename(&temporario, &caminho_indice)
            .map_err(|e| format!("não consegui mover para {}: {e}", caminho_indice.display()))?;

        let quantos = serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(&caminho_indice).unwrap_or_default(),
        )
        .ok()
        .and_then(|v| v.get("apps").and_then(|a| a.as_array()).map(|a| a.len()))
        .unwrap_or(0);
        eprintln!("catálogo: {caminho_indice:?} agora com {quantos} app(s)");
        return Ok(());
    }

    // `--simbolos`: imprime o que o tradutor de `buttons.map` entende e sai.
    //
    // Serve para verificar o mapa de um aparelho **sem o aparelho**: se o mapa
    // declara um símbolo que não está nesta lista, o botão é descartado em
    // silêncio (aconteceu duas vezes, ver AGENTS.md). A lista é a do próprio
    // tradutor, não uma cópia — `scripts/verificar-mapa.mjs` compara com ela.
    if args.iter().any(|a| a == "--simbolos") {
        for simbolo in input_evdev::SIMBOLOS_CONHECIDOS {
            println!("{simbolo}");
        }
        return Ok(());
    }

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

    // `--tela` existe para poder olhar QUALQUER tela num tamanho de painel que
    // não temos em mãos. Sem ele, `--dump-frame` só mostra a Home e a adaptação
    // das outras telas fica sem como conferir.
    let tela_inicial = match args.iter().position(|a| a == "--tela") {
        Some(i) => Some(
            args.get(i + 1)
                .ok_or("--tela precisa de um nome")?
                .to_ascii_lowercase(),
        ),
        None => None,
    };

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
    informar_projecao(&painter);

    // O catálogo vem do sistema. Sem ele, o launcher mostra o catálogo de
    // demonstração — o aparelho continua utilizável, mas nenhum app é
    // iniciável, porque não há caminho para executar.
    let (mut state, catalogo) = {
        let caminho = catalogo::caminho();
        match catalogo::Catalogo::carregar(&caminho) {
            Ok(c) => {
                // Cartão removível pode não estar montado neste boot. Podar em
                // tempo de execução é o que impede um cartão ausente de virar
                // cartão que aparece e não abre.
                let c = c.podar_sem_binario(|slug, caminho| {
                    eprintln!("catálogo: {slug} fora — {caminho} não existe (cartão removido?)");
                });
                if c.vazio() {
                    eprintln!("catálogo: nenhum app com binário presente; usando demonstração");
                    (State::new(demo_games(), demo_apps()), None)
                } else {
                    eprintln!("catálogo: {} app(s) de {}", c.games.len(), caminho.display());
                    (State::new(c.games.clone(), demo_apps()), Some(c))
                }
            }
            Err(e) => {
                eprintln!("catálogo: {e}; usando demonstração");
                (State::new(demo_games(), demo_apps()), None)
            }
        }
    };

    if let Some(tela) = tela_inicial.as_deref() {
        abrir_tela(&mut state, tela)?;
        eprintln!("tela forcada por --tela: {tela}");
    }

    // `--sem-radio`: finge um aparelho sem Wi-Fi nem Bluetooth.
    //
    // Existe porque não há R35S na mão e ele **não tem rádio**: a única forma de
    // ver a tela que o usuário daquele aparelho vai ver é simular a capacidade
    // ausente. Sem esta chave, a verificação do R35S no lado do app ficaria
    // esperando hardware que talvez não venha.
    if args.iter().any(|a| a == "--sem-radio") {
        state = giochi_launcher_core::update(
            state,
            giochi_launcher_core::Cmd::CapacidadesSemFio {
                wifi: false,
                bluetooth: false,
            },
        );
        eprintln!("--sem-radio: aparelho simulado sem rádio (perfil R35S)");
    }

    // `--rede-real`: descarta o exemplo e pede uma varredura de verdade.
    //
    // As telas de rede abrem com dados de exemplo para fotografar o layout sem
    // hardware. Isso é bom para olhar o desenho e péssimo para provar que o
    // `wpa_cli` funciona: a foto mostraria números inventados e chamaria de
    // sucesso. Este sinal limpa o exemplo e liga a varredura real.
    if args.iter().any(|a| a == "--rede-real") {
        state.sem_fio.redes.clear();
        state.sem_fio.dispositivos.clear();
        state.sem_fio.conectado = None;
        state.sem_fio.erro = None;
        state.sem_fio.escaneando = true;
        state.sem_fio.bt_escaneando = true;
        eprintln!("--rede-real: varredura de verdade pedida");
    }

    // `--foco zona:indice`: coloca o destaque onde o teste precisa vê-lo.
    //
    // Existe porque um quadro parado no estado inicial nunca mostra o anel de
    // foco da tela fotografada — e foi justamente a falta de foco no painel de
    // configurações a queixa do usuário. Sem isto, "tem anel" e "não tem anel"
    // produzem o mesmo BMP.
    if let Some(i) = args.iter().position(|a| a == "--foco") {
        let pedido = args.get(i + 1).ok_or("--foco precisa de zona:indice")?;
        let (zona, indice) = pedido
            .split_once(':')
            .ok_or_else(|| format!("--foco espera zona:indice, veio {pedido}"))?;
        let zona = zona_com_nome(zona).ok_or_else(|| format!("--foco: zona desconhecida: {zona}"))?;
        let indice: usize = indice
            .parse()
            .map_err(|_| format!("--foco: índice inválido: {indice}"))?;
        state.focus = giochi_launcher_core::Focus::at(zona, indice);
        // Andar na coluna troca a categoria no reducer (`nav`); o flag tem de
        // fazer o mesmo, senão o destaque e o conteúdo divergem — que é
        // exatamente o estado que ele deveria ajudar a investigar.
        if zona == giochi_launcher_core::Zone::SettingsSidebar
            && indice < giochi_launcher_core::SETTINGS_CATEGORIES.len()
        {
            state.settings_category = indice;
        }
        eprintln!(
            "foco forcado por --foco: {zona:?} {indice} (categoria {})",
            state.settings_category
        );
    }

    // `--categoria <nome|indice>`: escolhe a categoria das configurações.
    //
    // `--tela ajustes` só troca a aba; a categoria fica na primeira. Sem esta
    // chave não havia como fotografar "Som" nem exercitar uma linha ajustável
    // — e `--foco categorias:N` sozinho era **enganoso**: punha o destaque na
    // coluna sem mudar `settings_category`, então o `A` abria a linha da
    // categoria anterior. O diagnóstico media outro estado e chamava de ok.
    if let Some(i) = args.iter().position(|a| a == "--categoria") {
        let pedido = args.get(i + 1).ok_or("--categoria precisa de um nome")?;
        let indice = pedido
            .parse::<usize>()
            .ok()
            .filter(|n| *n < giochi_launcher_core::SETTINGS_CATEGORIES.len())
            .or_else(|| {
                giochi_launcher_core::SETTINGS_CATEGORIES
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(pedido))
            })
            .ok_or_else(|| {
                format!(
                    "--categoria: {pedido} desconhecida\nuse: {}",
                    giochi_launcher_core::SETTINGS_CATEGORIES.join(", ")
                )
            })?;
        state.settings_category = indice;
        state.tab = giochi_launcher_core::Tab::Settings;
        eprintln!("--categoria: {pedido} (indice {indice})");
    }

    // `--teclas "left,left,up"`: despacha uma sequência de comandos.
    //
    // Existe porque a navegação não tem como ser provada de fora: `--foco`
    // posiciona o destaque, mas não consegue mostrar o **resultado de andar**.
    // Foi justamente uma seta que não voltava ("não consigo voltar") que
    // passou por 169 testes verdes — o teste olhava o estado, não o caminho.
    //
    // Aceita só teclas: é diagnóstico de navegação, não um interpretador.
    // As teclas são **traduzidas** aqui e aplicadas depois, com o `App` já
    // montado e um quadro desenhado.
    //
    // A ordem não é detalhe: o arranjo da grade (colunas e itens por página) é
    // publicado pelo desenho. Aplicando as teclas antes do primeiro quadro, a
    // navegação usaria o padrão do estado — 4 colunas — que é exatamente o
    // número errado que causou o bug, e a ferramenta mostraria o comportamento
    // antigo como se fosse o atual.
    let sequencia: Option<(String, Vec<giochi_launcher_core::Cmd>)> =
        if let Some(i) = args.iter().position(|a| a == "--teclas") {
            let lista = args.get(i + 1).ok_or("--teclas precisa de uma lista")?;
            let mut cmds = Vec::new();
            for tecla in lista.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
                cmds.push(match tecla.to_ascii_lowercase().as_str() {
                    "up" | "cima" => giochi_launcher_core::Cmd::Nav(giochi_launcher_core::Dir::Up),
                    "down" | "baixo" => giochi_launcher_core::Cmd::Nav(giochi_launcher_core::Dir::Down),
                    "left" | "esquerda" => giochi_launcher_core::Cmd::Nav(giochi_launcher_core::Dir::Left),
                    "right" | "direita" => giochi_launcher_core::Cmd::Nav(giochi_launcher_core::Dir::Right),
                    "a" | "confirmar" => giochi_launcher_core::Cmd::Confirm,
                    "l2" | "subprev" => giochi_launcher_core::Cmd::SubPrev,
                    "r2" | "subnext" => giochi_launcher_core::Cmd::SubNext,
                    "l" | "tabprev" => giochi_launcher_core::Cmd::TabPrev,
                    "r" | "tabnext" => giochi_launcher_core::Cmd::TabNext,
                    "b" | "voltar" => giochi_launcher_core::Cmd::Back,
                    outra => return Err(format!("--teclas: tecla desconhecida: {outra}")),
                });
            }
            Some((lista.clone(), cmds))
        } else {
            None
        };

    // `--lancar <id|slug>`: pede o lançamento já na abertura.
    //
    // Existe porque o caminho "abrir display → rodar app → reabrir" não tem
    // como ser exercitado de fora: `/dev/fb0` devolve lixo sob KMSDRM e
    // `--dump-frame` abre processo novo. Sem isto, verificar o lançamento
    // exigiria um dedo no aparelho, e verificação que depende de dedo não
    // entra em rodada automatizada.
    //
    // Escreve em `state.lancar`, que é exatamente o campo que o reducer do
    // botão "Jogar" preenche — então o teste percorre o mesmo código, não uma
    // cópia dele.
    if let Some(i) = args.iter().position(|a| a == "--lancar") {
        let pedido = args.get(i + 1).ok_or("--lancar precisa de id ou slug")?;
        let id = pedido
            .parse::<u32>()
            .ok()
            .filter(|id| (*id as usize) < state.games.len())
            .or_else(|| {
                catalogo.as_ref().and_then(|c| {
                    c.alvos.iter().position(|a| &a.slug == pedido).map(|p| p as u32)
                })
            })
            .ok_or_else(|| format!("--lancar: app desconhecido: {pedido}"))?;
        state.lancar = Some(id);
        eprintln!("--lancar: pedido de lançamento para o app {id} ({pedido})");
    }

    // Entrada por evdev: o SDL fica só com a janela e o desenho.
    let map = input_evdev::ButtonsMap::load(std::path::Path::new(
        input_evdev::DEFAULT_MAP_PATH,
    ));
    if !map.is_complete() {
        eprintln!(
            "aviso: mapa de botões incompleto; usando o padrão embutido onde faltar"
        );
    }
    let evdev = input_evdev::EvdevInput::open(std::path::Path::new("/dev/input"), map.clone());
    if evdev.device_count() == 0 {
        eprintln!(
            "aviso: nenhum dispositivo em /dev/input — o controle não responderá.\n\
             No PC, isso é esperado se você não tem controle conectado."
        );
    }

    let mut app = app::App::new(painter, state, evdev);

    // `--teclas`: um quadro primeiro (para o desenho publicar o arranjo da
    // grade), depois a sequência pelo mesmo `dispatch` da entrada real.
    if let Some((lista, cmds)) = sequencia {
        app.render();
        for cmd in cmds {
            app.aplicar(cmd);
            // Um quadro por comando, como no laço real: é o desenho que publica
            // o arranjo da grade, e ele muda quando a aba ou o filtro muda.
            app.render();
        }
        // O filtro e a categoria entram no log porque "o gatilho trocou de
        // sub-aba" é exatamente o que se quer provar — e o foco sozinho não
        // mostra isso (ele fica na grade de propósito).
        let estado = app.estado();
        eprintln!(
            "--teclas: {} -> foco {:?} {} | filtro {} | categoria {} | grade {}x{}",
            lista,
            estado.focus.zone,
            estado.focus.index,
            estado.library_filter.label(),
            giochi_launcher_core::SETTINGS_CATEGORIES
                .get(estado.settings_category)
                .copied()
                .unwrap_or("?"),
            estado.grade_biblioteca.cols(),
            estado.grade_biblioteca.por_pagina()
        );

        // Sai depois de imprimir: `--teclas` é diagnóstico, e ficar no laço
        // dependia de um `timeout` que o aparelho não tem (`sh: timeout: not
        // found`) — a ferramenta travava em vez de responder. Com
        // `--dump-frame` o laço continua, porque a foto é o resultado.
        // Só sai quando não há outro diagnóstico pendente: `--lancar` e
        // `--conectar` acontecem **no laço**, e sair aqui os engoliria em
        // silêncio.
        let tem_lancamento = app.estado().lancar.is_some();
        let tem_rede = args
            .iter()
            .any(|a| a == "--conectar" || a == "--parear");
        if dump_frame.is_none() && !tem_lancamento && !tem_rede {
            return Ok(());
        }
    }

    // `--conectar ssid[:senha]` e `--parear mac`: exercitam o caminho de rede
    // sem dedo na tela. Imprimem o resultado e saem — são diagnóstico, não uso
    // normal, e por isso não esperam o laço de entrada.
    if let Some(i) = args.iter().position(|a| a == "--conectar") {
        let pedido = args.get(i + 1).ok_or("--conectar precisa de ssid[:senha]")?;
        let (ssid, senha) = pedido.split_once(':').unwrap_or((pedido, ""));
        eprintln!("--conectar: {ssid} (senha de {} caractere(s))", senha.chars().count());
        app.pedir_sem_fio(semfio::Pedido::Conectar {
            ssid: ssid.to_string(),
            senha: senha.to_string(),
        });
    }
    if let Some(i) = args.iter().position(|a| a == "--parear") {
        let mac = args.get(i + 1).ok_or("--parear precisa de um MAC")?;
        eprintln!("--parear: {mac}");
        app.pedir_sem_fio(semfio::Pedido::Parear {
            mac: mac.to_string(),
        });
    }
    let diagnosticando = args.iter().any(|a| a == "--conectar") || args.iter().any(|a| a == "--parear");

    let mut events = sdl.event_pump()?;
    let mut last = Instant::now();
    let inicio = Instant::now();

    'outer: loop {
        for event in events.poll_iter() {
            if !app.handle_event(&event) {
                break 'outer;
            }
        }

        // Pedido de lançamento: o núcleo decidiu *o que* abrir, e aqui é o
        // único lugar que sabe *como* — fechar o display, rodar, reabrir.
        //
        // Antes desta checagem o botão "Jogar" só mostrava um aviso. Não havia
        // como testar a integração porque não havia jogo; agora há
        // (`app/jogos/volley`), e sem este trecho ele continuaria inalcançável.
        if let Some(id) = app.tomar_pedido_de_lancamento() {
            let alvo = catalogo.as_ref().and_then(|c| c.alvo(id)).cloned();

            // Consome o `App`: o `Painter` morre aqui e o DRM master é liberado.
            // Sem isso o filho recebe `kmsdrm not available` — medido no X35H.
            let estado = app.encerrar();

            let mut recado: Option<String> = None;
            match alvo {
                Some(alvo) => {
                    if let Err(e) = system::lancar(&alvo) {
                        recado = Some(e);
                    }
                }
                None => {
                    recado = Some(
                        "este app não está no catálogo do sistema: sem caminho para executar"
                            .to_string(),
                    );
                }
            }

            // Reabre a tela — o jogo pode ter deixado o conector em qualquer
            // modo, então a janela é criada do zero, não "reaproveitada".
            let painter = criar_painter(&video, largura, altura, fullscreen, &font_path)?;
            informar_projecao(&painter);
            let evdev = input_evdev::EvdevInput::open(
                std::path::Path::new("/dev/input"),
                map.clone(),
            );
            app = app::App::new(painter, estado, evdev);
            if let Some(msg) = recado {
                app.avisar(&msg);
            }
            last = Instant::now();
        }

        let now = Instant::now();
        let delta = now.duration_since(last).as_secs_f32();
        last = now;

        app.tick(delta.min(0.1));
        app.render();

        // Diagnóstico de rede: o veredito é o aviso na tela, então espera ele
        // aparecer (ou desiste) e encerra com o texto no stderr.
        if diagnosticando {
            if let Some(msg) = app.ultimo_aviso() {
                eprintln!("rede: {msg}");
                return Ok(());
            }
            if inicio.elapsed().as_secs() > 40 {
                eprintln!("rede: sem resposta em 40 s");
                return Ok(());
            }
        }

        if let Some(path) = dump_frame.as_deref()
            .filter(|_| inicio.elapsed().as_millis() as u64 >= espera_ms)
        {
            // Com `--tela` a tela pedida pode não ser `Running` (sono, desligar).
            // Nesse caso grava no primeiro quadro, senão o laço nunca termina.
            let alvo = app.state.screen == giochi_launcher_core::Screen::Running
                || tela_inicial.is_some();
            if alvo {
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

/// Cria janela e painter. Existe como função porque isso acontece **duas
/// vezes**: na abertura e a cada volta de um jogo.
fn criar_painter(
    video: &sdl2::VideoSubsystem,
    largura: u32,
    altura: u32,
    fullscreen: bool,
    font_path: &str,
) -> Result<draw::Painter, String> {
    let mut builder = video.window("Giochi Launcher", largura, altura);
    if fullscreen {
        builder.fullscreen_desktop();
    }
    let window = builder
        .position_centered()
        .resizable()
        .build()
        .map_err(|e| format!("janela: {e}"))?;
    draw::Painter::new(window, font_path)
}

/// Registra no log o par painel × projeto.
///
/// O projeto acompanha o painel (ver `draw::projecao`): a escala é sempre 1,0 e
/// o layout é que se adapta. Registrar o par evita confundir "projeto menor" com
/// bug de desenho.
///
/// O painel vem do `Painter`, não do tamanho pedido: sob `kmsdrm` o SDL ignora o
/// pedido e usa o modo do conector, e anunciar o pedido produzia
/// "output 1280x720 desenha em projeto 640x480" — três números que se
/// contradizem na mesma linha.
fn informar_projecao(painter: &draw::Painter) {
    let (dw, dh) = painter.logical_size();
    if (dw, dh) == (draw::DESIGN_W as u32, draw::DESIGN_H as u32) {
        return;
    }
    let m = views::Moldura::de(painter);
    let (ow, oh) = painter.painel();
    eprintln!(
        "output {ow}x{oh} desenha em projeto {dw}x{dh} \
         (escala 1,0; densidade {:.2}{})",
        m.k,
        if m.compacto { ", compacta" } else { "" }
    );
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

/// Leva o estado a uma tela nomeada, para `--tela` poder fotografá-la.
///
/// Os nomes são os das telas que existem, não invenção: cada valor precisa ter
/// um `match` em `app::render`. Um nome desconhecido falha com a lista, em vez
/// de desenhar a Home e fingir que atendeu.
fn abrir_tela(state: &mut State, nome: &str) -> Result<(), String> {
    use giochi_launcher_core::{Overlay, Screen, Tab};
    // Toda tela é fotografada em `Screen::Running`: é o único estado em que o
    // laço de `--dump-frame` grava.
    state.screen = Screen::Running;
    match nome {
        "home" => state.tab = Tab::Home,
        "biblioteca" | "library" => state.tab = Tab::Library,
        "apps" => state.tab = Tab::Apps,
        "ajustes" | "settings" => state.tab = Tab::Settings,
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
        "sono" | "sleeping" => state.screen = Screen::Sleeping,
        // Telas de rede. Vêm com dados de exemplo porque a lista vazia esconde
        // justamente o que interessa conferir: truncamento de SSID longo,
        // muitas redes empilhadas, senha comprida demais para o campo.
        "wifi" => {
            state.tab = giochi_launcher_core::Tab::Settings;
            state.settings_category = 2;
            state.sem_fio = exemplo_redes();
            state.overlays.push(Overlay::WifiList);
        }
        "senha" | "wifisenha" => {
            state.tab = giochi_launcher_core::Tab::Settings;
            state.settings_category = 2;
            state.sem_fio = exemplo_redes();
            state.sem_fio.alvo = Some("Kill.exe".into());
            state.sem_fio.senha = "senha-de-exemplo".into();
            state.overlays.push(Overlay::WifiPassword);
        }
        "bluetooth" | "bt" => {
            state.tab = giochi_launcher_core::Tab::Settings;
            state.settings_category = 2;
            state.sem_fio = exemplo_redes();
            state.overlays.push(Overlay::BtList);
        }
        outro => {
            return Err(format!(
                "tela desconhecida: {outro}\n\
                 use: home, biblioteca, apps, ajustes, busca, rapido, detalhe, \
                 energia, sono, wifi, senha, bluetooth"
            ))
        }
    }
    Ok(())
}

/// Nome de zona aceito em `--foco`.
///
/// A lista é explícita, não um `Debug` parseado: renomear uma variante não deve
/// mudar em silêncio o que os scripts de verificação pedem.
fn zona_com_nome(nome: &str) -> Option<giochi_launcher_core::Zone> {
    use giochi_launcher_core::Zone;
    Some(match nome.to_ascii_lowercase().as_str() {
        "home" | "carrossel" => Zone::HomeCarousel,
        "filtros" => Zone::LibraryFilters,
        "biblioteca" | "grade" => Zone::LibraryGrid,
        "apps" => Zone::AppsGrid,
        "categorias" => Zone::SettingsSidebar,
        "ajustes" | "conteudo" => Zone::SettingsContent,
        "wifi" => Zone::WifiNetworks,
        "senha" => Zone::PasswordKeys,
        "bluetooth" | "bt" => Zone::BtDevices,
        "teclado" => Zone::SearchKeyboard,
        "resultados" => Zone::SearchResults,
        "rapido" => Zone::QuickSettings,
        "detalhe" => Zone::GameDetailActions,
        "energia" => Zone::PowerActions,
        _ => return None,
    })
}

/// Redes e aparelhos de exemplo para fotografar as telas sem hardware.
///
/// Os nomes vêm de uma varredura real do X55 (06/11/2026) — inclusive um SSID
/// longo, que é o que testa o truncamento, e uma rede sem nome.
fn exemplo_redes() -> giochi_launcher_core::SemFio {
    use giochi_launcher_core::{DispositivoBt, Rede, SemFio};
    SemFio {
        redes: vec![
            Rede {
                ssid: "Kill.exe".into(),
                sinal: 100,
                protegida: true,
                salva: true,
            },
            Rede {
                ssid: "ISABELA_2.4GHz".into(),
                sinal: 74,
                protegida: true,
                salva: false,
            },
            Rede {
                ssid: "ORLEANS".into(),
                sinal: 60,
                protegida: true,
                salva: false,
            },
            Rede {
                ssid: "Cafe Aberto Sem Senha".into(),
                sinal: 48,
                protegida: false,
                salva: false,
            },
        ],
        conectado: Some("Kill.exe".into()),
        dispositivos: vec![
            DispositivoBt {
                mac: "AC:12:2F:34:56:78".into(),
                nome: "Controle Sem Fio".into(),
                pareado: true,
            },
            DispositivoBt {
                mac: "52:79:B8:05:84:F2".into(),
                nome: "aparelho sem nome".into(),
                pareado: false,
            },
        ],
        ..SemFio::default()
    }
}

/// Catálogo de demonstração.
///
/// São os quatro jogos que existem no projeto — o protótipo listava títulos
/// comerciais que não estão no catálogo. A leitura real virá de
/// `apps.json` + `/apps` no aparelho.
fn demo_games() -> Vec<Game> {
    // Os anos são os do protótipo (`launcher.html`): eles existem como dado do
    // jogo, não como decoração — é o que aparece no cartão e na ficha.
    vec![
        Game::new(0, "Arcade Volleyball", "Esporte", 0.0, "nunca", true).com_ano(1988),
        Game::new(1, "Colossal Cave Adventure", "Aventura", 0.0, "nunca", false).com_ano(1976),
        Game::new(2, "Spacewar", "Ação", 0.0, "nunca", true).com_ano(1962),
        Game::new(3, "Shattered Pixel Dungeon", "Roguelike", 0.0, "nunca", false).com_ano(2014),
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
