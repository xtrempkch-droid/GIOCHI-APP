//! Comandos de entrada.
//!
//! O protótipo usava strings (`'NAV_UP'`, `'CONFIRM'`) e um `switch` com
//! comparação de texto. Aqui são enums: erro de digitação vira erro de
//! compilação, e o `match` do reducer é exaustivo por obrigação do compilador.

/// Direção de navegação no foco.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// Tudo que pode acontecer com o launcher.
///
/// Deixou de ser `Copy` quando passou a carregar listas (`RedesWifi`,
/// `DispositivosBt`): uma varredura devolve `Vec`, e copiar isso a cada
/// despacho seria desperdício. `Clone` basta — o reducer consome o comando.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    /// Direcional (d-pad ou analógico).
    Nav(Dir),
    /// Botão de confirmação (A).
    Confirm,
    /// Botão de voltar (B).
    Back,
    /// Botão de detalhes/opções (X).
    Options,
    /// Botão de busca (Y).
    Search,

    /// Aba anterior (L1 / ombro esquerdo).
    TabPrev,
    /// Próxima aba (R1 / ombro direito).
    TabNext,
    /// Sub-aba anterior (L2 / gatilho esquerdo).
    ///
    /// Hierarquia igual à do controle: L1/R1 troca de aba, L2/R2 troca de
    /// sub-aba dentro dela. Existe porque as sub-abas só eram alcançáveis
    /// movendo o foco até a faixa delas e andando de lado — "a navegação está
    /// como somente esquerda ou direita". Agora a troca funciona de qualquer
    /// ponto da aba, sem tirar o usuário de onde ele está olhando.
    SubPrev,
    /// Sub-aba seguinte (R2 / gatilho direito).
    SubNext,

    /// Abre/fecha o painel de ajustes rápidos (Select).
    ToggleQuickSettings,

    /// Toque curto no botão de energia.
    PowerShort,
    /// Toque longo no botão de energia.
    PowerHold,

    /// Acordar da suspensão.
    Wake,
    /// Reiniciar a partir do estado inicial.
    Reboot,
    /// Encerrar o boot simulado.
    BootDone,

    /// Esconder o toast atual.
    HideToast,

    /// Alternar Wi-Fi.
    ToggleWifi,
    /// Alternar Bluetooth.
    ToggleBluetooth,
    /// Alternar HUD de performance.
    ToggleHud,

    /// Alternar filtro da biblioteca.
    SetFilter(state::Filter),
    /// Definir modo de desempenho.
    SetPerf(state::Perf),
    /// Somar/subtrair volume, em passos de 5.
    AdjustVolume(i8),
    /// Somar/subtrair brilho, em passos de 5.
    AdjustBrightness(i8),
    /// Definir volume absoluto (toque no slider).
    SetVolume(u8),
    /// Definir brilho absoluto (toque no slider).
    SetBrightness(u8),

    /// Digitar um caractere na busca.
    SearchPush(char),
    /// Apagar o último caractere da busca.
    SearchBackspace,

    /// Atualizar leituras do HUD.
    TickHud,

    /// Atualizar o nível de bateria. `None` = sem leitura.
    SetBattery(Option<u8>),
    /// Temperatura do SoC em °C. `None` = sem sensor; a HUD mostra `—`.
    ///
    /// Comando separado de `SetBattery` porque as duas leituras falham por
    /// motivos diferentes (gauge ausente × zona térmica ausente) e a UI precisa
    /// poder mostrar uma sem a outra.
    SetTemperature(Option<u16>),
    /// Informar se há controle conectado.
    SetPad(bool),

    // ── comandos de ponteiro/toque ─────────────────────────────
    //
    // O gamepad só consegue *mover* o foco; um dedo escolhe um alvo direto.
    // Sem estes comandos, a UI teria de simular dezenas de `Nav` para chegar
    // ao item tocado — e o resultado dependeria da ordem do layout.

    /// Move o foco para uma posição específica.
    FocusAt(focus::Zone, usize),
    /// Vai direto para uma aba.
    GoTab(state::Tab),
    /// Abre o detalhe do jogo pelo índice no catálogo.
    OpenGame(usize),
    /// Age sobre o app pelo índice.
    OpenApp(usize),
    /// Vai direto para uma categoria de configuração.
    GoSettingsCat(usize),

    // ── Wi-Fi e Bluetooth ────────────────────────────────────
    //
    // A UI **pede**; quem executa é o `ui-sdl` (`wpa_cli`, `bluetoothctl`).
    // O núcleo continua puro: estes comandos só mudam o que a tela sabe.

    /// Abre a lista de redes. Dispara `Cmd::PediScanWifi` do lado da UI.
    AbrirWifi,
    /// Uma varredura terminou. `redes` vazia + `erro` explica o motivo.
    RedesWifi(Vec<crate::state::Rede>, Option<String>),
    /// Começou a varrer: a UI mostra "procurando…" em vez de lista vazia.
    EscaneandoWifi(bool),
    /// Escolheu a linha da lista. Rede protegida abre o teclado; aberta conecta.
    EscolherRede(usize),
    /// Confirmou a senha digitada.
    ConectarSenha,
    /// Cancelou a senha (não conecta).
    CancelarSenha,
    /// Alterna mostrar a senha em claro. Não muda o valor, só o desenho.
    MostrarSenha,
    /// Resultado de uma tentativa de conexão. `None` = deu certo.
    ResultadoWifi(Option<String>),
    AbrirBluetooth,
    DispositivosBt(Vec<crate::state::DispositivoBt>, Option<String>),
    EscaneandoBt(bool),
    /// Escolheu um dispositivo da lista para parear.
    ParearBt(usize),
    ResultadoBt(Option<String>),
}

impl Cmd {
    /// Comando que só existe porque uma pessoa mexeu no aparelho.
    ///
    /// É o que separa "o usuário pressionou algo" de "o sistema reportou algo".
    /// Só o primeiro acorda a tela: uma leitura de bateria que chega enquanto o
    /// aparelho dorme não pode acendê-lo, senão a tela ligaria sozinha.
    pub fn is_user_input(&self) -> bool {
        match self {
            Cmd::Nav(_)
            | Cmd::Confirm
            | Cmd::Back
            | Cmd::Options
            | Cmd::Search
            | Cmd::TabPrev
            | Cmd::TabNext
            | Cmd::SubPrev
            | Cmd::SubNext
            | Cmd::ToggleQuickSettings
            | Cmd::PowerShort
            | Cmd::PowerHold
            | Cmd::ToggleWifi
            | Cmd::ToggleBluetooth
            | Cmd::ToggleHud
            | Cmd::SetFilter(_)
            | Cmd::SetPerf(_)
            | Cmd::AdjustVolume(_)
            | Cmd::AdjustBrightness(_)
            | Cmd::SetVolume(_)
            | Cmd::SetBrightness(_)
            | Cmd::SearchPush(_)
            | Cmd::SearchBackspace
            | Cmd::FocusAt(_, _)
            | Cmd::GoTab(_)
            | Cmd::OpenGame(_)
            | Cmd::OpenApp(_)
            | Cmd::GoSettingsCat(_)
            | Cmd::AbrirWifi
            | Cmd::AbrirBluetooth
            | Cmd::EscolherRede(_)
            | Cmd::ConectarSenha
            | Cmd::CancelarSenha
            | Cmd::MostrarSenha
            | Cmd::ParearBt(_) => true,

            // Relógio do HUD, bateria, temperatura, presença de controle, boot
            // e o próprio despertar não vêm de um dedo ou de um botão.
            Cmd::TickHud
            | Cmd::RedesWifi(_, _)
            | Cmd::EscaneandoWifi(_)
            | Cmd::ResultadoWifi(_)
            | Cmd::DispositivosBt(_, _)
            | Cmd::EscaneandoBt(_)
            | Cmd::ResultadoBt(_)
            | Cmd::SetBattery(_)
            | Cmd::SetTemperature(_)
            | Cmd::SetPad(_)
            | Cmd::BootDone
            | Cmd::HideToast
            | Cmd::Wake
            | Cmd::Reboot => false,
        }
    }
}

use crate::focus;
use crate::state;
