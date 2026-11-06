//! Reducer: a única função que muda o estado.
//!
//! Protótipo e port compartilham a mesma ideia (estado → comando → estado), mas
//! aqui ela é levada a sério: nenhuma função abaixo escreve em nada externo,
//! então o comportamento inteiro é testável sem tela.

use crate::cmd::{Cmd, Dir};
use crate::focus::{navigate, Focus, Zone};
use crate::state::{Filter, HUD, Overlay, Screen, Settings, State, Toast, KEYBOARD, KEYBOARD_COLS};
// `Game` e `Tab` só aparecem na construção de estado de teste (`demo` e `mod
// tests`); importá-los sempre geraria aviso no build de produção.
#[cfg(test)]
use crate::state::{Game, Tab};

/// Backspace no teclado virtual.
const BACKSPACE: char = '\u{8}';

pub fn reduce(state: State, cmd: Cmd) -> State {
    // Fora de `Running`, entrada do usuário acorda — e é consumida pelo
    // despertar. Sem isso, o botão que acorda também age: apertar A na tela de
    // sono abriria o detalhe do jogo por baixo, e o usuário acordaria já com um
    // overlay aberto que não pediu.
    // `Boot` fica de fora de propósito: `BootDone` chega justamente nesse
    // estado, e interceptar aqui impediria o launcher de sair do boot.
    if matches!(
        state.screen,
        Screen::Sleeping | Screen::Shutdown | Screen::Reboot
    ) {
        return from_locked_screen(state, cmd);
    }

    match cmd {
        Cmd::Nav(dir) => nav(state, dir),
        Cmd::Confirm => confirm(state),
        Cmd::Back => go_back(state),
        Cmd::Options => open_detail(state),
        Cmd::Search => open_search(state),
        Cmd::TabPrev => tab_step(state, -1),
        Cmd::TabNext => tab_step(state, 1),
        Cmd::ToggleQuickSettings => toggle_quick_settings(state),
        Cmd::PowerShort => power_short(state),
        Cmd::PowerHold => power_hold(state),
        Cmd::Wake => State {
            screen: Screen::Running,
            ..state
        },
        Cmd::Reboot => reboot(state),
        Cmd::BootDone => {
            if state.screen == Screen::Boot {
                State {
                    screen: Screen::Running,
                    ..state
                }
            } else {
                state
            }
        }
        Cmd::HideToast => State {
            toast: Toast::default(),
            ..state
        },
        Cmd::ToggleWifi => State {
            settings: Settings {
                wifi: !state.settings.wifi,
                ..state.settings
            },
            ..state
        },
        Cmd::ToggleBluetooth => State {
            settings: Settings {
                bluetooth: !state.settings.bluetooth,
                ..state.settings
            },
            ..state
        },
        Cmd::ToggleHud => State {
            settings: Settings {
                hud: !state.settings.hud,
                ..state.settings
            },
            ..state
        },
        Cmd::SetFilter(filter) => set_filter(state, filter),
        Cmd::SetPerf(perf) => State {
            settings: Settings {
                perf,
                ..state.settings
            },
            ..state
        },
        Cmd::AdjustVolume(delta) => {
            let value = state.settings.volume as i16 + delta as i16 * 5;
            set_volume(state, value)
        }
        Cmd::AdjustBrightness(delta) => {
            let value = state.settings.brightness as i16 + delta as i16 * 5;
            set_brightness(state, value)
        }
        Cmd::SetVolume(value) => set_volume(state, value as i16),
        Cmd::SetBrightness(value) => set_brightness(state, value as i16),
        Cmd::SearchPush(c) => push_char(state, c),
        Cmd::SearchBackspace => pop_char(state),
        // As leituras reais vêm do sistema; ver `TickHud` em `docs/arquitetura.md`.
        Cmd::TickHud => state,
        Cmd::SetBattery(level) => State {
            battery: level,
            ..state
        },
        Cmd::SetTemperature(celsius) => State {
            hud: HUD {
                temp_c: celsius.unwrap_or(0),
                ..state.hud
            },
            ..state
        },
        Cmd::SetPad(connected) => State {
            pad_connected: connected,
            ..state
        },

        // ── ponteiro/toque ───────────────────────────────────
        Cmd::FocusAt(zone, index) => {
            if state.zone_len(zone) == 0 {
                state
            } else {
                focus_on(state, zone, index)
            }
        }
        Cmd::GoTab(tab) => {
            if state.has_overlay() || state.screen != Screen::Running {
                return state;
            }
            let zone = tab.zone();
            let index = state.saved_focus.get(&zone).copied().unwrap_or(0);
            focus_on(State { tab, ..state }, zone, index)
        }
        Cmd::OpenGame(index) => {
            if index < state.games.len() {
                let state = State {
                    selected_game: Some(index),
                    ..state
                };
                push(state, Overlay::GameDetail)
            } else {
                state
            }
        }
        Cmd::OpenApp(index) => {
            let title = state.apps.get(index).map(|app| app.title.clone());
            match title {
                Some(title) => toast(state, &format!("Abrindo {title}...")),
                None => state,
            }
        }
        Cmd::GoSettingsCat(index) => {
            if index < crate::state::SETTINGS_CATEGORIES.len() {
                State {
                    settings_category: index,
                    ..state
                }
            } else {
                state
            }
        }
    }
}

// ── navegação ────────────────────────────────────────────────

/// Tela bloqueada (sono ou desligado) → o que a entrada pode fazer.
///
/// A regra é uma só: **qualquer** entrada de pessoa tira a tela do estado
/// travado, e nada além disso acontece naquele toque.
fn from_locked_screen(state: State, cmd: Cmd) -> State {
    match state.screen {
        Screen::Sleeping => {
            // A tela de sono promete "pressione qualquer botão para acordar";
            // é isto que cumpre a promessa. `Wake` também passa por aqui.
            if cmd.is_user_input() || cmd == Cmd::Wake {
                State {
                    screen: Screen::Running,
                    ..state
                }
            } else {
                state
            }
        }
        // Qualquer botão reinicia, como diz o texto da tela de desligado.
        Screen::Shutdown => {
            if cmd.is_user_input() || cmd == Cmd::Reboot {
                to_reboot(state)
            } else {
                state
            }
        }
        // Em `Reboot` o pedido já foi feito: se o sistema reiniciasse de fato,
        // este processo não existiria mais. Chegar aqui com um botão significa
        // que o `reboot` falhou, então a única saída honesta é voltar para a
        // interface — insistir prenderia o usuário em "Reiniciando..." para
        // sempre.
        Screen::Reboot => {
            if cmd.is_user_input() || cmd == Cmd::Reboot {
                State {
                    screen: Screen::Running,
                    overlays: Vec::new(),
                    toast: Toast::default(),
                    ..state
                }
            } else {
                state
            }
        }
        // Inalcançável: só se entra aqui nas telas travadas.
        Screen::Boot | Screen::Running => state,
    }
}

fn nav(state: State, dir: Dir) -> State {
    // Ajustes rápidos: horizontal ajusta, não move.
    if state.focus.zone == Zone::QuickSettings && matches!(dir, Dir::Left | Dir::Right) {
        let delta: i16 = if dir == Dir::Right { 5 } else { -5 };
        return match state.focus.index {
            0 => {
                let value = state.settings.volume as i16 + delta;
                set_volume(state, value)
            }
            1 => {
                let value = state.settings.brightness as i16 + delta;
                set_brightness(state, value)
            }
            2 => {
                let step: isize = if dir == Dir::Right { 1 } else { -1 };
                State {
                    settings: Settings {
                        perf: state.settings.perf.step(step),
                        ..state.settings
                    },
                    ..state
                }
            }
            _ => state,
        };
    }

    // Filtros: horizontal troca o filtro ativo.
    if state.focus.zone == Zone::LibraryFilters && matches!(dir, Dir::Left | Dir::Right) {
        let step: isize = if dir == Dir::Right { 1 } else { -1 };
        let current = Filter::ALL
            .iter()
            .position(|f| *f == state.library_filter)
            .unwrap_or(0);
        let next = (current as isize + step).rem_euclid(Filter::ALL.len() as isize) as usize;
        return set_filter(state, Filter::ALL[next]);
    }

    match navigate(&state, dir) {
        Some(focus) => {
            let carousel = if focus.zone == Zone::HomeCarousel {
                focus.index
            } else {
                state.carousel
            };
            State {
                focus,
                carousel,
                ..state
            }
        }
        None => state,
    }
}

fn set_filter(state: State, filter: Filter) -> State {
    let index = Filter::ALL
        .iter()
        .position(|f| *f == filter)
        .unwrap_or(0);
    State {
        library_filter: filter,
        focus: Focus::at(Zone::LibraryFilters, index),
        ..state
    }
}

// ── confirmação ──────────────────────────────────────────────

fn confirm(state: State) -> State {
    let zone = state.focus.zone;
    let index = state.focus.index;

    match zone {
        Zone::HomeCarousel => {
            // Clona o título antes de mover o estado: evita manter empréstimo
            // de `state` enquanto `state` é consumido.
            let title = state
                .focused_game()
                .map(|game| game.title.clone());
            match title {
                Some(title) => toast(state, &format!("Iniciando {title}...")),
                None => state,
            }
        }

        Zone::LibraryGrid => {
            let filtered = state.filtered_games();
            match filtered.get(index).copied() {
                Some(game_index) => {
                    let state = State {
                        selected_game: Some(game_index),
                        ..state
                    };
                    push(state, Overlay::GameDetail)
                }
                None => state,
            }
        }

        Zone::AppsGrid => {
            let title = state.apps.get(index).map(|app| app.title.clone());
            match title {
                Some(title) => toast(state, &format!("Abrindo {title}...")),
                None => state,
            }
        }

        Zone::SettingsSidebar => State {
            settings_category: index,
            ..state
        },

        Zone::LibraryFilters => state,

        Zone::SearchKeyboard => {
            let row = index / KEYBOARD_COLS;
            let col = index % KEYBOARD_COLS;
            match KEYBOARD.get(row).and_then(|r| r.get(col)).copied() {
                Some(BACKSPACE) => pop_char(state),
                Some(c) => push_char(state, c),
                None => state,
            }
        }

        Zone::SearchResults => {
            let results = state.search_results();
            match results.get(index).copied() {
                Some(game_index) => {
                    let state = State {
                        selected_game: Some(game_index),
                        ..state
                    };
                    push(state, Overlay::GameDetail)
                }
                None => state,
            }
        }

        Zone::QuickSettings => match index {
            3 => reduce(state, Cmd::ToggleWifi),
            4 => reduce(state, Cmd::ToggleBluetooth),
            5 => reduce(state, Cmd::ToggleHud),
            6 => to_sleeping(state),
            7 => to_boot(state),
            _ => state,
        },

        Zone::GameDetailActions => match index {
            0 => {
                // O "Jogar" mostrava `Iniciando X...` e não abria nada: era só
                // um aviso, e o usuário apertava de novo. O pedido de verdade
                // viaja em `state.lancar`; quem executa é o `ui-sdl`, que
                // precisa liberar o DRM antes de ceder a tela.
                let alvo = state
                    .selected_game
                    .and_then(|i| state.games.get(i))
                    .map(|game| (game.id, game.title.clone()));
                match alvo {
                    Some((id, title)) => {
                        let s = toast(go_back(state), &format!("Iniciando {title}..."));
                        State {
                            lancar: Some(id),
                            ..s
                        }
                    }
                    None => state,
                }
            }
            1 => toggle_favorite(state),
            2 => toast(state, "Opções do jogo"),
            3 => go_back(state),
            _ => state,
        },

        Zone::PowerActions => match index {
            0 => to_sleeping(state),
            1 => to_reboot(state),
            2 => to_shutdown(state),
            3 => go_back(state),
            _ => state,
        },
    }
}

fn toggle_favorite(state: State) -> State {
    let Some(selected) = state.selected_game else {
        return state;
    };

    let mut games = state.games.clone();
    if let Some(game) = games.get_mut(selected) {
        game.favorite = !game.favorite;
    }

    let favorite = games.get(selected).map(|g| g.favorite).unwrap_or(false);
    let message = if favorite {
        "Adicionado aos favoritos"
    } else {
        "Removido dos favoritos"
    };

    toast(State { games, ..state }, message)
}

// ── pilha de overlays ────────────────────────────────────────

/// Empilha um overlay e move o foco para ele, guardando onde estava.
fn push(state: State, overlay: Overlay) -> State {
    if state.overlays.contains(&overlay) {
        return state;
    }

    let mut saved_focus = state.saved_focus.clone();
    saved_focus.insert(state.focus.zone, state.focus.index);

    let mut overlays = state.overlays.clone();
    overlays.push(overlay);

    State {
        overlays,
        saved_focus,
        focus: Focus::at(overlay.zone(), 0),
        ..state
    }
}

/// Desempilha e devolve o foco para onde estava.
fn go_back(state: State) -> State {
    if state.overlays.is_empty() {
        return state;
    }

    let mut overlays = state.overlays.clone();
    overlays.pop();

    let zone = match overlays.last() {
        Some(overlay) => overlay.zone(),
        None => state.tab.zone(),
    };
    let index = state.saved_focus.get(&zone).copied().unwrap_or(0);

    focus_on(State { overlays, ..state }, zone, index)
}

fn open_detail(state: State) -> State {
    match state.focus.zone {
        Zone::HomeCarousel => match state.carousel {
            index if index < state.games.len() => {
                let state = State {
                    selected_game: Some(index),
                    ..state
                };
                push(state, Overlay::GameDetail)
            }
            _ => state,
        },
        Zone::LibraryGrid => {
            let filtered = state.filtered_games();
            match filtered.get(state.focus.index).copied() {
                Some(game_index) => {
                    let state = State {
                        selected_game: Some(game_index),
                        ..state
                    };
                    push(state, Overlay::GameDetail)
                }
                None => state,
            }
        }
        _ => state,
    }
}

fn open_search(state: State) -> State {
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }
    let state = State {
        search_query: String::new(),
        ..state
    };
    push(state, Overlay::Search)
}

fn toggle_quick_settings(state: State) -> State {
    if state.top_overlay() == Some(Overlay::QuickSettings) {
        return go_back(state);
    }
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }
    push(state, Overlay::QuickSettings)
}

// ── ciclo de vida ────────────────────────────────────────────

/// Move o foco para `zone`/`index`, mantendo o carrossel coerente com o foco.
///
/// `carousel` é campo derivado: `views` desenha o item central por
/// `state.carousel`, não por `focus.index`. Toda troca de zona passa por aqui
/// para que o item destacado e o item desenhado não possam divergir.
fn focus_on(state: State, zone: Zone, index: usize) -> State {
    let len = state.zone_len(zone);
    let index = if len == 0 { 0 } else { index.min(len - 1) };
    let carousel = if zone == Zone::HomeCarousel {
        index
    } else {
        state.carousel
    };
    State {
        focus: Focus::at(zone, index),
        carousel,
        ..state
    }
}

fn tab_step(state: State, delta: isize) -> State {
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }

    let mut saved_focus = state.saved_focus.clone();
    saved_focus.insert(state.focus.zone, state.focus.index);

    let tab = state.tab.step(delta);
    let zone = tab.zone();
    let index = saved_focus.get(&zone).copied().unwrap_or(0);

    focus_on(
        State {
            tab,
            saved_focus,
            ..state
        },
        zone,
        index,
    )
}

fn power_short(state: State) -> State {
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }
    to_sleeping(state)
}

fn power_hold(state: State) -> State {
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }
    push(state, Overlay::PowerPopup)
}

fn to_sleeping(state: State) -> State {
    State {
        screen: Screen::Sleeping,
        overlays: Vec::new(),
        toast: Toast::default(),
        ..state
    }
}

/// Desligar limpa toast e overlays.
///
/// O protótipo disparava `showToastEffect` e `shutdown` no mesmo comando: a tela
/// mostrava "Reiniciando..." e "Sistema desligado" ao mesmo tempo.
fn to_shutdown(state: State) -> State {
    State {
        screen: Screen::Shutdown,
        overlays: Vec::new(),
        toast: Toast::default(),
        ..state
    }
}

/// Volta ao boot, preservando catálogo e apps.
fn to_boot(state: State) -> State {
    let mut next = State::new(state.games.clone(), state.apps.clone());
    next.screen = Screen::Boot;
    next
}

/// Pede reinício do sistema.
fn to_reboot(state: State) -> State {
    State {
        screen: Screen::Reboot,
        overlays: Vec::new(),
        toast: Toast::default(),
        ..state
    }
}

fn reboot(state: State) -> State {
    to_boot(state)
}

// ── valores e busca ──────────────────────────────────────────

fn set_volume(state: State, value: i16) -> State {
    let volume = value.clamp(0, 100) as u8;
    State {
        settings: Settings {
            volume,
            ..state.settings
        },
        ..state
    }
}

fn set_brightness(state: State, value: i16) -> State {
    let brightness = value.clamp(0, 100) as u8;
    State {
        settings: Settings {
            brightness,
            ..state.settings
        },
        ..state
    }
}

fn push_char(state: State, c: char) -> State {
    let mut query = state.search_query.clone();
    query.push(c);
    State {
        search_query: query,
        ..state
    }
}

fn pop_char(state: State) -> State {
    let mut query = state.search_query.clone();
    query.pop();
    State {
        search_query: query,
        ..state
    }
}

fn toast(state: State, message: &str) -> State {
    State {
        toast: Toast::show(message),
        ..state
    }
}

/// Constrói um estado pronto para teste.
#[cfg(test)]
fn demo() -> State {
    let games: Vec<Game> = (0..8)
        .map(|i| {
            Game::new(
                i as u32,
                &format!("Jogo {i}"),
                "Cat",
                1.0,
                "hoje",
                i % 2 == 0,
            )
        })
        .collect();
    let mut state = State::new(games, Vec::new());
    state.screen = Screen::Running;
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmar_na_biblioteca_abre_detalhe_e_guarda_foco() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 3);

        let s = reduce(s, Cmd::Confirm);
        assert_eq!(s.overlays, vec![Overlay::GameDetail]);
        assert_eq!(s.focus.zone, Zone::GameDetailActions);
        assert_eq!(s.saved_focus.get(&Zone::LibraryGrid), Some(&3));
        assert_eq!(s.selected_game, Some(3));
    }

    #[test]
    fn voltar_restaura_o_foco_anterior() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 3);

        let s = reduce(s, Cmd::Confirm);
        let s = reduce(s, Cmd::Back);

        assert!(s.overlays.is_empty());
        assert_eq!(s.focus, Focus::at(Zone::LibraryGrid, 3));
    }

    #[test]
    fn desligar_limpa_toast_e_overlays() {
        let mut s = demo();
        s = push(s, Overlay::PowerPopup);
        s.toast = Toast::show("algo");
        s.focus = Focus::at(Zone::PowerActions, 2);

        let s = reduce(s, Cmd::Confirm);

        assert_eq!(s.screen, Screen::Shutdown);
        assert!(s.overlays.is_empty(), "não pode sobrar overlay");
        assert!(!s.toast.visible, "não pode sobrar toast contraditório");
    }

    #[test]
    fn troca_de_aba_e_bloqueada_com_overlay_aberto() {
        let mut s = demo();
        s = push(s, Overlay::QuickSettings);

        let s = reduce(s, Cmd::TabNext);
        assert_eq!(s.tab, Tab::Home, "aba não muda com overlay aberto");
    }

    #[test]
    fn troca_de_aba_preserva_foco_por_aba() {
        let mut s = demo();
        s.focus = Focus::at(Zone::HomeCarousel, 5);

        let s = reduce(s, Cmd::TabNext);
        assert_eq!(s.tab, Tab::Library);

        let s = reduce(s, Cmd::TabPrev);
        assert_eq!(s.tab, Tab::Home);
        assert_eq!(s.carousel, 5, "carrossel volta na posição anterior");
    }

    #[test]
    fn tocar_num_cartao_lateral_traz_ele_para_o_centro() {
        let s = demo();
        assert_eq!(s.carousel, 0);

        // `FocusAt` vem do toque: o cartão tocado precisa virar o central,
        // senão o destaque e o desenho divergem.
        let s = reduce(s, Cmd::FocusAt(Zone::HomeCarousel, 4));

        assert_eq!(s.focus, Focus::at(Zone::HomeCarousel, 4));
        assert_eq!(s.carousel, 4, "o cartão tocado passa a ser o desenhado");
    }

    #[test]
    fn troca_de_aba_devolve_o_cartao_tocado() {
        let s = demo();

        let s = reduce(s, Cmd::FocusAt(Zone::HomeCarousel, 6));
        let s = reduce(s, Cmd::TabNext);
        let s = reduce(s, Cmd::TabPrev);

        assert_eq!(s.tab, Tab::Home);
        assert_eq!(s.carousel, 6, "voltar para a Home não perde a seleção");
    }

    #[test]
    fn foco_fora_da_home_nao_mexe_no_carrossel() {
        let s = demo();
        let s = reduce(s, Cmd::FocusAt(Zone::HomeCarousel, 3));

        let s = reduce(s, Cmd::TabNext);
        let s = reduce(s, Cmd::FocusAt(Zone::LibraryGrid, 1));

        assert_eq!(s.carousel, 3, "o carrossel guarda a posição da Home");
    }

    #[test]
    fn cartao_alem_do_catalogo_e_limitado() {
        let s = demo();
        let s = reduce(s, Cmd::FocusAt(Zone::HomeCarousel, 99));

        assert_eq!(s.carousel, 7, "índice fora da lista cai no último jogo");
        assert_eq!(s.focus, Focus::at(Zone::HomeCarousel, 7));
    }

    #[test]
    fn qualquer_botao_acorda_a_tela() {
        // Bug relatado no aparelho: dormia e nenhum botão voltava. A tela de
        // sono promete "pressione qualquer botão para acordar".
        for cmd in [
            Cmd::Nav(Dir::Right),
            Cmd::Confirm,
            Cmd::Back,
            Cmd::Options,
            Cmd::TabNext,
            Cmd::PowerHold,
            Cmd::ToggleQuickSettings,
            Cmd::FocusAt(Zone::HomeCarousel, 1),
            Cmd::GoTab(Tab::Apps),
        ] {
            let mut s = demo();
            s.screen = Screen::Sleeping;
            s = reduce(s, cmd);
            assert_eq!(s.screen, Screen::Running, "{cmd:?} não acordou a tela");
        }
    }

    #[test]
    fn o_botao_que_acorda_nao_age() {
        let mut s = demo();
        s.screen = Screen::Sleeping;
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 2);

        // A sobre um jogo da biblioteca: acorda, mas NÃO abre o detalhe.
        let s = reduce(s, Cmd::Confirm);

        assert_eq!(s.screen, Screen::Running);
        assert!(
            s.overlays.is_empty(),
            "o toque que acorda não pode abrir overlay"
        );
        assert_eq!(s.focus, Focus::at(Zone::LibraryGrid, 2), "foco intacto");

        // Setas também não movem o foco durante o despertar.
        let mut s = demo();
        s.screen = Screen::Sleeping;
        s.focus = Focus::at(Zone::HomeCarousel, 3);
        let s = reduce(s, Cmd::Nav(Dir::Right));
        assert_eq!(s.screen, Screen::Running);
        assert_eq!(s.focus, Focus::at(Zone::HomeCarousel, 3));
    }

    #[test]
    fn leitura_de_sistema_nao_acorda_a_tela() {
        // Bateria chegando enquanto dorme não pode acender o aparelho.
        for cmd in [
            Cmd::TickHud,
            Cmd::SetBattery(Some(80)),
            Cmd::SetPad(true),
            Cmd::HideToast,
        ] {
            let mut s = demo();
            s.screen = Screen::Sleeping;
            let s = reduce(s, cmd);
            assert_eq!(
                s.screen,
                Screen::Sleeping,
                "{cmd:?} não é entrada de usuário e não pode acordar"
            );
        }
    }

    #[test]
    fn desligado_reinicia_com_qualquer_botao() {
        let mut s = demo();
        s.screen = Screen::Shutdown;

        let s = reduce(s, Cmd::Confirm);

        // `Reboot`, não `Boot`: `Boot` é o estado inicial do launcher, e o
        // sistema precisa saber a diferença para chamar `reboot` de verdade.
        assert_eq!(s.screen, Screen::Reboot, "botão na tela de desligado reinicia");
    }

    #[test]
    fn reiniciar_nao_e_o_mesmo_que_boot_inicial() {
        // A distinção existe para o app poder chamar `reboot`. Se voltasse para
        // `Boot`, "reiniciar" e "primeiro boot" seriam indistinguíveis.
        let mut s = demo();
        s.screen = Screen::Running;
        s.focus = Focus::at(Zone::PowerActions, 1);

        let s = reduce(s, Cmd::Confirm);

        assert_eq!(s.screen, Screen::Reboot);
        assert_ne!(s.screen, Screen::Boot);
    }

    #[test]
    fn reiniciar_limpa_overlay_e_toast() {
        let mut s = demo();
        s = push(s, Overlay::PowerPopup);
        s.toast = Toast::show("algo");
        s.focus = Focus::at(Zone::PowerActions, 1);

        let s = reduce(s, Cmd::Confirm);

        assert_eq!(s.screen, Screen::Reboot);
        assert!(s.overlays.is_empty(), "não pode sobrar overlay");
        assert!(!s.toast.visible, "não pode sobrar toast");
    }

    #[test]
    fn botao_em_reboot_devolve_a_interface_em_vez_de_prender() {
        // Se `reboot` falhou, o usuário não pode ficar preso em
        // "Reiniciando...": apertar qualquer botão volta ao app.
        let mut s = demo();
        s.screen = Screen::Reboot;

        let s = reduce(s, Cmd::Confirm);

        assert_eq!(
            s.screen,
            Screen::Running,
            "falha de reboot precisa ter saída"
        );
    }

    #[test]
    fn leitura_de_sistema_nao_reinicia() {
        // Mesma regra do sono: bateria chegando não pode reiniciar o aparelho.
        let mut s = demo();
        s.screen = Screen::Reboot;

        let s = reduce(s, Cmd::SetBattery(Some(50)));

        assert_eq!(s.screen, Screen::Reboot, "só entrada de pessoa age");
    }

    #[test]
    fn boot_ainda_conclui() {
        // O guarda de tela travada não pode interceptar o fim do boot.
        let mut s = demo();
        s.screen = Screen::Boot;

        let s = reduce(s, Cmd::BootDone);

        assert_eq!(s.screen, Screen::Running);
    }

    #[test]
    fn volume_respeita_limites() {
        let mut s = demo();
        s.settings.volume = 98;
        let s = reduce(s, Cmd::AdjustVolume(1));
        assert_eq!(s.settings.volume, 100);

        let s = reduce(s, Cmd::AdjustVolume(1));
        assert_eq!(s.settings.volume, 100, "não passa de 100");

        let mut s = demo();
        s.settings.volume = 3;
        let s = reduce(s, Cmd::AdjustVolume(-1));
        assert_eq!(s.settings.volume, 0);
    }

    #[test]
    fn digitar_e_apagar_na_busca() {
        let mut s = demo();
        s = open_search(s);

        for c in "abc".chars() {
            s = reduce(s, Cmd::SearchPush(c));
        }
        assert_eq!(s.search_query, "abc");

        s = reduce(s, Cmd::SearchBackspace);
        assert_eq!(s.search_query, "ab");

        // Apagar além do início não estoura.
        s = reduce(s, Cmd::SearchBackspace);
        s = reduce(s, Cmd::SearchBackspace);
        s = reduce(s, Cmd::SearchBackspace);
        assert_eq!(s.search_query, "");
    }

    #[test]
    fn favoritar_e_desfavoritar_pelo_detalhe() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 1);
        s = reduce(s, Cmd::Confirm);
        assert_eq!(s.selected_game, Some(1));

        let before = s.games[1].favorite;
        s.focus = Focus::at(Zone::GameDetailActions, 1);
        s = reduce(s, Cmd::Confirm);
        assert_eq!(s.games[1].favorite, !before);
        assert!(s.toast.visible);
    }

    /// "Jogar" precisa **pedir o lançamento**, não só avisar.
    ///
    /// O botão exibia `Iniciando X...` e não abria nada: o reducer trocava o
    /// toast e voltava para a Home, e o usuário apertava de novo. O aviso
    /// sozinho é pior que nada, porque parece que funcionou.
    #[test]
    fn jogar_pede_o_lancamento_do_app() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 2);
        s = reduce(s, Cmd::Confirm);
        let id = s.games[2].id;

        s.focus = Focus::at(Zone::GameDetailActions, 0);
        s = reduce(s, Cmd::Confirm);

        assert_eq!(s.lancar, Some(id), "o id do jogo precisa ir no pedido");
        // E volta para a Home: ao retornar do jogo o usuário cai na interface,
        // não numa ficha de jogo que ele já iniciou.
        assert!(s.overlays.is_empty());
    }

    /// Voltar e favoritar **não** pedem lançamento.
    ///
    /// Sem isto, um `lancar` grudado no estado faria o launcher tentar abrir o
    /// jogo a cada ação na ficha.
    #[test]
    fn outras_acoes_da_ficha_nao_pedem_lancamento() {
        for indice in [1usize, 2, 3] {
            let mut s = demo();
            s.tab = Tab::Library;
            s.focus = Focus::at(Zone::LibraryGrid, 0);
            s = reduce(s, Cmd::Confirm);
            s.focus = Focus::at(Zone::GameDetailActions, indice);
            let s = reduce(s, Cmd::Confirm);
            assert_eq!(s.lancar, None, "ação {indice} não pode pedir lançamento");
        }
    }

    #[test]
    fn dormir_limpa_overlays() {
        let mut s = demo();
        s = push(s, Overlay::QuickSettings);
        s.focus = Focus::at(Zone::QuickSettings, 6);

        let s = reduce(s, Cmd::Confirm);
        assert_eq!(s.screen, Screen::Sleeping);
        assert!(s.overlays.is_empty());
    }

    #[test]
    fn acordar_volta_a_rodar() {
        let mut s = demo();
        s.screen = Screen::Sleeping;
        let s = reduce(s, Cmd::Wake);
        assert_eq!(s.screen, Screen::Running);
    }

    #[test]
    fn botao_de_energia_nao_age_com_overlay_aberto() {
        let mut s = demo();
        s = push(s, Overlay::GameDetail);

        let s = reduce(s, Cmd::PowerShort);
        assert_eq!(s.screen, Screen::Running, "não suspende com overlay aberto");
    }

    #[test]
    fn filtro_de_favoritos_reduz_a_lista() {
        let mut s = demo();
        let s2 = set_filter(s.clone(), Filter::Favorites);
        assert_eq!(s2.filtered_games().len(), 4, "metade dos 8 é favorita");

        s.library_filter = Filter::All;
        assert_eq!(s.filtered_games().len(), 8);
    }

    #[test]
    fn percorre_as_quatro_abas_em_laco() {
        let mut s = demo();
        for expected in [Tab::Library, Tab::Apps, Tab::Settings, Tab::Home] {
            s = reduce(s, Cmd::TabNext);
            assert_eq!(s.tab, expected);
        }
    }

    #[test]
    fn hud_tick_nao_altera_estado() {
        let s = demo();
        let after = reduce(s.clone(), Cmd::TickHud);
        assert_eq!(s, after, "TickHud é no-op até haver leitura real");
    }

    #[test]
    fn confirmar_sem_jogos_nao_estoura() {
        let mut s = State::new(Vec::new(), Vec::new());
        s.screen = Screen::Running;
        let after = reduce(s, Cmd::Confirm);
        assert!(after.overlays.is_empty());
        assert_eq!(after.selected_game, None);
    }
}
