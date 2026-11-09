//! Helpers numéricos pequenos e puros.
//!
//! Existem para não repetir `if x < a { a } else if x > b { b } else { x }` em
//! cinco arquivos — e, principalmente, para que os limites da câmera tenham **um**
//! lugar onde são testados.

/// Prende `valor` em `[min, max]`.
///
/// Se `valor` for `NaN`, devolve `min`: um `NaN` que chega na câmera vira uma
/// matriz de visão inválida, e a tela fica preta sem erro nenhum. Preferir um
/// limite errado a uma tela preta é a escolha menos ruim — e o chamador consegue
/// detectar o problema porque o valor fica no limite, não aleatório.
pub fn clamp(valor: f32, min: f32, max: f32) -> f32 {
    if valor.is_nan() {
        return min;
    }
    if valor < min {
        min
    } else if valor > max {
        max
    } else {
        valor
    }
}

/// Interpolação linear de `a` até `b` por `t`.
///
/// `t` não é preso em `0..=1` de propósito: extrapolar é útil em animação, e quem
/// quer o intervalo fecha com [`clamp`].
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Converte graus em radianos.
pub fn graus_para_radianos(graus: f32) -> f32 {
    graus * std::f32::consts::PI / 180.0
}

/// Converte radianos em graus.
pub fn radianos_para_graus(radianos: f32) -> f32 {
    radianos * 180.0 / std::f32::consts::PI
}

/// Traz um ângulo para `(-PI, PI]`.
///
/// O yaw da câmera é acumulado a cada quadro; sem reduzir, girar o mouse por dez
/// minutos deixa o ângulo em milhares de radianos e a precisão do `f32` começa a
/// aparecer como tremida. Reduzir por `TAU` não muda a direção e mantém o número
/// pequeno.
pub fn normalizar_angulo(angulo: f32) -> f32 {
    if !angulo.is_finite() {
        return 0.0;
    }

    const TAU: f32 = std::f32::consts::TAU;
    let mut reduzido = angulo % TAU;
    if reduzido <= -std::f32::consts::PI {
        reduzido += TAU;
    } else if reduzido > std::f32::consts::PI {
        reduzido -= TAU;
    }
    reduzido
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_prende_nos_dois_lados() {
        assert_eq!(clamp(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp(15.0, 0.0, 10.0), 10.0);
        assert_eq!(clamp(5.0, 0.0, 10.0), 5.0);
    }

    #[test]
    fn clamp_trata_nan_como_o_limite_inferior() {
        // `f32::clamp` propaga NaN; a câmera não pode. Ver a doc da função.
        assert_eq!(clamp(f32::NAN, 5.0, 15.0), 5.0);
    }

    #[test]
    fn lerp_acerta_as_pontas_e_o_meio() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
    }

    #[test]
    fn graus_e_radianos_ida_e_volta() {
        for graus in [-180.0, -60.0, 0.0, 45.0, 80.0, 180.0] {
            let voltou = radianos_para_graus(graus_para_radianos(graus));
            assert!(
                (voltou - graus).abs() < 1e-4,
                "{graus} virou {voltou} na ida e volta"
            );
        }
    }

    #[test]
    fn normalizar_angulo_cabe_em_menos_um_pi_a_pi() {
        let casos = [
            0.0,
            3.0,
            std::f32::consts::PI,
            -std::f32::consts::PI,
            std::f32::consts::TAU,
            -std::f32::consts::TAU,
            100.0,
            -100.0,
        ];
        for angulo in casos {
            let n = normalizar_angulo(angulo);
            assert!(
                n > -std::f32::consts::PI - 1e-5 && n <= std::f32::consts::PI + 1e-5,
                "{angulo} normalizou para {n}, fora de (-PI, PI]"
            );
        }
    }

    #[test]
    fn normalizar_angulo_nao_muda_a_direcao() {
        // Reduzir por TAU é a mesma direção: o seno e o cosseno têm de bater.
        let angulo = 7.5_f32;
        let n = normalizar_angulo(angulo);
        assert!((angulo.sin() - n.sin()).abs() < 1e-4);
        assert!((angulo.cos() - n.cos()).abs() < 1e-4);
    }

    #[test]
    fn normalizar_angulo_nao_aceita_infinito() {
        assert_eq!(normalizar_angulo(f32::INFINITY), 0.0);
        assert_eq!(normalizar_angulo(f32::NEG_INFINITY), 0.0);
        assert_eq!(normalizar_angulo(f32::NAN), 0.0);
    }
}
