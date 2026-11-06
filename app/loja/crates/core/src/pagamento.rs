//! O pedido: máquina de estados que **não deixa** o cliente se declarar pago.
//!
//! # A regra que o tipo garante
//!
//! `Estado::Pago` carrega uma `LicencaAssinada`. Não existe outro jeito de
//! chegar lá: só o servidor consegue assinar uma licença, porque a chave dele
//! não está nesta máquina. Então "cliente diz que pagou" não compila — e essa é
//! a diferença entre uma loja e um botão que muda de cor.
//!
//! # O que a loja NUNCA vê
//!
//! Número de cartão. Cartão é pago numa página hospedada pelo provedor, aberta
//! no navegador do usuário: o dado nunca passa por este processo. Não é
//! preciosismo — é a diferença entre PCI-DSS "SAQ A" e "SAQ D", e implementar
//! formulário de cartão significa assumir a segunda.
//!
//! PIX é diferente e por isso é o caminho principal aqui: o pagamento é um
//! **pedido** (uma string), que podemos mostrar sem manipular credencial
//! nenhuma.
//!
//! # Por que a máquina de estados é explícita
//!
//! Uma loja com `bool pago` aceita transições como "expirado → pago" e
//! "recusado → pago" vindas de qualquer lugar do código. As transições aqui são
//! métodos que consomem `self` e devolvem `Result`: depois de expirar, o pedido
//! não volta a aceitar confirmação, e o tipo impede.

use std::fmt;

use crate::licenca::{LicencaAssinada, Veredito};
use crate::preco::Preco;

/// Cobrança PIX: o que o usuário precisa para pagar.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pix {
    /// O "copia e cola" (BR Code). É a UX principal do PIX, mais que o QR.
    pub copia_e_cola: String,
    pub valor: Preco,
    pub expira_em: u64,
    /// Nome de quem recebe, para o usuário conferir antes de confirmar.
    pub recebedor: String,
}

/// Link de checkout hospedado (cartão). O dado do cartão não passa por aqui.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CheckoutExterno {
    pub url: String,
    pub expira_em: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    /// Pedido criado, ainda sem instrução de pagamento.
    Criado,
    /// Instrução de pagamento em mãos, esperando o servidor confirmar.
    Aguardando {
        pix: Option<Pix>,
        checkout: Option<CheckoutExterno>,
        expira_em: u64,
    },
    /// Confirmado **pelo servidor**, com licença assinada.
    Pago { licenca: LicencaAssinada },
    Expirado,
    Recusado { motivo: String },
}

impl Estado {
    pub fn rotulo(&self) -> &'static str {
        match self {
            Self::Criado => "criado",
            Self::Aguardando { .. } => "aguardando pagamento",
            Self::Pago { .. } => "pago",
            Self::Expirado => "expirado",
            Self::Recusado { .. } => "recusado",
        }
    }

    /// Estados finais não mudam mais. Reabrir um pedido expirado é o caminho
    /// mais curto para entregar jogo sem pagamento.
    pub fn finalizado(&self) -> bool {
        matches!(self, Self::Pago { .. } | Self::Expirado | Self::Recusado { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransicaoInvalida {
    pub de: &'static str,
    pub para: &'static str,
}

impl fmt::Display for TransicaoInvalida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "de '{}' não vai para '{}'", self.de, self.para)
    }
}

impl std::error::Error for TransicaoInvalida {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    pub id: String,
    /// Slug do app comprado.
    pub app: String,
    pub preco: Preco,
    /// Chave pública do aparelho de destino, em base64.
    pub aparelho: String,
    pub criado_em: u64,
    estado: Estado,
}

impl Pedido {
    pub fn novo(
        id: impl Into<String>,
        app: impl Into<String>,
        preco: Preco,
        aparelho: impl Into<String>,
        criado_em: u64,
    ) -> Self {
        Self {
            id: id.into(),
            app: app.into(),
            preco,
            aparelho: aparelho.into(),
            criado_em,
            estado: Estado::Criado,
        }
    }

    pub fn estado(&self) -> &Estado {
        &self.estado
    }

    /// Anexa a instrução de pagamento. De `Criado` apenas.
    ///
    /// Pelo menos uma das duas formas é obrigatória: um pedido "aguardando" sem
    /// nada para pagar deixaria a interface esperando um evento que ninguém sabe
    /// como provocar.
    pub fn aguardar(
        mut self,
        pix: Option<Pix>,
        checkout: Option<CheckoutExterno>,
        expira_em: u64,
    ) -> Result<Self, TransicaoInvalida> {
        if !matches!(self.estado, Estado::Criado) {
            return Err(TransicaoInvalida {
                de: self.estado.rotulo(),
                para: "aguardando pagamento",
            });
        }
        if pix.is_none() && checkout.is_none() {
            // Não é transição inválida: é pedido malformado. Devolver erro de
            // transição faria o chamador procurar no lugar errado.
            return Err(TransicaoInvalida {
                de: "criado",
                para: "aguardando pagamento sem instrução de pagamento",
            });
        }
        self.estado = Estado::Aguardando { pix, checkout, expira_em };
        Ok(self)
    }

    /// Confirma o pagamento. **Só o servidor chama isto**, e a prova é o
    /// argumento: uma licença assinada, que o cliente não consegue produzir.
    pub fn confirmar(mut self, licenca: LicencaAssinada) -> Result<Self, TransicaoInvalida> {
        if !matches!(self.estado, Estado::Aguardando { .. }) {
            return Err(TransicaoInvalida {
                de: self.estado.rotulo(),
                para: "pago",
            });
        }
        // A licença tem de ser do app deste pedido. Sem esta checagem, um
        // servidor com bug (ou um cliente com uma licença de app grátis)
        // entregaria o app caro.
        if licenca.licenca.app != self.app {
            return Err(TransicaoInvalida {
                de: "aguardando pagamento",
                para: "pago com licença de outro app",
            });
        }
        self.estado = Estado::Pago { licenca };
        Ok(self)
    }

    /// Expira. Só de `Criado`/`Aguardando`, e só depois do prazo.
    ///
    /// A checagem de tempo existe para o caso de o relógio do PC estar errado:
    /// expirar um pedido que ainda vale obrigaria o usuário a comprar de novo.
    pub fn expirar(mut self, agora: u64) -> Result<Self, TransicaoInvalida> {
        let expira = match &self.estado {
            Estado::Aguardando { expira_em, .. } => Some(*expira_em),
            Estado::Criado => None,
            _ => {
                return Err(TransicaoInvalida {
                    de: self.estado.rotulo(),
                    para: "expirado",
                })
            }
        };
        if let Some(e) = expira {
            if agora < e {
                return Err(TransicaoInvalida {
                    de: "aguardando pagamento (ainda no prazo)",
                    para: "expirado",
                });
            }
        }
        self.estado = Estado::Expirado;
        Ok(self)
    }

    pub fn recusar(mut self, motivo: impl Into<String>) -> Result<Self, TransicaoInvalida> {
        if self.estado.finalizado() {
            return Err(TransicaoInvalida {
                de: self.estado.rotulo(),
                para: "recusado",
            });
        }
        self.estado = Estado::Recusado { motivo: motivo.into() };
        Ok(self)
    }

    /// A licença, se o pedido foi pago.
    pub fn licenca(&self) -> Option<&LicencaAssinada> {
        match &self.estado {
            Estado::Pago { licenca } => Some(licenca),
            _ => None,
        }
    }

    /// Confere a licença do pedido no aparelho de destino.
    ///
    /// Delega para `LicencaAssinada::verificar`, e confere o app: um pedido pago
    /// de outro app não instala este.
    pub fn validar_no_aparelho(
        &self,
        chave_de_licenca: &crate::ChavePublica,
        aparelho: &crate::ChavePublica,
        agora: u64,
    ) -> Veredito {
        let Some(licenca) = self.licenca() else {
            return Veredito::NaoSei {
                motivo: format!("pedido {} não foi pago ({})", self.id, self.estado.rotulo()),
            };
        };
        let v = licenca.verificar(chave_de_licenca, aparelho, agora);
        if !v.libera() {
            return v;
        }
        crate::licenca::confere_app(&licenca.licenca, &self.app)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::chave::ChavePublica;
    use crate::licenca::{Licenca, VERSAO};
    use crate::preco::Moeda;
    use ed25519_dalek::SigningKey;

    fn chave_lic() -> SigningKey {
        SigningKey::from_bytes(&[11u8; 32])
    }
    fn publica_de(k: &SigningKey) -> ChavePublica {
        ChavePublica::de_bytes(&k.verifying_key().to_bytes()).unwrap()
    }
    fn aparelho() -> ChavePublica {
        ChavePublica::de_bytes(&SigningKey::from_bytes(&[13u8; 32]).verifying_key().to_bytes()).unwrap()
    }

    fn licenca_para(app: &str) -> LicencaAssinada {
        LicencaAssinada::emitir(
            Licenca {
                versao: VERSAO,
                app: app.into(),
                versao_app: "1.0.0".into(),
                aparelho: aparelho().base64(),
                pedido: "PED-1".into(),
                emitida_em: 1_700_000_000,
                expira_em: None,
            },
            &chave_lic(),
        )
    }

    fn pix() -> Pix {
        Pix {
            copia_e_cola: "00020126...".into(),
            valor: Preco::new(1990, Moeda::BRL),
            expira_em: 1_700_000_600,
            recebedor: "Giochi".into(),
        }
    }

    fn aguardando() -> Pedido {
        Pedido::novo("PED-1", "pacman", Preco::brl(1990), aparelho().base64(), 1_700_000_000)
            .aguardar(Some(pix()), None, 1_700_000_600)
            .unwrap()
    }

    #[test]
    fn fluxo_feliz_chega_a_pago_com_licenca() {
        let p = aguardando().confirmar(licenca_para("pacman")).unwrap();
        assert!(matches!(p.estado(), Estado::Pago { .. }));
        assert!(p.licenca().is_some());
        assert_eq!(p.estado().rotulo(), "pago");
    }

    #[test]
    fn nao_da_para_confirmar_pedido_apenas_criado() {
        // O caminho do bug clássico: pular a etapa de pagamento.
        let p = Pedido::novo("P", "pacman", Preco::brl(100), aparelho().base64(), 0);
        assert!(p.confirmar(licenca_para("pacman")).is_err());
    }

    #[test]
    fn pedido_expirado_nao_aceita_confirmacao_depois() {
        let p = aguardando().expirar(1_700_000_700).unwrap();
        assert_eq!(p.estado().rotulo(), "expirado");
        assert!(p.confirmar(licenca_para("pacman")).is_err(), "expirado voltou a aceitar pagamento");
    }

    #[test]
    fn nao_expira_antes_do_prazo() {
        let r = aguardando().expirar(1_700_000_100);
        assert!(r.is_err(), "expirou um pedido que ainda valia");
    }

    #[test]
    fn licenca_de_outro_app_nao_confirma_o_pedido() {
        // Servidor com bug, ou cliente reaproveitando licença de app grátis.
        let r = aguardando().confirmar(licenca_para("volley"));
        assert!(r.is_err(), "pedido de pacman aceitou licença de volley");
    }

    #[test]
    fn pedido_sem_instrucao_de_pagamento_e_recusado() {
        let r = Pedido::novo("P", "pacman", Preco::brl(100), "x", 0).aguardar(None, None, 99);
        assert!(r.is_err(), "pedido aguardando sem nada para pagar");
    }

    #[test]
    fn aceita_cartao_por_checkout_externo() {
        let checkout = CheckoutExterno { url: "https://pay.exemplo/x".into(), expira_em: 1_700_000_600 };
        let p = Pedido::novo("P", "pacman", Preco::brl(100), aparelho().base64(), 0)
            .aguardar(None, Some(checkout), 1_700_000_600)
            .unwrap();
        assert!(matches!(p.estado(), Estado::Aguardando { checkout: Some(_), .. }));
    }

    #[test]
    fn confirmar_duas_vezes_nao_e_permitido() {
        let p = aguardando().confirmar(licenca_para("pacman")).unwrap();
        assert!(p.confirmar(licenca_para("pacman")).is_err());
    }

    #[test]
    fn recusar_depois_de_pago_nao_e_permitido() {
        let p = aguardando().confirmar(licenca_para("pacman")).unwrap();
        assert!(p.recusar("estorno").is_err());
    }

    #[test]
    fn validar_no_aparelho_do_pedido_pago_da_valido() {
        let p = aguardando().confirmar(licenca_para("pacman")).unwrap();
        let v = p.validar_no_aparelho(&publica_de(&chave_lic()), &aparelho(), 1_700_000_100);
        assert_eq!(v, Veredito::Valida);
    }

    #[test]
    fn validar_no_aparelho_de_pedido_nao_pago_devolve_nao_sei() {
        // `NaoSei` e não `Valida`: pedido não pago não libera nada.
        let p = aguardando();
        let v = p.validar_no_aparelho(&publica_de(&chave_lic()), &aparelho(), 1_700_000_100);
        assert!(matches!(v, Veredito::NaoSei { .. }), "{v:?}");
        assert!(!v.libera());
    }

    #[test]
    fn preco_fica_em_centavos_no_pedido() {
        let p = aguardando();
        assert_eq!(p.preco, Preco::brl(1990));
        assert_eq!(p.preco.to_string(), "R$ 19,90");
    }
}
