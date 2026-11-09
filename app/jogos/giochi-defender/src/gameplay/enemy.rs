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
use giochi_engine_3d::math3d::Vec3;

/// Aresta do cubo, em unidades de mundo.
pub const LADO_PADRAO: f32 = 0.8;

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
        }
    }

    /// Nasce em `posicao` indo para o centro da arena.
    pub fn spawn_at(posicao: Vec3, velocidade: f32) -> Self {
        Self {
            position: posicao,
            speed: if velocidade.is_finite() {
                velocidade.max(0.0)
            } else {
                0.0
            },
            alive: true,
            size: LADO_PADRAO,
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
        let cubo = Enemy::spawn_at(Vec3::new(10.0, 0.0, 0.0), 3.0);
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
        let mut cubo = Enemy::spawn_at(Vec3::new(10.0, 0.0, 0.0), 5.0);
        cubo.update(2.0, Vec3::ZERO);
        assert!(cubo.distancia(Vec3::ZERO) < 1e-3, "{:?}", cubo.position);
    }

    #[test]
    fn nao_atravessa_o_alvo_em_um_passo_grande() {
        let mut cubo = Enemy::spawn_at(Vec3::new(3.0, 0.0, 0.0), 100.0);
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
        let cubo = Enemy::spawn_at(Vec3::new(1.0, 2.0, 3.0), f32::NAN);
        assert_eq!(cubo.speed, 0.0);
        let mut cubo = cubo;
        cubo.update(1.0, Vec3::ZERO);
        assert!(cubo.position.is_finite());
    }

    #[test]
    fn dt_invalido_e_ignorado() {
        let mut cubo = Enemy::spawn_at(Vec3::new(1.0, 0.0, 0.0), 5.0);
        let antes = cubo.position;
        cubo.update(0.0, Vec3::ZERO);
        cubo.update(-1.0, Vec3::ZERO);
        cubo.update(f32::NAN, Vec3::ZERO);
        assert_eq!(cubo.position, antes);
    }

    #[test]
    fn a_caixa_tem_o_lado_do_cubo() {
        let cubo = Enemy::spawn_at(Vec3::new(2.0, 0.0, 0.0), 1.0);
        let caixa = cubo.aabb();
        assert!((caixa.meio.x - LADO_PADRAO / 2.0).abs() < 1e-6);
        assert!((caixa.centro.x - 2.0).abs() < 1e-6);
    }
}
