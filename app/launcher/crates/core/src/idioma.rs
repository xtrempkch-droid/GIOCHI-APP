//! Idioma da interface: os dez do site, com inglês como padrão comercial.
//!
//! ## Por que os mesmos dez do site
//!
//! O site (`GIOCHI-OS`, `scripts/lib/i18n.mjs`) já é publicado nos dez idiomas
//! mais falados por número total de falantes (Ethnologue): inglês, chinês,
//! hindi, espanhol, francês, árabe, bengali, português, russo e urdu. Um
//! launcher com outra lista obrigaria a mesma pessoa a aprender duas listas, e
//! o idioma que ela escolheu na página não estaria no aparelho. A ordem também
//! é a do site, pelo mesmo motivo.
//!
//! ## Por que o padrão é inglês
//!
//! É o único dos dez que a maioria dos compradores de fora consegue ler sem ter
//! escolhido nada. Quem abre o aparelho pela primeira vez recebe a tela de
//! boas-vindas com a escolha; quem não escolher (ou apertar `B`) fica em inglês,
//! que é o padrão comercial do produto — e não o português de quem escreveu o
//! código.
//!
//! ## Por que uma tabela, e não dez `match`
//!
//! As traduções ficam **lado a lado**, uma linha por chave: quem revisa um
//! idioma vê a frase e a original na mesma linha. A completude não fica
//! dependendo de disciplina porque é verificada por dois testes:
//!
//! 1. `toda_chave_da_tabela_bate_com_o_enum` — a linha `i` da tabela é a
//!    chave `i` do enum (o `#[repr(usize)]` faz o índice da chave ser o próprio
//!    discriminante, então a consulta é `O(1)` e uma linha fora de ordem é
//!    reprovada em vez de traduzir a frase errada);
//! 2. `nenhum_texto_vazio` — nenhuma das dez colunas de nenhuma chave está
//!    vazia. Sem isto, "esqueci o urdu desta linha" apareceria como um rótulo
//!    em branco na tela, que é o mesmo defeito do `—` que nunca chega.
//!
//! ## O que este módulo **não** resolve: desenho de texto
//!
//! Ele diz *qual* frase mostrar. Quem desenha é o `ui-sdl`, e ali há duas
//! limitações medidas, ambas declaradas em `docs/idiomas.md`:
//!
//! - **fonte**: o `DejaVuSans.ttf` do aparelho não tem glifos de chinês,
//!   devanágari nem bengali. O `draw.rs` monta uma cadeia de fontes por idioma e
//!   escolhe, por string, a primeira que cobre todos os caracteres;
//! - **bidi/shaping**: o `SDL2_ttf` sem HarfBuzz não junta letras árabes nem
//!   aplica direção direita-para-esquerda. Árabe e urdu aparecem com letras
//!   isoladas e na ordem do código. `Dir::Rtl` está registrado aqui para quando
//!   o desenho souber usar, e o `App` avisa no log ao abrir num idioma RTL —
//!   declarar o que não está pronto é melhor que fingir que está.

/// Declara as chaves e a lista `TODAS` a partir de uma linha por chave.
macro_rules! chaves {
    ($($variante:ident => $chave:literal),* $(,)?) => {
        /// Chave de texto da interface.
        ///
        /// `#[repr(usize)]` de propósito: sem discriminante explícito, o índice
        /// da variante **é** a ordem de declaração, e a tabela de traduções pode
        /// ser consultada por índice em vez de por busca linear. Ver o teste
        /// `toda_chave_da_tabela_bate_com_o_enum`.
        #[repr(usize)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Chave {
            $($variante),*
        }

        impl Chave {
            /// Todas as chaves, na ordem da declaração.
            pub const TODAS: &'static [Chave] = &[$(Chave::$variante),*];

            /// Identificador estável da chave, como aparece na tabela.
            pub fn id(self) -> &'static str {
                match self {
                    $(Chave::$variante => $chave),*
                }
            }

            /// Posição na tabela. É o discriminante, por causa do `repr`.
            pub const fn indice(self) -> usize {
                self as usize
            }
        }
    };
}

chaves! {
    TabHome => "tab.home",
    TabLibrary => "tab.library",
    TabApps => "tab.apps",
    TabSettings => "tab.settings",

    HintNavigate => "hint.navigate",
    HintSelect => "hint.select",
    HintBack => "hint.back",
    HintAdjust => "hint.adjust",
    HintClose => "hint.close",
    HintAction => "hint.action",
    HintConfirm => "hint.confirm",
    HintCancel => "hint.cancel",
    HintConnect => "hint.connect",
    HintNetwork => "hint.network",
    HintType => "hint.type",
    HintShow => "hint.show",
    HintPair => "hint.pair",
    HintDevice => "hint.device",
    HintEnter => "hint.enter",
    HintCategory => "hint.category",
    HintTab => "hint.tab",
    HintQuick => "hint.quick",
    HintCategories => "hint.categories",
    HintOpen => "hint.open",
    HintPlay => "hint.play",
    HintDetails => "hint.details",
    HintSearch => "hint.search",
    HintFilter => "hint.filter",
    HintChoose => "hint.choose",

    HomeEmptyTitle => "home.empty.title",
    HomeEmptyHint => "home.empty.hint",
    HomePlay => "home.play",
    LibraryTitle => "library.title",
    LibraryEmpty => "library.empty",
    AppsEmptyTitle => "apps.empty.title",
    AppsEmptyHint => "apps.empty.hint",

    FilterAll => "filter.all",
    FilterFavorites => "filter.favorites",
    FilterRecent => "filter.recent",

    PerfEco => "perf.eco",
    PerfBalanced => "perf.balanced",
    PerfBoost => "perf.boost",

    SetAppearance => "set.appearance",
    SetSound => "set.sound",
    SetNetwork => "set.network",
    SetController => "set.controller",
    SetSystem => "set.system",
    SetAbout => "set.about",

    SetTheme => "set.theme",
    SetLanguage => "set.language",
    SetScale => "set.scale",
    SetScaleValue => "set.scale.value",
    SetVolume => "set.volume",
    SetOutput => "set.output",
    SetOutputValue => "set.output.value",
    Wifi => "wifi",
    Bluetooth => "bluetooth",
    SetNoRadio => "set.no_radio",
    SetNotConnected => "set.not_connected",
    SetVersion => "set.version",
    SetPerformance => "set.performance",

    CtrlConfirm => "ctrl.confirm",
    CtrlBack => "ctrl.back",
    CtrlDetails => "ctrl.details",
    CtrlSearch => "ctrl.search",
    CtrlSwitchTab => "ctrl.switch_tab",
    CtrlQuick => "ctrl.quick",
    CtrlPower => "ctrl.power",

    AboutLine1 => "about.1",
    AboutLine2 => "about.2",
    AboutLine3 => "about.3",
    AboutLine4 => "about.4",
    AboutLine5 => "about.5",

    QuickTitle => "quick.title",
    QuickVolume => "quick.volume",
    QuickBrightness => "quick.brightness",
    QuickHud => "quick.hud",

    PowerTitle => "power.title",
    PowerSub => "power.sub",
    PowerSuspend => "power.suspend",
    PowerRestart => "power.restart",
    PowerShutdown => "power.shutdown",
    PowerCancel => "power.cancel",

    SearchPlaceholder => "search.placeholder",
    SearchEmpty => "search.empty",
    SearchResults => "search.results",
    SearchSpace => "search.space",

    DetailYear => "detail.year",
    DetailHours => "detail.hours",
    DetailLast => "detail.last",
    DetailFavorite => "detail.favorite",
    DetailYes => "detail.yes",
    DetailFavoriteAdd => "detail.favorite_add",
    DetailFavoriteRemove => "detail.favorite_remove",
    DetailOptions => "detail.options",

    GameNeverPlayed => "game.never_played",

    WifiConnected => "wifi.connected",
    WifiSaved => "wifi.saved",
    WifiOpen => "wifi.open",
    WifiNoRadioFooter => "wifi.no_radio.footer",
    WifiNoRadioEmpty => "wifi.no_radio.empty",
    WifiScanning => "wifi.scanning",
    WifiNone => "wifi.none",
    WifiNoneEmpty => "wifi.none.empty",
    WifiFound => "wifi.found",
    WifiScanAction => "wifi.scan_action",

    BtConnected => "bt.connected",
    BtPaired => "bt.paired",
    BtNoRadioFooter => "bt.no_radio.footer",
    BtNoRadioEmpty => "bt.no_radio.empty",
    BtScanning => "bt.scanning",
    BtNone => "bt.none",
    BtNoneEmpty => "bt.none.empty",
    BtFound => "bt.found",
    BtScanAction => "bt.scan_action",

    ScanAgain => "scan.again",
    ScanningShort => "scanning.short",
    ErrorPrefix => "error.prefix",

    PasswordTitle => "password.title",
    PasswordConnect => "password.connect",
    PasswordConnectEmpty => "password.connect_empty",

    ScreenSleep => "screen.sleep",
    ScreenOff => "screen.off",
    ScreenOffRestart => "screen.off.restart",
    ScreenReboot => "screen.reboot",
    PowerFailTitle => "power.fail.title",
    PowerFailHold => "power.fail.hold",
    PowerFailOk => "power.fail.ok",

    ToastConnected => "toast.connected",
    ToastConnecting => "toast.connecting",
    ToastNotSaved => "toast.not_saved",
    ToastPaired => "toast.paired",
    ToastOpening => "toast.opening",
    ToastStarting => "toast.starting",
    ToastVolume => "toast.volume",
    ToastThemeSingle => "toast.theme_single",
    ToastScaleFollows => "toast.scale_follows",
    ToastLanguage => "toast.language",
    ToastUseArrows => "toast.use_arrows",
    ToastGameOptions => "toast.game_options",

    WelcomeTitle => "welcome.title",
    WelcomeLede => "welcome.lede",
    WelcomeConfirm => "welcome.confirm",
    WelcomeEnglishDefault => "welcome.english_default",
}

/// Direção de escrita.
///
/// Registrada porque é fato do idioma, não decoração: quem desenha precisa
/// saber para onde ancorar o texto no dia em que o desenho souber espelhar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

/// Idioma da interface.
///
/// `Default` é **inglês**: é o padrão comercial do produto e o que a tela de
/// boas-vindas oferece já selecionado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Idioma {
    #[default]
    En = 0,
    Zh = 1,
    Hi = 2,
    Es = 3,
    Fr = 4,
    Ar = 5,
    Bn = 6,
    Pt = 7,
    Ru = 8,
    Ur = 9,
}

impl Idioma {
    /// Os dez idiomas, na ordem do site.
    pub const TODOS: [Idioma; 10] = [
        Idioma::En,
        Idioma::Zh,
        Idioma::Hi,
        Idioma::Es,
        Idioma::Fr,
        Idioma::Ar,
        Idioma::Bn,
        Idioma::Pt,
        Idioma::Ru,
        Idioma::Ur,
    ];

    /// Idioma com que o aparelho nasce: inglês.
    pub const PADRAO: Idioma = Idioma::En;

    /// Posição na tabela de traduções. É o discriminante.
    pub const fn indice(self) -> usize {
        self as usize
    }

    /// Código curto, igual ao do site (`en`, `zh`, …). É o que vai para o
    /// arquivo `/user/.config/giochi/language`.
    pub const fn codigo(self) -> &'static str {
        match self {
            Idioma::En => "en",
            Idioma::Zh => "zh",
            Idioma::Hi => "hi",
            Idioma::Es => "es",
            Idioma::Fr => "fr",
            Idioma::Ar => "ar",
            Idioma::Bn => "bn",
            Idioma::Pt => "pt",
            Idioma::Ru => "ru",
            Idioma::Ur => "ur",
        }
    }

    /// Nome do idioma **no próprio idioma**. É o que a lista mostra: quem procura
    /// o seu idioma procura o próprio alfabeto, não "Chinese (Mandarin)".
    pub const fn nome_proprio(self) -> &'static str {
        match self {
            Idioma::En => "English",
            Idioma::Zh => "中文",
            Idioma::Hi => "हिन्दी",
            Idioma::Es => "Español",
            Idioma::Fr => "Français",
            Idioma::Ar => "العربية",
            Idioma::Bn => "বাংলা",
            Idioma::Pt => "Português",
            Idioma::Ru => "Русский",
            Idioma::Ur => "اردو",
        }
    }

    /// Nome em inglês, para log e documentação.
    pub const fn nome(self) -> &'static str {
        match self {
            Idioma::En => "English",
            Idioma::Zh => "Chinese (Mandarin)",
            Idioma::Hi => "Hindi",
            Idioma::Es => "Spanish",
            Idioma::Fr => "French",
            Idioma::Ar => "Arabic",
            Idioma::Bn => "Bengali",
            Idioma::Pt => "Portuguese",
            Idioma::Ru => "Russian",
            Idioma::Ur => "Urdu",
        }
    }

    /// Etiqueta BCP-47, a mesma do site.
    pub const fn bcp47(self) -> &'static str {
        match self {
            Idioma::En => "en",
            Idioma::Zh => "zh-CN",
            Idioma::Hi => "hi",
            Idioma::Es => "es",
            Idioma::Fr => "fr",
            Idioma::Ar => "ar",
            Idioma::Bn => "bn",
            Idioma::Pt => "pt-BR",
            Idioma::Ru => "ru",
            Idioma::Ur => "ur",
        }
    }

    pub const fn dir(self) -> Dir {
        match self {
            Idioma::Ar | Idioma::Ur => Dir::Rtl,
            _ => Dir::Ltr,
        }
    }

    /// Idioma a partir do código gravado no arquivo. Aceita o código curto e a
    /// etiqueta BCP-47 inteira (`pt-BR`), porque as duas aparecem em
    /// configuração escrita à mão.
    pub fn de_codigo(codigo: &str) -> Option<Idioma> {
        let codigo = codigo.trim().to_ascii_lowercase();
        let base = codigo.split(['-', '_']).next().unwrap_or("");
        Idioma::TODOS
            .iter()
            .copied()
            .find(|i| i.codigo() == base || i.bcp47().to_ascii_lowercase() == codigo)
    }

    pub fn proximo(self) -> Idioma {
        let i = (self.indice() + 1) % Idioma::TODOS.len();
        Idioma::TODOS[i]
    }

    pub fn anterior(self) -> Idioma {
        let i = (self.indice() + Idioma::TODOS.len() - 1) % Idioma::TODOS.len();
        Idioma::TODOS[i]
    }

    /// Amostra da escrita do idioma, para **medir** cobertura de fonte.
    ///
    /// Não é texto de interface: são caracteres que obrigatoriamente precisam
    /// de glifo na escrita deste idioma (nome próprio, dígito, letra). Serve a
    /// `--fontes` responder "este aparelho desenha este idioma?" medindo, e não
    /// deduzindo do tamanho da lista de fontes — a inferência errava justamente
    /// no caso mais comum: o português, cuja escrita o DejaVu cobre, recebia o
    /// veredito "pode sair com glifo faltando".
    pub const fn amostra(self) -> &'static str {
        match self {
            Idioma::En => "Giochi 0123",
            Idioma::Es => "Giochi añ 0123",
            Idioma::Fr => "Giochi éàç 0123",
            Idioma::Pt => "Giochi ãõç 0123",
            Idioma::Ru => "Гиочи 0123",
            Idioma::Ar => "جيوچي ٠١٢٣",
            Idioma::Ur => "گیوچی ٠١٢٣",
            Idioma::Hi => "गियोची ०१२३",
            Idioma::Bn => "গিওচি ০১২৩",
            Idioma::Zh => "吉欧奇 0123",
        }
    }

    /// Famílias de fonte que cobrem a escrita deste idioma, na ordem em que
    /// devem ser tentadas.
    ///
    /// São **nomes de arquivo**, não caminhos: onde procurar é assunto de quem
    /// desenha (`ui-sdl::draw`), que conhece as árvores do Buildroot e das
    /// distros. O núcleo só sabe qual escrita o idioma usa.
    ///
    /// A ordem importa e não é alfabética: a primeira da lista é a que desenha
    /// melhor o idioma, e as seguintes são o que costuma existir quando ela não
    /// está. `DejaVuSans.ttf` fecha todas — e é também o que aparece quando
    /// nenhuma das anteriores foi encontrada, ainda que sem os glifos.
    pub const fn fontes(self) -> &'static [&'static str] {
        match self {
            // Acentuação latina é coberta pelo DejaVu do aparelho.
            Idioma::En | Idioma::Es | Idioma::Fr | Idioma::Pt => &["DejaVuSans.ttf"],
            // Cirílico está no DejaVu; a Noto entra primeiro só por peso visual.
            Idioma::Ru => &["NotoSans-Regular.ttf", "DejaVuSans.ttf"],
            Idioma::Ar => &[
                "NotoNaskhArabic-Regular.ttf",
                "NotoSansArabic-Regular.ttf",
                "DejaVuSans.ttf",
            ],
            Idioma::Ur => &[
                "NotoNastaliqUrdu-Regular.ttf",
                "NotoNaskhArabic-Regular.ttf",
                "NotoSansArabic-Regular.ttf",
                "DejaVuSans.ttf",
            ],
            Idioma::Hi => &[
                "NotoSansDevanagari-Regular.ttf",
                "NotoSansDevanagariUI-Regular.ttf",
                "Lohit-Devanagari.ttf",
                "DejaVuSans.ttf",
            ],
            Idioma::Bn => &[
                "NotoSansBengali-Regular.ttf",
                "NotoSansBengaliUI-Regular.ttf",
                "Lohit-Bengali.ttf",
                "DejaVuSans.ttf",
            ],
            Idioma::Zh => &[
                "NotoSansCJK-Regular.ttc",
                "NotoSansCJKsc-Regular.otf",
                "NotoSansSC-Regular.otf",
                "wqy-zenhei.ttc",
                "wqy-microhei.ttc",
                "DejaVuSans.ttf",
            ],
        }
    }
}

/// Texto de uma chave no idioma pedido.
///
/// Nunca entra em pânico: uma tabela curta (erro de edição) devolve string
/// vazia em vez de derrubar o launcher — a tela fica sem um rótulo, e o teste
/// de completude acusa antes de chegar ao aparelho.
pub fn texto(idioma: Idioma, chave: Chave) -> &'static str {
    match TEXTOS.get(chave.indice()) {
        Some((_, colunas)) => colunas.get(idioma.indice()).copied().unwrap_or(""),
        None => "",
    }
}

/// Texto com um valor no lugar de `{}`.
///
/// Substituição literal em vez de `format!`: os textos traduzidos são dados, e
/// uma tradução com `{` solto viraria erro de formatação em tempo de execução.
pub fn texto_com(idioma: Idioma, chave: Chave, valor: &str) -> String {
    texto(idioma, chave).replacen("{}", valor, 1)
}

/// Uma linha por chave, dez colunas por linha — na ordem de [`Idioma::TODOS`].
///
/// Manter a tradução ao lado do original é o que permite revisar um idioma sem
/// abrir outro arquivo. A ordem das linhas tem de casar com a do enum; há teste.
type Traducao = (&'static str, [&'static str; 10]);

pub static TEXTOS: &[Traducao] = &[
    // ── abas ─────────────────────────────────────────────────
    ("tab.home", ["Home", "主页", "होम", "Inicio", "Accueil", "الرئيسية", "হোম", "Início", "Главная", "ہوم"]),
    ("tab.library", ["Games", "游戏", "गेम", "Juegos", "Jeux", "الألعاب", "গেম", "Jogos", "Игры", "گیمز"]),
    ("tab.apps", ["Apps", "应用", "ऐप्स", "Apps", "Apps", "التطبيقات", "অ্যাপ", "Apps", "Приложения", "ایپس"]),
    ("tab.settings", ["Settings", "设置", "सेटिंग्स", "Ajustes", "Réglages", "الإعدادات", "সেটিংস", "Config", "Настройки", "ترتیبات"]),

    // ── dicas do rodapé ──────────────────────────────────────
    ("hint.navigate", ["Navigate", "浏览", "नेविगेट करें", "Navegar", "Naviguer", "تنقّل", "নেভিগেট", "Navegar", "Навигация", "نیویگیٹ کریں"]),
    ("hint.select", ["Select", "选择", "चुनें", "Seleccionar", "Choisir", "اختر", "নির্বাচন", "Selecionar", "Выбрать", "منتخب کریں"]),
    ("hint.back", ["Back", "返回", "वापस", "Volver", "Retour", "رجوع", "ফিরুন", "Voltar", "Назад", "واپس"]),
    ("hint.adjust", ["Adjust", "调整", "समायोजित करें", "Ajustar", "Régler", "اضبط", "সমন্বয়", "Ajustar", "Изменить", "ایڈجسٹ کریں"]),
    ("hint.close", ["Close", "关闭", "बंद करें", "Cerrar", "Fermer", "إغلاق", "বন্ধ", "Fechar", "Закрыть", "بند کریں"]),
    ("hint.action", ["Action", "操作", "क्रिया", "Acción", "Action", "إجراء", "ক্রিয়া", "Ação", "Действие", "عمل"]),
    ("hint.confirm", ["Confirm", "确认", "पुष्टि करें", "Confirmar", "Confirmer", "تأكيد", "নিশ্চিত করুন", "Confirmar", "Подтвердить", "تصدیق کریں"]),
    ("hint.cancel", ["Cancel", "取消", "रद्द करें", "Cancelar", "Annuler", "إلغاء", "বাতিল", "Cancelar", "Отмена", "منسوخ کریں"]),
    ("hint.connect", ["Connect", "连接", "कनेक्ट करें", "Conectar", "Connecter", "اتصال", "সংযোগ", "Conectar", "Подключить", "جڑیں"]),
    ("hint.network", ["Network", "网络", "नेटवर्क", "Red", "Réseau", "شبكة", "নেটওয়ার্ক", "Rede", "Сеть", "نیٹ ورک"]),
    ("hint.type", ["Type", "输入", "लिखें", "Escribir", "Saisir", "اكتب", "লিখুন", "Digitar", "Ввод", "لکھیں"]),
    ("hint.show", ["Show", "显示", "दिखाएँ", "Mostrar", "Afficher", "أظهر", "দেখান", "Mostrar", "Показать", "دکھائیں"]),
    ("hint.pair", ["Pair", "配对", "पेयर करें", "Emparejar", "Appairer", "إقران", "পেয়ার", "Parear", "Сопрячь", "پیئر کریں"]),
    ("hint.device", ["Device", "设备", "डिवाइस", "Dispositivo", "Appareil", "جهاز", "ডিভাইস", "Aparelho", "Устройство", "ڈیوائس"]),
    ("hint.enter", ["Enter", "进入", "प्रवेश", "Entrar", "Entrer", "دخول", "প্রবেশ", "Entrar", "Войти", "داخل جائیں"]),
    ("hint.category", ["Category", "分类", "श्रेणी", "Categoría", "Catégorie", "فئة", "বিভাগ", "Categoria", "Раздел", "زمرہ"]),
    ("hint.tab", ["Tab", "标签页", "टैब", "Pestaña", "Onglet", "تبويب", "ট্যাব", "Aba", "Вкладка", "ٹیب"]),
    ("hint.quick", ["Quick", "快捷", "त्वरित", "Rápido", "Rapide", "سريع", "দ্রুত", "Ajustes", "Быстро", "فوری"]),
    ("hint.categories", ["Categories", "分类", "श्रेणियाँ", "Categorías", "Catégories", "الفئات", "বিভাগ", "Categorias", "Разделы", "زمرے"]),
    ("hint.open", ["Open", "打开", "खोलें", "Abrir", "Ouvrir", "فتح", "খুলুন", "Abrir", "Открыть", "کھولیں"]),
    ("hint.play", ["Play", "开始", "खेलें", "Jugar", "Jouer", "العب", "খেলুন", "Jogar", "Играть", "کھیلیں"]),
    ("hint.details", ["Details", "详情", "विवरण", "Detalles", "Détails", "تفاصيل", "বিবরণ", "Detalhes", "Подробнее", "تفصیلات"]),
    ("hint.search", ["Search", "搜索", "खोजें", "Buscar", "Rechercher", "بحث", "খুঁজুন", "Buscar", "Поиск", "تلاش"]),
    ("hint.filter", ["Filter", "筛选", "फ़िल्टर", "Filtro", "Filtre", "تصفية", "ফিল্টার", "Filtro", "Фильтр", "فلٹر"]),
    ("hint.choose", ["Choose", "选择", "चुनें", "Elegir", "Choisir", "اختر", "বেছে নিন", "Escolher", "Выбрать", "منتخب کریں"]),

    // ── telas vazias ─────────────────────────────────────────
    ("home.empty.title", ["No game installed.", "没有安装游戏。", "कोई गेम इंस्टॉल नहीं है।", "No hay juegos instalados.", "Aucun jeu installé.", "لا توجد ألعاب مثبتة.", "কোনো গেম ইনস্টল নেই।", "Nenhum jogo instalado.", "Игры не установлены.", "کوئی گیم انسٹال نہیں ہے۔"]),
    ("home.empty.hint", ["the launcher looks in /opt/giochi/games and /user/games", "启动器查找 /opt/giochi/games 和 /user/games", "लॉन्चर /opt/giochi/games और /user/games में देखता है", "el launcher busca en /opt/giochi/games y /user/games", "le launcher cherche dans /opt/giochi/games et /user/games", "يبحث المشغّل في ‎/opt/giochi/games و‎/user/games", "লঞ্চার /opt/giochi/games ও /user/games দেখে", "o launcher procura em /opt/giochi/games e /user/games", "лаунчер ищет в /opt/giochi/games и /user/games", "لانچر /opt/giochi/games اور /user/games دیکھتا ہے"]),
    ("home.play", ["Play", "开始游戏", "खेलें", "Jugar", "Jouer", "العب", "খেলুন", "Jogar", "Играть", "کھیلیں"]),
    ("library.title", ["LIBRARY", "游戏库", "गेम लाइब्रेरी", "BIBLIOTECA", "BIBLIOTHÈQUE", "المكتبة", "লাইব্রেরি", "BIBLIOTECA", "БИБЛИОТЕКА", "لائبریری"]),
    ("library.empty", ["No game in this category", "此分类没有游戏", "इस श्रेणी में कोई गेम नहीं", "No hay juegos en esta categoría", "Aucun jeu dans cette catégorie", "لا توجد ألعاب في هذه الفئة", "এই বিভাগে কোনো গেম নেই", "Nenhum jogo nesta categoria", "Нет игр в этой категории", "اس زمرے میں کوئی گیم نہیں"]),
    ("apps.empty.title", ["No app installed.", "没有安装应用。", "कोई ऐप इंस्टॉल नहीं है।", "No hay apps instaladas.", "Aucune app installée.", "لا توجد تطبيقات مثبتة.", "কোনো অ্যাপ ইনস্টল নেই।", "Nenhum app instalado.", "Приложения не установлены.", "کوئی ایپ انسٹال نہیں ہے۔"]),
    ("apps.empty.hint", ["the launcher looks in /opt/giochi/apps and /user/apps", "启动器查找 /opt/giochi/apps 和 /user/apps", "लॉन्चर /opt/giochi/apps और /user/apps में देखता है", "el launcher busca en /opt/giochi/apps y /user/apps", "le launcher cherche dans /opt/giochi/apps et /user/apps", "يبحث المشغّل في ‎/opt/giochi/apps و‎/user/apps", "লঞ্চার /opt/giochi/apps ও /user/apps দেখে", "o launcher procura em /opt/giochi/apps e /user/apps", "лаунчер ищет в /opt/giochi/apps и /user/apps", "لانچر /opt/giochi/apps اور /user/apps دیکھتا ہے"]),

    // ── filtros e desempenho ─────────────────────────────────
    ("filter.all", ["All", "全部", "सभी", "Todos", "Tous", "الكل", "সব", "Todos", "Все", "سب"]),
    ("filter.favorites", ["Favorites", "收藏", "पसंदीदा", "Favoritos", "Favoris", "المفضلة", "প্রিয়", "Favoritos", "Избранное", "پسندیدہ"]),
    ("filter.recent", ["Recent", "最近", "हाल के", "Recientes", "Récents", "الأحدث", "সাম্প্রতিক", "Recentes", "Недавние", "حالیہ"]),
    ("perf.eco", ["Eco", "节能", "इको", "Eco", "Éco", "اقتصادي", "ইকো", "Eco", "Эко", "ایکو"]),
    ("perf.balanced", ["Balanced", "均衡", "संतुलित", "Equilibrado", "Équilibré", "متوازن", "ভারসাম্য", "Equilibrado", "Сбалансированный", "متوازن"]),
    ("perf.boost", ["Boost", "性能", "बूस्ट", "Turbo", "Turbo", "تعزيز", "বুস্ট", "Turbo", "Турбо", "بوسٹ"]),

    // ── categorias de configuração ───────────────────────────
    ("set.appearance", ["Appearance", "外观", "रूप", "Apariencia", "Apparence", "المظهر", "চেহারা", "Aparência", "Оформление", "ظاہری شکل"]),
    ("set.sound", ["Sound", "声音", "ध्वनि", "Sonido", "Son", "الصوت", "শব্দ", "Som", "Звук", "آواز"]),
    ("set.network", ["Network & BT", "网络与蓝牙", "नेटवर्क और ब्लूटूथ", "Red y BT", "Réseau et BT", "الشبكة والبلوتوث", "নেটওয়ার্ক ও ব্লুটুথ", "Rede e BT", "Сеть и BT", "نیٹ ورک اور بلوٹوتھ"]),
    ("set.controller", ["Controller", "手柄", "कंट्रोलर", "Mando", "Manette", "ذراع التحكم", "কন্ট্রোলার", "Controle", "Геймпад", "کنٹرولر"]),
    ("set.system", ["System", "系统", "सिस्टम", "Sistema", "Système", "النظام", "সিস্টেম", "Sistema", "Система", "سسٹم"]),
    ("set.about", ["About", "关于", "परिचय", "Acerca de", "À propos", "حول", "পরিচিতি", "Sobre", "О программе", "تعارف"]),

    // ── linhas de configuração ───────────────────────────────
    ("set.theme", ["Theme", "主题", "थीम", "Tema", "Thème", "السمة", "থিম", "Tema", "Тема", "تھیم"]),
    ("set.language", ["Language", "语言", "भाषा", "Idioma", "Langue", "اللغة", "ভাষা", "Idioma", "Язык", "زبان"]),
    ("set.scale", ["Scale", "缩放", "स्केल", "Escala", "Échelle", "المقياس", "স্কেল", "Escala", "Масштаб", "پیمانہ"]),
    ("set.scale.value", ["follows the panel", "跟随屏幕", "पैनल के अनुसार", "sigue al panel", "suit l'écran", "يتبع الشاشة", "প্যানেল অনুযায়ী", "acompanha o painel", "по размеру экрана", "پینل کے مطابق"]),
    ("set.volume", ["Master volume", "主音量", "मुख्य वॉल्यूम", "Volumen general", "Volume général", "مستوى الصوت", "মাস্টার ভলিউম", "Volume geral", "Общая громкость", "مجموعی آواز"]),
    ("set.output", ["Default output", "默认输出", "डिफ़ॉल्ट आउटपुट", "Salida predeterminada", "Sortie par défaut", "الخرج الافتراضي", "ডিফল্ট আউটপুট", "Saída padrão", "Выход по умолчанию", "ڈیفالٹ آؤٹ پٹ"]),
    ("set.output.value", ["Stereo speaker", "立体声扬声器", "स्टीरियो स्पीकर", "Altavoz estéreo", "Haut-parleur stéréo", "مكبر صوت ستيريو", "স্টেরিও স্পিকার", "Alto-falante estéreo", "Стереодинамик", "سٹیریو اسپیکر"]),
    ("wifi", ["Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi", "Wi-Fi"]),
    ("bluetooth", ["Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth", "Bluetooth"]),
    ("set.no_radio", ["no radio on this device", "此设备没有无线模块", "इस डिवाइस में रेडियो नहीं है", "sin radio en este dispositivo", "pas de radio sur cet appareil", "لا يوجد راديو في هذا الجهاز", "এই ডিভাইসে রেডিও নেই", "sem rádio neste aparelho", "в этом устройстве нет радио", "اس ڈیوائس میں ریڈیو نہیں ہے"]),
    ("set.not_connected", ["not connected", "未连接", "कनेक्ट नहीं है", "sin conexión", "non connecté", "غير متصل", "সংযুক্ত নয়", "não conectado", "не подключено", "منسلک نہیں"]),
    ("set.version", ["Version", "版本", "संस्करण", "Versión", "Version", "الإصدار", "সংস্করণ", "Versão", "Версия", "ورژن"]),
    ("set.performance", ["Performance", "性能", "प्रदर्शन", "Rendimiento", "Performance", "الأداء", "পারফরম্যান্স", "Desempenho", "Производительность", "کارکردگی"]),

    // ── mapa do controle ─────────────────────────────────────
    ("ctrl.confirm", ["Confirm", "确认", "पुष्टि", "Confirmar", "Confirmer", "تأكيد", "নিশ্চিত", "Confirmar", "Подтвердить", "تصدیق"]),
    ("ctrl.back", ["Back", "返回", "वापस", "Volver", "Retour", "رجوع", "ফিরুন", "Voltar", "Назад", "واپس"]),
    ("ctrl.details", ["Details", "详情", "विवरण", "Detalles", "Détails", "تفاصيل", "বিবরণ", "Detalhes", "Подробнее", "تفصیلات"]),
    ("ctrl.search", ["Search", "搜索", "खोज", "Buscar", "Rechercher", "بحث", "খোঁজ", "Buscar", "Поиск", "تلاش"]),
    ("ctrl.switch_tab", ["Switch tab", "切换标签页", "टैब बदलें", "Cambiar pestaña", "Changer d'onglet", "تبديل التبويب", "ট্যাব বদলান", "Trocar aba", "Сменить вкладку", "ٹیب بدلیں"]),
    ("ctrl.quick", ["Quick settings", "快捷设置", "त्वरित सेटिंग्स", "Ajustes rápidos", "Réglages rapides", "إعدادات سريعة", "দ্রুত সেটিংস", "Ajustes rápidos", "Быстрые настройки", "فوری ترتیبات"]),
    ("ctrl.power", ["Power menu", "电源菜单", "पावर मेनू", "Menú de energía", "Menu d'alimentation", "قائمة الطاقة", "পাওয়ার মেনু", "Menu de energia", "Меню питания", "پاور مینو"]),

    // ── sobre ────────────────────────────────────────────────
    ("about.1", ["Giochi Launcher — SDL2 with its own renderer.", "Giochi Launcher — SDL2，自带渲染器。", "Giochi Launcher — SDL2 com renderizador próprio.", "Giochi Launcher — SDL2 con renderizador propio.", "Giochi Launcher — SDL2 avec rendu maison.", "Giochi Launcher — SDL2 برسم خاص.", "Giochi Launcher — SDL2 নিজস্ব রেন্ডারার।", "Giochi Launcher — SDL2 com renderer próprio.", "Giochi Launcher — SDL2 со своим рендерером.", "Giochi Launcher — SDL2 اپنے رینڈرر کے ساتھ۔"]),
    ("about.2", ["Interface designed for a gamepad; touch as an extra.", "界面为手柄设计，触屏为辅助。", "इंटरफ़ेस गेमपैड के लिए; स्पर्श अतिरिक्त है।", "Interfaz pensada para mando; el toque es extra.", "Interface pensée pour la manette ; le tactile est un plus.", "الواجهة مصممة لذراع التحكم، واللمس إضافة.", "ইন্টারফেস গেমপ্যাডের জন্য; স্পর্শ অতিরিক্ত।", "Interface desenhada para gamepad; toque como adicional.", "Интерфейс рассчитан на геймпад; касание — дополнение.", "انٹرفیس گیم پیڈ کے لیے؛ ٹچ اضافی ہے۔"]),
    ("about.3", ["Single state, reducer, focus by zone, overlays separate from the tab.", "单一状态、reducer、分区焦点，浮层与标签页分离。", "एकल स्टेट, रिड्यूसर, ज़ोन के अनुसार फ़ोकस, ओवरले टैब से अलग।", "Estado único, reducer, foco por zona, capas separadas de la pestaña.", "État unique, reducer, focus par zone, surcouches séparées de l'onglet.", "حالة واحدة، مخفّض، تركيز حسب المنطقة، وطبقات منفصلة عن التبويب.", "একক স্টেট, রিডিউসার, জোন অনুযায়ী ফোকাস, ওভারলে ট্যাব থেকে আলাদা।", "Estado único, reducer, foco por zona, overlays separados da aba.", "Одно состояние, редьюсер, фокус по зонам, оверлеи отдельно от вкладки.", "ایک حالت، ریڈیوسر، زون کے حساب سے فوکس، اوورلے ٹیب سے الگ۔"]),
    ("about.4", ["Where there is no sensor reading, the screen shows — instead of", "没有传感器读数时，屏幕显示 — 而不是", "जहाँ सेंसर रीडिंग नहीं है, स्क्रीन — दिखाती है, बजाय", "Donde no hay lectura de sensor, la pantalla muestra — en vez de", "Sans lecture de capteur, l'écran affiche — au lieu de", "حيث لا توجد قراءة من المستشعر، تعرض الشاشة — بدلًا من", "যেখানে সেন্সর রিডিং নেই, স্ক্রিন — দেখায়, বরং", "Onde não há leitura de sensor, a tela mostra — em vez de", "Где нет показаний датчика, экран показывает — вместо того чтобы", "جہاں سینسر ریڈنگ نہیں، اسکرین — دکھاتی ہے، بجائے"]),
    ("about.5", ["inventing a number.", "编造一个数字。", "संख्या गढ़ने के।", "inventar un número.", "d'inventer un nombre.", "اختراع رقم.", "সংখ্যা বানানোর।", "inventar um número.", "придумывать число.", "نمبر بنانے کے۔"]),

    // ── ajustes rápidos e energia ────────────────────────────
    ("quick.title", ["Quick settings", "快捷设置", "त्वरित सेटिंग्स", "Ajustes rápidos", "Réglages rapides", "إعدادات سريعة", "দ্রুত সেটিংস", "Ajustes rápidos", "Быстрые настройки", "فوری ترتیبات"]),
    ("quick.volume", ["Volume", "音量", "वॉल्यूम", "Volumen", "Volume", "الصوت", "ভলিউম", "Volume", "Громкость", "آواز"]),
    ("quick.brightness", ["Brightness", "亮度", "चमक", "Brillo", "Luminosité", "السطوع", "উজ্জ্বলতা", "Brilho", "Яркость", "روشنی"]),
    ("quick.hud", ["FPS HUD", "FPS 显示", "FPS HUD", "HUD de FPS", "HUD FPS", "عرض FPS", "FPS HUD", "HUD de FPS", "HUD FPS", "FPS HUD"]),
    ("power.title", ["Power options", "电源选项", "पावर विकल्प", "Opciones de energía", "Options d'alimentation", "خيارات الطاقة", "পাওয়ার অপশন", "Opções de energia", "Параметры питания", "پاور اختیارات"]),
    ("power.sub", ["What do you want to do?", "您想做什么？", "आप क्या करना चाहते हैं?", "¿Qué desea hacer?", "Que voulez-vous faire ?", "ماذا تريد أن تفعل؟", "আপনি কী করতে চান?", "O que deseja fazer?", "Что вы хотите сделать?", "آپ کیا کرنا چاہتے ہیں؟"]),
    ("power.suspend", ["Suspend", "休眠", "सस्पेंड", "Suspender", "Suspendre", "تعليق", "সাসপেন্ড", "Suspender", "Спящий режим", "معطل کریں"]),
    ("power.restart", ["Restart", "重启", "रीस्टार्ट", "Reiniciar", "Redémarrer", "إعادة التشغيل", "রিস্টার্ট", "Reiniciar", "Перезагрузка", "دوبارہ شروع"]),
    ("power.shutdown", ["Shut down", "关机", "शट डाउन", "Apagar", "Éteindre", "إيقاف التشغيل", "শাট ডাউন", "Desligar", "Выключить", "بند کریں"]),
    ("power.cancel", ["Cancel", "取消", "रद्द करें", "Cancelar", "Annuler", "إلغاء", "বাতিল", "Cancelar", "Отмена", "منسوخ کریں"]),

    // ── busca ────────────────────────────────────────────────
    ("search.placeholder", ["Search game...", "搜索游戏…", "गेम खोजें…", "Buscar juego...", "Rechercher un jeu...", "ابحث عن لعبة...", "গেম খুঁজুন...", "Buscar jogo...", "Поиск игры...", "گیم تلاش کریں..."]),
    ("search.empty", ["Type to search...", "输入以搜索…", "खोजने के लिए लिखें…", "Escriba para buscar...", "Saisissez pour rechercher...", "اكتب للبحث...", "খুঁজতে লিখুন...", "Digite para buscar...", "Введите для поиска...", "تلاش کے لیے لکھیں..."]),
    ("search.results", ["Results ({})", "结果 ({})", "परिणाम ({})", "Resultados ({})", "Résultats ({})", "النتائج ({})", "ফলাফল ({})", "Resultados ({})", "Результаты ({})", "نتائج ({})"]),
    ("search.space", ["SPC", "空格", "स्पेस", "ESP", "ESP", "مسافة", "স্পেস", "ESP", "ПРОБЕЛ", "اسپیس"]),

    // ── ficha do jogo ────────────────────────────────────────
    ("detail.year", ["YEAR", "年份", "वर्ष", "AÑO", "ANNÉE", "السنة", "বছর", "ANO", "ГОД", "سال"]),
    ("detail.hours", ["HOURS", "小时", "घंटे", "HORAS", "HEURES", "الساعات", "ঘণ্টা", "HORAS", "ЧАСЫ", "گھنٹے"]),
    ("detail.last", ["LAST PLAYED", "最近游玩", "अंतिम बार", "ÚLTIMA VEZ", "DERNIÈRE FOIS", "آخر مرة", "সর্বশেষ", "ÚLTIMA VEZ", "ПОСЛЕДНИЙ РАЗ", "آخری بار"]),
    ("detail.favorite", ["FAVORITE", "收藏", "पसंदीदा", "FAVORITO", "FAVORI", "المفضلة", "প্রিয়", "FAVORITO", "ИЗБРАННОЕ", "پسندیدہ"]),
    ("detail.yes", ["Yes", "是", "हाँ", "Sí", "Oui", "نعم", "হ্যাঁ", "Sim", "Да", "ہاں"]),
    ("detail.favorite_add", ["Favorite", "收藏", "पसंदीदा बनाएँ", "Favorito", "Favori", "إضافة للمفضلة", "প্রিয় করুন", "Favoritar", "В избранное", "پسندیدہ بنائیں"]),
    ("detail.favorite_remove", ["Favorited", "已收藏", "पसंदीदा", "Favoritado", "Favori ajouté", "في المفضلة", "প্রিয় করা", "Favoritado", "В избранном", "پسندیدہ"]),
    ("detail.options", ["Options", "选项", "विकल्प", "Opciones", "Options", "خيارات", "অপশন", "Opções", "Параметры", "اختیارات"]),
    ("game.never_played", ["never played", "从未游玩", "कभी नहीं खेला", "nunca jugado", "jamais joué", "لم تُلعب بعد", "কখনো খেলা হয়নি", "nunca jogado", "ещё не играли", "کبھی نہیں کھیلا"]),

    // ── Wi-Fi ────────────────────────────────────────────────
    ("wifi.connected", ["connected", "已连接", "कनेक्टेड", "conectada", "connecté", "متصلة", "সংযুক্ত", "conectada", "подключено", "منسلک"]),
    ("wifi.saved", ["saved", "已保存", "सहेजा", "guardada", "enregistré", "محفوظة", "সংরক্ষিত", "salva", "сохранено", "محفوظ"]),
    ("wifi.open", ["open", "开放", "खुला", "abierta", "ouvert", "مفتوحة", "খোলা", "aberta", "открытая", "کھلا"]),
    ("wifi.no_radio.footer", ["this device has no Wi-Fi", "此设备没有 Wi-Fi", "इस डिवाइस में Wi-Fi नहीं है", "este dispositivo no tiene Wi-Fi", "cet appareil n'a pas de Wi-Fi", "هذا الجهاز بلا واي فاي", "এই ডিভাইসে Wi-Fi নেই", "este aparelho não tem Wi-Fi", "в этом устройстве нет Wi-Fi", "اس ڈیوائس میں وائی فائی نہیں ہے"]),
    ("wifi.no_radio.empty", ["No Wi-Fi radio on this device", "此设备没有 Wi-Fi 模块", "इस डिवाइस में Wi-Fi रेडियो नहीं है", "Sin radio Wi-Fi en este dispositivo", "Aucune radio Wi-Fi sur cet appareil", "لا يوجد راديو واي فاي في هذا الجهاز", "এই ডিভাইসে Wi-Fi রেডিও নেই", "Sem rádio Wi-Fi neste aparelho", "В этом устройстве нет Wi-Fi", "اس ڈیوائس میں وائی فائی ریڈیو نہیں ہے"]),
    ("wifi.scanning", ["scanning for networks…", "正在搜索网络…", "नेटवर्क खोज रहे हैं…", "buscando redes…", "recherche de réseaux…", "جارٍ البحث عن شبكات…", "নেটওয়ার্ক খোঁজা হচ্ছে…", "procurando redes…", "поиск сетей…", "نیٹ ورک تلاش ہو رہے ہیں…"]),
    ("wifi.none", ["no network found", "未找到网络", "कोई नेटवर्क नहीं मिला", "no se encontró ninguna red", "aucun réseau trouvé", "لم يتم العثور على شبكة", "কোনো নেটওয়ার্ক পাওয়া যায়নি", "nenhuma rede encontrada", "сети не найдены", "کوئی نیٹ ورک نہیں ملا"]),
    ("wifi.none.empty", ["None — A to scan", "无 — 按 A 搜索", "कोई नहीं — खोजने के लिए A", "Ninguna — A para buscar", "Aucun — A pour chercher", "لا شيء — A للبحث", "কিছু নেই — খুঁজতে A", "Nenhuma — A para procurar", "Нет — A для поиска", "کوئی نہیں — تلاش کے لیے A"]),
    ("wifi.found", ["{} network(s) · A connects", "{} 个网络 · A 连接", "{} नेटवर्क · A कनेक्ट करता है", "{} red(es) · A conecta", "{} réseau(x) · A connecte", "{} شبكة · A للاتصال", "{} নেটওয়ার্ক · A সংযোগ করে", "{} rede(s) · A conecta", "{} сет(ей) · A подключает", "{} نیٹ ورک · A جوڑتا ہے"]),
    ("wifi.scan_action", ["Scan networks", "搜索网络", "नेटवर्क खोजें", "Buscar redes", "Chercher des réseaux", "ابحث عن شبكات", "নেটওয়ার্ক খুঁজুন", "Procurar redes", "Искать сети", "نیٹ ورک تلاش کریں"]),

    // ── Bluetooth ────────────────────────────────────────────
    ("bt.connected", ["connected", "已连接", "कनेक्टेड", "conectado", "connecté", "متصل", "সংযুক্ত", "conectado", "подключено", "منسلک"]),
    ("bt.paired", ["paired", "已配对", "पेयर किया", "emparejado", "appairé", "مقترن", "পেয়ার করা", "pareado", "сопряжено", "پیئر شدہ"]),
    ("bt.no_radio.footer", ["this device has no Bluetooth", "此设备没有蓝牙", "इस डिवाइस में ब्लूटूथ नहीं है", "este dispositivo no tiene Bluetooth", "cet appareil n'a pas de Bluetooth", "هذا الجهاز بلا بلوتوث", "এই ডিভাইসে ব্লুটুথ নেই", "este aparelho não tem Bluetooth", "в этом устройстве нет Bluetooth", "اس ڈیوائس میں بلوٹوتھ نہیں ہے"]),
    ("bt.no_radio.empty", ["No Bluetooth radio on this device", "此设备没有蓝牙模块", "इस डिवाइस में ब्लूटूथ रेडियो नहीं है", "Sin radio Bluetooth en este dispositivo", "Aucune radio Bluetooth sur cet appareil", "لا يوجد راديو بلوتوث في هذا الجهاز", "এই ডিভাইসে ব্লুটুথ রেডিও নেই", "Sem rádio Bluetooth neste aparelho", "В этом устройстве нет Bluetooth", "اس ڈیوائس میں بلوٹوتھ ریڈیو نہیں ہے"]),
    ("bt.scanning", ["scanning for devices…", "正在搜索设备…", "डिवाइस खोज रहे हैं…", "buscando dispositivos…", "recherche d'appareils…", "جارٍ البحث عن أجهزة…", "ডিভাইস খোঁজা হচ্ছে…", "procurando aparelhos…", "поиск устройств…", "ڈیوائس تلاش ہو رہی ہیں…"]),
    ("bt.none", ["no device", "没有设备", "कोई डिवाइस नहीं", "ningún dispositivo", "aucun appareil", "لا يوجد جهاز", "কোনো ডিভাইস নেই", "nenhum aparelho", "нет устройств", "کوئی ڈیوائس نہیں"]),
    ("bt.none.empty", ["None — A to scan", "无 — 按 A 搜索", "कोई नहीं — खोजने के लिए A", "Ninguno — A para buscar", "Aucun — A pour chercher", "لا شيء — A للبحث", "কিছু নেই — খুঁজতে A", "Nenhum — A para procurar", "Нет — A для поиска", "کوئی نہیں — تلاش کے لیے A"]),
    ("bt.found", ["{} device(s) · A pairs", "{} 个设备 · A 配对", "{} डिवाइस · A पेयर करता है", "{} dispositivo(s) · A empareja", "{} appareil(s) · A appaire", "{} جهاز · A للإقران", "{} ডিভাইস · A পেয়ার করে", "{} aparelho(s) · A pareia", "{} устройств · A сопрягает", "{} ڈیوائس · A پیئر کرتا ہے"]),
    ("bt.scan_action", ["Scan devices", "搜索设备", "डिवाइस खोजें", "Buscar dispositivos", "Chercher des appareils", "ابحث عن أجهزة", "ডিভাইস খুঁজুন", "Procurar aparelhos", "Искать устройства", "ڈیوائس تلاش کریں"]),

    ("scan.again", ["Scan again", "重新搜索", "फिर खोजें", "Buscar de nuevo", "Chercher encore", "ابحث مرة أخرى", "আবার খুঁজুন", "Procurar de novo", "Искать снова", "دوبارہ تلاش کریں"]),
    ("scanning.short", ["Scanning…", "搜索中…", "खोज रहे हैं…", "Buscando…", "Recherche…", "جارٍ البحث…", "খোঁজা হচ্ছে…", "Procurando…", "Поиск…", "تلاش جاری…"]),
    ("error.prefix", ["error: {}", "错误：{}", "त्रुटि: {}", "error: {}", "erreur : {}", "خطأ: {}", "ত্রুটি: {}", "erro: {}", "ошибка: {}", "خرابی: {}"]),

    // ── senha do Wi-Fi ───────────────────────────────────────
    ("password.title", ["Password for {}", "{} 的密码", "{} का पासवर्ड", "Contraseña de {}", "Mot de passe de {}", "كلمة سر {}", "{} এর পাসওয়ার্ড", "Senha de {}", "Пароль для {}", "{} کا پاس ورڈ"]),
    ("password.connect", ["Connect", "连接", "कनेक्ट करें", "Conectar", "Connecter", "اتصال", "সংযোগ", "Conectar", "Подключить", "جڑیں"]),
    ("password.connect_empty", ["Connect (type the password)", "连接（请输入密码）", "कनेक्ट करें (पासवर्ड लिखें)", "Conectar (escriba la contraseña)", "Connecter (saisissez le mot de passe)", "اتصال (اكتب كلمة السر)", "সংযোগ (পাসওয়ার্ড লিখুন)", "Conectar (digite a senha)", "Подключить (введите пароль)", "جڑیں (پاس ورڈ لکھیں)"]),

    // ── telas de estado ──────────────────────────────────────
    ("screen.sleep", ["Touch or press any button to wake.", "触摸或按任意键唤醒。", "जगाने के लिए स्पर्श करें या कोई बटन दबाएँ।", "Toque o pulse cualquier botón para despertar.", "Touchez ou appuyez sur un bouton pour réveiller.", "المس أو اضغط أي زر للإيقاظ.", "জাগাতে স্পর্শ করুন বা যেকোনো বোতাম চাপুন।", "Toque ou pressione qualquer botão para acordar.", "Коснитесь или нажмите любую кнопку.", "جگانے کے لیے چھوئیں یا کوئی بٹن دبائیں۔"]),
    ("screen.off", ["System is off.", "系统已关机。", "सिस्टम बंद है।", "Sistema apagado.", "Système éteint.", "النظام مغلق.", "সিস্টেম বন্ধ।", "Sistema desligado.", "Система выключена.", "سسٹم بند ہے۔"]),
    ("screen.off.restart", ["Press A to restart.", "按 A 重新启动。", "रीस्टार्ट के लिए A दबाएँ।", "Pulse A para reiniciar.", "Appuyez sur A pour redémarrer.", "اضغط A لإعادة التشغيل.", "রিস্টার্ট করতে A চাপুন।", "Pressione A para reiniciar.", "Нажмите A для перезагрузки.", "دوبارہ شروع کرنے کے لیے A دبائیں۔"]),
    ("screen.reboot", ["Restarting...", "正在重启…", "रीस्टार्ट हो रहा है…", "Reiniciando...", "Redémarrage...", "جارٍ إعادة التشغيل...", "রিস্টার্ট হচ্ছে...", "Reiniciando...", "Перезагрузка...", "دوبارہ شروع ہو رہا ہے..."]),
    ("power.fail.title", ["Could not power off the device", "无法关闭设备", "डिवाइस बंद नहीं हो सका", "No se pudo apagar el dispositivo", "Impossible d'éteindre l'appareil", "تعذّر إيقاف الجهاز", "ডিভাইস বন্ধ করা যায়নি", "Não foi possível desligar o aparelho", "Не удалось выключить устройство", "ڈیوائس بند نہیں ہو سکی"]),
    ("power.fail.hold", ["Hold the power button for 8 s to force it.", "长按电源键 8 秒强制关机。", "ज़बरदस्ती के लिए पावर बटन 8 सेकंड दबाए रखें।", "Mantenga el botón de energía 8 s para forzar.", "Maintenez le bouton d'alimentation 8 s pour forcer.", "اضغط زر الطاقة 8 ثوانٍ للإجبار.", "জোর করতে পাওয়ার বোতাম ৮ সেকেন্ড চেপে ধরুন।", "Segure o botão de energia por 8 s para forçar.", "Удерживайте кнопку питания 8 с.", "زبردستی کے لیے پاور بٹن 8 سیکنڈ دبائے رکھیں۔"]),
    ("power.fail.ok", ["When it works, the system turns off with the backlight off.", "正常时，系统关机并关闭背光。", "काम करने पर सिस्टम बैकलाइट बंद करके बंद हो जाता है।", "Cuando funciona, el sistema se apaga con la retroiluminación apagada.", "Quand cela marche, le système s'éteint rétroéclairage éteint.", "عندما يعمل، يُطفأ النظام مع إطفاء الإضاءة الخلفية.", "কাজ করলে সিস্টেম ব্যাকলাইট বন্ধ করে শাটডাউন হয়।", "Quando funciona, o sistema desliga com o brilho apagado.", "Когда работает, система выключается с погасшей подсветкой.", "جب کام کرتا ہے، سسٹم بیک لائٹ بند کر کے بند ہوتا ہے۔"]),

    // ── mensagens curtas (toast) ─────────────────────────────
    ("toast.connected", ["Connected", "已连接", "कनेक्ट हो गया", "Conectado", "Connecté", "تم الاتصال", "সংযুক্ত হয়েছে", "Conectado", "Подключено", "منسلک ہو گیا"]),
    ("toast.connecting", ["Connecting to {}...", "正在连接 {}…", "{} से कनेक्ट हो रहे हैं…", "Conectando a {}...", "Connexion à {}...", "جارٍ الاتصال بـ {}...", "{} এ সংযোগ হচ্ছে...", "Conectando a {}...", "Подключение к {}...", "{} سے جڑ رہے ہیں..."]),
    ("toast.not_saved", ["Connected to {} — the network was not saved: it will ask for the password again", "已连接 {} — 网络未保存：下次需要重新输入密码", "{} से कनेक्ट — नेटवर्क सहेजा नहीं गया: फिर पासवर्ड माँगेगा", "Conectado a {} — la red no se guardó: pedirá la contraseña otra vez", "Connecté à {} — réseau non enregistré : le mot de passe sera redemandé", "تم الاتصال بـ {} — لم تُحفظ الشبكة: سيطلب كلمة السر مجددًا", "{} এ সংযুক্ত — নেটওয়ার্ক সংরক্ষিত হয়নি: আবার পাসওয়ার্ড চাইবে", "Conectado a {} — a rede não foi salva: vai pedir a senha de novo", "Подключено к {} — сеть не сохранена: пароль спросят снова", "{} سے منسلک — نیٹ ورک محفوظ نہیں ہوا: دوبارہ پاس ورڈ مانگے گا"]),
    ("toast.paired", ["Paired", "已配对", "पेयर हो गया", "Emparejado", "Appairé", "تم الإقران", "পেয়ার হয়েছে", "Pareado", "Сопряжено", "پیئر ہو گیا"]),
    ("toast.opening", ["Opening {}...", "正在打开 {}…", "{} खोल रहे हैं…", "Abriendo {}...", "Ouverture de {}...", "جارٍ فتح {}...", "{} খোলা হচ্ছে...", "Abrindo {}...", "Открываем {}...", "{} کھل رہا ہے..."]),
    ("toast.starting", ["Starting {}...", "正在启动 {}…", "{} शुरू हो रहा है…", "Iniciando {}...", "Lancement de {}...", "جارٍ تشغيل {}...", "{} চালু হচ্ছে...", "Iniciando {}...", "Запускаем {}...", "{} شروع ہو رہا ہے..."]),
    ("toast.volume", ["Volume: {}% — use ← →", "音量：{}% — 使用 ← →", "वॉल्यूम: {}% — ← → से बदलें", "Volumen: {}% — use ← →", "Volume : {}% — utilisez ← →", "الصوت: {}% — استخدم ← →", "ভলিউম: {}% — ← → ব্যবহার করুন", "Volume: {}% — use ← →", "Громкость: {}% — используйте ← →", "آواز: {}% — ← → استعمال کریں"]),
    ("toast.theme_single", ["Single theme for now", "目前只有一个主题", "अभी एक ही थीम है", "Un solo tema por ahora", "Un seul thème pour l'instant", "سمة واحدة حاليًا", "এখন একটিই থিম", "Tema único por enquanto", "Пока одна тема", "ابھی ایک ہی تھیم ہے"]),
    ("toast.scale_follows", ["The scale follows the panel", "缩放跟随屏幕", "स्केल पैनल के अनुसार है", "La escala sigue al panel", "L'échelle suit l'écran", "المقياس يتبع الشاشة", "স্কেল প্যানেল অনুযায়ী", "A escala acompanha o painel", "Масштаб следует за экраном", "پیمانہ پینل کے مطابق ہے"]),
    ("toast.language", ["Language: {}", "语言：{}", "भाषा: {}", "Idioma: {}", "Langue : {}", "اللغة: {}", "ভাষা: {}", "Idioma: {}", "Язык: {}", "زبان: {}"]),
    ("toast.use_arrows", ["Use ← → to adjust", "使用 ← → 调整", "समायोजित करने के लिए ← →", "Use ← → para ajustar", "Utilisez ← → pour régler", "استخدم ← → للضبط", "সমন্বয় করতে ← → ব্যবহার করুন", "Use ← → para ajustar", "Используйте ← → для изменения", "ایڈجسٹ کرنے کے لیے ← → استعمال کریں"]),
    ("toast.game_options", ["Game options", "游戏选项", "गेम विकल्प", "Opciones del juego", "Options du jeu", "خيارات اللعبة", "গেমের অপশন", "Opções do jogo", "Параметры игры", "گیم کے اختیارات"]),

    // ── boas-vindas ──────────────────────────────────────────
    ("welcome.title", ["Welcome to Giochi", "欢迎使用 Giochi", "Giochi में आपका स्वागत है", "Bienvenido a Giochi", "Bienvenue dans Giochi", "مرحبًا بك في Giochi", "Giochi-তে স্বাগতম", "Bem-vindo ao Giochi", "Добро пожаловать в Giochi", "Giochi میں خوش آمدید"]),
    ("welcome.lede", ["Choose your language. You can change it later in Settings › Appearance.", "选择您的语言。之后可在“设置 › 外观”中更改。", "अपनी भाषा चुनें। इसे बाद में सेटिंग्स › रूप में बदल सकते हैं।", "Elija su idioma. Puede cambiarlo luego en Ajustes › Apariencia.", "Choisissez votre langue. Modifiable ensuite dans Réglages › Apparence.", "اختر لغتك. يمكنك تغييرها لاحقًا في الإعدادات › المظهر.", "আপনার ভাষা বেছে নিন। পরে সেটিংস › চেহারায় বদলাতে পারবেন।", "Escolha o seu idioma. Pode mudá-lo depois em Config › Aparência.", "Выберите язык. Позже его можно сменить в Настройки › Оформление.", "اپنی زبان منتخب کریں۔ بعد میں ترتیبات › ظاہری شکل میں بدل سکتے ہیں۔"]),
    ("welcome.confirm", ["Continue", "继续", "जारी रखें", "Continuar", "Continuer", "متابعة", "চালিয়ে যান", "Continuar", "Продолжить", "جاری رکھیں"]),
    ("welcome.english_default", ["Starts in English", "以英语开始", "अंग्रेज़ी में शुरू", "Empieza en inglés", "Démarre en anglais", "يبدأ بالإنجليزية", "ইংরেজিতে শুরু", "Começa em inglês", "Начинается на английском", "انگریزی میں شروع"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A linha `i` da tabela tem de ser a chave `i` do enum.
    ///
    /// A consulta é por índice (`Chave::indice()`), então uma linha inserida
    /// fora de ordem traduziria a frase errada **sem erro em lugar nenhum**:
    /// `Ajustes` apareceria onde devia estar `Idioma`. Este teste é o que
    /// transforma essa classe de defeito em build vermelho.
    #[test]
    fn toda_chave_da_tabela_bate_com_o_enum() {
        assert_eq!(
            TEXTOS.len(),
            Chave::TODAS.len(),
            "a tabela e o enum precisam ter o mesmo número de chaves"
        );
        for (i, chave) in Chave::TODAS.iter().enumerate() {
            assert_eq!(
                TEXTOS[i].0,
                chave.id(),
                "linha {i} da tabela é {:?} e a chave {i} do enum é {:?}",
                TEXTOS[i].0,
                chave.id()
            );
        }
    }

    /// Nenhuma tradução vazia, em nenhum dos dez idiomas.
    ///
    /// Um idioma em branco na tela não parece "faltou tradução": parece um
    /// rótulo que não carregou, e o usuário conclui que o app está quebrado.
    #[test]
    fn nenhum_texto_vazio() {
        for chave in Chave::TODAS {
            for idioma in Idioma::TODOS {
                let t = texto(idioma, *chave);
                assert!(
                    !t.trim().is_empty(),
                    "{} está vazio em {} ({})",
                    chave.id(),
                    idioma.nome(),
                    idioma.codigo()
                );
            }
        }
    }

    /// Os dez idiomas do site, na mesma ordem e com os mesmos códigos.
    #[test]
    fn os_dez_idiomas_sao_os_do_site() {
        let codigos: Vec<&str> = Idioma::TODOS.iter().map(|i| i.codigo()).collect();
        assert_eq!(
            codigos,
            vec!["en", "zh", "hi", "es", "fr", "ar", "bn", "pt", "ru", "ur"]
        );
        // O padrão do produto é inglês, e não o idioma de quem escreveu o app.
        assert_eq!(Idioma::default(), Idioma::En);
        assert_eq!(Idioma::PADRAO, Idioma::En);
    }

    /// O código do arquivo volta ao idioma certo, inclusive escrito à mão.
    #[test]
    fn codigo_do_arquivo_encontra_o_idioma() {
        assert_eq!(Idioma::de_codigo("pt"), Some(Idioma::Pt));
        assert_eq!(Idioma::de_codigo("pt-BR"), Some(Idioma::Pt));
        assert_eq!(Idioma::de_codigo(" PT_br "), Some(Idioma::Pt));
        assert_eq!(Idioma::de_codigo("zh-CN"), Some(Idioma::Zh));
        assert_eq!(Idioma::de_codigo("ur"), Some(Idioma::Ur));
        // Desconhecido não vira "o padrão" em silêncio: quem chama decide.
        assert_eq!(Idioma::de_codigo("tlh"), None);
        assert_eq!(Idioma::de_codigo(""), None);
    }

    /// `proximo`/`anterior` percorrem os dez e dão a volta.
    #[test]
    fn ciclo_dos_idiomas_da_a_volta_nos_dois_sentidos() {
        let mut i = Idioma::En;
        for _ in 0..Idioma::TODOS.len() {
            i = i.proximo();
        }
        assert_eq!(i, Idioma::En, "dez passos para a frente voltam ao começo");
        assert_eq!(Idioma::En.anterior(), Idioma::Ur, "para trás dá a volta");
        assert_eq!(Idioma::Ur.proximo(), Idioma::En);
    }

    /// Árabe e urdu são RTL — e o resto não é.
    #[test]
    fn rtl_esta_declarado() {
        assert_eq!(Idioma::Ar.dir(), Dir::Rtl);
        assert_eq!(Idioma::Ur.dir(), Dir::Rtl);
        for i in [Idioma::En, Idioma::Zh, Idioma::Hi, Idioma::Es, Idioma::Fr] {
            assert_eq!(i.dir(), Dir::Ltr);
        }
    }

    /// A amostra de cada idioma tem caracteres **próprios** da escrita, não só
    /// latinos.
    ///
    /// Se a amostra do chinês fosse "Giochi", o DejaVu a cobriria e `--fontes`
    /// diria "tem fonte" num aparelho que desenha quadradinhos. A regra que o
    /// teste afirma: os idiomas de escrita não-latina têm, na amostra, pelo
    /// menos um caractere fora do ASCII latino.
    #[test]
    fn a_amostra_usa_a_escrita_do_idioma() {
        for idioma in Idioma::TODOS {
            let amostra = idioma.amostra();
            assert!(!amostra.is_empty(), "{} sem amostra", idioma.nome());
            // Só o inglês pode ter amostra puramente ASCII: os outros três
            // latinos precisam do acento (é o que o DejaVu cobre além do ASCII,
            // e é o que se quer verificar).
            if idioma == Idioma::En {
                assert!(amostra.is_ascii(), "o inglês é a referência ASCII");
            } else {
                assert!(
                    !amostra.is_ascii() || idioma.fontes().len() > 1,
                    "{}: amostra só ASCII sem declarar fonte própria da escrita",
                    idioma.nome()
                );
            }
            // As latinas são ASCII+acento; as outras têm de ter algo fora do
            // latino, senão a medição não mede nada.
            if !matches!(idioma, Idioma::En | Idioma::Es | Idioma::Fr | Idioma::Pt) {
                assert!(
                    amostra.chars().any(|c| !c.is_ascii() && !('\u{80}'..='\u{24F}').contains(&c)),
                    "{}: amostra sem caractere da escrita própria ({amostra:?})",
                    idioma.nome()
                );
            }
        }
    }

    /// Todo idioma precisa de pelo menos uma fonte candidata, e o DejaVu fecha
    /// todas as listas — é o arquivo que o aparelho tem hoje.
    #[test]
    fn todo_idioma_indica_fonte() {
        for idioma in Idioma::TODOS {
            assert!(!idioma.fontes().is_empty(), "{} sem fonte", idioma.nome());
            assert_eq!(
                *idioma.fontes().last().unwrap(),
                "DejaVuSans.ttf",
                "a última fonte de {} tem de ser a que o aparelho sempre tem",
                idioma.nome()
            );
        }
        // Chinês, hindi e bengali não têm glifo no DejaVu: precisam de outra
        // fonte indicada, senão o idioma aparece como quadradinhos.
        for idioma in [Idioma::Zh, Idioma::Hi, Idioma::Bn] {
            assert!(
                idioma.fontes().len() > 1,
                "{} só aponta o DejaVu, que não cobre a escrita",
                idioma.nome()
            );
        }
    }

    /// `texto_com` troca o marcador e não engasga com chave de outra coluna.
    #[test]
    fn texto_com_substitui_o_marcador() {
        assert_eq!(
            texto_com(Idioma::En, Chave::WifiFound, "3"),
            "3 network(s) · A connects"
        );
        // Português usa o mesmo marcador, com o texto dele.
        assert_eq!(
            texto_com(Idioma::Pt, Chave::ToastLanguage, "English"),
            "Idioma: English"
        );
        // Chave sem marcador devolve o texto intacto.
        assert_eq!(texto_com(Idioma::En, Chave::HintBack, "x"), "Back");
    }

    /// O inglês é o texto de referência: as chaves curtas precisam caber no
    /// rodapé, e a régua é a mesma do desenho (nada acima de 18 caracteres).
    #[test]
    fn dicas_do_rodape_sao_curtas() {
        for codigo in ["en", "pt", "fr"] {
            let idioma = Idioma::de_codigo(codigo).unwrap();
            for chave in [
                Chave::HintNavigate,
                Chave::HintSelect,
                Chave::HintBack,
                Chave::HintAdjust,
                Chave::HintClose,
                Chave::HintConfirm,
                Chave::HintCancel,
                Chave::HintConnect,
                Chave::HintType,
                Chave::HintShow,
                Chave::HintPair,
                Chave::HintEnter,
                Chave::HintTab,
                Chave::HintOpen,
                Chave::HintPlay,
                Chave::HintDetails,
                Chave::HintSearch,
                Chave::HintFilter,
            ] {
                let t = texto(idioma, chave);
                // O rodapé trunca, mas uma dica longa empurra as outras para
                // fora da tela — o usuário perde a tecla que precisa.
                assert!(
                    t.chars().count() <= 18,
                    "{} em {} tem {} caracteres: {t:?}",
                    chave.id(),
                    idioma.nome(),
                    t.chars().count()
                );
            }
        }
    }
}
