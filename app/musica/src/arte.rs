//! Arte gerada: a capa de cada faixa, em pixels RGBA.
//!
//! Não há PNG por faixa. A capa é pintada aqui a partir dos quatro números que
//! [`giochi_musica_core::capa`] derivou do nome — mesma cor para a mesma faixa,
//! sempre. Isso evita ler ID3/FLAC picture (um decodificador por formato) e
//! evita um asset por música no pacote.
//!
//! O desenho é **puro**: entra [`Capa`], sai `Vec<u8>` RGBA. Sem SDL, sem
//! arquivo, testável.

use giochi_musica_core::capa::{rgb_de_matiz, rgb_hsl, Capa};

/// Lado da textura de capa, em pixels.
///
/// 96 px e não 256: a capa é desenhada com no máximo ~200 px na tela, e a
/// diferença visual acima de 96 px desaparece no painel de 640×480. Cada textura
/// ocupa 96×96×4 = 36 KB; 256 px ocuparia 256 KB — sete vezes mais memória de
/// vídeo por capa em cache.
pub const LADO: u32 = 96;

/// Pinta a capa e devolve RGBA com `LADO * LADO * 4` bytes.
pub fn pintar(capa: &Capa) -> Vec<u8> {
    let lado = LADO as i32;
    let mut px = vec![0u8; (lado * lado * 4) as usize];

    let base = rgb_de_matiz(capa.matiz);
    let detalhe = rgb_de_matiz(capa.matiz_detalhe);
    let claro = rgb_hsl(capa.matiz, 0.55, 0.62);

    // Mistura sobre o que já está pintado e mantém o alvo opaco. Escrever o
    // alfa da pincelada no pixel deixava a capa translúcida — o fundo do app
    // aparecia através dela, e o resultado parecia "desbotado", não sutil.
    let mut por = |x: i32, y: i32, cor: (u8, u8, u8), alfa: u8| {
        if x < 0 || y < 0 || x >= lado || y >= lado {
            return;
        }
        let i = ((y * lado + x) * 4) as usize;
        let a = u32::from(alfa);
        let inv = 255 - a;
        px[i] = ((u32::from(cor.0) * a + u32::from(px[i]) * inv) / 255) as u8;
        px[i + 1] = ((u32::from(cor.1) * a + u32::from(px[i + 1]) * inv) / 255) as u8;
        px[i + 2] = ((u32::from(cor.2) * a + u32::from(px[i + 2]) * inv) / 255) as u8;
        px[i + 3] = 255;
    };

    for y in 0..lado {
        for x in 0..lado {
            por(x, y, base, 255);
        }
    }

    // Faixa diagonal clara: dá um ponto de referência à capa sem texto.
    let inicio = (capa.faixa as i32 % 80) + 4;
    for y in 0..lado {
        let deslocamento = (lado - y) / 3;
        for x in 0..lado {
            let d = (x - deslocamento).rem_euclid(lado);
            if (inicio * lado / 100) <= d && d < (inicio * lado / 100) + 6 {
                por(x, y, claro, 150);
            }
        }
    }

    // Listras verticais na cor de detalhe, do lado direito.
    let passo = lado / (capa.listras as i32 + 2);
    for n in 0..capa.listras as i32 {
        let x = lado - 8 - n * passo;
        for y in lado / 8..lado - lado / 8 {
            por(x, y, detalhe, 190);
            por(x + 1, y, detalhe, 110);
        }
    }

    // Círculo central (o "disco"), contornado.
    let (cx, cy, r) = (lado / 2, lado / 2, lado / 4);
    for y in 0..lado {
        for x in 0..lado {
            let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            let r2 = r * r;
            if (d2 - r2).abs() < r * 2 {
                por(x, y, detalhe, 220);
            } else if d2 < r2 / 9 {
                por(x, y, claro, 230);
            }
        }
    }

    // Moldura: separa a capa do fundo, mesmo quando a matiz é próxima.
    for x in 0..lado {
        por(x, 0, claro, 255);
        por(x, lado - 1, claro, 255);
    }
    for y in 0..lado {
        por(0, y, claro, 255);
        por(lado - 1, y, claro, 255);
    }

    px
}

/// Altura de uma barra do equalizador, em pixels.
///
/// Animado por **contagem de quadros**, nunca pelo relógio: o desenho é
/// determinístico, então um quadro capturado duas vezes sai idêntico — é o que
/// permite comparar aparelho e PC (`--dump-frame`). O mesmo vale para o Star
/// Dust, e pela mesma razão.
///
/// `quadro / 4` faz a barra mudar ~15 vezes por segundo a 60 Hz: rápido o
/// bastante para parecer som, devagar o bastante para não piscar.
pub fn altura_do_vu(quadro: u64, barra: usize, semente: u64, maximo: i32) -> i32 {
    if maximo <= 0 {
        return 0;
    }
    let mut h = semente
        .wrapping_add((quadro / 4).wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add((barra as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 29;
    (h % (maximo as u64 + 1)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use giochi_musica_core::capa::capa_de;

    #[test]
    fn capa_tem_o_tamanho_rgba_esperado() {
        let px = pintar(&capa_de("Faixa 1.mp3"));
        assert_eq!(px.len(), (LADO * LADO * 4) as usize);
    }

    #[test]
    fn capa_e_opaca_em_todo_pixel() {
        // Pixel com alfa 0 vira buraco no desenho, e o fundo do app apareceria
        // dentro da capa.
        let px = pintar(&capa_de("Música.ogg"));
        for (i, valor) in px.iter().enumerate() {
            if i % 4 == 3 {
                assert_eq!(*valor, 255, "alfa do pixel {}", i / 4);
            }
        }
    }

    #[test]
    fn capas_de_faixas_diferentes_nao_sao_iguais() {
        let a = pintar(&capa_de("Faixa 1.mp3"));
        let b = pintar(&capa_de("Faixa 2.mp3"));
        assert_ne!(a, b);
    }

    #[test]
    fn a_mesma_faixa_desenha_a_mesma_capa() {
        assert_eq!(pintar(&capa_de("Faixa 1.mp3")), pintar(&capa_de("Faixa 1.mp3")));
    }

    #[test]
    fn equalizador_e_deterministico_e_limitado() {
        let a: Vec<i32> = (0..4).map(|b| altura_do_vu(120, b, 42, 8)).collect();
        let b: Vec<i32> = (0..4).map(|b| altura_do_vu(120, b, 42, 8)).collect();
        assert_eq!(a, b, "duas chamadas com a mesma entrada divergem");
        for q in 0..200u64 {
            for barra in 0..4 {
                let h = altura_do_vu(q, barra, 7, 8);
                assert!((0..=8).contains(&h), "altura {h} fora da faixa");
            }
        }
        assert_eq!(altura_do_vu(0, 0, 1, 0), 0);
    }

    #[test]
    fn equalizador_muda_com_o_tempo() {
        // Um equalizador parado não comunica som nenhum; se todas as alturas de
        // todos os quadros fossem iguais, este teste reprovaria.
        let mut alturas = std::collections::HashSet::new();
        for q in 0..40u64 {
            alturas.insert(altura_do_vu(q, 0, 3, 8));
        }
        assert!(alturas.len() > 2, "o equalizador praticamente não se move");
    }

    #[test]
    fn moldura_aparece_em_todas_as_bordas() {
        let px = pintar(&capa_de("x.wav"));
        let lado = LADO as usize;
        let pixel = |x: usize, y: usize| {
            let i = (y * lado + x) * 4;
            (px[i], px[i + 1], px[i + 2])
        };
        // A moldura é a mesma nos quatro cantos: é ela que separa a capa do fundo.
        let canto = pixel(0, 0);
        assert_eq!(pixel(lado - 1, 0), canto);
        assert_eq!(pixel(0, lado - 1), canto);
        assert_eq!(pixel(lado - 1, lado - 1), canto);
    }
}
