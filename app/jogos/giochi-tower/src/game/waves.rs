//! Spawner de ondas (GDD §5).
//!
//! O GDD fixa, por onda, a **duração** (6 min), o **pico de vivos** e a
//! composição em tipos. Este módulo transforma isso em fluxo de spawn:
//!
//! - cada grupo de `waves_campaign.ron` vira um [`GroupRuntime`] com o seu
//!   intervalo;
//! - o spawner avança o relógio dos grupos e devolve os tipos que devem nascer
//!   neste tick;
//! - o teto de vivos (`HostProfileDef::max_enemies`) é respeitado: enquanto o
//!   campo estiver cheio, o spawner **segura** os grupos em vez de descartar
//!   inimigos. É a diferença entre uma onda difícil e uma onda que não aconteceu.
//!
//! Posição de spawn é decisão de quem tem o mapa (bordas, GDD §6), então o
//! spawner devolve **tipos**, não inimigos.

use crate::config::{EnemyKind, GameConfig};

/// Estado de um grupo de spawn dentro da onda.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupRuntime {
    /// Tipo que este grupo gera.
    pub kind: EnemyKind,
    /// Quantos ainda faltam nascer.
    pub remaining: u32,
    /// Intervalo entre nascimentos, em segundos.
    pub interval_s: f32,
    /// Tempo acumulado desde o último nascimento.
    pub accumulator_s: f32,
}

/// Spawner de uma onda.
#[derive(Debug, Clone, PartialEq)]
pub struct Spawner {
    /// Número da onda que está sendo gerada.
    pub wave: u32,
    /// Grupos ativos.
    pub groups: Vec<GroupRuntime>,
    /// Quantos já nasceram nesta onda.
    pub spawned: u32,
    /// Quantos esta onda gera no total.
    pub total: u32,
    /// Pico de vivos permitido pela onda (GDD §5), limitado pelo perfil.
    pub peak_alive: u32,
    /// `true` quando não há mais nada a nascer.
    pub exhausted: bool,
}

impl GroupRuntime {
    /// Monta um grupo da onda.
    ///
    /// `abre_a_onda` marca o grupo que gera o **primeiro** inimigo no primeiro
    /// tick. Sem isso o campo fica vazio pelo intervalo inteiro — 1,8 s na onda
    /// 2, quase 3 s na onda 6 — e o que o jogador vê é "a onda não começou".
    ///
    /// Só o primeiro grupo abre. Deixar todos prontos faria a onda 2 largar
    /// Grunt, Ranged e Flying no mesmo tick, e "introduz Ranged e Flying"
    /// (GDD §5) viraria três tipos simultâneos sem rampa. Os demais entram na
    /// própria cadência — é o que dá o escalonamento que um TD espera.
    fn pronto(kind: EnemyKind, remaining: u32, interval_s: f32, abre_a_onda: bool) -> Self {
        let interval_s = interval_s.max(0.01);
        GroupRuntime {
            kind,
            remaining,
            interval_s,
            accumulator_s: if abre_a_onda { interval_s } else { 0.0 },
        }
    }
}

impl Spawner {
    /// Monta o spawner de uma onda da campanha.
    ///
    /// Onda fora da faixa devolve um spawner vazio e exaurido: quem chama decide
    /// se é erro (campanha) ou transição (Endless).
    pub fn for_campaign_wave(wave: u32, config: &GameConfig) -> Self {
        match config.campaign_wave(wave) {
            Some(def) => Self::from_groups(wave, def.peak_alive, &def.spawns),
            None => Self::empty(wave),
        }
    }

    /// Monta o spawner de uma onda do Endless (GDD §4).
    ///
    /// Não existe composição no GDD para onda ≥ 7; a decisão tomada é **reusar a
    /// composição da onda 6** e crescer a contagem por
    /// `endless.spawn_count_growth` a cada onda acima da campanha, com o pico
    /// crescendo por `peak_alive_growth`. Ver `DECISIONS.md` (D-023).
    pub fn for_endless_wave(wave: u32, config: &GameConfig) -> Self {
        let Some(last) = config.campaign_wave(config.campaign.wave_count) else {
            return Self::empty(wave);
        };
        let extra = wave.saturating_sub(config.campaign.wave_count);
        let growth = 1.0 + config.endless.spawn_count_growth * extra as f32;
        let peak = config
            .endless
            .peak_alive_base
            .saturating_add(config.endless.peak_alive_growth.saturating_mul(extra));

        let groups: Vec<GroupRuntime> = last
            .spawns
            .iter()
            .enumerate()
            .map(|(i, group)| {
                GroupRuntime::pronto(
                    group.kind,
                    ((group.count as f32) * growth).round().max(1.0) as u32,
                    group.interval_s,
                    i == 0,
                )
            })
            .collect();

        let total = groups.iter().map(|g| g.remaining).sum();
        Self {
            wave,
            groups,
            spawned: 0,
            total,
            peak_alive: peak,
            exhausted: total == 0,
        }
    }

    fn from_groups(wave: u32, peak_alive: u32, spawns: &[crate::config::SpawnGroupDef]) -> Self {
        let groups: Vec<GroupRuntime> = spawns
            .iter()
            .enumerate()
            .map(|(i, group)| {
                GroupRuntime::pronto(group.kind, group.count, group.interval_s, i == 0)
            })
            .collect();
        let total = groups.iter().map(|g| g.remaining).sum();
        Self {
            wave,
            groups,
            spawned: 0,
            total,
            peak_alive,
            exhausted: total == 0,
        }
    }

    fn empty(wave: u32) -> Self {
        Self {
            wave,
            groups: Vec::new(),
            spawned: 0,
            total: 0,
            peak_alive: 0,
            exhausted: true,
        }
    }

    /// Quantos ainda faltam nascer.
    pub fn remaining(&self) -> u32 {
        self.groups.iter().map(|g| g.remaining).sum()
    }

    /// `true` quando todos os inimigos da onda já nasceram.
    pub fn is_exhausted(&self) -> bool {
        self.remaining() == 0
    }

    /// Avança o relógio e devolve os tipos que devem nascer neste tick.
    ///
    /// `alive` e `cap` implementam o teto: com o campo cheio nada nasce e **o
    /// tempo não é acumulado** — os grupos continuam prontos, e é por isso que a
    /// onda não "perde" inimigos quando o teto aperta.
    pub fn update(&mut self, dt: f32, alive: usize, cap: usize) -> Vec<EnemyKind> {
        let mut born = Vec::new();
        if self.is_exhausted() {
            self.exhausted = true;
            return born;
        }

        let headroom = cap.saturating_sub(alive);
        if headroom == 0 {
            // Campo cheio: não acumula intervalo, senão a primeira vaga livre
            // vomitaria uma rajada da onda inteira.
            return born;
        }

        for group in self.groups.iter_mut() {
            if group.remaining == 0 || born.len() >= headroom {
                continue;
            }
            group.accumulator_s += dt;
            while group.accumulator_s >= group.interval_s
                && group.remaining > 0
                && born.len() < headroom
            {
                group.accumulator_s -= group.interval_s;
                group.remaining -= 1;
                self.spawned = self.spawned.saturating_add(1);
                born.push(group.kind);
            }
            // Um grupo que acabou não guarda acumulador para a próxima onda.
            if group.remaining == 0 {
                group.accumulator_s = 0.0;
            }
        }

        if self.is_exhausted() {
            self.exhausted = true;
        }
        born
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    #[test]
    fn wave_one_matches_the_configured_composition() {
        let c = config();
        let s = Spawner::for_campaign_wave(1, &c);
        let expected: u32 = c.campaign_wave(1).unwrap().spawns.iter().map(|g| g.count).sum();
        assert_eq!(s.total, expected);
        assert_eq!(s.total, 60, "onda 1 do RON");
        assert_eq!(s.peak_alive, 30);
        assert!(!s.is_exhausted());
    }

    #[test]
    fn the_sixth_wave_is_the_biggest_one() {
        let c = config();
        let mut previous = 0;
        for wave in 1..=c.campaign.wave_count {
            let s = Spawner::for_campaign_wave(wave, &c);
            assert!(s.total >= previous, "onda {wave} menor que a anterior");
            previous = s.total;
        }
    }

    #[test]
    fn the_alive_cap_holds_spawns_instead_of_dropping_them() {
        let c = config();
        let mut s = Spawner::for_campaign_wave(1, &c);
        // Campo cheio: 10 s sem nada nascer.
        for _ in 0..600 {
            assert!(s.update(1.0 / 60.0, 30, 30).is_empty());
        }
        assert_eq!(s.spawned, 0);
        assert_eq!(s.remaining(), s.total, "nada foi descartado");

        // Uma vaga livre: nasce exatamente um, no primeiro tick possível.
        let born = s.update(1.0 / 60.0, 29, 30);
        assert_eq!(born.len(), 1);
    }

    #[test]
    fn the_whole_wave_spawns_and_then_stops() {
        let c = config();
        let mut s = Spawner::for_campaign_wave(1, &c);
        // `u32` como o `total`: contar em `usize` e comparar com `u32` era o
        // que impedia este teste de compilar, e um `as` no assert esconderia
        // que os dois medem a mesma coisa.
        let mut born: u32 = 0;
        for _ in 0..(600 * 60) {
            born += s.update(1.0 / 60.0, 0, 999).len() as u32;
            if s.is_exhausted() {
                break;
            }
        }
        assert_eq!(born, s.total);
        assert!(s.is_exhausted());
        assert_eq!(s.remaining(), 0);
        assert!(s.update(1.0, 0, 999).is_empty(), "onda exaurida não spawna mais");
    }

    #[test]
    fn intervals_are_respected_per_group() {
        let c = config();
        let mut s = Spawner::for_campaign_wave(2, &c);
        // Onda 2 tem Grunt a cada 1,8 s; em 1 s só pode nascer o primeiro.
        let first = s.update(1.0 / 60.0, 0, 999);
        assert_eq!(first.len(), 1, "primeiro nasce no primeiro tick");
        let mut extra = 0;
        for _ in 0..59 {
            extra += s.update(1.0 / 60.0, 0, 999).len();
        }
        assert_eq!(extra, 0, "em 1 s não cabe um segundo Grunt (1,8 s)");
    }

    #[test]
    fn an_out_of_range_wave_is_empty_and_exhausted() {
        let c = config();
        let s = Spawner::for_campaign_wave(99, &c);
        assert_eq!(s.total, 0);
        assert!(s.is_exhausted());
    }

    #[test]
    fn endless_grows_past_the_campaign() {
        let c = config();
        let w7 = Spawner::for_endless_wave(7, &c);
        let w8 = Spawner::for_endless_wave(8, &c);
        assert!(w7.total > 0);
        assert!(w8.total > w7.total, "onda 8 do Endless maior que a 7");
        assert!(w8.peak_alive > w7.peak_alive);
    }
}
