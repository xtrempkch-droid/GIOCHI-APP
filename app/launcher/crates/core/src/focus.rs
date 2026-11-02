//! Foco e navegação espacial.
//!
//! O foco é `(zona, índice)`, como no protótipo — a diferença é que a zona é
//! enum, então não existe `zoneCount` caindo no `default: 0` e travando a
//! navegação em silêncio.

use crate::cmd::Dir;
use crate::state::State;

/// Regiões navegáveis da interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    HomeCarousel,
    LibraryFilters,
    LibraryGrid,
    AppsGrid,
    SettingsSidebar,
    SearchKeyboard,
    SearchResults,
    QuickSettings,
    GameDetailActions,
    PowerActions,
}

impl Zone {
    /// Colunas da grade. Zonas verticais (lista) têm 1.
    pub fn columns(self) -> usize {
        match self {
            Zone::HomeCarousel => 1,
            Zone::LibraryFilters => 3,
            Zone::LibraryGrid => 4,
            Zone::AppsGrid => 4,
            Zone::SettingsSidebar => 1,
            Zone::SearchKeyboard => 7,
            Zone::SearchResults => 1,
            Zone::QuickSettings => 1,
            Zone::GameDetailActions => 4,
            Zone::PowerActions => 1,
        }
    }

    /// Zonas que percorrem em laço (o carrossel dá a volta).
    pub fn wraps(self) -> bool {
        matches!(self, Zone::HomeCarousel)
    }
}

/// Posição de foco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Focus {
    pub zone: Zone,
    pub index: usize,
}

impl Focus {
    pub fn new(zone: Zone) -> Self {
        Self { zone, index: 0 }
    }

    pub fn at(zone: Zone, index: usize) -> Self {
        Self { zone, index }
    }
}

/// Calcula o próximo foco, ou `None` se a navegação não muda o foco.
///
/// Devolver `None` em vez de uma cópia do mesmo estado deixa explícito que
/// nada mudou — e permite ao reducer decidir se o comando tem outro efeito
/// (ajustar volume, trocar filtro).
pub fn navigate(state: &State, dir: Dir) -> Option<Focus> {
    let focus = state.focus;
    let zone = focus.zone;

    // Carrossel: só horizontal, com wrap.
    if zone == Zone::HomeCarousel {
        if !matches!(dir, Dir::Left | Dir::Right) {
            return None;
        }
        let len = state.zone_len(zone);
        if len == 0 {
            return None;
        }
        let step: isize = if dir == Dir::Right { 1 } else { -1 };
        let next = (focus.index as isize + step).rem_euclid(len as isize) as usize;
        return Some(Focus::at(zone, next));
    }

    // Ajustes rápidos: vertical move, horizontal é ajuste (tratado no reducer).
    if zone == Zone::QuickSettings {
        let len = state.zone_len(zone);
        let next = match dir {
            Dir::Up => focus.index.saturating_sub(1),
            Dir::Down => (focus.index + 1).min(len.saturating_sub(1)),
            Dir::Left | Dir::Right => return None,
        };
        return if next == focus.index {
            None
        } else {
            Some(Focus::at(zone, next))
        };
    }

    // Filtros: horizontal troca filtro (reducer), vertical desce para a grade.
    if zone == Zone::LibraryFilters {
        return match dir {
            Dir::Down => Some(Focus::at(Zone::LibraryGrid, 0)),
            _ => None,
        };
    }

    // Busca: sair da grade de resultados volta para o teclado.
    if zone == Zone::SearchResults && dir == Dir::Left {
        return Some(Focus::at(Zone::SearchKeyboard, 0));
    }

    // Busca: da última coluna do teclado, se há resultado, entra na lista.
    if zone == Zone::SearchKeyboard && dir == Dir::Right {
        let col = focus.index % Zone::SearchKeyboard.columns();
        if col == Zone::SearchKeyboard.columns() - 1 && !state.search_results().is_empty() {
            return Some(Focus::at(Zone::SearchResults, 0));
        }
    }

    let cols = zone.columns();
    let len = state.zone_len(zone);
    if cols == 0 || len == 0 {
        return None;
    }

    // Biblioteca: subir da primeira linha volta para os filtros.
    if zone == Zone::LibraryGrid && dir == Dir::Up && focus.index < cols {
        return Some(Focus::at(Zone::LibraryFilters, 0));
    }

    let rows = len.div_ceil(cols);
    let row = focus.index / cols;
    let col = focus.index % cols;

    let (next_row, next_col) = match dir {
        Dir::Up => (row.saturating_sub(1), col),
        Dir::Down => ((row + 1).min(rows.saturating_sub(1)), col),
        Dir::Left => (row, col.saturating_sub(1)),
        Dir::Right => (row, (col + 1).min(cols.saturating_sub(1))),
    };

    // A última linha pode estar incompleta: recua para o último item válido.
    let mut next = next_row * cols + next_col;
    if next >= len {
        next = len - 1;
    }

    if next == focus.index {
        None
    } else {
        Some(Focus::at(zone, next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{App, Game, State};

    fn state_with(games: usize, apps: usize) -> State {
        let games = (0..games)
            .map(|i| Game::new(i as u32, &format!("G{i}"), "Cat", 1.0, "hoje", i % 2 == 0))
            .collect();
        let apps = (0..apps)
            .map(|i| App::new(&format!("a{i}"), &format!("A{i}"), "icon", "Cat"))
            .collect();
        State::new(games, apps)
    }

    #[test]
    fn carrossel_da_a_volta_nos_dois_sentidos() {
        let mut s = state_with(8, 0);
        s.focus = Focus::at(Zone::HomeCarousel, 0);

        let left = navigate(&s, Dir::Left).unwrap();
        assert_eq!(left.index, 7, "esquerda a partir de 0 deve ir para o último");

        s.focus = Focus::at(Zone::HomeCarousel, 7);
        let right = navigate(&s, Dir::Right).unwrap();
        assert_eq!(right.index, 0, "direita a partir do último deve voltar a 0");
    }

    #[test]
    fn carrossel_ignora_vertical() {
        let s = state_with(8, 0);
        assert!(navigate(&s, Dir::Up).is_none());
        assert!(navigate(&s, Dir::Down).is_none());
    }

    #[test]
    fn grade_anda_em_bloco_de_quatro_colunas() {
        let mut s = state_with(8, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 0);

        assert_eq!(navigate(&s, Dir::Right).unwrap().index, 1);
        assert_eq!(navigate(&s, Dir::Down).unwrap().index, 4);
    }

    #[test]
    fn grade_nao_sai_pelas_bordas() {
        let mut s = state_with(8, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 0);

        assert!(navigate(&s, Dir::Left).is_none(), "coluna 0 não sai para a esquerda");
        assert!(navigate(&s, Dir::Up).is_some(), "subir da primeira linha vai aos filtros");
    }

    #[test]
    fn ultima_linha_incompleta_recua_para_item_valido() {
        // 6 jogos em grade de 4: segunda linha tem 2 itens (índices 4 e 5).
        let mut s = state_with(6, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 5);

        let next = navigate(&s, Dir::Right);
        assert!(next.is_none(), "à direita do último item não há para onde ir");
    }

    #[test]
    fn subir_da_primeira_linha_vai_para_filtros() {
        let mut s = state_with(8, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 2);

        let next = navigate(&s, Dir::Up).unwrap();
        assert_eq!(next.zone, Zone::LibraryFilters);
        assert_eq!(next.index, 0);
    }

    #[test]
    fn ajustes_rapidos_limitam_nas_pontas() {
        let mut s = state_with(0, 0);
        s.overlays.push(crate::state::Overlay::QuickSettings);
        s.focus = Focus::at(Zone::QuickSettings, 0);

        assert!(navigate(&s, Dir::Up).is_none(), "não passa do topo");
        assert_eq!(navigate(&s, Dir::Down).unwrap().index, 1);

        s.focus = Focus::at(Zone::QuickSettings, 7);
        assert!(navigate(&s, Dir::Down).is_none(), "não passa do fim");
    }

    #[test]
    fn teclado_sai_para_resultados_a_partir_da_ultima_coluna() {
        let mut s = state_with(8, 0);
        s.search_query = "G1".to_string();
        s.overlays.push(crate::state::Overlay::Search);
        // Linha 0, coluna 6 (última coluna).
        s.focus = Focus::at(Zone::SearchKeyboard, 6);

        let next = navigate(&s, Dir::Right).unwrap();
        assert_eq!(next.zone, Zone::SearchResults);
    }

    #[test]
    fn teclado_nao_sai_para_resultados_sem_resultado() {
        let mut s = state_with(8, 0);
        s.search_query = "zzzz".to_string();
        s.overlays.push(crate::state::Overlay::Search);
        s.focus = Focus::at(Zone::SearchKeyboard, 6);

        assert!(navigate(&s, Dir::Right).is_none());
    }

    #[test]
    fn resultados_voltam_para_o_teclado_a_esquerda() {
        let mut s = state_with(8, 0);
        s.search_query = "G1".to_string();
        s.overlays.push(crate::state::Overlay::Search);
        s.focus = Focus::at(Zone::SearchResults, 0);

        let next = navigate(&s, Dir::Left).unwrap();
        assert_eq!(next.zone, Zone::SearchKeyboard);
    }
}
