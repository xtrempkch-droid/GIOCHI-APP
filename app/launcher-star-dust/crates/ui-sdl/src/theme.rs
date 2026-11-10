//! Paleta e métricas, transcritas do protótipo.
//!
//! Os nomes vêm direto das variáveis CSS, para a comparação entre o HTML e o
//! binário continuar óbvia quando alguém for ajustar o visual.

use sdl2::pixels::Color;

pub const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a: 255,
    }
}

pub const fn rgba(hex: u32, a: u8) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a,
    }
}

pub const BG: Color = rgb(0x070a12);
pub const PANEL: Color = rgb(0x0f1621);
pub const PANEL2: Color = rgb(0x1a2436);
pub const CARD: Color = rgb(0x22304a);
pub const LINE: Color = rgba(0x78a0c8, 30);
pub const LINE2: Color = rgba(0x78a0c8, 76);
pub const ACCENT: Color = rgb(0x66c0f4);
pub const OK: Color = rgb(0x10b981);
pub const WARN: Color = rgb(0xf59e0b);
pub const DANGER: Color = rgb(0xef4444);
pub const TEXT: Color = rgb(0xf1f5f9);
pub const DIM: Color = rgb(0x94a3b8);
pub const FAINT: Color = rgba(0x94a3b8, 140);
pub const BLACK: Color = rgb(0x000000);
pub const WHITE: Color = rgb(0xffffff);

// Métricas do layout de projeto (1280x720).
pub const TOPBAR_H: i32 = 56;
pub const FOOTER_H: i32 = 54;
pub const SAFE_X: i32 = 160;
// A largura útil não é constante: vem de `views::Moldura::safe_w`, calculada do
// painel. A antiga `SAFE_W = 960` era do desenho fixo de 1280×720, e usá-la no
// layout é o que reencolhia a fonte num painel pequeno — por isso foi removida
// em vez de mantida sem uso.

pub const RADIUS_CARD: i32 = 18;
pub const RADIUS_PANEL: i32 = 16;
pub const RADIUS_TILE: i32 = 10;
pub const RADIUS_CHIP: i32 = 7;

/// Alvo mínimo de toque, em unidades de projeto.
pub const TOUCH: i32 = 44;

// Tamanhos de fonte, em pontos do espaço de projeto.
pub const F_TINY: u16 = 9;
pub const F_SMALL: u16 = 11;
pub const F_BODY: u16 = 13;
pub const F_TITLE: u16 = 26;
pub const F_HERO: u16 = 28;
pub const F_LOGO: u16 = 32;

/// Cor de destaque por jogo, derivada do que o catálogo declara.
///
/// No protótipo cada jogo trazia três cores no dado. Aqui a cor nasce do id,
/// para o núcleo não carregar campo de apresentação.
pub fn game_accent(id: u32) -> Color {
    const PALETTE: [Color; 6] = [
        rgb(0x3b82f6),
        rgb(0x14b8a6),
        rgb(0x8b5cf6),
        rgb(0xf59e0b),
        rgb(0xef4444),
        rgb(0x22c55e),
    ];
    PALETTE[(id as usize) % PALETTE.len()]
}

/// Cor escura correspondente, para o gradiente do cartão.
pub fn game_base(id: u32) -> Color {
    const PALETTE: [Color; 6] = [
        rgb(0x0c1e5f),
        rgb(0x042f2e),
        rgb(0x1a0e24),
        rgb(0x451a03),
        rgb(0x5f0f0f),
        rgb(0x052e16),
    ];
    PALETTE[(id as usize) % PALETTE.len()]
}

/// Mistura linear, usada nos gradientes aproximados por faixas.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        g: (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        b: (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
        a: (a.a as f32 + (b.a as f32 - a.a as f32) * t) as u8,
    }
}

pub fn with_alpha(color: Color, a: u8) -> Color {
    Color { a, ..color }
}
