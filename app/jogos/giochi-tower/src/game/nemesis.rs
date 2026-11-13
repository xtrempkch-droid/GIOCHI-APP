//! "O Coronel" — o nemesis (GDD §10).
//!
//! Spawna na onda 1 numa borda, **evolui a cada onda que sobrevive** e, se morrer,
//! não volta. A posição que o minimapa mostra é sempre o **último avistamento**,
//! defasado em `nemesis.minimap_delay_s` (5 s no GDD) — nunca a posição real.
//!
//! # Mesma forma dos bosses
//!
//! O nemesis devolve [`NemesisEvent`] e o `GameState` aplica. Assim o tier 5
//! ("mais forte que a Queen", GDD §10) é uma linha de configuração, não um caso
//! especial espalhado pelo código.

use glam::Vec3;

use crate::config::{GameConfig, ItemId, NemesisTierDef};

/// Intervalo entre ataques do nemesis, em segundos.
pub const NEMESIS_ATTACK_INTERVAL_S: f32 = 1.5;

/// Distância a que o nemesis acerta um jogador.
pub const NEMESIS_REACH_M: f32 = 2.5;

/// O que o nemesis quer que aconteça.
#[derive(Debug, Clone, PartialEq)]
pub enum NemesisEvent {
    /// Dano massivo na torre (penalidade por ignorar, GDD §10).
    HitTower {
        /// Dano bruto.
        damage: f32,
    },
    /// Acertou um jogador.
    HitPlayer {
        /// Quem.
        player_id: u8,
        /// Dano.
        damage: f32,
    },
    /// Quantas viradas ficam na fila.
    Summon {
        /// Quantos.
        count: u32,
    },
}

/// O nemesis vivo.
/// Teto de tier do nemesis. Cinco viradas, e no sexto ele fica como está:
/// "mais forte que a Queen, mas matável" (GDD §10).
pub const TIER_MAX: u8 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct Nemesis {
    /// Tier atual (0..=5).
    pub tier: u8,
    /// Posição real no mundo.
    pub position: Vec3,
    /// HP atual.
    pub hp: f32,
    /// HP máximo do tier.
    pub max_hp: f32,
    /// Dano por acerto do tier.
    pub damage: f32,
    /// Último avistamento conhecido (vai para o minimapa).
    pub last_seen: Vec3,
    /// Segundos até o próximo avistamento ser registrado.
    pub sighting_timer_s: f32,
    /// Relógio de ataque.
    pub attack_timer_s: f32,
    /// Relógio de invocação.
    pub summon_timer_s: f32,
    /// Vivo (se morrer, a caçada acaba na run).
    pub alive: bool,
}

impl Nemesis {
    /// Cria o nemesis no tier 0 (GDD §10).
    pub fn spawn(config: &GameConfig, position: Vec3) -> Self {
        let tier = Self::tier_def(config, 0).cloned().unwrap_or(NemesisTierDef {
            tier: 0,
            hp: config.nemesis.base_hp,
            damage: config.nemesis.base_damage,
            gains: Vec::new(),
            damage_reduction_pct: 0.0,
            dash: false,
            summons: 0,
            summons_period_s: 0.0,
            ranged_attack: false,
        });
        Self {
            tier: 0,
            position: Vec3::new(position.x, 0.0, position.z),
            hp: tier.hp,
            max_hp: tier.hp,
            damage: tier.damage,
            last_seen: Vec3::ZERO,
            sighting_timer_s: 0.0,
            attack_timer_s: NEMESIS_ATTACK_INTERVAL_S,
            summon_timer_s: if tier.summons > 0 {
                tier.summons_period_s.max(1.0)
            } else {
                f32::INFINITY
            },
            alive: true,
        }
    }

    /// Definição de um tier no RON.
    pub fn tier_def(config: &GameConfig, tier: u32) -> Option<&NemesisTierDef> {
        config.nemesis.tiers.iter().find(|t| t.tier == tier)
    }

    /// `true` se está vivo.
    pub fn is_alive(&self) -> bool {
        self.alive && self.hp > 0.0
    }

    /// Fração de HP.
    pub fn hp_pct(&self) -> f32 {
        if self.max_hp <= 0.0 {
            0.0
        } else {
            (self.hp / self.max_hp).clamp(0.0, 1.0)
        }
    }

    /// Sobe um tier ao fim da onda sobrevivida. Devolve `true` se subiu.
    ///
    /// No tier máximo (5) ele fica como está: "mais forte que a Queen, mas
    /// matável" (GDD §10), sem crescer para sempre.
    pub fn on_wave_survived(&mut self, config: &GameConfig) -> bool {
        if !self.is_alive() || self.tier >= TIER_MAX {
            return false;
        }
        let next = u32::from(self.tier) + 1;
        let Some(def) = Self::tier_def(config, next) else {
            return false;
        };
        // Atribui `next`, não `def.tier`: `next` é o valor que o guard acima já
        // provou caber em `u8` e estar abaixo do teto. `def.tier` vem do RON e
        // pode divergir do índice que o encontrou — confiar nele deixaria uma
        // config errada escrever qualquer coisa no estado do jogo.
        self.tier = u8::try_from(next).unwrap_or(TIER_MAX);
        // HP novo é o do tier; cura junto (o Coronel "cresce" entre ondas).
        let ratio = self.hp_pct().max(0.01);
        self.max_hp = def.hp;
        self.hp = (def.hp * ratio).max(1.0);
        self.damage = def.damage;
        if def.summons > 0 {
            self.summon_timer_s = def.summons_period_s.max(1.0);
        }
        log::info!(
            "Coronel subiu para o tier {}: HP {:.0}, dano {:.0}",
            self.tier,
            self.max_hp,
            self.damage
        );
        true
    }

    /// Aplica dano, com a redução do tier. Devolve `true` se matou.
    pub fn take_damage(&mut self, amount: f32, config: &GameConfig) -> bool {
        if amount <= 0.0 || !self.is_alive() {
            return false;
        }
        let reduction = Self::tier_def(config, u32::from(self.tier))
            .map(|t| t.damage_reduction_pct)
            .unwrap_or(0.0)
            .clamp(0.0, 95.0);
        let effective = amount * (1.0 - reduction / 100.0);
        self.hp -= effective;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.alive = false;
            return true;
        }
        false
    }

    /// Um passo: persegue o jogador vivo mais próximo (ou a torre) e ataca.
    pub fn step(
        &mut self,
        dt: f32,
        nearest_player: Option<(u8, Vec3)>,
        tower: Vec3,
        config: &GameConfig,
    ) -> Vec<NemesisEvent> {
        let mut events = Vec::new();
        if !self.is_alive() {
            return events;
        }
        let def = Self::tier_def(config, u32::from(self.tier)).cloned();
        let speed = 3.0 + f32::from(self.tier) * 0.4;

        let target = nearest_player.map(|(_, pos)| pos).unwrap_or(tower);
        let delta = target - self.position;
        let distance = delta.length();
        if distance > NEMESIS_REACH_M {
            self.position += delta.normalize_or_zero() * speed * dt;
            self.position.y = 0.0;
        }

        // Aviso de posição defasado (GDD §10: ~5 s).
        self.sighting_timer_s -= dt;
        if self.sighting_timer_s <= 0.0 {
            self.last_seen = self.position;
            self.sighting_timer_s = config.nemesis.minimap_delay_s.max(0.5);
        }

        // Ataque.
        self.attack_timer_s -= dt;
        if self.attack_timer_s <= 0.0 {
            self.attack_timer_s = NEMESIS_ATTACK_INTERVAL_S;
            if self.position.distance(tower) <= crate::game::tower::TOWER_RADIUS_M {
                events.push(NemesisEvent::HitTower {
                    damage: config.nemesis.tower_damage_massive,
                });
            } else if let Some((id, pos)) = nearest_player {
                if self.position.distance(pos) <= NEMESIS_REACH_M {
                    events.push(NemesisEvent::HitPlayer {
                        player_id: id,
                        damage: self.damage,
                    });
                }
            }
        }

        // Invocação a partir do tier 2.
        if let Some(def) = def {
            if def.summons > 0 && def.summons_period_s > 0.0 {
                self.summon_timer_s -= dt;
                if self.summon_timer_s <= 0.0 {
                    self.summon_timer_s = def.summons_period_s.max(1.0);
                    events.push(NemesisEvent::Summon {
                        count: def.summons,
                    });
                }
            }
        }

        events
    }

    /// Recompensa por matar: score e o item exclusivo (GDD §10).
    pub fn reward(&self, config: &GameConfig) -> (u32, ItemId) {
        let score = config
            .nemesis
            .reward_score_per_tier
            .saturating_mul(u32::from(self.tier));
        (score, config.nemesis.exclusive_item)
    }

    /// Posição que o minimapa pode mostrar (defasada).
    pub fn broadcast_pos(&self) -> Vec3 {
        self.last_seen
    }

    /// Ganhos acumulados do tier atual, para o log/HUD.
    pub fn gains(&self, config: &GameConfig) -> Vec<String> {
        Self::tier_def(config, u32::from(self.tier))
            .map(|t| t.gains.clone())
            .unwrap_or_default()
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
    fn tier_zero_is_the_gdd_grunt_and_the_final_tier_is_stronger_than_the_queen() {
        let c = config();
        let n = Nemesis::spawn(&c, Vec3::ZERO);
        assert_eq!(n.tier, 0);
        assert_eq!(n.max_hp, 300.0, "GDD §10: tier 0 tem 300 HP");
        assert_eq!(n.damage, 20.0);

        let tier5 = Nemesis::tier_def(&c, 5).expect("tier 5");
        assert_eq!(tier5.hp, 5000.0, "GDD §10: HP do tier 5");
        // "Mais forte que a Queen" é HP **efetivo**: 5000 com 30% de redução de
        // dano exigem 7143 de dano bruto, contra 6000 da Queen. Comparar HP cru
        // reprovaria os dois números do GDD ao mesmo tempo.
        assert!(
            c.nemesis_tier5_effective_hp() > c.boss(crate::config::BossKind::Queen).unwrap().hp,
            "tier 5 efetivo {:.0} contra Queen {}",
            c.nemesis_tier5_effective_hp(),
            c.boss(crate::config::BossKind::Queen).unwrap().hp
        );
    }

    #[test]
    fn surviving_a_wave_raises_the_tier_and_stops_at_five() {
        let c = config();
        let mut n = Nemesis::spawn(&c, Vec3::ZERO);
        for expected in 1..=5 {
            assert!(n.on_wave_survived(&c));
            assert_eq!(n.tier, expected);
        }
        assert!(!n.on_wave_survived(&c), "no tier 5 ele para de crescer");
        assert_eq!(n.tier, 5);
        assert_eq!(n.max_hp, 5000.0);
    }

    #[test]
    fn the_tier_three_reduces_damage_by_thirty_percent() {
        let c = config();
        let mut n = Nemesis::spawn(&c, Vec3::ZERO);
        for _ in 0..3 {
            n.on_wave_survived(&c);
        }
        assert_eq!(n.tier, 3);
        let before = n.hp;
        n.take_damage(100.0, &c);
        // 30% de redução: 100 pontos brutos viram 70.
        assert!((before - n.hp - 70.0).abs() < 1.0e-3);
    }

    #[test]
    fn killing_pays_score_by_tier_and_the_exclusive_item() {
        let c = config();
        let mut n = Nemesis::spawn(&c, Vec3::ZERO);
        for _ in 0..3 {
            n.on_wave_survived(&c);
        }
        let (score, item) = n.reward(&c);
        assert_eq!(score, 600, "tier 3 × 200");
        assert_eq!(item, c.nemesis.exclusive_item);
        assert!(c.item(item).map(|i| i.exclusive).unwrap_or(false));
    }

    #[test]
    fn a_dead_nemesis_never_comes_back() {
        let c = config();
        let mut n = Nemesis::spawn(&c, Vec3::ZERO);
        assert!(n.take_damage(1.0e6, &c));
        assert!(!n.is_alive());
        assert!(!n.on_wave_survived(&c), "morto não evolui");
        assert!(n.step(1.0, None, Vec3::ZERO, &c).is_empty());
    }

    #[test]
    fn the_broadcast_position_is_the_delayed_sighting() {
        let c = config();
        let mut n = Nemesis::spawn(&c, Vec3::new(50.0, 0.0, 50.0));
        // Sem step, nada foi avistado ainda.
        assert_eq!(n.broadcast_pos(), Vec3::ZERO);
        n.step(0.1, None, crate::game::tower::TOWER_POSITION, &c);
        assert_ne!(n.broadcast_pos(), Vec3::ZERO, "o primeiro avistamento registra");
        // A posição real anda, a do minimapa demora.
        let seen = n.broadcast_pos();
        n.step(0.1, None, crate::game::tower::TOWER_POSITION, &c);
        assert_eq!(n.broadcast_pos(), seen, "5 s de defasagem");
    }
}
