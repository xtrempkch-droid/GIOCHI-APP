//! Desenho: paleta, fundo pré-rasterizado e dígitos sem fonte.
//!
//! ## Por que o fundo é uma textura
//!
//! O tabuleiro são 24×18 = **432 células** de xadrez. Desenhá-las a cada quadro
//! seriam 432 `fill_rect` a 45 Hz — 19 mil chamadas por segundo só para pintar
//! o que não muda. O fundo é rasterizado **uma vez** numa `Surface`, vira
//! textura e cada quadro custa **um** `copy`. O resto do quadro são ~60
//! retângulos pequenos (corpo, comida, placar).
//!
//! ## Por que não há fonte
//!
//! `SDL2_ttf` traz `freetype` e `harfbuzz` junto. O R35S **não tem `libasound`**
//! e um binário que exige biblioteca ausente não sobe — a lição já custou uma
//! sessão. Um placar de dígitos em sete segmentos custa 30 linhas de retângulo e
//! mantém o `NEEDED` em três bibliotecas. Jogo que não pede nada à imagem
//! instala em qualquer aparelho.

use giochi_cobrinha_core::Geometria;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::video::WindowContext;

use crate::video::Rotacao;

// Paleta do launcher (`theme.rs`), para o jogo não parecer de outro produto.
pub const FUNDO: Color = Color::RGB(11, 12, 20);
pub const XADREZ: Color = Color::RGB(17, 19, 29);
pub const BORDA: Color = Color::RGB(45, 52, 74);
pub const CORPO: Color = Color::RGB(96, 165, 250);
pub const CABECA: Color = Color::RGB(147, 197, 253);
pub const COMIDA: Color = Color::RGB(248, 113, 113);
pub const TEXTO: Color = Color::RGB(203, 213, 225);
pub const APAGADO: Color = Color::RGB(71, 85, 105);

/// Os sete segmentos de cada dígito, na ordem `a b c d e f g`.
const SEGMENTOS: [[bool; 7]; 10] = [
    [true, true, true, true, true, true, false],    // 0
    [false, true, true, false, false, false, false], // 1
    [true, true, false, true, true, false, true],   // 2
    [true, true, true, true, false, false, true],   // 3
    [false, true, true, false, false, true, true],  // 4
    [true, false, true, true, false, true, true],   // 5
    [true, false, true, true, true, true, true],    // 6
    [true, true, true, false, false, false, false], // 7
    [true, true, true, true, true, true, true],     // 8
    [true, true, true, true, false, true, true],    // 9
];

/// Retângulos de um dígito. `x`,`y` é o canto; a altura é o dobro da largura.
///
/// Devolve em coordenada de projeto; quem chama gira.
pub fn retangulos_do_digito(digito: u8, x: i32, y: i32, w: i32) -> Vec<(i32, i32, i32, i32)> {
    if digito > 9 || w <= 0 {
        return Vec::new();
    }
    let h = w * 2;
    let t = (w / 6).max(2);
    let meio = y + h / 2;
    let mut saida = Vec::with_capacity(7);
    let ligados = SEGMENTOS[digito as usize];

    // a — topo
    if ligados[0] {
        saida.push((x + t, y, w - 2 * t, t));
    }
    // b — direita superior
    if ligados[1] {
        saida.push((x + w - t, y + t, t, meio - t / 2 - (y + t)));
    }
    // c — direita inferior
    if ligados[2] {
        saida.push((x + w - t, meio + t / 2, t, y + h - t - (meio + t / 2)));
    }
    // d — base
    if ligados[3] {
        saida.push((x + t, y + h - t, w - 2 * t, t));
    }
    // e — esquerda inferior
    if ligados[4] {
        saida.push((x, meio + t / 2, t, y + h - t - (meio + t / 2)));
    }
    // f — esquerda superior
    if ligados[5] {
        saida.push((x, y + t, t, meio - t / 2 - (y + t)));
    }
    // g — meio
    if ligados[6] {
        saida.push((x + t, meio - t / 2, w - 2 * t, t));
    }
    saida
}

/// Largura que um número ocupa desenhado com dígitos de largura `w`.
pub fn largura_do_numero(n: u32, w: i32) -> i32 {
    let digitos = digitos_de(n) as i32;
    digitos * w + (digitos - 1) * (w / 3).max(2)
}

pub fn digitos_de(n: u32) -> u32 {
    let mut d = 1;
    let mut v = n / 10;
    while v > 0 {
        d += 1;
        v /= 10;
    }
    d
}

/// Rasteriza o tabuleiro numa `Surface` já **girada** para o espaço de
/// dispositivo. Uma vez por execução.
pub fn rasterizar_fundo(
    geom: &Geometria,
    rot: Rotacao,
    design: (i32, i32),
    painel: (u32, u32),
) -> Surface<'static> {
    let mut superficie =
        Surface::new(painel.0, painel.1, sdl2::pixels::PixelFormatEnum::RGBA32)
            .expect("surface do fundo");
    superficie
        .fill_rect(None, FUNDO)
        .expect("limpar o fundo");

    let mut pintar = |x: i32, y: i32, w: i32, h: i32, cor: Color| {
        if w <= 0 || h <= 0 {
            return;
        }
        let (dx, dy, dw, dh) = crate::video::girar_rect(rot, design, x, y, w, h);
        let _ = superficie.fill_rect(Rect::new(dx, dy, dw as u32, dh as u32), cor);
    };

    // Xadrez: a célula alterna para o olho conseguir contar as casas sem
    // precisar de linha divisória (linha em grade 24×18 vira ruído visual).
    for linha in 0..giochi_cobrinha_core::LINHAS {
        for coluna in 0..giochi_cobrinha_core::COLUNAS {
            if (linha + coluna) % 2 == 0 {
                let (x, y) = geom.celula_em(coluna, linha);
                pintar(x, y, geom.celula, geom.celula, XADREZ);
            }
        }
    }

    // Moldura do campo: 2 px, para a parede ser visível como parede.
    let esp = 2;
    let (cx, cy, cw, ch) = (geom.campo_x, geom.campo_y, geom.campo_w(), geom.campo_h());
    pintar(cx - esp, cy - esp, cw + 2 * esp, esp, BORDA);
    pintar(cx - esp, cy + ch, cw + 2 * esp, esp, BORDA);
    pintar(cx - esp, cy, esp, ch, BORDA);
    pintar(cx + cw, cy, esp, ch, BORDA);

    superficie
}

/// O que o laço precisa para desenhar.
pub struct Tela {
    pub canvas: WindowCanvas,
    fundo: Texture<'static>,
    pub geom: Geometria,
    pub rot: Rotacao,
    pub design: (i32, i32),
    /// Verdadeiro quando o backend não é `software`. O `--quadro` usa isto para
    /// avisar que o BMP pode sair preto por causa do swap.
    pub renderer_acelerado: bool,
}

impl Tela {
    pub fn nova(
        mut canvas: WindowCanvas,
        geom: Geometria,
        rot: Rotacao,
        design: (i32, i32),
        painel: (u32, u32),
    ) -> Self {
        let superficie = rasterizar_fundo(&geom, rot, design, painel);
        // O `TextureCreator` precisa viver tanto quanto a textura; ele é vazado
        // de propósito (o processo é o jogo inteiro, e o padrão já é usado no
        // launcher para o mesmo problema de tempo de vida).
        let criador: &'static TextureCreator<WindowContext> =
            Box::leak(Box::new(canvas.texture_creator()));
        let fundo = superficie
            .as_texture(criador)
            .expect("textura do fundo");
        let _ = canvas.set_blend_mode(sdl2::render::BlendMode::None);
        let renderer_acelerado = canvas
            .info()
            .name
            .to_ascii_lowercase()
            .contains("gles");
        Tela { canvas, fundo, geom, rot, design, renderer_acelerado }
    }

    /// Retângulo de projeto → `Rect` de dispositivo.
    pub fn r(&self, x: i32, y: i32, w: i32, h: i32) -> Rect {
        let (dx, dy, dw, dh) = crate::video::girar_rect(self.rot, self.design, x, y, w, h);
        Rect::new(dx, dy, dw.max(0) as u32, dh.max(0) as u32)
    }

    pub fn preencher(&mut self, x: i32, y: i32, w: i32, h: i32, cor: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.set_draw_color(cor);
        let _ = self.canvas.fill_rect(self.r(x, y, w, h));
    }

    /// Um quadro completo: fundo (1 blit) + o que mudou.
    pub fn comecar_quadro(&mut self) {
        self.canvas.set_draw_color(FUNDO);
        let _ = self.canvas.clear();
        let _ = self.canvas.copy(&self.fundo, None, None);
    }

    pub fn terminar_quadro(&mut self) {
        self.canvas.present();
    }

    /// Número em sete segmentos, alinhado à direita em `x`.
    pub fn numero(&mut self, n: u32, x_direita: i32, y: i32, w: i32, cor: Color) {
        let espaco = (w / 3).max(2);
        let mut x = x_direita;
        let mut valor = n;
        loop {
            let digito = (valor % 10) as u8;
            x -= w;
            for (rx, ry, rw, rh) in retangulos_do_digito(digito, x, y, w) {
                self.preencher(rx, ry, rw, rh, cor);
            }
            valor /= 10;
            if valor == 0 {
                break;
            }
            x -= espaco;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_digito_desenha_algum_segmento() {
        for d in 0..=9u8 {
            let r = retangulos_do_digito(d, 0, 0, 12);
            assert!(!r.is_empty(), "dígito {d} sem segmento");
            assert!(r.len() <= 7, "dígito {d} com {} segmentos", r.len());
        }
    }

    #[test]
    fn oito_acende_os_sete() {
        assert_eq!(retangulos_do_digito(8, 0, 0, 12).len(), 7);
    }

    #[test]
    fn zero_tem_seis_e_um_tem_dois() {
        assert_eq!(retangulos_do_digito(0, 0, 0, 12).len(), 6);
        assert_eq!(retangulos_do_digito(1, 0, 0, 12).len(), 2);
    }

    #[test]
    fn segmentos_ficam_dentro_da_caixa() {
        // Segmento fora da caixa invade o dígito vizinho e o placar vira ilegível.
        for d in 0..=9u8 {
            for (x, y, w, h) in retangulos_do_digito(d, 10, 20, 12) {
                assert!(x >= 10, "dígito {d}: x {x} antes da caixa");
                assert!(y >= 20, "dígito {d}: y {y} antes da caixa");
                assert!(x + w <= 22, "dígito {d}: estoura à direita em {}", x + w);
                assert!(y + h <= 44, "dígito {d}: estoura embaixo em {}", y + h);
                assert!(w > 0 && h > 0, "dígito {d}: segmento degenerado {w}x{h}");
            }
        }
    }

    #[test]
    fn digito_invalido_nao_desenha() {
        assert!(retangulos_do_digito(10, 0, 0, 12).is_empty());
        assert!(retangulos_do_digito(3, 0, 0, 0).is_empty());
    }

    #[test]
    fn a_largura_do_numero_acompanha_os_digitos() {
        assert_eq!(largura_do_numero(0, 10), 10);
        assert_eq!(largura_do_numero(9, 10), 10);
        assert!(largura_do_numero(10, 10) > 10);
        assert!(largura_do_numero(100, 10) > largura_do_numero(10, 10));
    }

    #[test]
    fn contagem_de_digitos() {
        assert_eq!(digitos_de(0), 1);
        assert_eq!(digitos_de(7), 1);
        assert_eq!(digitos_de(10), 2);
        assert_eq!(digitos_de(999), 3);
        assert_eq!(digitos_de(1000), 4);
    }
}
