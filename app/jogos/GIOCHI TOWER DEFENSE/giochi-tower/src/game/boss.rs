//! Bosses (GDD §9): Tanker, Plasma, Brain e Queen.
//!
//! Cada boss é um corpo só, com HP próprio e um conjunto de **eventos** que o
//! `GameState` traduz em dano, spawn e projétil. O boss não conhece o mundo: ele
//! devolve intenções (`BossEvent`) e quem tem o mapa decide onde elas caem. É o
//! que permite testar o padrão de cada um sem montar uma partida inteira.
//!
//! Fases são por porcentagem de HP, vindas de `config/bosses.ron` — a Queen tem
//! três, os outros têm uma. O `speed_multiplier` da fase é aplicado ao movimento,
//! e `slam_double`/`projectile_fan` mudam o que o boss emite.

use glam::Vec3;

use crate::config::{BossDef, BossKind, EnemyKind, GameConfig};

/// Intervalo entre ataques corpo a corpo de um boss, em segundos.
pub const BOSS_MELEE_INTERVAL_S: f32 = 2.0;

/// O que o boss quer que aconteça neste passo.
#[derive(Debug, Clone, PartialEq)]
pub enum BossEvent {
    /// Dano em área (stomp/slam da Queen).
    Stomp {
        /// Centro da área.
        center: Vec3,
        /// Raio, em metros.
        radius_m: f32,
        /// Dano antes da armadura.
        damage: f32,
    },
    /// Invoca adds.
    Summon {
        /// Tipo dos adds.
        kind: EnemyKind,
        /// Quantos.
        count: u32,
    },
    /// Projétil no alvo mais próximo (Plasma, Queen fase 2+).
    Projectile {
        /// Dano do projétil.
        damage: f32,
        /// Quantos projéteis no leque.
        count: u32,
    },
    /// Teleportou para `to`.
    Teleported {
        /// Nova posição.
        to: Vec3,
    },
}

/// Um boss vivo.
#[derive(Debug, Clone, PartialEq)]
pub struct Boss {
    /// Tipo.
    pub kind: BossKind,
    /// Posição no chão.
    pub position: Vec3,
    /// HP atual.
    pub hp: f32,
    /// HP máximo.
    pub max_hp: f32,
    /// Fase atual (0-based).
    pub phase: u8,
    /// Relógio do stomp.
    pub stomp_timer_s: f32,
    /// Relógio de invocação.
    pub summon_timer_s: f32,
    /// Relógio do ataque corpo a corpo.
    pub melee_timer_s: f32,
    /// Relógio do teleporte (Plasma).
    pub teleport_timer_s: f32,
}

impl Boss {
    /// Cria o boss na posição dada, com os valores do RON.
    pub fn spawn(def: &BossDef, position: Vec3) -> Self {
        Self {
            kind: def.id,
            position: Vec3::new(position.x, 0.0, position.z),
            hp: def.hp,
            max_hp: def.hp,
            phase: 0,
            stomp_timer_s: def.stomp_period_s.unwrap_or(f32::INFINITY),
            summon_timer_s: def.summon_period_s.unwrap_or(f32::INFINITY),
            melee_timer_s: BOSS_MELEE_INTERVAL_S,
            teleport_timer_s: 0.0,
        }
    }

    /// Vivo.
    pub fn is_alive(&self) -> bool {
        self.hp > 0.0
    }

    /// Fração de HP, em `0..=1`.
    pub fn hp_pct(&self) -> f32 {
        if self.max_hp <= 0.0 {
            0.0
        } else {
            (self.hp / self.max_hp).clamp(0.0, 1.0)
        }
    }

    /// Porcentagem inteira, para o HUD e o `WaveState`.
    pub fn hp_pct_u8(&self) -> u8 {
        (self.hp_pct() * 100.0).round().clamp(0.0, 100.0) as u8
    }

    /// Byte do tipo, para o `WaveState.boss_id`.
    pub fn kind_byte(&self) -> u8 {
        match self.kind {
            BossKind::Tanker => 1,
            BossKind::Plasma => 2,
            BossKind::Brain => 3,
            BossKind::Queen => 4,
        }
    }

    /// Fase que corresponde ao HP atual, segundo `def.phases`.
    ///
    /// Devolve `0` quando o boss não declara fases (Tanker, Plasma, Brain) — eles
    /// têm um comportamento só.
    pub fn phase_for(&self, def: &BossDef) -> u8 {
        let pct = self.hp_pct() * 100.0;
        for (index, phase) in def.phases.iter().enumerate() {
            if pct <= phase.from_hp_pct && pct >= phase.to_hp_pct {
                return index as u8;
            }
        }
        def.phases.len().saturating_sub(1) as u8
    }

    /// Aplica dano. Devolve `true` quando este golpe matou.
    pub fn take_damage(&mut self, amount: f32) -> bool {
        if amount <= 0.0 || !self.is_alive() {
            return false;
        }
        self.hp -= amount;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            return true;
        }
        false
    }

    /// Um passo do boss. Devolve o que ele quer fazer neste intervalo.
    ///
    /// `nearest_player` é o jogador vivo mais próximo (o Plasma reage a ele, o
    /// Brain foge dele); `tower` é o destino de quem não liga para jogador.
    pub fn step(
        &mut self,
        dt: f32,
        def: &BossDef,
        nearest_player: Option<Vec3>,
        tower: Vec3,
        config: &GameConfig,
    ) -> Vec<BossEvent> {
        let mut events = Vec::new();
        if !self.is_alive() {
            return events;
        }
        self.phase = self.phase_for(def);
        let phase = def.phases.get(self.phase as usize);

        // --- movimento -------------------------------------------------
        let (speed_mult, keeps_distance) = match phase {
            Some(p) => (p.speed_multiplier.max(0.1), def.keeps_distance_m),
            None => (1.0, def.keeps_distance_m),
        };
        let mut target = tower;
        if let Some(player) = nearest_player {
            match self.kind {
                BossKind::Brain => {
                    // Foge: alvo é o ponto oposto ao jogador.
                    target = self.position + (self.position - player).normalize_or_zero() * 25.0;
                }
                BossKind::Plasma => target = player,
                _ => {}
            }
        }
        if let Some(keep) = keeps_distance {
            if let Some(player) = nearest_player {
                let distance = (self.position - player).length();
                if distance < keep {
                    // Recua em vez de encostar.
                    target = self.position + (self.position - player).normalize_or_zero() * 10.0;
                }
            }
        }
        let speed = def.speed_mps * speed_mult;
        let delta = target - self.position;
        if delta.length() > 0.1 {
            self.position += delta.normalize_or_zero() * speed * dt;
            self.position.y = 0.0;
        }

        // --- teleporte do Plasma ---------------------------------------
        if let (Some(trigger), Some(distance)) = (def.teleport_trigger_m, def.teleport_distance_m) {
            self.teleport_timer_s -= dt;
            if let Some(player) = nearest_player {
                if self.position.distance(player) < trigger && self.teleport_timer_s <= 0.0 {
                    let away = (self.position - player).normalize_or_zero();
                    let to = self.position + away * distance;
                    let map = crate::game::camera::MapBounds::new();
                    self.position = map.clamp(Vec3::new(to.x, 0.0, to.z));
                    self.teleport_timer_s = 4.0;
                    events.push(BossEvent::Teleported { to: self.position });
                }
            }
        }

        // --- stomp / slam ----------------------------------------------
        if let Some(radius) = def.stomp_radius_m {
            if self.stomp_timer_s.is_finite() {
                self.stomp_timer_s -= dt;
                if self.stomp_timer_s <= 0.0 {
                    let period = def.stomp_period_s.unwrap_or(15.0).max(0.5);
                    self.stomp_timer_s = period;
                    let damage = def.damage_melee;
                    events.push(BossEvent::Stomp {
                        center: self.position,
                        radius_m: radius,
                        damage,
                    });
                    if phase.map(|p| p.slam_double).unwrap_or(false) {
                        events.push(BossEvent::Stomp {
                            center: self.position,
                            radius_m: radius * 0.75,
                            damage: damage * 0.6,
                        });
                    }
                }
            }
        }

        // --- invocação --------------------------------------------------
        let summon_period = phase
            .filter(|p| p.summon_period_s > 0.0)
            .map(|p| p.summon_period_s)
            .or(def.summon_period_s);
        if let (Some(period), Some(count)) = (summon_period, def.summon_count) {
            if period.is_finite() && period > 0.0 {
                self.summon_timer_s -= dt;
                if self.summon_timer_s <= 0.0 {
                    self.summon_timer_s = period;
                    let phase_count = phase.map(|p| p.summon_count).unwrap_or(count);
                    events.push(BossEvent::Summon {
                        kind: EnemyKind::Swarm,
                        count: phase_count.max(1),
                    });
                }
            }
        }

        // --- projéteis ---------------------------------------------------
        let fan = phase.map(|p| p.projectile_fan).unwrap_or(0);
        if def.damage_projectile > 0.0 && (fan > 0 || self.kind == BossKind::Plasma) {
            self.melee_timer_s -= dt;
            if self.melee_timer_s <= 0.0 {
                self.melee_timer_s = BOSS_MELEE_INTERVAL_S;
                events.push(BossEvent::Projectile {
                    damage: def.damage_projectile,
                    count: fan.max(1),
                });
            }
        } else {
            // Corpo a corpo: quem não tem projétil usa o relógio de melee para
            // nada — o dano de contato é resolvido no `GameState` pela distância.
            self.melee_timer_s = BOSS_MELEE_INTERVAL_S;
        }

        let _ = config;
        events
    }

    /// `true` se o boss declara essa onda no RON (usado pelo spawner).
    pub fn wave_of(kind: BossKind, config: &GameConfig) -> Option<u32> {
        config.boss(kind).map(|b| b.wave)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn boss(kind: BossKind) -> (Boss, BossDef) {
        let c = config();
        let def = c.boss(kind).expect("boss existe").clone();
        (Boss::spawn(&def, Vec3::new(0.0, 0.0, 40.0)), def)
    }

    #[test]
    fn the_four_bosses_have_the_gdd_hp() {
        let c = config();
        for (kind, hp) in [
            (BossKind::Tanker, 2500.0),
            (BossKind::Plasma, 2000.0),
            (BossKind::Brain, 3000.0),
            (BossKind::Queen, 6000.0),
        ] {
            let def = c.boss(kind).expect("boss");
            assert_eq!(def.hp, hp, "HP de {kind:?} (GDD §9)");
            let boss = Boss::spawn(def, Vec3::ZERO);
            assert_eq!(boss.max_hp, hp);
            assert!(boss.is_alive());
        }
    }

    #[test]
    fn the_queen_walks_through_three_phases() {
        let (mut queen, def) = boss(BossKind::Queen);
        assert_eq!(queen.phase_for(&def), 0, "100%: fase 1");
        queen.hp = queen.max_hp * 0.5;
        assert_eq!(queen.phase_for(&def), 1, "50%: fase 2");
        queen.hp = queen.max_hp * 0.2;
        assert_eq!(queen.phase_for(&def), 2, "20%: fase 3");
    }

    #[test]
    fn the_tanker_stomps_on_its_period() {
        let c = config();
        let (mut tanker, def) = boss(BossKind::Tanker);
        let mut stomps = 0;
        for _ in 0..(60 * 16) {
            for event in tanker.step(1.0 / 60.0, &def, None, Vec3::ZERO, &c) {
                if matches!(event, BossEvent::Stomp { .. }) {
                    stomps += 1;
                }
            }
        }
        assert_eq!(stomps, 1, "15 s de período e 16 s de simulação");
    }

    #[test]
    fn the_plasma_keeps_its_distance_and_shoots() {
        let c = config();
        let (mut plasma, def) = boss(BossKind::Plasma);
        let player = Vec3::new(0.0, 0.0, 38.0); // 2 m: dentro do gatilho
        let mut teleported = false;
        let mut shots = 0;
        for _ in 0..120 {
            for event in plasma.step(1.0 / 60.0, &def, Some(player), Vec3::ZERO, &c) {
                match event {
                    BossEvent::Teleported { .. } => teleported = true,
                    BossEvent::Projectile { .. } => shots += 1,
                    _ => {}
                }
            }
        }
        assert!(teleported, "o Plasma teleporta quando o jogador encosta");
        assert!(shots > 0, "e atira de longe");
        assert!(plasma.position.distance(Vec3::new(0.0, 0.0, 40.0)) > 1.0);
    }

    #[test]
    fn the_brain_flees_and_summons() {
        let c = config();
        let (mut brain, def) = boss(BossKind::Brain);
        brain.position = Vec3::new(0.0, 0.0, 10.0);
        let player = Vec3::new(0.0, 0.0, 8.0);
        let mut summons = 0;
        for _ in 0..(60 * 21) {
            for event in brain.step(1.0 / 60.0, &def, Some(player), Vec3::ZERO, &c) {
                if let BossEvent::Summon { count, .. } = event {
                    summons += count;
                }
            }
        }
        assert!(summons >= 4, "GDD §9.3: 4 adds a cada 20 s, veio {summons}");
        assert!(brain.position.z < 10.0, "o Brain se afasta do jogador");
    }

    #[test]
    fn damage_kills_and_the_phase_never_regresses() {
        let (mut queen, def) = boss(BossKind::Queen);
        queen.hp = queen.max_hp * 0.55;
        let _ = queen.step(0.0, &def, None, Vec3::ZERO, &config());
        assert_eq!(queen.phase, 1);
        queen.hp = queen.max_hp * 0.95;
        let _ = queen.step(0.0, &def, None, Vec3::ZERO, &config());
        assert_eq!(queen.phase, 0, "curar volta de fase, como o HP manda");
        assert!(queen.take_damage(1.0e6));
        assert_eq!(queen.hp, 0.0);
        assert!(!queen.is_alive());
    }
}
