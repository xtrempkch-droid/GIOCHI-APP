//! Painter: as primitivas de desenho sobre o `SDL_Renderer`.
//!
//! ## Por que escalar com `set_logical_size`
//!
//! O protótipo calculava a escala à mão e errava (usava 960 e 1280 no mesmo
//! cálculo, cortando 106 px de cada lado em 640×480). O SDL2 já resolve isso:
//! `set_logical_size(1280, 720)` desenha num espaço fixo e o SDL letterboxa
//! mantendo proporção. Todo desenho abaixo usa coordenadas de **projeto**, e a
//! conversão de toque é `window_to_logical`.
//!
//! ## Por que gradiente em faixas
//!
//! `SDL_Renderer` não tem gradiente. Em vez de shader próprio (que exigiria
//! GLES direto e quebraria o backend software do SDL), o gradiente é
//! aproximado por faixas de 4 px. Com 4 px de altura a diferença é invisível e
//! o custo é baixo (~80 chamadas por cartão).

use std::collections::HashMap;

use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::surface::Surface;
use sdl2::render::{BlendMode, TextureCreator, WindowCanvas};
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::{Window, WindowContext};

use crate::icons::{self, Prim};
use crate::theme;

/// Alinhamento horizontal do texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Recorte retangular, para desenhar listas que rolam.
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub struct Painter {
    canvas: WindowCanvas,
    /// Guardado porque `Texture` empresta do `TextureCreator`. Criar um por
    /// texto obrigaria a manter os dois vivos ao mesmo tempo dentro da função.
    creator: TextureCreator<WindowContext>,
    ttf: Option<&'static Sdl2TtfContext>,
    /// Fonte por tamanho. Guardadas sem empréstimo do `Painter`, por isso
    /// `'static`: ver `take_font`.
    fonts: HashMap<u16, Font<'static, 'static>>,
    font_path: &'static str,
    clip_stack: Vec<Option<Clip>>,
}

impl Painter {
    pub fn new(window: Window, font_path: &str) -> Result<Self, String> {
        let mut canvas = window
            .into_canvas()
            .present_vsync()
            .build()
            .map_err(|e| format!("canvas: {e}"))?;

        canvas.set_blend_mode(BlendMode::Blend);
        // Espaço de desenho fixo; o SDL letterboxa mantendo proporção.
        canvas
            .set_logical_size(1280, 720)
            .map_err(|e| format!("logical size: {e}"))?;

        // `Sdl2TtfContext` fica vazado de propósito: precisa viver mais que as
        // fontes, e o programa termina antes disso importar.
        let ttf: Option<&'static Sdl2TtfContext> = match sdl2::ttf::init() {
            Ok(context) => Some(Box::leak(Box::new(context))),
            Err(e) => {
                eprintln!("aviso: TTF indisponível ({e}); o texto não será desenhado");
                None
            }
        };

        // O caminho precisa ser `'static` para a `Font` ser `'static`.
        let font_path: &'static str = Box::leak(font_path.to_string().into_boxed_str());

        let creator = canvas.texture_creator();

        Ok(Self {
            canvas,
            creator,
            ttf,
            fonts: HashMap::new(),
            font_path,
            clip_stack: Vec::new(),
        })
    }

    fn take_font(&mut self, size: u16) -> Option<Font<'static, 'static>> {
        if let Some(font) = self.fonts.remove(&size) {
            return Some(font);
        }
        let ttf = self.ttf?;
        match ttf.load_font(self.font_path, size) {
            Ok(font) => Some(font),
            Err(e) => {
                eprintln!("aviso: fonte não carregada ({e})");
                None
            }
        }
    }

    // ── quadro ───────────────────────────────────────────────

    pub fn clear(&mut self, color: Color) {
        self.canvas.set_draw_color(color);
        self.canvas.clear();
    }

    pub fn present(&mut self) {
        self.canvas.present();
    }

    /// Tamanho do espaço de projeto, o que `set_logical_size` fixou.
    ///
    /// Não confundir com `output_size`, que é o tamanho real em pixels da
    /// janela: é justamente a diferença entre os dois que define a escala.
    pub fn logical_size(&self) -> (u32, u32) {
        let (w, h) = self.canvas.logical_size();
        if w == 0 || h == 0 {
            (1280, 720)
        } else {
            (w, h)
        }
    }

    /// Grava o quadro atual em BMP.
    ///
    /// Serve para conferir o desenho numa máquina sem tela, com
    /// `SDL_VIDEODRIVER=dummy` — é a única forma de validar a UI aqui sem
    /// hardware. Não é usado no aparelho.
    pub fn save_frame(&self, path: &str) -> Result<(), String> {
        let (w, h) = self.canvas.output_size()?;
        let format = PixelFormatEnum::ABGR8888;
        let mut pixels = self.canvas.read_pixels(None, format)?;
        let pitch = w * format.byte_size_per_pixel() as u32;
        let surface = Surface::from_data(&mut pixels, w, h, pitch, format)?;
        surface.save_bmp(path)
    }

    /// Converte coordenada de janela (toque) para o espaço de projeto.
    ///
    /// O wrapper `Canvas::window_to_logical` **não existe** no `sdl2` 0.37, mas
    /// o `SDL_RenderWindowToLogical` do C existe e faz exatamente esta conta.
    /// Refazê-la aqui evita `unsafe` sobre o ponteiro cru do renderer, que é
    /// privado no crate. É a mesma matemática: escala uniforme
    /// (`min` dos dois eixos) e letterbox centrado.
    pub fn window_to_logical(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (out_w, out_h) = self.canvas.output_size().ok()?;
        let (lw, lh) = self.logical_size();
        if out_w == 0 || out_h == 0 {
            return None;
        }
        let scale = (out_w as f32 / lw as f32).min(out_h as f32 / lh as f32);
        let off_x = (out_w as f32 - lw as f32 * scale) / 2.0;
        let off_y = (out_h as f32 - lh as f32 * scale) / 2.0;
        Some((
            ((x as f32 - off_x) / scale).round() as i32,
            ((y as f32 - off_y) / scale).round() as i32,
        ))
    }

    // ── recorte ──────────────────────────────────────────────

    pub fn push_clip(&mut self, clip: Clip) {
        let previous = self.clip_stack.last().copied().flatten();
        let merged = match previous {
            Some(p) => intersect(p, clip),
            None => Some(clip),
        };
        self.clip_stack.push(merged);
        self.apply_clip();
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.apply_clip();
    }

    fn apply_clip(&mut self) {
        let clip = self.clip_stack.last().copied().flatten();
        match clip {
            Some(c) => {
                let _ = self.canvas.set_clip_rect(Rect::new(c.x, c.y, c.w.max(0) as u32, c.h.max(0) as u32));
            }
            None => self.canvas.set_clip_rect(None),
        }
    }

    // ── retângulos ───────────────────────────────────────────

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.set_draw_color(color);
        let _ = self
            .canvas
            .fill_rect(Rect::new(x, y, w as u32, h as u32));
    }

    pub fn stroke_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.set_draw_color(color);
        let _ = self
            .canvas
            .draw_rect(Rect::new(x, y, w as u32, h as u32));
    }

    /// Retângulo com cantos arredondados, por varredura de linhas.
    pub fn fill_round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.fill_rect(x, y, w, h, color);
            return;
        }

        let mut spans: Vec<Rect> = Vec::with_capacity(h as usize);
        for row in 0..h {
            let inset = if row < r {
                corner_inset(row, r)
            } else if row >= h - r {
                corner_inset(h - 1 - row, r)
            } else {
                0
            };
            let width = w - inset * 2;
            if width > 0 {
                spans.push(Rect::new(x + inset, y + row, width as u32, 1));
            }
        }

        self.canvas.set_draw_color(color);
        let _ = self.canvas.fill_rects(&spans);
    }

    pub fn stroke_round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.stroke_rect(x, y, w, h, color);
            return;
        }
        // Retas centrais.
        self.fill_rect(x + r, y, w - 2 * r, 1, color);
        self.fill_rect(x + r, y + h - 1, w - 2 * r, 1, color);
        self.fill_rect(x, y + r, 1, h - 2 * r, color);
        self.fill_rect(x + w - 1, y + r, 1, h - 2 * r, color);
        // Cantos.
        self.arc_corner(x + r, y + r, r, 180.0, 270.0, color);
        self.arc_corner(x + w - r, y + r, r, 270.0, 360.0, color);
        self.arc_corner(x + w - r, y + h - r, r, 0.0, 90.0, color);
        self.arc_corner(x + r, y + h - r, r, 90.0, 180.0, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_corner(&mut self, cx: i32, cy: i32, r: i32, from: f32, to: f32, color: Color) {
        let steps = (r as f32 * 1.6).max(6.0) as i32;
        self.canvas.set_draw_color(color);
        let mut previous: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let t = from + (to - from) * (i as f32 / steps as f32);
            let rad = t.to_radians();
            let point = (
                cx + (rad.cos() * r as f32).round() as i32,
                cy + (rad.sin() * r as f32).round() as i32,
            );
            if let Some(prev) = previous {
                let _ = self.canvas.draw_line(prev, point);
            }
            previous = Some(point);
        }
    }

    /// Gradiente vertical dentro de um retângulo arredondado.
    ///
    /// Faixas de 4 px. O inset do canto é calculado pela borda mais restritiva
    /// da faixa, o que evita degrau visível.
    #[allow(clippy::too_many_arguments)]
    pub fn vgradient_round_rect(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: i32,
        top: Color,
        bottom: Color,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        const BAND: i32 = 4;
        let r = radius.min(w / 2).min(h / 2).max(0);
        let mut row = 0;
        while row < h {
            let band_h = BAND.min(h - row);
            let inset_top = inset_at(row, r, h);
            let inset_bottom = inset_at(row + band_h - 1, r, h);
            let inset = inset_top.max(inset_bottom);
            let t = row as f32 / (h.max(2) - 1) as f32;
            let color = theme::mix(top, bottom, t);
            let width = w - inset * 2;
            if width > 0 {
                self.fill_rect(x + inset, y + row, width, band_h, color);
            }
            row += BAND;
        }
    }

    // ── linhas e círculos ────────────────────────────────────

    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        self.canvas.set_draw_color(color);
        let _ = self.canvas.draw_line((x1, y1), (x2, y2));
    }

    pub fn stroke_circle(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        self.canvas.set_draw_color(color);
        let steps = (r as f32 * 6.0).max(12.0) as i32;
        let mut previous: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let t = std::f32::consts::TAU * (i as f32 / steps as f32);
            let point = (
                cx + (t.cos() * r as f32).round() as i32,
                cy + (t.sin() * r as f32).round() as i32,
            );
            if let Some(prev) = previous {
                let _ = self.canvas.draw_line(prev, point);
            }
            previous = Some(point);
        }
    }

    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        let mut spans: Vec<Rect> = Vec::with_capacity((r * 2) as usize);
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt().round() as i32;
            if half > 0 {
                spans.push(Rect::new(cx - half, cy + dy, (half * 2) as u32, 1));
            }
        }
        self.canvas.set_draw_color(color);
        let _ = self.canvas.fill_rects(&spans);
    }

    // ── ícones ───────────────────────────────────────────────

    /// Desenha um ícone centrado em (cx, cy) com o tamanho dado em unidades de
    /// projeto. `size` é a aresta da caixa 24×24 escalada.
    pub fn icon(&mut self, name: &str, cx: i32, cy: i32, size: i32, color: Color) {
        let Some(icon) = icons::get(name) else {
            return;
        };
        let scale = size as f32 / 24.0;
        let map = |x: f32, y: f32| -> (i32, i32) {
            (
                cx - size / 2 + (x * scale).round() as i32,
                cy - size / 2 + (y * scale).round() as i32,
            )
        };

        for prim in icon.prims {
            match prim {
                Prim::Line(x1, y1, x2, y2) => {
                    let a = map(*x1, *y1);
                    let b = map(*x2, *y2);
                    self.line(a.0, a.1, b.0, b.1, color);
                }
                Prim::Poly(points, filled) => {
                    if points.len() < 2 {
                        continue;
                    }
                    if *filled {
                        self.fill_polygon(points, &map, color);
                    } else {
                        for pair in points.windows(2) {
                            let a = map(pair[0].0, pair[0].1);
                            let b = map(pair[1].0, pair[1].1);
                            self.line(a.0, a.1, b.0, b.1, color);
                        }
                    }
                }
                Prim::Circle(x, y, r) => {
                    let center = map(*x, *y);
                    let radius = (r * scale).round().max(1.0) as i32;
                    self.stroke_circle(center.0, center.1, radius, color);
                }
                Prim::Rect(x, y, w, h) => {
                    let a = map(*x, *y);
                    let b = map(x + w, y + h);
                    self.stroke_rect(a.0, a.1, b.0 - a.0, b.1 - a.1, color);
                }
            }
        }
    }

    fn fill_polygon(
        &mut self,
        points: &[(f32, f32)],
        map: &impl Fn(f32, f32) -> (i32, i32),
        color: Color,
    ) {
        let mapped: Vec<(i32, i32)> = points.iter().map(|(x, y)| map(*x, *y)).collect();
        let min_y = mapped.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = mapped.iter().map(|p| p.1).max().unwrap_or(0);

        let mut spans: Vec<Rect> = Vec::new();
        for y in min_y..=max_y {
            let mut crossings: Vec<i32> = Vec::new();
            for i in 0..mapped.len() {
                let (x1, y1) = mapped[i];
                let (x2, y2) = mapped[(i + 1) % mapped.len()];
                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    let t = (y - y1) as f32 / (y2 - y1) as f32;
                    crossings.push((x1 as f32 + t * (x2 - x1) as f32).round() as i32);
                }
            }
            crossings.sort_unstable();
            for pair in crossings.chunks(2) {
                if pair.len() == 2 && pair[1] > pair[0] {
                    spans.push(Rect::new(pair[0], y, (pair[1] - pair[0]) as u32, 1));
                }
            }
        }
        if !spans.is_empty() {
            self.canvas.set_draw_color(color);
            let _ = self.canvas.fill_rects(&spans);
        }
    }

    // ── texto ────────────────────────────────────────────────

    pub fn text_size(&mut self, text: &str, size: u16) -> (i32, i32) {
        let Some(font) = self.take_font(size) else {
            return (0, 0);
        };
        let measured = font.size_of(text).unwrap_or((0, 0));
        self.fonts.insert(size, font);
        (measured.0 as i32, measured.1 as i32)
    }

    /// Desenha texto e devolve a largura usada.
    pub fn text(&mut self, x: i32, y: i32, text: &str, size: u16, color: Color, align: Align) -> i32 {
        if text.is_empty() {
            return 0;
        }
        let Some(font) = self.take_font(size) else {
            return 0;
        };

        let rendered = font.render(text).blended(color);
        let mut width = 0;

        if let Ok(surface) = rendered {
            width = surface.width() as i32;
            if let Ok(texture) = self.creator.create_texture_from_surface(&surface) {
                let dx = match align {
                    Align::Left => x,
                    Align::Center => x - width / 2,
                    Align::Right => x - width,
                };
                let dst = Rect::new(dx, y, surface.width(), surface.height());
                let _ = self.canvas.copy(&texture, None, Some(dst));
            }
        }

        self.fonts.insert(size, font);
        width
    }

    pub fn text_left(&mut self, x: i32, y: i32, text: &str, size: u16, color: Color) -> i32 {
        self.text(x, y, text, size, color, Align::Left)
    }

    pub fn text_center(&mut self, cx: i32, y: i32, text: &str, size: u16, color: Color) -> i32 {
        self.text(cx, y, text, size, color, Align::Center)
    }

    pub fn text_right(&mut self, rx: i32, y: i32, text: &str, size: u16, color: Color) -> i32 {
        self.text(rx, y, text, size, color, Align::Right)
    }
}

/// Inset horizontal do canto arredondado na linha `row`.
fn corner_inset(row: i32, r: i32) -> i32 {
    let dy = (r - 1 - row) as f32;
    let rr = r as f32;
    (rr - (rr * rr - dy * dy).max(0.0).sqrt()).round() as i32
}

/// Inset considerando topo e base do retângulo.
fn inset_at(row: i32, r: i32, h: i32) -> i32 {
    if r == 0 {
        return 0;
    }
    if row < r {
        corner_inset(row, r)
    } else if row >= h - r {
        corner_inset(h - 1 - row, r)
    } else {
        0
    }
}

fn intersect(a: Clip, b: Clip) -> Option<Clip> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    if right <= x || bottom <= y {
        return None;
    }
    Some(Clip {
        x,
        y,
        w: right - x,
        h: bottom - y,
    })
}
