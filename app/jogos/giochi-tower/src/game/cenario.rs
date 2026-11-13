//! O planeta morto: onde cada rocha, cratera, mesa e destroço fica.
//!
//! # Por que isto é dado, e não desenho
//!
//! O pedido de arte do dono é uma cena: soldados defendendo um posto numa horda
//! de bugs, num planeta **sem vegetação e com pedras espalhadas**. Isso é
//! composição — quantas pedras, de que tamanho, a que distância da torre —, e
//! composição escrita dentro do laço de desenho não tem como ser testada.
//!
//! Aqui é uma função pura: [`montar`] devolve as cópias com posição, altura,
//! giro e cor. O laço só agrupa e desenha.
//!
//! # Determinístico, e por um motivo medido
//!
//! O gerador de números é um xorshift com semente fixa, não `rand`. Assim dois
//! processos (PC × aparelho, duas execuções do CI) desenham **o mesmo campo** —
//! e é o que permite comparar quadro a quadro sem que a paisagem seja a
//! diferença. O mesmo motivo do passo fixo da simulação.
//!
//! # As regras de lugar
//!
//! | Regra | Por quê |
//! | --- | --- |
//! | nada a menos de [`RAIO_LIVRE_DA_TORRE_M`] da torre | a torre é o alvo; uma pedra na frente esconde a barra de HP e o próprio modelo |
//! | pedra **dentro** do campo é baixa ([`ALTURA_MAXIMA_NO_CAMPO_M`]) | o inimigo atravessa cenário (não há colisão), e uma rocha de 3 m no meio do corredor parece um bug atravessando pedra |
//! | mesa e bloco grande só **fora** do campo | o campo é 320×320 m e o `MapBounds` prende o jogador nele; cenário alto lá dentro taparia a câmera isométrica |
//! | o chão é maior que o campo ([`LADO_DO_CHAO_M`]) | sem isso as mesas do horizonte flutuariam no vazio — o plano antigo tinha exatamente o tamanho do campo |
//!
//! A borda do campo ganha um anel de rochas: é o que explica, sem texto, por que
//! o jogador não sai dali. O `MapBounds` já o prendia; agora dá para ver.

use glam::Vec3;

use crate::engine_bridge::cena::Copia;
use crate::game::camera::MapBounds;

/// Semente do campo. Trocar muda a paisagem inteira e não muda nada da jogabilidade.
pub const SEMENTE: u32 = 0x9E37_79B9;

/// Lado do plano do chão, em metros.
///
/// É **maior** que o campo de batalha de propósito: a câmera tem plano distante de
/// 420 m e o horizonte precisa de chão até lá. O plano antigo media exatamente o
/// campo (320 m) e o mundo terminava numa linha reta visível.
pub const LADO_DO_CHAO_M: f32 = 600.0;

/// Lado de uma repetição da textura de regolito, em metros.
///
/// # O número é medido, e o limite é do shader
///
/// A 3 m o chão ficava com uma **grade de quadrados** de 3 m: o `v_uv` chega a
/// `600/3 = 200` e o interpolador de UV do GLES 2 é `mediump` (10 bits de
/// mantissa), então a quantização da coordenada vira degrau visível a cada
/// repetição. Medido em `/tmp` comparando 3, 6, 8, 12 e 20 m: a grade some a
/// partir de ~12 m (50 repetições).
///
/// 12 m dá 1,4 cm por texel num BMP de 64×64 — a 10 px/m no painel de 640×480
/// isso é detalhe de sobra, e é o maior valor que ainda desenha grão em vez de
/// mancha.
pub const TILE_DO_CHAO_M: f32 = 12.0;

/// Raio em volta da torre onde nada é plantado.
pub const RAIO_LIVRE_DA_TORRE_M: f32 = 18.0;

/// Altura máxima de uma peça **dentro** do campo jogável.
pub const ALTURA_MAXIMA_NO_CAMPO_M: f32 = 1.1;

/// Quantas pedras pequenas ficam espalhadas pelo campo.
///
/// O número foi medido, não escolhido por gosto: a câmera cobre ~60×45 m, que é
/// 2,7 % do campo de 320×320 m. Com 34 pedras isso dava **menos de uma** pedra
/// no quadro — a "planície com pedras espalhadas" do pedido não aparecia. Com 320,
/// cada tela mostra 8–10, que é o que faz o chão ler como rochoso.
pub const PEDRAS_NO_CAMPO: usize = 320;

/// Quantas pedras formam o anel da borda.
pub const PEDRAS_NA_BORDA: usize = 96;

/// Quantas crateras de impacto.
pub const CRATERAS: usize = 34;

/// Altura da borda da cratera no modelo (o arquivo tem 0,06 m de borda num raio
/// de 1 m). É a constante que converte "diâmetro desejado" em "altura pedida",
/// porque `Copia` só escala pela altura.
pub const BORDA_DA_CRATERA_M: f32 = 0.06;

/// Quantas mesas no horizonte.
pub const MESAS: usize = 16;

/// Quantos destroços de nave.
pub const DESTROCOS: usize = 7;

/// Cores do cenário. São dados porque o caminho instanciado não amostra textura:
/// o que o jogador vê é esta cor multiplicada pela luz.
///
/// # Contraste contra o chão, e não "cor de pedra"
///
/// A primeira versão usava `0,46 0,40 0,34` — praticamente a cor do regolito
/// (`0,478 0,412 0,345`). O resultado medido no quadro foi **zero silhueta**: as
/// pedras estavam lá, com a mesma luminância do chão, e a tela lia como uma
/// planície vazia. Rocha tem de ser **basalto escuro** para recortar contra a
/// poeira clara.
pub const COR_ROCHA: [f32; 4] = [0.34, 0.30, 0.26, 1.0];
/// Espigão: basalto mais escuro que a rocha solta.
pub const COR_ESPIGAO: [f32; 4] = [0.25, 0.22, 0.19, 1.0];
/// Cratera: mais escura que o regolito, senão a bacia não lê como buraco.
pub const COR_CRATERA: [f32; 4] = [0.36, 0.31, 0.27, 1.0];
/// Mesa: silhueta do horizonte, mais clara que as pedras do campo.
///
/// Ela é longe e o ar tem poeira: se tivesse a mesma cor da rocha daqui, o
/// horizonte viraria uma parede de basalto. A distância lava a cor, e o que sobra
/// é a silhueta.
pub const COR_MESA: [f32; 4] = [0.50, 0.43, 0.38, 1.0];
/// Destroço: aço, o único material frio da cena.
pub const COR_DESTROCO: [f32; 4] = [0.42, 0.44, 0.47, 1.0];

/// Xorshift32 — determinístico, sem dependência e sem estado global.
#[derive(Debug, Clone, Copy)]
pub struct Sorteio(u32);

impl Sorteio {
    /// Um sorteio a partir de uma semente. `0` é proibido pelo xorshift.
    pub const fn novo(semente: u32) -> Self {
        Self(if semente == 0 { 0x1234_5678 } else { semente })
    }

    /// Próximo inteiro.
    pub fn proximo(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Um `f32` em `0.0..1.0`.
    pub fn fracao(&mut self) -> f32 {
        (self.proximo() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Um `f32` em `min..max`.
    pub fn entre(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.fracao()
    }
}

/// Uma cópia do cenário, com o nome do modelo.
fn copia(modelo: &'static str, x: f32, z: f32, altura: f32, giro: f32, cor: [f32; 4]) -> Copia {
    Copia {
        modelo,
        pos: Vec3::new(x, 0.0, z),
        altura,
        giro,
        cor,
    }
}

/// `true` se o ponto está fora do campo jogável (a borda exata conta como dentro).
fn fora_do_campo(x: f32, z: f32, limite: f32) -> bool {
    x.abs() > limite || z.abs() > limite
}

/// Um ponto no **perímetro do quadrado**, `recuo` metros para fora do campo.
///
/// # Por que não é `raio * (cos, sin)`
///
/// O campo é quadrado e o raio é circular: a 45°, um ponto de raio `160·1,15`
/// está a 130 m em cada eixo e cai **dentro** do campo. Foi exatamente assim que
/// um destroço e depois uma mesa nasceram no meio do campo — o `debug_assert` e o
/// teste pegaram, mas o defeito é de conta, não de dado. Ancorar no quadrado
/// resolve para qualquer `t` em `0..1`.
///
/// `lado` cicla os quatro lados; `t` anda ao longo de um deles.
fn ponto_no_perimetro(lado: usize, t: f32, recuo: f32, campo: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    match lado % 4 {
        0 => (-campo - recuo, -campo + t * 2.0 * campo),
        1 => (campo + recuo, -campo + t * 2.0 * campo),
        2 => (-campo + t * 2.0 * campo, -campo - recuo),
        _ => (-campo + t * 2.0 * campo, campo + recuo),
    }
}

/// Monta o cenário inteiro. Sempre na mesma ordem e sempre igual.
pub fn montar() -> Vec<Copia> {
    let campo = MapBounds::new().half_extent_m;
    let mut sorteio = Sorteio::novo(SEMENTE);
    let mut copias =
        Vec::with_capacity(PEDRAS_NO_CAMPO + PEDRAS_NA_BORDA + CRATERAS + MESAS + DESTROCOS);

    // ── Pedras pequenas dentro do campo ───────────────────────────────
    //
    // Baixas por regra: o inimigo não tem colisão com cenário, então uma pedra
    // alta aqui viraria um bug atravessando rocha. O que se procura é o chão
    // "salpicado", não obstáculo.
    let mut tentativas = 0;
    let mut no_campo = 0;
    while no_campo < PEDRAS_NO_CAMPO && tentativas < PEDRAS_NO_CAMPO * 40 {
        tentativas += 1;
        let x = sorteio.entre(-campo * 0.94, campo * 0.94);
        let z = sorteio.entre(-campo * 0.94, campo * 0.94);
        if (x * x + z * z).sqrt() < RAIO_LIVRE_DA_TORRE_M {
            continue;
        }
        // Duas faixas: seixos (a maioria, que dão textura ao chão) e blocos
        // baixos (os que se veem como rocha de longe). Nenhum passa do teto.
        let altura = if sorteio.fracao() < 0.72 {
            sorteio.entre(0.18, 0.5)
        } else {
            sorteio.entre(0.55, ALTURA_MAXIMA_NO_CAMPO_M)
        };
        let giro = sorteio.entre(0.0, std::f32::consts::TAU);
        copias.push(copia("rocha", x, z, altura, giro, COR_ROCHA));
        no_campo += 1;
    }

    // ── Anel de rochas na borda ───────────────────────────────────────
    //
    // Percorre o perímetro do quadrado em vez de sortear ângulo: um anel
    // circular deixaria os cantos vazios, e o campo é quadrado.
    let lados = 4;
    let por_lado = PEDRAS_NA_BORDA / lados;
    for lado in 0..lados {
        for i in 0..por_lado {
            let t = (i as f32 + 0.5) / por_lado as f32;
            let jitter = sorteio.entre(0.0, 8.0);
            let (x, z) = match lado {
                0 => (-campo - jitter, -campo + t * 2.0 * campo),
                1 => (campo + jitter, -campo + t * 2.0 * campo),
                2 => (-campo + t * 2.0 * campo, -campo - jitter),
                _ => (-campo + t * 2.0 * campo, campo + jitter),
            };
            let altura = sorteio.entre(1.6, 4.6);
            let giro = sorteio.entre(0.0, std::f32::consts::TAU);
            let modelo = if sorteio.fracao() < 0.18 { "espigao" } else { "rocha" };
            let cor = if modelo == "espigao" { COR_ESPIGAO } else { COR_ROCHA };
            copias.push(copia(modelo, x, z, altura, giro, cor));
        }
    }

    // ── Crateras de impacto ───────────────────────────────────────────
    //
    // A cratera é uma bacia de 1 m de raio e 0,06 m de borda no arquivo. A
    // escala vem da **altura** (é o único campo de `Copia`): para um diâmetro
    // `d`, a altura pedida é `0,06 · d/2`. A conta está aqui, escrita, e não
    // espalhada em números mágicos no laço.
    let mut crateras = 0;
    tentativas = 0;
    while crateras < CRATERAS && tentativas < CRATERAS * 40 {
        tentativas += 1;
        let x = sorteio.entre(-campo * 0.9, campo * 0.9);
        let z = sorteio.entre(-campo * 0.9, campo * 0.9);
        if (x * x + z * z).sqrt() < RAIO_LIVRE_DA_TORRE_M + 6.0 {
            continue;
        }
        let diametro = sorteio.entre(7.0, 19.0);
        let giro = sorteio.entre(0.0, std::f32::consts::TAU);
        copias.push(copia(
            "cratera",
            x,
            z,
            BORDA_DA_CRATERA_M * diametro * 0.5,
            giro,
            COR_CRATERA,
        ));
        crateras += 1;
    }

    // ── Destroços: a guerra já passou por aqui ────────────────────────
    //
    // Fora do campo, junto do anel de rochas. Um casco de 4 m **dentro** do
    // campo seria um obstáculo que o inimigo atravessa — e o inimigo atravessa
    // mesmo, porque não há colisão com cenário. Aqui ele fica no cinturão de
    // entulho em volta do posto, que é onde a leitura de "guerra" funciona.
    for i in 0..DESTROCOS {
        // Ancorado no **quadrado**, não num raio: a 45° um ponto de 170 m de raio
        // está a 120 m de cada eixo e cai dentro do campo. Foi o `debug_assert`
        // que pegou, e é por isso que ele está aqui.
        let t = sorteio.entre(0.12, 0.88);
        let recuo = sorteio.entre(6.0, 26.0);
        let (x, z) = ponto_no_perimetro(i, t, recuo, campo);
        debug_assert!(fora_do_campo(x, z, campo), "destroço dentro do campo: {x} {z}");
        let altura = sorteio.entre(2.6, 5.4);
        let giro = sorteio.entre(0.0, std::f32::consts::TAU);
        copias.push(copia("destroco", x, z, altura, giro, COR_DESTROCO));
    }

    // ── Mesas do horizonte ────────────────────────────────────────────
    //
    // Fora do campo e além do anel de rochas. São a silhueta que fecha o
    // cenário: sem elas o mundo termina numa borda reta de plano.
    for i in 0..MESAS {
        let por_lado = MESAS.div_ceil(4);
        let t = ((i % por_lado) as f32 + 0.5) / por_lado as f32 + sorteio.entre(-0.08, 0.08);
        // 30 a 70 m para fora da borda: fora do campo (o jogador não chega perto)
        // e **dentro** do plano distante de 900 m a partir do olho. A primeira
        // versão usava até 2,1× (672 m) e as mesas eram cortadas pelo plano de
        // corte — o sintoma foram retângulos escuros flutuando.
        let recuo = sorteio.entre(30.0, 70.0);
        let (x, z) = ponto_no_perimetro(i / por_lado.max(1), t, recuo, campo);
        debug_assert!(fora_do_campo(x, z, campo), "mesa dentro do campo: {x} {z}");
        // 10 a 26 m de altura: o modelo tem 1 m de altura e 2 m de base, então a
        // mesa sai com 20 a 52 m de largura. Alto o bastante para fechar o
        // horizonte, baixo o bastante para não virar parede.
        let altura = sorteio.entre(10.0, 26.0);
        let giro = sorteio.entre(0.0, std::f32::consts::TAU);
        copias.push(copia("mesa", x, z, altura, giro, COR_MESA));
    }

    copias
}

/// O cenário agrupado em lotes, pronto para desenhar.
///
/// Feito uma vez na abertura: o cenário não muda durante a partida, e reagrupar
/// por quadro alocaria `Vec`s à toa. O laço só percorre o resultado.
pub fn lotes() -> Vec<crate::engine_bridge::cena::Lote> {
    crate::engine_bridge::cena::agrupar(&montar())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_bridge::modelos::CENARIO;

    #[test]
    fn o_cenario_e_o_mesmo_em_toda_execucao() {
        // É o que permite comparar quadro do PC com quadro do aparelho: se a
        // paisagem mudasse entre processos, toda diferença seria atribuída a ela.
        let a = montar();
        let b = montar();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x, y, "cenário não determinístico em {x:?}");
        }
    }

    #[test]
    fn toda_peca_do_cenario_tem_modelo_no_catalogo() {
        for copia in montar() {
            assert!(
                CENARIO.contains(&copia.modelo),
                "'{}' não está em modelos::CENARIO",
                copia.modelo
            );
        }
    }

    #[test]
    fn nada_nasce_em_cima_da_torre() {
        // A torre é o alvo e a barra de HP fica acima dela: uma rocha aqui
        // esconde as duas coisas, e o jogador perde a referência do mapa.
        for copia in montar() {
            let d = (copia.pos.x * copia.pos.x + copia.pos.z * copia.pos.z).sqrt();
            assert!(
                d >= RAIO_LIVRE_DA_TORRE_M,
                "'{}' a {d:.1} m da torre (mínimo {RAIO_LIVRE_DA_TORRE_M})",
                copia.modelo
            );
        }
    }

    #[test]
    fn dentro_do_campo_so_ha_peca_baixa() {
        // O inimigo não colide com cenário. Uma rocha alta no corredor aparece
        // como um bug atravessando pedra — e isso o jogador lê como bug do jogo.
        let campo = MapBounds::new().half_extent_m;
        for copia in montar().iter().filter(|c| {
            // A cratera é um **buraco no chão**: a altura dela é a escala do
            // diâmetro, não a de um obstáculo. Quem pode esconder o inimigo é
            // pedra, espigão, destroço e mesa.
            c.modelo != "cratera" && !fora_do_campo(c.pos.x, c.pos.z, campo)
        }) {
            assert!(
                copia.altura <= ALTURA_MAXIMA_NO_CAMPO_M,
                "'{}' com {:.2} m dentro do campo: o teto é {ALTURA_MAXIMA_NO_CAMPO_M}",
                copia.modelo,
                copia.altura
            );
        }
    }

    #[test]
    fn as_mesas_do_horizonte_ficam_fora_do_campo() {
        // A câmera é isométrica e baixa: uma mesa de 40 m dentro do campo taparia
        // metade da tela. Elas são horizonte, não obstáculo.
        let campo = MapBounds::new().half_extent_m;
        let tudo = montar();
        let mesas: Vec<&Copia> = tudo.iter().filter(|c| c.modelo == "mesa").collect();
        assert_eq!(mesas.len(), MESAS, "mesas faltando: {}", mesas.len());
        for mesa in mesas {
            assert!(
                fora_do_campo(mesa.pos.x, mesa.pos.z, campo),
                "mesa dentro do campo em ({:.1}, {:.1})",
                mesa.pos.x,
                mesa.pos.z
            );
            assert!(
                mesa.altura >= 10.0,
                "mesa de {:.1} m some no horizonte",
                mesa.altura
            );
        }
    }

    #[test]
    fn o_anel_da_borda_cobre_os_quatro_lados() {
        // Um anel circular deixaria os cantos vazios e o campo quadrado apareceria
        // como um círculo de rochas com quatro buracos.
        let campo = MapBounds::new().half_extent_m;
        let tudo = montar();
        // O anel é feito de rocha **e** espigão (18 % dos lugares): contar só
        // "rocha" mediria o sorteio, não a cobertura da borda.
        let borda: Vec<&Copia> = tudo
            .iter()
            .filter(|c| (c.modelo == "rocha" || c.modelo == "espigao") && c.altura >= 1.5)
            .collect();
        assert!(
            borda.len() >= PEDRAS_NA_BORDA,
            "só {} pedra(s) de borda",
            borda.len()
        );

        // Conta quantas estão perto de cada lado do quadrado. As expressões são
        // repetidas de propósito: uma tabela de closures não unifica tipo, e
        // boxear `dyn Fn` num teste só esconde a conta.
        let limiar = 9.0;
        let esquerda = borda
            .iter()
            .filter(|c| c.pos.x < -campo && (-campo - c.pos.x) <= limiar)
            .count();
        let direita = borda
            .iter()
            .filter(|c| c.pos.x > campo && (c.pos.x - campo) <= limiar)
            .count();
        let frente = borda
            .iter()
            .filter(|c| c.pos.z < -campo && (-campo - c.pos.z) <= limiar)
            .count();
        let tras = borda
            .iter()
            .filter(|c| c.pos.z > campo && (c.pos.z - campo) <= limiar)
            .count();
        for (lado, n) in [("esquerdo", esquerda), ("direito", direita), ("frontal", frente), ("traseiro", tras)] {
            assert!(n >= 20, "lado {lado} tem só {n} pedras");
        }
    }

    #[test]
    fn as_crateras_tem_o_tamanho_pedido_e_nao_um_qualquer() {
        // A altura da cratera é derivada do diâmetro (0,06 · d/2). Se alguém
        // trocar a conta, a bacia vira uma tigela gigante ou some no chão — e
        // nenhum erro aparece.
        for copia in montar().iter().filter(|c| c.modelo == "cratera") {
            let diametro = copia.altura / BORDA_DA_CRATERA_M * 2.0;
            assert!(
                (7.0..=19.0).contains(&diametro),
                "cratera de diâmetro {diametro:.1} m (faixa 7..19)"
            );
        }
    }

    #[test]
    fn o_sorteio_e_deterministico_e_cobre_o_intervalo() {
        let mut a = Sorteio::novo(42);
        let mut b = Sorteio::novo(42);
        let mut menor = f32::INFINITY;
        let mut maior = f32::NEG_INFINITY;
        for _ in 0..1000 {
            let x = a.fracao();
            assert_eq!(x, b.fracao());
            assert!((0.0..1.0).contains(&x), "{x}");
            menor = menor.min(x);
            maior = maior.max(x);
        }
        assert!(menor < 0.05 && maior > 0.95, "{menor}..{maior}");
    }

    #[test]
    fn a_cena_inteira_cabe_em_poucos_lotes() {
        // Cada lote é uma chamada de desenho por quadro. O cenário inteiro tem
        // ~160 cópias; se cada uma virasse um lote, seria mais que a UI e a horda
        // somadas, no Cortex-A35.
        let lotes = lotes();
        assert!(
            lotes.len() <= CENARIO.len(),
            "{} lote(s) para {} modelo(s)",
            lotes.len(),
            CENARIO.len()
        );
        let copias: usize = lotes.iter().map(|l| l.matrizes.len()).sum();
        assert_eq!(copias, montar().len());
    }
}
