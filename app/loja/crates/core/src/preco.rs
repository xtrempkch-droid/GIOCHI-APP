//! Dinheiro em inteiro, nunca em ponto flutuante.
//!
//! # Por que não `f64`
//!
//! `0.1 + 0.2 != 0.3` em ponto flutuante. Num carrinho de compras isso é um
//! centavo que aparece ou desaparece, e o tipo de bug que só é notado quando o
//! cliente reclama do valor cobrado. Centavos em `u32` não têm esse problema e
//! cabem em R$ 42 milhões.
//!
//! A moeda acompanha o preço porque o mesmo catálogo pode ter item em BRL e em
//! USD — e somar moedas diferentes sem erro de compilação é como o bug entra.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Moeda {
    BRL,
    USD,
    EUR,
}

impl Moeda {
    pub fn simbolo(self) -> &'static str {
        match self {
            Self::BRL => "R$",
            Self::USD => "US$",
            Self::EUR => "€",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Preco {
    pub centavos: u32,
    pub moeda: Moeda,
}

impl Preco {
    pub const fn new(centavos: u32, moeda: Moeda) -> Self {
        Self { centavos, moeda }
    }

    pub const fn brl(centavos: u32) -> Self {
        Self::new(centavos, Moeda::BRL)
    }

    pub const fn gratis() -> Self {
        Self::brl(0)
    }

    pub fn eh_gratis(self) -> bool {
        self.centavos == 0
    }

    /// Soma dois preços, recusando moedas diferentes.
    ///
    /// `Option` e não pânico: moeda diferente é entrada inválida do servidor, e
    /// derrubar a loja por causa disso seria pior que recusar o item.
    pub fn somar(self, outro: Self) -> Option<Self> {
        if self.moeda != outro.moeda {
            return None;
        }
        self.centavos
            .checked_add(outro.centavos)
            .map(|c| Self::new(c, self.moeda))
    }
}

impl fmt::Display for Preco {
    /// `R$ 12,34` — vírgula decimal e duas casas, que é como se lê em português.
    ///
    /// Formatar à mão em vez de usar uma crate de locale: são três moedas e uma
    /// regra, e uma dependência a mais num binário que o usuário baixa não se
    /// paga por isso.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.centavos == 0 {
            return write!(f, "Grátis");
        }
        write!(
            f,
            "{} {},{:02}",
            self.moeda.simbolo(),
            self.centavos / 100,
            self.centavos % 100
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formata_com_dois_digitos() {
        assert_eq!(Preco::brl(1234).to_string(), "R$ 12,34");
        // 5 centavos não pode virar "R$ 0,5".
        assert_eq!(Preco::brl(5).to_string(), "R$ 0,05");
        assert_eq!(Preco::brl(100).to_string(), "R$ 1,00");
    }

    #[test]
    fn zero_e_gratis_e_nao_zero_reais() {
        assert_eq!(Preco::gratis().to_string(), "Grátis");
        assert!(Preco::gratis().eh_gratis());
        assert!(!Preco::brl(1).eh_gratis());
    }

    #[test]
    fn somar_moeda_diferente_e_recusado() {
        let brl = Preco::brl(100);
        let usd = Preco::new(100, Moeda::USD);
        assert_eq!(brl.somar(usd), None);
    }

    #[test]
    fn somar_nao_estoura() {
        let maximo = Preco::brl(u32::MAX);
        assert_eq!(maximo.somar(Preco::brl(1)), None);
    }

    #[test]
    fn centavos_nao_perdem_precisao() {
        // O caso que motiva o inteiro: 0,10 + 0,20.
        let a = Preco::brl(10);
        let b = Preco::brl(20);
        assert_eq!(a.somar(b).unwrap(), Preco::brl(30));
        assert_eq!(Preco::brl(30).to_string(), "R$ 0,30");
    }
}
