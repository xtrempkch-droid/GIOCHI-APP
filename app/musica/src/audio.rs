//! O mixer, encapsulado.
//!
//! O app fala com o [`AudioSystem`] da engine — a API de **música** dela
//! (`load_music`/`play_music`/`pause_music`), que faz streaming e não carrega a
//! faixa inteira na memória.
//!
//! ## O que o exemplo usava, e por que é outro caminho
//!
//! O app de exemplo chamava `play_sound` — o caminho de **efeito sonoro**, que
//! decodifica o arquivo inteiro para a RAM. Um álbum de 40 minutos viraria
//! ~400 MB de PCM num aparelho com 1 GB. Aqui a faixa é transmitida pelo mixer,
//! e só uma está aberta por vez.
//!
//! ## Nada falha em silêncio
//!
//! Toda chamada que pode falhar devolve `Err(String)` **com o motivo do mixer**,
//! e o app mostra essa frase na barra de transporte. Foi assim que este projeto
//! aprendeu que `libasound` ausente virava volume `—` sem explicação: um
//! `Result` descartado num `let _ =` é indistinguível de sucesso.

use giochi_engine::{AudioSystem, MusicHandle, MusicState, Platform};

/// Estado do mixer do app.
pub struct Som {
    sistema: Option<AudioSystem>,
    /// Handle da faixa aberta no mixer (uma por vez).
    atual: Option<MusicHandle>,
    /// Caminho da faixa aberta, para não recarregar do disco ao retomar.
    caminho: Option<String>,
    volume: i32,
    /// Por que o áudio não está disponível, quando não está.
    aviso: Option<String>,
}

impl Som {
    /// Abre o mixer.
    ///
    /// Falhar aqui **não** derruba o app: sem áudio, a interface continua
    /// navegável e a barra de transporte diz o motivo. Um tocador que não abre
    /// porque a placa não abriu não deixa nem o usuário descobrir qual é o
    /// problema.
    pub fn abrir(platform: &Platform, volume: i32, com_som: bool) -> Som {
        if !com_som {
            eprintln!("som: desligado por --sem-som");
            return Som::sem_som("áudio desligado (--sem-som)".to_string(), volume);
        }
        match AudioSystem::new(platform) {
            Ok(mut sistema) => {
                sistema.set_volume(volume);
                let (major, minor, patch) = giochi_engine::audio::mixer_version();
                eprintln!(
                    "som: mixer {major}.{minor}.{patch}, volume {volume} — {}",
                    if sistema.is_available() {
                        "dispositivo aberto"
                    } else {
                        "sem dispositivo (roda silencioso)"
                    }
                );
                Som {
                    aviso: if sistema.is_available() {
                        None
                    } else {
                        Some("o mixer abriu sem dispositivo de som".to_string())
                    },
                    sistema: Some(sistema),
                    atual: None,
                    caminho: None,
                    volume,
                }
            }
            Err(e) => {
                eprintln!("som: mixer indisponível: {e}");
                Som::sem_som(format!("sem áudio: {e}"), volume)
            }
        }
    }

    fn sem_som(aviso: String, volume: i32) -> Som {
        Som {
            sistema: None,
            atual: None,
            caminho: None,
            volume,
            aviso: Some(aviso),
        }
    }

    /// Motivo de não ter áudio, se for o caso.
    pub fn aviso(&self) -> Option<&str> {
        self.aviso.as_deref()
    }

    /// Volume atual do mixer (o que foi pedido, não o que a placa aceitou —
    /// o `SDL_mixer` não devolve o volume efetivo).
    #[allow(dead_code)]
    pub fn volume(&self) -> i32 {
        self.volume
    }

    /// Abre (se preciso) e toca a faixa.
    ///
    /// Reaproveita o handle quando é a **mesma** faixa: recarregar do disco
    /// para tocar de novo custaria abrir e decodificar o arquivo outra vez.
    pub fn tocar(&mut self, caminho: &str) -> Result<(), String> {
        let Some(sistema) = self.sistema.as_mut() else {
            return Err(self
                .aviso
                .clone()
                .unwrap_or_else(|| "sem áudio".to_string()));
        };

        if self.caminho.as_deref() != Some(caminho) || self.atual.is_none() {
            sistema.stop_music();
            if let Some(handle) = self.atual.take() {
                // `destroy_music` para a reprodução **qualquer que seja** o
                // handle: o `SDL_mixer` não diz qual `Mix_Music` está tocando,
                // e liberar uma faixa em execução é use-after-free na thread do
                // mixer. Parar antes é obrigatório, não precaução.
                sistema.destroy_music(handle);
            }
            let handle = sistema
                .load_music(caminho)
                .map_err(|e| format!("não consegui abrir {caminho}: {e}"))?;
            self.atual = Some(handle);
            self.caminho = Some(caminho.to_string());
        }

        let handle = self.atual.ok_or_else(|| "faixa sem handle".to_string())?;
        sistema
            .play_music(handle, 0)
            .map_err(|e| format!("não consegui tocar {caminho}: {e}"))
    }

    pub fn pausar(&mut self) {
        if let Some(sistema) = self.sistema.as_mut() {
            sistema.pause_music();
        }
    }

    pub fn retomar(&mut self) {
        if let Some(sistema) = self.sistema.as_mut() {
            sistema.resume_music();
        }
    }

    pub fn parar(&mut self) {
        if let Some(sistema) = self.sistema.as_mut() {
            sistema.stop_music();
        }
    }

    pub fn definir_volume(&mut self, volume: i32) {
        self.volume = volume;
        if let Some(sistema) = self.sistema.as_mut() {
            sistema.set_volume(volume);
        }
    }

    /// Estado do mixer, para o app perceber que a faixa terminou.
    ///
    /// `Stopped` com uma faixa marcada como tocando significa "acabou": é assim
    /// que o fim do arquivo chega, já que o mixer não tem callback de término.
    pub fn estado(&self) -> MusicState {
        match self.sistema.as_ref() {
            Some(sistema) => sistema.music_state(),
            None => MusicState::Stopped,
        }
    }

    /// Solta o mixer e o que estiver aberto.
    pub fn encerrar(&mut self) {
        if let Some(sistema) = self.sistema.as_mut() {
            sistema.stop_music();
            if let Some(handle) = self.atual.take() {
                sistema.destroy_music(handle);
            }
        }
        self.caminho = None;
    }
}
