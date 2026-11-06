//! HTTP mínimo para falar com o provedor de licenças.
//!
//! # Por que `curl` de novo
//!
//! Mesma razão do baixador: `curl` existe nos três sistemas que a loja suporta,
//! e trocá-lo por uma árvore de crates de TLS aumenta a superfície de atualização
//! de um binário que o usuário baixa e executa.
//!
//! # O que é testável, e como
//!
//! A parte que erra é a **montagem da requisição** (método, cabeçalhos, corpo
//! form-encoded) e o **parse da resposta**. As duas são funções puras aqui, com
//! teste. Além disso há um teste de ponta a ponta contra um servidor TCP local
//! escrito no próprio teste — sem rede, sem mock de biblioteca, com socket de
//! verdade.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resposta {
    pub status: u16,
    pub corpo: String,
}

impl Resposta {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroHttp {
    SemCurl(String),
    /// Falha de transporte: sem código HTTP, o servidor não respondeu.
    Transporte(String),
    /// Resposta recebida, status fora de 2xx.
    Status { status: u16, corpo: String },
}

impl fmt::Display for ErroHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemCurl(e) => write!(f, "curl não encontrado ({e}); é preciso no PATH"),
            Self::Transporte(e) => write!(f, "sem resposta do servidor: {e}"),
            Self::Status { status, corpo } => write!(f, "servidor respondeu {status}: {corpo}"),
        }
    }
}

impl std::error::Error for ErroHttp {}

/// Uma requisição, antes de virar linha de comando.
///
/// Existe como struct para poder ser **comparada em teste**: montar a linha de
/// comando à mão e comparar string é frágil (a ordem dos argumentos varia), e
/// não montar nada é pior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requisicao {
    pub metodo: Metodo,
    pub url: String,
    pub cabecalhos: Vec<(String, String)>,
    pub corpo: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metodo {
    Get,
    Post,
}

impl Requisicao {
    pub fn get(url: impl Into<String>) -> Self {
        Self { metodo: Metodo::Get, url: url.into(), cabecalhos: Vec::new(), corpo: None }
    }

    /// Form-encoded, que é o que a API de licença do Lemon Squeezy pede.
    ///
    /// Não é JSON de propósito: o corpo form é o que está documentado, e
    /// adivinhar que JSON também funciona produziria um teste que passa contra
    /// um servidor de mentira e falha em produção.
    pub fn post_form(url: impl Into<String>, campos: &[(&str, &str)]) -> Self {
        Self {
            metodo: Metodo::Post,
            url: url.into(),
            cabecalhos: vec![("Content-Type".into(), "application/x-www-form-urlencoded".into())],
            corpo: Some(campos.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")),
        }
    }

    pub fn com_cabecalho(mut self, nome: &str, valor: &str) -> Self {
        self.cabecalhos.push((nome.to_string(), valor.to_string()));
        self
    }

    /// Os argumentos do `curl`, na ordem em que precisam sair.
    pub fn argumentos(&self) -> Vec<String> {
        let mut a = vec!["-sS".to_string(), "-w".to_string(), "\\n%{http_code}".to_string()];
        a.push("-X".into());
        a.push(match self.metodo {
            Metodo::Get => "GET".into(),
            Metodo::Post => "POST".into(),
        });
        for (k, v) in &self.cabecalhos {
            a.push("-H".into());
            a.push(format!("{k}: {v}"));
        }
        if let Some(c) = &self.corpo {
            // Só `--data-raw`. Uma versão anterior empurrava `--data` antes
            // dele, e o curl lia o **literal** "--data-raw" como sendo o corpo
            // da requisição — o servidor recebia a string "--data-raw" em vez
            // dos campos. O teste pegou.
            //
            // `--data` (sem `raw`) trata `@nome` como arquivo: uma chave que
            // comece com `@` faria o curl tentar abrir um arquivo.
            a.push("--data-raw".into());
            a.push(c.clone());
        }
        a.push(self.url.clone());
        a
    }
}

/// Executa a requisição.
///
/// A última linha da saída é o código HTTP, por causa do `-w`; o resto é o corpo.
/// Isso evita precisar de `-f` (que descartaria o corpo do erro, que é justamente
/// o que diz **por que** falhou).
pub fn executar(req: &Requisicao) -> Result<Resposta, ErroHttp> {
    let saida = std::process::Command::new("curl")
        .args(req.argumentos())
        .output()
        .map_err(|e| ErroHttp::SemCurl(e.to_string()))?;

    if !saida.status.success() && saida.stdout.is_empty() {
        return Err(ErroHttp::Transporte(
            String::from_utf8_lossy(&saida.stderr).trim().to_string(),
        ));
    }

    let texto = String::from_utf8_lossy(&saida.stdout).to_string();
    let (corpo, codigo) = match texto.rsplit_once('\n') {
        Some((c, s)) => (c.to_string(), s.trim().to_string()),
        None => (String::new(), texto.trim().to_string()),
    };
    let status: u16 = codigo.parse().map_err(|_| ErroHttp::Transporte(format!("código ilegível: {codigo:?}")))?;

    if !(200..300).contains(&status) {
        return Err(ErroHttp::Status { status, corpo });
    }
    Ok(Resposta { status, corpo })
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn get_monta_os_argumentos_na_ordem_certa() {
        let r = Requisicao::get("https://api.exemplo/x").com_cabecalho("Authorization", "Bearer K");
        let a = r.argumentos();
        assert_eq!(a[0], "-sS");
        assert!(a.contains(&"GET".to_string()));
        assert_eq!(a.iter().filter(|x| *x == "-H").count(), 1);
        assert_eq!(*a.last().unwrap(), "https://api.exemplo/x");
    }

    #[test]
    fn post_form_usa_data_raw_para_nao_ler_arquivo() {
        // `--data` trata `@nome` como arquivo. Uma chave de licença que comece
        // com `@` faria o curl tentar ler um arquivo e o erro seria "não achei
        // o arquivo", apontando para o lugar errado.
        let r = Requisicao::post_form("https://x", &[("license_key", "@suspeita")]);
        let a = r.argumentos();
        assert!(a.contains(&"--data-raw".to_string()), "{a:?}");
        assert!(!a.contains(&"--data".to_string()), "{a:?}");
        assert!(a.contains(&"license_key=@suspeita".to_string()));
    }

    #[test]
    fn form_une_campos_com_e_comercial() {
        let r = Requisicao::post_form("https://x", &[("a", "1"), ("b", "2")]);
        assert_eq!(r.corpo.as_deref(), Some("a=1&b=2"));
    }

    /// Sobe um servidor de uma requisição só e devolve o que ele recebeu.
    fn servidor(resposta: &'static str) -> (u16, std::sync::mpsc::Receiver<String>) {
        use std::sync::mpsc;
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                // Ler **até o fim dos cabeçalhos e do corpo**. Um `read` único
                // devolve só os cabeçalhos quando o curl manda o corpo em outro
                // segmento TCP, e o teste falhava por isso — não por defeito do
                // código testado.
                let mut recebido = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            recebido.extend_from_slice(&buf[..n]);
                            // Corpo completo quando o que veio depois do
                            // `\r\n\r\n` chega ao Content-Length.
                            let texto = String::from_utf8_lossy(&recebido).to_string();
                            if let Some(fim) = texto.find("\r\n\r\n") {
                                let corpo = &texto[fim + 4..];
                                let esperado = texto
                                    .lines()
                                    .find_map(|l| {
                                        l.to_ascii_lowercase()
                                            .strip_prefix("content-length:")
                                            .and_then(|v| v.trim().parse::<usize>().ok())
                                    })
                                    .unwrap_or(0);
                                if corpo.len() >= esperado {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&recebido).to_string());
                let _ = s.write_all(resposta.as_bytes());
            }
        });
        (porta, rx)
    }

    #[test]
    fn le_o_codigo_da_ultima_linha_e_o_corpo_do_resto() {
        // O `-w "\n%{http_code}"` põe o código depois do corpo; separar errado
        // faria o JSON da resposta ser descartado como se fosse status.
        let (porta, _rx) = servidor(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"id\":\"ab\"}\n200",
        );
        let r = executar(&Requisicao::get(format!("http://127.0.0.1:{porta}/"))).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.corpo.trim(), "{\"id\":\"ab\"}");
    }

    #[test]
    fn status_de_erro_vira_erro_com_o_corpo_preservado() {
        // O corpo de erro é o que diz POR QUE falhou ("license key is invalid").
        // Descartá-lo transformaria um problema diagnosticável em "falhou".
        let (porta, _rx) = servidor(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 21\r\n\r\n{\"error\":\"invalid\"}\n404",
        );
        match executar(&Requisicao::get(format!("http://127.0.0.1:{porta}/"))) {
            Err(ErroHttp::Status { status, corpo }) => {
                assert_eq!(status, 404);
                assert!(corpo.contains("invalid"), "{corpo}");
            }
            outro => panic!("esperava Status, veio {outro:?}"),
        }
    }

    #[test]
    fn o_post_chega_com_metodo_e_corpo_no_servidor() {
        let (porta, rx) = servidor("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}\n200");
        let _ = executar(&Requisicao::post_form(
            format!("http://127.0.0.1:{porta}/ativar"),
            &[("license_key", "abc-123"), ("instance_name", "aparelho")],
        ))
        .unwrap();
        let recebido = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(recebido.starts_with("POST /ativar"), "{recebido}");
        assert!(recebido.contains("license_key=abc-123"), "{recebido}");
        assert!(recebido.contains("instance_name=aparelho"), "{recebido}");
    }
}
