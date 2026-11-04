//! Laço do aplicativo: eventos → comandos → estado → desenho.
//!
//! A regra que mantém isso simples: **a UI não decide nada**. Ela traduz
//! entrada em `Cmd`, chama o reducer e desenha o resultado. Toda a lógica
//! (navegação, pilha, permissões) vive no crate `core`, que é testável sem
//! placa.

use giochi_launcher_core::{
    Cmd, Dir, GamepadFrame, Mapper, Screen, State, Tab, Zone,
};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::backlight::Backlight;
use crate::draw::Painter;
use crate::power::{PowerAction, PowerWatch};
use crate::system::{Acao, TentativaUnica};
use crate::volume::{abrir_mixer, MixerBox, Repeticao, Volume};
use crate::input_evdev::EvdevInput;
use crate::theme;
use crate::views::{self, Hit, Layout};

/// Quadros de boot antes de entrar na interface (~1 s a 60 Hz).
const BOOT_FRAMES: u32 = 60;

pub struct App {
    pub state: State,
    mapper: Mapper,
    painter: Painter,
    layout: Layout,
    /// Entrada por evdev. O SDL é usado só para desenhar — ver `input_evdev`.
    evdev: EvdevInput,
    window_size: (u32, u32),
    boot_frames: u32,
    start_was_down: bool,
    fps_frames: u32,
    fps_elapsed: f32,
    /// Uma vez que a fonte falha, não repete o aviso a cada quadro.
    font_warned: bool,
    /// Só redesenha quando algo mudou.
    ///
    /// A UI é praticamente estática: sem animação, dois quadros seguidos são
    /// idênticos. Redesenhar 60 vezes por segundo uma tela que não mudou é gasto
    /// puro de CPU e bateria num portátil.
    ///
    /// Só redesenhar quando algo muda é mais barato que cachear textura: o
    /// catálogo é pequeno, mas o custo de recriar cada texto por quadro aparece
    /// no RK3326. Como não há animação, dois quadros seguidos são idênticos.
    dirty: bool,
    /// Tempo restante do aviso na tela, em segundos.
    toast_timer: f32,
    /// Último minuto desenhado, para atualizar o relógio.
    last_minute: u64,
    /// Tempo restante até reconferir o arquivo de rotação.
    rotation_poll: f32,
    /// Backlight: apagar ao dormir, restaurar ao acordar.
    backlight: Backlight,
    /// Botão de energia físico.
    power: PowerWatch,
    /// Volume real do mixer (não só o número da UI).
    volume: Volume<MixerBox>,
    /// Repetição das teclas de volume enquanto seguradas.
    rep_cima: Repeticao,
    rep_baixo: Repeticao,
    /// Mensagem da última tentativa de desligar/reiniciar que falhou.
    aviso_sistema: Option<String>,
    /// Evita reexecutar `poweroff`/`reboot` a cada quadro quando falham.
    tentativa_desligar: TentativaUnica,
}

/// Duração do aviso, igual ao protótipo.
const TOAST_SECONDS: f32 = 2.2;

impl App {
    /// Grava o último quadro em BMP. Ver `Painter::save_frame`.
    pub fn save_frame(&self, path: &str) -> Result<(), String> {
        self.painter.save_frame(path)
    }

    pub fn new(painter: Painter, mut state: State, evdev: EvdevInput) -> Self {
        // O `giochi-init` já restaurou o brilho salvo no boot. Ler o hardware
        // evita a UI mostrar um número que não corresponde à tela — e evita o
        // app sobrescrever a escolha do usuário com um padrão de código.
        //
        // Fica fora do literal da struct para poder mexer em `state` antes dele
        // ser movido, e também porque a ordem importa: ler primeiro, montar
        // depois.
        let backlight = Backlight::discover();
        if let Some(pct) = backlight.fracao_atual() {
            state.settings.brightness = pct;
        }

        // Volume: o sistema não guarda estado ALSA (não há `alsactl` no init),
        // então a escolha do usuário vive em /user/.config/giochi/volume e é
        // reaplicada aqui. Ler o hardware primeiro e o arquivo depois garante
        // que a intenção do usuário vence o padrão do codec.
        let mut volume = Volume::novo(abrir_mixer());
        if volume.disponivel() {
            volume.restaurar_salvo();
        }
        state.settings.volume = volume.atual();
        eprintln!(
            "volume: {}% ({})",
            volume.atual(),
            if volume.disponivel() {
                "mixer ALSA encontrado"
            } else {
                "sem mixer; a UI funciona sem som"
            }
        );

        Self {
            state,
            mapper: Mapper::new(),
            painter,
            layout: Layout::default(),
            evdev,
            window_size: (1280, 720),
            boot_frames: 0,
            start_was_down: false,
            fps_frames: 0,
            fps_elapsed: 0.0,
            font_warned: false,
            dirty: true,
            toast_timer: 0.0,
            last_minute: 0,
            rotation_poll: 0.0,
            backlight,
            power: PowerWatch::novo(),
            volume,
            rep_cima: Repeticao::novo(),
            rep_baixo: Repeticao::novo(),
            aviso_sistema: None,
            tentativa_desligar: TentativaUnica::nova(),
        }
    }

    /// Mostra um aviso na tela a partir da camada de UI.
    ///
    /// Existe para o retorno de um jogo: quando o processo não pôde ser
    /// iniciado, o motivo precisa aparecer — senão o usuário aperta "Jogar",
    /// volta para a Home sem explicação e conclui que o botão está quebrado.
    ///
    /// O temporizador é do `App`, não do estado (ver `tick_toast`), então
    /// precisa ser reiniciado aqui também.
    pub fn avisar(&mut self, mensagem: &str) {
        self.state.toast = giochi_launcher_core::Toast::show(mensagem);
        self.toast_timer = TOAST_SECONDS;
        self.dirty = true;
    }

    /// Pede o estado de volta e **libera a tela**.
    ///
    /// Consome o `App`, então o `Painter` é destruído no fim da função — e com
    /// ele o `WindowCanvas`, que é quem segura o DRM master. É isso que permite
    /// o jogo abrir o KMSDRM: medido no X35H, um segundo processo enquanto o
    /// launcher vive recebe `kmsdrm not available`.
    ///
    /// Também é por isso que esta função existe em vez de um método que só
    /// pausa o desenho: não há como "soltar" o master sem fechar o dispositivo.
    pub fn encerrar(self) -> State {
        // `..` descarta o resto: mapper (evita tecla presa ao voltar), layout,
        // medidores de FPS e o watch de energia.
        let Self { state, .. } = self;
        state
    }

    /// Consome o pedido de lançamento, se houver.
    ///
    /// `take()` e não `clone()`: o pedido vale uma vez. Sem isso o launcher
    /// tentaria iniciar o jogo em todo quadro e nunca mais desenharia nada.
    pub fn tomar_pedido_de_lancamento(&mut self) -> Option<u32> {
        self.state.lancar.take()
    }

    fn dispatch(&mut self, cmd: Cmd) {
        // Rastro de entrada para diagnóstico em campo.
        //
        // Existe porque **não há como ver a tela de fora**: com KMSDRM o
        // `/dev/fb0` não é o buffer que está sendo varrido (devolve lixo), e
        // `--dump-frame` abre um processo novo, que sempre desenha o estado
        // inicial — nenhum dos dois mostra a instância viva.
        //
        // Com `GIOCHI_DEBUG_CMDS=1` cada comando vira uma linha no log, então
        // "apertei e nada aconteceu" deixa de ser adivinhação: se o comando
        // aparece, o app reagiu e o problema é de desenho; se não aparece, o
        // problema é antes do app (mapa, código, driver).
        if std::env::var("GIOCHI_DEBUG_CMDS").is_ok() {
            eprintln!("cmd: {cmd:?}  tela={:?}", self.state.screen);
        }

        let tela_antes = self.state.screen;
        let brilho_antes = self.state.settings.brightness;
        let volume_antes = self.state.settings.volume;
        let before = self.state.clone();
        self.state = giochi_launcher_core::update(self.state.clone(), cmd);
        if self.state != before {
            self.dirty = true;
        }
        self.sync_backlight(tela_antes, self.state.screen);
        self.sync_brilho(brilho_antes, self.state.settings.brightness);
        self.sync_volume(volume_antes, self.state.settings.volume);
        self.sync_sistema(tela_antes, self.state.screen);
    }

    /// Desenha uma vez antes de executar o comando do sistema.
    ///
    /// `poweroff` bloqueia até o sistema parar. Sem este quadro, a tela ficaria
    /// congelada no menu de energia durante todo o desligamento — o usuário
    /// acharia que travou. Aqui ele lê o que está acontecendo.
    fn avisar_antes_de_executar(&mut self, mensagem: &str) {
        self.painter.clear(theme::BG);
        views::mensagem_central(&mut self.painter, mensagem);
        self.painter.present();
        eprintln!("sistema: {mensagem}");
    }

    /// Executa o desligar/reiniciar de verdade.
    ///
    /// O núcleo só troca a tela: ele é puro e precisa continuar testável sem
    /// hardware. Quem chama `poweroff` é esta camada.
    ///
    /// Se o comando **falhar**, a mensagem vai para a tela — um aparelho que
    /// mostra "Sistema desligado" e continua ligado é pior que um que diz o que
    /// deu errado.
    ///
    /// Esta documentação já afirmou que "no X55 acontece, porque o PMIC não
    /// declara `system-power-controller`". **É falso e foi medido:** o `poweroff`
    /// do X55 corta a energia (03/11/2026). O tratamento de falha continua
    /// correto e necessário; o que saiu foi a causa inventada.
    fn sync_sistema(&mut self, antes: Screen, agora: Screen) {
        // Qualquer saída das telas de sistema libera nova tentativa. Sem isso,
        // depois de uma falha o usuário não conseguiria pedir de novo.
        if !matches!(agora, Screen::Shutdown | Screen::Reboot) {
            self.tentativa_desligar.resetar();
            self.aviso_sistema = None;
        }

        match agora {
            Screen::Shutdown if antes != Screen::Shutdown => {
                if self.tentativa_desligar.primeira_vez() {
                    self.avisar_antes_de_executar("Sistema desligado.");
                    self.aviso_sistema = Some(crate::system::executar(Acao::Desligar));
                    self.dirty = true;
                }
            }
            Screen::Reboot if antes != Screen::Reboot => {
                if self.tentativa_desligar.primeira_vez() {
                    self.avisar_antes_de_executar("Reiniciando...");
                    self.aviso_sistema = Some(crate::system::executar(Acao::Reiniciar));
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    /// Leva o volume da UI ao mixer.
    ///
    /// Antes disto o volume era só um número: mudar na tela não mudava o som.
    fn sync_volume(&mut self, antes: u8, agora: u8) {
        if antes == agora || !self.volume.disponivel() {
            return;
        }
        let aplicado = self.volume.definir(agora);
        eprintln!("volume: {antes}% -> {aplicado}%");
    }

    /// Leva o slider de brilho ao hardware.
    ///
    /// Antes disto o controle era decorativo: mudava o número na tela e nada no
    /// painel. O núcleo guarda a intenção (`settings.brightness`); quem escreve
    /// em `sysfs` é a camada de UI, porque o núcleo é puro e testável sem
    /// hardware.
    fn sync_brilho(&mut self, antes: u8, agora: u8) {
        if antes == agora || !self.backlight.disponivel() {
            return;
        }
        self.backlight.definir_fracao(agora);
    }

    /// Apaga o painel ao dormir e devolve o brilho ao acordar.
    ///
    /// Fica aqui, e não no núcleo, porque é escrita em `sysfs`: o núcleo é puro
    /// de propósito e precisa continuar testável sem hardware.
    fn sync_backlight(&mut self, antes: Screen, agora: Screen) {
        if antes == agora || !self.backlight.disponivel() {
            return;
        }
        match agora {
            Screen::Sleeping => {
                self.backlight.apagar();
                eprintln!("sono: backlight apagado");
            }
            // Só restaura quando sai do sono de fato. Ir para `Shutdown` ou
            // `Boot` deixa a tela como está, senão a mensagem de desligado
            // ficaria invisível.
            Screen::Running if antes == Screen::Sleeping => {
                self.backlight.restaurar();
                eprintln!("acordado: backlight restaurado");
            }
            _ => {}
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Quit { .. } => return false,

            Event::KeyDown {
                keycode: Some(key), ..
            } => {
                if let Some(cmd) = keyboard_command(*key) {
                    self.dispatch(cmd);
                }
                // No PC, o Enter também acorda e reinicia.
                if *key == Keycode::Return {
                    match self.state.screen {
                        Screen::Sleeping => self.dispatch(Cmd::Wake),
                        Screen::Shutdown => self.dispatch(Cmd::Reboot),
                        _ => {}
                    }
                }
            }

            Event::MouseButtonDown {
                x, y, mouse_btn, ..
            } if *mouse_btn == sdl2::mouse::MouseButton::Left => {
                self.pointer(*x, *y);
            }

            Event::FingerDown { x, y, .. } => {
                // Coordenadas de toque vêm normalizadas (0..1).
                let px = (x * self.window_size.0 as f32) as i32;
                let py = (y * self.window_size.1 as f32) as i32;
                self.pointer(px, py);
            }

            Event::Window { win_event, .. } => {
                if let sdl2::event::WindowEvent::SizeChanged(w, h) = win_event {
                    self.window_size = (*w as u32, *h as u32);
                }
            }

            _ => {}
        }
        true
    }

    /// Um toque escolhe um alvo direto — não simula navegação.
    fn pointer(&mut self, window_x: i32, window_y: i32) {
        match self.state.screen {
            Screen::Sleeping => {
                self.dispatch(Cmd::Wake);
                return;
            }
            Screen::Shutdown | Screen::Reboot => {
                self.dispatch(Cmd::Reboot);
                return;
            }
            Screen::Boot => return,
            Screen::Running => {}
        }

        let Some((x, y)) = self.painter.window_to_logical(window_x, window_y) else {
            return;
        };
        let Some(hit) = self.layout.hit(x, y) else {
            return;
        };

        match hit {
            Hit::Tab(tab) => self.dispatch(Cmd::GoTab(tab)),
            Hit::Filter(filter) => self.dispatch(Cmd::SetFilter(filter)),
            Hit::Carousel(index) => {
                // Igual ao protótipo: o cartão lateral entra no centro, e só o
                // cartão central abre o detalhe.
                if self.state.carousel == index {
                    self.dispatch(Cmd::OpenGame(index));
                } else {
                    self.dispatch(Cmd::FocusAt(Zone::HomeCarousel, index));
                }
            }
            Hit::Game(slot) => {
                // `slot` é a posição na lista filtrada, não no catálogo.
                if let Some(game_index) = self.state.filtered_games().get(slot).copied() {
                    self.dispatch(Cmd::OpenGame(game_index));
                }
            }
            Hit::App(index) => self.dispatch(Cmd::OpenApp(index)),
            Hit::SettingsCat(index) => self.dispatch(Cmd::GoSettingsCat(index)),
            Hit::QsRow(index) => {
                self.dispatch(Cmd::FocusAt(Zone::QuickSettings, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::QsButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::QuickSettings, index as usize));
                self.dispatch(Cmd::Confirm);
            }
            Hit::DetailButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::GameDetailActions, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::PowerButton(index) => {
                self.dispatch(Cmd::FocusAt(Zone::PowerActions, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::Key(index) => {
                self.dispatch(Cmd::FocusAt(Zone::SearchKeyboard, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::Result(index) => {
                self.dispatch(Cmd::FocusAt(Zone::SearchResults, index));
                self.dispatch(Cmd::Confirm);
            }
            Hit::PowerTap => self.dispatch(Cmd::PowerShort),
        }
    }

    /// Chamado a cada quadro: gamepad, relógios e boot.
    pub fn tick(&mut self, delta_seconds: f32) {
        if self.state.screen == Screen::Boot {
            self.boot_frames += 1;
            if self.boot_frames >= BOOT_FRAMES {
                self.dispatch(Cmd::BootDone);
            }
        }

        self.poll_gamepad(delta_seconds);
        self.tick_toast(delta_seconds);
        self.tick_rotation(delta_seconds);

        self.fps_frames += 1;
        self.fps_elapsed += delta_seconds;
        if self.fps_elapsed >= 1.0 {
            self.state.hud.fps = (self.fps_frames as f32 / self.fps_elapsed).round() as u16;
            self.fps_frames = 0;
            self.fps_elapsed = 0.0;
            // O HUD mostra um número que muda a cada segundo; sem isso ele
            // congelaria com o redesenho condicional.
            if self.state.settings.hud {
                self.dirty = true;
            }
        }

        // O relógio da barra superior muda a cada minuto. É a única coisa que
        // obriga redesenho sem interação do usuário.
        let minute = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 60)
            .unwrap_or(0);
        if minute != self.last_minute {
            self.last_minute = minute;
            self.dirty = true;
        }
    }

    /// Relê `/user/.config/giochi/rotation` de vez em quando.
    ///
    /// Uma vez por segundo, porque ler arquivo a 60 Hz é desperdício e o efeito
    /// desejado (girar a tela) é humano, não animação. Serve para conferir o
    /// sentido **com a tela aberta**: os dois sentidos diferem 180°, então quem
    /// olha sabe na hora se está certo.
    fn tick_rotation(&mut self, delta_seconds: f32) {
        self.rotation_poll -= delta_seconds;
        if self.rotation_poll > 0.0 {
            return;
        }
        self.rotation_poll = 1.0;

        let Some(rot) = crate::draw::rotacao_do_arquivo() else {
            return;
        };
        if rot == self.painter.rotation() {
            return;
        }
        match self.painter.set_rotation(rot) {
            Ok(()) => {
                eprintln!("rotação alterada em tempo real: {rot:?}");
                self.dirty = true;
            }
            Err(e) => eprintln!("aviso: não foi possível girar a tela ({e})"),
        }
    }

    /// O aviso some sozinho.
    ///
    /// O núcleo expõe `Toast` e `HideToast`, mas não tem relógio — de propósito:
    /// tempo não pertence ao estado, pertence ao laço. Sem esta contagem, o aviso
    /// ficaria na tela para sempre, que era um bug do protótipo antes do
    /// `setTimeout` entrar.
    fn tick_toast(&mut self, delta_seconds: f32) {
        if self.state.toast.visible {
            self.toast_timer -= delta_seconds;
            if self.toast_timer <= 0.0 {
                self.toast_timer = 0.0;
                self.dispatch(Cmd::HideToast);
            }
        } else {
            self.toast_timer = TOAST_SECONDS;
        }
    }

    fn poll_gamepad(&mut self, delta_seconds: f32) {
        self.evdev.poll();

        let connected = self.evdev.device_count() > 0;
        if connected != self.state.pad_connected {
            self.dispatch(Cmd::SetPad(connected));
        }

        // `frame()` devolve valor, então o empréstimo de `evdev` termina antes de
        // `mapper` e `dispatch` precisarem de `&mut self`.
        let frame: GamepadFrame = self.evdev.frame();

        // `update` roda mesmo com a tela dormindo: se ficasse de fora, a borda e
        // a repetição internas envelheceriam e o primeiro quadro depois de
        // acordar emitiria um `Confirm` fantasma — o usuário acordaria dentro de
        // um jogo por causa do botão segurado.
        let commands = self.mapper.update(frame);

        // `start` abre o menu de energia: ação de borda, fora do `Mapper`.
        let start_down = self.evdev.start_pressed();
        let start_edge = start_down && !self.start_was_down;
        self.start_was_down = start_down;

        // Dormindo ou desligado, a primeira entrada acorda — e só isso. O resto
        // do quadro é descartado aqui para o botão que acorda não agir: era
        // exatamente o caso relatado no aparelho, em que nenhum botão voltava
        // porque só toque e teclado chamavam `Wake`.
        // Botão de energia físico. Fora do `buttons.map` porque é botão do
        // sistema (PMIC), não do controle — ver `crate::power`.
        let power_down = self.evdev.power_pressed();
        let power_acao = self.power.update(power_down, delta_seconds);

        match self.state.screen {
            Screen::Sleeping | Screen::Shutdown | Screen::Reboot => {
                if !commands.is_empty() || start_edge || power_down {
                    let cmd = if self.state.screen == Screen::Sleeping {
                        Cmd::Wake
                    } else {
                        Cmd::Reboot
                    };
                    eprintln!("acordando: entrada com a tela travada");
                    // O gesto serviu para acordar: consumir evita que soltar o
                    // botão emita `Curto` e durma de novo em seguida.
                    self.power.consumir();
                    self.dispatch(cmd);
                }
                return;
            }
            Screen::Boot | Screen::Running => {}
        }

        for cmd in commands {
            self.dispatch(cmd);
        }
        if start_edge {
            self.dispatch(Cmd::PowerHold);
        }

        // Teclas de volume do aparelho (`gpio-keys-vol`, declaradas no mapa).
        let (cima, baixo) = self.evdev.volume_pressed();
        let passos_cima = self.rep_cima.update(cima, delta_seconds);
        let passos_baixo = self.rep_baixo.update(baixo, delta_seconds);
        if passos_cima > 0 {
            self.dispatch(Cmd::AdjustVolume(passos_cima as i8));
        }
        if passos_baixo > 0 {
            self.dispatch(Cmd::AdjustVolume(-(passos_baixo as i8)));
        }

        match power_acao {
            Some(PowerAction::Longo) => {
                eprintln!(
                    "botão de energia: segurado {:.1}s, abrindo menu de energia",
                    self.power.segurado()
                );
                self.dispatch(Cmd::PowerHold);
            }
            Some(PowerAction::Curto) => {
                eprintln!("botão de energia: toque curto, dormindo");
                self.dispatch(Cmd::PowerShort);
            }
            None => {}
        }
    }

    pub fn render(&mut self) {
        // Nada mudou: não redesenha. Ver `dirty` na struct.
        if !self.dirty {
            return;
        }
        self.dirty = false;

        self.painter.clear(theme::BG);

        // Brilho ambiente no topo, na cor do jogo em foco. Faixas de alfa
        // decrescente: sem shader, e o custo é uma dúzia de retângulos.
        // As medidas saem de `Moldura`, não de 1280×720 fixo: num painel 4:3
        // uma faixa de 1280 de largura passa da borda e o brilho fica torto.
        if let Some(game) = self.state.focused_game() {
            let glow = theme::game_accent(game.id);
            let m = views::Moldura::de(&self.painter);
            let faixa = m.e(22);
            let mut y = 0;
            let mut alpha = 46;
            while y < m.e(260) && alpha > 0 {
                self.painter
                    .fill_rect(0, y, m.w, faixa, theme::with_alpha(glow, alpha as u8));
                y += faixa;
                alpha -= 4;
            }
        }

        self.layout = Layout::default();

        views::chrome_top(&mut self.painter, &self.state, &mut self.layout);

        match self.state.tab {
            Tab::Home => views::home(&mut self.painter, &self.state, &mut self.layout),
            Tab::Library => views::library(&mut self.painter, &self.state, &mut self.layout),
            Tab::Apps => views::apps(&mut self.painter, &self.state, &mut self.layout),
            Tab::Settings => views::settings(&mut self.painter, &self.state, &mut self.layout),
        }

        views::chrome_foot(&mut self.painter, &self.state);

        // Aviso de falha de desligar/reiniciar. Fica por cima da tela preta para
        // que o usuário saiba por que o aparelho continua ligado.
        if let Some(msg) = self.aviso_sistema.clone() {
            views::aviso_sistema(&mut self.painter, &msg);
        }
        views::overlays(&mut self.painter, &self.state, &mut self.layout);
        views::toast(&mut self.painter, &self.state);
        views::hud(&mut self.painter, &self.state);

        if !self.font_warned && self.painter.text_size("x", 12).0 == 0 {
            eprintln!(
                "aviso: nenhuma fonte carregada — o texto não será desenhado.\n\
                 Coloque um TTF em assets/fonts/ ou passe --font <caminho>."
            );
            self.font_warned = true;
        }

        self.painter.present();
    }
}

fn keyboard_command(key: Keycode) -> Option<Cmd> {
    match key {
        Keycode::Up => Some(Cmd::Nav(Dir::Up)),
        Keycode::Down => Some(Cmd::Nav(Dir::Down)),
        Keycode::Left => Some(Cmd::Nav(Dir::Left)),
        Keycode::Right => Some(Cmd::Nav(Dir::Right)),
        Keycode::Return | Keycode::Space => Some(Cmd::Confirm),
        Keycode::Escape => Some(Cmd::Back),
        Keycode::X => Some(Cmd::Options),
        Keycode::Y => Some(Cmd::Search),
        Keycode::Q => Some(Cmd::TabPrev),
        Keycode::E => Some(Cmd::TabNext),
        Keycode::Tab => Some(Cmd::ToggleQuickSettings),
        Keycode::P => Some(Cmd::PowerShort),
        _ => None,
    }
}
