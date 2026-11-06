//! Foco e navegação espacial.
//!
//! O foco é `(zona, índice)`, como no protótipo — a diferença é que a zona é
//! enum, então não existe `zoneCount` caindo no `default: 0` e travando a
//! navegação em silêncio.

use crate::cmd::Dir;
use crate::state::{State, KEYBOARD_COLS};

/// Regiões navegáveis da interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    HomeCarousel,
    LibraryFilters,
    LibraryGrid,
    AppsGrid,
    SettingsSidebar,
    /// Linhas do painel de conteúdo das configurações.
    ///
    /// Existia `SettingsSidebar` e **não** existia esta. O painel da direita era
    /// desenhado como texto e nenhuma linha recebia foco — a aba de ajustes era
    /// navegável só na coluna da esquerda, e as opções de dentro não abriam.
    SettingsContent,
    /// Lista de redes Wi-Fi.
    WifiNetworks,
    /// Teclado da senha. Usa o mesmo `KEYBOARD` da busca, com outro destino.
    PasswordKeys,
    /// Lista de dispositivos Bluetooth.
    BtDevices,
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
            Zone::SettingsContent => 1,
            Zone::WifiNetworks => 1,
            Zone::PasswordKeys => KEYBOARD_COLS,
            Zone::BtDevices => 1,
            Zone::SearchKeyboard => 7,
            Zone::SearchResults => 1,
            Zone::QuickSettings => 1,
            Zone::GameDetailActions => 4,
            Zone::PowerActions => 1,
        }
    }

    /// Zonas que percorrem em laço: andar numa direção nunca fica sem efeito.
    ///
    /// Estava ligado só no carrossel, e a função existia sem uso nenhum — a
    /// intenção aparecia no código e não chegava ao teclado. Com os 4 jogos do
    /// catálogo numa grade de 4 colunas, `→` ia de 0 a 3 e ali **morria**, e
    /// `↓` não fazia nada: era a queixa "a navegação da aba Jogos não é livre".
    ///
    /// Numa lista maior o problema é pior: para ir do fim de uma linha ao
    /// começo da seguinte era preciso `↓` e depois `←` quatro vezes.
    pub fn wraps(self) -> bool {
        matches!(
            self,
            Zone::HomeCarousel | Zone::LibraryGrid | Zone::AppsGrid
        )
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

    // Configurações: a coluna lateral é a esquerda do painel de conteúdo.
    //
    // Sem esta ligação o painel da direita nunca recebia foco: dava para
    // destacar a categoria, mas não para entrar nas opções de dentro. Era a
    // queixa literal — "a aba de configuração não é navegável".
    if zone == Zone::SettingsSidebar && dir == Dir::Right {
        return if state.zone_len(Zone::SettingsContent) == 0 {
            None
        } else {
            Some(Focus::at(Zone::SettingsContent, 0))
        };
    }

    // E a volta: ← do painel devolve para a coluna, na categoria atual.
    if zone == Zone::SettingsContent && dir == Dir::Left {
        return Some(Focus::at(Zone::SettingsSidebar, state.settings_category));
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

    // Grades de conteúdo usam o arranjo que a **view desenhou**.
    //
    // ## O bug que isto corrige
    //
    // A navegação assumia `Zone::columns()` (4 fixo) e a view escolhia as
    // colunas por geometria (`grade_para`) e paginava. No aparelho, com 4
    // jogos num painel 16:9, a grade desenhada é **2×2** — então `↓` ia de 0
    // para 4 (recuando para 3) e `→` ia de 1 para 2, em diagonal. Era a queixa
    // "a navegação da aba Jogos não é livre": o d-pad não seguia a tela.
    //
    // Agora existe **um** número, e ele vem do desenho (`State::grade_*`).
    let grade = match zone {
        Zone::LibraryGrid => state.grade_biblioteca,
        Zone::AppsGrid => state.grade_apps,
        _ => crate::viewport::Grade::new(cols, len.min(cols * 2).max(cols)),
    };
    let cols = grade.cols();
    let por_pagina = grade.por_pagina();
    let rows = por_pagina.div_ceil(cols).max(1);

    // Biblioteca: subir da primeira linha **da página** volta para os filtros.
    //
    // A condição é a linha dentro da página, não `index < cols`: a página 2
    // começa no índice `por_pagina`, e ali o índice é grande embora a linha seja
    // a primeira.
    if zone == Zone::LibraryGrid && dir == Dir::Up && focus.index % por_pagina < cols {
        return Some(Focus::at(Zone::LibraryFilters, 0));
    }

    // Grades de conteúdo andam em laço — ver `Zone::wraps`.
    //
    // Vertical anda `cols` de cada vez, que é exatamente a célula de baixo na
    // grade desenhada. Quando passa do fim da página, o índice cai na primeira
    // linha da página seguinte — e a view deriva a página do índice, então a
    // paginação acompanha sozinha.
    //
    // Horizontal percorre o **índice inteiro**: `→` no último cartão de uma
    // linha continua no primeiro da seguinte, e no fim volta ao começo.
    if zone.wraps() {
        let linha = (focus.index % por_pagina) / cols;

        let mut next = match dir {
            Dir::Left => (focus.index + len - 1) % len,
            Dir::Right => (focus.index + 1) % len,
            Dir::Down => {
                if linha + 1 < rows {
                    focus.index + cols
                } else {
                    // Última linha da página: volta à primeira, mesma coluna.
                    focus.index - linha * cols
                }
            }
            Dir::Up => {
                if linha > 0 {
                    focus.index - cols
                } else {
                    // Primeira linha da página: desce para a última.
                    focus.index + (rows - 1) * cols
                }
            }
        };

        // A última linha pode estar incompleta: a coluna de origem pode não
        // existir nela, e aí o destino é o último item.
        if next >= len {
            next = len - 1;
        }

        return if next == focus.index {
            None
        } else {
            Some(Focus::at(zone, next))
        };
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

        // `←` na coluna 0 agora dá a volta e vai ao último item: a grade anda
        // em laço (ver `Zone::wraps`). Antes este teste afirmava `is_none()`,
        // ou seja, **fixava** o comportamento que o usuário chamou de travado.
        assert_eq!(navigate(&s, Dir::Left).unwrap().index, 7);
        assert!(
            navigate(&s, Dir::Up).is_some(),
            "subir da primeira linha vai aos filtros"
        );
    }

    #[test]
    fn ultima_linha_incompleta_recua_para_item_valido() {
        // 6 jogos em grade de 4: segunda linha tem 2 itens (índices 4 e 5).
        let mut s = state_with(6, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 5);

        // `→` no último item volta ao primeiro — não é "não há para onde ir".
        // Era o que este teste afirmava antes, e é o bug relatado: com os 4
        // jogos do catálogo (uma linha só) o `→` parava no quarto cartão.
        let next = navigate(&s, Dir::Right).expect("→ no último item deve dar a volta");
        assert_eq!(next.index, 0);
    }

    /// A queixa literal — "corrija a navegação livre na aba Jogos" — em teste.
    ///
    /// O catálogo do aparelho tem 4 jogos e a grade tem 4 colunas: uma linha
    /// só. Antes, `→` ia de 0 a 3 e ali morria, e `↓` não fazia nada, porque
    /// `LibraryGrid` não estava em `Zone::wraps`.
    #[test]
    fn grade_de_quatro_jogos_percorre_em_laco() {
        let mut s = state_with(4, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 0);

        let mut indices = vec![0];
        let mut atual = 0;
        for _ in 0..4 {
            s.focus = Focus::at(Zone::LibraryGrid, atual);
            atual = navigate(&s, Dir::Right).expect("→ deve sempre andar").index;
            indices.push(atual);
        }
        assert_eq!(
            indices,
            vec![0, 1, 2, 3, 0],
            "quatro passos à direita devem dar a volta nos quatro jogos"
        );

        // E no sentido oposto.
        s.focus = Focus::at(Zone::LibraryGrid, 0);
        assert_eq!(navigate(&s, Dir::Left).unwrap().index, 3);
    }

    #[test]
    fn direita_no_fim_da_linha_continua_na_linha_de_baixo() {
        // 6 jogos em 4 colunas: 3 é o último da primeira linha, 4 o primeiro
        // da segunda. Antes era preciso `↓` e depois `←` três vezes.
        let mut s = state_with(6, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 3);

        assert_eq!(navigate(&s, Dir::Right).unwrap().index, 4);
        assert_eq!(navigate(&s, Dir::Left).unwrap().index, 2);
    }

    #[test]
    fn descer_da_ultima_linha_volta_ao_topo() {
        let mut s = state_with(6, 0);
        s.tab = crate::state::Tab::Library;
        s.focus = Focus::at(Zone::LibraryGrid, 4); // linha 1, coluna 0

        assert_eq!(navigate(&s, Dir::Down).unwrap().index, 0);
    }

    #[test]
    fn aba_apps_tambem_percorre_em_laco() {
        let mut s = state_with(0, 4);
        s.tab = crate::state::Tab::Apps;
        s.focus = Focus::at(Zone::AppsGrid, 3);

        assert_eq!(navigate(&s, Dir::Right).unwrap().index, 0);
        s.focus = Focus::at(Zone::AppsGrid, 0);
        assert_eq!(navigate(&s, Dir::Left).unwrap().index, 3);
    }

    /// O bug relatado — "corrija a navegação livre na aba Jogos" — em teste.
    ///
    /// No aparelho o catálogo tem 4 jogos e o painel é 16:9, então a grade
    /// **desenhada** é 2×2 (`grade_para(4, 936, 538, 9) == (2, 2)`). A navegação
    /// assumia 4 colunas: `↓` de 0 ia para 4, que não existe, e recuava para 3 —
    /// a célula de baixo é a 2. O destaque pulava em diagonal.
    #[test]
    fn descer_segue_a_grade_desenhada_e_nao_quatro_colunas() {
        let mut s = state_with(4, 0);
        s.tab = crate::state::Tab::Library;
        s.grade_biblioteca = crate::viewport::Grade::new(2, 4);
        s.focus = Focus::at(Zone::LibraryGrid, 0);

        assert_eq!(
            navigate(&s, Dir::Down).unwrap().index,
            2,
            "a célula de baixo do cartão 0 numa grade 2×2 é a 2"
        );
        assert_eq!(navigate(&s, Dir::Right).unwrap().index, 1);
    }

    #[test]
    fn descer_na_ultima_linha_volta_a_primeira() {
        let mut s = state_with(4, 0);
        s.tab = crate::state::Tab::Library;
        s.grade_biblioteca = crate::viewport::Grade::new(2, 4);
        s.focus = Focus::at(Zone::LibraryGrid, 2); // linha 1, coluna 0

        assert_eq!(navigate(&s, Dir::Down).unwrap().index, 0);
        assert_eq!(navigate(&s, Dir::Up).unwrap().index, 0, "sobe para a linha 0");
    }

    /// A saída para os filtros vale na primeira linha **de cada página**, não
    /// só quando o índice é pequeno: a página 2 começa no índice `por_pagina`.
    #[test]
    fn subir_da_primeira_linha_da_segunda_pagina_vai_aos_filtros() {
        let mut s = state_with(16, 0);
        s.tab = crate::state::Tab::Library;
        s.grade_biblioteca = crate::viewport::Grade::new(4, 8);
        s.focus = Focus::at(Zone::LibraryGrid, 9); // página 2, linha 0, coluna 1

        let next = navigate(&s, Dir::Up).unwrap();
        assert_eq!(next.zone, Zone::LibraryFilters);
    }

    /// E o contrário: na segunda linha de uma página, `↑` anda para cima em vez
    /// de pular para os filtros.
    #[test]
    fn subir_da_segunda_linha_anda_para_cima() {
        let mut s = state_with(16, 0);
        s.tab = crate::state::Tab::Library;
        s.grade_biblioteca = crate::viewport::Grade::new(4, 8);
        s.focus = Focus::at(Zone::LibraryGrid, 12); // página 2, linha 1, coluna 0

        assert_eq!(navigate(&s, Dir::Up).unwrap().index, 8);
    }

    /// O teclado da busca **não** entra no laço: `→` na última tecla de uma
    /// linha não deve pular para a primeira da seguinte, porque o teclado é
    /// lido como grade de caracteres, não como lista de itens.
    #[test]
    fn teclado_da_busca_nao_percorre_em_laco() {
        assert!(!Zone::SearchKeyboard.wraps());
        assert!(!Zone::PasswordKeys.wraps());
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

    /// A queixa do usuário, em teste: "a aba de configuração não é navegável".
    ///
    /// Antes, a coluna da esquerda recebia foco e o painel da direita não — só
    /// existia `Zone::SettingsSidebar`. Dava para destacar "Rede e BT" e não
    /// dava para entrar em Wi-Fi. Nenhum teste olhava para isso.
    #[test]
    fn direita_entra_no_painel_de_configuracoes() {
        let mut s = state_with(1, 1);
        s.tab = crate::state::Tab::Settings;
        s.settings_category = 2; // rede
        s.focus = Focus::at(Zone::SettingsSidebar, 2);

        let next = navigate(&s, Dir::Right).expect("tem de entrar no painel");
        assert_eq!(next.zone, Zone::SettingsContent);
        assert_eq!(next.index, 0, "entra pela primeira linha");
    }

    #[test]
    fn esquerda_do_painel_volta_para_a_coluna() {
        let mut s = state_with(1, 1);
        s.tab = crate::state::Tab::Settings;
        s.settings_category = 2;
        s.focus = Focus::at(Zone::SettingsContent, 0);

        let next = navigate(&s, Dir::Left).unwrap();
        assert_eq!(next.zone, Zone::SettingsSidebar);
        assert_eq!(next.index, 2, "volta na categoria em que estava");
    }

    #[test]
    fn painel_de_configuracoes_nao_sai_pelas_bordas() {
        let mut s = state_with(1, 1);
        s.tab = crate::state::Tab::Settings;
        s.settings_category = 2; // duas linhas
        s.focus = Focus::at(Zone::SettingsContent, 0);
        assert!(navigate(&s, Dir::Up).is_none(), "não passa do topo");

        s.focus = Focus::at(Zone::SettingsContent, 1);
        assert!(navigate(&s, Dir::Down).is_none(), "não passa do fim");
    }

    #[test]
    fn categoria_sem_linhas_nao_recebe_foco() {
        let mut s = state_with(1, 1);
        s.tab = crate::state::Tab::Settings;
        s.settings_category = 5; // "sobre": só texto
        s.focus = Focus::at(Zone::SettingsSidebar, 5);
        assert!(
            navigate(&s, Dir::Right).is_none(),
            "entrar num painel vazio prenderia o foco sem nada para onde ir"
        );
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
