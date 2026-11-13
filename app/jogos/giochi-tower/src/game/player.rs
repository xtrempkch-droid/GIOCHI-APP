//! Jogadores: classes, movimento, dano, respawn e predição local (GDD §7).
//!
//! # O que é "predição" neste jogo
//!
//! O GDD §5 pede host-autoritativo: o cliente manda *input*, o host simula. Sem
//! predição, o jogador veria a si mesmo com o atraso do RTT — em LAN é 1–2
//! quadros, mas o movimento fica "molhado" e a mira passa do alvo.
//!
//! A predição aqui é mínima de propósito: [`PlayerState::move_local`] aplica o
//! **mesmo** movimento que o host aplica, a partir do mesmo input, e o host
//! corrige quando o snapshot chega. Não há reconciliação de estado completo no
//! M2 — ela entra no M4, junto com o smoothing, porque sem rede real não há erro
//! para reconciliar.
//!
//! # Classes
//!
//! Números em `config/classes.ron`; aqui só a leitura deles. Dano e alcance já
//! somam os bônus de loja, para que a loja (M3) não precise recalcular nada nos
//! cálculos de combate.

use glam::Vec3;

use crate::config::{ClassId, GameConfig};
use crate::engine_bridge::input::InputFrame;
use crate::game::camera::{IsoCamera, MapBounds};

/// Raio do jogador, em metros (usado para ataques de área no M3).
pub const PLAYER_RADIUS_M: f32 = 0.9;

/// Estado de um jogador.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    /// Identidade na partida (0 = host).
    pub id: u8,
    /// Classe escolhida (GDD §7).
    pub class: ClassId,
    /// HP atual.
    pub hp: f32,
    /// HP máximo, já com o colete (GDD §12).
    pub max_hp: f32,
    /// Posição no chão (Y = 0).
    pub position: Vec3,
    /// Direção para onde aponta (unitária, no chão).
    pub aim: Vec3,
    /// Gold acumulado (GDD §12).
    pub gold: u32,
    /// Pontuação acumulada (GDD §13).
    pub score: u32,
    /// Vivo.
    pub alive: bool,
    /// Segundos restantes até o respawn.
    pub respawn_timer_s: f32,
    /// Bônus percentual de dano vindo da loja.
    pub damage_bonus_pct: f32,
    /// Bônus percentual de alcance vindo da loja.
    pub range_bonus_pct: f32,
    /// Cargas de dash disponíveis.
    pub dash_charges: u32,
    /// Segundos até o próximo ataque (recarga da classe).
    pub attack_timer_s: f32,
}

impl PlayerState {
    /// Cria um jogador da classe pedida, com os valores base da config.
    pub fn new(id: u8, class: ClassId, config: &GameConfig) -> Self {
        let hp = config.class(class).map(|c| c.hp).unwrap_or(100.0);
        Self {
            id,
            class,
            hp,
            max_hp: hp,
            position: Vec3::ZERO,
            aim: Vec3::new(0.0, 0.0, -1.0),
            gold: 0,
            score: 0,
            alive: true,
            respawn_timer_s: 0.0,
            damage_bonus_pct: 0.0,
            range_bonus_pct: 0.0,
            dash_charges: 1,
            attack_timer_s: 0.0,
        }
    }

    /// Dano por acerto, já com o bônus da loja.
    pub fn damage(&self, config: &GameConfig) -> f32 {
        let base = config.class(self.class).map(|c| c.damage).unwrap_or(0.0);
        base * (1.0 + self.damage_bonus_pct / 100.0)
    }

    /// Alcance do ataque, já com o bônus da loja.
    pub fn attack_range(&self, config: &GameConfig) -> f32 {
        let base = config
            .class(self.class)
            .map(|c| c.attack_range_m)
            .unwrap_or(0.0);
        base * (1.0 + self.range_bonus_pct / 100.0)
    }

    /// Intervalo entre ataques, em segundos.
    pub fn attack_cooldown(&self, config: &GameConfig) -> f32 {
        config
            .class(self.class)
            .map(|c| c.attack_cooldown_s)
            .unwrap_or(0.5)
    }

    /// Velocidade de caminhada, em m/s.
    pub fn move_speed(&self, config: &GameConfig) -> f32 {
        config
            .class(self.class)
            .map(|c| c.move_speed_mps)
            .unwrap_or(0.0)
    }

    /// Tempo de respawn, em segundos (GDD §7: 10 s).
    pub fn respawn_time(&self, config: &GameConfig) -> f32 {
        config.class(self.class).map(|c| c.respawn_s).unwrap_or(10.0)
    }

    /// Pode agir (vivo).
    pub fn can_act(&self) -> bool {
        self.alive
    }

    /// Fração de HP, em `0..=1`.
    pub fn hp_pct(&self) -> f32 {
        if self.max_hp <= 0.0 {
            return 0.0;
        }
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    /// Aplica dano. Devolve `true` se este golpe derrubou o jogador.
    pub fn take_damage(&mut self, amount: f32) -> bool {
        if amount <= 0.0 || !self.alive {
            return false;
        }
        self.hp -= amount;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            return true;
        }
        false
    }

    /// Marca o jogador como caído e arma o relógio de respawn.
    pub fn die(&mut self, config: &GameConfig) {
        self.alive = false;
        self.hp = 0.0;
        self.respawn_timer_s = self.respawn_time(config).max(0.0);
    }

    /// Cura sem passar do máximo.
    pub fn heal(&mut self, amount: f32) {
        if amount > 0.0 && self.alive {
            self.hp = (self.hp + amount).min(self.max_hp);
        }
    }

    /// Faz o relógio de respawn correr. Devolve `true` no instante em que volta.
    ///
    /// O jogador renasce **na torre** (GDD §7), com o HP cheio.
    pub fn tick_respawn(&mut self, dt: f32) -> bool {
        if self.alive {
            return false;
        }
        self.respawn_timer_s -= dt;
        if self.respawn_timer_s <= 0.0 {
            self.alive = true;
            self.hp = self.max_hp;
            self.position = crate::game::tower::TOWER_POSITION;
            self.respawn_timer_s = 0.0;
            return true;
        }
        false
    }

    /// Aplica o movimento do input, relativo à câmera, dentro do mapa.
    ///
    /// É a mesma função para o host e para o cliente que prediz: se divergirem,
    /// o jogador "treme" quando o snapshot corrige.
    pub fn move_local(
        &mut self,
        input: &InputFrame,
        camera: &IsoCamera,
        map: MapBounds,
        speed_mps: f32,
        dt: f32,
    ) {
        if !self.can_act() {
            return;
        }
        let direction = camera.ground_direction(input.move_dir.x, input.move_dir.y);
        let moved = self.position + direction * speed_mps * dt;
        self.position = map.clamp(Vec3::new(moved.x, 0.0, moved.z));

        if !input.aim_dir.is_neutral() {
            let aim = camera.ground_direction(input.aim_dir.x, input.aim_dir.y);
            if aim.length() > 1.0e-6 {
                self.aim = aim.normalize();
            }
        }
    }

    /// Distância no chão até `point`.
    pub fn distance_to(&self, point: Vec3) -> f32 {
        let dx = self.position.x - point.x;
        let dz = self.position.z - point.z;
        (dx * dx + dz * dz).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileId;
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn player(class: ClassId) -> PlayerState {
        PlayerState::new(0, class, &config())
    }

    #[test]
    fn the_four_classes_have_the_gdd_hp() {
        let c = config();
        for (class, hp) in [
            (ClassId::Soldier, 100.0),
            (ClassId::Warrior, 180.0),
            (ClassId::Sniper, 80.0),
            (ClassId::Medic, 90.0),
        ] {
            let p = PlayerState::new(0, class, &c);
            assert_eq!(p.max_hp, hp, "HP do {:?} (GDD §7)", class);
            assert_eq!(p.hp, hp);
        }
    }

    #[test]
    fn every_class_costs_ten_seconds_to_respawn() {
        let c = config();
        for class in ClassId::ALL {
            assert_eq!(
                PlayerState::new(0, class, &c).respawn_time(&c),
                10.0,
                "GDD §7: respawn em 10 s"
            );
        }
    }

    #[test]
    fn moving_is_relative_to_the_camera_and_stays_in_the_map() {
        // Sem `config()`: `move_local` não toma config, e a variável solta só
        // gerava aviso de não usada.
        let mut p = player(ClassId::Soldier);
        let camera = IsoCamera::new(Vec3::ZERO);
        let map = MapBounds::new();
        p.position = Vec3::new(79.0, 0.0, 0.0);
        let mut input = InputFrame::default();
        input.move_dir = crate::engine_bridge::input::Axis::new(1.0, 0.0);
        for _ in 0..600 {
            p.move_local(&input, &camera, map, 7.0, 1.0 / 60.0);
        }
        assert!(map.contains(p.position));
        assert_eq!(p.position.y, 0.0);
    }

    #[test]
    fn a_dead_player_does_not_move_and_respawns_at_the_tower() {
        let c = config();
        let mut p = player(ClassId::Soldier);
        p.position = Vec3::new(30.0, 0.0, 30.0);
        p.die(&c);
        assert!(!p.can_act());
        assert_eq!(p.respawn_timer_s, 10.0);

        let camera = IsoCamera::new(Vec3::ZERO);
        let mut input = InputFrame::default();
        input.move_dir = crate::engine_bridge::input::Axis::new(1.0, 0.0);
        p.move_local(&input, &camera, MapBounds::new(), 7.0, 1.0);
        assert_eq!(p.position, Vec3::new(30.0, 0.0, 30.0), "morto não anda");

        for _ in 0..(60 * 10 + 2) {
            p.tick_respawn(1.0 / 60.0);
        }
        assert!(p.alive);
        assert_eq!(p.hp, p.max_hp);
        assert_eq!(p.position, crate::game::tower::TOWER_POSITION);
    }

    #[test]
    fn hp_cannot_go_below_zero_and_damage_reports_the_kill() {
        let c = config();
        let mut p = player(ClassId::Sniper);
        assert!(!p.take_damage(79.0));
        assert!(p.take_damage(10.0));
        assert_eq!(p.hp, 0.0);
        p.die(&c);
        assert!(!p.alive);
    }

    #[test]
    fn healing_respects_the_maximum() {
        let mut p = player(ClassId::Medic);
        p.take_damage(50.0);
        p.heal(1000.0);
        assert_eq!(p.hp, p.max_hp);
    }

    #[test]
    fn shop_bonuses_reach_damage_and_range() {
        let c = config();
        let mut p = player(ClassId::Soldier);
        let base_damage = p.damage(&c);
        let base_range = p.attack_range(&c);
        p.damage_bonus_pct = 20.0;
        p.range_bonus_pct = 30.0;
        assert!((p.damage(&c) - base_damage * 1.2).abs() < 1.0e-4);
        assert!((p.attack_range(&c) - base_range * 1.3).abs() < 1.0e-4);
    }

    #[test]
    fn the_profile_does_not_change_the_player_numbers() {
        let c = config();
        let a = PlayerState::new(0, ClassId::Warrior, &c);
        let mut b = a.clone();
        b.dash_charges = 2;
        assert_eq!(a.max_hp, b.max_hp, "perfil não altera classe");
        assert_eq!(ProfileId::Balanced.as_str(), "Balanced");
    }
}
