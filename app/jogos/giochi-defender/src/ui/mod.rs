//! A camada de HUD sobre a cena 3D.
//!
//! ## Por que a câmera do HUD e não um segundo renderer
//!
//! A engine 3D desenha malha; não há caminho de 2D dentro dela (ADR-037: um jogo
//! escolhe um renderer). O HUD então é geometria em espaço de tela, desenhada
//! com uma câmera de `fov` 90° no plano de [`reticle::PROFUNDIDADE`] — a mesma
//! solução que o `neon-drift` já usa e mediu.
//!
//! O plano próximo em 9,0 com o HUD a 10,0 é o detalhe que não pode erradicar:
//! dá profundidade normalizada ~0,10, à frente de qualquer geometria do mundo
//! (≥0,87 a partir de 2 m). Com os planos 0,5..60 — que é o padrão da câmera —
//! o HUD cairia em ~0,96 e **sumiria atrás do asteroide**.

pub mod reticle;

use giochi_engine_3d::math3d::Camera3D;
use giochi_engine_3d::math3d::Vec3;

/// Câmera do HUD para um tamanho de tela.
///
/// `fov` de 90° é o que torna a conversão de [`reticle::pixel_para_hud`] exata:
/// a meia-altura visível a `PROFUNDIDADE` unidades é exatamente a própria
/// `PROFUNDIDADE`.
pub fn camera(tela: (u32, u32)) -> Camera3D {
    Camera3D::from_parts(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP)
        .with_fov_y(std::f32::consts::FRAC_PI_2)
        .with_clip_planes(9.0, 4000.0)
        .with_drawable_size(tela.0, tela.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_do_hud_e_valida_em_todos_os_paineis() {
        for tela in [(640, 480), (800, 600), (1280, 720), (720, 1280)] {
            let camera = camera(tela);
            assert!(
                camera.view_projection().is_ok(),
                "{tela:?}: matriz de visão-projeção inválida"
            );
        }
    }

    #[test]
    fn o_hud_fica_a_frente_de_qualquer_geometria_do_mundo() {
        // O teste que prende o plano próximo: o HUD a 10 unidades com planos
        // 9..4000 tem profundidade normalizada pequena; o mundo mais próximo
        // (2 m, que é a esfera de colisão) tem de ficar atrás.
        let tela = (640, 480);
        let hud = camera(tela);
        let matriz = hud.view_projection().expect("matriz");
        let projetado = matriz
            .project_point(Vec3::new(0.0, 0.0, -reticle::PROFUNDIDADE))
            .expect("ponto do HUD projetável");
        assert!(
            projetado.z < 0.2,
            "profundidade normalizada do HUD {:.3} — atrás do mundo",
            projetado.z
        );

        // O mundo usa a câmera do jogo, mas a conta de profundidade é a mesma
        // fórmula; 2 m adiante do olho já dá ≥ 0,87 com planos 0,1..400.
        let mundo = Camera3D::from_parts(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP)
            .with_clip_planes(0.1, 400.0)
            .with_drawable_size(tela.0, tela.1);
        let perto = mundo
            .view_projection()
            .expect("matriz do mundo")
            .project_point(Vec3::new(0.0, 0.0, -2.0))
            .expect("ponto do mundo");
        assert!(
            perto.z > projetado.z,
            "o mundo a 2 m ({:.3}) ficou à frente do HUD ({:.3})",
            perto.z,
            projetado.z
        );
    }
}
