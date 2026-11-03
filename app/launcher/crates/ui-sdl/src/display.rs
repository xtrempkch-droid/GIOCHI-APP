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
    /// Ninguém declarou; deduzido do formato do output.
    Deduzido,
}

impl Fonte {
    pub fn descricao(self) -> &'static str {
        match self {
            Fonte::Ambiente => "GIOCHI_ROTATE",
            Fonte::Usuario => "arquivo do usuário",
            Fonte::Sistema => "sistema (fbcon/rotate)",
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

/// Lê a orientação que o sistema declarou, se declarou.
pub fn rotacao_do_sistema() -> Option<Rotation> {
    rotacao_do_sistema_em(CAMINHO_FBCON)
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
    fn cada_fonte_se_identifica_no_log() {
        assert!(Fonte::Sistema.descricao().contains("sistema"));
        assert!(Fonte::Usuario.descricao().contains("usuário"));
    }
}
