//! Ritmo do jogo: simulação em passo fixo, desenho limitado a no máximo 75 Hz.
//!
//! ## Por que dois relógios
//!
//! A cobrinha **anda em degraus**: a cada `1/TIQUE` de segundo ela dá um passo
//! de uma célula. Se a simulação rodasse no ritmo do desenho, a velocidade
//! mudaria com a carga da máquina — o mesmo jogo seria mais difícil num aparelho
//! lento, e o recorde deixaria de significar alguma coisa.
//!
//! Então:
//!
//! - **simulação** com passo fixo (`1/TIQUE`), para ser determinística;
//! - **desenho** com teto de `ALVO_FPS`, sem ultrapassar `MAX_FPS`.
//!
//! E o que liga os dois é o **alpha**: quanto falta para o próximo passo, de 0 a
//! 1. Com ele a cabeça desenha no meio do caminho entre duas células e o
//! movimento sai liso mesmo com a simulação a 8 Hz.
//!
//! O limite de desenho é 75 Hz, pedido do projeto para aproveitar o RK3566 sem
//! permitir que a renderização exceda esse orçamento. A simulação não muda.
//!
//! ## Por que não `sleep` de quadro inteiro
//!
//! dormir `1/75` fixo a cada quadro acumula erro: o trabalho do quadro soma ao
//! sono, e o ritmo real cai. Aqui a dormida é calculada contra um **prazo
//! absoluto**, então o erro não acumula. E ela nunca é maior que o prazo: se o
//! quadro atrasou, dorme zero — não tenta "recuperar" dormindo negativo.

/// Frequência padrão de desenho.
pub const ALVO_FPS: u32 = 75;
/// Limite absoluto de desenho, também aplicado ao relógio do núcleo.
pub const MAX_FPS: u32 = 75;

/// Passos de simulação por segundo no início.
pub const TIQUE_BASE: f32 = 7.0;
/// Aceleração por comida, até o teto. Sobe 0,5 passo por comida.
pub const ACELERACAO: f32 = 0.5;
/// Teto de velocidade: 16 passos/s ainda dá tempo de reagir num direcional.
pub const TIQUE_MAX: f32 = 16.0;

/// Quantos passos a simulação deve dar, e onde interpolar o desenho.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Passos {
    /// Passos a executar neste quadro. 0 é o caso comum (75 Hz de desenho para
    /// 8 Hz de simulação dá ~1 passo a cada 5 quadros).
    pub ticks: u32,
    /// Fração do próximo passo já transcorrida, em `[0, 1)`.
    pub alpha: f32,
    /// Segundos a dormir antes do próximo quadro. Zero quando atrasado.
    pub dormir: f64,
}

pub struct Relogio {
    /// Duração de um passo de simulação, em segundos.
    passo: f64,
    /// Intervalo mínimo entre desenhos, em segundos.
    intervalo: f64,
    /// Tempo não consumido pela simulação.
    acumulado: f64,
    /// Instante em que o próximo desenho deve acontecer.
    prazo: f64,
    /// Instante do último quadro.
    agora: f64,
    /// Quadros seguidos que estouraram o orçamento — diagnóstico de aperto.
    atrasados: u32,
}

impl Relogio {
    pub fn novo() -> Self {
        Self::com_ritmo(TIQUE_BASE, ALVO_FPS)
    }

    pub fn com_ritmo(tiques_por_segundo: f32, fps: u32) -> Self {
        Relogio {
            passo: 1.0 / tiques_por_segundo.max(1.0) as f64,
            intervalo: 1.0 / fps.clamp(1, MAX_FPS) as f64,
            acumulado: 0.0,
            prazo: 0.0,
            agora: 0.0,
            atrasados: 0,
        }
    }

    /// Muda a velocidade da simulação sem perder o acumulado.
    ///
    /// Chamado quando a cobra come: a velocidade sobe, e o próximo passo tem de
    /// usar o passo novo. Zerar o acumulado aqui faria a cobra "engasgar" a cada
    /// comida.
    pub fn definir_tiques(&mut self, tiques_por_segundo: f32) {
        self.passo = 1.0 / tiques_por_segundo.max(1.0) as f64;
    }

    pub fn passo(&self) -> f64 {
        self.passo
    }

    pub fn intervalo(&self) -> f64 {
        self.intervalo
    }

    /// Quadros que chegaram tarde desde o último reset. Serve de sinal de
    /// aperto — e de nada além disso: não corrige nada sozinho.
    pub fn atrasados(&self) -> u32 {
        self.atrasados
    }

    /// Avança para o instante `agora_segundos` (relógio monotônico do chamador).
    ///
    /// O chamador passa o tempo, não o `Instant`: assim o teste injeta qualquer
    /// sequência, inclusive saltos, sem esperar de verdade.
    pub fn avancar(&mut self, agora_segundos: f64) -> Passos {
        let mut dt = agora_segundos - self.agora;
        self.agora = agora_segundos;

        // Primeiro quadro, ou relógio que andou para trás: não simula nada.
        if dt < 0.0 {
            dt = 0.0;
        }

        // Teto de recuperação: meio segundo. Sem isto, um travamento de 3 s
        // (troca de contexto, apagão de tela) enfileira 24 passos e a cobra
        // teleporta — o clássico "espiral da morte" dos jogos com passo fixo.
        let teto = 0.5;
        if dt > teto {
            dt = teto;
        }

        self.acumulado += dt;

        let mut ticks = 0u32;
        while self.acumulado >= self.passo {
            self.acumulado -= self.passo;
            ticks += 1;
            // Trava de segurança: 12 passos por quadro é o máximo plausível.
            if ticks >= 12 {
                self.acumulado = 0.0;
                break;
            }
        }

        let alpha = (self.acumulado / self.passo) as f32;

        // Prazo absoluto: o erro de um quadro não se soma ao do seguinte.
        if self.prazo <= 0.0 {
            self.prazo = agora_segundos + self.intervalo;
        }
        let dormir = (self.prazo - agora_segundos).max(0.0);
        if dormir <= 0.0 {
            self.atrasados = self.atrasados.saturating_add(1);
            // Reancora no futuro imediato em vez de acumular dívida.
            self.prazo = agora_segundos + self.intervalo;
        } else {
            self.atrasados = 0;
            self.prazo += self.intervalo;
        }

        Passos {
            ticks,
            alpha: alpha.clamp(0.0, 0.999),
            dormir,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn passo_fixo_de_simulacao() {
        // 1 segundo de tempo real a 7 passos/s dá exatamente 7 passos.
        let mut r = Relogio::com_ritmo(7.0, 45);
        let mut ticks = 0;
        for i in 1..=45 {
            ticks += r.avancar(i as f64 / 45.0).ticks;
        }
        assert_eq!(ticks, 7, "1 s a 7 passos/s tem de dar 7 passos");
    }

    #[test]
    fn quarenta_e_cinco_quadros_por_segundo_cabem_no_intervalo() {
        let mut r = Relogio::com_ritmo(7.0, 45);
        // A 45 Hz o intervalo é 1/45 ≈ 22,2 ms. Pedir 45 quadros em 1 s tem de
        // caber, não estourar.
        let mut atrasados = 0;
        for i in 1..=45 {
            let p = r.avancar(i as f64 / 45.0);
            if p.dormir <= 0.0 {
                atrasados += 1;
            }
        }
        assert_eq!(
            atrasados, 0,
            "45 quadros em 1 s não podem ser todos 'atrasados'"
        );
    }

    #[test]
    fn frequencia_do_relogio_nunca_ultrapassa_75_hz() {
        let r = Relogio::com_ritmo(7.0, 120);
        assert_eq!(r.intervalo(), 1.0 / f64::from(MAX_FPS));
        assert_eq!(ALVO_FPS, MAX_FPS);
    }

    #[test]
    fn alpha_fica_entre_zero_e_um() {
        let mut r = Relogio::novo();
        for i in 0..200 {
            let p = r.avancar(i as f64 * 0.011);
            assert!((0.0..1.0).contains(&p.alpha), "alpha {}", p.alpha);
        }
    }

    #[test]
    fn travamento_longo_nao_teleporta_a_cobra() {
        // 3 segundos parados não podem virar 21 passos de uma vez: o teto de
        // recuperação existe para isso.
        let mut r = Relogio::novo();
        r.avancar(0.0);
        let p = r.avancar(3.0);
        assert!(p.ticks <= 6, "3 s viraram {} passos", p.ticks);
    }

    #[test]
    fn relogio_que_anda_para_tras_nao_simula() {
        let mut r = Relogio::novo();
        r.avancar(10.0);
        let p = r.avancar(9.0);
        assert_eq!(p.ticks, 0);
    }

    #[test]
    fn acelerar_nao_engasga() {
        let mut r = Relogio::com_ritmo(7.0, 45);
        r.avancar(0.0);
        r.definir_tiques(8.0);
        assert!((r.passo() - 0.125).abs() < 1e-9, "passo {:?}", r.passo());
    }

    #[test]
    fn a_45_hz_o_ritmo_real_nao_escorrega() {
        // Um segundo de quadros a 45 Hz tem de consumir ~1 s de simulação, não
        // 0,7 — é o erro acumulado que este teste pega.
        let mut r = Relogio::com_ritmo(TIQUE_BASE, ALVO_FPS);
        let mut total = 0u32;
        for i in 0..=45 {
            total += r.avancar(i as f64 / 45.0).ticks;
        }
        assert!(
            (6..=8).contains(&total),
            "1 s deu {total} passos, esperado ~7"
        );
    }
}
