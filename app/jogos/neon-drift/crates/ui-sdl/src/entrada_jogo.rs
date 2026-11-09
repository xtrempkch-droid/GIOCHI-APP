//! Junta teclado e controle numa `Entrada` só.
//!
//! ## Por que a fusão é uma função pura
//!
//! Ela decide coisas que dão para errar em silêncio: "seta + A somam ou o maior
//! vence?", "frear e acelerar juntos faz o quê?", "espaço é freio de mão ou
//! sair?". Se isso morasse dentro do laço de eventos do SDL, a única forma de
//! conferir seria apertar teclas no aparelho.
//!
//! Aqui entra máscara (`u32`), estado de teclas e fase da corrida; sai a entrada
//! do carro e as ações de tela.

use giochi_neon_drift_core::carro::Entrada;
use giochi_neon_drift_core::Fase;

use crate::evdev;

/// Teclas que o jogo observa, como estado (não evento).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Teclas {
    /// Cima (acelerar).
    pub cima: bool,
    /// Baixo (frear).
    pub baixo: bool,
    /// Esquerda.
    pub esquerda: bool,
    /// Direita.
    pub direita: bool,
    /// Espaço — freio de mão.
    pub espaco: bool,
    /// Shift — nitro.
    pub shift: bool,
    /// Enter — começar/reiniciar.
    pub enter: bool,
    /// Esc — sair.
    pub escape: bool,
    /// W/A/S/D, equivalentes às setas.
    pub wasd: bool,
}

/// Ações de tela que o laço precisa executar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Acoes {
    /// Começar a corrida (tela de título).
    pub comecar: bool,
    /// Pedido de troca de mapa, `-1` ou `+1`; `0` é "nada".
    ///
    /// Só tem efeito na tela de título. Ali o carro não anda, então a seta
    /// esquerda/direita está livre — e é onde o jogador procura o mapa, não na
    /// linha de comando de um aparelho sem teclado.
    pub trocar_mapa: i32,
    /// Reiniciar (tela de resultado).
    pub reiniciar: bool,
    /// Sair do jogo.
    pub sair: bool,
}

/// Combina as duas fontes de entrada com a fase atual.
pub fn combinar(bits: u32, teclas: &Teclas, fase: Fase) -> (Entrada, Acoes) {
    // Teclado e controle **somam** com `max`, não com `or` booleano: se as duas
    // fontes dizem "acelerar", o carro não acelera duas vezes.
    let mut acelerar: f32 = if bits & evdev::ACELERAR != 0 { 1.0 } else { 0.0 };
    let mut frear: f32 = if bits & evdev::FREAR != 0 { 1.0 } else { 0.0 };
    let mut virar: f32 = 0.0;

    if bits & evdev::ESQUERDA != 0 {
        virar -= 1.0;
    }
    if bits & evdev::DIREITA != 0 {
        virar += 1.0;
    }

    if teclas.cima {
        acelerar = acelerar.max(1.0);
    }
    if teclas.baixo {
        frear = frear.max(1.0);
    }
    if teclas.esquerda {
        virar -= 1.0;
    }
    if teclas.direita {
        virar += 1.0;
    }

    let freio_mao = bits & evdev::FREIO_MAO != 0 || teclas.espaco;
    let nitro = bits & evdev::NITRO != 0 || teclas.shift;

    let entrada = Entrada {
        acelerar,
        frear,
        virar: virar.clamp(-1.0, 1.0),
        freio_mao,
        nitro,
    };

    let confirmar = bits & evdev::CONFIRMAR != 0 || teclas.enter;
    let sair = bits & evdev::SAIR != 0 || teclas.escape;

    // No título as setas escolhem o mapa. Em qualquer outra fase elas voltam a
    // ser volante, e `trocar_mapa` fica 0 — uma tecla com dois significados
    // simultâneos seria pior que tecla morta.
    let trocar_mapa = if fase == Fase::Titulo {
        let mut passo = 0;
        if bits & evdev::ESQUERDA != 0 || teclas.esquerda {
            passo -= 1;
        }
        if bits & evdev::DIREITA != 0 || teclas.direita {
            passo += 1;
        }
        passo
    } else {
        0
    };

    let acoes = Acoes {
        comecar: confirmar && fase == Fase::Titulo,
        trocar_mapa,
        reiniciar: confirmar && fase == Fase::Resultado,
        sair,
    };

    (entrada, acoes)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sem_teclas() -> Teclas {
        Teclas::default()
    }

    #[test]
    fn sem_nada_apertado_o_carro_fica_parado() {
        let (e, a) = combinar(0, &sem_teclas(), Fase::Correndo);
        assert_eq!(e, Entrada::PARADA);
        assert!(!a.comecar && !a.reiniciar && !a.sair);
        assert_eq!(a.trocar_mapa, 0);
    }

    #[test]
    fn a_do_controle_acelera_e_direcional_vira() {
        let (e, _) = combinar(evdev::ACELERAR | evdev::DIREITA, &sem_teclas(), Fase::Correndo);
        assert_eq!(e.acelerar, 1.0);
        assert_eq!(e.virar, 1.0);
    }

    #[test]
    fn esquerda_e_direita_juntos_cancelam() {
        // Acontece de verdade: o jogador aperta as duas no pânico. O carro tem de
        // seguir reto, não escolher um lado por ordem de leitura.
        let (e, _) = combinar(evdev::ESQUERDA | evdev::DIREITA, &sem_teclas(), Fase::Correndo);
        assert_eq!(e.virar, 0.0);
    }

    #[test]
    fn teclado_e_controle_somam_para_um_so() {
        // `or` booleano em `f32` não existe; o que não pode acontecer é virar 2.0
        // e o carro girar duas vezes mais rápido com as duas fontes apertadas.
        let teclas = Teclas {
            cima: true,
            direita: true,
            ..sem_teclas()
        };
        let (e, _) = combinar(evdev::ACELERAR | evdev::DIREITA, &teclas, Fase::Correndo);
        assert_eq!(e.acelerar, 1.0);
        assert_eq!(e.virar, 1.0, "virar não pode passar de 1");
    }

    #[test]
    fn espaco_e_freio_de_mao_e_shift_e_nitro() {
        let teclas = Teclas {
            espaco: true,
            shift: true,
            ..sem_teclas()
        };
        let (e, _) = combinar(0, &teclas, Fase::Correndo);
        assert!(e.freio_mao);
        assert!(e.nitro);
    }

    #[test]
    fn l1_e_r2_do_controle_fazem_o_mesmo_que_espaco_e_shift() {
        let (e, _) = combinar(evdev::FREIO_MAO | evdev::NITRO, &sem_teclas(), Fase::Correndo);
        assert!(e.freio_mao);
        assert!(e.nitro);
    }

    #[test]
    fn enter_no_titulo_comeca_a_corrida() {
        let teclas = Teclas {
            enter: true,
            ..sem_teclas()
        };
        let (_, a) = combinar(0, &teclas, Fase::Titulo);
        assert!(a.comecar, "enter no título tem de começar");
        assert!(!a.reiniciar);
    }

    #[test]
    fn enter_no_resultado_reinicia() {
        let teclas = Teclas {
            enter: true,
            ..sem_teclas()
        };
        let (_, a) = combinar(0, &teclas, Fase::Resultado);
        assert!(a.reiniciar);
        assert!(!a.comecar);
    }

    #[test]
    fn enter_correndo_nao_faz_nada() {
        // Sem isso, apertar start no meio da corrida reiniciaria a partida e o
        // jogador perderia o progresso sem entender.
        let teclas = Teclas {
            enter: true,
            ..sem_teclas()
        };
        let (_, a) = combinar(0, &teclas, Fase::Correndo);
        assert!(!a.comecar && !a.reiniciar);
    }

    #[test]
    fn esc_e_select_saem_em_qualquer_fase() {
        for fase in [Fase::Titulo, Fase::Correndo, Fase::Resultado] {
            let teclas = Teclas {
                escape: true,
                ..sem_teclas()
            };
            let (_, a) = combinar(0, &teclas, fase);
            assert!(a.sair, "{fase:?}");
            let (_, a) = combinar(evdev::SAIR, &sem_teclas(), fase);
            assert!(a.sair, "{fase:?} pelo controle");
        }
    }

    #[test]
    fn nenhuma_combinacao_produz_entrada_fora_da_faixa() {
        // Varredura: virar sempre em -1..1 e acelerar/frear em 0..1, com
        // qualquer máscara e qualquer tecla.
        for bits in 0..=u32::from(u8::MAX) {
            for teclas in [
                Teclas::default(),
                Teclas {
                    cima: true,
                    baixo: true,
                    esquerda: true,
                    direita: true,
                    ..Teclas::default()
                },
            ] {
                let (e, _) = combinar(bits, &teclas, Fase::Correndo);
                assert!((-1.0..=1.0).contains(&e.virar), "virar {}", e.virar);
                assert!((0.0..=1.0).contains(&e.acelerar));
                assert!((0.0..=1.0).contains(&e.frear));
            }
        }
    }

    #[test]
    fn seta_no_titulo_troca_de_mapa_e_na_corrida_vira_volante() {
        let esq = Teclas {
            esquerda: true,
            ..sem_teclas()
        };
        let (_, a) = combinar(0, &esq, Fase::Titulo);
        assert_eq!(a.trocar_mapa, -1);
        let (e, a) = combinar(0, &esq, Fase::Correndo);
        assert_eq!(a.trocar_mapa, 0, "na corrida a seta é volante, não menu");
        assert_eq!(e.virar, -1.0);

        let (_, a) = combinar(evdev::DIREITA, &sem_teclas(), Fase::Titulo);
        assert_eq!(a.trocar_mapa, 1, "o direcional do controle também escolhe");
    }
}
