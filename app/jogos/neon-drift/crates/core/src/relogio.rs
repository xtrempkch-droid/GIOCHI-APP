//! Relógio de simulação: passo fixo e teto de quadros.
//!
//! ## Por que passo fixo
//!
//! A física integra aceleração e a direção muda por `dt`. Com `dt` variável, a
//! mesma curva fecha diferente em 60 e em 75 Hz — e o carro fica mais ou menos
//! "escorregadio" conforme a carga do aparelho. Passo fixo torna a jogabilidade
//! **idêntica** em qualquer taxa.
//!
//! ## Por que o teto de 75
//!
//! É o teto do painel desta família de aparelhos, e o pedido é explícito. O
//! relógio nunca agenda mais de 75 quadros por segundo; o laço ainda dorme o
//! que sobra, então o jogo não consome CPU tentando passar do limite.

use std::time::{Duration, Instant};

/// Teto de quadros por segundo.
pub const MAX_FPS: u32 = 75;
/// Frequência de desenho padrão (igual ao teto).
pub const ALVO_FPS: u32 = MAX_FPS;

/// Frequência nominal da simulação, em passos por segundo.
///
/// Igual à de desenho: com passo fixo em 120 Hz e tela de 75 o jogador veria dois
/// passos em alguns quadros e um em outros, e a interpolação teria de esconder
/// isso. A 75 Hz cada quadro é um passo, e o que se vê é o que a física fez.
pub const TIQUE: f32 = 75.0;

/// Resultado de avançar o relógio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Passos {
    /// Quantos passos de simulação rodar.
    pub ticks: u32,
    /// Fração do passo corrente, 0..1, para interpolar o desenho.
    pub alpha: f32,
    /// Quanto dormir antes do próximo quadro.
    pub dormir: Duration,
}

/// Relógio de passo fixo.
#[derive(Debug, Clone)]
pub struct Relogio {
    passo: f64,
    intervalo: f64,
    acumulado: f64,
    prazo: f64,
    agora: f64,
}

impl Relogio {
    /// Relógio padrão: 75 passos por segundo, teto de 75.
    pub fn novo() -> Self {
        Self::com_ritmo(TIQUE, ALVO_FPS)
    }

    /// Relógio com ritmo escolhido. O teto é aplicado de qualquer forma.
    pub fn com_ritmo(tiques_por_segundo: f32, fps: u32) -> Self {
        Self {
            passo: 1.0 / tiques_por_segundo.max(1.0) as f64,
            intervalo: 1.0 / fps.clamp(1, MAX_FPS) as f64,
            acumulado: 0.0,
            prazo: 0.0,
            agora: 0.0,
        }
    }

    /// Duração de um passo de simulação, em segundos.
    pub fn passo(&self) -> f64 {
        self.passo
    }

    /// Intervalo mínimo entre quadros, em segundos.
    pub fn intervalo(&self) -> f64 {
        self.intervalo
    }

    /// Avança para `agora_segundos` (relógio monotônico do chamador).
    ///
    /// O chamador passa o tempo em vez de um `Instant` para o teste injetar
    /// qualquer sequência, inclusive saltos, sem esperar de verdade.
    pub fn avancar(&mut self, agora_segundos: f64) -> Passos {
        let dt = agora_segundos - self.agora;
        self.agora = agora_segundos;

        if dt < 0.0 {
            // Relógio que andou para trás (ajuste de hora, suspensão): não
            // simula nada e reancora.
            self.acumulado = 0.0;
            self.prazo = agora_segundos + self.intervalo;
            return Passos {
                ticks: 0,
                alpha: 0.0,
                dormir: Duration::from_secs_f64(self.intervalo),
            };
        }

        self.acumulado += dt;

        // Teto de passos por avanço: um quadro perdido de 2 s não pode virar 150
        // passos e travar o aparelho justamente quando ele já está lento.
        const MAX_TICKS: u32 = 6;
        let mut ticks = 0;
        // Epsilon: 75 somas de 1/75 em `f64` não dão exatamente 1, e sem a
        // folga o último passo do segundo era engolido (74 quadros em vez de 75).
        while self.acumulado >= self.passo - 1e-9 && ticks < MAX_TICKS {
            self.acumulado -= self.passo;
            ticks += 1;
        }
        if ticks == MAX_TICKS {
            self.acumulado = 0.0;
        }

        let alpha = (self.acumulado / self.passo) as f32;

        if self.prazo <= 0.0 {
            self.prazo = agora_segundos + self.intervalo;
        }
        let mut dormir = self.prazo - agora_segundos;
        if dormir <= 0.0 {
            // Chegou tarde: reancora no futuro imediato em vez de acumular
            // dívida, que faria o laço tentar "recuperar" quadros perdidos.
            self.prazo = agora_segundos + self.intervalo;
            dormir = self.intervalo;
        } else {
            // Avança um intervalo, mas **nunca** mais que dois à frente do
            // agora: um chamador que chega mais rápido que o teto (o CI, um
            // `--quadros` acelerado) acumulava espera indefinidamente, e o
            // `dormir` chegava a três intervalos.
            self.prazo = (self.prazo + self.intervalo).min(agora_segundos + 2.0 * self.intervalo);
        }

        Passos {
            ticks,
            alpha: alpha.clamp(0.0, 0.999),
            dormir: Duration::from_secs_f64(dormir.max(0.0)),
        }
    }

    /// Instante atual, em segundos, como o relógio o conhece.
    pub fn agora(&self) -> f64 {
        self.agora
    }
}

impl Default for Relogio {
    fn default() -> Self {
        Self::novo()
    }
}

/// Marca o instante de início do laço.
pub fn iniciar() -> Instant {
    Instant::now()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn um_segundo_a_75_hz_da_75_passos() {
        let mut r = Relogio::novo();
        let mut ticks = 0;
        for i in 1..=75 {
            ticks += r.avancar(i as f64 / 75.0).ticks;
        }
        assert_eq!(ticks, 75, "1 s a 75 Hz tem de dar 75 passos");
    }

    #[test]
    fn o_teto_de_fps_nunca_passa_de_75() {
        let r = Relogio::com_ritmo(TIQUE, 120);
        assert_eq!(r.intervalo(), 1.0 / f64::from(MAX_FPS));
        assert_eq!(ALVO_FPS, MAX_FPS);
        assert_eq!(MAX_FPS, 75);
    }

    #[test]
    fn passo_fixo_independe_do_ritmo_de_desenho() {
        let mut a = Relogio::com_ritmo(75.0, 75);
        let mut b = Relogio::com_ritmo(75.0, 30);
        let mut ticks_a = 0;
        let mut ticks_b = 0;
        for i in 1..=75 {
            ticks_a += a.avancar(i as f64 / 75.0).ticks;
            ticks_b += b.avancar(i as f64 / 75.0).ticks;
        }
        assert_eq!(ticks_a, ticks_b, "a simulação não pode depender dos quadros");
    }

    #[test]
    fn salto_grande_nao_vira_150_passos() {
        let mut r = Relogio::novo();
        r.avancar(0.0);
        let p = r.avancar(3.0);
        assert!(p.ticks <= 6, "um quadro de 3 s pediu {} passos", p.ticks);
    }

    #[test]
    fn relogio_para_tras_nao_simula() {
        let mut r = Relogio::novo();
        r.avancar(10.0);
        let p = r.avancar(9.0);
        assert_eq!(p.ticks, 0);
        assert!(p.dormir > Duration::ZERO);
    }

    #[test]
    fn alpha_fica_entre_zero_e_um() {
        let mut r = Relogio::novo();
        for i in 0..200 {
            let p = r.avancar(i as f64 / 137.0);
            assert!((0.0..1.0).contains(&p.alpha), "alpha {}", p.alpha);
        }
    }

    #[test]
    fn o_teto_segura_quando_os_quadros_chegam_rapido() {
        // A 200 Hz de chamadas, o relógio de 75 Hz tem de mandar dormir mais que
        // um intervalo em alguns quadros — é justamente o teto agindo. O que não
        // pode é a espera acumular sem limite.
        let mut r = Relogio::novo();
        let mut maior: f64 = 0.0;
        for i in 0..200 {
            let p = r.avancar(i as f64 / 200.0);
            maior = maior.max(p.dormir.as_secs_f64());
        }
        assert!(
            maior <= 3.0 * r.intervalo(),
            "espera de {maior:.4} s com intervalo {:.4} s",
            r.intervalo()
        );
        assert!(maior > r.intervalo(), "o teto de 75 Hz não segurou nada");
    }

    #[test]
    fn em_um_segundo_com_quadros_rapidos_o_ritmo_fica_no_teto() {
        // 200 chamadas em 1 s com teto de 75: a soma das esperas tem de dar
        // ~1 s, e o número de passos de simulação ~75. Se o teto não segurasse,
        // a simulação correria a 200 passos por segundo.
        let mut r = Relogio::novo();
        let mut ticks = 0;
        let mut espera = 0.0f64;
        for i in 1..=200 {
            let p = r.avancar(i as f64 / 200.0);
            ticks += p.ticks;
            espera += p.dormir.as_secs_f64();
        }
        assert!((74..=76).contains(&ticks), "ticks={ticks}");
        assert!(espera > 0.5, "espera total {espera:.2} s");
    }
}
