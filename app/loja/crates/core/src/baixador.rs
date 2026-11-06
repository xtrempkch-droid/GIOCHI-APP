//! Baixar o `.giochi` — e a decisão sobre o `butlerd`, que mora aqui.
//!
//! # Por que não `butlerd`
//!
//! O `butlerd` é um daemon JSON-RPC do itch.io que faz **download incremental
//! por blocos**, e existe para um problema específico: builds de dezenas de GB,
//! onde rebaixar tudo a cada versão é inviável.
//!
//! Nosso caso é o oposto, e é medido: o pacote de um jogo nativo deste
//! repositório tem **81 KB** (`arcade-volleyball.giochi`, 81.408 bytes). Baixar
//! 81 KB de novo não é problema — é mais rápido que o handshake do daemon.
//!
//! | | butlerd | curl com retomada |
//! | --- | --- | --- |
//! | peças para empacotar e supervisionar | daemon + protocolo + estado | **nenhuma** |
//! | retomada de download interrompido | sim | **sim** (`-C -`) |
//! | ganho com pacote de 81 KB | zero | zero |
//! | ganho com pacote de 5 GB | grande | pequeno |
//!
//! A regra para reconsiderar, escrita para não ser redescoberta por intuição:
//! **quando o catálogo tiver pacotes acima de ~500 MB**, o custo de rebaixar
//! inteiro passa a doer e o `butlerd` volta a ser a conversa. Antes disso, é
//! peça a mais para instalar, supervisionar e explicar no suporte.
//!
//! # Por que `curl` e não uma crate de HTTP
//!
//! `curl` existe nos três sistemas que a loja suporta: Linux, macOS (desde
//! sempre) e Windows (10 1803+). Com `reqwest`/`ureq` eu trocaria uma dependência
//! de sistema **já presente** por uma árvore de crates de TLS, e cada uma é
//! superfície de atualização num binário que o usuário baixa e executa.
//!
//! O que o `curl` faz e que eu teria de reimplementar: retomada, redirecionamento,
//! validação de certificado, proxy corporativo, IPv6, HTTP/2. Não é pouco.
//!
//! O preço: erro de rede chega como código de saída, e a mensagem tem de ser
//! traduzida. É o que `ErroDownload` faz.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDownload {
    /// A URL não é https e o baixador não permite http.
    UrlInsegura(String),
    /// O binário não está no PATH, ou não executa.
    SemCurl(String),
    /// Falha de rede/DNS/TLS, com o que o curl disse.
    Falhou { codigo: i32, detalhe: String },
    /// Baixou, mas o arquivo não aparece no disco.
    NaoGravou(PathBuf),
}

impl fmt::Display for ErroDownload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UrlInsegura(u) => write!(f, "recusado baixar de URL sem https: {u}"),
            Self::SemCurl(e) => write!(f, "curl não encontrado ({e}); é preciso no PATH"),
            Self::Falhou { codigo, detalhe } => {
                if detalhe.is_empty() {
                    write!(f, "download falhou (curl saiu {codigo})")
                } else {
                    write!(f, "download falhou: {detalhe}")
                }
            }
            Self::NaoGravou(p) => write!(f, "download terminou mas {} não existe", p.display()),
        }
    }
}

impl std::error::Error for ErroDownload {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relato {
    pub destino: PathBuf,
    pub bytes: u64,
    /// `true` quando havia arquivo parcial e o curl continuou dele.
    pub retomado: bool,
}

/// Quem sabe buscar bytes. Existe para que o fluxo que **usa** o download seja
/// testável sem rede — o `Simulado` abaixo é a outra implementação.
pub trait Baixador {
    fn baixar(&self, url: &str, destino: &Path) -> Result<Relato, ErroDownload>;
}

/// Implementação real, sobre o `curl` do sistema.
#[derive(Debug, Clone)]
pub struct Curl {
    pub binario: String,
    /// `false` só para teste local contra `127.0.0.1`.
    pub exigir_https: bool,
    pub tentativas: u32,
}

impl Default for Curl {
    fn default() -> Self {
        Self {
            binario: "curl".into(),
            exigir_https: true,
            tentativas: 3,
        }
    }
}

impl Curl {
    pub fn local(porta: u16) -> Self {
        // Na prática: `Curl { exigir_https: false, ..Default::default() }`.
        // Existe como construtor nomeado para o teste deixar claro por que a
        // exigência caiu, em vez de parecer descuido.
        let _ = porta;
        Self { exigir_https: false, ..Default::default() }
    }
}

impl Baixador for Curl {
    fn baixar(&self, url: &str, destino: &Path) -> Result<Relato, ErroDownload> {
        if self.exigir_https && !url.starts_with("https://") {
            return Err(ErroDownload::UrlInsegura(url.to_string()));
        }

        // Já existe um parcial? Então `-C -` continua dele. É a retomada que o
        // `butlerd` faria por blocos, e para o nosso tamanho basta.
        let parcial = destino.metadata().map(|m| m.len() > 0).unwrap_or(false);

        if let Some(pai) = destino.parent() {
            if !pai.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(pai);
            }
        }

        let saida = std::process::Command::new(&self.binario)
            .arg("-fsSL")
            .arg("--retry")
            .arg(self.tentativas.to_string())
            .arg("--retry-delay")
            .arg("1")
            // `-C -` só faz sentido se o arquivo existe; sem ele o curl erra.
            .args(if parcial { vec!["-C", "-"] } else { vec![] })
            .arg("-o")
            .arg(destino)
            .arg(url)
            .output()
            .map_err(|e| ErroDownload::SemCurl(e.to_string()))?;

        if !saida.status.success() {
            return Err(ErroDownload::Falhou {
                codigo: saida.status.code().unwrap_or(-1),
                // O stderr do curl já vem com "curl: (22) The requested URL
                // returned error: 404", que é a informação útil.
                detalhe: String::from_utf8_lossy(&saida.stderr).trim().to_string(),
            });
        }

        let bytes = destino
            .metadata()
            .map_err(|_| ErroDownload::NaoGravou(destino.to_path_buf()))?
            .len();
        Ok(Relato { destino: destino.to_path_buf(), bytes, retomado: parcial })
    }
}

/// Baixador de mentira, para exercitar o fluxo sem rede.
///
/// Também serve para os casos que a rede real quase nunca produz na hora que a
/// gente precisa: 404, corpo truncado, conteúdo trocado.
#[derive(Debug, Default, Clone)]
pub struct Simulado {
    respostas: HashMap<String, Result<Vec<u8>, ErroDownload>>,
}

impl Simulado {
    pub fn novo() -> Self {
        Self::default()
    }

    pub fn com(mut self, url: &str, conteudo: Vec<u8>) -> Self {
        self.respostas.insert(url.to_string(), Ok(conteudo));
        self
    }

    pub fn com_erro(mut self, url: &str, erro: ErroDownload) -> Self {
        self.respostas.insert(url.to_string(), Err(erro));
        self
    }
}

impl Baixador for Simulado {
    fn baixar(&self, url: &str, destino: &Path) -> Result<Relato, ErroDownload> {
        match self.respostas.get(url) {
            Some(Ok(conteudo)) => {
                if let Some(pai) = destino.parent() {
                    let _ = std::fs::create_dir_all(pai);
                }
                std::fs::write(destino, conteudo).map_err(|e| ErroDownload::Falhou {
                    codigo: -1,
                    detalhe: e.to_string(),
                })?;
                Ok(Relato { destino: destino.to_path_buf(), bytes: conteudo.len() as u64, retomado: false })
            }
            Some(Err(e)) => Err(e.clone()),
            None => Err(ErroDownload::Falhou {
                codigo: 22,
                detalhe: "404".into(),
            }),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("giochi-baixa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn simulado_grava_e_relata_o_tamanho() {
        let d = temp();
        let alvo = d.join("x.giochi");
        let b = Simulado::novo().com("https://x/y", b"abc".to_vec());
        let r = b.baixar("https://x/y", &alvo).unwrap();
        assert_eq!(r.bytes, 3);
        assert_eq!(std::fs::read(&alvo).unwrap(), b"abc");
    }

    #[test]
    fn simulado_devolve_o_erro_configurado() {
        let d = temp();
        let b = Simulado::novo().com_erro(
            "https://x/y",
            ErroDownload::Falhou { codigo: 22, detalhe: "404".into() },
        );
        assert_eq!(
            b.baixar("https://x/y", &d.join("a")).err(),
            Some(ErroDownload::Falhou { codigo: 22, detalhe: "404".into() })
        );
    }

    #[test]
    fn url_sem_https_e_recusada_antes_de_executar_curl() {
        // A checagem tem de acontecer **antes** do processo. Se depois, o curl
        // já teria feito a requisição.
        let c = Curl::default();
        let r = c.baixar("http://x/y", &temp().join("a"));
        assert!(matches!(r, Err(ErroDownload::UrlInsegura(_))), "{r:?}");
    }

    #[test]
    fn exigir_https_desligado_permite_localhost() {
        let c = Curl::local(0);
        assert!(!c.exigir_https);
        // A tentativa real falha (nada escutando), mas não por URL insegura:
        // é a prova de que a checagem saiu do caminho.
        let r = c.baixar("http://127.0.0.1:1/x", &temp().join("a"));
        assert!(!matches!(r, Err(ErroDownload::UrlInsegura(_))), "{r:?}");
    }
}
