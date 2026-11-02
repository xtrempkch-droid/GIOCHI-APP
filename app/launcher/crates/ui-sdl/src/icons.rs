//! Ícones vetoriais desenhados com primitivas.
//!
//! O protótipo usava emoji (`🎮 📁 🌐 🛒`) e, no console, isso vira retângulo
//! vazio — não existe fonte de emoji no rootfs do Giochi.
//!
//! Em vez de carregar PNG, os ícones são **dados**: linhas, polilinhas,
//! círculos e retângulos num espaço 24×24, iguais aos do SVG original. O
//! `Painter` escala e rasteriza. Vantagem prática: nitidez em qualquer DPI e
//! nenhum asset binário para versionar.

/// Primitiva de desenho no espaço 24×24.
#[derive(Clone, Copy, Debug)]
pub enum Prim {
    Line(f32, f32, f32, f32),
    /// Polilinha. `true` preenche o polígono.
    Poly(&'static [(f32, f32)], bool),
    /// Contorno de círculo: (cx, cy, r).
    Circle(f32, f32, f32),
    Rect(f32, f32, f32, f32),
}

pub struct Icon {
    pub prims: &'static [Prim],
}

// ── Tabelas ──────────────────────────────────────────────────

static HOME: [Prim; 2] = [
    Prim::Poly(
        &[
            (3.0, 9.0),
            (12.0, 2.0),
            (21.0, 9.0),
            (21.0, 20.0),
            (19.0, 22.0),
            (5.0, 22.0),
            (3.0, 20.0),
        ],
        false,
    ),
    Prim::Poly(
        &[(9.0, 22.0), (9.0, 12.0), (15.0, 12.0), (15.0, 22.0)],
        false,
    ),
];

static GAMEPAD: [Prim; 4] = [
    Prim::Poly(
        &[
            (6.7, 5.0),
            (17.3, 5.0),
            (21.0, 8.6),
            (21.0, 16.0),
            (20.0, 19.0),
            (17.0, 19.0),
            (16.0, 18.0),
            (8.0, 18.0),
            (7.0, 19.0),
            (4.0, 19.0),
            (3.0, 16.0),
            (3.0, 8.6),
        ],
        false,
    ),
    Prim::Line(6.0, 12.0, 10.0, 12.0),
    Prim::Line(8.0, 10.0, 8.0, 14.0),
    Prim::Circle(15.0, 13.0, 1.0),
];

static APPS: [Prim; 4] = [
    Prim::Rect(3.0, 3.0, 7.0, 7.0),
    Prim::Rect(14.0, 3.0, 7.0, 7.0),
    Prim::Rect(3.0, 14.0, 7.0, 7.0),
    Prim::Rect(14.0, 14.0, 7.0, 7.0),
];

static SETTINGS: [Prim; 9] = [
    Prim::Circle(12.0, 12.0, 3.0),
    Prim::Circle(12.0, 12.0, 9.0),
    Prim::Line(12.0, 2.0, 12.0, 5.0),
    Prim::Line(12.0, 19.0, 12.0, 22.0),
    Prim::Line(2.0, 12.0, 5.0, 12.0),
    Prim::Line(19.0, 12.0, 22.0, 12.0),
    Prim::Line(4.9, 4.9, 7.0, 7.0),
    Prim::Line(17.0, 17.0, 19.1, 19.1),
    Prim::Line(4.9, 19.1, 7.0, 17.0),
];

static POWER: [Prim; 2] = [
    Prim::Line(12.0, 2.0, 12.0, 12.0),
    Prim::Poly(
        &[
            (18.4, 6.6),
            (21.0, 11.0),
            (20.0, 16.0),
            (16.0, 20.0),
            (12.0, 21.0),
            (8.0, 20.0),
            (4.0, 16.0),
            (3.0, 11.0),
            (5.6, 6.6),
        ],
        false,
    ),
];

static PLAY: [Prim; 1] = [Prim::Poly(
    &[(6.0, 4.0), (20.0, 12.0), (6.0, 20.0)],
    true,
)];

static STAR: [Prim; 1] = [Prim::Poly(
    &[
        (12.0, 2.0),
        (15.0, 9.0),
        (22.0, 9.3),
        (16.5, 14.0),
        (18.3, 21.0),
        (12.0, 17.2),
        (5.7, 21.0),
        (7.5, 14.0),
        (2.0, 9.3),
        (9.0, 9.0),
    ],
    true,
)];

static CHECK: [Prim; 1] = [Prim::Poly(
    &[(20.0, 6.0), (9.0, 17.0), (4.0, 12.0)],
    false,
)];

static WIFI: [Prim; 4] = [
    Prim::Poly(&[(1.4, 9.0), (12.0, 3.0), (22.6, 9.0)], false),
    Prim::Poly(&[(5.0, 12.5), (12.0, 8.0), (19.0, 12.5)], false),
    Prim::Poly(&[(8.5, 16.1), (12.0, 13.5), (15.5, 16.1)], false),
    Prim::Circle(12.0, 20.0, 1.0),
];

static BT: [Prim; 1] = [Prim::Poly(
    &[
        (6.5, 6.5),
        (17.5, 17.5),
        (12.0, 23.0),
        (12.0, 1.0),
        (17.5, 6.5),
        (6.5, 17.5),
    ],
    false,
)];

static VOL: [Prim; 3] = [
    Prim::Poly(
        &[
            (11.0, 5.0),
            (6.0, 9.0),
            (2.0, 9.0),
            (2.0, 15.0),
            (6.0, 15.0),
            (11.0, 19.0),
        ],
        false,
    ),
    Prim::Poly(&[(15.5, 8.5), (17.0, 12.0), (15.5, 15.5)], false),
    Prim::Poly(&[(19.1, 4.9), (21.0, 12.0), (19.1, 19.1)], false),
];

static SUN: [Prim; 9] = [
    Prim::Circle(12.0, 12.0, 4.0),
    Prim::Line(12.0, 2.0, 12.0, 4.0),
    Prim::Line(12.0, 20.0, 12.0, 22.0),
    Prim::Line(2.0, 12.0, 4.0, 12.0),
    Prim::Line(20.0, 12.0, 22.0, 12.0),
    Prim::Line(4.9, 4.9, 6.3, 6.3),
    Prim::Line(17.7, 17.7, 19.1, 19.1),
    Prim::Line(4.9, 19.1, 6.3, 17.7),
    Prim::Line(17.7, 6.3, 19.1, 4.9),
];

static BOLT: [Prim; 1] = [Prim::Poly(
    &[
        (13.0, 2.0),
        (3.0, 14.0),
        (12.0, 14.0),
        (11.0, 22.0),
        (21.0, 10.0),
        (12.0, 10.0),
    ],
    true,
)];

static CHART: [Prim; 3] = [
    Prim::Line(18.0, 20.0, 18.0, 10.0),
    Prim::Line(12.0, 20.0, 12.0, 4.0),
    Prim::Line(6.0, 20.0, 6.0, 14.0),
];

static MOON: [Prim; 2] = [
    Prim::Poly(
        &[
            (21.0, 12.8),
            (19.0, 17.0),
            (15.0, 20.0),
            (11.2, 20.6),
            (7.6, 19.2),
            (4.8, 16.2),
            (3.4, 12.6),
            (3.8, 8.8),
            (5.9, 5.6),
            (9.2, 3.6),
        ],
        false,
    ),
    Prim::Poly(
        &[(11.2, 3.0), (13.4, 6.6), (14.2, 10.6), (13.2, 14.4), (11.0, 17.4)],
        false,
    ),
];

static SEARCH: [Prim; 2] = [
    Prim::Circle(11.0, 11.0, 8.0),
    Prim::Line(21.0, 21.0, 16.65, 16.65),
];

/// Devolve o ícone pelo nome usado nos dados do catálogo.
pub fn get(name: &str) -> Option<&'static Icon> {
    macro_rules! icon {
        ($prims:ident) => {
            Some(&Icon { prims: &$prims })
        };
    }
    match name {
        "home" => icon!(HOME),
        "gamepad" => icon!(GAMEPAD),
        "apps" => icon!(APPS),
        "settings" => icon!(SETTINGS),
        "power" => icon!(POWER),
        "play" => icon!(PLAY),
        "star" => icon!(STAR),
        "check" => icon!(CHECK),
        "wifi" => icon!(WIFI),
        "bt" => icon!(BT),
        "vol" => icon!(VOL),
        "sun" => icon!(SUN),
        "bolt" => icon!(BOLT),
        "chart" => icon!(CHART),
        "moon" => icon!(MOON),
        "search" => icon!(SEARCH),
        _ => None,
    }
}
