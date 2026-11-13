//! Escala por onda (GDD §8), do lado de fora do crate.
//!
//! A conta também é testada em `src/config.rs`; aqui ela é conferida sobre o
//! **inimigo real** criado pelo spawner, que é o número que o jogador enfrenta.
//! Ter as duas pontas cobertas é o que impede a divergência silenciosa entre a
//! fórmula e o construtor.

use giochi_tower::config::{EnemyKind, GameConfig};
use giochi_tower::game::enemy::Enemy;
use glam::Vec3;
use std::path::Path;

fn config() -> GameConfig {
    GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config válida")
}

#[test]
fn hp_damage_and_speed_follow_the_gdd_formula() {
    let c = config();
    let def = c.enemy(EnemyKind::Grunt).expect("grunt");
    for wave in 1..=6u32 {
        let scaling = c.scaling_for_wave(wave);
        let enemy = Enemy::spawn(1, def, Vec3::ZERO, scaling);
        let n = wave as f32;
        assert!(
            (enemy.max_hp - 30.0 * (1.0 + 0.15 * n)).abs() < 1.0e-3,
            "onda {wave}: HP {} != {}",
            enemy.max_hp,
            30.0 * (1.0 + 0.15 * n)
        );
        assert!((enemy.damage - 5.0 * (1.0 + 0.10 * n)).abs() < 1.0e-3);
        assert!(
            (enemy.speed_mps - def.speed_mps * (1.0 + 0.05 * n)).abs() < 1.0e-3
        );
    }
}

#[test]
fn every_wave_is_harder_than_the_previous_one() {
    let c = config();
    let def = c.enemy(EnemyKind::Tank).expect("tank");
    let mut previous = 0.0;
    for wave in 1..=c.campaign.wave_count {
        let enemy = Enemy::spawn(1, def, Vec3::ZERO, c.scaling_for_wave(wave));
        assert!(
            enemy.max_hp > previous,
            "onda {wave} não é mais dura que {}",
            wave - 1
        );
        previous = enemy.max_hp;
    }
}

#[test]
fn the_endless_scaling_continues_past_the_campaign() {
    let c = config();
    let last = c.scaling_for_endless_wave(c.campaign.wave_count);
    let next = c.scaling_for_endless_wave(c.campaign.wave_count + 1);
    assert!(next.hp > last.hp, "onda 7 tem de ser mais dura que a 6");
    assert!(next.damage > last.damage);
}

#[test]
fn the_five_types_keep_their_relative_order_after_scaling() {
    let c = config();
    let wave = 6;
    let scaling = c.scaling_for_wave(wave);
    let hp = |kind| {
        Enemy::spawn(1, c.enemy(kind).unwrap(), Vec3::ZERO, scaling).max_hp
    };
    assert!(hp(EnemyKind::Swarm) < hp(EnemyKind::Grunt));
    assert!(hp(EnemyKind::Grunt) < hp(EnemyKind::Flying));
    assert!(hp(EnemyKind::Flying) < hp(EnemyKind::Ranged));
    assert!(hp(EnemyKind::Ranged) < hp(EnemyKind::Tank));
}
