//! Pontuação de drift: combo, banco de pontos e o que se perde ao bater.
//!
//! ## A regra que faz o drift valer a pena
//!
//! Drift sem consequência é enfeite: o jogador vira normal, que é mais rápido.
//! Aqui ele tem **três** incentivos que se somam:
//!
//! 1. **pontos por segundo** proporcionais à velocidade × deslize;
//! 2. **multiplicador** que cresce enquanto o drift continua (até ×5) — então
//!    emendar duas curvas num drift só vale muito mais que duas derrapadas
//!    curtas;
//! 3. **nitro**: derrapar recarrega (`NITRO_RECARGA_DRIFT`), ou seja, o drift
//!    paga a velocidade que ele custou.
//!
//! E o que se perde: bater zera o combo **em curso**, mas nunca os pontos já
//! banhados. Zerar o total transformaria um erro em "perdi a partida"; zerar só
//! o combo mantém a tensão de "não quero bater agora".

/// Pontos por unidade de (velocidade × deslize × segundo).
///
/// Calibrado para uma volta de ~40 s render entre 8 e 15 mil pontos quando o
/// jogador derrapa nas curvas: número grande o suficiente para parecer placar,
/// pequeno o suficiente para caber no HUD sem notação científica.
pub const GANHO: f32 = 0.85;
/// Multiplicador máximo do combo.
pub const COMBO_MAX: f32 = 5.0;
/// Segundos de drift para o combo chegar ao máximo.
pub const COMBO_TEMPO: f32 = 4.0;
/// Deslize mínimo (m/s) para o drift existir.
///
/// Sem ele, "derrapando" com deslize zero entrava em combo e ficava ativo sem
/// pontuar — o HUD anunciava DRIFT com o carro andando em linha reta.
pub const DESLIZE_MIN: f32 = 1.2;

/// Estado da pontuação de drift.
#[derive(Debug, Clone, Default)]
pub struct Drift {
    /// Pontos já garantidos (sobrevivem a batida).
    pub pontos: u32,
    /// Pontos do drift em curso, ainda não garantidos.
    pub atual: f32,
    /// Multiplicador do drift em curso, 1..`COMBO_MAX`.
    pub combo: f32,
    /// Melhor drift único da corrida.
    pub melhor: u32,
    /// `true` enquanto o carro derrapa.
    pub ativo: bool,
    /// Duração do drift em curso.
    pub tempo: f32,
    /// Quanto entrou no último drift fechado — usado no aviso da tela.
    pub ultimo_ganho: u32,
}

impl Drift {
    /// Nova pontuação zerada.
    pub fn novo() -> Self {
        Self {
            combo: 1.0,
            ..Self::default()
        }
    }

    /// Total que o jogador vê: o garantido mais o que está em curso.
    pub fn total(&self) -> u32 {
        self.pontos + self.atual as u32
    }

    /// Multiplicador arredondado para o HUD (`x1`..`x5`).
    pub fn combo_inteiro(&self) -> u32 {
        self.combo.round().clamp(1.0, COMBO_MAX) as u32
    }

    /// Atualiza um passo.
    ///
    /// `derrapando` vem do carro; `velocidade` e `derrapagem` também. A função
    /// é pura em relação ao mundo: não conhece pista nem câmera, então o teste
    /// injeta números e confere a conta.
    pub fn passo(&mut self, derrapando: bool, velocidade: f32, derrapagem: f32, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if derrapando && velocidade > 0.0 && derrapagem.abs() >= DESLIZE_MIN {
            if !self.ativo {
                self.ativo = true;
                self.tempo = 0.0;
                self.atual = 0.0;
                self.combo = 1.0;
            }
            self.tempo += dt;
            // Combo cresce linear até `COMBO_MAX` em `COMBO_TEMPO` segundos.
            self.combo = (1.0 + (self.tempo / COMBO_TEMPO) * (COMBO_MAX - 1.0)).min(COMBO_MAX);
            let ganho = velocidade * derrapagem.abs() * GANHO * self.combo * dt;
            self.atual += ganho.max(0.0);
        } else if self.ativo {
            self.fechar();
        }
    }

    /// Encerra o drift em curso e garante os pontos.
    fn fechar(&mut self) {
        let ganho = self.atual as u32;
        self.pontos = self.pontos.saturating_add(ganho);
        self.melhor = self.melhor.max(ganho);
        self.ultimo_ganho = ganho;
        self.atual = 0.0;
        self.combo = 1.0;
        self.ativo = false;
        self.tempo = 0.0;
    }

    /// Batida: perde o drift em curso, mantém o que já foi garantido.
    pub fn colidir(&mut self) {
        self.atual = 0.0;
        self.combo = 1.0;
        self.ativo = false;
        self.tempo = 0.0;
    }

    /// Zera tudo (reinício da corrida).
    pub fn reiniciar(&mut self) {
        *self = Self::novo();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn derrapar(d: &mut Drift, segundos: f32, velocidade: f32, deslize: f32) {
        let dt = 1.0 / 75.0;
        for _ in 0..(segundos / dt).round() as u32 {
            d.passo(true, velocidade, deslize, dt);
        }
    }

    #[test]
    fn drift_sem_derrapagem_nao_pontua() {
        let mut d = Drift::novo();
        derrapar(&mut d, 2.0, 40.0, 0.0);
        assert_eq!(d.total(), 0);
        assert!(!d.ativo);
    }

    #[test]
    fn drift_pontua_e_o_combo_cresce() {
        let mut d = Drift::novo();
        derrapar(&mut d, 1.0, 40.0, 6.0);
        let primeira_metade = d.atual;
        assert!(primeira_metade > 0.0, "1 s de drift não pontuou");
        assert!(d.combo > 1.0, "combo ficou em {}", d.combo);
        derrapar(&mut d, 3.0, 40.0, 6.0);
        assert!(d.combo > 4.0, "combo devia chegar perto do máximo: {}", d.combo);
    }

    #[test]
    fn combo_satura_no_maximo() {
        let mut d = Drift::novo();
        derrapar(&mut d, 30.0, 40.0, 6.0);
        assert!(d.combo <= COMBO_MAX + 0.001, "combo passou do máximo: {}", d.combo);
        assert_eq!(d.combo_inteiro(), 5);
    }

    #[test]
    fn um_drift_longo_vale_mais_que_varios_curtos() {
        // É o coração do "drift divertido": emendar tem de compensar.
        let mut longo = Drift::novo();
        derrapar(&mut longo, 4.0, 40.0, 6.0);
        longo.passo(false, 40.0, 0.0, 1.0 / 75.0);
        let pontos_longo = longo.pontos;

        let mut curtos = Drift::novo();
        for _ in 0..8 {
            derrapar(&mut curtos, 0.5, 40.0, 6.0);
            curtos.passo(false, 40.0, 0.0, 1.0 / 75.0);
        }
        let pontos_curtos = curtos.pontos;

        assert!(
            (pontos_longo as f32) > pontos_curtos as f32 * 1.4,
            "4 s de uma vez ({pontos_longo}) contra 8×0,5 s ({pontos_curtos}): o combo não está pagando"
        );
    }

    #[test]
    fn ao_soltar_o_drift_os_pontos_sao_garantidos() {
        let mut d = Drift::novo();
        derrapar(&mut d, 2.0, 40.0, 6.0);
        assert_eq!(d.pontos, 0, "nada é garantido antes de soltar");
        assert!(d.ativo);
        d.passo(false, 40.0, 0.0, 1.0 / 75.0);
        assert!(!d.ativo);
        assert!(d.pontos > 0, "soltar o drift não garantiu os pontos");
        assert!(d.melhor > 0);
        assert_eq!(d.combo, 1.0);
    }

    #[test]
    fn bater_perde_o_drift_em_curso_mas_nao_o_garantido() {
        let mut d = Drift::novo();
        derrapar(&mut d, 2.0, 40.0, 6.0);
        d.passo(false, 40.0, 0.0, 1.0 / 75.0);
        let garantido = d.pontos;
        assert!(garantido > 0);

        derrapar(&mut d, 1.5, 40.0, 6.0);
        assert!(d.atual > 0.0);
        d.colidir();
        assert_eq!(d.pontos, garantido, "a batida não pode apagar o que já era do jogador");
        assert_eq!(d.atual, 0.0);
        assert_eq!(d.combo, 1.0);
    }

    #[test]
    fn veloz_derrapa_mais_que_devagar() {
        let mut devagar = Drift::novo();
        derrapar(&mut devagar, 1.0, 15.0, 5.0);
        let mut rapido = Drift::novo();
        derrapar(&mut rapido, 1.0, 55.0, 5.0);
        assert!(
            rapido.atual > devagar.atual * 2.0,
            "a 55 m/s rendeu {:.0} e a 15 m/s rendeu {:.0}",
            rapido.atual,
            devagar.atual
        );
    }

    #[test]
    fn reiniciar_zera_tudo() {
        let mut d = Drift::novo();
        derrapar(&mut d, 2.0, 40.0, 6.0);
        d.passo(false, 40.0, 0.0, 1.0 / 75.0);
        d.reiniciar();
        assert_eq!(d.total(), 0);
        assert_eq!(d.melhor, 0);
        assert_eq!(d.combo, 1.0);
        assert!(!d.ativo);
    }

    #[test]
    fn total_soma_o_em_curso() {
        let mut d = Drift::novo();
        derrapar(&mut d, 1.0, 40.0, 6.0);
        assert_eq!(d.pontos, 0);
        assert_eq!(d.total(), d.atual as u32);
    }
}
