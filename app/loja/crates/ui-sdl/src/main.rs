//! Entrada da loja.
//!
//!   giochi-loja                          janela 1180×760 com o catálogo embutido
//!   giochi-loja --catalogo <arq.json>    carrega um catálogo assinado
//!   giochi-loja --demo                   habilita o fluxo de pagamento de teste
//!   giochi-loja --dump-frame <arq.bmp>   desenha a tela atual num BMP e sai
//!   giochi-loja --teclas up,down,enter   aplica teclas e imprime a tela resultante
//!   giochi-loja --teclas down --json     imprime o estado em JSON (ASCII puro)
//!   giochi-loja --fonte <arquivo.ttf>    fonte alternativa
//!
//! `--dump-frame` e `--teclas` existem pelo mesmo motivo que no launcher: uma
//! interface que só pode ser vista com olho humano não entra em verificação
//! automática, e foi assim que o launcher ficou meses sem prova de que desenhava.
//! Com os dois, dá para comparar quadros pixel a pixel e provar que a navegação
//! mudou o que devia mudar.
//!
//! `SDL_VIDEODRIVER=dummy` faz o mesmo caminho rodar sem tela — no PC, e é
//! assim que o teste roda.

mod capa;
mod desenho;
mod ui;

use giochi_loja_core::catalogo::{Catalogo, CatalogoAssinado};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use ui::{Estado, Tecla};

const LARGURA: u32 = 1180;
const ALTURA: u32 = 760;

fn ajuda() {
    println!(
        r#"giochi-loja — loja do Giochi para o PC

  --catalogo <arquivo>   catálogo assinado (JSON)
  --demo                 fluxo de pagamento de teste (código PIX não pagável)
  --dump-frame <bmp>     desenha e grava um BMP, sem abrir janela
  --teclas <lista>       teclas separadas por vírgula: up,down,left,right,enter,esc
  --json                 imprime o estado em JSON, sem texto acentuado
  --fonte <ttf>          fonte alternativa
  --chave-catalogo <arg> chave pública do catálogo: arquivo .pem ou o valor
  --ajuda                esta mensagem

Variáveis:
  GIOCHI_LOJA_FONTE      mesma coisa que --fonte
  SDL_VIDEODRIVER=dummy  roda sem tela (necessário para --dump-frame no PC)

Sem --catalogo a loja abre com um catálogo de demonstração para que a tela
possa ser vista. O catálogo de verdade é assinado e, sem --chave-catalogo, a
assinatura não é conferida — a loja diz isso na abertura."#
    );
}

/// Catálogo de demonstração: existe para a tela ter conteúdo sem rede.
///
/// Ele **não** é assinado, e a loja avisa. Um catálogo embutido que fingisse ser
/// o real seria a mesma mentira do `#002` que apareceu no cartão do launcher.
fn catalogo_de_demonstracao() -> Catalogo {
    use giochi_loja_core::catalogo::Item;
    use giochi_loja_core::preco::{Moeda, Preco};

    let item = |slug: &str, titulo: &str, ano: u16, centavos: u32, desc: &str| Item {
        slug: slug.into(),
        titulo: titulo.into(),
        ano,
        descricao: desc.into(),
        preco: Preco::new(centavos, Moeda::BRL),
        versao: "1.0.0".into(),
        tamanho_bytes: 81_408,
        sha256: String::new(),
        url: format!("https://cdn.giochi.example/{slug}.giochi"),
        assinatura: String::new(),
        chave_pacote: String::new(),
    };

    Catalogo {
        versao: giochi_loja_core::catalogo::VERSAO,
        gerado_em: 0,
        invalida_antes: None,
        itens: vec![
            item("arcade-volleyball", "Arcade Volleyball", 1988, 0,
                 "Vôlei de praia em dois contra dois, port nativo em C++ e SDL2, roda no aparelho."),
            item("pacman", "Pac-Man", 1980, 1990,
                 "Labirinto 21x21 com quatro fantasmas de perseguição distinta. Nativo em Rust."),
            item("cobrinha", "Cobrinha", 1997, 990,
                 "Grade de 24 colunas com passos discretos. O clássico da cobrinha."),
            item("volley", "Volley", 1988, 1490,
                 "Vôlei em Rust e SDL2, feito neste repositório para exercitar o ciclo lançar/voltar."),
        ],
    }
}

fn tecla_de(texto: &str) -> Option<Tecla> {
    Some(match texto {
        "up" | "cima" => Tecla::Cima,
        "down" | "baixo" => Tecla::Baixo,
        "left" | "esquerda" => Tecla::Esquerda,
        "right" | "direita" => Tecla::Direita,
        "enter" | "confirmar" => Tecla::Confirmar,
        "esc" | "voltar" => Tecla::Voltar,
        _ => return None,
    })
}

fn fonte_padrao() -> String {
    // A fonte não vai no repositório (mesma decisão do launcher). Ordem:
    // variável, CWD, caminhos do sistema. No Windows e no macOS o caminho do
    // sistema resolve; se nada existir, o texto simplesmente não aparece e o
    // layout continua de pé.
    if let Ok(f) = std::env::var("GIOCHI_LOJA_FONTE") {
        return f;
    }
    for c in [
        "assets/fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/Library/Fonts/Arial.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ] {
        if std::path::Path::new(c).is_file() {
            return c.to_string();
        }
    }
    String::new()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut catalogo_path: Option<String> = None;
    let mut dump: Option<String> = None;
    let mut teclas: Vec<String> = Vec::new();
    let mut demo = false;
    let mut fonte = fonte_padrao();
    let mut chave_catalogo: Option<String> = None;
    let mut json = false;
    let mut mostrar_mensagem = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ajuda" | "-h" => {
                ajuda();
                return Ok(());
            }
            "--catalogo" => {
                i += 1;
                catalogo_path = args.get(i).cloned();
            }
            "--dump-frame" => {
                i += 1;
                dump = args.get(i).cloned();
            }
            "--teclas" => {
                i += 1;
                teclas = args
                    .get(i)
                    .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                    .unwrap_or_default();
            }
            "--fonte" => {
                i += 1;
                fonte = args.get(i).cloned().unwrap_or(fonte);
            }
            "--chave-catalogo" => {
                i += 1;
                chave_catalogo = args.get(i).cloned();
            }
            "--demo" => demo = true,
            "--json" => json = true,
            // Diagnóstico do contrato: imprime exatamente os bytes que este
            // binário assina/verifica. Foi o que permitiu achar a divergência
            // com o gerador em JS, e voltará a ser útil na próxima mudança de
            // formato.
            "--mensagem-catalogo" => mostrar_mensagem = true,
            outro => return Err(format!("argumento desconhecido: {outro} (use --ajuda)")),
        }
        i += 1;
    }

    // Catálogo: arquivo, ou demonstração.
    let catalogo = if let Some(caminho) = &catalogo_path {
        let texto = std::fs::read_to_string(caminho)
            .map_err(|e| format!("não consegui ler {caminho}: {e}"))?;
        let assinado = CatalogoAssinado::de_json(&texto)
            .map_err(|e| format!("{caminho}: {e}"))?;

        match &chave_catalogo {
            Some(b64) => {
                // Aceita **caminho de arquivo** ou o valor direto. Quem publica
                // tem o `.pub.pem` no disco; exigir que ele cole a chave no
                // terminal é o tipo de atrito que produz erro de aspas — e
                // produzir "chave com 0 bytes" como primeira mensagem.
                let chave = if std::path::Path::new(b64).is_file() {
                    giochi_loja_core::ChavePublica::de_arquivo(b64)
                } else {
                    giochi_loja_core::ChavePublica::de_texto(b64)
                }
                .map_err(|e| format!("chave do catálogo ({b64}): {e}"))?;
                // Verificar é obrigatório quando a chave é dada: um catálogo que
                // não passa não abre, e a loja não "avisa e continua".
                assinado
                    .verificar(&chave)
                    .map_err(|e| format!("catálogo recusado: {e}"))?;
                eprintln!("catálogo verificado contra a chave informada");
                assinado.catalogo
            }
            None => {
                eprintln!(
                    "AVISO catálogo carregado SEM verificação de assinatura \
                     (passe --chave-catalogo); os itens podem ser de qualquer origem"
                );
                assinado.catalogo
            }
        }
    } else {
        eprintln!("nenhum --catalogo: usando o catálogo de demonstração (não assinado)");
        catalogo_de_demonstracao()
    };

    if mostrar_mensagem {
        // A mensagem canônica do catálogo **como carregado**, antes de qualquer
        // transformação: é o que o gerador precisa produzir byte a byte.
        print!("{}", String::from_utf8_lossy(&catalogo.mensagem()));
        return Ok(());
    }

    let mut estado = Estado::novo(LARGURA as i32, ALTURA as i32).com_catalogo(catalogo);
    if demo {
        estado = estado.demo();
    }

    // Carrega a fonte antes de abrir a janela: `ttf::init()` não pode ser
    // chamado duas vezes, e guardar o contexto é o que evita isso quando a
    // janela é recriada.
    let mut ttf: Option<&'static sdl2::ttf::Sdl2TtfContext> = None;
    if !fonte.is_empty() {
        let ok = sdl2::ttf::init().is_ok();
        if ok {
            // `init` devolve a cada chamada um contexto novo; o `Box::leak` é o
            // preço de um tipo que não é `Send`/`Sync` e precisa viver o
            // processo inteiro. O launcher faz o mesmo, pelo mesmo motivo.
            ttf = Some(Box::leak(Box::new(sdl2::ttf::init().map_err(|e| e.to_string())?)));
        }
    }

    // `--teclas` sem `--dump-frame` pressupõe que alguém quer ver a tela final;
    // sem driver gráfico isso não existe. O caminho é sempre: aplicar teclas,
    // desenhar, gravar.
    let headless = dump.is_some();

    let sdl = sdl2::init()?;
    let _ttf_guard = sdl2::ttf::init().ok();
    let video = sdl.video()?;

    // `position_centered` devolve um `WindowBuilder` diferente do de
    // `window()`, então as duas metades do `if` não têm o mesmo tipo. O jeito
    // que compila é montar um builder só e encadear a posição à parte.
    let mut builder = video.window("Giochi Loja", LARGURA, ALTURA);
    if !headless {
        builder.position_centered();
    }
    let janela = builder.build().map_err(|e| e.to_string())?;
    let canvas = janela.into_canvas().build().map_err(|e| e.to_string())?;

    let mut pintor = desenho::Pintor::novo(canvas, ttf, fonte, LARGURA as i32, ALTURA as i32);

    // Aplica as teclas pedidas antes do primeiro desenho: a tela final é a que
    // importa, e desenhar o estado inicial primeiro foi o bug do `--teclas` no
    // launcher (usava a grade de 4 colunas, que era o valor padrão, e não o real).
    let mut saiu = false;
    for nome in &teclas {
        match tecla_de(nome) {
            Some(t) => {
                if !estado.tecla(t) {
                    saiu = true;
                    break;
                }
            }
            None => return Err(format!("tecla desconhecida: {nome}")),
        }
    }
    // `--json` existe porque **verificação de CI não pode depender de texto
    // acentuado**. O PowerShell 5.1 lê a saída de um processo nativo com a code
    // page OEM, e "seleção" chega como "seleÃ§Ã£o" — o mesmo defeito que já
    // quebrou a perna Windows dos apps de Python. Aqui a saída de máquina é
    // ASCII puro e o texto bonito fica para o humano.
    if json {
        println!(
            "{}",
            serde_json::json!({
                "tela": format!("{:?}", estado.tela),
                "selecao": estado.selecao,
                "item": estado.catalogo.as_ref()
                    .and_then(|c| c.itens.get(estado.selecao))
                    .map(|i| i.slug.clone()),
                "preco_centavos": estado.catalogo.as_ref()
                    .and_then(|c| c.itens.get(estado.selecao))
                    .map(|i| i.preco.centavos),
                "mensagem": estado.mensagem,
                // A grade vai no JSON para a verificação poder afirmar o
                // invariante ("↓ anda exatamente uma linha") em vez de fixar um
                // número que muda com o layout — foi o que quebrou o CI quando
                // a lista virou grade.
                "colunas": estado.grade.colunas,
                "por_pagina": estado.grade.por_pagina,
            })
        );
    } else {
        println!(
            "tela: {:?}  seleção: {}  {}{}",
            estado.tela,
            estado.selecao,
            estado
                .catalogo
                .as_ref()
                .and_then(|c| c.itens.get(estado.selecao))
                .map(|i| format!("item: {} · {}", i.titulo, i.preco))
                .unwrap_or_else(|| "sem item".into()),
            if estado.mensagem.is_empty() { String::new() } else { format!("  msg: {}", estado.mensagem) }
        );
    }

    estado.desenhar(&mut pintor);

    if let Some(caminho) = &dump {
        pintor.para_bmp(caminho)?;
        eprintln!("quadro gravado: {caminho}");
        return Ok(());
    }
    if saiu {
        return Ok(());
    }

    let mut eventos = sdl.event_pump()?;
    'laco: loop {
        for evento in eventos.poll_iter() {
            let t = match evento {
                Event::Quit { .. } => break 'laco,
                Event::KeyDown { keycode: Some(k), .. } => match k {
                    Keycode::Up => Some(Tecla::Cima),
                    Keycode::Down => Some(Tecla::Baixo),
                    Keycode::Left => Some(Tecla::Esquerda),
                    Keycode::Right => Some(Tecla::Direita),
                    Keycode::Return | Keycode::KpEnter => Some(Tecla::Confirmar),
                    Keycode::Escape => Some(Tecla::Voltar),
                    Keycode::Q => Some(Tecla::Sair),
                    _ => None,
                },
                _ => None,
            };
            if let Some(t) = t {
                if !estado.tecla(t) {
                    break 'laco;
                }
            }
        }
        estado.desenhar(&mut pintor);
        pintor.canvas.present();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}
