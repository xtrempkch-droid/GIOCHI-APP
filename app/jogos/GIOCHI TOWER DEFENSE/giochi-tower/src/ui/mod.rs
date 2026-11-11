//! Interface: HUD, minimapa e tela da loja (GDD §17).
//!
//! # Por que modelos e não desenho
//!
//! A engine 0.3.1 **não tem texto nem 2D**: `RenderContext3D` só desenha malha
//! 3D com `Camera3D` (perspectiva) e não há upload de textura
//! (`DECISIONS.md`, D-009). Desenhar isto na tela exige a câmera falsa de 90°
//! sobre o 3D que o `giochi-defender` usa — decisão adiada de propósito, para o
//! HUD nascer com o dado certo em vez de nascer feio.
//!
//! O que existe aqui são **modelos de apresentação**: números já formatados e
//! posições já normalizadas. Quem desenhar (cubes em 2D, texto via fonte, ou uma
//! `capi` com texto no futuro) consome isto e não recalcula nada. Isso também
//! torna a UI testável sem GPU — o que, sem compilador nesta máquina, é a única
//! verificação possível.

/// Conteúdo do HUD superior e das barras.
#[derive(Debug, Clone, PartialEq)]
pub struct Hud {
    /// Linha da onda e do relógio (`Onda 3/6 — 04:12`).
    pub wave_line: String,
    /// HP do jogador local, em `0..=1`.
    pub player_hp_frac: f32,
    /// Texto do HP (`100/130`).
    pub player_hp_text: String,
    /// HP da torre, em `0..=1`.
    pub tower_hp_frac: f32,
    /// Texto da torre (`5000/8000`).
    pub tower_hp_text: String,
    /// Gold do jogador local.
    pub gold: u32,
    /// Pontuação.
    pub score: u32,
    /// Inimigos vivos (contagem).
    pub enemies: usize,
    /// Indicador de boss, se houver: nome e % de HP.
    pub boss_line: Option<String>,
    /// Indicador do Coronel: tier e se já foi avistado.
    pub nemesis_line: Option<String>,
    /// Fase legível (para o canto).
    pub phase: &'static str,
}

impl Hud {
    /// Todas as linhas de texto, na ordem de desenho.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            self.wave_line.clone(),
            format!("Torre {} ({:.0}%)", self.tower_hp_text, self.tower_hp_frac * 100.0),
            format!("HP {} · {} gold · {} pts", self.player_hp_text, self.gold, self.score),
            format!("{} · {} inimigo(s)", self.phase, self.enemies),
        ];
        if let Some(boss) = &self.boss_line {
            lines.push(boss.clone());
        }
        if let Some(nemesis) = &self.nemesis_line {
            lines.push(nemesis.clone());
        }
        lines
    }
}

/// Minimapa: posições normalizadas em `-1..=1` (o desenho mapeia para pixels).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Minimap {
    /// Torre (sempre o centro).
    pub tower: [f32; 2],
    /// Aliados vivos.
    pub allies: Vec<[f32; 2]>,
    /// Último avistamento do Coronel — só se ele já foi visto (GDD §10).
    pub nemesis: Option<[f32; 2]>,
}

/// Tela da loja.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopScreen {
    /// Aberta?
    pub open: bool,
    /// Nomes dos itens, com preço e marcação.
    pub lines: Vec<String>,
    /// Item sob o cursor.
    pub cursor: usize,
}

/// Um quadro inteiro de UI.
#[derive(Debug, Clone, PartialEq)]
pub struct UiFrame {
    /// HUD.
    pub hud: Hud,
    /// Minimapa.
    pub minimap: Minimap,
    /// Loja (fechada fora do intervalo).
    pub shop: ShopScreen,
}

/// Monta o quadro de UI a partir do estado.
pub fn build(state: &crate::game::state::GameState, config: &crate::config::GameConfig) -> UiFrame {
    let local = state.local();
    let boss_line = state.boss.as_ref().map(|boss| {
        format!("{} — {}% (fase {})", boss.kind.as_str(), boss.hp_pct_u8(), boss.phase + 1)
    });
    let nemesis_line = state.nemesis.as_ref().map(|nemesis| {
        format!(
            "O Coronel — tier {} ({} HP)",
            nemesis.tier,
            nemesis.max_hp as u32
        )
    });

    let hud = Hud {
        wave_line: state.hud_line(config),
        player_hp_frac: local.hp_pct(),
        player_hp_text: format!("{:.0}/{:.0}", local.hp, local.max_hp),
        tower_hp_frac: state.tower.hp_pct(),
        tower_hp_text: format!("{:.0}/{:.0}", state.tower.hp, state.tower.max_hp),
        gold: local.gold,
        score: local.score,
        enemies: state.enemies_alive(),
        boss_line,
        nemesis_line,
        phase: state.phase.as_str(),
    };

    let half = state.map.half_extent_m.max(1.0);
    let normalize = |position: glam::Vec3| [position.x / half, position.z / half];
    let minimap = Minimap {
        tower: [0.0, 0.0],
        allies: state
            .players
            .iter()
            .filter(|p| p.can_act())
            .map(|p| normalize(p.position))
            .collect(),
        // `broadcast_pos` é o último avistamento defasado; `Vec3::ZERO`
        // significa "ainda não visto" e não vira ponto no mapa.
        nemesis: state
            .nemesis
            .as_ref()
            .map(|n| n.broadcast_pos())
            .filter(|pos| *pos != glam::Vec3::ZERO)
            .map(normalize),
    };

    let shop = ShopScreen {
        open: state.phase == crate::game::state::Phase::Shop,
        lines: state
            .shop
            .menu(local.gold, config)
            .iter()
            .map(|entry| {
                let mark = if entry.exhausted {
                    "esgotado"
                } else if entry.affordable {
                    "ok"
                } else {
                    "sem gold"
                };
                format!("{:<24} {:>4} gold  [{}]", entry.label, entry.price, mark)
            })
            .collect(),
        cursor: state.shop_cursor,
    };

    UiFrame {
        hud,
        minimap,
        shop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ClassId, GameConfig, ProfileId};
    use crate::game::state::{GameState, Phase};
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    #[test]
    fn the_hud_carries_the_numbers_the_gdd_asks_for() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        state.phase_timer_s = 122.0;
        state.wave = 3;
        state.local_mut().gold = 45;
        state.local_mut().score = 70;

        let frame = build(&state, &c);
        assert_eq!(frame.hud.wave_line, "Onda 3/6 — 03:58 · torre 5000/5000 · 0 inimigo(s) · 45 gold");
        assert_eq!(frame.hud.gold, 45);
        assert_eq!(frame.hud.score, 70);
        assert!((frame.hud.tower_hp_frac - 1.0).abs() < 1.0e-6);
        assert_eq!(frame.hud.player_hp_text, "100/100");
        assert!(!frame.hud.lines().is_empty());
    }

    #[test]
    fn the_minimap_normalises_positions_to_the_map() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Sniper, ProfileId::Balanced);
        state.local_mut().position = glam::Vec3::new(40.0, 0.0, -40.0);
        let frame = build(&state, &c);
        assert_eq!(frame.minimap.tower, [0.0, 0.0]);
        assert_eq!(frame.minimap.allies.len(), 1);
        // 40 m num mapa de meia-extensão 80 = 0,5.
        assert!((frame.minimap.allies[0][0] - 0.5).abs() < 1.0e-5);
        assert!((frame.minimap.allies[0][1] + 0.5).abs() < 1.0e-5);
        assert!(frame.minimap.nemesis.is_none(), "ainda não foi avistado");
    }

    #[test]
    fn a_dead_ally_leaves_the_minimap() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Medic, ProfileId::Balanced);
        state.local_mut().die(&c);
        let frame = build(&state, &c);
        assert!(frame.minimap.allies.is_empty());
    }

    #[test]
    fn the_shop_is_closed_outside_the_interval_and_lists_nine_items_inside() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        assert!(!build(&state, &c).shop.open);
        assert_eq!(build(&state, &c).shop.lines.len(), 9, "a loja lista os 9 itens");

        state.phase = Phase::Shop;
        state.local_mut().gold = 100;
        let frame = build(&state, &c);
        assert!(frame.shop.open);
        assert!(frame.shop.lines.iter().any(|l| l.contains("Rifle melhorado")));
        assert!(frame.shop.lines.iter().any(|l| l.contains("sem gold")));
        assert!(frame.shop.lines.iter().any(|l| l.contains("ok")));
    }

    #[test]
    fn a_boss_and_the_colonel_appear_only_when_they_are_in_the_world() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        assert!(build(&state, &c).hud.boss_line.is_none());

        state.boss = Some(crate::game::boss::Boss::spawn(
            c.boss(crate::config::BossKind::Queen).expect("rainha"),
            glam::Vec3::ZERO,
        ));
        let frame = build(&state, &c);
        let line = frame.hud.boss_line.expect("indicador de boss");
        assert!(line.contains("Queen"), "{line}");
        assert!(line.contains("100%"), "{line}");
    }
}
