//! O `Painter` do launcher sobre a **giochi-engine do GIOCHI-ENGINE-3D**.
//!
//! ## O que muda, e o que não muda
//!
//! A API pública é a mesma do `crates/ui-sdl/src/draw.rs` de produção: é isso
//! que permite `views.rs` — 2.000 linhas de layout — compilar **sem uma linha de
//! alteração** e desenhar a mesma tela. A única coisa trocada é o que está
//! embaixo: `WindowCanvas` do `sdl2` cru → `RenderContext` da engine.
//!
//! ## Por que o crate da engine é o do repositório 3D
//!
//! O `GIOCHI-ENGINE-3D` é o workspace completo: o crate `giochi-engine` (2D,
//! API C-shaped com handles `u64`), o `giochi-engine-3d` (GLES/EGL), o
//! `giochi-engine-capi` (ABI em C) e o `giochi-engine-lua`. O `GIOCHI-ENGINE`
//! separado é a versão 0.1.0 da mesma engine 2D, com API baseada em tempos de
//! vida (`RenderContext<'_>`, `Texture<'a>`). O alvo é o 0.2.0.
//!
//! A consequência prática é que **não existe tempo de vida em lugar nenhum**:
//! textura é `u64`, criada com `create_texture_from_rgba` e liberada com
//! `destroy_texture`. O `Painter` cria a textura do texto, desenha e libera —
//! sem cache de textura, porque o cache mudaria o consumo de memória sem mudar
//! um pixel.
//!
//! A alternativa era reescrever as telas. Isso produziria duas UIs que começam
//! iguais e divergem na primeira correção — e o defeito apareceria só na tela,
//! como já aconteceu com o carrossel fora de sincronia e com o rodapé prometendo
//! `A — Jogar` onde `A` abria a ficha.
//!
//! ## Por que a matemática de rotação foi copiada, não compartilhada
//!
//! São 7 funções puras (`rotate_point`, `rotate_rect`, `projecao`…). Compartilhar
//! exigiria extrair um módulo do launcher de produção, e o pedido desta app é
//! justamente **não mexer nele**. As funções estão cobertas pelos mesmos testes
//! aqui, então uma divergência de comportamento reprova nos dois lados.
//!
//! ## O que a engine não tinha, e agora tem
//!
//! O porte precisou de quatro primitivas que a engine não expunha — `draw_line`,
//! `stroke_rect`, `fill_rects` e `set_blend_mode`. Todas entraram na engine com
//! teste de pixel; sem elas não há como desenhar uma UI (só preenchimento
//! sólido). O alfa é o caso mais visível: sem blend, toda cor com alfa < 255
//! sairia opaca, e o texto — que é textura com alfa — sairia com caixa preta em
//! volta das letras.

use std::collections::HashMap;

use giochi_engine::{Color as CorEngine, Rect as RectEngine, RenderContext, Vec2};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::ttf::{Font, Sdl2TtfContext};

/// Espaço de projeto de referência — a proporção (16:9), não o tamanho.
pub const DESIGN_W: i32 = 1280;
pub const DESIGN_H: i32 = 720;

/// Onde o app procura a fonte, em ordem. A mesma lista do launcher de produção:
/// o Buildroot é o que importa, e foi ele que já apareceu sem rótulo nenhum.
pub const CANDIDATOS_FONTE: [&str; 6] = [
    "assets/fonts/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans/DejaVuSans.ttf",
    "app/launcher/assets/fonts/DejaVuSans.ttf",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Rotação do desenho em relação ao espaço de projeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    None,
    Cw90,
    Ccw90,
}

/// Tamanhos envolvidos em desenhar num painel de tamanho qualquer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projecao {
    pub design: (i32, i32),
    pub logico: (u32, u32),
}

/// Espaço de projeto para um painel de `out` pixels.
///
/// Mesma regra do launcher de produção: o projeto **acompanha o painel**, então
/// a escala é sempre 1,0 e a fonte mantém o tamanho em pixels. Projetar em
/// 1280×720 fixo encolhia `F_TINY` de 9 px para 4,5 px num painel 640×480.
pub fn projecao(rot: Rotation, out: (u32, u32)) -> Projecao {
    let (ow, oh) = out;
    match rot {
        Rotation::None => Projecao {
            design: (ow as i32, oh as i32),
            logico: (ow, oh),
        },
        Rotation::Cw90 | Rotation::Ccw90 => Projecao {
            design: (oh as i32, ow as i32),
            logico: (ow, oh),
        },
    }
}

// ── matemática da rotação (pura, testável sem SDL) ──────────

pub fn rotate_point(rot: Rotation, design: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotation::None => (x, y),
        Rotation::Cw90 => (design.1 - y, x),
        Rotation::Ccw90 => (y, design.0 - x),
    }
}

pub fn unrotate_point(rot: Rotation, design: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotation::None => (x, y),
        Rotation::Cw90 => (y, design.1 - x),
        Rotation::Ccw90 => (design.0 - y, x),
    }
}

pub fn rotate_rect(
    rot: Rotation,
    design: (i32, i32),
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> (i32, i32, i32, i32) {
    match rot {
        Rotation::None => (x, y, w, h),
        Rotation::Cw90 => (design.1 - y - h, x, h, w),
        Rotation::Ccw90 => (y, design.0 - x - w, h, w),
    }
}

/// Interpreta `0`, `90`, `270`/`ccw`. `None` = não configurado.
pub fn rotacao_de_valor(valor: &str) -> Option<Rotation> {
    match valor.trim() {
        "0" => Some(Rotation::None),
        "90" | "cw" => Some(Rotation::Cw90),
        "270" | "ccw" => Some(Rotation::Ccw90),
        _ => None,
    }
}

const ROTATION_FILE: &str = "/user/.config/giochi/rotation";

pub fn rotacao_do_arquivo() -> Option<Rotation> {
    std::fs::read_to_string(ROTATION_FILE)
        .ok()
        .and_then(|t| rotacao_de_valor(&t))
}

/// O que o sistema declara em `/sys/class/graphics/fbcon/rotate`.
fn rotacao_do_sistema() -> Option<Rotation> {
    let texto = std::fs::read_to_string("/sys/class/graphics/fbcon/rotate").ok()?;
    match texto.trim() {
        "0" => Some(Rotation::None),
        "1" => Some(Rotation::Cw90),
        "3" => Some(Rotation::Ccw90),
        _ => None,
    }
}

/// Extrai `fbcon=rotate:N` de um `cmdline`.
///
/// Mesma regra do launcher de produção: o sysfs só perde para o `cmdline`
/// quando diz **zero**. Medido no X55 em 07/11/2026 (imagem `60e326f`): o sysfs
/// respondia `0` e o `cmdline` pedia `fbcon=rotate:3` — sem o cruzamento, a UI
/// saía deitada num painel retrato.
pub fn rotacao_do_cmdline(texto: &str) -> Option<u32> {
    texto
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("fbcon=rotate:"))
        .and_then(|v| v.parse::<u32>().ok())
}

fn rotacao_do_sistema_com_fonte() -> Option<(Rotation, Fonte)> {
    let sysfs = rotacao_do_sistema();
    if matches!(sysfs, Some(Rotation::None) | None) {
        if let Ok(texto) = std::fs::read_to_string("/proc/cmdline") {
            if let Some(pedido) = rotacao_do_cmdline(&texto) {
                if pedido != 0 {
                    let rot = match pedido {
                        1 => Some(Rotation::Cw90),
                        3 => Some(Rotation::Ccw90),
                        _ => None,
                    };
                    if let Some(rot) = rot {
                        eprintln!(
                            "aviso: fbcon/rotate diz 0 mas o cmdline pede fbcon=rotate:{pedido} — usando o cmdline"
                        );
                        return Some((rot, Fonte::SistemaCmdline));
                    }
                }
            }
        }
    }
    sysfs.map(|rot| (rot, Fonte::Sistema))
}

/// Recorte retangular, **em espaço de projeto**.
///
/// Em espaço de projeto e não de dispositivo de propósito: assim o recorte vale
/// igual com e sem giro, e o texto pode ser recortado pela *origem* da textura
/// em vez de exigir recorte do canvas — que a engine não expõe.
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Clip {
    fn intersecao(self, outro: Clip) -> Option<Clip> {
        let x = self.x.max(outro.x);
        let y = self.y.max(outro.y);
        let right = (self.x + self.w).min(outro.x + outro.w);
        let bottom = (self.y + self.h).min(outro.y + outro.h);
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
}

fn para_engine(c: Color) -> CorEngine {
    CorEngine::new(c.r, c.g, c.b, c.a)
}

/// Retângulo em espaço de dispositivo.
///
/// A engine usa `Rect` de `f32` (é o tipo dela, `#[repr(C)]`, compartilhado com
/// o C ABI); o launcher raciocina em pixels inteiros. A conversão fica aqui,
/// num lugar só, para não espalhar `as f32` pelas primitivas.
fn rect_dev(x: i32, y: i32, w: i32, h: i32) -> RectEngine {
    RectEngine::new(x as f32, y as f32, w as f32, h as f32)
}

pub struct Painter {
    render: RenderContext,
    ttf: Option<&'static Sdl2TtfContext>,
    fonts: HashMap<u16, Font<'static, 'static>>,
    font_path: &'static str,
    avisou_fonte: bool,
    clip_stack: Vec<Option<Clip>>,
    rot: Rotation,
    design: (i32, i32),
    /// Chamadas de desenho emitidas ao backend desde a criação.
    ///
    /// Contado **aqui**, no ponto em que se chama o `RenderContext`, e não no
    /// chamador. A primeira versão contava no módulo de efeitos e mentia: com
    /// uma mutação que desfazia a batelada (uma chamada por partícula) o número
    /// continuava 25, porque o contador somava "quantas vezes eu pedi um lote".
    /// Um contador que mede a intenção não mede o custo.
    desenhos: u32,
}

impl Painter {
    pub fn new(render: RenderContext, font_path: &str) -> Result<Self, String> {
        // O alfa do **alvo** é habilitado pela própria engine, na criação do
        // canvas: textura já nascia em `Blend`, o canvas não. Ver
        // `build_canvas` no crate da engine.
        let (ow, oh) = render.size();
        let (rot, fonte) = rotacao_configurada(oh > ow);
        eprintln!(
            "rotação: {rot:?} (fonte: {}) [output {ow}x{oh}]",
            fonte.descricao()
        );
        // Qual backend abriu **de fato**, não qual foi pedido. Carregar `libEGL`
        // em `/proc/maps` não prova nada: o vídeo `kmsdrm` usa EGL/GBM em
        // qualquer renderer. Quem responde é `SDL_GetRendererInfo`.
        eprintln!(
            "renderer: {} ({}{})",
            render.renderer_name(),
            if render.renderer_is_accelerated() { "acelerado" } else { "software" },
            ""
        );

        let proj = projecao(rot, (ow, oh));
        eprintln!(
            "output {ow}x{oh} desenha em projeto {}x{} (escala 1,0)",
            proj.design.0, proj.design.1
        );

        // `Sdl2TtfContext` não pode ser inicializado duas vezes. O launcher de
        // produção tem o mesmo cuidado, e o defeito que ele evita é medido:
        // a segunda abertura devolvia a interface **sem nenhum texto**.
        thread_local! {
            static TTF: std::cell::RefCell<Option<&'static Sdl2TtfContext>> =
                const { std::cell::RefCell::new(None) };
        }
        let ttf: Option<&'static Sdl2TtfContext> = TTF.with(|celula| {
            let mut guardado = celula.borrow_mut();
            if guardado.is_none() {
                match sdl2::ttf::init() {
                    Ok(contexto) => *guardado = Some(Box::leak(Box::new(contexto))),
                    Err(e) => {
                        eprintln!("aviso: TTF indisponível ({e}); o texto não será desenhado");
                        return None;
                    }
                }
            }
            *guardado
        });

        let font_path: &'static str = Box::leak(font_path.to_string().into_boxed_str());

        Ok(Self {
            render,
            ttf,
            fonts: HashMap::new(),
            font_path,
            avisou_fonte: false,
            clip_stack: Vec::new(),
            rot,
            design: proj.design,
            desenhos: 0,
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
                if !self.avisou_fonte {
                    self.avisou_fonte = true;
                    eprintln!("aviso: fonte não carregada ({e})");
                    eprintln!("  procurei em: {}", CANDIDATOS_FONTE.join(", "));
                }
                None
            }
        }
    }

    // ── quadro ───────────────────────────────────────────────

    pub fn clear(&mut self, color: Color) {
        self.render.clear(para_engine(color));
    }

    pub fn present(&mut self) {
        self.render.present();
    }

    pub fn rotation(&self) -> Rotation {
        self.rot
    }

    pub fn set_rotation(&mut self, rot: Rotation) -> Result<(), String> {
        if rot == self.rot {
            return Ok(());
        }
        self.rot = rot;
        self.design = projecao(rot, self.render.size()).design;
        self.clip_stack.clear();
        Ok(())
    }

    /// Chamadas de desenho emitidas até agora.
    pub fn desenhos(&self) -> u32 {
        self.desenhos
    }

    /// `true` quando o desenho foi feito; o contador não é escrito à toa.
    pub fn logical_size(&self) -> (u32, u32) {
        (self.design.0.max(1) as u32, self.design.1.max(1) as u32)
    }

    pub fn painel(&self) -> (u32, u32) {
        let (dw, dh) = self.logical_size();
        match self.rot {
            Rotation::None => (dw, dh),
            Rotation::Cw90 | Rotation::Ccw90 => (dh, dw),
        }
    }

    /// Grava o quadro atual em BMP de 24 bpp.
    ///
    /// Lê **antes** de apresentar: com backend acelerado o swap esvazia o
    /// buffer e a captura sai preta — defeito já medido neste projeto no X35H e
    /// no R35S.
    pub fn save_frame(&mut self, path: &str) -> Result<(), String> {
        let (w, h) = self.painel();
        let rgba = self
            .render
            .read_pixels(RectEngine::new(0.0, 0.0, w as f32, h as f32))
            .map_err(|e| format!("read_pixels: {e}"))?;
        gravar_bmp(path, w, h, &rgba)
    }

    pub fn window_to_logical(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (out_w, out_h) = self.render.size();
        let (lw, lh) = self.logical_size();
        if out_w == 0 || out_h == 0 {
            return None;
        }
        let scale = (out_w as f32 / lw as f32).min(out_h as f32 / lh as f32);
        let off_x = (out_w as f32 - lw as f32 * scale) / 2.0;
        let off_y = (out_h as f32 - lh as f32 * scale) / 2.0;
        let lx = ((x as f32 - off_x) / scale).round() as i32;
        let ly = ((y as f32 - off_y) / scale).round() as i32;
        Some(unrotate_point(self.rot, self.design, lx, ly))
    }

    // ── recorte ──────────────────────────────────────────────

    pub fn push_clip(&mut self, clip: Clip) {
        let anterior = self.clip_stack.last().copied().flatten();
        let mesclado = match anterior {
            Some(p) => p.intersecao(clip),
            None => Some(clip),
        };
        self.clip_stack.push(mesclado);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn clip_projeto(&self) -> Option<Clip> {
        self.clip_stack.last().copied().flatten()
    }

    /// O mesmo recorte em espaço de dispositivo.
    fn clip_dev(&self) -> Option<(i32, i32, i32, i32)> {
        self.clip_projeto()
            .map(|c| rotate_rect(self.rot, self.design, c.x, c.y, c.w, c.h))
    }

    // ── retângulos ───────────────────────────────────────────

    fn map_rect(&self, x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
        rotate_rect(self.rot, self.design, x, y, w, h)
    }

    fn map_point(&self, x: i32, y: i32) -> (i32, i32) {
        rotate_point(self.rot, self.design, x, y)
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let (x, y, w, h) = self.map_rect(x, y, w, h);
        self.fill_rect_dev(x, y, w, h, color);
    }

    /// Vários retângulos de **uma vez**, num único `fill_rects` da engine.
    ///
    /// Existe para as scanlines dos efeitos: 160 retângulos de 1 px em 160
    /// `draw_rect` seriam 160 chamadas de desenho por quadro; em lote é **uma**.
    /// O custo continua sendo a área desenhada — o que o lote evita é a
    /// chamada, não o pixel.
    ///
    /// Com recorte ativo cai para retângulo a retângulo: `fill_rects` da engine
    /// não recorta, e um efeito que ignorasse o recorte apareceria por cima de
    /// um overlay que deveria cobri-lo.
    pub fn fill_rects(&mut self, rects: &[(i32, i32, i32, i32)], color: Color) {
        if self.clip_dev().is_some() {
            for &(x, y, w, h) in rects {
                self.fill_rect(x, y, w, h, color);
            }
            return;
        }
        let mut lote: Vec<RectEngine> = Vec::with_capacity(rects.len());
        for &(x, y, w, h) in rects {
            if w <= 0 || h <= 0 {
                continue;
            }
            let (x, y, w, h) = self.map_rect(x, y, w, h);
            lote.push(rect_dev(x, y, w, h));
        }
        if !lote.is_empty() {
            self.desenhos += 1;
            self.render.fill_rects(&lote, para_engine(color));
        }
    }

    /// Vários círculos de **uma vez**, num único `fill_rects`.
    ///
    /// `fill_circle` faz um `fill_rects` por círculo, e cada um é uma chamada de
    /// volta ao backend. Desenhar 130 brasas custava 129 chamadas — medido no
    /// X35H, +2,9 ms por quadro. Aqui as varreduras de todos os círculos vão num
    /// lote só, então 130 brasas custam **uma**.
    ///
    /// Os círculos precisam ter a **mesma cor**: `fill_rects` recebe uma cor para
    /// o lote inteiro. Quem tem alfa variável separa em faixas e chama uma vez
    /// por faixa.
    pub fn fill_circles(&mut self, circulos: &[(i32, i32, i32)], color: Color) {
        if circulos.is_empty() {
            return;
        }
        if self.clip_dev().is_some() {
            for &(cx, cy, r) in circulos {
                self.fill_circle(cx, cy, r, color);
            }
            return;
        }
        let mut lote: Vec<RectEngine> = Vec::with_capacity(circulos.len() * 8);
        for &(cx, cy, r) in circulos {
            if r <= 0 {
                continue;
            }
            let (cx, cy) = self.map_point(cx, cy);
            for dy in -r..=r {
                let meia = ((r * r - dy * dy) as f32).sqrt().round() as i32;
                if meia > 0 {
                    lote.push(rect_dev(cx - meia, cy + dy, meia * 2, 1));
                }
            }
        }
        if !lote.is_empty() {
            self.desenhos += 1;
            self.render.fill_rects(&lote, para_engine(color));
        }
    }

    /// Linha em espaço de projeto.
    ///
    /// `line_dev` já existia (os ícones usam), mas era privada: os efeitos
    /// precisam de linha solta, e expor a versão em espaço de projeto é o que
    /// impede a camada de efeitos de repetir a matemática de rotação.
    pub fn line(&mut self, a: (i32, i32), b: (i32, i32), color: Color) {
        let a = self.map_point(a.0, a.1);
        let b = self.map_point(b.0, b.1);
        self.line_dev(a, b, color);
    }

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
    // Recebem coordenadas já giradas. As públicas mapeiam **uma vez**; sem essa
    // separação, `stroke_round_rect` chamaria `fill_rect` e as coordenadas
    // girariam duas vezes — o defeito clássico de porte como este.

    fn fill_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (x, y, w, h) = match self.clip_dev() {
            Some(c) => {
                let c = Clip {
                    x: c.0,
                    y: c.1,
                    w: c.2,
                    h: c.3,
                };
                let alvo = Clip { x, y, w, h };
                match alvo.intersecao(c) {
                    Some(recorte) => (recorte.x, recorte.y, recorte.w, recorte.h),
                    None => return,
                }
            }
            None => (x, y, w, h),
        };
        self.desenhos += 1;
        self.render.draw_rect(rect_dev(x, y, w, h), para_engine(color));
    }

    fn stroke_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        if self.clip_dev().is_some() {
            self.fill_rect_dev(x, y, w, 1, color);
            self.fill_rect_dev(x, y + h - 1, w, 1, color);
            self.fill_rect_dev(x, y, 1, h, color);
            self.fill_rect_dev(x + w - 1, y, 1, h, color);
            return;
        }
        self.desenhos += 1;
        self.render.stroke_rect(rect_dev(x, y, w, h), para_engine(color));
    }

    fn fill_round_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.fill_rect_dev(x, y, w, h, color);
            return;
        }

        let mut spans: Vec<RectEngine> = Vec::with_capacity(h as usize);
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
                spans.push(rect_dev(x + inset, y + row, width, 1));
            }
        }

        if let Some(c) = self.clip_dev() {
            let clip = Clip {
                x: c.0,
                y: c.1,
                w: c.2,
                h: c.3,
            };
            spans = spans
                .into_iter()
                .filter_map(|s| {
                    let recorte = Clip {
                        x: s.x.round() as i32,
                        y: s.y.round() as i32,
                        w: s.w.round() as i32,
                        h: s.h.round() as i32,
                    }
                    .intersecao(clip)?;
                    Some(rect_dev(recorte.x, recorte.y, recorte.w, recorte.h))
                })
                .collect();
        }

        self.desenhos += 1;
        self.render.fill_rects(&spans, para_engine(color));
    }

    fn stroke_round_rect_dev(&mut self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.stroke_rect_dev(x, y, w, h, color);
            return;
        }
        self.fill_rect_dev(x + r, y, w - 2 * r, 1, color);
        self.fill_rect_dev(x + r, y + h - 1, w - 2 * r, 1, color);
        self.fill_rect_dev(x, y + r, 1, h - 2 * r, color);
        self.fill_rect_dev(x + w - 1, y + r, 1, h - 2 * r, color);
        self.arc_corner_dev(x + r, y + r, r, 180.0, 270.0, color);
        self.arc_corner_dev(x + w - r, y + r, r, 270.0, 360.0, color);
        self.arc_corner_dev(x + w - r, y + h - r, r, 0.0, 90.0, color);
        self.arc_corner_dev(x + r, y + h - r, r, 90.0, 180.0, color);
    }

    fn line_dev(&mut self, a: (i32, i32), b: (i32, i32), color: Color) {
        if self.clip_dev().is_some() {
            // Com recorte, a linha é rasterizada ponto a ponto: a engine não
            // expõe `set_clip_rect`, e desenhar a linha inteira furaria o
            // recorte da lista de busca.
            for ponto in bresenham(a, b) {
                self.fill_rect_dev(ponto.0, ponto.1, 1, 1, color);
            }
            return;
        }
        self.desenhos += 1;
        self.render.draw_line(
            Vec2::new(a.0 as f32, a.1 as f32),
            Vec2::new(b.0 as f32, b.1 as f32),
            para_engine(color),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_corner_dev(&mut self, cx: i32, cy: i32, r: i32, from: f32, to: f32, color: Color) {
        let steps = (r as f32 * 1.6).max(6.0) as i32;
        let mut previous: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let t = from + (to - from) * (i as f32 / steps as f32);
            let rad = t.to_radians();
            let point = (
                cx + (rad.cos() * r as f32).round() as i32,
                cy + (rad.sin() * r as f32).round() as i32,
            );
            if let Some(prev) = previous {
                self.line_dev(prev, point, color);
            }
            previous = Some(point);
        }
    }

    /// Gradiente vertical em faixas de 4 px, dentro de um retângulo arredondado.
    ///
    /// Faixas e não shader: o backend `software` do SDL é o plano B do RK3326 e
    /// não tem shader. 4 px de altura tornam a diferença invisível.
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
            let color = crate::theme::mix(top, bottom, t);
            let width = w - inset * 2;
            if width > 0 {
                self.fill_rect(x + inset, y + row, width, band_h, color);
            }
            row += BAND;
        }
    }

    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        let (cx, cy) = self.map_point(cx, cy);
        let mut spans: Vec<RectEngine> = Vec::with_capacity((r * 2) as usize);
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt().round() as i32;
            if half > 0 {
                spans.push(rect_dev(cx - half, cy + dy, half * 2, 1));
            }
        }
        if let Some(c) = self.clip_dev() {
            let clip = Clip {
                x: c.0,
                y: c.1,
                w: c.2,
                h: c.3,
            };
            spans = spans
                .into_iter()
                .filter_map(|s| {
                    let recorte = Clip {
                        x: s.x.round() as i32,
                        y: s.y.round() as i32,
                        w: s.w.round() as i32,
                        h: s.h.round() as i32,
                    }
                    .intersecao(clip)?;
                    Some(rect_dev(recorte.x, recorte.y, recorte.w, recorte.h))
                })
                .collect();
        }
        self.desenhos += 1;
        self.render.fill_rects(&spans, para_engine(color));
    }

    fn stroke_circle_dev(&mut self, cx: i32, cy: i32, r: i32, color: Color) {
        if r <= 0 {
            return;
        }
        let steps = (r as f32 * 6.0).max(12.0) as i32;
        let mut previous: Option<(i32, i32)> = None;
        for i in 0..=steps {
            let t = std::f32::consts::TAU * (i as f32 / steps as f32);
            let point = (
                cx + (t.cos() * r as f32).round() as i32,
                cy + (t.sin() * r as f32).round() as i32,
            );
            if let Some(prev) = previous {
                self.line_dev(prev, point, color);
            }
            previous = Some(point);
        }
    }

    // ── ícones ───────────────────────────────────────────────

    pub fn icon(&mut self, name: &str, cx: i32, cy: i32, size: i32, color: Color) {
        let Some(icon) = crate::icons::get(name) else {
            return;
        };
        let scale = size as f32 / 24.0;
        let rot = self.rot;
        let design = self.design;
        let map = move |x: f32, y: f32| -> (i32, i32) {
            let px = cx - size / 2 + (x * scale).round() as i32;
            let py = cy - size / 2 + (y * scale).round() as i32;
            rotate_point(rot, design, px, py)
        };

        for prim in icon.prims {
            match prim {
                crate::icons::Prim::Line(x1, y1, x2, y2) => {
                    let a = map(*x1, *y1);
                    let b = map(*x2, *y2);
                    self.line_dev(a, b, color);
                }
                crate::icons::Prim::Poly(points, filled) => {
                    if points.len() < 2 {
                        continue;
                    }
                    if *filled {
                        self.fill_polygon(points, &map, color);
                    } else {
                        for pair in points.windows(2) {
                            let a = map(pair[0].0, pair[0].1);
                            let b = map(pair[1].0, pair[1].1);
                            self.line_dev(a, b, color);
                        }
                    }
                }
                crate::icons::Prim::Circle(x, y, r) => {
                    let center = map(*x, *y);
                    let radius = (r * scale).round().max(1.0) as i32;
                    self.stroke_circle_dev(center.0, center.1, radius, color);
                }
                crate::icons::Prim::Rect(x, y, w, h) => {
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

        let mut spans: Vec<RectEngine> = Vec::new();
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
                    spans.push(rect_dev(pair[0], y, pair[1] - pair[0], 1));
                }
            }
        }
        if !spans.is_empty() {
            self.desenhos += 1;
            self.render.fill_rects(&spans, para_engine(color));
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

    pub fn text(&mut self, x: i32, y: i32, text: &str, size: u16, color: Color, align: Align) -> i32 {
        if text.is_empty() {
            return 0;
        }
        let Some(font) = self.take_font(size) else {
            return 0;
        };

        let mut width = 0;
        match font.render(text).blended(color) {
            Err(e) => self.reclamar(&format!("render do texto falhou: {e}")),
            Ok(surface) => {
                let (tw, th) = (surface.width() as i32, surface.height() as i32);
                width = tw;
                let destino = match align {
                    Align::Left => x,
                    Align::Center => x - tw / 2,
                    Align::Right => x - tw,
                };

                // Recorte em espaço de projeto, aplicado na **origem** da
                // textura: a engine não expõe `set_clip_rect`, e a lista de
                // busca depende de recortar.
                let (visivel, origem) = match self.clip_projeto() {
                    Some(clip) => {
                        let corpo = Clip { x: destino, y, w: tw, h: th };
                        match corpo.intersecao(clip) {
                            Some(c) => (
                                c,
                                Clip { x: c.x - destino, y: c.y - y, w: c.w, h: c.h },
                            ),
                            None => {
                                self.fonts.insert(size, font);
                                return width;
                            }
                        }
                    }
                    None => (
                        Clip { x: destino, y, w: tw, h: th },
                        Clip { x: 0, y: 0, w: tw, h: th },
                    ),
                };

                if let Err(e) = self.desenhar_textura(&surface, tw, th, visivel, origem) {
                    self.reclamar(&e);
                }
            }
        }

        self.fonts.insert(size, font);
        width
    }

    /// Reporta a primeira falha de texto e depois cala.
    ///
    /// Uma vez por processo: `text` roda centenas de vezes por quadro e um
    /// caminho quebrado imprimiria milhares de linhas iguais, soterração que
    /// este projeto já sofreu com a fonte.
    fn reclamar(&mut self, mensagem: &str) {
        if !self.avisou_fonte {
            self.avisou_fonte = true;
            eprintln!("aviso: {mensagem}");
        }
    }

    fn desenhar_textura(
        &mut self,
        surface: &sdl2::surface::Surface<'_>,
        tw: i32,
        th: i32,
        visivel: Clip,
        origem: Clip,
    ) -> Result<(), String> {
        // RGBA32 e não ABGR8888: `create_texture_from_rgba` da engine espera
        // RGBA na ordem de bytes, e converter para ABGR aqui inverteria canais.
        let convertida = surface
            .convert_format(PixelFormatEnum::RGBA32)
            .map_err(|e| format!("convert_format: {e}"))?;
        let pixels = convertida
            .without_lock()
            .ok_or_else(|| "surface sem acesso direto aos pixels".to_string())?;
        let pitch = convertida.pitch() as usize;
        let row_bytes = tw as usize * 4;
        if row_bytes == 0 || pixels.len() < pitch * th as usize {
            return Err(format!(
                "pixels insuficientes: {} bytes, esperado {}",
                pixels.len(),
                pitch * th as usize
            ));
        }
        let mut plana = Vec::with_capacity(row_bytes * th as usize);
        for row in 0..th as usize {
            let inicio = row * pitch;
            plana.extend_from_slice(&pixels[inicio..inicio + row_bytes]);
        }

        // Textura por handle: criar, desenhar e liberar na mesma chamada. Sem
        // cache porque o cache mudaria o consumo de memória sem mudar um pixel —
        // e o objetivo deste app é provar o desenho, não otimizar texto.
        let textura = self
            .render
            .create_texture_from_rgba(tw as u32, th as u32, &plana)
            .map_err(|e| format!("create_texture_from_rgba: {e}"))?;

        // O destino NÃO é o retângulo girado: `copy_ex` escala a textura para
        // caber no destino e só depois gira. Medidas trocadas comprimem o texto
        // antes de girar — foi o bug que só a contagem de pixels claros pegou.
        let girado = rotate_rect(self.rot, self.design, visivel.x, visivel.y, visivel.w, visivel.h);
        let angle = match self.rot {
            Rotation::None => 0.0,
            Rotation::Cw90 => 90.0,
            Rotation::Ccw90 => 270.0,
        };
        let dst = match self.rot {
            Rotation::None => rect_dev(visivel.x, visivel.y, visivel.w, visivel.h),
            _ => {
                let (cx, cy) = (girado.0 + girado.2 / 2, girado.1 + girado.3 / 2);
                rect_dev(
                    cx - visivel.w / 2,
                    cy - visivel.h / 2,
                    visivel.w,
                    visivel.h,
                )
            }
        };
        let src = rect_dev(origem.x, origem.y, origem.w, origem.h);
        self.desenhos += 1;
        let resultado = self
            .render
            .draw_texture_ex(textura, src, dst, angle, false, false)
            .map_err(|e| format!("draw_texture_ex: {e}"));
        self.render.destroy_texture(textura);
        resultado
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

/// De onde veio a rotação, para o log dizer a verdade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fonte {
    Ambiente,
    Usuario,
    Sistema,
    /// O sysfs dizia `0` e o `cmdline` pedia rotação.
    SistemaCmdline,
    Deduzido,
}

impl Fonte {
    pub fn descricao(self) -> &'static str {
        match self {
            Fonte::Ambiente => "GIOCHI_ROTATE",
            Fonte::Usuario => "arquivo do usuário",
            Fonte::Sistema => "sistema (fbcon/rotate)",
            Fonte::SistemaCmdline => "sistema (cmdline; fbcon/rotate dizia 0)",
            Fonte::Deduzido => "deduzido do formato do painel",
        }
    }
}

fn rotacao_configurada(formato_retrato: bool) -> (Rotation, Fonte) {
    if let Ok(v) = std::env::var("GIOCHI_ROTATE") {
        if let Some(rot) = rotacao_de_valor(&v) {
            return (rot, Fonte::Ambiente);
        }
    }
    if let Some(rot) = rotacao_do_arquivo() {
        return (rot, Fonte::Usuario);
    }
    if let Some((rot, fonte)) = rotacao_do_sistema_com_fonte() {
        return (rot, fonte);
    }
    let rot = if formato_retrato {
        Rotation::Ccw90
    } else {
        Rotation::None
    };
    (rot, Fonte::Deduzido)
}

/// Inset horizontal do canto arredondado na linha `row`.
fn corner_inset(row: i32, r: i32) -> i32 {
    let dy = (r - 1 - row) as f32;
    let rr = r as f32;
    (rr - (rr * rr - dy * dy).max(0.0).sqrt()).round() as i32
}

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

/// Rasterização de linha por Bresenham, para o caminho com recorte.
fn bresenham(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut erro = dx + dy;
    let mut pontos = Vec::new();
    loop {
        pontos.push((x0, y0));
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * erro;
        if e2 >= dy {
            erro += dy;
            x0 += sx;
        }
        if e2 <= dx {
            erro += dx;
            y0 += sy;
        }
    }
    pontos
}

/// Escreve BMP de 24 bpp, de baixo para cima, a partir de pixels RGBA.
///
/// 24 bpp porque é o que o `verificar-frame.mjs` lê (aceita 24 e 32), e porque
/// é o que o `Surface::save_bmp` do SDL produz — assim os dois lados da
/// comparação estão no mesmo formato.
fn gravar_bmp(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let linha_bruta = (w * 3) as usize;
    let padding = (4 - linha_bruta % 4) % 4;
    let linha = linha_bruta + padding;
    let tamanho_imagem = linha * h as usize;
    let tamanho_arquivo = 54 + tamanho_imagem;

    let mut out = Vec::with_capacity(tamanho_arquivo);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(tamanho_arquivo as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(tamanho_imagem as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..h).rev() {
        let inicio = (y * w * 4) as usize;
        for x in 0..w as usize {
            let p = inicio + x * 4;
            out.push(rgba[p + 2]);
            out.push(rgba[p + 1]);
            out.push(rgba[p]);
        }
        out.extend(std::iter::repeat_n(0u8, padding));
    }

    std::fs::write(path, out).map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODOS: [Rotation; 3] = [Rotation::None, Rotation::Cw90, Rotation::Ccw90];
    const REF: (i32, i32) = (DESIGN_W, DESIGN_H);

    #[test]
    fn le_os_valores_de_configuracao() {
        assert_eq!(rotacao_de_valor("0"), Some(Rotation::None));
        assert_eq!(rotacao_de_valor("90"), Some(Rotation::Cw90));
        assert_eq!(rotacao_de_valor("270"), Some(Rotation::Ccw90));
        assert_eq!(rotacao_de_valor("270\n"), Some(Rotation::Ccw90));
        assert_eq!(rotacao_de_valor(" ccw "), Some(Rotation::Ccw90));
        assert_eq!(rotacao_de_valor(""), None);
        assert_eq!(rotacao_de_valor("lixo"), None);
        assert_eq!(rotacao_de_valor("180"), None, "180 não é suportado");
    }

    #[test]
    fn ida_e_volta_devolve_o_ponto() {
        for rot in TODOS {
            for (x, y) in [(0, 0), (1279, 0), (0, 719), (1279, 719), (640, 360)] {
                let (dx, dy) = rotate_point(rot, REF, x, y);
                assert_eq!(unrotate_point(rot, REF, dx, dy), (x, y), "{rot:?} perdeu ({x},{y})");
            }
        }
    }

    #[test]
    fn projecao_acompanha_o_painel_e_nao_encolhe() {
        for (w, h) in [(640u32, 480u32), (800, 600), (1280, 720)] {
            let p = projecao(Rotation::None, (w, h));
            assert_eq!(p.design, (w as i32, h as i32));
            assert_eq!(p.logico, (w, h));
        }
    }

    #[test]
    fn projecao_com_giro_troca_as_medidas() {
        let p = projecao(Rotation::Ccw90, (720, 1280));
        assert_eq!(p.design, (1280, 720));
        assert_eq!(p.logico, (720, 1280));
    }

    #[test]
    fn rotacao_cobre_o_painel_inteiro() {
        for rot in [Rotation::Cw90, Rotation::Ccw90] {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for (x, y) in [(0, 0), (DESIGN_W, 0), (0, DESIGN_H), (DESIGN_W, DESIGN_H)] {
                let (dx, dy) = rotate_point(rot, REF, x, y);
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
    fn le_a_rotacao_do_cmdline_e_nao_confunde_ausencia_com_zero() {
        assert_eq!(rotacao_do_cmdline("root=/dev/x fbcon=rotate:3"), Some(3));
        assert_eq!(rotacao_do_cmdline("console=ttyS2,1500000n8"), None);
        assert_eq!(rotacao_do_cmdline("fbcon=rotate:lixo"), None);
    }

    #[test]
    fn recorte_intersecta_em_espaco_de_projeto() {
        let a = Clip { x: 0, y: 0, w: 100, h: 100 };
        let b = Clip { x: 50, y: 50, w: 100, h: 100 };
        let c = a.intersecao(b).expect("devem se cruzar");
        assert_eq!((c.x, c.y, c.w, c.h), (50, 50, 50, 50));
        assert!(a.intersecao(Clip { x: 200, y: 0, w: 10, h: 10 }).is_none());
    }

    #[test]
    fn bresenham_comeca_e_termina_nos_extremos() {
        let pontos = bresenham((2, 3), (10, 7));
        assert_eq!(pontos.first(), Some(&(2, 3)));
        assert_eq!(pontos.last(), Some(&(10, 7)));
        assert_eq!(pontos.len(), 9, "10-2 = 8 passos, 9 pontos");
    }
}
