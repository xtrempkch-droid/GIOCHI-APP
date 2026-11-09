//! Texturas procedurais: asfalto, concreto, rocha, mata, guarda-corpo e neon.
//!
//! ## Por que gerar em vez de carregar PNG
//!
//! Duas razões, e a segunda é a que decide:
//!
//! 1. **O aparelho não decodifica PNG.** Medido no R35S: o `libSDL2_image` da
//!    imagem foi compilado sem `--enable-png`, e `IMG_Load` responde "PNG images
//!    are not supported". Um jogo que dependesse de PNG não abriria lá.
//! 2. **Uma textura é uma fórmula.** Asfalto é ruído escuro com pedrisco;
//!    concreto são blocos com junta; mata é ruído verde em duas escalas. Isso são
//!    vinte linhas cada, e gerar custa microssegundos no boot — sem arquivo para
//!    empacotar, sem caminho para faltar no aparelho e sem binário opaco no
//!    repositório.
//!
//! É a mesma decisão dos ícones do launcher (listas de linhas em vez de PNG) e
//! das malhas do carro (aritmética em vez de `.obj`).
//!
//! ## Determinismo
//!
//! O sorteio é um xorshift com semente fixa por textura. Duas execuções geram
//! bytes idênticos, então a comparação de quadro PC × aparelho continua valendo
//! — uma textura aleatória de verdade faria cada execução desenhar diferente e
//! nenhum teste de imagem poderia existir.
//!
//! ## Tudo é cinza-claro por baixo
//!
//! A cor final no shader é `amostra × tint`. As texturas nascem **neutras**
//! (cinza com variação) e a cor vem do `tint`, que é o que permite usar a mesma
//! textura de concreto no muro azulado da avenida e na pedra da serra sem gerar
//! duas texturas.

/// Lado das texturas, em pixels. 64 é potência de dois (exigência do GLES 2.0
/// para `REPEAT`) e dá densidade suficiente para 4 m de asfalto.
pub const LADO: u32 = 64;

/// Uma textura pronta para subir para a GPU.
pub struct Textura {
    /// Nome, para o diagnóstico.
    pub nome: &'static str,
    /// Largura em pixels.
    pub largura: u32,
    /// Altura em pixels.
    pub altura: u32,
    /// RGBA8, linha a linha.
    pub pixels: Vec<u8>,
}

impl Textura {
    /// Quantos bytes tem a textura.
    pub fn bytes(&self) -> usize {
        self.pixels.len()
    }
}

/// xorshift32 com semente, para o ruído ser reprodutível.
struct Sorteio(u32);

impl Sorteio {
    fn new(semente: u32) -> Self {
        // Semente zero trava o xorshift em zero para sempre.
        Self(if semente == 0 { 0x9e37_79b9 } else { semente })
    }

    fn proximo(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// `0.0..1.0`.
    fn unidades(&mut self) -> f32 {
        (self.proximo() >> 8) as f32 / 16_777_216.0
    }

}

/// Empacota um pixel RGBA.
fn px(r: f32, g: f32, b: f32, a: f32) -> [u8; 4] {
    [
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
        (a.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

/// Asfalto: ruído escuro com pedrisco claro e uma faixa de desgaste.
///
/// A textura é **acinzentada e clara**: o escuro vem do `tint` do material. Uma
/// textura já escura, multiplicada por um tint escuro, vira preto e o asfalto
/// some — foi o que aconteceu com a primeira tentativa de paleta do jogo.
pub fn asfalto() -> Textura {
    let lado = LADO;
    let mut s = Sorteio::new(0x7f4a_1c3b);
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            // Base: cinza médio com variação fina.
            let base = 0.84 + s.unidades() * 0.09;
            // Pedrisco: pontos claros esparsos, o que dá a leitura de asfalto
            // granulado de longe.
            let grao = if s.unidades() > 0.86 {
                0.10 + s.unidades() * 0.10
            } else {
                0.0
            };
            // Trinca: uma linha escura serpenteando no eixo X. Sem ela a textura
            // repete de forma visível a cada 4 m.
            let trinca = if (y as f32 + (x as f32 * 0.6).sin() * 3.0).rem_euclid(13.0) < 1.0 {
                -0.16
            } else {
                0.0
            };
            let v = base + grao + trinca;
            pixels.extend_from_slice(&px(v, v * 1.01, v * 1.05, 1.0));
        }
    }
    Textura {
        nome: "asfalto",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Concreto: blocos com junta escura e manchas de umidade.
pub fn concreto() -> Textura {
    let lado = LADO;
    const BLOCO: u32 = 16;
    let mut s = Sorteio::new(0x2b91_55d3);
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            // Fiada alternada: a cada duas carreiras os blocos deslocam meio
            // bloco. Sem o deslocamento o muro lê como grade.
            let carreira = y / BLOCO;
            let desloc = if carreira % 2 == 0 { 0 } else { BLOCO / 2 };
            let na_junta = y % BLOCO == 0 || (x + desloc) % BLOCO == 0;
            let base = if na_junta {
                0.62
            } else {
                0.86 + s.unidades() * 0.07
            };
            // Mancha de umidade: ruído de baixa frequência (seno), porque um
            // segundo sorteio por pixel só faria chuvisco.
            let mancha = ((x as f32 * 0.14).sin() * (y as f32 * 0.11).cos()) * 0.06;
            let v = base + mancha;
            pixels.extend_from_slice(&px(v, v, v * 1.04, 1.0));
        }
    }
    Textura {
        nome: "concreto",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Rocha: escura, com veios diagonais e aresta clara.
///
/// É o material da parede de pedra da serra — e o contraste com o concreto é o
/// que faz o mapa novo ler como outro lugar, e não como a avenida com outro
/// traçado.
pub fn rocha() -> Textura {
    let lado = LADO;
    let mut s = Sorteio::new(0x51c2_9a07);
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            let base = 0.62 + s.unidades() * 0.16;
            // Veio diagonal: duas frequências somadas dão um risco que não
            // repete no olho.
            let veio = ((x as f32 * 0.5 + y as f32 * 0.9).sin() * 0.5 + 0.5).powi(6);
            let v = base + veio * 0.22;
            // Leve tom quente na rocha: pedra pura cinza lê como concreto sujo.
            pixels.extend_from_slice(&px(v * 1.05, v, v * 0.93, 1.0));
        }
    }
    Textura {
        nome: "rocha",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Mata atlântica vista de longe: manchas de verde em duas escalas.
///
/// Não é folha — é **copa**. A 10 m de distância o que se vê são massas verdes
/// com pontos de luz, e é isso que a textura entrega.
pub fn mata() -> Textura {
    let lado = LADO;
    let mut s = Sorteio::new(0x18f3_b627);
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            // Massa: seno cruzado em escala de 8 px.
            let massa = ((x as f32 * 0.8).sin() * (y as f32 * 0.72).cos()) * 0.5 + 0.5;
            // Folhagem: ruído fino por cima.
            let folha = s.unidades();
            let verde = 0.62 + massa * 0.22 + folha * 0.14;
            // Um em cada sete pixels é um ponto claro — reflexo de folha.
            let brilho = if s.unidades() > 0.86 { 0.16 } else { 0.0 };
            pixels.extend_from_slice(&px(verde * 0.42 + brilho, verde + brilho, verde * 0.40, 1.0));
        }
    }
    Textura {
        nome: "mata",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Guarda-corpo: metal com montantes verticais.
///
/// A textura é aplicada no plano da chapa, então as faixas verticais viram as
/// nervuras — e elas passando depressa são metade da sensação de velocidade na
/// serra.
pub fn guarda_corpo() -> Textura {
    let lado = LADO;
    let mut s = Sorteio::new(0x6a05_2e11);
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            // Nervura a cada 8 px, com sombra de um lado e luz do outro.
            let fase = x % 8;
            let v = match fase {
                0 => 0.52,
                1 => 0.98,
                2 => 0.90,
                7 => 0.68,
                _ => 0.82 + s.unidades() * 0.05,
            };
            // Barra horizontal no meio: o perfil do guarda-corpo.
            let v = if (y as i32 - 32).abs() < 2 { v + 0.10 } else { v };
            let _ = y;
            pixels.extend_from_slice(&px(v, v, v * 1.02, 1.0));
        }
    }
    Textura {
        nome: "guarda-corpo",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Painel de neon: faixas diagonais claras sobre fundo escuro.
///
/// A única textura com **alfa**: ela é aplicada nos outdoors e nas faixas
/// luminosas, e o alfa deixa o fundo da cena aparecer entre as faixas.
pub fn painel_neon() -> Textura {
    let lado = LADO;
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            // Faixa diagonal de 6 px a cada 16: `(x+y) mod 16 < 6`.
            let faixa = (x + y) % 16 < 6;
            if faixa {
                pixels.extend_from_slice(&px(1.0, 0.96, 0.92, 1.0));
            } else {
                // Fundo com alfa baixo: o painel "vaza" o neon em vez de ser uma
                // placa opaca no meio da noite.
                pixels.extend_from_slice(&px(0.10, 0.10, 0.14, 0.35));
            }
        }
    }
    Textura {
        nome: "painel-neon",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Xadrez de largada/chegada: 8×8 casas em preto e branco.
pub fn xadrez() -> Textura {
    let lado = LADO;
    const CASA: u32 = 8;
    let mut pixels = Vec::with_capacity((lado * lado * 4) as usize);
    for y in 0..lado {
        for x in 0..lado {
            let claro = ((x / CASA) + (y / CASA)) % 2 == 0;
            let v = if claro { 0.96 } else { 0.10 };
            pixels.extend_from_slice(&px(v, v, v, 1.0));
        }
    }
    Textura {
        nome: "xadrez",
        largura: lado,
        altura: lado,
        pixels,
    }
}

/// Todas as texturas do jogo, na ordem em que são criadas.
pub fn todas() -> Vec<Textura> {
    vec![
        asfalto(),
        concreto(),
        rocha(),
        mata(),
        guarda_corpo(),
        painel_neon(),
        xadrez(),
    ]
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn toda_textura_tem_o_tamanho_declarado() {
        for t in todas() {
            assert_eq!(t.largura, LADO, "{}", t.nome);
            assert_eq!(t.altura, LADO, "{}", t.nome);
            assert_eq!(
                t.bytes(),
                (LADO * LADO * 4) as usize,
                "{}: {} bytes não batem com {LADO}x{LADO} RGBA",
                t.nome,
                t.bytes()
            );
        }
    }

    #[test]
    fn a_geracao_e_deterministica() {
        // Sem isto cada execução desenharia um asfalto diferente e nenhuma
        // comparação PC × aparelho poderia existir.
        let a = asfalto();
        let b = asfalto();
        assert_eq!(a.pixels, b.pixels, "duas chamadas deram bitmaps diferentes");
    }

    #[test]
    fn nenhuma_textura_e_degenerada() {
        // Uma textura de cor única repete sem dar relevo e o material "some" no
        // meio da cena — o sintoma é o objeto parecer não ter textura.
        for t in todas() {
            let mut menor = 255u8;
            let mut maior = 0u8;
            for p in t.pixels.chunks(4) {
                menor = menor.min(p[0]);
                maior = maior.max(p[0]);
            }
            assert!(
                maior as i32 - menor as i32 > 20,
                "{}: variação de {} níveis — textura chapada",
                t.nome,
                maior as i32 - menor as i32
            );
        }
    }

    #[test]
    fn as_texturas_sao_claras_o_suficiente_para_o_tint_mandar() {
        // O material do jogo já é escuro (noite), e a cor final é
        // `amostra × tint`. Uma textura escura multiplicada por tint escuro vira
        // preto: foi o que apagou a primeira versão do asfalto.
        for t in todas() {
            // O **maior canal** de cada pixel, e não o vermelho: a mata é verde
            // de propósito, e medir o R dela reprovaria uma textura correta. O
            // que precisa ser claro é a luminância que o `tint` vai multiplicar.
            let soma: u64 = t
                .pixels
                .chunks(4)
                .map(|p| p[0].max(p[1]).max(p[2]) as u64)
                .sum();
            let media = soma / (LADO * LADO) as u64;
            assert!(
                media > 90,
                "{}: média de {media} de 255 — escura demais para receber tint",
                t.nome
            );
        }
    }

    #[test]
    fn o_painel_de_neon_tem_alfa_e_as_outras_nao() {
        let painel = painel_neon();
        let com_alfa = painel.pixels.chunks(4).filter(|p| p[3] < 250).count();
        assert!(com_alfa > 500, "o painel devia ter fundo translúcido: {com_alfa} px");

        for nome in ["asfalto", "concreto", "rocha", "mata", "guarda-corpo", "xadrez"] {
            let t = todas().into_iter().find(|t| t.nome == nome).unwrap();
            assert!(
                t.pixels.chunks(4).all(|p| p[3] == 255),
                "{nome} não devia ter alfa variável"
            );
        }
    }

    #[test]
    fn o_xadrez_alterna_casas() {
        let t = xadrez();
        let em = |x: u32, y: u32| t.pixels[((y * LADO + x) * 4) as usize];
        assert!(em(0, 0) > 200, "a primeira casa devia ser clara");
        assert!(em(8, 0) < 60, "a casa seguinte devia ser escura");
        assert!(em(0, 8) < 60, "a casa de baixo devia ser escura");
        assert!(em(8, 8) > 200, "a diagonal devia voltar a ser clara");
    }
}
