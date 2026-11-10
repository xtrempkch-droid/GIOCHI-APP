//! Modos de dificuldade: **três números**, e o que cada um muda.
//!
//! ## A distância da câmera é o modo
//!
//! O pedido do dono foi explícito: o modo fácil afasta a câmera para o **dobro**
//! da distância, o médio para 75 % a mais, e o difícil mantém a distância atual e
//! sobe um pouco a velocidade dos cubos. Ou seja: o que o jogador sente primeiro
//! é **quanto do campo ele vê**, e a pressão vem depois.
//!
//! Isso tem uma consequência que não é óbvia e está tratada em
//! [`crate::gameplay::campo`]: a casca de spawn tem de acompanhar a distância,
//! senão no modo fácil os cubos nascem **atrás da câmera**. E se o campo cresce,
//! a velocidade tem de crescer junto — senão o cubo demora o dobro para chegar e
//! o modo fácil vira "modo lento", que não é o que foi pedido.
//!
//! ## Por que multiplicadores e não valores absolutos
//!
//! A onda 1 foi medida no bloco 1b e a progressão no 3b. Se cada modo trouxesse
//! uma tabela própria de intervalo e velocidade, as três envelheceriam diferente
//! — e a lição das "duas listas" já apareceu duas vezes neste repositório. Os
//! modos são **fatores** aplicados sobre a mesma tabela de ondas.

use std::fmt;

/// A distância da câmera no modo atual, que é a referência dos três modos.
///
/// Igual a `crate::gameplay::camera::OrbitalCamera::new().distance`; há teste
/// comparando os dois, porque duas cópias do mesmo número separadas por um
/// `use` divergem na primeira vez que alguém mexer numa delas.
pub const DISTANCIA_DE_REFERENCIA: f32 = 9.0;

/// Quanto o modo fácil afasta a câmera, como múltiplo da referência.
pub const FATOR_FACIL: f32 = 2.0;
/// Quanto o modo médio afasta a câmera — 75 % a mais, como pedido.
pub const FATOR_MEDIO: f32 = 1.75;

/// Como o jogo está configurado antes de a partida começar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    /// Câmera no dobro da distância, spawn mais espaçado e cubos mais lentos.
    Facil,
    /// Câmera 75 % mais longe. É o padrão.
    #[default]
    Medio,
    /// A distância de sempre e a velocidade um pouco maior.
    Dificil,
}

impl Modo {
    /// Os três, na ordem em que o menu os percorre.
    pub fn todos() -> [Modo; 3] {
        [Modo::Facil, Modo::Medio, Modo::Dificil]
    }

    /// Identificador de linha de comando e de arquivo.
    pub fn id(self) -> &'static str {
        match self {
            Modo::Facil => "facil",
            Modo::Medio => "medio",
            Modo::Dificil => "dificil",
        }
    }

    /// Nome desenhado no menu.
    pub fn nome(self) -> &'static str {
        match self {
            Modo::Facil => "FACIL",
            Modo::Medio => "MEDIO",
            Modo::Dificil => "DIFICIL",
        }
    }

    /// Uma linha dizendo o que o modo muda, para o menu não ser só um rótulo.
    pub fn descricao(self) -> &'static str {
        match self {
            Modo::Facil => "camera 2x mais longe, cubos mais lentos",
            Modo::Medio => "camera 75% mais longe",
            Modo::Dificil => "camera atual, cubos 15% mais rapidos",
        }
    }

    /// Distância da câmera ao objeto defendido, em unidades de mundo.
    pub fn distancia_da_camera(self) -> f32 {
        match self {
            Modo::Facil => DISTANCIA_DE_REFERENCIA * FATOR_FACIL,
            Modo::Medio => DISTANCIA_DE_REFERENCIA * FATOR_MEDIO,
            Modo::Dificil => DISTANCIA_DE_REFERENCIA,
        }
    }

    /// Fator aplicado à velocidade dos cubos.
    ///
    /// O fácil freia e o difícil acelera; o médio é a referência medida no 3b.
    pub fn fator_velocidade(self) -> f32 {
        match self {
            Modo::Facil => 0.90,
            Modo::Medio => 1.00,
            Modo::Dificil => 1.15,
        }
    }

    /// Fator aplicado ao intervalo entre spawns.
    ///
    /// Maior que 1 espaça (mais fácil); menor que 1 adensa. É o par inverso do
    /// fator de velocidade: um sozinho mudaria a ocupação do pool em vez da
    /// dificuldade.
    pub fn fator_intervalo(self) -> f32 {
        match self {
            Modo::Facil => 1.25,
            Modo::Medio => 1.00,
            // 1,00 no difícil de propósito: o pedido foi "velocidade levemente
            // aumentada". Mexer **também** no intervalo somou duas pressões e o
            // piloto perdeu 5 de 5 sem ganhar nenhuma onda — medido, e é a mesma
            // "dificuldade injusta" que o bloco 3b rejeitou.
            Modo::Dificil => 1.00,
        }
    }

    /// Próximo modo, dando a volta.
    pub fn proximo(self) -> Self {
        match self {
            Modo::Facil => Modo::Medio,
            Modo::Medio => Modo::Dificil,
            Modo::Dificil => Modo::Facil,
        }
    }

    /// Modo anterior, dando a volta.
    pub fn anterior(self) -> Self {
        self.proximo().proximo()
    }

    /// Lê o identificador, aceitando acento e maiúscula.
    pub fn parse(texto: &str) -> Option<Self> {
        match texto.trim().to_lowercase().as_str() {
            "facil" | "fácil" | "easy" => Some(Modo::Facil),
            "medio" | "médio" | "medium" | "normal" => Some(Modo::Medio),
            "dificil" | "difícil" | "hard" => Some(Modo::Dificil),
            _ => None,
        }
    }
}

impl fmt::Display for Modo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_distancias_seguem_o_pedido() {
        // Fácil: o dobro. Médio: 75 % a mais. Difícil: a de sempre.
        assert_eq!(Modo::Facil.distancia_da_camera(), 18.0);
        assert_eq!(Modo::Medio.distancia_da_camera(), 15.75);
        assert_eq!(Modo::Dificil.distancia_da_camera(), 9.0);

        // E a relação entre eles é a que foi pedida, não um número solto.
        assert_eq!(
            Modo::Facil.distancia_da_camera(),
            DISTANCIA_DE_REFERENCIA * 2.0
        );
        assert_eq!(
            Modo::Medio.distancia_da_camera(),
            DISTANCIA_DE_REFERENCIA * 1.75
        );
    }

    #[test]
    fn a_referencia_bate_com_a_camera_de_verdade() {
        // Duas cópias do mesmo 9.0: se alguém mudar a câmera e não o modo, o
        // difícil deixa de ser "a distância atual" sem nenhum erro aparecer.
        let camera = crate::gameplay::camera::OrbitalCamera::new();
        assert_eq!(camera.distance, DISTANCIA_DE_REFERENCIA);
        assert_eq!(Modo::Dificil.distancia_da_camera(), camera.distance);
    }

    #[test]
    fn a_pressao_cresce_do_facil_para_o_dificil() {
        let facil = Modo::Facil;
        let medio = Modo::Medio;
        let dificil = Modo::Dificil;

        // Velocidade sobe e o intervalo encolhe do fácil para o médio.
        assert!(facil.fator_velocidade() < medio.fator_velocidade());
        assert!(medio.fator_velocidade() <= dificil.fator_velocidade());
        assert!(facil.fator_intervalo() > medio.fator_intervalo());
        assert!(medio.fator_intervalo() >= dificil.fator_intervalo());

        // O médio é exatamente a referência medida no 3b: mexer nele mudaria a
        // dificuldade que o piloto automático aprovou no modo padrão.
        assert_eq!(medio.fator_velocidade(), 1.0);
        assert_eq!(medio.fator_intervalo(), 1.0);

        // E a pressão total é estritamente crescente: velocidade ÷ intervalo.
        let pressao = |m: Modo| m.fator_velocidade() / m.fator_intervalo();
        assert!(pressao(facil) < pressao(medio));
        assert!(pressao(medio) < pressao(dificil));
    }

    #[test]
    fn o_dificil_acelera_levemente_e_nao_o_dobro() {
        // "levemente aumentada" foi o pedido. 15 % é o suficiente para sentir e
        // não tanto para o cubo chegar antes de o jogador girar a câmera.
        let fator = Modo::Dificil.fator_velocidade();
        assert!((fator - 1.0).abs() > 0.05, "tem de dar para sentir");
        assert!(fator < 1.35, "acima disto deixa de ser leve: {fator}");
    }

    #[test]
    fn o_menu_percorre_os_tres_em_laco() {
        let mut modo = Modo::Facil;
        let mut vistos = Vec::new();
        for _ in 0..3 {
            vistos.push(modo);
            modo = modo.proximo();
        }
        assert_eq!(vistos, Modo::todos().to_vec());
        assert_eq!(modo, Modo::Facil, "três passos voltam ao começo");

        assert_eq!(Modo::Facil.anterior(), Modo::Dificil);
        assert_eq!(Modo::Medio.anterior(), Modo::Facil);
    }

    #[test]
    fn o_identificador_e_lido_com_acento_e_maiuscula() {
        for (texto, esperado) in [
            ("facil", Modo::Facil),
            ("FÁCIL", Modo::Facil),
            ("Medio", Modo::Medio),
            ("médio", Modo::Medio),
            ("dificil", Modo::Dificil),
            ("  DIFÍCIL  ", Modo::Dificil),
        ] {
            assert_eq!(Modo::parse(texto), Some(esperado), "texto {texto:?}");
        }
        assert_eq!(Modo::parse("impossivel"), None);
        assert_eq!(Modo::parse(""), None);
    }

    #[test]
    fn cada_modo_diz_o_que_muda() {
        // O menu desenha a descrição; três textos iguais não informariam nada.
        let textos: Vec<&str> = Modo::todos().iter().map(|m| m.descricao()).collect();
        assert_eq!(textos.len(), 3);
        assert_ne!(textos[0], textos[1]);
        assert_ne!(textos[1], textos[2]);
        assert_ne!(textos[0], textos[2]);
    }
}
