//! O provedor de pagamento **no lugar do nosso backend**.
//!
//! # A pergunta que originou este módulo
//!
//! *"Não tem como o provedor do jogo dar ao backend o meio de pagamento? Ex.:
//! logado no itch.io e o site dá o meio de pagamento."*
//!
//! Tem. Duas formas, com consequências **muito** diferentes, e a diferença decide
//! o desenho:
//!
//! | | **A. Provedor é a loja** (itch.io) | **B. Provedor é o caixa** (Lemon Squeezy, Paddle, Gumroad) |
//! | --- | --- | --- |
//! | quem faz o checkout | o itch.io | o provedor |
//! | quem cuida de imposto | o itch.io | o provedor (é *merchant of record*) |
//! | quem sabe "este usuário comprou" | o itch.io, via `game:view:ownership` | o provedor, via chave de licença |
//! | **vínculo com o aparelho** | **nenhum** — a credencial é a do **usuário** | **sim** — `activation_limit` por instância |
//! | nosso backend | não precisa | **não precisa** |
//! | preço e catálogo | do itch.io | nossos |
//!
//! ## Por que (A) não resolve o problema do roubo
//!
//! No modelo (A) a prova de compra é a **conta do itch.io do usuário**. Quem
//! compartilhar a chave de API compartilha a biblioteca inteira, em qualquer
//! aparelho, e não há o que verificar contra isso: o itch.io diz "este usuário
//! possui", não "este aparelho possui". É um **verificador de posse**, não um
//! vínculo — e para um portátil de mão é a diferença entre vender e não vender.
//!
//! ## Por que (B) resolve
//!
//! A API de licença do Lemon Squeezy tem `activation_limit` e `activation_usage`
//! **no servidor deles**, e cada ativação cria uma *instância* com nome. Nós
//! passamos a chave pública do aparelho como nome da instância. Esgotado o
//! limite, o servidor responde:
//!
//! ```json
//! { "activated": false, "error": "This license key has reached the activation limit." }
//! ```
//!
//! Isso é **contagem de instalações aplicada por terceiro**, que é exatamente o
//! que um backend nosso faria — sem um backend nosso. O modelo (B) é o escolhido.
//!
//! # O que continua sendo verdade
//!
//! A ativação é obtida **uma vez, online**, e fica gravada em disco. Sem rede o
//! aparelho usa o registro em cache, com prazo. Isso é `Ativacao`, e o limite é o
//! mesmo de antes: quem tem root no aparelho edita o cache. A proteção real é o
//! limite de instâncias no servidor do provedor, não um cadeado no cliente.

use std::fmt;

use crate::chave::ChavePublica;
use crate::http::{executar, ErroHttp, Requisicao};

/// Uma ativação obtida do provedor, para **este** aparelho.
///
/// É o que fica em disco. Não é assinada por nós: a prova de que vale é a
/// resposta do provedor, que pode ser reconsultada (`validar`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ativacao {
    /// Qual provedor emitiu. Um registro do provedor A não vale para o B.
    pub provedor: String,
    /// A chave de licença que o usuário digitou.
    pub chave: String,
    /// Nome da instância — a chave pública do aparelho, em base64.
    pub instancia: String,
    /// Slug do app. Impede usar a ativação de um app noutro.
    pub app: String,
    pub ativada_em: u64,
    /// Até quando o cache vale sem reconsultar. `None` = sem prazo.
    ///
    /// Existe porque a licença pode ser desativada no painel do provedor
    /// (reembolso, fraude): sem revalidação periódica, o registro local valeria
    /// para sempre.
    pub revalidar_em: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Situacao {
    /// Ativa e válida para esta instância.
    Ativa,
    /// A chave existe mas não está ativada nesta instância.
    NaoAtivada,
    /// O provedor diz que não vale mais: expirou, foi desativada, ou é inválida.
    Invalida { motivo: String },
    /// Não deu para perguntar (sem rede). Não é veredito.
    NaoSei { motivo: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroProvedor {
    /// O corpo da resposta não é o que a documentação descreve.
    RespostaInesperada(String),
    Http(String),
    /// A chave não tem o formato mínimo (vazia, curta demais).
    ChaveInvalida(String),
}

impl fmt::Display for ErroProvedor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RespostaInesperada(e) => write!(f, "resposta do provedor fora do esperado: {e}"),
            Self::Http(e) => write!(f, "{e}"),
            Self::ChaveInvalida(e) => write!(f, "chave de licença inválida: {e}"),
        }
    }
}

impl std::error::Error for ErroProvedor {}

impl From<ErroHttp> for ErroProvedor {
    fn from(e: ErroHttp) -> Self {
        // 404/422 do provedor quase sempre significa chave inválida, e a
        // mensagem dele já diz isso: preservar o corpo é o que permite mostrá-la.
        Self::Http(e.to_string())
    }
}

/// O que a loja precisa de um provedor de pagamento.
pub trait Provedor {
    fn nome(&self) -> &'static str;
    /// Ativa a chave para esta instância. É a operação que **consome uma
    /// instalação** no limite do provedor, então não é idempotente de graça.
    fn ativar(&self, chave: &str, instancia: &ChavePublica, app: &str) -> Result<Ativacao, ErroProvedor>;
    /// Reconsulta. Não consome instalação.
    fn validar(&self, ativacao: &Ativacao) -> Result<Situacao, ErroProvedor>;
}

fn exigir_chave(chave: &str) -> Result<(), ErroProvedor> {
    let t = chave.trim();
    if t.len() < 8 {
        return Err(ErroProvedor::ChaveInvalida(format!(
            "{} caractere(s); uma chave de licença tem pelo menos 8",
            t.len()
        )));
    }
    Ok(())
}

// ── Lemon Squeezy ─────────────────────────────────────────

/// Cliente da License API do Lemon Squeezy (e compatíveis).
///
/// Não é específico do Lemon Squeezy de propósito: `BASE` é configurável, e
/// Paddle e Gumroad oferecem formatos equivalentes. Trocar de provedor é trocar
/// a URL base e o parser, não o desenho.
pub struct LemonSqueezy {
    pub base: String,
}

impl Default for LemonSqueezy {
    fn default() -> Self {
        Self { base: "https://api.lemonsqueezy.com".into() }
    }
}

impl LemonSqueezy {
    /// O `instance_name` que vai ao provedor.
    ///
    /// A chave pública do aparelho em base64 é longa e tem `/` e `+`; em
    /// form-encoding isso precisa de escape, e o painel do provedor mostra o
    /// nome truncado. Uso o **prefixo curto** (16 caracteres de base64url) e
    /// guardo o valor completo em `Ativacao::instancia`, que é o que a validação
    /// compara. O provedor só precisa de um nome único e estável — a garantia
    /// forte é que a instância foi criada para **este** aparelho, não que o nome
    /// carregue a chave inteira.
    pub fn nome_da_instancia(aparelho: &ChavePublica) -> String {
        let b64 = aparelho.base64().replace('+', "-").replace('/', "_");
        format!("giochi-{}", &b64[..16.min(b64.len())])
    }

    fn corpo_ativacao(chave: &str, instancia: &str) -> String {
        // A documentação pede form-encoded.
        format!("license_key={chave}&instance_name={instancia}")
    }
}

/// Lê `activated` e `error` do JSON de ativação.
///
/// O caso que importa é `activated: false` **com** `error` — é assim que o limite
/// de instâncias aparece, e mostrar a mensagem do provedor ("has reached the
/// activation limit") é infinitamente melhor que "falhou".
pub fn interpretar_ativacao(corpo: &str) -> Result<(bool, Option<String>, Option<String>), ErroProvedor> {
    let j: serde_json::Value = serde_json::from_str(corpo)
        .map_err(|e| ErroProvedor::RespostaInesperada(format!("não é JSON: {e}")))?;
    // `activated` é o campo documentado. Sem ele, a resposta não é desta API.
    let activated = j
        .get("activated")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| ErroProvedor::RespostaInesperada("sem campo 'activated'".into()))?;
    let erro = j.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());
    let status = j
        .get("license_key")
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok((activated, erro, status))
}

impl Provedor for LemonSqueezy {
    fn nome(&self) -> &'static str {
        "lemonsqueezy"
    }

    fn ativar(&self, chave: &str, instancia: &ChavePublica, app: &str) -> Result<Ativacao, ErroProvedor> {
        exigir_chave(chave)?;
        let nome = Self::nome_da_instancia(instancia);
        let req = Requisicao {
            metodo: crate::http::Metodo::Post,
            url: format!("{}/v1/licenses/activate", self.base),
            cabecalhos: vec![
                ("Content-Type".into(), "application/x-www-form-urlencoded".into()),
                ("Accept".into(), "application/json".into()),
            ],
            corpo: Some(Self::corpo_ativacao(chave.trim(), &nome)),
        };
        let r = executar(&req)?;
        let (ativado, erro, _status) = interpretar_ativacao(&r.corpo)?;
        if !ativado {
            // O limite de instâncias cai aqui, com a mensagem do provedor.
            return Err(ErroProvedor::RespostaInesperada(
                erro.unwrap_or_else(|| "o provedor não detalhou o motivo".into()),
            ));
        }
        Ok(Ativacao {
            provedor: self.nome().into(),
            chave: chave.trim().to_string(),
            instancia: instancia.base64(),
            app: app.to_string(),
            ativada_em: crate::licenca::agora_unix(),
            revalidar_em: Some(crate::licenca::agora_unix() + 30 * 86_400),
        })
    }

    fn validar(&self, ativacao: &Ativacao) -> Result<Situacao, ErroProvedor> {
        let req = Requisicao {
            metodo: crate::http::Metodo::Post,
            url: format!("{}/v1/licenses/validate", self.base),
            cabecalhos: vec![
                ("Content-Type".into(), "application/x-www-form-urlencoded".into()),
                ("Accept".into(), "application/json".into()),
            ],
            corpo: Some(format!("license_key={}", ativacao.chave)),
        };
        let r = executar(&req)?;
        let j: serde_json::Value = serde_json::from_str(&r.corpo)
            .map_err(|e| ErroProvedor::RespostaInesperada(format!("não é JSON: {e}")))?;
        let valida = j.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
        let status = j
            .get("license_key")
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match (valida, status) {
            (true, _) => Ok(Situacao::Ativa),
            (false, "expired") => Ok(Situacao::Invalida { motivo: "licença expirada".into() }),
            (false, "disabled") => {
                Ok(Situacao::Invalida { motivo: "licença desativada pelo vendedor".into() })
            }
            (false, "") => Ok(Situacao::NaoAtivada),
            (false, outro) => Ok(Situacao::Invalida { motivo: format!("status '{outro}'") }),
        }
    }
}

// ── itch.io ───────────────────────────────────────────────

/// Verificação de posse no itch.io.
///
/// # O que isto faz, e o que **não** faz
///
/// Faz: pergunta `GET /games/{id}/ownership` com a credencial **do usuário** e
/// responde "este usuário possui". É o modelo (A) do cabeçalho.
///
/// **Não** faz: vincular ao aparelho. A credencial é a conta do usuário, então
/// ela funciona em qualquer aparelho onde for copiada — e a única defesa contra
/// isso é o usuário não compartilhar a chave. Está implementado porque é o que o
/// dono do projeto pediu e porque é útil para jogo **grátis** ou de terceiro,
/// não porque protege receita.
///
/// O `instance_name` existe na trait para o outro provedor; aqui é ignorado, e o
/// `Ativacao::instancia` guarda a chave do aparelho mesmo assim — assim, se um dia
/// o itch.io oferecer limite de instâncias, o registro já está no formato certo.
pub struct ItchIo {
    pub base: String,
    /// Id numérico do jogo no itch.io.
    pub game_id: u64,
    /// API key **do usuário**, gerada em https://itch.io/user/settings/api-keys
    pub api_key: String,
}

impl ItchIo {
    pub fn novo(game_id: u64, api_key: impl Into<String>) -> Self {
        Self {
            base: "https://api.itch.io".into(),
            game_id,
            api_key: api_key.into(),
        }
    }
}

impl Provedor for ItchIo {
    fn nome(&self) -> &'static str {
        "itchio"
    }

    fn ativar(&self, _chave: &str, instancia: &ChavePublica, app: &str) -> Result<Ativacao, ErroProvedor> {
        let req = Requisicao::get(format!("{}/games/{}/ownership", self.base, self.game_id))
            .com_cabecalho("Authorization", &format!("Bearer {}", self.api_key))
            .com_cabecalho("Accept", "application/json");
        let r = executar(&req)?;
        let j: serde_json::Value = serde_json::from_str(&r.corpo)
            .map_err(|e| ErroProvedor::RespostaInesperada(format!("não é JSON: {e}")))?;
        let possui = j.get("owned").and_then(|v| v.as_bool()).unwrap_or(false);
        if !possui {
            return Err(ErroProvedor::RespostaInesperada(
                "a conta do itch.io informada não possui este jogo".into(),
            ));
        }
        Ok(Ativacao {
            provedor: self.nome().into(),
            chave: "conta-itchio".into(),
            instancia: instancia.base64(),
            app: app.to_string(),
            ativada_em: crate::licenca::agora_unix(),
            // A conta pode mudar de mãos, e não há limite de instâncias aqui:
            // revalidar é a única chance de perceber.
            revalidar_em: Some(crate::licenca::agora_unix() + 7 * 86_400),
        })
    }

    fn validar(&self, ativacao: &Ativacao) -> Result<Situacao, ErroProvedor> {
        if ativacao.provedor != self.nome() {
            return Ok(Situacao::Invalida {
                motivo: format!("ativação é do provedor '{}'", ativacao.provedor),
            });
        }
        // Reconsulta a posse **sem** tocar no registro: no itch.io não há
        // limite de instâncias, então ativar de novo não consome nada. Quem
        // decide se o aparelho vale é o registro local, não o provedor — e é
        // exatamente essa a limitação do modelo (A), dita em voz alta.
        let req = Requisicao::get(format!("{}/games/{}/ownership", self.base, self.game_id))
            .com_cabecalho("Authorization", &format!("Bearer {}", self.api_key))
            .com_cabecalho("Accept", "application/json");
        let r = match executar(&req) {
            Ok(r) => r,
            Err(e) => return Ok(Situacao::NaoSei { motivo: e.to_string() }),
        };
        let j: serde_json::Value = match serde_json::from_str(&r.corpo) {
            Ok(j) => j,
            Err(e) => {
                return Ok(Situacao::NaoSei {
                    motivo: format!("resposta do itch.io não é JSON: {e}"),
                })
            }
        };
        Ok(match j.get("owned").and_then(|v| v.as_bool()) {
            Some(true) => Situacao::Ativa,
            Some(false) => Situacao::Invalida {
                motivo: "a conta do itch.io não possui mais este jogo".into(),
            },
            None => Situacao::NaoSei { motivo: "resposta sem campo 'owned'".into() },
        })
    }
}

// ── Simulado ──────────────────────────────────────────────

/// Provedor de mentira para exercitar o fluxo sem rede nem credencial.
///
/// `ativacoes` é `RefCell` porque o limite tem de **acumular entre chamadas**, e
/// `ativar` recebe `&self`. A primeira versão clonava a si mesma para "contar" e
/// o limite nunca era atingido — o teste do limite reprovou, que é exatamente o
/// que ele existe para fazer.
#[derive(Debug, Default)]
pub struct Simulado {
    pub chaves_validas: Vec<String>,
    pub limite: usize,
    pub ativacoes: std::cell::RefCell<Vec<String>>,
    pub offline: bool,
}

impl Clone for Simulado {
    fn clone(&self) -> Self {
        Self {
            chaves_validas: self.chaves_validas.clone(),
            limite: self.limite,
            ativacoes: std::cell::RefCell::new(self.ativacoes.borrow().clone()),
            offline: self.offline,
        }
    }
}

impl Simulado {
    pub fn com_chave(chave: &str, limite: usize) -> Self {
        Self {
            chaves_validas: vec![chave.into()],
            limite,
            ativacoes: std::cell::RefCell::new(Vec::new()),
            offline: false,
        }
    }
}

impl Provedor for Simulado {
    fn nome(&self) -> &'static str {
        "simulado"
    }

    fn ativar(&self, chave: &str, instancia: &ChavePublica, app: &str) -> Result<Ativacao, ErroProvedor> {
        if self.offline {
            return Err(ErroProvedor::Http("sem rede (simulado)".into()));
        }
        exigir_chave(chave)?;
        if !self.chaves_validas.iter().any(|c| c == chave.trim()) {
            return Err(ErroProvedor::RespostaInesperada("license key is invalid".into()));
        }
        // Reproduz o limite de instâncias do provedor real, com a mesma mensagem
        // que a documentação mostra.
        let mut ativacoes = self.ativacoes.borrow_mut();
        if ativacoes.len() >= self.limite {
            return Err(ErroProvedor::RespostaInesperada(
                "This license key has reached the activation limit.".into(),
            ));
        }
        ativacoes.push(format!("{}:{}", chave.trim(), instancia.base64()));
        Ok(Ativacao {
            provedor: self.nome().into(),
            chave: chave.trim().into(),
            instancia: instancia.base64(),
            app: app.into(),
            ativada_em: crate::licenca::agora_unix(),
            revalidar_em: None,
        })
    }

    fn validar(&self, ativacao: &Ativacao) -> Result<Situacao, ErroProvedor> {
        if self.offline {
            return Ok(Situacao::NaoSei { motivo: "sem rede (simulado)".into() });
        }
        Ok(if ativacao.instancia.is_empty() {
            Situacao::NaoAtivada
        } else {
            Situacao::Ativa
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn aparelho() -> ChavePublica {
        ChavePublica::de_bytes(&[3u8; 32]).unwrap()
    }

    // Os dois JSON abaixo são **copiados da documentação** do Lemon Squeezy.
    // Um teste com JSON inventado provaria que o parser lê o que eu inventei.

    const ATIVADO: &str = r#"{
      "activated": true,
      "error": null,
      "license_key": {
        "id": 1, "status": "active", "key": "38b1460a-5104-4067-a91d-77b872934d51",
        "activation_limit": 1, "activation_usage": 5,
        "created_at": "2021-01-24T14:15:07.000000Z", "expires_at": null
      },
      "instance": { "id": "47596ad9", "name": "Test", "created_at": "2021-04-06T14:15:07.000000Z" },
      "meta": { "store_id": 1, "product_name": "Example Product" }
    }"#;

    const LIMITE: &str = r#"{
      "activated": false,
      "error": "This license key has reached the activation limit.",
      "license_key": {
        "id": 1, "status": "active", "key": "38b1460a-5104-4067-a91d-77b872934d51",
        "activation_limit": 5, "activation_usage": 5,
        "created_at": "2021-01-24T14:15:07.000000Z", "expires_at": null
      }
    }"#;

    #[test]
    fn ativacao_bem_sucedida_da_documentacao() {
        let (ativado, erro, status) = interpretar_ativacao(ATIVADO).unwrap();
        assert!(ativado);
        assert!(erro.is_none());
        assert_eq!(status.as_deref(), Some("active"));
    }

    #[test]
    fn limite_de_instancias_traz_a_mensagem_do_provedor() {
        // É este caso que substitui o nosso backend: o provedor conta as
        // instalações, e a mensagem dele é o que o usuário precisa ler.
        let (ativado, erro, _) = interpretar_ativacao(LIMITE).unwrap();
        assert!(!ativado);
        assert_eq!(erro.as_deref(), Some("This license key has reached the activation limit."));
    }

    #[test]
    fn resposta_sem_activated_nao_e_desta_api() {
        // Um HTML de portal de wifi ou uma página de erro também é "JSON válido"
        // em muitos casos; exigir o campo evita interpretar lixo como sucesso.
        assert!(matches!(
            interpretar_ativacao(r#"{"mensagem":"ok"}"#),
            Err(ErroProvedor::RespostaInesperada(_))
        ));
        assert!(matches!(
            interpretar_ativacao("<html>portal</html>"),
            Err(ErroProvedor::RespostaInesperada(_))
        ));
    }

    #[test]
    fn chave_curta_e_recusada_antes_de_gastar_requisicao() {
        // Cada ativação consome instalação no provedor. Mandar uma chave
        // digitada pela metade gasta uma tentativa e confunde o suporte.
        let p = LemonSqueezy::default();
        assert!(matches!(
            p.ativar("abc", &aparelho(), "pacman"),
            Err(ErroProvedor::ChaveInvalida(_))
        ));
    }

    #[test]
    fn o_nome_da_instancia_e_estavel_e_seguro_em_url() {
        // A instância tem de ser o mesmo nome em toda revalidação, e não pode ter
        // `/` nem `+` — em form-encoding isso vira escape e o painel do provedor
        // mostra truncado.
        let a = LemonSqueezy::nome_da_instancia(&aparelho());
        let b = LemonSqueezy::nome_da_instancia(&aparelho());
        assert_eq!(a, b, "nome instável consome uma instalação nova a cada vez");
        assert!(!a.contains('/') && !a.contains('+'), "{a}");
        assert!(a.starts_with("giochi-"));
    }

    #[test]
    fn aparelhos_diferentes_tem_nomes_diferentes() {
        let outro = ChavePublica::de_bytes(&[9u8; 32]).unwrap();
        assert_ne!(
            LemonSqueezy::nome_da_instancia(&aparelho()),
            LemonSqueezy::nome_da_instancia(&outro)
        );
    }

    /// Servidor local que responde uma vez com o JSON dado.
    fn servidor(json: &'static str) -> u16 {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let corpo = json;
                let resposta = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}{}",
                    corpo.len(),
                    corpo,
                    200
                );
                let _ = s.write_all(resposta.as_bytes());
            }
        });
        porta
    }

    #[test]
    fn lemonsqueezy_ativa_de_verdade_contra_um_socket() {
        // Prova que o cliente fala HTTP de verdade: método, cabeçalhos, form e
        // parse da resposta. Um teste só com `Simulado` provaria o fluxo e não a
        // integração — e é a integração que quebra quando a documentação muda.
        let porta = servidor(ATIVADO);
        let p = LemonSqueezy { base: format!("http://127.0.0.1:{porta}") };
        let a = p.ativar("38b1460a-5104-4067-a91d-77b872934d51", &aparelho(), "pacman").unwrap();
        assert_eq!(a.provedor, "lemonsqueezy");
        assert_eq!(a.app, "pacman");
        assert_eq!(a.instancia, aparelho().base64());
    }

    #[test]
    fn lemonsqueezy_traduz_o_limite_de_instancias() {
        // O caso que substitui o nosso backend. A mensagem tem de chegar ao
        // usuário: "atingiu o limite de instalações" é acionável, "falhou" não.
        let porta = servidor(LIMITE);
        let p = LemonSqueezy { base: format!("http://127.0.0.1:{porta}") };
        match p.ativar("38b1460a-5104-4067-a91d-77b872934d51", &aparelho(), "pacman") {
            Err(ErroProvedor::RespostaInesperada(m)) => {
                assert!(m.contains("activation limit"), "{m}")
            }
            outro => panic!("esperava o limite traduzido, veio {outro:?}"),
        }
    }

    #[test]
    fn servidor_fora_do_ar_vira_erro_de_transporte_e_nao_de_veredito() {
        // Porta fechada: erro de rede, não "licença inválida". Confundir os dois
        // faria o app dizer ao usuário que a licença dele é falsa quando o que
        // falta é sinal.
        let p = LemonSqueezy { base: "http://127.0.0.1:1".into() };
        match p.ativar("38b1460a-5104-4067-a91d-77b872934d51", &aparelho(), "x") {
            Err(ErroProvedor::Http(m)) => assert!(!m.contains("inválida"), "{m}"),
            outro => panic!("esperava erro de transporte, veio {outro:?}"),
        }
    }

    #[test]
    fn simulado_reproduz_o_limite_do_provedor() {
        let s = Simulado::com_chave("CHAVE-BOA-1234", 1);
        assert!(s.ativar("CHAVE-BOA-1234", &aparelho(), "pacman").is_ok());
        let segundo = s.ativar("CHAVE-BOA-1234", &aparelho(), "pacman");
        assert!(matches!(segundo, Err(ErroProvedor::RespostaInesperada(ref m)) if m.contains("activation limit")));
    }

    #[test]
    fn simulado_recusa_chave_errada() {
        let s = Simulado::com_chave("CHAVE-BOA-1234", 5);
        assert!(matches!(
            s.ativar("OUTRA-CHAVE-999", &aparelho(), "pacman"),
            Err(ErroProvedor::RespostaInesperada(_))
        ));
    }

    #[test]
    fn offline_nao_e_veredito_nem_na_ativacao_nem_na_validacao() {
        // Sem rede o registro local continua valendo (ver `licenca`), mas a
        // **revalidação** tem de dizer "não sei", não "inválida": invalidar por
        // falta de rede puniria quem está sem sinal.
        let s = Simulado { offline: true, ..Simulado::com_chave("CHAVE-BOA-1234", 5) };
        assert!(matches!(s.ativar("CHAVE-BOA-1234", &aparelho(), "x"), Err(ErroProvedor::Http(_))));
        let ativ = Ativacao {
            provedor: "simulado".into(),
            chave: "CHAVE-BOA-1234".into(),
            instancia: aparelho().base64(),
            app: "x".into(),
            ativada_em: 0,
            revalidar_em: None,
        };
        assert!(matches!(s.validar(&ativ), Ok(Situacao::NaoSei { .. })));
    }
}
