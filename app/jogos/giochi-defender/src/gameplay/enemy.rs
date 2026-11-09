//! O inimigo: um cubo que voa para o asteroide.
//!
//! ## Por que um cubo
//!
//! É a forma mais barata que lê como ameaça em 3D *low-poly*, e a caixa alinhada
//! aos eixos dá um AABB exato de graça — sem isso a colisão precisaria de SAT ou
//! de esfera circunscrita, e a esfera faria o cubo "acertar" antes de encostar.
//!
//! ## O cubo não gira
//!
//! Girar exigiria um AABB diferente por cubo (matriz de rotação), e a colisão
//! deixaria de ser exata. Um cubo que não gira e aponta para o alvo é a escolha
//! que mantém o teste honesto.

use crate::gameplay::collision::Aabb;
use crate::utils::math::lerp;
use giochi_engine_3d::math3d::{Color3D, Vec3};

/// Aresta do cubo, em unidades de mundo.
pub const LADO_PADRAO: f32 = 0.8;

/// Extremo frio da paleta do inimigo: vermelho fundo.
///
/// A variação por cubo existe para dois cubos próximos não virarem uma mancha
/// só. Com uma cor única, dois cubos encostados somem um no outro e o jogador
/// perde a conta de quantos vêm.
pub const COR_VERMELHA: Color3D = Color3D::new(0.72, 0.10, 0.08, 1.0);
/// Extremo quente da paleta: laranja.
///
/// Mais claro que o vermelho **de propósito**: a rampa muda matiz e luminância
/// juntas, então dois cubos de variações diferentes se separam por cor e por
/// brilho ao mesmo tempo. Uma rampa só de matiz, com brilho constante, deixa dois
/// cubos sobrepostos com o mesmo tom médio — e a leitura de "quantos vêm" é
/// justamente o que a variação existe para salvar.
pub const COR_LARANJA: Color3D = Color3D::new(1.0, 0.58, 0.16, 1.0);

/// Cor do cubo para uma variação em `0.0..=1.0`.
///
/// Interpolação linear: os extremos são exatamente [`COR_VERMELHA`] e
/// [`COR_LARANJA`], o que evita a armadilha de a constante e a função darem
/// cores diferentes no mesmo ponto (aconteceu na primeira versão, com um fator
/// de brilho extra).
pub fn cor_do_cubo(variacao: f32) -> Color3D {
    let t = if variacao.is_finite() {
        variacao.clamp(0.0, 1.0)
    } else {
        0.0
    };
    Color3D::new(
        lerp(COR_VERMELHA.r, COR_LARANJA.r, t),
        lerp(COR_VERMELHA.g, COR_LARANJA.g, t),
        lerp(COR_VERMELHA.b, COR_LARANJA.b, t),
        1.0,
    )
}

/// Luminância percebida de uma cor (Rec. 709).
pub fn luminancia(cor: Color3D) -> f32 {
    0.2126 * cor.r + 0.7152 * cor.g + 0.0722 * cor.b
}

/// Um cubo hostil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enemy {
    /// Posição no mundo.
    pub position: Vec3,
    /// Velocidade em unidades por segundo.
    pub speed: f32,
    /// Vivo (no ar e colidível).
    pub alive: bool,
    /// Aresta do cubo.
    pub size: f32,
    /// Cor desta unidade, fixada no nascimento.
    ///
    /// No cubo e não no desenho: com a cor no desenho, todos os cubos teriam de
    /// ser desenhados com a mesma cor (a engine usa uma cor por desenho) ou a cor
    /// mudaria a cada quadro. Guardada no cubo, ela é estável e testável sem GL.
    pub cor: Color3D,
}

impl Enemy {
    /// Um cubo morto, para inicializar o pool sem `Option`.
    ///
    /// `Vec3::ZERO` e `alive = false`: um cubo morto não é desenhado nem
    /// colidido, então a posição dele não significa nada — e não vale gastar uma
    /// alocação por slot para dizer isso.
    pub const fn morto() -> Self {
        Self {
            position: Vec3::ZERO,
            speed: 0.0,
            alive: false,
            size: LADO_PADRAO,
            cor: COR_VERMELHA,
        }
    }

    /// Nasce em `posicao` indo para o centro da arena, com a cor da variação
    /// dada.
    pub fn spawn_at(posicao: Vec3, velocidade: f32, variacao: f32) -> Self {
        Self {
            position: posicao,
            speed: if velocidade.is_finite() {
                velocidade.max(0.0)
            } else {
                0.0
            },
            alive: true,
            size: LADO_PADRAO,
            cor: cor_do_cubo(variacao),
        }
    }

    /// Direção unitária até o alvo, ou zero se já está em cima dele.
    pub fn direcao_ao_alvo(&self, alvo: Vec3) -> Vec3 {
        let d = Vec3::new(
            alvo.x - self.position.x,
            alvo.y - self.position.y,
            alvo.z - self.position.z,
        );
        if d.length() <= 1e-6 {
            return Vec3::ZERO;
        }
        d.normalized()
    }

    /// Avança em direção ao alvo.
    ///
    /// A direção é recalculada a cada passo em vez de guardada: custa uma
    /// normalização e faz o cubo continuar correto se o alvo um dia se mover (o
    /// que o bloco 2 vai querer para o "atirador").
    pub fn update(&mut self, dt: f32, alvo: Vec3) {
        if !self.alive || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let direcao = self.direcao_ao_alvo(alvo);
        // O passo é limitado pela distância que falta. Sem este `min`, um passo
        // grande atravessa o alvo e o cubo passa a oscilar em volta dele — e
        // "atravessou o asteroide sem doer" é o defeito que este `min` evita
        // para qualquer `dt`, inclusive o de um quadro travado.
        let passo = (self.speed * dt).min(self.distancia(alvo));
        self.position = Vec3::new(
            self.position.x + direcao.x * passo,
            self.position.y + direcao.y * passo,
            self.position.z + direcao.z * passo,
        );
    }

    /// Distância ao alvo.
    pub fn distancia(&self, alvo: Vec3) -> f32 {
        let dx = alvo.x - self.position.x;
        let dy = alvo.y - self.position.y;
        let dz = alvo.z - self.position.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Caixa do cubo, para a colisão.
    pub fn aabb(&self) -> Aabb {
        Aabb::do_cubo(self.position, self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_cubo_nasce_vivo_e_apontando_para_o_centro() {
        let cubo = Enemy::spawn_at(Vec3::new(10.0, 0.0, 0.0), 3.0, 0.5);
        assert!(cubo.alive);
        assert_eq!(cubo.size, LADO_PADRAO);
        let direcao = cubo.direcao_ao_alvo(Vec3::ZERO);
        assert!((direcao.x + 1.0).abs() < 1e-5, "{direcao:?}");
        assert!(direcao.y.abs() < 1e-5);
    }

    #[test]
    fn ele_chega_ao_alvo_na_conta_certa() {
        // 10 unidades a 5 u/s são 2 s. Um passo de 10 s tem de pousar no alvo,
        // não passar dele — a direção é recalculada e o excesso é descartado
        // pelo `normalized` de um vetor quase zero.
        let mut cubo = Enemy::spawn_at(Vec3::new(10.0, 0.0, 0.0), 5.0, 0.0);
        cubo.update(2.0, Vec3::ZERO);
        assert!(cubo.distancia(Vec3::ZERO) < 1e-3, "{:?}", cubo.position);
    }

    #[test]
    fn nao_atravessa_o_alvo_em_um_passo_grande() {
        let mut cubo = Enemy::spawn_at(Vec3::new(3.0, 0.0, 0.0), 100.0, 0.0);
        for _ in 0..10 {
            cubo.update(0.1, Vec3::ZERO);
        }
        // Se ele passasse do centro, a distância voltaria a crescer.
        assert!(cubo.distancia(Vec3::ZERO) < 1e-3, "{:?}", cubo.position);
    }

    #[test]
    fn um_cubo_morto_nao_se_move() {
        let mut cubo = Enemy::morto();
        cubo.update(1.0, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(cubo.position, Vec3::ZERO);
    }

    #[test]
    fn velocidade_invalida_nao_gera_nan() {
        let cubo = Enemy::spawn_at(Vec3::new(1.0, 2.0, 3.0), f32::NAN, 0.0);
        assert_eq!(cubo.speed, 0.0);
        let mut cubo = cubo;
        cubo.update(1.0, Vec3::ZERO);
        assert!(cubo.position.is_finite());
    }

    #[test]
    fn dt_invalido_e_ignorado() {
        let mut cubo = Enemy::spawn_at(Vec3::new(1.0, 0.0, 0.0), 5.0, 0.0);
        let antes = cubo.position;
        cubo.update(0.0, Vec3::ZERO);
        cubo.update(-1.0, Vec3::ZERO);
        cubo.update(f32::NAN, Vec3::ZERO);
        assert_eq!(cubo.position, antes);
    }

    #[test]
    fn a_paleta_do_inimigo_e_vermelho_laranja() {
        // A regra do plano: cubos vermelho-laranja. Em HSL isso é matiz perto de
        // 0-30 graus, ou seja vermelho dominante e azul no chão — e é isto que
        // separa o cubo do asteroide cinza e do céu azulado.
        for passo in 0..=10 {
            let cor = cor_do_cubo(passo as f32 / 10.0);
            assert!(cor.r > 0.6, "r {:.2} baixo demais", cor.r);
            assert!(cor.r > cor.g, "r {:.2} não domina g {:.2}", cor.r, cor.g);
            assert!(cor.g > cor.b, "g {:.2} não domina b {:.2}", cor.g, cor.b);
            assert_eq!(cor.a, 1.0);
        }
    }

    #[test]
    fn os_extremos_da_paleta_sao_as_constantes() {
        // A primeira versão aplicava um fator de brilho por cima da interpolação,
        // e `cor_do_cubo(0.0)` deixou de ser `COR_VERMELHA`. Duas fontes para a
        // mesma cor é o começo de uma divergência silenciosa.
        assert_eq!(cor_do_cubo(0.0), COR_VERMELHA);
        assert_eq!(cor_do_cubo(1.0), COR_LARANJA);
    }

    #[test]
    fn a_luminancia_sobe_com_a_variacao() {
        // É a rampa que separa dois cubos sobrepostos. Só matiz não basta: dois
        // cubos de tons diferentes ficam com o mesmo brilho médio e somem um no
        // outro quando um passa na frente do outro.
        let mut anterior = luminancia(cor_do_cubo(0.0));
        for passo in 1..=10 {
            let atual = luminancia(cor_do_cubo(passo as f32 / 10.0));
            assert!(
                atual > anterior,
                "passo {passo}: luminância caiu de {anterior:.3} para {atual:.3}"
            );
            anterior = atual;
        }
        assert!(
            luminancia(COR_LARANJA) > luminancia(COR_VERMELHA) * 1.5,
            "a faixa inteira é estreita demais para separar dois cubos"
        );
    }

    #[test]
    fn a_cor_muda_de_verdade_entre_variacoes_vizinhas() {
        // Duas variações quaisquer têm de dar cores distinguíveis a olho. O
        // limite de 0,10 no canal verde é ~25 níveis de 255: visível num painel
        // de 3,5".
        for passo in 0..10 {
            let a = cor_do_cubo(passo as f32 / 10.0);
            let b = cor_do_cubo((passo + 1) as f32 / 10.0);
            assert!(
                (a.g - b.g).abs() > 0.04 || (luminancia(a) - luminancia(b)).abs() > 0.04,
                "passo {passo}: {a:?} e {b:?} são a mesma cor na tela"
            );
        }
    }

    #[test]
    fn variacao_fora_da_faixa_e_presa() {
        // `NaN` viraria `NaN` na cor, e uma cor `NaN` atravessa o uniforme e
        // pinta o cubo de preto sem erro nenhum.
        assert_eq!(cor_do_cubo(-5.0), cor_do_cubo(0.0));
        assert_eq!(cor_do_cubo(5.0), cor_do_cubo(1.0));
        let com_nan = cor_do_cubo(f32::NAN);
        assert!(com_nan.r.is_finite() && com_nan.g.is_finite() && com_nan.b.is_finite());
    }

    #[test]
    fn o_cubo_nasce_com_a_cor_da_variacao() {
        let a = Enemy::spawn_at(Vec3::ZERO, 1.0, 0.0);
        let b = Enemy::spawn_at(Vec3::ZERO, 1.0, 1.0);
        assert_eq!(a.cor, COR_VERMELHA);
        assert_eq!(b.cor, COR_LARANJA);
        assert_ne!(a.cor, b.cor);
    }

    #[test]
    fn a_caixa_tem_o_lado_do_cubo() {
        let cubo = Enemy::spawn_at(Vec3::new(2.0, 0.0, 0.0), 1.0, 0.0);
        let caixa = cubo.aabb();
        assert!((caixa.meio.x - LADO_PADRAO / 2.0).abs() < 1e-6);
        assert!((caixa.centro.x - 2.0).abs() < 1e-6);
    }
}
