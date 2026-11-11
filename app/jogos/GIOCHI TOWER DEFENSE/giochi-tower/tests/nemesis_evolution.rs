//! Evolução do nemesis (GDD §10), pela API pública.
//!
//! O que estes testes fixam: o Coronel sobe um tier por onda sobrevivida, para no
//! 5, ganha redução de dano no tier 3, fica **mais forte que a Queen** no tier 5, e
//! a recompensa por matar é `tier × 200` mais o item que não existe na loja.

use glam::Vec3;
use giochi_tower::config::{BossKind, EnemyKind, GameConfig};
use giochi_tower::game::boss::Boss;
use giochi_tower::game::enemy::Enemy;
use giochi_tower::game::nemesis::Nemesis;
use std::path::Path;

fn config() -> GameConfig {
    GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config válida")
}

#[test]
fn the_colonel_starts_at_tier_zero_with_the_gdd_numbers() {
    let c = config();
    let n = Nemesis::spawn(&c, Vec3::ZERO);
    assert_eq!(n.tier, 0);
    assert_eq!(n.max_hp, 300.0);
    assert_eq!(n.damage, 20.0);
    assert!(n.is_alive());
}

#[test]
fn each_survived_wave_raises_the_tier_up_to_five() {
    let c = config();
    let mut n = Nemesis::spawn(&c, Vec3::ZERO);

    n.on_wave_survived(&c); // fim da onda 1
    assert_eq!(n.tier, 1);
    assert_eq!(n.gains(&c).len(), 1, "ganhou o dash");

    n.on_wave_survived(&c); // fim da onda 2
    assert_eq!(n.tier, 2);
    assert!(n.gains(&c).iter().any(|g| g.contains("adds")));

    n.on_wave_survived(&c); // fim da onda 3
    assert_eq!(n.tier, 3, "30% de redução de dano");

    n.on_wave_survived(&c); // fim da onda 4
    n.on_wave_survived(&c); // fim da onda 5
    assert_eq!(n.tier, 5);
    assert_eq!(n.max_hp, 5000.0);
    assert_eq!(n.damage, 80.0);

    assert!(!n.on_wave_survived(&c), "não passa do tier 5");
}

#[test]
fn tier_five_is_stronger_than_the_queen_but_still_killable() {
    let c = config();
    let mut n = Nemesis::spawn(&c, Vec3::ZERO);
    for _ in 0..5 {
        n.on_wave_survived(&c);
    }
    let queen = c.boss(BossKind::Queen).expect("rainha");
    assert!(
        n.max_hp > queen.hp,
        "GDD §10: o Coronel no tier 5 é mais forte que a Queen"
    );

    // Matável: com a redução de 30%, ainda morre com dano suficiente.
    let mut total = 0.0;
    while n.is_alive() && total < 20_000.0 {
        n.take_damage(100.0, &c);
        total += 100.0;
    }
    assert!(!n.is_alive(), "e matável");
}

#[test]
fn killing_pays_by_tier_plus_the_exclusive_item() {
    let c = config();
    let mut n = Nemesis::spawn(&c, Vec3::ZERO);
    for _ in 0..4 {
        n.on_wave_survived(&c);
    }
    let (score, item) = n.reward(&c);
    assert_eq!(score, 4 * 200);
    let def = c.item(item).expect("item exclusivo existe no RON");
    assert!(def.exclusive);
    assert_eq!(def.price, 0, "item exclusivo não tem preço de loja");
}

#[test]
fn a_dead_colonel_does_not_come_back_and_does_not_evolve() {
    let c = config();
    let mut n = Nemesis::spawn(&c, Vec3::ZERO);
    assert!(n.take_damage(1.0e6, &c));
    assert!(!n.is_alive());
    assert!(!n.on_wave_survived(&c));
    assert!(n.step(1.0, None, Vec3::ZERO, &c).is_empty());
}

#[test]
fn the_bosses_and_the_colonel_are_different_threats() {
    let c = config();
    // Boss: associado a uma onda fixa do GDD §9.
    for (kind, wave) in [
        (BossKind::Tanker, 3u32),
        (BossKind::Plasma, 4),
        (BossKind::Brain, 5),
        (BossKind::Queen, 6),
    ] {
        assert_eq!(
            Boss::wave_of(kind, &c),
            Some(wave),
            "{kind:?} é da onda {wave}"
        );
    }
    // Coronel: sem onda de boss, e spawna na 1.
    assert_eq!(c.nemesis.spawn_wave, 1);
    // E os adds invocados são comuns, não bosses.
    assert!(c.enemy(EnemyKind::Swarm).is_some());
}
