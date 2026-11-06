//! Capa de cartão, desenhada — não carregada.
//!
//! # Por que não usar `capa.png` do pacote
//!
//! Porque **não dá**, hoje, e é melhor dizer do que fingir:
//!
//! - o SDL2 puro carrega BMP, não PNG. Ler PNG exigiria `SDL2_image` (mais uma
//!   biblioteca para empacotar nos três sistemas) ou um decodificador inflate
//!   escrito aqui;
//! - a capa de um pacote só existe **depois** de baixar o pacote, e a grade
//!   precisa desenhar antes disso.
//!
//! # O que isto é
//!
//! Uma capa **determinística** derivada do slug: mesma slug, mesma arte, sempre.
//! Dois tons tirados do hash, uma diagonal, e as iniciais do título. Num grid de
//! 12 cartões isso lê como uma loja; um retângulo cinza com o nome leria como
//! protótipo.
//!
//! Não é enfeite: identifica o item. Duas capas iguais significariam duas slugs
//! iguais, e isso o catálogo não permite.
//!
//! # O caminho para a arte de verdade
//!
//! Quando o catálogo tiver URL de capa, o caminho é `SDL2_image` (que lê PNG e
//! WebP) com a capa baixada para o cache ao lado do catálogo, e esta função
//! passa a ser o *fallback* de item sem arte — que é o que ela é hoje, para todos.

use crate::desenho::{Pintor, ACCENT, TEXT};

/// Hash estável e barato: FNV-1a de 64 bits.
///
/// Não é criptográfico e não precisa ser — é escolha de cor. `DefaultHasher` do
/// Rust **não é estável entre versões**, e uma capa que muda de cor quando o
/// compilador é atualizado é o tipo de inconsistência que ninguém sabe explicar
/// depois.
fn fnv1a(texto: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// HSL→RGB só com o que a capa precisa: matiz variável, saturação e luz fixas.
///
/// Fixas de propósito: capa de loja com cor aleatória em saturação alta fica
/// ilegível ao lado do texto branco.
fn matiz_para_cor(h: u8, luz: f32) -> (u8, u8, u8) {
    let matiz = (h as f32 / 255.0) * 360.0;
    let sat = 0.45;
    let c = (1.0 - (2.0 * luz - 1.0).abs()) * sat;
    let x = c * (1.0 - ((matiz / 60.0) % 2.0 - 1.0).abs());
    let m = luz - c / 2.0;
    let (r, g, b) = match matiz as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        (((r + m) * 255.0) as i32).clamp(0, 255) as u8,
        (((g + m) * 255.0) as i32).clamp(0, 255) as u8,
        (((b + m) * 255.0) as i32).clamp(0, 255) as u8,
    )
}

/// Tudo que distingue uma capa de outra.
///
/// # Por que três campos, e não só a cor
///
/// Matiz de 8 bits em 12 itens colide (problema do aniversário: ~23% de chance).
/// Medi, e colidiu de verdade — `volley` e `painel` caíram na mesma matiz.
///
/// A cor não é o único diferenciador possível, então não deve ser o único:
/// **matiz + matiz do detalhe + deslocamento da faixa** dá 256×256×8
/// combinações, e as iniciais do título ainda separam no texto. O teste abaixo
/// exige que o **conjunto** seja único, que é o requisito real — não que a
/// matiz sozinha seja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assinatura {
    pub matiz: u8,
    pub matiz_detalhe: u8,
    /// 0..8: desloca a faixa diagonal e a bolinha.
    pub desloc: u8,
}

pub fn assinatura(slug: &str) -> Assinatura {
    let h = fnv1a(slug);
    Assinatura {
        // Bytes diferentes do hash para cada campo: usar o mesmo byte daria
        // sempre a mesma relação entre fundo e detalhe, e todas as capas
        // ficariam com o mesmo contraste.
        matiz: (h & 0xff) as u8,
        matiz_detalhe: ((h >> 8) & 0xff) as u8,
        desloc: ((h >> 24) & 0x7) as u8,
    }
}

/// As iniciais para o centro da capa: até duas, das primeiras palavras.
fn iniciais(titulo: &str) -> String {
    let palavras: Vec<&str> = titulo.split_whitespace().filter(|p| !p.is_empty()).collect();
    match palavras.len() {
        0 => "?".into(),
        1 => palavras[0].chars().take(2).collect::<String>().to_uppercase(),
        _ => format!(
            "{}{}",
            palavras[0].chars().next().unwrap_or('?'),
            palavras[1].chars().next().unwrap_or('?')
        )
        .to_uppercase(),
    }
}

/// Desenha a capa num retângulo.
///
/// `selecionado` engrossa a borda e clareia; é o único sinal de foco, então tem
/// de ser visível de longe — a grade não tem cursor de mouse.
pub fn desenhar(p: &mut Pintor, x: i32, y: i32, w: i32, h: i32, slug: &str, titulo: &str, selecionado: bool) {
    let a = assinatura(slug);
    let (r1, g1, b1) = matiz_para_cor(a.matiz, 0.34);
    let (r2, g2, b2) = matiz_para_cor(a.matiz_detalhe, 0.50);

    let fundo = sdl2::pixels::Color::RGB(r1, g1, b1);
    let detalhe = sdl2::pixels::Color::RGB(r2, g2, b2);

    p.arredondado(x, y, w, h, 10, fundo);

    // Faixa diagonal: três retângulos deslocados leem como diagonal sem precisar
    // de polígono. `fill_rect` é o que existe; o SDL_Renderer tem `fill_f_rect`,
    // mas ele depende do backend e no software fica serrilhado.
    let largura_faixa = (w / 5).max(8);
    let passo = (a.desloc as i32 + 1) * (h / 24);
    for i in 0..3 {
        let desloc = passo * i;
        p.retangulo(x + w - largura_faixa - desloc / 2, y + desloc / 3, largura_faixa, h / 4, detalhe);
    }

    // Bolinha no canto: dá "arte" sem competir com o texto. A posição também
    // varia com o deslocamento, o que separa duas capas de mesma cor.
    let raio = (w / 9).max(5);
    let bx = x + w - raio * 3;
    let by = y + raio + (a.desloc as i32) * (h / 40);
    p.arredondado(bx, by, raio * 2, raio * 2, raio, detalhe);

    // Iniciais, grandes e centrais.
    let letras = iniciais(titulo);
    let tamanho = (h as f32 * 0.42) as u16;
    let (lw, lh) = p.text_size(&letras, tamanho);
    p.texto(
        x + w / 2 - lw / 2,
        y + h / 2 - lh / 2,
        &letras,
        tamanho,
        sdl2::pixels::Color { a: 210, ..TEXT },
        crate::desenho::Alinhamento::Esquerda,
    );

    if selecionado {
        // Borda em duas camadas: uma grossa por dentro e a de acento por fora,
        // que é o que dá contraste tanto em capa escura quanto clara.
        p.contorno(x, y, w, h, ACCENT);
        p.contorno(x + 1, y + 1, w - 2, h - 2, ACCENT);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_mesma_slug_da_sempre_a_mesma_cor() {
        // Determinismo é o requisito: cor que muda entre execuções faz a tela
        // parecer quebrada e ninguém consegue reproduzir.
        assert_eq!(fnv1a("pacman"), fnv1a("pacman"));
        assert_ne!(fnv1a("pacman"), fnv1a("cobrinha"));
    }

    const CATALOGO: [&str; 12] = [
        "pacman", "cobrinha", "volley", "arcade-volleyball", "spacewar",
        "colossal-cave-adventure", "shattered-pixel-dungeon", "loja",
        "forge", "store", "painel", "launcher",
    ];

    #[test]
    fn a_matiz_sozinha_colide_e_foi_medido() {
        // Este teste existe para **documentar a colisão**, não para fingir que
        // não há. `volley` e `painel` caem na mesma matiz de 8 bits; com 12
        // itens a chance de pelo menos uma colisão é ~23%.
        let cores: Vec<u8> = CATALOGO.iter().map(|s| assinatura(s).matiz).collect();
        let unicas: std::collections::HashSet<_> = cores.iter().collect();
        assert!(
            unicas.len() < CATALOGO.len(),
            "a colisão sumiu — se a matiz ficou mais larga, atualize este teste: {cores:?}"
        );
    }

    #[test]
    fn o_conjunto_matiz_detalhe_desloc_e_unico() {
        // O requisito de verdade: nenhum par de cartões com a mesma **arte**.
        let assinaturas: Vec<Assinatura> = CATALOGO.iter().map(|s| assinatura(s)).collect();
        let unicas: std::collections::HashSet<_> =
            assinaturas.iter().map(|a| (a.matiz, a.matiz_detalhe, a.desloc)).collect();
        assert_eq!(
            unicas.len(),
            CATALOGO.len(),
            "duas capas ficariam idênticas: {assinaturas:?}"
        );
    }

    #[test]
    fn as_iniciais_separam_itens_de_mesma_cor() {
        // `volley` e `painel` compartilham a matiz; "VO" e "PA" separam.
        assert_ne!(iniciais("Volley"), iniciais("Painel"));
        assert_eq!(assinatura("volley").matiz, assinatura("painel").matiz);
    }

    #[test]
    fn iniciais_de_uma_e_de_duas_palavras() {
        assert_eq!(iniciais("Pac-Man"), "PA");
        assert_eq!(iniciais("Arcade Volleyball"), "AV");
        assert_eq!(iniciais("Cobrinha"), "CO");
        assert_eq!(iniciais(""), "?");
        assert_eq!(iniciais("   "), "?");
    }

    #[test]
    fn cor_fica_dentro_da_faixa_de_legibilidade() {
        // Saturação e luz fixas: o teste garante que nenhuma matiz produz preto
        // ou branco puro, onde o texto não teria contraste.
        for h in 0..=255u8 {
            let (r, g, b) = matiz_para_cor(h, 0.34);
            let soma = r as u32 + g as u32 + b as u32;
            assert!(soma > 60, "matiz {h} escura demais: {soma}");
            assert!(soma < 600, "matiz {h} clara demais: {soma}");
        }
    }
}
