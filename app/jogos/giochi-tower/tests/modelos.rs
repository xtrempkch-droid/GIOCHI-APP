//! Os modelos e as texturas que o jogo carrega existem, carregam e têm a forma
//! que o gerador promete.
//!
//! # Por que testar asset em Rust e não só em JS
//!
//! O gerador (`scripts/gerar-modelos.mjs`) garante que o arquivo é reproduzível.
//! Ele **não** garante que o loader da engine o aceita: uma face `f v/vt` num
//! formato que o `tobj` recusa, ou um UV fora de `0..1`, produz um arquivo que o
//! Node escreve feliz e o jogo recusa em execução — no aparelho, sem terminal.
//!
//! `giochi_engine_3d::mesh::load_obj` é **puro** (não abre GL), então o caminho
//! real do jogo pode ser exercitado aqui, sem contexto gráfico.

use std::path::{Path, PathBuf};

use giochi_engine_3d::mesh::load_obj;

/// Os modelos que o jogo usa. Um nome a mais aqui reprova — e um a menos também,
/// porque `o_catalogo_de_modelos_do_jogo_esta_completo` compara com a tabela.
const MODELOS: [&str; 15] = [
    "soldado",
    "guerreiro",
    "sniper",
    "medico",
    "grunt",
    "tank",
    "ranged",
    "swarm",
    "flying",
    "tanker",
    "plasma",
    "brain",
    "queen",
    "coronel",
    "torre",
];

/// O cenário do planeta morto (`engine_bridge::modelos::CENARIO`).
///
/// Fica numa lista separada de [`MODELOS`] porque responde outra pergunta: não
/// "o personagem carrega?", e sim "a paisagem carrega e tem a proporção que o
/// gerador promete?". A torre é mais alta que qualquer personagem; uma mesa de
/// 40 m não é, e misturar as duas listas faria o teste da torre reprovar por um
/// motivo que não é defeito.
const CENARIO: [&str; 5] = ["rocha", "espigao", "cratera", "mesa", "destroco"];

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn todo_modelo_do_catalogo_carrega_e_desenha() {
    for nome in MODELOS {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, indices) = load_obj(&caminho)
            .unwrap_or_else(|erro| panic!("{nome}.obj não carregou: {erro}"));

        assert!(
            vertices.len() >= 24,
            "{nome}.obj tem {} vértices — menos que um cubo, então o arquivo saiu truncado",
            vertices.len()
        );
        assert_eq!(
            indices.len() % 3,
            0,
            "{nome}.obj: {} índices não formam triângulos inteiros",
            indices.len()
        );
        // Nenhum índice fora da faixa: o loader não valida, e na GPU isso é
        // leitura fora dos limites, que aparece como superfície piscando sem erro
        // de GL.
        let maior = *indices.iter().max().expect("índices");
        assert!(
            (maior as usize) < vertices.len(),
            "{nome}.obj: índice {maior} com {} vértices",
            vertices.len()
        );

        // Normal não nula não é "detalhe": o loader deriva a normal de face quando
        // ela falta, e o shader `textured` multiplica a cor por `0.35+0.65·lambert`.
        // Normal zero em todo vértice = modelo preto ou dependente de driver.
        for (i, v) in vertices.iter().enumerate() {
            let n = v.normal;
            let modulo = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                modulo > 0.5,
                "{nome}.obj: vértice {i} com normal {:?} (módulo {modulo:.3})",
                n
            );
            let p = v.position;
            assert!(
                p.iter().all(|c| c.is_finite()),
                "{nome}.obj: vértice {i} com posição não finita {p:?}"
            );
        }
    }
}

#[test]
fn as_texturas_sao_bmp_de_24_bits_do_tamanho_declarado() {
    for nome in MODELOS {
        let caminho = raiz().join("assets").join("texturas").join(format!("{nome}.bmp"));
        let bytes = std::fs::read(&caminho)
            .unwrap_or_else(|erro| panic!("{nome}.bmp não abriu: {erro}"));

        assert_eq!(&bytes[0..2], b"BM", "{nome}.bmp não é BMP");
        let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let largura = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
        let altura = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
        let bits = u16::from_le_bytes(bytes[28..30].try_into().unwrap());
        let compressao = u32::from_le_bytes(bytes[30..34].try_into().unwrap());

        assert_eq!((largura, altura), (64, 64), "{nome}.bmp: {largura}x{altura}");
        assert_eq!(bits, 24, "{nome}.bmp: {bits} bits (o decoder do aparelho lê 24)");
        assert_eq!(compressao, 0, "{nome}.bmp é comprimido (BI_RLE); ler como cru dá lixo");

        // Linhas de baixo para cima, alinhadas em 4 bytes: 64*3 = 192, que já é
        // múltiplo de 4 — se alguém mudar o lado para 72, o cálculo continua certo.
        let linha = (largura as usize * 3).div_ceil(4) * 4;
        let esperado = offset + linha * altura as usize;
        assert_eq!(bytes.len(), esperado, "{nome}.bmp: tamanho não bate com o cabeçalho");
    }
}

#[test]
fn as_texturas_nao_sao_uma_cor_so() {
    // Uma textura de cor única volta a ser um cubo pintado: o shader tem Lambert
    // fixo e é a variação de tom na textura que desenha o relevo. O gerador
    // escreve painéis, emenda e acento; se todos sumirem, este teste reprova.
    for nome in MODELOS {
        let caminho = raiz().join("assets").join("texturas").join(format!("{nome}.bmp"));
        let bytes = std::fs::read(&caminho).expect("bmp");
        let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        let cores: std::collections::BTreeSet<[u8; 3]> = bytes[offset..]
            .chunks_exact(3)
            .map(|p| [p[2], p[1], p[0]])
            .collect();
        assert!(
            cores.len() >= 16,
            "{nome}.bmp tem {} cor(es) distintas — a textura saiu chapada",
            cores.len()
        );
    }
}

#[test]
fn os_personagens_tem_altura_diferente_e_nao_sao_o_mesmo_cubo() {
    // O pedido que originou esta rodada foi "os personagens estão como apenas
    // cubos". Um teste que só contasse vértices passaria com 15 cubos
    // subdivididos; o que distingue modelo de cubo é a **caixa** de cada um ser
    // diferente e ter mais de uma silhueta.
    let mut alturas = Vec::new();
    for nome in MODELOS {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, _) = load_obj(&caminho).expect("carrega");
        let altura = vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max)
            - vertices
                .iter()
                .map(|v| v.position[1])
                .fold(f32::INFINITY, f32::min);
        assert!(altura > 0.3, "{nome}: altura {altura:.2} m — degenerado");
        alturas.push((nome, altura));
    }

    let distintas: std::collections::BTreeSet<u32> =
        alturas.iter().map(|(_, a)| (a * 10.0).round() as u32).collect();
    assert!(
        distintas.len() >= 5,
        "os modelos têm só {} alturas distintas — parecem o mesmo cubo escalado: {alturas:?}",
        distintas.len()
    );
}

#[test]
fn a_torre_e_mais_alta_que_qualquer_personagem() {
    // O GDD §11 faz da torre o alvo a defender. Se um inimigo for mais alto que
    // ela, o jogador perde a referência de onde está o centro do mapa.
    let altura_de = |nome: &str| {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, _) = load_obj(&caminho).expect("carrega");
        vertices.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max)
    };
    let torre = altura_de("torre");
    for nome in MODELOS.iter().filter(|n| **n != "torre") {
        let altura = altura_de(nome);
        assert!(
            altura < torre,
            "{nome} tem {altura:.2} m e a torre {torre:.2} m — o alvo some no meio da horda"
        );
    }
}

#[test]
fn os_bugs_sao_mais_largos_que_altos_e_os_soldados_nao() {
    // O pedido do dono foi "soldados se defendendo de uma horda de bugs". Um
    // inseto e um humanoide com textura de inseto têm a mesma altura; o que os
    // separa é a **proporção**: aracnídeo é baixo, largo e comprido; soldado é
    // alto e estreito.
    //
    // Um teste que só contasse vértices passaria com qualquer coisa. Este mede a
    // caixa, que é o que o olho usa a 20 m de distância.
    let caixa = |nome: &str| {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, _) = load_obj(&caminho).expect("carrega");
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &vertices {
            for eixo in 0..3 {
                min[eixo] = min[eixo].min(v.position[eixo]);
                max[eixo] = max[eixo].max(v.position[eixo]);
            }
        }
        [max[0] - min[0], max[1] - min[1], max[2] - min[2]]
    };

    let bugs = ["grunt", "tank", "ranged", "swarm", "flying"];
    for nome in bugs {
        let [largura, altura, comprimento] = caixa(nome);
        let maior = largura.max(comprimento);
        assert!(
            maior > altura * 1.12,
            "{nome}: {largura:.2}×{altura:.2}×{comprimento:.2} — bug não é mais largo que alto, parece um soldado"
        );
    }

    for nome in ["soldado", "guerreiro", "sniper", "medico"] {
        let [largura, altura, comprimento] = caixa(nome);
        // O Guerreiro carrega um escudo no antebraço e o Sniper um fuzil de 1,65 m,
        // então a folga é sobre a **largura** (o que o escudo alarga) e sobre o
        // comprimento (o que o fuzil alonga). O que se exige é que a altura mande
        // na silhueta — é o que faz o soldado ler em pé.
        assert!(
            altura > largura * 1.15 && altura > comprimento * 0.9,
            "{nome}: {largura:.2}×{altura:.2}×{comprimento:.2} — soldado tem de ser mais alto que largo"
        );
    }
}

#[test]
fn os_bosses_e_o_coronel_tambem_sao_aracnideos() {
    // Os quatro bosses e o nemesis usam a mesma família de silhueta, em escala.
    // Se um deles voltar a ser humanoide (o bug que originou esta rodada), a
    // horda deixa de ter um "chefe" reconhecível como inseto.
    for nome in ["tanker", "plasma", "brain", "queen", "coronel"] {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, _) = load_obj(&caminho).expect("carrega");
        let extensao = |eixo: usize| {
            vertices
                .iter()
                .map(|v| v.position[eixo])
                .fold(f32::NEG_INFINITY, f32::max)
                - vertices
                    .iter()
                    .map(|v| v.position[eixo])
                    .fold(f32::INFINITY, f32::min)
        };
        let (largura, altura, comprimento) = (extensao(0), extensao(1), extensao(2));
        let maior = largura.max(comprimento);
        assert!(
            maior > altura * 1.05,
            "{nome}: {largura:.2}×{altura:.2}×{comprimento:.2} — um boss humanoide outra vez?"
        );
    }
}

#[test]
fn o_cenario_carrega_e_tem_a_proporcao_que_o_jogo_usa() {
    for nome in CENARIO {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let (vertices, indices) = load_obj(&caminho)
            .unwrap_or_else(|erro| panic!("{nome}.obj não carregou: {erro}"));
        assert!(vertices.len() >= 24, "{nome}.obj: {} vértices", vertices.len());
        assert_eq!(indices.len() % 3, 0, "{nome}.obj: triângulos incompletos");

        let caminho = raiz().join("assets").join("texturas").join(format!("{nome}.bmp"));
        let bytes = std::fs::read(&caminho).unwrap_or_else(|e| panic!("{nome}.bmp: {e}"));
        assert_eq!(&bytes[0..2], b"BM", "{nome}.bmp não é BMP");
    }

    // A cratera é chão, não obstáculo: o perfil é raso de propósito, e a altura
    // é a constante que converte diâmetro em escala (`BORDA_DA_CRATERA_M`).
    let caminho = raiz().join("assets").join("modelos").join("cratera.obj");
    let (vertices, _) = load_obj(&caminho).expect("cratera");
    let altura = vertices
        .iter()
        .map(|v| v.position[1])
        .fold(f32::NEG_INFINITY, f32::max)
        - vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min);
    assert!(
        (altura - giochi_tower::game::cenario::BORDA_DA_CRATERA_M).abs() < 1.0e-3,
        "cratera mede {altura:.3} m e a constante diz {} — ajuste as duas",
        giochi_tower::game::cenario::BORDA_DA_CRATERA_M
    );
}

#[test]
fn a_textura_do_chao_e_bmp_quadrada_de_potencia_de_dois() {
    // O GLES 2.0 só aceita REPEAT em textura de potência de dois; o loader da
    // engine troca para CLAMP_TO_EDGE em qualquer outro tamanho, e o chão de
    // 600 m com CLAMP vira uma faixa esticada — sem erro de GL.
    let bytes = std::fs::read(raiz().join("assets").join("texturas").join("terreno.bmp"))
        .expect("terreno.bmp");
    assert_eq!(&bytes[0..2], b"BM");
    let largura = i32::from_le_bytes(bytes[18..22].try_into().unwrap());
    let altura = i32::from_le_bytes(bytes[22..26].try_into().unwrap());
    assert_eq!(largura, altura, "terreno.bmp: {largura}x{altura}");
    assert!(
        largura > 0 && (largura & (largura - 1)) == 0,
        "terreno.bmp: {largura} não é potência de dois"
    );

    // E não pode ser uma cor só: uma textura chapada não mostra que o chão tem
    // grão, e o teste de repetição não teria o que medir.
    let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let cores: std::collections::BTreeSet<[u8; 3]> = bytes[offset..]
        .chunks_exact(3)
        .map(|p| [p[2], p[1], p[0]])
        .collect();
    assert!(cores.len() >= 64, "terreno.bmp tem {} cor(es)", cores.len());
}

#[test]
fn os_modelos_tem_normal_por_vertice_e_nao_so_por_face() {
    // A ausência de `vn` faz o loader derivar **uma normal por triângulo**, e o
    // corpo vira um totem de placas — foi assim que o soldado apareceu na
    // vitrine antes desta correção. O teste exige que o arquivo declare `vn` e
    // que a normal de cada vértice seja unitária (uma normal não normalizada
    // escurece o modelo de forma proporcional, e ninguém liga o sintoma à conta).
    for nome in MODELOS.iter().chain(CENARIO.iter()) {
        let caminho = raiz().join("assets").join("modelos").join(format!("{nome}.obj"));
        let texto = std::fs::read_to_string(&caminho).expect("lê o obj");
        let normais: Vec<[f32; 3]> = texto
            .lines()
            .filter_map(|linha| linha.strip_prefix("vn "))
            .map(|resto| {
                let valores: Vec<f32> = resto.split_whitespace().map(|n| n.parse().unwrap()).collect();
                [valores[0], valores[1], valores[2]]
            })
            .collect();
        assert!(
            normais.len() >= 24,
            "{nome}.obj tem {} normal(is) declarada(s) — sem `vn` o modelo sai facetado",
            normais.len()
        );
        for (i, n) in normais.iter().enumerate() {
            let modulo = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                (modulo - 1.0).abs() < 1e-3,
                "{nome}.obj: normal {i} com módulo {modulo:.3} — tem de ser unitária"
            );
        }
    }
}

#[test]
fn a_normal_do_aracnideo_aponta_para_fora_do_corpo() {
    // Normal invertida não dá erro em lugar nenhum: o shader `textured`
    // multiplica por `0.35 + 0.65·lambert`, e o bug simplesmente aparece escuro
    // do lado iluminado. Aqui o teste usa a convexidade do abdômen: a normal de
    // um vértice tem de ter produto escalar positivo com a direção que sai do
    // centro do corpo.
    let caminho = raiz().join("assets").join("modelos").join("grunt.obj");
    let (vertices, _) = load_obj(&caminho).expect("grunt");
    let mut centro = [0.0_f32; 3];
    for v in &vertices {
        for (eixo, soma) in centro.iter_mut().enumerate() {
            *soma += v.position[eixo];
        }
    }
    for c in &mut centro {
        *c /= vertices.len() as f32;
    }

    // A média, e não a contagem: as pernas apontam para fora do **eixo da perna**,
    // não do centro do corpo, então exigir "todo vértice para fora" reprovaria um
    // modelo correto. O que a média distingue é inversão **global** — com sinal
    // trocado ela fica negativa, e é esse o defeito que se quer pegar.
    let mut soma = 0.0;
    for v in &vertices {
        let d = glam::Vec3::new(
            v.position[0] - centro[0],
            v.position[1] - centro[1],
            v.position[2] - centro[2],
        );
        let distancia = d.length();
        if distancia < 1e-4 {
            continue;
        }
        let n = glam::Vec3::from_array(v.normal);
        soma += d.dot(n) / distancia;
    }
    // 0,21 é o valor de um modelo **correto** (medido): a maioria dos vértices de
    // um aracnídeo está nas pernas, cujas normais são tangentes ao centro do corpo
    // e cancelam entre si. O que o número separa é o sinal — com todas as normais
    // trocadas, a mesma conta dá **-0,21**.
    let media = soma / vertices.len() as f32;
    assert!(
        media > 0.12,
        "a média de (normal · direção radial) é {media:.2} — o valor de um modelo são é ~0,21 e o de um invertido, ~-0,21"
    );
}
