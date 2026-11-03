//! Leitura do botão de energia físico.
//!
//! O botão do PMIC (`rk805 pwrkey`, `KEY_POWER`) não é botão de controle: ele
//! existe para ligar/desligar o aparelho, independente de estar num jogo ou na
//! interface. Por isso não passa pelo `buttons.map` — um mapa por aparelho não
//! deveria precisar declarar que o botão de energia existe.
//!
//! ## Duas ações, uma borda
//!
//! | Gesto | Ação |
//! | --- | --- |
//! | Toque curto (solta antes do limiar) | dormir |
//! | Segurar até o limiar | menu de energia |
//!
//! O menu dispara **ao atingir o limiar**, não ao soltar: esperar a soltura
//! faria o usuário segurar 3 s sem nenhum sinal de que algo aconteceu — que é
//! exatamente a queixa que originou este módulo ("apertei e não aconteceu nada").
//!
//! A medição vive aqui, e não no laço, porque é lógica pura: dá para testar os
//! limiares sem hardware e sem relógio real.

/// O que o gesto pediu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    /// Toque curto: dormir.
    Curto,
    /// Segurou o suficiente: menu de energia.
    Longo,
}

/// Acompanha o botão entre quadros e decide quando emitir a ação.
pub struct PowerWatch {
    limiar: f32,
    segurado: f32,
    /// Já emitiu `Longo` neste pressionamento; impede repetir a cada quadro.
    disparou: bool,
    pressionado: bool,
}

impl PowerWatch {
    /// Tempo até o gesto virar "menu de energia".
    ///
    /// Um segundo é o ponto em que a intenção deixa de ser "quero dormir" e
    /// passa a ser "quero o menu". Mais que isso e o usuário segura sem
    /// retorno; menos e um toque um pouco lento dorme sem querer.
    pub const LIMIAR_PADRAO: f32 = 1.0;

    pub fn novo() -> Self {
        Self::com_limiar(Self::LIMIAR_PADRAO)
    }

    pub fn com_limiar(limiar: f32) -> Self {
        Self {
            limiar: limiar.max(0.05),
            segurado: 0.0,
            disparou: false,
            pressionado: false,
        }
    }

    /// Quanto tempo o botão está pressionado neste ciclo.
    pub fn segurado(&self) -> f32 {
        self.segurado
    }

    /// Alimenta o estado do botão e devolve a ação a executar, se houver.
    ///
    /// `pressionado` é o estado atual do `KEY_POWER`; `delta` é o tempo desde o
    /// quadro anterior, em segundos.
    pub fn update(&mut self, pressionado: bool, delta: f32) -> Option<PowerAction> {
        let antes = self.pressionado;
        self.pressionado = pressionado;

        let apertou = pressionado && !antes;
        let soltou = !pressionado && antes;

        if apertou {
            self.segurado = 0.0;
            self.disparou = false;
        }

        if pressionado {
            if !self.disparou {
                self.segurado += delta.max(0.0);
                if self.segurado >= self.limiar {
                    self.disparou = true;
                    return Some(PowerAction::Longo);
                }
            }
            return None;
        }

        if soltou {
            let acao = if self.disparou {
                None
            } else {
                Some(PowerAction::Curto)
            };
            self.segurado = 0.0;
            self.disparou = false;
            return acao;
        }

        None
    }

    /// Marca o pressionamento atual como já resolvido.
    ///
    /// Usado quando o toque serviu para **acordar**: sem isto, soltar o botão
    /// logo depois de acordar emitiria `Curto` e o aparelho dormiria de novo —
    /// um vaivém que parece "o botão não funciona".
    pub fn consumir(&mut self) {
        self.disparou = true;
        self.segurado = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toque_curto_dorme() {
        let mut w = PowerWatch::novo();
        assert_eq!(w.update(true, 0.016), None);
        assert_eq!(w.update(true, 0.1), None);
        assert_eq!(w.update(false, 0.016), Some(PowerAction::Curto));
    }

    #[test]
    fn segurar_abre_o_menu_ao_atingir_o_limiar() {
        let mut w = PowerWatch::novo();
        // 0,9 s: ainda não.
        for _ in 0..56 {
            assert_eq!(w.update(true, 0.016), None);
        }
        // passa de 1 s: dispara uma vez.
        let mut disparos = 0;
        for _ in 0..10 {
            if w.update(true, 0.016) == Some(PowerAction::Longo) {
                disparos += 1;
            }
        }
        assert_eq!(disparos, 1, "não pode reabrir o menu a cada quadro");

        // Soltar depois de disparar não dorme.
        assert_eq!(w.update(false, 0.016), None);
    }

    #[test]
    fn soltar_antes_do_limiar_nao_abre_menu() {
        let mut w = PowerWatch::com_limiar(1.0);
        w.update(true, 0.016);
        w.update(true, 0.5);
        assert_eq!(w.update(false, 0.016), Some(PowerAction::Curto));
    }

    #[test]
    fn consumir_evita_dormir_de_novo_ao_acordar() {
        // O toque que acorda já foi usado; ao soltar, nada deve acontecer.
        let mut w = PowerWatch::novo();
        w.update(true, 0.016);
        w.consumir();
        assert_eq!(w.update(false, 0.016), None);
    }

    #[test]
    fn soltar_sem_apertar_nao_faz_nada() {
        let mut w = PowerWatch::novo();
        assert_eq!(w.update(false, 0.016), None);
        assert_eq!(w.update(false, 0.016), None);
    }

    #[test]
    fn delta_grande_dispara_na_hora() {
        // Quadro lento (ex.: carga de textura) não pode engolir o gesto.
        let mut w = PowerWatch::novo();
        assert_eq!(w.update(true, 1.5), Some(PowerAction::Longo));
    }

    #[test]
    fn apos_soltar_o_proximo_toque_recomeca() {
        let mut w = PowerWatch::novo();
        w.update(true, 1.5);
        w.update(false, 0.016);
        // Toque curto novo, do zero.
        w.update(true, 0.05);
        assert_eq!(w.update(false, 0.016), Some(PowerAction::Curto));
    }
}
