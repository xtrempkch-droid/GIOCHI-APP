//! A mira: geometria da retícula em espaço de tela.
//!
//! ## O desenho vem do ícone que o dono mandou
//!
//! Anéis concêntricos finos, com quatro traços externos, monocromático e
//! antialiasing por alfa — a linguagem visual de uma retícula de tiro. A
//! diferença é que aqui ela é **geometria de triângulos**, não um PNG: o
//! `SDL2_image` do aparelho não decodifica PNG (AGENTS.md §24), e uma mira de
//! 24 px não justifica carregar decodificador.
//!
//! ## Como a tela vira mundo
//!
//! A engine 3D só desenha malha. A ponte usada é a mesma do `neon-drift`: uma
//! câmera na origem olhando para `-Z` com `fov` de 90° e o HUD a
//! [`PROFUNDIDADE`] unidades. Nessa configuração, um ponto a `z = -PROFUNDIDADE`
//! tem `y = ndc_y · PROFUNDIDADE` e `x = ndc_x · PROFUNDIDADE · aspecto`, o que
//! dá a conversão exata de pixel para mundo:
//!
//! ```text
//! mundo_x = (px - largura/2)  · 2·PROFUNDIDADE / altura
//! mundo_y = (altura/2 - py)   · 2·PROFUNDIDADE / altura
//! ```
//!
//! O mesmo divisor (a **altura**) nos dois eixos é o que mantém a mira redonda
//! num painel 4:3 e num 16:9: a escala é uniforme em pixels, e o aspecto entra
//! só na câmera.
//!
//! ## Um desenho para os dois estados
//!
//! A mira normal e a do "acertou" são a mesma malha, com cor e escala
//! diferentes. Duas malhas seriam duas verdades sobre a mesma forma, e a
//! segunda envelheceria sozinha.

use crate::utils::procedural_mesh::Malha;
use giochi_engine_3d::mesh::Vertex;

/// Distância do plano do HUD, em unidades de mundo.
///
/// A câmera do HUD usa `fov` de 90° e planos 9..4000 (ver [`super::camera`]),
/// então o HUD fica com profundidade normalizada ~0,10 — à frente de qualquer
/// coisa do mundo, que começa em ~0,87. Sem isso a mira sumiria atrás do
/// asteroide.
pub const PROFUNDIDADE: f32 = 10.0;

/// Raio do anel externo, em pixels.
pub const RAIO_EXTERNO: f32 = 13.0;
/// Espessura do anel externo, em pixels.
pub const ESPESSURA_EXTERNA: f32 = 1.6;
/// Raio do anel interno, em pixels.
pub const RAIO_INTERNO: f32 = 5.0;
/// Espessura do anel interno, em pixels.
pub const ESPESSURA_INTERNA: f32 = 1.2;
/// Comprimento dos quatro traços externos, em pixels.
pub const TRACO: f32 = 6.0;
/// Espessura dos traços, em pixels.
pub const ESPESSURA_TRACO: f32 = 1.6;
/// Raio do ponto central, em pixels.
pub const PONTO: f32 = 1.1;
/// Segmentos do anel externo. 24 dá uma circunferência lisa a 13 px de raio.
pub const SEGMENTOS_EXTERNOS: u32 = 24;
/// Segmentos do anel interno.
pub const SEGMENTOS_INTERNOS: u32 = 16;

/// Cor da mira em repouso: branco levemente frio, para destacar do céu azulado.
pub const COR_REPOUSO: [f32; 4] = [0.86, 0.91, 1.0, 0.85];
/// Cor no instante do acerto.
pub const COR_ACERTO: [f32; 4] = [1.0, 0.35, 0.25, 1.0];
/// Opacidade da mira em repouso.
///
/// Não é 1,0 porque a mira fica **sobre o asteroide**: com alfa cheio ela vira um
/// anel branco chapado no meio da rocha; com 0,85 o fundo atravessa e o olho lê
/// "mira sobre a pedra". O acerto vai a 1,0 — o realce é justamente o contraste.
pub const ALFA_REPOUSO: f32 = 0.85;
/// Escala aplicada à mira no acerto.
pub const ESCALA_ACERTO: f32 = 1.35;
/// Duração do realce de acerto, em segundos.
pub const DURACAO_ACERTO: f32 = 0.05;

/// Converte um ponto em pixels para o espaço do HUD.
///
/// `(0, 0)` é o canto superior esquerdo da tela, como em qualquer HUD.
pub fn pixel_para_hud(px: f32, py: f32, tela: (u32, u32)) -> (f32, f32) {
    let altura = tela.1.max(1) as f32;
    let escala = 2.0 * PROFUNDIDADE / altura;
    (
        (px - tela.0 as f32 * 0.5) * escala,
        (tela.1 as f32 * 0.5 - py) * escala,
    )
}

/// Meia-largura da tela em unidades de HUD.
pub fn meia_largura_hud(tela: (u32, u32)) -> f32 {
    let altura = tela.1.max(1) as f32;
    tela.0 as f32 * 0.5 * (2.0 * PROFUNDIDADE / altura)
}

/// Um quadrilátero convexo, emitido como dois triângulos.
fn quad(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, cantos: [[f32; 3]; 4]) {
    const NORMAL: [f32; 3] = [0.0, 0.0, 1.0];
    // Dois triângulos com a mesma ordem de enrolamento dos cantos. A câmera do
    // HUD olha para `-Z`, então o lado visível é o `+Z`; com o descarte de faces
    // ligado, inverter a ordem faria a mira sumir.
    for triangulo in [[0u32, 1, 2], [0, 2, 3]] {
        for i in triangulo {
            indices.push(vertices.len() as u32);
            vertices.push(Vertex::flat(cantos[i as usize], NORMAL));
        }
    }
}

/// Um retângulo centrado em `(cx, cy)`, alinhado aos eixos, em pixels.
fn retangulo(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    cx: f32,
    cy: f32,
    largura: f32,
    altura: f32,
    tela: (u32, u32),
) {
    let (x0, y0) = pixel_para_hud(cx - largura * 0.5, cy + altura * 0.5, tela);
    let (x1, y1) = pixel_para_hud(cx + largura * 0.5, cy + altura * 0.5, tela);
    let (x2, y2) = pixel_para_hud(cx + largura * 0.5, cy - altura * 0.5, tela);
    let (x3, y3) = pixel_para_hud(cx - largura * 0.5, cy - altura * 0.5, tela);
    let z = -PROFUNDIDADE;
    quad(
        vertices,
        indices,
        [[x0, y0, z], [x1, y1, z], [x2, y2, z], [x3, y3, z]],
    );
}

/// Um anel (coroa circular) em torno do centro da tela, em pixels.
fn anel(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    raio: f32,
    espessura: f32,
    segmentos: u32,
    tela: (u32, u32),
) {
    let (cx, cy) = (tela.0 as f32 * 0.5, tela.1 as f32 * 0.5);
    let raio_interno = (raio - espessura * 0.5).max(0.1);
    let raio_externo = raio + espessura * 0.5;
    let passos = segmentos.max(3);

    for passo in 0..passos {
        let a0 = passo as f32 / passos as f32 * std::f32::consts::TAU;
        let a1 = (passo + 1) as f32 / passos as f32 * std::f32::consts::TAU;
        // Cantos em pixels, depois convertidos: assim a espessura é constante em
        // pixels e não em unidades de mundo — que é o que mantém a mira com o
        // mesmo peso visual em qualquer painel.
        let ponto = |angulo: f32, r: f32| {
            pixel_para_hud(cx + r * angulo.cos(), cy + r * angulo.sin(), tela)
        };
        let (i0, j0) = ponto(a0, raio_interno);
        let (i1, j1) = ponto(a1, raio_interno);
        let (e0, f0) = ponto(a0, raio_externo);
        let (e1, f1) = ponto(a1, raio_externo);
        let z = -PROFUNDIDADE;
        // Anti-horário visto de `+Z`, como os retângulos: o canto interno do
        // ângulo inicial, o interno do final, o externo do final e o externo do
        // inicial. A ordem "interna → externa → externa → interna" desenhava o
        // anel em sentido horário e o `cull_face` o removia **em silêncio** —
        // medido: só os quatro traços apareciam no quadro, 52 px alterados.
        quad(
            vertices,
            indices,
            [[i0, j0, z], [i1, j1, z], [e1, f1, z], [e0, f0, z]],
        );
    }
}

/// Constrói a mira inteira numa malha só.
///
/// Uma malha e não seis: a mira é **um** desenho por quadro, e seis chamadas para
/// desenhar uma retícula de 24 px gastariam o orçamento do R5 com nada.
pub fn construir(tela: (u32, u32)) -> Malha {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let (cx, cy) = (tela.0 as f32 * 0.5, tela.1 as f32 * 0.5);

    anel(
        &mut vertices,
        &mut indices,
        RAIO_EXTERNO,
        ESPESSURA_EXTERNA,
        SEGMENTOS_EXTERNOS,
        tela,
    );
    anel(
        &mut vertices,
        &mut indices,
        RAIO_INTERNO,
        ESPESSURA_INTERNA,
        SEGMENTOS_INTERNOS,
        tela,
    );

    // Quatro traços para fora do anel, nos eixos. O vão entre o anel e o traço é
    // o que os torna legíveis: encostados, viram uma cruz gorda.
    let inicio = RAIO_EXTERNO + ESPESSURA_EXTERNA * 0.5 + 1.5;
    let meio = inicio + TRACO * 0.5;
    for (dx, dy, largura, altura) in [
        (0.0, -meio, ESPESSURA_TRACO, TRACO),
        (0.0, meio, ESPESSURA_TRACO, TRACO),
        (-meio, 0.0, TRACO, ESPESSURA_TRACO),
        (meio, 0.0, TRACO, ESPESSURA_TRACO),
    ] {
        retangulo(
            &mut vertices,
            &mut indices,
            cx + dx,
            cy + dy,
            largura,
            altura,
            tela,
        );
    }

    // Ponto central: é ele que marca o pixel exato do raio. Sem ele, uma mira de
    // anel vazio deixa dúvida sobre onde está o centro.
    retangulo(
        &mut vertices,
        &mut indices,
        cx,
        cy,
        PONTO * 2.0,
        PONTO * 2.0,
        tela,
    );

    Malha { vertices, indices }
}

/// Estado visual da mira: repouso ou realce de acerto.
///
/// Vive no núcleo, sem GL, porque "o realce dura 50 ms" é uma regra que dá para
/// testar sem abrir janela — e é a regra que o jogador sente como resposta ao
/// tiro.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mira {
    /// Tempo restante de realce, em segundos.
    pub realce: f32,
}

impl Mira {
    /// Mira em repouso.
    pub fn new() -> Self {
        Self { realce: 0.0 }
    }

    /// Marca um acerto: realce por [`DURACAO_ACERTO`].
    pub fn acertou(&mut self) {
        self.realce = DURACAO_ACERTO;
    }

    /// Avança o realce.
    ///
    /// Um `dt` maior que a duração zera em vez de acumular negativo: o realce
    /// não tem "dívida", e um número negativo faria `realcando()` mentir.
    pub fn atualizar(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.realce = (self.realce - dt).max(0.0);
    }

    /// Está no instante do realce?
    pub fn realcando(&self) -> bool {
        self.realce > 0.0
    }

    /// Cor atual, em RGBA linear.
    pub fn cor(&self) -> [f32; 4] {
        if self.realcando() {
            COR_ACERTO
        } else {
            COR_REPOUSO
        }
    }

    /// Escala atual.
    pub fn escala(&self) -> f32 {
        if self.realcando() {
            ESCALA_ACERTO
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use giochi_engine_3d::math3d::Color3D;

    const TELA: (u32, u32) = (640, 480);

    #[test]
    fn a_mira_e_bem_mais_clara_que_a_rocha() {
        // A mira é desenhada **sobre o asteroide**, no centro da tela. Se a cor
        // dela estivesse perto da cor da rocha, ela desapareceria — e foi o que
        // aconteceu na primeira versão, com o shader `flat` escurecendo tudo em
        // 46 %. O contraste mínimo aqui é 1,4× de luminância.
        let lum = |c: [f32; 4]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let rocha = Color3D::new(0.58, 0.60, 0.66, 1.0).to_array();
        assert!(
            lum(COR_REPOUSO) > lum(rocha) * 1.4,
            "mira {:.2} contra rocha {:.2}",
            lum(COR_REPOUSO),
            lum(rocha)
        );
        assert!(lum(COR_ACERTO) < lum(COR_REPOUSO), "o acerto é mais escuro");
        // Pelo caminho que o desenho usa (`Mira::cor`), e não pela constante
        // solta: comparar duas constantes é conta que o clippy resolve em tempo
        // de compilação, e o que importa é a cor que chega ao uniforme.
        let acerto = Mira {
            realce: DURACAO_ACERTO,
        }
        .cor();
        assert!(
            acerto[0] > acerto[1] * 2.0,
            "o acerto devia puxar para o vermelho: {acerto:?}"
        );
    }

    #[test]
    fn o_realce_dura_os_cinquenta_milissegundos() {
        let mut mira = Mira::new();
        assert!(!mira.realcando());
        mira.acertou();
        assert!(mira.realcando());
        mira.atualizar(DURACAO_ACERTO * 0.5);
        assert!(mira.realcando(), "sumiu antes da metade");
        mira.atualizar(DURACAO_ACERTO * 0.6);
        assert!(!mira.realcando(), "durou mais que a constante");
        assert_eq!(mira.realce, 0.0, "o realce não pode ficar negativo");
    }

    #[test]
    fn o_realce_muda_cor_e_escala() {
        let mut mira = Mira::new();
        let repouso = (mira.cor(), mira.escala());
        mira.acertou();
        let acerto = (mira.cor(), mira.escala());
        assert_ne!(repouso.0, acerto.0, "a cor não mudou no acerto");
        assert!(acerto.1 > repouso.1, "a escala não cresceu no acerto");
    }

    #[test]
    fn um_passo_longo_zera_o_realce() {
        // Um quadro de 200 ms (aparelho travado) não pode deixar o realce
        // "devendo" tempo para os quadros seguintes.
        let mut mira = Mira::new();
        mira.acertou();
        mira.atualizar(0.2);
        assert_eq!(mira.realce, 0.0);
        assert!(!mira.realcando());
    }

    #[test]
    fn dt_invalido_nao_mexe_no_realce() {
        let mut mira = Mira::new();
        mira.acertou();
        let antes = mira;
        mira.atualizar(0.0);
        mira.atualizar(-1.0);
        mira.atualizar(f32::NAN);
        assert_eq!(mira, antes);
    }

    fn extensao(malha: &Malha) -> (f32, f32) {
        let (minimo, maximo) = malha.limites();
        (maximo[0] - minimo[0], maximo[1] - minimo[1])
    }

    fn centroide(malha: &Malha) -> (f32, f32) {
        let n = malha.vertices.len() as f32;
        let soma = malha.vertices.iter().fold([0.0f32; 2], |mut acc, v| {
            acc[0] += v.position[0];
            acc[1] += v.position[1];
            acc
        });
        (soma[0] / n, soma[1] / n)
    }

    fn raio_maximo(malha: &Malha) -> f32 {
        malha
            .vertices
            .iter()
            .map(|v| (v.position[0] * v.position[0] + v.position[1] * v.position[1]).sqrt())
            .fold(0.0, f32::max)
    }

    #[test]
    fn a_mira_fica_centrada_na_tela() {
        let malha = construir(TELA);
        let (cx, cy) = centroide(&malha);
        assert!(cx.abs() < 0.02, "centroide x {cx}");
        assert!(cy.abs() < 0.02, "centroide y {cy}");
    }

    #[test]
    fn a_mira_e_redonda_e_simetrica() {
        // Quatro braços e dois anéis: o espelho em x e em y tem de bater. Um
        // braço faltando ou um anel com o dobro de segmentos de um lado aparece
        // aqui, e não numa olhada de 24 px.
        let malha = construir(TELA);
        let (largura, altura) = extensao(&malha);
        assert!(
            (largura - altura).abs() < 1e-3,
            "a mira não está redonda: {largura} x {altura}"
        );

        let mut espelhado = 0;
        for v in &malha.vertices {
            let (ex, ey) = (-v.position[0], -v.position[1]);
            if malha
                .vertices
                .iter()
                .any(|o| (o.position[0] - ex).abs() < 1e-4 && (o.position[1] - ey).abs() < 1e-4)
            {
                espelhado += 1;
            }
        }
        assert_eq!(
            espelhado,
            malha.vertices.len(),
            "a mira não é simétrica em relação ao centro"
        );
    }

    #[test]
    fn os_tracos_vem_depois_do_anel() {
        // Precisa haver vão: encostados, o anel e os quatro traços leem como uma
        // cruz gorda e o centro desaparece.
        let malha = construir(TELA);
        let raio = raio_maximo(&malha);
        let escala = 2.0 * PROFUNDIDADE / TELA.1 as f32;
        let esperado = (RAIO_EXTERNO + ESPESSURA_EXTERNA * 0.5 + 1.5 + TRACO) * escala;
        assert!(
            (raio - esperado).abs() < 1e-3,
            "raio máximo {raio}, esperado {esperado}"
        );
        let folga = (raio - (RAIO_EXTERNO + ESPESSURA_EXTERNA * 0.5) * escala) * TELA.1 as f32
            / (2.0 * PROFUNDIDADE);
        assert!(folga > 3.0, "vão de apenas {folga} px entre anel e traço");
    }

    #[test]
    fn a_mira_cabe_na_tela_em_todos_os_paineis() {
        // A 480p, 600p, 720p e retrato. O raio é em pixels, então o que pode
        // estourar é o painel pequeno — e um braço saindo da tela é mira que
        // perde o sentido.
        for tela in [(640, 480), (800, 600), (1280, 720), (320, 240), (720, 1280)] {
            let malha = construir(tela);
            let meia = meia_largura_hud(tela);
            let (minimo, maximo) = malha.limites();
            assert!(
                minimo[0] > -meia && maximo[0] < meia,
                "{tela:?}: a mira estoura a tela em x ({minimo:?}..{maximo:?})"
            );
            assert!(
                minimo[1] > -PROFUNDIDADE && maximo[1] < PROFUNDIDADE,
                "{tela:?}: a mira estoura a tela em y"
            );
        }
    }

    #[test]
    fn o_anel_nao_e_um_disco_e_o_ponto_central_existe() {
        // Dois defeitos de uma vez, medidos por vértice: um anel preenchido por
        // engano (raio interno 0) bloquearia o centro da tela, que é onde o
        // jogador precisa ver o alvo; e sem o ponto o centro fica ambíguo.
        //
        // Os vértices são **cantos** de quad, então o ponto de 1,1 px tem cantos
        // a 1,55 px do centro — e é essa a medida que vale, não o raio do desenho.
        let malha = construir(TELA);
        let escala = 2.0 * PROFUNDIDADE / TELA.1 as f32;
        let raio = |v: &Vertex| (v.position[0].powi(2) + v.position[1].powi(2)).sqrt() / escala;

        // Seis entradas e não quatro: um quad vira dois triângulos, e a malha é
        // não indexada — os cantos 0 e 2 aparecem duas vezes.
        let dentro = malha.vertices.iter().filter(|v| raio(v) < 4.0).count();
        assert_eq!(
            dentro, 6,
            "esperava só o ponto central dentro de 4 px; achei {dentro} entradas \
             (anel preenchido?)"
        );

        let ponto = malha
            .vertices
            .iter()
            .filter(|v| {
                let r = raio(v);
                (1.0..2.5).contains(&r)
            })
            .count();
        assert_eq!(
            ponto, 6,
            "o ponto central devia ter 6 entradas (um quad) a 1,1 px"
        );
    }

    #[test]
    fn a_malha_e_uma_so_e_fechada_em_triangulos() {
        let malha = construir(TELA);
        assert_eq!(malha.indices.len() % 3, 0);
        assert_eq!(
            malha.vertices.len(),
            malha.indices.len(),
            "malha não indexada"
        );
        // 2 anéis × segmentos + 4 traços + 1 ponto, dois triângulos cada.
        let quads = SEGMENTOS_EXTERNOS + SEGMENTOS_INTERNOS + 4 + 1;
        assert_eq!(malha.triangulos() as u32, quads * 2);
    }

    #[test]
    fn todos_os_triangulos_estao_no_mesmo_sentido() {
        // A câmera do HUD olha para `-Z` e o descarte de faces está ligado com
        // `FrontFace::CounterClockwise`. Um quadrilátero emitido ao contrário é
        // removido **sem erro nenhum**: o desenho some e o único sinal é o
        // quadro. Foi assim que o anel inteiro desapareceu na primeira versão —
        // os quatro traços apareceram e os dois anéis não.
        let malha = construir(TELA);
        let mut horarios = 0;
        for t in malha.vertices.chunks_exact(3) {
            let (x0, y0) = (t[0].position[0], t[0].position[1]);
            let (x1, y1) = (t[1].position[0], t[1].position[1]);
            let (x2, y2) = (t[2].position[0], t[2].position[1]);
            let area = ((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)) * 0.5;
            if area <= 0.0 {
                horarios += 1;
            }
        }
        assert_eq!(
            horarios, 0,
            "{horarios} triângulo(s) em sentido horário: seriam removidos pelo \
             cull_face e a mira sairia incompleta"
        );
    }

    #[test]
    fn a_construcao_e_deterministica() {
        assert_eq!(construir(TELA), construir(TELA));
        assert_ne!(
            construir(TELA),
            construir((1280, 720)),
            "a mira não pode ter a mesma geometria em telas diferentes"
        );
    }

    #[test]
    fn a_espessura_e_a_mesma_em_pixels_nos_dois_paineis() {
        // O que tem de ser constante é o peso **visual**: 1,6 px de espessura em
        // 480p e em 720p. Em unidades de mundo ela mudaria com o painel, e a mira
        // ficaria fina demais no aparelho.
        for tela in [(640, 480), (1280, 720)] {
            let escala = 2.0 * PROFUNDIDADE / tela.1 as f32;
            let raio = raio_maximo(&construir(tela)) / escala;
            let esperado = RAIO_EXTERNO + ESPESSURA_EXTERNA * 0.5 + 1.5 + TRACO;
            // 0,05 px de folga: o canto de um retângulo não cai exatamente no
            // raio nominal, e exigir o valor exato mediria o arredondamento do
            // `f32` em vez do peso visual.
            assert!(
                (raio - esperado).abs() < 0.05,
                "{tela:?}: raio {raio} px contra {esperado} px esperados"
            );
        }
    }

    #[test]
    fn a_conversao_de_pixel_bate_com_os_cantos() {
        // Canto superior esquerdo e inferior direito, em unidades de HUD: a
        // largura tem de ser 2·meia_largura e a altura 2·PROFUNDIDADE.
        let (x0, y0) = pixel_para_hud(0.0, 0.0, TELA);
        let (x1, y1) = pixel_para_hud(TELA.0 as f32, TELA.1 as f32, TELA);
        assert!((x0 + meia_largura_hud(TELA)).abs() < 1e-4);
        assert!((x1 - meia_largura_hud(TELA)).abs() < 1e-4);
        assert!((y0 - PROFUNDIDADE).abs() < 1e-4);
        assert!((y1 + PROFUNDIDADE).abs() < 1e-4);
    }
}
