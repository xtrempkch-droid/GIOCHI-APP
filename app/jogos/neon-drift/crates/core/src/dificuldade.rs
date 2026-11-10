//! Os quatro modos de dificuldade da corrida.
//!
//! ## O que o modo muda
//!
//! Duas alavancas, e as duas são **fatores** aplicados sobre a tabela que já
//! existe — nunca valores absolutos. A velocidade da IA nasce de
//! [`crate::ia::VEL_BASE`] e a do jogador de [`crate::carro::VEL_MAX`]; um modo
//! que trouxesse números próprios envelheceria sozinho no dia em que alguém
//! ajustasse a base (a lição das "duas listas" já apareceu duas vezes aqui).
//!
//! | Modo | Adversários | Carro do jogador |
//! | --- | --- | --- |
//! | fácil | 88 % | 100 % |
//! | normal | 100 % | 100 % |
//! | difícil | 108 % | 100 % |
//! | **desafiante** | 110 % | **95 %** |
//!
//! No `normal` os dois fatores são 1,0 de propósito: é a corrida que foi medida
//! e balanceada nas rodadas anteriores, e um modo novo não pode reescrevê-la em
//! silêncio.
//!
//! ## Por que o "desafiante" mexe no jogador
//!
//! Pedido do dono: "no modo desafiante o carro do usuário será 5 % mais lento que
//! os outros carros". Isso é diferente de "IA 5 % mais rápida" porque o
//! **velocímetro do jogador muda**: o topo cai de 62 para 58,9 m/s (e o do nitro
//! de 82 para 77,9), e o número na tela é o mesmo que o carro entrega. Um fator
//! só na IA deixaria o velocímetro mentindo sobre a desvantagem.

use std::fmt;

/// Quão difícil é a corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dificuldade {
    /// Adversários mais lentos: serve para conhecer a pista.
    Facil,
    /// A corrida balanceada — padrão.
    #[default]
    Normal,
    /// Adversários 8 % mais rápidos.
    Dificil,
    /// Adversários 10 % mais rápidos **e** o carro do jogador 5 % mais lento.
    Desafiante,
}

impl Dificuldade {
    /// As quatro, na ordem em que o menu as percorre.
    pub fn todos() -> [Dificuldade; 4] {
        [
            Dificuldade::Facil,
            Dificuldade::Normal,
            Dificuldade::Dificil,
            Dificuldade::Desafiante,
        ]
    }

    /// Identificador de linha de comando.
    pub fn id(self) -> &'static str {
        match self {
            Dificuldade::Facil => "facil",
            Dificuldade::Normal => "normal",
            Dificuldade::Dificil => "dificil",
            Dificuldade::Desafiante => "desafiante",
        }
    }

    /// Nome desenhado na tela.
    pub fn nome(self) -> &'static str {
        match self {
            Dificuldade::Facil => "FACIL",
            Dificuldade::Normal => "NORMAL",
            Dificuldade::Dificil => "DIFICIL",
            Dificuldade::Desafiante => "DESAFIANTE",
        }
    }

    /// Uma linha dizendo o que muda, para o menu não ser só um rótulo.
    pub fn descricao(self) -> &'static str {
        match self {
            Dificuldade::Facil => "adversarios 12% mais lentos",
            Dificuldade::Normal => "a corrida balanceada",
            Dificuldade::Dificil => "adversarios 8% mais rapidos",
            Dificuldade::Desafiante => "adversarios 10% mais rapidos e voce 5% mais lento",
        }
    }

    /// Multiplicador da velocidade dos adversários.
    pub fn fator_ia(self) -> f32 {
        match self {
            Dificuldade::Facil => 0.88,
            Dificuldade::Normal => 1.00,
            Dificuldade::Dificil => 1.08,
            Dificuldade::Desafiante => 1.10,
        }
    }

    /// Multiplicador do teto de velocidade do carro do jogador.
    ///
    /// Só o `desafiante` não é 1,0 — e é o que o pedido descreve: 5 % mais lento
    /// que os outros.
    pub fn fator_jogador(self) -> f32 {
        match self {
            Dificuldade::Facil | Dificuldade::Normal | Dificuldade::Dificil => 1.0,
            Dificuldade::Desafiante => 0.95,
        }
    }

    /// Próxima, dando a volta.
    pub fn proxima(self) -> Self {
        let todos = Self::todos();
        let atual = todos.iter().position(|d| *d == self).unwrap_or(0);
        todos[(atual + 1) % todos.len()]
    }

    /// Anterior, dando a volta.
    pub fn anterior(self) -> Self {
        let todos = Self::todos();
        let atual = todos.iter().position(|d| *d == self).unwrap_or(0);
        todos[(atual + todos.len() - 1) % todos.len()]
    }

    /// Lê o identificador, aceitando acento e maiúscula.
    pub fn parse(texto: &str) -> Option<Self> {
        match texto.trim().to_lowercase().as_str() {
            "facil" | "fácil" | "easy" => Some(Dificuldade::Facil),
            "normal" | "medio" | "médio" | "medium" => Some(Dificuldade::Normal),
            "dificil" | "difícil" | "hard" => Some(Dificuldade::Dificil),
            "desafiante" | "challenger" | "insano" => Some(Dificuldade::Desafiante),
            _ => None,
        }
    }
}

impl fmt::Display for Dificuldade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_normal_e_a_corrida_que_ja_existia() {
        // A regressão que este teste trava: nenhum modo novo pode mudar a
        // corrida medida nas rodadas anteriores em silêncio.
        assert_eq!(Dificuldade::default(), Dificuldade::Normal);
        assert_eq!(Dificuldade::Normal.fator_ia(), 1.0);
        assert_eq!(Dificuldade::Normal.fator_jogador(), 1.0);
    }

    #[test]
    fn a_pressao_cresce_do_facil_ao_desafiante() {
        let pressao = |d: Dificuldade| d.fator_ia() / d.fator_jogador();
        let valores: Vec<f32> = Dificuldade::todos().iter().map(|d| pressao(*d)).collect();
        for i in 1..valores.len() {
            assert!(
                valores[i] > valores[i - 1],
                "{} não é mais difícil que o anterior: {:?}",
                Dificuldade::todos()[i].nome(),
                valores
            );
        }
    }

    #[test]
    fn o_desafiante_deixa_o_jogador_cinco_por_cento_mais_lento() {
        // O pedido é literal: 5 % mais lento **que os outros carros**. Como a
        // IA fica em 100 % e o jogador em 95 %, a diferença entre eles é de
        // 5 pontos percentuais.
        let d = Dificuldade::Desafiante;
        assert_eq!(d.fator_jogador(), 0.95);
        assert!((1.0 - d.fator_jogador() - 0.05).abs() < 1e-6);
        // E só ele: os outros três mantêm o carro do jogador como está.
        for outra in [Dificuldade::Facil, Dificuldade::Normal, Dificuldade::Dificil] {
            assert_eq!(outra.fator_jogador(), 1.0, "{}", outra.nome());
        }
    }

    #[test]
    fn o_teto_do_desafiante_e_medivel_no_velocimetro() {
        // O que o jogador vê: o topo do carro cai 5 %, e o do nitro também —
        // senão o nitro daria uma velocidade que o resto do carro não sustenta.
        let teto = crate::carro::VEL_MAX * Dificuldade::Desafiante.fator_jogador();
        assert!((teto - 58.9).abs() < 0.05, "teto {teto}");
        let nitro = crate::carro::VEL_NITRO_MAX * Dificuldade::Desafiante.fator_jogador();
        assert!((nitro - 77.9).abs() < 0.05, "nitro {nitro}");
    }

    #[test]
    fn o_menu_percorre_as_quatro_em_laco() {
        let mut d = Dificuldade::Facil;
        let mut vistos = Vec::new();
        for _ in 0..4 {
            vistos.push(d);
            d = d.proxima();
        }
        assert_eq!(vistos, Dificuldade::todos().to_vec());
        assert_eq!(d, Dificuldade::Facil);
        assert_eq!(Dificuldade::Facil.anterior(), Dificuldade::Desafiante);
    }

    #[test]
    fn cada_modo_tem_nome_e_descricao_proprios() {
        let descricoes: Vec<&str> = Dificuldade::todos().iter().map(|d| d.descricao()).collect();
        for i in 0..descricoes.len() {
            for j in (i + 1)..descricoes.len() {
                assert_ne!(descricoes[i], descricoes[j]);
            }
        }
    }

    #[test]
    fn o_identificador_e_lido_com_acento_e_maiuscula() {
        for (texto, esperado) in [
            ("facil", Dificuldade::Facil),
            ("FÁCIL", Dificuldade::Facil),
            ("Normal", Dificuldade::Normal),
            ("difícil", Dificuldade::Dificil),
            ("  DESAFIANTE  ", Dificuldade::Desafiante),
        ] {
            assert_eq!(Dificuldade::parse(texto), Some(esperado), "{texto:?}");
        }
        assert_eq!(Dificuldade::parse("impossivel"), None);
        for d in Dificuldade::todos() {
            assert_eq!(Dificuldade::parse(d.id()), Some(d));
        }
    }
}
