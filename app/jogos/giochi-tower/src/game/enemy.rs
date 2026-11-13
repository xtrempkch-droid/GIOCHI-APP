//! Inimigos comuns: os cinco tipos do GDD §8, spawn e IA de movimento.
//!
//! # Escala por onda
//!
//! O GDD fixa a fórmula e ela é aplicada na **criação** do inimigo
//! ([`Enemy::spawn`]) a partir de [`crate::config::Scaling`]. Isso é de propósito:
//! inimigo que já nasceu não muda de HP quando a onda avança, e reiniciar uma onda
//! produz exatamente os mesmos números (o que o M6 vai querer para balancear).
//!
//! # IA e o perfil de host
//!
//! A IA roda a `ai_hz` (GDD §16: 20 Hz no Full, 12 Hz no Rk3326), **desacoplada do
//! render**. Quem chama decide o intervalo; este módulo só sabe dar um passo.
//! Pathfinding é plugável por natureza: [`Enemy::steering_target`] devolve o ponto
//! que o inimigo quer alcançar, e `config::HostProfileDef::pathfinding` escolhe
//! como chegar nele. No M2 só existe o steering (ir em linha reta); A* entra no
//! M5, quando houver obstáculos no mapa para justificar o grafo.

use glam::Vec3;

use crate::config::{EnemyBehavior, EnemyDef, EnemyKind, Scaling};

/// Raio de colisão de um inimigo comum, em metros.
pub const ENEMY_RADIUS_M: f32 = 0.8;

/// Distância a que um inimigo "vê" um jogador e troca de alvo (GDD §8: o nemesis
/// pressiona jogadores isolados; os comuns vão na torre, salvo se o jogador
/// estiver no caminho).
pub const AGGRO_RADIUS_M: f32 = 12.0;

/// Intervalo entre ataques de um inimigo, em segundos.
pub const ATTACK_INTERVAL_S: f32 = 1.0;

/// Um inimigo vivo no mundo do host.
#[derive(Debug, Clone, PartialEq)]
pub struct Enemy {
    /// Identificador estável durante a vida (vai no snapshot, é `u16` como o
    /// `SnapshotInimigo` do `giochi-net`).
    pub id: u16,
    /// Tipo.
    pub kind: EnemyKind,
    /// Posição no plano do jogo (Y = 0; voadores usam Y para o desenho).
    pub position: Vec3,
    /// Vida atual.
    pub hp: f32,
    /// Vida máxima, com a escala da onda já aplicada.
    pub max_hp: f32,
    /// Dano por ataque, com a escala da onda já aplicada.
    pub damage: f32,
    /// Velocidade, com a escala da onda já aplicada.
    pub speed_mps: f32,
    /// Distância a que ataca.
    pub attack_range_m: f32,
    /// Como se move.
    pub behavior: EnemyBehavior,
    /// Ignora o chão/obstáculos.
    pub flying: bool,
    /// Contador até o próximo ataque.
    pub attack_timer_s: f32,
    /// Fase de chefe (0 = não é chefe). Preenchido pelo M3.
    pub boss_phase: u8,
    /// Alvo atual no plano do jogo.
    pub target: Vec3,
}

impl Enemy {
    /// Cria um inimigo do tipo `def` em `position`, já escalado pela onda.
    pub fn spawn(id: u16, def: &EnemyDef, position: Vec3, scaling: Scaling) -> Self {
        let hp = (def.hp * scaling.hp).max(1.0);
        Self {
            id,
            kind: def.id,
            position: Vec3::new(position.x, 0.0, position.z),
            hp,
            max_hp: hp,
            damage: (def.damage * scaling.damage).max(0.0),
            speed_mps: (def.speed_mps * scaling.speed).max(0.0),
            attack_range_m: def.attack_range_m.max(0.0),
            behavior: def.behavior,
            flying: def.flying,
            attack_timer_s: ATTACK_INTERVAL_S,
            boss_phase: 0,
            target: Vec3::ZERO,
        }
    }

    /// `true` enquanto o inimigo pode agir.
    pub fn is_alive(&self) -> bool {
        self.hp > 0.0
    }

    /// Aplica dano. Devolve `true` se este golpe matou.
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

    /// Distância no plano do chão até `point`.
    pub fn distance_to(&self, point: Vec3) -> f32 {
        let dx = self.position.x - point.x;
        let dz = self.position.z - point.z;
        (dx * dx + dz * dz).sqrt()
    }

    /// Distância até a torre.
    pub fn distance_to_tower(&self) -> f32 {
        self.distance_to(crate::game::tower::TOWER_POSITION)
    }

    /// Ponto que o inimigo quer alcançar neste passo.
    ///
    /// `nearest_player` é a posição do jogador vivo mais próximo, se houver um
    /// dentro do raio de aggro. Cada comportamento do GDD §8 responde diferente:
    /// o `Ranged` **para** a uma distância, o `Swarm` mantém a coesão (o alvo é o
    /// grupo, representado pela torre) e o `Flying` ignora obstáculos porque voa.
    pub fn steering_target(&self, nearest_player: Option<Vec3>) -> Vec3 {
        let tower = crate::game::tower::TOWER_POSITION;
        match self.behavior {
            EnemyBehavior::Straight | EnemyBehavior::SlowHeavy | EnemyBehavior::Flying => tower,
            EnemyBehavior::SwarmGroup => nearest_player.unwrap_or(tower),
            EnemyBehavior::StopAndShoot => nearest_player.unwrap_or(tower),
        }
    }

    /// Alvo verdadeiramente perseguido neste passo, considerando aggro.
    pub fn pick_target(&self, nearest_player: Option<Vec3>) -> Vec3 {
        match nearest_player {
            Some(player) if self.distance_to(player) <= AGGRO_RADIUS_M => player,
            _ => self.steering_target(None),
        }
    }

    /// Um passo de movimento em direção a `target`.
    ///
    /// O `StopAndShoot` para antes de encostar: se já está dentro do alcance, não
    /// anda — é o "para e atira" do GDD §8.
    pub fn move_towards(&mut self, target: Vec3, dt: f32) {
        let distance = self.distance_to(target);
        let stop_at = if self.behavior == EnemyBehavior::StopAndShoot {
            self.attack_range_m
        } else {
            0.0
        };
        if distance <= stop_at + 0.05 || distance <= 1.0e-4 {
            return;
        }
        let step = (self.speed_mps * dt).min(distance - stop_at);
        if step <= 0.0 {
            return;
        }
        let dx = (target.x - self.position.x) / distance;
        let dz = (target.z - self.position.z) / distance;
        self.position.x += dx * step;
        self.position.z += dz * step;
        self.target = target;
    }

    /// Faz o relógio de ataque correr. Devolve `true` quando é hora de acertar.
    pub fn tick_attack(&mut self, dt: f32, in_range: bool) -> bool {
        if !in_range || !self.is_alive() {
            // Fora de alcance o relógio não corre, mas também não zera: um
            // inimigo que encosta na torre ataca no ritmo certo, sem rajada.
            return false;
        }
        // Cadência com tolerância de `f32` e resíduo preservado — o porquê
        // completo está em `game::timers`, junto com o teste que fixa a taxa.
        use crate::game::timers;
        timers::recarregavel(&mut self.attack_timer_s, dt, ATTACK_INTERVAL_S)
    }

    /// `true` se está ao alcance de acertar `point`.
    pub fn in_range_of(&self, point: Vec3) -> bool {
        self.distance_to(point) <= self.attack_range_m + ENEMY_RADIUS_M
    }

    /// HP como porcentagem, para o snapshot.
    pub fn hp_u16(&self) -> u16 {
        if self.max_hp <= 0.0 {
            return 0;
        }
        ((self.hp / self.max_hp).clamp(0.0, 1.0) * f32::from(u16::MAX)) as u16
    }

    /// Byte do tipo, para o `SnapshotInimigo.tipo` do `giochi-net`.
    pub fn kind_byte(&self) -> u8 {
        match self.kind {
            EnemyKind::Grunt => 0,
            EnemyKind::Tank => 1,
            EnemyKind::Ranged => 2,
            EnemyKind::Swarm => 3,
            EnemyKind::Flying => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn enemy(kind: EnemyKind, wave: u32) -> Enemy {
        let c = config();
        let def = c.enemy(kind).expect("tipo existe");
        Enemy::spawn(1, def, Vec3::new(0.0, 0.0, 50.0), c.scaling_for_wave(wave))
    }

    #[test]
    fn the_five_types_scale_by_the_gdd_formula() {
        let c = config();
        let wave = 3;
        let scaling = c.scaling_for_wave(wave);
        let def = c.enemy(EnemyKind::Grunt).expect("grunt");
        let e = Enemy::spawn(1, def, Vec3::ZERO, scaling);
        // 30 × (1 + 0.15·3) = 30 × 1.45
        assert!((e.max_hp - 43.5).abs() < 1.0e-4, "hp {}", e.max_hp);
        // 5 × (1 + 0.10·3) = 6.5
        assert!((e.damage - 6.5).abs() < 1.0e-4);
        // 2,2 × (1 + 0.05·3) = 2,53
        assert!((e.speed_mps - 2.53).abs() < 1.0e-4);
    }

    #[test]
    fn later_waves_are_strictly_harder() {
        let w1 = enemy(EnemyKind::Tank, 1);
        let w6 = enemy(EnemyKind::Tank, 6);
        assert!(w6.max_hp > w1.max_hp);
        assert!(w6.damage > w1.damage);
        assert!(w6.speed_mps > w1.speed_mps);
    }

    #[test]
    fn a_walking_enemy_gets_closer_to_the_tower() {
        let mut e = enemy(EnemyKind::Grunt, 1);
        let before = e.distance_to_tower();
        e.move_towards(crate::game::tower::TOWER_POSITION, 1.0);
        assert!(e.distance_to_tower() < before);
    }

    #[test]
    fn the_ranged_enemy_stops_at_its_attack_range() {
        let mut e = enemy(EnemyKind::Ranged, 1);
        for _ in 0..600 {
            e.move_towards(crate::game::tower::TOWER_POSITION, 0.1);
        }
        let distance = e.distance_to_tower();
        assert!(
            (distance - e.attack_range_m).abs() < 0.2,
            "parou a {distance} m, alcance {} m",
            e.attack_range_m
        );
        assert!(e.in_range_of(crate::game::tower::TOWER_POSITION));
    }

    #[test]
    fn flying_ignores_nothing_in_this_snapshot_but_keeps_going_straight() {
        let mut e = enemy(EnemyKind::Flying, 1);
        e.move_towards(crate::game::tower::TOWER_POSITION, 1.0);
        let expected = e.speed_mps;
        // Andou exatamente a velocidade num segundo (partindo de longe).
        assert!((e.distance_to_tower() - (50.0 - expected)).abs() < 1.0e-3);
    }

    #[test]
    fn the_attack_cadence_is_one_hit_per_interval() {
        let mut e = enemy(EnemyKind::Grunt, 1);
        let mut hits = 0;
        for _ in 0..(60 * 3) {
            if e.tick_attack(1.0 / 60.0, true) {
                hits += 1;
            }
        }
        // 3 s a 1 ataque/s = 3 golpes (o primeiro precisa de 1 s de relógio).
        assert_eq!(hits, 3, "3 s de contato renderam {hits} golpes");
    }

    #[test]
    fn out_of_range_nothing_happens() {
        let mut e = enemy(EnemyKind::Grunt, 1);
        assert!(!e.tick_attack(1.0, false));
        assert_eq!(e.attack_timer_s, ATTACK_INTERVAL_S);
    }

    #[test]
    fn the_target_switches_to_a_nearby_player() {
        let e = enemy(EnemyKind::Grunt, 1);
        let near = Vec3::new(0.0, 0.0, 45.0);
        assert_eq!(e.pick_target(Some(near)), near, "jogador a 5 m é alvo");

        let far = Vec3::new(0.0, 0.0, -60.0);
        assert_eq!(
            e.pick_target(Some(far)),
            crate::game::tower::TOWER_POSITION,
            "jogador fora do aggro não desvia o Grunt da torre"
        );
    }

    #[test]
    fn damage_never_turns_negative_and_reports_the_kill() {
        let mut e = enemy(EnemyKind::Grunt, 1);
        let hp = e.hp;
        assert!(!e.take_damage(hp - 1.0));
        assert!(e.take_damage(5.0));
        assert_eq!(e.hp, 0.0);
        assert!(!e.is_alive());
    }

    #[test]
    fn the_snapshot_byte_of_each_type_is_unique() {
        let mut seen = Vec::new();
        for kind in EnemyKind::ALL {
            let byte = enemy(kind, 1).kind_byte();
            assert!(!seen.contains(&byte), "byte repetido para {kind:?}");
            seen.push(byte);
        }
    }
}
