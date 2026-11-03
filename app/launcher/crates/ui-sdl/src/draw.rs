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

#[cfg(feature = "texto")]
use std::collections::HashMap;

use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::surface::Surface;
use sdl2::render::{BlendMode, WindowCanvas};
#[cfg(feature = "texto")]
use sdl2::render::TextureCreator;
#[cfg(feature = "texto")]
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::Window;
#[cfg(feature = "texto")]
use sdl2::video::WindowContext;

use crate::icons::{self, Prim};
use crate::theme;

/// Alinhamento horizontal do texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Rotação do desenho em relação ao espaço de projeto.
///
/// Existe porque o painel do aparelho é retrato (720x1280) enquanto a UI é
/// desenhada em 1280x720. Sem girar, o conteúdo sai deitado e com tarja preta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    None,
    /// 90° no sentido horário: o topo do projeto vai para a direita do painel.
    Cw90,
    /// 90° no sentido anti-horário: o topo do projeto vai para a esquerda do
    /// painel.
    Ccw90,
}

// Não existe constante dizendo qual sentido este aparelho usa. A orientação é
// um fato do sistema (montagem do painel), publicado em
// `/sys/class/graphics/fbcon/rotate`, e é lida por `crate::display`. Um app que
// carrega "o X55 precisa de anti-horário" fica errado na primeira revisão de
// hardware — e esse é o tipo de erro que o usuário percebe como "de ponta a
// cabeça" sem ter como diagnosticar.

/// Dimensões do espaço de projeto. `DESIGN_H` é o eixo que vira X do
/// dispositivo quando a rotação é aplicada.
const DESIGN_W: i32 = 1280;
const DESIGN_H: i32 = 720;

// ── matemática da rotação (pura, testável sem SDL) ──────────

/// Ponto do espaço de projeto → espaço de dispositivo.
pub fn rotate_point(rot: Rotation, x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotation::None => (x, y),
        Rotation::Cw90 => (DESIGN_H - y, x),
        Rotation::Ccw90 => (y, DESIGN_W - x),
    }
}

/// Inverso de [`rotate_point`].
pub fn unrotate_point(rot: Rotation, x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotation::None => (x, y),
        Rotation::Cw90 => (y, DESIGN_H - x),
        Rotation::Ccw90 => (DESIGN_W - y, x),
    }
}

/// Retângulo do espaço de projeto → espaço de dispositivo.
///
/// Um retângulo alinhado aos eixos continua retângulo sob múltiplo de 90°, então
/// largura e altura trocam e o canto é reposicionado. É isso que permite girar
/// canto arredondado, borda e gradiente sem tratamento especial.
pub fn rotate_rect(rot: Rotation, x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
    match rot {
        Rotation::None => (x, y, w, h),
        Rotation::Cw90 => (DESIGN_H - y - h, x, h, w),
        Rotation::Ccw90 => (y, DESIGN_W - x - w, h, w),
    }
}

/// Espaço lógico que o SDL deve usar para cada rotação.
///
/// Sem rotação é o espaço de projeto; com rotação é o do dispositivo, porque as
/// primitivas já emitem coordenadas giradas e o encaixe passa a ser exato (o SDL
/// só letterboxa se a proporção não bater).
fn logical_size_for(rot: Rotation) -> (u32, u32) {
    match rot {
        Rotation::None => (DESIGN_W as u32, DESIGN_H as u32),
        Rotation::Cw90 | Rotation::Ccw90 => (DESIGN_H as u32, DESIGN_W as u32),
    }
}

/// Interpreta `0`, `90`, `270`/`ccw`. `None` = não configurado.
fn rotacao_de_valor(valor: &str) -> Option<Rotation> {
    match valor.trim() {
        "0" => Some(Rotation::None),
        "90" | "cw" => Some(Rotation::Cw90),
        "270" | "ccw" => Some(Rotation::Ccw90),
        _ => None,
    }
}

/// De onde ler a rotação configurada, na ordem de precedência.
///
/// O arquivo existe para o aparelho: `giochi-init` inicia o launcher sem
/// variáveis de ambiente, então sem ele trocar o sentido exigiria recompilar.
/// Fica no mesmo diretório do brilho salvo, que o init já usa.
const ROTATION_FILE: &str = "/user/.config/giochi/rotation";

/// Só o arquivo — a variável de ambiente é lida uma vez, na abertura.
///
/// Separado porque o laço quer exatamente isso: mudanças em disco, não em
/// ambiente (que exigiria reiniciar o processo de qualquer forma).
pub fn rotacao_do_arquivo() -> Option<Rotation> {
    std::fs::read_to_string(ROTATION_FILE)
        .ok()
        .and_then(|t| rotacao_de_valor(&t))
}

/// Rotação e a fonte que a decidiu, na ordem de precedência documentada em
/// `crate::display`.
///
/// `formato_retrato` é o último recurso: só entra quando ninguém declarou nada.
fn rotacao_configurada(formato_retrato: bool) -> (Rotation, crate::display::Fonte) {
    use crate::display::{rotacao_do_sistema, Fonte};

    if let Ok(v) = std::env::var("GIOCHI_ROTATE") {
        if let Some(rot) = rotacao_de_valor(&v) {
            return (rot, Fonte::Ambiente);
        }
    }
    if let Some(rot) = rotacao_do_arquivo() {
        return (rot, Fonte::Usuario);
    }
    if let Some(rot) = rotacao_do_sistema() {
        return (rot, Fonte::Sistema);
    }

    // Ninguém declarou. Se o painel é retrato, algo precisa ser escolhido;
    // registra que foi palpite para não virar folclore depois.
    let rot = if formato_retrato {
        Rotation::Ccw90
    } else {
        Rotation::None
    };
    (rot, Fonte::Deduzido)
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
    #[cfg(feature = "texto")]
    creator: TextureCreator<WindowContext>,
    #[cfg(feature = "texto")]
    ttf: Option<&'static Sdl2TtfContext>,
    /// Fonte por tamanho. Guardadas sem empréstimo do `Painter`, por isso
    /// `'static`: ver `take_font`.
    #[cfg(feature = "texto")]
    fonts: HashMap<u16, Font<'static, 'static>>,
    #[cfg(feature = "texto")]
    font_path: &'static str,
    clip_stack: Vec<Option<Clip>>,
    rot: Rotation,
}

impl Painter {
    pub fn new(window: Window, font_path: &str) -> Result<Self, String> {
        // Sem a feature `texto`, o parâmetro existe só para manter a mesma
        // assinatura nos dois builds.
        let _ = font_path;

        let mut canvas = window
            .into_canvas()
            .present_vsync()
            .build()
            .map_err(|e| format!("canvas: {e}"))?;

        canvas.set_blend_mode(BlendMode::Blend);

        // Rotação: variável de ambiente, depois arquivo, depois dedução pelo
        // formato do output. Painel retrato com projeto paisagem só fecha
        // girando.
        let (ow, oh) = canvas
            .output_size()
            .unwrap_or((DESIGN_W as u32, DESIGN_H as u32));
        let (rot, fonte) = rotacao_configurada(oh > ow);
        eprintln!(
            "rotação: {rot:?} (fonte: {}) [output {ow}x{oh}]",
            fonte.descricao()
        );

        // Espaço de desenho fixo; o SDL letterboxa mantendo proporção. Com
        // rotação, o espaço lógico passa a ser o do dispositivo, porque as
        // primitivas já emitem coordenadas giradas e o encaixe é exato.
        let (lw, lh) = logical_size_for(rot);
        canvas
            .set_logical_size(lw, lh)
            .map_err(|e| format!("logical size: {e}"))?;

        #[cfg(feature = "texto")]
        let creator = canvas.texture_creator();

        #[cfg(feature = "texto")]
        {
            // `Sdl2TtfContext` fica vazado de propósito: precisa viver mais que
            // as fontes, e o programa termina antes disso importar.
            let ttf: Option<&'static Sdl2TtfContext> = match sdl2::ttf::init() {
                Ok(context) => Some(Box::leak(Box::new(context))),
                Err(e) => {
                    eprintln!("aviso: TTF indisponível ({e}); o texto não será desenhado");
                    None
                }
            };

            // O caminho precisa ser `'static` para a `Font` ser `'static`.
            let font_path: &'static str = Box::leak(font_path.to_string().into_boxed_str());

            Ok(Self {
                canvas,
                creator,
                ttf,
                fonts: HashMap::new(),
                font_path,
                clip_stack: Vec::new(),
                rot,
            })
        }

        #[cfg(not(feature = "texto"))]
        {
            eprintln!("aviso: compilado sem a feature `texto`; nenhum texto será desenhado");
            Ok(Self {
                canvas,
                clip_stack: Vec::new(),
                rot,
            })
        }
    }

    #[cfg(feature = "texto")]
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

    /// Rotação em uso agora.
    pub fn rotation(&self) -> Rotation {
        self.rot
    }

    /// Troca a rotação com o launcher já aberto.
    ///
    /// Existe para conferir o sentido no aparelho sem recompilar: quem olha a
    /// tela sabe em um segundo se está certo, e os dois sentidos diferem 180°.
    pub fn set_rotation(&mut self, rot: Rotation) -> Result<(), String> {
        if rot == self.rot {
            return Ok(());
        }
        self.rot = rot;
        let (lw, lh) = logical_size_for(rot);
        self.canvas
            .set_logical_size(lw, lh)
            .map_err(|e| format!("logical size: {e}"))?;
        self.clip_stack.clear();
        Ok(())
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
        let lx = ((x as f32 - off_x) / scale).round() as i32;
        let ly = ((y as f32 - off_y) / scale).round() as i32;
        // Com rotação, o espaço lógico é o do dispositivo: desfaz o giro para
        // que o toque volte a cair nas coordenadas de projeto usadas pelo layout.
        Some(self.unmap_point(lx, ly))
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
        let clip = self
            .clip_stack
            .last()
            .copied()
            .flatten()
            .map(|c| {
                let (x, y, w, h) = self.map_rect(c.x, c.y, c.w, c.h);
                Clip { x, y, w, h }
            });
        match clip {
            Some(c) => {
                let _ = self.canvas.set_clip_rect(Rect::new(c.x, c.y, c.w.max(0) as u32, c.h.max(0) as u32));
            }
            None => self.canvas.set_clip_rect(None),
        }
    }

    // ── retângulos ───────────────────────────────────────────

    /// Converte um retângulo do espaço de projeto para o do dispositivo.
    ///
    /// Gira 90° no sentido horário: `(x, y) → (ALT - y - h, x)` e os eixos
    /// trocam. Um retângulo continua retângulo, então canto arredondado, borda e
    /// gradiente não precisam de tratamento especial — basta entrar aqui.
    fn map_rect(&self, x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
        rotate_rect(self.rot, x, y, w, h)
    }

    fn map_point(&self, x: i32, y: i32) -> (i32, i32) {
        rotate_point(self.rot, x, y)
    }

    /// Inverso de `map_point`: do dispositivo para o projeto.
    fn unmap_point(&self, x: i32, y: i32) -> (i32, i32) {
        unrotate_point(self.rot, x, y)
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let (x, y, w, h) = self.map_rect(x, y, w, h);
        self.fill_rect_dev(x, y, w, h, color);
    }

    /// Retângulo com cantos arredondados, por varredura de linhas.
    pub fn fill_round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        let (x, y, w, h) = self.map_rect(x, y, w, h);
        self.fill_round_rect_dev(x, y, w, h, radius, color);
    }

    pub fn stroke_round_rect(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        let (x, y, w, h) = self.map_rect(x, y, w, h);
        self.stroke_round_rect_dev(x, y, w, h, radius, color);
    }

    // ── primitivas em espaço de dispositivo ──────────────────
    //
    // Tudo abaixo recebe coordenadas já transformadas. Existem para que as
    // funções públicas façam o mapeamento uma única vez: sem essa separação,
    // `stroke_round_rect` chamaria `fill_rect` e as coordenadas seriam giradas
    // duas vezes.

    fn fill_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.set_draw_color(color);
        let _ = self
            .canvas
            .fill_rect(Rect::new(x, y, w as u32, h as u32));
    }

    fn stroke_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.set_draw_color(color);
        let _ = self
            .canvas
            .draw_rect(Rect::new(x, y, w as u32, h as u32));
    }

    fn fill_round_rect_dev(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: i32,
        color: Color,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.fill_rect_dev(x, y, w, h, color);
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

    fn stroke_round_rect_dev(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: i32,
        color: Color,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.stroke_rect_dev(x, y, w, h, color);
            return;
        }
        // Retas centrais.
        self.fill_rect_dev(x + r, y, w - 2 * r, 1, color);
        self.fill_rect_dev(x + r, y + h - 1, w - 2 * r, 1, color);
        self.fill_rect_dev(x, y + r, 1, h - 2 * r, color);
        self.fill_rect_dev(x + w - 1, y + r, 1, h - 2 * r, color);
        // Cantos.
        self.arc_corner_dev(x + r, y + r, r, 180.0, 270.0, color);
        self.arc_corner_dev(x + w - r, y + r, r, 270.0, 360.0, color);
        self.arc_corner_dev(x + w - r, y + h - r, r, 0.0, 90.0, color);
        self.arc_corner_dev(x + r, y + h - r, r, 90.0, 180.0, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_corner_dev(&mut self, cx: i32, cy: i32, r: i32, from: f32, to: f32, color: Color) {
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
    ///
    /// Sob rotação, cada faixa é um retângulo e é girada junto: o gradiente
    /// acompanha a imagem sem precisar de eixo alternativo.
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

    fn stroke_circle_dev(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
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
        let (cx, cy) = self.map_point(cx, cy);
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
    ///
    /// O mapeamento do ícone já aplica a rotação no *ponto*, então o
    /// preenchimento por varredura roda no espaço do dispositivo. Isso funciona
    /// para polígono em qualquer ângulo, ao contrário de girar só o retângulo
    /// envolvente.
    pub fn icon(&mut self, name: &str, cx: i32, cy: i32, size: i32, color: Color) {
        let Some(icon) = icons::get(name) else {
            return;
        };
        let scale = size as f32 / 24.0;
        let rot = self.rot;
        let map = move |x: f32, y: f32| -> (i32, i32) {
            let px = cx - size / 2 + (x * scale).round() as i32;
            let py = cy - size / 2 + (y * scale).round() as i32;
            rotate_point(rot, px, py)
        };

        for prim in icon.prims {
            match prim {
                Prim::Line(x1, y1, x2, y2) => {
                    let a = map(*x1, *y1);
                    let b = map(*x2, *y2);
                    self.canvas.set_draw_color(color);
                    let _ = self.canvas.draw_line(a, b);
                }
                Prim::Poly(points, filled) => {
                    if points.len() < 2 {
                        continue;
                    }
                    if *filled {
                        self.fill_polygon(points, &map, color);
                    } else {
                        self.canvas.set_draw_color(color);
                        for pair in points.windows(2) {
                            let a = map(pair[0].0, pair[0].1);
                            let b = map(pair[1].0, pair[1].1);
                            let _ = self.canvas.draw_line(a, b);
                        }
                    }
                }
                Prim::Circle(x, y, r) => {
                    let center = map(*x, *y);
                    let radius = (r * scale).round().max(1.0) as i32;
                    self.stroke_circle_dev(center.0, center.1, radius, color);
                }
                Prim::Rect(x, y, w, h) => {
                    let a = map(*x, *y);
                    let b = map(x + w, y + h);
                    let (rx, ry) = (a.0.min(b.0), a.1.min(b.1));
                    self.stroke_rect_dev(rx, ry, (b.0 - a.0).abs(), (b.1 - a.1).abs(), color);
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

    #[cfg(feature = "texto")]
    pub fn text_size(&mut self, text: &str, size: u16) -> (i32, i32) {
        let Some(font) = self.take_font(size) else {
            return (0, 0);
        };
        let measured = font.size_of(text).unwrap_or((0, 0));
        self.fonts.insert(size, font);
        (measured.0 as i32, measured.1 as i32)
    }

    /// Sem TTF, o layout precisa de uma noção de altura para centralizar.
    /// Aproximar por `size` mantém as faixas de texto no lugar em vez de
    /// colapsar tudo na mesma linha.
    #[cfg(not(feature = "texto"))]
    pub fn text_size(&mut self, _text: &str, size: u16) -> (i32, i32) {
        (0, size as i32)
    }

    /// Desenha texto e devolve a largura usada.
    #[cfg(feature = "texto")]
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

    #[cfg(not(feature = "texto"))]
    pub fn text(
        &mut self,
        _x: i32,
        _y: i32,
        _text: &str,
        _size: u16,
        _color: Color,
        _align: Align,
    ) -> i32 {
        0
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

#[cfg(test)]
mod tests {
    use super::*;

    const TODOS: [Rotation; 3] = [Rotation::None, Rotation::Cw90, Rotation::Ccw90];

    #[test]
    fn le_os_valores_de_configuracao() {
        assert_eq!(rotacao_de_valor("0"), Some(Rotation::None));
        assert_eq!(rotacao_de_valor("90"), Some(Rotation::Cw90));
        assert_eq!(rotacao_de_valor("270"), Some(Rotation::Ccw90));
        assert_eq!(rotacao_de_valor("270\n"), Some(Rotation::Ccw90), "aceita newline");
        assert_eq!(rotacao_de_valor(" ccw "), Some(Rotation::Ccw90));
        assert_eq!(rotacao_de_valor(""), None);
        assert_eq!(rotacao_de_valor("lixo"), None);
        assert_eq!(rotacao_de_valor("180"), None, "180 não é suportado");
    }

    #[test]
    fn ida_e_volta_devolve_o_ponto() {
        for rot in TODOS {
            for (x, y) in [(0, 0), (1279, 0), (0, 719), (1279, 719), (640, 360)] {
                let (dx, dy) = rotate_point(rot, x, y);
                assert_eq!(
                    unrotate_point(rot, dx, dy),
                    (x, y),
                    "{rot:?} perdeu ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn rotacao_cobre_o_painel_inteiro() {
        // Os quatro cantos do projeto precisam cair nos quatro cantos do painel
        // retrato: se algum sair fora, aparece tarja preta.
        for rot in [Rotation::Cw90, Rotation::Ccw90] {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for (x, y) in [(0, 0), (DESIGN_W, 0), (0, DESIGN_H), (DESIGN_W, DESIGN_H)] {
                let (dx, dy) = rotate_point(rot, x, y);
                xs.push(dx);
                ys.push(dy);
            }
            xs.sort_unstable();
            ys.sort_unstable();
            assert_eq!((xs[0], xs[3]), (0, DESIGN_H), "{rot:?} eixo X");
            assert_eq!((ys[0], ys[3]), (0, DESIGN_W), "{rot:?} eixo Y");
        }
    }

    #[test]
    fn um_sentido_e_o_outro_mais_180_graus() {
        // Esta é a relação que explica o sintoma: girar para o lado errado não
        // "quase" acerta — entrega a imagem de cabeça para baixo, exatamente
        // 180°. Um giro de 180° no painel é (dx,dy) → (ALT-dx, LARG-dy).
        for (x, y) in [(0, 0), (100, 0), (0, 50), (1279, 719), (640, 360)] {
            let (cwx, cwy) = rotate_point(Rotation::Cw90, x, y);
            let (ccwx, ccwy) = rotate_point(Rotation::Ccw90, x, y);
            let de_cabeca_para_baixo = (DESIGN_H - ccwx, DESIGN_W - ccwy);
            assert_eq!(
                (cwx, cwy),
                de_cabeca_para_baixo,
                "({x},{y}): horário deve ser o anti-horário virado 180°"
            );
        }
    }

    #[test]
    fn retangulo_gira_trocando_largura_e_altura() {
        // Barra do topo: 1280x56 em (0,0). Anti-horário põe o topo à esquerda.
        let (x, y, w, h) = rotate_rect(Rotation::Ccw90, 0, 0, DESIGN_W, 56);
        assert_eq!((w, h), (56, DESIGN_W));
        assert_eq!(x, 0, "topo vai para a borda esquerda");
        assert_eq!(y, 0);

        // Rodapé: 1280x54 no fim do eixo Y.
        let (x, _, w, h) = rotate_rect(Rotation::Ccw90, 0, DESIGN_H - 54, DESIGN_W, 54);
        assert_eq!((w, h), (54, DESIGN_W));
        assert_eq!(x, DESIGN_H - 54, "rodapé vai para a borda direita");
    }

    #[test]
    fn retangulo_fica_dentro_do_painel() {
        for rot in [Rotation::Cw90, Rotation::Ccw90] {
            let (x, y, w, h) = rotate_rect(rot, 0, 0, DESIGN_W, DESIGN_H);
            assert_eq!((x, y), (0, 0), "{rot:?} origem");
            assert_eq!((w, h), (DESIGN_H, DESIGN_W), "{rot:?} tamanho");
        }
    }

    #[test]
    fn canto_do_retangulo_bate_com_o_ponto_girado() {
        // O canto superior-esquerdo do retângulo girado tem de coincidir com o
        // giro do canto superior-esquerdo do original — senão clip e desenho
        // divergem sob rotação.
        for rot in [Rotation::Cw90, Rotation::Ccw90] {
            let (x, y, w, h) = (10, 20, 100, 40);
            let (rx, ry, rw, rh) = rotate_rect(rot, x, y, w, h);
            let (px, py) = rotate_point(rot, x, y);
            // Sob rotação o "canto" que vira origem depende do sentido.
            let origem_esperada = match rot {
                Rotation::Cw90 => (rx + rw, ry),
                Rotation::Ccw90 => (rx, ry + rh),
                Rotation::None => (rx, ry),
            };
            assert_eq!(
                (px, py),
                origem_esperada,
                "{rot:?}: ponto girado não casa com o retângulo girado"
            );
        }
    }
}
