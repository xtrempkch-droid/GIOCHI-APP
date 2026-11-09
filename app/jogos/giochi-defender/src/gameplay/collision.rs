//! Colisão: esfera × caixa e raio × caixa.
//!
//! ## Por que implementado aqui e não na engine
//!
//! A engine não tem primitiva de colisão, e a regra R1 diz que o que falta de
//! pequeno se implementa no jogo. São duas funções de vinte linhas com teste —
//! não vale abrir uma dependência nem pedir mudança na engine.
//!
//! ## Os dois testes que importam de verdade
//!
//! "Acertou" e "não acertou" são fáceis de acertar por acidente. Os casos que
//! quebram na prática são **encostar exatamente** (o `<=` do raio e o `t = 0` do
//! raio que começa dentro da caixa) e **raio paralelo a um eixo**, que faz o
//! `1.0 / 0.0` virar infinito e o algoritmo devolver um acerto inexistente.

use giochi_engine_3d::math3d::Vec3;

/// Caixa alinhada aos eixos, descrita por centro e meio-tamanho.
///
/// Centro + meio e não mínimo/máximo: é a forma que o cubo do inimigo já tem
/// (posição e lado) e evita converter em dois sentidos no caminho quente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Centro da caixa.
    pub centro: Vec3,
    /// Meio-tamanho por eixo (nunca negativo).
    pub meio: Vec3,
}

impl Aabb {
    /// Caixa a partir do centro e do meio-tamanho.
    pub fn new(centro: Vec3, meio: Vec3) -> Self {
        Self {
            centro,
            meio: Vec3::new(meio.x.abs(), meio.y.abs(), meio.z.abs()),
        }
    }

    /// Caixa de um cubo de aresta `lado` centrado em `centro`.
    pub fn do_cubo(centro: Vec3, lado: f32) -> Self {
        let metade = lado.abs() * 0.5;
        Self::new(centro, Vec3::new(metade, metade, metade))
    }

    /// Esfera que contém a caixa (raio = meia diagonal).
    ///
    /// É o teste barato que descarta a maioria dos pares antes do exato.
    pub fn raio_circunscrito(&self) -> f32 {
        (self.meio.x * self.meio.x + self.meio.y * self.meio.y + self.meio.z * self.meio.z).sqrt()
    }
}

/// A esfera `(centro, raio)` intersecta a caixa?
///
/// Teste do ponto mais próximo: se a distância do centro da esfera ao ponto da
/// caixa mais próximo dele for menor que o raio, há sobreposição. Funciona com o
/// centro fora **e** dentro da caixa, sem caso especial.
pub fn sphere_aabb_intersect(centro: Vec3, raio: f32, caixa: Aabb) -> bool {
    // `raio >= 0.0` é falso para `NaN`, e é o que barra o raio inválido — a
    // forma negada existe para o `NaN` cair aqui em vez de passar adiante.
    if raio.is_nan() || raio < 0.0 || !raio.is_finite() {
        return false;
    }

    let c = centro.to_array();
    let b = caixa.centro.to_array();
    let h = caixa.meio.to_array();

    let mut distancia2 = 0.0;
    for eixo in 0..3 {
        let minimo = b[eixo] - h[eixo];
        let maximo = b[eixo] + h[eixo];
        let fora = if c[eixo] < minimo {
            minimo - c[eixo]
        } else if c[eixo] > maximo {
            c[eixo] - maximo
        } else {
            0.0
        };
        distancia2 += fora * fora;
    }

    distancia2 <= raio * raio
}

/// Distância até a caixa, ao longo de `direcao`, ou `None` se não acertar.
///
/// `direcao` deve ser unitária para o valor devolvido ser uma distância em
/// unidades de mundo; com um vetor qualquer ele é o parâmetro `t` da reta.
///
/// Devolve `Some(0.0)` quando a origem já está dentro — o tiro que nasce dentro
/// do cubo acerta na hora, e devolver `None` faria o raio atravessá-lo.
pub fn raycast_aabb(origem: Vec3, direcao: Vec3, caixa: Aabb) -> Option<f32> {
    let o = origem.to_array();
    let d = direcao.to_array();
    let b = caixa.centro.to_array();
    let h = caixa.meio.to_array();

    if !d.iter().all(|valor| valor.is_finite()) {
        return None;
    }

    let mut entrada = f32::NEG_INFINITY;
    let mut saida = f32::INFINITY;

    for eixo in 0..3 {
        let minimo = b[eixo] - h[eixo];
        let maximo = b[eixo] + h[eixo];

        if d[eixo].abs() < 1e-8 {
            // Raio paralelo a este eixo: ou está dentro da faixa para sempre, ou
            // nunca entra nela. Sem este caso, `1/0` vira infinito e o `min`/`max`
            // de `NaN` devolve um acerto que não existe.
            if o[eixo] < minimo || o[eixo] > maximo {
                return None;
            }
            continue;
        }

        let inverso = 1.0 / d[eixo];
        let mut t1 = (minimo - o[eixo]) * inverso;
        let mut t2 = (maximo - o[eixo]) * inverso;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        entrada = entrada.max(t1);
        saida = saida.min(t2);
        if entrada > saida {
            return None;
        }
    }

    if saida < 0.0 {
        // A caixa está inteira atrás da origem.
        return None;
    }

    Some(entrada.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cubo_na_origem(lado: f32) -> Aabb {
        Aabb::do_cubo(Vec3::ZERO, lado)
    }

    #[test]
    fn esfera_centro_no_meio_do_cubo_acerta() {
        assert!(sphere_aabb_intersect(Vec3::ZERO, 0.1, cubo_na_origem(2.0)));
    }

    #[test]
    fn esfera_longe_nao_acerta() {
        assert!(!sphere_aabb_intersect(
            Vec3::new(10.0, 0.0, 0.0),
            1.0,
            cubo_na_origem(2.0)
        ));
    }

    #[test]
    fn esfera_encostando_na_face_conta_como_colisao() {
        // Face em x = 1.0; centro a 1.5 com raio 0.5 toca exatamente. Este é o
        // caso que decide se o cubo "chega" ou "atravessa" antes de doer.
        assert!(sphere_aabb_intersect(
            Vec3::new(1.5, 0.0, 0.0),
            0.5,
            cubo_na_origem(2.0)
        ));
        assert!(!sphere_aabb_intersect(
            Vec3::new(1.5, 0.0, 0.0),
            0.49,
            cubo_na_origem(2.0)
        ));
    }

    #[test]
    fn esfera_diagonal_precisa_do_raio_ate_o_canto() {
        // (2,2,2) está a sqrt(3) ≈ 1.732 do canto (1,1,1).
        let centro = Vec3::new(2.0, 2.0, 2.0);
        assert!(!sphere_aabb_intersect(centro, 1.7, cubo_na_origem(2.0)));
        assert!(sphere_aabb_intersect(centro, 1.75, cubo_na_origem(2.0)));
    }

    #[test]
    fn esfera_de_raio_invalido_nao_acerta() {
        assert!(!sphere_aabb_intersect(
            Vec3::ZERO,
            -1.0,
            cubo_na_origem(2.0)
        ));
        assert!(!sphere_aabb_intersect(
            Vec3::ZERO,
            f32::NAN,
            cubo_na_origem(2.0)
        ));
    }

    #[test]
    fn raio_em_cheio_acerta_a_face_da_frente() {
        let t = raycast_aabb(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, -1.0),
            cubo_na_origem(2.0),
        )
        .expect("devia acertar");
        assert!((t - 9.0).abs() < 1e-4, "t = {t}, esperado 9");
    }

    #[test]
    fn raio_que_passa_de_lado_nao_acerta() {
        assert!(raycast_aabb(
            Vec3::new(5.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, -1.0),
            cubo_na_origem(2.0)
        )
        .is_none());
    }

    #[test]
    fn raio_apontando_para_o_lado_oposto_nao_acerta() {
        assert!(raycast_aabb(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 1.0),
            cubo_na_origem(2.0)
        )
        .is_none());
    }

    #[test]
    fn raio_dentro_do_cubo_acerta_na_hora() {
        // O tiro que nasce dentro tem de acertar; `None` faria o cubo ficar
        // invulnerável encostado na câmera.
        assert_eq!(
            raycast_aabb(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), cubo_na_origem(2.0)),
            Some(0.0)
        );
    }

    #[test]
    fn raio_paralelo_a_um_eixo_fora_da_faixa_nao_acerta() {
        // `1.0 / 0.0` é infinito e o algoritmo de slab devolveria um acerto
        // fantasma sem a guarda.
        assert!(raycast_aabb(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            cubo_na_origem(2.0)
        )
        .is_none());
    }

    #[test]
    fn raio_paralelo_a_um_eixo_dentro_da_faixa_acerta() {
        let t = raycast_aabb(
            Vec3::new(-10.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            cubo_na_origem(2.0),
        )
        .expect("devia acertar");
        assert!((t - 9.0).abs() < 1e-4, "t = {t}, esperado 9");
    }

    #[test]
    fn raio_que_raspa_o_canto_acerta() {
        // Origem na diagonal, direção **unitária** (sem normalizar, `t` é o
        // parâmetro da reta e não uma distância — a diferença é de 1,7 unidade e
        // foi o que este teste pegou na primeira versão).
        let direcao = Vec3::new(-1.0, -1.0, -1.0).normalized();
        let t = raycast_aabb(Vec3::new(3.0, 3.0, 3.0), direcao, cubo_na_origem(2.0))
            .expect("devia acertar o canto");
        // O primeiro plano tocado é x = 1, a uma distância de (3-1)*sqrt(3).
        assert!((t - 2.0 * 3.0_f32.sqrt()).abs() < 1e-3, "t = {t}");
    }

    #[test]
    fn raio_com_direcao_invalida_nao_acerta() {
        assert!(raycast_aabb(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(f32::NAN, 0.0, -1.0),
            cubo_na_origem(2.0)
        )
        .is_none());
    }

    #[test]
    fn o_raio_circunscrito_cobre_o_canto() {
        let caixa = cubo_na_origem(2.0);
        assert!((caixa.raio_circunscrito() - 3.0_f32.sqrt()).abs() < 1e-5);
    }
}
