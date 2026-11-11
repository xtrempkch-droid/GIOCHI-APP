//! A torre central — o alvo a defender (GDD §11).
//!
//! HP, armadura e regeneração vêm de `config/tower.ron`; os tetos de loja também,
//! e são aplicados aqui para que a loja (M3) não precise saber aritmética.
//!
//! # Fórmula de armadura (não está no GDD)
//!
//! O GDD dá os números de armadura (5 → 15) mas não diz o que eles fazem. Adotada
//! uma curva de redução com retorno decrescente:
//!
//! ```text
//! dano_final = dano_bruto · 100 / (100 + armadura · 10)
//! ```
//!
//! Armadura 5 → 66,7% do dano · armadura 15 → 40%. Sem a curva, ou a armadura é
//! irrelevante (subtração fixa pequena) ou vira imunidade (subtração fixa grande);
//! e a escala do GDD (5 a 15) só faz sentido num intervalo assim.

use crate::config::TowerConfig;

/// Raio da torre, em metros: a distância a que um inimigo "chegou" nela.
pub const TOWER_RADIUS_M: f32 = 8.0;

/// Posição da torre no mapa: o centro (GDD §6, "torre nas 4 telas centrais").
pub const TOWER_POSITION: glam::Vec3 = glam::Vec3::ZERO;

/// Estado da torre.
#[derive(Debug, Clone, PartialEq)]
pub struct Tower {
    /// HP atual.
    pub hp: f32,
    /// HP máximo (sobe com upgrade de loja).
    pub max_hp: f32,
    /// Armadura.
    pub armor: f32,
    /// Regeneração, por segundo.
    pub regen_per_s: f32,
    /// Dano recebido no último tick, para efeito de HUD (M6).
    pub last_damage: f32,
}

impl Tower {
    /// Torre nos valores base do GDD.
    pub fn new(config: &TowerConfig) -> Self {
        Self {
            hp: config.hp_base,
            max_hp: config.hp_base,
            armor: config.armor_base,
            regen_per_s: config.regen_base,
            last_damage: 0.0,
        }
    }

    /// `true` enquanto a torre está de pé.
    pub fn is_alive(&self) -> bool {
        self.hp > 0.0
    }

    /// Fração de HP restante, em `0..=1`.
    pub fn hp_pct(&self) -> f32 {
        if self.max_hp <= 0.0 {
            return 0.0;
        }
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    /// Dano efetivo depois da armadura.
    pub fn mitigated_damage(&self, raw: f32) -> f32 {
        if raw <= 0.0 {
            return 0.0;
        }
        raw * 100.0 / (100.0 + self.armor.max(0.0) * 10.0)
    }

    /// Aplica dano bruto e devolve o dano efetivo.
    pub fn take_damage(&mut self, raw: f32) -> f32 {
        let effective = self.mitigated_damage(raw);
        self.hp = (self.hp - effective).max(0.0);
        self.last_damage = effective;
        effective
    }

    /// Cura sem passar do máximo.
    pub fn heal(&mut self, amount: f32) {
        if amount > 0.0 {
            self.hp = (self.hp + amount).min(self.max_hp);
        }
    }

    /// Sobe o HP máximo respeitando o teto de loja (GDD §11: 5000 → 8000).
    pub fn upgrade_max_hp(&mut self, amount: f32, config: &TowerConfig) {
        if amount <= 0.0 {
            return;
        }
        let room = (config.hp_max_shop - self.max_hp).max(0.0);
        let gain = amount.min(room);
        self.max_hp += gain;
        self.hp += gain;
    }

    /// Sobe a armadura respeitando o teto de loja (5 → 15).
    pub fn upgrade_armor(&mut self, amount: f32, config: &TowerConfig) {
        if amount <= 0.0 {
            return;
        }
        let room = (config.armor_max_shop - self.armor).max(0.0);
        self.armor += amount.min(room);
    }

    /// Soma regeneração respeitando o teto de loja (0 → 10/s).
    pub fn upgrade_regen(&mut self, amount: f32, config: &TowerConfig) {
        if amount <= 0.0 {
            return;
        }
        let room = (config.regen_max_shop - self.regen_per_s).max(0.0);
        self.regen_per_s += amount.min(room);
    }

    /// Um passo de simulação: regenera.
    pub fn step(&mut self, dt: f32) {
        if self.regen_per_s > 0.0 && self.is_alive() {
            let before = self.hp;
            self.hp = (self.hp + self.regen_per_s * dt).min(self.max_hp);
            if self.hp > before {
                self.last_damage = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn config() -> TowerConfig {
        crate::config::GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config"))
            .expect("config válida")
            .tower
    }

    #[test]
    fn the_tower_starts_with_the_gdd_numbers() {
        let t = Tower::new(&config());
        assert_eq!(t.hp, 5000.0);
        assert_eq!(t.max_hp, 5000.0);
        assert_eq!(t.armor, 5.0);
        assert_eq!(t.regen_per_s, 0.0);
        assert!(t.is_alive());
    }

    #[test]
    fn armor_reduces_damage_with_diminishing_returns() {
        let t = Tower::new(&config());
        // armadura 5: 100 / 150
        assert!((t.mitigated_damage(150.0) - 100.0).abs() < 1.0e-4);
        let mut heavy = Tower::new(&config());
        heavy.armor = 15.0;
        // armadura 15: 100 / 250
        assert!((heavy.mitigated_damage(150.0) - 60.0).abs() < 1.0e-4);
        assert!(heavy.mitigated_damage(150.0) < t.mitigated_damage(150.0));
    }

    #[test]
    fn hp_never_goes_below_zero() {
        let mut t = Tower::new(&config());
        t.take_damage(100_000.0);
        assert_eq!(t.hp, 0.0);
        assert!(!t.is_alive());
    }

    #[test]
    fn upgrades_respect_the_shop_ceilings() {
        let config = config();
        let mut t = Tower::new(&config);
        for _ in 0..20 {
            t.upgrade_max_hp(500.0, &config);
            t.upgrade_armor(2.0, &config);
            t.upgrade_regen(2.0, &config);
        }
        assert_eq!(t.max_hp, 8000.0, "GDD §11: teto de HP");
        assert_eq!(t.armor, 15.0, "GDD §11: teto de armadura");
        assert_eq!(t.regen_per_s, 10.0, "GDD §11: teto de regeneração");
    }

    #[test]
    fn regen_heals_up_to_the_maximum_only() {
        let config = config();
        let mut t = Tower::new(&config);
        t.regen_per_s = 10.0;
        t.take_damage(1000.0);
        for _ in 0..1000 {
            t.step(1.0);
        }
        assert_eq!(t.hp, t.max_hp);
    }
}
