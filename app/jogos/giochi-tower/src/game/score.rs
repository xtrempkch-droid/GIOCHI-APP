//! Pontuação (GDD §13).
//!
//! Todo valor vem de `config/score.ron`; este módulo só acumula e sabe a regra de
//! cada evento. O bônus de "HP da torre preservado" é o único que depende do fim
//! da run, e é aplicado uma vez só — [`Score::finish`] é idempotente para que um
//! laço de encerramento chamado duas vezes não dobre a pontuação.

use crate::config::ScoreConfig;

/// Placar da partida.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Score {
    /// Inimigos comuns mortos.
    pub common_kills: u32,
    /// Bosses mortos.
    pub boss_kills: u32,
    /// Vezes que o nemesis foi morto (0 ou 1 por run, em tese).
    pub nemesis_kills: u32,
    /// Ondas limpas.
    pub waves_cleared: u32,
    /// Pontos acumulados.
    pub points: u32,
    /// `true` depois de [`Score::finish`], para a idempotência.
    finished: bool,
}

impl Score {
    /// Placar zerado.
    pub fn new() -> Self {
        Self::default()
    }

    /// Kill comum. Devolve os pontos creditados.
    pub fn add_common_kill(&mut self, config: &ScoreConfig) -> u32 {
        self.common_kills = self.common_kills.saturating_add(1);
        self.credit(config.kill_common)
    }

    /// Kill de boss (GDD §13: 500).
    pub fn add_boss_kill(&mut self, config: &ScoreConfig) -> u32 {
        self.boss_kills = self.boss_kills.saturating_add(1);
        self.credit(config.kill_boss)
    }

    /// Kill do nemesis (GDD §13: tier × 200).
    pub fn add_nemesis_kill(&mut self, tier: u32, config: &ScoreConfig) -> u32 {
        self.nemesis_kills = self.nemesis_kills.saturating_add(1);
        self.credit(config.kill_nemesis_per_tier.saturating_mul(tier))
    }

    /// Onda limpa (GDD §13: 100 × número da onda).
    pub fn add_wave_clear(&mut self, wave: u32, config: &ScoreConfig) -> u32 {
        self.waves_cleared = self.waves_cleared.saturating_add(1);
        self.credit(config.wave_clear_per_wave.saturating_mul(wave))
    }

    /// Bônus de fim de run: 1 ponto por `tower_hp_per_point` HP restante.
    ///
    /// Idempotente: chamar de novo não soma outra vez.
    pub fn finish(&mut self, tower_hp: f32, config: &ScoreConfig) -> u32 {
        if self.finished {
            return 0;
        }
        self.finished = true;
        let per = config.tower_hp_per_point.max(1) as f32;
        let bonus = (tower_hp.max(0.0) / per).floor() as u32;
        self.credit(bonus)
    }

    /// `true` se o bônus final já foi aplicado.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    fn credit(&mut self, points: u32) -> u32 {
        self.points = self.points.saturating_add(points);
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use std::path::Path;

    fn config() -> ScoreConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config"))
            .expect("config válida")
            .score
    }

    #[test]
    fn the_gdd_values_are_the_ones_configured() {
        let c = config();
        assert_eq!(c.kill_common, 10);
        assert_eq!(c.kill_boss, 500);
        assert_eq!(c.kill_nemesis_per_tier, 200);
        assert_eq!(c.wave_clear_per_wave, 100);
        assert_eq!(c.tower_hp_per_point, 10);
    }

    #[test]
    fn events_add_up_like_the_table() {
        let c = config();
        let mut s = Score::new();
        s.add_common_kill(&c); // 10
        s.add_common_kill(&c); // 20
        s.add_boss_kill(&c); // 520
        s.add_nemesis_kill(3, &c); // 520 + 600 = 1120
        s.add_wave_clear(4, &c); // 1120 + 400 = 1520
        assert_eq!(s.points, 1520);
        assert_eq!(s.common_kills, 2);
        assert_eq!(s.boss_kills, 1);
        assert_eq!(s.nemesis_kills, 1);
        assert_eq!(s.waves_cleared, 1);
    }

    #[test]
    fn the_final_bonus_is_one_point_per_ten_tower_hp() {
        let c = config();
        let mut s = Score::new();
        assert_eq!(s.finish(5000.0, &c), 500);
        assert_eq!(s.points, 500);
    }

    #[test]
    fn finishing_twice_does_not_double_the_bonus() {
        let c = config();
        let mut s = Score::new();
        s.finish(1000.0, &c);
        let after_first = s.points;
        assert_eq!(s.finish(1000.0, &c), 0);
        assert_eq!(s.points, after_first);
        assert!(s.is_finished());
    }

    #[test]
    fn a_dead_tower_adds_no_bonus() {
        let c = config();
        let mut s = Score::new();
        assert_eq!(s.finish(0.0, &c), 0);
        assert_eq!(s.points, 0);
    }
}
