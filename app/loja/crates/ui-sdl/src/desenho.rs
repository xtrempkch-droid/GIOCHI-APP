//! Paleta e desenho.
//!
//! A paleta é a mesma do launcher, transcrita — não importada. São apps
//! diferentes: um não deve poder quebrar o outro porque alguém mudou uma cor.
//! Custa 15 linhas duplicadas e evita acoplar dois binários que viajam em
//! lugares diferentes (o launcher vai no aparelho, a loja no PC).
//!
//! O desenho é primitivo de propósito: retângulo, retângulo arredondado por
//! faixas, linha e texto. Sem toolkit, sem tema, sem layout engine — o que a
//! loja mostra é lista, ficha e código PIX.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::WindowContext;

pub const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a: 255,
    }
}

pub const BG: Color = rgb(0x070a12);
pub const PANEL: Color = rgb(0x0f1621);
pub const PANEL2: Color = rgb(0x1a2436);
pub const CARD: Color = rgb(0x22304a);
pub const LINE: Color = rgb(0x2c3b52);
pub const ACCENT: Color = rgb(0x66c0f4);
pub const OK: Color = rgb(0x10b981);
pub const WARN: Color = rgb(0xf59e0b);
pub const DANGER: Color = rgb(0xef4444);
pub const TEXT: Color = rgb(0xf1f5f9);
pub const DIM: Color = rgb(0x94a3b8);

pub const TOPO_H: i32 = 56;
pub const RODAPE_H: i32 = 54;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Alinhamento {
    Esquerda,
    Centro,
    Direita,
}

pub struct Pintor {
    pub canvas: WindowCanvas,
    creator: TextureCreator<WindowContext>,
    ttf: Option<&'static Sdl2TtfContext>,
    pub fonte: String,
    pub largura: i32,
    pub altura: i32,
}

impl Pintor {
    pub fn novo(
        mut canvas: WindowCanvas,
        ttf: Option<&'static Sdl2TtfContext>,
        fonte: String,
        largura: i32,
        altura: i32,
    ) -> Self {
        // O `TextureCreator` vive junto porque a `Texture` de texto empresta
        // dele: guardar os dois no mesmo lugar é o que permite criar texto a
        // cada quadro sem `unsafe`.
        let creator = canvas.texture_creator();
        Self { canvas, creator, ttf, fonte, largura, altura }
    }

    pub fn limpar(&mut self, cor: Color) {
        self.canvas.set_draw_color(cor);
        self.canvas.clear();
    }

    pub fn retangulo(&mut self, x: i32, y: i32, w: i32, h: i32, cor: Color) {
        self.canvas.set_draw_color(cor);
        let _ = self.canvas.fill_rect(Rect::new(x, y, w.max(0) as u32, h.max(0) as u32));
    }

    pub fn contorno(&mut self, x: i32, y: i32, w: i32, h: i32, cor: Color) {
        self.canvas.set_draw_color(cor);
        let _ = self.canvas.draw_rect(Rect::new(x, y, w.max(0) as u32, h.max(0) as u32));
    }

    pub fn linha(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, cor: Color) {
        self.canvas.set_draw_color(cor);
        let _ = self.canvas.draw_line((x1, y1), (x2, y2));
    }

    /// Retângulo com cantos arredondados, por faixas horizontais.
    ///
    /// Não é preciosismo: `fill_rect` com raio zero dá um visual de terminal, e
    /// um arco de verdade exigiria shader ou rasterizador próprio. Três faixas
    /// por canto resolvem na distância em que o olho vê.
    pub fn arredondado(&mut self, x: i32, y: i32, w: i32, h: i32, raio: i32, cor: Color) {
        let r = raio.min(w / 2).min(h / 2).max(0);
        if r == 0 {
            self.retangulo(x, y, w, h, cor);
            return;
        }
        self.retangulo(x + r, y, w - 2 * r, h, cor);
        self.retangulo(x, y + r, r, h - 2 * r, cor);
        self.retangulo(x + w - r, y + r, r, h - 2 * r, cor);
        // Cantos: 3 faixas, cada uma recuada.
        for i in 0..3 {
            let recuo = r - (r * (i + 1)) / 4;
            let faixa_h = r / 3 + 1;
            let yy_ini = y + r - recuo + i * faixa_h;
            let yy_fim = y + h - r + recuo - i * faixa_h - faixa_h;
            self.retangulo(x + r - recuo, yy_ini, recuo, r / 3, cor);
            self.retangulo(x + r - recuo, yy_fim, recuo, r / 3, cor);
            self.retangulo(x + w - r, yy_ini, recuo, r / 3, cor);
            self.retangulo(x + w - r, yy_fim, recuo, r / 3, cor);
        }
    }

    pub fn text_size(&mut self, texto: &str, tamanho: u16) -> (i32, i32) {
        let Some(ttf) = self.ttf else {
            // Sem fonte, altura aproximada: o layout não colapsa e o resto dos
            // elementos continua legível. É o mesmo compromisso do launcher.
            return (texto.len() as i32 * (tamanho as i32 / 2), tamanho as i32);
        };
        match ttf.load_font(&self.fonte, tamanho) {
            Ok(f) => f.size_of(texto).map(|(w, h)| (w as i32, h as i32)).unwrap_or((0, 0)),
            Err(_) => (texto.len() as i32 * 6, tamanho as i32),
        }
    }

    pub fn texto(
        &mut self,
        x: i32,
        y: i32,
        texto: &str,
        tamanho: u16,
        cor: Color,
        alinhamento: Alinhamento,
    ) -> i32 {
        let Some(ttf) = self.ttf else {
            return 0;
        };
        let Ok(fonte) = ttf.load_font(&self.fonte, tamanho) else {
            return 0;
        };
        let Ok(superficie) = fonte.render(texto).blended(cor) else {
            return 0;
        };
        let Ok(textura) = self.creator.create_texture_from_surface(&superficie) else {
            return 0;
        };
        let (w, h) = (textura.query().width as i32, textura.query().height as i32);
        let dx = match alinhamento {
            Alinhamento::Esquerda => x,
            Alinhamento::Centro => x - w / 2,
            Alinhamento::Direita => x - w,
        };
        let _ = self.canvas.copy(&textura, None, Rect::new(dx, y, w as u32, h as u32));
        w
    }

    /// Texto truncado com reticências se não couber.
    ///
    /// Uma descrição longa vazando por cima do botão é o tipo de defeito que só
    /// aparece com dados reais — o catálogo de teste tem título curto.
    pub fn texto_limitado(
        &mut self,
        x: i32,
        y: i32,
        texto: &str,
        tamanho: u16,
        cor: Color,
        largura_max: i32,
    ) -> i32 {
        let (w, _) = self.text_size(texto, tamanho);
        if w <= largura_max {
            return self.texto(x, y, texto, tamanho, cor, Alinhamento::Esquerda);
        }
        let mut corte = texto.to_string();
        while !corte.is_empty() {
            corte.pop();
            let (w2, _) = self.text_size(&format!("{corte}…"), tamanho);
            if w2 <= largura_max {
                break;
            }
        }
        self.texto(x, y, &format!("{corte}…"), tamanho, cor, Alinhamento::Esquerda)
    }

    pub fn para_bmp(&mut self, caminho: &str) -> Result<(), String> {
        // `read_pixels` devolve RGBA de baixo para cima; o BMP é escrito
        // invertido por conta disso. É o mesmo caminho do `--dump-frame` do
        // launcher, e foi ele que revelou que o BMP "mentia" sobre a posição.
        let (w, h) = (self.largura as u32, self.altura as u32);
        // Na 0.37 a função devolve o `Vec` pronto (não recebe buffer): a
        // assinatura antiga com `&mut [u8]` era da 0.34 e o erro é só "argumentos
        // a mais".
        let pixels = self
            .canvas
            .read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)
            .map_err(|e| e.to_string())?;

        let mut bmp = Vec::with_capacity(54 + pixels.len());
        let tamanho_arquivo = 54 + pixels.len() as u32;
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&tamanho_arquivo.to_le_bytes());
        bmp.extend_from_slice(&[0, 0, 0, 0]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(w as i32).to_le_bytes());
        bmp.extend_from_slice(&(h as i32).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&[0u8; 24]);

        for linha in (0..h).rev() {
            for col in 0..w {
                let i = ((linha * w + col) * 4) as usize;
                bmp.extend_from_slice(&[pixels[i + 2], pixels[i + 1], pixels[i], 255]);
            }
        }
        std::fs::write(caminho, bmp).map_err(|e| e.to_string())
    }
}
