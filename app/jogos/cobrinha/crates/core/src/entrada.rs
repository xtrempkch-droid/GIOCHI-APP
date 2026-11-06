//! Tradução de botão em intenção.
//!
//! O código evdev cru e o código de tecla chegam de fontes diferentes (a thread
//! de `/dev/input` e o SDL), mas o jogo só conhece `Acao`. Isso mantém o núcleo
//! sem dependência de plataforma e faz o mapeamento ser uma tabela testável.

use crate::jogo::Direcao;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    Dir(Direcao),
    /// Começar, ou recomeçar depois de perder.
    Confirmar,
    Sair,
}

/// Códigos evdev dos botões do aparelho.
///
/// O R35S declara 19 códigos (`gpio-keys-vol` + `r36s_Gamepad`); o X55 declara
/// 20. Os do direcional estão na faixa `BTN_DPAD_*` (544-547), não em `KEY_*`,
/// e é por isso que parecem códigos sem nome quando se lê o `/proc` rápido.
pub mod evdev {
    pub const DPAD_UP: u16 = 544;
    pub const DPAD_DOWN: u16 = 545;
    pub const DPAD_LEFT: u16 = 546;
    pub const DPAD_RIGHT: u16 = 547;

    pub const SOUTH: u16 = 304;
    pub const EAST: u16 = 305;
    pub const START: u16 = 315;
    pub const SELECT: u16 = 314;
}

/// Converte código evdev em ação. `None` = botão que o jogo ignora.
pub fn de_evdev(codigo: u16) -> Option<Acao> {
    use evdev::*;
    match codigo {
        DPAD_UP => Some(Acao::Dir(Direcao::Cima)),
        DPAD_DOWN => Some(Acao::Dir(Direcao::Baixo)),
        DPAD_LEFT => Some(Acao::Dir(Direcao::Esquerda)),
        DPAD_RIGHT => Some(Acao::Dir(Direcao::Direita)),
        SOUTH | START => Some(Acao::Confirmar),
        EAST | SELECT => Some(Acao::Sair),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_direcional_vira_direcao() {
        assert_eq!(de_evdev(544), Some(Acao::Dir(Direcao::Cima)));
        assert_eq!(de_evdev(547), Some(Acao::Dir(Direcao::Direita)));
    }

    #[test]
    fn a_e_start_confirmam_b_e_select_saem() {
        assert_eq!(de_evdev(304), Some(Acao::Confirmar));
        assert_eq!(de_evdev(315), Some(Acao::Confirmar));
        assert_eq!(de_evdev(305), Some(Acao::Sair));
        assert_eq!(de_evdev(314), Some(Acao::Sair));
    }

    #[test]
    fn codigo_desconhecido_nao_vira_acao() {
        // O kernel declara 116 (KEY_POWER) no X55 e não no R35S. Um botão que o
        // jogo não conhece não pode virar `Confirmar` por acidente.
        assert_eq!(de_evdev(116), None);
        assert_eq!(de_evdev(0), None);
    }

    #[test]
    fn volume_nao_controla_o_jogo() {
        assert_eq!(de_evdev(114), None);
        assert_eq!(de_evdev(115), None);
    }
}
