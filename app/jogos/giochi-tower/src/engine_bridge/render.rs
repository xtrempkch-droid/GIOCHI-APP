//! Fachada de desenho sobre a GIOCHI-ENGINE-3D.
//!
//! Tudo o que o jogo precisa do renderer 3D passa por aqui: abrir a janela
//! OpenGL-ES, subir as malhas do protótipo, desenhar a cena de um quadro e
//! capturar BMP. O resto do jogo não conhece `RenderContext3D`.
//!
//! # Ordem de inicialização (não é negociável)
//!
//! `configure_attributes` → `Window::new_opengl` → `context::create`. Os
//! atributos `SDL_GL_*` são globais do processo e lidos na **criação da janela**;
//! inverter a ordem abre contexto 2.0 pedindo 3.0, ou falha
//! (`gl_context.rs:90-95`, `cube_demo.rs:129-133`).
//!
//! # KMSDRM
//!
//! No aparelho o driver de vídeo é `KMSDRM` e a janela tem de ser fullscreen: o
//! painel manda no tamanho, não o `WindowConfig` (ADR-035). A comparação do nome
//! do driver é feita sem caixa porque ele vem em maiúsculas (`sdl.rs:246-247`).

use std::path::Path;

use glam::Vec3 as GlamVec3;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine::platform::Platform;
use giochi_engine_3d::context::{self, GlState, GlesVersion};
use giochi_engine_3d::math3d::{Camera3D, Color3D, Quat, Transform3D, Vec3 as EngineVec3};
use giochi_engine_3d::render3d::DirectionalLight;
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::{MeshHandle, RenderContext3D, ShaderHandle};

use crate::errors::{GameError, Result};
use crate::ui::{self, layout};

/// Cor de fundo padrão (o "vazio" da sala do M1).
pub const CLEAR_COLOR: Color3D = Color3D::new(0.055, 0.070, 0.105, 1.0);

/// Cor do chão.
pub const GROUND_COLOR: Color3D = Color3D::new(0.135, 0.150, 0.170, 1.0);

/// Cor da torre provisória (o marcador do centro do mapa).
pub const TOWER_COLOR: Color3D = Color3D::new(0.290, 0.520, 0.860, 1.0);

/// Cor do marcador do jogador.
pub const PLAYER_COLOR: Color3D = Color3D::new(0.950, 0.820, 0.300, 1.0);

/// Cor do eixo X (referência de orientação na sala vazia).
pub const AXIS_X_COLOR: Color3D = Color3D::new(0.760, 0.290, 0.290, 1.0);

/// Cor do eixo Z (referência de orientação na sala vazia).
pub const AXIS_Z_COLOR: Color3D = Color3D::new(0.290, 0.760, 0.420, 1.0);

/// Luz que faz a cor chegar à tela sem atenuação.
///
/// `ambient = 1.0` e cor de luz preta: no `phong.frag` o resultado é
/// `cor * 1.0 + cor * 0 * difusa + 0 * especular`, ou seja, a própria cor. O HUD
/// não é um objeto do mundo e não pode escurecer conforme a câmera orbita — com
/// a luz do sol o texto do canto mudaria de brilho junto com o chão.
///
/// É o mesmo truque que o `neon-drift` usa para o neon e o `giochi-defender` para
/// a mira, pelo mesmo motivo.
pub fn luz_do_hud() -> DirectionalLight {
    let mut luz = DirectionalLight::sun();
    luz.color = Color3D::new(0.0, 0.0, 0.0, 1.0);
    luz.ambient = 1.0;
    luz.specular = 0.0;
    luz
}

/// Janela + contexto GLES + as malhas compartilhadas do jogo.
pub struct RenderBridge {
    window: Window,
    render: RenderContext3D,
    shader: ShaderHandle,
    plane: MeshHandle,
    cube: MeshHandle,
    /// Quadrilátero unitário do HUD: a geometria não muda por quadro, só as
    /// matrizes de instância.
    hud_quad: MeshHandle,
    hud_shader: ShaderHandle,
    /// Lotes do último quadro, para o diagnóstico contar chamadas.
    hud_lotes: Vec<ui::Lote>,
    /// Chamadas que o **driver** emitiu no último quadro. Diferente de
    /// `hud_lotes.len()` quando não há instancing: aí `draw_instanced` cai no
    /// caminho comum e emite uma por instância. Sem este número, "9 chamadas"
    /// seria o que eu pedi, não o que saiu.
    hud_chamadas_gl: u32,
    driver: &'static str,
    requested_gles: GlesVersion,
    frames: u64,
}

impl RenderBridge {
    /// Abre a janela e sobe o contexto 3D.
    ///
    /// `title` vai como argumento de `Window::new_opengl`: `WindowConfig` não tem
    /// campo de título nesta versão da engine.
    pub fn new(platform: &Platform, width: u32, height: u32, title: &str) -> Result<Self> {
        let requested_gles = GlesVersion::from_env();
        context::configure_attributes(platform, requested_gles)?;

        let mut config = WindowConfig::new(width.max(1), height.max(1));
        let driver = platform.video_driver_name();
        if driver.eq_ignore_ascii_case("kmsdrm") {
            // Sem compositor no aparelho: a janela É o painel.
            config = config.with_fullscreen();
        }

        let window = Window::new_opengl(platform, config, title)?;
        let mut render = RenderContext3D::new(context::create(&window)?);

        render.set_clear_color(CLEAR_COLOR);
        render.set_light(DirectionalLight::sun());

        // Blending ligado para o **quadro inteiro**, e não só para o HUD.
        //
        // A engine aplica o `GlState` em `begin_frame` e não oferece troca no
        // meio do quadro (`set_state` só grava; ver `renderer.rs`). O mundo é
        // opaco, então o blend não muda um pixel dele; o HUD usa alfa no véu da
        // loja, na trilha das barras e nos painéis. Ligar só para o HUD exigiria
        // um segundo `begin_frame`, que limparia o quadro.
        render.set_state(GlState {
            blend: true,
            ..GlState::default()
        });

        let shader = render.create_builtin_shader(BuiltinShader::Phong)?;
        let plane = render.create_plane_mesh(crate::game::camera::MapBounds::new().size_m(), 1)?;
        let cube = render.create_cube_mesh(1.0)?;
        let (hud_vertices, hud_indices) = ui::quad_da_hud();
        let hud_quad = render.create_mesh(&hud_vertices, &hud_indices)?;
        let hud_shader = render.create_builtin_shader(BuiltinShader::Instanced)?;

        Ok(Self {
            window,
            render,
            shader,
            plane,
            cube,
            hud_quad,
            hud_shader,
            hud_lotes: Vec::new(),
            hud_chamadas_gl: 0,
            driver,
            requested_gles,
            frames: 0,
        })
    }

    /// Nome do driver de vídeo em uso, como a plataforma reportou na abertura.
    pub fn driver(&self) -> &'static str {
        self.driver
    }

    /// Versão de GLES pedida na inicialização.
    pub fn requested_gles(&self) -> GlesVersion {
        self.requested_gles
    }

    /// Versão de GLES realmente obtida.
    pub fn gles_major(&self) -> u8 {
        self.render.gles_version().major()
    }

    /// Nome do adaptador gráfico.
    pub fn adapter(&self) -> &str {
        self.render.adapter()
    }

    /// `true` quando o driver entregou 3.0 (não pedido, apenas observado).
    pub fn supports_gles3(&self) -> bool {
        self.render.supports_gles3()
    }

    /// Tamanho real do drawable. Sempre este, nunca o pedido: no KMSDRM o painel
    /// sobrescreve o `WindowConfig`.
    pub fn drawable_size(&self) -> (u32, u32) {
        self.window.drawable_size()
    }

    /// Quantos quadros já foram apresentados.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Malha do plano do chão (1×1 m escalado no desenho, ver [`Self::draw_ground`]).
    pub fn plane(&self) -> MeshHandle {
        self.plane
    }

    /// Inicia o quadro (limpa cor e profundidade, aplica viewport).
    pub fn begin(&mut self) {
        self.render.begin_frame(&self.window);
    }

    /// Apresenta o quadro e drena a fila de erros do GL.
    ///
    /// Um erro de GL aqui vira `GameError`: é o que transforma "tela preta" em
    /// falha nomeada, como em `cube_demo.rs:227-229`.
    pub fn end(&mut self) -> Result<()> {
        self.render.end_frame(&self.window);
        self.frames += 1;
        let error = self.render.take_gl_error();
        if error != 0 {
            return Err(GameError::message(format!("GL 0x{error:04x} no quadro {}", self.frames)));
        }
        Ok(())
    }

    /// Desenha o chão do mapa inteiro.
    pub fn draw_ground(&mut self, camera: &Camera3D) -> Result<()> {
        let model = Transform3D::IDENTITY;
        self.draw(self.plane, &model, camera, GROUND_COLOR)
    }

    /// Desenha um cubo de aresta `size` centrado em `position`.
    ///
    /// A malha é um cubo unitário centrado na origem; o tamanho vem da escala do
    /// `Transform3D`, que a engine compõe como T·R·S (`transform.rs:72-76`).
    pub fn draw_cube(
        &mut self,
        camera: &Camera3D,
        position: GlamVec3,
        size: f32,
        color: Color3D,
    ) -> Result<()> {
        let edge = size.max(0.01);
        let model = Transform3D::new(
            EngineVec3::from_glam(position),
            Quat::IDENTITY,
            EngineVec3::new(edge, edge, edge),
        );
        self.draw(self.cube, &model, camera, color)
    }

    /// Desenha um cubo com rotação e escala arbitrárias (para os testes de cena).
    pub fn draw_cube_transformed(
        &mut self,
        camera: &Camera3D,
        model: &Transform3D,
        color: Color3D,
    ) -> Result<()> {
        self.draw(self.cube, model, camera, color)
    }

    fn draw(
        &mut self,
        mesh: MeshHandle,
        model: &Transform3D,
        camera: &Camera3D,
        color: Color3D,
    ) -> Result<()> {
        // `Ok(false)` significa handle obsoleto: a engine não trata isso como
        // erro, e um jogo que ignora o booleano desenha nada e acha que desenhou.
        let drew = self
            .render
            .draw_mesh_colored(mesh, self.shader, model, camera, color)?;
        if !drew {
            log::warn!("draw ignorado: malha {mesh} ou shader não existe mais");
        }
        Ok(())
    }

    /// Desenha o HUD sobre a cena 3D.
    ///
    /// Tem de ser chamada **depois** do mundo e **antes** de [`Self::end`]: o HUD
    /// divide o mesmo depth buffer, e o plano próximo em 9,0 da câmera de 90° é o
    /// que garante que ele fica na frente.
    ///
    /// A luz é trocada para a emissiva aqui e restaurada para o sol no fim. Sem a
    /// restauração, o quadro seguinte desenharia o mundo com `ambient = 1.0` e
    /// cor de luz preta — tudo chapado, e o defeito apareceria como "a iluminação
    /// quebrou", longe da causa.
    pub fn draw_ui(&mut self, plano: &layout::Plano) -> Result<()> {
        let (largura, altura) = self.window.drawable_size();
        if largura == 0 || altura == 0 {
            return Ok(());
        }
        let tela = layout::Tela::new(largura, altura);
        let lotes = ui::lotes(plano, tela);

        let camera = ui::camera((largura, altura));
        self.render.set_light(luz_do_hud());
        let mut chamadas = 0;
        for lote in &lotes {
            let cor = Color3D::new(lote.cor[0], lote.cor[1], lote.cor[2], lote.cor[3]);
            chamadas += self.render.draw_instanced(
                self.hud_quad,
                self.hud_shader,
                &lote.matrizes,
                &camera,
                cor,
            )?;
        }
        self.hud_chamadas_gl = chamadas;
        self.render.set_light(DirectionalLight::sun());
        self.hud_lotes = lotes;
        Ok(())
    }

    /// Quantas chamadas de desenho o HUD custou no último quadro.
    ///
    /// Uma por cor. É o número que responde "eu agrupei?" — e que um
    /// `draw_rect` por letra levaria a centenas.
    pub fn hud_draw_calls(&self) -> usize {
        self.hud_lotes.len()
    }

    /// Quantas instâncias (retângulos) o HUD desenhou no último quadro.
    pub fn hud_instances(&self) -> usize {
        self.hud_lotes.iter().map(|l| l.matrizes.len()).sum()
    }

    /// Quantas chamadas o driver emitiu para o HUD no último quadro.
    ///
    /// Igual a [`Self::hud_draw_calls`] quando há instancing; igual ao número de
    /// instâncias quando não há. É o que separa "eu agrupei" de "o driver
    /// agrupou".
    pub fn hud_gl_draws(&self) -> u32 {
        self.hud_chamadas_gl
    }

    /// Captura o quadro atual em BMP de 24 bits.
    ///
    /// Tem de ser chamada **antes** de [`Self::end`]: depois do swap, o back buffer
    /// já pertence ao quadro seguinte (`cube_demo.rs:211-212`).
    pub fn capture_bmp(&self, path: &Path) -> Result<()> {
        let (width, height) = self.render.drawable_size();
        let pixels = self.render.read_pixels(width, height)?;
        write_bmp(path, width, height, &pixels)?;
        log::info!("quadro: {} ({width}x{height})", path.display());
        Ok(())
    }

    /// Libera malhas, shader e o contexto.
    pub fn destroy_all(&mut self) {
        // Os retornos são `bool` (handle existia?). Aqui não há decisão a tomar com
        // eles, e ignorá-los explicitamente evita o aviso de `must_use`.
        let _ = self.render.destroy_mesh(self.plane);
        let _ = self.render.destroy_mesh(self.cube);
        let _ = self.render.destroy_mesh(self.hud_quad);
        let _ = self.render.destroy_shader(self.shader);
        let _ = self.render.destroy_shader(self.hud_shader);
        let _ = self.render.release_all();
    }
}

/// Escreve um BMP 24 bits a partir de pixels RGBA de `glReadPixels`.
///
/// O GL devolve as linhas de baixo para cima e o BMP com altura positiva guarda
/// exatamente nessa ordem, então as linhas são copiadas em sequência — trocá-las
/// produziria um quadro de cabeça para baixo que "parece" um bug de câmera.
fn write_bmp(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err(GameError::message(format!(
            "captura curta: {} bytes para {width}x{height} (esperava {expected})",
            rgba.len()
        )));
    }

    let row_bytes = width as usize * 3;
    let padding = (4 - (row_bytes % 4)) % 4;
    let row_size = row_bytes + padding;
    let pixel_bytes = row_size * height as usize;
    let file_size = 54 + pixel_bytes;

    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in 0..height as usize {
        let row = &rgba[y * width as usize * 4..(y + 1) * width as usize * 4];
        for x in 0..width as usize {
            let p = &row[x * 4..x * 4 + 4];
            out.push(p[2]); // B
            out.push(p[1]); // G
            out.push(p[0]); // R
        }
        out.extend(std::iter::repeat_n(0u8, padding));
    }

    std::fs::write(path, &out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bmp_header_is_a_24_bit_windows_bitmap() {
        let dir = std::env::temp_dir().join("giochi-tower-bmp");
        std::fs::create_dir_all(&dir).expect("tmp");
        let path = dir.join("header.bmp");
        let pixels = vec![255u8; 4 * 2 * 2];
        write_bmp(&path, 2, 2, &pixels).expect("escrever bmp");

        let bytes = std::fs::read(&path).expect("ler bmp");
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize, bytes.len());
        assert_eq!(u16::from_le_bytes(bytes[28..30].try_into().unwrap()), 24);
        // 2 px de largura = 6 bytes por linha, mais 2 de padding.
        let row_size = 8;
        assert_eq!(bytes.len(), 54 + row_size * 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_short_capture_is_refused_instead_of_panicking() {
        let err = write_bmp(Path::new("nao-usado.bmp"), 4, 4, &[0u8; 8]);
        assert!(err.is_err());
    }
}
