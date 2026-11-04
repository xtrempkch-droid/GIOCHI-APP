//! Desenho das telas e coleta de áreas tocáveis.
//!
//! Cada função desenha e, quando um elemento é interativo, registra seu
//! retângulo num [`Layout`]. É assim que o toque funciona sem duplicar
//! geometria: quem desenha sabe onde o elemento está, então é quem registra.
//! Separar "desenhar" de "calcular posições" seria a receita para os dois
//! saírem de sincronia.

use giochi_launcher_core::state::{
    Filter, Overlay, Tab, KEYBOARD, KEYBOARD_COLS, QUICK_SETTINGS_ROWS, SETTINGS_CATEGORIES,
};
use giochi_launcher_core::{State, Zone};
use sdl2::rect::Rect;

use crate::draw::{Clip, Painter};
use crate::theme;

/// Medidas do layout para o painel atual, **em proporção**.
///
/// # Por que isto existe
///
/// O layout era escrito em coordenadas fixas de 1280×720. Num painel 640×480 o
/// SDL escalava 0,5 e `F_TINY` (9 px) virava **4,5 px** — ilegível. Medido no
/// X35H: o texto saía com metade do tamanho do X55.
///
/// Agora o espaço de projeto é o painel (ver `draw::projecao`), então a escala é
/// sempre 1,0 e as fontes mantêm o tamanho em pixels. Em troca, as posições
/// precisam ser calculadas: é o que esta struct faz.
///
/// Os números continuam sendo **os mesmos do desenho em 1280×720** — mudam por
/// proporção, não por gosto. Num painel 1280×720 o resultado é idêntico ao
/// anterior, o que permite conferir a mudança comparando frames.
#[derive(Clone, Copy, Debug)]
pub struct Moldura {
    pub w: i32,
    pub h: i32,
    /// Escala em relação ao desenho de referência (1280×720).
    pub k: f32,
    /// Margem lateral confortável.
    pub safe_x: i32,
    /// Largura útil entre as margens.
    pub safe_w: i32,
    /// Topo da área de conteúdo (abaixo da barra superior).
    pub top: i32,
    /// Altura da área de conteúdo (entre a barra e o rodapé).
    pub main_h: i32,
    /// Y do rodapé.
    pub foot_y: i32,
    pub pad: i32,
    /// Painel pequeno o bastante para o desenho de 1280×720 não caber por
    /// proporção: as telas trocam para a variante densa.
    ///
    /// O corte é por **pixels de device**, não por proporção: 640×480 e 800×600
    /// são painéis onde o texto de 13 px ocupa fração grande da tela e onde
    /// mostrar três cartões lado a lado deixa cada um com menos de 90 px úteis.
    pub compacto: bool,
}

impl Moldura {
    pub fn de(p: &Painter) -> Self {
        let (w, h) = p.logical_size();
        let (w, h) = (w as i32, h as i32);
        // A referência é 1280×720. `k` escala espessuras e raios (o que não dá
        // para fazer só por proporção de largura/altura).
        let k = (w as f32 / 1280.0).min(h as f32 / 720.0);
        // As barras **não** escalam. O conteúdo delas é texto e área de toque em
        // pixels de device (chip de 36 px, ícone de 14 px), então encolhê-las
        // pela densidade fazia o chip vazar da barra e o rodapé sair pela borda
        // de baixo: medido em 640×480, o rodapé desenhava até y=484 num painel
        // de 480. Elas reservam a altura do projeto em qualquer painel.
        let barra = theme::TOPBAR_H;
        let rodape = theme::FOOTER_H;
        let margem = (theme::SAFE_X as f32 * k).round() as i32;
        let top = barra;
        let main_h = (h - barra - rodape).max(1);
        Self {
            w,
            h,
            k,
            safe_x: margem,
            safe_w: (w - margem * 2).max(1),
            top,
            main_h,
            foot_y: barra + main_h,
            pad: (12.0 * k).round().max(4.0) as i32,
            compacto: w < 900 || h < 560,
        }
    }

    /// Escala um comprimento do desenho de referência.
    pub fn e(&self, v: i32) -> i32 {
        (v as f32 * self.k).round() as i32
    }

    /// X para centralizar uma caixa de largura `w`.
    pub fn centro(&self, w: i32) -> i32 {
        (self.w - w) / 2
    }
}

pub const LIB_PAGE: usize = 8;

/// Escolhe colunas e linhas para `itens` numa área `w`×`h` com `gap` entre
/// células, **maximizando a área da célula** sob uma faixa de proporção usável.
///
/// # Por que não um 4×2 fixo
///
/// A grade era sempre 4 colunas por 2 linhas. Num painel 640×480 isso dá
/// células de 110×172 — uma tira em pé, onde o título de 13 px não cabe — e,
/// como o catálogo tem 4 jogos, a segunda linha ficava vazia com metade da tela
/// sem uso. Numa área 4:3 o melhor arranjo para 4 itens é 2×2.
///
/// A faixa de proporção existe porque área igual não significa legibilidade: uma
/// célula 110×353 tem a mesma área de uma 229×172, e só a segunda comporta um
/// título de duas palavras.
pub fn grade_para(itens: usize, w: i32, h: i32, gap: i32) -> (i32, i32) {
    let itens = itens.max(1);
    let (mut melhor_cols, mut melhor_rows) = (1i32, itens as i32);
    let mut melhor_area = -1i64;
    let mut melhor_vazios = usize::MAX;

    for cols in 1..=itens {
        let rows = itens.div_ceil(cols);
        let cw = (w - gap * (cols as i32 - 1)) / cols as i32;
        let ch = (h - gap * (rows as i32 - 1)) / rows as i32;
        if cw <= 0 || ch <= 0 {
            continue;
        }
        let aspecto = cw as f32 / ch as f32;
        if !(0.62..=2.2).contains(&aspecto) {
            continue;
        }
        let area = cw as i64 * ch as i64;
        let vazios = cols * rows - itens;
        if area > melhor_area || (area == melhor_area && vazios < melhor_vazios) {
            (melhor_cols, melhor_rows) = (cols as i32, rows as i32);
            melhor_area = area;
            melhor_vazios = vazios;
        }
    }
    (melhor_cols, melhor_rows)
}

/// Ação disparada por um toque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Tab(Tab),
    Filter(Filter),
    Carousel(usize),
    Game(usize),
    App(usize),
    SettingsCat(usize),
    QsRow(usize),
    QsButton(u8),
    DetailButton(usize),
    PowerButton(usize),
    Key(usize),
    Result(usize),
    PowerTap,
}

/// Áreas tocáveis do quadro atual.
#[derive(Default)]
pub struct Layout {
    pub hits: Vec<(Rect, Hit)>,
}

impl Layout {
    pub fn hit(&self, x: i32, y: i32) -> Option<Hit> {
        self.hits
            .iter()
            .find(|(rect, _)| {
                x >= rect.x()
                    && y >= rect.y()
                    && x < rect.x() + rect.width() as i32
                    && y < rect.y() + rect.height() as i32
            })
            .map(|(_, hit)| *hit)
    }
}

fn vcenter(y: i32, h: i32, text_h: i32) -> i32 {
    y + (h - text_h) / 2 - 1
}

// ════════════════════════════════════════════════════════════
// BARRA SUPERIOR
// ════════════════════════════════════════════════════════════
pub fn chrome_top(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    p.fill_rect(m.safe_x, m.top - 1, m.safe_w, 1, theme::LINE);
    // Origem da barra: as medidas internas são em pixels de device (chip de
    // 36, ícone de 14), então tudo é deslocado por y0 em vez de começar em 10.
    let y0 = m.top - theme::TOPBAR_H;

    let blocked = state.has_overlay() || state.screen != giochi_launcher_core::Screen::Running;
    let mut x = m.safe_x;

    // No painel compacto as abas ficam só com o rótulo: o ícone custa 32 px por
    // aba e é o que fazia a barra inteira (4 abas + 6 indicadores) somar 660 px
    // numa largura útil de 480, desenhando um bloco por cima do outro.
    let com_icone = !m.compacto;

    for tab in Tab::ALL {
        let label = tab.label();
        let text_w = p.text_size(label, theme::F_SMALL).0;
        let w = if com_icone { text_w + 34 + 22 } else { text_w + 24 };
        let active = tab == state.tab;

        if active {
            p.fill_round_rect(x, y0 + 10, w, 36, theme::RADIUS_CHIP, theme::CARD);
            p.stroke_round_rect(x, y0 + 10, w, 36, theme::RADIUS_CHIP, theme::LINE2);
        }
        let color = if active { theme::TEXT } else { theme::DIM };
        let icon_name = match tab {
            Tab::Home => "home",
            Tab::Library => "gamepad",
            Tab::Apps => "apps",
            Tab::Settings => "settings",
        };
        if com_icone {
            p.icon(icon_name, x + 18, y0 + 28, 14, color);
        }
        let texto_x = if com_icone { x + 32 } else { x + 12 };
        let ty = vcenter(y0 + 10, 36, p.text_size(label, theme::F_SMALL).1);
        p.text_left(texto_x, ty, label, theme::F_SMALL, color);

        if !blocked {
            layout.hits.push((Rect::new(x, y0 + 10, w as u32, 36), Hit::Tab(tab)));
        }
        x += w + 3;
    }

    // O bloco direito só desenha o que cabe à direita das abas. Cada indicador
    // é opcional; passar por cima das abas não é.
    let inicio_direito = x + 6;
    let cabe = |rx: i32, w: i32| rx - w >= inicio_direito;

    // Lado direito: controle, wifi, bateria, relógio, energia.
    let mut rx = m.safe_x + m.safe_w;

    // Botão de energia.
    let power_w = 34;
    rx -= power_w;
    p.fill_round_rect(rx, y0 + 11, power_w, 34, 6, theme::with_alpha(theme::DANGER, 30));
    p.stroke_round_rect(rx, y0 + 11, power_w, 34, 6, theme::with_alpha(theme::DANGER, 90));
    p.icon("power", rx + power_w / 2, y0 + 28, 16, theme::DANGER);
    layout
        .hits
        .push((Rect::new(rx, y0 + 11, power_w as u32, 34), Hit::PowerTap));
    rx -= 8;

    // Relógio.
    let clock = current_clock();
    let clock_w = p.text_size(&clock, theme::F_BODY).0;
    if cabe(rx, clock_w) {
        rx -= clock_w;
        p.text_right(rx + clock_w, y0 + 22, &clock, theme::F_BODY, theme::TEXT);
        rx -= 10;
    }

    // Bateria.
    let battery = state.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
    if cabe(rx, p.text_size(&battery, theme::F_TINY).0 + 22) {
        rx = badge(p, layout, rx, &battery, theme::TEXT, None);
        rx -= 6;
    }

    // Wi-Fi e controle: indicadores de estado, os primeiros a sair quando a
    // barra está apertada — a mesma informação está em Ajustes.
    if !m.compacto {
        let wifi_on = state.settings.wifi;
        if cabe(rx, p.text_size("Wi-Fi", theme::F_TINY).0 + 34) {
            rx = badge(p, layout, rx, "Wi-Fi", if wifi_on { theme::OK } else { theme::DIM }, Some("wifi"));
            rx -= 6;
        }

        let pad_label = if state.pad_connected { "ON" } else { "OFF" };
        let pad_color = if state.pad_connected { theme::OK } else { theme::DIM };
        if cabe(rx, p.text_size(pad_label, theme::F_TINY).0 + 34) {
            rx = badge(p, layout, rx, pad_label, pad_color, Some("gamepad"));
            rx -= 6;
        }
    }

    // Ajustes rápidos.
    if cabe(rx, 34) {
        rx -= 34;
        p.fill_round_rect(rx, y0 + 11, 34, 34, 6, theme::PANEL2);
        p.icon("settings", rx + 17, y0 + 28, 16, theme::DIM);
        rx -= 8;
    }

    // Texto "Giochi" à esquerda do bloco direito, se sobrar espaço.
    let _ = rx;
}

fn badge(
    p: &mut Painter,
    layout: &mut Layout,
    right: i32,
    label: &str,
    color: sdl2::pixels::Color,
    icon_name: Option<&str>,
) -> i32 {
    let text_w = p.text_size(label, theme::F_TINY).0;
    let has_icon = icon_name.is_some();
    let w = text_w + 20 + if has_icon { 16 } else { 0 };
    let x = right - w;
    p.fill_round_rect(x, 14, w, 28, 14, theme::with_alpha(theme::BLACK, 120));
    p.stroke_round_rect(x, 14, w, 28, 14, theme::LINE);
    let mut tx = x + 9;
    if let Some(name) = icon_name {
        p.icon(name, tx + 5, 28, 12, color);
        tx += 15;
    }
    let ty = vcenter(14, 28, p.text_size(label, theme::F_TINY).1);
    p.text_left(tx, ty, label, theme::F_TINY, color);
    let _ = layout;
    x
}

fn current_clock() -> String {
    // Sem dependência de data/hora: o horário real vem do sistema via `chrono`
    // ou de `/proc/driver/rtc` na integração. Aqui fica explícito que é
    // provisório em vez de fingir um relógio.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let minutes = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    format!("{hours:02}:{minutes:02}")
}

// ════════════════════════════════════════════════════════════
// RODAPÉ
// ════════════════════════════════════════════════════════════
pub fn chrome_foot(p: &mut Painter, state: &State) {
    let m = Moldura::de(p);
    p.fill_rect(m.safe_x, m.foot_y, m.safe_w, 1, theme::LINE);

    let hints: &[(&str, &str)] = match state.top_overlay() {
        Some(Overlay::Search) => &[("↑↓←→", "Navegar"), ("A", "Selecionar"), ("B", "Voltar")],
        Some(Overlay::QuickSettings) => &[("↑↓", "Navegar"), ("←→", "Ajustar"), ("B", "Fechar")],
        Some(Overlay::GameDetail) => &[("←→", "Ação"), ("A", "Confirmar"), ("B", "Voltar")],
        Some(Overlay::PowerPopup) => &[("↑↓", "Navegar"), ("A", "Confirmar"), ("B", "Cancelar")],
        None => &[
            ("A", "Jogar"),
            ("X", "Detalhes"),
            ("Y", "Buscar"),
            ("LB/RB", "Aba"),
            ("Select", "Ajustes"),
        ],
    };

    // Centralizado na faixa do rodapé. Antes era `foot_y + 18` fixo: num painel
    // de 480 px o chip de 16 px começava em y=468 e era cortado pela borda.
    let h_txt = p.text_size("Ay", theme::F_TINY).1;
    let chip_h = h_txt + 6;
    let faixa = (m.h - m.foot_y).max(chip_h);
    let chip_y = m.foot_y + (faixa - chip_h) / 2;
    let y = chip_y + (chip_h - h_txt) / 2;

    // A assinatura de versão sai quando não há largura: ela é a informação mais
    // dispensável do rodapé, e sobrepor as dicas seria pior.
    let build = format!("giochi-launcher 0.1.0 • {}", state.settings.perf.label());
    let build_w = p.text_size(&build, theme::F_TINY).0;

    let mut x = m.safe_x + 4;
    let limite = m.safe_x + m.safe_w - 4 - build_w - 16;
    for (key, label) in hints {
        let key_w = p.text_size(key, theme::F_TINY).0 + 10;
        let label_w = p.text_size(label, theme::F_TINY).0;
        if x + key_w + 5 + label_w > limite {
            break;
        }
        p.fill_round_rect(x, chip_y, key_w, chip_h, 3, theme::with_alpha(theme::WHITE, 20));
        let ky = vcenter(chip_y, chip_h, p.text_size(key, theme::F_TINY).1);
        p.text_left(x + 5, ky, key, theme::F_TINY, theme::ACCENT);
        x += key_w + 5;
        p.text_left(x, y, label, theme::F_TINY, theme::DIM);
        x += label_w + 12;
    }

    p.text_right(m.safe_x + m.safe_w - 4, y, &build, theme::F_TINY, theme::FAINT);
}

// ════════════════════════════════════════════════════════════
// HOME — carrossel
// ════════════════════════════════════════════════════════════
pub fn home(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let n = state.games.len();
    if n == 0 {
        p.text_center(
            m.safe_x + m.safe_w / 2,
            m.top + m.main_h / 2,
            "Catálogo vazio.",
            theme::F_SMALL,
            theme::FAINT,
        );
        return;
    }

    // ── o cartão se dimensiona pela área útil ──
    //
    // A versão anterior usava 520×320 fixos e escalava pela densidade. Num
    // painel 640×480 isso dava 260×160: o texto continuava com 26 px para um
    // cartão pela metade, e "meta" e o botão "Jogar" caíam no mesmo y — um
    // desenhado por cima do outro. Aqui o cartão é o maior que cabe mantendo a
    // proporção, e o conteúdo é empilhado de baixo para cima a partir das
    // métricas reais do texto, então não existe posição fixa para sobrepor.
    const ASPECTO: f32 = 520.0 / 320.0;
    let dock_h = m.e(64);
    let area_h = (m.main_h - dock_h).max(120);

    let mut card_w = if m.compacto {
        m.safe_w
    } else {
        m.safe_w.min(m.e(520))
    };
    let mut card_h = (card_w as f32 / ASPECTO).round() as i32;
    if card_h > area_h {
        card_h = area_h;
        card_w = (card_h as f32 * ASPECTO).round() as i32;
    }

    let y = m.top + area_h / 2 - card_h / 2;
    let x = m.safe_x + (m.safe_w - card_w) / 2;
    let radius = m.e(theme::RADIUS_CARD).max(4);

    // Vizinhos só quando sobra tela. Em 640×480 o cartão já ocupa a largura
    // útil; desenhar os laterais colocaria um por cima do outro.
    if !m.compacto {
        // Espaçamento derivado da largura do cartão: 0,81 é a distância em que
        // os cartões laterais encostam no central sem invadi-lo.
        let spacing = (card_w as f32 * 0.81).round() as i32;
        let cx = m.safe_x + m.safe_w / 2;
        for offset in [-1i32, 1] {
            let idx = ((state.carousel as i32 + offset).rem_euclid(n as i32)) as usize;
            let game = &state.games[idx];
            let w = (card_w as f32 * 0.62) as i32;
            let h = (card_h as f32 * 0.62) as i32;
            let bx = cx + offset * spacing - w / 2;
            let by = m.top + area_h / 2 - h / 2;
            let base = theme::game_base(game.id);
            let r = (radius as f32 * 0.62) as i32;
            p.vgradient_round_rect(bx, by, w, h, r, base, theme::mix(base, theme::BLACK, 0.35));
            p.fill_round_rect(bx, by, w, h, r, theme::with_alpha(theme::BLACK, 140));
        }
    }

    let game = &state.games[state.carousel.min(n - 1)];
    let accent = theme::game_accent(game.id);
    let base = theme::game_base(game.id);

    // Halo: anel grosso antes do cartão, espessura pela densidade.
    let halo = m.e(2).max(1);
    p.stroke_round_rect(
        x - halo,
        y - halo,
        card_w + halo * 2,
        card_h + halo * 2,
        radius + halo,
        accent,
    );
    p.vgradient_round_rect(x, y, card_w, card_h, radius, base, theme::mix(base, theme::BLACK, 0.35));

    // Letra grande ao fundo, proporcional ao cartão (não em px de projeto).
    let letter = game.title.chars().next().unwrap_or('?').to_string();
    let letter_px = ((card_h as f32) * 0.375).max(24.0) as u16;
    p.text_right(
        x + card_w - m.pad,
        y + m.pad / 2,
        &letter,
        letter_px,
        theme::with_alpha(theme::WHITE, 16),
    );

    // ── pilha derivada das métricas do texto ──
    //
    // Ancorar tudo no fundo deixava o cartão com um vazio grande em cima: a
    // altura do cartão muda com o painel, a altura do texto não. O bloco
    // (categoria, título, meta) fica centrado entre o topo do cartão e o botão,
    // então o vazio se divide em cima e embaixo em vez de se acumular.
    let pad = ((card_h as f32 * 0.07).round() as i32).max(m.pad);
    let f_cat = theme::F_TINY;
    let f_tit = theme::F_TITLE;
    let f_meta = theme::F_SMALL;
    let h_cat = p.text_size("Ay", f_cat).1;
    let h_tit = p.text_size("Ay", f_tit).1;
    let h_meta = p.text_size("Ay", f_meta).1;
    let gap = (h_meta / 2).max(3);

    let btn_h = theme::TOUCH.min((card_h / 5).max(26));
    let btn_w = (p.text_size("Jogar", f_meta).0 + 46).min(card_w - pad * 2);
    let by = y + card_h - pad - btn_h;

    let avail = card_w - pad * 2;
    let (tit_a, tit_b) = split_title(&game.title, avail, &mut |s| p.text_size(s, f_tit).0);
    let linhas = if tit_b.is_empty() { 1 } else { 2 };

    // A categoria é o elemento mais dispensável: sai antes de qualquer
    // sobreposição, porque some uma informação secundária em vez de virar bug.
    let mut com_categoria = true;
    let mut bloco_h = h_cat + gap + linhas * h_tit + gap + h_meta;
    let area_top = y + pad;
    let area_bot = by - gap;
    if bloco_h > area_bot - area_top {
        com_categoria = false;
        bloco_h -= h_cat + gap;
    }
    let bloco_y = area_top + ((area_bot - area_top - bloco_h) / 2).max(0);

    let cat_y = bloco_y;
    let tit_y = if com_categoria { cat_y + h_cat + gap } else { bloco_y };
    let meta_y = tit_y + linhas * h_tit + gap;

    if com_categoria {
        let cat = game.category.to_uppercase();
        p.text_left(x + pad, cat_y, &cat, f_cat, accent);
    }

    p.text_left(x + pad, tit_y, &tit_a, f_tit, theme::WHITE);
    if !tit_b.is_empty() {
        p.text_left(x + pad, tit_y + h_tit, &tit_b, f_tit, theme::WHITE);
    }

    let meta = format!("{} • {}", game.year_label(), game.play_label());
    let meta = truncate(&meta, avail, &mut |s| p.text_size(s, f_meta).0);
    p.text_left(x + pad, meta_y, &meta, f_meta, theme::with_alpha(theme::WHITE, 170));

    let bx = x + pad;
    p.fill_round_rect(bx, by, btn_w, btn_h, m.e(8).max(3), theme::with_alpha(theme::WHITE, 40));
    p.stroke_round_rect(bx, by, btn_w, btn_h, m.e(8).max(3), theme::with_alpha(theme::WHITE, 80));
    let icone = (h_meta as f32 * 1.1).round() as i32;
    p.icon("play", bx + m.e(20).max(10), by + btn_h / 2, icone, theme::WHITE);
    let ty = vcenter(by, btn_h, h_meta);
    p.text_left(bx + m.e(34).max(22), ty, "Jogar", f_meta, theme::WHITE);

    layout
        .hits
        .push((Rect::new(x, y, card_w as u32, card_h as u32), Hit::Carousel(state.carousel.min(n - 1))));

    // ── pontos e legenda, no espaço reservado ──
    let dot_y = m.top + area_h + dock_h / 2 - m.e(14).max(6);
    let dot_gap = m.e(11).max(6);
    let ativo = m.e(18).max(10);
    let total = (n as i32 - 1) * dot_gap + ativo;
    let mut dx = m.safe_x + m.safe_w / 2 - total / 2;
    for i in 0..n {
        if i == state.carousel {
            p.fill_round_rect(dx, dot_y, ativo, m.e(6).max(3), m.e(3).max(1), theme::ACCENT);
            dx += ativo + dot_gap - ativo;
        } else {
            let r = m.e(3).max(2);
            p.fill_circle(dx + r, dot_y + m.e(3).max(1), r, theme::with_alpha(theme::WHITE, 40));
            dx += dot_gap;
        }
    }

    // A legenda só entra se o rodapé não for atropelada por ela.
    let legenda_y = dot_y + m.e(14).max(8);
    if legenda_y + h_meta <= m.foot_y - m.e(4).max(2) {
        let line = format!("{} • {} • {}", game.title, game.category, game.year_label());
        let line = truncate(&line, m.safe_w, &mut |s| p.text_size(s, theme::F_SMALL).0);
        p.text_center(m.safe_x + m.safe_w / 2, legenda_y, &line, theme::F_SMALL, theme::DIM);
    }
}

/// Quebra um título em duas linhas por espaço.
fn split_title(text: &str, max_w: i32, measure: &mut impl FnMut(&str) -> i32) -> (String, String) {
    let words: Vec<&str> = text.split(' ').collect();
    let mut first = String::new();
    let mut rest = String::new();
    for (i, word) in words.iter().enumerate() {
        if first.is_empty() {
            first.push_str(word);
            continue;
        }
        let candidate = format!("{first} {word}");
        if measure(&candidate) <= max_w {
            first = candidate;
        } else {
            rest = words[i..].join(" ");
            break;
        }
    }
    if rest.is_empty() {
        rest = String::new();
    }
    (first, rest)
}

// ════════════════════════════════════════════════════════════
// BIBLIOTECA
// ════════════════════════════════════════════════════════════
pub fn library(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let games = state.filtered_games();
    let focused_index = if state.focus.zone == Zone::LibraryGrid {
        Some(state.focus.index)
    } else {
        None
    };

    // Cabeçalho com filtros.
    p.text_left(m.safe_x + 4, m.top + 12, "BIBLIOTECA", theme::F_TINY, theme::DIM);

    let mut fx = m.safe_x + m.safe_w - 4;
    for filter in Filter::ALL.iter().rev() {
        let label = filter.label();
        let w = p.text_size(label, theme::F_TINY).0 + 24;
        fx -= w;
        let active = state.library_filter == *filter;
        let focused = state.focus.zone == Zone::LibraryFilters
            && Filter::ALL.iter().position(|f| f == filter) == Some(state.focus.index);

        if active {
            p.fill_round_rect(fx, m.top + 6, w, 30, 15, theme::CARD);
            p.stroke_round_rect(fx, m.top + 6, w, 30, 15, theme::LINE2);
        }
        if focused {
            p.stroke_round_rect(fx, m.top + 6, w, 30, 15, theme::ACCENT);
        }
        let color = if active { theme::TEXT } else { theme::DIM };
        let ty = vcenter(m.top + 6, 30, p.text_size(label, theme::F_TINY).1);
        p.text_center(fx + w / 2, ty, label, theme::F_TINY, color);
        layout
            .hits
            .push((Rect::new(fx, m.top + 6, w as u32, 30), Hit::Filter(*filter)));

        fx -= 5;
    }

    let grid_top = m.top + m.e(48).max(34);
    let grid_h = (m.main_h - (grid_top - m.top) - m.e(24).max(14)).max(40);
    let gap = m.e(9).max(4);
    // O arranjo sai da capacidade da página, não do número de itens visíveis:
    // a grade não pode mudar de forma quando o usuário vira a página.
    let alvo = games.len().min(LIB_PAGE).max(1);
    let (cols, rows) = grade_para(alvo, m.safe_w - m.pad * 2, grid_h, gap);
    let por_pagina = (cols * rows).max(1) as usize;
    let page = focused_index.map(|i| i / por_pagina).unwrap_or(0);
    let pages = games.len().div_ceil(por_pagina).max(1);
    let cell_w = (m.safe_w - m.pad * 2 - gap * (cols - 1)) / cols;
    let cell_h = (grid_h - gap * (rows - 1)) / rows;

    if games.is_empty() {
        p.text_center(
            m.safe_x + m.safe_w / 2,
            grid_top + grid_h / 2,
            "Nenhum jogo nesta categoria",
            theme::F_SMALL,
            theme::FAINT,
        );
        return;
    }

    let start = page * por_pagina;
    let r_tile = m.e(theme::RADIUS_TILE).max(3);
    // Métricas do texto, para o rodapé da célula não depender de 32/16 fixos:
    // numa célula de 110 px de altura esses deslocamentos caem um sobre o outro.
    let h_body = p.text_size("Ay", theme::F_BODY).1;
    let h_tiny = p.text_size("Ay", theme::F_TINY).1;
    let interno = m.e(10).max(5);

    for slot in 0..por_pagina {
        let Some(game_index) = games.get(start + slot).copied() else {
            break;
        };
        let game = &state.games[game_index];
        let row = slot as i32 / cols;
        let col = slot as i32 % cols;
        let x = m.safe_x + m.pad + col * (cell_w + gap);
        let y = grid_top + row * (cell_h + gap);

        let accent = theme::game_accent(game.id);
        let base = theme::game_base(game.id);
        p.vgradient_round_rect(x, y, cell_w, cell_h, r_tile, base, theme::mix(base, theme::BLACK, 0.4));

        let focused = focused_index == Some(start + slot);
        if focused {
            p.stroke_round_rect(x, y, cell_w, cell_h, r_tile, accent);
        }

        if game.favorite {
            p.icon("star", x + cell_w - m.e(16).max(10), y + m.e(16).max(8), m.e(12).max(8), theme::WARN);
        }

        // Etiqueta de categoria.
        let cat = game.category.to_uppercase();
        let cw = p.text_size(&cat, theme::F_TINY).0 + m.e(12).max(8);
        let chip_h = h_tiny + m.e(6).max(3);
        p.fill_round_rect(
            x + interno,
            y + interno,
            cw,
            chip_h,
            m.e(3).max(1),
            theme::with_alpha(theme::BLACK, 120),
        );
        p.text_left(x + interno + m.e(6).max(3), y + interno + m.e(2).max(1), &cat, theme::F_TINY, accent);

        // Título e ano, ancorados no rodapé da célula.
        let year_y = y + cell_h - interno - h_tiny;
        let title_y = year_y - m.e(2).max(1) - h_body;
        if title_y > y + interno + chip_h + 2 {
            let title = truncate(&game.title, cell_w - interno * 2, &mut |s| p.text_size(s, theme::F_BODY).0);
            p.text_left(x + interno, title_y, &title, theme::F_BODY, theme::WHITE);
            p.text_left(
                x + interno,
                year_y,
                &game.year_label(),
                theme::F_TINY,
                theme::with_alpha(theme::WHITE, 140),
            );
        }

        layout
            .hits
            .push((Rect::new(x, y, cell_w as u32, cell_h as u32), Hit::Game(start + slot)));
    }

    let pager = format!("{} jogo(s) • página {}/{}", games.len(), page + 1, pages);
    p.text_center(m.safe_x + m.safe_w / 2, m.foot_y - 20, &pager, theme::F_TINY, theme::DIM);
}

// ════════════════════════════════════════════════════════════
// APPS
// ════════════════════════════════════════════════════════════
pub fn apps(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let gap = m.e(9).max(4);
    let (cols, rows) = grade_para(state.apps.len().max(1), m.safe_w - m.pad * 2, m.main_h - m.pad * 2, gap);
    let cell_w = (m.safe_w - m.pad * 2 - gap * (cols - 1)) / cols;
    let cell_h = (m.main_h - m.pad * 2 - gap * (rows - 1)) / rows;
    let r_tile = m.e(theme::RADIUS_TILE).max(3);
    let h_body = p.text_size("Ay", theme::F_BODY).1;
    let h_tiny = p.text_size("Ay", theme::F_TINY).1;
    let interno = m.e(14).max(7);

    for (i, app) in state.apps.iter().enumerate() {
        let row = i as i32 / cols;
        let col = i as i32 % cols;
        if row >= rows {
            break;
        }
        let x = m.safe_x + m.pad + col * (cell_w + gap);
        let y = m.top + m.pad + row * (cell_h + gap);

        p.fill_round_rect(x, y, cell_w, cell_h, r_tile, theme::PANEL2);
        let focused = state.focus.zone == Zone::AppsGrid && state.focus.index == i;
        p.stroke_round_rect(
            x,
            y,
            cell_w,
            cell_h,
            r_tile,
            if focused { theme::ACCENT } else { theme::LINE },
        );

        // Monograma: primeira letra num quadrado, no mesmo tratamento dos
        // cartões de jogo. Evita depender de ícone por app.
        let letter = app.title.chars().next().unwrap_or('?').to_string();
        let mono = ((cell_h as f32 * 0.26) as i32).clamp(m.e(40).max(20), m.e(40).max(20) * 2);
        p.fill_round_rect(
            x + interno,
            y + interno,
            mono,
            mono,
            m.e(8).max(3),
            theme::with_alpha(theme::ACCENT, 40),
        );
        let f_mono = (mono as f32 * 0.5) as u16;
        let ty_mono = y + interno + (mono - p.text_size("A", f_mono).1) / 2;
        p.text_center(x + interno + mono / 2, ty_mono, &letter, f_mono, theme::ACCENT);

        // Texto ancorado no rodapé, como na grade de jogos.
        let cat_y = y + cell_h - interno - h_tiny;
        let title_y = cat_y - m.e(2).max(1) - h_body;
        if title_y > y + interno + mono + 2 {
            let title = truncate(&app.title, cell_w - interno * 2, &mut |s| p.text_size(s, theme::F_BODY).0);
            p.text_left(x + interno, title_y, &title, theme::F_BODY, theme::TEXT);
            let cat = truncate(&app.category, cell_w - interno * 2, &mut |s| p.text_size(s, theme::F_TINY).0);
            p.text_left(x + interno, cat_y, &cat, theme::F_TINY, theme::DIM);
        }

        layout
            .hits
            .push((Rect::new(x, y, cell_w as u32, cell_h as u32), Hit::App(i)));
    }
}

// ════════════════════════════════════════════════════════════
// CONFIGURAÇÕES
// ════════════════════════════════════════════════════════════
pub fn settings(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    // A coluna lateral era 230 px fixos: em 640×480 isso comia metade da tela e
    // deixava 224 px de conteúdo, onde título e valor caíam um sobre o outro.
    let side_w = ((m.safe_w as f32) * 0.30).round().clamp(110.0, 260.0) as i32;
    let side_x = m.safe_x + m.pad;
    let content_x = side_x + side_w + m.e(14).max(8);
    let content_w = (m.safe_x + m.safe_w - m.pad - content_x).max(80);
    let r_chip = m.e(theme::RADIUS_CHIP).max(3);
    let r_caixa = m.e(8).max(3);

    // Linha divisória.
    p.fill_rect(side_x + side_w, m.top + 6, 1, m.main_h - 16, theme::LINE);

    let mut y = m.top + 12;
    for (i, name) in SETTINGS_CATEGORIES.iter().enumerate() {
        let active = state.settings_category == i;
        let focused = state.focus.zone == Zone::SettingsSidebar && state.focus.index == i;
        let row_h = theme::TOUCH;

        let chip_w = side_w - m.e(12).max(6);
        if active {
            p.fill_round_rect(side_x, y, chip_w, row_h, r_chip, theme::CARD);
            p.stroke_round_rect(side_x, y, chip_w, row_h, r_chip, theme::LINE2);
        }
        if focused {
            p.stroke_round_rect(side_x, y, chip_w, row_h, r_chip, theme::ACCENT);
        }

        let color = if active { theme::ACCENT } else { theme::DIM };
        let ty = vcenter(y, row_h, p.text_size(name, theme::F_SMALL).1);
        let label = label_for_category(name);
        let label = truncate(&label, chip_w - m.e(12).max(6) * 2, &mut |s| p.text_size(s, theme::F_SMALL).0);
        p.text_left(side_x + m.e(12).max(6), ty, &label, theme::F_SMALL, color);

        layout
            .hits
            .push((Rect::new(side_x, y, chip_w as u32, row_h as u32), Hit::SettingsCat(i)));
        y += row_h + m.e(3).max(1);
    }

    // Conteúdo.
    let category = SETTINGS_CATEGORIES
        .get(state.settings_category)
        .copied()
        .unwrap_or("appearance");
    let heading = label_for_category(category);
    p.text_left(content_x, m.top + 12, &heading, theme::F_BODY, theme::TEXT);

    let rows: Vec<(String, String)> = match category {
        "appearance" => vec![
            ("Tema Giochi Dark".into(), "Otimizado para OLED".into()),
            ("Escala da interface".into(), "Letterbox centrado, sem corte".into()),
        ],
        "sound" => vec![
            ("Volume geral".into(), format!("{}%", state.settings.volume)),
            ("Saída padrão".into(), "Alto-falante estéreo".into()),
        ],
        "network" => vec![
            ("Wi-Fi".into(), if state.settings.wifi { "Ligado".into() } else { "Desligado".into() }),
            ("Bluetooth".into(), if state.settings.bluetooth { "Ativo".into() } else { "Desligado".into() }),
        ],
        "controller" => vec![
            ("A / Cross".into(), "Confirmar".into()),
            ("B / Circle".into(), "Voltar".into()),
            ("X / Square".into(), "Detalhes".into()),
            ("Y / Triangle".into(), "Buscar".into()),
            ("LB / RB".into(), "Trocar aba".into()),
            ("Select".into(), "Ajustes rápidos".into()),
            ("Start".into(), "Menu de energia".into()),
        ],
        "system" => vec![
            ("Versão".into(), "launcher 0.1.0".into()),
            ("Desempenho".into(), state.settings.perf.label().into()),
        ],
        _ => Vec::new(),
    };

    if category == "about" {
        let lines = [
            "Giochi Launcher — SDL2 com renderer próprio.",
            "",
            "Interface desenhada para gamepad; toque como adicional.",
            "Estado único, reducer, foco por zona, overlays separados da aba.",
            "",
            "Onde não há leitura de sensor, a tela mostra — em vez de",
            "inventar um número.",
        ];
        let mut ay = m.top + m.e(40).max(26);
        let passo = p.text_size("Ay", theme::F_SMALL).1 + m.e(7).max(3);
        for line in lines {
            if ay + passo > m.foot_y {
                break;
            }
            let text = truncate(line, content_w, &mut |s| p.text_size(s, theme::F_SMALL).0);
            p.text_left(content_x, ay, &text, theme::F_SMALL, theme::DIM);
            ay += passo;
        }
    } else {
        let mut ry = m.top + m.e(40).max(26);
        let borda = m.e(12).max(6);
        for (title, value) in &rows {
            if ry + theme::TOUCH > m.foot_y {
                break;
            }
            p.fill_round_rect(content_x, ry, content_w, theme::TOUCH, r_caixa, theme::with_alpha(theme::BLACK, 70));
            p.stroke_round_rect(content_x, ry, content_w, theme::TOUCH, r_caixa, theme::LINE);
            // Título e valor no mesmo y: em 640×480 a coluna tem ~320 px, então
            // o título é truncado pelo espaço que sobra depois do valor. Sem
            // isso os dois se desenham por cima.
            let valor = truncate(value, (content_w / 2).max(40), &mut |s| p.text_size(s, theme::F_SMALL).0);
            let valor_w = p.text_size(&valor, theme::F_SMALL).0;
            let titulo = truncate(title, content_w - valor_w - borda * 3, &mut |s| p.text_size(s, theme::F_SMALL).0);
            let ty = vcenter(ry, theme::TOUCH, p.text_size(title, theme::F_SMALL).1);
            p.text_left(content_x + borda, ty, &titulo, theme::F_SMALL, theme::TEXT);
            let vy = vcenter(ry, theme::TOUCH, p.text_size(value, theme::F_SMALL).1);
            p.text_right(content_x + content_w - borda, vy, &valor, theme::F_SMALL, theme::ACCENT);
            ry += theme::TOUCH + m.e(7).max(3);
        }
    }
}

fn label_for_category(name: &str) -> String {
    match name {
        "appearance" => "Aparência",
        "sound" => "Som",
        "network" => "Rede e BT",
        "controller" => "Controle",
        "system" => "Sistema",
        _ => "Sobre",
    }
    .to_string()
}

// ════════════════════════════════════════════════════════════
// OVERLAYS
// ════════════════════════════════════════════════════════════
/// Uma linha centralizada sobre fundo limpo.
///
/// Usada no instante entre o pedido de desligar e o sistema parar, quando ainda
/// há tempo de desenhar um quadro.
pub fn mensagem_central(p: &mut Painter, texto: &str) {
    let m = Moldura::de(p);
    p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
    p.text_center(m.w / 2, m.h / 2, texto, theme::F_BODY, theme::DIM);
}

/// Tela de falha ao desligar/reiniciar.
///
/// Existe para o caso em que o comando **falha** — `poweroff` que retorna com
/// erro, binário ausente, permissão negada. Mostrar "Sistema desligado" nessa
/// situação é mentira: aqui o usuário lê o motivo real, que vem de
/// `system::executar`.
///
/// **Não** existe porque "o X55 não corta a energia". Isso já foi afirmado aqui
/// e é falso: medido em 03/11/2026, o `poweroff` do X55 desliga de fato. A tela
/// continua útil para qualquer falha — só não pressupõe uma causa específica.
pub fn aviso_sistema(p: &mut Painter, mensagem: &str) {
    let m = Moldura::de(p);
    // Largura e posição vinham de 1280×720 fixos: num painel de 640 px o aviso
    // nascia fora da tela.
    let w = (m.w - m.safe_x).min(m.e(760).max(360));
    let x = m.centro(w);
    let h = m.e(120).max(84);
    let y = m.h - h - m.e(80).max(16);
    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::PANEL2);
    p.stroke_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::DANGER);
    let cx = m.w / 2;
    p.text_center(cx, y + m.e(22).max(10), "Não foi possível desligar o aparelho", theme::F_BODY, theme::DANGER);
    let msg = truncate(mensagem, w - m.e(40).max(16), &mut |s| p.text_size(s, theme::F_TINY).0);
    p.text_center(cx, y + m.e(56).max(30), &msg, theme::F_TINY, theme::TEXT);
    p.text_center(cx, y + m.e(84).max(48), "Segure o botão de energia por 8 s para forçar.", theme::F_TINY, theme::DIM);
    p.text_center(cx, y + m.e(100).max(60), "Quando funciona, o sistema desliga com o brilho apagado.", theme::F_TINY, theme::DIM);
}

pub fn overlays(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    if state.overlays.is_empty() && state.screen == giochi_launcher_core::Screen::Running {
        return;
    }

    // Escurece o fundo.
    p.fill_rect(0, 0, m.w, m.h, theme::with_alpha(theme::BLACK, 200));

    for overlay in &state.overlays {
        match overlay {
            Overlay::QuickSettings => quick_settings(p, state, layout),
            Overlay::GameDetail => game_detail(p, state, layout),
            Overlay::PowerPopup => power_popup(p, state, layout),
            Overlay::Search => search(p, state, layout),
        }
    }

    match state.screen {
        giochi_launcher_core::Screen::Sleeping => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            let lua = m.e(72).max(40);
            p.icon("moon", cx, m.h / 2 - m.e(28).max(12), lua, theme::with_alpha(theme::ACCENT, 130));
            let aviso = "Toque ou pressione qualquer botão para acordar.";
            let aviso = truncate(aviso, m.safe_w, &mut |s| p.text_size(s, theme::F_SMALL).0);
            p.text_center(cx, m.h / 2 + m.e(20).max(10), &aviso, theme::F_SMALL, theme::DIM);
        }
        giochi_launcher_core::Screen::Shutdown => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            p.text_center(cx, m.h / 2 - m.e(12).max(6), "Sistema desligado.", theme::F_BODY, theme::FAINT);
            p.text_center(cx, m.h / 2 + m.e(14).max(8), "Pressione A para reiniciar.", theme::F_SMALL, theme::FAINT);
        }
        giochi_launcher_core::Screen::Reboot => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            p.text_center(m.w / 2, m.h / 2, "Reiniciando...", theme::F_BODY, theme::DIM);
        }
        giochi_launcher_core::Screen::Boot => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            p.text_center(cx, m.h / 2 - m.e(42).max(24), "GIOCHI", theme::F_LOGO, theme::ACCENT);
            p.text_center(cx, m.h / 2 + m.e(2).max(1), "launcher 0.1.0", theme::F_TINY, theme::FAINT);
        }
        giochi_launcher_core::Screen::Running => {}
    }
}

fn panel(p: &mut Painter, x: i32, y: i32, w: i32, h: i32) {
    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::PANEL);
    p.stroke_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::LINE2);
}

fn quick_settings(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    // O painel era 520×388 em y=140 fixos: num painel 480 px de altura ele
    // terminava em y=528, fora da tela, e os dois botões ficavam invisíveis.
    let w = m.safe_w.min(m.e(520).max(300));
    let x = m.centro(w);
    // 6 controles + 2 botões, igual ao `QUICK_SETTINGS_ROWS` do núcleo.
    let rows = QUICK_SETTINGS_ROWS - 2;
    let row_h = m.e(40).max(30);
    let gap_r = m.e(6).max(3);
    let header = m.e(48).max(32);
    let btn_h = m.e(theme::TOUCH).max(30);
    let borda = m.e(16).max(8);
    let h = header + rows as i32 * (row_h + gap_r) + btn_h + borda * 2;
    let y = ((m.h - h) / 2).max(m.top);
    let r_painel = m.e(theme::RADIUS_PANEL).max(4);
    let r_caixa = m.e(8).max(3);
    p.fill_round_rect(x, y, w, h, r_painel, theme::PANEL);
    p.stroke_round_rect(x, y, w, h, r_painel, theme::LINE2);

    p.text_left(x + borda, y + borda, "Ajustes rápidos", theme::F_BODY, theme::TEXT);

    let focused = |index: usize| state.focus.zone == Zone::QuickSettings && state.focus.index == index;
    let mut ry = y + header;

    for index in 0..rows {
        if focused(index) {
            p.stroke_round_rect(x + 8, ry, w - 16, row_h, r_caixa, theme::ACCENT);
        }
        p.fill_round_rect(x + 8, ry, w - 16, row_h, r_caixa, theme::with_alpha(theme::BLACK, 70));

        let (label, icon_name) = match index {
            0 => ("Volume", "vol"),
            1 => ("Brilho", "sun"),
            2 => ("Desempenho", "bolt"),
            3 => ("Wi-Fi", "wifi"),
            4 => ("Bluetooth", "bt"),
            _ => ("HUD de FPS", "chart"),
        };
        let icone = (row_h as f32 * 0.35) as i32;
        p.icon(icon_name, x + borda + icone / 2, ry + row_h / 2, icone, theme::ACCENT);
        let ty = vcenter(ry, row_h, p.text_size(label, theme::F_SMALL).1);
        p.text_left(x + borda + icone + m.e(6).max(3), ty, label, theme::F_SMALL, theme::TEXT);

        // Barra e valor em fração da largura: em 520 px fixos a barra tinha
        // 220 px; num painel estreito ela passava do painel.
        let bar_x = x + (w as f32 * 0.40) as i32;
        let bar_w = (w as f32 * 0.40) as i32;
        match index {
            0 | 1 => {
                let value = if index == 0 { state.settings.volume } else { state.settings.brightness };
                let bar_y = ry + row_h / 2 - 3;
                p.fill_round_rect(bar_x, bar_y, bar_w, 6, 3, theme::with_alpha(theme::WHITE, 30));
                p.fill_round_rect(bar_x, bar_y, (bar_w * value as i32 / 100).max(2), 6, 3, theme::ACCENT);
                p.text_right(x + w - borda, ty, &format!("{value}%"), theme::F_TINY, theme::ACCENT);
            }
            2 => {
                p.text_right(x + w - borda, ty, state.settings.perf.label(), theme::F_TINY, theme::ACCENT);
            }
            _ => {
                let on = match index {
                    3 => state.settings.wifi,
                    4 => state.settings.bluetooth,
                    _ => state.settings.hud,
                };
                let trilho = m.e(34).max(22);
                let knob_x = x + w - borda - trilho;
                let trilho_y = ry + row_h / 2 - m.e(8).max(5);
                p.fill_round_rect(knob_x, trilho_y, trilho, m.e(16).max(10), m.e(8).max(3), if on { theme::with_alpha(theme::ACCENT, 80) } else { theme::with_alpha(theme::WHITE, 30) });
                let r = (m.e(7).max(4)) as i32;
                let dot_x = if on { knob_x + trilho - r * 2 - m.e(2).max(1) } else { knob_x + m.e(2).max(1) };
                p.fill_circle(dot_x + r, trilho_y + m.e(8).max(5), r, if on { theme::ACCENT } else { theme::DIM });
            }
        }

        layout
            .hits
            .push((Rect::new(x + 8, ry, (w - 16) as u32, row_h as u32), Hit::QsRow(index)));
        ry += row_h + 6;
    }

    // Ações.
    let btn_w = (w - borda * 2 - m.e(8).max(4)) / 2;
    let by = y + h - borda - btn_h;
    for (i, label) in ["Suspender", "Reiniciar"].iter().enumerate() {
        let bx = x + 8 + i as i32 * (btn_w + 8);
        let danger = i == 1;
        p.fill_round_rect(
            bx,
            by,
            btn_w,
            btn_h,
            r_caixa,
            if danger { theme::with_alpha(theme::DANGER, 40) } else { theme::PANEL2 },
        );
        if focused(6 + i) {
            p.stroke_round_rect(bx, by, btn_w, btn_h, r_caixa, theme::ACCENT);
        }
        let ty = vcenter(by, btn_h, p.text_size(label, theme::F_SMALL).1);
        p.text_center(bx + btn_w / 2, ty, label, theme::F_SMALL, if danger { theme::DANGER } else { theme::TEXT });
        layout.hits.push((
            Rect::new(bx, by, btn_w as u32, btn_h as u32),
            Hit::QsButton(6 + i as u8),
        ));
    }
}

fn game_detail(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let Some(game) = state.selected_game.and_then(|i| state.games.get(i)) else {
        return;
    };

    // Largura de 760 fixa dava x negativo num painel de 640: o painel nascia
    // fora da tela dos dois lados.
    let w = m.safe_w.min(m.e(760).max(320));
    let x = m.centro(w);
    let borda = m.e(24).max(10);
    let hero_h = m.e(210).max(120);
    let acoes_h = m.e(theme::TOUCH).max(30) + m.e(24).max(8);
    let h = hero_h + acoes_h;
    let y = ((m.h - h) / 2).max(m.top);
    let r_painel = m.e(theme::RADIUS_PANEL).max(4);

    p.fill_round_rect(x, y, w, h, r_painel, theme::PANEL);

    let accent = theme::game_accent(game.id);
    let base = theme::game_base(game.id);
    p.vgradient_round_rect(x, y, w, hero_h, r_painel, base, theme::mix(base, theme::BLACK, 0.4));

    let letter = game.title.chars().next().unwrap_or('?').to_string();
    let letter_px = ((hero_h as f32) * 0.52).max(24.0) as u16;
    p.text_right(x + w - borda, y + m.e(4).max(2), &letter, letter_px, theme::with_alpha(theme::WHITE, 18));

    p.text_left(x + borda, y + m.e(22).max(10), &game.category.to_uppercase(), theme::F_TINY, accent);
    let titulo = truncate(&game.title, w - borda * 2, &mut |s| p.text_size(s, theme::F_HERO).0);
    p.text_left(x + borda, y + m.e(42).max(20), &titulo, theme::F_HERO, theme::WHITE);

    // Estatísticas.
    let stats = [
        ("ANO", game.year_label()),
        ("HORAS", game.play_label()),
        ("ÚLTIMA", game.last_played.clone()),
        ("FAVORITO", if game.favorite { "Sim".into() } else { "—".into() }),
    ];
    // As estatísticas se distribuem na largura útil em vez de andarem 150 px
    // fixos: em 640×480 a quarta coluna caía fora do painel.
    let coluna = (w - borda * 2) / stats.len() as i32;
    let stats_y = y + hero_h - m.e(90).max(46);
    for (i, (label, value)) in stats.iter().enumerate() {
        let sx = x + borda + coluna * i as i32;
        p.text_left(sx, stats_y, label, theme::F_TINY, theme::with_alpha(theme::WHITE, 140));
        let v = truncate(value, coluna - m.e(8).max(4), &mut |s| p.text_size(s, theme::F_BODY).0);
        p.text_left(sx, stats_y + m.e(16).max(8), &v, theme::F_BODY, theme::WHITE);
    }

    // Ações.
    let labels = [
        "Jogar".to_string(),
        if game.favorite { "Favoritado".into() } else { "Favoritar".into() },
        "Opções".to_string(),
        "Voltar".to_string(),
    ];
    let gap = m.e(8).max(3);
    let btn_w = ((w - borda * 2 - gap * (labels.len() as i32 - 1)) / labels.len() as i32).max(40);
    let btn_h = m.e(theme::TOUCH).max(30);
    let mut bx = x + borda;
    let by = y + hero_h + m.e(12).max(4);
    for (i, label) in labels.iter().enumerate() {
        let focused = state.focus.zone == Zone::GameDetailActions && state.focus.index == i;
        let primary = i == 0;
        let r = m.e(8).max(3);
        p.fill_round_rect(bx, by, btn_w, btn_h, r, if primary { theme::ACCENT } else { theme::PANEL2 });
        if focused {
            p.stroke_round_rect(bx, by, btn_w, btn_h, r, theme::WHITE);
        }
        let color = if primary { theme::BG } else { theme::TEXT };
        let ty = vcenter(by, btn_h, p.text_size(label, theme::F_SMALL).1);
        let rotulo = truncate(label, btn_w - m.e(12).max(6), &mut |s| p.text_size(s, theme::F_SMALL).0);
        p.text_center(bx + btn_w / 2, ty, &rotulo, theme::F_SMALL, color);
        layout.hits.push((
            Rect::new(bx, by, btn_w as u32, btn_h as u32),
            Hit::DetailButton(i),
        ));
        bx += btn_w + gap;
    }
}

fn power_popup(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    // 340×340 em y=190 fixos terminavam em y=530 num painel de 480 de altura:
    // metade do menu ficava fora da tela.
    let labels = ["Suspender", "Reiniciar", "Desligar", "Cancelar"];
    let borda = m.e(20).max(8);
    let btn_h = m.e(theme::TOUCH).max(30);
    let gap = m.e(5).max(2);
    let icone = m.e(48).max(28);
    let header = icone + m.e(46).max(24);
    let w = m.safe_w.min(m.e(340).max(260));
    let x = m.centro(w);
    let h = header + labels.len() as i32 * (btn_h + gap) + borda;
    let y = ((m.h - h) / 2).max(m.top);
    let r_painel = m.e(theme::RADIUS_PANEL).max(4);
    let r_btn = m.e(7).max(3);
    p.fill_round_rect(x, y, w, h, r_painel, theme::PANEL);
    p.stroke_round_rect(x, y, w, h, r_painel, theme::LINE2);

    let cx = x + w / 2;
    p.fill_round_rect(cx - icone / 2, y + m.e(20).max(8), icone, icone, m.e(12).max(4), theme::with_alpha(theme::DANGER, 40));
    p.icon("power", cx, y + m.e(20).max(8) + icone / 2, (icone as f32 * 0.46) as i32, theme::DANGER);

    p.text_center(cx, y + icone + m.e(12).max(4), "Opções de energia", theme::F_BODY, theme::TEXT);
    p.text_center(cx, y + icone + m.e(34).max(18), "O que deseja fazer?", theme::F_TINY, theme::DIM);

    let mut by = y + header;
    for (i, label) in labels.iter().enumerate() {
        let danger = i == 2;
        let focused = state.focus.zone == Zone::PowerActions && state.focus.index == i;
        p.fill_round_rect(
            x + borda,
            by,
            w - borda * 2,
            btn_h,
            r_btn,
            if danger { theme::with_alpha(theme::DANGER, 200) } else { theme::PANEL2 },
        );
        if focused {
            p.stroke_round_rect(x + borda, by, w - borda * 2, btn_h, r_btn, theme::ACCENT);
        }
        let color = if danger { theme::WHITE } else if i == 3 { theme::DIM } else { theme::TEXT };
        let ty = vcenter(by, btn_h, p.text_size(label, theme::F_SMALL).1);
        p.text_center(cx, ty, label, theme::F_SMALL, color);
        layout.hits.push((
            Rect::new(x + borda, by, (w - borda * 2) as u32, btn_h as u32),
            Hit::PowerButton(i),
        ));
        by += btn_h + gap;
    }
}

fn search(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    // 900×520 em y=110 fixos: num painel 640×480 o painel nascia em x=-130 e
    // terminava em y=630. Agora a caixa cabe, e as teclas se dividem na largura
    // que sobra em vez de manterem 52 px cada.
    let borda = m.e(16).max(6);
    let linha_y = m.e(48).max(26);
    let gap = m.e(5).max(2);
    let cols_res = if m.compacto { 0 } else { 1 };
    let key_w_base = m.e(52).max(26);
    let min_w_teclado = KEYBOARD_COLS as i32 * key_w_base + (KEYBOARD_COLS as i32 - 1) * gap + borda * 2;
    let w = m.safe_w.min(m.e(900).max(min_w_teclado));
    // Com resultados ao lado, a área do teclado fica com ~60% da largura.
    let faixa_teclado = if cols_res == 1 { (w as f32 * 0.62) as i32 } else { w - borda * 2 };
    let key_w = ((faixa_teclado - gap * (KEYBOARD_COLS as i32 - 1)) / KEYBOARD_COLS as i32).max(16);
    let key_h = m.e(theme::TOUCH).max(26);
    let h_teclado = linha_y + KEYBOARD.len() as i32 * (key_h + gap) + borda;
    let h = m.main_h.min(m.e(520).max(h_teclado));
    let x = m.centro(w);
    let y = m.top + (m.main_h - h) / 2;
    let r_painel = m.e(theme::RADIUS_PANEL).max(4);
    p.fill_round_rect(x, y, w, h, r_painel, theme::PANEL);
    p.stroke_round_rect(x, y, w, h, r_painel, theme::LINE2);

    p.icon("search", x + borda + m.e(10).max(4), y + m.e(30).max(14), m.e(18).max(10), theme::ACCENT);
    let query = if state.search_query.is_empty() {
        "Buscar jogo...".to_string()
    } else {
        state.search_query.clone()
    };
    let qcolor = if state.search_query.is_empty() { theme::FAINT } else { theme::TEXT };
    p.text_left(x + borda + m.e(28).max(12), y + m.e(22).max(10), &query, theme::F_BODY, qcolor);
    p.fill_rect(x, y + linha_y, w, 1, theme::LINE);

    // Teclado.
    let kb_x = x + borda;
    let kb_y = y + linha_y + m.e(12).max(4);
    let r_tecla = m.e(7).max(3);

    for row in 0..KEYBOARD.len() {
        for col in 0..KEYBOARD_COLS {
            let index = row * KEYBOARD_COLS + col;
            let kx = kb_x + col as i32 * (key_w + gap);
            let ky = kb_y + row as i32 * (key_h + gap);
            let focused = state.focus.zone == Zone::SearchKeyboard && state.focus.index == index;

            p.fill_round_rect(kx, ky, key_w, key_h, r_tecla, theme::PANEL2);
            p.stroke_round_rect(kx, ky, key_w, key_h, r_tecla, if focused { theme::ACCENT } else { theme::LINE });

            let ch = KEYBOARD[row][col];
            let label = if ch == ' ' {
                "ESP".to_string()
            } else if ch == '\u{8}' {
                "<".to_string()
            } else {
                ch.to_string()
            };
            let color = if focused { theme::ACCENT } else { theme::TEXT };
            let ty = vcenter(ky, key_h, p.text_size(&label, theme::F_SMALL).1);
            p.text_center(kx + key_w / 2, ty, &label, theme::F_SMALL, color);

            layout.hits.push((
                Rect::new(kx, ky, key_w as u32, key_h as u32),
                Hit::Key(index),
            ));
        }
    }

    // Resultados.
    //
    // Com resultados ao lado (painel largo) a coluna fica à direita do teclado.
    // Num painel estreito não sobra largura para isso: em 640×480 o teclado
    // ocupa a caixa inteira e a coluna era espremida em 60 px na borda, por
    // cima das teclas. Ali os resultados vão **abaixo** do teclado.
    let results = state.search_results();
    let abaixo = m.compacto;
    let (res_x, res_w, res_topo) = if abaixo {
        (
            x + borda,
            w - borda * 2,
            kb_y + KEYBOARD.len() as i32 * (key_h + gap) + m.e(8).max(4),
        )
    } else {
        let rx = kb_x + KEYBOARD_COLS as i32 * (key_w + gap) + m.e(14).max(6);
        (rx, (x + w - borda - rx).max(60), kb_y)
    };
    p.text_left(res_x, res_topo, &format!("Resultados ({})", results.len()), theme::F_TINY, theme::DIM);

    let mut ry = res_topo + m.e(20).max(10);
    let clip_top = ry;
    let clip_h = (y + h - borda - ry).max(20);
    p.push_clip(Clip { x: res_x, y: clip_top, w: res_w, h: clip_h });

    if results.is_empty() {
        p.text_center(res_x + res_w / 2, ry + 30, "Digite para buscar...", theme::F_SMALL, theme::FAINT);
    } else {
        for (i, game_index) in results.iter().enumerate() {
            let game = &state.games[*game_index];
            let row_h = theme::TOUCH;
            let focused = state.focus.zone == Zone::SearchResults && state.focus.index == i;

            p.fill_round_rect(res_x, ry, res_w, row_h - 4, 7, theme::PANEL2);
            p.stroke_round_rect(res_x, ry, res_w, row_h - 4, 7, if focused { theme::ACCENT } else { theme::LINE });

            let accent = theme::game_accent(game.id);
            p.fill_round_rect(res_x + 8, ry + 8, 28, 28, 5, accent);

            let title = truncate(&game.title, res_w - 60, &mut |s| p.text_size(s, theme::F_SMALL).0);
            let ty = vcenter(ry, row_h - 4, p.text_size(&title, theme::F_SMALL).1);
            p.text_left(res_x + 44, ty, &title, theme::F_SMALL, theme::TEXT);

            layout.hits.push((
                Rect::new(res_x, ry, res_w as u32, (row_h - 4) as u32),
                Hit::Result(i),
            ));
            ry += row_h;
        }
    }

    p.pop_clip();
}

// ════════════════════════════════════════════════════════════
// AVISOS
// ════════════════════════════════════════════════════════════
pub fn toast(p: &mut Painter, state: &State) {
    let m = Moldura::de(p);
    if !state.toast.visible || state.toast.message.is_empty() {
        return;
    }
    let text = &state.toast.message;
    let tw = p.text_size(text, theme::F_SMALL).0;
    let w = (tw + m.e(60).max(30)).min(m.safe_w);
    let h = m.e(38).max(24);
    let x = m.w / 2 - w / 2;
    let y = m.h - m.e(64) - h;

    panel(p, x, y, w, h);
    p.icon("check", x + 22, y + h / 2, 14, theme::ACCENT);
    let ty = vcenter(y, h, p.text_size(text, theme::F_SMALL).1);
    p.text_left(x + 38, ty, text, theme::F_SMALL, theme::TEXT);
}

pub fn hud(p: &mut Painter, state: &State) {
    let m = Moldura::de(p);
    if !state.settings.hud {
        return;
    }
    let w = 170;
    let h = 40;
    let x = m.w - m.e(170) - m.e(170);
    let y = 66;

    p.fill_round_rect(x, y, w, h, 8, theme::with_alpha(theme::BLACK, 180));
    p.stroke_round_rect(x, y, w, h, 8, theme::LINE2);

    let items = [
        ("FPS", if state.hud.fps > 0 { state.hud.fps.to_string() } else { "—".into() }),
        ("TEMP", "—".into()),
        ("TDP", "—".into()),
    ];
    let mut ix = x + 14;
    for (label, value) in items {
        p.text_left(ix, y + 8, label, theme::F_TINY, theme::DIM);
        p.text_left(ix, y + 20, &value, theme::F_SMALL, theme::ACCENT);
        ix += 54;
    }
}

fn truncate(text: &str, max_w: i32, measure: &mut impl FnMut(&str) -> i32) -> String {
    if measure(text) <= max_w {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let candidate = format!("{out}{ch}");
        if measure(&format!("{candidate}…")) > max_w {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

/// Extensão local para rótulos que não existem no núcleo.
trait GameLabel {
    fn year_label(&self) -> String;
    fn play_label(&self) -> String;
}

impl GameLabel for giochi_launcher_core::Game {
    fn year_label(&self) -> String {
        // O núcleo não carrega ano: usa a categoria e o id como fallback
        // estável até o catálogo trazer o campo.
        format!("#{:03}", self.id)
    }

    fn play_label(&self) -> String {
        if self.hours > 0.0 {
            format!("{:.1}h", self.hours)
        } else {
            "nunca jogado".to_string()
        }
    }
}


#[cfg(test)]
mod tests {
    use super::grade_para;

    /// As escolhas medidas nos painéis que existem de fato.
    ///
    /// 640×480 e 1280×720 são aparelhos reais (X35H e X55); os números aqui são
    /// os que o app desenha, não exemplos inventados.
    #[test]
    fn grade_de_quatro_itens_usa_duas_colunas() {
        // Área útil medida no X35H: safe_w 480 menos 12 de padding, 377 de altura.
        assert_eq!(grade_para(4, 468, 377, 5), (2, 2));
        // E no X55, 936×538: 4 colunas por 1 linha deixariam a célula em pé.
        assert_eq!(grade_para(4, 936, 538, 9), (2, 2));
    }

    #[test]
    fn grade_de_oito_itens_mantem_quatro_colunas_no_painel_grande() {
        assert_eq!(grade_para(8, 936, 538, 9), (4, 2));
    }

    #[test]
    fn grade_de_tres_itens_nao_deixa_coluna_sobrando() {
        assert_eq!(grade_para(3, 468, 413, 5), (2, 2));
    }

    /// A célula precisa caber um título: uma tira de 30 px de largura tem área,
    /// mas não serve. `grade_para` recusa proporções fora de 0,62..2,2.
    #[test]
    fn grade_nunca_escolhe_celula_em_tira() {
        for itens in 1..=12 {
            for (w, h) in [(468, 377), (936, 538), (600, 300), (300, 400)] {
                let (cols, rows) = grade_para(itens, w, h, 5);
                let cw = (w - 5 * (cols - 1)) / cols;
                let ch = (h - 5 * (rows - 1)) / rows;
                assert!(cols >= 1 && rows >= 1, "grade vazia para {itens} itens");
                assert!(
                    (cols * rows) as usize >= itens,
                    "{itens} itens não cabem em {cols}x{rows}"
                );
                let aspecto = cw as f32 / ch as f32;
                if itens > 1 {
                    assert!(
                        (0.62..=2.2).contains(&aspecto) || cols == 1,
                        "célula {cw}x{ch} (aspecto {aspecto:.2}) para {itens} itens"
                    );
                }
            }
        }
    }

    /// Um item só não tem com o que comparar: a grade degenera para 1×1.
    #[test]
    fn grade_de_um_item_ocupa_tudo() {
        assert_eq!(grade_para(1, 468, 377, 5), (1, 1));
    }
}
