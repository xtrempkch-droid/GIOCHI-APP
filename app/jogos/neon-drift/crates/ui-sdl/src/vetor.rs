//! Operações vetoriais que a engine não expõe.
//!
//! `Vec3` da engine é `#[repr(C)]` e **não implementa `Add`/`Mul`**: ela existe
//! para atravessar a ABI em C, não para ser conveniente em Rust. Em vez de
//! espalhar `Vec3::new(a.x + b.x, ...)` por toda a cena, as operações que faltam
//! ficam aqui, nomeadas e testadas.

use giochi_engine_3d::math3d::Vec3;

pub fn somar(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

pub fn subtrair(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

pub fn escalar(a: Vec3, k: f32) -> Vec3 {
    Vec3::new(a.x * k, a.y * k, a.z * k)
}

pub fn misturar(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    somar(escalar(a, 1.0 - t), escalar(b, t))
}

pub fn produto_vetorial(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

pub fn direcao_normalizada(a: Vec3) -> Vec3 {
    let n = comprimento(a);
    if n < 1e-6 {
        Vec3::ZERO
    } else {
        escalar(a, 1.0 / n)
    }
}

pub fn comprimento(a: Vec3) -> f32 {
    (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
}

/// Produto escalar. A engine não expõe `dot` no `Vec3` (ADR: `Vec3` é `#[repr(C)]`
/// e só carrega o que a ABI precisa), então ele mora aqui.
pub fn produto_escalar(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Base de billboard: direita e cima ortogonais à direção de visão.
///
/// Devolve `(direita, cima)`. Se a câmera olhar reto para cima (produto vetorial
/// nulo), cai para `Vec3::UP` — sem isso, um quadro desenhado do topo dividiria
/// por zero e as partículas sumiriam.
pub fn base_billboard(frente: Vec3) -> (Vec3, Vec3) {
    let mut direita = produto_vetorial(frente, Vec3::UP);
    if comprimento(direita) < 1e-4 {
        direita = produto_vetorial(frente, Vec3::new(0.0, 0.0, 1.0));
    }
    let direita = direcao_normalizada(direita);
    let cima = direcao_normalizada(produto_vetorial(direita, frente));
    (direita, cima)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn misturar_nos_extremos_devolve_os_originais() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(-4.0, 5.0, 6.0);
        assert_eq!(misturar(a, b, 0.0), a);
        assert_eq!(misturar(a, b, 1.0), b);
    }

    #[test]
    fn normalizar_vetor_nulo_nao_estoura() {
        assert_eq!(direcao_normalizada(Vec3::ZERO), Vec3::ZERO);
    }

    #[test]
    fn a_base_do_billboard_e_ortogonal_a_frente() {
        let frente = direcao_normalizada(Vec3::new(0.3, -0.2, -1.0));
        let (direita, cima) = base_billboard(frente);
        assert!(produto_escalar(direita, frente).abs() < 1e-4, "direita não ortogonal");
        assert!(produto_escalar(cima, frente).abs() < 1e-4, "cima não ortogonal");
        assert!(produto_escalar(direita, cima).abs() < 1e-4, "base não ortogonal");
    }

    #[test]
    fn olhando_para_o_z_negativo_a_direita_aponta_para_x() {
        let (direita, cima) = base_billboard(Vec3::new(0.0, 0.0, -1.0));
        assert!((direita.x - 1.0).abs() < 1e-4, "direita {direita:?}");
        assert!((cima.y - 1.0).abs() < 1e-4, "cima {cima:?}");
    }

    #[test]
    fn olhando_para_o_topo_a_base_continua_valida() {
        let (direita, cima) = base_billboard(Vec3::new(0.0, -1.0, 0.0));
        assert!(comprimento(direita) > 0.99, "direita degenerou");
        assert!(comprimento(cima) > 0.99, "cima degenerou");
    }
}
