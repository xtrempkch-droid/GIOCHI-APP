//! Estado compartilhado entre as cenas.
//!
//! Só dados e regras: nada aqui conhece SDL, GL ou desenho. É o que permite
//! testar "perder a última vida termina o jogo" sem abrir janela — e o que faz a
//! mesma regra valer para o menu, para o HUD e para a cena de derrota.

/// Vidas com que uma partida começa.
pub const VIDAS_INICIAIS: i32 = 5;
/// Waves da partida. A terceira vencida é a vitória.
pub const WAVES_TOTAIS: u32 = 3;

/// Abates que a **primeira** onda exige.
pub const ABATES_DA_ONDA_1: u32 = 8;
/// Quanto a cota cresce por onda.
pub const CRESCIMENTO_DA_ONDA: u32 = 4;

/// Quantos abates a onda `wave` exige para terminar.
///
/// A cota cresce porque a dificuldade tem de crescer em algum lugar: o spawn é
/// constante (2 s), então sem cota crescente a onda 3 seria igual à 1.
pub fn cota_da_onda(wave: u32) -> u32 {
    let wave = wave.max(1);
    ABATES_DA_ONDA_1 + (wave - 1) * CRESCIMENTO_DA_ONDA
}

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
    /// Abates já feitos nesta onda.
    pub abates_na_onda: u32,
}

impl GameState {
    /// Partida nova.
    pub fn new() -> Self {
        Self {
            score: 0,
            lives: VIDAS_INICIAIS,
            wave: 1,
            paused: false,
            abates_na_onda: 0,
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

    /// Registra um abate. Devolve `Some(nova_onda)` quando a cota da onda foi
    /// cumprida e a próxima começou.
    ///
    /// O `Option` existe para o som e o aviso: quem chama precisa saber **que**
    /// virou a onda, e comparar `wave` antes e depois no chamador seria a mesma
    /// regra escrita duas vezes.
    ///
    /// Abates depois da última onda não fazem nada: a partida já terminou e
    /// `venceu()` é quem responde por isso.
    pub fn registrou_abate(&mut self) -> Option<u32> {
        if self.acabou() || self.venceu() {
            return None;
        }
        self.abates_na_onda = self.abates_na_onda.saturating_add(1);
        if self.abates_na_onda < cota_da_onda(self.wave) {
            return None;
        }
        self.abates_na_onda = 0;
        self.wave = self.wave.saturating_add(1);
        Some(self.wave)
    }

    /// Abates que faltam para a onda atual terminar.
    pub fn faltam_para_a_onda(&self) -> u32 {
        cota_da_onda(self.wave).saturating_sub(self.abates_na_onda)
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
    fn a_cota_da_onda_cresce_e_a_primeira_e_a_menor() {
        assert_eq!(cota_da_onda(1), ABATES_DA_ONDA_1);
        assert_eq!(cota_da_onda(2), ABATES_DA_ONDA_1 + CRESCIMENTO_DA_ONDA);
        assert!(cota_da_onda(3) > cota_da_onda(2));
        // Wave 0 não existe, mas a função não pode devolver zero para ela: uma
        // cota zero avançaria a onda a cada quadro.
        assert_eq!(cota_da_onda(0), cota_da_onda(1));
    }

    #[test]
    fn a_onda_so_vira_quando_a_cota_e_cumprida() {
        let mut estado = GameState::new();
        for i in 1..cota_da_onda(1) {
            assert_eq!(estado.registrou_abate(), None, "abate {i} virou cedo");
            assert_eq!(estado.wave, 1);
        }
        assert_eq!(
            estado.registrou_abate(),
            Some(2),
            "o último abate tinha de virar a onda"
        );
        assert_eq!(estado.wave, 2);
        assert_eq!(estado.abates_na_onda, 0, "a contagem recomeça");
    }

    #[test]
    fn tres_ondas_cumpridas_vencem_a_partida() {
        // Antes disto, `wave` nunca era escrito e a vitória era inalcançável: o
        // `venceu()` existia, o teste dele passava, e nenhum caminho chegava lá.
        let mut estado = GameState::new();
        for onda in 1..=WAVES_TOTAIS {
            assert_eq!(estado.wave, onda);
            assert!(!estado.venceu(), "venceu antes da hora na onda {onda}");
            for _ in 0..cota_da_onda(onda) {
                estado.registrou_abate();
            }
        }
        assert_eq!(estado.wave, WAVES_TOTAIS + 1);
        assert!(estado.venceu());
    }

    #[test]
    fn abate_depois_do_fim_nao_conta() {
        let mut estado = GameState::new();
        for _ in 0..cota_da_onda(1) {
            estado.registrou_abate();
        }
        estado.wave = WAVES_TOTAIS + 1;
        let antes = estado.abates_na_onda;
        assert_eq!(estado.registrou_abate(), None);
        assert_eq!(
            estado.abates_na_onda, antes,
            "a contagem andou depois do fim"
        );

        let mut perdido = GameState::new();
        for _ in 0..VIDAS_INICIAIS {
            perdido.perdeu_vida();
        }
        assert_eq!(perdido.registrou_abate(), None);
    }

    #[test]
    fn faltam_para_a_onda_diminui_a_cada_abate() {
        // Laço **com contagem**, não `while faltam > 0`: a cada onda a cota
        // reinicia, então a condição nunca fica falsa e a primeira versão deste
        // teste girou até o `cargo test` desistir.
        let mut estado = GameState::new();
        assert_eq!(estado.faltam_para_a_onda(), cota_da_onda(1));
        for esperado in (0..cota_da_onda(1)).rev() {
            assert_eq!(estado.faltam_para_a_onda(), esperado + 1);
            estado.registrou_abate();
        }
        // Cumprida a cota, a onda virou e a contagem recomeça na cota nova.
        assert_eq!(estado.wave, 2);
        assert_eq!(estado.faltam_para_a_onda(), cota_da_onda(2));
    }

    #[test]
    fn reiniciar_zera_a_contagem_da_onda() {
        let mut estado = GameState::new();
        estado.registrou_abate();
        estado.registrou_abate();
        estado.reiniciar();
        assert_eq!(estado.abates_na_onda, 0);
        assert_eq!(estado.wave, 1);
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
