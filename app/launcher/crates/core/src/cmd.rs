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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl Cmd {
    /// Comando que só existe porque uma pessoa mexeu no aparelho.
    ///
    /// É o que separa "o usuário pressionou algo" de "o sistema reportou algo".
    /// Só o primeiro acorda a tela: uma leitura de bateria que chega enquanto o
    /// aparelho dorme não pode acendê-lo, senão a tela ligaria sozinha.
    pub fn is_user_input(self) -> bool {
        match self {
            Cmd::Nav(_)
            | Cmd::Confirm
            | Cmd::Back
            | Cmd::Options
            | Cmd::Search
            | Cmd::TabPrev
            | Cmd::TabNext
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
            | Cmd::GoSettingsCat(_) => true,

            // Relógio do HUD, bateria, presença de controle, boot e o próprio
            // despertar não vêm de um dedo ou de um botão.
            Cmd::TickHud
            | Cmd::SetBattery(_)
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
