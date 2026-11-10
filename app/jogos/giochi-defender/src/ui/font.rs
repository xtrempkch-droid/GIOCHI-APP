//! Fonte de 5×7 **em dados**, não em arquivo.
//!
//! ## Por que desenhar texto com pontos em vez de carregar uma fonte
//!
//! O jogo não pode depender de `SDL2_ttf`: a regra deste repositório para jogos
//! é usar só o SDL2 e a engine 3D, e nenhum dos dois desenha glifo. Uma fonte em
//! arquivo seria mais um item para empacotar, mais um caminho para faltar na
//! imagem (o R35S já apareceu sem ela) e mais uma licença para discutir.
//!
//! Uma fonte de 5×7 bits resolve tudo: nítida em qualquer resolução — cada ponto
//! vira um retângulo do tamanho que se quiser — zero dependência e zero licença.
//! O `neon-drift` chegou à mesma conclusão pelo mesmo caminho.
//!
//! ## O que está aqui
//!
//! Só a tabela e funções puras. Nenhuma chamada de desenho: é o que permite
//! testar "o texto cabe na tela" sem abrir janela.

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
/// Avanço de um glifo: largura mais o espaço.
pub const AVANCO: i32 = LARGURA + ESPACO;

const fn g(linhas: [u8; 7]) -> Glifo {
    linhas
}

/// Devolve o glifo de `c`, ou `None` quando o caractere não existe na tabela.
///
/// Caixa alta, dígitos e a pontuação que o HUD usa. Minúsculas **não** são
/// dobradas em silêncio: isso esconderia erro de digitação no texto da tela, e
/// a HUD deste jogo é toda em caixa alta de propósito. A única exceção é `x`,
/// que existe como sinal de multiplicação.
pub fn glifo(c: char) -> Option<Glifo> {
    Some(match c {
        'A' => g([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'B' => g([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        'C' => g([
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ]),
        'D' => g([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        'E' => g([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        'F' => g([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'G' => g([
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ]),
        'H' => g([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        'I' => g([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ]),
        'J' => g([
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ]),
        'K' => g([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        'L' => g([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        'M' => g([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        'N' => g([
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ]),
        'O' => g([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'P' => g([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        'Q' => g([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        'R' => g([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        'S' => g([
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        'T' => g([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'U' => g([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        'V' => g([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        'W' => g([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ]),
        'X' => g([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        'Y' => g([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        'Z' => g([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        '0' => g([
            0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        '1' => g([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        '2' => g([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        '3' => g([
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ]),
        '4' => g([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        '5' => g([
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ]),
        '6' => g([
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        '7' => g([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        '8' => g([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        '9' => g([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ]),
        ' ' => g([0; 7]),
        ':' => g([
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ]),
        '-' => g([
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ]),
        '+' => g([
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ]),
        '.' => g([
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ]),
        '/' => g([
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ]),
        '%' => g([
            0b11001, 0b11011, 0b00010, 0b00100, 0b01000, 0b11011, 0b10011,
        ]),
        '!' => g([
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ]),
        '?' => g([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ]),
        'x' => g([
            0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001,
        ]),
        _ => return None,
    })
}

/// Células acesas de `texto`, em coordenadas de ponto (coluna, linha).
///
/// O cursor anda [`AVANCO`] por caractere, inclusive nos desconhecidos: um `ç`
/// perdido deixa um buraco do tamanho de uma letra em vez de embaralhar o resto
/// da linha.
pub fn pontos(texto: &str) -> Vec<[i32; 2]> {
    let mut saida = Vec::with_capacity(texto.len() * 12);
    let mut cursor = 0;
    for c in texto.chars() {
        if let Some(glifo) = glifo(c) {
            for (linha, bits) in glifo.iter().enumerate() {
                for coluna in 0..LARGURA {
                    let aceso = bits & (1 << (LARGURA - 1 - coluna)) != 0;
                    if aceso {
                        saida.push([cursor + coluna, linha as i32]);
                    }
                }
            }
        }
        cursor += AVANCO;
    }
    saida
}

/// Quantos pontos acesos um texto tem — medida barata para teste.
pub fn pontos_acesos(texto: &str) -> usize {
    pontos(texto).len()
}

/// Largura de `texto` em pontos, **sem** contar o espaço que sobra no fim.
pub fn largura_texto(texto: &str) -> i32 {
    if texto.is_empty() {
        return 0;
    }
    texto.chars().count() as i32 * AVANCO - ESPACO
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALFABETO: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 :.-+/%!?x";

    #[test]
    fn todo_glifo_da_tabela_tem_a_altura_certa() {
        for c in ALFABETO.chars() {
            let glifo = glifo(c).unwrap_or_else(|| panic!("glifo ausente para {c:?}"));
            assert_eq!(glifo.len(), ALTURA as usize, "altura de {c:?}");
        }
    }

    #[test]
    fn nenhum_glifo_usa_bit_fora_das_cinco_colunas() {
        // O bit 5 e acima estão fora da largura. Um deles aceso some na hora de
        // desenhar (o laço vai de 0 a LARGURA) e a letra sai faltando um pedaço,
        // sem erro nenhum.
        for c in ALFABETO.chars() {
            let glifo = glifo(c).unwrap();
            for (linha, bits) in glifo.iter().enumerate() {
                assert_eq!(
                    bits & 0b1110_0000,
                    0,
                    "{c:?} linha {linha}: bits {bits:#010b} passam das 5 colunas"
                );
            }
        }
    }

    #[test]
    fn minuscula_nao_e_dobrada_em_silencio() {
        assert!(glifo('a').is_none(), "só 'x' minúsculo existe");
        assert!(glifo('x').is_some());
        assert!(glifo('ç').is_none());
    }

    #[test]
    fn caractere_desconhecido_ainda_avanca_o_cursor() {
        // Um acento perdido não pode colar as letras seguintes.
        let com_buraco = pontos("AÇB");
        let esperado = pontos("A B");
        assert_eq!(
            com_buraco, esperado,
            "o 'Ç' tinha de ocupar o lugar de um glifo"
        );
    }

    #[test]
    fn a_largura_desconta_o_espaco_do_fim() {
        assert_eq!(largura_texto(""), 0);
        assert_eq!(largura_texto("A"), LARGURA);
        assert_eq!(largura_texto("AB"), 2 * AVANCO - ESPACO);
        // n letras ocupam n*AVANCO-ESPACO: o avanço é 6 e a largura é 5, então
        // cada letra a mais acrescenta um avanço inteiro, não uma largura.
        for n in 1..=10 {
            let texto: String = std::iter::repeat_n('A', n).collect();
            assert_eq!(largura_texto(&texto), n as i32 * AVANCO - ESPACO);
        }
    }

    #[test]
    fn o_espaco_nao_desenha_nada_mas_ocupa_lugar() {
        assert_eq!(pontos_acesos(" "), 0);
        assert_eq!(largura_texto(" "), LARGURA);
        assert_eq!(pontos_acesos("A B"), pontos_acesos("AB"));
    }

    #[test]
    fn os_dez_digitos_sao_diferentes_entre_si() {
        // Dois dígitos iguais na tabela é o erro que ninguém vê até o placar
        // mostrar 00 em vez de 60.
        let mut vistos = Vec::new();
        for d in '0'..='9' {
            let glifo = glifo(d).unwrap();
            assert!(
                !vistos.contains(&glifo),
                "dígito {d} repetido na tabela de glifos"
            );
            vistos.push(glifo);
        }
    }

    #[test]
    fn as_letras_sao_todas_distintas() {
        let mut vistos: Vec<Glifo> = Vec::new();
        for c in 'A'..='Z' {
            let glifo = glifo(c).unwrap();
            assert!(!vistos.contains(&glifo), "letra {c} repetida na tabela");
            vistos.push(glifo);
        }
    }

    #[test]
    fn os_pontos_ficam_dentro_da_caixa_do_texto() {
        let texto = "PONTOS 001200";
        let largura = largura_texto(texto);
        for [x, y] in pontos(texto) {
            assert!((0..largura).contains(&x), "ponto fora da largura: {x}");
            assert!((0..ALTURA).contains(&y), "ponto fora da altura: {y}");
        }
    }

    #[test]
    fn o_numero_de_pontos_cresce_com_o_texto() {
        assert!(pontos_acesos("1") < pontos_acesos("11"));
        assert_eq!(pontos_acesos("11"), 2 * pontos_acesos("1"));
    }
}
