//! Loja (GDD §12) — e a validação que o host faz.
//!
//! # Toda compra é validada aqui
//!
//! O cliente manda `ShopBuy { player_id, item_id }`; nada do que ele afirma sobre
//! preço ou gold é usado. `Shop::buy` confere, nesta ordem: loja aberta → item
//! existe e é vendido → gold suficiente → limite de estoque. Só então aplica o
//! efeito, e devolve o preço pago.
//!
//! # `stack_limit`
//!
//! `0` significa **sem limite** (consumíveis e reparo de torre); qualquer outro
//! número é o máximo de compras daquele item na run. O índice de compras é por
//! item, e o limite é checado antes de debitar gold — a ordem importa: debitar e
//! depois descobrir que estourou o limite seria um bug que só apareceria na loja.

use thiserror::Error;

use crate::config::{GameConfig, ItemDef, ItemEffect, ItemId};
use crate::game::player::PlayerState;
use crate::game::tower::Tower;

/// Por que uma compra foi recusada.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShopError {
    /// A loja não está no intervalo entre ondas.
    #[error("a loja só abre no intervalo entre ondas")]
    Closed,
    /// O item não existe no RON.
    #[error("item desconhecido")]
    UnknownItem,
    /// O item existe mas não é vendido (exclusivo do nemesis).
    #[error("este item não é vendido na loja")]
    NotForSale,
    /// Gold insuficiente.
    #[error("gold insuficiente: precisa {price}, tem {gold}")]
    NotEnoughGold {
        /// Preço do item.
        price: u32,
        /// Gold do jogador.
        gold: u32,
    },
    /// Limite de compras do item atingido.
    #[error("limite de compras deste item atingido ({limit})")]
    OutOfStock {
        /// Limite configurado.
        limit: u32,
    },
}

/// Uma linha da loja, para a interface.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopEntry {
    /// Item.
    pub id: ItemId,
    /// Nome exibido.
    pub label: String,
    /// Preço.
    pub price: u32,
    /// O jogador pode pagar agora.
    pub affordable: bool,
    /// Já esgotou o limite.
    pub exhausted: bool,
}

/// Estado da loja.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shop {
    /// Aberta (só no intervalo, GDD §4).
    pub open: bool,
    /// Quantas vezes cada item já foi comprado, na ordem de `items.ron`.
    purchases: Vec<u32>,
}

impl Shop {
    /// Loja fechada.
    pub fn new() -> Self {
        Self::default()
    }

    /// Abre (entrada no intervalo).
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Fecha (saída do intervalo).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Quantas compras de `id` já houve.
    pub fn purchases_of(&self, id: ItemId, config: &GameConfig) -> u32 {
        config
            .items
            .items
            .iter()
            .position(|item| item.id == id)
            .and_then(|index| self.purchases.get(index).copied())
            .unwrap_or(0)
    }

    /// Confere se a compra é possível, **sem** aplicar nada.
    pub fn can_buy<'a>(
        &self,
        id: ItemId,
        gold: u32,
        config: &'a GameConfig,
    ) -> Result<&'a ItemDef, ShopError> {
        if !self.open {
            return Err(ShopError::Closed);
        }
        let def = config.item(id).ok_or(ShopError::UnknownItem)?;
        if def.exclusive || def.price == 0 {
            return Err(ShopError::NotForSale);
        }
        if def.stack_limit > 0 && self.purchases_of(id, config) >= def.stack_limit {
            return Err(ShopError::OutOfStock {
                limit: def.stack_limit,
            });
        }
        if gold < def.price {
            return Err(ShopError::NotEnoughGold {
                price: def.price,
                gold,
            });
        }
        Ok(def)
    }

    /// Compra: valida, debita e aplica. Devolve o preço pago.
    ///
    /// O efeito é aplicado **depois** de o gold ser debitado, com a mesma definição
    /// já validada — não há como "comprar" sem pagar.
    pub fn buy(
        &mut self,
        id: ItemId,
        player: &mut PlayerState,
        tower: &mut Tower,
        config: &GameConfig,
    ) -> Result<u32, ShopError> {
        let def = self.can_buy(id, player.gold, config)?;
        let price = def.price;
        let effect = def.effect;
        let magnitude = def.magnitude;
        let index = config
            .items
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or(ShopError::UnknownItem)?;

        player.gold -= price;
        if self.purchases.len() <= index {
            self.purchases.resize(index + 1, 0);
        }
        self.purchases[index] = self.purchases[index].saturating_add(1);

        apply_effect(effect, magnitude, player, tower, config);
        log::info!(
            "loja: jogador {} comprou '{}' por {price} gold (restam {})",
            player.id,
            def.label,
            player.gold
        );
        Ok(price)
    }

    /// Linhas da loja com o que o jogador pode pagar agora.
    pub fn menu(&self, gold: u32, config: &GameConfig) -> Vec<ShopEntry> {
        ItemId::SHOP
            .iter()
            .filter_map(|id| config.item(*id))
            .map(|def| ShopEntry {
                id: def.id,
                label: def.label.clone(),
                price: def.price,
                affordable: gold >= def.price,
                exhausted: def.stack_limit > 0
                    && self.purchases_of(def.id, config) >= def.stack_limit,
            })
            .collect()
    }
}

/// Aplica o efeito de um item.
fn apply_effect(
    effect: ItemEffect,
    magnitude: f32,
    player: &mut PlayerState,
    tower: &mut Tower,
    config: &GameConfig,
) {
    match effect {
        ItemEffect::PlayerDamagePct => player.damage_bonus_pct += magnitude,
        ItemEffect::PlayerRangePct => player.range_bonus_pct += magnitude,
        ItemEffect::PlayerMaxHpFlat => {
            player.max_hp += magnitude;
            player.hp += magnitude;
        }
        ItemEffect::PlayerHealFlat => player.heal(magnitude),
        ItemEffect::PlayerDashCharge => {
            player.dash_charges = player.dash_charges.saturating_add(magnitude.max(0.0) as u32)
        }
        ItemEffect::TowerHealFlat => tower.heal(magnitude),
        ItemEffect::TowerMaxHpFlat => tower.upgrade_max_hp(magnitude, &config.tower),
        ItemEffect::TowerArmorFlat => tower.upgrade_armor(magnitude, &config.tower),
        ItemEffect::TowerRegenPerSecond => tower.upgrade_regen(magnitude, &config.tower),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClassId;
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn fixture(config: &GameConfig) -> (Shop, PlayerState, Tower) {
        let mut shop = Shop::new();
        shop.open();
        let player = PlayerState::new(0, ClassId::Soldier, config);
        let tower = Tower::new(&config.tower);
        (shop, player, tower)
    }

    #[test]
    fn a_closed_shop_refuses_everything() {
        let c = config();
        let mut shop = Shop::new();
        let mut player = PlayerState::new(0, ClassId::Soldier, &c);
        player.gold = 10_000;
        let mut tower = Tower::new(&c.tower);
        assert_eq!(
            shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c),
            Err(ShopError::Closed)
        );
        assert_eq!(player.gold, 10_000, "nada foi debitado");
    }

    #[test]
    fn without_gold_nothing_is_bought() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 79; // rifle custa 80
        let error = shop
            .buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c)
            .expect_err("sem gold");
        assert_eq!(error, ShopError::NotEnoughGold { price: 80, gold: 79 });
        assert_eq!(player.gold, 79);
        assert_eq!(player.damage_bonus_pct, 0.0);
    }

    #[test]
    fn a_purchase_debits_and_applies_the_effect() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 100;
        let paid = shop
            .buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c)
            .expect("compra");
        assert_eq!(paid, 80);
        assert_eq!(player.gold, 20);
        assert_eq!(player.damage_bonus_pct, 20.0, "+20% de dano");

        player.gold = 60;
        shop.buy(ItemId::Scope, &mut player, &mut tower, &c)
            .expect("scope");
        assert_eq!(player.range_bonus_pct, 30.0);
    }

    #[test]
    fn the_stack_limit_stops_the_fourth_rifle() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        let limit = c.item(ItemId::RifleUpgrade).map(|i| i.stack_limit).unwrap_or(0);
        assert_eq!(limit, 3, "rifle tem limite 3 no RON");
        player.gold = 10_000;
        for _ in 0..limit {
            shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c)
                .expect("dentro do limite");
        }
        assert_eq!(
            shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c),
            Err(ShopError::OutOfStock { limit })
        );
    }

    #[test]
    fn consumables_have_no_limit() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 1000;
        for _ in 0..11 {
            shop.buy(ItemId::HealingPotion, &mut player, &mut tower, &c)
                .expect("poção é ilimitada");
        }
        assert_eq!(shop.purchases_of(ItemId::HealingPotion, &c), 11);
    }

    #[test]
    fn the_exclusive_item_is_not_for_sale() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 10_000;
        assert_eq!(
            shop.buy(
                c.nemesis.exclusive_item,
                &mut player,
                &mut tower,
                &c
            ),
            Err(ShopError::NotForSale)
        );
    }

    #[test]
    fn the_tower_repair_is_capped_by_the_maximum() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 10_000;
        tower.hp = 100.0;
        shop.buy(ItemId::TowerRepair, &mut player, &mut tower, &c)
            .expect("reparo");
        assert_eq!(tower.hp, 600.0, "+500 HP");
        tower.hp = tower.max_hp - 100.0;
        shop.buy(ItemId::TowerRepair, &mut player, &mut tower, &c)
            .expect("reparo");
        assert_eq!(tower.hp, tower.max_hp, "não passa do máximo");
    }

    #[test]
    fn the_menu_marks_what_is_affordable_and_exhausted() {
        let c = config();
        let (mut shop, mut player, mut tower) = fixture(&c);
        player.gold = 100;
        let menu = shop.menu(player.gold, &c);
        assert_eq!(menu.len(), 9, "GDD §12: nove itens de loja");
        let rifle = menu.iter().find(|e| e.id == ItemId::RifleUpgrade).unwrap();
        assert!(rifle.affordable, "80 com 100 de gold");
        let regen = menu.iter().find(|e| e.id == ItemId::TowerRegen).unwrap();
        assert!(!regen.affordable, "200 com 100 de gold");

        player.gold = 10_000;
        for _ in 0..3 {
            shop.buy(ItemId::RifleUpgrade, &mut player, &mut tower, &c)
                .expect("compra");
        }
        let menu = shop.menu(player.gold, &c);
        let rifle = menu.iter().find(|e| e.id == ItemId::RifleUpgrade).unwrap();
        assert!(rifle.exhausted);
    }
}
