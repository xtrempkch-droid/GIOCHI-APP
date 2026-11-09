//! Fonte de 5×7 em dados, não em arquivo.
//!
//! ## Por que desenhar texto com pontos em vez de carregar uma fonte
//!
//! O jogo **não pode** depender de `SDL2_ttf`: a regra deste repositório para
//! jogos é usar só o SDL2 e a engine 3D, e nenhum dos dois desenha glifo. Uma
//! fonte em arquivo seria mais um item para empacotar, mais um caminho para
//! faltar na imagem, e mais uma licença para discutir.
//!
//! Uma fonte de 5×7 bits resolve tudo: nítida em qualquer resolução (cada ponto
//! vira um retângulo do tamanho que se quiser), zero dependência e zero licença.
//! A cobrinha já desenha o placar em sete segmentos pelo mesmo motivo.
//!
//! ## O que está aqui
//!
//! A tabela em si e funções puras — nenhuma chamada de desenho. Isso permite
//! testar "a palavra cabe na largura X" sem abrir janela.

/// Uma letra: 7 linhas de 5 bits, da esquerda para a direita, de cima para baixo.
///
/// O bit 4 é a coluna 0. `0b11111` é uma barra cheia; `0b10001` são as duas
/// pontas de cima de um `U`.
pub type Glifo = [u8; 7];

/// Altura de um glifo, em pontos.
pub const ALTURA: i32 = 7;
/// Largura de um glifo, em pontos.
pub const LARGURA: i32 = 5;
/// Espaço entre glifos, em pontos.
pub const ESPACO: i32 = 1;
/// Avanço de um glifo: largura + espaço. O último glifo também avança, e quem
/// mede o texto desconta esse espaço no fim (`largura_texto`).
pub const AVANCO: i32 = LARGURA + ESPACO;

const fn g(linhas: [u8; 7]) -> Glifo {
    linhas
}

/// Devolve o glifo de `c`, ou `None` quando o caractere não existe na tabela.
///
/// Maiúsculas, dígitos e pontuação de HUD. Minúsculas **não** são dobradas para
/// maiúsculas em silêncio: isso esconderia erro de digitação no texto da tela, e
/// o HUD deste jogo é todo em caixa alta de propósito.
pub fn glifo(c: char) -> Option<Glifo> {
    Some(match c {
        'A' => g([0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'C' => g([0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => g([0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
        'E' => g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => g([0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => g([0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
        'H' => g([0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => g([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111]),
        'J' => g([0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
        'K' => g([0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => g([0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => g([0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => g([0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => g([0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => g([0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
        'R' => g([0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => g([0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
        'T' => g([0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => g([0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'W' => g([0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010]),
        'X' => g([0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => g([0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '0' => g([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => g([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
        '3' => g([0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
        '4' => g([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => g([0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => g([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => g([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => g([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => g([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
        ':' => g([0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000]),
        '.' => g([0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100]),
        ',' => g([0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000]),
        '-' => g([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '+' => g([0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
        '/' => g([0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000]),
        '%' => g([0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011]),
        '!' => g([0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
        '?' => g([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
        '\'' => g([0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
        '(' => g([0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
        ')' => g([0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
        'x' => g([0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001]),
        ' ' => g([0; 7]),
        '—' => g([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        _ => return None,
    })
}

/// Largura, em pontos, de um texto de uma linha.
///
/// O espaço entre glifos fica **entre** eles: o último não conta o seu. Um texto
/// vazio mede `0`, e não `-ESPACO`.
pub fn largura_texto(texto: &str) -> i32 {
    let n = texto.chars().count() as i32;
    if n == 0 {
        return 0;
    }
    n * AVANCO - ESPACO
}

/// Pontos acesos do texto, em coordenadas de ponto (x para a direita, y para
/// baixo), com a origem no canto superior esquerdo do primeiro glifo.
///
/// Caractere desconhecido vira espaço em vez de erro: um HUD que falha todo por
/// causa de um acento seria pior que um HUD com um buraco.
pub fn pontos(texto: &str) -> Vec<[i32; 2]> {
    let mut saida = Vec::with_capacity(texto.len() * 12);
    let mut cursor = 0;
    for c in texto.chars() {
        let Some(glifo) = glifo(c) else {
            cursor += AVANCO;
            continue;
        };
        for (linha, bits) in glifo.iter().enumerate() {
            for coluna in 0..LARGURA {
                let aceso = bits & (1 << (LARGURA - 1 - coluna)) != 0;
                if aceso {
                    saida.push([cursor + coluna, linha as i32]);
                }
            }
        }
        cursor += AVANCO;
    }
    saida
}

/// Quantos pontos acesos um texto tem. Serve de medida barata em teste.
pub fn pontos_acesos(texto: &str) -> usize {
    pontos(texto).len()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn glifos_conhecidos_tem_a_altura_certa() {
        for c in "ABCXYZ0123456789:.-/%!".chars() {
            let gl = glifo(c).unwrap_or_else(|| panic!("glifo ausente para {c:?}"));
            assert_eq!(gl.len(), ALTURA as usize, "altura de {c:?}");
        }
    }

    #[test]
    fn minuscula_nao_e_dobrada_em_silencio() {
        assert!(glifo('a').is_none(), "só 'x' minúsculo existe, como sinal de vezes");
        assert!(glifo('x').is_some());
        assert!(glifo('ç').is_none());
    }

    #[test]
    fn largura_desconta_o_espaco_do_fim() {
        assert_eq!(largura_texto(""), 0);
        assert_eq!(largura_texto("A"), LARGURA);
        assert_eq!(largura_texto("AB"), LARGURA * 2 + ESPACO);
        assert_eq!(largura_texto("ABC"), LARGURA * 3 + ESPACO * 2);
    }

    #[test]
    fn pontos_ficam_dentro_da_caixa_do_texto() {
        let texto = "LAP 1/2";
        let largura = largura_texto(texto);
        for [x, y] in pontos(texto) {
            assert!((0..largura).contains(&x), "x={x} fora de 0..{largura}");
            assert!((0..ALTURA).contains(&y), "y={y} fora de 0..{ALTURA}");
        }
    }

    #[test]
    fn espaco_nao_acende_nada_e_nao_e_erro() {
        assert_eq!(pontos_acesos("   "), 0);
        assert_eq!(largura_texto("   "), AVANCO * 3 - ESPACO);
    }

    #[test]
    fn caractere_desconhecido_vira_espaco() {
        let com_acento = pontos("AÇÃO");
        let com_espacos = pontos("A  O");
        assert_eq!(
            com_acento, com_espacos,
            "o desconhecido avança o cursor em vez de sumir com o glifo seguinte"
        );
    }

    #[test]
    fn as_letras_do_hud_existem() {
        // Toda palavra que o jogo desenha precisa estar na tabela — faltar um
        // glifo é um buraco no meio da tela, e nada acusaria.
        let textos = [
            "NEON DRIFT",
            "JOGAR",
            "REINICIAR",
            "DRIFT",
            "NITRO",
            "VOLTA",
            "POS",
            "VITORIA",
            "DERROTA",
            "TEMPO",
            "PONTOS",
            "MARCHA",
            "WASD OU SETAS",
            "ESPACO FRENAGEM",
            "SHIFT NITRO",
            "ENTER PARA COMECAR",
        ];
        for texto in textos {
            let faltando: Vec<char> = texto.chars().filter(|c| glifo(*c).is_none()).collect();
            assert!(faltando.is_empty(), "faltam glifos em {texto:?}: {faltando:?}");
        }
    }

    #[test]
    fn cada_letra_e_diferente_da_outra() {
        // Um glifo copiado por engano produz texto legível e errado — o tipo de
        // defeito que ninguém vê na tela.
        let mut vistos: Vec<(char, Glifo)> = Vec::new();
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars() {
            let gl = glifo(c).unwrap();
            if let Some((outro, _)) = vistos.iter().find(|(_, g)| *g == gl) {
                // `O` e `0` são propositalmente distintos; igualdade é bug.
                panic!("{c:?} tem o mesmo glifo de {outro:?}");
            }
            vistos.push((c, gl));
        }
    }

    #[test]
    fn letra_vazia_seria_invisivel() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars() {
            let gl = glifo(c).unwrap();
            assert!(
                gl.iter().any(|linha| *linha != 0),
                "{c:?} está em branco e não desenharia nada"
            );
        }
    }
}
