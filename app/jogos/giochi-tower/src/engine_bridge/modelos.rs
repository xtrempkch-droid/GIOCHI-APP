//! Modelos 3D e texturas do jogo, carregados dos arquivos que o gerador escreve.
//!
//! # O que existia antes, e por que mudou
//!
//! Até o snapshot de 13/11/2026 a engine só desenhava primitiva procedural
//! (cubo, esfera, plano) e não criava textura nenhuma: o vértice carregava `uv`
//! desde a fase 11, mas todo shader `textured` amostrava a unidade 0 vazia e saía
//! preto. Era por isso que os personagens deste jogo eram cubos — não por escolha
//! de arte.
//!
//! Hoje existem `load_obj_mesh` e `create_texture`, e este módulo é o único lugar
//! que sabe disso. O resto do jogo pede [`Catalogo::desenhar`] e recebe um modelo
//! desenhado.
//!
//! # A caixa é medida, não declarada
//!
//! `altura` e `raio` saem dos vértices do OBJ. Uma tabela escrita à mão
//! envelheceria na primeira vez que alguém mexesse no gerador, e o sintoma seria
//! o personagem afundando no chão ou flutuando — sem erro.
//!
//! # Se um arquivo faltar
//!
//! `Catalogo::carregar` **não** falha o jogo: registra o erro e aquele modelo
//! passa a usar o cubo. Um asset ausente num pacote mal montado tem de degradar,
//! não impedir de jogar. O erro vai para o log uma vez, com o nome do arquivo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use giochi_engine_3d::math3d::{Camera3D, Color3D, Quat, Transform3D, Vec3};
use giochi_engine_3d::{MeshHandle, RenderContext3D, TextureHandle};

use crate::errors::{GameError, Result};

/// Um modelo pronto para desenhar: malha, textura e a caixa medida.
#[derive(Debug, Clone, Copy)]
pub struct Modelo {
    /// Malha na GPU.
    pub malha: MeshHandle,
    /// Textura na GPU.
    pub textura: TextureHandle,
    /// Altura medida do arquivo, em metros.
    pub altura: f32,
    /// Raio horizontal (o maior entre X e Z), em metros.
    pub raio: f32,
    /// O modelo veio do disco, ou é o cubo de reserva?
    pub do_disco: bool,
}

/// Um pedido de desenho de modelo.
///
/// Struct em vez de seis parâmetros soltos: `altura` e `giro` são ambos `f32`, e
/// trocá-los na chamada não daria erro — daria um personagem deitado do tamanho
/// errado, que é o tipo de defeito que só aparece na tela.
#[derive(Debug, Clone, Copy)]
pub struct Pedido<'a> {
    /// Nome no catálogo (`soldado`, `tank`, `torre`…).
    pub modelo: &'a str,
    /// Base do modelo (o pé fica em `y = 0` do arquivo, então isto é o chão).
    pub pos: Vec3,
    /// Altura desejada em metros — a escala sai de `altura / modelo.altura`.
    pub altura: f32,
    /// Giro em torno de Y, em radianos.
    pub giro: f32,
    /// Cor multiplicada sobre a textura (`Color3D::WHITE` mostra o arquivo).
    pub tint: Color3D,
}

impl<'a> Pedido<'a> {
    /// Um pedido com giro zero e sem tingimento.
    pub fn novo(modelo: &'a str, pos: Vec3, altura: f32) -> Self {
        Self {
            modelo,
            pos,
            altura,
            giro: 0.0,
            tint: Color3D::WHITE,
        }
    }

    /// Muda o giro.
    pub fn com_giro(mut self, giro: f32) -> Self {
        self.giro = giro;
        self
    }

    /// Muda o tingimento.
    pub fn com_tint(mut self, tint: Color3D) -> Self {
        self.tint = tint;
        self
    }
}

/// Os modelos do jogo, por nome.
#[derive(Debug, Default)]
pub struct Catalogo {
    modelos: BTreeMap<String, Modelo>,
    shader: Option<giochi_engine_3d::ShaderHandle>,
}

/// Os nomes que o jogo usa. A ordem importa só para o log.
pub const NOMES: [&str; 15] = [
    "soldado", "guerreiro", "sniper", "medico", "grunt", "tank", "ranged", "swarm", "flying",
    "tanker", "plasma", "brain", "queen", "coronel", "torre",
];

/// Onde os assets moram, em ordem de tentativa.
///
/// A instalação no aparelho copia o binário para
/// `<app>/bin/` e os assets para `<app>/assets/`, então o caminho relativo ao
/// executável é o primeiro. Os outros cobrem `cargo run` e `cargo test`.
pub fn diretorios_candidatos() -> Vec<PathBuf> {
    let mut saida = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            saida.push(dir.join("assets"));
            if let Some(acima) = dir.parent() {
                saida.push(acima.join("assets"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        saida.push(cwd.join("assets"));
    }
    saida.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    saida
}

/// Devolve o primeiro diretório de assets que existe e tem `modelos/`.
///
/// `GIOCHI_TOWER_ASSETS` **vence** a busca e, se ela não tiver `modelos/`, o
/// resultado é `None` — não uma queda silenciosa para o diretório do
/// repositório. A diferença importa para testar o caminho "sem assets": engolir
/// o pedido faria o teste medir o diretório errado e passar sem provar nada.
pub fn achar_diretorio() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(crate::ASSETS_DIR_ENV) {
        let dir = PathBuf::from(dir);
        return if dir.join("modelos").is_dir() {
            Some(dir)
        } else {
            log::warn!(
                "{}={} não tem modelos/; personagens ficam como cubos",
                crate::ASSETS_DIR_ENV,
                dir.display()
            );
            None
        };
    }
    diretorios_candidatos()
        .into_iter()
        .find(|dir| dir.join("modelos").is_dir())
}

impl Catalogo {
    /// Carrega todos os modelos de `dir` (o diretório `assets/`).
    ///
    /// Um modelo que falha entra com o cubo de reserva e o erro fica no log: o
    /// jogo continua jogável e o defeito é visível no primeiro quadro (um cubo no
    /// meio dos modelos), não numa tela preta.
    pub fn carregar(
        render: &mut RenderContext3D,
        dir: &Path,
        cubo: MeshHandle,
        textura_reserva: TextureHandle,
    ) -> Result<Self> {
        use giochi_engine_3d::shader::BuiltinShader;
        let shader = render.create_builtin_shader(BuiltinShader::Textured)?;
        let mut modelos = BTreeMap::new();

        for nome in NOMES {
            let obj = dir.join("modelos").join(format!("{nome}.obj"));
            let bmp = dir.join("texturas").join(format!("{nome}.bmp"));

            let malha = match render.load_obj_mesh(&obj) {
                Ok(malha) => malha,
                Err(erro) => {
                    log::warn!("modelo '{nome}' indisponível ({}): usando cubo — {erro}", obj.display());
                    modelos.insert(
                        nome.to_string(),
                        Modelo {
                            malha: cubo,
                            textura: textura_reserva,
                            altura: 1.0,
                            raio: 0.5,
                            do_disco: false,
                        },
                    );
                    continue;
                }
            };
            let textura = match render.create_texture(&bmp) {
                Ok(textura) => textura,
                Err(erro) => {
                    log::warn!("textura '{nome}' indisponível ({}): — {erro}", bmp.display());
                    textura_reserva
                }
            };
            let (altura, raio) = medir(&obj).unwrap_or((1.8, 0.4));
            modelos.insert(
                nome.to_string(),
                Modelo {
                    malha,
                    textura,
                    altura,
                    raio,
                    do_disco: true,
                },
            );
        }

        let do_disco = modelos.values().filter(|m| m.do_disco).count();
        log::info!(
            "modelos: {do_disco}/{} de {}, texturas: {} na GPU",
            NOMES.len(),
            dir.display(),
            render.texture_count()
        );

        Ok(Self {
            modelos,
            shader: Some(shader),
        })
    }

    /// O modelo com esse nome, se existir.
    pub fn get(&self, nome: &str) -> Option<&Modelo> {
        self.modelos.get(nome)
    }

    /// Quantos modelos vieram do disco (e não do cubo de reserva).
    pub fn do_disco(&self) -> usize {
        self.modelos.values().filter(|m| m.do_disco).count()
    }

    /// Desenha um modelo texturizado.
    pub fn desenhar(
        &self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
        pedido: Pedido<'_>,
    ) -> Result<()> {
        let Some(modelo) = self.modelos.get(pedido.modelo) else {
            return Err(GameError::message(format!(
                "modelo '{}' não está no catálogo",
                pedido.modelo
            )));
        };
        let Some(shader) = self.shader else {
            return Ok(());
        };
        let escala = if modelo.altura > 0.01 {
            pedido.altura / modelo.altura
        } else {
            1.0
        };
        let modelo_matriz = Transform3D::new(
            pedido.pos,
            Quat::from_axis_angle(Vec3::UP, pedido.giro),
            Vec3::new(escala, escala, escala),
        );
        let desenhou = render.draw_mesh_textured(
            modelo.malha,
            shader,
            modelo.textura,
            &modelo_matriz,
            camera,
            pedido.tint,
        )?;
        if !desenhou {
            log::warn!("modelo '{}': desenho ignorado (malha ou textura obsoleta)", pedido.modelo);
        }
        Ok(())
    }

    /// Desenha `n` cópias de um modelo numa chamada, com a cor no lugar da textura.
    ///
    /// É o caminho da horda: 100 inimigos texturizados custariam 100 chamadas de
    /// desenho, e no Cortex-A35 cada uma custa ~1,5 µs **de CPU** só para submeter
    /// — sem contar o custo por pixel. Uma chamada por tipo mantém o orçamento.
    /// A silhueta continua sendo a do modelo; o que se perde é o detalhe da
    /// textura, que a 640×480 e 20 m de distância não se vê.
    pub fn desenhar_lote(
        &self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
        nome: &str,
        matrizes: &[giochi_engine_3d::math3d::Mat4],
        cor: Color3D,
        shader_instanciado: giochi_engine_3d::ShaderHandle,
    ) -> Result<u32> {
        let Some(modelo) = self.modelos.get(nome) else {
            return Ok(0);
        };
        Ok(render.draw_instanced(
            modelo.malha,
            shader_instanciado,
            matrizes,
            camera,
            cor,
        )?)
    }

    /// Libera malhas e texturas.
    pub fn destruir(&mut self, render: &mut RenderContext3D, cubo: MeshHandle, reserva: TextureHandle) {
        // Nomes repetidos apontam para o cubo de reserva: destruir duas vezes
        // devolveria `false` na segunda e não é erro, mas é ruído no log.
        let mut malhas: Vec<MeshHandle> = Vec::new();
        let mut texturas: Vec<TextureHandle> = Vec::new();
        for modelo in self.modelos.values() {
            if modelo.malha != cubo && !malhas.contains(&modelo.malha) {
                malhas.push(modelo.malha);
            }
            if modelo.textura != reserva && !texturas.contains(&modelo.textura) {
                texturas.push(modelo.textura);
            }
        }
        for malha in malhas {
            let _ = render.destroy_mesh(malha);
        }
        for textura in texturas {
            let _ = render.destroy_texture(textura);
        }
        if let Some(shader) = self.shader.take() {
            let _ = render.destroy_shader(shader);
        }
        self.modelos.clear();
    }
}

/// Mede altura e raio de um OBJ sem GPU.
///
/// Reusa o loader da engine (`mesh::load_obj` é puro), então o número vem do
/// mesmo arquivo que a GPU vai desenhar — e não de um cálculo paralelo que pode
/// divergir na primeira vez que alguém mexer no gerador.
fn medir(caminho: &Path) -> Option<(f32, f32)> {
    let (vertices, _) = giochi_engine_3d::mesh::load_obj(caminho).ok()?;
    let mut y_min = f32::INFINITY;
    let mut y_max = f32::NEG_INFINITY;
    let mut raio: f32 = 0.0;
    for v in &vertices {
        y_min = y_min.min(v.position[1]);
        y_max = y_max.max(v.position[1]);
        raio = raio.max(v.position[0].abs()).max(v.position[2].abs());
    }
    Some(((y_max - y_min).max(0.01), raio.max(0.01)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_diretorio_de_assets_e_encontrado_no_repositorio() {
        let dir = achar_diretorio().expect("assets/ com modelos/");
        assert!(dir.join("modelos").join("soldado.obj").is_file(), "{dir:?}");
        assert!(dir.join("texturas").join("soldado.bmp").is_file(), "{dir:?}");
    }

    #[test]
    fn cada_nome_do_catalogo_tem_arquivo() {
        let dir = achar_diretorio().expect("assets");
        for nome in NOMES {
            assert!(
                dir.join("modelos").join(format!("{nome}.obj")).is_file(),
                "falta o modelo {nome}"
            );
            assert!(
                dir.join("texturas").join(format!("{nome}.bmp")).is_file(),
                "falta a textura de {nome}"
            );
        }
    }

    #[test]
    fn a_medida_sai_do_arquivo_e_nao_de_uma_tabela() {
        let dir = achar_diretorio().expect("assets");
        let (altura_soldado, raio_soldado) =
            medir(&dir.join("modelos").join("soldado.obj")).expect("mede");
        let (altura_torre, _) = medir(&dir.join("modelos").join("torre.obj")).expect("mede");
        // Um humanoide de 1,8 m: a medida é da geometria, então a faixa é larga de
        // propósito — o teste existe para pegar "mediu o eixo errado".
        assert!(
            (1.0..3.0).contains(&altura_soldado),
            "soldado mede {altura_soldado:.2} m de altura"
        );
        assert!(
            (0.2..1.2).contains(&raio_soldado),
            "soldado mede {raio_soldado:.2} m de raio"
        );
        assert!(
            altura_torre > altura_soldado,
            "torre {altura_torre:.2} m contra soldado {altura_soldado:.2} m"
        );
    }

    #[test]
    fn um_arquivo_ausente_devolve_none_em_vez_de_panico() {
        // O catálogo chama `medir` para todo modelo; um arquivo apagado tem de cair
        // no parâmetro padrão, não derrubar o carregamento inteiro.
        assert!(medir(Path::new("/nao/existe.obj")).is_none());
    }
}
