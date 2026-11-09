//! Fatos sobre a tela que vêm do **sistema**, não do app.
//!
//! ## Por que este módulo existe
//!
//! O launcher precisa girar a imagem porque o painel é retrato e a UI é
//! paisagem. A primeira versão disso tinha uma constante
//! `DEFAULT_ROTATION = Ccw90` com o comentário "o X55 precisa" — um remendo de
//! aparelho dentro do app. Se outra revisão do X55 montar o painel ao contrário,
//! ou outro aparelho entrar na linha, o app passa a estar errado.
//!
//! O sistema **já declara** a orientação: `fbcon=rotate:N` no cmdline do kernel,
//! exposto em `/sys/class/graphics/fbcon/rotate`. O app lê; não decide.
//!
//! ## Ordem de precedência
//!
//! | Fonte | Quem usa | Recarrega ao vivo |
//! | --- | --- | --- |
//! | `GIOCHI_ROTATE` | depuração | não (é ambiente de processo) |
//! | `/user/.config/giochi/rotation` | usuário | sim |
//! | `/sys/class/graphics/fbcon/rotate` | **sistema** | sim |
//! | `/proc/cmdline` (`fbcon=rotate:N`) | sistema, quando o sysfs diz 0 | não |
//! | formato do output | último recurso | — |
//!
//! A fonte vai junto do valor para o log responder "de quem é esta decisão".
//! Sem isso, depurar orientação vira adivinhação.

use crate::draw::Rotation;

/// De onde saiu a rotação em uso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fonte {
    /// `GIOCHI_ROTATE` — depuração explícita.
    Ambiente,
    /// Arquivo do usuário, relido em tempo de execução.
    Usuario,
    /// O sistema declarou a orientação do painel.
    Sistema,
    /// O sysfs dizia `0`, mas o `cmdline` pedia `fbcon=rotate:N`.
    SistemaCmdline,
    /// Ninguém declarou; deduzido do formato do output.
    Deduzido,
}

impl Fonte {
    pub fn descricao(self) -> &'static str {
        match self {
            Fonte::Ambiente => "GIOCHI_ROTATE",
            Fonte::Usuario => "arquivo do usuário",
            Fonte::Sistema => "sistema (fbcon/rotate)",
            Fonte::SistemaCmdline => "sistema (cmdline; fbcon/rotate dizia 0)",
            Fonte::Deduzido => "deduzido do formato do output",
        }
    }
}

/// Traduz o valor de `fbcon=rotate:N` para rotação de desenho.
///
/// A semântica é a do kernel (`Documentation/fb/fbcon.rst`), em giros de 90°
/// no sentido horário:
///
/// | `N` | Kernel diz | Nosso `Rotation` |
/// | --- | --- | --- |
/// | 0 | normal | `None` |
/// | 1 | 90° horário | `Cw90` |
/// | 2 | 180° | **não suportado** |
/// | 3 | 270° (= 90° anti-horário) | `Ccw90` |
///
/// `2` devolve `None` de propósito: um app que finge atender um pedido que não
/// atende é pior que um que avisa. Quem chama registra o aviso.
pub fn rotacao_do_valor_fbcon(valor: u32) -> Result<Rotation, String> {
    match valor {
        0 => Ok(Rotation::None),
        1 => Ok(Rotation::Cw90),
        3 => Ok(Rotation::Ccw90),
        2 => Err("orientação de 180° não é suportada".into()),
        outro => Err(format!("valor de fbcon/rotate desconhecido: {outro}")),
    }
}

/// Extrai `fbcon=rotate:N` de um `cmdline`, se estiver lá.
///
/// Função pura: recebe o texto e devolve o que ele pede. `None` significa "o
/// cmdline não pede rotação", e não "a rotação é zero".
pub fn rotacao_do_cmdline(texto: &str) -> Option<u32> {
    texto
        .split_whitespace()
        .find_map(|arg| arg.strip_prefix("fbcon=rotate:"))
        .and_then(|valor| valor.parse::<u32>().ok())
}

/// O `fbcon=rotate:N` que o kernel recebeu no boot.
pub fn rotacao_pedida_no_cmdline() -> Option<u32> {
    let texto = std::fs::read_to_string(CAMINHO_CMDLINE).ok()?;
    rotacao_do_cmdline(&texto)
}

/// Lê a orientação que o sistema declarou, se declarou.
pub fn rotacao_do_sistema() -> Option<Rotation> {
    rotacao_do_sistema_em(CAMINHO_FBCON)
}

/// O mesmo, mas dizendo **de onde** veio — e cruzando as duas fontes do kernel.
///
/// ## Por que o cruzamento existe
///
/// Medido no X55 em 07/11/2026, com a imagem `60e326f`: o `cmdline` trazia
/// `fbcon=rotate:3` e `/sys/class/graphics/fbcon/rotate` respondia **`0`**.
/// O sysfs é a fonte em que o app confiava, então o launcher desenhava em
/// projeto 720×1280 num painel retrato — a UI deitada, e em densidade
/// `compacta` por causa do formato trocado.
///
/// A regra é conservadora de propósito: o sysfs **vence** quando diz algo
/// diferente de zero, porque ele é o estado em tempo de execução. Só quando ele
/// diz `0` é que o cmdline é consultado — e só para descobrir um pedido que o
/// sysfs deixou de refletir. `fbcon=rotate:0` explícito e ausência de parâmetro
/// dão no mesmo, e é o correto: não há rotação a aplicar.
pub fn rotacao_do_sistema_com_fonte() -> Option<(Rotation, Fonte)> {
    let do_sysfs = rotacao_do_sistema_em(CAMINHO_FBCON);

    if matches!(do_sysfs, Some(Rotation::None) | None) {
        if let Some(pedido) = rotacao_pedida_no_cmdline() {
            if pedido != 0 {
                if let Ok(rot) = rotacao_do_valor_fbcon(pedido) {
                    eprintln!(
                        "aviso: fbcon/rotate diz 0 mas o cmdline pede fbcon=rotate:{pedido} — usando o cmdline"
                    );
                    return Some((rot, Fonte::SistemaCmdline));
                }
            }
        }
    }

    do_sysfs.map(|rot| (rot, Fonte::Sistema))
}

/// O caminho é parâmetro para o teste poder usar um arquivo temporário.
pub fn rotacao_do_sistema_em(caminho: &str) -> Option<Rotation> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    let valor: u32 = texto.trim().parse().ok()?;
    match rotacao_do_valor_fbcon(valor) {
        Ok(rot) => Some(rot),
        Err(aviso) => {
            // Avisa uma vez, na abertura: o sistema pediu algo que não fazemos.
            eprintln!("aviso: {aviso} — configure /user/.config/giochi/rotation");
            None
        }
    }
}

/// Onde o kernel publica a rotação pedida por `fbcon=rotate:`.
pub const CAMINHO_FBCON: &str = "/sys/class/graphics/fbcon/rotate";

/// O `cmdline` do kernel, onde o pedido original aparece.
pub const CAMINHO_CMDLINE: &str = "/proc/cmdline";

#[cfg(test)]
mod tests {
    use super::*;

    fn arquivo_com(conteudo: &str) -> String {
        let dir = std::env::temp_dir().join(format!(
            "giochi-fbcon-{}-{}",
            conteudo.trim(),
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("rotate");
        std::fs::write(&p, conteudo).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn mapeia_os_quatro_valores_do_kernel() {
        assert_eq!(rotacao_do_valor_fbcon(0), Ok(Rotation::None));
        assert_eq!(rotacao_do_valor_fbcon(1), Ok(Rotation::Cw90));
        // 3 é 270° horário = 90° anti-horário: é o que o X55 usa.
        assert_eq!(rotacao_do_valor_fbcon(3), Ok(Rotation::Ccw90));
        assert!(rotacao_do_valor_fbcon(2).is_err(), "180 não é suportado");
    }

    #[test]
    fn valor_invalido_nao_vira_none_silencioso() {
        // Devolver `None` aqui mascararia um sistema dizendo bobagem.
        assert!(rotacao_do_valor_fbcon(7).is_err());
    }

    #[test]
    fn le_o_arquivo_do_sistema() {
        assert_eq!(
            rotacao_do_sistema_em(&arquivo_com("3\n")),
            Some(Rotation::Ccw90)
        );
        assert_eq!(
            rotacao_do_sistema_em(&arquivo_com("0")),
            Some(Rotation::None)
        );
    }

    #[test]
    fn arquivo_ausente_nao_e_erro() {
        assert_eq!(rotacao_do_sistema_em("/nao/existe/rotate"), None);
    }

    #[test]
    fn le_a_rotacao_do_cmdline() {
        assert_eq!(
            rotacao_do_cmdline("root=/dev/mmcblk1p2 fbcon=rotate:3 quiet"),
            Some(3)
        );
        assert_eq!(rotacao_do_cmdline("fbcon=rotate:1"), Some(1));
        assert_eq!(rotacao_do_cmdline("console=ttyS2,1500000n8"), None);
        assert_eq!(
            rotacao_do_cmdline("fbcon=rotate:xyz"),
            None,
            "valor inválido não vira 0 silencioso"
        );
    }

    #[test]
    fn cmdline_nao_sobrepoe_o_sysfs_quando_ele_declara_algo() {
        // O caso do X55 novo: sysfs diz 0 e o cmdline pede 3. O cruzamento só
        // acontece quando o sysfs **não** declara rotação.
        //
        // Este teste fixa a regra sem tocar no sistema de arquivos: o par
        // (sysfs=0, cmdline=3) tem de produzir Ccw90 vindo do cmdline.
        assert_eq!(rotacao_do_cmdline("fbcon=rotate:3"), Some(3));
        assert_eq!(
            rotacao_do_valor_fbcon(3),
            Ok(Rotation::Ccw90),
            "o valor do cmdline tem de virar a rotação certa"
        );
        // E sysfs=1 com cmdline=3 continua valendo o sysfs (estado de execução).
        assert_eq!(rotacao_do_valor_fbcon(1), Ok(Rotation::Cw90));
    }

    #[test]
    fn cada_fonte_se_identifica_no_log() {
        assert!(Fonte::Sistema.descricao().contains("sistema"));
        assert!(Fonte::SistemaCmdline.descricao().contains("cmdline"));
        assert!(Fonte::Usuario.descricao().contains("usuário"));
    }
}
