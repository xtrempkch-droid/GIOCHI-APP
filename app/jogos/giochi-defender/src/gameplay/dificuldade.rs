//! Dificuldade por onda: **dados**, não números espalhados pelo código.
//!
//! ## Por que existe
//!
//! Até o 3a, o spawn era uma constante (`INTERVALO_SPAWN = 2.0`,
//! `VELOCIDADE = 3.0`) — a onda 3 jogava exatamente igual à onda 1, e a única
//! coisa que crescia era a cota de abates. O jogador vencia por tempo, não por
//! pressão.
//!
//! Aqui cada onda tem intervalo de spawn, velocidade e quantos cubos nascem por
//! vez. As três crescem, e as três são **medidas** por [`crate::gameplay::piloto`]
//! e pelo `--balanceamento`, não estimadas: "parece difícil" não é dado.
//!
//! ## O teto do pool manda no intervalo
//!
//! O envelope do R5 é de 15 cubos. Se a onda 3 nascer um cubo por segundo com
//! velocidade 4, um jogador que não acerta nenhum enche o pool em 15 s, e a
//! partir daí o intervalo deixa de significar qualquer coisa (o `break` no pool
//! cheio come o tempo). Por isso o teste
//! `a_onda_mais_dificil_ainda_cabe_no_pool` compara o pior caso geométrico com a
//! capacidade — e é ele que impede alguém de "só baixar o intervalo" e achar que
//! aumentou a dificuldade.

/// Parâmetros de uma onda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Onda {
    /// Segundos entre nascimentos.
    pub intervalo: f32,
    /// Velocidade dos cubos, em unidades por segundo.
    pub velocidade: f32,
    /// Quantos cubos nascem em cada nascimento.
    pub por_vez: u32,
}

/// Intervalo da primeira onda. **Igual ao do 1b**: a primeira partida não podia
/// mudar de dificuldade por causa deste bloco.
pub const INTERVALO_BASE: f32 = 2.0;
/// Velocidade da primeira onda, igual à do 1b.
pub const VELOCIDADE_BASE: f32 = 3.0;
/// Quanto o intervalo encolhe por onda (fração do valor anterior).
pub const ENCOLHIMENTO_DO_INTERVALO: f32 = 0.72;
/// Quanto a velocidade cresce por onda (fração do valor anterior).
pub const CRESCIMENTO_DA_VELOCIDADE: f32 = 1.18;
/// Piso do intervalo, em segundos.
///
/// Abaixo disto o spawn vira uma parede de cubos que o pool não aguenta — e o
/// jogador não tem como mirar em oito coisas ao mesmo tempo de qualquer forma.
pub const INTERVALO_MINIMO: f32 = 0.9;
/// Teto da velocidade, em unidades por segundo.
///
/// 5,0 é o que mantém o cubo legível: ele cruza os ~15 m de raio em 3 s, e a
/// 6 m/s ele chega antes de o jogador conseguir girar a câmera.
pub const VELOCIDADE_MAXIMA: f32 = 5.0;

/// Parâmetros da onda `numero` (começando em 1).
pub fn da_onda(numero: u32) -> Onda {
    let n = numero.max(1) as i32 - 1;
    let intervalo = (INTERVALO_BASE * ENCOLHIMENTO_DO_INTERVALO.powi(n)).max(INTERVALO_MINIMO);
    let velocidade = (VELOCIDADE_BASE * CRESCIMENTO_DA_VELOCIDADE.powi(n)).min(VELOCIDADE_MAXIMA);
    // A pressão vem de ter de acertar **vários ao mesmo tempo**, não de um spawn
    // mais rápido (que o piso do intervalo já limita). Medido: com um cubo por
    // vez o piloto automático vencia as três ondas sem perder uma vida.
    let por_vez = numero.clamp(1, 3);
    Onda {
        intervalo,
        velocidade,
        por_vez,
    }
}

/// Quantos cubos a onda `numero` pode pôr no ar ao mesmo tempo, no pior caso.
///
/// É o número que tem de caber no pool: quanto tempo um cubo leva do spawn até o
/// asteroide, dividido pelo intervalo, vezes quantos nascem por vez.
pub fn ocupacao_maxima(numero: u32) -> f32 {
    let onda = da_onda(numero);
    let trajeto =
        crate::gameplay::partida::RAIO_SPAWN_MIN - crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE;
    let vida = trajeto / onda.velocidade;
    (vida / onda.intervalo) * onda.por_vez as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::enemy_pool::CAPACIDADE;

    #[test]
    fn a_primeira_onda_e_igual_a_do_bloco_1b() {
        // A regressão que este teste trava: mexer na progressão não pode mudar
        // a primeira partida, que foi medida e ajustada no 1b/1c.
        let onda = da_onda(1);
        assert_eq!(onda.intervalo, INTERVALO_BASE);
        assert_eq!(onda.intervalo, 2.0);
        assert_eq!(onda.velocidade, VELOCIDADE_BASE);
        assert_eq!(onda.velocidade, 3.0);
        assert_eq!(onda.por_vez, 1);
    }

    #[test]
    fn a_dificuldade_cresce_onda_a_onda_ate_o_teto() {
        let mut anterior = da_onda(1);
        for numero in 2..=crate::state::WAVES_TOTAIS {
            let atual = da_onda(numero);
            assert!(
                atual.intervalo <= anterior.intervalo,
                "onda {numero}: intervalo não encolheu"
            );
            assert!(
                atual.velocidade >= anterior.velocidade,
                "onda {numero}: velocidade não cresceu"
            );
            anterior = atual;
        }
        // E cresce de verdade: onda 3 tem de ser mais difícil que onda 1.
        assert!(da_onda(3).intervalo < da_onda(1).intervalo);
        assert!(da_onda(3).velocidade > da_onda(1).velocidade);
    }

    #[test]
    fn os_limites_seguram_ondas_absurdas() {
        for numero in [10, 100, 10_000, u32::MAX] {
            let onda = da_onda(numero);
            assert!(
                onda.intervalo >= INTERVALO_MINIMO,
                "onda {numero}: intervalo {:.3} abaixo do piso",
                onda.intervalo
            );
            assert!(
                onda.velocidade <= VELOCIDADE_MAXIMA,
                "onda {numero}: velocidade {:.3} acima do teto",
                onda.velocidade
            );
            assert!(onda.intervalo.is_finite());
            assert!(onda.velocidade.is_finite());
        }
        // `da_onda(0)` não existe, mas não pode explodir nem devolver zero.
        assert_eq!(da_onda(0), da_onda(1));
    }

    #[test]
    fn a_onda_mais_dificil_ainda_cabe_no_pool() {
        // O teto do R5 é de 15 cubos. Se a ocupação passar disso, o pool enche e
        // o intervalo deixa de significar dificuldade — vira `break` silencioso.
        for numero in 1..=crate::state::WAVES_TOTAIS {
            let ocupacao = ocupacao_maxima(numero);
            assert!(
                ocupacao < CAPACIDADE as f32,
                "onda {numero}: ocupação {ocupacao:.1} contra pool de {CAPACIDADE}"
            );
        }
        // E há folga: o jogador precisa de espaço para errar.
        let pior = (1..=crate::state::WAVES_TOTAIS)
            .map(ocupacao_maxima)
            .fold(0.0f32, f32::max);
        assert!(
            pior < CAPACIDADE as f32 * 0.75,
            "a onda mais difícil ocupa {pior:.1} de {CAPACIDADE} — sem folga"
        );
    }

    #[test]
    fn a_ocupacao_cresce_com_a_onda() {
        let mut anterior = 0.0;
        for numero in 1..=crate::state::WAVES_TOTAIS {
            let atual = ocupacao_maxima(numero);
            assert!(
                atual > anterior,
                "onda {numero}: ocupação {atual:.2} não passou de {anterior:.2}"
            );
            anterior = atual;
        }
    }

    #[test]
    fn a_pressao_cresce_em_cubos_por_vez() {
        // A pressão vem de ter de acertar **vários ao mesmo tempo**, não de um
        // spawn mais rápido — o piso do intervalo já limita esse caminho.
        //
        // Medido com o piloto automático: com um cubo por vez em todas as ondas
        // ele vencia 5/5 sem perder **uma vida**; com 1/2/3 e o intervalo a 72 %
        // por onda, vence 4/5 e termina com 2,8 vidas — ganhável, não trivial.
        assert_eq!(da_onda(1).por_vez, 1);
        assert_eq!(da_onda(2).por_vez, 2);
        assert_eq!(da_onda(3).por_vez, 3);
        // E não cresce sem limite: o pool é de 15.
        assert_eq!(da_onda(10).por_vez, 3);
    }
}
