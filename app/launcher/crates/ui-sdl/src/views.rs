//! Desenho das telas e coleta de áreas tocáveis.
//!
//! Cada função desenha e, quando um elemento é interativo, registra seu
//! retângulo num [`Layout`]. É assim que o toque funciona sem duplicar
//! geometria: quem desenha sabe onde o elemento está, então é quem registra.
//! Separar "desenhar" de "calcular posições" seria a receita para os dois
//! saírem de sincronia.

use giochi_launcher_core::idioma::{self, Chave, Idioma};
use giochi_launcher_core::state::{
    settings_ajustavel, Filter, Overlay, Tab, KEYBOARD, KEYBOARD_COLS, QUICK_SETTINGS_ROWS,
    SETTINGS_CATEGORIES,
};
use giochi_launcher_core::State;
use giochi_launcher_core::Zone;
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

/// Reexportado do núcleo: existe **uma** implementação.
///
/// Estava aqui, e o núcleo tinha outra ideia do arranjo (4 colunas fixas) — o
/// desenho escolhia 2×2 e a navegação andava como se fossem 4. Ver
/// [`giochi_launcher_core::Grade`].
pub use giochi_launcher_core::grade_para;

/// Ação disparada por um toque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Tab(Tab),
    Filter(Filter),
    Carousel(usize),
    Game(usize),
    App(usize),
    SettingsCat(usize),
    /// Linha do painel de configurações.
    SettingsRow(usize),
    /// Rede Wi-Fi na lista (o último índice é "procurar").
    WifiNet(usize),
    /// Tecla do teclado de senha.
    PasswordKey(usize),
    /// Botão "Conectar" do teclado de senha.
    PasswordConfirm,
    /// Idioma na lista de boas-vindas.
    Idioma(usize),
    /// Dispositivo Bluetooth na lista.
    BtDevice(usize),
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
    /// Arranjo da grade de jogos deste quadro.
    ///
    /// O desenho é quem sabe quantas colunas cabem — depende de geometria e de
    /// métrica de fonte. A navegação precisa do mesmo número para o d-pad
    /// seguir a grade que está na tela, então o valor é publicado aqui e o
    /// `App` o copia para o estado. Sem isso havia duas verdades: 4 colunas na
    /// navegação e 2×2 no desenho.
    pub grade_biblioteca: Option<giochi_launcher_core::Grade>,
    /// Idem para a aba Apps.
    pub grade_apps: Option<giochi_launcher_core::Grade>,
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

/// Texto de uma chave no idioma escolhido.
///
/// Três letras em vez de `idioma::texto(state.settings.idioma, …)` em cada uma
/// das ~90 chamadas de desenho: a linha de desenho fica legível, e trocar o
/// tipo de retorno da tradução (para `Cow`, por exemplo) não vira uma edição em
/// cada tela.
fn t(state: &State, chave: Chave) -> &'static str {
    idioma::texto(state.settings.idioma, chave)
}

/// Idem, com um valor no lugar de `{}` (SSID, título do app, contagem).
fn tv(state: &State, chave: Chave, valor: &str) -> String {
    idioma::texto_com(state.settings.idioma, chave, valor)
}

// ════════════════════════════════════════════════════════════
// BARRA SUPERIOR
// ════════════════════════════════════════════════════════════
pub fn chrome_top(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    p.fill_rect(m.safe_x, m.top - 1, m.safe_w, 1, theme::line());
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
        let label = tab.label(state.settings.idioma);
        let text_w = p.text_size(label, theme::F_SMALL).0;
        let w = if com_icone { text_w + 34 + 22 } else { text_w + 24 };
        let active = tab == state.tab;

        if active {
            p.fill_round_rect(x, y0 + 10, w, 36, theme::RADIUS_CHIP, theme::card());
            p.stroke_round_rect(x, y0 + 10, w, 36, theme::RADIUS_CHIP, theme::line2());
        }
        let color = if active { theme::text() } else { theme::dim() };
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
    p.fill_round_rect(rx, y0 + 11, power_w, 34, 6, theme::with_alpha(theme::danger(), 30));
    p.stroke_round_rect(rx, y0 + 11, power_w, 34, 6, theme::with_alpha(theme::danger(), 90));
    p.icon("power", rx + power_w / 2, y0 + 28, 16, theme::danger());
    layout
        .hits
        .push((Rect::new(rx, y0 + 11, power_w as u32, 34), Hit::PowerTap));
    rx -= 8;

    // Relógio.
    let clock = current_clock();
    let clock_w = p.text_size(&clock, theme::F_BODY).0;
    if cabe(rx, clock_w) {
        rx -= clock_w;
        p.text_right(rx + clock_w, y0 + 22, &clock, theme::F_BODY, theme::text());
        rx -= 10;
    }

    // Bateria.
    let battery = state.battery.map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
    if cabe(rx, p.text_size(&battery, theme::F_TINY).0 + 22) {
        rx = badge(p, layout, rx, &battery, theme::text(), None);
        rx -= 6;
    }

    // Wi-Fi e controle: indicadores de estado, os primeiros a sair quando a
    // barra está apertada — a mesma informação está em Ajustes.
    if !m.compacto {
        let wifi_on = state.settings.wifi;
        if cabe(rx, p.text_size("Wi-Fi", theme::F_TINY).0 + 34) {
            rx = badge(p, layout, rx, "Wi-Fi", if wifi_on { theme::ok() } else { theme::dim() }, Some("wifi"));
            rx -= 6;
        }

        let pad_label = if state.pad_connected { "ON" } else { "OFF" };
        let pad_color = if state.pad_connected { theme::ok() } else { theme::dim() };
        if cabe(rx, p.text_size(pad_label, theme::F_TINY).0 + 34) {
            rx = badge(p, layout, rx, pad_label, pad_color, Some("gamepad"));
            rx -= 6;
        }
    }

    // Ajustes rápidos.
    if cabe(rx, 34) {
        rx -= 34;
        p.fill_round_rect(rx, y0 + 11, 34, 34, 6, theme::panel2());
        p.icon("settings", rx + 17, y0 + 28, 16, theme::dim());
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
    p.stroke_round_rect(x, 14, w, 28, 14, theme::line());
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
    // ## `GIOCHI_HORA_FIXA` existe para a comparação de quadros
    //
    // O relógio é lido do sistema, e o aparelho pode estar com a hora errada —
    // sem NTP, ele fica horas atrás do PC. Medido em 09/11/2026 na imagem nova
    // do X35H: aparelho `21:58 UTC` contra PC `03:44 UTC`. A comparação
    // pixel a pixel então acusava **99,9 %** em todas as telas, com a diferença
    // inteira em x 482..517, y 26..34 — os quatro dígitos do relógio. Nenhuma
    // regressão de desenho, e um portão que reprova sempre ensina a ignorá-lo.
    //
    // Repetir a captura não resolve (o minuto não vira em 1 s), e acertar o
    // relógio do aparelho seria mudar estado do sistema para fazer um teste
    // passar. Congelar nos **dois** lados é determinístico.
    if let Ok(fixa) = std::env::var("GIOCHI_HORA_FIXA") {
        if !fixa.is_empty() {
            return fixa;
        }
    }
    // Sem dependência de data/hora: o horário real vem do sistema via `chrono`
    // ou de `/proc/driver/rtc` na integração. Aqui fica explícito que é
    // provisório em vez de fingir um relógio.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    formatar_relogio(secs)
}

/// `HH:MM` a partir de segundos Unix. Puro, para poder ser testado sem relógio.
fn formatar_relogio(secs: u64) -> String {
    let minutes = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    format!("{hours:02}:{minutes:02}")
}

// ════════════════════════════════════════════════════════════
// RODAPÉ
// ════════════════════════════════════════════════════════════
/// Dicas do rodapé para o estado atual.
///
/// ## Por que é função, e não um `match` no meio do desenho
///
/// O rodapé já mentiu duas vezes: dizia `A — Jogar` numa aba onde `A` não
/// iniciava nada, e `Jogar` na Biblioteca, onde `A` abre a ficha. Um `match`
/// enterrado em 40 linhas de desenho não tem como ser testado, então a mentira
/// só aparece quando alguém olha a tela e desconfia.
///
/// Aqui a lista é dado: o teste pergunta "a Biblioteca anuncia L2/R2?" e
/// reprova se a resposta mudar sem que a tecla mude junto.
pub fn dicas(state: &State) -> Vec<(&'static str, &'static str)> {
    let i = state.settings.idioma;
    let t = |c: Chave| idioma::texto(i, c);
    match state.top_overlay() {
        Some(Overlay::Search) => vec![
            ("↑↓←→", t(Chave::HintNavigate)),
            ("A", t(Chave::HintSelect)),
            ("B", t(Chave::HintBack)),
        ],
        Some(Overlay::QuickSettings) => vec![
            ("↑↓", t(Chave::HintNavigate)),
            ("←→", t(Chave::HintAdjust)),
            ("B", t(Chave::HintClose)),
        ],
        Some(Overlay::GameDetail) => vec![
            ("←→", t(Chave::HintAction)),
            ("A", t(Chave::HintConfirm)),
            ("B", t(Chave::HintBack)),
        ],
        Some(Overlay::PowerPopup) => vec![
            ("↑↓", t(Chave::HintNavigate)),
            ("A", t(Chave::HintConfirm)),
            ("B", t(Chave::HintCancel)),
        ],
        Some(Overlay::WifiList) => vec![
            ("A", t(Chave::HintConnect)),
            ("B", t(Chave::HintBack)),
            ("↑↓", t(Chave::HintNetwork)),
        ],
        // Boas-vindas: as duas saídas ficam escritas. `B` **não** é "voltar"
        // aqui (não há para onde voltar) — é "ficar em inglês", e o rótulo diz
        // isso em vez de prometer uma volta que não existe.
        Some(Overlay::BoasVindas) => vec![
            ("↑↓", t(Chave::HintChoose)),
            ("A", t(Chave::HintConfirm)),
            ("B", t(Chave::WelcomeEnglishDefault)),
        ],
        // `↓ Conectar` entrou em 14/11/2026: a tela não tinha confirmação e a
        // senha digitada nunca entrava em vigor. O rodapé é o único lugar onde
        // o jogador descobre que existe uma saída.
        Some(Overlay::WifiPassword) => vec![
            ("A", t(Chave::HintType)),
            ("X", t(Chave::HintShow)),
            ("↓", t(Chave::HintConnect)),
            ("B", t(Chave::HintCancel)),
        ],
        Some(Overlay::BtList) => vec![
            ("A", t(Chave::HintPair)),
            ("B", t(Chave::HintBack)),
            ("↑↓", t(Chave::HintDevice)),
        ],
        // Configurações: as dicas mudam com a coluna em foco.
        //
        // Antes havia uma dica só para a aba inteira, e ela não dizia que ← →
        // atravessa para o painel da direita nem que A abre Wi-Fi. O usuário
        // concluiu — com razão — que a aba não era navegável.
        None if state.tab == Tab::Settings && state.focus.zone == Zone::SettingsSidebar => vec![
            ("→", t(Chave::HintEnter)),
            ("L2/R2", t(Chave::HintCategory)),
            ("LB/RB", t(Chave::HintTab)),
            ("Select", t(Chave::HintQuick)),
        ],
        // Linha com valor ajustável: as setas ajustam, e a saída é o `B`.
        // Sem isto o rodapé prometia "← Categorias" numa linha onde ← ajustava.
        None if state.tab == Tab::Settings
            && state.focus.zone == Zone::SettingsContent
            && settings_ajustavel(state.settings_category, state.focus.index) =>
        {
            vec![
                ("←→", t(Chave::HintAdjust)),
                ("L2/R2", t(Chave::HintCategory)),
                ("B", t(Chave::HintCategories)),
            ]
        }
        None if state.tab == Tab::Settings => vec![
            ("A", t(Chave::HintOpen)),
            ("L2/R2", t(Chave::HintCategory)),
            ("←", t(Chave::HintCategories)),
            ("B", t(Chave::HintBack)),
        ],
        // O rodapé era um só para Home, Biblioteca e Apps — e as três fazem
        // coisas diferentes com o `A`: na Home ele abre o jogo, na Biblioteca
        // abre a ficha, nos Apps não abre nada. Dizer "Jogar" na Biblioteca era
        // a mesma mentira que o aviso "Iniciando..." sem iniciar: a tela
        // promete um efeito que a tecla não tem.
        None if state.tab == Tab::Home => vec![
            ("A", t(Chave::HintPlay)),
            ("X", t(Chave::HintDetails)),
            ("Y", t(Chave::HintSearch)),
            ("LB/RB", t(Chave::HintTab)),
            ("Select", t(Chave::HintQuick)),
        ],
        // Biblioteca: os gatilhos trocam o filtro. A dica aparece porque o
        // atalho não se descobre sozinho — a faixa de filtros também responde a
        // ← →, então quem não sabe do L2/R2 nunca tenta.
        None if state.tab == Tab::Library => vec![
            ("A", t(Chave::HintOpen)),
            ("X", t(Chave::HintDetails)),
            ("Y", t(Chave::HintSearch)),
            ("L2/R2", t(Chave::HintFilter)),
            ("LB/RB", t(Chave::HintTab)),
        ],
        // A grade de apps é de demonstração: os apps não estão no catálogo do
        // sistema, então não há entrypoint para executar. O rodapé diz o que a
        // tecla faz hoje, não o que faria com uma loja instalada.
        None if state.tab == Tab::Apps => vec![
            ("X", t(Chave::HintDetails)),
            ("LB/RB", t(Chave::HintTab)),
        ],
        None => vec![("LB/RB", t(Chave::HintTab))],
    }
}

pub fn chrome_foot(p: &mut Painter, state: &State) {
    let m = Moldura::de(p);
    p.fill_rect(m.safe_x, m.foot_y, m.safe_w, 1, theme::line());

    let hints = dicas(state);

    // Centralizado na faixa do rodapé. Antes era `foot_y + 18` fixo: num painel
    // de 480 px o chip de 16 px começava em y=468 e era cortado pela borda.
    let h_txt = p.text_size("Ay", theme::F_TINY).1;
    let chip_h = h_txt + 6;
    let faixa = (m.h - m.foot_y).max(chip_h);
    let chip_y = m.foot_y + (faixa - chip_h) / 2;
    let y = chip_y + (chip_h - h_txt) / 2;

    // A assinatura de versão sai quando não há largura: ela é a informação mais
    // dispensável do rodapé, e sobrepor as dicas seria pior.
    let build = format!(
        "giochi-launcher 0.1.0 • {}",
        state.settings.perf.label(state.settings.idioma)
    );
    let build_w = p.text_size(&build, theme::F_TINY).0;

    let mut x = m.safe_x + 4;
    let limite = m.safe_x + m.safe_w - 4 - build_w - 16;
    for (key, label) in &hints {
        let key_w = p.text_size(key, theme::F_TINY).0 + 10;
        let label_w = p.text_size(label, theme::F_TINY).0;
        if x + key_w + 5 + label_w > limite {
            break;
        }
        p.fill_round_rect(x, chip_y, key_w, chip_h, 3, theme::with_alpha(theme::WHITE, 20));
        let ky = vcenter(chip_y, chip_h, p.text_size(key, theme::F_TINY).1);
        p.text_left(x + 5, ky, key, theme::F_TINY, theme::accent());
        x += key_w + 5;
        p.text_left(x, y, label, theme::F_TINY, theme::dim());
        x += label_w + 12;
    }

    p.text_right(m.safe_x + m.safe_w - 4, y, &build, theme::F_TINY, theme::faint());
}

// ════════════════════════════════════════════════════════════
// HOME — carrossel
// ════════════════════════════════════════════════════════════
pub fn home(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let n = state.games.len();
    if n == 0 {
        // Duas linhas em vez de uma: "Catálogo vazio" descreve o sintoma, e
        // quem está com o aparelho na mão precisa saber a causa. Aconteceu
        // numa imagem nova, sem `index.json`, e a tela não dizia o que fazer.
        let cx = m.safe_x + m.safe_w / 2;
        let cy = m.top + m.main_h / 2;
        p.text_center(
            cx,
            cy - m.e(10),
            t(state, Chave::HomeEmptyTitle),
            theme::F_SMALL,
            theme::text(),
        );
        p.text_center(
            cx,
            cy + m.e(10),
            t(state, Chave::HomeEmptyHint),
            theme::F_TINY,
            theme::faint(),
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
    let jogar = t(state, Chave::HomePlay);
    let btn_w = (p.text_size(jogar, f_meta).0 + 46).min(card_w - pad * 2);
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

    let meta = format!(
        "{} • {}",
        game.year_label(),
        game.play_label(state.settings.idioma)
    );
    let meta = truncate(&meta, avail, &mut |s| p.text_size(s, f_meta).0);
    p.text_left(x + pad, meta_y, &meta, f_meta, theme::with_alpha(theme::WHITE, 170));

    let bx = x + pad;
    p.fill_round_rect(bx, by, btn_w, btn_h, m.e(8).max(3), theme::with_alpha(theme::WHITE, 40));
    p.stroke_round_rect(bx, by, btn_w, btn_h, m.e(8).max(3), theme::with_alpha(theme::WHITE, 80));
    let icone = (h_meta as f32 * 1.1).round() as i32;
    p.icon("play", bx + m.e(20).max(10), by + btn_h / 2, icone, theme::WHITE);
    let ty = vcenter(by, btn_h, h_meta);
    p.text_left(bx + m.e(34).max(22), ty, jogar, f_meta, theme::WHITE);

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
            p.fill_round_rect(dx, dot_y, ativo, m.e(6).max(3), m.e(3).max(1), theme::accent());
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
        p.text_center(m.safe_x + m.safe_w / 2, legenda_y, &line, theme::F_SMALL, theme::dim());
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
    p.text_left(
        m.safe_x + 4,
        m.top + 12,
        t(state, Chave::LibraryTitle),
        theme::F_TINY,
        theme::dim(),
    );

    let mut fx = m.safe_x + m.safe_w - 4;
    for filter in Filter::ALL.iter().rev() {
        let label = filter.label(state.settings.idioma);
        let w = p.text_size(label, theme::F_TINY).0 + 24;
        fx -= w;
        let active = state.library_filter == *filter;
        let focused = state.focus.zone == Zone::LibraryFilters
            && Filter::ALL.iter().position(|f| f == filter) == Some(state.focus.index);

        if active {
            p.fill_round_rect(fx, m.top + 6, w, 30, 15, theme::card());
            p.stroke_round_rect(fx, m.top + 6, w, 30, 15, theme::line2());
        }
        if focused {
            p.stroke_round_rect(fx, m.top + 6, w, 30, 15, theme::accent());
        }
        let color = if active { theme::text() } else { theme::dim() };
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
    let alvo = games.len().clamp(1, LIB_PAGE);
    let grade = giochi_launcher_core::grade_de(alvo, m.safe_w - m.pad * 2, grid_h, gap);
    layout.grade_biblioteca = Some(grade);
    let (cols, rows) = grade_para(alvo, m.safe_w - m.pad * 2, grid_h, gap);
    let por_pagina = grade.por_pagina();
    let page = focused_index.map(|i| i / por_pagina).unwrap_or(0);
    let pages = games.len().div_ceil(por_pagina).max(1);
    let cell_w = (m.safe_w - m.pad * 2 - gap * (cols - 1)) / cols;
    let cell_h = (grid_h - gap * (rows - 1)) / rows;

    if games.is_empty() {
        p.text_center(
            m.safe_x + m.safe_w / 2,
            grid_top + grid_h / 2,
            t(state, Chave::LibraryEmpty),
            theme::F_SMALL,
            theme::faint(),
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
            p.icon("star", x + cell_w - m.e(16).max(10), y + m.e(16).max(8), m.e(12).max(8), theme::warn());
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
    p.text_center(m.safe_x + m.safe_w / 2, m.foot_y - 20, &pager, theme::F_TINY, theme::dim());
}

// ════════════════════════════════════════════════════════════
// APPS
// ════════════════════════════════════════════════════════════
pub fn apps(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);

    if state.apps.is_empty() {
        // Antes a lista de apps era escrita no código (`demo_apps()`), então
        // esta tela era inalcançável. Com a detecção ela é o caso normal de um
        // aparelho recém-gravado — e dizer *onde* o launcher procurou é o que
        // transforma "não tem nada" em diagnóstico.
        layout.grade_apps = None;
        let cx = m.safe_x + m.safe_w / 2;
        let cy = m.top + m.main_h / 2;
        p.text_center(
            cx,
            cy - m.e(10),
            t(state, Chave::AppsEmptyTitle),
            theme::F_SMALL,
            theme::text(),
        );
        p.text_center(
            cx,
            cy + m.e(10),
            t(state, Chave::AppsEmptyHint),
            theme::F_TINY,
            theme::faint(),
        );
        return;
    }

    let gap = m.e(9).max(4);
    // Mesma conta do desenho, publicada para a navegação — ver `Layout`.
    let itens_apps = state.apps.len().max(1);
    layout.grade_apps = Some(giochi_launcher_core::grade_de(
        itens_apps,
        m.safe_w - m.pad * 2,
        m.main_h - m.pad * 2,
        gap,
    ));
    let (cols, rows) = grade_para(itens_apps, m.safe_w - m.pad * 2, m.main_h - m.pad * 2, gap);
    // Teto do ladrilho.
    //
    // A lista era fixa em oito (`demo_apps()`), então a grade nunca tinha menos
    // de oito itens e o ladrilho nunca foi esticado. Com a detecção, um aparelho
    // com **um** app produziria um único quadrado do tamanho da tela — que lê
    // como interface quebrada, não como grade. O teto não muda `cols`/`rows`
    // publicados, então a navegação continua seguindo a grade desenhada.
    let lado_max = m.e(150).clamp(56, 220);
    let cell_w = ((m.safe_w - m.pad * 2 - gap * (cols - 1)) / cols).min(lado_max);
    let cell_h = ((m.main_h - m.pad * 2 - gap * (rows - 1)) / rows).min(lado_max);
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

        p.fill_round_rect(x, y, cell_w, cell_h, r_tile, theme::panel2());
        let focused = state.focus.zone == Zone::AppsGrid && state.focus.index == i;
        p.stroke_round_rect(
            x,
            y,
            cell_w,
            cell_h,
            r_tile,
            if focused { theme::accent() } else { theme::line() },
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
            theme::with_alpha(theme::accent(), 40),
        );
        let f_mono = (mono as f32 * 0.5) as u16;
        let ty_mono = y + interno + (mono - p.text_size("A", f_mono).1) / 2;
        p.text_center(x + interno + mono / 2, ty_mono, &letter, f_mono, theme::accent());

        // Texto ancorado no rodapé, como na grade de jogos.
        let cat_y = y + cell_h - interno - h_tiny;
        let title_y = cat_y - m.e(2).max(1) - h_body;
        if title_y > y + interno + mono + 2 {
            let title = truncate(&app.title, cell_w - interno * 2, &mut |s| p.text_size(s, theme::F_BODY).0);
            p.text_left(x + interno, title_y, &title, theme::F_BODY, theme::text());
            let cat = truncate(&app.category, cell_w - interno * 2, &mut |s| p.text_size(s, theme::F_TINY).0);
            p.text_left(x + interno, cat_y, &cat, theme::F_TINY, theme::dim());
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
    p.fill_rect(side_x + side_w, m.top + 6, 1, m.main_h - 16, theme::line());

    let mut y = m.top + 12;
    for (i, name) in SETTINGS_CATEGORIES.iter().enumerate() {
        let active = state.settings_category == i;
        let focused = state.focus.zone == Zone::SettingsSidebar && state.focus.index == i;
        let row_h = theme::TOUCH;

        let chip_w = side_w - m.e(12).max(6);
        if active {
            p.fill_round_rect(side_x, y, chip_w, row_h, r_chip, theme::card());
            p.stroke_round_rect(side_x, y, chip_w, row_h, r_chip, theme::line2());
        }
        if focused {
            p.stroke_round_rect(side_x, y, chip_w, row_h, r_chip, theme::accent());
        }

        let color = if active { theme::accent() } else { theme::dim() };
        let ty = vcenter(y, row_h, p.text_size(name, theme::F_SMALL).1);
        let label = label_for_category(name, state.settings.idioma);
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
    let heading = label_for_category(category, state.settings.idioma);
    p.text_left(content_x, m.top + 12, &heading, theme::F_BODY, theme::text());

    let rows = linhas_da_categoria(category, state);

    if category == "about" {
        // Cada linha vem da tabela de traduções. A linha vazia separa os dois
        // parágrafos e não é um texto a traduzir, por isso ela fica fora.
        let lines = [
            t(state, Chave::AboutLine1),
            "",
            t(state, Chave::AboutLine2),
            t(state, Chave::AboutLine3),
            "",
            t(state, Chave::AboutLine4),
            t(state, Chave::AboutLine5),
        ];
        let mut ay = m.top + m.e(40).max(26);
        let passo = p.text_size("Ay", theme::F_SMALL).1 + m.e(7).max(3);
        for line in lines {
            if ay + passo > m.foot_y {
                break;
            }
            let text = truncate(line, content_w, &mut |s| p.text_size(s, theme::F_SMALL).0);
            p.text_left(content_x, ay, &text, theme::F_SMALL, theme::dim());
            ay += passo;
        }
    } else {
        let mut ry = m.top + m.e(40).max(26);
        let borda = m.e(12).max(6);
        for (i, (title, value)) in rows.iter().enumerate() {
            if ry + theme::TOUCH > m.foot_y {
                break;
            }
            let focado = state.focus.zone == Zone::SettingsContent && state.focus.index == i;
            p.fill_round_rect(content_x, ry, content_w, theme::TOUCH, r_caixa, theme::with_alpha(theme::BLACK, 70));
            p.stroke_round_rect(
                content_x,
                ry,
                content_w,
                theme::TOUCH,
                r_caixa,
                if focado { theme::accent() } else { theme::line() },
            );
            layout
                .hits
                .push((Rect::new(content_x, ry, content_w as u32, theme::TOUCH as u32), Hit::SettingsRow(i)));
            // Título e valor no mesmo y: em 640×480 a coluna tem ~320 px, então
            // o título é truncado pelo espaço que sobra depois do valor. Sem
            // isso os dois se desenham por cima.
            let valor = truncate(value, (content_w / 2).max(40), &mut |s| p.text_size(s, theme::F_SMALL).0);
            let valor_w = p.text_size(&valor, theme::F_SMALL).0;
            let titulo = truncate(title, content_w - valor_w - borda * 3, &mut |s| p.text_size(s, theme::F_SMALL).0);
            let ty = vcenter(ry, theme::TOUCH, p.text_size(title, theme::F_SMALL).1);
            p.text_left(content_x + borda, ty, &titulo, theme::F_SMALL, theme::text());
            let vy = vcenter(ry, theme::TOUCH, p.text_size(value, theme::F_SMALL).1);
            p.text_right(content_x + content_w - borda, vy, &valor, theme::F_SMALL, theme::accent());
            ry += theme::TOUCH + m.e(7).max(3);
        }
    }
}

/// Linhas do painel de conteúdo de uma categoria.
///
/// Fica em função própria porque o **núcleo também precisa saber o número**:
/// `settings_rows()` decide até onde o foco pode andar. Antes ele era um `match`
/// separado, e dois lugares descrevendo a mesma lista é como o carrossel ficou
/// dessincronizado do foco. Aqui há um teste que compara os dois.
fn linhas_da_categoria(category: &str, state: &State) -> Vec<(String, String)> {
    match category {
        // Estes dois diziam o que **não é**:
        //
        // - "Tema Giochi Dark" sugeria uma lista de temas que não existe: a
        //   paleta é constante em `theme.rs`, uma só. O usuário apertou → para
        //   trocar e nada aconteceu, porque nada havia para trocar.
        // - "Letterbox centrado, sem corte" descrevia o layout anterior. Desde
        //   `draw::projecao` não há letterbox: a área de projeto **é** o painel
        //   (0,00 % de preto medido nos dois aparelhos).
        //
        // Substituto de dado ausente precisa ser declarado ausente, não
        // preenchido com o que era verdade antes — a mesma regra do
        // `year_label()`, que formatava o id do cartão.
        //
        // O idioma entrou **nesta** lista, e não numa categoria própria, porque
        // é onde a pessoa procura: a mesma tela que se abre para mudar o visual.
        // É o pedido do dono — "lá onde troca o tema terá a opção de trocar o
        // idioma".
        "appearance" => vec![
            (
                t(state, Chave::SetTheme).into(),
                state.settings.tema.rotulo().into(),
            ),
            (
                t(state, Chave::SetLanguage).into(),
                state.settings.idioma.nome_proprio().into(),
            ),
            (
                t(state, Chave::SetScale).into(),
                t(state, Chave::SetScaleValue).into(),
            ),
        ],
        "sound" => vec![
            (
                t(state, Chave::SetVolume).into(),
                format!("{}%", state.settings.volume),
            ),
            (
                t(state, Chave::SetOutput).into(),
                t(state, Chave::SetOutputValue).into(),
            ),
        ],
        // O valor vem do que o sistema **respondeu**, não de um booleano da UI.
        //
        // Antes as duas linhas mostravam "Ligado/Desligado" de um campo que
        // ninguém alternava: a informação era sempre a mesma e não dizia se
        // havia conexão. Agora mostra o SSID conectado ou "não conectado".
        // Três estados, não dois: **não tem rádio** é diferente de **não
        // conectado**. O R35S não tem Wi-Fi nem Bluetooth (não existe nó de
        // rádio nos DTBs do r33s/r35s/r36s); ali "não conectado" sugeriria que
        // dá para conectar. Mesma regra do "Tema Giochi Dark": capacidade
        // ausente tem de ser declarada ausente.
        "network" => vec![
            (
                t(state, Chave::Wifi).into(),
                if state.sem_fio.tem_wifi == Some(false) {
                    t(state, Chave::SetNoRadio).into()
                } else {
                    match &state.sem_fio.conectado {
                        Some(ssid) => ssid.clone(),
                        None => t(state, Chave::SetNotConnected).into(),
                    }
                },
            ),
            (
                t(state, Chave::Bluetooth).into(),
                if state.sem_fio.tem_bt == Some(false) {
                    t(state, Chave::SetNoRadio).into()
                } else {
                    match &state.sem_fio.bt_conectado {
                        Some(mac) => state
                            .sem_fio
                            .dispositivos
                            .iter()
                            .find(|d| &d.mac == mac)
                            .map(|d| d.nome.clone())
                            .unwrap_or_else(|| mac.clone()),
                        None => t(state, Chave::SetNotConnected).into(),
                    }
                },
            ),
        ],
        // O nome do botão ("A / Cross") **não** é traduzido: é o que está
        // gravado no plástico do controle. O que ele faz é.
        "controller" => vec![
            ("A / Cross".into(), t(state, Chave::CtrlConfirm).into()),
            ("B / Circle".into(), t(state, Chave::CtrlBack).into()),
            ("X / Square".into(), t(state, Chave::CtrlDetails).into()),
            ("Y / Triangle".into(), t(state, Chave::CtrlSearch).into()),
            ("LB / RB".into(), t(state, Chave::CtrlSwitchTab).into()),
            ("Select".into(), t(state, Chave::CtrlQuick).into()),
            ("Start".into(), t(state, Chave::CtrlPower).into()),
        ],
        "system" => vec![
            (t(state, Chave::SetVersion).into(), "launcher 0.1.0".into()),
            (
                t(state, Chave::SetPerformance).into(),
                state.settings.perf.label(state.settings.idioma).into(),
            ),
        ],
        _ => Vec::new(),
    }
}

fn label_for_category(name: &str, idioma: Idioma) -> String {
    let chave = match name {
        "appearance" => Chave::SetAppearance,
        "sound" => Chave::SetSound,
        "network" => Chave::SetNetwork,
        "controller" => Chave::SetController,
        "system" => Chave::SetSystem,
        _ => Chave::SetAbout,
    };
    idioma::texto(idioma, chave).to_string()
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
    p.text_center(m.w / 2, m.h / 2, texto, theme::F_BODY, theme::dim());
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
pub fn aviso_sistema(p: &mut Painter, mensagem: &str, idioma: Idioma) {
    let m = Moldura::de(p);
    // Largura e posição vinham de 1280×720 fixos: num painel de 640 px o aviso
    // nascia fora da tela.
    let w = (m.w - m.safe_x).min(m.e(760).max(360));
    let x = m.centro(w);
    let h = m.e(120).max(84);
    let y = m.h - h - m.e(80).max(16);
    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::panel2());
    p.stroke_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::danger());
    let cx = m.w / 2;
    p.text_center(
        cx,
        y + m.e(22).max(10),
        idioma::texto(idioma, Chave::PowerFailTitle),
        theme::F_BODY,
        theme::danger(),
    );
    let msg = truncate(mensagem, w - m.e(40).max(16), &mut |s| p.text_size(s, theme::F_TINY).0);
    p.text_center(cx, y + m.e(56).max(30), &msg, theme::F_TINY, theme::text());
    p.text_center(
        cx,
        y + m.e(84).max(48),
        idioma::texto(idioma, Chave::PowerFailHold),
        theme::F_TINY,
        theme::dim(),
    );
    p.text_center(
        cx,
        y + m.e(100).max(60),
        idioma::texto(idioma, Chave::PowerFailOk),
        theme::F_TINY,
        theme::dim(),
    );
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
            Overlay::WifiList => wifi_list(p, state, layout),
            Overlay::WifiPassword => wifi_password(p, state, layout),
            Overlay::BtList => bt_list(p, state, layout),
            Overlay::BoasVindas => boas_vindas(p, state, layout),
        }
    }

    match state.screen {
        giochi_launcher_core::Screen::Sleeping => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            let lua = m.e(72).max(40);
            p.icon("moon", cx, m.h / 2 - m.e(28).max(12), lua, theme::with_alpha(theme::accent(), 130));
            let aviso = t(state, Chave::ScreenSleep);
            let aviso = truncate(aviso, m.safe_w, &mut |s| p.text_size(s, theme::F_SMALL).0);
            p.text_center(cx, m.h / 2 + m.e(20).max(10), &aviso, theme::F_SMALL, theme::dim());
        }
        giochi_launcher_core::Screen::Shutdown => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            p.text_center(
                cx,
                m.h / 2 - m.e(12).max(6),
                t(state, Chave::ScreenOff),
                theme::F_BODY,
                theme::faint(),
            );
            p.text_center(
                cx,
                m.h / 2 + m.e(14).max(8),
                t(state, Chave::ScreenOffRestart),
                theme::F_SMALL,
                theme::faint(),
            );
        }
        giochi_launcher_core::Screen::Reboot => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            p.text_center(
                m.w / 2,
                m.h / 2,
                t(state, Chave::ScreenReboot),
                theme::F_BODY,
                theme::dim(),
            );
        }
        giochi_launcher_core::Screen::Boot => {
            p.fill_rect(0, 0, m.w, m.h, theme::BLACK);
            let cx = m.w / 2;
            p.text_center(cx, m.h / 2 - m.e(42).max(24), "GIOCHI", theme::F_LOGO, theme::accent());
            p.text_center(cx, m.h / 2 + m.e(2).max(1), "launcher 0.1.0", theme::F_TINY, theme::faint());
        }
        giochi_launcher_core::Screen::Running => {}
    }
}

fn panel(p: &mut Painter, x: i32, y: i32, w: i32, h: i32) {
    p.fill_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::panel());
    p.stroke_round_rect(x, y, w, h, theme::RADIUS_PANEL, theme::line2());
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
    p.fill_round_rect(x, y, w, h, r_painel, theme::panel());
    p.stroke_round_rect(x, y, w, h, r_painel, theme::line2());

    p.text_left(
        x + borda,
        y + borda,
        t(state, Chave::QuickTitle),
        theme::F_BODY,
        theme::text(),
    );

    let focused = |index: usize| state.focus.zone == Zone::QuickSettings && state.focus.index == index;
    let mut ry = y + header;

    for index in 0..rows {
        if focused(index) {
            p.stroke_round_rect(x + 8, ry, w - 16, row_h, r_caixa, theme::accent());
        }
        p.fill_round_rect(x + 8, ry, w - 16, row_h, r_caixa, theme::with_alpha(theme::BLACK, 70));

        let (label, icon_name) = match index {
            0 => (t(state, Chave::QuickVolume), "vol"),
            1 => (t(state, Chave::QuickBrightness), "sun"),
            2 => (t(state, Chave::SetPerformance), "bolt"),
            3 => (t(state, Chave::Wifi), "wifi"),
            4 => (t(state, Chave::Bluetooth), "bt"),
            _ => (t(state, Chave::QuickHud), "chart"),
        };
        let icone = (row_h as f32 * 0.35) as i32;
        p.icon(icon_name, x + borda + icone / 2, ry + row_h / 2, icone, theme::accent());
        let ty = vcenter(ry, row_h, p.text_size(label, theme::F_SMALL).1);
        p.text_left(x + borda + icone + m.e(6).max(3), ty, label, theme::F_SMALL, theme::text());

        // Barra e valor em fração da largura: em 520 px fixos a barra tinha
        // 220 px; num painel estreito ela passava do painel.
        let bar_x = x + (w as f32 * 0.40) as i32;
        let bar_w = (w as f32 * 0.40) as i32;
        match index {
            0 | 1 => {
                let value = if index == 0 { state.settings.volume } else { state.settings.brightness };
                let bar_y = ry + row_h / 2 - 3;
                p.fill_round_rect(bar_x, bar_y, bar_w, 6, 3, theme::with_alpha(theme::WHITE, 30));
                p.fill_round_rect(bar_x, bar_y, (bar_w * value as i32 / 100).max(2), 6, 3, theme::accent());
                p.text_right(x + w - borda, ty, &format!("{value}%"), theme::F_TINY, theme::accent());
            }
            2 => {
                p.text_right(
                    x + w - borda,
                    ty,
                    state.settings.perf.label(state.settings.idioma),
                    theme::F_TINY,
                    theme::accent(),
                );
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
                p.fill_round_rect(knob_x, trilho_y, trilho, m.e(16).max(10), m.e(8).max(3), if on { theme::with_alpha(theme::accent(), 80) } else { theme::with_alpha(theme::WHITE, 30) });
                let r = m.e(7).max(4);
                let dot_x = if on { knob_x + trilho - r * 2 - m.e(2).max(1) } else { knob_x + m.e(2).max(1) };
                p.fill_circle(dot_x + r, trilho_y + m.e(8).max(5), r, if on { theme::accent() } else { theme::dim() });
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
    let acoes = [t(state, Chave::PowerSuspend), t(state, Chave::PowerRestart)];
    for (i, label) in acoes.iter().enumerate() {
        let bx = x + 8 + i as i32 * (btn_w + 8);
        let danger = i == 1;
        p.fill_round_rect(
            bx,
            by,
            btn_w,
            btn_h,
            r_caixa,
            if danger { theme::with_alpha(theme::danger(), 40) } else { theme::panel2() },
        );
        if focused(6 + i) {
            p.stroke_round_rect(bx, by, btn_w, btn_h, r_caixa, theme::accent());
        }
        let ty = vcenter(by, btn_h, p.text_size(label, theme::F_SMALL).1);
        p.text_center(bx + btn_w / 2, ty, label, theme::F_SMALL, if danger { theme::danger() } else { theme::text() });
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

    p.fill_round_rect(x, y, w, h, r_painel, theme::panel());

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
        (t(state, Chave::DetailYear), game.year_label()),
        (t(state, Chave::DetailHours), game.play_label(state.settings.idioma)),
        (t(state, Chave::DetailLast), game.last_played.clone()),
        (
            t(state, Chave::DetailFavorite),
            if game.favorite {
                t(state, Chave::DetailYes).into()
            } else {
                "—".into()
            },
        ),
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
        t(state, Chave::HomePlay).to_string(),
        if game.favorite {
            t(state, Chave::DetailFavoriteRemove).to_string()
        } else {
            t(state, Chave::DetailFavoriteAdd).to_string()
        },
        t(state, Chave::DetailOptions).to_string(),
        t(state, Chave::HintBack).to_string(),
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
        p.fill_round_rect(bx, by, btn_w, btn_h, r, if primary { theme::accent() } else { theme::panel2() });
        if focused {
            p.stroke_round_rect(bx, by, btn_w, btn_h, r, theme::WHITE);
        }
        let color = if primary { theme::bg() } else { theme::text() };
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
    let labels = [
        t(state, Chave::PowerSuspend),
        t(state, Chave::PowerRestart),
        t(state, Chave::PowerShutdown),
        t(state, Chave::PowerCancel),
    ];
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
    p.fill_round_rect(x, y, w, h, r_painel, theme::panel());
    p.stroke_round_rect(x, y, w, h, r_painel, theme::line2());

    let cx = x + w / 2;
    p.fill_round_rect(cx - icone / 2, y + m.e(20).max(8), icone, icone, m.e(12).max(4), theme::with_alpha(theme::danger(), 40));
    p.icon("power", cx, y + m.e(20).max(8) + icone / 2, (icone as f32 * 0.46) as i32, theme::danger());

    p.text_center(
        cx,
        y + icone + m.e(12).max(4),
        t(state, Chave::PowerTitle),
        theme::F_BODY,
        theme::text(),
    );
    p.text_center(
        cx,
        y + icone + m.e(34).max(18),
        t(state, Chave::PowerSub),
        theme::F_TINY,
        theme::dim(),
    );

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
            if danger { theme::with_alpha(theme::danger(), 200) } else { theme::panel2() },
        );
        if focused {
            p.stroke_round_rect(x + borda, by, w - borda * 2, btn_h, r_btn, theme::accent());
        }
        let color = if danger { theme::WHITE } else if i == 3 { theme::dim() } else { theme::text() };
        let ty = vcenter(by, btn_h, p.text_size(label, theme::F_SMALL).1);
        p.text_center(cx, ty, label, theme::F_SMALL, color);
        layout.hits.push((
            Rect::new(x + borda, by, (w - borda * 2) as u32, btn_h as u32),
            Hit::PowerButton(i),
        ));
        by += btn_h + gap;
    }
}

/// Tela de boas-vindas: a escolha do idioma na primeira abertura.
///
/// ## Por que a tela existe
///
/// O aparelho sai de fábrica em inglês, que é o padrão comercial. Quem não lê
/// inglês abriria o produto e encontraria uma interface que não entende, sem
/// caminho de volta antes de aprender onde ficam os ajustes. Perguntar uma vez,
/// antes de mostrar o resto, resolve isso com uma tela.
///
/// ## Ela aparece uma vez só
///
/// Quem decide é `State::idioma_definido`, e quem o liga é a escolha (ou o
/// arquivo `/user/.config/giochi/language` já existente). Uma tela que
/// reaparece a cada boot obrigaria quem já escolheu a escolher de novo.
///
/// ## O título muda com o foco
///
/// Título e instrução são desenhados **no idioma que está sob o cursor**, não
/// no atual (que ainda é o padrão). É o único jeito de quem chegou aqui sem
/// entender inglês confirmar que escolheu o idioma certo antes de apertar `A`:
/// ele vê a frase na própria língua enquanto anda na lista.
fn boas_vindas(p: &mut Painter, state: &State, layout: &mut Layout) {
    let m = Moldura::de(p);
    let idiomas = Idioma::TODOS;
    let foco = state.focus.index.min(idiomas.len() - 1);
    let idioma_foco = idiomas[foco];

    let borda = m.e(18).max(8);
    let w = m.safe_w.min(m.e(560).max(280));
    let x = m.centro(w);

    let h_titulo = p.text_size("Ay", theme::F_HERO).1;
    let h_corpo = p.text_size("Ay", theme::F_SMALL).1;
    let h_tiny = p.text_size("Ay", theme::F_TINY).1;
    let gap = m.e(8).max(4);

    let cabecalho = borda + h_titulo + gap + h_corpo * 2 + gap;
    let rodape = borda + h_tiny;
    // Dez linhas têm de caber na altura que sobra. O piso de 22 px mantém a
    // linha tocável; se não couber, o painel rola para fora — e aí a escolha
    // ainda funciona pelo d-pad, que não depende de geometria.
    let disponivel = (m.main_h - cabecalho - rodape).max(1);
    let linha_h = (disponivel / idiomas.len() as i32).clamp(22, 38);
    let h = (cabecalho + linha_h * idiomas.len() as i32 + rodape).min(m.main_h);
    let y = (m.top + (m.main_h - h) / 2).max(m.top);

    let r_painel = m.e(theme::RADIUS_PANEL).max(4);
    p.fill_round_rect(x, y, w, h, r_painel, theme::panel());
    p.stroke_round_rect(x, y, w, h, r_painel, theme::line2());

    let cx = x + w / 2;
    let ty = y + borda;
    p.text_center(
        cx,
        ty,
        idioma::texto(idioma_foco, Chave::WelcomeTitle),
        theme::F_HERO,
        theme::accent(),
    );

    // A instrução ocupa até duas linhas: em 640×480 ela não cabe numa só em
    // nenhum dos dez idiomas.
    let lede = idioma::texto(idioma_foco, Chave::WelcomeLede);
    let (l1, l2) = split_title(lede, w - borda * 2, &mut |s| p.text_size(s, theme::F_SMALL).0);
    let ly = ty + h_titulo + gap;
    p.text_center(cx, ly, &l1, theme::F_SMALL, theme::dim());
    if !l2.is_empty() {
        p.text_center(cx, ly + h_corpo, &l2, theme::F_SMALL, theme::dim());
    }

    let mut ry = y + cabecalho;
    let r_linha = m.e(8).max(3);
    for (i, idioma) in idiomas.iter().enumerate() {
        let focado = i == foco;
        let largura = w - borda * 2;
        p.fill_round_rect(
            x + borda,
            ry,
            largura,
            linha_h,
            r_linha,
            if focado {
                theme::with_alpha(theme::accent(), 40)
            } else {
                theme::with_alpha(theme::BLACK, 70)
            },
        );
        if focado {
            p.stroke_round_rect(x + borda, ry, largura, linha_h, r_linha, theme::accent());
        }

        let cor = if focado { theme::text() } else { theme::dim() };
        let ny = vcenter(ry, linha_h, p.text_size(idioma.nome_proprio(), theme::F_SMALL).1);
        p.text_left(x + borda + gap, ny, idioma.nome_proprio(), theme::F_SMALL, cor);

        // O nome em inglês fica à direita e em tom apagado. Ele não é enfeite:
        // se a fonte do aparelho não tiver os glifos daquele idioma (chinês,
        // devanágari, bengali — ver `docs/idiomas.md`), a linha esquerda sai
        // vazia, e sem esta segunda coluna a lista teria dez linhas anônimas.
        let ny_ingles = vcenter(ry, linha_h, p.text_size(idioma.nome(), theme::F_TINY).1);
        p.text_right(
            x + w - borda - gap,
            ny_ingles,
            idioma.nome(),
            theme::F_TINY,
            theme::faint(),
        );

        layout.hits.push((
            Rect::new(x + borda, ry, largura as u32, linha_h as u32),
            Hit::Idioma(i),
        ));
        ry += linha_h;
    }

    // Rodapé: o inglês já está escolhido e `A` confirma o que está em foco.
    let nota = format!(
        "{}: {}",
        idioma::texto(idioma_foco, Chave::WelcomeConfirm),
        idioma_foco.nome_proprio()
    );
    p.text_center(
        cx,
        y + h - rodape + m.e(2).max(1),
        &nota,
        theme::F_TINY,
        theme::faint(),
    );
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
    p.fill_round_rect(x, y, w, h, r_painel, theme::panel());
    p.stroke_round_rect(x, y, w, h, r_painel, theme::line2());

    p.icon("search", x + borda + m.e(10).max(4), y + m.e(30).max(14), m.e(18).max(10), theme::accent());
    let query = if state.search_query.is_empty() {
        t(state, Chave::SearchPlaceholder).to_string()
    } else {
        state.search_query.clone()
    };
    let qcolor = if state.search_query.is_empty() { theme::faint() } else { theme::text() };
    p.text_left(x + borda + m.e(28).max(12), y + m.e(22).max(10), &query, theme::F_BODY, qcolor);
    p.fill_rect(x, y + linha_y, w, 1, theme::line());

    // Teclado.
    let kb_x = x + borda;
    let kb_y = y + linha_y + m.e(12).max(4);
    let r_tecla = m.e(7).max(3);

    // `enumerate` em vez de indexar pelo laço: a posição na grade **é** a
    // posição do caractere, e ler `KEYBOARD[row][col]` deixava as duas
    // dependentes de `row`/`col` continuarem sincronizados à mão.
    for (row, linha) in KEYBOARD.iter().enumerate() {
        for (col, &ch) in linha.iter().enumerate() {
            let index = row * KEYBOARD_COLS + col;
            let kx = kb_x + col as i32 * (key_w + gap);
            let ky = kb_y + row as i32 * (key_h + gap);
            let focused = state.focus.zone == Zone::SearchKeyboard && state.focus.index == index;

            p.fill_round_rect(kx, ky, key_w, key_h, r_tecla, theme::panel2());
            p.stroke_round_rect(kx, ky, key_w, key_h, r_tecla, if focused { theme::accent() } else { theme::line() });


            let label = if ch == ' ' {
                t(state, Chave::SearchSpace).to_string()
            } else if ch == '\u{8}' {
                "<".to_string()
            } else {
                ch.to_string()
            };
            let color = if focused { theme::accent() } else { theme::text() };
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
    let titulo_res = tv(state, Chave::SearchResults, &results.len().to_string());
    p.text_left(res_x, res_topo, &titulo_res, theme::F_TINY, theme::dim());

    let mut ry = res_topo + m.e(20).max(10);
    let clip_top = ry;
    let clip_h = (y + h - borda - ry).max(20);
    p.push_clip(Clip { x: res_x, y: clip_top, w: res_w, h: clip_h });

    if results.is_empty() {
        p.text_center(
            res_x + res_w / 2,
            ry + 30,
            t(state, Chave::SearchEmpty),
            theme::F_SMALL,
            theme::faint(),
        );
    } else {
        for (i, game_index) in results.iter().enumerate() {
            let game = &state.games[*game_index];
            let row_h = theme::TOUCH;
            let focused = state.focus.zone == Zone::SearchResults && state.focus.index == i;

            p.fill_round_rect(res_x, ry, res_w, row_h - 4, 7, theme::panel2());
            p.stroke_round_rect(res_x, ry, res_w, row_h - 4, 7, if focused { theme::accent() } else { theme::line() });

            let accent = theme::game_accent(game.id);
            p.fill_round_rect(res_x + 8, ry + 8, 28, 28, 5, accent);

            let title = truncate(&game.title, res_w - 60, &mut |s| p.text_size(s, theme::F_SMALL).0);
            let ty = vcenter(ry, row_h - 4, p.text_size(&title, theme::F_SMALL).1);
            p.text_left(res_x + 44, ty, &title, theme::F_SMALL, theme::text());

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
    p.icon("check", x + 22, y + h / 2, 14, theme::accent());
    let ty = vcenter(y, h, p.text_size(text, theme::F_SMALL).1);
    p.text_left(x + 38, ty, text, theme::F_SMALL, theme::text());
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
    p.stroke_round_rect(x, y, w, h, 8, theme::line2());

    let items = [
        ("FPS", if state.hud.fps > 0 { state.hud.fps.to_string() } else { "—".into() }),
        ("TEMP", "—".into()),
        ("TDP", "—".into()),
    ];
    let mut ix = x + 14;
    for (label, value) in items {
        p.text_left(ix, y + 8, label, theme::F_TINY, theme::dim());
        p.text_left(ix, y + 20, &value, theme::F_SMALL, theme::accent());
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
    fn play_label(&self, idioma: Idioma) -> String;
}

impl GameLabel for giochi_launcher_core::Game {
    fn year_label(&self) -> String {
        // O ano vem do catálogo. Já foi `#{:03}` com o id: o cartão mostrava
        // `Spacewar • Ação • #002`, que é o índice do cartão e não informação
        // nenhuma — o usuário viu na tela e perguntou o que era. Um número
        // interno não pode aparecer como se fosse dado do jogo.
        if self.year > 0 {
            self.year.to_string()
        } else {
            "—".to_string()
        }
    }

    fn play_label(&self, idioma: Idioma) -> String {
        if self.hours > 0.0 {
            format!("{:.1}h", self.hours)
        } else {
            idioma::texto(idioma, Chave::GameNeverPlayed).to_string()
        }
    }
}



// ── Wi-Fi e Bluetooth ──────────────────────────────────────

/// Painel de lista, usado por Wi-Fi e Bluetooth.
///
/// Os dois são a mesma forma: uma lista com um item final que dispara nova
/// varredura. Escrever duas telas quase iguais garantiria que uma envelhecesse
/// diferente da outra.
struct Lista<'a> {
    titulo: &'a str,
    rodape: String,
    itens: Vec<(String, String)>, // (título, detalhe)
    /// Texto do item final ("Procurar redes", "Procurar aparelhos").
    acao: &'a str,
    /// Texto do item final quando não há nada na lista. Vazio = usa `acao`.
    vazio: String,
}

fn painel_lista(p: &mut Painter, state: &State, layout: &mut Layout, lista: Lista, zona: Zone) {
    let m = Moldura::de(p);
    let w = m.safe_w.min(m.e(620).max(300));
    let x = m.w / 2 - w / 2;

    let linha_h = m.e(theme::TOUCH).max(34);
    let gap = m.e(4).max(2);
    let total = lista.itens.len() + 1;
    let altura_lista = linha_h * total as i32 + gap * (total as i32 - 1).max(0);

    let cabecalho = m.e(44).max(28);
    let rodape_h = m.e(26).max(18);
    let h = (cabecalho + altura_lista + rodape_h + m.e(16).max(8)).min(m.main_h);
    let y = (m.top + (m.main_h - h) / 2).max(m.top);

    p.fill_round_rect(x, y, w, h, m.e(theme::RADIUS_PANEL).max(4), theme::panel());
    p.stroke_round_rect(x, y, w, h, m.e(theme::RADIUS_PANEL).max(4), theme::line2());
    p.text_left(x + m.e(16).max(8), y + m.e(12).max(6), lista.titulo, theme::F_BODY, theme::text());

    let mut ry = y + cabecalho;
    let borda = m.e(10).max(5);

    for (i, (titulo, detalhe)) in lista.itens.iter().enumerate() {
        let focado = state.focus.zone == zona && state.focus.index == i;
        p.fill_round_rect(x + m.e(6).max(3), ry, w - m.e(12).max(6), linha_h, m.e(8).max(3),
            if focado { theme::panel2() } else { theme::with_alpha(theme::BLACK, 70) });
        if focado {
            p.stroke_round_rect(x + m.e(6).max(3), ry, w - m.e(12).max(6), linha_h, m.e(8).max(3), theme::accent());
        }
        let dw = p.text_size(detalhe, theme::F_TINY).0;
        let t = truncate(titulo, w - borda * 3 - dw, &mut |s| p.text_size(s, theme::F_SMALL).0);
        let ty = vcenter(ry, linha_h, p.text_size(&t, theme::F_SMALL).1);
        p.text_left(x + borda, ty, &t, theme::F_SMALL, theme::text());
        let dy = vcenter(ry, linha_h, p.text_size(detalhe, theme::F_TINY).1);
        p.text_right(x + w - borda, dy, detalhe, theme::F_TINY, theme::accent());
        layout.hits.push((
            Rect::new(x + m.e(6).max(3), ry, (w - m.e(12).max(6)) as u32, linha_h as u32),
            if zona == Zone::BtDevices { Hit::BtDevice(i) } else { Hit::WifiNet(i) },
        ));
        ry += linha_h + gap;
    }

    // Item final: nova varredura. Sempre presente, mesmo com lista vazia — é o
    // único caminho de volta quando a primeira varredura não achou nada.
    let i = lista.itens.len();
    let focado = state.focus.zone == zona && state.focus.index == i;
    p.fill_round_rect(x + m.e(6).max(3), ry, w - m.e(12).max(6), linha_h, m.e(8).max(3),
        if focado { theme::panel2() } else { theme::with_alpha(theme::BLACK, 70) });
    if focado {
        p.stroke_round_rect(x + m.e(6).max(3), ry, w - m.e(12).max(6), linha_h, m.e(8).max(3), theme::accent());
    }
    let texto_acao = if lista.vazio.is_empty() { lista.acao.to_string() } else { lista.vazio.to_string() };
    let cor = if lista.vazio.is_empty() { theme::accent() } else { theme::dim() };
    let ty = vcenter(ry, linha_h, p.text_size(&texto_acao, theme::F_SMALL).1);
    p.text_left(x + borda, ty, &texto_acao, theme::F_SMALL, cor);
    layout.hits.push((
        Rect::new(x + m.e(6).max(3), ry, (w - m.e(12).max(6)) as u32, linha_h as u32),
        if zona == Zone::BtDevices { Hit::BtDevice(i) } else { Hit::WifiNet(i) },
    ));

    // Rodapé: o estado real, não uma promessa.
    let rodape = truncate(&lista.rodape, w - borda * 2, &mut |s| p.text_size(s, theme::F_TINY).0);
    p.text_left(x + borda, y + h - rodape_h + m.e(4).max(2), &rodape, theme::F_TINY, theme::dim());
}

fn wifi_list(p: &mut Painter, state: &State, layout: &mut Layout) {
    let sf = &state.sem_fio;

    let itens: Vec<(String, String)> = sf
        .redes
        .iter()
        .map(|r| {
            let detalhe = if Some(&r.ssid) == sf.conectado.as_ref() {
                t(state, Chave::WifiConnected).to_string()
            } else if r.salva {
                t(state, Chave::WifiSaved).to_string()
            } else if r.protegida {
                format!("{}% ▮", r.sinal)
            } else {
                t(state, Chave::WifiOpen).to_string()
            };
            (r.ssid.clone(), detalhe)
        })
        .collect();

    let (rodape, vazio) = if sf.tem_wifi == Some(false) {
        (
            t(state, Chave::WifiNoRadioFooter).to_string(),
            t(state, Chave::WifiNoRadioEmpty).to_string(),
        )
    } else if let Some(erro) = &sf.erro {
        // O motivo vem da camada de sistema (wpa_cli/BlueZ) e é diagnóstico:
        // ele cita `wpa_state` e códigos do BlueZ, que não se traduzem. Ver
        // `docs/idiomas.md` — é a única parte da tela que não acompanha o
        // idioma, e o porquê está escrito lá.
        (tv(state, Chave::ErrorPrefix, erro), String::new())
    } else if sf.escaneando {
        (
            t(state, Chave::WifiScanning).to_string(),
            t(state, Chave::ScanningShort).to_string(),
        )
    } else if itens.is_empty() {
        (
            t(state, Chave::WifiNone).to_string(),
            t(state, Chave::WifiNoneEmpty).to_string(),
        )
    } else {
        (
            tv(state, Chave::WifiFound, &itens.len().to_string()),
            t(state, Chave::ScanAgain).to_string(),
        )
    };

    painel_lista(
        p,
        state,
        layout,
        Lista {
            titulo: t(state, Chave::Wifi),
            rodape,
            itens,
            acao: t(state, Chave::WifiScanAction),
            vazio: if vazio.is_empty() { String::new() } else { vazio },
        },
        Zone::WifiNetworks,
    );
}

fn bt_list(p: &mut Painter, state: &State, layout: &mut Layout) {
    let sf = &state.sem_fio;
    let itens: Vec<(String, String)> = sf
        .dispositivos
        .iter()
        .map(|d| {
            let detalhe = if Some(&d.mac) == sf.bt_conectado.as_ref() {
                t(state, Chave::BtConnected).to_string()
            } else if d.pareado {
                t(state, Chave::BtPaired).to_string()
            } else {
                d.mac.clone()
            };
            (d.nome.clone(), detalhe)
        })
        .collect();

    let (rodape, vazio) = if sf.tem_bt == Some(false) {
        (
            t(state, Chave::BtNoRadioFooter).to_string(),
            t(state, Chave::BtNoRadioEmpty).to_string(),
        )
    } else if let Some(erro) = &sf.erro {
        (tv(state, Chave::ErrorPrefix, erro), String::new())
    } else if sf.bt_escaneando {
        (
            t(state, Chave::BtScanning).to_string(),
            t(state, Chave::ScanningShort).to_string(),
        )
    } else if itens.is_empty() {
        (
            t(state, Chave::BtNone).to_string(),
            t(state, Chave::BtNoneEmpty).to_string(),
        )
    } else {
        (
            tv(state, Chave::BtFound, &itens.len().to_string()),
            t(state, Chave::ScanAgain).to_string(),
        )
    };

    painel_lista(
        p,
        state,
        layout,
        Lista {
            titulo: t(state, Chave::Bluetooth),
            rodape,
            itens,
            acao: t(state, Chave::BtScanAction),
            vazio,
        },
        Zone::BtDevices,
    );
}

/// Teclado de senha.
///
/// Reusa o `KEYBOARD` da busca — mesma grade, mesmas teclas, `Cmd` diferente.
/// Inventar um teclado de senha separado duplicaria 40 linhas de layout para
/// ganhar nada: os caracteres são os mesmos.
fn wifi_password(p: &mut Painter, state: &State, layout: &mut Layout) {
    use giochi_launcher_core::{KEYBOARD, KEYBOARD_COLS};

    let m = Moldura::de(p);
    let w = m.safe_w.min(m.e(600).max(300));
    let x = m.w / 2 - w / 2;

    let ssid = state.sem_fio.alvo.clone().unwrap_or_default();
    let campo_h = m.e(34).max(24);
    let tecla_h = m.e(theme::TOUCH).max(30);
    let gap = m.e(4).max(2);
    let linhas = KEYBOARD.len() as i32;
    let cabecalho = m.e(30).max(20);
    // Soma o botão Conectar: sem ele o painel terminaria em cima da própria
    // saída, e o botão desenharia fora da moldura.
    let h = (cabecalho
        + campo_h
        + gap * 2
        + tecla_h * linhas
        + gap * (linhas - 1)
        + gap * 2
        + tecla_h
        + m.e(20).max(10))
        .min(m.main_h);
    let y = (m.top + (m.main_h - h) / 2).max(m.top);

    p.fill_round_rect(x, y, w, h, m.e(theme::RADIUS_PANEL).max(4), theme::panel());
    p.stroke_round_rect(x, y, w, h, m.e(theme::RADIUS_PANEL).max(4), theme::line2());

    let titulo_bruto = tv(state, Chave::PasswordTitle, &ssid);
    let titulo = truncate(&titulo_bruto, w - m.e(24).max(12), &mut |s| p.text_size(s, theme::F_BODY).0);
    p.text_left(x + m.e(12).max(6), y + m.e(8).max(4), &titulo, theme::F_BODY, theme::text());

    // Campo: bolinhas ou claro, alternado por X. O comprimento aparece sempre —
    // sem isso não se sabe se a tecla pegou.
    let cy = y + cabecalho;
    let borda = m.e(10).max(5);
    p.fill_round_rect(x + m.e(8).max(4), cy, w - m.e(16).max(8), campo_h, m.e(6).max(3),
        theme::with_alpha(theme::BLACK, 120));
    p.stroke_round_rect(x + m.e(8).max(4), cy, w - m.e(16).max(8), campo_h, m.e(6).max(3), theme::line());
    let visivel = &state.sem_fio.senha;
    let mostrado = if state.sem_fio.senha_visivel {
        visivel.clone()
    } else {
        "•".repeat(visivel.chars().count().min(32))
    };
    let mostrado = truncate(&mostrado, w - m.e(28).max(14), &mut |s| p.text_size(s, theme::F_BODY).0);
    let ty = vcenter(cy, campo_h, p.text_size(&mostrado, theme::F_BODY).1);
    p.text_left(x + borda + m.e(4).max(2), ty, &mostrado, theme::F_BODY, theme::accent());

    // Teclado.
    let ky0 = cy + campo_h + gap * 2;
    let tecla_w = (w - m.e(16).max(8) - gap * (KEYBOARD_COLS as i32 - 1)) / KEYBOARD_COLS as i32;
    for (r, linha) in KEYBOARD.iter().enumerate() {
        for (c, ch) in linha.iter().enumerate() {
            let idx = r * KEYBOARD_COLS + c;
            let focado = state.focus.zone == Zone::PasswordKeys && state.focus.index == idx;
            let tx = x + m.e(8).max(4) + c as i32 * (tecla_w + gap);
            let ty2 = ky0 + r as i32 * (tecla_h + gap);
            p.fill_round_rect(tx, ty2, tecla_w, tecla_h, m.e(6).max(3),
                if focado { theme::accent() } else { theme::panel2() });
            if focado {
                p.stroke_round_rect(tx, ty2, tecla_w, tecla_h, m.e(6).max(3), theme::WHITE);
            }
            let rotulo = if *ch == '\u{8}' { "⌫".to_string() } else { ch.to_string() };
            let cor = if focado { theme::bg() } else { theme::text() };
            let tcy = vcenter(ty2, tecla_h, p.text_size(&rotulo, theme::F_SMALL).1);
            p.text_center(tx + tecla_w / 2, tcy, &rotulo, theme::F_SMALL, cor);
            layout.hits.push((
                Rect::new(tx, ty2, tecla_w as u32, tecla_h as u32),
                Hit::PasswordKey(idx),
            ));
        }
    }

    // Botão Conectar — a saída do teclado.
    //
    // ## Por que ele existe
    //
    // Até 14/11/2026 a tela não tinha como confirmar nada. `A` digitava, `B`
    // descartava, e a senha nunca entrava em vigor: o comando `ConectarSenha`
    // estava no enum e **ninguém o emitia**. O relato do dono foi literal —
    // *"quando se digita a senha não há como confirmar"*.
    //
    // Ele fica **abaixo** do teclado e não dentro da grade de propósito: `A`
    // numa tecla digita, e uma célula da grade que não digitasse quebraria a
    // regra que a fileira de 7 colunas estabelece. Fora dela, o foco é um lugar
    // diferente com um significado diferente — e o `↓` da última linha chega
    // aqui.
    //
    // Desabilitado com o campo vazio: um `FAIL` do supplicant não ensina nada,
    // e o `ui-sdl` já avisa o motivo quando o usuário insiste.
    let by = ky0 + linhas * tecla_h + (linhas - 1) * gap + gap * 2;
    let bx = x + m.e(8).max(4);
    let bw = w - m.e(16).max(8);
    let bh = tecla_h;
    let focado = state.focus.zone == Zone::PasswordActions;
    let vazio = state.sem_fio.senha.is_empty();
    let fundo = if vazio {
        theme::panel2()
    } else if focado {
        theme::accent()
    } else {
        theme::with_alpha(theme::accent(), 90)
    };
    p.fill_round_rect(bx, by, bw, bh, m.e(6).max(3), fundo);
    if focado {
        p.stroke_round_rect(bx, by, bw, bh, m.e(6).max(3), theme::WHITE);
    }
    let rotulo = if vazio {
        t(state, Chave::PasswordConnectEmpty)
    } else {
        t(state, Chave::PasswordConnect)
    };
    let cor = if vazio {
        theme::dim()
    } else if focado {
        theme::bg()
    } else {
        theme::text()
    };
    let tcy = vcenter(by, bh, p.text_size(rotulo, theme::F_BODY).1);
    p.text_center(bx + bw / 2, tcy, rotulo, theme::F_BODY, cor);
    layout.hits.push((
        Rect::new(bx, by, bw as u32, bh as u32),
        Hit::PasswordConfirm,
    ));
}


#[cfg(test)]
mod tests {
    use super::{formatar_relogio, grade_para};

    fn chaves(state: &giochi_launcher_core::State) -> Vec<&'static str> {
        super::dicas(state).iter().map(|(k, _)| *k).collect()
    }

    /// O relógio é o que fazia a comparação de quadros reprovar 99,9 %.
    ///
    /// Aparelho sem NTP fica horas atrás do PC; os quatro dígitos do relógio
    /// diferiam e o portão acusava divergência em todas as telas. `formatar_`
    /// `relogio` é a parte pura — `GIOCHI_HORA_FIXA` (lida em `current_clock`)
    /// é o que congela os dois lados.
    #[test]
    fn relogio_formata_hh_mm() {
        assert_eq!(formatar_relogio(0), "00:00");
        assert_eq!(formatar_relogio(59), "00:00");
        assert_eq!(formatar_relogio(60), "00:01");
        // 2026-11-09 03:44 UTC (o instante medido no PC nesta rodada)
        assert_eq!(formatar_relogio(1_794_195_840), "03:44");
        // vira o dia, e a hora volta a zero em vez de mostrar 24
        assert_eq!(formatar_relogio(86_400), "00:00");
        assert_eq!(formatar_relogio(23 * 3600 + 59 * 60), "23:59");
    }

    /// O rodapé só pode anunciar o gatilho onde ele faz alguma coisa.
    ///
    /// O rodapé já mentiu duas vezes (`A — Jogar` que não iniciava, `Jogar` na
    /// Biblioteca onde `A` abre a ficha). Anunciar "L2/R2" numa aba sem sub-aba
    /// seria a terceira — e o usuário apertaria a tecla sem retorno nenhum.
    #[test]
    fn rodape_so_anuncia_gatilho_onde_ele_age() {
        use giochi_launcher_core::{State, Tab};

        let state = State::new(Vec::new(), Vec::new());

        for tab in [Tab::Library, Tab::Settings] {
            let s = State { tab, ..state.clone() };
            assert!(
                chaves(&s).contains(&"L2/R2"),
                "{tab:?} tem sub-abas e o rodapé precisa dizer qual tecla as troca"
            );
        }

        for tab in [Tab::Home, Tab::Apps] {
            let s = State { tab, ..state.clone() };
            assert!(
                !chaves(&s).contains(&"L2/R2"),
                "{tab:?} não tem sub-abas: anunciar o gatilho seria promessa vazia"
            );
        }
    }

    /// `A` só é anunciado como "Jogar" na Home, que é a única aba onde ele inicia.
    ///
    /// A comparação é com a **chave traduzida**, não com um texto: o padrão do
    /// aparelho é inglês e o usuário pode estar em qualquer um dos dez idiomas.
    /// Amarrar o teste a "Jogar" reprovaria as dez traduções sem que a regra
    /// tivesse mudado.
    #[test]
    fn rodape_nao_promete_jogar_onde_a_apenas_abre() {
        use giochi_launcher_core::idioma::{texto, Chave};
        use giochi_launcher_core::{Idioma, State, Tab};

        let base = State::new(Vec::new(), Vec::new());
        let acao = |tab: Tab, idioma: Idioma| {
            let mut s = State { tab, ..base.clone() };
            s.settings.idioma = idioma;
            super::dicas(&s)
                .iter()
                .find(|(k, _)| *k == "A")
                .map(|(_, v)| v.to_string())
        };

        for idioma in [Idioma::En, Idioma::Pt, Idioma::Zh] {
            assert_eq!(
                acao(Tab::Home, idioma).as_deref(),
                Some(texto(idioma, Chave::HintPlay)),
                "na Home o A inicia: {}",
                idioma.nome()
            );
            assert_eq!(
                acao(Tab::Library, idioma).as_deref(),
                Some(texto(idioma, Chave::HintOpen)),
                "na Biblioteca o A abre a ficha: {}",
                idioma.nome()
            );
            assert_eq!(
                acao(Tab::Apps, idioma),
                None,
                "nos Apps o A não faz nada ({})",
                idioma.nome()
            );
        }
    }

    /// A linha do idioma mostra o idioma **escolhido**, no idioma da interface.
    ///
    /// É o que o dono pediu: a opção de trocar o idioma mora onde se troca o
    /// tema. Se a linha mostrasse `Idioma: Idioma` (rótulo traduzido, valor não)
    /// o usuário que não lê o idioma atual não teria como saber onde está.
    #[test]
    fn linha_de_idioma_mostra_o_idioma_escolhido() {
        use giochi_launcher_core::{Idioma, State};

        let base = State::new(Vec::new(), Vec::new());
        let linha = |idioma: Idioma| {
            let mut s = State { ..base.clone() };
            s.settings.idioma = idioma;
            super::linhas_da_categoria("appearance", &s)
        };

        let pt = linha(Idioma::Pt);
        assert_eq!(pt.len(), 3, "tema, idioma e escala");
        assert_eq!(pt[0].0, "Tema");
        assert_eq!(pt[0].1, "Giochi Dark", "nome do tema não se traduz");
        assert_eq!(pt[1].0, "Idioma");
        assert_eq!(pt[1].1, "Português", "o valor é o nome no próprio idioma");

        // Em chinês, o rótulo muda e o valor continua sendo o nome nativo.
        let zh = linha(Idioma::Zh);
        assert_eq!(zh[1].0, "语言");
        assert_eq!(zh[1].1, "中文");

        // O inglês é o padrão comercial: quem não escolheu nada cai aqui.
        let en = linha(Idioma::En);
        assert_eq!(en[1].0, "Language");
        assert_eq!(en[1].1, "English");
    }

    /// A linha da escala continua sendo informativa, e não um ajuste.
    #[test]
    fn linha_de_escala_declara_que_acompanha_o_painel() {
        use giochi_launcher_core::State;

        let s = State::new(Vec::new(), Vec::new());
        let linhas = super::linhas_da_categoria("appearance", &s);
        assert_eq!(linhas[2].0, "Scale");
        assert_eq!(linhas[2].1, "follows the panel");
    }

    /// O núcleo e o desenho precisam concordar sobre quantas linhas a categoria
    /// tem.
    ///
    /// Divergir não dá erro em lugar nenhum: o foco simplesmente para antes da
    /// última linha, ou anda por uma linha que não existe. É a mesma classe de
    /// erro do carrossel dessincronizado — dois lugares descrevendo a mesma
    /// lista. Este teste é o elo que faltava entre `views::linhas_da_categoria`
    /// e `state::settings_rows`.
    #[test]
    fn foco_e_desenho_concordam_no_numero_de_linhas() {
        use giochi_launcher_core::{settings_rows, State, SETTINGS_CATEGORIES};

        let state = State::new(Vec::new(), Vec::new());
        for (i, categoria) in SETTINGS_CATEGORIES.iter().enumerate() {
            let desenhadas = super::linhas_da_categoria(categoria, &state).len();
            assert_eq!(
                desenhadas,
                settings_rows(i),
                "categoria {categoria} (índice {i}): o desenho tem {desenhadas}                  linha(s) e o foco permite {}",
                settings_rows(i)
            );
        }
    }

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

    /// O rótulo de ano é do **jogo**, não do cartão.
    ///
    /// Já esteve errado: `year_label` devolvia `#{:03}` com o id e o cartão
    /// mostrava `Spacewar • Ação • #002`. Lido na tela do X35H, isso passa por
    /// informação do jogo — o usuário perguntou o que era. Índice interno não
    /// pode chegar ao desenho.
    #[test]
    fn ano_do_jogo_nao_e_o_indice_do_cartao() {
        use super::GameLabel;
        use giochi_launcher_core::Game;

        let g = Game::new(7, "Spacewar", "Ação", 0.0, "nunca", false).com_ano(1962);
        assert_eq!(g.year_label(), "1962");
        assert_ne!(g.year_label(), "#007");

        // Sem ano no catálogo a UI diz que não sabe; não inventa número.
        let sem_ano = Game::new(7, "Spacewar", "Ação", 0.0, "nunca", false);
        assert_eq!(sem_ano.year_label(), "—");
    }
}
