//! Sorteio determinístico (xorshift64*).
//!
//! ## Por que não `rand`
//!
//! `rand` traz uma dependência, uma semente do sistema e a promessa de qualidade
//! estatística que este jogo não precisa. O requisito aqui é o oposto: a **mesma**
//! sequência em qualquer máquina, sempre. É o que permite comparar duas capturas
//! de quadro e afirmar que a diferença é do código, não do sorteio.
//!
//! O estado é de 64 bits, o período é grande o bastante para uma partida e o
//! custo é de três `xor` e uma multiplicação por número.

/// Gerador determinístico com semente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sorteio {
    estado: u64,
}

impl Sorteio {
    /// Cria o gerador. Semente `0` é trocada: é o único estado absorvente do
    /// xorshift, e um gerador preso em zero devolveria sempre o mesmo número.
    pub fn new(semente: u64) -> Self {
        Self {
            estado: if semente == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                semente
            },
        }
    }

    /// Próximo número bruto de 64 bits.
    pub fn proximo(&mut self) -> u64 {
        let mut x = self.estado;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.estado = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Número em `[min, max)`.
    pub fn faixa(&mut self, min: f32, max: f32) -> f32 {
        // 24 bits: o suficiente para um `f32` em `[0, 1)` sem perder resolução e
        // sem o viés que `as f32` sobre 64 bits produziria.
        let passo = (self.proximo() >> 40) as f32 / (1u32 << 24) as f32;
        min + (max - min) * passo
    }

    /// Ponto uniforme na esfera unitária.
    ///
    /// `z` uniforme e ângulo uniforme: é o método sem rejeição e sem concentrar
    /// pontos nos polos (que é o que acontece ao normalizar três uniformes).
    pub fn direcao_unitaria(&mut self) -> [f32; 3] {
        let z = self.faixa(-1.0, 1.0);
        let angulo = self.faixa(0.0, std::f32::consts::TAU);
        let anel = (1.0 - z * z).max(0.0).sqrt();
        [anel * angulo.cos(), z, anel * angulo.sin()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mesma_semente_da_a_mesma_sequencia() {
        let mut a = Sorteio::new(42);
        let mut b = Sorteio::new(42);
        for _ in 0..100 {
            assert_eq!(a.proximo(), b.proximo());
        }
    }

    #[test]
    fn sementes_diferentes_divergem() {
        let mut a = Sorteio::new(1);
        let mut b = Sorteio::new(2);
        assert_ne!(a.proximo(), b.proximo());
    }

    #[test]
    fn a_semente_zero_nao_trava_o_gerador() {
        let mut sorteio = Sorteio::new(0);
        let primeiro = sorteio.proximo();
        let segundo = sorteio.proximo();
        assert_ne!(primeiro, segundo, "gerador preso em zero");
        assert_ne!(primeiro, 0);
    }

    #[test]
    fn a_faixa_cobre_o_intervalo_pedido() {
        let mut sorteio = Sorteio::new(7);
        let mut minimo = f32::INFINITY;
        let mut maximo = f32::NEG_INFINITY;
        for _ in 0..10_000 {
            let valor = sorteio.faixa(-1.0, 1.0);
            minimo = minimo.min(valor);
            maximo = maximo.max(valor);
        }
        assert!(minimo < -0.99, "mínimo {minimo}");
        assert!(maximo > 0.99, "máximo {maximo}");
    }

    #[test]
    fn a_faixa_nunca_sai_dos_limites() {
        let mut sorteio = Sorteio::new(99);
        for _ in 0..10_000 {
            let valor = sorteio.faixa(3.0, 4.0);
            assert!((3.0..4.0).contains(&valor), "{valor} fora de [3, 4)");
        }
    }

    #[test]
    fn a_direcao_e_unitaria_e_nao_se_concentra_nos_polos() {
        let mut sorteio = Sorteio::new(5);
        let mut no_polo = 0;
        for _ in 0..2000 {
            let [x, y, z] = sorteio.direcao_unitaria();
            let tamanho = (x * x + y * y + z * z).sqrt();
            assert!((tamanho - 1.0).abs() < 1e-4, "direção de tamanho {tamanho}");
            if z.abs() > 0.9 {
                no_polo += 1;
            }
        }
        // Numa distribuição uniforme, |z| > 0.9 é 10 % da esfera. Concentrar nos
        // polos daria muito mais; este limite é folgado de propósito.
        assert!(
            (100..300).contains(&no_polo),
            "{no_polo} de 2000 pontos na calota polar (esperado ~200)"
        );
    }
}
