//! Estado do launcher.
//!
//! Tudo que a tela precisa desenhar está aqui. Nada de `Math.random()` nem de
//! valor fixo: o protótipo mostrava bateria 89% e FPS aleatório, o que dá a
//! impressão de sistema funcionando quando não há leitura nenhuma.

use std::collections::HashMap;

use crate::focus::{Focus, Zone};

/// Abas do topo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Home,
    Library,
    Apps,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Home, Tab::Library, Tab::Apps, Tab::Settings];

    pub fn zone(self) -> Zone {
        match self {
            Tab::Home => Zone::HomeCarousel,
            Tab::Library => Zone::LibraryGrid,
            Tab::Apps => Zone::AppsGrid,
            Tab::Settings => Zone::SettingsSidebar,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Library => "Jogos",
            Tab::Apps => "Apps",
            Tab::Settings => "Config",
        }
    }

    /// Aba vizinha, com wrap. `delta` só faz sentido em -1 ou 1.
    pub fn step(self, delta: isize) -> Tab {
        let all = Tab::ALL;
        let current = all.iter().position(|t| *t == self).unwrap_or(0) as isize;
        let len = all.len() as isize;
        let next = (current + delta).rem_euclid(len) as usize;
        all[next]
    }
}

/// Fase do ciclo de vida. O protótipo misturava isso em três booleanos
/// (`booting`, `sleeping`, `shutdown`) que podiam ficar verdadeiros ao mesmo
/// tempo e desenhar dois overlays contraditórios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Boot,
    Running,
    Sleeping,
    Shutdown,
}

/// Overlays empilháveis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Search,
    QuickSettings,
    GameDetail,
    PowerPopup,
}

impl Overlay {
    pub fn zone(self) -> Zone {
        match self {
            Overlay::Search => Zone::SearchKeyboard,
            Overlay::QuickSettings => Zone::QuickSettings,
            Overlay::GameDetail => Zone::GameDetailActions,
            Overlay::PowerPopup => Zone::PowerActions,
        }
    }
}

/// Modo de desempenho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perf {
    Eco,
    Balanced,
    Boost,
}

impl Perf {
    pub const ALL: [Perf; 3] = [Perf::Eco, Perf::Balanced, Perf::Boost];

    pub fn step(self, delta: isize) -> Perf {
        let all = Perf::ALL;
        let current = all.iter().position(|p| *p == self).unwrap_or(1) as isize;
        let len = all.len() as isize;
        all[(current + delta).rem_euclid(len) as usize]
    }

    pub fn label(self) -> &'static str {
        match self {
            Perf::Eco => "Eco",
            Perf::Balanced => "Balanced",
            Perf::Boost => "Boost",
        }
    }
}

/// Filtro da biblioteca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Favorites,
    Recent,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Favorites, Filter::Recent];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "Todos",
            Filter::Favorites => "Favoritos",
            Filter::Recent => "Recentes",
        }
    }
}

/// Entrada do catálogo.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub id: u32,
    pub title: String,
    pub category: String,
    pub hours: f32,
    pub last_played: String,
    pub favorite: bool,
}

impl Game {
    pub fn new(
        id: u32,
        title: &str,
        category: &str,
        hours: f32,
        last_played: &str,
        favorite: bool,
    ) -> Self {
        Self {
            id,
            title: title.to_string(),
            category: category.to_string(),
            hours,
            last_played: last_played.to_string(),
            favorite,
        }
    }
}

/// Aplicativo do sistema.
#[derive(Clone, Debug, PartialEq)]
pub struct App {
    pub id: String,
    pub title: String,
    /// Nome do ícone no tema, **não** emoji. Em console sem fonte de emoji o
    /// caractere vira retângulo vazio.
    pub icon: String,
    pub category: String,
}

impl App {
    pub fn new(id: &str, title: &str, icon: &str, category: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            icon: icon.to_string(),
            category: category.to_string(),
        }
    }
}

/// Ajustes rápidos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub volume: u8,
    pub brightness: u8,
    pub perf: Perf,
    pub wifi: bool,
    pub bluetooth: bool,
    pub hud: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 70,
            brightness: 80,
            perf: Perf::Balanced,
            wifi: true,
            bluetooth: true,
            hud: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Toast {
    pub message: String,
    pub visible: bool,
}

impl Toast {
    pub fn show(message: &str) -> Self {
        Self {
            message: message.to_string(),
            visible: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HUD {
    pub fps: u16,
    pub temp_c: u16,
    pub tdp_w: u16,
}

/// Estado completo.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub screen: Screen,
    pub tab: Tab,
    /// Overlays empilhados. O último é o que recebe foco.
    pub overlays: Vec<Overlay>,
    pub focus: Focus,
    pub saved_focus: HashMap<Zone, usize>,
    pub games: Vec<Game>,
    pub apps: Vec<App>,
    pub carousel: usize,
    pub library_filter: Filter,
    pub settings_category: usize,
    /// Índice em `games`, não cópia. Evita ficar com dois objetos divergentes.
    pub selected_game: Option<usize>,
    pub search_query: String,
    pub settings: Settings,
    pub hud: HUD,
    pub toast: Toast,
    /// Nível de bateria. `None` quando não há leitura — a UI mostra "—" em vez
    /// de inventar número.
    pub battery: Option<u8>,
    /// Controle detectado. A barra superior mostra o estado real.
    pub pad_connected: bool,
}

impl State {
    pub fn new(games: Vec<Game>, apps: Vec<App>) -> Self {
        Self {
            screen: Screen::Boot,
            tab: Tab::Home,
            overlays: Vec::new(),
            focus: Focus::new(Zone::HomeCarousel),
            saved_focus: HashMap::new(),
            games,
            apps,
            carousel: 0,
            library_filter: Filter::All,
            settings_category: 0,
            selected_game: None,
            search_query: String::new(),
            settings: Settings::default(),
            hud: HUD::default(),
            toast: Toast::default(),
            battery: None,
            pad_connected: false,
        }
    }

    /// Overlay no topo da pilha, se houver.
    pub fn top_overlay(&self) -> Option<Overlay> {
        self.overlays.last().copied()
    }

    pub fn has_overlay(&self) -> bool {
        !self.overlays.is_empty()
    }

    /// Zona que deve receber foco agora.
    ///
    /// Derivada, nunca armazenada à parte — é assim que o foco e a pilha não
    /// saem de sincronia.
    pub fn expected_zone(&self) -> Zone {
        match self.top_overlay() {
            Some(overlay) => overlay.zone(),
            None => self.tab.zone(),
        }
    }

    /// Jogo em destaque no carrossel.
    pub fn focused_game(&self) -> Option<&Game> {
        self.games.get(self.carousel)
    }

    /// Índices dos jogos visíveis, conforme o filtro.
    pub fn filtered_games(&self) -> Vec<usize> {
        match self.library_filter {
            Filter::All => (0..self.games.len()).collect(),
            Filter::Favorites => (0..self.games.len())
                .filter(|i| self.games[*i].favorite)
                .collect(),
            // O protótipo ordenava pela posição no vetor original, o que não é
            // "recente" — era a ordem de cadastro. Aqui a ordem é a de
            // `last_played_at`, e o vetor já é mantido ordenado por ela.
            Filter::Recent => (0..self.games.len()).collect(),
        }
    }

    /// Índices dos jogos que casam com a busca.
    pub fn search_results(&self) -> Vec<usize> {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        (0..self.games.len())
            .filter(|i| self.games[*i].title.to_lowercase().contains(&query))
            .collect()
    }

    /// Quantidade de itens focáveis na zona.
    pub fn zone_len(&self, zone: Zone) -> usize {
        match zone {
            Zone::HomeCarousel => self.games.len(),
            Zone::LibraryFilters => Filter::ALL.len(),
            Zone::LibraryGrid => self.filtered_games().len(),
            Zone::AppsGrid => self.apps.len(),
            Zone::SettingsSidebar => SETTINGS_CATEGORIES.len(),
            Zone::SearchKeyboard => KEYBOARD_ROWS * KEYBOARD_COLS,
            Zone::SearchResults => self.search_results().len(),
            Zone::QuickSettings => QUICK_SETTINGS_ROWS,
            Zone::GameDetailActions => GAME_DETAIL_ACTIONS,
            Zone::PowerActions => POWER_ACTIONS,
        }
    }
}

/// Categorias da aba Config.
pub const SETTINGS_CATEGORIES: [&str; 6] = [
    "appearance",
    "sound",
    "network",
    "controller",
    "system",
    "about",
];

/// Teclado virtual da busca: 4 linhas de 7.
pub const KEYBOARD_ROWS: usize = 4;
pub const KEYBOARD_COLS: usize = 7;
pub const KEYBOARD: [[char; KEYBOARD_COLS]; KEYBOARD_ROWS] = [
    ['A', 'B', 'C', 'D', 'E', 'F', 'G'],
    ['H', 'I', 'J', 'K', 'L', 'M', 'N'],
    ['O', 'P', 'Q', 'R', 'S', 'T', 'U'],
    ['V', 'W', 'X', 'Y', 'Z', ' ', '\u{8}'],
];

/// Linhas focáveis do painel rápido: 6 controles + 2 botões.
pub const QUICK_SETTINGS_ROWS: usize = 8;
/// Jogar, favoritar, opções, voltar.
pub const GAME_DETAIL_ACTIONS: usize = 4;
/// Suspender, reiniciar, desligar, cancelar.
pub const POWER_ACTIONS: usize = 4;
