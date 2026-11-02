//! Desenho das telas e coleta de áreas tocáveis.
//!
//! Cada função desenha e, quando um elemento é interativo, registra seu
//! retângulo num [`Layout`]. É assim que o toque funciona sem duplicar
//! geometria: quem desenha sabe onde o elemento está, então é quem registra.
//! Separar "desenhar" de "calcular posições" seria a receita para os dois
//! saírem de sincronia.

use giochi_launcher_core::state::{Filter, Overlay, Tab, KEYBOARD, KEYBOARD_COLS, SETTINGS_CATEGORIES};
use giochi_launcher_core::{State, Zone};
use sdl2::rect::Rect;

use crate::draw::{Clip, Painter};
use crate::icons;
use crate::theme;

pub const SAFE_X: i32 = theme::SAFE_X;
pub const SAFE_W: i32 = theme::SAFE_W;
pub const PAD: i32 = 12;
pub const TOP: i32 = theme::TOPBAR_H;
pub const MAIN_H: i32 = 720 - theme::TOPBAR_H - theme::FOOTER_H;
pub const FOOT_Y: i32 = theme::TOPBAR_H + MAIN_H;
pub const LIB_PAGE: usize = 8;

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
    p.fill_rect(SAFE_X, TOP - 1, SAFE_W, 1, theme::LINE);

    let blocked = state.has_overlay() || state.screen != giochi_launcher_core::Screen::Running;
    let mut x = SAFE_X;

    for tab in Tab::ALL {
        let label = tab.label();
        let text_w = p.text_size(label, theme::F_SMALL).0;
        let w = text_w + 34 + 22;
        let active = tab == state.tab;

        if active {
            p.fill_round_rect(x, 10, w, 36, theme::RADIUS_CHIP, theme::CARD);
            p.stroke_round_rect(x, 10, w, 36, theme::RADIUS_CHIP, theme::LINE2);
        }
        let color = if active { theme::TEXT } else { theme::DIM };
        let icon_name = match tab {
            Tab::Home => "home",
            Tab::Library => "gamepad",
            Tab::Apps => "apps",
            Tab::Settings => "settings",
        };
        p.icon(icon_name, x + 18, 28, 14, color);
        let ty = vcenter(10, 36, p.text_size(label, theme::F_SMALL).1);
        p.text_left(x + 32, ty, label, theme::F_SMALL, color);

        if !blocked {
            layout.hits.push((Rect::new(x, 10, w as u32, 36), Hit::Tab(tab)));
        }
        x += w + 3;
    }

    // Lado direito: controle, wifi, bateria, relógio, energia.
    let mut rx = SAFE_X + SAFE_W;

    // Botão de energia.
    let power_w = 34;
    rx -= power_w;
    p.fill_round_rect(rx, 11, power_w, 34, 6, theme::with_alpha(theme::DANGER, 30));
    p.stroke_round_rect(rx, 11, power_w, 34, 6, theme::with_alpha(theme::DANGER, 90));
    p.icon("power", rx + power_w / 2, 28, 16, theme::DANGER);
    layout
        .hits
        .push((Rect::new(rx, 11, power_w as u32, 34), Hit::PowerTap));
    rx -= 8;

    // Relógio.
    let clock = current_clock();
    let clock_w = p.text_size(&clock, theme::F_BODY).0;
    rx -= clock_w;
    p.text_right(rx + clock_w, 22, &clock, theme::F_BODY, theme::TEXT);
    rx -= 10;

    // Bateria.
    let battery = state.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
    rx = badge(p, layout, rx, &battery, theme::TEXT, None);
    rx -= 6;

    // Wi-Fi.
    let wifi_on = state.settings.wifi;
    rx = badge(p, layout, rx, "Wi-Fi", if wifi_on { theme::OK } else { theme::DIM }, Some("wifi"));
    rx -= 6;

    // Controle: mostra estado real, não decoração.
    let pad_label = if state.pad_connected { "ON" } else { "OFF" };
    let pad_color = if state.pad_connected { theme::OK } else { theme::DIM };
    rx = badge(p, layout, rx, pad_label, pad_color, Some("gamepad"));
    rx -= 6;

    // Ajustes rápidos.
    rx -= 34;
    p.fill_round_rect(rx, 11, 34, 34, 6, theme::PANEL2);
    p.icon("settings", rx + 17, 28, 16, theme::DIM);
    rx -= 8;

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
    p.fill_rect(SAFE_X, FOOT_Y, SAFE_W, 1, theme::LINE);

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

    let mut x = SAFE_X + 4;
    let y = FOOT_Y + 18;
    for (key, label) in hints {
        let key_w = p.text_size(key, theme::F_TINY).0 + 10;
        p.fill_round_rect(x, y - 3, key_w, 16, 3, theme::with_alpha(theme::WHITE, 20));
        let ky = vcenter(y - 3, 16, p.text_size(key, theme::F_TINY).1);
        p.text_left(x + 5, ky, key, theme::F_TINY, theme::ACCENT);
        x += key_w + 5;
        let lw = p.text_left(x, y, label, theme::F_TINY, theme::DIM);
        x += lw + 12;
    }

    let build = format!("giochi-launcher 0.1.0 • {}", state.settings.perf.label());
    p.text_right(SAFE_X + SAFE_W - 4, y, &build, theme::F_TINY, theme::FAINT);
}

// ════════════════════════════════════════════════════════════
// HOME — carrossel
// ════════════════════════════════════════════════════════════
pub fn home(p: &mut Painter, state: &State, layout: &mut Layout) {
    let n = state.games.len();
    if n == 0 {
        p.text_center(SAFE_X + SAFE_W / 2, TOP + MAIN_H / 2, "Catálogo vazio.", theme::F_SMALL, theme::FAINT);
        return;
    }

    let center_y = TOP + MAIN_H / 2 - 20;
    let card_w = 520;
    let card_h = 320;

    // Desenha laterais primeiro, para o focado ficar por cima.
    for offset in [-1i32, 1, 0] {
        let idx = ((state.carousel as i32 + offset).rem_euclid(n as i32)) as usize;
        let game = &state.games[idx];
        let focused = offset == 0;
        let scale = if focused { 1.0 } else { 0.62 };
        let w = (card_w as f32 * scale) as i32;
        let h = (card_h as f32 * scale) as i32;
        let cx = SAFE_X + SAFE_W / 2 + (offset * 420);
        let x = cx - w / 2;
        let y = center_y - h / 2;

        let accent = theme::game_accent(game.id);
        let base = theme::game_base(game.id);
        let radius = (theme::RADIUS_CARD as f32 * scale) as i32;

        if focused {
            // Halo: anel largo antes do cartão.
            p.stroke_round_rect(x - 2, y - 2, w + 4, h + 4, radius + 2, accent);
        }
        p.vgradient_round_rect(x, y, w, h, radius, base, theme::mix(base, theme::BLACK, 0.35));

        if !focused {
            // Escurece as laterais para dar profundidade.
            p.fill_round_rect(x, y, w, h, radius, theme::with_alpha(theme::BLACK, 140));
            continue;
        }

        // Letra grande ao fundo, como no protótipo.
        let letter = game.title.chars().next().unwrap_or('?').to_string();
        p.text_right(x + w - 24, y + 10, &letter, 120, theme::with_alpha(theme::WHITE, 16));

        // Categoria.
        let cat = game.category.to_uppercase();
        let cy = y + h - 132;
        p.text_left(x + 24, cy, &cat, theme::F_TINY, accent);

        // Título (quebra simples em duas linhas, se preciso).
        let title = &game.title;
        let title_w = p.text_size(title, theme::F_TITLE).0;
        if title_w <= w - 48 {
            p.text_left(x + 24, cy + 16, title, theme::F_TITLE, theme::WHITE);
        } else {
            // Quebra no último espaço que caiba.
            let (a, b) = split_title(title, w - 48, &mut |s| p.text_size(s, theme::F_TITLE).0);
            p.text_left(x + 24, cy + 10, &a, theme::F_TITLE, theme::WHITE);
            p.text_left(x + 24, cy + 38, &b, theme::F_TITLE, theme::WHITE);
        }

        let meta = format!("{} • {}", game.year_label(), game.play_label());
        p.text_left(x + 24, cy + 86, &meta, theme::F_SMALL, theme::with_alpha(theme::WHITE, 170));

        // Botão "Jogar".
        let btn_w = 132;
        let btn_h = theme::TOUCH;
        let bx = x + 24;
        let by = y + h - 56;
        p.fill_round_rect(bx, by, btn_w, btn_h, 8, theme::with_alpha(theme::WHITE, 40));
        p.stroke_round_rect(bx, by, btn_w, btn_h, 8, theme::with_alpha(theme::WHITE, 80));
        p.icon("play", bx + 20, by + btn_h / 2, 12, theme::WHITE);
        let ty = vcenter(by, btn_h, p.text_size("Jogar", theme::F_SMALL).1);
        p.text_left(bx + 34, ty, "Jogar", theme::F_SMALL, theme::WHITE);

        layout
            .hits
            .push((Rect::new(x, y, w as u32, h as u32), Hit::Carousel(idx)));
    }

    // Pontos de posição.
    let dots = state.games.len();
    let dot_gap = 11;
    let total = dots as i32 * dot_gap;
    let mut dx = SAFE_X + SAFE_W / 2 - total / 2;
    let dy = center_y + card_h / 2 + 24;
    for i in 0..dots {
        if i == state.carousel {
            p.fill_round_rect(dx, dy, 18, 6, 3, theme::ACCENT);
            dx += 18 + (dot_gap - 18);
        } else {
            p.fill_circle(dx + 3, dy + 3, 3, theme::with_alpha(theme::WHITE, 40));
            dx += dot_gap;
        }
    }

    // Contexto abaixo dos pontos.
    if let Some(game) = state.focused_game() {
        let line = format!("{} • {} • {}", game.title, game.category, game.year_label());
        p.text_center(SAFE_X + SAFE_W / 2, dy + 18, &line, theme::F_SMALL, theme::DIM);
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
    let games = state.filtered_games();
    let focused_index = if state.focus.zone == Zone::LibraryGrid {
        Some(state.focus.index)
    } else {
        None
    };
    let page = focused_index.map(|i| i / LIB_PAGE).unwrap_or(0);
    let pages = games.len().div_ceil(LIB_PAGE).max(1);

    // Cabeçalho com filtros.
    p.text_left(SAFE_X + 4, TOP + 12, "BIBLIOTECA", theme::F_TINY, theme::DIM);

    let mut fx = SAFE_X + SAFE_W - 4;
    for filter in Filter::ALL.iter().rev() {
        let label = filter.label();
        let w = p.text_size(label, theme::F_TINY).0 + 24;
        fx -= w;
        let active = state.library_filter == *filter;
        let focused = state.focus.zone == Zone::LibraryFilters
            && Filter::ALL.iter().position(|f| f == filter) == Some(state.focus.index);

        if active {
            p.fill_round_rect(fx, TOP + 6, w, 30, 15, theme::CARD);
            p.stroke_round_rect(fx, TOP + 6, w, 30, 15, theme::LINE2);
        }
        if focused {
            p.stroke_round_rect(fx, TOP + 6, w, 30, 15, theme::ACCENT);
        }
        let color = if active { theme::TEXT } else { theme::DIM };
        let ty = vcenter(TOP + 6, 30, p.text_size(label, theme::F_TINY).1);
        p.text_center(fx + w / 2, ty, label, theme::F_TINY, color);
        layout
            .hits
            .push((Rect::new(fx, TOP + 6, w as u32, 30), Hit::Filter(*filter)));

        fx -= 5;
    }

    let grid_top = TOP + 48;
    let grid_h = MAIN_H - 48 - 24;
    let cols = 4;
    let rows = 2;
    let gap = 9;
    let cell_w = (SAFE_W - PAD * 2 - gap * (cols - 1)) / cols;
    let cell_h = (grid_h - gap * (rows - 1)) / rows;

    if games.is_empty() {
        p.text_center(
            SAFE_X + SAFE_W / 2,
            grid_top + grid_h / 2,
            "Nenhum jogo nesta categoria",
            theme::F_SMALL,
            theme::FAINT,
        );
        return;
    }

    let start = page * LIB_PAGE;
    for slot in 0..LIB_PAGE {
        let Some(game_index) = games.get(start + slot).copied() else {
            break;
        };
        let game = &state.games[game_index];
        let row = slot / cols;
        let col = slot % cols;
        let x = SAFE_X + PAD + col * (cell_w + gap);
        let y = grid_top + row * (cell_h + gap);

        let accent = theme::game_accent(game.id);
        let base = theme::game_base(game.id);
        p.vgradient_round_rect(x, y, cell_w, cell_h, theme::RADIUS_TILE, base, theme::mix(base, theme::BLACK, 0.4));

        let focused = focused_index == Some(start + slot);
        if focused {
            p.stroke_round_rect(x, y, cell_w, cell_h, theme::RADIUS_TILE, accent);
        }

        if game.favorite {
            p.icon("star", x + cell_w - 16, y + 16, 12, theme::WARN);
        }

        // Etiqueta de categoria.
        let cat = game.category.to_uppercase();
        let cw = p.text_size(&cat, theme::F_TINY).0 + 12;
        p.fill_round_rect(x + 10, y + 10, cw, 16, 3, theme::with_alpha(theme::BLACK, 120));
        p.text_left(x + 16, y + 12, &cat, theme::F_TINY, accent);

        // Título truncado.
        let title = truncate(&game.title, cell_w - 20, &mut |s| p.text_size(s, theme::F_BODY).0);
        p.text_left(x + 10, y + cell_h - 32, &title, theme::F_BODY, theme::WHITE);
        p.text_left(x + 10, y + cell_h - 16, &game.year_label(), theme::F_TINY, theme::with_alpha(theme::WHITE, 140));

        layout
            .hits
            .push((Rect::new(x, y, cell_w as u32, cell_h as u32), Hit::Game(start + slot)));
    }

    let pager = format!("{} jogo(s) • página {}/{}", games.len(), page + 1, pages);
    p.text_center(SAFE_X + SAFE_W / 2, FOOT_Y - 20, &pager, theme::F_TINY, theme::DIM);
}

// ════════════════════════════════════════════════════════════
// APPS
// ════════════════════════════════════════════════════════════
pub fn apps(p: &mut Painter, state: &State, layout: &mut Layout) {
    let cols = 4;
    let rows = 2;
    let gap = 9;
    let cell_w = (SAFE_W - PAD * 2 - gap * (cols - 1)) / cols;
    let cell_h = (MAIN_H - PAD * 2 - gap * (rows - 1)) / rows;

    for (i, app) in state.apps.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        if row >= rows {
            break;
        }
        let x = SAFE_X + PAD + col * (cell_w + gap);
        let y = TOP + PAD + row * (cell_h + gap);

        p.fill_round_rect(x, y, cell_w, cell_h, theme::RADIUS_TILE, theme::PANEL2);
        let focused = state.focus.zone == Zone::AppsGrid && state.focus.index == i;
        p.stroke_round_rect(
            x,
            y,
            cell_w,
            cell_h,
            theme::RADIUS_TILE,
            if focused { theme::ACCENT } else { theme::LINE },
        );

        // Monograma: primeira letra num quadrado, no mesmo tratamento dos
        // cartões de jogo. Evita depender de ícone por app.
        let letter = app.title.chars().next().unwrap_or('?').to_string();
        p.fill_round_rect(x + 14, y + 14, 40, 40, 8, theme::with_alpha(theme::ACCENT, 40));
        p.text_center(x + 34, y + 24, &letter, 20, theme::ACCENT);

        p.text_left(x + 14, y + cell_h - 44, &app.title, theme::F_BODY, theme::TEXT);
        p.text_left(x + 14, y + cell_h - 26, &app.category, theme::F_TINY, theme::DIM);

        layout
            .hits
            .push((Rect::new(x, y, cell_w as u32, cell_h as u32), Hit::App(i)));
    }
}

// ════════════════════════════════════════════════════════════
// CONFIGURAÇÕES
// ════════════════════════════════════════════════════════════
pub fn settings(p: &mut Painter, state: &State, layout: &mut Layout) {
    let side_w = 230;
    let side_x = SAFE_X + PAD;
    let content_x = side_x + side_w + 14;
    let content_w = SAFE_X + SAFE_W - PAD - content_x;

    // Linha divisória.
    p.fill_rect(side_x + side_w, TOP + 6, 1, MAIN_H - 16, theme::LINE);

    let mut y = TOP + 12;
    for (i, name) in SETTINGS_CATEGORIES.iter().enumerate() {
        let active = state.settings_category == i;
        let focused = state.focus.zone == Zone::SettingsSidebar && state.focus.index == i;
        let row_h = theme::TOUCH;

        if active {
            p.fill_round_rect(side_x, y, side_w - 12, row_h, theme::RADIUS_CHIP, theme::CARD);
            p.stroke_round_rect(side_x, y, side_w - 12, row_h, theme::RADIUS_CHIP, theme::LINE2);
        }
        if focused {
            p.stroke_round_rect(side_x, y, side_w - 12, row_h, theme::RADIUS_CHIP, theme::ACCENT);
        }

        let color = if active { theme::ACCENT } else { theme::DIM };
        let ty = vcenter(y, row_h, p.text_size(name, theme::F_SMALL).1);
        p.text_left(side_x + 12, ty, &label_for_category(name), theme::F_SMALL, color);

        layout
            .hits
            .push((Rect::new(side_x, y, (side_w - 12) as u32, row_h as u32), Hit::SettingsCat(i)));
        y += row_h + 3;
    }

    // Conteúdo.
    let category = SETTINGS_CATEGORIES
        .get(state.settings_category)
        .copied()
        .unwrap_or("appearance");
    let heading = label_for_category(category);
    p.text_left(content_x, TOP + 12, &heading, theme::F_BODY, theme::TEXT);

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
        let mut ay = TOP + 40;
        for line in lines {
            p.text_left(content_x, ay, line, theme::F_SMALL, theme::DIM);
            ay += 20;
        }
    } else {
        let mut ry = TOP + 40;
        for (title, value) in &rows {
            p.fill_round_rect(content_x, ry, content_w, theme::TOUCH, 8, theme::with_alpha(theme::BLACK, 70));
            p.stroke_round_rect(content_x, ry, content_w, theme::TOUCH, 8, theme::LINE);
            let ty = vcenter(ry, theme::TOUCH, p.text_size(title, theme::F_SMALL).1);
            p.text_left(content_x + 12, ty, title, theme::F_SMALL, theme::TEXT);
            let vy = vcenter(ry, theme::TOUCH, p.text_size(value, theme::F_SMALL).1);
            p.text_right(content_x + content_w - 12, vy, value, theme::F_SMALL, theme::ACCENT);
            ry += theme::TOUCH + 7;
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
pub fn overlays(p: &mut Painter, state: &State, layout: &mut Layout) {
    if state.overlays.is_empty() && state.screen == giochi_launcher_core::Screen::Running {
        return;
    }

    // Escurece o fundo.
    p.fill_rect(0, 0, 1280, 720, theme::with_alpha(theme::BLACK, 200));

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
            p.fill_rect(0, 0, 1280, 720, theme::BLACK);
            p.icon("moon", 640, 320, 72, theme::with_alpha(theme::ACCENT, 130));
            p.text_center(640, 380, "Toque ou pressione qualquer botão para acordar.", theme::F_SMALL, theme::DIM);
        }
        giochi_launcher_core::Screen::Shutdown => {
            p.fill_rect(0, 0, 1280, 720, theme::BLACK);
            p.text_center(640, 350, "Sistema desligado.", theme::F_BODY, theme::FAINT);
            p.text_center(640, 374, "Pressione A para reiniciar.", theme::F_SMALL, theme::FAINT);
        }
        giochi_launcher_core::Screen::Boot => {
            p.fill_rect(0, 0, 1280, 720, theme::BLACK);
            p.text_center(640, 330, "GIOCHI", theme::F_LOGO, theme::ACCENT);
            p.text_center(640, 372, "launcher 0.1.0", theme::F_TINY, theme::FAINT);
        }
        giochi_launcher_core::Screen::Running => {}
    }
}

fn panel(p: &mut Painter, x: i32, y: i32, w: i32, h: i32) {
    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::PANEL);
    p.stroke_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::LINE2);
}

fn quick_settings(p: &mut Painter, state: &State, layout: &mut Layout) {
    let w = 520;
    let x = (1280 - w) / 2;
    let y = 140;
    let rows = 6;
    let h = 52 + rows * 46 + 60;
    panel(p, x, y, w, h);

    p.text_left(x + 16, y + 16, "Ajustes rápidos", theme::F_BODY, theme::TEXT);

    let focused = |index: usize| state.focus.zone == Zone::QuickSettings && state.focus.index == index;
    let mut ry = y + 48;

    for index in 0..rows {
        let row_h = 40;
        if focused(index) {
            p.stroke_round_rect(x + 8, ry, w - 16, row_h, 8, theme::ACCENT);
        }
        p.fill_round_rect(x + 8, ry, w - 16, row_h, 8, theme::with_alpha(theme::BLACK, 70));

        let (label, icon_name) = match index {
            0 => ("Volume", "vol"),
            1 => ("Brilho", "sun"),
            2 => ("Desempenho", "bolt"),
            3 => ("Wi-Fi", "wifi"),
            4 => ("Bluetooth", "bt"),
            _ => ("HUD de FPS", "chart"),
        };
        p.icon(icon_name, x + 26, ry + row_h / 2, 14, theme::ACCENT);
        let ty = vcenter(ry, row_h, p.text_size(label, theme::F_SMALL).1);
        p.text_left(x + 40, ty, label, theme::F_SMALL, theme::TEXT);

        match index {
            0 | 1 => {
                let value = if index == 0 { state.settings.volume } else { state.settings.brightness };
                let bar_x = x + 200;
                let bar_w = w - 300;
                let bar_y = ry + row_h / 2 - 3;
                p.fill_round_rect(bar_x, bar_y, bar_w, 6, 3, theme::with_alpha(theme::WHITE, 30));
                p.fill_round_rect(bar_x, bar_y, (bar_w * value as i32 / 100).max(2), 6, 3, theme::ACCENT);
                p.text_right(x + w - 20, ty, &format!("{value}%"), theme::F_TINY, theme::ACCENT);
            }
            2 => {
                p.text_right(x + w - 20, ty, state.settings.perf.label(), theme::F_TINY, theme::ACCENT);
            }
            _ => {
                let on = match index {
                    3 => state.settings.wifi,
                    4 => state.settings.bluetooth,
                    _ => state.settings.hud,
                };
                let knob_x = x + w - 52;
                p.fill_round_rect(knob_x, ry + 12, 34, 16, 8, if on { theme::with_alpha(theme::ACCENT, 80) } else { theme::with_alpha(theme::WHITE, 30) });
                let dot_x = if on { knob_x + 20 } else { knob_x + 2 };
                p.fill_circle(dot_x + 7, ry + 20, 7, if on { theme::ACCENT } else { theme::DIM });
            }
        }

        layout
            .hits
            .push((Rect::new(x + 8, ry, (w - 16) as u32, row_h as u32), Hit::QsRow(index)));
        ry += row_h + 6;
    }

    // Ações.
    let btn_w = (w - 24) / 2;
    let by = y + h - 52;
    for (i, label) in ["Suspender", "Reiniciar"].iter().enumerate() {
        let bx = x + 8 + i as i32 * (btn_w + 8);
        let danger = i == 1;
        p.fill_round_rect(
            bx,
            by,
            btn_w,
            theme::TOUCH,
            8,
            if danger { theme::with_alpha(theme::DANGER, 40) } else { theme::PANEL2 },
        );
        if focused(6 + i) {
            p.stroke_round_rect(bx, by, btn_w, theme::TOUCH, 8, theme::ACCENT);
        }
        let ty = vcenter(by, theme::TOUCH, p.text_size(label, theme::F_SMALL).1);
        p.text_center(bx + btn_w / 2, ty, label, theme::F_SMALL, if danger { theme::DANGER } else { theme::TEXT });
        layout.hits.push((
            Rect::new(bx, by, btn_w as u32, theme::TOUCH as u32),
            Hit::QsButton(6 + i as u8),
        ));
    }
}

fn game_detail(p: &mut Painter, state: &State, layout: &mut Layout) {
    let Some(game) = state.selected_game.and_then(|i| state.games.get(i)) else {
        return;
    };

    let w = 760;
    let x = (1280 - w) / 2;
    let y = 150;
    let hero_h = 210;
    let h = hero_h + 70;

    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::PANEL);

    let accent = theme::game_accent(game.id);
    let base = theme::game_base(game.id);
    p.vgradient_round_rect(x, y, w, hero_h, theme::RADIUS_PANEL, base, theme::mix(base, theme::BLACK, 0.4));

    let letter = game.title.chars().next().unwrap_or('?').to_string();
    p.text_right(x + w - 24, y + 4, &letter, 110, theme::with_alpha(theme::WHITE, 18));

    p.text_left(x + 24, y + 22, &game.category.to_uppercase(), theme::F_TINY, accent);
    p.text_left(x + 24, y + 42, &game.title, theme::F_HERO, theme::WHITE);

    // Estatísticas.
    let stats = [
        ("ANO", game.year_label()),
        ("HORAS", game.play_label()),
        ("ÚLTIMA", game.last_played.clone()),
        ("FAVORITO", if game.favorite { "Sim".into() } else { "—".into() }),
    ];
    let mut sx = x + 24;
    for (label, value) in stats {
        p.text_left(sx, y + 120, label, theme::F_TINY, theme::with_alpha(theme::WHITE, 140));
        p.text_left(sx, y + 136, &value, theme::F_BODY, theme::WHITE);
        sx += 150;
    }

    // Ações.
    let labels = [
        "Jogar".to_string(),
        if game.favorite { "Favoritado".into() } else { "Favoritar".into() },
        "Opções".to_string(),
        "Voltar".to_string(),
    ];
    let btn_w = 150;
    let gap = 8;
    let total = labels.len() as i32 * btn_w + (labels.len() as i32 - 1) * gap;
    let mut bx = x + w - 12 - total;
    let by = y + hero_h + 12;
    for (i, label) in labels.iter().enumerate() {
        let focused = state.focus.zone == Zone::GameDetailActions && state.focus.index == i;
        let primary = i == 0;
        p.fill_round_rect(
            bx,
            by,
            btn_w,
            theme::TOUCH,
            8,
            if primary { theme::ACCENT } else { theme::PANEL2 },
        );
        if focused {
            p.stroke_round_rect(bx, by, btn_w, theme::TOUCH, 8, theme::WHITE);
        }
        let color = if primary { theme::BG } else { theme::TEXT };
        let ty = vcenter(by, theme::TOUCH, p.text_size(label, theme::F_SMALL).1);
        p.text_center(bx + btn_w / 2, ty, label, theme::F_SMALL, color);
        layout.hits.push((
            Rect::new(bx, by, btn_w as u32, theme::TOUCH as u32),
            Hit::DetailButton(i),
        ));
        bx += btn_w + gap;
    }
}

fn power_popup(p: &mut Painter, state: &State, layout: &mut Layout) {
    let w = 340;
    let x = (1280 - w) / 2;
    let y = 190;
    let h = 340;
    panel(p, x, y, w, h);

    p.fill_round_rect(x + w / 2 - 24, y + 20, 48, 48, 12, theme::with_alpha(theme::DANGER, 40));
    p.icon("power", x + w / 2, y + 44, 22, theme::DANGER);

    p.text_center(x + w / 2, y + 80, "Opções de energia", theme::F_BODY, theme::TEXT);
    p.text_center(x + w / 2, y + 102, "O que deseja fazer?", theme::F_TINY, theme::DIM);

    let labels = ["Suspender", "Reiniciar", "Desligar", "Cancelar"];
    let mut by = y + 128;
    for (i, label) in labels.iter().enumerate() {
        let danger = i == 2;
        let focused = state.focus.zone == Zone::PowerActions && state.focus.index == i;
        p.fill_round_rect(
            x + 20,
            by,
            w - 40,
            theme::TOUCH,
            7,
            if danger { theme::with_alpha(theme::DANGER, 200) } else { theme::PANEL2 },
        );
        if focused {
            p.stroke_round_rect(x + 20, by, w - 40, theme::TOUCH, 7, theme::ACCENT);
        }
        let color = if danger { theme::WHITE } else if i == 3 { theme::DIM } else { theme::TEXT };
        let ty = vcenter(by, theme::TOUCH, p.text_size(label, theme::F_SMALL).1);
        p.text_center(x + w / 2, ty, label, theme::F_SMALL, color);
        layout.hits.push((
            Rect::new(x + 20, by, (w - 40) as u32, theme::TOUCH as u32),
            Hit::PowerButton(i),
        ));
        by += theme::TOUCH + 5;
    }
}

fn search(p: &mut Painter, state: &State, layout: &mut Layout) {
    let w = 900;
    let h = 520;
    let x = (1280 - w) / 2;
    let y = 110;
    panel(p, x, y, w, h);

    p.icon("search", x + 26, y + 30, 18, theme::ACCENT);
    let query = if state.search_query.is_empty() {
        "Buscar jogo...".to_string()
    } else {
        state.search_query.clone()
    };
    let qcolor = if state.search_query.is_empty() { theme::FAINT } else { theme::TEXT };
    p.text_left(x + 44, y + 22, &query, theme::F_BODY, qcolor);
    p.fill_rect(x, y + 48, w, 1, theme::LINE);

    // Teclado.
    let key_w = 52;
    let key_h = theme::TOUCH;
    let gap = 5;
    let kb_x = x + 16;
    let kb_y = y + 60;

    for row in 0..KEYBOARD.len() {
        for col in 0..KEYBOARD_COLS {
            let index = row * KEYBOARD_COLS + col;
            let kx = kb_x + col as i32 * (key_w + gap);
            let ky = kb_y + row as i32 * (key_h + gap);
            let focused = state.focus.zone == Zone::SearchKeyboard && state.focus.index == index;

            p.fill_round_rect(kx, ky, key_w, key_h, 7, theme::PANEL2);
            p.stroke_round_rect(kx, ky, key_w, key_h, 7, if focused { theme::ACCENT } else { theme::LINE });

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
    let results = state.search_results();
    let res_x = kb_x + KEYBOARD_COLS as i32 * (key_w + gap) + 14;
    let res_w = x + w - 16 - res_x;
    p.text_left(res_x, y + 60, &format!("Resultados ({})", results.len()), theme::F_TINY, theme::DIM);

    let mut ry = y + 80;
    let clip_top = ry;
    let clip_h = y + h - 16 - ry;
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
    if !state.toast.visible || state.toast.message.is_empty() {
        return;
    }
    let text = &state.toast.message;
    let tw = p.text_size(text, theme::F_SMALL).0;
    let w = tw + 60;
    let h = 38;
    let x = 640 - w / 2;
    let y = 720 - 64 - h;

    panel(p, x, y, w, h);
    p.icon("check", x + 22, y + h / 2, 14, theme::ACCENT);
    let ty = vcenter(y, h, p.text_size(text, theme::F_SMALL).1);
    p.text_left(x + 38, ty, text, theme::F_SMALL, theme::TEXT);
}

pub fn hud(p: &mut Painter, state: &State) {
    if !state.settings.hud {
        return;
    }
    let w = 170;
    let h = 40;
    let x = 1280 - 170 - 170;
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

/// Reexport para o `app` não depender de detalhes de layout.
pub use giochi_launcher_core::Screen;
