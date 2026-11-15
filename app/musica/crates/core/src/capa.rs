//! Capa desenhada a partir do nome da faixa.
//!
//! Não há arquivo de imagem: a capa é **gerada** do nome. Duas razões, e as duas
//! já foram medidas no catálogo da loja (`docs/loja.md`):
//!
//! 1. Música do usuário não traz capa embutida em todo formato, e ler ID3/FLAC
//!    picture exigiria um decodificador de metadados por formato.
//! 2. Um hash determinístico dá uma cor estável por faixa, e a mesma faixa tem
//!    sempre a mesma cara — diferente de sorteio, que muda a cada abertura.
//!
//! O que a medição ensinou e está codificado aqui: **a matiz sozinha colide.**
//! São 360 valores para qualquer quantidade de faixas, então dois nomes na mesma
//! matiz acontece de verdade (a loja mediu dois apps em 12 itens). Por isso a
//! identidade da capa é o conjunto (matiz, detalhe, faixa), e há um teste que
//! exige unicidade do conjunto e outro que **documenta** a colisão da matiz
//! sozinha para ninguém "consertar" o que não é defeito.

/// FNV-1a de 64 bits.
///
/// Não é `DefaultHasher`: o hash padrão do Rust não é estável entre versões do
/// compilador, e aí a cor da capa mudaria de uma build para outra sem nenhuma
/// mudança no app.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Identidade visual de uma capa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capa {
    /// Matiz do fundo, em graus (0 a 359).
    pub matiz: u16,
    /// Matiz do detalhe (a faixa e o círculo), em graus.
    pub matiz_detalhe: u16,
    /// Deslocamento vertical da faixa, em porcentagem da altura.
    pub faixa: u8,
    /// Quantas listras de detalhe desenhar.
    pub listras: u8,
}

/// Deriva a capa do nome do arquivo.
pub fn capa_de(nome: &str) -> Capa {
    let h = fnv1a64(nome.as_bytes());
    Capa {
        matiz: (h % 360) as u16,
        matiz_detalhe: ((h >> 9) % 360) as u16,
        faixa: ((h >> 18) % 60) as u8 + 20,
        listras: ((h >> 27) % 4) as u8 + 2,
    }
}

/// Converte matiz HSL em RGB, com saturação e luminosidade fixas.
///
/// A luminosidade fica abaixo do meio de propósito: capa escura aceita texto
/// claro em qualquer matiz, e a UI não precisa de contorno no rótulo.
pub fn rgb_de_matiz(matiz: u16) -> (u8, u8, u8) {
    rgb_hsl(matiz, 0.58, 0.42)
}

/// HSL → RGB, com `t` (matiz) em graus.
pub fn rgb_hsl(matiz: u16, saturacao: f32, luz: f32) -> (u8, u8, u8) {
    let h = (f32::from(matiz % 360)) / 360.0;
    let s = saturacao.clamp(0.0, 1.0);
    let l = luz.clamp(0.0, 1.0);

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let canal = |t: f32| -> u8 {
        let t = if t < 0.0 { t + 1.0 } else { t };
        let t = if t > 1.0 { t - 1.0 } else { t };
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };

    (
        canal(h + 1.0 / 3.0),
        canal(h),
        canal(h - 1.0 / 3.0),
    )
}

/// Luminância relativa (ITU-R BT.601), em 0,0 a 1,0.
pub fn luminancia(rgb: (u8, u8, u8)) -> f32 {
    (0.299 * f32::from(rgb.0) + 0.587 * f32::from(rgb.1) + 0.114 * f32::from(rgb.2))
        / 255.0
}

/// Cor do texto sobre a capa: preto sobre fundo claro, branco sobre escuro.
///
/// Sem isto, as matizes amarelas (que passam de 0,5 de luminância mesmo com
/// luminosidade 0,42) ficariam com texto branco ilegível — e a cor da capa é
/// derivada do nome, então a legibilidade não pode depender da sorte do nome.
pub fn cor_do_texto(matiz: u16) -> (u8, u8, u8) {
    if luminancia(rgb_de_matiz(matiz)) > 0.46 {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_tem_os_vetores_conhecidos() {
        // Valores publicados da FNV-1a 64: se estes mudarem, a cor de todas as
        // capas muda junto — o teste é o que impede a troca silenciosa.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn capa_e_deterministica() {
        let a = capa_de("Faixa 1.mp3");
        let b = capa_de("Faixa 1.mp3");
        assert_eq!(a, b);
        assert_ne!(capa_de("Faixa 1.mp3"), capa_de("Faixa 2.mp3"));
    }

    #[test]
    fn campos_da_capa_ficam_nos_limites() {
        for nome in ["", "a", "Música", "10.000 Maniacs - Candy.mp3", "日本"] {
            let c = capa_de(nome);
            assert!(c.matiz < 360);
            assert!(c.matiz_detalhe < 360);
            assert!((20..80).contains(&c.faixa), "faixa {}", c.faixa);
            assert!((2..=5).contains(&c.listras), "listras {}", c.listras);
        }
    }

    #[test]
    fn conjunto_identifica_capas_que_a_matiz_sozinha_nao_identifica() {
        // O conjunto tem de ser único para nomes distintos: é ele que diferencia
        // duas faixas na tela.
        let nomes: Vec<String> = (0..40).map(|i| format!("Faixa {i:02} - Nome {}", i * 7)).collect();
        let mut conjuntos: Vec<(u16, u16, u8, u8)> = nomes.iter().map(|n| {
            let c = capa_de(n);
            (c.matiz, c.matiz_detalhe, c.faixa, c.listras)
        }).collect();
        conjuntos.sort_unstable();
        let antes = conjuntos.len();
        conjuntos.dedup();
        assert_eq!(conjuntos.len(), antes, "duas faixas com a mesma capa");
    }

    #[test]
    fn a_matiz_sozinha_colide_e_isso_e_esperado() {
        // Documenta a limitação: 360 matizes não identificam N faixas. Se algum
        // dia este teste falhar porque a lista passou a não colidir, o comentário
        // da loja deixou de valer — e a unicidade do conjunto continua sendo a
        // garantia de verdade.
        let nomes: Vec<String> = (0..40).map(|i| format!("Faixa {i:02} - Nome {}", i * 7)).collect();
        let mut matizes: Vec<u16> = nomes.iter().map(|n| capa_de(n).matiz).collect();
        matizes.sort_unstable();
        let antes = matizes.len();
        matizes.dedup();
        assert!(
            matizes.len() < antes,
            "a matiz passou a ser única nesta amostra; reveja o comentário"
        );
    }

    #[test]
    fn matiz_vira_rgb_na_faixa_certa() {
        // Vermelho, verde e azul: a conversão HSL→RGB é testada pelo que ela
        // promete, não pelos números que saíram da primeira tentativa.
        let (r, g, b) = rgb_de_matiz(0);
        assert!(r > 150 && g < 90 && b < 90, "0° = {r},{g},{b}");
        let (r, g, b) = rgb_de_matiz(120);
        assert!(g > 150 && r < 90 && b < 90, "120° = {r},{g},{b}");
        let (r, g, b) = rgb_de_matiz(240);
        assert!(b > 150 && r < 90 && g < 90, "240° = {r},{g},{b}");
    }

    #[test]
    fn toda_matiz_produz_cor_valida() {
        for matiz in 0..360u16 {
            let (r, g, b) = rgb_de_matiz(matiz);
            let l = luminancia((r, g, b));
            assert!((0.0..=1.0).contains(&l), "matiz {matiz} deu luminância {l}");
            // A capa nunca é branca nem preta pura: seria indistinguível do fundo
            // do app ou do texto.
            assert!(!(r == 0 && g == 0 && b == 0), "matiz {matiz} preta");
            assert!(!(r == 255 && g == 255 && b == 255), "matiz {matiz} branca");
        }
    }

    #[test]
    fn matiz_fora_do_intervalo_da_a_volta() {
        assert_eq!(rgb_de_matiz(0), rgb_de_matiz(360));
        assert_eq!(rgb_de_matiz(30), rgb_de_matiz(390));
    }

    #[test]
    fn texto_contrasta_com_a_capa() {
        // Amarelo é o caso que quebra a regra ingênua "texto sempre branco".
        assert_eq!(cor_do_texto(55), (0, 0, 0));
        assert_eq!(cor_do_texto(240), (255, 255, 255));
        for matiz in 0..360u16 {
            let cor = cor_do_texto(matiz);
            let fundo = luminancia(rgb_de_matiz(matiz));
            let claro = luminancia(cor);
            assert!(
                (fundo - claro).abs() > 0.3,
                "matiz {matiz}: contraste {fundo:.2} x {claro:.2}"
            );
        }
    }
}
