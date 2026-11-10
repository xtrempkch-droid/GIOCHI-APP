//! Reducer: a única função que muda o estado.
//!
//! Protótipo e port compartilham a mesma ideia (estado → comando → estado), mas
//! aqui ela é levada a sério: nenhuma função abaixo escreve em nada externo,
//! então o comportamento inteiro é testável sem tela.

use crate::cmd::{Cmd, Dir};
use crate::focus::{navigate, Focus, Zone};
use crate::state::{Filter, HUD, Overlay, Screen, Settings, State, Toast, KEYBOARD, KEYBOARD_COLS};
// `Game` só aparece na construção de estado de teste (`demo` e `mod tests`);
// importá-lo sempre geraria aviso no build de produção. `Tab` passou a ser
// usado pelo reducer em si quando os gatilhos ganharam `sub_tab`.
#[cfg(test)]
use crate::state::Game;
use crate::state::Tab;

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
        Cmd::SubPrev => sub_tab(state, -1),
        Cmd::SubNext => sub_tab(state, 1),
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
        // ── Wi-Fi / Bluetooth ────────────────────────────────
        Cmd::RedesWifi(redes, erro) => State {
            sem_fio: crate::state::SemFio {
                redes,
                escaneando: false,
                erro,
                ..state.sem_fio
            },
            ..state
        },
        Cmd::EscaneandoWifi(ligado) => State {
            sem_fio: crate::state::SemFio {
                escaneando: ligado,
                ..state.sem_fio
            },
            ..state
        },
        Cmd::ResultadoWifi(erro) => {
            let s = State {
                sem_fio: crate::state::SemFio {
                    erro: erro.clone(),
                    ..state.sem_fio
                },
                ..state
            };
            match erro {
                // Deu certo: sai da senha e volta para a lista.
                None => {
                    let s = State {
                        sem_fio: crate::state::SemFio {
                            conectado: s.sem_fio.alvo.clone(),
                            ..s.sem_fio.clone()
                        },
                        ..s
                    };
                    let mut s = go_back(s);
                    s.sem_fio.limpar_senha();
                    toast(s, "Conectado")
                }
                Some(motivo) => toast(s, &motivo),
            }
        }
        Cmd::CancelarSenha => {
            let mut s = go_back(state);
            s.sem_fio.limpar_senha();
            s
        }
        Cmd::MostrarSenha => State {
            sem_fio: crate::state::SemFio {
                senha_visivel: !state.sem_fio.senha_visivel,
                ..state.sem_fio
            },
            ..state
        },
        Cmd::DispositivosBt(lista, erro) => State {
            sem_fio: crate::state::SemFio {
                dispositivos: lista,
                bt_escaneando: false,
                erro,
                ..state.sem_fio
            },
            ..state
        },
        Cmd::EscaneandoBt(ligado) => State {
            sem_fio: crate::state::SemFio {
                bt_escaneando: ligado,
                ..state.sem_fio
            },
            ..state
        },
        Cmd::ResultadoBt(erro) => match erro {
            None => {
                let alvo = state.sem_fio.bt_alvo.clone();
                let s = State {
                    sem_fio: crate::state::SemFio {
                        bt_conectado: alvo,
                        ..state.sem_fio
                    },
                    ..state
                };
                toast(s, "Pareado")
            }
            Some(motivo) => toast(state, &motivo),
        },
        // Estes três não mudam estado por si: a UI lê o campo e executa.
        // Mantidos no `match` para o compilador exigir que alguém pense neles.
        Cmd::EscolherRede(_) | Cmd::ConectarSenha | Cmd::ParearBt(_) => state,
        // Abrir já marca "procurando": a UI executa a varredura quando vê este
        // campo passar de falso para verdadeiro. Se a marcação ficasse na UI,
        // haveria duas rotas para o mesmo efeito — e a tela abriria vazia por um
        // instante parecendo "nenhuma rede".
        // Capacidade de rádio do aparelho. Guardada como fato, não como erro:
        // a tela precisa dizer "não tem" antes de o usuário tentar.
        Cmd::CapacidadesSemFio { wifi, bluetooth } => State {
            sem_fio: crate::state::SemFio {
                tem_wifi: Some(wifi),
                tem_bt: Some(bluetooth),
                ..state.sem_fio
            },
            ..state
        },
        // Sem rádio, não marca "procurando": seria a tela prometer uma varredura
        // que nunca vai acontecer. O overlay mostra o motivo.
        Cmd::AbrirWifi => {
            let tem = state.sem_fio.tem_wifi.unwrap_or(true);
            push(
                State {
                    sem_fio: crate::state::SemFio {
                        escaneando: tem,
                        erro: None,
                        ..state.sem_fio
                    },
                    ..state
                },
                Overlay::WifiList,
            )
        }
        Cmd::AbrirBluetooth => {
            let tem = state.sem_fio.tem_bt.unwrap_or(true);
            push(
                State {
                    sem_fio: crate::state::SemFio {
                        bt_escaneando: tem,
                        erro: None,
                        ..state.sem_fio
                    },
                    ..state
                },
                Overlay::BtList,
            )
        }
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
                // O toque move o foco junto: antes só `settings_category`
                // mudava e o destaque continuava na categoria anterior.
                focus_on(
                    State {
                        settings_category: index,
                        ..state
                    },
                    Zone::SettingsSidebar,
                    index,
                )
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

    // Configurações: ← → **só** ajustam quando a linha tem o que ajustar.
    //
    // ## O bug que isto conserta
    //
    // A versão anterior tratava toda linha do painel como ajustável e terminava
    // com `return state`, que descarta a seta esquerda antes de `navigate()` vê-la.
    // Resultado medido no aparelho: entrar em "Aparência → Tema", apertar ← e
    // nada acontecer — o painel não tinha saída, e o `← Categorias` do rodapé
    // era promessa vazia.
    //
    // A consulta agora é à mesma função que o desenho usa
    // (`settings_ajustavel`), então linha ajustável e linha navegável não podem
    // divergir. Nas demais, a seta cai no `navigate()` e `←` volta para a coluna.
    if state.focus.zone == Zone::SettingsContent
        && matches!(dir, Dir::Left | Dir::Right)
        && crate::state::settings_ajustavel(state.settings_category, state.focus.index)
    {
        let passo: isize = if dir == Dir::Right { 1 } else { -1 };
        let categoria = crate::state::SETTINGS_CATEGORIES
            .get(state.settings_category)
            .copied()
            .unwrap_or("");
        return match categoria {
            "sound" => {
                let alvo = state.settings.volume as i16 + (passo * 5) as i16;
                set_volume(state, alvo)
            }
            "system" => State {
                settings: Settings {
                    perf: state.settings.perf.step(passo),
                    ..state.settings
                },
                ..state
            },
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
        // Mesma função do gatilho: aqui o foco já está na faixa, então o chip
        // acompanha — que é o comportamento que sempre existiu neste caminho.
        return trocar_filtro(state, Filter::ALL[next]);
    }

    match navigate(&state, dir) {
        Some(focus) => {
            let carousel = if focus.zone == Zone::HomeCarousel {
                focus.index
            } else {
                state.carousel
            };
            // Andar pela coluna lateral muda a categoria na hora, como as abas
            // do topo. Exigir Confirm em cada item fazia o painel da direita
            // mostrar o conteúdo da categoria anterior enquanto o destaque
            // estava em outra.
            let settings_category = if focus.zone == Zone::SettingsSidebar
                && focus.index < crate::state::SETTINGS_CATEGORIES.len()
            {
                focus.index
            } else {
                state.settings_category
            };
            State {
                focus,
                carousel,
                settings_category,
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

/// Troca de sub-aba: filtros na Biblioteca, categorias nos Ajustes.
///
/// ## A regra que faz isto valer a pena
///
/// Quem troca de sub-aba **não sai do lugar onde está olhando**. Se o foco está
/// na grade de jogos, ele fica na grade; se está na faixa de filtros, acompanha
/// o chip novo; se está na coluna de categorias, acompanha a categoria.
///
/// Sem essa regra, `set_filter` (que existe para o `← →` **dentro** da faixa)
/// arrancaria o usuário da grade a cada toque no gatilho — o oposto de atalho,
/// porque obrigaria a voltar para baixo com o direcional toda vez.
fn sub_tab(state: State, delta: isize) -> State {
    if state.has_overlay() || state.screen != Screen::Running {
        return state;
    }

    match state.tab {
        Tab::Library => {
            let atual = Filter::ALL
                .iter()
                .position(|f| *f == state.library_filter)
                .unwrap_or(0);
            let novo = (atual as isize + delta).rem_euclid(Filter::ALL.len() as isize) as usize;
            trocar_filtro(state, Filter::ALL[novo])
        }
        Tab::Settings => trocar_categoria(state, delta),
        // Home e Apps não têm sub-abas; o gatilho fica sem efeito em vez de
        // fazer outra coisa, senão a mesma tecla teria dois significados.
        _ => state,
    }
}

/// Aplica o filtro preservando o lugar do usuário na tela.
fn trocar_filtro(state: State, filtro: Filter) -> State {
    let index = Filter::ALL.iter().position(|f| *f == filtro).unwrap_or(0);
    let na_faixa = state.focus.zone == Zone::LibraryFilters;
    let s = State {
        library_filter: filtro,
        ..state
    };
    if na_faixa {
        focus_on(s, Zone::LibraryFilters, index)
    } else {
        // `focus_on` re-clampa o índice à nova lista: um filtro mais curto não
        // pode deixar o foco apontando para um cartão que não existe mais.
        let (zona, indice) = (s.focus.zone, s.focus.index);
        focus_on(s, zona, indice)
    }
}

/// Cicla a categoria das configurações preservando o painel em foco.
fn trocar_categoria(state: State, delta: isize) -> State {
    let total = crate::state::SETTINGS_CATEGORIES.len() as isize;
    let atual = state.settings_category as isize;
    let novo = (atual + delta).rem_euclid(total) as usize;

    let s = State {
        settings_category: novo,
        ..state
    };

    match s.focus.zone {
        // Na coluna: o destaque acompanha a categoria, como no `↑↓`.
        Zone::SettingsSidebar => focus_on(s, Zone::SettingsSidebar, novo),
        // No painel: fica no painel, mas re-clampado — uma categoria com menos
        // linhas não pode deixar o foco numa linha que não existe.
        Zone::SettingsContent => {
            if crate::state::settings_rows(novo) == 0 {
                // Categoria só de texto ("Sobre"): não há linha para focar,
                // então o foco volta para a coluna em vez de sumir.
                focus_on(s, Zone::SettingsSidebar, novo)
            } else {
                let indice = s.focus.index;
                focus_on(s, Zone::SettingsContent, indice)
            }
        }
        _ => s,
    }
}

// ── confirmação ──────────────────────────────────────────────

fn confirm(state: State) -> State {
    let zone = state.focus.zone;
    let index = state.focus.index;

    match zone {
        Zone::HomeCarousel => {
            // **A aqui abre o jogo.** Antes só aparecia o aviso "Iniciando X..."
            // e nada acontecia: era um substituto de ação, não a ação.
            //
            // É o mesmo defeito que o botão "Jogar" da ficha tinha, uma camada
            // acima — a correção foi feita em `GameDetailActions` e este ramo
            // ficou para trás. E o rodapé da Home anuncia literalmente
            // `A — Jogar`, então a promessa estava na tela.
            //
            // O detalhe continua no X (`Cmd::Options` → `open_detail`), que é o
            // que o rodapé diz na tecla ao lado. Dois rótulos diferentes, dois
            // efeitos diferentes.
            //
            // Clona id e título antes de mover o estado: `toast` consome `state`
            // e o empréstimo de `focused_game` não pode sobreviver a isso.
            let alvo = state.focused_game().map(|game| (game.id, game.title.clone()));
            match alvo {
                Some((id, title)) => {
                    let s = toast(state, &format!("Iniciando {title}..."));
                    State {
                        lancar: Some(id),
                        ..s
                    }
                }
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

        // Cada linha do painel de configurações agora **faz alguma coisa**.
        //
        // Antes o painel da direita era só texto: `Zone::SettingsContent` não
        // existia, então nenhuma linha recebia foco e nada abria. Era a queixa
        // literal — "a aba de configuração não é navegável".
        Zone::SettingsContent => {
            let categoria = crate::state::SETTINGS_CATEGORIES
                .get(state.settings_category)
                .copied()
                .unwrap_or("about");
            match (categoria, index) {
                // Rede: a linha 0 abre Wi-Fi, a 1 abre Bluetooth.
                ("network", 0) => push(state, Overlay::WifiList),
                ("network", 1) => push(state, Overlay::BtList),
                ("sound", 0) => {
                    // Volume não abre tela: o ajuste é aqui mesmo, com ← →.
                    // Um overlay para um número seria mais passos, não menos.
                    let atual = state.settings.volume;
                    toast(state, &format!("Volume: {atual}% — use ← →"))
                }
                ("appearance", 0) => toast(state, "Tema único por enquanto"),
                ("appearance", 1) => toast(state, "A escala acompanha o painel"),
                ("system", 0) => toast(state, "launcher 0.1.0"),
                _ => toast(state, "Use ← → para ajustar"),
            }
        }

        // Lista de redes: a última linha é "procurar de novo".
        Zone::WifiNetworks => {
            let total = state.sem_fio.redes.len();
            if index >= total {
                // Varredura: a UI precisa saber para chamar o `wpa_cli`.
                //
                // Num aparelho **sem rádio** (R35S) esta linha fica inerte: sem
                // a guarda, apertar A nela ligaria "procurando…" para sempre —
                // uma promessa que o hardware nunca vai cumprir.
                if state.sem_fio.tem_wifi == Some(false) {
                    return state;
                }
                State {
                    sem_fio: crate::state::SemFio {
                        escaneando: true,
                        erro: None,
                        ..state.sem_fio
                    },
                    ..state
                }
            } else {
                let rede = state.sem_fio.redes[index].clone();
                if rede.protegida && !rede.salva {
                    // Pede a senha: guarda o alvo e limpa o campo.
                    let s = State {
                        sem_fio: crate::state::SemFio {
                            alvo: Some(rede.ssid),
                            senha: String::new(),
                            senha_visivel: false,
                            erro: None,
                            ..state.sem_fio
                        },
                        ..state
                    };
                    push(s, Overlay::WifiPassword)
                } else {
                    // Rede aberta ou já salva: conecta direto.
                    State {
                        sem_fio: crate::state::SemFio {
                            alvo: Some(rede.ssid),
                            ..state.sem_fio
                        },
                        ..state
                    }
                }
            }
        }

        // Teclado da senha: mesma grade da busca, destino diferente.
        Zone::PasswordKeys => {
            let row = index / crate::state::KEYBOARD_COLS;
            let col = index % crate::state::KEYBOARD_COLS;
            match crate::state::KEYBOARD
                .get(row)
                .and_then(|r| r.get(col))
                .copied()
            {
                Some(BACKSPACE) => State {
                    sem_fio: crate::state::SemFio {
                        senha: {
                            let mut s = state.sem_fio.senha.clone();
                            s.pop();
                            s
                        },
                        ..state.sem_fio
                    },
                    ..state
                },
                Some(c) => State {
                    sem_fio: crate::state::SemFio {
                        senha: format!("{}{c}", state.sem_fio.senha),
                        ..state.sem_fio
                    },
                    ..state
                },
                None => state,
            }
        }

        // Bluetooth: última linha é "procurar de novo".
        Zone::BtDevices => {
            let total = state.sem_fio.dispositivos.len();
            if index >= total {
                // Mesma guarda do Wi-Fi: sem rádio, "procurar" não faz nada.
                if state.sem_fio.tem_bt == Some(false) {
                    return state;
                }
                State {
                    sem_fio: crate::state::SemFio {
                        bt_escaneando: true,
                        ..state.sem_fio
                    },
                    ..state
                }
            } else {
                let mac = state.sem_fio.dispositivos[index].mac.clone();
                State {
                    sem_fio: crate::state::SemFio {
                        bt_alvo: Some(mac),
                        ..state.sem_fio
                    },
                    ..state
                }
            }
        }

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
        // Sem overlay, "voltar" sobe um nível dentro da própria aba.
        //
        // Antes isto devolvia o estado intacto: dentro do painel de
        // configurações `B` não fazia nada, e como `←` também estava engolido
        // pela linha ajustável, a tela não tinha **nenhuma** saída. Quem entra
        // numa categoria fica preso se as duas rotas de volta falham juntas.
        //
        // Vale só para o painel: nas abas de conteúdo único (Home, Biblioteca)
        // não há nível acima, e voltar continua sem efeito.
        if state.focus.zone == Zone::SettingsContent {
            return focus_on(
                State {
                    focus: Focus::at(Zone::SettingsSidebar, state.settings_category),
                    ..state
                },
                Zone::SettingsSidebar,
                state.settings_category,
            );
        }
        return state;
    }

    let mut overlays = state.overlays.clone();
    overlays.pop();

    let zone = match overlays.last() {
        Some(overlay) => overlay.zone(),
        None => state.tab.zone(),
    };
    let index = state.saved_focus.get(&zone).copied().unwrap_or(0);

    // Sair do teclado de senha apaga o que foi digitado.
    //
    // Sem isto a senha continua na memória depois de `B`: o usuário desiste,
    // volta à lista e ela segue guardada. `CancelarSenha` já fazia isso; `B`
    // não fazia, e o `B` é justamente o que o rodapé anuncia.
    let saindo_do_teclado = state.top_overlay() == Some(Overlay::WifiPassword);
    let mut s = State { overlays, ..state };
    if saindo_do_teclado {
        s.sem_fio.limpar_senha();
    }

    focus_on(s, zone, index)
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
            let nome = format!("{cmd:?}");
            s = reduce(s, cmd);
            assert_eq!(s.screen, Screen::Running, "{nome} não acordou a tela");
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
            let nome = format!("{cmd:?}");
            let s = reduce(s, cmd);
            assert_eq!(
                s.screen,
                Screen::Sleeping,
                "{nome} não é entrada de usuário e não pode acordar"
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

    /// O bug relatado: "aba home cards não estão abrindo o jogo quando se
    /// escolhe o botão jogar".
    ///
    /// `A` na Home caía num ramo que só trocava o aviso para `Iniciando X...`
    /// — o mesmo substituto de ação que já tinha sido corrigido na ficha, uma
    /// camada acima. E o rodapé da Home anuncia `A — Jogar`, então a promessa
    /// estava escrita na tela.
    /// `R2` na Biblioteca troca o filtro **sem tirar o usuário da grade**.
    ///
    /// `set_filter` (o caminho do `← →` dentro da faixa) move o foco para a
    /// faixa. Reusá-lo aqui faria cada toque no gatilho arrancar o foco da
    /// grade — o oposto de atalho: obrigaria a descer de novo toda vez.
    #[test]
    fn gatilho_troca_o_filtro_e_o_foco_fica_na_grade() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 2);

        s = reduce(s, Cmd::SubNext);
        assert_eq!(s.library_filter, Filter::Favorites);
        assert_eq!(
            s.focus.zone,
            Zone::LibraryGrid,
            "o gatilho não pode arrancar o foco da grade"
        );
        assert_eq!(s.focus.index, 2);

        s = reduce(s, Cmd::SubPrev);
        assert_eq!(s.library_filter, Filter::All);
        assert_eq!(s.focus.zone, Zone::LibraryGrid);
    }

    #[test]
    fn gatilho_na_faixa_de_filtros_acompanha_o_chip() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.focus = Focus::at(Zone::LibraryFilters, 0);

        s = reduce(s, Cmd::SubNext);
        assert_eq!(s.library_filter, Filter::Favorites);
        assert_eq!(s.focus.zone, Zone::LibraryFilters);
        assert_eq!(s.focus.index, 1, "o destaque segue o chip escolhido");
    }

    #[test]
    fn gatilho_da_a_volta_nos_filtros() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.library_filter = Filter::All;
        s.focus = Focus::at(Zone::LibraryGrid, 0);

        s = reduce(s, Cmd::SubPrev);
        assert_eq!(s.library_filter, Filter::Recent, "para trás dá a volta");
        s = reduce(s, Cmd::SubNext);
        assert_eq!(s.library_filter, Filter::All);
    }

    /// Filtro mais curto não pode deixar o foco num cartão que não existe.
    ///
    /// "Recentes" costuma ter menos itens que "Todos"; um índice alto ficaria
    /// apontando para o vazio e o `A` não abriria nada — sem erro nenhum.
    #[test]
    fn filtro_mais_curto_reclampa_o_foco() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.library_filter = Filter::All;
        s.focus = Focus::at(Zone::LibraryGrid, 7); // último de 8

        s = reduce(s, Cmd::SubNext); // → Favoritos (só os pares: 4 itens)
        let total = s.filtered_games().len();
        assert!(total > 0);
        assert!(
            s.focus.index < total,
            "foco {} fora da lista de {total}",
            s.focus.index
        );
    }

    /// Nos Ajustes o gatilho troca de categoria.
    #[test]
    fn gatilho_troca_a_categoria_dos_ajustes() {
        let mut s = demo();
        s.tab = Tab::Settings;
        s.settings_category = 0;
        s.focus = Focus::at(Zone::SettingsSidebar, 0);

        s = reduce(s, Cmd::SubNext);
        assert_eq!(s.settings_category, 1);
        assert_eq!(s.focus.index, 1, "na coluna, o destaque acompanha");

        s = reduce(s, Cmd::SubPrev);
        assert_eq!(s.settings_category, 0);
    }

    #[test]
    fn gatilho_no_painel_de_ajustes_fica_no_painel() {
        let mut s = demo();
        s.tab = Tab::Settings;
        s.settings_category = 2; // rede, 2 linhas
        s.focus = Focus::at(Zone::SettingsContent, 0);

        s = reduce(s, Cmd::SubNext); // → controle, 7 linhas
        assert_eq!(s.settings_category, 3);
        assert_eq!(s.focus.zone, Zone::SettingsContent, "continua no painel");
    }

    /// Categoria sem linhas ("Sobre") não pode deixar o foco num painel vazio.
    #[test]
    fn gatilho_para_categoria_sem_linhas_devolve_o_foco_a_coluna() {
        let mut s = demo();
        s.tab = Tab::Settings;
        s.settings_category = 4; // sistema, 2 linhas
        s.focus = Focus::at(Zone::SettingsContent, 1);

        s = reduce(s, Cmd::SubNext); // → sobre, 0 linhas
        assert_eq!(s.settings_category, 5);
        assert_eq!(
            s.focus.zone,
            Zone::SettingsSidebar,
            "painel vazio prenderia o foco sem nada para onde ir"
        );
    }

    /// Com overlay aberto o gatilho não troca nada por baixo.
    /// R35S: **não tem rádio**. A tela não pode dizer "não conectado".
    ///
    /// "Não conectado" sugere que dá para conectar. Num aparelho sem Wi-Fi nem
    /// Bluetooth — os DTBs do r33s/r35s/r36s não têm nó de rádio — isso é
    /// promessa de capacidade inexistente, a mesma classe do "Tema Giochi Dark".
    #[test]
    fn aparelho_sem_radio_nao_promete_conexao() {
        let mut s = demo();
        s = reduce(s, Cmd::CapacidadesSemFio { wifi: false, bluetooth: false });

        assert_eq!(s.sem_fio.tem_wifi, Some(false));
        assert_eq!(s.sem_fio.tem_bt, Some(false));
        assert!(s.sem_fio.sem_radio());
        // E não pode estar "procurando" o que não existe.
        assert!(!s.sem_fio.escaneando);
        assert!(!s.sem_fio.bt_escaneando);
    }

    /// Abrir Wi-Fi/BT sem rádio não liga a animação de varredura.
    ///
    /// Se ligasse, a tela pulsaria "procurando…" para sempre — pior que dizer
    /// "não tem", porque parece que está tentando.
    #[test]
    fn abrir_lista_sem_radio_nao_marca_procurando() {
        for (cmd, campo, overlay) in [
            (Cmd::AbrirWifi, 0usize, Overlay::WifiList),
            (Cmd::AbrirBluetooth, 1, Overlay::BtList),
        ] {
            let mut s = demo();
            s = reduce(s, Cmd::CapacidadesSemFio { wifi: false, bluetooth: false });
            s = reduce(s, cmd);
            assert_eq!(s.top_overlay(), Some(overlay));
            let procurando = if campo == 0 {
                s.sem_fio.escaneando
            } else {
                s.sem_fio.bt_escaneando
            };
            assert!(!procurando, "{overlay:?}: não pode procurar sem rádio");
        }
    }

    /// Com rádio, tudo continua como antes — a correção não pode quebrar o X55.
    /// Apertar "procurar de novo" num aparelho sem rádio não pode iniciar nada.
    ///
    /// A linha final da lista existe sempre; sem guarda ela ligaria
    /// "procurando…" indefinidamente — o usuário veria um spinner eterno em vez
    /// do motivo.
    #[test]
    fn procurar_de_novo_e_inerte_sem_radio() {
        let mut s = demo();
        s = reduce(s, Cmd::CapacidadesSemFio { wifi: false, bluetooth: false });
        s = reduce(s, Cmd::AbrirWifi);
        // nenhuma rede, então a única linha é a de procurar
        s.focus = Focus::at(Zone::WifiNetworks, 0);
        let s = reduce(s, Cmd::Confirm);
        assert!(!s.sem_fio.escaneando, "não pode procurar sem rádio");

        let mut b = demo();
        b = reduce(b, Cmd::CapacidadesSemFio { wifi: false, bluetooth: false });
        b = reduce(b, Cmd::AbrirBluetooth);
        b.focus = Focus::at(Zone::BtDevices, 0);
        let b = reduce(b, Cmd::Confirm);
        assert!(!b.sem_fio.bt_escaneando, "não pode procurar sem rádio");
    }

    #[test]
    fn aparelho_com_radio_continua_procurando() {
        let mut s = demo();
        s = reduce(s, Cmd::CapacidadesSemFio { wifi: true, bluetooth: true });
        assert!(!s.sem_fio.sem_radio());

        s = reduce(s, Cmd::AbrirWifi);
        assert!(s.sem_fio.escaneando, "com rádio tem de procurar");
    }

    /// Antes da sondagem não se sabe de nada — e desconhecido não é ausência.
    #[test]
    fn capacidade_desconhecida_nao_e_tratada_como_ausente() {
        let s = demo();
        assert_eq!(s.sem_fio.tem_wifi, None);
        assert!(!s.sem_fio.sem_radio(), "None não pode virar 'sem rádio'");
    }

    #[test]
    fn gatilho_nao_age_com_overlay_aberto() {
        let mut s = demo();
        s.tab = Tab::Library;
        s.overlays.push(Overlay::QuickSettings);
        s = reduce(s, Cmd::SubNext);
        assert_eq!(s.library_filter, Filter::All);
        assert_eq!(s.overlays.len(), 1);
    }

    /// Home e Apps não têm sub-aba: os gatilhos ficam inertes em vez de fazer
    /// outra coisa. A mesma tecla com dois significados é pior que tecla morta.
    #[test]
    fn gatilho_nao_faz_nada_onde_nao_ha_sub_aba() {
        for tab in [Tab::Home, Tab::Apps] {
            let mut s = demo();
            s.tab = tab;
            s.focus = Focus::at(tab.zone(), 0);
            let antes = s.clone();
            s = reduce(s, Cmd::SubNext);
            assert_eq!(s.library_filter, antes.library_filter);
            assert_eq!(s.settings_category, antes.settings_category);
            assert_eq!(s.tab, tab);
        }
    }

    #[test]
    fn a_na_home_pede_o_lancamento_do_jogo_em_foco() {
        let mut s = demo();
        s.tab = Tab::Home;
        s.focus = Focus::at(Zone::HomeCarousel, 3);
        s.carousel = 3;
        let esperado = s.games[3].id;

        s = reduce(s, Cmd::Confirm);

        assert_eq!(
            s.lancar,
            Some(esperado),
            "A na Home tem de pedir o lançamento, não só avisar"
        );
    }

    /// Todo caminho que **promete** iniciar um jogo tem de pedir o lançamento.
    ///
    /// A correção anterior foi feita só na ficha e a Home ficou para trás. Este
    /// teste percorre os três caminhos de uma vez: qualquer ramo novo que
    /// mostre "Iniciando…" sem preencher `lancar` reprova aqui.
    #[test]
    fn todo_caminho_de_iniciar_jogo_pede_lancamento() {
        // 1. Home: A no cartão.
        let mut home = demo();
        home.tab = Tab::Home;
        home.focus = Focus::at(Zone::HomeCarousel, 1);
        home.carousel = 1;
        let id_home = home.games[1].id;
        let home = reduce(home, Cmd::Confirm);

        // 2. Home: X abre a ficha, depois A em "Jogar".
        let mut via_ficha_home = demo();
        via_ficha_home.tab = Tab::Home;
        via_ficha_home.focus = Focus::at(Zone::HomeCarousel, 2);
        via_ficha_home.carousel = 2;
        let id_ficha = via_ficha_home.games[2].id;
        let mut via_ficha_home = reduce(via_ficha_home, Cmd::Options);
        via_ficha_home.focus = Focus::at(Zone::GameDetailActions, 0);
        let via_ficha_home = reduce(via_ficha_home, Cmd::Confirm);

        // 3. Biblioteca: A no cartão abre a ficha, A em "Jogar".
        let mut via_biblioteca = demo();
        via_biblioteca.tab = Tab::Library;
        via_biblioteca.focus = Focus::at(Zone::LibraryGrid, 0);
        let id_bib = via_biblioteca.filtered_games()[0];
        let id_bib = via_biblioteca.games[id_bib].id;
        let mut via_biblioteca = reduce(via_biblioteca, Cmd::Confirm);
        via_biblioteca.focus = Focus::at(Zone::GameDetailActions, 0);
        let via_biblioteca = reduce(via_biblioteca, Cmd::Confirm);

        assert_eq!(home.lancar, Some(id_home), "Home: A no cartão");
        assert_eq!(
            via_ficha_home.lancar,
            Some(id_ficha),
            "Home: X na ficha, A em Jogar"
        );
        assert_eq!(
            via_biblioteca.lancar,
            Some(id_bib),
            "Biblioteca: ficha, A em Jogar"
        );
    }

    #[test]
    fn a_na_home_sem_jogos_nao_estoura() {
        let mut s = State::new(Vec::new(), Vec::new());
        s.screen = Screen::Running;
        s.focus = Focus::at(Zone::HomeCarousel, 0);
        let s = reduce(s, Cmd::Confirm);
        assert_eq!(s.lancar, None);
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

    /// O bug relatado no aparelho: "se eu entrar em uma das configs não
    /// consigo voltar".
    ///
    /// A causa era um `if` de linha ajustável que terminava em `return state` e
    /// descartava a seta esquerda **antes** de `navigate()` rodar. O teste
    /// percorre as seis categorias e todas as linhas de cada uma: qualquer uma
    /// que não devolva o foco à coluna reprova aqui, e nenhuma combinação nova
    /// escapa.
    #[test]
    fn sair_do_painel_com_esquerda_funciona_em_toda_linha() {
        for (cat, _) in crate::state::SETTINGS_CATEGORIES.iter().enumerate() {
            let linhas = crate::state::settings_rows(cat);
            for linha in 0..linhas {
                let mut s = demo();
                s.tab = Tab::Settings;
                s.settings_category = cat;
                s.focus = Focus::at(Zone::SettingsContent, linha);
                let antes = s.clone();

                s = reduce(s, Cmd::Nav(Dir::Left));

                // Linha ajustável: ← ajusta, então o foco **fica**. Mas tem de
                // ajustar **de verdade** — senão a tela promete um ajuste que
                // não existe, que foi metade da queixa ("aperto seta para
                // trocar o tema e não faz nada").
                //
                // A primeira versão deste teste só verificava que o foco ficava.
                // Com `settings_ajustavel` devolvendo `true` para tudo, ela
                // continuava passando: um teste que se desculpa não testa nada.
                if crate::state::settings_ajustavel(cat, linha) {
                    assert_eq!(
                        s.focus.zone,
                        Zone::SettingsContent,
                        "categoria {cat} linha {linha}: ajustável, ← deve ajustar"
                    );
                    assert_ne!(
                        s.settings, antes.settings,
                        "categoria {cat} linha {linha}: declarada ajustável, mas ← \
                         não mudou nada — é o 'aperto e não faz nada' de novo"
                    );
                    continue;
                }

                assert_eq!(
                    s.focus.zone,
                    Zone::SettingsSidebar,
                    "categoria {cat} linha {linha}: ← NÃO voltou para a coluna"
                );
                assert_eq!(
                    s.focus.index, cat,
                    "categoria {cat} linha {linha}: voltou na categoria errada"
                );
            }
        }
    }

    /// A outra saída: `B` sobe um nível mesmo sem overlay.
    ///
    /// Precisa existir porque nas linhas ajustáveis `←` não volta — e ter só
    /// uma rota de saída foi o que transformou um bug pequeno em tela sem saída.
    /// Nenhuma zona pode capturar uma seta e não ter saída.
    ///
    /// É a generalização do bug da seta esquerda: um `if` que trata as setas
    /// como ajuste e termina em `return state` cria uma zona da qual não se
    /// sai. Aqui, para cada zona, ou existe uma direção que leva a outra zona,
    /// ou existe uma tecla que sai (`B`). Zona que falhe nas duas é armadilha.
    #[test]
    fn nenhuma_zona_captura_as_setas_sem_ter_saida() {
        use crate::state::Overlay;

        // Zonas-raiz: são o nível mais alto da aba, não há para onde "voltar".
        const RAIZES: [Zone; 4] = [
            Zone::HomeCarousel,
            Zone::LibraryGrid,
            Zone::AppsGrid,
            Zone::SettingsSidebar,
        ];

        let zonas_com_overlay = [
            (Overlay::WifiList, Zone::WifiNetworks),
            (Overlay::WifiPassword, Zone::PasswordKeys),
            (Overlay::BtList, Zone::BtDevices),
        ];

        let mut checadas = 0usize;
        for zona in [
            Zone::LibraryFilters,
            Zone::SettingsContent,
            Zone::SearchKeyboard,
            Zone::SearchResults,
            Zone::QuickSettings,
            Zone::GameDetailActions,
            Zone::PowerActions,
            Zone::WifiNetworks,
            Zone::PasswordKeys,
            Zone::BtDevices,
        ] {
            if RAIZES.contains(&zona) {
                continue;
            }
            checadas += 1;

            // Uma base plausível para cada zona: precisa de linhas para ter foco.
            let mut base = demo();
            base.tab = Tab::Settings;
            base.settings_category = 2;
            base.overlays = match zona {
                Zone::WifiNetworks => vec![Overlay::WifiList],
                Zone::PasswordKeys => vec![Overlay::WifiPassword],
                Zone::BtDevices => vec![Overlay::BtList],
                Zone::QuickSettings => vec![Overlay::QuickSettings],
                Zone::GameDetailActions => {
                    base.selected_game = Some(0);
                    vec![Overlay::GameDetail]
                }
                Zone::PowerActions => vec![Overlay::PowerPopup],
                Zone::SearchKeyboard | Zone::SearchResults => vec![Overlay::Search],
                _ => Vec::new(),
            };
            if zona == Zone::LibraryFilters {
                base.tab = Tab::Library;
            }
            if zona == Zone::SearchResults || zona == Zone::SearchKeyboard {
                base.search_query = "jogo".into();
            }
            let linhas = base.zone_len(zona);
            base.focus = Focus::at(zona, 0);

            for linha in 0..linhas.max(1) {
                let mut s = base.clone();
                s.focus = Focus::at(zona, linha.min(linhas.max(1) - 1));

                let foge_por_seta = [Dir::Up, Dir::Down, Dir::Left, Dir::Right]
                    .iter()
                    .any(|d| reduce(s.clone(), Cmd::Nav(*d)).focus.zone != zona);

                let foge_por_b = reduce(s.clone(), Cmd::Back).focus.zone != zona
                    || reduce(s.clone(), Cmd::Back).overlays.len() < s.overlays.len();

                assert!(
                    foge_por_seta || foge_por_b,
                    "{zona:?} linha {linha}: nenhuma seta sai e B não sai — armadilha"
                );
            }
        }
        assert_eq!(checadas, 10, "a lista de zonas a checar mudou; revise este teste");
        let _ = zonas_com_overlay;
    }

    #[test]
    fn b_volta_do_painel_para_a_coluna() {
        let mut s = demo();
        s.tab = Tab::Settings;
        s.settings_category = 1;
        s.focus = Focus::at(Zone::SettingsContent, 0);

        s = reduce(s, Cmd::Back);
        assert_eq!(s.focus.zone, Zone::SettingsSidebar);
        assert_eq!(s.focus.index, 1);
    }

    #[test]
    fn seta_na_linha_ajustavel_muda_o_valor_e_nao_o_foco() {
        let mut s = demo();
        s.tab = Tab::Settings;
        s.settings_category = 1; // som
        s.settings.volume = 40;
        s.focus = Focus::at(Zone::SettingsContent, 0);

        s = reduce(s, Cmd::Nav(Dir::Right));
        assert_eq!(s.settings.volume, 45, "→ aumenta o volume");
        assert_eq!(s.focus.zone, Zone::SettingsContent);

        s = reduce(s, Cmd::Nav(Dir::Left));
        assert_eq!(s.settings.volume, 40, "← diminui o volume");
    }

    #[test]
    fn linha_nao_ajustavel_nao_tem_rodape_de_ajuste() {
        // `settings_ajustavel` é consultada pelo rodapé; se ela dissesse "sim"
        // para o tema, a tela voltaria a prometer um ajuste que não existe.
        for (cat, _) in crate::state::SETTINGS_CATEGORIES.iter().enumerate() {
            let nome = crate::state::SETTINGS_CATEGORIES[cat];
            for linha in 0..crate::state::settings_rows(cat) {
                let ajustavel = crate::state::settings_ajustavel(cat, linha);
                if nome == "appearance" {
                    assert!(!ajustavel, "aparência não tem valor ajustável");
                }
            }
        }
    }

    #[test]
    fn voltar_do_teclado_de_senha_apaga_o_que_foi_digitado() {
        use crate::state::{Rede, SemFio};

        let mut s = demo();
        s.sem_fio = SemFio {
            redes: vec![Rede {
                ssid: "Kill.exe".into(),
                sinal: 80,
                protegida: true,
                salva: false,
            }],
            ..SemFio::default()
        };
        s = reduce(s, Cmd::AbrirWifi);
        s = reduce(s, Cmd::Confirm); // escolhe a única rede: abre o teclado
        assert_eq!(s.top_overlay(), Some(Overlay::WifiPassword));
        assert_eq!(s.sem_fio.alvo.as_deref(), Some("Kill.exe"));

        s.sem_fio.senha = "segredo-digitado".into();
        s = reduce(s, Cmd::Back);

        assert_eq!(s.top_overlay(), Some(Overlay::WifiList), "volta para a lista");
        assert!(s.sem_fio.senha.is_empty(), "a senha não pode sobreviver ao B");
        assert!(s.sem_fio.alvo.is_none(), "nem o alvo");
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
