//! Volume do sistema, para as teclas de volume funcionarem **durante o jogo**.
//!
//! ## A pergunta que este módulo responde
//!
//! "O som do jogo acompanha o volume do sistema?" — **sim, e sempre acompanhou.**
//! O jogo nunca chama `AudioSystem::set_volume`, então o mixer do SDL fica no
//! máximo e quem escala a saída é o `Master` do ALSA, que é o volume do sistema.
//! Medido no X35H: `Master` 255/255 (100 %, 0,00 dB).
//!
//! ## O que faltava, então
//!
//! **As teclas.** Elas são lidas pelo *launcher*, que fica em `fork`+`wait`
//! enquanto o jogo roda — bloqueado, sem bombear eventos. Resultado: durante a
//! partida ninguém lê `KEY_VOLUMEUP`/`VOLUMEDOWN`, e o jogador não consegue
//! abaixar o som justamente quando está jogando.
//!
//! Este módulo lê as mesmas teclas e mexe no **mesmo** `Master` do ALSA, então
//! não há dois volumes para divergir: o que o jogador ajusta no jogo é o que o
//! launcher mostra depois, e o que o boot restaura.
//!
//! ## Por que `amixer` e não o FFI que o launcher usa
//!
//! O launcher fala com a `libasound` por `dlopen` (643 linhas, com FFI manual).
//! Copiar aquilo para cá traria o mesmo poder e o mesmo custo de manutenção para
//! **uma** função: subir e descer 5 %. `amixer` já faz exatamente isso, existe na
//! imagem (medido: `/usr/bin/amixer`) e é chamado **no evento**, não por quadro.
//!
//! O limite fica registrado: quando `amixer` não existe — o R35S não tem
//! `libasound` e portanto não tem `amixer` — o módulo degrada para "sem
//! controle", avisa **uma vez** e o jogo continua. Nunca derruba nada e nunca
//! aparece como erro por quadro.
//!
//! ## Por que a lógica é separada do processo
//!
//! `Volume` é genérico sobre [`Mixer`], então passo, limite e persistência têm
//! teste sem placa de som e sem `fork` — e o [`Alsa`] é a única parte que depende
//! de hardware.

use std::process::Command;

/// Onde o volume escolhido é gravado — o **mesmo** arquivo que o launcher lê no
/// boot (`/user/.config/giochi/volume`). Um arquivo, um dono por vez.
pub const VOLUME_SALVO: &str = "/user/.config/giochi/volume";
/// Passo de cada toque, em pontos percentuais.
pub const PASSO: u8 = 5;

/// Interface de mixer do sistema.
pub trait Mixer {
    /// Volume atual em 0..=100, ou `None` quando não há controle.
    fn ler(&mut self) -> Option<u8>;
    /// Aplica um volume em 0..=100. `false` quando não foi possível.
    fn definir(&mut self, pct: u8) -> bool;
}

/// Controle de volume do jogo.
#[derive(Debug, Clone)]
pub struct Volume<M: Mixer> {
    mixer: M,
    /// Último valor conhecido.
    nivel: u8,
    /// `false` quando o mixer não respondeu — o chamador para de perguntar.
    disponivel: bool,
}

impl<M: Mixer> Volume<M> {
    /// Lê o volume do sistema na abertura.
    pub fn novo(mut mixer: M) -> Self {
        let (nivel, disponivel) = match mixer.ler() {
            Some(pct) => (pct.min(100), true),
            None => (100, false),
        };
        Self {
            mixer,
            nivel,
            disponivel,
        }
    }

    /// O mixer respondeu?
    pub fn disponivel(&self) -> bool {
        self.disponivel
    }

    /// Volume atual em 0..=100.
    pub fn atual(&self) -> u8 {
        self.nivel
    }

    /// Aumenta em [`PASSO`], saturando em 100.
    pub fn aumentar(&mut self) -> u8 {
        self.definir(self.nivel.saturating_add(PASSO))
    }

    /// Diminui em [`PASSO`], saturando em 0.
    pub fn diminuir(&mut self) -> u8 {
        self.definir(self.nivel.saturating_sub(PASSO))
    }

    /// Aplica `pct`, grava e devolve o que ficou de fato.
    ///
    /// O valor devolvido é o **aplicado**, não o pedido: se o mixer recusar, o
    /// nível local não pode divergir do que o alto-falante está fazendo.
    pub fn definir(&mut self, pct: u8) -> u8 {
        if !self.disponivel {
            return self.nivel;
        }
        let alvo = pct.min(100);
        if !self.mixer.definir(alvo) {
            // Uma recusa só: se o mixer sumiu, ele não volta no meio da partida.
            self.disponivel = false;
            log::warn!("volume: o mixer parou de responder; controle desligado");
            return self.nivel;
        }
        self.nivel = alvo;
        gravar(alvo);
        self.nivel
    }
}

/// Grava o volume, para o launcher restaurar no próximo boot.
fn gravar(pct: u8) {
    let caminho = std::path::Path::new(VOLUME_SALVO);
    if let Some(pai) = caminho.parent() {
        if let Err(erro) = std::fs::create_dir_all(pai) {
            log::debug!("volume: não consegui criar {pai:?} ({erro})");
            return;
        }
    }
    if let Err(erro) = std::fs::write(caminho, format!("{pct}\n")) {
        log::debug!("volume: não consegui gravar {caminho:?} ({erro})");
    }
}

/// Volume via `amixer` — o mesmo `Master` que o launcher controla.
#[derive(Debug, Default, Clone, Copy)]
pub struct Alsa;

impl Alsa {
    /// Placa padrão do aparelho. `-c 0` porque o `rk817` é a placa 0 nos
    /// aparelhos medidos; sem `-c`, o `amixer` usa a `default`, que no Buildroot
    /// aponta para a mesma.
    fn comando() -> Command {
        let mut comando = Command::new("amixer");
        comando.arg("-c").arg("0");
        comando
    }
}

/// Extrai a primeira porcentagem `[NN%]` da saída do `amixer`.
fn primeira_porcentagem(saida: &str) -> Option<u8> {
    let inicio = saida.find('[')?;
    let resto = &saida[inicio + 1..];
    let fim = resto.find('%')?;
    resto[..fim].trim().parse::<u8>().ok().map(|p| p.min(100))
}

impl Mixer for Alsa {
    fn ler(&mut self) -> Option<u8> {
        let saida = Self::comando()
            .args(["sget", "Master"])
            .output()
            .ok()?
            .stdout;
        primeira_porcentagem(&String::from_utf8_lossy(&saida))
    }

    fn definir(&mut self, pct: u8) -> bool {
        Self::comando()
            .args(["sset", "Master", &format!("{}%", pct.min(100))])
            .output()
            .map(|saida| saida.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mixer de mentira: guarda o que recebeu e pode recusar.
    #[derive(Default)]
    struct Falso {
        nivel: Option<u8>,
        recusa: bool,
        chamadas: Vec<u8>,
    }

    impl Mixer for Falso {
        fn ler(&mut self) -> Option<u8> {
            self.nivel
        }
        fn definir(&mut self, pct: u8) -> bool {
            if self.recusa {
                return false;
            }
            self.chamadas.push(pct);
            self.nivel = Some(pct);
            true
        }
    }

    fn volume_com(nivel: Option<u8>) -> Volume<Falso> {
        Volume::novo(Falso {
            nivel,
            ..Falso::default()
        })
    }

    #[test]
    fn le_o_volume_do_sistema_na_abertura() {
        assert_eq!(volume_com(Some(75)).atual(), 75);
        assert!(volume_com(Some(75)).disponivel());
    }

    #[test]
    fn sem_mixer_o_controle_fica_indisponivel_e_nao_finge() {
        let mut volume = volume_com(None);
        assert!(!volume.disponivel());
        assert_eq!(volume.atual(), 100);
        // Pedir para mudar não pode inventar um nível novo.
        assert_eq!(volume.aumentar(), 100);
        assert_eq!(volume.diminuir(), 100);
        assert!(volume.mixer.chamadas.is_empty(), "chamou o mixer sem poder");
    }

    #[test]
    fn o_passo_e_de_cinco_em_cinco() {
        let mut volume = volume_com(Some(50));
        assert_eq!(volume.aumentar(), 55);
        assert_eq!(volume.aumentar(), 60);
        assert_eq!(volume.diminuir(), 55);
        assert_eq!(volume.mixer.chamadas, vec![55, 60, 55]);
    }

    #[test]
    fn os_limites_saturam_sem_dar_a_volta() {
        // `saturating_*` e não `+`/`-`: com `u8` cru, diminuir de 0 daria 251.
        let mut volume = volume_com(Some(100));
        assert_eq!(volume.aumentar(), 100);
        let mut volume = volume_com(Some(0));
        assert_eq!(volume.diminuir(), 0);
        // E de 98 sobe para 100, não para 103.
        let mut volume = volume_com(Some(98));
        assert_eq!(volume.aumentar(), 100);
        // De 3 desce para 0, não para 254.
        let mut volume = volume_com(Some(3));
        assert_eq!(volume.diminuir(), 0);
    }

    #[test]
    fn acima_de_cem_e_preso_na_leitura() {
        let volume = volume_com(Some(200));
        assert_eq!(volume.atual(), 100);
    }

    #[test]
    fn a_recusa_do_mixer_nao_muda_o_nivel_local() {
        // O nível mostrado tem de ser o que o alto-falante está fazendo. Se o
        // mixer recusa e o app acredita que mudou, o jogador fica com um número
        // na tela que não corresponde a som nenhum.
        let mut volume = Volume::novo(Falso {
            nivel: Some(70),
            recusa: true,
            chamadas: Vec::new(),
        });
        assert_eq!(volume.aumentar(), 70);
        assert_eq!(volume.atual(), 70);
        assert!(
            !volume.disponivel(),
            "um mixer que recusou não pode continuar sendo consultado"
        );
    }

    #[test]
    fn a_primeira_porcentagem_e_a_lida() {
        // Saída real do `amixer -c 0 sget Master` do X35H, com as duas linhas de
        // canal: a primeira porcentagem é a do canal esquerdo, e é a que vale.
        let amostra = "\
Simple mixer control 'Master',0
  Capabilities: pvolume cvolume
  Limits: Playback 0 - 255 Capture 0 - 255
  Front Left: Playback 255 [100%] [0.00dB] Capture 255 [100%] [0.00dB]
  Front Right: Playback 255 [100%] [0.00dB] Capture 255 [100%] [0.00dB]";
        assert_eq!(primeira_porcentagem(amostra), Some(100));

        let meio = "  Front Left: Playback 128 [50%] [-6.00dB]";
        assert_eq!(primeira_porcentagem(meio), Some(50));

        let zero = "  Mono: Playback 0 [0%] [-inf]";
        assert_eq!(primeira_porcentagem(zero), Some(0));
    }

    #[test]
    fn saida_estranha_nao_vira_volume_zero() {
        // Um `amixer` que falhou imprime erro e nenhuma porcentagem. Devolver
        // `0` faria o jogo começar mudo sem motivo.
        for ruim in [
            "amixer: Unable to find simple control 'Master',0",
            "",
            "[]",
            "[abc%]",
        ] {
            assert_eq!(primeira_porcentagem(ruim), None, "entrada {ruim:?}");
        }
    }

    #[test]
    fn a_porcentagem_acima_de_cem_e_presa() {
        assert_eq!(primeira_porcentagem("[150%]"), Some(100));
    }
}
