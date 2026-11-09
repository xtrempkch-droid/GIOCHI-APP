use std::error::Error;

use giochi_cobrinha_core::{Jogo, COLUNAS, LINHAS};
use giochi_engine_3d::math3d::{Camera3D, Color3D, Mat4, Quat, Transform3D, Vec3};
use giochi_engine_3d::mesh::{cube, StaticBatchBuilder};
use giochi_engine_3d::render3d::{MeshHandle, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::RenderContext3D;

use crate::video::Rotacao;

const FUNDO: Color3D = Color3D::new(0.025, 0.035, 0.075, 1.0);
const TABULEIRO_CLARO: Color3D = Color3D::new(0.12, 0.21, 0.34, 1.0);
const TABULEIRO_ESCURO: Color3D = Color3D::new(0.075, 0.13, 0.24, 1.0);
const BORDA: Color3D = Color3D::new(0.22, 0.46, 0.67, 1.0);
const CORPO: Color3D = Color3D::new(0.12, 0.78, 0.72, 1.0);
const CABECA: Color3D = Color3D::new(0.35, 1.0, 0.78, 1.0);
const COMIDA: Color3D = Color3D::new(1.0, 0.22, 0.3, 1.0);
const PLACAR: Color3D = Color3D::new(1.0, 0.72, 0.24, 1.0);
const PRONTA: Color3D = Color3D::new(0.22, 0.96, 0.48, 1.0);
const PERDIDA: Color3D = Color3D::new(1.0, 0.16, 0.22, 1.0);

pub struct Cena3D {
    casas_claras: MeshHandle,
    casas_escuras: MeshHandle,
    paredes: MeshHandle,
    painel_placar: MeshHandle,
    cubo: MeshHandle,
    fruta: MeshHandle,
    phong: ShaderHandle,
    instanciado: Option<ShaderHandle>,
    instancing: bool,
    rotacao: Rotacao,
    corpo: Vec<Mat4>,
    digitos: Vec<Mat4>,
}

impl Cena3D {
    pub fn nova(render: &mut RenderContext3D, rotacao: Rotacao) -> Result<Self, Box<dyn Error>> {
        let (vertices, indices) = cube(1.0);
        let mut claras = StaticBatchBuilder::with_capacity(
            (COLUNAS * LINHAS / 2) as usize * vertices.len(),
            (COLUNAS * LINHAS / 2) as usize * indices.len(),
        );
        let mut escuras = StaticBatchBuilder::with_capacity(
            (COLUNAS * LINHAS / 2) as usize * vertices.len(),
            (COLUNAS * LINHAS / 2) as usize * indices.len(),
        );

        for linha in 0..LINHAS {
            for coluna in 0..COLUNAS {
                let (x, z) = mundo(coluna as f32, linha as f32, rotacao);
                let model = modelo(x, -0.08, z, Vec3::new(0.97, 0.16, 0.97));
                if (linha + coluna) % 2 == 0 {
                    claras = claras.add_mesh(&vertices, &indices, &model)?;
                } else {
                    escuras = escuras.add_mesh(&vertices, &indices, &model)?;
                }
            }
        }

        let casas_claras = enviar_lote(render, &claras)?;
        let casas_escuras = enviar_lote(render, &escuras)?;
        let paredes = criar_paredes(render, &vertices, &indices, rotacao)?;
        let painel_placar = render.create_mesh(&vertices, &indices)?;
        let cubo = render.create_mesh(&vertices, &indices)?;
        let fruta = render.create_sphere_mesh(0.42, 12, 16)?;
        let phong = render.create_builtin_shader(BuiltinShader::Phong)?;
        let instancing = render.supports_instancing();
        let instanciado = if instancing {
            Some(render.create_builtin_shader(BuiltinShader::Instanced)?)
        } else {
            None
        };

        Ok(Self {
            casas_claras,
            casas_escuras,
            paredes,
            painel_placar,
            cubo,
            fruta,
            phong,
            instanciado,
            instancing,
            rotacao,
            corpo: Vec::with_capacity((COLUNAS * LINHAS) as usize),
            digitos: Vec::with_capacity(21),
        })
    }

    pub fn usa_instancing(&self) -> bool {
        self.instancing
    }

    pub fn preparar_quadro(&self, render: &mut RenderContext3D) {
        render.set_clear_color(FUNDO);
    }

    pub fn desenhar(
        &mut self,
        render: &mut RenderContext3D,
        jogo: &Jogo,
        alpha: f32,
        tempo: f32,
    ) -> Result<(), Box<dyn Error>> {
        let camera = camera_para(render.drawable_size(), tempo);
        render.draw_mesh_colored(
            self.casas_claras,
            self.phong,
            &Transform3D::IDENTITY,
            &camera,
            TABULEIRO_CLARO,
        )?;
        render.draw_mesh_colored(
            self.casas_escuras,
            self.phong,
            &Transform3D::IDENTITY,
            &camera,
            TABULEIRO_ESCURO,
        )?;
        render.draw_mesh_colored(
            self.paredes,
            self.phong,
            &Transform3D::IDENTITY,
            &camera,
            BORDA,
        )?;

        self.desenhar_placar(render, &camera, jogo.pontos)?;
        self.desenhar_corpo(render, &camera, jogo, alpha)?;

        let (fx, fz) = mundo(jogo.comida.x as f32, jogo.comida.y as f32, self.rotacao);
        let pulsar = 1.0 + 0.12 * tempo.sin();
        let comida = Transform3D::new(
            Vec3::new(fx, 0.43, fz),
            Quat::from_axis_angle(Vec3::UP, tempo),
            Vec3::new(pulsar, pulsar, pulsar),
        );
        render.draw_mesh_colored(self.fruta, self.phong, &comida, &camera, COMIDA)?;

        match jogo.estado {
            giochi_cobrinha_core::Estado::Pronto => {
                let pronto = Transform3D::new(
                    Vec3::new(0.0, 1.0 + 0.12 * tempo.sin(), 0.0),
                    Quat::from_axis_angle(Vec3::UP, tempo * 0.7),
                    Vec3::new(0.66, 0.66, 0.66),
                );
                render.draw_mesh_colored(self.cubo, self.phong, &pronto, &camera, PRONTA)?;
            }
            giochi_cobrinha_core::Estado::Perdido => {
                let (hx, hz) = mundo(
                    jogo.cabeca_pos().x as f32,
                    jogo.cabeca_pos().y as f32,
                    self.rotacao,
                );
                let fim = Transform3D::new(
                    Vec3::new(hx, 0.62, hz),
                    Quat::from_axis_angle(Vec3::UP, tempo),
                    Vec3::new(0.7, 0.7, 0.7),
                );
                render.draw_mesh_colored(self.fruta, self.phong, &fim, &camera, PERDIDA)?;
            }
            giochi_cobrinha_core::Estado::Jogando => {}
        }

        Ok(())
    }

    fn desenhar_corpo(
        &mut self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
        jogo: &Jogo,
        alpha: f32,
    ) -> Result<(), Box<dyn Error>> {
        self.corpo.clear();
        for indice in 1..jogo.tamanho() {
            let pos = jogo.segmento(indice);
            let (x, z) = mundo(pos.x as f32, pos.y as f32, self.rotacao);
            self.corpo
                .push(modelo(x, 0.21, z, Vec3::new(0.78, 0.32, 0.78)));
        }

        if let Some(shader) = self.instanciado {
            render.draw_instanced(self.cubo, shader, &self.corpo, camera, CORPO)?;
        } else {
            for matriz in &self.corpo {
                let model = Transform3D::new(
                    Vec3::new(matriz.data[12], matriz.data[13], matriz.data[14]),
                    Quat::IDENTITY,
                    Vec3::new(0.78, 0.32, 0.78),
                );
                render.draw_mesh_colored(self.cubo, self.phong, &model, camera, CORPO)?;
            }
        }

        let (hx, hz) = if jogo.estado == giochi_cobrinha_core::Estado::Jogando {
            let (x, y) = jogo.cabeca_interpolada(alpha);
            mundo(x, y, self.rotacao)
        } else {
            mundo(
                jogo.cabeca_pos().x as f32,
                jogo.cabeca_pos().y as f32,
                self.rotacao,
            )
        };
        let cabeca = Transform3D::new(
            Vec3::new(hx, 0.28, hz),
            Quat::IDENTITY,
            Vec3::new(0.9, 0.4, 0.9),
        );
        render.draw_mesh_colored(self.cubo, self.phong, &cabeca, camera, CABECA)?;
        Ok(())
    }

    fn desenhar_placar(
        &mut self,
        render: &mut RenderContext3D,
        camera: &Camera3D,
        pontos: u32,
    ) -> Result<(), Box<dyn Error>> {
        let profundidade = profundidade(self.rotacao);
        let placa = Transform3D::new(
            Vec3::new(0.0, 0.03, profundidade / 2.0 + 1.25),
            Quat::IDENTITY,
            Vec3::new(3.5, 0.16, 1.45),
        );
        render.draw_mesh_colored(
            self.painel_placar,
            self.phong,
            &placa,
            camera,
            Color3D::new(0.035, 0.07, 0.12, 1.0),
        )?;

        self.digitos.clear();
        let valor = pontos.min(999);
        let centenas = valor / 100;
        let dezenas = (valor / 10) % 10;
        let unidades = valor % 10;
        for (i, digito) in [centenas, dezenas, unidades].into_iter().enumerate() {
            let x = (i as f32 - 1.0) * 0.9;
            acrescentar_digito(&mut self.digitos, digito, x, profundidade / 2.0 + 1.25);
        }

        if self.instancing {
            render.draw_instanced(
                self.cubo,
                self.instanciado.expect("shader de instancing carregado"),
                &self.digitos,
                camera,
                PLACAR,
            )?;
        } else {
            for matriz in &self.digitos {
                let model = Transform3D::new(
                    Vec3::new(matriz.data[12], matriz.data[13], matriz.data[14]),
                    Quat::IDENTITY,
                    Vec3::new(matriz.data[0], matriz.data[5], matriz.data[10]),
                );
                render.draw_mesh_colored(self.cubo, self.phong, &model, camera, PLACAR)?;
            }
        }
        Ok(())
    }
}

fn enviar_lote(
    render: &mut RenderContext3D,
    lote: &StaticBatchBuilder,
) -> Result<MeshHandle, Box<dyn Error>> {
    let (vertices, indices) = lote.geometry();
    Ok(render.create_mesh(vertices, indices)?)
}

fn criar_paredes(
    render: &mut RenderContext3D,
    vertices: &[giochi_engine_3d::mesh::Vertex],
    indices: &[u32],
    rotacao: Rotacao,
) -> Result<MeshHandle, Box<dyn Error>> {
    let (largura, profundidade) = dimensoes(rotacao);
    let mut lote = StaticBatchBuilder::with_capacity(4 * vertices.len(), 4 * indices.len());
    for (x, z, escala) in [
        (
            -largura / 2.0,
            0.0,
            Vec3::new(0.24, 0.6, profundidade + 0.35),
        ),
        (
            largura / 2.0,
            0.0,
            Vec3::new(0.24, 0.6, profundidade + 0.35),
        ),
        (
            0.0,
            -profundidade / 2.0,
            Vec3::new(largura + 0.35, 0.6, 0.24),
        ),
        (
            0.0,
            profundidade / 2.0,
            Vec3::new(largura + 0.35, 0.6, 0.24),
        ),
    ] {
        let model = modelo(x, 0.22, z, escala);
        lote = lote.add_mesh(vertices, indices, &model)?;
    }
    enviar_lote(render, &lote)
}

fn modelo(x: f32, y: f32, z: f32, escala: Vec3) -> Mat4 {
    Transform3D::new(Vec3::new(x, y, z), Quat::IDENTITY, escala).matrix()
}

fn camera_para(tamanho: (u32, u32), tempo: f32) -> Camera3D {
    let fov = 0.88;
    let distancia = 26.0;
    Camera3D::from_parts(
        Vec3::new(tempo.sin() * 0.35, distancia, 8.0),
        Vec3::ZERO,
        Vec3::new(0.0, 0.0, -1.0),
    )
    .with_fov_y(fov)
    .with_clip_planes(0.1, 100.0)
    .with_drawable_size(tamanho.0, tamanho.1)
}

fn dimensoes(rotacao: Rotacao) -> (f32, f32) {
    match rotacao {
        Rotacao::Nenhuma => (COLUNAS as f32, LINHAS as f32),
        Rotacao::Cw90 | Rotacao::Ccw90 => (LINHAS as f32, COLUNAS as f32),
    }
}

fn profundidade(rotacao: Rotacao) -> f32 {
    dimensoes(rotacao).1
}

fn mundo(coluna: f32, linha: f32, rotacao: Rotacao) -> (f32, f32) {
    let centro_x = (COLUNAS - 1) as f32 / 2.0;
    let centro_y = (LINHAS - 1) as f32 / 2.0;
    match rotacao {
        Rotacao::Nenhuma => (coluna - centro_x, linha - centro_y),
        Rotacao::Cw90 => (centro_y - linha, coluna - centro_x),
        Rotacao::Ccw90 => (linha - centro_y, centro_x - coluna),
    }
}

fn acrescentar_digito(buffer: &mut Vec<Mat4>, digito: u32, centro_x: f32, z: f32) {
    const SEGMENTOS: [[bool; 7]; 10] = [
        [true, true, true, true, true, true, false],
        [false, true, true, false, false, false, false],
        [true, true, false, true, true, false, true],
        [true, true, true, true, false, false, true],
        [false, true, true, false, false, true, true],
        [true, false, true, true, false, true, true],
        [true, false, true, true, true, true, true],
        [true, true, true, false, false, false, false],
        [true, true, true, true, true, true, true],
        [true, true, true, true, false, true, true],
    ];
    let tam = 0.42;
    let esp = 0.075;
    let posicoes = [
        (0.0, -0.36, tam, esp),
        (0.19, -0.18, esp, tam * 0.82),
        (0.19, 0.18, esp, tam * 0.82),
        (0.0, 0.0, tam, esp),
        (-0.19, 0.18, esp, tam * 0.82),
        (-0.19, -0.18, esp, tam * 0.82),
        (0.0, 0.36, tam, esp),
    ];
    for (ativo, (dx, dz, w, h)) in SEGMENTOS[digito as usize].into_iter().zip(posicoes) {
        if ativo {
            buffer.push(modelo(centro_x + dx, 0.19, z + dz, Vec3::new(w, 0.12, h)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabuleiro_rotacionado_preserva_todas_as_casas() {
        for rotacao in [Rotacao::Nenhuma, Rotacao::Cw90, Rotacao::Ccw90] {
            let mut pontos = std::collections::BTreeSet::new();
            for y in 0..LINHAS {
                for x in 0..COLUNAS {
                    let (wx, wz) = mundo(x as f32, y as f32, rotacao);
                    assert!(pontos.insert((wx.to_bits(), wz.to_bits())));
                }
            }
            assert_eq!(pontos.len(), (COLUNAS * LINHAS) as usize);
        }
    }

    #[test]
    fn grade_em_retrato_troca_largura_e_profundidade() {
        assert_eq!(dimensoes(Rotacao::Nenhuma), (24.0, 18.0));
        assert_eq!(dimensoes(Rotacao::Ccw90), (18.0, 24.0));
        assert_eq!(dimensoes(Rotacao::Cw90), (18.0, 24.0));
    }
}
