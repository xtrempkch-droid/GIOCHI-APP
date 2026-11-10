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

pub mod font;
pub mod hud;
pub mod reticle;

use giochi_engine_3d::math3d::Camera3D;
use giochi_engine_3d::math3d::Mat4;
use giochi_engine_3d::math3d::Quat;
use giochi_engine_3d::math3d::Transform3D;
use giochi_engine_3d::math3d::Vec3;

use crate::utils::procedural_mesh::{quad_unitario, Malha};
use hud::{Cena, Tela};

/// Quantos degraus de alfa a batelada distingue.
///
/// A cor é uniforme por desenho, então cada (cor, alfa) distinto custa uma
/// chamada. Sem quantizar, cada `+100` desbotando em seu próprio instante seria
/// uma chamada — e oito abates seguidos custariam oito. Com oito degraus, todos
/// eles caem em no máximo oito lotes, e a diferença de alfa entre dois degraus
/// (12 %) é invisível a 60 Hz num efeito que dura 0,9 s.
const DEGRAUS_DE_ALFA: f32 = 8.0;

/// Um lote de instâncias que compartilham a mesma cor.
#[derive(Debug, Clone, PartialEq)]
pub struct Lote {
    /// Cor com o alfa já quantizado.
    pub cor: [f32; 4],
    /// Uma matriz por quadrilátero, em unidades da câmera do HUD.
    pub matrizes: Vec<Mat4>,
}

/// Matriz de instância para um retângulo **em pixels** da tela.
///
/// O quadrilátero unitário tem 1 unidade de lado centrado na origem; a matriz o
/// leva ao centro do retângulo e o estica para `(w, h)`.
///
/// ## O fator 2 não é decoração
///
/// Com `fov` de 90° a **meia**-altura visível a `PROFUNDIDADE` unidades é a
/// própria `PROFUNDIDADE`; a altura inteira é `2·PROFUNDIDADE` e é ela que cobre
/// `tela.altura` pixels. Então 1 px vale `2·PROFUNDIDADE / altura` unidades, que
/// é exatamente a escala de [`reticle::pixel_para_hud`].
///
/// A primeira versão usou `PROFUNDIDADE` sem o 2, e o sintoma foi mudo: os
/// marcadores de vida pedidos com 14 px saíam com **7**, e as células dos glifos
/// saíam com metade do lado — o texto continuava legível (as posições estão
/// certas, só o tamanho da célula errava) e nada no log dizia nada. O teste
/// `um_retangulo_de_pixels_ocupa_os_mesmos_pixels` fecha essa porta.
pub fn matriz_de_pixels(x: f32, y: f32, w: f32, h: f32, tela: Tela) -> Mat4 {
    let largura = tela.largura.max(1.0);
    let altura = tela.altura.max(1.0);
    let aspecto = largura / altura;
    let profundidade = reticle::PROFUNDIDADE;

    let ndc_x = (x + w * 0.5) / largura * 2.0 - 1.0;
    let ndc_y = 1.0 - (y + h * 0.5) / altura * 2.0;

    Transform3D::new(
        Vec3::new(
            ndc_x * profundidade * aspecto,
            ndc_y * profundidade,
            -profundidade,
        ),
        Quat::IDENTITY,
        Vec3::new(
            (w / largura) * 2.0 * profundidade * aspecto,
            (h / altura) * 2.0 * profundidade,
            1.0,
        ),
    )
    .matrix()
}

/// Chave de batelada: cor com o alfa quantizado.
fn chave(cor: [f32; 4]) -> [f32; 4] {
    let alfa = (cor[3].clamp(0.0, 1.0) * DEGRAUS_DE_ALFA).round() / DEGRAUS_DE_ALFA;
    [cor[0], cor[1], cor[2], alfa]
}

/// Converte a cena da HUD em lotes de desenho.
///
/// Cada célula acesa de cada glifo e cada retângulo vira uma instância do
/// quadrilátero unitário. O texto de um quadro inteiro sai em **uma** chamada
/// por cor: a HUD completa do jogo cabe em menos de dez.
pub fn lotes(cena: &Cena, tela: Tela) -> Vec<Lote> {
    let mut saida: Vec<Lote> = Vec::new();

    let mut empurrar = |matriz: Mat4, cor: [f32; 4]| {
        let cor = chave(cor);
        match saida.iter_mut().find(|lote| lote.cor == cor) {
            Some(lote) => lote.matrizes.push(matriz),
            None => saida.push(Lote {
                cor,
                matrizes: vec![matriz],
            }),
        }
    };

    for retangulo in &cena.retangulos {
        if retangulo.cor[3] <= 0.0 || retangulo.w <= 0.0 || retangulo.h <= 0.0 {
            continue;
        }
        empurrar(
            matriz_de_pixels(retangulo.x, retangulo.y, retangulo.w, retangulo.h, tela),
            retangulo.cor,
        );
    }

    for texto in &cena.textos {
        if texto.cor[3] <= 0.0 {
            continue;
        }
        // 1,06 de sobreposição: sem ela cada célula fica com uma fresta de fundo
        // e o texto parece serrilhado. O `neon-drift` mediu o mesmo.
        let lado = texto.escala * 1.06;
        for [px, py] in font::pontos(&texto.conteudo) {
            empurrar(
                matriz_de_pixels(
                    texto.x + px as f32 * texto.escala,
                    texto.y + py as f32 * texto.escala,
                    lado,
                    lado,
                    tela,
                ),
                texto.cor,
            );
        }
    }

    saida
}

/// A malha do quadrilátero unitário usada por todo o HUD.
pub fn quad_da_hud() -> Malha {
    quad_unitario()
}

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
    fn um_retangulo_de_pixels_ocupa_os_mesmos_pixels() {
        // O teste que falta doía: com o fator 2 esquecido na escala, um
        // marcador de 14 px saía com 7 e nada no log dizia nada. Aqui a matriz
        // de instância é aplicada aos cantos do quad unitário e projetada com a
        // **mesma** câmera do desenho; o resultado tem de cair no retângulo
        // pedido, em pixels.
        for (lw, lh) in [(640u32, 480u32), (800, 600), (1280, 720), (720, 1280)] {
            let tela = Tela::new(lw, lh);
            let camera = camera((lw, lh));
            let matriz = camera.view_projection().expect("matriz do HUD");
            for (x, y, w, h) in [
                (0.0, 0.0, 14.0, 14.0),
                (100.0, 50.0, 30.0, 21.0),
                (lw as f32 - 60.0, lh as f32 - 40.0, 60.0, 40.0),
            ] {
                let instancia = super::matriz_de_pixels(x, y, w, h, tela);
                let cantos = [
                    [-0.5f32, -0.5, 0.0],
                    [0.5, -0.5, 0.0],
                    [0.5, 0.5, 0.0],
                    [-0.5, 0.5, 0.0],
                ];
                let mut xs = Vec::new();
                let mut ys = Vec::new();
                for canto in cantos {
                    let mundo = instancia.transform_point(Vec3::new(canto[0], canto[1], canto[2]));
                    let ndc = matriz.project_point(mundo).expect("projetável");
                    xs.push((ndc.x + 1.0) * 0.5 * lw as f32);
                    ys.push((1.0 - ndc.y) * 0.5 * lh as f32);
                }
                let (min_x, max_x) = (f32::MIN, f32::MAX);
                let esq = xs.iter().cloned().fold(max_x, f32::min);
                let dir = xs.iter().cloned().fold(min_x, f32::max);
                let topo = ys.iter().cloned().fold(max_x, f32::min);
                let base = ys.iter().cloned().fold(min_x, f32::max);

                assert!(
                    (esq - x).abs() < 0.5 && (dir - (x + w)).abs() < 0.5,
                    "{lw}x{lh}: quadrilátero em x {esq:.1}..{dir:.1}, pedido {x:.0}..{:.0}",
                    x + w
                );
                assert!(
                    (topo - y).abs() < 0.5 && (base - (y + h)).abs() < 0.5,
                    "{lw}x{lh}: quadrilátero em y {topo:.1}..{base:.1}, pedido {y:.0}..{:.0}",
                    y + h
                );
            }
        }
    }

    #[test]
    fn a_batelada_junta_o_que_tem_a_mesma_cor() {
        // Uma chamada por cor é o que faz a HUD caber em poucos desenhos; sem a
        // batelada, cada célula de glifo seria uma chamada.
        let tela = Tela::new(640, 480);
        let cena = hud::cena(
            &crate::state::GameState::new(),
            &hud::Hud::new(),
            tela,
            crate::gameplay::weapon::PULSAR.nome,
        );
        let lotes = lotes(&cena, tela);
        let celulas: usize = cena
            .textos
            .iter()
            .map(|t| font::pontos_acesos(&t.conteudo))
            .sum();
        let instancias: usize = lotes.iter().map(|l| l.matrizes.len()).sum();

        assert!(
            celulas > 300,
            "a HUD tem de ter centenas de células: {celulas}"
        );
        assert_eq!(
            instancias,
            celulas + cena.retangulos.len(),
            "toda célula e todo retângulo têm de virar instância"
        );
        assert!(
            lotes.len() <= 4,
            "{} lote(s) para {} instâncias: a batelada não está juntando",
            lotes.len(),
            instancias
        );
    }

    #[test]
    fn o_alfa_quantizado_nao_multiplica_lotes() {
        let tela = Tela::new(640, 480);
        let mut estado_hud = hud::Hud::new();
        // Oito abates, cada um numa idade diferente: sem quantizar, oito alfas.
        for i in 0..8 {
            estado_hud.abateu(100.0 + i as f32 * 20.0, 200.0, 100);
            estado_hud.atualizar(0.02);
        }
        let cena = hud::efeitos(&crate::state::GameState::new(), &estado_hud, tela);
        let lotes = lotes(&cena, tela);
        assert_eq!(estado_hud.flutuantes(), 8);
        assert!(
            lotes.len() <= 8,
            "8 correios vivos viraram {} lotes",
            lotes.len()
        );
    }

    #[test]
    fn texto_invisivel_nao_vira_instancia() {
        let tela = Tela::new(640, 480);
        let cena = Cena {
            textos: vec![hud::Texto {
                conteudo: "PONTOS".to_string(),
                x: 10.0,
                y: 10.0,
                escala: 2.0,
                cor: [1.0, 1.0, 1.0, 0.0],
            }],
            retangulos: Vec::new(),
        };
        assert!(
            lotes(&cena, tela).is_empty(),
            "texto com alfa 0 não pode custar desenho"
        );
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
