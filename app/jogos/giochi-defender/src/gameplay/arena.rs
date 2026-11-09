//! A arena: o asteroide central e o céu.
//!
//! ## O asteroide
//!
//! Icosaedro subdividido uma vez (80 triângulos) com distorção determinística.
//! Normais por face, então cada triângulo tem o seu tom e o objeto lê como
//! *low-poly* em vez de esfera lisa. O raio nominal é [`RAIO_ASTEROIDE`]; a
//! distorção fica dentro de [`DISTORCAO`] dele, o que dá ao bloco 1b uma esfera
//! de colisão que **sempre** contém a malha.
//!
//! ## O céu
//!
//! 500 estrelas assadas numa malha por faixa de brilho: cubos minúsculos e não
//! quads, porque um quad deitado desaparece quando visto de lado — e a câmera
//! orbita 360°. Três faixas em vez de 500 desenhos: o envelope deste projeto é de
//! poucas chamadas por quadro, e uma estrela não vale uma chamada.
//!
//! As estrelas são desenhadas com luz **emissiva** (ambiente 1.0, cor de luz
//! preta): sem isso o `phong` multiplicaria a cor pela componente difusa e cada
//! estrela ficaria com o brilho dependendo da face que aponta para a câmera — o
//! céu cintilaria ao girar.
//!
//! ## Iluminação do bloco 1c
//!
//! A rocha e os cubos usam o shader `phong` com uma luz direcional **vinda do
//! topo**. O `flat` já tinha um termo de Lambert, mas com a direção fixa dentro
//! do shader: dava para ver a forma e não dava para escolher de onde vem a luz.
//! Com o `phong` a direção, a cor, o ambiente e o especular são do jogo, e o que
//! muda entre a rocha e o cubo é a dureza do brilho — rocha fosca, metal quente.

use crate::utils::procedural_mesh::{generate_icosphere, generate_starfield};
use giochi_engine_3d::math3d::{Camera3D, Color3D, Transform3D, Vec3};
use giochi_engine_3d::render3d::{DirectionalLight, MeshHandle, RenderContext3D, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::Result3D;

/// Raio nominal do asteroide, em unidades de mundo.
pub const RAIO_ASTEROIDE: f32 = 2.0;
/// Fração do raio que a distorção desloca, para mais ou para menos.
pub const DISTORCAO: f32 = 0.22;
/// Distância do céu ao centro da arena.
///
/// Grande o bastante para que orbitar a até 15 unidades não mude a posição
/// aparente das estrelas (paralaxe de 12 %) e pequena o bastante para caber no
/// plano distante de 400.
pub const RAIO_CEU: f32 = 120.0;
/// Total de estrelas. Somadas as três faixas de [`FAIXAS`].
pub const ESTRELAS: u32 = 500;

/// Cor do asteroide: cinza-médio, levemente azulado para não parecer concreto.
pub const COR_ASTEROIDE: Color3D = Color3D::new(0.58, 0.60, 0.66, 1.0);

/// Cor de fundo do espaço.
pub const COR_ESPACO: Color3D = Color3D::new(0.015, 0.018, 0.032, 1.0);

/// As três faixas de estrelas: `(quantidade, aresta do cubo, cor)`.
///
/// A soma das quantidades tem de dar [`ESTRELAS`]; há teste. Separar por brilho
/// em vez de variar a cor por vértice é uma limitação da engine atual (a cor é
/// uniforme por desenho) usada a favor: três chamadas dão o céu inteiro.
pub const FAIXAS: [(u32, f32, [f32; 3]); 3] = [
    (90, 0.55, [0.86, 0.91, 1.00]),
    (170, 0.42, [0.62, 0.68, 0.82]),
    (240, 0.30, [0.40, 0.45, 0.58]),
];

/// Uma faixa de estrelas já na GPU.
#[derive(Debug, Clone, Copy)]
struct FaixaEstelar {
    malha: MeshHandle,
    cor: Color3D,
}

/// Asteroide + céu, com os recursos de GPU que eles usam.
pub struct Arena {
    asteroide: MeshHandle,
    estrelas: Vec<FaixaEstelar>,
    shader_asteroide: ShaderHandle,
    shader_estrelas: ShaderHandle,
}

impl Arena {
    /// Cria malhas e shaders.
    ///
    /// Dois shaders e não um: o asteroide usa `flat` (o facetado aparece pelo
    /// termo de Lambert fixo do shader) e as estrelas usam `phong`, que é o único
    /// que respeita `ambient` — é o que as deixa com brilho constante.
    pub fn new(render: &mut RenderContext3D) -> Result3D<Self> {
        let malha = generate_icosphere(1, RAIO_ASTEROIDE, DISTORCAO);
        let asteroide = render.create_mesh(&malha.vertices, &malha.indices)?;
        let shader_asteroide = render.create_builtin_shader(BuiltinShader::Phong)?;
        let shader_estrelas = render.create_builtin_shader(BuiltinShader::Phong)?;

        let mut estrelas = Vec::with_capacity(FAIXAS.len());
        for (indice, (quantidade, aresta, cor)) in FAIXAS.iter().enumerate() {
            let malha = generate_starfield(
                *quantidade,
                RAIO_CEU,
                *aresta,
                // Semente derivada do índice: as três faixas não compartilham
                // posições, senão as estrelas se sobrepõem e o céu parece menor.
                0x5EED_0000 + indice as u64,
            );
            let handle = render.create_mesh(&malha.vertices, &malha.indices)?;
            estrelas.push(FaixaEstelar {
                malha: handle,
                cor: Color3D::new(cor[0], cor[1], cor[2], 1.0),
            });
        }

        Ok(Self {
            asteroide,
            estrelas,
            shader_asteroide,
            shader_estrelas,
        })
    }

    /// Raio nominal do asteroide — o que o bloco 1b vai usar como esfera de
    /// colisão.
    pub fn raio(&self) -> f32 {
        RAIO_ASTEROIDE
    }

    /// Raio **externo** da malha, já com a distorção.
    ///
    /// Um inimigo que encosta aqui já tocou a rocha; usar o raio nominal deixaria
    /// as pontas atravessarem o cubo antes de contar o dano.
    pub fn raio_externo(&self) -> f32 {
        RAIO_ASTEROIDE * (1.0 + DISTORCAO)
    }

    /// Quantas estrelas o céu tem, somando as faixas.
    pub fn estrelas(&self) -> u32 {
        FAIXAS.iter().map(|(quantidade, _, _)| quantidade).sum()
    }

    /// Desenha o céu e o asteroide.
    pub fn desenhar(&self, render: &mut RenderContext3D, camera: &Camera3D) -> Result3D<()> {
        // Céu primeiro e sem teste de profundidade útil: as estrelas estão a 120
        // unidades, atrás de tudo, e o teste normal já as ordena.
        render.set_light(luz_emissiva());
        for faixa in &self.estrelas {
            render.draw_mesh_colored(
                faixa.malha,
                self.shader_estrelas,
                &Transform3D::IDENTITY,
                camera,
                faixa.cor,
            )?;
        }

        render.set_light(luz_do_asteroide());
        render.draw_mesh_colored(
            self.asteroide,
            self.shader_asteroide,
            &Transform3D::IDENTITY,
            camera,
            COR_ASTEROIDE,
        )?;

        Ok(())
    }

    /// Devolve à engine tudo o que a arena criou.
    ///
    /// Chamado no fim da execução; a engine também limpa no `Drop`, mas depender
    /// disso esconde um vazamento em vez de evitá-lo.
    pub fn destruir(self, render: &mut RenderContext3D) {
        for faixa in self.estrelas {
            render.destroy_mesh(faixa.malha);
        }
        render.destroy_mesh(self.asteroide);
        render.destroy_shader(self.shader_asteroide);
        render.destroy_shader(self.shader_estrelas);
    }
}

/// Luz da rocha: branca, do alto e **do lado da câmera**, com brilho fosco.
///
/// A direção aponta **para onde a luz viaja** (convenção da engine, `docs/3D.md`
/// §5.1), então "do topo" é um Y negativo. O `x` e o `z` não são zero de
/// propósito: luz exatamente vertical dá o mesmo termo difuso a todas as faces
/// laterais e o cubo vira um hexágono plano.
///
/// O `z` é **negativo** porque a câmera inicial fica em `+Z` olhando para `-Z`:
/// com a luz viajando para `-z`, a face que o jogador vê é a iluminada. Medido
/// com a luz viajando para `+z` (a primeira versão), a face da câmera ficava só
/// com o ambiente e a maior área da tela caía em `(38,40,44)` — um asteroide
/// quase preto com o topo claro, que é o oposto de volume.
pub fn luz_do_asteroide() -> DirectionalLight {
    let mut luz = DirectionalLight::sun();
    luz.direction = Vec3::new(-0.32, -1.0, -0.50);
    // Frio, para a rocha não puxar para o amarelo do sol padrão.
    luz.color = Color3D::new(0.92, 0.95, 1.0, 1.0);
    luz.ambient = 0.24;
    luz.specular = 0.16;
    luz.shininess = 18.0;
    luz
}

/// Luz dos inimigos: mais quente, mais dura e mais especular que a da rocha.
///
/// `ambient` mais alto porque o cubo é a ameaça: um cubo escuro contra um céu
/// escuro some, e "não vi de onde veio" é o pior defeito que este jogo pode ter.
pub fn luz_dos_inimigos() -> DirectionalLight {
    let mut luz = DirectionalLight::sun();
    // Mesma razão da rocha para o `z` negativo: o cubo que vem na direção do
    // jogador tem de estar iluminado do lado que ele vê.
    luz.direction = Vec3::new(-0.26, -1.0, -0.55);
    luz.color = Color3D::new(1.0, 0.94, 0.86, 1.0);
    luz.ambient = 0.36;
    luz.specular = 0.55;
    luz.shininess = 42.0;
    luz
}

/// Luz que faz a cor do objeto chegar à tela sem atenuação.
///
/// `ambient = 1.0` e cor de luz preta: no `phong.frag` o resultado é
/// `cor * 1.0 + cor * 0 * difusa + 0 * especular`, ou seja, a própria cor. É o
/// mesmo truque que o `neon-drift` usa para o neon.
fn luz_emissiva() -> DirectionalLight {
    let mut luz = DirectionalLight::sun();
    luz.color = Color3D::new(0.0, 0.0, 0.0, 1.0);
    luz.ambient = 1.0;
    luz.specular = 0.0;
    luz
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_faixas_somam_o_total_de_estrelas() {
        let soma: u32 = FAIXAS.iter().map(|(quantidade, _, _)| quantidade).sum();
        assert_eq!(soma, ESTRELAS);
    }

    #[test]
    fn a_cor_de_cada_faixa_e_decrescente() {
        // O céu tem de ter hierarquia: brilhante, média, fraca. Se as três
        // ficarem iguais, ele vira um padrão uniforme e some a profundidade.
        let brilho = |cor: [f32; 3]| cor[0] + cor[1] + cor[2];
        assert!(brilho(FAIXAS[0].2) > brilho(FAIXAS[1].2));
        assert!(brilho(FAIXAS[1].2) > brilho(FAIXAS[2].2));
    }

    #[test]
    fn a_aresta_diminui_com_o_brilho() {
        assert!(FAIXAS[0].1 > FAIXAS[1].1);
        assert!(FAIXAS[1].1 > FAIXAS[2].1);
    }

    #[test]
    fn a_luz_das_estrelas_devolve_a_cor_pura() {
        // O que garante que as estrelas não cintilam: o termo ambiente é a cor
        // inteira e os outros dois são zero.
        let luz = luz_emissiva();
        assert_eq!(luz.ambient, 1.0);
        assert_eq!(luz.color.r, 0.0);
        assert_eq!(luz.color.g, 0.0);
        assert_eq!(luz.color.b, 0.0);
        assert_eq!(luz.specular, 0.0);
    }

    #[test]
    fn o_raio_externo_cobre_a_distorcao() {
        // O bloco 1b usa este número para decidir "encostou". Ele tem de ser
        // maior que o raio nominal, senão as pontas do asteroide atravessam o
        // cubo antes de contar o dano.
        let arena_raio = RAIO_ASTEROIDE * (1.0 + DISTORCAO);
        assert!(arena_raio > RAIO_ASTEROIDE);
        assert!((arena_raio - 2.44).abs() < 1e-4);
    }
}
