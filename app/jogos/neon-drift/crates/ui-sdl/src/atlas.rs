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

/// Uma imagem RGBA8 decodificada, com a origem no **canto superior esquerdo**.
pub struct Imagem {
    /// Largura em pixels.
    pub largura: u32,
    /// Altura em pixels.
    pub altura: u32,
    /// RGBA8, linha a linha, de cima para baixo.
    pub pixels: Vec<u8>,
}

/// Lê um BMP sem compressão, 24 ou 32 bits.
///
/// ## Por que BMP e não PNG
///
/// Pelo mesmo motivo que as texturas são geradas: **o jogo não linka
/// `SDL2_image`** — o `NEEDED` do binário é SDL2, `libgcc_s`, `libm` e `libc`.
/// BMP é o único formato que este leitor entrega sem dependência nova, e é o
/// que sai de qualquer conversor.
///
/// (A imagem de 07/11 do R35S também não decodificava PNG; a de 09/11 decodifica.
/// Isso não reabre a decisão, porque o bloqueio é a dependência, não a imagem.)
///
/// ## O que é aceito
///
/// `BITMAPINFOHEADER` (40 bytes, o de longe mais comum) com `biCompression = 0`
/// e 24 ou 32 bits. Altura **negativa** é BMP de cima para baixo, e o sinal é o
/// único aviso: ignorá-lo desenha a folha de sprites de cabeça para baixo, o que
/// num puff radial não aparece e num risco aparece.
///
/// Devolve `None` em vez de entrar em pânico: um arquivo que o usuário apontou
/// por engano não pode derrubar o jogo, e a queda para o atlas procedural é o
/// comportamento certo.
pub fn ler_bmp(dados: &[u8]) -> Option<Imagem> {
    if dados.len() < 54 || &dados[0..2] != b"BM" {
        return None;
    }
    let u32_em = |i: usize| -> u32 {
        u32::from_le_bytes([dados[i], dados[i + 1], dados[i + 2], dados[i + 3]])
    };
    let i32_em = |i: usize| -> i32 {
        i32::from_le_bytes([dados[i], dados[i + 1], dados[i + 2], dados[i + 3]])
    };
    let u16_em = |i: usize| -> u16 { u16::from_le_bytes([dados[i], dados[i + 1]]) };

    let deslocamento = u32_em(10) as usize;
    let largura = i32_em(18);
    let altura_com_sinal = i32_em(22);
    let bits = u16_em(28);
    let compressao = u32_em(30);

    if largura <= 0 || altura_com_sinal == 0 {
        return None;
    }
    if compressao != 0 || (bits != 24 && bits != 32) {
        return None;
    }

    let de_cima = altura_com_sinal < 0;
    let altura = altura_com_sinal.unsigned_abs();
    let largura = largura as u32;
    let bpp = (bits / 8) as usize;
    // Cada linha é alinhada em 4 bytes no BMP. Sem esse passo a imagem sai
    // **cisalhada**: cada linha começa deslocada um pouco mais que a anterior.
    let passo = ((largura as usize * bpp + 3) / 4) * 4;
    if deslocamento + passo * altura as usize > dados.len() {
        return None;
    }

    let mut pixels = vec![255u8; (largura * altura * 4) as usize];
    for y in 0..altura as usize {
        let linha = if de_cima {
            y
        } else {
            altura as usize - 1 - y
        };
        for x in 0..largura as usize {
            let i = deslocamento + linha * passo + x * bpp;
            let o = (y * largura as usize + x) * 4;
            // BMP guarda BGR(A); o atlas é RGBA.
            pixels[o] = dados[i + 2];
            pixels[o + 1] = dados[i + 1];
            pixels[o + 2] = dados[i];
            pixels[o + 3] = if bpp == 4 { dados[i + 3] } else { 255 };
        }
    }
    Some(Imagem {
        largura,
        altura,
        pixels,
    })
}

/// Converte uma folha de sprites externa no **layout canônico** deste atlas.
///
/// A folha de origem é dividida em `colunas` colunas e `ceil(celulas/colunas)`
/// linhas; a célula *i* vai para a posição `Forma::TODAS[i].indice()` do atlas
/// resultante, reamostrada para [`CELULA`]×[`CELULA`].
///
/// A normalização existe para o resto do código não precisar saber de onde a
/// folha veio: `retangulo()`, `quad_celula()` e o desenho instanciado continuam
/// falando de um 128×128 com quatro colunas. Uma folha 8×1 em faixa entra
/// igualzinha a uma 4×2.
///
/// O RGB da origem é **descartado** e substituído por branco, porque a cor vem
/// do `tint`. O alfa vem do próprio alfa (se 32 bits) ou da **luminância** (se
/// 24 bits): uma folha de sprites salva em 24 bits perdeu o canal alfa, e o que
/// sobrou de útil é o que está claro — o branco vira opaco e o preto vira
/// transparente, que é exatamente a convenção das folhas de efeito.
pub fn normalizar_folha(folha: &Imagem, colunas: u32) -> Option<Vec<u8>> {
    if colunas == 0 || folha.largura < colunas || folha.altura == 0 {
        return None;
    }
    // `linhas` conta as **formas**, não as células do atlas: o atlas tem 16
    // células (4×4) e só 8 são usadas. Usar `CELULAS` aqui dividia a altura da
    // folha por 4 em vez de 2, e cada célula amostrava a faixa errada — medido:
    // a célula 4 lia o sprite 0, e o teste de layout acusou em 121 contra 157.
    let linhas = (Forma::TODAS.len() as u32).div_ceil(colunas);
    if folha.altura < linhas {
        return None;
    }
    let largura_celula = folha.largura / colunas;
    let altura_celula = folha.altura / linhas;
    if largura_celula == 0 || altura_celula == 0 {
        return None;
    }

    let mut pixels = vec![255u8; (LADO * LADO * 4) as usize];
    for (i, byte) in pixels.iter_mut().enumerate() {
        if i % 4 == 3 {
            *byte = 0;
        }
    }

    for forma in Forma::TODAS {
        let origem = forma.indice();
        let coluna_origem = origem as u32 % colunas;
        let linha_origem = origem as u32 / colunas;
        let (coluna_alvo, linha_alvo) = (origem as u32 % COLUNAS, origem as u32 / COLUNAS);
        for y in 0..CELULA {
            for x in 0..CELULA {
                // Reamostragem por vizinho mais próximo: a célula de origem tem
                // outro tamanho, e interpolar alfa entre dois sprites da folha
                // misturaria formas vizinhas na borda.
                let sx = coluna_origem * largura_celula + x * largura_celula / CELULA;
                let sy = linha_origem * altura_celula + y * altura_celula / CELULA;
                let o = ((sy * folha.largura + sx) * 4) as usize;
                let a = if folha.pixels[o + 3] < 255 {
                    folha.pixels[o + 3]
                } else {
                    // 24 bits (ou alfa todo opaco): luminância como máscara.
                    ((u32::from(folha.pixels[o]) * 299
                        + u32::from(folha.pixels[o + 1]) * 587
                        + u32::from(folha.pixels[o + 2]) * 114)
                        / 1000) as u8
                };
                let p = ((linha_alvo * CELULA + y) * LADO + (coluna_alvo * CELULA + x)) * 4;
                pixels[p as usize + 3] = a;
            }
        }
    }
    Some(pixels)
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

/// De onde vieram os pixels do atlas.
///
/// Existe porque "gerou" e "carregou a sua folha" desenham igual no painel e
/// diferente no diagnóstico: um `GIOCHI_SPRITES` com o caminho errado cairia no
/// atlas procedural **em silêncio**, e o usuário concluiria que a folha dele não
/// serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    /// Formas procedurais (o padrão).
    Procedural,
    /// Folha externa em BMP.
    Folha,
}

impl Origem {
    /// Nome estável, para o log.
    pub fn nome(self) -> &'static str {
        match self {
            Origem::Procedural => "procedural",
            Origem::Folha => "folha externa (BMP)",
        }
    }
}

/// Monta o atlas a partir de `caminho`, caindo no procedural quando não der.
///
/// `colunas` é quantas colunas a folha tem; `None` significa "não carregue
/// nada". Devolve também o motivo da queda, para o chamador poder **avisar** em
/// vez de mostrar sprites errados sem explicação.
pub fn carregar_ou_gerar(caminho: Option<&str>, colunas: u32) -> (Vec<u8>, Origem, Option<String>) {
    let Some(caminho) = caminho else {
        return (gerar(), Origem::Procedural, None);
    };
    let dados = match std::fs::read(caminho) {
        Ok(dados) => dados,
        Err(e) => {
            return (
                gerar(),
                Origem::Procedural,
                Some(format!("não consegui abrir {caminho}: {e}")),
            )
        }
    };
    let Some(imagem) = ler_bmp(&dados) else {
        return (
            gerar(),
            Origem::Procedural,
            Some(format!(
                "{caminho} não é BMP sem compressão de 24 ou 32 bits"
            )),
        );
    };
    match normalizar_folha(&imagem, colunas) {
        Some(pixels) => (pixels, Origem::Folha, None),
        None => (
            gerar(),
            Origem::Procedural,
            Some(format!(
                "folha {}x{} não cabe em {colunas} colunas × {} linhas",
                imagem.largura,
                imagem.altura,
                (Forma::TODAS.len() as u32).div_ceil(colunas.max(1))
            )),
        ),
    }
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

    /// Monta um BMP 24 bits de `largura`×`altura` com um pixel por `cor(x, y)`.
    ///
    /// Escrever o BMP no teste em vez de guardar um arquivo binário no
    /// repositório é o mesmo raciocínio das texturas: um `.bmp` de teste é
    /// opaco na revisão, e ninguém vê o que ele contém.
    fn bmp_24(largura: u32, altura: u32, de_cima: bool, cor: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let passo = ((largura as usize * 3 + 3) / 4) * 4;
        let mut dados = vec![0u8; 54 + passo * altura as usize];
        dados[0..2].copy_from_slice(b"BM");
        dados[10..14].copy_from_slice(&54u32.to_le_bytes());
        dados[18..22].copy_from_slice(&(largura as i32).to_le_bytes());
        let h = if de_cima {
            -(altura as i32)
        } else {
            altura as i32
        };
        dados[22..26].copy_from_slice(&h.to_le_bytes());
        dados[28..30].copy_from_slice(&24u16.to_le_bytes());
        // `cor(x, y)` é o pixel na **ordem do arquivo**, não na ordem da imagem:
        // escrever já invertido esconderia justamente a inversão que o leitor tem
        // de fazer.
        for y in 0..altura {
            for x in 0..largura {
                let [r, g, b] = cor(x, y);
                let i = 54 + y as usize * passo + x as usize * 3;
                dados[i] = b;
                dados[i + 1] = g;
                dados[i + 2] = r;
            }
        }
        dados
    }

    #[test]
    fn o_bmp_e_lido_de_baixo_para_cima_por_padrao() {
        // Convenção do formato, e a única coisa que separa "a folha está certa"
        // de "a folha está de cabeça para baixo". Num puff radial não aparece;
        // num risco de nitro aparece imediatamente.
        let dados = bmp_24(2, 2, false, |x, y| if y == 0 {
            [255, 0, 0]
        } else {
            [0, 0, 255]
        });
        let img = ler_bmp(&dados).expect("BMP válido");
        let px = |x: u32, y: u32| {
            let o = ((y * 2 + x) * 4) as usize;
            [img.pixels[o], img.pixels[o + 1], img.pixels[o + 2]]
        };
        // A linha 0 do arquivo (vermelha) é o **rodapé** da imagem, porque a
        // altura é positiva.
        assert_eq!(px(0, 1), [255, 0, 0], "a última linha da imagem devia ser a vermelha");
        assert_eq!(px(0, 0), [0, 0, 255]);
    }

    #[test]
    fn altura_negativa_inverte_a_leitura() {
        let dados = bmp_24(2, 2, true, |_, y| if y == 0 {
            [255, 0, 0]
        } else {
            [0, 0, 255]
        });
        let img = ler_bmp(&dados).expect("BMP válido");
        let o = 0;
        assert_eq!([img.pixels[o], img.pixels[o + 1], img.pixels[o + 2]], [255, 0, 0]);
    }

    #[test]
    fn arquivo_que_nao_e_bmp_devolve_none_em_vez_de_panico() {
        // Um caminho apontado por engano não pode derrubar o jogo — a queda para
        // o atlas procedural é o comportamento certo, e ela precisa do `None`.
        assert!(ler_bmp(b"").is_none());
        assert!(ler_bmp(b"GIF89a e o resto").is_none());
        // BMP truncado no meio dos pixels.
        let mut dados = bmp_24(8, 8, false, |_, _| [0, 0, 0]);
        dados.truncate(80);
        assert!(ler_bmp(&dados).is_none());
        // Comprimido (BI_RLE8): aceitar e ler como se fosse cru desenharia lixo.
        let mut comprimido = bmp_24(4, 4, false, |_, _| [0, 0, 0]);
        comprimido[30..34].copy_from_slice(&1u32.to_le_bytes());
        assert!(ler_bmp(&comprimido).is_none());
    }

    #[test]
    fn a_folha_externa_cai_no_mesmo_layout_de_quatro_colunas() {
        // O ponto inteiro da normalização: uma faixa 8×1 tem de produzir o
        // **mesmo** atlas que uma folha 4×2, senão `retangulo()` (que assume
        // quatro colunas) desenha a forma errada.
        let cor_da_celula = |i: u32| -> [u8; 3] { [(i * 30) as u8, 200, 40] };
        let faixa = bmp_24(8 * 8, 8, true, |x, _| cor_da_celula(x / 8));
        let img = ler_bmp(&faixa).expect("faixa");
        let atlas_faixa = normalizar_folha(&img, 8).expect("faixa 8x1 normaliza");

        let grade = bmp_24(4 * 8, 2 * 8, true, |x, y| {
            cor_da_celula(x / 8 + (y / 8) * 4)
        });
        let img = ler_bmp(&grade).expect("grade");
        let atlas_grade = normalizar_folha(&img, 4).expect("grade 4x2 normaliza");

        if atlas_faixa != atlas_grade {
            let i = atlas_faixa
                .iter()
                .zip(&atlas_grade)
                .position(|(a, b)| a != b)
                .unwrap();
            let px = i / 4;
            panic!(
                "primeiro byte diferente no índice {i} (x={}, y={}, canal={}): faixa={} grade={}",
                px % LADO as usize,
                px / LADO as usize,
                i % 4,
                atlas_faixa[i],
                atlas_grade[i]
            );
        }
        assert_eq!(atlas_faixa.len(), (LADO * LADO * 4) as usize);
    }

    #[test]
    fn a_folha_sem_alfa_usa_a_luminancia_como_mascara() {
        // Folha salva em 24 bits perdeu o alfa. O que sobrou de útil é o claro:
        // branco vira opaco, preto vira transparente — a convenção das folhas de
        // efeito. Sem isso, todo sprite de uma folha 24 bits sairia como um
        // retângulo cheio.
        // 4 colunas × 2 linhas de células: a de cima branca, a de baixo preta.
        let folha = bmp_24(4 * 8, 2 * 8, true, |_, y| {
            if y < 8 {
                [255, 255, 255]
            } else {
                [0, 0, 0]
            }
        });
        let img = ler_bmp(&folha).expect("BMP");
        let atlas = normalizar_folha(&img, 4).expect("normaliza");
        let alfa = |x: u32, y: u32| atlas[((y * LADO + x) * 4 + 3) as usize];
        // Célula 0 veio do branco.
        assert_eq!(alfa(2, 2), 255, "o branco devia virar opaco");
        // Célula 4 (segunda linha do atlas) veio do preto.
        assert_eq!(alfa(2, CELULA + 2), 0, "o preto devia virar transparente");
    }

    #[test]
    fn folha_menor_que_o_numero_de_celulas_e_recusada() {
        // Recusar é melhor que esticar: uma folha de 3 sprites com 8 formas
        // pedidas desenharia três formas e cinco vazias, e o sintoma seria
        // "metade dos efeitos sumiu".
        let folha = bmp_24(8, 8, false, |_, _| [255, 255, 255]);
        let img = ler_bmp(&folha).expect("BMP");
        assert!(normalizar_folha(&img, 0).is_none(), "zero colunas");
        assert!(normalizar_folha(&img, 9).is_none(), "mais colunas que pixels");
        // Uma linha de imagem não comporta duas linhas de célula.
        let baixa = bmp_24(2 * 8, 1, true, |_, _| [255, 255, 255]);
        let img = ler_bmp(&baixa).expect("BMP");
        assert!(
            normalizar_folha(&img, 2).is_none(),
            "uma imagem de 1 px de altura não tem as 4 linhas que 8 formas pedem em 2 colunas"
        );
    }

    #[test]
    fn caminho_inexistente_cai_no_procedural_e_diz_por_que() {
        let (pixels, origem, problema) =
            carregar_ou_gerar(Some("/caminho/que/nao/existe.bmp"), 4);
        assert_eq!(origem, Origem::Procedural);
        assert_eq!(pixels, gerar());
        let motivo = problema.expect("o motivo tem de vir junto");
        assert!(motivo.contains("não consegui abrir"), "motivo: {motivo}");
    }

    #[test]
    fn sem_caminho_nenhum_o_atlas_e_o_procedural_e_sem_aviso() {
        // O caso normal não pode imprimir aviso: um jogo que reclama do que não
        // foi pedido ensina a ignorar o log.
        let (pixels, origem, problema) = carregar_ou_gerar(None, 4);
        assert_eq!(origem, Origem::Procedural);
        assert!(problema.is_none());
        assert_eq!(pixels, gerar());
    }

    #[test]
    fn gerar_duas_vezes_da_os_mesmos_bytes() {
        // Determinismo: a comparação de quadro PC × aparelho depende de o atlas
        // ser o mesmo nos dois. Não há sorteio aqui, mas o teste fica para o dia
        // em que alguém acrescentar ruído a uma forma.
        assert_eq!(gerar(), gerar());
    }
}
