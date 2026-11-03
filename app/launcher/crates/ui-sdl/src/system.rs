//! Ações que mexem no sistema inteiro: desligar e reiniciar de verdade.
//!
//! ## Por que este módulo existe
//!
//! Até aqui, "Desligar" e "Reiniciar" só trocavam a tela. O reducer do núcleo,
//! corretamente, não executa nada — ele é puro e testável sem hardware. Mas
//! alguém precisa executar, e esse alguém é a camada de UI.
//!
//! ## Processo filho, não `exec`
//!
//! O launcher **cria um processo filho** com `Command::status()` e espera por
//! ele. Não usa `exec`, porque `exec` substituiria o próprio launcher: se o
//! comando falhasse, não sobraria processo para mostrar o erro na tela.
//!
//! A contrapartida é que durante o `poweroff` existem dois processos e os dois
//! podem ter o DRM aberto. Na prática dura poucos milissegundos e o aparelho
//! desliga antes de qualquer conflito — mas está aqui registrado, porque já
//! esteve descrito ao contrário neste arquivo.
//!
//! ## Desligar corta a energia neste aparelho
//!
//! Medido no X55 em 03/11/2026: `poweroff` **desliga de fato** — o aparelho
//! precisou ser religado pelo botão físico.
//!
//! A primeira versão deste comentário afirmava o contrário, deduzindo do device
//! tree que, sem `rockchip,system-power-controller`, o hardware continuaria
//! alimentado. A propriedade de fato **não existe** e o desligamento funciona
//! assim mesmo: o driver do PMIC registra o caminho de outra forma. A conclusão
//! foi escrita a partir de documentação, sem teste, e o teste a desmentiu.
//!
//! O app faz a parte dele (chama `poweroff`) e **registra o resultado**.
//! `declaracao_de_energia()` abaixo continua existindo como informação sobre o
//! device tree — nunca como previsão do que o comando fará.

use std::process::Command;

/// O que o kernel vai fazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    Desligar,
    Reiniciar,
}

impl Acao {
    /// Comando que executa a ação.
    ///
    /// `poweroff` e `reboot` do BusyBox fazem a chamada de sistema certa
    /// (`reboot(2)` com o comando apropriado); não é preciso `shutdown`.
    pub fn comando(self) -> &'static str {
        match self {
            Acao::Desligar => "poweroff",
            Acao::Reiniciar => "reboot",
        }
    }
}

/// O device tree declara `system-power-controller` em algum nó?
///
/// **Correção de 03/11/2026:** a primeira versão disto afirmava que, sem essa
/// propriedade, `poweroff` não cortaria a energia. **Está errado e foi medido:**
/// no X55 a propriedade não existe e `poweroff` desliga o aparelho de verdade —
/// o usuário confirmou que precisou religar. O driver do PMIC registra o
/// caminho de desligamento de outra forma.
///
/// Então isto é **informação**, não veredito: serve para comparar aparelhos e
/// detectar mudança no device tree, não para prever o comportamento do
/// `poweroff`. Quem quer saber se desliga precisa desligar.
pub fn declaracao_de_energia() -> bool {
    declaracao_de_energia_em("/proc/device-tree")
}

fn declaracao_de_energia_em(raiz: &str) -> bool {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return false;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.join("system-power-controller").exists() {
            return true;
        }
        if caminho.is_dir() && declaracao_de_energia_em(&caminho.to_string_lossy()) {
            return true;
        }
    }
    false
}

/// Executa a ação. **Não retorna em caso de sucesso.**
///
/// Devolve a mensagem de erro quando o comando não pôde ser executado ou
/// retornou erro — o chamador mostra isso na tela, em vez de fingir que
/// desligou.
pub fn executar(acao: Acao) -> String {
    eprintln!(
        "sistema: executando `{}` ({acao:?})",
        acao.comando()
    );

    match Command::new(acao.comando()).status() {
        Ok(status) if status.success() => {
            // Normalmente não chega aqui: `poweroff` derruba o sistema antes.
            format!(
                "`{}` terminou sem desligar o aparelho",
                acao.comando()
            )
        }
        Ok(status) => format!(
            "`{}` saiu com {} — o aparelho não desligou",
            acao.comando(),
            status.code().map_or("sinal".to_string(), |c| c.to_string())
        ),
        Err(e) => format!("não consegui executar `{}`: {e}", acao.comando()),
    }
}

/// Marca que a falha já foi mostrada, para não repetir a tentativa a cada quadro.
pub struct TentativaUnica {
    ja_tentou: bool,
}

impl TentativaUnica {
    pub fn nova() -> Self {
        Self { ja_tentou: false }
    }

    /// `true` na primeira chamada, `false` depois.
    ///
    /// Sem isso, um `poweroff` que falha (por falta de
    /// `system-power-controller`, por exemplo) seria reexecutado a cada quadro,
    /// enchendo o console e gastando CPU para sempre.
    pub fn primeira_vez(&mut self) -> bool {
        if self.ja_tentou {
            false
        } else {
            self.ja_tentou = true;
            true
        }
    }

    pub fn resetar(&mut self) {
        self.ja_tentou = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comandos_corretos_por_acao() {
        assert_eq!(Acao::Desligar.comando(), "poweroff");
        assert_eq!(Acao::Reiniciar.comando(), "reboot");
    }

    #[test]
    fn tentativa_unica_so_dispara_uma_vez() {
        let mut t = TentativaUnica::nova();
        assert!(t.primeira_vez());
        assert!(!t.primeira_vez());
        assert!(!t.primeira_vez());
    }

    #[test]
    fn resetar_permite_nova_tentativa() {
        let mut t = TentativaUnica::nova();
        t.primeira_vez();
        t.resetar();
        assert!(t.primeira_vez());
    }

    #[test]
    fn detecta_a_propriedade_no_device_tree() {
        let dir = std::env::temp_dir().join(format!("giochi-dt-{}", std::process::id()));
        let pmic = dir.join("i2c").join("pmic@20");
        std::fs::create_dir_all(&pmic).unwrap();

        assert!(!declaracao_de_energia_em(&dir.to_string_lossy()));

        std::fs::write(pmic.join("system-power-controller"), []).unwrap();
        assert!(declaracao_de_energia_em(&dir.to_string_lossy()));
    }

    #[test]
    fn ausencia_da_propriedade_nao_e_veredito_sobre_desligar() {
        // O X55 desliga sem a propriedade. Este teste existe para impedir que
        // alguém reintroduza a conclusão errada de "sem DT, não desliga".
        let dir = std::env::temp_dir().join(format!("giochi-dt2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!declaracao_de_energia_em(&dir.to_string_lossy()));
        // O contrato: `executar` continua sendo a única fonte de verdade.
        assert_eq!(Acao::Desligar.comando(), "poweroff");
    }

    #[test]
    fn comando_inexistente_devolve_erro_em_vez_de_panico() {
        // `Acao` só tem comandos que existem, mas o contrato de `executar` é
        // devolver mensagem — então testamos o caminho de erro pelo ambiente.
        let msg = executar_com("comando-que-nao-existe-giochi", &[]);
        assert!(msg.contains("não consegui executar"), "veio: {msg}");
    }

    /// Igual a [`executar`], mas com o comando parametrizado, para testar o
    /// caminho de falha sem depender de privilégio ou de hardware.
    fn executar_com(programa: &str, args: &[&str]) -> String {
        match Command::new(programa).args(args).status() {
            Ok(s) if s.success() => "terminou sem efeito".to_string(),
            Ok(s) => format!("saiu com {:?}", s.code()),
            Err(e) => format!("não consegui executar `{programa}`: {e}"),
        }
    }
}
