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
    /// Arquivo onde a escolha do usuário fica gravada.
    ///
    /// Parametrizado para o teste poder apontar para um temporário: escrever em
    /// `/user/.config` de verdade num teste mexeria na máquina que roda o teste.
    arquivo_escolha: PathBuf,
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
        bl.restaurar_salvo();
        bl
    }

    /// Reaplica o brilho que o usuário escolheu numa sessão anterior.
    ///
    /// O `giochi-init` deveria fazer isto no boot, mas **o `inittab` do aparelho
    /// não o inicia** (ver docs/contrato-app-sistema.md). Enquanto isso, o app
    /// faz — pelo mesmo motivo que já faz com o volume: sem isso a escolha do
    /// usuário some a cada reinício e o painel volta ao padrão do hardware
    /// (medido: arquivo 1332, hardware 833 depois de um boot).
    ///
    /// É idempotente com o `giochi-init`: quando ele voltar a rodar, os dois
    /// aplicam o mesmo valor.
    pub fn restaurar_salvo(&mut self) {
        let Some(texto) = std::fs::read_to_string(&self.arquivo_escolha).ok() else {
            return;
        };
        let Ok(valor) = texto.trim().parse::<u32>() else {
            return;
        };
        // 0 nunca é intenção de brilho: é tela apagada por processo que morreu,
        // já tratado por `destravar_tela_apagada`.
        if valor == 0 {
            return;
        }
        let valor = match self.maximo() {
            Some(max) => valor.min(max),
            None => valor,
        };
        self.escrever(valor);
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
            arquivo_escolha: PathBuf::from(BRILHO_SALVO),
        }
    }

    /// Construtor para teste, apontando para um arquivo qualquer.
    pub fn com_arquivo(arquivo: Option<PathBuf>) -> Self {
        Self {
            arquivo,
            salvo: None,
            arquivo_escolha: PathBuf::from(BRILHO_SALVO),
        }
    }

    /// Igual a [`com_arquivo`](Self::com_arquivo), mas com o arquivo de escolha
    /// também injetável.
    #[cfg(test)]
    pub fn com_arquivos(arquivo: Option<PathBuf>, escolha: PathBuf) -> Self {
        Self {
            arquivo,
            salvo: None,
            arquivo_escolha: escolha,
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

    fn maximo(&self) -> Option<u32> {
        let dir = self.arquivo.as_ref()?.parent()?;
        let texto = std::fs::read_to_string(dir.join("max_brightness")).ok()?;
        texto.trim().parse::<u32>().ok().filter(|v| *v > 0)
    }

    /// Brilho atual em porcentagem, para a UI mostrar o valor do hardware.
    pub fn fracao_atual(&self) -> Option<u8> {
        let atual = self.ler()?;
        let max = self.maximo()?;
        Some(((atual as f32 / max as f32) * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    /// Aplica um brilho de 0 a 100 e grava a escolha para o próximo boot.
    ///
    /// Grava no arquivo que o `giochi-init` lê (`/user/.config/giochi/brightness`)
    /// porque é este o contrato: o init restaura no boot, mas quem sabe a
    /// intenção do usuário é o app. Sem essa escrita, o brilho escolhido some a
    /// cada reinício e o init restaura um valor que ninguém escolheu.
    pub fn definir_fracao(&mut self, pct: u8) {
        let Some(max) = self.maximo() else {
            return;
        };
        let valor = ((pct.min(100) as u32 * max) / 100).max(1);
        if !self.escrever(valor) {
            return;
        }
        // Se a tela está apagada, isto vira o novo valor de retorno: o usuário
        // acabou de dizer como quer a tela.
        if self.salvo.is_some() {
            self.salvo = Some(valor);
        }
        self.persistir(valor);
    }

    fn persistir(&self, valor: u32) {
        if let Some(dir) = self.arquivo_escolha.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.arquivo_escolha, valor.to_string());
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
    fn restaurar_salvo_reaplica_a_escolha_do_usuario() {
        // Reproduz o que foi medido no aparelho: arquivo 1332, hardware 833
        // depois de um boot, porque o `giochi-init` não roda.
        let dir = dir_com_max("restaurar", 833, 1666);
        let escolha = dir.join("escolha");
        std::fs::write(&escolha, "1332").unwrap();

        let mut bl = Backlight::com_arquivos(Some(dir.join("brightness")), escolha);
        bl.restaurar_salvo();

        assert_eq!(ler(&dir.join("brightness")), "1332");
    }

    #[test]
    fn sem_escolha_salva_o_hardware_fica_intacto() {
        let dir = dir_com_max("semescolha", 833, 1666);
        let mut bl =
            Backlight::com_arquivos(Some(dir.join("brightness")), dir.join("nao-existe"));
        bl.restaurar_salvo();
        assert_eq!(ler(&dir.join("brightness")), "833");
    }

    #[test]
    fn escolha_salva_zero_nao_apaga_a_tela() {
        // 0 no arquivo é tela apagada por processo que morreu, não intenção.
        let dir = dir_com_max("zerosalvo", 833, 1666);
        let escolha = dir.join("escolha");
        std::fs::write(&escolha, "0").unwrap();

        let mut bl = Backlight::com_arquivos(Some(dir.join("brightness")), escolha);
        bl.restaurar_salvo();

        assert_eq!(ler(&dir.join("brightness")), "833", "não pode apagar");
    }

    #[test]
    fn escolha_acima_do_maximo_e_aparada() {
        // Se o painel mudar em outra revisão, o valor antigo não estoura.
        let dir = dir_com_max("aparar", 500, 1000);
        let escolha = dir.join("escolha");
        std::fs::write(&escolha, "1200").unwrap();

        let mut bl = Backlight::com_arquivos(Some(dir.join("brightness")), escolha);
        bl.restaurar_salvo();

        assert_eq!(ler(&dir.join("brightness")), "1000");
    }

    #[test]
    fn brilho_normal_nao_e_mexido_na_abertura() {
        let caminho = arquivo_temporario("normal", 416);
        let mut bl = Backlight::com_arquivo(Some(caminho.clone()));
        bl.destravar_tela_apagada();
        assert_eq!(ler(&caminho), "416", "abrir com brilho normal não pode mexer");
    }

    fn dir_com_max(nome: &str, atual: u32, max: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("giochi-bl-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("brightness"), atual.to_string()).unwrap();
        std::fs::write(dir.join("max_brightness"), max.to_string()).unwrap();
        dir
    }

    #[test]
    fn fracao_atual_reflete_o_hardware() {
        let dir = dir_com_max("fracao", 833, 1666);
        let bl = Backlight::com_arquivo(Some(dir.join("brightness")));
        assert_eq!(bl.fracao_atual(), Some(50));
    }

    #[test]
    fn definir_fracao_converte_para_a_escala_do_painel() {
        let dir = dir_com_max("define", 833, 1666);
        let mut bl = Backlight::com_arquivo(Some(dir.join("brightness")));

        bl.definir_fracao(20);

        // 20% de 1666 = 333
        assert_eq!(ler(&dir.join("brightness")), "333");
        assert_eq!(bl.fracao_atual(), Some(20));
    }

    #[test]
    fn brilho_zero_vira_um_e_nao_apaga_de_vez() {
        // Zero no slider é "o mais escuro possível", não "desligar a tela":
        // desligar é função do sono. Deixar 0 aqui tornaria a tela
        // irrecuperável sem teclado.
        let dir = dir_com_max("zero", 833, 1666);
        let mut bl = Backlight::com_arquivo(Some(dir.join("brightness")));

        bl.definir_fracao(0);

        assert_eq!(ler(&dir.join("brightness")), "1");
    }

    #[test]
    fn mudar_brilho_dormindo_atualiza_o_valor_de_retorno() {
        let dir = dir_com_max("dormindo", 800, 1600);
        let mut bl = Backlight::com_arquivo(Some(dir.join("brightness")));

        bl.apagar();
        bl.definir_fracao(80); // usuário mexeu no slider dormindo
        bl.restaurar();

        // 80% de 1600 = 1280, não o 800 de antes.
        assert_eq!(ler(&dir.join("brightness")), "1280");
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
