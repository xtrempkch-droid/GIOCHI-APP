//! Orientação do painel: qual rotação aplicar e **de onde veio** a resposta.
//!
//! Este módulo existe porque a orientação é fato do sistema, não do app: o X55
//! tem painel 720×1280 e o sistema pede 270°; o X35H é 640×480 e não pede nada.
//! Um app que fixa um sentido funciona em um aparelho e sai deitado no outro.
//!
//! A precedência é a mesma do launcher de produção, e a **leitura é injetada**
//! (`ler`) para que a regra — que é o que erra — seja testada sem depender do
//! `/sys` da máquina que roda o teste.

/// Onde o usuário pode forçar a rotação pela interface do sistema.
pub const ARQUIVO_ROTACAO: &str = "/user/.config/giochi/rotation";
/// O que o sistema declara como estado de execução.
pub const SYSFS_ROTACAO: &str = "/sys/class/graphics/fbcon/rotate";
/// O que a linha de comando do kernel pediu.
pub const CMDLINE: &str = "/proc/cmdline";

/// Sentido de rotação a aplicar no desenho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotacao {
    Nenhuma,
    /// 90° no sentido horário (o painel está deitado).
    Horario,
    /// 270°, o caso do X55: painel retrato com o sistema pedindo `Ccw90`.
    AntiHorario,
}

impl Rotacao {
    pub fn gira(self) -> bool {
        !matches!(self, Rotacao::Nenhuma)
    }

    /// Texto para o log de abertura. Diz o sentido, não o número: `Ccw90` é
    /// verificável na tela, `3` não é.
    pub fn descricao(self) -> &'static str {
        match self {
            Rotacao::Nenhuma => "nenhuma",
            Rotacao::Horario => "horário (90°)",
            Rotacao::AntiHorario => "anti-horário (270°)",
        }
    }
}

/// Quem respondeu. Vai para o log porque a pergunta seguinte é sempre "por quê".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fonte {
    Ambiente,
    Usuario,
    Sistema,
    Cmdline,
    /// Deduzido do formato do painel — último recurso.
    Deduzido,
    /// O sistema pediu uma rotação que o app não implementa (180°).
    NaoSuportado,
}

impl Fonte {
    pub fn descricao(self) -> &'static str {
        match self {
            Fonte::Ambiente => "GIOCHI_ROTATE",
            Fonte::Usuario => ARQUIVO_ROTACAO,
            Fonte::Sistema => SYSFS_ROTACAO,
            Fonte::Cmdline => "/proc/cmdline (fbcon=rotate:N)",
            Fonte::Deduzido => "formato do painel",
            Fonte::NaoSuportado => "rotação de 180° não é suportada",
        }
    }
}

/// Espaço de projeto para um painel de `painel` pixels.
///
/// Sem giro, projeto = painel. Com giro, o projeto é o painel deitado — e a
/// escala continua **1,0**, então a fonte mantém o tamanho em pixels. Projetar
/// num 1280×720 fixo encolhia a fonte para metade num painel 640×480.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projecao {
    pub projeto: (i32, i32),
    pub painel: (u32, u32),
}

pub fn projecao(rot: Rotacao, painel: (u32, u32)) -> Projecao {
    let (w, h) = (painel.0.max(1), painel.1.max(1));
    let projeto = match rot {
        Rotacao::Nenhuma => (w as i32, h as i32),
        Rotacao::Horario | Rotacao::AntiHorario => (h as i32, w as i32),
    };
    Projecao {
        projeto,
        painel: (w, h),
    }
}

/// Leva um ponto do espaço de projeto para o espaço de dispositivo.
pub fn rotate_point(rot: Rotacao, projeto: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotacao::Nenhuma => (x, y),
        Rotacao::Horario => (projeto.1 - y, x),
        Rotacao::AntiHorario => (y, projeto.0 - x),
    }
}

/// Desfaz a rotação — o caminho de volta do toque.
///
/// Sem chamador no app, mas fica: é a **inversa** que prova que `rotate_point`
/// girou no sentido certo, e "de ponta a cabeça" é invisível para quem lê a
/// fórmula.
pub fn unrotate_point(rot: Rotacao, projeto: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotacao::Nenhuma => (x, y),
        Rotacao::Horario => (y, projeto.1 - x),
        Rotacao::AntiHorario => (projeto.0 - y, x),
    }
}

/// Leva um retângulo para o espaço de dispositivo, trocando largura e altura.
///
/// A inversa de `rotate_rect` **não** é "girar para o outro lado": as dimensões
/// trocam junto, e esquecer isso produz uma transposição que passa despercebida
/// em quadros quase simétricos.
pub fn rotate_rect(
    rot: Rotacao,
    projeto: (i32, i32),
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> (i32, i32, i32, i32) {
    match rot {
        Rotacao::Nenhuma => (x, y, w, h),
        Rotacao::Horario => (projeto.1 - y - h, x, h, w),
        Rotacao::AntiHorario => (y, projeto.0 - x - w, h, w),
    }
}

/// Interpreta um valor escrito à mão (`0`, `90`, `ccw`…).
///
/// `None` = valor desconhecido ou não suportado. O 180° cai aqui de propósito:
/// o app avisa e pede configuração manual em vez de fingir que girou.
pub fn de_valor(valor: &str) -> Option<Rotacao> {
    match valor.trim().to_ascii_lowercase().as_str() {
        "0" | "none" | "nenhuma" => Some(Rotacao::Nenhuma),
        "1" | "90" | "cw" | "horario" | "horário" => Some(Rotacao::Horario),
        "3" | "270" | "ccw" | "anti-horario" | "anti-horário" => Some(Rotacao::AntiHorario),
        _ => None,
    }
}

fn pede_180(valor: &str) -> bool {
    matches!(valor.trim(), "2" | "180")
}

/// Extrai `fbcon=rotate:N` de uma linha de comando do kernel.
pub fn do_cmdline(texto: &str) -> Option<u32> {
    texto
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("fbcon=rotate:"))
        .and_then(|v| v.parse::<u32>().ok())
}

/// Decide a rotação, na ordem do launcher de produção.
///
/// O cruzamento com o `cmdline` existe por medição: no X55 o sysfs respondia
/// `0` enquanto o `cmdline` pedia `fbcon=rotate:3`, e a UI saía deitada num
/// painel retrato. O sysfs vence quando diz algo diferente de zero — é o estado
/// de execução; só o `0` é desempatado.
pub fn decidir(
    env: Option<&str>,
    mut ler: impl FnMut(&str) -> Option<String>,
    painel: (u32, u32),
) -> (Rotacao, Fonte) {
    if let Some(valor) = env {
        if pede_180(valor) {
            return (Rotacao::Nenhuma, Fonte::NaoSuportado);
        }
        if let Some(rot) = de_valor(valor) {
            return (rot, Fonte::Ambiente);
        }
    }

    if let Some(texto) = ler(ARQUIVO_ROTACAO) {
        if pede_180(&texto) {
            return (Rotacao::Nenhuma, Fonte::NaoSuportado);
        }
        if let Some(rot) = de_valor(&texto) {
            return (rot, Fonte::Usuario);
        }
    }

    let sysfs = ler(SYSFS_ROTACAO).map(|t| t.trim().to_string());
    match sysfs.as_deref() {
        Some("1") => return (Rotacao::Horario, Fonte::Sistema),
        Some("3") => return (Rotacao::AntiHorario, Fonte::Sistema),
        Some("2") => return (Rotacao::Nenhuma, Fonte::NaoSuportado),
        _ => {}
    }

    // Só chega aqui quando o sysfs está ausente ou em `0` (o "não sei" dele).
    if let Some(texto) = ler(CMDLINE) {
        if let Some(pedido) = do_cmdline(&texto) {
            match pedido {
                1 => return (Rotacao::Horario, Fonte::Cmdline),
                3 => return (Rotacao::AntiHorario, Fonte::Cmdline),
                2 => return (Rotacao::Nenhuma, Fonte::NaoSuportado),
                _ => {}
            }
        }
    }

    // Último recurso: painel retrato sem nenhum pedido do sistema. Não é
    // adivinhação gratuita — sem girar, um painel 720×1280 desenharia a
    // interface de lado. A fonte fica marcada como deduzida no log.
    let (w, h) = painel;
    if h > w {
        (Rotacao::AntiHorario, Fonte::Deduzido)
    } else {
        (Rotacao::Nenhuma, Fonte::Deduzido)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leitor(fbcon: Option<&str>, cmdline: Option<&str>) -> impl FnMut(&str) -> Option<String> {
        let fbcon = fbcon.map(str::to_string);
        let cmdline = cmdline.map(str::to_string);
        move |caminho: &str| match caminho {
            SYSFS_ROTACAO => fbcon.clone(),
            CMDLINE => cmdline.clone(),
            _ => None,
        }
    }

    // ── projeção e matemática ───────────────────────────────

    #[test]
    fn sem_giro_projeto_e_o_painel() {
        let p = projecao(Rotacao::Nenhuma, (640, 480));
        assert_eq!(p.projeto, (640, 480));
        assert_eq!(p.painel, (640, 480));
    }

    #[test]
    fn com_giro_projeto_e_o_painel_deitado() {
        let p = projecao(Rotacao::AntiHorario, (720, 1280));
        assert_eq!(p.projeto, (1280, 720));
        // A escala é 1,0: o projeto tem a mesma área do painel.
        assert_eq!(p.projeto.0 * p.projeto.1, 720 * 1280);
    }

    #[test]
    fn rotacao_de_ponto_e_rect_concordam() {
        let projeto = (1280, 720);
        for rot in [Rotacao::Horario, Rotacao::AntiHorario] {
            let (x, y, w, h) = rotate_rect(rot, projeto, 100, 40, 30, 20);
            // Os dois cantos do retângulo têm de cair nos cantos rotacionados.
            let a = rotate_point(rot, projeto, 100, 40);
            let b = rotate_point(rot, projeto, 130, 60);
            assert_eq!((x.min(x + w), y.min(y + h)), (a.0.min(b.0), a.1.min(b.1)));
            assert_eq!((w, h), (20, 30), "largura e altura trocam no giro");
        }
    }

    #[test]
    fn ida_e_volta_devolve_o_ponto_original() {
        let projeto = (1280, 720);
        for rot in [Rotacao::Nenhuma, Rotacao::Horario, Rotacao::AntiHorario] {
            for (x, y) in [(0, 0), (1280, 0), (0, 720), (1280, 720), (500, 300)] {
                let ida = rotate_point(rot, projeto, x, y);
                let volta = unrotate_point(rot, projeto, ida.0, ida.1);
                assert_eq!(volta, (x, y), "{rot:?} em {x},{y}");
            }
        }
    }

    #[test]
    fn horario_e_anti_horario_levam_os_cantos_aos_cantos() {
        // Os dois sentidos diferem (errar o sentido dá "de ponta a cabeça"),
        // mas os dois têm de levar os quatro cantos do projeto aos quatro
        // cantos do dispositivo — é o que prova que a área foi coberta.
        let projeto = (1280, 720);
        let dispositivo = (720, 1280);
        for rot in [Rotacao::Horario, Rotacao::AntiHorario] {
            let mut cantos: Vec<(i32, i32)> = [(0, 0), (projeto.0, 0), (0, projeto.1), (projeto.0, projeto.1)]
                .iter()
                .map(|(x, y)| rotate_point(rot, projeto, *x, *y))
                .collect();
            cantos.sort_unstable();
            let mut esperado = vec![
                (0, 0),
                (dispositivo.0, 0),
                (0, dispositivo.1),
                (dispositivo.0, dispositivo.1),
            ];
            esperado.sort_unstable();
            assert_eq!(cantos, esperado, "{rot:?}");
        }
        // E os dois sentidos não são o mesmo desenho.
        assert_ne!(
            rotate_point(Rotacao::Horario, projeto, 300, 200),
            rotate_point(Rotacao::AntiHorario, projeto, 300, 200)
        );
    }

    #[test]
    fn valores_de_configuracao_sao_interpretados() {
        assert_eq!(de_valor("0"), Some(Rotacao::Nenhuma));
        assert_eq!(de_valor("90"), Some(Rotacao::Horario));
        assert_eq!(de_valor(" ccw\n"), Some(Rotacao::AntiHorario));
        assert_eq!(de_valor("270"), Some(Rotacao::AntiHorario));
        assert_eq!(de_valor("2"), None);
        assert_eq!(de_valor("180"), None);
        assert_eq!(de_valor("deitado"), None);
    }

    // ── precedência ─────────────────────────────────────────

    #[test]
    fn ambiente_vence_o_sistema() {
        let (rot, fonte) = decidir(Some("90"), leitor(Some("3"), None), (720, 1280));
        assert_eq!(rot, Rotacao::Horario);
        assert_eq!(fonte, Fonte::Ambiente);
    }

    #[test]
    fn arquivo_do_usuario_vence_o_sistema() {
        let ler = |caminho: &str| match caminho {
            ARQUIVO_ROTACAO => Some("0".to_string()),
            SYSFS_ROTACAO => Some("3".to_string()),
            _ => None,
        };
        let (rot, fonte) = decidir(None, ler, (720, 1280));
        assert_eq!(rot, Rotacao::Nenhuma);
        assert_eq!(fonte, Fonte::Usuario);
    }

    #[test]
    fn sysfs_diferente_de_zero_vence_o_cmdline() {
        let (rot, fonte) = decidir(
            None,
            leitor(Some("1"), Some("root=/dev/mmcblk0 fbcon=rotate:3")),
            (1280, 720),
        );
        assert_eq!(rot, Rotacao::Horario);
        assert_eq!(fonte, Fonte::Sistema);
    }

    #[test]
    fn sysfs_zero_e_desempatado_pelo_cmdline() {
        // Medido no X55: sysfs 0 e cmdline pedindo 3. Sem este cruzamento a UI
        // saía deitada num painel retrato.
        let (rot, fonte) = decidir(
            None,
            leitor(Some("0"), Some("console=ttyS2 fbcon=rotate:3 quiet")),
            (720, 1280),
        );
        assert_eq!(rot, Rotacao::AntiHorario);
        assert_eq!(fonte, Fonte::Cmdline);
    }

    #[test]
    fn sem_sistema_nada_e_deduzido_pelo_formato_do_painel() {
        let (rot, fonte) = decidir(None, leitor(None, None), (720, 1280));
        assert_eq!(rot, Rotacao::AntiHorario);
        assert_eq!(fonte, Fonte::Deduzido);

        let (rot, fonte) = decidir(None, leitor(None, None), (640, 480));
        assert_eq!(rot, Rotacao::Nenhuma);
        assert_eq!(fonte, Fonte::Deduzido);
    }

    #[test]
    fn cento_e_oitenta_graus_e_recusado_com_motivo() {
        // Fingir que girou é pior que não girar: o usuário veria a interface de
        // lado e procuraria defeito no app.
        let (rot, fonte) = decidir(None, leitor(Some("2"), None), (720, 1280));
        assert_eq!(rot, Rotacao::Nenhuma);
        assert_eq!(fonte, Fonte::NaoSuportado);
        assert!(fonte.descricao().contains("180"));
    }

    #[test]
    fn cmdline_sem_a_opcao_nao_estoura() {
        assert_eq!(do_cmdline("console=ttyS2 quiet"), None);
        assert_eq!(do_cmdline("fbcon=rotate:"), None);
        assert_eq!(do_cmdline("fbcon=rotate:x"), None);
        assert_eq!(do_cmdline(""), None);
        assert_eq!(do_cmdline("fbcon=rotate:1"), Some(1));
    }

    #[test]
    fn toda_fonte_tem_descricao() {
        for fonte in [
            Fonte::Ambiente,
            Fonte::Usuario,
            Fonte::Sistema,
            Fonte::Cmdline,
            Fonte::Deduzido,
            Fonte::NaoSuportado,
        ] {
            assert!(!fonte.descricao().is_empty());
        }
    }
}
