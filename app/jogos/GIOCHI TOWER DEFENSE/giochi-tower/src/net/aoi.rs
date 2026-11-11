//! Interest Management: quem vê qual inimigo (GDD §6 e §15).
//!
//! O raio vem da câmera do jogador ([`IsoCamera::aoi_radius`]), que já é
//! "~1,5 × meia-diagonal da tela" traduzido para o frustum persistente. O filtro
//! em si é o do `giochi-net` ([`AreaDeInteresse::contem`]): usar o mesmo tipo que
//! o `Host::definir_area` recebe garante que o que o cliente vê é exatamente o
//! que o crate entregaria.
//!
//! # Onde o filtro roda
//!
//! Duas vezes, de propósito:
//!
//! 1. **no host**, ao montar o snapshot de cada cliente (é o que economiza banda);
//! 2. **no cliente**, ao aplicar um filtro extra ([`filter`]) — barato, e protege
//!    contra host que mande demais (versão antiga, configuração errada).
//!
//! Filtrar de novo no cliente não substitui o do host: o custo de banda já foi
//! pago. É defesa em profundidade, não a economia.

use glam::Vec3;
use giochi_net::aoi::AreaDeInteresse;
use giochi_net::wire::SnapshotInimigo;

use crate::game::camera::IsoCamera;
use crate::game::enemy::Enemy;

/// Área de interesse circular centrada num ponto do chão.
pub fn area_at(center: Vec3, radius_m: f32) -> AreaDeInteresse {
    AreaDeInteresse::nova(center.x, center.z, radius_m.max(0.0))
}

/// Área de interesse de uma câmera, com o multiplicador do perfil de host.
pub fn area_from_camera(camera: &IsoCamera, aspect: f32, aoi_scale: f32) -> AreaDeInteresse {
    area_at(camera.focus, camera.aoi_radius(aspect, aoi_scale))
}

/// Mantém apenas os inimigos dentro da área.
pub fn filter(enemies: &[SnapshotInimigo], area: &AreaDeInteresse) -> Vec<SnapshotInimigo> {
    enemies
        .iter()
        .copied()
        .filter(|enemy| area.contem(enemy))
        .collect()
}

/// Monta o snapshot que o host publica a partir dos inimigos vivos.
///
/// `tipo` e `fase_chefe` são bytes porque o `giochi-net` não conhece o
/// vocabulário do jogo — é o contrato dele (“a engine transporta, o jogo
/// interpreta”).
pub fn from_enemies(enemies: &[Enemy]) -> Vec<SnapshotInimigo> {
    enemies
        .iter()
        .filter(|enemy| enemy.is_alive())
        .map(|enemy| SnapshotInimigo {
            id: enemy.id,
            x: enemy.position.x,
            z: enemy.position.z,
            hp: enemy.hp_u16(),
            tipo: enemy.kind_byte(),
            fase_chefe: enemy.boss_phase,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use giochi_net::wire::SnapshotInimigo;

    fn snap(id: u16, x: f32, z: f32) -> SnapshotInimigo {
        SnapshotInimigo {
            id,
            x,
            z,
            hp: 100,
            tipo: 0,
            fase_chefe: 0,
        }
    }

    #[test]
    fn the_area_keeps_what_is_inside_and_drops_the_rest() {
        let area = area_at(Vec3::ZERO, 10.0);
        let input = vec![
            snap(1, 0.0, 0.0),
            snap(2, 9.0, 0.0),
            snap(3, 30.0, 0.0),
            snap(4, 0.0, -25.0),
        ];
        let visible = filter(&input, &area);
        let ids: Vec<u16> = visible.iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn the_border_is_inclusive() {
        let area = area_at(Vec3::new(5.0, 0.0, 5.0), 3.0);
        // Exatamente sobre o raio: fica dentro (evita o inimigo que "pisca" na borda).
        assert_eq!(filter(&[snap(1, 8.0, 5.0)], &area).len(), 1);
        assert!(filter(&[snap(2, 8.01, 5.0)], &area).is_empty());
    }

    #[test]
    fn an_empty_area_sees_nothing() {
        let area = area_at(Vec3::ZERO, 0.0);
        assert!(filter(&[snap(1, 0.0, 0.0)], &area).is_empty());
        assert!(filter(&[], &area).is_empty());
    }

    #[test]
    fn the_camera_area_scales_with_the_profile() {
        let camera = IsoCamera::new(Vec3::ZERO);
        let generous = camera.aoi_radius(16.0 / 9.0, 1.5);
        let aggressive = camera.aoi_radius(16.0 / 9.0, 1.0);
        assert!(aggressive < generous);
        // A área derivada da câmera filtra o que a de raio pequeno filtraria.
        let far = area_at(Vec3::new(generous * 0.9, 0.0, 0.0), 0.5);
        assert!(filter(&[snap(1, generous * 0.9, 0.0)], &far).len() <= 1);
    }

    #[test]
    fn dead_enemies_never_enter_a_snapshot() {
        use crate::config::GameConfig;
        use crate::config::EnemyKind;
        use crate::game::enemy::Enemy;
        use std::path::Path;

        let config =
            GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).unwrap();
        let def = config.enemy(EnemyKind::Grunt).unwrap();
        let mut alive = Enemy::spawn(1, def, Vec3::ZERO, config.scaling_for_wave(1));
        let mut dead = Enemy::spawn(2, def, Vec3::ZERO, config.scaling_for_wave(1));
        dead.take_damage(1.0e6);

        let snapshot = from_enemies(&[alive.clone(), dead]);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].id, 1);

        alive.take_damage(0.1);
        let _ = from_enemies(&[alive]);
    }
}
