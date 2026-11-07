//! Orientação e espaço de projeto.
//!
//! ## O sistema manda, o app obedece
//!
//! O X55 tem painel **retrato** (720×1280) e o console desenha **paisagem**. A
//! informação de como girar não está no app: está em `fbcon=rotate:N` no cmdline
//! do kernel, exposto em `/sys/class/graphics/fbcon/rotate`. Um app que fixa o
//! sentido acerta num aparelho e erra no seguinte — já aconteceu aqui.
//!
//! Precedência: `GIOCHI_ROTATE` → `/user/.config/giochi/rotation` →
//! `fbcon/rotate` → dedução pelo formato do painel.
//!
//! ## Escala 1,0 sempre
//!
//! O espaço de projeto é **o do dispositivo** (com as medidas trocadas quando há
//! giro). Não há `set_logical_size`, não há esticar: cada pixel de projeto é um
//! pixel de device. Num painel 640×480 a célula sai 22 px; num 1280×720 sai 34.
//! Isso é "sem upscaling" — o desenho nunca é rasterizado num tamanho e exibido
//! em outro.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotacao {
    Nenhuma,
    Cw90,
    Ccw90,
}

impl Rotacao {
    pub fn do_valor(valor: &str) -> Option<Self> {
        match valor.trim() {
            "0" | "none" => Some(Rotacao::Nenhuma),
            "90" | "cw" => Some(Rotacao::Cw90),
            "270" | "ccw" => Some(Rotacao::Ccw90),
            _ => None,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Rotacao::Nenhuma => "Nenhuma",
            Rotacao::Cw90 => "Cw90",
            Rotacao::Ccw90 => "Ccw90",
        }
    }

}

/// Traduz o inteiro de `fbcon/rotate` em rotação de desenho.
///
/// `3` é 270° no sentido horário, que é o que o X55 usa — e é `Ccw90` na nossa
/// convenção de desenho. `2` (180°) não é suportado: avisar é melhor que fingir.
pub fn do_numero(valor: i32) -> Result<Option<Rotacao>, String> {
    match valor {
        0 => Ok(Some(Rotacao::Nenhuma)),
        1 => Ok(Some(Rotacao::Cw90)),
        3 => Ok(Some(Rotacao::Ccw90)),
        2 => Err("fbcon/rotate=2 (180 graus) nao e suportado".into()),
        outro => Err(format!("fbcon/rotate={outro} desconhecido")),
    }
}

pub const CAMINHO_FBCON: &str = "/sys/class/graphics/fbcon/rotate";
pub const CAMINHO_USUARIO: &str = "/user/.config/giochi/rotation";

/// Lê a rotação de um arquivo com o inteiro. `None` se o arquivo não existe.
pub fn do_arquivo(caminho: &str) -> Option<Result<Option<Rotacao>, String>> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    let n: i32 = texto.trim().parse().ok()?;
    Some(do_numero(n))
}

/// Deduz pelo formato: painel mais alto que largo é retrato e precisa de giro.
fn deduzir(largura: u32, altura: u32) -> Rotacao {
    if altura > largura {
        Rotacao::Ccw90
    } else {
        Rotacao::Nenhuma
    }
}

/// Qual rotação usar, e de onde veio a decisão (para o log de abertura).
///
/// `confiar_no_sistema` é falso quando o painel foi **simulado** (`--tamanho`):
/// nesse caso o `fbcon/rotate` do PC não descreve o painel pedido, e obedecê-lo
/// faria o teste de 720×1280 desenhar em paisagem — o oposto do aparelho.
pub fn resolver(largura: u32, altura: u32, confiar_no_sistema: bool) -> (Rotacao, &'static str) {
    if let Ok(texto) = std::env::var("GIOCHI_ROTATE") {
        if let Some(r) = Rotacao::do_valor(&texto) {
            return (r, "GIOCHI_ROTATE");
        }
    }
    if let Some(Ok(Some(r))) = do_arquivo(CAMINHO_USUARIO) {
        return (r, "config do usuario");
    }
    if confiar_no_sistema {
        if let Some(Ok(Some(r))) = do_arquivo(CAMINHO_FBCON) {
            return (r, "sistema (fbcon/rotate)");
        }
    }
    (deduzir(largura, altura), "formato do painel")
}

/// Medidas do espaço de projeto.
///
/// Com giro, o painel é retrato e o projeto é paisagem: as medidas trocam. Sem
/// giro, projeto é painel.
pub fn projetar(rot: Rotacao, painel: (u32, u32)) -> (u32, u32) {
    match rot {
        Rotacao::Nenhuma => painel,
        Rotacao::Cw90 | Rotacao::Ccw90 => (painel.1, painel.0),
    }
}

/// Ponto do espaço de projeto → espaço de dispositivo.
///
/// É a implementação de referência: [`girar_rect`] tem de concordar com ela nos
/// dois cantos, e o teste compara as duas. Só o teste a chama — duas fórmulas
/// para a mesma rotação divergem em silêncio, e foi assim que a UI já saiu de
/// cabeça para baixo uma vez.
#[allow(dead_code)]
pub fn girar_ponto(rot: Rotacao, design: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    match rot {
        Rotacao::Nenhuma => (x, y),
        Rotacao::Cw90 => (design.1 - y, x),
        Rotacao::Ccw90 => (y, design.0 - x),
    }
}

/// Retângulo do espaço de projeto → espaço de dispositivo.
///
/// Retângulo alinhado aos eixos continua retângulo sob múltiplo de 90°, então
/// largura e altura trocam e o canto é reposicionado.
pub fn girar_rect(
    rot: Rotacao,
    design: (i32, i32),
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> (i32, i32, i32, i32) {
    match rot {
        Rotacao::Nenhuma => (x, y, w, h),
        Rotacao::Cw90 => (design.1 - y - h, x, h, w),
        Rotacao::Ccw90 => (y, design.0 - x - w, h, w),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sem_giro_o_projeto_e_o_painel() {
        assert_eq!(projetar(Rotacao::Nenhuma, (640, 480)), (640, 480));
    }

    #[test]
    fn com_giro_troca_as_medidas() {
        assert_eq!(projetar(Rotacao::Ccw90, (720, 1280)), (1280, 720));
        assert_eq!(projetar(Rotacao::Cw90, (720, 1280)), (1280, 720));
    }

    #[test]
    fn retangulo_cabe_dentro_do_painel_depois_de_girado() {
        // O erro clássico é o retângulo sair pela borda por um pixel de sinal
        // trocado. Aqui se verifica o canto e o tamanho, não só a origem.
        let design = (1280, 720);
        let painel = (720, 1280);
        for rot in [Rotacao::Cw90, Rotacao::Ccw90] {
            let (x, y, w, h) = girar_rect(rot, design, 0, 0, design.0, design.1);
            assert!(x >= 0 && y >= 0, "{rot:?}: canto negativo ({x},{y})");
            assert!(
                x + w <= painel.0 as i32 && y + h <= painel.1 as i32,
                "{rot:?}: {w}x{h} em ({x},{y}) estoura {painel:?}"
            );
            assert_eq!((w, h), (design.1, design.0), "{rot:?}: medidas não trocaram");
        }
    }

    #[test]
    fn girar_ponto_e_a_base_de_girar_rect() {
        // `girar_rect` tem de concordar com `girar_ponto` no canto e no oposto:
        // duas formulas para a mesma coisa divergem em silêncio.
        for rot in [Rotacao::Nenhuma, Rotacao::Cw90, Rotacao::Ccw90] {
            let design = (1280, 720);
            let (x, y, w, h) = (100, 200, 30, 40);
            let (dx, dy, dw, dh) = girar_rect(rot, design, x, y, w, h);
            let (px, py) = girar_ponto(rot, design, x, y);
            let (qx, qy) = girar_ponto(rot, design, x + w, y + h);
            let esperado_x = dx.min(px.min(qx));
            let esperado_y = dy.min(py.min(qy));
            assert_eq!((dx, dy), (esperado_x, esperado_y), "{rot:?}: canto divergiu");
            assert_eq!(
                (pw(dx, px, qx), ph(dy, py, qy)),
                (dw, dh),
                "{rot:?}: tamanho divergiu"
            );
        }
    }

    fn pw(d: i32, a: i32, b: i32) -> i32 {
        d.max(a).max(b) - d.min(a).min(b)
    }
    fn ph(d: i32, a: i32, b: i32) -> i32 {
        d.max(a).max(b) - d.min(a).min(b)
    }

    #[test]
    fn fbcon_3_e_anti_horario() {
        // O X55 precisa de anti-horário (`fbcon=rotate:3`). Errar isto deixou a
        // UI de cabeça para baixo uma vez, e a diferença é de 180°.
        assert_eq!(do_numero(3), Ok(Some(Rotacao::Ccw90)));
        assert_eq!(do_numero(1), Ok(Some(Rotacao::Cw90)));
        assert_eq!(do_numero(0), Ok(Some(Rotacao::Nenhuma)));
    }

    #[test]
    fn cento_e_oitenta_graus_e_recusado_com_motivo() {
        let r = do_numero(2);
        assert!(r.is_err(), "180° tem de ser erro explícito, não silêncio");
        assert!(r.unwrap_err().contains("180"));
    }

    #[test]
    fn painel_retrato_deduz_giro() {
        assert_eq!(deduzir(720, 1280), Rotacao::Ccw90);
        assert_eq!(deduzir(640, 480), Rotacao::Nenhuma);
        assert_eq!(deduzir(480, 640), Rotacao::Ccw90);
    }

    #[test]
    fn painel_simulado_nao_obedece_ao_fbcon_do_pc() {
        // `--tamanho 720x1280` no PC pede um painel retrato. Se a decisão
        // viesse do `fbcon/rotate` do PC (que é 0), o teste mostraria paisagem
        // e diria que o aparelho está certo quando não está.
        let (rot, fonte) = resolver(720, 1280, false);
        assert_eq!(rot, Rotacao::Ccw90, "fonte {fonte}");
    }

    #[test]
    fn valor_de_texto_aceita_as_duas_formas() {
        assert_eq!(Rotacao::do_valor("ccw"), Some(Rotacao::Ccw90));
        assert_eq!(Rotacao::do_valor("270"), Some(Rotacao::Ccw90));
        assert_eq!(Rotacao::do_valor("cw"), Some(Rotacao::Cw90));
        assert_eq!(Rotacao::do_valor("lixo"), None);
    }
}

/// Quem está segurando o DRM master, se for caso.
///
/// ## Por que isto existe
///
/// Quando o KMSDRM falha, a mensagem que aparece é `kmsdrm not available` — ou,
/// pior, `Is a DRM device that is a GPU and is being used by an X server`,
/// que manda parar um X server que **não existe** no aparelho. A equipe da
/// engine documentou a mesma armadilha (`KMSDRM.md` §8): o que segura o
/// dispositivo é o **launcher do próprio console**.
///
/// O KMSDRM é exclusivo: um segundo processo não abre o mesmo conector. Então a
/// pergunta útil não é "deu erro?", é **"quem está com ele?"** — e a resposta
/// tem de sair na mensagem, com o comando que resolve.
///
/// Devolve a linha de comando do processo, se ele for do Giochi.
pub fn quem_segura_o_drm() -> Option<String> {
    let entradas = std::fs::read_dir("/proc").ok()?;
    for entrada in entradas.flatten() {
        let nome = entrada.file_name();
        let nome = nome.to_string_lossy();
        if !nome.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(bytes) = std::fs::read(entrada.path().join("cmdline")) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let cmdline = String::from_utf8_lossy(&bytes).replace('\0', " ");
        let cmdline = cmdline.trim();
        // Só processos do Giochi interessam: apontar o `git` do usuário como
        // culpado seria a mesma mentira do "pare seu X server".
        if cmdline.contains("giochi-launcher") || cmdline.contains("giochi-init") {
            return Some(format!("pid {nome}: {cmdline}"));
        }
    }
    None
}

/// Mensagem para quando o dispositivo de vídeo não abre.
///
/// Separa três causas que exigem ações diferentes:
///
/// | causa | o que a mensagem diz |
/// | --- | --- |
/// | ninguém do Giochi está rodando | o driver não expõe KMSDRM |
/// | o launcher está rodando | ele segura o DRM; libere e tente de novo |
/// | só o `giochi-init` | o supervisor está sozinho (provável `RESTART_MAX`) |
pub fn erro_de_video(original: &str) -> String {
    let mut msg = format!("não consegui abrir o vídeo ({original}).");

    if !std::path::Path::new("/dev/dri/card0").exists() {
        msg.push_str(
            "\n  /dev/dri/card0 não existe: o driver não expôs o conector.",
        );
        msg.push_str("\n  Isso é imagem/driver (Giochi System), não o app.");
        return msg;
    }

    match quem_segura_o_drm() {
        Some(quem) if quem.contains("giochi-launcher") => {
            msg.push_str(&format!("\n  O launcher está rodando e detém o DRM master: {quem}"));
            msg.push_str(
                "\n  O KMSDRM é exclusivo — um segundo processo não abre o conector.",
            );
            msg.push_str(
                "\n  \"pare seu X server\" não se aplica: não há X nem Wayland neste aparelho.",
            );
            msg.push_str("\n  Libere o console e tente de novo:");
            msg.push_str("\n    bash scripts/aparelho.sh verificar-jogo <jogo>");
        }
        Some(quem) => {
            msg.push_str(&format!("\n  O supervisor está sozinho: {quem}"));
            msg.push_str(
                "\n  Provável `RESTART_MAX` esgotado (5 quedas em 60 s): ele para de",
            );
            msg.push_str(
                "\n  reiniciar e fica dormindo. `bash scripts/aparelho.sh subir` reinicia",
            );
            msg.push_str("\n  o supervisor e recupera a cota.");
        }
        None => {
            msg.push_str(
                "\n  Nenhum processo do Giochi está rodando, então ninguém segura o DRM:",
            );
            msg.push_str("\n  o driver não abriu o conector (imagem/driver, Giochi System).");
        }
    }
    msg
}

#[cfg(test)]
mod testes_erro {
    use super::*;

    #[test]
    fn a_mensagem_sempre_nomeia_uma_causa() {
        // A regra que veio do campo: mensagem que sugere a causa errada custa
        // mais que nenhuma mensagem. Qualquer que seja o ramo, ela tem de dizer
        // de quem é o problema.
        let msg = erro_de_video("kmsdrm not available");
        assert!(msg.contains("kmsdrm not available"), "preserva o erro original");
        assert!(
            msg.contains("/dev/dri/card0")
                || msg.contains("DRM master")
                || msg.contains("RESTART_MAX")
                || msg.contains("driver"),
            "mensagem sem causa identificável: {msg}"
        );
    }

    #[test]
    fn a_deteccao_nao_casa_com_o_proprio_processo_de_teste() {
        // O processo que procura também aparece na lista — foi assim que o
        // `pid_do_giochi_init` do `aparelho.sh` devolveu o pid do próprio
        // `ssh`. Aqui a busca filtra por nome do Giochi, e o binário de teste
        // não é o launcher.
        let proprio = std::fs::read("/proc/self/cmdline")
            .map(|b| String::from_utf8_lossy(&b).replace('\0', " "))
            .unwrap_or_default();
        assert!(
            !proprio.contains("giochi-launcher"),
            "o binário de teste não pode se identificar como launcher"
        );
    }

    #[test]
    fn a_mensagem_nega_o_x_server_quando_o_launcher_segura() {
        // A frase "stop your X server" aparece em app com KMSDRM e manda
        // procurar um servidor gráfico que este aparelho não tem.
        let msg = erro_de_video("Is a DRM device that is a GPU and is being used by an X server");
        assert!(
            msg.contains("não há X nem Wayland") || msg.contains("/dev/dri/card0"),
            "{msg}"
        );
    }
}
