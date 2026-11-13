//! Contagem de tempo para cadências de combate.
//!
//! ## O bug que este módulo existe para não repetir
//!
//! Sessenta passos de `1/60` somam **0,99999994** em `f32`, não 1,0. Um
//! contador que dispara em `<= 0.0` puro perde o tique do intervalo exato:
//!
//! ```text
//! esperado: golpes em t = 1,0  2,0  3,0   → 3 golpes em 3 s
//! medido:   golpes em t = 1,02 2,03 3,05 → 2 golpes em 3 s
//! ```
//!
//! E o erro não se corrige sozinho: ele se acumula, porque o resíduo é
//! descartado a cada disparo. Numa onda de 6 min isso é ~2% menos dano recebido
//! — invisível na tela, impossível de atribuir a alguma coisa depois.
//!
//! ## As duas partes da correção
//!
//! 1. **Tolerância** de 0,1 ms: ordens de grandeza abaixo de um passo de
//!    simulação (16,7 ms a 60 Hz) e acima do erro de `f32` em 60 passos. Faz o
//!    disparo cair no tique certo.
//! 2. **Soma** do intervalo em vez de sobrescrever: o resíduo atravessa o ciclo,
//!    então a taxa média converge para o intervalo em vez de ficar lenta.
//!
//! A regra mora aqui, e não copiada em cada `tick_*`, porque duas cópias
//! envelhecem diferente — a lição que este repositório já pagou com o enum de
//! runtimes e com o catálogo (`AGENTS.md` §16 e §26).

/// Tolerância de comparação, em segundos.
pub const TOLERANCIA_S: f32 = 1.0e-4;

/// Conta `dt` num contador que recarrega em `intervalo`.
///
/// Devolve `true` no passo em que o contador dispara. `intervalo <= 0` ou não
/// finito nunca dispara — chamar com dado inválido não pode virar rajada.
pub fn recarregavel(timer: &mut f32, dt: f32, intervalo: f32) -> bool {
    if !intervalo.is_finite() || intervalo <= 0.0 || !dt.is_finite() {
        return false;
    }
    *timer -= dt;
    if *timer <= TOLERANCIA_S {
        *timer += intervalo;
        // Um contador que ficou muito atrás (pausa longa, dt gigante) não pode
        // devolver décadas de disparos: reancora no intervalo.
        if *timer < 0.0 {
            *timer = intervalo;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_um_segundo_a_sessenta_hz_da_um_disparo_por_segundo() {
        let mut t = 1.0;
        let mut disparos = 0;
        for _ in 0..180 {
            if recarregavel(&mut t, 1.0 / 60.0, 1.0) {
                disparos += 1;
            }
        }
        assert_eq!(disparos, 3, "3 s a 1/s = 3 disparos, veio {disparos}");
    }

    #[test]
    fn a_taxa_media_nao_escapa_do_intervalo() {
        // 60 s a 0,5/s têm de dar 30 disparos, não 29. É o resíduo descartado
        // que fazia o contador atrasar para sempre.
        let mut t = 0.5;
        let mut disparos = 0;
        for _ in 0..(60 * 60) {
            if recarregavel(&mut t, 1.0 / 60.0, 0.5) {
                disparos += 1;
            }
        }
        assert_eq!(disparos, 120, "60 s a 2 Hz = 120, veio {disparos}");
    }

    #[test]
    fn intervalo_invalido_nunca_dispara() {
        let mut t = 1.0;
        assert!(!recarregavel(&mut t, 1.0, 0.0));
        assert!(!recarregavel(&mut t, 1.0, f32::NAN));
        assert!(!recarregavel(&mut t, 1.0, -3.0));
    }

    #[test]
    fn dt_gigante_nao_vira_rajada() {
        // Uma pausa de 10 s num contador de 1 s não pode disparar 10 vezes num
        // passo só (o `step` devolve um evento por chamada, não uma fila).
        let mut t = 1.0;
        assert!(recarregavel(&mut t, 10.0, 1.0));
        assert!(t > 0.0, "contador reancorado, não negativo: {t}");
    }
}
