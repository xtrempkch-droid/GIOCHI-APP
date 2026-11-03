//! Controle do backlight do painel.
//!
//! Dormir sem apagar o backlight é meia-sola: a tela mostra a lua, mas o painel
//! continua consumindo o mesmo que ligado. Num portátil isso é a diferença entre
//! "dormiu" e "está gastando bateria com um desenho parado".
//!
//! O caminho é injetável para poder testar sem `/sys`: escrever em `sysfs` de
//! verdade num teste seria mexer no hardware da máquina que roda o teste.

use std::path::{Path, PathBuf};

/// Onde o `giochi-init` lê o brilho salvo no boot.
const BRILHO_SALVO: &str = "/user/.config/giochi/brightness";

/// Backlight do painel, com o último valor para restaurar.
pub struct Backlight {
    /// `None` quando não há backlight gravável — o launcher continua funcionando,
    /// só sem apagar a tela.
    arquivo: Option<PathBuf>,
    /// Valor antes de apagar. `None` = não está apagado por nós.
    salvo: Option<u32>,
}

impl Backlight {
    /// Procura o primeiro `/sys/class/backlight/*/brightness` gravável.
    pub fn discover() -> Self {
        let raiz = Path::new("/sys/class/backlight");
        let Ok(entradas) = std::fs::read_dir(raiz) else {
            return Self::sem_controle();
        };

        let mut caminhos: Vec<PathBuf> = entradas
            .flatten()
            .map(|e| e.path().join("brightness"))
            .filter(|p| p.exists())
            .collect();
        // Ordem estável: sem isso, dois backlights poderiam alternar entre boots.
        caminhos.sort();

        let mut bl = Self::com_arquivo(caminhos.into_iter().next());
        bl.destravar_tela_apagada();
        bl
    }

    /// Se o brilho veio a zero, alguém apagou o painel e não o acendeu de volta.
    ///
    /// Acontece quando o launcher morre dormindo: o `salvo` mora na memória do
    /// processo, então o próximo a abrir leria `0` como se fosse o normal e a
    /// tela ficaria preta para sempre — sem erro nenhum no console. Reacende
    /// usando o mesmo arquivo que o `giochi-init` já usa no boot.
    fn destravar_tela_apagada(&mut self) {
        if self.ler() != Some(0) {
            return;
        }
        let valor = std::fs::read_to_string(BRILHO_SALVO)
            .ok()
            .and_then(|t| t.trim().parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| self.metade());
        if self.escrever(valor) {
            eprintln!("aviso: backlight estava em 0; reacendido para {valor}");
        }
    }

    fn metade(&self) -> u32 {
        let maximo = self
            .arquivo
            .as_ref()
            .and_then(|p| p.parent())
            .map(|dir| dir.join("max_brightness"))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| t.trim().parse::<u32>().ok())
            .filter(|v| *v > 0);
        match maximo {
            Some(max) => max / 2,
            None => 1,
        }
    }

    pub fn sem_controle() -> Self {
        Self {
            arquivo: None,
            salvo: None,
        }
    }

    /// Construtor para teste, apontando para um arquivo qualquer.
    pub fn com_arquivo(arquivo: Option<PathBuf>) -> Self {
        Self {
            arquivo,
            salvo: None,
        }
    }

    pub fn disponivel(&self) -> bool {
        self.arquivo.is_some()
    }

    fn ler(&self) -> Option<u32> {
        let texto = std::fs::read_to_string(self.arquivo.as_ref()?).ok()?;
        texto.trim().parse().ok()
    }

    fn escrever(&self, valor: u32) -> bool {
        let Some(arquivo) = self.arquivo.as_ref() else {
            return false;
        };
        std::fs::write(arquivo, valor.to_string()).is_ok()
    }

    /// Apaga o painel, guardando o valor atual.
    ///
    /// Idempotente: chamar duas vezes não salva o próprio zero como "valor
    /// anterior", que deixaria a tela apagada para sempre ao acordar.
    pub fn apagar(&mut self) {
        if self.salvo.is_some() {
            return;
        }
        let Some(atual) = self.ler() else {
            return;
        };
        if self.escrever(0) {
            self.salvo = Some(atual);
        }
    }

    /// Devolve o brilho que estava antes de [`apagar`](Self::apagar).
    pub fn restaurar(&mut self) {
        let Some(valor) = self.salvo.take() else {
            return;
        };
        self.escrever(valor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arquivo_temporario(nome: &str, inicial: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("giochi-backlight-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("brightness");
        std::fs::write(&caminho, inicial.to_string()).unwrap();
        caminho
    }

    fn ler(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap().trim().to_string()
    }

    #[test]
    fn sem_backlight_nao_falha() {
        let mut b = Backlight::sem_controle();
        assert!(!b.disponivel());
        b.apagar();
        b.restaurar();
        // Não deveria entrar em pânico: é o caso de máquina sem painel.
    }

    #[test]
    fn apagar_zera_e_restaurar_devolve() {
        let caminho = arquivo_temporario("restaura", 833);
        let mut b = Backlight::com_arquivo(Some(caminho.clone()));

        b.apagar();
        assert_eq!(ler(&caminho), "0");

        b.restaurar();
        assert_eq!(ler(&caminho), "833", "brilho anterior precisa voltar");
    }

    #[test]
    fn apagar_duas_vezes_nao_perde_o_valor_original() {
        // O bug clássico: dormir duas vezes seguidas salvaria 0 como "anterior"
        // e ao acordar a tela ficaria apagada.
        let caminho = arquivo_temporario("idempotente", 500);
        let mut b = Backlight::com_arquivo(Some(caminho.clone()));

        b.apagar();
        b.apagar();
        b.apagar();
        assert_eq!(ler(&caminho), "0");

        b.restaurar();
        assert_eq!(ler(&caminho), "500");
    }

    #[test]
    fn restaurar_sem_apagar_nao_escreve() {
        let caminho = arquivo_temporario("semapagar", 700);
        let mut b = Backlight::com_arquivo(Some(caminho.clone()));

        b.restaurar();
        assert_eq!(ler(&caminho), "700", "não pode mexer sem ter apagado");
    }

    #[test]
    fn tela_preta_de_processo_morto_volta_a_acender() {
        // Cenário: o launcher morreu dormindo e deixou brightness=0.
        let dir = std::env::temp_dir().join(format!("giochi-backlight-recupera-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("brightness"), "0").unwrap();
        std::fs::write(dir.join("max_brightness"), "1666").unwrap();

        let mut bl = Backlight::com_arquivo(Some(dir.join("brightness")));
        bl.destravar_tela_apagada();

        assert_eq!(ler(&dir.join("brightness")), "833", "volta para a metade");
    }

    #[test]
    fn brilho_normal_nao_e_mexido_na_abertura() {
        let caminho = arquivo_temporario("normal", 416);
        let mut bl = Backlight::com_arquivo(Some(caminho.clone()));
        bl.destravar_tela_apagada();
        assert_eq!(ler(&caminho), "416", "abrir com brilho normal não pode mexer");
    }

    #[test]
    fn arquivo_ausente_e_tratado_como_sem_controle() {
        let inexistente = std::env::temp_dir().join("giochi-backlight-nao-existe/brightness");
        let mut b = Backlight::com_arquivo(Some(inexistente));
        b.apagar();
        b.restaurar();
        // Sem pânico, sem arquivo criado.
    }
}
