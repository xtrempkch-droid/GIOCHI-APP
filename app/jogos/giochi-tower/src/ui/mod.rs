//! Interface: HUD, minimapa e tela da loja (GDD §17).
//!
//! # Como a UI é desenhada sobre uma engine que só sabe 3D
//!
//! A GIOCHI-ENGINE-3D **não tem texto nem 2D**: `RenderContext3D` desenha malha
//! com `Camera3D` e não tem upload de textura (`DECISIONS.md`, D-009). A ponte é
//! a mesma que o `neon-drift` e o `giochi-defender` mediram: uma câmera na origem
//! olhando para `-Z` com `fov` de 90° e o HUD a [`PROFUNDIDADE`] unidades.
//!
//! Nessa configuração a conversão de pixel para mundo é exata:
//!
//! ```text
//! mundo_x = (px - largura/2) · 2·PROFUNDIDADE / altura
//! mundo_y = (altura/2 - py)  · 2·PROFUNDIDADE / altura
//! ```
//!
//! O mesmo divisor (a **altura**) nos dois eixos é o que mantém um quadrado
//! quadrado num painel 4:3 e num 16:9 — a escala é uniforme em pixels e o
//! aspecto entra só na câmera, por [`matriz_de_pixels`].
//!
//! # Três camadas, e cada uma responde uma pergunta
//!
//! | Camada | Pergunta | Precisa de GPU? |
//! |---|---|---|
//! | [`build`] | quais são os **números** da tela? | não |
//! | [`layout::montar`] | onde cada coisa fica, em **pixels**? | não |
//! | [`lotes`] | quais **retângulos** e com que cor? | não |
//!
//! Só `engine_bridge::render` toca o GL. Isso é o que permite testar "a loja
//! cabe em 640×480?" sem abrir janela — e a razão de o layout ter nascido antes
//! do desenho, em vez de depois.
//!
//! # Custo: uma chamada de desenho por cor
//!
//! Cada retângulo e cada ponto aceso de cada glifo é uma **instância** de um
//! único quadrilátero unitário; [`lotes`] agrupa por cor e o desenho emite uma
//! `draw_instanced` por grupo. O HUD inteiro sai em menos de dez chamadas. Um
//! `draw_rect` por letra custaria centenas, que é o erro que o `launcher-teste`
//! pagou com +7,3 ms por quadro.

pub mod font;
pub mod layout;

use giochi_engine_3d::math3d::{Camera3D, Mat4, Quat, Transform3D, Vec3};
use giochi_engine_3d::mesh::Vertex;

use layout::{Caixa, Tela};

// --------------------------------------------------------------------- cores

/// Texto neutro do HUD.
pub const COR_TEXTO: [f32; 4] = [0.88, 0.92, 0.98, 1.0];
/// Números, relógio e placar.
pub const COR_DESTAQUE: [f32; 4] = [0.45, 0.85, 1.0, 1.0];
/// Ouro: o que se pode gastar.
pub const COR_OURO: [f32; 4] = [1.0, 0.80, 0.25, 1.0];
/// Dano, boss e derrota.
pub const COR_ALERTA: [f32; 4] = [1.0, 0.32, 0.30, 1.0];
/// O Coronel — a ameaça que não é boss de onda.
pub const COR_NEMESIS: [f32; 4] = [0.85, 0.45, 1.0, 1.0];
/// Vida do jogador.
pub const COR_VIDA: [f32; 4] = [0.35, 0.90, 0.45, 1.0];
/// Vida da torre.
pub const COR_TORRE: [f32; 4] = [0.40, 0.62, 0.95, 1.0];
/// Fundo de barra.
pub const COR_TRILHA: [f32; 4] = [0.12, 0.14, 0.19, 0.85];
/// Fundo de painel opaco o suficiente para o texto por cima.
pub const COR_PAINEL: [f32; 4] = [0.06, 0.08, 0.12, 0.92];
/// Véu sobre o jogo quando a loja abre.
pub const COR_VEU: [f32; 4] = [0.02, 0.03, 0.05, 0.72];
/// Moldura do minimapa.
pub const COR_MOLDURA: [f32; 4] = [0.35, 0.45, 0.60, 0.90];

/// Abaixo disto a barra muda de cor: o aviso tem de ser visível antes do fim.
const LIMIAR_CRITICO: f32 = 0.25;

/// Cor de uma barra de vida, misturando para o vermelho no fim.
///
/// A cor **não** é escolhida pela fração sozinha: uma torre azul e um jogador
/// verde continuam identificáveis até o vermelho entrar. Trocar a cor inteira
/// faria o jogador perder de vista de quem é a barra justamente no momento em
/// que ele mais precisa dela.
pub fn cor_da_barra(fracao: f32, base: [f32; 4]) -> [f32; 4] {
    let f = fracao.clamp(0.0, 1.0);
    if f >= LIMIAR_CRITICO {
        return base;
    }
    let t = 1.0 - f / LIMIAR_CRITICO;
    [
        base[0] + (COR_ALERTA[0] - base[0]) * t,
        base[1] + (COR_ALERTA[1] - base[1]) * t,
        base[2] + (COR_ALERTA[2] - base[2]) * t,
        base[3],
    ]
}

// -------------------------------------------------------------------- modelos

/// Conteúdo do HUD superior e das barras.
#[derive(Debug, Clone, PartialEq)]
pub struct Hud {
    /// Rótulo da onda (`3/6` na campanha, `7/∞` no Endless).
    pub wave_label: String,
    /// Relógio da fase (`03:58`).
    pub clock: String,
    /// Linha completa do HUD — vai para o log e para os testes, não para a tela.
    pub wave_line: String,
    /// HP do jogador local, em `0..=1`.
    pub player_hp_frac: f32,
    /// Texto do HP (`100/130`).
    pub player_hp_text: String,
    /// HP da torre, em `0..=1`.
    pub tower_hp_frac: f32,
    /// Texto da torre (`5000/8000`).
    pub tower_hp_text: String,
    /// Gold do jogador local.
    pub gold: u32,
    /// Pontuação.
    pub score: u32,
    /// Inimigos vivos (contagem).
    pub enemies: usize,
    /// Indicador de boss, se houver: nome e % de HP.
    pub boss_line: Option<String>,
    /// Indicador do Coronel: tier e HP.
    pub nemesis_line: Option<String>,
    /// Fase legível (para o canto).
    pub phase: &'static str,
}

impl Hud {
    /// Todas as linhas de texto, na ordem de leitura — para log e teste.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            self.wave_line.clone(),
            format!(
                "Torre {} ({:.0}%)",
                self.tower_hp_text,
                self.tower_hp_frac * 100.0
            ),
            format!(
                "HP {} · {} gold · {} pts",
                self.player_hp_text, self.gold, self.score
            ),
            format!("{} · {} inimigo(s)", self.phase, self.enemies),
        ];
        if let Some(boss) = &self.boss_line {
            lines.push(boss.clone());
        }
        if let Some(nemesis) = &self.nemesis_line {
            lines.push(nemesis.clone());
        }
        lines
    }
}

/// Minimapa: posições normalizadas em `-1..=1` (o desenho mapeia para pixels).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Minimap {
    /// Torre (sempre o centro).
    pub tower: [f32; 2],
    /// Aliados vivos.
    pub allies: Vec<[f32; 2]>,
    /// Último avistamento do Coronel — só se ele já foi visto (GDD §10).
    pub nemesis: Option<[f32; 2]>,
}

/// Tela da loja.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopScreen {
    /// Aberta?
    pub open: bool,
    /// Gold disponível, para o título.
    pub gold: u32,
    /// Nomes dos itens, com preço e marcação.
    pub lines: Vec<String>,
    /// Item sob o cursor.
    pub cursor: usize,
}

/// Um quadro inteiro de UI.
#[derive(Debug, Clone, PartialEq)]
pub struct UiFrame {
    /// HUD.
    pub hud: Hud,
    /// Minimapa.
    pub minimap: Minimap,
    /// Loja (fechada fora do intervalo).
    pub shop: ShopScreen,
}

/// Monta os modelos de UI a partir do estado.
pub fn build(state: &crate::game::state::GameState, config: &crate::config::GameConfig) -> UiFrame {
    let local = state.local();
    let boss_line = state.boss.as_ref().map(|boss| {
        format!(
            "{} — {}% (fase {})",
            boss.kind.as_str(),
            boss.hp_pct_u8(),
            boss.phase + 1
        )
    });
    let nemesis_line = state.nemesis.as_ref().map(|nemesis| {
        format!(
            "O Coronel — tier {} ({} HP)",
            nemesis.tier, nemesis.max_hp as u32
        )
    });

    let hud = Hud {
        wave_label: state.wave_label(config),
        clock: state.clock_label(config),
        wave_line: state.hud_line(config),
        player_hp_frac: local.hp_pct(),
        player_hp_text: format!("{:.0}/{:.0}", local.hp, local.max_hp),
        tower_hp_frac: state.tower.hp_pct(),
        tower_hp_text: format!("{:.0}/{:.0}", state.tower.hp, state.tower.max_hp),
        gold: local.gold,
        score: local.score,
        enemies: state.enemies_alive(),
        boss_line,
        nemesis_line,
        phase: state.phase.as_str(),
    };

    let half = state.map.half_extent_m.max(1.0);
    let normalize = |position: glam::Vec3| [position.x / half, position.z / half];
    let minimap = Minimap {
        tower: [0.0, 0.0],
        allies: state
            .players
            .iter()
            .filter(|p| p.can_act())
            .map(|p| normalize(p.position))
            .collect(),
        // `broadcast_pos` é o último avistamento defasado; `Vec3::ZERO`
        // significa "ainda não visto" e não vira ponto no mapa.
        nemesis: state
            .nemesis
            .as_ref()
            .map(|n| n.broadcast_pos())
            .filter(|pos| *pos != glam::Vec3::ZERO)
            .map(normalize),
    };

    let shop = ShopScreen {
        open: state.phase == crate::game::state::Phase::Shop,
        gold: local.gold,
        lines: state
            .shop
            .menu(local.gold, config)
            .iter()
            .map(|entry| {
                let mark = if entry.exhausted {
                    "esgotado"
                } else if entry.affordable {
                    "ok"
                } else {
                    "sem gold"
                };
                format!("{:<24} {:>4} gold  [{}]", entry.label, entry.price, mark)
            })
            .collect(),
        cursor: state.shop_cursor,
    };

    UiFrame {
        hud,
        minimap,
        shop,
    }
}

// ------------------------------------------------------------------- desenho

/// Distância do plano do HUD, em unidades de mundo.
///
/// A câmera do HUD usa `fov` de 90° e planos 9..4000, então o HUD fica com
/// profundidade normalizada ~0,10 — à frente de qualquer coisa do mundo. Com os
/// planos padrão (0,1..1000) ele cairia em ~0,99 e **sumiria** atrás do chão.
/// O `neon-drift` e o `giochi-defender` mediram este número pelo mesmo motivo.
pub const PROFUNDIDADE: f32 = 10.0;

/// Um lote de instâncias que compartilham a mesma cor.
#[derive(Debug, Clone, PartialEq)]
pub struct Lote {
    /// Cor com o alfa já quantizado.
    pub cor: [f32; 4],
    /// Uma matriz por quadrilátero, em unidades da câmera do HUD.
    pub matrizes: Vec<Mat4>,
}

/// Quantos degraus de alfa a batelada distingue.
///
/// A cor é uniforme por desenho, então cada alfa distinto custaria uma chamada.
/// Oito degraus cobrem desde o véu da loja (0,72) até as molduras (0,90) sem que
/// dois tons visivelmente diferentes caiam no mesmo lote.
const DEGRAUS_DE_ALFA: f32 = 8.0;

/// Matriz de instância para um retângulo **em pixels** da tela.
///
/// O quadrilátero unitário tem 1 unidade de lado centrado na origem; a matriz o
/// leva ao centro do retângulo e o estica para `(w, h)`.
///
/// ## O fator 2 não é decoração
///
/// Com `fov` de 90° a **meia**-altura visível a `PROFUNDIDADE` unidades é a
/// própria `PROFUNDIDADE`; a altura inteira é `2·PROFUNDIDADE`, e é ela que
/// cobre `tela.altura` pixels. Esquecer o 2 desenha tudo com metade do tamanho —
/// e o texto continua legível, porque as posições estão certas, então o defeito
/// passa despercebido. Há teste.
pub fn matriz_de_pixels(x: f32, y: f32, w: f32, h: f32, tela: Tela) -> Mat4 {
    let largura = tela.largura.max(1.0);
    let altura = tela.altura.max(1.0);
    let aspecto = largura / altura;
    let profundidade = PROFUNDIDADE;

    let ndc_x = (x + w * 0.5) / largura * 2.0 - 1.0;
    let ndc_y = 1.0 - (y + h * 0.5) / altura * 2.0;

    Transform3D::new(
        Vec3::new(
            ndc_x * profundidade * aspecto,
            ndc_y * profundidade,
            -profundidade,
        ),
        Quat::IDENTITY,
        Vec3::new(
            (w / largura) * 2.0 * profundidade * aspecto,
            (h / altura) * 2.0 * profundidade,
            1.0,
        ),
    )
    .matrix()
}

/// Chave de batelada: cor com o alfa quantizado.
fn chave(cor: [f32; 4]) -> [f32; 4] {
    let alfa = (cor[3].clamp(0.0, 1.0) * DEGRAUS_DE_ALFA).round() / DEGRAUS_DE_ALFA;
    [cor[0], cor[1], cor[2], alfa]
}

/// Converte o plano em lotes de desenho.
///
/// Cada retângulo e cada ponto aceso de cada glifo vira uma instância do
/// quadrilátero unitário. O HUD de um quadro inteiro sai em **uma** chamada por
/// cor.
pub fn lotes(plano: &layout::Plano, tela: Tela) -> Vec<Lote> {
    let mut saida: Vec<Lote> = Vec::new();

    let mut empurrar = |matriz: Mat4, cor: [f32; 4]| {
        let cor = chave(cor);
        match saida.iter_mut().find(|lote| lote.cor == cor) {
            Some(lote) => lote.matrizes.push(matriz),
            None => saida.push(Lote {
                cor,
                matrizes: vec![matriz],
            }),
        }
    };

    for retangulo in &plano.retangulos {
        let Caixa { x, y, w, h } = retangulo.caixa;
        if retangulo.cor[3] <= 0.0 || w <= 0.0 || h <= 0.0 {
            continue;
        }
        empurrar(matriz_de_pixels(x, y, w, h, tela), retangulo.cor);
    }

    for texto in &plano.textos {
        if texto.cor[3] <= 0.0 {
            continue;
        }
        // Retângulos máximos, não um por ponto aceso: 2,8× menos instâncias com
        // cobertura idêntica (`font::retangulos`). No R35S cada instância custa
        // 1,47 µs, e era isso que punha a loja a 29,9 fps.
        //
        // O 1,06 de sobreposição fecha a fresta de fundo entre instâncias
        // vizinhas; ele é **absoluto** (0,06 de um ponto), e não proporcional,
        // para um retângulo de 5 pontos não inchar 30 % e borrar a letra.
        let sobreposicao = texto.escala * 0.06;
        for [px, py, pw, ph] in font::retangulos(&texto.conteudo) {
            empurrar(
                matriz_de_pixels(
                    texto.x + px as f32 * texto.escala,
                    texto.y + py as f32 * texto.escala,
                    pw as f32 * texto.escala + sobreposicao,
                    ph as f32 * texto.escala + sobreposicao,
                    tela,
                ),
                texto.cor,
            );
        }
    }

    saida
}

/// Vértices e índices do quadrilátero unitário usado por todo o HUD.
///
/// Um só, criado na abertura: o que muda por quadro são as matrizes de
/// instância, não a geometria.
pub fn quad_da_hud() -> (Vec<Vertex>, Vec<u32>) {
    const NORMAL: [f32; 3] = [0.0, 0.0, 1.0];
    // A câmera do HUD olha para `-Z`, então o lado visível é o `+Z`; com o
    // descarte de faces ligado, inverter a ordem faria o HUD sumir.
    let cantos = [
        [-0.5f32, -0.5, 0.0],
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
    (vertices, indices)
}

/// Câmera do HUD para um tamanho de tela.
///
/// `fov` de 90° é o que torna [`matriz_de_pixels`] exata, e o plano próximo em
/// 9,0 é o que mantém o HUD à frente de tudo.
pub fn camera(tela: (u32, u32)) -> Camera3D {
    Camera3D::from_parts(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::UP)
        .with_fov_y(std::f32::consts::FRAC_PI_2)
        .with_clip_planes(9.0, 4000.0)
        .with_drawable_size(tela.0, tela.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ClassId, GameConfig, ProfileId};
    use crate::game::state::{GameState, Phase};
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    #[test]
    fn the_hud_carries_the_numbers_the_gdd_asks_for() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        // Sem `Phase::Combat` a partida está em `Warmup`, cuja duração já
        // passou — e o relógio mostrava `00:00` em vez de `03:58`. O teste
        // queria medir o rótulo, não a fase inicial.
        state.phase = Phase::Combat;
        state.phase_timer_s = 122.0;
        state.wave = 3;
        state.local_mut().gold = 45;
        state.local_mut().score = 70;

        let frame = build(&state, &c);
        assert_eq!(frame.hud.wave_label, "3/6");
        assert_eq!(frame.hud.clock, "03:58");
        assert_eq!(
            frame.hud.wave_line,
            "Onda 3/6 — 03:58 · torre 5000/5000 · 0 inimigo(s) · 45 gold"
        );
        assert_eq!(frame.hud.gold, 45);
        assert_eq!(frame.hud.score, 70);
        assert!((frame.hud.tower_hp_frac - 1.0).abs() < 1.0e-6);
        assert_eq!(frame.hud.player_hp_text, "100/100");
        assert!(!frame.hud.lines().is_empty());
    }

    #[test]
    fn the_minimap_normalises_positions_to_the_map() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Sniper, ProfileId::Balanced);
        state.local_mut().position = glam::Vec3::new(40.0, 0.0, -40.0);
        let frame = build(&state, &c);
        assert_eq!(frame.minimap.tower, [0.0, 0.0]);
        assert_eq!(frame.minimap.allies.len(), 1);
        // 40 m num mapa de meia-extensão 80 = 0,5.
        assert!((frame.minimap.allies[0][0] - 0.5).abs() < 1.0e-5);
        assert!((frame.minimap.allies[0][1] + 0.5).abs() < 1.0e-5);
        assert!(frame.minimap.nemesis.is_none(), "ainda não foi avistado");
    }

    #[test]
    fn a_dead_ally_leaves_the_minimap() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Medic, ProfileId::Balanced);
        state.local_mut().die(&c);
        let frame = build(&state, &c);
        assert!(frame.minimap.allies.is_empty());
    }

    #[test]
    fn the_shop_is_closed_outside_the_interval_and_lists_nine_items_inside() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        assert!(!build(&state, &c).shop.open);
        assert_eq!(
            build(&state, &c).shop.lines.len(),
            9,
            "a loja lista os 9 itens"
        );

        state.phase = Phase::Shop;
        state.local_mut().gold = 100;
        let frame = build(&state, &c);
        assert!(frame.shop.open);
        assert_eq!(frame.shop.gold, 100);
        assert!(frame.shop.lines.iter().any(|l| l.contains("Rifle melhorado")));
        assert!(frame.shop.lines.iter().any(|l| l.contains("sem gold")));
        assert!(frame.shop.lines.iter().any(|l| l.contains("ok")));
    }

    #[test]
    fn a_boss_and_the_colonel_appear_only_when_they_are_in_the_world() {
        let c = config();
        let mut state = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        assert!(build(&state, &c).hud.boss_line.is_none());

        state.boss = Some(crate::game::boss::Boss::spawn(
            c.boss(crate::config::BossKind::Queen).expect("rainha"),
            glam::Vec3::ZERO,
        ));
        let frame = build(&state, &c);
        let line = frame.hud.boss_line.expect("indicador de boss");
        assert!(line.contains("Queen"), "{line}");
        assert!(line.contains("100%"), "{line}");
    }

    #[test]
    fn a_camera_do_hud_e_valida_em_todos_os_paineis() {
        for tela in [(640, 480), (800, 600), (1280, 720), (720, 1280)] {
            let camera = camera(tela);
            assert!(
                camera.view_projection().is_ok(),
                "{tela:?}: matriz de visão-projeção inválida"
            );
        }
    }

    #[test]
    fn um_retangulo_de_pixels_ocupa_os_mesmos_pixels() {
        // O teste que falta doía: com o fator 2 esquecido na escala, um
        // retângulo de 14 px saía com 7 e nada no log dizia nada. Aqui a matriz
        // de instância é aplicada aos cantos do quad unitário e projetada com a
        // **mesma** câmera do desenho; o resultado tem de cair no retângulo
        // pedido, em pixels.
        for (lw, lh) in [(640u32, 480u32), (800, 600), (1280, 720), (720, 1280)] {
            let tela = Tela::new(lw, lh);
            let matriz_camera = camera((lw, lh)).view_projection().expect("matriz do HUD");
            for (x, y, w, h) in [
                (0.0, 0.0, 14.0, 14.0),
                (100.0, 50.0, 30.0, 21.0),
                (lw as f32 - 60.0, lh as f32 - 40.0, 60.0, 40.0),
            ] {
                let instancia = matriz_de_pixels(x, y, w, h, tela);
                let cantos = [
                    [-0.5f32, -0.5, 0.0],
                    [0.5, -0.5, 0.0],
                    [0.5, 0.5, 0.0],
                    [-0.5, 0.5, 0.0],
                ];
                let mut minimos = [f32::MAX; 2];
                let mut maximos = [f32::MIN; 2];
                for canto in cantos {
                    let mundo = instancia.transform_point(Vec3::new(canto[0], canto[1], 0.0));
                    let ndc = matriz_camera.project_point(mundo).expect("projeção");
                    let px = (ndc.x + 1.0) * 0.5 * lw as f32;
                    let py = (1.0 - ndc.y) * 0.5 * lh as f32;
                    minimos[0] = minimos[0].min(px);
                    minimos[1] = minimos[1].min(py);
                    maximos[0] = maximos[0].max(px);
                    maximos[1] = maximos[1].max(py);
                }
                assert!(
                    (minimos[0] - x).abs() < 0.5,
                    "{lw}x{lh}: x esquerdo {minimos:?} != {x}"
                );
                assert!(
                    (minimos[1] - y).abs() < 0.5,
                    "{lw}x{lh}: y topo {minimos:?} != {y}"
                );
                assert!(
                    (maximos[0] - (x + w)).abs() < 0.5,
                    "{lw}x{lh}: x direito {maximos:?} != {}",
                    x + w
                );
                assert!(
                    (maximos[1] - (y + h)).abs() < 0.5,
                    "{lw}x{lh}: y base {maximos:?} != {}",
                    y + h
                );
            }
        }
    }

    #[test]
    fn o_hud_inteiro_cabe_em_poucas_chamadas_de_desenho() {
        // Um `draw_rect` por letra custaria centenas de chamadas. Este limite é
        // o que transforma "eu agrupei" em número.
        let c = config();
        let mut estado = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        estado.phase = Phase::Combat;
        estado.phase_timer_s = 122.0;
        let frame = build(&estado, &c);
        let tela = Tela::new(640, 480);
        let plano = layout::montar(&frame, tela);
        let lotes = lotes(&plano, tela);
        assert!(!lotes.is_empty(), "o HUD não desenhou nada");
        assert!(
            lotes.len() <= 12,
            "o HUD gastou {} chamadas de desenho",
            lotes.len()
        );
        // E tudo o que foi pedido virou instância: nada é descartado em silêncio.
        let instancias: usize = lotes.iter().map(|l| l.matrizes.len()).sum();
        let esperado = plano.retangulos.len()
            + plano
                .textos
                .iter()
                .map(|t| font::retangulos(&t.conteudo).len())
                .sum::<usize>();
        assert_eq!(instancias, esperado);
    }

    #[test]
    fn o_alfa_e_quantizado_para_o_lote_nao_multiplicar() {
        assert_eq!(chave([0.5, 0.5, 0.5, 1.0]), [0.5, 0.5, 0.5, 1.0]);
        // 0,72 e 0,70 caem no mesmo degrau; 0,72 e 0,10 não.
        assert_eq!(chave([0.0, 0.0, 0.0, 0.72]), chave([0.0, 0.0, 0.0, 0.70]));
        assert_ne!(chave([0.0, 0.0, 0.0, 0.72]), chave([0.0, 0.0, 0.0, 0.10]));
    }

    #[test]
    fn a_barra_fica_vermelha_so_no_fim_e_mantem_a_identidade_antes() {
        let cheia = cor_da_barra(1.0, COR_TORRE);
        assert_eq!(cheia, COR_TORRE);
        assert_eq!(cor_da_barra(0.5, COR_TORRE), COR_TORRE);
        assert_eq!(cor_da_barra(LIMIAR_CRITICO, COR_TORRE), COR_TORRE);

        let critica = cor_da_barra(0.0, COR_TORRE);
        assert_eq!(critica, COR_ALERTA, "no fim a barra é do alerta");
        // No meio do caminho ela já não é nem uma nem outra: a transição é
        // suave, não um degrau.
        let meio = cor_da_barra(LIMIAR_CRITICO * 0.5, COR_VIDA);
        assert!(meio[1] < COR_VIDA[1] && meio[1] > COR_ALERTA[1], "{meio:?}");
    }

    #[test]
    fn o_quad_do_hud_tem_dois_triangulos_e_normal_para_a_camera() {
        let (vertices, indices) = quad_da_hud();
        assert_eq!(indices.len(), 6);
        assert_eq!(vertices.len(), 6);
        for v in &vertices {
            assert_eq!(v.normal, [0.0, 0.0, 1.0]);
        }
    }
}
