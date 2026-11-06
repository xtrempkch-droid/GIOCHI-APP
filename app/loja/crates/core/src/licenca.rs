//! Licença: a prova de que **este aparelho** comprou **este app**.
//!
//! # Por que ela é separada da assinatura do pacote
//!
//! A assinatura do `.giochi` responde "veio da Giochi?". Se ela fosse também a
//! resposta para "este usuário pagou?", então **um** pacote assinado serviria
//! para todo mundo: bastaria copiar o arquivo. É exatamente o cenário que
//! preocupa o dono do projeto, e ele não se resolve com criptografia melhor —
//! resolve-se **separando as duas perguntas**.
//!
//! | | assinatura do pacote | licença |
//! | --- | --- | --- |
//! | assina | a Giochi (chave de catálogo) | a Giochi (chave de **licença**) |
//! | vale para | o arquivo | **um aparelho** |
//! | copiar para outro PC | funciona, e deve funcionar | **não** funciona |
//!
//! # O que "este aparelho" significa
//!
//! O aparelho gera um par **nele mesmo** e a privada nunca sai. A licença carrega
//! a pública. Copiar o `.giochi` e a licença para outro aparelho falha, porque a
//! pública do outro é diferente.
//!
//! Isso não é inquebrável, e não fingimos que seja: quem tem root no aparelho
//! pode copiar também a chave privada do aparelho, ou simplesmente rodar o
//! binário sem passar pelo launcher. O objetivo é que a **cópia casual** pare,
//! e que a proteção real seja o pagamento no servidor — não um cadeado no
//! cliente, que sempre perde.
//!
//! # Formato do payload: texto, não JSON
//!
//! A mensagem assinada é montada campo a campo (`mensagem()`). Assinar o JSON
//! serializado funcionaria até alguém reserializar com outro espaçamento ou outra
//! ordem de chaves, e a verificação passaria a falhar sem o conteúdo ter mudado.
//! Este projeto já pagou esse preço uma vez, com a assinatura do `.giochi` em
//! base64 contra os 64 bytes crus: dois formatos da mesma coisa, cada lado
//! testando a si mesmo.

use std::fmt;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};

use crate::chave::{ChavePublica, TAMANHO_ASSINATURA};

/// Versão do formato. Um verificador antigo recusa em vez de adivinhar.
pub const VERSAO: u8 = 1;

/// Tolerância para relógio do aparelho, em segundos.
///
/// O aparelho liga sem hora certa e o NTP corrige alguns segundos depois. Uma
/// licença emitida "no futuro" por 5 minutos é relógio atrasado, não fraude.
pub const TOLERANCIA_RELOGIO: u64 = 300;

/// O que a licença afirma.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Licenca {
    pub versao: u8,
    /// Slug do app. A licença de um app não vale para outro.
    pub app: String,
    /// Versão do app licenciada. Vazio = qualquer versão do slug.
    pub versao_app: String,
    /// Chave pública do aparelho, em base64 (32 bytes).
    pub aparelho: String,
    /// Id do pedido que originou a licença. Rastreabilidade, não segurança.
    pub pedido: String,
    pub emitida_em: u64,
    /// `None` = perpétua. Perpétua não depende do relógio do aparelho, e isso é
    /// a maior parte da robustez que se ganha de graça.
    pub expira_em: Option<u64>,
}

impl Licenca {
    /// A mensagem canônica que é assinada.
    ///
    /// Determinística e legível. Qualquer mudança de conteúdo muda a mensagem;
    /// nenhuma mudança de formatação muda.
    pub fn mensagem(&self) -> Vec<u8> {
        format!(
            "giochi-licenca-v{}\napp={}\nversao={}\naparelho={}\npedido={}\nemitida={}\nexpira={}\n",
            self.versao,
            self.app,
            self.versao_app,
            self.aparelho,
            self.pedido,
            self.emitida_em,
            match self.expira_em {
                Some(t) => t.to_string(),
                None => "-".to_string(),
            }
        )
        .into_bytes()
    }
}

/// Licença + a assinatura dela. É o que fica no aparelho.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LicencaAssinada {
    pub licenca: Licenca,
    /// base64 dos 64 bytes crus.
    pub assinatura: String,
}

/// O veredito da verificação. Quatro valores, não dois.
///
/// `NaoSei` existe porque "não consegui julgar" é diferente de "recusei". O
/// relógio do aparelho é o caso concreto: sem hora confiável, uma licença
/// vencida parece válida e uma válida parece vencida. Responder `false` para os
/// dois esconde qual é o problema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredito {
    /// Confere, é deste aparelho, e não venceu.
    Valida,
    /// A assinatura não confere: licença adulterada, ou não é da Giochi.
    AssinaturaInvalida,
    /// Assinada pela Giochi, mas para outro aparelho.
    OutroAparelho { esperado: String, encontrado: String },
    /// Assinada para este aparelho, mas de outro app.
    OutroApp { esperado: String, encontrado: String },
    /// Venceu.
    Expirada { expirou_em: u64 },
    /// Não deu para julgar: formato novo, ou relógio fora da faixa.
    NaoSei { motivo: String },
}

impl Veredito {
    /// Só `Valida` libera. `NaoSei` **não** libera por omissão.
    ///
    /// É a escolha conservadora, e é deliberada: um verificador que aceita o que
    /// não entendeu transforma qualquer falha de configuração em porta aberta.
    pub fn libera(&self) -> bool {
        matches!(self, Self::Valida)
    }

    pub fn motivo(&self) -> String {
        match self {
            Self::Valida => "licença válida".into(),
            Self::AssinaturaInvalida => {
                "assinatura da licença não confere — não foi a Giochi que emitiu".into()
            }
            Self::OutroAparelho { .. } => {
                "esta licença foi emitida para outro aparelho".into()
            }
            Self::OutroApp { esperado, encontrado } => {
                format!("licença de '{encontrado}', não de '{esperado}'")
            }
            Self::Expirada { .. } => "licença vencida".into(),
            Self::NaoSei { motivo } => motivo.clone(),
        }
    }
}

impl fmt::Display for Veredito {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.motivo())
    }
}

impl LicencaAssinada {
    /// Emite e assina. **Papel do servidor.**
    ///
    /// Está aqui para que o fluxo seja testável sem servidor. Em produção a
    /// chave de licença não existe em máquina de cliente — se ela existir, a
    /// licença deixa de valer qualquer coisa.
    pub fn emitir(licenca: Licenca, chave_de_licenca: &SigningKey) -> Self {
        let assinatura = chave_de_licenca.sign(&licenca.mensagem());
        Self {
            licenca,
            assinatura: B64.encode(assinatura.to_bytes()),
        }
    }

    /// Confere a licença **neste** aparelho, agora.
    pub fn verificar(
        &self,
        chave_de_licenca: &ChavePublica,
        aparelho: &ChavePublica,
        agora: u64,
    ) -> Veredito {
        if self.licenca.versao != VERSAO {
            return Veredito::NaoSei {
                motivo: format!(
                    "licença no formato v{}, e este launcher entende v{VERSAO}",
                    self.licenca.versao
                ),
            };
        }

        let assinatura = match B64.decode(self.assinatura.trim()) {
            Ok(a) => a,
            Err(e) => {
                return Veredito::NaoSei {
                    motivo: format!("assinatura da licença não é base64: {e}"),
                }
            }
        };
        // O tamanho é conferido dentro de `verifica`, mas separar aqui permite
        // dizer *por que* recusou — 89 bytes é base64-gravado-como-texto.
        if assinatura.len() != TAMANHO_ASSINATURA {
            return Veredito::NaoSei {
                motivo: format!(
                    "assinatura da licença com {} bytes; Ed25519 tem {TAMANHO_ASSINATURA}",
                    assinatura.len()
                ),
            };
        }
        if !chave_de_licenca.verifica(&self.licenca.mensagem(), &assinatura) {
            return Veredito::AssinaturaInvalida;
        }

        // Só depois de a assinatura valer os campos merecem confiança.
        if self.licenca.aparelho != aparelho.base64() {
            return Veredito::OutroAparelho {
                esperado: aparelho.base64(),
                encontrado: self.licenca.aparelho.clone(),
            };
        }

        if let Some(expira) = self.licenca.expira_em {
            // Relógio muito atrás da emissão: não dá para saber se venceu.
            if agora.saturating_add(TOLERANCIA_RELOGIO) < self.licenca.emitida_em {
                return Veredito::NaoSei {
                    motivo: format!(
                        "relógio do aparelho ({agora}) está atrás da emissão ({})",
                        self.licenca.emitida_em
                    ),
                };
            }
            if agora > expira.saturating_add(TOLERANCIA_RELOGIO) {
                return Veredito::Expirada { expirou_em: expira };
            }
        }

        Veredito::Valida
    }

    /// Grava ao lado do app no aparelho.
    pub fn para_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn de_json(texto: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(texto)
    }
}

/// Confere se a licença é deste app.
///
/// Separado de `verificar` porque a comparação de app é política de quem chama:
/// o launcher sabe qual app está lançando, o servidor pode querer conferir
/// antes.
pub fn confere_app(licenca: &Licenca, slug: &str) -> Veredito {
    if licenca.app == slug {
        Veredito::Valida
    } else {
        Veredito::OutroApp {
            esperado: slug.to_string(),
            encontrado: licenca.app.clone(),
        }
    }
}

/// Relógio do aparelho: `0` quando o sistema não sabe a hora.
///
/// Não é `SystemTime::now()` direto porque em Rust `now()` antes de 1970 entra
/// em pânico em algumas plataformas, e o aparelho liga sem RTC.
pub fn agora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn chave_de_licenca() -> SigningKey {
        SigningKey::from_bytes(&[3u8; 32])
    }

    fn chave_do_aparelho() -> ChavePublica {
        let s = SigningKey::from_bytes(&[5u8; 32]);
        ChavePublica::de_bytes(&s.verifying_key().to_bytes()).unwrap()
    }

    fn publica_de(k: &SigningKey) -> ChavePublica {
        ChavePublica::de_bytes(&k.verifying_key().to_bytes()).unwrap()
    }

    fn licenca(aparelho: &ChavePublica) -> Licenca {
        Licenca {
            versao: VERSAO,
            app: "pacman".into(),
            versao_app: "1.0.0".into(),
            aparelho: aparelho.base64(),
            pedido: "PED-123".into(),
            emitida_em: 1_700_000_000,
            expira_em: None,
        }
    }

    #[test]
    fn licenca_legitima_deste_aparelho_e_valida() {
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let l = LicencaAssinada::emitir(licenca(&ap), &k);
        assert_eq!(l.verificar(&publica_de(&k), &ap, 1_700_000_100), Veredito::Valida);
        assert!(l.verificar(&publica_de(&k), &ap, 1_700_000_100).libera());
    }

    #[test]
    fn copiar_o_pacote_e_a_licenca_para_outro_aparelho_falha() {
        // O cenário que preocupa: alguém copia os arquivos para o aparelho do
        // vizinho. A assinatura continua perfeita; o aparelho é que não é.
        let k = chave_de_licenca();
        let meu = chave_do_aparelho();
        let outro = ChavePublica::de_bytes(&SigningKey::from_bytes(&[8u8; 32]).verifying_key().to_bytes()).unwrap();
        let l = LicencaAssinada::emitir(licenca(&meu), &k);

        let v = l.verificar(&publica_de(&k), &outro, 1_700_000_100);
        assert!(matches!(v, Veredito::OutroAparelho { .. }), "{v:?}");
        assert!(!v.libera());
    }

    #[test]
    fn licenca_adulterada_muda_a_mensagem_e_e_recusada() {
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = LicencaAssinada::emitir(licenca(&ap), &k);
        // Troca o app: se a assinatura não cobrisse o conteúdo, isto passaria.
        l.licenca.app = "jogo-caro".into();

        let v = l.verificar(&publica_de(&k), &ap, 1_700_000_100);
        assert_eq!(v, Veredito::AssinaturaInvalida);
    }

    #[test]
    fn licenca_emitida_por_outra_chave_e_recusada() {
        let ap = chave_do_aparelho();
        let falsario = SigningKey::from_bytes(&[42u8; 32]);
        let l = LicencaAssinada::emitir(licenca(&ap), &falsario);

        let v = l.verificar(&publica_de(&chave_de_licenca()), &ap, 1_700_000_100);
        assert_eq!(v, Veredito::AssinaturaInvalida);
    }

    #[test]
    fn reserializar_o_json_nao_invalida_a_licenca() {
        // O motivo de assinar texto canônico e não o JSON: ida e volta por
        // `serde_json` com espaçamento diferente tem de continuar valendo.
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let original = LicencaAssinada::emitir(licenca(&ap), &k);

        let compacto = serde_json::to_string(&original).unwrap();
        let voltou = LicencaAssinada::de_json(&compacto).unwrap();
        assert_eq!(voltou.verificar(&publica_de(&k), &ap, 1_700_000_100), Veredito::Valida);
    }

    #[test]
    fn licenca_vencida_e_recusada() {
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = licenca(&ap);
        l.expira_em = Some(1_700_000_000);
        let assinada = LicencaAssinada::emitir(l, &k);

        let v = assinada.verificar(&publica_de(&k), &ap, 1_700_000_000 + TOLERANCIA_RELOGIO + 1);
        assert!(matches!(v, Veredito::Expirada { .. }), "{v:?}");
        assert!(!v.libera());
    }

    #[test]
    fn dentro_da_tolerancia_ainda_vale() {
        // Venceu há 2 minutos e o relógio do aparelho pode estar 5 atrás:
        // recusar aqui seria hostil com quem só ligou o aparelho.
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = licenca(&ap);
        l.expira_em = Some(1_700_000_000);
        let assinada = LicencaAssinada::emitir(l, &k);

        let v = assinada.verificar(&publica_de(&k), &ap, 1_700_000_000 + 120);
        assert_eq!(v, Veredito::Valida);
    }

    #[test]
    fn perpetua_nao_depende_do_relogio() {
        // Relógio em 1970 (aparelho sem RTC) e licença perpétua: vale.
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let l = LicencaAssinada::emitir(licenca(&ap), &k);
        assert_eq!(l.verificar(&publica_de(&k), &ap, 0), Veredito::Valida);
    }

    #[test]
    fn relogio_atrasado_em_licenca_com_prazo_da_nao_sei_e_nao_libera() {
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = licenca(&ap);
        l.expira_em = Some(1_800_000_000);
        let assinada = LicencaAssinada::emitir(l, &k);

        // Relógio em 1970, emissão em 2023: não dá para julgar o vencimento.
        let v = assinada.verificar(&publica_de(&k), &ap, 0);
        assert!(matches!(v, Veredito::NaoSei { .. }), "{v:?}");
        assert!(!v.libera(), "NaoSei não pode liberar por omissão");
    }

    #[test]
    fn formato_futuro_e_recusado_com_motivo_e_nao_panico() {
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = licenca(&ap);
        l.versao = 99;
        let assinada = LicencaAssinada::emitir(l, &k);

        let v = assinada.verificar(&publica_de(&k), &ap, 1_700_000_100);
        assert!(matches!(v, Veredito::NaoSei { .. }), "{v:?}");
        assert!(v.motivo().contains("v99"), "{}", v.motivo());
    }

    #[test]
    fn assinatura_em_base64_duplo_nao_passa_calado() {
        // Base64 da string base64: tamanho diferente de 64 → NaoSei com motivo.
        let k = chave_de_licenca();
        let ap = chave_do_aparelho();
        let mut l = LicencaAssinada::emitir(licenca(&ap), &k);
        l.assinatura = B64.encode(l.assinatura.as_bytes());

        let v = l.verificar(&publica_de(&k), &ap, 1_700_000_100);
        assert!(matches!(v, Veredito::NaoSei { .. }), "{v:?}");
    }

    #[test]
    fn licenca_de_outro_app_e_recusada_pela_conferencia_de_app() {
        let l = licenca(&chave_do_aparelho());
        assert_eq!(confere_app(&l, "pacman"), Veredito::Valida);
        assert!(matches!(confere_app(&l, "volley"), Veredito::OutroApp { .. }));
    }

    #[test]
    fn a_mensagem_e_estavel_e_legivel() {
        let l = licenca(&chave_do_aparelho());
        let m = String::from_utf8(l.mensagem()).unwrap();
        assert!(m.starts_with("giochi-licenca-v1\n"));
        assert!(m.contains("app=pacman\n"));
        // Perpétua é "-", não "None": o formato não pode mudar com a linguagem.
        assert!(m.contains("expira=-\n"));
    }
}
