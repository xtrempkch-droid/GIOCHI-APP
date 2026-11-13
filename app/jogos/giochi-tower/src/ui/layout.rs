//! Layout do HUD em **pixels** e a conversão para lotes de desenho.
//!
//! # Por que em pixels e não em unidades de HUD
//!
//! A conversão para as unidades da câmera de 90° é uma linha e acontece no
//! desenho ([`matriz_de_pixels`]). O layout fica em pixels porque é nessa
//! unidade que as perguntas reais se fazem: "o rótulo da torre cabe ao lado do
//! placar?", "o minimapa invade o rodapé?". Um layout em unidades relativas
//! responderia as duas, mas ninguém as escreveria assim.
//!
//! # Escala proporcional ao **menor** lado
//!
//! Não à altura. Num painel retrato (720×1280) escalar pela altura daria letra
//! de 37 px e os blocos do topo se cruzariam. O menor lado é o que de fato
//! limita o espaço horizontal — a mesma regra que o `launcher` aprendeu com o
//! `draw::projecao` e o `giochi-defender` registrou no próprio HUD.
//!
//! # Nada se sobrepõe, e há teste
//!
//! Os blocos são funções com nome ([`faixa_topo`], [`barra_da_torre`],
//! [`bloco_rodape`], [`minimapa`]) para que um teste meça a **caixa** de cada um
//! em quatro tamanhos e reprove quando dois se tocarem. No `neon-drift` o rodapé
//! já empilhou velocidade, marcha e barra na mesma faixa de 40 px, e o texto
//! ficou ilegível numa tela de 3,5".

use super::font;
use super::UiFrame;

/// Altura de uma linha de texto, em pixels, para uma escala.
fn altura_da_linha(escala: f32) -> f32 {
    font::ALTURA as f32 * escala
}

/// Um retângulo em pixels da tela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caixa {
    /// Canto esquerdo.
    pub x: f32,
    /// Topo.
    pub y: f32,
    /// Largura.
    pub w: f32,
    /// Altura.
    pub h: f32,
}

impl Caixa {
    /// Constrói uma caixa.
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Direita.
    pub fn direita(&self) -> f32 {
        self.x + self.w
    }

    /// Base.
    pub fn base(&self) -> f32 {
        self.y + self.h
    }

    /// As duas caixas se tocam (borda compartilhada **conta** como toque).
    pub fn cruza(&self, outra: &Caixa) -> bool {
        self.x < outra.direita()
            && outra.x < self.direita()
            && self.y < outra.base()
            && outra.y < self.base()
    }

    /// A caixa está inteira dentro de uma tela de `tela`.
    pub fn dentro_de(&self, tela: Tela) -> bool {
        self.x >= -0.5
            && self.y >= -0.5
            && self.direita() <= tela.largura + 0.5
            && self.base() <= tela.altura + 0.5
    }
}

/// Um texto posicionado, com o canto superior esquerdo em pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Texto {
    /// O que escrever. **Já normalizado** pela [`font::normalizar`].
    pub conteudo: String,
    /// Canto esquerdo, em pixels.
    pub x: f32,
    /// Topo, em pixels.
    pub y: f32,
    /// Pixels por ponto do glifo.
    pub escala: f32,
    /// Cor com alfa.
    pub cor: [f32; 4],
}

impl Texto {
    /// Cria um texto já normalizado.
    pub fn new(conteudo: impl AsRef<str>, x: f32, y: f32, escala: f32, cor: [f32; 4]) -> Self {
        Self {
            conteudo: font::normalizar(conteudo.as_ref()),
            x,
            y,
            escala,
            cor,
        }
    }

    /// Largura ocupada, em pixels.
    pub fn largura(&self) -> f32 {
        font::largura_texto(&self.conteudo) as f32 * self.escala
    }

    /// Altura ocupada, em pixels.
    pub fn altura(&self) -> f32 {
        altura_da_linha(self.escala)
    }

    /// Caixa ocupada.
    pub fn caixa(&self) -> Caixa {
        Caixa::new(self.x, self.y, self.largura(), self.altura())
    }

    /// Direita do texto.
    pub fn direita(&self) -> f32 {
        self.x + self.largura()
    }
}

/// Um retângulo cheio — fundo de painel, trilha de barra, ponto do minimapa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    /// Caixa em pixels.
    pub caixa: Caixa,
    /// Cor com alfa.
    pub cor: [f32; 4],
}

impl Retangulo {
    /// Cria um retângulo a partir da caixa.
    pub fn new(caixa: Caixa, cor: [f32; 4]) -> Self {
        Self { caixa, cor }
    }
}

/// Tudo o que o HUD desenha num quadro.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Plano {
    /// Retângulos, desenhados antes dos textos.
    pub retangulos: Vec<Retangulo>,
    /// Textos, em ordem de desenho.
    pub textos: Vec<Texto>,
}

impl Plano {
    /// Acrescenta outra parte do plano a esta.
    pub fn juntar(&mut self, outra: Plano) {
        self.retangulos.extend(outra.retangulos);
        self.textos.extend(outra.textos);
    }

    /// A caixa que envolve tudo o que foi desenhado.
    ///
    /// `None` num plano vazio: um retângulo de área zero em `(0,0)` passaria por
    /// "cabe na tela" e esconderia o plano que não foi montado.
    pub fn caixa(&self) -> Option<Caixa> {
        let mut caixas = self
            .retangulos
            .iter()
            .map(|r| r.caixa)
            .chain(self.textos.iter().map(|t| t.caixa()));
        let primeira = caixas.next()?;
        Some(caixas.fold(primeira, |acc, c| {
            let x = acc.x.min(c.x);
            let y = acc.y.min(c.y);
            Caixa::new(
                x,
                y,
                acc.direita().max(c.direita()) - x,
                acc.base().max(c.base()) - y,
            )
        }))
    }
}

/// Tamanho da tela em pixels, como o desenho o vê.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tela {
    /// Largura desenhável.
    pub largura: f32,
    /// Altura desenhável.
    pub altura: f32,
}

impl Tela {
    /// Constrói a partir do tamanho do drawable.
    pub fn new(largura: u32, altura: u32) -> Self {
        Self {
            largura: largura.max(1) as f32,
            altura: altura.max(1) as f32,
        }
    }

    /// O menor lado, com piso 1 para não degenerar numa janela de 0 px.
    pub fn menor_lado(&self) -> f32 {
        self.largura.min(self.altura).max(1.0)
    }
}

/// Pixels por ponto do glifo.
///
/// Proporcional ao menor lado com piso 1,0: 480p dá 2,0 (letra de 10×14 px),
/// 720p dá 3,0. O piso existe porque num painel minúsculo a letra viraria pó em
/// vez de continuar legível.
pub fn escala(tela: Tela) -> f32 {
    (tela.menor_lado() / 240.0).max(1.0)
}

/// Distância das bordas, em pixels.
pub fn margem(tela: Tela) -> f32 {
    (tela.menor_lado() * 0.045).round().max(6.0)
}

/// Espaçamento curto (4 pontos de glifo).
fn passo(tela: Tela) -> f32 {
    (escala(tela) * 4.0).round().max(3.0)
}

/// Altura de uma barra de vida.
fn altura_da_barra(tela: Tela) -> f32 {
    (escala(tela) * 4.0).round().max(4.0)
}

/// Faixa do topo: onda, relógio e gold.
pub fn faixa_topo(tela: Tela) -> Caixa {
    let m = margem(tela);
    Caixa::new(
        m,
        m,
        tela.largura - 2.0 * m,
        altura_da_linha(escala(tela)),
    )
}

/// Barra da torre, logo abaixo da faixa do topo.
pub fn barra_da_torre(tela: Tela) -> Caixa {
    let m = margem(tela);
    Caixa::new(
        m,
        faixa_topo(tela).base() + passo(tela),
        tela.largura - 2.0 * m,
        altura_da_barra(tela),
    )
}

/// Rótulo sob a barra da torre (HP da torre à esquerda, placar à direita).
pub fn rotulo_da_torre(tela: Tela) -> Caixa {
    let m = margem(tela);
    let y = barra_da_torre(tela).base() + (passo(tela) * 0.5).round();
    Caixa::new(
        m,
        y,
        tela.largura - 2.0 * m,
        altura_da_linha(escala(tela)),
    )
}

/// Faixa central dos avisos (boss e Coronel): duas linhas no máximo.
pub fn faixa_de_avisos(tela: Tela) -> Caixa {
    let topo = rotulo_da_torre(tela).base() + passo(tela);
    let m = margem(tela);
    Caixa::new(
        m,
        topo,
        tela.largura - 2.0 * m,
        2.0 * altura_da_linha(escala(tela)) + (passo(tela) * 0.5).round(),
    )
}

/// Bloco do rodapé, encostado à esquerda: HP, barra e fase.
pub fn bloco_rodape(tela: Tela) -> Caixa {
    let m = margem(tela);
    let linha = altura_da_linha(escala(tela));
    let barra = altura_da_barra(tela);
    // Montado **de baixo para cima**: a fase encosta na margem inferior e cada
    // degrau sobe pela altura real do anterior. Somar deslocamentos fixos é o
    // que empilhou três elementos na mesma faixa no `neon-drift`.
    let fase_y = tela.altura - m - linha;
    let barra_y = fase_y - passo(tela) - barra;
    let hp_y = barra_y - (passo(tela) * 0.5).round() - linha;
    Caixa::new(
        m,
        hp_y,
        (tela.menor_lado() * 0.42).round().max(120.0),
        tela.altura - m - hp_y,
    )
}

/// Minimapa quadrado no canto inferior direito.
pub fn minimapa(tela: Tela) -> Caixa {
    let m = margem(tela);
    let lado = (tela.menor_lado() * 0.22).round().max(48.0);
    Caixa::new(tela.largura - m - lado, tela.altura - m - lado, lado, lado)
}

/// Converte uma coordenada normalizada do minimapa (`-1..=1`) em pixels.
pub fn minimapa_pixel(normalizado: [f32; 2], area: Caixa) -> (f32, f32) {
    let x = area.x + (normalizado[0].clamp(-1.0, 1.0) + 1.0) * 0.5 * area.w;
    // `+z` do mundo é "para baixo" no mapa, como no desenho do chão.
    let y = area.y + (normalizado[1].clamp(-1.0, 1.0) + 1.0) * 0.5 * area.h;
    (x, y)
}

/// Menor escala que faz `maior_largura_em_pontos` caber em `disponivel` pixels.
///
/// Nunca devolve zero nem negativo: um texto que não cabe em escala nenhuma sai
/// com a escala mínima e transborda, o que é **visível** — multiplicar por zero
/// faria o texto sumir e o defeito viraria silêncio.
pub fn escala_que_cabe(maior_largura_em_pontos: i32, disponivel: f32, maxima: f32) -> f32 {
    if maior_largura_em_pontos <= 0 {
        return maxima;
    }
    (disponivel / maior_largura_em_pontos as f32)
        .min(maxima)
        .max(0.5)
}

/// Monta o plano de um quadro de interface.
pub fn montar(frame: &UiFrame, tela: Tela) -> Plano {
    let mut plano = Plano::default();
    let e = escala(tela);
    let h = &frame.hud;

    plano.juntar(hud(h, tela, e));
    plano.juntar(barra_do_minimapa(&frame.minimap, tela));
    if frame.shop.open && !frame.shop.lines.is_empty() {
        plano.juntar(loja(&frame.shop, tela, e));
    }
    plano
}

/// O HUD propriamente dito: topo, barra da torre, avisos e rodapé.
fn hud(h: &super::Hud, tela: Tela, e: f32) -> Plano {
    let mut plano = Plano::default();
    let linha = altura_da_linha(e);

    // --- faixa do topo: onda à esquerda, relógio ao centro, gold à direita ---
    let topo = faixa_topo(tela);
    plano
        .textos
        .push(Texto::new(format!("ONDA {}", h.wave_label), topo.x, topo.y, e, super::COR_TEXTO));

    let relogio = Texto::new(h.clock.clone(), 0.0, topo.y, e, super::COR_DESTAQUE);
    let x = topo.x + (topo.w - relogio.largura()) * 0.5;
    plano.textos.push(Texto::new(h.clock.clone(), x, topo.y, e, super::COR_DESTAQUE));

    let ouro = Texto::new(format!("{} G", h.gold), 0.0, topo.y, e, super::COR_OURO);
    let x = topo.direita() - ouro.largura();
    plano
        .textos
        .push(Texto::new(format!("{} G", h.gold), x, topo.y, e, super::COR_OURO));

    // --- barra da torre ---
    let barra = barra_da_torre(tela);
    plano.retangulos.push(Retangulo::new(barra, super::COR_TRILHA));
    let fracao = h.tower_hp_frac.clamp(0.0, 1.0);
    // Zero de vida não gera retângulo de largura zero: o `lotes` o descartaria,
    // e um plano que carrega dado que não desenha mente sobre o que foi pedido.
    if fracao > 0.0 {
        plano.retangulos.push(Retangulo::new(
            Caixa::new(barra.x, barra.y, barra.w * fracao, barra.h),
            super::cor_da_barra(fracao, super::COR_TORRE),
        ));
    }

    let rotulo = rotulo_da_torre(tela);
    plano.textos.push(Texto::new(
        format!("TORRE {}", h.tower_hp_text),
        rotulo.x,
        rotulo.y,
        e,
        super::COR_TEXTO,
    ));
    let pontos = Texto::new(format!("{} PTS", h.score), 0.0, rotulo.y, e, super::COR_DESTAQUE);
    let x = rotulo.direita() - pontos.largura();
    plano.textos.push(Texto::new(
        format!("{} PTS", h.score),
        x,
        rotulo.y,
        e,
        super::COR_DESTAQUE,
    ));

    // --- avisos: boss e Coronel, centralizados ---
    let avisos = faixa_de_avisos(tela);
    let mut y = avisos.y;
    let mut desenhar_aviso = |texto: &str, cor: [f32; 4], y: &mut f32| {
        let item = Texto::new(texto, 0.0, *y, e, cor);
        let x = avisos.x + (avisos.w - item.largura()).max(0.0) * 0.5;
        plano.textos.push(Texto::new(texto, x, *y, e, cor));
        *y += linha + (passo(tela) * 0.5).round();
    };
    if let Some(boss) = &h.boss_line {
        desenhar_aviso(boss, super::COR_ALERTA, &mut y);
    }
    if let Some(nemesis) = &h.nemesis_line {
        desenhar_aviso(nemesis, super::COR_NEMESIS, &mut y);
    }

    // --- rodapé: HP, barra e fase ---
    let rodape = bloco_rodape(tela);
    plano.textos.push(Texto::new(
        format!("HP {}", h.player_hp_text),
        rodape.x,
        rodape.y,
        e,
        super::COR_TEXTO,
    ));
    let barra_hp_y = rodape.y + linha + (passo(tela) * 0.5).round();
    let barra_hp = Caixa::new(
        rodape.x,
        barra_hp_y,
        rodape.w,
        altura_da_barra(tela),
    );
    plano.retangulos.push(Retangulo::new(barra_hp, super::COR_TRILHA));
    let fracao = h.player_hp_frac.clamp(0.0, 1.0);
    if fracao > 0.0 {
        plano.retangulos.push(Retangulo::new(
            Caixa::new(barra_hp.x, barra_hp.y, barra_hp.w * fracao, barra_hp.h),
            super::cor_da_barra(fracao, super::COR_VIDA),
        ));
    }
    plano.textos.push(Texto::new(
        format!("{} : {} INIMIGO", h.phase, h.enemies),
        rodape.x,
        rodape.base() - linha,
        e,
        super::COR_TEXTO,
    ));

    plano
}

/// O minimapa: moldura, torre, aliados e o último avistamento do Coronel.
fn barra_do_minimapa(mapa: &super::Minimap, tela: Tela) -> Plano {
    let mut plano = Plano::default();
    let area = minimapa(tela);
    let e = escala(tela);
    let ponto = (e * 1.5).round().max(2.0);

    plano.retangulos.push(Retangulo::new(area, super::COR_PAINEL));
    // Moldura em quatro tiras: a engine não tem contorno, e `draw_rect` por
    // retângulo é o que existe.
    let t = (e * 0.5).round().max(1.0);
    for caixa in [
        Caixa::new(area.x, area.y, area.w, t),
        Caixa::new(area.x, area.base() - t, area.w, t),
        Caixa::new(area.x, area.y, t, area.h),
        Caixa::new(area.direita() - t, area.y, t, area.h),
    ] {
        plano.retangulos.push(Retangulo::new(caixa, super::COR_MOLDURA));
    }

    let mut ponto_em = |normalizado: [f32; 2], cor: [f32; 4], lado: f32| {
        let (x, y) = minimapa_pixel(normalizado, area);
        plano.retangulos.push(Retangulo::new(
            Caixa::new(x - lado * 0.5, y - lado * 0.5, lado, lado),
            cor,
        ));
    };

    ponto_em(mapa.tower, super::COR_TORRE, ponto * 1.6);
    for aliado in &mapa.allies {
        ponto_em(*aliado, super::COR_VIDA, ponto);
    }
    if let Some(nemesis) = mapa.nemesis {
        ponto_em(nemesis, super::COR_NEMESIS, ponto * 1.3);
    }

    plano
}

/// A tela da loja: fundo escurecido, painel e as nove linhas.
fn loja(loja: &super::ShopScreen, tela: Tela, e: f32) -> Plano {
    let mut plano = Plano::default();

    // Fundo escurecido sobre o jogo: a loja pausa a ação, e a tela precisa dizer
    // isso sem esconder completamente onde o jogador está.
    plano.retangulos.push(Retangulo::new(
        Caixa::new(0.0, 0.0, tela.largura, tela.altura),
        super::COR_VEU,
    ));

    let m = margem(tela);
    let interno = (e * 6.0).round().max(6.0);
    let disponivel = tela.largura - 2.0 * m - 2.0 * interno;

    // Uma escala só para o bloco inteiro: linhas de tamanhos diferentes ficariam
    // tremendo a cada compra, porque o `{:<24}` muda de largura com o conteúdo.
    let maior = loja
        .lines
        .iter()
        .map(|l| font::largura_texto(&font::normalizar(l)))
        .chain(std::iter::once(font::largura_texto("LOJA - CIMA/BAIXO ESCOLHE, ATIRAR COMPRA")))
        .max()
        .unwrap_or(0);
    let e_loja = escala_que_cabe(maior, disponivel, e);

    let passo_loja = (altura_da_linha(e_loja) + (e_loja * 2.0).round()).max(1.0);
    let altura_painel = 2.0 * interno
        + altura_da_linha(e_loja) * 2.0
        + passo_loja * loja.lines.len() as f32
        + (e_loja * 6.0).round();
    let largura_painel = (maior as f32 * e_loja + 2.0 * interno)
        .min(tela.largura - 2.0 * m)
        .max(1.0);
    let painel = Caixa::new(
        ((tela.largura - largura_painel) * 0.5).round(),
        ((tela.altura - altura_painel) * 0.5).max(m),
        largura_painel,
        altura_painel,
    );
    plano
        .retangulos
        .push(Retangulo::new(painel, super::COR_PAINEL));

    let mut y = painel.y + interno;
    plano.textos.push(Texto::new(
        format!("LOJA - {} GOLD", loja.gold),
        painel.x + interno,
        y,
        e_loja,
        super::COR_OURO,
    ));
    y += altura_da_linha(e_loja) + (e_loja * 2.0).round();

    for (indice, texto) in loja.lines.iter().enumerate() {
        if indice == loja.cursor {
            // Marcador à esquerda em vez de prefixo no texto: um `> ` mudaria a
            // largura de **uma** linha e a escala do bloco inteiro junto.
            plano.retangulos.push(Retangulo::new(
                Caixa::new(
                    painel.x + interno * 0.5,
                    y,
                    (e_loja * 2.0).round().max(2.0),
                    altura_da_linha(e_loja),
                ),
                super::COR_DESTAQUE,
            ));
        }
        // O `[ok]`/`[sem gold]` já vem no texto; a cor é do marcador, não da
        // linha — pintar por conteúdo seria uma segunda verdade sobre o preço.
        plano.textos.push(Texto::new(
            texto,
            painel.x + interno * 1.5,
            y,
            e_loja,
            super::COR_TEXTO,
        ));
        y += passo_loja;
    }

    y += (e_loja * 4.0).round();
    plano.textos.push(Texto::new(
        "CIMA/BAIXO ESCOLHE, ATIRAR COMPRA",
        painel.x + interno,
        y,
        e_loja,
        super::COR_DESTAQUE,
    ));

    plano
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ClassId, GameConfig, ProfileId};
    use crate::game::state::{GameState, Phase};
    use std::path::Path;

    const PAINEIS: [(u32, u32); 4] = [(640, 480), (800, 600), (1280, 720), (720, 1280)];

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn partida(gold: u32) -> GameState {
        let c = config();
        let mut estado = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        estado.phase = Phase::Combat;
        estado.phase_timer_s = 122.0;
        estado.wave = 3;
        estado.local_mut().gold = gold;
        estado.local_mut().score = 70;
        estado
    }

    #[test]
    fn a_escala_cresce_com_o_painel_e_tem_piso() {
        assert_eq!(escala(Tela::new(640, 480)), 2.0);
        assert_eq!(escala(Tela::new(800, 600)), 2.5);
        assert_eq!(escala(Tela::new(1280, 720)), 3.0);
        // 240×240 daria 1,0; abaixo disso o piso segura a letra legível.
        assert_eq!(escala(Tela::new(160, 120)), 1.0);
    }

    #[test]
    fn os_blocos_do_hud_nao_se_sobrepoem_em_painel_nenhum() {
        for (l, a) in PAINEIS {
            let tela = Tela::new(l, a);
            let blocos = [
                ("topo", faixa_topo(tela)),
                ("barra da torre", barra_da_torre(tela)),
                ("rotulo da torre", rotulo_da_torre(tela)),
                ("avisos", faixa_de_avisos(tela)),
                ("rodape", bloco_rodape(tela)),
                ("minimapa", minimapa(tela)),
            ];
            for (i, (nome_a, a_caixa)) in blocos.iter().enumerate() {
                assert!(
                    a_caixa.dentro_de(tela),
                    "{l}x{a}: {nome_a} sai da tela: {a_caixa:?}"
                );
                for (nome_b, b_caixa) in blocos.iter().skip(i + 1) {
                    assert!(
                        !a_caixa.cruza(b_caixa),
                        "{l}x{a}: {nome_a} cruza {nome_b} ({a_caixa:?} × {b_caixa:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn o_hud_montado_cabe_na_tela_em_todos_os_paineis() {
        let c = config();
        let estado = partida(45);
        let frame = super::super::build(&estado, &c);
        for (l, a) in PAINEIS {
            let tela = Tela::new(l, a);
            let plano = montar(&frame, tela);
            let caixa = plano.caixa().expect("plano não vazio");
            assert!(caixa.dentro_de(tela), "{l}x{a}: plano sai da tela: {caixa:?}");
        }
    }

    #[test]
    fn a_loja_com_nove_itens_cabe_na_tela_de_640x480() {
        let c = config();
        let mut estado = partida(100);
        estado.phase = Phase::Shop;
        let frame = super::super::build(&estado, &c);
        assert!(frame.shop.open);
        let tela = Tela::new(640, 480);
        let plano = montar(&frame, tela);
        let caixa = plano.caixa().expect("plano");
        assert!(caixa.dentro_de(tela), "a loja transborda: {caixa:?}");
        // A linha mais longa tem de sobrar folga: sem ela o texto seria cortado
        // pela borda e o item mais caro ficaria ilegível.
        let maior = plano
            .textos
            .iter()
            .map(|t| t.direita())
            .fold(0.0_f32, f32::max);
        assert!(maior <= tela.largura, "texto passa da direita: {maior}");
    }

    #[test]
    fn o_minimapa_mapeia_o_centro_para_o_meio_da_caixa() {
        let area = Caixa::new(100.0, 50.0, 200.0, 200.0);
        let (x, y) = minimapa_pixel([0.0, 0.0], area);
        assert!((x - 200.0).abs() < 0.01, "{x}");
        assert!((y - 150.0).abs() < 0.01, "{y}");
        // Fora do mapa é grampeado: um inimigo no limite não pode desenhar fora
        // da moldura.
        let (x, y) = minimapa_pixel([4.0, -4.0], area);
        assert!((x - 300.0).abs() < 0.01, "{x}");
        assert!((y - 50.0).abs() < 0.01, "{y}");
    }

    #[test]
    fn todo_texto_do_hud_usa_caracteres_que_a_fonte_tem() {
        let c = config();
        let mut estado = partida(45);
        estado.phase = Phase::Shop;
        let frame = super::super::build(&estado, &c);
        for (l, a) in PAINEIS {
            let plano = montar(&frame, Tela::new(l, a));
            for texto in &plano.textos {
                for c in texto.conteudo.chars() {
                    assert!(
                        font::glifo(c).is_some(),
                        "{l}x{a}: {c:?} não existe na fonte, no texto {:?}",
                        texto.conteudo
                    );
                }
            }
        }
    }

    #[test]
    fn a_barra_cheia_ocupa_a_largura_toda_e_a_vazia_nao_desenha() {
        let c = config();
        let mut estado = partida(0);
        estado.tower.hp = estado.tower.max_hp;
        let tela = Tela::new(640, 480);
        let cheio = montar(&super::super::build(&estado, &c), tela);
        let barra = barra_da_torre(tela);
        let preenchida = cheio
            .retangulos
            .iter()
            .find(|r| (r.caixa.y - barra.y).abs() < 0.01 && r.caixa.w < barra.w + 0.01)
            .expect("barra da torre");
        assert!(
            (preenchida.caixa.w - barra.w).abs() < 0.01,
            "com HP cheio a barra tem de cobrir a trilha inteira: {:?}",
            preenchida.caixa
        );

        let mut vazio = partida(0);
        vazio.tower.hp = 0.0;
        let plano = montar(&super::super::build(&vazio, &c), tela);
        let zerada = plano
            .retangulos
            .iter()
            .filter(|r| (r.caixa.y - barra.y).abs() < 0.01)
            .count();
        assert_eq!(zerada, 1, "com HP zero só a trilha aparece");
    }

    #[test]
    fn a_caixa_de_um_plano_vazio_e_nenhuma() {
        assert!(Plano::default().caixa().is_none());
    }

    #[test]
    fn escala_que_cabe_nunca_chega_a_zero() {
        // Multiplicar por zero faria o texto sumir, e "não cabe" viraria
        // "não desenhou" — o defeito silencioso de sempre.
        assert!(escala_que_cabe(10_000, 100.0, 3.0) >= 0.5);
        assert_eq!(escala_que_cabe(0, 100.0, 2.0), 2.0);
        assert_eq!(escala_que_cabe(10, 1000.0, 2.0), 2.0, "não cresce além do teto");
    }
}
