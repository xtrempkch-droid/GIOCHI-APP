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

    /// Metros de mundo por repetição de textura.
    ///
    /// Fixar isto em **um** número é o que faz a textura ter a mesma densidade no
    /// asfalto de 640×480 e no de 1280×720: sem ele, a mesma parede de 8 m
    /// mostraria 8 tijolos num painel e 8 tijolos no outro, mas cada tijolo com
    /// tamanho diferente em tela.
    pub const METROS_POR_TILE: f32 = 3.0;

    /// Quadrilátero com **altura por canto**, para o chão de uma pista com
    /// relevo.
    ///
    /// `chao` recebe um `y` só, e numa descida o vão entre duas amostras
    /// atravessa o terreno: os cantos vêm em alturas diferentes. O UV sai da
    /// posição no mundo dividida pelo tile, então a textura acompanha a curva em
    /// vez de esticar.
    pub fn chao3(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        c: [f32; 2],
        d: [f32; 2],
        ya: f32,
        yb: f32,
        yc: f32,
        yd: f32,
        tile: f32,
    ) -> &mut Self {
        // UV a partir da **posição no mundo**: é o que faz a textura continuar de
        // um quadrilátero para o seguinte em vez de reiniciar em cada um. Com UV
        // 0..1 por quad, o asfalto de 962 m mostraria 481 repetições da textura
        // inteira, uma por segmento, e a emenda apareceria como faixa.
        let tile = tile.max(0.25);
        let base = self.vertices.len() as u32;
        let cantos = [
            [a[0], ya, a[1]],
            [b[0], yb, b[1]],
            [c[0], yc, c[1]],
            [d[0], yd, d[1]],
        ];
        for canto in cantos.iter() {
            self.vertices.push(Vertex::new(
                *canto,
                [0.0, 1.0, 0.0],
                [canto[0] / tile, canto[2] / tile],
            ));
        }
        // Sentido pela normal, como o `quad`: a ordem dos cantos não importa.
        let u = [cantos[1][0] - cantos[0][0], 0.0, cantos[1][2] - cantos[0][2]];
        let v = [cantos[2][0] - cantos[0][0], 0.0, cantos[2][2] - cantos[0][2]];
        let cruz_y = u[2] * v[0] - u[0] * v[2];
        if cruz_y >= 0.0 {
            self.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.indices
                .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
        }
        self
    }

    /// Caixa com UV **proporcional ao tamanho de cada face**.
    ///
    /// `caixa_girada` usa o cubo unitário, cujas faces têm UV 0..1. Numa caixa de
    /// 30 m de comprimento isso significa uma única repetição da textura esticada
    /// por 30 m — o tijolo vira um borrão de 3 m. Aqui cada face recebe
    /// `largura/tile` por `altura/tile`, então a densidade é a mesma em qualquer
    /// tamanho.
    ///
    /// As 6 faces são emitidas separadamente porque cada uma precisa da sua
    /// própria contagem de repetições.
    pub fn caixa_texturizada(
        &mut self,
        centro: [f32; 3],
        tamanho: [f32; 3],
        giro: f32,
        tile: f32,
    ) -> &mut Self {
        let tile = tile.max(0.25);
        let (sen, cos) = giro.sin_cos();
        let (hx, hy, hz) = (tamanho[0] * 0.5, tamanho[1] * 0.5, tamanho[2] * 0.5);
        let uv = |eixo_u: f32, eixo_v: f32| -> [f32; 2] {
            [
                (eixo_u / tile).max(0.01),
                (eixo_v / tile).max(0.01),
            ]
        };
        // Normal e quatro cantos de cada face, no espaço local da caixa.
        // Ordem dos cantos: baixo-esquerda, baixo-direita, cima-direita,
        // cima-esquerda — o `quad` corrige o sentido pela normal.
        let faces: [([f32; 3], [[f32; 3]; 4], [f32; 2]); 6] = [
            (
                [0.0, 0.0, 1.0],
                [
                    [-hx, -hy, hz],
                    [hx, -hy, hz],
                    [hx, hy, hz],
                    [-hx, hy, hz],
                ],
                uv(tamanho[0], tamanho[1]),
            ),
            (
                [0.0, 0.0, -1.0],
                [
                    [hx, -hy, -hz],
                    [-hx, -hy, -hz],
                    [-hx, hy, -hz],
                    [hx, hy, -hz],
                ],
                uv(tamanho[0], tamanho[1]),
            ),
            (
                [1.0, 0.0, 0.0],
                [
                    [hx, -hy, hz],
                    [hx, -hy, -hz],
                    [hx, hy, -hz],
                    [hx, hy, hz],
                ],
                uv(tamanho[2], tamanho[1]),
            ),
            (
                [-1.0, 0.0, 0.0],
                [
                    [-hx, -hy, -hz],
                    [-hx, -hy, hz],
                    [-hx, hy, hz],
                    [-hx, hy, -hz],
                ],
                uv(tamanho[2], tamanho[1]),
            ),
            (
                [0.0, 1.0, 0.0],
                [
                    [-hx, hy, hz],
                    [hx, hy, hz],
                    [hx, hy, -hz],
                    [-hx, hy, -hz],
                ],
                uv(tamanho[0], tamanho[2]),
            ),
            (
                [0.0, -1.0, 0.0],
                [
                    [-hx, -hy, -hz],
                    [hx, -hy, -hz],
                    [hx, -hy, hz],
                    [-hx, -hy, hz],
                ],
                uv(tamanho[0], tamanho[2]),
            ),
        ];

        let girar = |p: [f32; 3]| -> [f32; 3] {
            [
                p[0] * cos + p[2] * sen + centro[0],
                p[1] + centro[1],
                -p[0] * sen + p[2] * cos + centro[2],
            ]
        };
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (normal, cantos, escala_uv) in faces {
            let n = [normal[0] * cos + normal[2] * sen, normal[1], -normal[0] * sen + normal[2] * cos];
            let base = self.vertices.len() as u32;
            for (canto, uv) in cantos.iter().zip(uvs.iter()) {
                self.vertices.push(Vertex::new(
                    girar(*canto),
                    n,
                    [uv[0] * escala_uv[0], uv[1] * escala_uv[1]],
                ));
            }
            // Sentido deduzido pela normal, como no `quad`: sem isto a caixa
            // texturizada some com `cull_face` ligado, e "some" é o único aviso.
            let g = [girar(cantos[0]), girar(cantos[1]), girar(cantos[2])];
            let u = [g[1][0] - g[0][0], g[1][1] - g[0][1], g[1][2] - g[0][2]];
            let v = [g[2][0] - g[0][0], g[2][1] - g[0][1], g[2][2] - g[0][2]];
            let cruz = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let concordam = cruz[0] * n[0] + cruz[1] * n[1] + cruz[2] * n[2] >= 0.0;
            if concordam {
                self.indices
                    .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                self.indices
                    .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
            }
        }
        self
    }

    /// Parede em **tira**: liga duas amostras consecutivas da pista.
    ///
    /// O paredão da serra era uma caixa de 19 m colocada a cada duas amostras
    /// (~8 m). Parecia contínuo na reta e **cortava a curva por dentro**: numa
    /// curva fechada de raio 20 m a corda de 19 m entra 2,2 m para dentro do
    /// asfalto, a câmera ficava dentro do bloco e o quadro virava uma parede
    /// de pedra sem pista nenhuma. A tira usa os dois centros reais, então
    /// acompanha o traçado por construção.
    ///
    /// Sai com as **duas faces**. Uma só economiza metade dos triângulos e
    /// aposta que a normal está do lado certo; quando não está, a parede some
    /// e o único sintoma é o buraco no horizonte.
    pub fn parede3(
        &mut self,
        a: [f32; 2],
        b: [f32; 2],
        ya: f32,
        yb: f32,
        altura: f32,
        normal: [f32; 2],
        tile: f32,
    ) -> &mut Self {
        let tile = tile.max(0.25);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let comp = (dx * dx + dz * dz).sqrt().max(0.001);
        let (ux, uz) = (dx / comp, dz / comp);
        let cantos = [
            [a[0], ya, a[1]],
            [b[0], yb, b[1]],
            [b[0], yb + altura, b[1]],
            [a[0], ya + altura, a[1]],
        ];
        let uvs: [[f32; 2]; 4] = [
            [(cantos[0][0] * ux + cantos[0][2] * uz) / tile, cantos[0][1] / tile],
            [(cantos[1][0] * ux + cantos[1][2] * uz) / tile, cantos[1][1] / tile],
            [(cantos[2][0] * ux + cantos[2][2] * uz) / tile, cantos[2][1] / tile],
            [(cantos[3][0] * ux + cantos[3][2] * uz) / tile, cantos[3][1] / tile],
        ];
        // A normal que a ordem `a_baixo, b_baixo, b_cima, a_cima` produz é
        // `(-uz, 0, ux)` — o produto vetorial de uma parede vertical não tem
        // componente y, então o teste do `quad` não serve aqui. Escolhido o
        // sentido, as duas faces saem com a ordem certa.
        let esquerda = (-uz, ux);
        for lado in [1.0f32, -1.0] {
            let n = [normal[0] * lado, 0.0, normal[1] * lado];
            let base = self.vertices.len() as u32;
            for (canto, uv) in cantos.iter().zip(uvs.iter()) {
                self.vertices.push(Vertex::new(*canto, n, *uv));
            }
            let concorda = esquerda.0 * n[0] + esquerda.1 * n[2] >= 0.0;
            if concorda {
                self.indices
                    .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                self.indices
                    .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
            }
        }
        self
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
        c.chao3(
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            0.5,
            0.5,
            0.5,
            0.5,
            1.0,
        );
        for v in c.vertices() {
            assert_eq!(v.normal, [0.0, 1.0, 0.0]);
            assert!((v.position[1] - 0.5).abs() < 1e-6);
        }
    }
}

#[cfg(test)]
mod testes_textura {
    use super::*;

    #[test]
    fn a_caixa_texturizada_tem_seis_faces_e_uv_proporcional() {
        let mut c = Construtor::new();
        c.caixa_texturizada([0.0, 0.0, 0.0], [6.0, 3.0, 9.0], 0.0, 3.0);
        // 6 faces × 4 vértices, 6 faces × 6 índices.
        assert_eq!(c.vertices().len(), 24);
        assert_eq!(c.indices().len(), 36);
        // A face de cima de uma caixa 6×3×9 com tile 3 tem de repetir 2× e 3×.
        let mut maior_u: f32 = 0.0;
        let mut maior_v: f32 = 0.0;
        for v in c.vertices() {
            maior_u = maior_u.max(v.uv[0]);
            maior_v = maior_v.max(v.uv[1]);
        }
        assert!((maior_u - 3.0).abs() < 1e-3, "u máximo {maior_u}");
        assert!((maior_v - 3.0).abs() < 1e-3, "v máximo {maior_v}");
    }

    #[test]
    fn a_caixa_texturizada_nao_some_com_descarte_de_faces() {
        // O `quad` deduz o sentido pela normal; a caixa tem de fazer o mesmo,
        // senão metade das faces desaparece e o sintoma é "a parede tem um
        // buraco" sem erro nenhum.
        let mut c = Construtor::new();
        c.caixa_texturizada([0.0, 0.0, 0.0], [4.0, 4.0, 4.0], 0.7, 2.0);
        let v = c.vertices();
        let i = c.indices();
        for tri in i.chunks(3) {
            let a = v[tri[0] as usize];
            let b = v[tri[1] as usize];
            let cc = v[tri[2] as usize];
            let u = [
                b.position[0] - a.position[0],
                b.position[1] - a.position[1],
                b.position[2] - a.position[2],
            ];
            let w = [
                cc.position[0] - a.position[0],
                cc.position[1] - a.position[1],
                cc.position[2] - a.position[2],
            ];
            let cruz = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            let produto = cruz[0] * a.normal[0] + cruz[1] * a.normal[1] + cruz[2] * a.normal[2];
            assert!(produto >= 0.0, "triângulo com a normal ao contrário");
        }
    }

    #[test]
    fn o_chao_com_alturas_inclina_o_quadrilatero() {
        let mut c = Construtor::new();
        c.chao3([0.0, 0.0], [0.0, 10.0], [4.0, 10.0], [4.0, 0.0], 8.0, 4.0, 4.0, 8.0, 3.0);
        let alturas: Vec<f32> = c.vertices().iter().map(|v| v.position[1]).collect();
        assert_eq!(alturas, vec![8.0, 4.0, 4.0, 8.0]);
        // E a normal continua para cima, senão o chão some com `cull_face`.
        assert!(c.vertices()[0].normal[1] > 0.9);
        // UV vem do mundo: um canto em (4, 0) com tile 3 dá u = 4/3.
        assert!((c.vertices()[3].uv[0] - 4.0 / 3.0).abs() < 1e-4);
        assert!((c.vertices()[1].uv[1] - 10.0 / 3.0).abs() < 1e-4);
    }

    // ── parede em tira ───────────────────────────────────────
    // O paredão da serra era uma caixa de 19 m a cada duas amostras. Na curva
    // fechada (raio ~20 m) a corda entrava 2,2 m no asfalto, a câmera ficava
    // dentro do bloco e o quadro virava pedra sem pista. O teste mede isso:
    // numa curva, o ponto médio da tira tem de estar **na** curva.

    #[test]
    fn a_parede_segue_a_curva_em_vez_de_cortar_por_dentro() {
        // Trecho de círculo de raio 20 m, 19 m de arco — o mesmo vão que a caixa
        // antiga cobria.
        let raio = 20.0f32;
        let passo = 19.0 / raio;
        let a = [raio, 0.0];
        let b = [raio * passo.cos(), raio * passo.sin()];
        let mut c = Construtor::new();
        c.parede3(a, b, 0.0, 0.0, 6.0, [-1.0, 0.0], 3.0);
        let v = c.vertices();
        // Vértices 0 e 1 são a base do primeiro lado: são exatamente as duas
        // amostras da pista, sem encurtar.
        assert!((v[0].position[0] - a[0]).abs() < 1e-4 && (v[0].position[2] - a[1]).abs() < 1e-4);
        assert!((v[1].position[0] - b[0]).abs() < 1e-4 && (v[1].position[2] - b[1]).abs() < 1e-4);
        // O ponto médio da corda fica a 19,4 m do centro; na curva, a 20. A
        // diferença é o que a caixa comia do asfalto.
        let mx = (a[0] + b[0]) * 0.5;
        let mz = (a[1] + b[1]) * 0.5;
        let corda = (mx * mx + mz * mz).sqrt();
        assert!(
            raio - corda > 1.0,
            "o teste não exercita a curva: a corda mede {corda:.2} de {raio:.2}"
        );
    }

    #[test]
    fn a_parede_sai_com_as_duas_faces() {
        // Uma face só economiza metade dos triângulos e aposta que a normal está
        // do lado certo; quando não está, a parede some e o sintoma é o buraco.
        let mut c = Construtor::new();
        c.parede3([0.0, 0.0], [0.0, 10.0], 0.0, 0.0, 4.0, [1.0, 0.0], 3.0);
        assert_eq!(c.vertices().len(), 8);
        assert_eq!(c.indices().len(), 12);
        let n0 = c.vertices()[0].normal;
        let n1 = c.vertices()[4].normal;
        assert!((n0[0] + n1[0]).abs() < 1e-5, "as duas faces olham para o mesmo lado");
        // E cada triângulo concorda com a sua normal, senão some com `cull_face`.
        let v = c.vertices();
        for tri in c.indices().chunks(3) {
            let a = v[tri[0] as usize];
            let b = v[tri[1] as usize];
            let d = v[tri[2] as usize];
            let u = [b.position[0] - a.position[0], b.position[1] - a.position[1], b.position[2] - a.position[2]];
            let w = [d.position[0] - a.position[0], d.position[1] - a.position[1], d.position[2] - a.position[2]];
            let cruz = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            let produto = cruz[0] * a.normal[0] + cruz[1] * a.normal[1] + cruz[2] * a.normal[2];
            assert!(produto >= 0.0, "face da parede com a normal ao contrário");
        }
    }

    #[test]
    fn a_parede_repete_a_textura_por_metro_e_nao_por_segmento() {
        // UV 0..1 por segmento faria a textura reiniciar a cada 4 m de serra e a
        // emenda apareceria como faixa vertical correndo ao lado do carro.
        let mut c = Construtor::new();
        c.parede3([0.0, 0.0], [12.0, 0.0], 0.0, 0.0, 5.0, [0.0, 1.0], 3.0);
        let v = c.vertices();
        assert!((v[1].uv[0] - 4.0).abs() < 1e-4, "u em b = {}", v[1].uv[0]);
        assert!((v[2].uv[1] - 5.0 / 3.0).abs() < 1e-4, "v no topo = {}", v[2].uv[1]);
    }
}
