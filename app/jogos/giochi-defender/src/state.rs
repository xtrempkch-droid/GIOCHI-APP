//! Estado compartilhado entre as cenas.
//!
//! Só dados e regras: nada aqui conhece SDL, GL ou desenho. É o que permite
//! testar "perder a última vida termina o jogo" sem abrir janela — e o que faz a
//! mesma regra valer para o menu, para o HUD e para a cena de derrota.

/// Vidas com que uma partida começa.
pub const VIDAS_INICIAIS: i32 = 5;
/// Waves da partida. A terceira vencida é a vitória.
pub const WAVES_TOTAIS: u32 = 3;

/// O que a partida acumulou até agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameState {
    /// Pontos somados.
    pub score: u32,
    /// Vidas restantes.
    pub lives: i32,
    /// Wave atual, começando em 1.
    pub wave: u32,
    /// Partida pausada: a simulação para, o desenho continua.
    pub paused: bool,
}

impl GameState {
    /// Partida nova.
    pub fn new() -> Self {
        Self {
            score: 0,
            lives: VIDAS_INICIAIS,
            wave: 1,
            paused: false,
        }
    }

    /// Volta ao estado inicial — o `R` da tela de derrota.
    pub fn reiniciar(&mut self) {
        *self = Self::new();
    }

    /// Soma pontos.
    pub fn pontuar(&mut self, pontos: u32) {
        self.score = self.score.saturating_add(pontos);
    }

    /// Tira uma vida e responde se o jogador chegou a zero.
    ///
    /// Devolver o "acabou" em vez de deixar o chamador comparar evita a cópia da
    /// regra em três cenas — e a divergência que vem junto.
    pub fn perdeu_vida(&mut self) -> bool {
        self.lives = (self.lives - 1).max(0);
        self.lives == 0
    }

    /// Sem vidas, a partida acabou.
    pub fn acabou(&self) -> bool {
        self.lives <= 0
    }

    /// Venceu a última wave?
    pub fn venceu(&self) -> bool {
        self.wave > WAVES_TOTAIS
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partida_comeca_com_cinco_vidas_e_na_primeira_wave() {
        let estado = GameState::new();
        assert_eq!(estado.lives, VIDAS_INICIAIS);
        assert_eq!(estado.lives, 5);
        assert_eq!(estado.wave, 1);
        assert_eq!(estado.score, 0);
        assert!(!estado.paused);
    }

    #[test]
    fn a_quinta_perda_termina_a_partida() {
        let mut estado = GameState::new();
        for _ in 0..VIDAS_INICIAIS - 1 {
            assert!(
                !estado.perdeu_vida(),
                "o jogo não podia acabar antes da hora"
            );
        }
        assert!(
            estado.perdeu_vida(),
            "a última vida tinha de terminar o jogo"
        );
        assert!(estado.acabou());
    }

    #[test]
    fn a_vida_nao_fica_negativa() {
        // `lives: i32` e não `u32` justamente para o caso de um dano duplo
        // escapar; mas o número mostrado no HUD não pode virar "-1".
        let mut estado = GameState::new();
        for _ in 0..20 {
            estado.perdeu_vida();
        }
        assert_eq!(estado.lives, 0);
    }

    #[test]
    fn reiniciar_devolve_tudo_ao_inicio() {
        let mut estado = GameState::new();
        estado.pontuar(1200);
        estado.wave = 3;
        estado.paused = true;
        estado.perdeu_vida();
        estado.reiniciar();
        assert_eq!(estado, GameState::new());
    }

    #[test]
    fn pontuar_nao_estoura() {
        let mut estado = GameState::new();
        estado.pontuar(u32::MAX);
        estado.pontuar(10);
        assert_eq!(estado.score, u32::MAX);
    }

    #[test]
    fn a_vitoria_exige_passar_da_ultima_wave() {
        let mut estado = GameState::new();
        estado.wave = WAVES_TOTAIS;
        assert!(!estado.venceu(), "a wave 3 ainda está em jogo");
        estado.wave = WAVES_TOTAIS + 1;
        assert!(estado.venceu());
    }
}
