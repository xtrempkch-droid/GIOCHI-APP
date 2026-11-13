//! Fonte de 5×7 **em dados**, não em arquivo.
//!
//! # Por que não uma fonte de verdade
//!
//! A regra deste repositório para jogos é usar só o SDL2 e a GIOCHI-ENGINE-3D, e
//! nenhum dos dois desenha glifo. Uma fonte em arquivo seria mais um item para
//! empacotar, mais um caminho para faltar na imagem (o R35S já apareceu sem ela)
//! e mais uma licença para discutir.
//!
//! Uma tabela de 5×7 bits resolve tudo: nítida em qualquer resolução — cada ponto
//! vira um retângulo do tamanho que se quiser —, zero dependência e zero licença.
//! O `neon-drift` e o `giochi-defender` chegaram à mesma conclusão pelo mesmo
//! caminho.
//!
//! # Caixa alta é o desenho, não uma limitação
//!
//! A tabela só tem maiúsculas, dígitos e a pontuação que a interface usa.
//! [`normalizar`] dobra para caixa alta e resolve acento — e é **explícita**
//! nisso: um `ç` vira `C` porque o desenho é esse, não porque o glifo sumiu. A
//! alternativa (glifos minúsculos) dobraria a tabela para ganhar um texto que
//! ninguém lê melhor num painel de 3,5".
//!
//! # O que está aqui
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

/// Devolve o glifo de `c`, ou `None` quando o caractere não existe na tabela.
///
/// Minúsculas **não** são dobradas aqui em silêncio: quem chama usa
/// [`normalizar`] primeiro. Uma dobra escondida neste ponto faria a largura
/// calculada em [`largura_texto`] discordar do que foi desenhado, porque lá o
/// texto já passou pela normalização.
pub fn glifo(c: char) -> Option<Glifo> {
    Some(match c {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        ' ' => [0; 7],
        '/' => [
            0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000,
        ],
        '%' => [
            0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100, 0b01000,
        ],
        '[' => [
            0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
        ],
        ']' => [
            0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '#' => [
            0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010,
        ],
        '\'' => [
            0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '*' => [
            0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000,
        ],
        // O Endless mostra `7/∞`; sem este glifo o rótulo da onda viraria
        // `7/#`, que é lixo visível no canto da tela toda partida.
        '∞' => [
            0b00000, 0b00000, 0b11011, 0b10101, 0b11011, 0b00000, 0b00000,
        ],
        _ => return None,
    })
}

/// Dobra um texto para o que a tabela sabe desenhar.
///
/// Maiúsculas, acento resolvido para a letra base, e os separadores tipográficos
/// que o GDD usa (`—`, `·`) virando os equivalentes ASCII. Caractere
/// desconhecido vira **espaço** — ele ocupa lugar e não desenha, então uma
/// palavra com acento perdido não cola nas vizinhas.
///
/// É idempotente: normalizar duas vezes dá o mesmo texto.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c {
            'a'..='z' => c.to_ascii_uppercase(),
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'ç' | 'Ç' => 'C',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'ñ' | 'Ñ' => 'N',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'ý' | 'ÿ' | 'Ý' => 'Y',
            // Travessão e meia-risca do GDD, e o ponto médio dos separadores.
            '—' | '–' | '−' => '-',
            '·' | '•' => ':',
            // Aspas tipográficas: `"` existe na tabela, as curvas não.
            '\u{201c}' | '\u{201d}' | '"' => '\'',
            c if glifo(c).is_some() => c,
            _ => ' ',
        })
        .collect()
}

/// Os pontos acesos de `texto`, em coordenadas de glifo.
///
/// A origem é o canto superior esquerdo do primeiro glifo. Texto que não passou
/// por [`normalizar`] tem os caracteres fora da tabela ignorados, mas **avança**:
/// o lugar é reservado do mesmo jeito.
pub fn pontos(texto: &str) -> Vec<[i32; 2]> {
    let mut saida = Vec::new();
    for (indice, c) in texto.chars().enumerate() {
        let Some(glifo) = glifo(c) else {
            continue;
        };
        let base_x = indice as i32 * AVANCO;
        for (linha, bits) in glifo.iter().enumerate() {
            for coluna in 0..LARGURA {
                if bits & (1 << (LARGURA - 1 - coluna)) != 0 {
                    saida.push([base_x + coluna, linha as i32]);
                }
            }
        }
    }
    saida
}

/// O texto decomposto em **retângulos máximos**, em coordenadas de glifo.
///
/// Cada item é `[x, y, largura, altura]`. A cobertura é idêntica à de
/// [`pontos`] — mesma célula acesa, mesmo desenho — com **2,8× menos
/// instâncias**: 14,9 pontos por caractere viram 5,3 retângulos.
///
/// # Por que isso é dinheiro e não estilo
///
/// No R35S cada instância do HUD custa **1,47 µs** de submissão (medido: 944
/// instâncias → 4,32 ms; 4 649 → 9,75 ms). Com a loja aberta o quadro ia a
/// **29,9 fps**; a decomposição em retângulos é o que devolve a folga sem mudar
/// um pixel do desenho.
///
/// O guloso é por glifo: estende para a direita enquanto a célula está acesa e
/// livre, depois para baixo enquanto a linha inteira do retângulo couber. A
/// marcação de "livre" é o que impede dois retângulos de cobrirem a mesma
/// célula — cobertura dupla desenharia a célula duas vezes e, com alfa, mudaria
/// a cor.
pub fn retangulos(texto: &str) -> Vec<[i32; 4]> {
    let mut saida = Vec::new();
    for (indice, c) in texto.chars().enumerate() {
        let Some(glifo) = glifo(c) else {
            continue;
        };
        let base_x = indice as i32 * AVANCO;
        let mut ocupado = [[false; LARGURA as usize]; ALTURA as usize];
        for linha in 0..ALTURA {
            for coluna in 0..LARGURA {
                let acesa = glifo[linha as usize] & (1 << (LARGURA - 1 - coluna)) != 0;
                if !acesa || ocupado[linha as usize][coluna as usize] {
                    continue;
                }
                // Largura: até a primeira célula apagada ou já coberta.
                let mut largura = 0;
                while coluna + largura < LARGURA
                    && glifo[linha as usize] & (1 << (LARGURA - 1 - coluna - largura)) != 0
                    && !ocupado[linha as usize][(coluna + largura) as usize]
                {
                    largura += 1;
                }
                // Altura: enquanto a faixa inteira couber livre.
                let mut altura = 1;
                while linha + altura < ALTURA {
                    let cabe = (0..largura).all(|x| {
                        glifo[(linha + altura) as usize]
                            & (1 << (LARGURA - 1 - coluna - x))
                            != 0
                            && !ocupado[(linha + altura) as usize][(coluna + x) as usize]
                    });
                    if !cabe {
                        break;
                    }
                    altura += 1;
                }
                for y in linha..linha + altura {
                    for x in coluna..coluna + largura {
                        ocupado[y as usize][x as usize] = true;
                    }
                }
                saida.push([base_x + coluna, linha, largura, altura]);
            }
        }
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

    const ALFABETO: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 /%().,[]-:+!?#<>='*";

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
        for c in ALFABETO.chars().chain(std::iter::once('∞')) {
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
    fn minuscula_nao_e_dobrada_em_silencio_pelo_glifo() {
        assert!(glifo('a').is_none(), "a tabela é de caixa alta");
        assert_eq!(normalizar("a"), "A");
    }

    #[test]
    fn a_normalizacao_resolve_acento_e_separador() {
        assert_eq!(normalizar("Onda 3/6"), "ONDA 3/6");
        assert_eq!(normalizar("Poção de cura"), "POCAO DE CURA");
        assert_eq!(normalizar("Queen — 100%"), "QUEEN - 100%");
        assert_eq!(normalizar("HP · 45 gold"), "HP : 45 GOLD");
        assert_eq!(normalizar("7/∞"), "7/∞", "o glifo do infinito existe");
        assert_eq!(normalizar("ação"), "ACAO");
    }

    #[test]
    fn normalizar_duas_vezes_da_o_mesmo_texto() {
        for texto in ["Poção de cura", "Onda 3/6 — 03:58", "Upgrade HP torre", "7/∞"] {
            let uma = normalizar(texto);
            assert_eq!(normalizar(&uma), uma, "{texto:?}");
        }
    }

    #[test]
    fn caractere_desconhecido_ainda_avanca_o_cursor() {
        // Um acento não resolvido não pode colar as letras seguintes.
        let com_buraco = pontos("A\u{20ac}B");
        let esperado = pontos("A B");
        assert_eq!(com_buraco, esperado, "o símbolo tinha de ocupar o lugar");
    }

    #[test]
    fn a_largura_desconta_o_espaco_do_fim() {
        assert_eq!(largura_texto(""), 0);
        assert_eq!(largura_texto("A"), LARGURA);
        assert_eq!(largura_texto("AB"), 2 * AVANCO - ESPACO);
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
            assert!(!vistos.contains(&glifo), "dígito {d} repetido");
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
    fn os_retangulos_cobrem_exatamente_os_pontos() {
        // A propriedade que faz a decomposicao ser segura: mesma celula acesa,
        // nem uma a mais nem uma a menos. Uma celula faltando some do desenho;
        // uma sobrando pinta duas vezes e, com alfa, muda a cor.
        for texto in ["ABCDEFGHIJKLMNOPQRSTUVWXYZ", "0123456789", "Onda 3/6 — 03:58"] {
            let normalizado = normalizar(texto);
            let mut cobertos = std::collections::BTreeSet::new();
            for [x, y, w, h] in retangulos(&normalizado) {
                for dy in 0..h {
                    for dx in 0..w {
                        assert!(
                            cobertos.insert((x + dx, y + dy)),
                            "celula ({}, {}) coberta duas vezes em {texto:?}",
                            x + dx,
                            y + dy
                        );
                    }
                }
            }
            let pontos: std::collections::BTreeSet<(i32, i32)> = self::pontos(&normalizado)
                .into_iter()
                .map(|[x, y]| (x, y))
                .collect();
            assert_eq!(cobertos, pontos, "cobertura diferente dos pontos em {texto:?}");
        }
    }

    #[test]
    fn os_retangulos_sao_menos_de_metade_dos_pontos() {
        // O numero que paga a otimizacao: 14,9 pontos por caractere viram 5,3
        // retangulos. Se a decomposicao regredir para "um retangulo por ponto",
        // o custo no aparelho volta a 30 fps com a loja aberta.
        let texto = "RIFLE MELHORADO         150 GOLD  [OK]";
        let pontos = pontos_acesos(texto);
        let retangulos = retangulos(texto).len();
        assert!(
            retangulos * 2 < pontos,
            "{retangulos} retangulos para {pontos} pontos nao e menos da metade"
        );
    }

    #[test]
    fn o_numero_de_pontos_cresce_com_o_texto() {
        assert!(pontos_acesos("1") < pontos_acesos("11"));
        assert_eq!(pontos_acesos("11"), 2 * pontos_acesos("1"));
    }
}
