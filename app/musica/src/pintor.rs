//! O pintor do app: a engine desenha, este módulo cuida de rotação e de texto.
//!
//! ## Por que um pintor próprio, e não o do `launcher-teste`
//!
//! O `app/launcher-teste/src/draw.rs` é um pintor completo sobre a mesma engine,
//! e reusá-lo por `#[path]` evitaria umas 300 linhas aqui. A troca foi recusada
//! por direção de dependência: o `launcher-teste` é **banco de prova** — existe
//! para experimentar a engine e é alterado por isso. Um app de produção que
//! dependesse dele herdaria cada experimento. O que é reusado é o que não muda
//! por experimento: `theme.rs` (a paleta), `icons.rs` (os glifos) e o leitor de
//! `/dev/input` — todos compartilhados com o launcher de produção, não com o
//! banco de prova.
//!
//! ## O que este módulo garante
//!
//! - **Rotação aplicada uma vez.** Cada primitiva pública leva a coordenada do
//!   espaço de projeto para o de dispositivo e delega; misturar os dois espaços
//!   giraria duas vezes.
//! - **Texto girado com o resto.** `copy_ex` roda **depois** de escalar: o
//!   destino tem de ter o tamanho *sem* giro, centrado no retângulo girado.
//!   Medidas trocadas comprimem o texto antes de girar — o defeito que só a
//!   contagem de pixels claros pegou no `launcher-teste`.
//! - **Contador no backend.** `desenhos` conta chamadas de desenho onde elas
//!   acontecem, não onde são pedidas: um contador que soma "quantos lotes eu
//!   pedi" mede a intenção, não o custo.

use std::collections::HashMap;

use giochi_engine::{Color as CorEngine, Rect as RectEngine, RenderContext, TextureHandle, Vec2};
use giochi_musica_core::rotacao::{projecao, rotate_point, rotate_rect, Fonte, Rotacao};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::ttf::{Font, Sdl2TtfContext};

use crate::icones::Prim;

/// Alinhamento horizontal do texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Onde procurar a fonte, em ordem. A mesma lista do launcher: o Buildroot é o
/// que importa, e foi ele que já apareceu sem rótulo nenhum.
pub const CANDIDATOS_FONTE: [&str; 6] = [
    "assets/fonts/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans/DejaVuSans.ttf",
    "app/launcher/assets/fonts/DejaVuSans.ttf",
];

fn para_engine(c: Color) -> CorEngine {
    CorEngine::new(c.r, c.g, c.b, c.a)
}

fn rect_dev(x: i32, y: i32, w: i32, h: i32) -> RectEngine {
    RectEngine::new(x as f32, y as f32, w as f32, h as f32)
}

/// De onde veio a rotação — vai para o log, porque a pergunta seguinte é sempre
/// "por quê".
pub struct DecisaoDeRotacao {
    pub rotacao: Rotacao,
    pub fonte: Fonte,
}

/// Decide a rotação lendo os arquivos do sistema.
///
/// `GIOCHI_ROTATE` é lido do ambiente; o resto vem de arquivo. O leitor devolve
/// `None` quando o arquivo não existe, que é diferente de existir e dizer `0` —
/// só o `0` é desempatado pelo `cmdline`.
pub fn rotacao_do_sistema(painel: (u32, u32)) -> DecisaoDeRotacao {
    let env = std::env::var("GIOCHI_ROTATE").ok();
    let (rotacao, fonte) = giochi_musica_core::rotacao::decidir(
        env.as_deref(),
        |caminho| std::fs::read_to_string(caminho).ok(),
        painel,
    );
    DecisaoDeRotacao { rotacao, fonte }
}

pub struct Pintor {
    render: RenderContext,
    ttf: Option<&'static Sdl2TtfContext>,
    fontes: HashMap<u16, Font<'static, 'static>>,
    fonte: &'static str,
    avisou_fonte: bool,
    rot: Rotacao,
    projeto: (i32, i32),
    desenhos: u32,
}

impl Pintor {
    pub fn novo(render: RenderContext, fonte: &str) -> Result<Pintor, String> {
        let painel = render.size();
        let DecisaoDeRotacao { rotacao, fonte: origem } = rotacao_do_sistema(painel);
        let proj = projecao(rotacao, painel);

        eprintln!(
            "tela: painel {}x{} → projeto {}x{} (escala 1,0)",
            painel.0, painel.1, proj.projeto.0, proj.projeto.1
        );
        eprintln!(
            "rotação: {} (fonte: {})",
            rotacao.descricao(),
            origem.descricao()
        );
        if !rotacao.gira() && painel.1 > painel.0 {
            eprintln!("aviso: painel retrato sem rotação — a interface sai deitada");
        }
        eprintln!(
            "renderer: {} ({})",
            render.renderer_name(),
            if render.renderer_is_accelerated() {
                "acelerado"
            } else {
                "software"
            }
        );

        // `Sdl2TtfContext` não pode ser inicializado duas vezes: a segunda
        // devolve "already initialized" e o app abre **sem nenhum texto**.
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

        let fonte: &'static str = Box::leak(fonte.to_string().into_boxed_str());

        Ok(Pintor {
            render,
            ttf,
            fontes: HashMap::new(),
            fonte,
            avisou_fonte: false,
            rot: rotacao,
            projeto: proj.projeto,
            desenhos: 0,
        })
    }

    pub fn rotacao(&self) -> Rotacao {
        self.rot
    }

    /// Tamanho do espaço de projeto (o que o layout usa).
    pub fn projeto(&self) -> (i32, i32) {
        self.projeto
    }

    /// Tamanho do painel (para a linha de diagnóstico).
    pub fn painel(&self) -> (u32, u32) {
        self.render.size()
    }

    /// Chamadas de desenho emitidas desde a criação.
    pub fn desenhos(&self) -> u32 {
        self.desenhos
    }

    pub fn clear(&mut self, cor: Color) {
        self.render.clear(para_engine(cor));
    }

    pub fn present(&mut self) {
        self.render.present();
    }

    fn map_rect(&self, x: i32, y: i32, w: i32, h: i32) -> (i32, i32, i32, i32) {
        rotate_rect(self.rot, self.projeto, x, y, w, h)
    }

    // ── retângulos e linhas ─────────────────────────────────

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, cor: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (x, y, w, h) = self.map_rect(x, y, w, h);
        self.desenhos += 1;
        self.render.draw_rect(rect_dev(x, y, w, h), para_engine(cor));
    }

    /// Vários retângulos numa chamada.
    ///
    /// Cada um é girado individualmente e o lote segue para a engine: um
    /// retângulo girado de 90° continua sendo um retângulo alinhado aos eixos,
    /// então o lote é válido nos dois sentidos.
    pub fn fill_rects(&mut self, rects: &[(i32, i32, i32, i32)], cor: Color) {
        let girados: Vec<RectEngine> = rects
            .iter()
            .filter(|(_, _, w, h)| *w > 0 && *h > 0)
            .map(|(x, y, w, h)| {
                let (x, y, w, h) = self.map_rect(*x, *y, *w, *h);
                rect_dev(x, y, w, h)
            })
            .collect();
        if girados.is_empty() {
            return;
        }
        self.desenhos += 1;
        self.render.fill_rects(&girados, para_engine(cor));
    }

    /// Contorno de retângulo com espessura, em quatro preenchimentos.
    pub fn rect_border(&mut self, x: i32, y: i32, w: i32, h: i32, espessura: i32, cor: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let e = espessura.max(1);
        self.fill_rects(
            &[
                (x, y, w, e),
                (x, y + h - e, w, e),
                (x, y, e, h),
                (x + w - e, y, e, h),
            ],
            cor,
        );
    }

    // ── texto ───────────────────────────────────────────────

    fn tomar_fonte(&mut self, tamanho: u16) -> Option<Font<'static, 'static>> {
        if let Some(fonte) = self.fontes.remove(&tamanho) {
            return Some(fonte);
        }
        let ttf = self.ttf?;
        match ttf.load_font(self.fonte, tamanho) {
            Ok(fonte) => Some(fonte),
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

    /// Largura e altura do texto, na fonte pedida.
    pub fn text_size(&mut self, texto: &str, tamanho: u16) -> (i32, i32) {
        let Some(fonte) = self.tomar_fonte(tamanho) else {
            return (0, 0);
        };
        let medido = fonte.size_of(texto).unwrap_or((0, 0));
        self.fontes.insert(tamanho, fonte);
        (medido.0 as i32, medido.1 as i32)
    }

    /// Desenha o texto e devolve a largura usada.
    pub fn text(
        &mut self,
        x: i32,
        y: i32,
        texto: &str,
        tamanho: u16,
        cor: Color,
        align: Align,
    ) -> i32 {
        if texto.is_empty() {
            return 0;
        }
        let Some(fonte) = self.tomar_fonte(tamanho) else {
            return 0;
        };

        let mut largura = 0;
        match fonte.render(texto).blended(cor) {
            Err(e) => self.reclamar(&format!("render do texto falhou: {e}")),
            Ok(surface) => {
                let (tw, th) = (surface.width() as i32, surface.height() as i32);
                largura = tw;
                let destino = match align {
                    Align::Left => x,
                    Align::Center => x - tw / 2,
                    Align::Right => x - tw,
                };
                if let Err(e) = self.texturar_surface(&surface, destino, y, tw, th) {
                    self.reclamar(&e);
                }
            }
        }
        self.fontes.insert(tamanho, fonte);
        largura
    }

    pub fn text_left(&mut self, x: i32, y: i32, texto: &str, tamanho: u16, cor: Color) -> i32 {
        self.text(x, y, texto, tamanho, cor, Align::Left)
    }

    pub fn text_center(&mut self, cx: i32, y: i32, texto: &str, tamanho: u16, cor: Color) -> i32 {
        self.text(cx, y, texto, tamanho, cor, Align::Center)
    }

    pub fn text_right(&mut self, rx: i32, y: i32, texto: &str, tamanho: u16, cor: Color) -> i32 {
        self.text(rx, y, texto, tamanho, cor, Align::Right)
    }

    fn texturar_surface(
        &mut self,
        surface: &sdl2::surface::Surface<'_>,
        x: i32,
        y: i32,
        tw: i32,
        th: i32,
    ) -> Result<(), String> {
        let convertida = surface
            .convert_format(PixelFormatEnum::RGBA32)
            .map_err(|e| format!("convert_format: {e}"))?;
        let pixels = convertida
            .without_lock()
            .ok_or_else(|| "surface sem acesso direto aos pixels".to_string())?;
        let pitch = convertida.pitch() as usize;
        let linha = tw as usize * 4;
        if linha == 0 || pixels.len() < pitch * th as usize {
            return Err(format!(
                "pixels insuficientes: {} bytes, esperado {}",
                pixels.len(),
                pitch * th as usize
            ));
        }
        let mut plana = Vec::with_capacity(linha * th as usize);
        for r in 0..th as usize {
            let inicio = r * pitch;
            plana.extend_from_slice(&pixels[inicio..inicio + linha]);
        }

        let textura = self
            .render
            .create_texture_from_rgba(tw as u32, th as u32, &plana)
            .map_err(|e| format!("create_texture_from_rgba: {e}"))?;
        let resultado = self.desenhar_textura_raw(textura, x, y, tw, th);
        self.render.destroy_texture(textura);
        resultado
    }

    /// Desenha uma textura cujo retângulo em projeto é `(x, y, w, h)`.
    fn desenhar_textura_raw(
        &mut self,
        textura: TextureHandle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> Result<(), String> {
        let src = rect_dev(0, 0, w, h);
        self.desenhar_textura_src(textura, src, x, y, w, h)
    }

    fn desenhar_textura_src(
        &mut self,
        textura: TextureHandle,
        src: RectEngine,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> Result<(), String> {
        let (gx, gy, gw, gh) = rotate_rect(self.rot, self.projeto, x, y, w, h);
        let angle = match self.rot {
            Rotacao::Nenhuma => 0.0,
            Rotacao::Horario => 90.0,
            Rotacao::AntiHorario => 270.0,
        };
        // O destino **não** é o retângulo girado: `copy_ex` escala para caber no
        // destino e só depois gira. Com medidas trocadas o texto sai comprimido
        // e girado — foi o defeito que só a contagem de pixels claros pegou.
        let dst = match self.rot {
            Rotacao::Nenhuma => rect_dev(x, y, w, h),
            _ => {
                let (cx, cy) = (gx + gw / 2, gy + gh / 2);
                rect_dev(cx - w / 2, cy - h / 2, w, h)
            }
        };
        self.desenhos += 1;
        self.render
            .draw_texture_ex(textura, src, dst, angle, false, false)
            .map_err(|e| format!("draw_texture_ex: {e}"))
    }

    fn reclamar(&mut self, mensagem: &str) {
        if !self.avisou_fonte {
            self.avisou_fonte = true;
            eprintln!("aviso: {mensagem}");
        }
    }

    // ── texturas de imagem ──────────────────────────────────

    pub fn criar_textura(&self, largura: u32, altura: u32, pixels: &[u8]) -> Result<TextureHandle, String> {
        self.render
            .create_texture_from_rgba(largura, altura, pixels)
            .map_err(|e| format!("create_texture_from_rgba: {e}"))
    }

    pub fn liberar_textura(&mut self, textura: TextureHandle) {
        self.render.destroy_texture(textura);
    }

    /// Desenha uma textura RGBA num retângulo do espaço de projeto.
    pub fn desenhar_textura(
        &mut self,
        textura: TextureHandle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> Result<(), String> {
        let (tw, th) = self
            .render
            .texture_size(textura)
            .ok_or_else(|| "textura inválida".to_string())?;
        let src = rect_dev(0, 0, tw as i32, th as i32);
        self.desenhar_textura_src(textura, src, x, y, w, h)
    }

    // ── ícones ──────────────────────────────────────────────

    /// Desenha um ícone da tabela compartilhada com o launcher.
    pub fn icon(&mut self, nome: &str, cx: i32, cy: i32, tamanho: i32, cor: Color) {
        let Some(icone) = crate::icones::get(nome) else {
            return;
        };
        let escala = tamanho as f32 / 24.0;
        let rot = self.rot;
        let projeto = self.projeto;
        let map = move |x: f32, y: f32| -> (i32, i32) {
            let px = cx - tamanho / 2 + (x * escala).round() as i32;
            let py = cy - tamanho / 2 + (y * escala).round() as i32;
            rotate_point(rot, projeto, px, py)
        };

        for prim in icone.prims {
            match prim {
                Prim::Line(x1, y1, x2, y2) => {
                    let a = map(*x1, *y1);
                    let b = map(*x2, *y2);
                    self.desenhos += 1;
                    self.render.draw_line(
                        Vec2::new(a.0 as f32, a.1 as f32),
                        Vec2::new(b.0 as f32, b.1 as f32),
                        para_engine(cor),
                    );
                }
                Prim::Poly(pontos, preenchido) => {
                    if pontos.len() < 2 {
                        continue;
                    }
                    if *preenchido {
                        self.preencher_poligono(pontos, &map, cor);
                    } else {
                        for par in pontos.windows(2) {
                            let a = map(par[0].0, par[0].1);
                            let b = map(par[1].0, par[1].1);
                            self.desenhos += 1;
                            self.render.draw_line(
                                Vec2::new(a.0 as f32, a.1 as f32),
                                Vec2::new(b.0 as f32, b.1 as f32),
                                para_engine(cor),
                            );
                        }
                    }
                }
                Prim::Circle(x, y, r) => {
                    self.circulo(map(*x, *y), (r * escala).round().max(1.0) as i32, cor);
                }
                Prim::Rect(x, y, w, h) => {
                    let a = map(*x, *y);
                    let b = map(x + w, y + h);
                    let e = (escala.round() as i32).max(1);
                    self.rect_border(
                        a.0.min(b.0),
                        a.1.min(b.1),
                        (b.0 - a.0).abs(),
                        (b.1 - a.1).abs(),
                        e,
                        cor,
                    );
                }
            }
        }
    }

    /// Contorno de círculo por faixas horizontais (uma chamada de desenho).
    fn circulo(&mut self, centro: (i32, i32), raio: i32, cor: Color) {
        if raio <= 0 {
            return;
        }
        let mut faixas = Vec::new();
        for dy in -raio..=raio {
            let dx = ((raio * raio - dy * dy) as f32).sqrt().round() as i32;
            if dx <= 0 {
                continue;
            }
            // Só a borda: dois pixels em cada ponta.
            faixas.push((centro.0 - dx, centro.1 + dy, 2, 1));
            faixas.push((centro.0 + dx - 1, centro.1 + dy, 2, 1));
        }
        self.fill_rects(&faixas, cor);
    }

    /// Preenche um polígono por varredura horizontal, num único lote.
    fn preencher_poligono(
        &mut self,
        pontos: &[(f32, f32)],
        map: &impl Fn(f32, f32) -> (i32, i32),
        cor: Color,
    ) {
        let mapeados: Vec<(i32, i32)> = pontos.iter().map(|(x, y)| map(*x, *y)).collect();
        let min_y = mapeados.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = mapeados.iter().map(|p| p.1).max().unwrap_or(0);

        let mut faixas: Vec<(i32, i32, i32, i32)> = Vec::new();
        for y in min_y..=max_y {
            let mut cruzamentos: Vec<i32> = Vec::new();
            for i in 0..mapeados.len() {
                let (x1, y1) = mapeados[i];
                let (x2, y2) = mapeados[(i + 1) % mapeados.len()];
                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    let t = (y - y1) as f32 / (y2 - y1) as f32;
                    cruzamentos.push((x1 as f32 + t * (x2 - x1) as f32).round() as i32);
                }
            }
            cruzamentos.sort_unstable();
            for par in cruzamentos.chunks(2) {
                if par.len() == 2 && par[1] > par[0] {
                    faixas.push((par[0], y, par[1] - par[0], 1));
                }
            }
        }
        self.fill_rects(&faixas, cor);
    }

    // ── captura ─────────────────────────────────────────────

    /// Grava o quadro atual em BMP de 24 bpp.
    ///
    /// Lê **antes** de apresentar: com backend acelerado, o swap esvazia o
    /// buffer e a captura sai preta — defeito já medido neste projeto no X35H e
    /// no R35S.
    pub fn save_frame(&mut self, caminho: &str) -> Result<(), String> {
        let (w, h) = self.painel();
        let rgba = self
            .render
            .read_pixels(RectEngine::new(0.0, 0.0, w as f32, h as f32))
            .map_err(|e| format!("read_pixels: {e}"))?;
        gravar_bmp(caminho, w, h, &rgba)
    }
}

/// Escreve BMP de 24 bpp, de baixo para cima, com padding de 4 bytes por linha.
///
/// 24 e não 32 bpp porque o arquivo é para inspeção humana e para o
/// `scripts/verificar-frame.mjs`, que lê `bytesPorPixel` do cabeçalho — e é
/// justamente por isso que ele lê: a primeira versão daquele script supunha
/// 24 bpp num arquivo de 32 e produziu uma rodada inteira de medições inválidas.
fn gravar_bmp(caminho: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
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

    std::fs::write(caminho, out).map_err(|e| format!("{caminho}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidatos_de_fonte_cobrem_buildroot_e_desktop() {
        assert!(CANDIDATOS_FONTE.iter().any(|c| c.contains("/usr/share/fonts/dejavu")));
        assert!(CANDIDATOS_FONTE.iter().any(|c| c.starts_with("assets/fonts/")));
    }

    #[test]
    fn bmp_tem_cabecalho_e_tamanho_coerentes() {
        // 3x2 RGBA: linha bruta 9 bytes, com padding até 12.
        let (w, h) = (3u32, 2u32);
        let rgba = vec![255u8; (w * h * 4) as usize];
        let caminho = std::env::temp_dir().join("giochi-musica-teste.bmp");
        gravar_bmp(caminho.to_str().unwrap(), w, h, &rgba).expect("gravar");
        let bytes = std::fs::read(&caminho).expect("ler");
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(bytes.len(), 54 + 12 * 2);
        assert_eq!(u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]), w);
        assert_eq!(u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]), h);
        assert_eq!(u16::from_le_bytes([bytes[28], bytes[29]]), 24);
        let _ = std::fs::remove_file(&caminho);
    }
}
