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
