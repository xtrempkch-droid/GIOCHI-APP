//! Malhas procedurais: carro, prédios, postes, túnel.
//!
//! ## Por que não há `.obj` nenhum
//!
//! Um carro de corrida estilizado é um punhado de caixas com proporções certas.
//! Guardar isso num `.obj` significaria mais um arquivo para empacotar, mais um
//! caminho para faltar no aparelho e um binário opaco no repositório — e o
//! `load_obj_mesh` da engine só existe para quem tem modelo de verdade.
//!
//! Aqui cada peça é aritmética: a mesma lição dos ícones do launcher, que são
//! listas de linhas em vez de PNG.

use giochi_engine_3d::mesh::{cube, Vertex};

/// Acumulador de geometria estática.
///
/// Junta caixas e quads num único par vértices/índices para virar **uma** malha.
/// Uma malha é um draw call: a pista inteira sai em um, os quatro carros em cinco
/// cada, e o orçamento do quadro fica previsível.
pub struct Construtor {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Construtor {
    /// Construtor vazio.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Construtor com capacidade estimada, para não realocar a cada peça.
    pub fn com_capacidade(vertices: usize, indices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(indices),
        }
    }

    /// Vértices acumulados.
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Índices acumulados.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// `true` quando não há nada (malha vazia não deve ser enviada à GPU).
    pub fn vazio(&self) -> bool {
        self.indices.is_empty()
    }

    /// Acrescenta um cubo unitário esticado para `tamanho` e centrado em `centro`.
    pub fn caixa(&mut self, centro: [f32; 3], tamanho: [f32; 3]) -> &mut Self {
        let (v, i) = cube(1.0);
        let base = self.vertices.len() as u32;
        self.vertices.extend(v.into_iter().map(|mut vert| {
            vert.position = [
                vert.position[0] * tamanho[0] + centro[0],
                vert.position[1] * tamanho[1] + centro[1],
                vert.position[2] * tamanho[2] + centro[2],
            ];
            vert
        }));
        self.indices
            .extend(i.into_iter().map(|indice| indice + base));
        self
    }

    /// Acrescenta uma caixa rotacionada em torno de Y por `giro` radianos.
    ///
    /// Usada pelas peças do carro (que é montado com o nariz em -Z e depois
    /// girado junto com o corpo) e pelos prédios alinhados à pista.
    pub fn caixa_girada(
        &mut self,
        centro: [f32; 3],
        tamanho: [f32; 3],
        giro: f32,
    ) -> &mut Self {
        let (sen, cos) = giro.sin_cos();
        let (v, i) = cube(1.0);
        let base = self.vertices.len() as u32;
        self.vertices.extend(v.into_iter().map(|mut vert| {
            let x = vert.position[0] * tamanho[0];
            let y = vert.position[1] * tamanho[1];
            let z = vert.position[2] * tamanho[2];
            vert.position = [
                x * cos + z * sen + centro[0],
                y + centro[1],
                -x * sen + z * cos + centro[2],
            ];
            // A normal também gira; sem isto a iluminação fica no plano antigo.
            let nx = vert.normal[0];
            let nz = vert.normal[2];
            vert.normal = [nx * cos + nz * sen, vert.normal[1], -nx * sen + nz * cos];
            vert
        }));
        self.indices
            .extend(i.into_iter().map(|indice| indice + base));
        self
    }

    /// Acrescenta um quadrilátero a partir de quatro cantos, com normal dada.
    ///
    /// **A ordem dos cantos não importa**: o sentido é deduzido da normal. Errar a
    /// ordem some com a peça sob `cull_face`, e o sintoma é cruel — o asfalto não
    /// aparece e não há erro nenhum, só uma tela de céu com prédios flutuando.
    /// Aconteceu na primeira versão do chão, e o `--so asfalto` mostrou um quadro
    /// 100% vazio com a malha tendo 2 560 vértices.
    pub fn quad(
        &mut self,
        cantos: [[f32; 3]; 4],
        normal: [f32; 3],
        uv_escala: f32,
    ) -> &mut Self {
        let base = self.vertices.len() as u32;
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (canto, uv) in cantos.iter().zip(uvs.iter()) {
            self.vertices.push(Vertex::new(
                *canto,
                normal,
                [uv[0] * uv_escala, uv[1] * uv_escala],
            ));
        }
        // Produto vetorial do primeiro triângulo: se aponta contra a normal, o
        // quadrilátero precisa ser emitido ao contrário.
        let a = cantos[0];
        let b = cantos[1];
        let c = cantos[2];
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cruz = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let concordam = cruz[0] * normal[0] + cruz[1] * normal[1] + cruz[2] * normal[2] >= 0.0;
        if concordam {
            self.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.indices
                .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
        }
        self
    }

    /// Quadrilátero no plano do chão (`y` constante), visto de cima.
    pub fn chao(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2], y: f32) -> &mut Self {
        self.quad(
            [
                [a[0], y, a[1]],
                [b[0], y, b[1]],
                [c[0], y, c[1]],
                [d[0], y, d[1]],
            ],
            [0.0, 1.0, 0.0],
            1.0,
        )
    }
}

impl Default for Construtor {
    fn default() -> Self {
        Self::new()
    }
}

/// Malha de um quadrado unitário no plano XY, virado para +Z.
///
/// É a peça de tudo o que é "colado na tela": partículas, rastro e o HUD. O
/// tamanho vem da matriz de modelo, então uma malha serve para todos.
pub fn quad() -> (Vec<Vertex>, Vec<u32>) {
    let mut c = Construtor::com_capacidade(4, 6);
    c.quad(
        [
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ],
        [0.0, 0.0, 1.0],
        1.0,
    );
    (c.vertices, c.indices)
}

/// Peças do carro, cada uma com a sua cor de desenho.
///
/// ## Por que em cinco malhas
///
/// `draw_mesh_colored` pinta a malha inteira de uma cor. Um carro de uma cor só
/// seria um bloco: sem vidro escuro, sem farol e sem lanterna, ele lê como
/// caixa. Cinco malhas dão cinco cores e continuam sendo cinco draw calls por
/// carro — barato o bastante para quatro carros a 75 Hz.
pub struct CarroMalhas {
    /// Lataria.
    pub corpo: (Vec<Vertex>, Vec<u32>),
    /// Vidros e teto.
    pub cabine: (Vec<Vertex>, Vec<u32>),
    /// Rodas.
    pub rodas: (Vec<Vertex>, Vec<u32>),
    /// Faróis (frente, -Z no espaço local).
    pub farois: (Vec<Vertex>, Vec<u32>),
    /// Lanternas (traseira, +Z).
    pub lanternas: (Vec<Vertex>, Vec<u32>),
}

/// Monta as malhas do carro.
///
/// O carro aponta para **-Z** no espaço local, que é a convenção da câmera da
/// engine (olhando para -Z). O desenho gira o conjunto pelo rumo do carro.
///
/// Comprimento 4,4 m, largura 1,9 m, altura 1,15 m: proporção de carro esportivo
/// pequeno, e não de tanque — a 640×480 isso é o que dá leitura de "carro" em vez
/// de "caixa".
pub fn carro() -> CarroMalhas {
    let mut corpo = Construtor::com_capacidade(200, 120);
    // Assoalho/chassi.
    corpo.caixa([0.0, 0.34, 0.0], [1.86, 0.34, 4.30]);
    // Capô, mais baixo e mais estreito que o chassi.
    corpo.caixa([0.0, 0.56, -1.35], [1.66, 0.22, 1.55]);
    // Traseira.
    corpo.caixa([0.0, 0.58, 1.45], [1.70, 0.26, 1.35]);
    // Saias laterais: dão a sombra que separa o carro do chão.
    corpo.caixa([-0.92, 0.28, 0.0], [0.16, 0.20, 3.90]);
    corpo.caixa([0.92, 0.28, 0.0], [0.16, 0.20, 3.90]);
    // Aerofólio: a silhueta traseira sem ele parece caminhonete.
    corpo.caixa([0.0, 0.92, 1.95], [1.80, 0.08, 0.42]);
    corpo.caixa([-0.78, 0.80, 1.92], [0.10, 0.26, 0.16]);
    corpo.caixa([0.78, 0.80, 1.92], [0.10, 0.26, 0.16]);

    let mut cabine = Construtor::com_capacidade(80, 48);
    // Cabine recuada e estreita: é o que dá a leitura de "carro", não de "tijolo".
    cabine.caixa([0.0, 0.86, 0.10], [1.42, 0.44, 1.85]);
    cabine.caixa([0.0, 1.06, 0.60], [1.20, 0.30, 1.05]);

    let mut rodas = Construtor::com_capacidade(160, 96);
    for x in [-0.88f32, 0.88] {
        for z in [-1.42f32, 1.42] {
            rodas.caixa([x, 0.30, z], [0.30, 0.60, 0.60]);
        }
    }

    let mut farois = Construtor::com_capacidade(60, 36);
    farois.caixa([-0.62, 0.54, -2.14], [0.44, 0.16, 0.10]);
    farois.caixa([0.62, 0.54, -2.14], [0.44, 0.16, 0.10]);

    let mut lanternas = Construtor::com_capacidade(60, 36);
    lanternas.caixa([-0.66, 0.64, 2.14], [0.42, 0.14, 0.10]);
    lanternas.caixa([0.66, 0.64, 2.14], [0.42, 0.14, 0.10]);

    CarroMalhas {
        corpo: (corpo.vertices, corpo.indices),
        cabine: (cabine.vertices, cabine.indices),
        rodas: (rodas.vertices, rodas.indices),
        farois: (farois.vertices, farois.indices),
        lanternas: (lanternas.vertices, lanternas.indices),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn uma_caixa_tem_vinte_e_quatro_vertices() {
        let mut c = Construtor::new();
        c.caixa([0.0; 3], [1.0; 3]);
        assert_eq!(c.vertices().len(), 24);
        assert_eq!(c.indices().len(), 36);
    }

    #[test]
    fn os_indices_nunca_passam_do_numero_de_vertices() {
        // Um índice fora da faixa é lixo de memória na GPU: o quadro sai com
        // triângulos aleatórios e não há erro de GL.
        let mut c = Construtor::new();
        c.caixa([0.0; 3], [1.0; 3]);
        c.caixa([2.0, 0.0, 0.0], [1.0; 3]);
        c.quad(
            [[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            [0.0, 0.0, 1.0],
            1.0,
        );
        let n = c.vertices().len() as u32;
        for indice in c.indices() {
            assert!(*indice < n, "índice {indice} >= {n}");
        }
    }

    #[test]
    fn a_caixa_girada_respeita_o_centro_e_o_tamanho() {
        let mut c = Construtor::new();
        c.caixa_girada([10.0, 0.0, -5.0], [2.0, 1.0, 4.0], std::f32::consts::FRAC_PI_2);
        // Girada 90°, a extensão em X passa a ser a antiga em Z.
        let xs: Vec<f32> = c.vertices().iter().map(|v| v.position[0]).collect();
        let zs: Vec<f32> = c.vertices().iter().map(|v| v.position[2]).collect();
        let dx = xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min);
        let dz = zs.iter().cloned().fold(f32::MIN, f32::max)
            - zs.iter().cloned().fold(f32::MAX, f32::min);
        assert!((dx - 4.0).abs() < 0.01, "largura girada {dx}");
        assert!((dz - 2.0).abs() < 0.01, "profundidade girada {dz}");
    }

    #[test]
    fn o_carro_fica_dentro_da_caixa_de_uma_vaga() {
        // 4,4 m de comprimento e 1,9 de largura: se isso crescer, dois carros
        // passam a se sobrepor na largada e a colisão vira loteria.
        let m = carro();
        let mut maior_x = 0.0f32;
        let mut maior_z = 0.0f32;
        for (v, _) in [&m.corpo, &m.cabine, &m.rodas, &m.farois, &m.lanternas] {
            for vert in v {
                maior_x = maior_x.max(vert.position[0].abs());
                maior_z = maior_z.max(vert.position[2].abs());
            }
        }
        assert!(maior_x <= 1.05, "largura {:.2} m", maior_x * 2.0);
        assert!(maior_z <= 2.35, "comprimento {:.2} m", maior_z * 2.0);
    }

    #[test]
    fn as_pecas_do_carro_nao_ficam_vazias() {
        let m = carro();
        for (nome, (v, i)) in [
            ("corpo", &m.corpo),
            ("cabine", &m.cabine),
            ("rodas", &m.rodas),
            ("farois", &m.farois),
            ("lanternas", &m.lanternas),
        ] {
            assert!(!v.is_empty(), "{nome} sem vértices");
            assert!(!i.is_empty(), "{nome} sem índices");
        }
    }

    #[test]
    fn o_quad_do_hud_tem_quatro_vertices() {
        let (v, i) = quad();
        assert_eq!(v.len(), 4);
        assert_eq!(i.len(), 6);
    }

    #[test]
    fn o_chao_aponta_a_normal_para_cima() {
        let mut c = Construtor::new();
        c.chao([0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], 0.5);
        for v in c.vertices() {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
            assert!((v.position[1] - 0.5).abs() < 1e-6);
        }
    }
}
