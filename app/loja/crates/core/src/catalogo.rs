//! Catálogo assinado: a lista do que existe, à venda, e por quanto.
//!
//! # Por que o catálogo é assinado
//!
//! Ele vem pela rede. Sem assinatura, quem estiver no meio do caminho troca a
//! URL de um item por uma URL própria e o usuário instala o que o atacante
//! quiser — e a assinatura do `.giochi` não salva, porque o atacante assina o
//! pacote **dele** com a chave **dele**. A verificação do catálogo é o que amarra
//! as duas: só entram pacotes cuja assinatura confere com uma chave que veio de
//! um catálogo assinado.
//!
//! É a mesma lógica de cadeia de confiança de um repositório de pacotes, e é por
//! isso que o catálogo carrega a **chave pública do pacote** em vez de nós
//! conhecermos cada desenvolvedor de antemão.
//!
//! # `invalida_antes`: como matar uma chave vazada
//!
//! Se a chave privada de assinatura vazar, não há como "despublicar" o que já
//! saiu. O que dá para fazer é declarar que pacotes assinados antes de um
//! instante não valem mais, e o catálogo assinado — que é a única coisa que o
//! aparelho aceita — leva essa data. O aparelho recusa o que for anterior.
//!
//! Sem isso, uma chave vazada vale para sempre, e é o cenário mais provável de
//! "alguém roubou os jogos".

use crate::chave::ChavePublica;
use crate::preco::Preco;

pub const VERSAO: u8 = 1;

/// Um app à venda.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub slug: String,
    pub titulo: String,
    /// Ano de lançamento. `0` = desconhecido, e a UI mostra `—`.
    ///
    /// O launcher já confundiu isto com o índice do cartão uma vez. Aqui o campo
    /// é opcional de propósito: preencher com um número interno é pior que
    /// mostrar que não se sabe.
    #[serde(default)]
    pub ano: u16,
    #[serde(default)]
    pub descricao: String,
    pub preco: Preco,
    pub versao: String,
    pub tamanho_bytes: u64,
    /// SHA-256 do `.giochi`, em hex.
    pub sha256: String,
    /// De onde baixar. HTTPS; o esquema é conferido por `Item::url_valida`.
    pub url: String,
    /// Assinatura do pacote, em base64 (64 bytes crus).
    pub assinatura: String,
    /// Chave que assinou **este** pacote.
    ///
    /// Um item pode ser assinado pela Giochi (chave de catálogo) ou por um
    /// terceiro aprovado. Ter o campo por item é o que permite as duas coisas
    /// sem multi-assinatura no formato do `.giochi`.
    pub chave_pacote: String,
}

impl Item {
    /// Recusa `http://`. Um catálogo assinado que aponta para HTTP simplesmente
    /// não é seguido: o conteúdo ainda seria verificado, mas anunciar a compra
    /// e o que o usuário baixa em claro não se justifica.
    pub fn url_valida(&self) -> bool {
        self.url.starts_with("https://")
    }

    pub fn chave(&self) -> Result<ChavePublica, crate::chave::ErroDeChave> {
        ChavePublica::de_texto(&self.chave_pacote)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Catalogo {
    pub versao: u8,
    pub gerado_em: u64,
    /// Pacotes assinados antes deste instante são recusados. `None` = sem corte.
    #[serde(default)]
    pub invalida_antes: Option<u64>,
    pub itens: Vec<Item>,
}

impl Catalogo {
    pub fn mensagem(&self) -> Vec<u8> {
        // Mesma escolha da licença: texto canônico, não JSON serializado.
        // Reordenar chaves não pode mudar o que foi assinado.
        let mut s = String::new();
        s.push_str(&format!("giochi-catalogo-v{}\n", self.versao));
        s.push_str(&format!("gerado={}\n", self.gerado_em));
        match self.invalida_antes {
            Some(t) => s.push_str(&format!("invalida_antes={t}\n")),
            None => s.push_str("invalida_antes=-\n"),
        }
        // Itens em ordem de slug: a ordem de exibição é da UI, não do formato.
        // Sem isto, dois catálogos com o mesmo conteúdo assinariam diferente.
        let mut itens: Vec<&Item> = self.itens.iter().collect();
        itens.sort_by(|a, b| a.slug.cmp(&b.slug));
        for i in itens {
            s.push_str(&format!(
                "item={}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                i.slug,
                i.titulo,
                i.ano,
                i.preco.centavos,
                match i.preco.moeda {
                    crate::preco::Moeda::BRL => "BRL",
                    crate::preco::Moeda::USD => "USD",
                    crate::preco::Moeda::EUR => "EUR",
                },
                i.versao,
                i.tamanho_bytes,
                i.sha256,
                i.url,
                i.assinatura,
                i.chave_pacote,
            ));
        }
        s.into_bytes()
    }

    pub fn item(&self, slug: &str) -> Option<&Item> {
        self.itens.iter().find(|i| i.slug == slug)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CatalogoAssinado {
    pub catalogo: Catalogo,
    pub assinatura: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroCatalogo {
    /// A assinatura não confere com a chave que o aparelho conhece.
    AssinaturaInvalida,
    /// Formato mais novo do que este binário entende.
    VersaoDesconhecida(u8),
    /// O catálogo é válido mas assinado antes do corte de revogação.
    Revogado { assinado_em: u64, corte: u64 },
    /// O catálogo em si está malformado.
    Malformado(String),
}

impl std::fmt::Display for ErroCatalogo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AssinaturaInvalida => write!(
                f,
                "catálogo com assinatura inválida — não foi a Giochi que publicou, \
                 ou alguém está no meio do caminho"
            ),
            Self::VersaoDesconhecida(v) => {
                write!(f, "catálogo v{v}; este app entende v{VERSAO} (atualize a loja)")
            }
            Self::Revogado { assinado_em, corte } => write!(
                f,
                "catálogo de {assinado_em} está revogado (corte em {corte}) — \
                 a chave de assinatura foi trocada"
            ),
            Self::Malformado(e) => write!(f, "catálogo malformado: {e}"),
        }
    }
}

impl std::error::Error for ErroCatalogo {}

impl CatalogoAssinado {
    pub fn de_json(texto: &str) -> Result<Self, ErroCatalogo> {
        serde_json::from_str(texto).map_err(|e| ErroCatalogo::Malformado(e.to_string()))
    }

    pub fn para_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    /// Verifica a assinatura e o corte de revogação.
    ///
    /// A ordem importa: assinatura primeiro. Um catálogo não assinado não tem
    /// `gerado_em` confiável, e usá-lo para decidir revogação seria deixar o
    /// atacante escolher a data.
    pub fn verificar(&self, chave_do_catalogo: &ChavePublica) -> Result<&Catalogo, ErroCatalogo> {
        if self.catalogo.versao != VERSAO {
            return Err(ErroCatalogo::VersaoDesconhecida(self.catalogo.versao));
        }
        let assinatura = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            self.assinatura.trim(),
        )
        .map_err(|e| ErroCatalogo::Malformado(format!("assinatura não é base64: {e}")))?;

        if !chave_do_catalogo.verifica(&self.catalogo.mensagem(), &assinatura) {
            return Err(ErroCatalogo::AssinaturaInvalida);
        }
        if let Some(corte) = self.catalogo.invalida_antes {
            if self.catalogo.gerado_em < corte {
                return Err(ErroCatalogo::Revogado {
                    assinado_em: self.catalogo.gerado_em,
                    corte,
                });
            }
        }
        Ok(&self.catalogo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::preco::Moeda;
    use ed25519_dalek::{Signer, SigningKey};

    fn chave() -> SigningKey {
        SigningKey::from_bytes(&[17u8; 32])
    }
    fn publica_de(k: &SigningKey) -> ChavePublica {
        ChavePublica::de_bytes(&k.verifying_key().to_bytes()).unwrap()
    }

    fn item(slug: &str) -> Item {
        Item {
            slug: slug.into(),
            titulo: "Jogo".into(),
            ano: 1988,
            descricao: "d".into(),
            preco: Preco::new(1990, Moeda::BRL),
            versao: "1.0.0".into(),
            tamanho_bytes: 81408,
            sha256: "31f9d6491cfae8bc".into(),
            url: format!("https://cdn.giochi.example/{slug}.giochi"),
            assinatura: "AAA=".into(),
            chave_pacote: publica_de(&chave()).base64(),
        }
    }

    fn assinado(cat: Catalogo) -> CatalogoAssinado {
        let a = chave().sign(&cat.mensagem());
        CatalogoAssinado {
            catalogo: cat,
            assinatura: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, a.to_bytes()),
        }
    }

    fn catalogo() -> Catalogo {
        Catalogo {
            versao: VERSAO,
            gerado_em: 1_700_000_000,
            invalida_antes: None,
            itens: vec![item("pacman"), item("volley")],
        }
    }

    #[test]
    fn catalogo_legitimo_verifica() {
        let c = assinado(catalogo());
        let ok = c.verificar(&publica_de(&chave())).unwrap();
        assert_eq!(ok.itens.len(), 2);
    }

    #[test]
    fn item_adulterado_e_recusado() {
        // O ataque: manter o catálogo assinado e trocar o preço.
        let mut c = assinado(catalogo());
        c.catalogo.itens[0].preco = Preco::brl(1);
        assert_eq!(
            c.verificar(&publica_de(&chave())),
            Err(ErroCatalogo::AssinaturaInvalida)
        );
    }

    #[test]
    fn trocar_a_url_do_pacote_e_recusado() {
        // O ataque que motiva tudo: apontar para um pacote do atacante.
        let mut c = assinado(catalogo());
        c.catalogo.itens[0].url = "https://atacante.example/pacman.giochi".into();
        assert_eq!(
            c.verificar(&publica_de(&chave())),
            Err(ErroCatalogo::AssinaturaInvalida)
        );
    }

    #[test]
    fn catalogo_assinado_por_outra_chave_e_recusado() {
        let cat = catalogo();
        let outro = SigningKey::from_bytes(&[99u8; 32]);
        let a = outro.sign(&cat.mensagem());
        let c = CatalogoAssinado {
            catalogo: cat,
            assinatura: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, a.to_bytes()),
        };
        assert_eq!(
            c.verificar(&publica_de(&chave())),
            Err(ErroCatalogo::AssinaturaInvalida)
        );
    }

    #[test]
    fn a_ordem_dos_itens_nao_muda_o_que_foi_assinado() {
        // A ordem é da interface, não do formato. Sem ordenar na mensagem, dois
        // catálogos idênticos em conteúdo assinariam diferente e um deles
        // falharia ao verificar depois de reordenado.
        let a = catalogo();
        let mut b = a.clone();
        b.itens.reverse();
        assert_eq!(a.mensagem(), b.mensagem());
    }

    #[test]
    fn revogacao_por_data_recusa_catalogo_antigo() {
        // Chave vazada: o corte é a única forma de parar o que já saiu.
        let mut cat = catalogo();
        cat.gerado_em = 1_000;
        cat.invalida_antes = Some(2_000);
        let c = assinado(cat);
        assert_eq!(
            c.verificar(&publica_de(&chave())),
            Err(ErroCatalogo::Revogado { assinado_em: 1_000, corte: 2_000 })
        );
    }

    #[test]
    fn data_de_revogacao_nao_pode_ser_escolhida_pelo_atacante() {
        // Se a revogação fosse checada antes da assinatura, bastaria um catálogo
        // não assinado com `invalida_antes` alto para derrubar a loja.
        let mut cat = catalogo();
        cat.invalida_antes = Some(9_999_999_999);
        let c = CatalogoAssinado { catalogo: cat, assinatura: "AAAA".into() };
        assert_eq!(
            c.verificar(&publica_de(&chave())),
            Err(ErroCatalogo::AssinaturaInvalida)
        );
    }

    #[test]
    fn versao_futura_pede_atualizacao_e_nao_falha_em_silencio() {
        let mut cat = catalogo();
        cat.versao = 7;
        let c = assinado(cat);
        assert_eq!(c.verificar(&publica_de(&chave())), Err(ErroCatalogo::VersaoDesconhecida(7)));
    }

    #[test]
    fn item_de_http_simples_e_marcado_invalido() {
        let mut i = item("x");
        assert!(i.url_valida());
        i.url = "http://cdn.giochi.example/x.giochi".into();
        assert!(!i.url_valida());
    }

    #[test]
    fn ida_e_volta_por_json_preserva_a_verificacao() {
        let c = assinado(catalogo());
        let texto = c.para_json();
        let voltou = CatalogoAssinado::de_json(&texto).unwrap();
        assert!(voltou.verificar(&publica_de(&chave())).is_ok());
    }

    #[test]
    fn encontrar_item_por_slug() {
        let c = catalogo();
        assert_eq!(c.item("pacman").unwrap().titulo, "Jogo");
        assert!(c.item("nao-existe").is_none());
    }
}
