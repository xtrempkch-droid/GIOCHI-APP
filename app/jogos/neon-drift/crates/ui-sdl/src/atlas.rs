//! Atlas de sprites: um único RGBA onde cada célula é uma forma.
//!
//! ## O que muda em relação às texturas de superfície
//!
//! `texturas.rs` gera materiais **opacos** — asfalto, pedra, mata. Aqui são
//! **máscaras de alfa**: o RGB é branco e o que desenha é o `a`. A cor vem do
//! `tint` do desenho, exatamente como a cor do asfalto vem do `tint` do material.
//!
//! Isso é o que faz uma fumaça nascer branca, ficar cinza e depois sumir sem
//! precisar de uma textura por cor: o alfa da célula dá a forma, o alfa da faixa
//! de vida dá o desbotamento, e o `tint` dá a cor.
//!
//! ## Por que um atlas e não uma textura por forma
//!
//! Uma partícula desenhada como quadrilátero sólido lê como **quadrado** — foi
//! assim até aqui, e num painel de 3,5" isso aparece. Trocar por um círculo macio
//! exige uma textura; trocar para oito formas exige oito texturas, e oito
//! `bind` por quadro. Um atlas com as oito células custa **um** bind.
//!
//! Cada célula vira uma **malha** cujo UV cobre só aquele retângulo (ver
//! `malha::quad_celula`). A seleção da célula é geometria, não uniforme — o que
//! é o que permite desenhar todas as cópias de uma vez com
//! `draw_instanced_textured`.
//!
//! ## Potência de dois
//!
//! 128×128 com células de 32×32. O GLES 2.0 só garante `REPEAT` em textura de
//! lado potência de dois, e o laço de desenho usa `REPEAT` por padrão — uma
//! textura 96×96 funcionaria no Mesa e falharia em driver embarcado.

/// Lado do atlas, em pixels.
pub const LADO: u32 = 128;

/// Lado de cada célula, em pixels.
pub const CELULA: u32 = 32;

/// Células por linha.
pub const COLUNAS: u32 = LADO / CELULA;

/// Quantas células o atlas tem.
pub const CELULAS: usize = (COLUNAS * COLUNAS) as usize;

/// As formas do atlas.
///
/// A ordem é contrato: o `índice` é o que a partícula guarda e o que o
/// `quad_celula` converte em UV. Trocar duas entradas de lugar troca as formas na
/// tela sem quebrar nada — por isso o teste `cada_forma_desenh_algo_diferente`
/// compara as células entre si.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Forma {
    /// Nuvem macia: gaussiana larga. Fumaça de pneu.
    Puff,
    /// Gota pequena e redonda, borda dura. Respingo de água.
    Gota,
    /// Estrela de quatro pontas com núcleo quente. Faísca de raspão.
    Faisca,
    /// Risco horizontal com cauda. Rastro de nitro.
    Risco,
    /// Risco vertical fino. Gota de chuva.
    Chuva,
    /// Halo radial suave. Mote de neon no ar.
    Halo,
    /// Borrão irregular com alfa parcial. Marca de pneu no chão.
    Borrao,
    /// Anel vazado. Fragmento de vidro em rotação.
    Anel,
}

impl Forma {
    /// Todas as formas, na ordem em que ocupam o atlas.
    pub const TODAS: [Forma; 8] = [
        Forma::Puff,
        Forma::Gota,
        Forma::Faisca,
        Forma::Risco,
        Forma::Chuva,
        Forma::Halo,
        Forma::Borrao,
        Forma::Anel,
    ];

    /// Posição no atlas.
    pub fn indice(self) -> usize {
        match self {
            Forma::Puff => 0,
            Forma::Gota => 1,
            Forma::Faisca => 2,
            Forma::Risco => 3,
            Forma::Chuva => 4,
            Forma::Halo => 5,
            Forma::Borrao => 6,
            Forma::Anel => 7,
        }
    }

    /// Fator de tamanho para a forma ocupar a mesma área que o quadrado cheio
    /// ocupava antes.
    ///
    /// Só a `Gota` precisa: as outras formas já encostam na borda da célula (o
    /// `Puff` tem 0,43 de alfa em r=0,5, o `Halo` chega a 0,6) e ampliá-las
    /// passaria a amostrar a célula vizinha.
    pub fn ganho(self) -> f32 {
        match self {
            // Raio desenhado 0,72 do meio-quadrado; o quadrado ia até 1,0 —
            // e o canto dele até 1,41. 1,30 devolve o diâmetro aparente.
            Forma::Gota => 1.30,
            _ => 1.0,
        }
    }

    /// Nome estável, para o diagnóstico.
    pub fn nome(self) -> &'static str {
        match self {
            Forma::Puff => "puff",
            Forma::Gota => "gota",
            Forma::Faisca => "faisca",
            Forma::Risco => "risco",
            Forma::Chuva => "chuva",
            Forma::Halo => "halo",
            Forma::Borrao => "borrao",
            Forma::Anel => "anel",
        }
    }
}

/// Retângulo UV (`[u0, v0, u1, v1]`) de uma célula.
///
/// O `v` cresce para baixo porque a linha 0 da imagem é o topo, e o atlas é
/// subido linha a linha — inverter aqui faz cada sprite sair de cabeça para baixo,
/// que num puff radial não aparece e numa cauda de risco aparece.
pub fn retangulo(indice: usize) -> [f32; 4] {
    debug_assert!(indice < CELULAS, "célula {indice} não existe no atlas");
    let coluna = (indice as u32) % COLUNAS;
    let linha = (indice as u32) / COLUNAS;
    let passo = 1.0 / COLUNAS as f32;
    let u0 = coluna as f32 * passo;
    let v0 = linha as f32 * passo;
    [u0, v0, u0 + passo, v0 + passo]
}

/// Gera o atlas inteiro em RGBA8.
///
/// Branco com alfa: a forma está toda no canal alfa (ver o cabeçalho do módulo).
pub fn gerar() -> Vec<u8> {
    // RGB branco em todo o atlas, inclusive nas células que ainda não têm forma:
    // o `tint` multiplica a amostra, e uma célula com RGB zero (que é o que um
    // `vec![0u8; ..]` daria) desenharia preto em vez de transparente. O alfa
    // começa em 0, então célula vazia não aparece.
    let mut pixels = vec![255u8; (LADO * LADO * 4) as usize];
    for (i, byte) in pixels.iter_mut().enumerate() {
        if i % 4 == 3 {
            *byte = 0;
        }
    }
    for forma in Forma::TODAS {
        let base = forma.indice();
        let (coluna, linha) = (base as u32 % COLUNAS, base as u32 / COLUNAS);
        for y in 0..CELULA {
            for x in 0..CELULA {
                // Coordenada normalizada no centro do texel, em -1..1.
                let u = (x as f32 + 0.5) / CELULA as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / CELULA as f32 * 2.0 - 1.0;
                let alfa = alfa_da_forma(forma, u, v);
                let px = ((linha * CELULA + y) * LADO + (coluna * CELULA + x)) * 4;
                pixels[px as usize] = 255;
                pixels[px as usize + 1] = 255;
                pixels[px as usize + 2] = 255;
                pixels[px as usize + 3] = (alfa.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    pixels
}

/// Alfa de uma forma numa coordenada normalizada `-1..1`.
///
/// Pura e sem estado: é o que permite testar "esta célula é radial", "esta é mais
/// larga que alta" sem GPU.
pub fn alfa_da_forma(forma: Forma, u: f32, v: f32) -> f32 {
    let r = (u * u + v * v).sqrt();
    match forma {
        // Gaussiana larga: `exp(-k r²)` dá a borda sem degrau que faz a fumaça
        // parecer um quadrado recortado.
        Forma::Puff => (-(r * r) * 3.4).exp() * 0.95,
        // Gota: raio 0,72 com um texel de antialias, e sem halo em volta.
        Forma::Gota => suave(0.72, 0.10, r) * 0.9,
        // Faísca: quatro pontas. |u|^8 + |v|^8 pequeno = cruz fina; o núcleo é
        // uma gaussiana curta que dá o "quente" no meio.
        Forma::Faisca => {
            let cruz = (-(u.abs().powi(8) + v.abs().powi(8)) * 5.0).exp();
            let nucleo = (-(r * r) * 9.0).exp();
            (cruz * 0.85 + nucleo * 0.9).min(1.0)
        }
        // Risco: comprido em u, fino em v, com cauda que afina para a esquerda.
        Forma::Risco => {
            let cauda = (1.0 - (u + 1.0) * 0.5).powf(0.7);
            let perfil = (-(v * v) * 22.0).exp();
            let ponta = suave(1.0, 0.25, u.abs());
            perfil * ponta * (0.35 + 0.65 * cauda)
        }
        // Chuva: o mesmo risco, na vertical e mais fino.
        Forma::Chuva => {
            let perfil = (-(u * u) * 30.0).exp();
            let comprimento = suave(1.0, 0.30, v.abs());
            perfil * comprimento
        }
        // Halo: bem aberto, com um miolo um pouco mais denso.
        Forma::Halo => {
            let externo = (-(r * r) * 2.0).exp();
            let miolo = (-(r * r) * 12.0).exp() * 0.35;
            (externo * 0.75 + miolo).min(1.0)
        }
        // Borrão: alfa parcial e mancha — a marca de pneu não é um disco.
        Forma::Borrao => {
            let mancha = 0.55
                + 0.45 * ((u * 3.1).sin() * (v * 4.7).sin() * 0.5 + 0.5);
            suave(0.95, 0.35, r) * mancha * 0.8
        }
        // Anel: buraco no meio, casca fina.
        Forma::Anel => {
            let casca = suave(0.30, 0.22, (r - 0.62).abs());
            casca * suave(1.0, 0.15, r)
        }
    }
}

/// 1 dentro de `raio`, 0 fora, com transição linear em `borda` texels.
///
/// Degrau puro (`if r < raio`) serrilha a borda de toda forma redonda, e o
/// serrilhado aparece no painel de 3,5" como cintilação a cada quadro.
fn suave(raio: f32, borda: f32, r: f32) -> f32 {
    let t = ((raio - r) / borda.max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_atlas_tem_o_tamanho_que_a_gpu_exige() {
        let pixels = gerar();
        assert_eq!(pixels.len(), (LADO * LADO * 4) as usize);
        // Potência de dois: `REPEAT` só é garantido nesse caso no GLES 2.0.
        assert!(LADO.is_power_of_two(), "o atlas tem de ser potência de dois");
        assert_eq!(LADO % CELULA, 0, "as células têm de caber inteiras");
    }

    #[test]
    fn o_rgb_e_branco_e_so_o_alfa_desenha() {
        // Se o RGB variasse, o `tint` não conseguiria escolher a cor: branco
        // vezes tint é tint, e qualquer outra coisa seria tint escurecido.
        let pixels = gerar();
        for (i, byte) in pixels.iter().enumerate() {
            if i % 4 != 3 {
                assert_eq!(*byte, 255, "byte {i} do atlas não é branco");
            }
        }
    }

    #[test]
    fn cada_forma_desenh_algo_diferente() {
        // Duas células iguais seriam uma forma que ninguém vê e um teste que
        // passa por acidente. Compara o alfa total de cada célula.
        let pixels = gerar();
        let mut somas = Vec::new();
        for forma in Forma::TODAS {
            let r = retangulo(forma.indice());
            let mut soma = 0u64;
            for y in 0..LADO {
                for x in 0..LADO {
                    let u = (x as f32 + 0.5) / LADO as f32;
                    let v = (y as f32 + 0.5) / LADO as f32;
                    if u >= r[0] && u < r[2] && v >= r[1] && v < r[3] {
                        soma += u64::from(pixels[((y * LADO + x) * 4 + 3) as usize]);
                    }
                }
            }
            assert!(soma > 0, "{} saiu vazia", forma.nome());
            somas.push((forma.nome(), soma));
        }
        for i in 0..somas.len() {
            for j in (i + 1)..somas.len() {
                assert_ne!(
                    somas[i].1, somas[j].1,
                    "{} e {} têm o mesmo peso de alfa — são a mesma forma",
                    somas[i].0, somas[j].0
                );
            }
        }
    }

    #[test]
    fn o_risco_e_mais_largo_que_alto_e_a_chuva_ao_contrario() {
        // A orientação é o que distingue os dois: uma gota de chuva desenhada na
        // horizontal cai de lado.
        let largura_sobre_altura = |forma: Forma| {
            let mut por_coluna = [0u32; CELULA as usize];
            let mut por_linha = [0u32; CELULA as usize];
            for y in 0..CELULA {
                for x in 0..CELULA {
                    let u = (x as f32 + 0.5) / CELULA as f32 * 2.0 - 1.0;
                    let v = (y as f32 + 0.5) / CELULA as f32 * 2.0 - 1.0;
                    let a = (alfa_da_forma(forma, u, v) * 255.0) as u32;
                    por_coluna[x as usize] += a;
                    por_linha[y as usize] += a;
                }
            }
            let colunas = por_coluna.iter().filter(|v| **v > 40 * 255 / 100).count();
            let linhas = por_linha.iter().filter(|v| **v > 40 * 255 / 100).count();
            (colunas, linhas)
        };

        let (colunas, linhas) = largura_sobre_altura(Forma::Risco);
        assert!(colunas > linhas, "o risco tem de ser horizontal ({colunas}x{linhas})");
        let (colunas, linhas) = largura_sobre_altura(Forma::Chuva);
        assert!(linhas > colunas, "a chuva tem de ser vertical ({colunas}x{linhas})");
    }

    #[test]
    fn o_anel_tem_buraco_e_o_puff_nao() {
        // Sem o buraco, "anel" é só um nome para "halo menor".
        assert!(alfa_da_forma(Forma::Anel, 0.0, 0.0) < 0.1);
        assert!(alfa_da_forma(Forma::Anel, 0.62, 0.0) > 0.5);
        assert!(alfa_da_forma(Forma::Puff, 0.0, 0.0) > 0.8);
    }

    #[test]
    fn o_retangulo_de_cada_celula_cabe_no_atlas_e_nao_se_sobrepoe() {
        let passo = 1.0 / COLUNAS as f32;
        for forma in Forma::TODAS {
            let r = retangulo(forma.indice());
            assert!((r[2] - r[0] - passo).abs() < 1e-6);
            assert!(r[0] >= 0.0 && r[2] <= 1.0 + 1e-6);
            assert!(r[1] >= 0.0 && r[3] <= 1.0 + 1e-6);
        }
        // A primeira célula começa em 0 e a última termina em 1.
        assert_eq!(retangulo(0), [0.0, 0.0, passo, passo]);
        let ultima = retangulo(CELULAS - 1);
        assert!((ultima[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn o_puff_desbota_em_muitos_texels_e_nao_em_degrau() {
        // A regressão que este teste existe para pegar é `if r < 0.5 { 1.0 } else
        // { 0.0 }`: uma borda serrilhada, que num painel de 3,5" cintila a cada
        // quadro e transforma a fumaça num losango duro. Uma forma suave tem
        // dezenas de valores intermediários na travessia.
        let mut intermediarios = 0;
        for x in 0..CELULA {
            let u = (x as f32 + 0.5) / CELULA as f32 * 2.0 - 1.0;
            let a = alfa_da_forma(Forma::Puff, u, 0.0);
            if a > 0.05 && a < 0.95 {
                intermediarios += 1;
            }
        }
        assert!(
            intermediarios >= 12,
            "o puff só desbota em {intermediarios} texels — isso é um degrau"
        );
    }

    #[test]
    fn toda_forma_tem_um_centro_mais_denso_que_a_borda() {
        // Vale para todas menos o anel, cujo centro é o buraco por definição.
        for forma in Forma::TODAS {
            if forma == Forma::Anel {
                continue;
            }
            let centro = alfa_da_forma(forma, 0.0, 0.0);
            let canto = alfa_da_forma(forma, 0.97, 0.97);
            assert!(
                centro > canto,
                "{} é mais densa no canto ({canto}) que no centro ({centro})",
                forma.nome()
            );
        }
    }

    #[test]
    fn gerar_duas_vezes_da_os_mesmos_bytes() {
        // Determinismo: a comparação de quadro PC × aparelho depende de o atlas
        // ser o mesmo nos dois. Não há sorteio aqui, mas o teste fica para o dia
        // em que alguém acrescentar ruído a uma forma.
        assert_eq!(gerar(), gerar());
    }
}
