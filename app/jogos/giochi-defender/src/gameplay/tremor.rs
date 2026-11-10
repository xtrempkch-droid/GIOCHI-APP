//! Tremor de câmera: a resposta física ao dano.
//!
//! ## Por que existe
//!
//! Um clarão vermelho avisa que **algo** aconteceu; o tremor avisa **onde** — foi
//! no asteroide que o jogador está olhando. Sem ele, o dano em 640×480 é um
//! retângulo de borda que some em meio segundo e não diz de que lado veio.
//!
//! ## Por que não é aleatório
//!
//! O deslocamento é uma soma de duas senoides com períodos primos entre si
//! (0,083 s e 0,131 s). Ruído daria um tremor diferente a cada quadro — e num
//! jogo que se mede por captura de quadro, "diferente a cada quadro" é o mesmo
//! que "impossível de comparar". Aqui dois processos no mesmo instante dão o
//! mesmo deslocamento.
//!
//! ## Por que o deslocamento é uma **translação**, e não um giro
//!
//! A primeira versão gemia `yaw`/`pitch`, e a medição mostrou que ela quase não
//! aparece: a câmera orbita o alvo, então girar em volta dele mantém o asteroide
//! **centrado** e o quadro muda só nas bordas. Medido no quadro: o centroide da
//! rocha andou **1,14 px** em 720p com 1,2° de tremor.
//!
//! Transladar a câmera (olho e alvo juntos, para a direção de visão não mudar)
//! desloca o quadro **inteiro**, e o asteroide junto. A [`AMPLITUDE`] é em
//! unidades de mundo justamente por isso, e o teste
//! `a_amplitude_virá_poucos_pixels_na_tela` amarra o número ao que o jogador vê.
//!
//! ## O que ele não faz
//!
//! Não mexe em `yaw`, `pitch` nem `distance` da câmera. A translação é somada por
//! cima na hora de montar a `Camera3D`, então ela **não acumula**: passar por dez
//! danos seguidos não deixa a câmera deslocada quando o tremor acaba.

/// Amplitude do tremor no pior caso, em **unidades de mundo**.
///
/// 0,22 unidade a 9 de distância com 60° de campo vertical dá ~14 px de
/// deslocamento num painel de 720p e ~9 px num de 480p: visível de relance, e
/// menor que a folga de mira de 10 % de um cubo de 0,8 m, então não faz errar
/// tiro que estava certo.
pub const AMPLITUDE: f32 = 0.22;
/// Duração do tremor de um dano, em segundos.
pub const DURACAO: f32 = 0.45;
/// Amplitude do tremor de um abate: um quarto do dano.
///
/// A arma já responde com o `+100` na tela; o tremor aqui é só o "recuo".
pub const AMPLITUDE_ABATE: f32 = AMPLITUDE * 0.25;

/// Estado do tremor da câmera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tremor {
    /// Tempo decorrido desde o último golpe, em segundos.
    idade: f32,
    /// Amplitude do último golpe, em radianos.
    forca: f32,
    /// Duração do último golpe, em segundos.
    duracao: f32,
}

impl Tremor {
    /// Sem tremor.
    pub fn novo() -> Self {
        Self {
            idade: DURACAO,
            forca: 0.0,
            duracao: DURACAO,
        }
    }

    /// Registra um golpe.
    ///
    /// O golpe mais forte **vence**: um abate durante um dano não pode abafar o
    /// dano. Golpe mais fraco durante um mais forte é ignorado, e não somado —
    /// somar faria dez abates seguidos virarem um terremoto.
    pub fn sacudir(&mut self, forca: f32, duracao: f32) {
        if !forca.is_finite() || forca <= 0.0 || !duracao.is_finite() || duracao <= 0.0 {
            return;
        }
        if forca >= self.forca_atual() {
            self.forca = forca;
            self.duracao = duracao;
            self.idade = 0.0;
        }
    }

    /// Dano no asteroide.
    pub fn sacudir_dano(&mut self) {
        self.sacudir(AMPLITUDE, DURACAO);
    }

    /// Cubo destruído.
    pub fn sacudir_abate(&mut self) {
        self.sacudir(AMPLITUDE_ABATE, DURACAO * 0.5);
    }

    /// Faz o tempo passar.
    pub fn atualizar(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.idade += dt;
        if self.idade >= self.duracao {
            self.forca = 0.0;
        }
    }

    /// Amplitude que ainda vale agora, em unidades de mundo.
    pub fn forca_atual(&self) -> f32 {
        if self.idade >= self.duracao || self.duracao <= 0.0 {
            return 0.0;
        }
        let progresso = (self.idade / self.duracao).clamp(0.0, 1.0);
        // Queda quadrática: o tremor some rápido, que é o que faz ele parecer
        // impacto e não terremoto.
        self.forca * (1.0 - progresso).powi(2)
    }

    /// Acabou?
    pub fn acabou(&self) -> bool {
        self.forca_atual() <= 0.0
    }

    /// Translação `(x, y)` a somar no **olho e no alvo** da câmera, em unidades
    /// de mundo.
    pub fn deslocamento(&self) -> (f32, f32) {
        let forca = self.forca_atual();
        if forca <= 0.0 {
            return (0.0, 0.0);
        }
        // Duas senoides com períodos primos: a soma nunca repete a fase dentro
        // de uma vida de tremor, e as duas direções ficam descorrelacionadas.
        let tau = std::f32::consts::TAU;
        let yaw = (self.idade * tau / 0.083).sin();
        let pitch = (self.idade * tau / 0.131).sin();
        (yaw * forca, pitch * forca * 0.7)
    }
}

impl Default for Tremor {
    fn default() -> Self {
        Self::novo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_golpe_nao_ha_deslocamento() {
        let tremor = Tremor::novo();
        assert!(tremor.acabou());
        assert_eq!(tremor.deslocamento(), (0.0, 0.0));
    }

    #[test]
    fn o_tremor_comeca_no_pico_e_some() {
        let mut tremor = Tremor::novo();
        tremor.sacudir_dano();
        assert!((tremor.forca_atual() - AMPLITUDE).abs() < 1e-6);

        tremor.atualizar(DURACAO * 0.5);
        let meio = tremor.forca_atual();
        // Queda quadrática: na metade do tempo ainda vale um quarto.
        assert!(
            (meio - AMPLITUDE * 0.25).abs() < 1e-4,
            "força na metade {meio:.5}, esperado {:.5}",
            AMPLITUDE * 0.25
        );

        tremor.atualizar(DURACAO);
        assert!(tremor.acabou());
        assert_eq!(tremor.deslocamento(), (0.0, 0.0));
    }

    #[test]
    fn o_deslocamento_e_deterministico() {
        // Ruído daria um quadro diferente a cada execução, e captura de quadro
        // deixaria de ser comparável.
        let mut a = Tremor::novo();
        let mut b = Tremor::novo();
        a.sacudir_dano();
        b.sacudir_dano();
        for _ in 0..20 {
            a.atualizar(0.016);
            b.atualizar(0.016);
            assert_eq!(a.deslocamento(), b.deslocamento());
        }
    }

    #[test]
    fn o_deslocamento_nunca_passa_da_amplitude() {
        let mut tremor = Tremor::novo();
        tremor.sacudir_dano();
        for _ in 0..40 {
            let (yaw, pitch) = tremor.deslocamento();
            assert!(yaw.abs() <= AMPLITUDE + 1e-6, "x {yaw:.5}");
            assert!(pitch.abs() <= AMPLITUDE + 1e-6, "y {pitch:.5}");
            tremor.atualizar(0.011);
        }
    }

    #[test]
    fn o_golpe_mais_forte_vence_e_o_mais_fraco_nao_soma() {
        let mut tremor = Tremor::novo();
        tremor.sacudir_abate();
        tremor.sacudir_dano();
        assert!(
            (tremor.forca_atual() - AMPLITUDE).abs() < 1e-6,
            "o dano tinha de dominar o abate"
        );

        // Um abate no meio de um dano não pode reduzir nem somar.
        tremor.sacudir_abate();
        assert!(
            (tremor.forca_atual() - AMPLITUDE).abs() < 1e-6,
            "o abate mais fraco não pode abafar o dano"
        );

        // Dez abates seguidos não podem virar terremoto.
        for _ in 0..10 {
            tremor.sacudir_abate();
        }
        assert!(tremor.forca_atual() <= AMPLITUDE + 1e-6);
    }

    #[test]
    fn o_dano_reinicia_o_tremor_que_estava_acabando() {
        let mut tremor = Tremor::novo();
        tremor.sacudir_dano();
        tremor.atualizar(DURACAO * 0.9);
        assert!(tremor.forca_atual() < AMPLITUDE * 0.05);

        tremor.sacudir_dano();
        assert!(
            (tremor.forca_atual() - AMPLITUDE).abs() < 1e-6,
            "um dano novo tinha de voltar ao pico"
        );
    }

    #[test]
    fn valores_invalidos_nao_ligam_o_tremor() {
        let mut tremor = Tremor::novo();
        for (forca, duracao) in [
            (f32::NAN, DURACAO),
            (0.0, DURACAO),
            (-1.0, DURACAO),
            (AMPLITUDE, f32::INFINITY),
            (AMPLITUDE, 0.0),
        ] {
            tremor.sacudir(forca, duracao);
        }
        assert!(tremor.acabou(), "entrada inválida ligou o tremor");
    }

    #[test]
    fn o_tempo_invalido_nao_mexe_no_tremor() {
        let mut tremor = Tremor::novo();
        tremor.sacudir_dano();
        let antes = tremor.forca_atual();
        tremor.atualizar(-1.0);
        tremor.atualizar(f32::NAN);
        assert_eq!(tremor.forca_atual(), antes);
    }

    #[test]
    fn a_amplitude_virá_poucos_pixels_na_tela() {
        // A amplitude é em unidades de mundo, e um número em unidades não diz
        // nada sobre o que o jogador vê. A conta: a altura visível a `d` com
        // campo vertical de 60° é `2·d·tan(30°)`, então `A` unidades valem
        // `A / (2·d·tan30) · altura` pixels.
        let distancia = crate::gameplay::camera::OrbitalCamera::new().distancia_segura();
        let visivel = 2.0 * distancia * (std::f32::consts::PI / 6.0).tan();
        for (altura, minimo, maximo) in [(480.0, 5.0, 15.0), (720.0, 8.0, 22.0)] {
            let pixels = AMPLITUDE / visivel * altura;
            assert!(
                (minimo..=maximo).contains(&pixels),
                "amplitude de {pixels:.1} px em {altura:.0}p, esperado entre {minimo} e {maximo}"
            );
        }
    }

    #[test]
    fn o_abate_balanca_menos_que_o_dano() {
        let mut dano = Tremor::novo();
        let mut abate = Tremor::novo();
        dano.sacudir_dano();
        abate.sacudir_abate();
        assert!(
            abate.forca_atual() < dano.forca_atual() * 0.5,
            "o abate tem de ser bem mais discreto: {:.5} contra {:.5}",
            abate.forca_atual(),
            dano.forca_atual()
        );
    }
}
