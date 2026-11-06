//! Emite uma licença de teste e confere que ela vale.
//!
//!     cargo run --example emitir_licenca -- \
//!         --app pacman --aparelho <b64 ou arquivo> --saida licenca.json
//!
//! # Isto é ferramenta de teste, e diz isso na saída
//!
//! Em produção quem assina licença é o **servidor**, com uma chave que não
//! existe em máquina de cliente. Este exemplo existe para que o fluxo inteiro —
//! emitir, gravar, ler, verificar no aparelho — possa ser exercitado sem
//! depender de um backend que ainda não existe.
//!
//! Se esta ferramenta um dia for distribuída com a loja, a licença deixa de
//! valer qualquer coisa: quem tem a chave de licença emite a própria.

use ed25519_dalek::{SigningKey, VerifyingKey};
use giochi_loja_core::chave::ChavePublica;
use giochi_loja_core::licenca::{agora_unix, Licenca, LicencaAssinada, Veredito, VERSAO};

fn ajuda() {
    println!(
        r#"emitir_licenca — FERRAMENTA DE TESTE (a chave de licença real fica no servidor)

  --app <slug>           app licenciado (obrigatório)
  --aparelho <valor>     chave pública do aparelho: base64, arquivo .pem ou .ed25519
  --chave <arquivo>      chave privada de licença (PEM/base64); sem ela, gera uma
  --saida <arquivo>      onde gravar o JSON (padrão: stdout)
  --dias <n>             validade em dias; 0 ou ausente = perpétua
  --pedido <id>          id do pedido (padrão: TESTE-<timestamp>)

Ao final, verifica a licença com a chave pública correspondente e informa se
passou — uma ferramenta que emite sem conferir não prova nada."#
    );
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut app = None;
    let mut aparelho = None;
    let mut chave: Option<SigningKey> = None;
    let mut saida = None;
    let mut dias: u64 = 0;
    let mut pedido = None;

    let mut i = 1;
    while i < args.len() {
        let valor = |i: usize| args.get(i + 1).cloned().ok_or_else(|| format!("{} precisa de valor", args[i]));
        match args[i].as_str() {
            "--ajuda" | "-h" => {
                ajuda();
                return Ok(());
            }
            "--app" => {
                app = Some(valor(i)?);
                i += 2;
            }
            "--aparelho" => {
                aparelho = Some(valor(i)?);
                i += 2;
            }
            "--chave" => {
                let caminho = valor(i)?;
                // Chave privada em PEM PKCS#8 ou 32 bytes crus. O caminho fácil
                // (arquivo de semente) é o que o exemplo usa.
                let bytes = std::fs::read(&caminho).map_err(|e| format!("{caminho}: {e}"))?;
                chave = Some(
                    SigningKey::from_bytes(&bytes[..32].try_into().map_err(|_| "chave curta")?),
                );
                i += 2;
            }
            "--saida" => {
                saida = Some(valor(i)?);
                i += 2;
            }
            "--dias" => {
                dias = valor(i)?.parse().map_err(|e| format!("--dias: {e}"))?;
                i += 2;
            }
            "--pedido" => {
                pedido = Some(valor(i)?);
                i += 2;
            }
            outro => return Err(format!("argumento desconhecido: {outro}")),
        }
    }

    let app = app.ok_or("--app é obrigatório (use --ajuda)")?;
    let aparelho_txt = aparelho.ok_or("--aparelho é obrigatório (use --ajuda)")?;

    // Aceita caminho de arquivo ou o valor direto: quem emite costuma ter o
    // arquivo do aparelho, não a string.
    let chave_aparelho = if std::path::Path::new(&aparelho_txt).is_file() {
        ChavePublica::de_arquivo(&aparelho_txt).map_err(|e| e.to_string())?
    } else {
        ChavePublica::de_texto(&aparelho_txt).map_err(|e| e.to_string())?
    };

    let chave = match chave {
        Some(c) => c,
        None => {
            println!("AVISO nenhuma --chave informada: gerando uma chave de TESTE");
            println!("      (uma licença assinada por ela só vale contra a pública dela)");
            SigningKey::from_bytes(&rand_bytes())
        }
    };

    let agora = agora_unix();
    let licenca = Licenca {
        versao: VERSAO,
        app: app.clone(),
        versao_app: String::new(),
        aparelho: chave_aparelho.base64(),
        pedido: pedido.unwrap_or_else(|| format!("TESTE-{agora}")),
        emitida_em: agora,
        expira_em: if dias == 0 { None } else { Some(agora + dias * 86_400) },
    };

    let assinada = LicencaAssinada::emitir(licenca, &chave);
    let json = assinada.para_json();

    match &saida {
        Some(caminho) => {
            std::fs::write(caminho, &json).map_err(|e| format!("{caminho}: {e}"))?;
            println!("licença gravada: {caminho}");
        }
        None => println!("{json}"),
    }

    // Confere o que acabou de sair. Emitir sem verificar é o teste que passa
    // dos dois lados e não prova nada.
    let publica = ChavePublica::de_bytes(&chave.verifying_key().to_bytes()).map_err(|e| e.to_string())?;
    match assinada.verificar(&publica, &chave_aparelho, agora) {
        Veredito::Valida => println!(
            "conferido: válida para '{}' no aparelho {}… ({} )",
            app,
            &chave_aparelho.base64()[..8],
            if dias == 0 { "perpétua".to_string() } else { format!("{dias} dia(s)") }
        ),
        outro => return Err(format!("a licença emitida NÃO verifica: {outro}")),
    }

    // A pública correspondente é o que vai para a imagem em
    // /etc/giochi/licenca-publica.pem.
    let pub_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        VerifyingKey::to_bytes(&chave.verifying_key()),
    );
    println!("chave pública de licença (para a imagem): {pub_b64}");
    Ok(())
}

/// Semente para a chave de teste.
///
/// Não é `rand` de verdade — é derivação do relógio e do pid. Serve para uma
/// chave descartável de teste e **não** serve para produção, e é por isso que
/// está escrito aqui em vez de parecer criptografia.
fn rand_bytes() -> [u8; 32] {
    let mut s = [0u8; 32];
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    for (i, b) in s.iter_mut().enumerate() {
        *b = (agora.wrapping_mul(i as u64 + 7) >> (i % 8)) as u8;
    }
    s
}
