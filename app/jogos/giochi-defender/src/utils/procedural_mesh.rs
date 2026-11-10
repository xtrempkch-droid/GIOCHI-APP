//! Malhas geradas em código — nada de arquivo de modelo.
//!
//! ## Por que procedural
//!
//! Um `.obj` de asteroide é um binário que precisa viajar no pacote, combinar com
//! a licença e existir na imagem do aparelho. Uma esfera distorcida por ruído
//! determinístico custa 40 linhas, não depende de arquivo e é **a mesma** em
//! qualquer máquina — o que permite testá-la por igualdade de bytes.
//!
//! ## Por que a malha volta como dados e não como `MeshHandle`
//!
//! O `create_mesh` precisa do `RenderContext3D`, que só existe com uma janela
//! aberta. Devolver vértices e índices deixa a geometria testável no CI, onde não
//! há GL — e o custo é uma linha no chamador.
//!
//! ## Normais por face, vértices duplicados
//!
//! O visual *low-poly* depende de cada triângulo ter a sua própria normal; com
//! normais compartilhadas entre faces a esfera fica lisa e o facetado desaparece.
//! Por isso a saída é **não indexada** (3 vértices por triângulo, índices triviais).
//! A solda existe só durante a subdivisão, para que a distorção não abra frestas:
//! dois triângulos que dividem um vértice recebem exatamente o mesmo deslocamento.

use crate::utils::rng::Sorteio;
use giochi_engine_3d::mesh::Vertex;
use std::collections::HashMap;

/// Uma malha pronta para [`create_mesh`](giochi_engine_3d::RenderContext3D::create_mesh).
#[derive(Debug, Clone, PartialEq)]
pub struct Malha {
    /// Vértices, três por triângulo.
    pub vertices: Vec<Vertex>,
    /// Índices, `0..vertices.len()`.
    pub indices: Vec<u32>,
}

impl Malha {
    /// Quantos triângulos a malha tem.
    pub fn triangulos(&self) -> usize {
        self.indices.len() / 3
    }

    /// Quantos vértices a malha tem.
    pub fn vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Varre os vértices acumulando `[min, max]` por eixo.
    ///
    /// Usado pelos testes e pelo diagnóstica de captura para provar que a malha
    /// cabe no volume que ela promete.
    pub fn limites(&self) -> ([f32; 3], [f32; 3]) {
        let mut minimo = [f32::INFINITY; 3];
        let mut maximo = [f32::NEG_INFINITY; 3];
        for vertice in &self.vertices {
            for eixo in 0..3 {
                minimo[eixo] = minimo[eixo].min(vertice.position[eixo]);
                maximo[eixo] = maximo[eixo].max(vertice.position[eixo]);
            }
        }
        (minimo, maximo)
    }
}

// ---------------------------------------------------------------------------
// Geometria: a malha é montada à mão e não depende da engine para ser gerada.
// ---------------------------------------------------------------------------

fn soma(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn diferenca(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn escala(a: [f32; 3], fator: f32) -> [f32; 3] {
    [a[0] * fator, a[1] * fator, a[2] * fator]
}

fn produto_vetorial(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn produto_escalar(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn comprimento(a: [f32; 3]) -> f32 {
    produto_escalar(a, a).sqrt()
}

/// Normaliza; devolve `[0, 1, 0]` para um vetor degenerado em vez de `NaN`.
///
/// Um `NaN` numa normal contamina a matriz de iluminação e a cena inteira fica
/// preta, com o erro aparecendo longe da causa.
fn normalizar(a: [f32; 3]) -> [f32; 3] {
    let tamanho = comprimento(a);
    if tamanho <= f32::EPSILON || !tamanho.is_finite() {
        return [0.0, 1.0, 0.0];
    }
    escala(a, 1.0 / tamanho)
}

/// Normal de um triângulo, pela regra da mão direita sobre a ordem dos vértices.
fn normal_da_face(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    normalizar(produto_vetorial(diferenca(b, a), diferenca(c, a)))
}

/// Ruído determinístico e contínuo sobre a direção unitária.
///
/// Soma de senos em vez de uma tabela de hash: é contínua por construção (não há
/// salto entre células), é a mesma em qualquer máquina — `sin` é determinístico e
/// não depende do estado global — e não precisa de semente. Devolve `-1.0..=1.0`.
///
/// Frequências escolhidas para dar uma silhueta de asteroide: a primeira dá as
/// protuberâncias grandes, a última o granulado.
fn ruido_dir(dir: [f32; 3]) -> f32 {
    let a = (dir[0] * 3.1 + dir[1] * 1.7 + dir[2] * 2.3).sin();
    let b = (dir[0] * 5.9 - dir[1] * 4.3 + dir[2] * 3.7).sin();
    let c = (dir[0] * 11.3 + dir[1] * 9.1 - dir[2] * 7.9).sin();
    0.55 * a + 0.30 * b + 0.15 * c
}

/// Costura de vértices durante a subdivisão.
///
/// A chave é a posição quantizada: dois triângulos que calculam o mesmo ponto
/// médio por caminhos diferentes (os dois lados de uma aresta) chegam à mesma
/// chave e recebem o mesmo índice. Sem isto a distorção aplicada depois abriria
/// uma fresta em cada aresta — o defeito clássico de esfera subdividida.
struct Costura {
    posicoes: Vec<[f32; 3]>,
    indice: HashMap<[i64; 3], u32>,
}

impl Costura {
    fn nova() -> Self {
        Self {
            posicoes: Vec::new(),
            indice: HashMap::new(),
        }
    }

    fn inserir(&mut self, posicao: [f32; 3]) -> u32 {
        const ESCALA: f32 = 1_000_000.0;
        let chave = [
            (posicao[0] * ESCALA).round() as i64,
            (posicao[1] * ESCALA).round() as i64,
            (posicao[2] * ESCALA).round() as i64,
        ];

        if let Some(&existente) = self.indice.get(&chave) {
            return existente;
        }

        let novo = self.posicoes.len() as u32;
        self.posicoes.push(posicao);
        self.indice.insert(chave, novo);
        novo
    }

    /// Ponto médio de uma aresta, projetado na esfera unitária.
    fn meio(&mut self, a: u32, b: u32) -> u32 {
        let pa = self.posicoes[a as usize];
        let pb = self.posicoes[b as usize];
        let medio = escala(soma(pa, pb), 0.5);
        let ponto = normalizar(medio);
        self.inserir(ponto)
    }
}

/// Vértices do icosaedro de referência, na esfera unitária.
///
/// Doze pontos com a proporção áurea: é o poliedro regular com mais faces que se
/// consegue com triângulos equiláteros, e 20 faces já leem como "asteroide".
fn icosaedro_vertices() -> Vec<[f32; 3]> {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let bruto = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    bruto.iter().map(|&p| normalizar(p)).collect()
}

/// As 20 faces, em sentido anti-horário visto de fora.
///
/// A ordem é a que faz a normal da face apontar para fora; invertê-la faz o
/// `cull_face` da engine sumir com a esfera inteira e a tela ficar vazia sem
/// nenhum erro. `as_normais_apontam_para_fora` é o teste que prende isto.
fn icosaedro_faces() -> Vec<[u32; 3]> {
    vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ]
}

/// Gera uma icosphere distorcida: o asteroide da arena.
///
/// `subdivisoes` = 0 dá as 20 faces do icosaedro; cada nível multiplica por 4
/// (1 → 80, 2 → 320). O valor usado no jogo é 1, que é o que lê como *low-poly*.
///
/// `amplitude` é a fração do raio que o ruído desloca, em `0.0..=0.5`. Zero dá
/// uma esfera perfeita — útil em teste, inútil como asteroide.
pub fn generate_icosphere(subdivisoes: u32, raio: f32, amplitude: f32) -> Malha {
    let raio = if raio > 0.0 && raio.is_finite() {
        raio
    } else {
        1.0
    };
    let amplitude = amplitude.clamp(0.0, 0.5);

    let mut costura = Costura::nova();
    for vertice in icosaedro_vertices() {
        costura.inserir(vertice);
    }
    let mut faces = icosaedro_faces();

    for _ in 0..subdivisoes.min(6) {
        let mut refinadas = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = costura.meio(a, b);
            let bc = costura.meio(b, c);
            let ca = costura.meio(c, a);
            refinadas.push([a, ab, ca]);
            refinadas.push([b, bc, ab]);
            refinadas.push([c, ca, bc]);
            refinadas.push([ab, bc, ca]);
        }
        faces = refinadas;
    }

    // A distorção é função da **direção**, não do índice: vértices costurados
    // recebem o mesmo deslocamento e a superfície continua fechada.
    let deslocadas: Vec<[f32; 3]> = costura
        .posicoes
        .iter()
        .map(|&ponto| {
            let dir = normalizar(ponto);
            let fator = 1.0 + amplitude * ruido_dir(dir);
            escala(dir, raio * fator)
        })
        .collect();

    emitir_faces(&deslocadas, &faces)
}

/// Cubo centrado na origem, com `tamanho` de aresta.
///
/// Seis faces, duas triângulos cada, normais para fora. Serve ao inimigo (bloco
/// 1b) e às estrelas do céu, que são cubos minúsculos e por isso leem como ponto
/// de qualquer ângulo — um quad deitado some quando visto de lado.
pub fn generate_cube(tamanho: f32) -> Malha {
    let metade = tamanho.abs().max(f32::EPSILON) / 2.0;
    let mut vertices = Vec::with_capacity(36);
    let mut indices = Vec::with_capacity(36);

    // Para cada face: normal, e dois eixos `u` e `v` tais que `u × v = normal`.
    // Montar assim garante o sentido anti-horário visto de fora para as seis
    // faces de uma vez, em vez de acertar seis listas à mão.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];

    for (normal, u, v) in faces {
        let centro = escala(normal, metade);
        let cantos = [
            soma(centro, soma(escala(u, -metade), escala(v, -metade))),
            soma(centro, soma(escala(u, metade), escala(v, -metade))),
            soma(centro, soma(escala(u, metade), escala(v, metade))),
            soma(centro, soma(escala(u, -metade), escala(v, metade))),
        ];

        for triangulo in [[0u32, 1, 2], [0, 2, 3]] {
            for indice in triangulo {
                indices.push(vertices.len() as u32);
                vertices.push(Vertex::flat(cantos[indice as usize], normal));
            }
        }
    }

    Malha { vertices, indices }
}

/// Quadrilátero unitário de 1×1 no plano XY, centrado na origem, de frente para
/// `+Z`.
///
/// É a geometria de **todas** as instâncias da HUD: uma célula de glifo, um
/// marcador de vida e uma faixa do clarão são o mesmo quad com matrizes
/// diferentes. Ter um quad só é o que faz o texto inteiro sair em uma chamada.
///
/// O sentido dos cantos é anti-horário visto de `+Z`, que é o lado para onde a
/// câmera do HUD olha — o contrário faria tudo sumir com o descarte de faces
/// ligado, e sem erro nenhum (foi assim que o anel da mira desapareceu).
pub fn quad_unitario() -> Malha {
    const NORMAL: [f32; 3] = [0.0, 0.0, 1.0];
    let cantos = [
        [-0.5, -0.5, 0.0],
        [0.5, -0.5, 0.0],
        [0.5, 0.5, 0.0],
        [-0.5, 0.5, 0.0],
    ];
    let mut vertices = Vec::with_capacity(6);
    let mut indices = Vec::with_capacity(6);
    for triangulo in [[0u32, 1, 2], [0, 2, 3]] {
        for indice in triangulo {
            indices.push(vertices.len() as u32);
            vertices.push(Vertex::flat(cantos[indice as usize], NORMAL));
        }
    }
    Malha { vertices, indices }
}

/// Campo de estrelas: `quantidade` cubos minúsculos numa casca esférica.
///
/// Uma malha só, e não um `MeshHandle` por estrela: 500 desenhos custariam 500
/// chamadas por quadro, que é justamente o que o envelope de performance deste
/// projeto não permite. Assar as estrelas numa malha dá **uma** chamada.
///
/// `semente` fixa o sorteio; a mesma semente produz a mesma malha byte a byte.
pub fn generate_starfield(quantidade: u32, raio: f32, tamanho: f32, semente: u64) -> Malha {
    let raio = if raio > 0.0 && raio.is_finite() {
        raio
    } else {
        1.0
    };
    let tamanho = if tamanho > 0.0 && tamanho.is_finite() {
        tamanho
    } else {
        0.4
    };

    let mut sorteio = Sorteio::new(semente);
    let mut vertices: Vec<Vertex> = Vec::with_capacity(quantidade as usize * 36);

    for _ in 0..quantidade {
        // Direção uniforme na esfera: `z` uniforme e ângulo uniforme é o método
        // sem rejeição e sem concentrar estrelas nos polos.
        let direcao = sorteio.direcao_unitaria();

        // Casca com espessura: sem isto o céu fica uma casca perfeita e o olho
        // percebe o padrão quando a câmera orbita.
        let distancia = raio * sorteio.faixa(0.88, 1.12);
        let centro = escala(direcao, distancia);
        let aresta = tamanho * sorteio.faixa(0.55, 1.45);

        let cubo = generate_cube(aresta);
        for vertice in cubo.vertices {
            vertices.push(Vertex::flat(soma(vertice.position, centro), vertice.normal));
        }
    }

    // Malha não indexada: o índice de cada vértice é a sua própria posição.
    let indices: Vec<u32> = (0..vertices.len() as u32).collect();

    Malha { vertices, indices }
}

/// Emite cada triângulo com três vértices próprios e a normal da face.
fn emitir_faces(posicoes: &[[f32; 3]], faces: &[[u32; 3]]) -> Malha {
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    let mut indices = Vec::with_capacity(faces.len() * 3);

    for &[a, b, c] in faces {
        let pa = posicoes[a as usize];
        let pb = posicoes[b as usize];
        let pc = posicoes[c as usize];
        let normal = normal_da_face(pa, pb, pc);

        for ponto in [pa, pb, pc] {
            indices.push(vertices.len() as u32);
            vertices.push(Vertex::flat(ponto, normal));
        }
    }

    Malha { vertices, indices }
}

// ---------------------------------------------------------------------------
// Composição: caixas em posição arbitrária e junção de malhas.
//
// Os três mapas do jogo são feitos de peças — anel, painel, cratera, mastro — e
// cada peça é uma caixa. Montar cada uma com `generate_cube` e depois transladar
// exigiria uma transformação por vértice no chamador; `caixa` faz isso uma vez e
// `juntar` transforma N peças em **uma** chamada de desenho.
// ---------------------------------------------------------------------------

/// Caixa alinhada aos eixos, centrada em `centro`, com meia-extensão por eixo.
///
/// `generate_cube` só produz cubos centrados na origem; os mapas precisam de
/// placas, mastros e anéis, que são caixas achatadas em posições diferentes.
pub fn caixa(centro: [f32; 3], metade: [f32; 3]) -> Malha {
    let m = [
        metade[0].abs().max(f32::EPSILON),
        metade[1].abs().max(f32::EPSILON),
        metade[2].abs().max(f32::EPSILON),
    ];
    let mut vertices = Vec::with_capacity(36);
    let mut indices = Vec::with_capacity(36);

    // Mesma construção do cubo: para cada face, dois eixos `u` e `v` tais que
    // `u × v = normal`, o que dá o sentido anti-horário visto de fora de uma vez.
    let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
    ];

    for (normal, u, v) in faces {
        let deslocamento = [normal[0] * m[0], normal[1] * m[1], normal[2] * m[2]];
        let cantos = [
            [
                centro[0] + deslocamento[0] - u[0] * m[0] - v[0] * m[1],
                centro[1] + deslocamento[1] - u[1] * m[0] - v[1] * m[1],
                centro[2] + deslocamento[2] - u[2] * m[0] - v[2] * m[1],
            ],
            [
                centro[0] + deslocamento[0] + u[0] * m[0] - v[0] * m[1],
                centro[1] + deslocamento[1] + u[1] * m[0] - v[1] * m[1],
                centro[2] + deslocamento[2] + u[2] * m[0] - v[2] * m[1],
            ],
            [
                centro[0] + deslocamento[0] + u[0] * m[0] + v[0] * m[1],
                centro[1] + deslocamento[1] + u[1] * m[0] + v[1] * m[1],
                centro[2] + deslocamento[2] + u[2] * m[0] + v[2] * m[1],
            ],
            [
                centro[0] + deslocamento[0] - u[0] * m[0] + v[0] * m[1],
                centro[1] + deslocamento[1] - u[1] * m[0] + v[1] * m[1],
                centro[2] + deslocamento[2] - u[2] * m[0] + v[2] * m[1],
            ],
        ];
        for triangulo in [[0u32, 1, 2], [0, 2, 3]] {
            for indice in triangulo {
                indices.push(vertices.len() as u32);
                vertices.push(Vertex::flat(cantos[indice as usize], normal));
            }
        }
    }

    Malha { vertices, indices }
}

/// Translada todos os vértices. As normais não mudam.
pub fn transladar(malha: &Malha, deslocamento: [f32; 3]) -> Malha {
    let mut saida = malha.clone();
    for vertice in &mut saida.vertices {
        for (posicao, delta) in vertice.position.iter_mut().zip(deslocamento) {
            *posicao += delta;
        }
    }
    saida
}

/// Estica a malha por eixo e **recalcula as normais**.
///
/// Recalcular não é zelo: com escala não uniforme (achatar uma esfera para virar
/// uma cratera, por exemplo) a normal antiga deixa de ser perpendicular à face, e
/// o termo difuso do `phong` ilumina o lado errado — o objeto fica com uma
/// mancha clara onde devia estar escuro, sem nenhum erro.
pub fn esticar(malha: &Malha, fator: [f32; 3]) -> Malha {
    let mut saida = malha.clone();
    for vertice in &mut saida.vertices {
        for (posicao, escala) in vertice.position.iter_mut().zip(fator) {
            *posicao *= escala;
        }
    }
    for triangulo in saida.indices.chunks_exact(3) {
        let [a, b, c] = [
            triangulo[0] as usize,
            triangulo[1] as usize,
            triangulo[2] as usize,
        ];
        let normal = normal_da_face(
            saida.vertices[a].position,
            saida.vertices[b].position,
            saida.vertices[c].position,
        );
        for indice in [a, b, c] {
            saida.vertices[indice].normal = normal;
        }
    }
    saida
}

/// Junta várias malhas numa só, deslocando os índices.
///
/// É o que mantém a regra do projeto: **cenário novo não cria chamada nova**.
/// Um anel de 24 caixas desenhado caixa por caixa custaria 24 chamadas; junto,
/// custa uma — e cada peça continua com a normal da sua face.
pub fn juntar(partes: &[Malha]) -> Malha {
    let total_vertices: usize = partes.iter().map(|m| m.vertices.len()).sum();
    let total_indices: usize = partes.iter().map(|m| m.indices.len()).sum();
    let mut vertices = Vec::with_capacity(total_vertices);
    let mut indices = Vec::with_capacity(total_indices);
    for parte in partes {
        let base = vertices.len() as u32;
        vertices.extend_from_slice(&parte.vertices);
        indices.extend(parte.indices.iter().map(|i| i + base));
    }
    Malha { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distancia_ao_centro(posicao: [f32; 3]) -> f32 {
        comprimento(posicao)
    }

    #[test]
    fn a_subdivisao_quadruplica_as_faces() {
        // 20 → 80 → 320 é a assinatura da subdivisão por ponto médio. Um laço que
        // esquecesse de dividir uma das quatro partes daria 60, e o erro passaria
        // como "um pouco menos detalhado".
        assert_eq!(generate_icosphere(0, 2.0, 0.0).triangulos(), 20);
        assert_eq!(generate_icosphere(1, 2.0, 0.0).triangulos(), 80);
        assert_eq!(generate_icosphere(2, 2.0, 0.0).triangulos(), 320);
        assert_eq!(generate_icosphere(3, 2.0, 0.0).triangulos(), 1280);
    }

    #[test]
    fn cada_triangulo_tem_tres_vertices_proprios() {
        let malha = generate_icosphere(1, 2.0, 0.2);
        assert_eq!(malha.vertices(), malha.triangulos() * 3);
        assert_eq!(malha.indices.len(), malha.vertices.len());
    }

    #[test]
    fn sem_amplitude_a_esfera_e_perfeita() {
        let malha = generate_icosphere(2, 3.0, 0.0);
        for vertice in &malha.vertices {
            let raio = distancia_ao_centro(vertice.position);
            assert!(
                (raio - 3.0).abs() < 1e-4,
                "raio {raio} devia ser 3.0 com amplitude zero"
            );
        }
    }

    #[test]
    fn a_distorcao_cabe_no_intervalo_prometido() {
        // O asteroide tem de caber na esfera de colisão que o bloco 1b vai usar,
        // e não pode encolher até sumir. A amplitude é o contrato.
        let raio = 2.0;
        let amplitude = 0.22;
        let malha = generate_icosphere(1, raio, amplitude);
        for vertice in &malha.vertices {
            let r = distancia_ao_centro(vertice.position);
            assert!(
                r >= raio * (1.0 - amplitude) - 1e-3 && r <= raio * (1.0 + amplitude) + 1e-3,
                "vértice a {r} do centro, fora de [{}; {}]",
                raio * (1.0 - amplitude),
                raio * (1.0 + amplitude)
            );
        }
    }

    #[test]
    fn as_normais_sao_unitarias() {
        for malha in [
            generate_icosphere(1, 2.0, 0.22),
            generate_cube(1.0),
            generate_starfield(8, 100.0, 0.5, 7),
        ] {
            for vertice in &malha.vertices {
                let tamanho = comprimento(vertice.normal);
                assert!(
                    (tamanho - 1.0).abs() < 1e-3,
                    "normal de tamanho {tamanho} em {malha:?}"
                );
            }
        }
    }

    fn as_normais_apontam_para_fora(malha: &Malha) {
        for triangulo in malha.vertices.chunks_exact(3) {
            let centro = escala(
                soma(
                    soma(triangulo[0].position, triangulo[1].position),
                    triangulo[2].position,
                ),
                1.0 / 3.0,
            );
            let normal = triangulo[0].normal;
            assert!(
                produto_escalar(normal, centro) > 0.0,
                "face em {centro:?} tem normal {normal:?} apontando para dentro — \
                 com `cull_face` ligado ela sumiria da tela"
            );
        }
    }

    #[test]
    fn as_normais_do_asteroide_apontam_para_fora() {
        as_normais_apontam_para_fora(&generate_icosphere(1, 2.0, 0.22));
    }

    #[test]
    fn as_normais_do_cubo_apontam_para_fora() {
        as_normais_apontam_para_fora(&generate_cube(1.0));
    }

    #[test]
    fn o_cubo_tem_as_seis_faces_no_tamanho_pedido() {
        let malha = generate_cube(2.0);
        assert_eq!(malha.triangulos(), 12);
        let (minimo, maximo) = malha.limites();
        for eixo in 0..3 {
            assert!((minimo[eixo] + 1.0).abs() < 1e-5, "{minimo:?}");
            assert!((maximo[eixo] - 1.0).abs() < 1e-5, "{maximo:?}");
        }
    }

    #[test]
    fn a_superficie_e_fechada_sem_frestas() {
        // Toda aresta de uma malha fechada pertence a exatamente dois triângulos.
        // Se a costura falhasse, a aresta apareceria uma vez só e a distorção
        // abriria uma fresta — invisível num quadro parado, óbvia em movimento.
        let malha = generate_icosphere(1, 2.0, 0.22);
        let mut arestas: HashMap<[[i64; 3]; 2], u32> = HashMap::new();
        let chave = |p: [f32; 3]| {
            const E: f32 = 100_000.0;
            [
                (p[0] * E).round() as i64,
                (p[1] * E).round() as i64,
                (p[2] * E).round() as i64,
            ]
        };

        for triangulo in malha.vertices.chunks_exact(3) {
            let a = chave(triangulo[0].position);
            let b = chave(triangulo[1].position);
            let c = chave(triangulo[2].position);
            for (p, q) in [(a, b), (b, c), (c, a)] {
                let ordenada = if p <= q { [p, q] } else { [q, p] };
                *arestas.entry(ordenada).or_insert(0) += 1;
            }
        }

        for (aresta, vezes) in &arestas {
            assert_eq!(
                *vezes, 2,
                "aresta {aresta:?} aparece {vezes} vez(es); a malha está aberta"
            );
        }
        // Um icosaedro subdividido uma vez tem 30 arestas de base × 4 = 120.
        assert_eq!(arestas.len(), 120);
    }

    #[test]
    fn a_geracao_e_deterministica() {
        // Sem isto, comparar duas capturas de quadro deixaria de valer, e o número
        // que diz "o desenho não mudou" mediria outro asteroide.
        assert_eq!(
            generate_icosphere(2, 2.0, 0.22),
            generate_icosphere(2, 2.0, 0.22)
        );
        assert_eq!(
            generate_starfield(64, 100.0, 0.5, 12345),
            generate_starfield(64, 100.0, 0.5, 12345)
        );
    }

    #[test]
    fn sementes_diferentes_sorteiam_ceus_diferentes() {
        assert_ne!(
            generate_starfield(16, 100.0, 0.5, 1),
            generate_starfield(16, 100.0, 0.5, 2)
        );
    }

    #[test]
    fn o_campo_de_estrelas_fica_na_casca_pedida() {
        let raio = 100.0;
        let malha = generate_starfield(40, raio, 0.5, 99);
        assert_eq!(malha.triangulos(), 40 * 12);
        for vertice in &malha.vertices {
            // O centro de cada cubo está na casca; o vértice fica a até meia
            // aresta de distância dele, e a aresta máxima é 0.5 * 1.45.
            let r = distancia_ao_centro(vertice.position);
            assert!(
                r > raio * 0.88 - 1.0 && r < raio * 1.12 + 1.0,
                "estrela a {r} do centro, fora da casca"
            );
        }
    }

    #[test]
    fn o_sorteio_cobre_o_intervalo_pedido() {
        let mut sorteio = Sorteio::new(7);
        let mut minimo = f32::INFINITY;
        let mut maximo = f32::NEG_INFINITY;
        for _ in 0..10_000 {
            let valor = sorteio.faixa(-1.0, 1.0);
            minimo = minimo.min(valor);
            maximo = maximo.max(valor);
        }
        assert!(minimo < -0.99, "mínimo {minimo}");
        assert!(maximo > 0.99, "máximo {maximo}");
    }

    #[test]
    fn raio_invalido_nao_produz_nan() {
        // Um `raio` zero viria de configuração errada; devolver uma malha
        // degenerada é melhor que espalhar `NaN` pela matriz de modelo.
        let malha = generate_icosphere(1, 0.0, 0.2);
        for vertice in &malha.vertices {
            for eixo in 0..3 {
                assert!(
                    vertice.position[eixo].is_finite(),
                    "posição não finita: {:?}",
                    vertice.position
                );
            }
        }
    }
}
