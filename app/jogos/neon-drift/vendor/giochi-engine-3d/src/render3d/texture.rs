//! Textures owned by the 3D context.
//!
//! ## Por que esta peça faltava
//!
//! A engine 3D já tinha tudo para desenhar textura **menos** a textura: o
//! `Vertex` carrega `uv`, o `gpu_mesh` liga o atributo `a_uv`, existe
//! `BuiltinShader::Textured` com os dois shaders compilando, e o `Program` já
//! tem `set_texture`. O que não havia era um jeito de criar uma textura GL a
//! partir de pixels e um `draw_*` que a ligasse — então o mundo do jogo era
//! feito de cor por malha.
//!
//! ## O que este módulo faz
//!
//! Cria a textura a partir de RGBA, guarda o handle atrás de um `u64` (mesma
//! disciplina do resto da engine: `0` é inválido, handles nunca são reusados) e
//! apaga quando o chamador pede. **Não** tem `Drop` que fala com o GL: o
//! contexto pode já ter morrido, e um `Drop` que chama `delete_texture` num
//! contexto destruído é um erro que só aparece em produção.
//!
//! A filtragem é `LINEAR` com `REPEAT`: a alternativa (`NEAREST`/`CLAMP`) faz a
//! textura de asfalto virar blocos e a de parede mostrar a emenda em cada
//! extremo de UV. Não há mipmap: o mundo é desenhado a poucos metros e gerar
//! mipmap de 6 texturas custaria memória sem ganhar nada num painel de 640×480.

use glow::HasContext;

use crate::error::{Error3D, Result3D};

/// Maior dimensão aceita. `8192` é o limite do Mali-G52; pedir mais devolve
/// `GL_INVALID_VALUE` em silêncio e a textura sai preta.
pub const MAX_DIMENSION: u32 = 8192;

/// Uma textura GL com as suas medidas.
pub(crate) struct GlTexture {
    /// Nome GL.
    pub(crate) handle: glow::Texture,
    /// Largura em pixels.
    pub(crate) width: u32,
    /// Altura em pixels.
    pub(crate) height: u32,
}

impl GlTexture {
    /// Cria a textura a partir de RGBA8 (linha a linha, `w*h*4` bytes).
    pub(crate) fn from_rgba(
        gl: &glow::Context,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result3D<Self> {
        if width == 0 || height == 0 {
            return Err(Error3D::InvalidArgument(
                "texture width and height must be non-zero".to_owned(),
            ));
        }
        if width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(Error3D::InvalidArgument(format!(
                "texture {width}x{height} exceeds the {MAX_DIMENSION} limit"
            )));
        }
        let esperado = width as usize * height as usize * 4;
        if pixels.len() != esperado {
            return Err(Error3D::InvalidArgument(format!(
                "pixel buffer has {} bytes; {width}x{height} RGBA needs {esperado}",
                pixels.len()
            )));
        }

        // SAFETY: `gen_texture`/`tex_image_2d`/`tex_parameter_i32` são GLES 2.0
        // puro, e o buffer tem exatamente `w*h*4` bytes — conferido acima.
        let handle = unsafe {
            let handle = gl
                .create_texture()
                .map_err(|e| Error3D::GlObject(format!("create_texture failed: {e}")))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(handle));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::REPEAT as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::REPEAT as i32,
            );
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(pixels),
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
            handle
        };

        Ok(Self {
            handle,
            width,
            height,
        })
    }

    /// Apaga a textura no contexto dado.
    pub(crate) fn destroy(self, gl: &glow::Context) {
        // SAFETY: a textura pertence a este contexto e `self` é consumido, então
        // não há como apagar duas vezes.
        unsafe { gl.delete_texture(self.handle) };
    }
}
