//! Cálculo de escala e área segura.
//!
//! ## O bug que isto corrige
//!
//! O protótipo calculava a escala com **duas larguras diferentes**:
//!
//! ```js
//! const scale = Math.min(vw/960, vh/720);   // 960 = largura da área segura
//! c.style.left = ((vw - 1280*scale)/2) + 'px';  // 1280 = largura do canvas
//! ```
//!
//! Em 640×480 (o alvo RK3326): `scale = min(0.667, 0.667) = 0.667`, e
//! `left = (640 - 853) / 2 = -106`. Ou seja, **106 px de cada lado ficavam
//! fora da tela**, comendo justamente a área segura.
//!
//! Aqui a escala e o posicionamento usam a **mesma** constante de desenho, e o
//! resultado é letterbox: sobra tarja, nunca corte.

/// Resolução de projeto. Todo layout é desenhado neste espaço.
pub const DESIGN_WIDTH: f32 = 1280.0;
pub const DESIGN_HEIGHT: f32 = 720.0;

/// Área segura horizontal, centrada no canvas. Fora dela ficam apenas
/// elementos decorativos (fundo, HUD).
pub const SAFE_WIDTH: f32 = 960.0;

/// Alvo mínimo de toque, em unidades de projeto. Abaixo disso o dedo erra —
/// o protótipo tinha `kbd` com 15 px de altura.
pub const MIN_TOUCH_TARGET: f32 = 44.0;

/// Mapeamento entre a tela real e o espaço de projeto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub screen_width: f32,
    pub screen_height: f32,
}

impl Viewport {
    /// Encaixa o espaço de projeto na tela, preservando proporção.
    pub fn fit(screen_width: f32, screen_height: f32) -> Self {
        let screen_width = screen_width.max(1.0);
        let screen_height = screen_height.max(1.0);

        let scale = (screen_width / DESIGN_WIDTH).min(screen_height / DESIGN_HEIGHT);

        let drawn_width = DESIGN_WIDTH * scale;
        let drawn_height = DESIGN_HEIGHT * scale;

        Self {
            scale,
            offset_x: (screen_width - drawn_width) / 2.0,
            offset_y: (screen_height - drawn_height) / 2.0,
            screen_width,
            screen_height,
        }
    }

    /// Converte coordenada de projeto para coordenada de tela.
    pub fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.offset_x + x * self.scale,
            self.offset_y + y * self.scale,
        )
    }

    /// Converte coordenada de tela para projeto.
    ///
    /// Devolve `None` quando o ponto cai na tarja (letterbox): ali não existe
    /// interface, e tratar clique na tarja como clique no canto da UI é como o
    /// toque erra o alvo sem ninguém entender por quê.
    pub fn to_design(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let dx = (x - self.offset_x) / self.scale;
        let dy = (y - self.offset_y) / self.scale;
        if dx < 0.0 || dy < 0.0 || dx > DESIGN_WIDTH || dy > DESIGN_HEIGHT {
            return None;
        }
        Some((dx, dy))
    }

    /// Retângulo da área segura em coordenadas de projeto.
    pub fn safe_area(&self) -> (f32, f32, f32, f32) {
        let x = (DESIGN_WIDTH - SAFE_WIDTH) / 2.0;
        (x, 0.0, SAFE_WIDTH, DESIGN_HEIGHT)
    }

    /// Um alvo de toque tem tamanho utilizável?
    pub fn touch_target_ok(&self, width: f32, height: f32) -> bool {
        width >= MIN_TOUCH_TARGET && height >= MIN_TOUCH_TARGET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn em_720p_escala_e_um_e_sem_tarja() {
        let vp = Viewport::fit(1280.0, 720.0);
        assert!((vp.scale - 1.0).abs() < f32::EPSILON);
        assert!(vp.offset_x.abs() < f32::EPSILON);
        assert!(vp.offset_y.abs() < f32::EPSILON);
    }

    #[test]
    fn em_640x480_letterbox_sem_corte() {
        // Alvo RK3326. O protótipo calculava left = -106 e cortava a interface.
        let vp = Viewport::fit(640.0, 480.0);
        assert!((vp.scale - 0.5).abs() < f32::EPSILON, "escala {}", vp.scale);
        assert!(vp.offset_x.abs() < f32::EPSILON, "offset_x {}", vp.offset_x);
        assert!((vp.offset_y - 60.0).abs() < f32::EPSILON, "offset_y {}", vp.offset_y);

        // Nenhum canto da área desenhada fica fora da tela.
        let (x0, y0) = vp.to_screen(0.0, 0.0);
        let (x1, y1) = vp.to_screen(DESIGN_WIDTH, DESIGN_HEIGHT);
        assert!(x0 >= 0.0 && y0 >= 0.0);
        assert!(x1 <= 640.0 + 0.01 && y1 <= 480.0 + 0.01);
    }

    #[test]
    fn em_1080p_escala_um_e_meio() {
        let vp = Viewport::fit(1920.0, 1080.0);
        assert!((vp.scale - 1.5).abs() < f32::EPSILON);
        assert!(vp.offset_x.abs() < f32::EPSILON);
        assert!(vp.offset_y.abs() < f32::EPSILON);
    }

    #[test]
    fn tela_estreita_ganha_tarja_lateral() {
        // 640×720: limita pela largura.
        let vp = Viewport::fit(640.0, 720.0);
        assert!((vp.scale - 0.5).abs() < f32::EPSILON);
        assert!((vp.offset_y - 180.0).abs() < f32::EPSILON);
    }

    #[test]
    fn to_design_recusa_ponto_na_tarja() {
        let vp = Viewport::fit(640.0, 480.0);
        assert!(vp.to_design(320.0, 10.0).is_none(), "y=10 está na tarja");
        assert!(vp.to_design(320.0, 240.0).is_some(), "centro é válido");
    }

    #[test]
    fn to_design_e_to_screen_sao_inversos() {
        let vp = Viewport::fit(1024.0, 768.0);
        let (sx, sy) = vp.to_screen(640.0, 360.0);
        let (dx, dy) = vp.to_design(sx, sy).expect("centro deve ser válido");
        assert!((dx - 640.0).abs() < 0.01);
        assert!((dy - 360.0).abs() < 0.01);
    }

    #[test]
    fn area_segura_e_centrada() {
        let vp = Viewport::fit(1280.0, 720.0);
        let (x, y, w, h) = vp.safe_area();
        assert!(x > 0.0 && (x * 2.0 + w - DESIGN_WIDTH).abs() < 0.01);
        assert!(y.abs() < f32::EPSILON && (h - DESIGN_HEIGHT).abs() < f32::EPSILON);
    }

    #[test]
    fn alvo_de_toque_pequeno_e_reprovado() {
        let vp = Viewport::fit(1280.0, 720.0);
        assert!(!vp.touch_target_ok(60.0, 15.0), "altura 15 px é o bug do protótipo");
        assert!(vp.touch_target_ok(60.0, 44.0));
    }

    #[test]
    fn tela_degenerada_nao_divide_por_zero() {
        let vp = Viewport::fit(0.0, 0.0);
        assert!(vp.scale > 0.0);
        assert!(vp.scale.is_finite());
    }
}
