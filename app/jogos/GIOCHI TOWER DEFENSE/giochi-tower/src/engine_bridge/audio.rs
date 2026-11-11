//! Áudio: fachada mínima sobre `AudioSystem`.
//!
//! O M1 não tem trilha nem efeitos — o GDD §18 pede áudio no polimento (M6). O
//! que existe aqui é a porta de entrada pronta, para que M6 não precise mexer no
//! bootstrap nem descobrir de novo as duas armadilhas da engine:
//!
//! 1. `AudioSystem::new` **não falha** quando não há dispositivo — o objeto fica
//!    `is_available() == false` e `load_sound`/`play_sound` passam a devolver
//!    erro. Um jogo que propaga esse erro morre em servidor sem placa de som.
//! 2. O volume é `0..=128` (`MAX_VOLUME`), não `0..=100`. Passar 100 "funciona"
//!    e deixa o jogo a 78% do volume que o autor imaginou.

use giochi_engine::audio::{AudioSystem, SoundHandle, MAX_VOLUME};
use giochi_engine::platform::Platform;

/// Volume inicial, em escala da engine (`0..=128`).
pub const DEFAULT_VOLUME: i32 = 96;

/// Fachada de áudio do jogo.
pub struct AudioBridge {
    system: Option<AudioSystem>,
    volume: i32,
}

impl AudioBridge {
    /// Abre o mixer, se houver dispositivo.
    ///
    /// Ausência de áudio é registrada e o jogo segue: no aparelho, sem placa, o
    /// jogo tem de rodar do mesmo jeito.
    pub fn new(platform: &Platform) -> Self {
        let system = match AudioSystem::new(platform) {
            Ok(mut audio) => {
                if audio.is_available() {
                    audio.set_volume(DEFAULT_VOLUME);
                    log::info!("áudio: mixer aberto, volume {DEFAULT_VOLUME}/{MAX_VOLUME}");
                    Some(audio)
                } else {
                    log::warn!("áudio: sem dispositivo — o jogo segue em silêncio");
                    None
                }
            }
            Err(err) => {
                log::warn!("áudio indisponível ({err}) — o jogo segue em silêncio");
                None
            }
        };

        Self {
            system,
            volume: DEFAULT_VOLUME,
        }
    }

    /// Há mixer utilizável.
    pub fn available(&self) -> bool {
        self.system.is_some()
    }

    /// Volume atual, na escala da engine.
    pub fn volume(&self) -> i32 {
        self.volume
    }

    /// Ajusta o volume, limitado a `0..=MAX_VOLUME`.
    pub fn set_volume(&mut self, volume: i32) {
        self.volume = volume.clamp(0, MAX_VOLUME);
        if let Some(system) = self.system.as_mut() {
            system.set_volume(self.volume);
        }
    }

    /// Carrega um som do disco. Devolve `None` quando não há áudio ou o arquivo
    /// não é aceito — nunca derruba o jogo por causa de um `.wav`.
    pub fn load_sound(&mut self, path: &str) -> Option<SoundHandle> {
        let system = self.system.as_mut()?;
        match system.load_sound(path) {
            Ok(handle) => Some(handle),
            Err(err) => {
                log::warn!("áudio: falha ao carregar '{path}': {err}");
                None
            }
        }
    }

    /// Toca um som uma vez.
    pub fn play(&mut self, handle: SoundHandle) {
        if let Some(system) = self.system.as_mut() {
            if let Err(err) = system.play_sound(handle, 0) {
                log::warn!("áudio: falha ao tocar: {err}");
            }
        }
    }

    /// Para todos os sons.
    pub fn stop_all(&mut self) {
        if let Some(system) = self.system.as_mut() {
            system.stop_all_sounds();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_volume_is_inside_the_engine_range() {
        assert!((0..=MAX_VOLUME).contains(&DEFAULT_VOLUME));
    }
}
