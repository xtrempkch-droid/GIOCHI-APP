//! Tocador de música do Giochi.
//!
//! O app que faltava no catálogo: até aqui os sete apps do repositório eram
//! **host** (rodam no PC) ou **tema** (são a própria interface), e o catálogo do
//! aparelho saía vazio por construção. Este é o primeiro app `image` que não é
//! interface — ele aparece na aba de apps do launcher e abre de lá.
//!
//! # O que ele faz
//!
//! Lê uma pasta (`/user/musica` no aparelho, `~/Música` no PC), lista as faixas
//! em ordem natural, desenha um carrossel de capas **geradas do nome do
//! arquivo** e toca pelo mixer da engine — com pausa, parada, próxima/anterior,
//! volume e relatório de erro na tela.
//!
//! # As três camadas, e por que separadas
//!
//! | Camada | Onde | Sabe o quê |
//! | --- | --- | --- |
//! | decisão | `crates/core` | ordem, foco, o que tocar, layout, rotação — **sem SDL**, 120 testes |
//! | desenho | `src/pintor.rs`, `src/tela.rs` | pixels, glifos, textura |
//! | mundo | `src/audio.rs`, `src/entrada.rs` | mixer e `/dev/input` |
//!
//! A divisão não é estilo: é o que permite `cargo test` no CI, onde não há SDL,
//! GPU, aparelho nem o repositório irmão da engine.
//!
//! # Flags de diagnóstico
//!
//! ```text
//! --pasta <dir>     pasta de música (padrão: /user/musica ou ~/Música)
//! --demo            biblioteca sintética, sem disco (para capturar tela)
//! --tocar <n>       começa tocando a faixa n
//! --volume <0-100>  volume inicial
//! --sem-som         não abre o mixer (mede só o desenho)
//! --tamanho LxA     força o tamanho da janela
//! --dump-frame <f>  grava um BMP e sai
//! --teclas <lista>  aplica ações e sai (ex.: right,right,a)
//! --bench <n>       mede ms/quadro e desenhos
//! ```

mod arte;
mod audio;
mod entrada;
mod pintor;
mod tela;

// A paleta e os glifos são os **mesmos arquivos** do launcher de produção, por
// `#[path]`. Uma segunda paleta divergiria na primeira correção, e o app de
// música passaria a ter outro azul que o resto do sistema.
// Os dois trazem itens que só o launcher usa (cores de perigo, ícones de
// ajuste, tamanhos de barra dele). O `allow` é **local ao módulo**: o código
// deste app continua sendo verificado, e a alternativa — copiar e apagar o que
// sobra — é como nasceram as cópias atrasadas do Star Dust.
#[allow(dead_code)]
#[path = "../../launcher/crates/ui-sdl/src/theme.rs"]
mod theme;
#[allow(dead_code)]
#[path = "../../launcher/crates/ui-sdl/src/icons.rs"]
mod icones;

use std::path::Path;
use std::time::Instant;

use giochi_engine::audio::MusicState;
use giochi_engine::{Platform, RenderContext, WindowConfig};
use giochi_musica_core::{
    montar, pasta_padrao, Acao, Biblioteca, Entrada, Estado, Efeito, Faixa, Vazio,
};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use audio::Som;
use entrada::{AcaoDoTeclado, Entrada as Leitor};
use pintor::{Pintor, CANDIDATOS_FONTE};
use tela::Telas;

fn main() {
    if let Err(e) = principal() {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

#[derive(Default)]
struct Opcoes {
    pasta: Option<String>,
    demo: bool,
    tamanho: Option<(u32, u32)>,
    fonte: Option<String>,
    dump_frame: Option<String>,
    teclas: Option<String>,
    tocar: Option<usize>,
    volume: Option<i32>,
    bench: u32,
    sem_som: bool,
    testar_som: f32,
}

fn argumentos() -> Result<Opcoes, String> {
    let mut o = Opcoes::default();
    // Aceita as duas formas: `--tamanho 640x480` e `--tamanho=640x480`. A
    // segunda é a que sai naturalmente de um script, e recusá-la produzia
    // "argumento desconhecido: --tamanho=640x480" — erro que manda procurar no
    // lugar errado, porque a flag existe.
    let mut argv: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.strip_prefix("--").and_then(|resto| resto.split_once('=')) {
            Some((flag, valor)) => {
                argv.push(format!("--{flag}"));
                argv.push(valor.to_string());
            }
            None => argv.push(arg),
        }
    }
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        let mut proximo = || {
            i += 1;
            argv.get(i).cloned()
        };
        match a.as_str() {
            "--pasta" => o.pasta = proximo(),
            "--demo" => o.demo = true,
            "--tamanho" => {
                let valor = proximo().ok_or("--tamanho precisa de LxA")?;
                let (l, a) = valor
                    .split_once('x')
                    .ok_or_else(|| format!("--tamanho inválido: {valor} (use 640x480)"))?;
                let l: u32 = l.parse().map_err(|_| format!("largura inválida: {l}"))?;
                let a: u32 = a.parse().map_err(|_| format!("altura inválida: {a}"))?;
                o.tamanho = Some((l, a));
            }
            "--fonte" => o.fonte = proximo(),
            "--dump-frame" => o.dump_frame = proximo(),
            "--teclas" => o.teclas = proximo(),
            "--tocar" => {
                let valor = proximo().ok_or("--tocar precisa de um índice")?;
                o.tocar = Some(valor.parse().map_err(|_| format!("--tocar inválido: {valor}"))?);
            }
            "--volume" => {
                let valor = proximo().ok_or("--volume precisa de um valor")?;
                o.volume = Some(valor.parse().map_err(|_| format!("--volume inválido: {valor}"))?);
            }
            "--bench" => {
                let valor = proximo().unwrap_or_else(|| "300".to_string());
                o.bench = valor.parse().map_err(|_| format!("--bench inválido: {valor}"))?;
            }
            "--sem-som" => o.sem_som = true,
            "--testar-som" => {
                let valor = proximo().unwrap_or_else(|| "6".to_string());
                o.testar_som = valor
                    .parse()
                    .map_err(|_| format!("--testar-som inválido: {valor}"))?;
            }
            "-h" | "--ajuda" => {
                ajuda();
                std::process::exit(0);
            }
            outro => return Err(format!("argumento desconhecido: {outro} (use --ajuda)")),
        }
        i += 1;
    }
    Ok(o)
}

fn ajuda() {
    println!("giochi-musica — tocador de música do Giochi");
    println!();
    println!("  --pasta <dir>      pasta de música (padrão /user/musica ou ~/Música)");
    println!("  --demo             biblioteca sintética, sem ler disco");
    println!("  --tocar <n>        começa tocando a faixa n");
    println!("  --volume <0-100>   volume inicial (padrão 70)");
    println!("  --sem-som          não abre o mixer");
    println!("  --tamanho LxA      força o tamanho da janela");
    println!("  --dump-frame <f>   grava um BMP e sai");
    println!("  --teclas <lista>   aplica ações e sai (left,right,a,x,y,b,lb,rb,reload)");
    println!("  --bench <n>        mede ms/quadro");
    println!("  --testar-som <s>   toca a faixa em foco e acompanha o mixer por s segundos");
    println!();
    println!("Teclado: ←→ escolher · espaço/Enter tocar · N/P faixa · S parar · ↑↓ volume · R reler · Esc sair");
}

fn principal() -> Result<(), String> {
    let opcoes = argumentos()?;

    let (largura, altura) = opcoes.tamanho.unwrap_or_else(tamanho_do_painel);
    eprintln!("giochi-musica {}: painel pedido {largura}x{altura}", giochi_musica_core::VERSAO);

    let platform = Platform::new().map_err(|e| format!("SDL: {e}"))?;
    let fonte = opcoes
        .fonte
        .clone()
        .or_else(|| {
            CANDIDATOS_FONTE
                .iter()
                .find(|c| Path::new(c).exists())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| format!("nenhuma fonte encontrada em: {}", CANDIDATOS_FONTE.join(", ")))?;

    let mut pintor = criar_pintor(&platform, largura, altura, &fonte)?;

    let pasta = opcoes
        .pasta
        .clone()
        .unwrap_or_else(|| pasta_padrao(std::env::var("GIOCHI_MUSICA").ok().as_deref(), |p| Path::new(p).is_dir(), &home()));

    let biblioteca = if opcoes.demo {
        eprintln!("biblioteca: demonstração (--demo)");
        demo()
    } else {
        let b = ler_pasta(&pasta);
        eprintln!("biblioteca: {} — {}", pasta, b.mensagem());
        b
    };

    let volume = opcoes.volume.unwrap_or(giochi_musica_core::Transporte::VOLUME_PADRAO);
    let mut estado = Estado::novo(biblioteca).com_volume(volume);
    let mut telas = Telas::nova();
    let mut som = Som::abrir(&platform, estado.transporte.volume, !opcoes.sem_som);

    // Pedido de tocar na abertura: é o mesmo caminho do `A`, então exercita o
    // mixer de verdade — e permite medir o áudio por SSH sem apertar botão.
    if let Some(indice) = opcoes.tocar {
        if indice < estado.biblioteca.total() {
            estado.aplicar(Acao::Focar(indice));
            let efeito = estado.aplicar(Acao::Alternar);
            if let Consequencia::Recado(r) = executar(
                efeito,
                &mut estado,
                &mut som,
                &pasta,
                &mut telas,
                &mut pintor,
            ) {
                eprintln!("erro: {r}");
                return Err(r);
            }
            eprintln!("--tocar: faixa {indice} enviada ao mixer");
        } else {
            return Err(format!(
                "--tocar {indice}: a biblioteca tem {} faixa(s)",
                estado.biblioteca.total()
            ));
        }
    }

    // ── áudio de verdade ─────────────────────────────────────
    //
    // Diagnóstico de campo: toca a faixa em foco e **acompanha o estado do
    // mixer** até ele parar. É o que distingue "o mixer aceitou a faixa"
    // (`play_music` devolveu Ok) de "o áudio está correndo": se a faixa de
    // 2,5 s vira `Stopped` sozinha em ~2,5 s, o mixer está consumindo o fluxo.
    // Sem esta prova, "o som funciona" seria afirmação, não medição.
    if opcoes.testar_som > 0.0 {
        if estado.biblioteca.esta_vazia() {
            return Err("--testar-som: não há faixa para tocar".to_string());
        }
        let efeito = estado.aplicar(Acao::Alternar);
        if let Consequencia::Recado(r) = executar(
            efeito,
            &mut estado,
            &mut som,
            &pasta,
            &mut telas,
            &mut pintor,
        ) {
            return Err(r);
        }
        let limite = std::time::Duration::from_secs_f32(opcoes.testar_som);
        let inicio = Instant::now();
        let mut anterior = som.estado();
        println!("--testar-som: estado inicial {anterior:?}");
        while inicio.elapsed() < limite {
            platform.delay_ms(100);
            let agora = som.estado();
            if agora != anterior {
                println!(
                    "--testar-som: {anterior:?} → {agora:?} em {:.1} s",
                    inicio.elapsed().as_secs_f32()
                );
                anterior = agora;
            }
            if agora == MusicState::Stopped {
                break;
            }
        }
        let decorrido = inicio.elapsed().as_secs_f32();
        println!(
            "--testar-som: fim em {decorrido:.1} s, estado {anterior:?} (volume {})",
            som.volume()
        );
        som.encerrar();
        telas.liberar(&mut pintor);
        return Ok(());
    }

    // ── medição ──────────────────────────────────────────────
    //
    // Sem dormir entre quadros: o que sai é o custo do desenho, que é o número
    // que decide se cabe no aparelho. O `present()` entra na conta de propósito
    // — num aparelho com KMSDRM o swap espera o vblank.
    if opcoes.bench > 0 {
        for _ in 0..20 {
            telas.desenhar(&mut pintor, &estado, &som, None);
        }
        let inicio = Instant::now();
        for _ in 0..opcoes.bench {
            telas.desenhar(&mut pintor, &estado, &som, None);
        }
        let decorrido = inicio.elapsed().as_secs_f64();
        let ms = decorrido * 1000.0 / f64::from(opcoes.bench);
        let (pw, ph) = pintor.painel();
        println!(
            "bench: {} quadros em {:.3} s — {ms:.2} ms/quadro ({:.1} fps) · {} desenho(s) · painel {pw}x{ph}",
            opcoes.bench,
            decorrido,
            1000.0 / ms,
            pintor.desenhos()
        );
        som.encerrar();
        telas.liberar(&mut pintor);
        return Ok(());
    }

    // `--teclas` é aplicado **depois do primeiro desenho**: a tela publica o
    // layout, e antes disso o desenho não existe. No launcher este detalhe já
    // foi um bug medido — a ferramenta imprimia o estado inicial e "provava"
    // que a navegação não funcionava.
    telas.desenhar(&mut pintor, &estado, &som, None);

    if let Some(lista) = &opcoes.teclas {
        for nome in lista.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let acao = acao_de_nome(nome).ok_or_else(|| format!("--teclas: tecla desconhecida: {nome}"))?;
            let efeito = estado.aplicar(acao);
            if let Consequencia::Sair = executar(efeito, &mut estado, &mut som, &pasta, &mut telas, &mut pintor) {
                break;
            }
            telas.desenhar(&mut pintor, &estado, &som, None);
        }
        eprintln!(
            "--teclas: {lista} → foco {} ({}) · mixer {:?} · volume {} · estado {}",
            estado.carrossel.foco(),
            estado
                .faixa_em_foco()
                .map(|f| f.titulo.as_str())
                .unwrap_or("—"),
            estado.indice_tocando(),
            estado.transporte.volume,
            estado.transporte.estado.rotulo()
        );
        som.encerrar();
        telas.liberar(&mut pintor);
        return Ok(());
    }

    if let Some(destino) = &opcoes.dump_frame {
        telas.desenhar(&mut pintor, &estado, &som, None);
        pintor.save_frame(destino)?;
        let (pw, ph) = pintor.painel();
        eprintln!(
            "quadro {pw}x{ph} gravado em {destino} ({} desenhos, rotação {})",
            pintor.desenhos(),
            pintor.rotacao().descricao()
        );
        som.encerrar();
        telas.liberar(&mut pintor);
        return Ok(());
    }

    // ── laço de uso ──────────────────────────────────────────
    let mut leitor = Leitor::abrir();
    let mut recado: Option<String> = None;

    loop {
        let eventos = platform.poll_events();
        let mut teclado = Vec::new();
        for evento in &eventos {
            if let Some(acao) = teclas_do_evento(evento) {
                teclado.push(acao);
            }
        }

        let mut sair = false;
        for acao in leitor.acoes(teclado) {
            let efeito = estado.aplicar(acao);
            match executar(efeito, &mut estado, &mut som, &pasta, &mut telas, &mut pintor) {
                Consequencia::Nada => {}
                Consequencia::Recado(r) => recado = Some(r),
                Consequencia::Sair => {
                    sair = true;
                    break;
                }
            }
        }
        if sair {
            break;
        }

        // Fim de faixa: o `SDL_mixer` não tem callback de término, e o estado
        // `Stopped` chegando sozinho é o único aviso. Sem isto a barra ficaria
        // dizendo "Tocando" para sempre depois da última nota.
        if estado.indice_tocando().is_some() && som.estado() == MusicState::Stopped {
            eprintln!("faixa terminou");
            estado.terminou();
            recado = None;
        }

        telas.desenhar(&mut pintor, &estado, &som, recado.as_deref());
        pintor.present();
        platform.delay_ms(16);
    }

    som.encerrar();
    telas.liberar(&mut pintor);
    Ok(())
}

/// O que fazer depois de uma decisão do núcleo.
enum Consequencia {
    Nada,
    Recado(String),
    Sair,
}

/// Executa o efeito no mundo: mixer e disco.
///
/// É o único lugar do app que chama o mixer e o `read_dir`, e por isso o único
/// que pode falhar em nome de todos. Cada falha vira `Recado`, que a barra de
/// transporte mostra — não `let _ =` nem `eprintln!` que ninguém lê no aparelho.
fn executar(
    efeito: Efeito,
    estado: &mut Estado,
    som: &mut Som,
    pasta: &str,
    telas: &mut Telas,
    pintor: &mut Pintor,
) -> Consequencia {
    match efeito {
        Efeito::Nada => Consequencia::Nada,
        Efeito::Tocar(indice) => {
            match estado.biblioteca.faixas().get(indice).map(|f| f.caminho.clone()) {
                Some(caminho) => match som.tocar(&caminho) {
                    Ok(()) => Consequencia::Nada,
                    Err(e) => {
                        eprintln!("erro: {e}");
                        Consequencia::Recado(e)
                    }
                },
                None => Consequencia::Recado("a faixa saiu da pasta".to_string()),
            }
        }
        Efeito::Retomar => {
            som.retomar();
            Consequencia::Nada
        }
        Efeito::Pausar => {
            som.pausar();
            Consequencia::Nada
        }
        Efeito::Parar => {
            som.parar();
            Consequencia::Nada
        }
        Efeito::Volume(v) => {
            som.definir_volume(v);
            Consequencia::Nada
        }
        Efeito::Recarregar => {
            let nova = ler_pasta(pasta);
            eprintln!("recarregando {pasta}: {}", nova.mensagem());
            estado.trocar_biblioteca(nova);
            // A capa é derivada do nome; a lista nova pode ter outros índices.
            // Reusar a textura antiga desenharia a capa de outra música.
            telas.liberar(pintor);
            // O mixer continua com o handle da faixa antiga; se ela saiu da
            // pasta, o próximo `Tocar` troca. Parar agora calaria uma música
            // que continua existindo — não fazer nada é o comportamento certo.
            Consequencia::Nada
        }
        Efeito::Sair => Consequencia::Sair,
    }
}

fn teclas_do_evento(evento: &Event) -> Option<AcaoDoTeclado> {
    match evento {
        Event::Quit { .. } => Some(AcaoDoTeclado::Sair),
        Event::KeyDown {
            keycode: Some(tecla),
            repeat,
            ..
        } => {
            // A repetição do sistema operacional é ignorada: quem repete o
            // direcional é o `Mapper`, com a curva medida no aparelho.
            if *repeat {
                return None;
            }
            let acao = match *tecla {
                Keycode::Escape | Keycode::Q => return Some(AcaoDoTeclado::Sair),
                Keycode::Left | Keycode::A => Acao::Mover(-1),
                Keycode::Right | Keycode::D => Acao::Mover(1),
                Keycode::Return | Keycode::Space => Acao::Alternar,
                Keycode::N | Keycode::PageDown => Acao::Proxima,
                Keycode::P | Keycode::PageUp => Acao::Anterior,
                Keycode::S => Acao::Parar,
                Keycode::Up | Keycode::Equals | Keycode::Plus => Acao::Volume(1),
                Keycode::Down | Keycode::Minus => Acao::Volume(-1),
                Keycode::R => Acao::Recarregar,
                _ => return None,
            };
            Some(AcaoDoTeclado::Acao(acao))
        }
        _ => None,
    }
}

fn acao_de_nome(nome: &str) -> Option<Acao> {
    match nome.trim().to_ascii_lowercase().as_str() {
        "left" | "esquerda" => Some(Acao::Mover(-1)),
        "right" | "direita" => Some(Acao::Mover(1)),
        "up" | "cima" => Some(Acao::Volume(1)),
        "down" | "baixo" => Some(Acao::Volume(-1)),
        "a" | "confirm" | "tocar" => Some(Acao::Alternar),
        "x" | "next" | "proxima" | "próxima" => Some(Acao::Proxima),
        "y" | "prev" | "anterior" => Some(Acao::Anterior),
        "b" | "stop" | "parar" => Some(Acao::Parar),
        "lb" | "menos" => Some(Acao::Volume(-1)),
        "rb" | "mais" => Some(Acao::Volume(1)),
        "reload" | "recarregar" => Some(Acao::Recarregar),
        "sair" | "exit" => Some(Acao::Sair),
        _ => None,
    }
}

/// Lê a pasta e monta a biblioteca.
///
/// `read_dir` é a única leitura de diretório do app; o resto — o que é música,
/// em que ordem, com que título — é decisão do núcleo, que tem teste.
fn ler_pasta(pasta: &str) -> Biblioteca {
    let diretorio = match std::fs::read_dir(pasta) {
        Ok(d) => d,
        Err(e) => {
            let motivo = match e.kind() {
                std::io::ErrorKind::NotFound => Vazio::PastaNaoExiste,
                std::io::ErrorKind::PermissionDenied => Vazio::SemLeitura,
                _ => Vazio::PastaNaoExiste,
            };
            return Biblioteca::Vazia {
                pasta: pasta.to_string(),
                motivo,
            };
        }
    };

    let mut nomes: Vec<(String, bool)> = Vec::new();
    for entrada in diretorio.flatten() {
        let nome = entrada.file_name().to_string_lossy().to_string();
        let eh_dir = entrada.file_type().map(|t| t.is_dir()).unwrap_or(false);
        nomes.push((nome, eh_dir));
    }
    nomes.sort_by(|a, b| a.0.cmp(&b.0));

    let entradas: Vec<Entrada> = nomes
        .iter()
        .map(|(nome, eh_dir)| Entrada {
            nome,
            eh_dir: *eh_dir,
        })
        .collect();

    let mut biblioteca = montar(pasta, &entradas);
    if let Biblioteca::Com { faixas, .. } = &mut biblioteca {
        for faixa in faixas.iter_mut() {
            medir(faixa);
        }
    }
    biblioteca
}

/// Completa a duração das faixas que declaram o tamanho no cabeçalho.
fn medir(faixa: &mut Faixa) {
    use std::io::Read;
    let Ok(meta) = std::fs::metadata(&faixa.caminho) else {
        return;
    };
    let Ok(mut arquivo) = std::fs::File::open(&faixa.caminho) else {
        return;
    };
    let mut cabecalho = vec![0u8; 4096];
    let lidos = arquivo.read(&mut cabecalho).unwrap_or(0);
    cabecalho.truncate(lidos);
    giochi_musica_core::biblioteca::medir_duracao(faixa, &cabecalho, meta.len());
}

/// Biblioteca sintética para `--demo` e `--dump-frame`.
///
/// Serve para capturar tela em máquina sem música — inclusive no CI. Mistura
/// nomes curtos e longos de propósito: o corte com reticências faz parte do que
/// a captura precisa mostrar.
fn demo() -> Biblioteca {
    let nomes = [
        "01 - Auroras de Vidro.wav",
        "02 - Noite de Cobre.mp3",
        "03 - Um Trem Chamado Desejo e um Nome Muito Comprido Para Caber na Tela.ogg",
        "04 - Vento Solar.flac",
        "05 - Cidade Submersa.wav",
        "06 - A Última Estação.mp3",
        "07 - Maré Alta.wav",
        "08 - Silêncio em Ré Menor.flac",
    ];
    let mut faixas: Vec<Faixa> = nomes
        .iter()
        .filter_map(|n| Faixa::nova("/user/musica", n))
        .collect();
    for (i, faixa) in faixas.iter_mut().enumerate() {
        // Durações conhecidas e desconhecidas juntas: a ficha mostra `—` onde
        // não há medida, e a captura tem de exercitar os dois casos.
        if faixa.formato == giochi_musica_core::Formato::Wav {
            faixa.duracao = Some(95.0 + i as f32 * 37.0);
        }
    }
    Biblioteca::Com {
        pasta: "/user/musica".to_string(),
        faixas,
    }
}

fn criar_pintor(
    platform: &Platform,
    largura: u32,
    altura: u32,
    fonte: &str,
) -> Result<Pintor, String> {
    let janela = platform
        .create_window(
            WindowConfig::new(largura, altura),
            "Giochi Música",
        )
        .map_err(|e| format!("janela: {e}"))?;
    let render = RenderContext::new(janela).map_err(|e| format!("renderer: {e}"))?;
    Pintor::novo(render, fonte)
}

/// Tamanho do painel, quando o app roda no aparelho.
///
/// O modo do conector não vem do `--tamanho`: no console, SDL/KMSDRM usa o modo
/// do painel. Ler o `virtual_size` do framebuffer é o que faz o app abrir no
/// tamanho certo sem uma constante por aparelho.
fn tamanho_do_painel() -> (u32, u32) {
    let texto = std::fs::read_to_string("/sys/class/graphics/fb0/virtual_size").unwrap_or_default();
    if let Some((l, a)) = texto.trim().split_once(',') {
        if let (Ok(l), Ok(a)) = (l.trim().parse::<u32>(), a.trim().parse::<u32>()) {
            return (l, a);
        }
    }
    (640, 480)
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn teclas_conhecidas_viram_acao() {
        assert_eq!(acao_de_nome("left"), Some(Acao::Mover(-1)));
        assert_eq!(acao_de_nome("A"), Some(Acao::Alternar));
        assert_eq!(acao_de_nome(" rb "), Some(Acao::Volume(1)));
        assert_eq!(acao_de_nome("recarregar"), Some(Acao::Recarregar));
        assert_eq!(acao_de_nome("sair"), Some(Acao::Sair));
        assert_eq!(acao_de_nome("tecla_que_nao_existe"), None);
    }

    #[test]
    fn demo_tem_faixas_suficientes_para_o_carrossel() {
        let b = demo();
        assert!(b.total() >= 5, "o carrossel precisa de vizinhos dos dois lados");
        // Mistura de durações medidas e não medidas: é o que a ficha mostra.
        let com_duracao = b.faixas().iter().filter(|f| f.duracao.is_some()).count();
        let sem_duracao = b.total() - com_duracao;
        assert!(com_duracao > 0 && sem_duracao > 0);
        // E pelo menos um título comprido, para o corte com reticências.
        assert!(b.faixas().iter().any(|f| f.titulo.chars().count() > 40));
    }

    #[test]
    fn teclado_ignora_repeticao_do_sistema() {
        let evento = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Right),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: true,
        };
        assert_eq!(teclas_do_evento(&evento), None);
        let evento = Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Escape),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        };
        assert_eq!(teclas_do_evento(&evento), Some(AcaoDoTeclado::Sair));
    }
}
