//! Validação de compra no host (GDD §12), pela API pública.
//!
//! A regra do prompt é "validação de compra sempre no host": o cliente manda o
//! *pedido*, nunca o preço. Estes testes percorrem exatamente o que o host tem de
//! recusar — loja fechada, gold curto, item exclusivo, estoque esgotado — e o que
//! ele tem de aceitar, com o efeito aplicado.

use giochi_tower::config::{ClassId, GameConfig, ItemId};
use giochi_tower::game::player::PlayerState;
use giochi_tower::game::shop::{Shop, ShopError};
use giochi_tower::game::tower::Tower;
use std::path::Path;

fn config() -> GameConfig {
    GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config válida")
}

fn open_shop(c: &GameConfig) -> (Shop, PlayerState, Tower) {
    let mut shop = Shop::new();
    shop.open();
    (
        shop,
        PlayerState::new(0, ClassId::Soldier, c),
        Tower::new(&c.tower),
    )
}

#[test]
fn the_host_rejects_a_purchase_the_client_should_not_make() {
    let c = config();
    let (mut shop, mut player, mut tower) = open_shop(&c);

    // Sem gold.
    player.gold = 0;
    assert!(matches!(
        shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c),
        Err(ShopError::NotEnoughGold { .. })
    ));

    // Loja fechada.
    player.gold = 10_000;
    shop.close();
    assert_eq!(
        shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c),
        Err(ShopError::Closed)
    );
    shop.open();

    // Item exclusivo do nemesis.
    assert_eq!(
        shop.buy(c.nemesis.exclusive_item, &mut player, &mut tower, &c),
        Err(ShopError::NotForSale)
    );
    assert_eq!(player.gold, 10_000, "nenhuma dessas recusas debita gold");
}

#[test]
fn each_accepted_purchase_applies_exactly_its_effect() {
    let c = config();
    let (mut shop, mut player, mut tower) = open_shop(&c);
    player.gold = 100_000;

    shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c).unwrap();
    assert_eq!(player.damage_bonus_pct, 20.0);

    shop.buy(ItemId::Scope, &mut player, &mut tower, &c).unwrap();
    assert_eq!(player.range_bonus_pct, 30.0);

    shop.buy(ItemId::Vest, &mut player, &mut tower, &c).unwrap();
    assert_eq!(player.max_hp, 130.0);

    shop.buy(ItemId::ExtraDash, &mut player, &mut tower, &c).unwrap();
    assert_eq!(player.dash_charges, 2);

    shop.buy(ItemId::TowerHpUpgrade, &mut player, &mut tower, &c).unwrap();
    assert_eq!(tower.max_hp, 5500.0);

    shop.buy(ItemId::TowerArmorUpgrade, &mut player, &mut tower, &c).unwrap();
    assert_eq!(tower.armor, 7.0);
}

#[test]
fn the_host_stops_at_the_stack_limit_and_at_the_tower_ceiling() {
    let c = config();
    let (mut shop, mut player, mut tower) = open_shop(&c);
    player.gold = 100_000;

    // Teto de HP da torre (GDD §11): 5000 → 8000.
    for _ in 0..20 {
        let _ = shop.buy(ItemId::TowerHpUpgrade, &mut player, &mut tower, &c);
    }
    assert_eq!(tower.max_hp, 8000.0, "o teto de loja vale");

    // Limite de estoque do rifle.
    let limit = c.item(ItemId::RifleUpgrade).unwrap().stack_limit;
    for _ in 0..limit {
        shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c).unwrap();
    }
    assert_eq!(
        shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c),
        Err(ShopError::OutOfStock { limit })
    );
}

#[test]
fn the_score_config_matches_the_gdd_table() {
    let c = config();
    // A tabela do GDD §13 é a fonte da pontuação; se ela mudar, o teste avisa.
    assert_eq!(c.score.kill_common, 10);
    assert_eq!(c.score.kill_boss, 500);
    assert_eq!(c.score.kill_nemesis_per_tier, 200);
    assert_eq!(c.score.wave_clear_per_wave, 100);
    assert_eq!(c.score.tower_hp_per_point, 10);
}
