//! Física arcade do carro do jogador.
//!
//! ## O modelo, e por que ele não tenta ser simulação
//!
//! Um carro de verdade tem pneu com curva de aderência, transferência de peso e
//! diferencial. Nada disso cabe (nem diverte) num jogo de 30 segundos por volta:
//! o que o jogador sente é **resposta imediata** — acelera forte, vira agora,
//! derrapa quando segura o freio de mão e volta a agarrar quando solta.
//!
//! Então o estado é pequeno de propósito: posição, rumo, velocidade no rumo e
//! uma **derrapagem lateral** separada. A derrapagem é o que dá o drift: ela é
//! uma velocidade perpendicular que decai com aderência, em vez de ser uma força
//! calculada. Um número a menos no estado é um número a menos para dessincronizar
//! do desenho.

use crate::pista::{Localizacao, Pista, Ponto};

/// Velocidade máxima sem nitro, em m/s (≈ 223 km/h).
pub const VEL_MAX: f32 = 62.0;
/// Velocidade máxima com nitro, em m/s (≈ 295 km/h).
pub const VEL_NITRO_MAX: f32 = 82.0;
/// Aceleração do motor a baixa velocidade, em m/s².
pub const ACEL: f32 = 19.0;
/// Desaceleração do freio, em m/s².
pub const FREIO: f32 = 42.0;
/// Rotação máxima do volante, em rad/s.
pub const GIRO_MAX: f32 = 1.75;
/// Aceleração extra do nitro, em m/s².
pub const NITRO_ACEL: f32 = 26.0;
/// Nitro consumido por segundo com o botão apertado.
pub const NITRO_GASTO: f32 = 30.0;
/// Nitro recarregado por segundo sem usar.
pub const NITRO_RECARGA: f32 = 8.0;
/// Nitro recarregado por segundo **enquanto derrapa** — drifting é a fonte.
pub const NITRO_RECARGA_DRIFT: f32 = 22.0;
/// Abaixo desta velocidade o carro não é considerado derrapando.
pub const VEL_MIN_DRIFT: f32 = 11.0;
/// Derrapagem lateral mínima (m/s) para contar como drift.
pub const DERRAPAGEM_MIN: f32 = 3.2;

/// O que o jogador está pedindo neste instante.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Entrada {
    /// 0..1 — acelerar.
    pub acelerar: f32,
    /// 0..1 — frear.
    pub frear: f32,
    /// -1..1 — virar (positivo para a direita).
    pub virar: f32,
    /// Freio de mão: solta a traseira.
    pub freio_mao: bool,
    /// Nitro.
    pub nitro: bool,
}

impl Entrada {
    /// Nada apertado.
    pub const PARADA: Self = Self {
        acelerar: 0.0,
        frear: 0.0,
        virar: 0.0,
        freio_mao: false,
        nitro: false,
    };

    /// Só acelera — usado no modo demonstração e nos testes.
    pub fn acelerando() -> Self {
        Self {
            acelerar: 1.0,
            ..Self::PARADA
        }
    }
}

/// Avisos que o carro emite no passo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Evento {
    /// Nada aconteceu.
    Nada,
    /// Bateu na borda ou num obstáculo.
    Bateu,
    /// Começou a derrapar.
    IniciouDrift,
    /// Terminou a derrapagem (o drift foi cobrado).
    TerminouDrift,
}

/// O carro do jogador.
#[derive(Debug, Clone)]
pub struct Carro {
    /// Posição no mundo.
    pub pos: Ponto,
    /// Rumo em radianos: 0 aponta para +Z, crescendo no sentido horário visto
    /// de cima.
    pub angulo: f32,
    /// Velocidade no rumo, em m/s. Pode ser negativa em marcha ré.
    pub velocidade: f32,
    /// Velocidade lateral, em m/s (positiva para a direita).
    pub derrapagem: f32,
    /// Nitro disponível, 0..100.
    pub nitro: f32,
    /// `true` no último passo em que o nitro foi aplicado.
    ///
    /// Existe para o **desenho** (FOV, partículas) e o som lerem o mesmo fato que
    /// a física usou. Perguntar de novo "o botão está apertado e há nitro?" no
    /// render duplicaria a regra, e as duas cópias envelheceriam diferente.
    pub nitro_usando: bool,
    /// `true` no quadro em que está derrapando.
    pub derrapando: bool,
    /// Quantos segundos o drift atual dura.
    pub tempo_drift: f32,
    /// Quanto o carro está fora do asfalto, em metros.
    pub fora: f32,
    /// Índice da amostra da pista mais próxima (dica para a busca do próximo
    /// quadro — sem ela a localização seria uma varredura da volta inteira).
    pub indice_pista: Option<usize>,
    /// Posição na volta, em metros.
    pub s: f32,
}

impl Carro {
    /// Um carro parado em `pos`, apontando para `angulo`.
    pub fn novo(pos: Ponto, angulo: f32) -> Self {
        Self {
            pos,
            angulo,
            velocidade: 0.0,
            derrapagem: 0.0,
            nitro: 100.0,
            nitro_usando: false,
            derrapando: false,
            tempo_drift: 0.0,
            fora: 0.0,
            indice_pista: None,
            s: 0.0,
        }
    }

    /// Coloca o carro na pista, em `s`, deslocado `lateral` metros.
    pub fn posicionar(&mut self, pista: &Pista, s: f32, lateral: f32) {
        let s = pista.normalizar_s(s);
        self.pos = pista.ponto_lateral(s, lateral);
        let direcao = pista.direcao_em(s);
        self.angulo = direcao.x.atan2(direcao.z);
        self.velocidade = 0.0;
        self.derrapagem = 0.0;
        self.derrapando = false;
        self.tempo_drift = 0.0;
        self.s = s;
        self.indice_pista = None;
    }

    /// Direção para onde o carro aponta (unitária).
    pub fn direcao(&self) -> Ponto {
        Ponto::new(self.angulo.sin(), self.angulo.cos())
    }

    /// Direita do carro (unitária).
    pub fn direita(&self) -> Ponto {
        self.direcao().perpendicular()
    }

    /// Velocidade em km/h, para o velocímetro.
    pub fn kmh(&self) -> f32 {
        self.velocidade.abs() * 3.6
    }

    /// Marcha de 1 a 6, derivada da velocidade.
    ///
    /// Derivada e não simulada: uma caixa de verdade teria embreagem, e o
    /// jogador não vê diferença — o que ele vê é o número mudar junto com o
    /// motor subindo de tom.
    pub fn marcha(&self) -> u8 {
        let fracao = (self.velocidade / VEL_NITRO_MAX).clamp(0.0, 1.0);
        (1.0 + fracao * 5.0).floor().clamp(1.0, 6.0) as u8
    }

    /// Rotação normalizada 0..1 dentro da marcha, para o som do motor.
    pub fn rpm(&self) -> f32 {
        let fracao = (self.velocidade / VEL_NITRO_MAX).clamp(0.0, 1.0) * 5.0;
        fracao - fracao.floor()
    }

    /// Nitro disponível, 0..1.
    pub fn nitro_fracao(&self) -> f32 {
        (self.nitro / 100.0).clamp(0.0, 1.0)
    }

    /// `true` quando o nitro está sendo aplicado neste quadro.
    pub fn nitro_ativo(&self, entrada: &Entrada) -> bool {
        entrada.nitro && self.nitro > 0.5
    }

    /// Avança a física um passo de `dt` segundos.
    ///
    /// `dt` é limitado internamente: um quadro perdido (o aparelho trocou de
    /// app, o cartão SD travou) entregaria um `dt` enorme e o carro atravessaria
    /// a pista. A simulação roda em passo fixo, então o limite é rede de
    /// segurança e não parte do modelo.
    pub fn passo(&mut self, entrada: &Entrada, pista: &Pista, dt: f32) -> Evento {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return Evento::Nada;
        }

        let nitro_ativo = self.nitro_ativo(entrada);
        self.nitro_usando = nitro_ativo;
        let vel = self.velocidade;

        // ── motor, freio e arrasto ────────────────────────────────
        let vel_topo = if nitro_ativo { VEL_NITRO_MAX } else { VEL_MAX };
        // A força do motor cai a zero **no** topo: é isso que faz a velocidade
        // máxima ser um limite de verdade, e não um número que o carro
        // ultrapassa em silêncio.
        //
        // Sem arrasto aerodinâmico somado enquanto se acelera, de propósito: a
        // primeira versão tinha `0.14v + 0.0085v²` e o carro estabilizava em
        // **32,7 m/s** em vez dos 62 que o velocímetro promete — o arrasto
        // comia o motor antes do topo, e o sintoma era "o carro não passa de
        // 118 km/h" sem nada apontar para a fórmula.
        let sobra = (1.0 - (vel / vel_topo).clamp(0.0, 1.0).powi(2)).max(0.0);
        let motor = entrada.acelerar.clamp(0.0, 1.0) * ACEL * sobra;
        let extra_nitro = if nitro_ativo { NITRO_ACEL } else { 0.0 };

        // Solto, o carro **desacelera**: freio-motor + arrasto. Sem isso ele
        // andaria de graça até o fim da corrida.
        let arrasto = if entrada.acelerar > 0.05 {
            0.55
        } else {
            4.5 + 0.07 * vel.abs() + 0.004 * vel * vel
        };

        // Fora do asfalto o atrito sobe muito: é o que faz a grama punir sem
        // precisar de parede.
        // O ombro punir tem de ser desconfortável, não terminal: `9 + fora·1,6`
        // somado ao freio de parede deixava o carro a 1,3 m/s e sem saída.
        let atrito_extra = if self.fora > 0.0 {
            4.5 + self.fora.min(3.0) * 1.2
        } else {
            0.0
        };

        let mut forca = motor + extra_nitro - arrasto - atrito_extra;

        // Freio e ré são a mesma tecla com significados diferentes, e **não se
        // somam**: somados, o carro parava e entrava em ré no mesmo instante a
        // 56 m/s². Medido: -8 m/s depois de 1 s freando de 40.
        if entrada.frear > 0.0 {
            if vel > 0.25 {
                forca -= entrada.frear.clamp(0.0, 1.0) * FREIO;
            } else if entrada.acelerar <= 0.0 {
                forca -= entrada.frear.clamp(0.0, 1.0) * FREIO * 0.30;
            }
        }

        self.velocidade = (vel + forca * dt).clamp(-8.0, VEL_NITRO_MAX);

        // ── direção ───────────────────────────────────────────────
        // Aderência: 1 no asfalto com o carro assentado, ~0,3 com o freio de mão.
        let aderencia = if entrada.freio_mao { 0.30 } else { 1.0 };
        // A direção responde menos em alta velocidade — senão a 200 km/h um
        // toque de seta joga o carro para fora da pista.
        let suaviza = 1.0 - (self.velocidade.abs() / VEL_NITRO_MAX).clamp(0.0, 1.0) * 0.45;
        // Com o freio de mão o carro **gira mais**: é o que faz o drift parecer
        // drift em vez de só um carro escorregando de lado.
        let giro_extra = if entrada.freio_mao { 1.30 } else { 1.0 };
        let giro = entrada.virar.clamp(-1.0, 1.0) * GIRO_MAX * suaviza * giro_extra;
        self.angulo += giro * dt;

        // ── derrapagem lateral ────────────────────────────────────
        // Alvo proporcional à velocidade e ao volante, e inversamente à
        // aderência. O freio de mão tira aderência, então o alvo cresce.
        // `1 - aderencia` é zero sem o freio de mão: sem ele **não há drift**,
        // e isso é escolha, não falta de modelo. Um carro que derrapa sozinho em
        // toda curva tira do jogador a decisão de onde derrapar, que é a graça.
        let alvo = entrada.virar.clamp(-1.0, 1.0)
            * self.velocidade.abs()
            * (1.0 - aderencia)
            * 0.55;
        // Decaimento exponencial: `1 - exp(-k dt)` é estável em qualquer `dt`,
        // ao contrário de `alvo * k * dt`, que oscila com `dt` grande.
        let k = 3.2 + aderencia * 3.0;
        self.derrapagem += (alvo - self.derrapagem) * (1.0 - (-k * dt).exp());
        // Sem o freio de mão a derrapagem some rápido; com ele, fica.
        if !entrada.freio_mao {
            self.derrapagem *= (-2.2 * dt).exp();
        }

        // ── posição ───────────────────────────────────────────────
        let direcao = self.direcao();
        let direita = self.direita();
        self.pos = self
            .pos
            .mais(direcao.vezes(self.velocidade * dt))
            .mais(direita.vezes(self.derrapagem * dt));

        // ── pista: limites, obstáculos, drift ─────────────────────
        let mut evento = Evento::Nada;
        let local = pista.localizar(self.pos, self.indice_pista);
        self.indice_pista = Some(local.indice);
        self.s = local.s;
        self.fora = local.fora_por();

        if let Some(evento_novo) = self.resolver_limites(pista, &local, dt) {
            evento = evento_novo;
        }
        if let Some(evento_novo) = self.resolver_obstaculos(pista) {
            evento = evento_novo;
        }

        // ── nitro ─────────────────────────────────────────────────
        if nitro_ativo {
            self.nitro = (self.nitro - NITRO_GASTO * dt).max(0.0);
        } else if self.derrapando {
            // Drift é a fonte de nitro: fecha o ciclo "derrapa para acelerar".
            self.nitro = (self.nitro + NITRO_RECARGA_DRIFT * dt).min(100.0);
        } else {
            self.nitro = (self.nitro + NITRO_RECARGA * dt).min(100.0);
        }

        // ── drift: histerese para não piscar no limite ────────────
        let deslize = self.derrapagem.abs();
        let rapido = self.velocidade > VEL_MIN_DRIFT;
        if !self.derrapando && rapido && deslize > DERRAPAGEM_MIN {
            self.derrapando = true;
            self.tempo_drift = 0.0;
            if evento == Evento::Nada {
                evento = Evento::IniciouDrift;
            }
        } else if self.derrapando {
            // Termina com folga: entrar em 3,2 e sair em 3,2 faria o drift
            // piscar a cada correção de volante.
            if !rapido || deslize < DERRAPAGEM_MIN * 0.55 {
                self.derrapando = false;
                self.tempo_drift = 0.0;
                if evento == Evento::Nada {
                    evento = Evento::TerminouDrift;
                }
            } else {
                self.tempo_drift += dt;
            }
        }

        evento
    }

    /// Empurra o carro de volta para o asfalto quando ele passa da borda.
    fn resolver_limites(
        &mut self,
        pista: &Pista,
        local: &Localizacao,
        dt: f32,
    ) -> Option<Evento> {
        // Margem de 2,2 m: há mureta e não há chão além dela.
        let limite = local.meia_largura + 2.2;
        if local.distancia <= limite {
            return None;
        }

        let s = local.s;
        let lado = if local.lateral >= 0.0 { 1.0 } else { -1.0 };
        self.pos = pista.ponto_lateral(s, lado * (limite - 0.05));
        let normal = pista.normal_em(s);

        // Bater de frente dói; raspar não. A perda era `0,30 + 0,55·|dir·n|`,
        // ou seja **30% por segundo mesmo paralelo à parede** — junto com o
        // atrito do ombro, isso prendia o carro a 1,3 m/s encostado. Medido: o
        // piloto ficou 40 s parado na linha de largada depois de sair largo da
        // curva fechada, e o mesmo aconteceria com o jogador.
        let de_frente = self.direcao().ponto(normal).abs();
        let perda = (0.05 + 0.90 * de_frente).clamp(0.0, 0.98);
        self.velocidade *= 1.0 - perda * dt.min(1.0) * 12.0;
        self.derrapagem *= -0.25;

        // A parede **endireita** o carro: sem isto, quem entra torto fica preso
        // empurrando a mureta com o nariz. Vira o rumo para o sentido da pista,
        // no lado para onde o carro já aponta.
        if de_frente > 0.05 {
            let tangente = pista.direcao_em(s);
            let para_frente = tangente.ponto(self.direcao()) >= 0.0;
            let alvo = if para_frente {
                tangente.x.atan2(tangente.z)
            } else {
                (-tangente.x).atan2(-tangente.z)
            };
            let mut erro = alvo - self.angulo;
            while erro > std::f32::consts::PI {
                erro -= std::f32::consts::TAU;
            }
            while erro < -std::f32::consts::PI {
                erro += std::f32::consts::TAU;
            }
            self.angulo += erro.clamp(-1.0, 1.0) * 3.0 * dt.min(0.1);
        }
        Some(Evento::Bateu)
    }

    /// Colisão com cones e barreiras: empurra e perde velocidade.
    fn resolver_obstaculos(&mut self, pista: &Pista) -> Option<Evento> {
        for obstaculo in pista.obstaculos() {
            // Só os obstáculos perto do `s` do carro podem colidir; a lista é
            // curta, mas o teste de distância é mais barato que a conta toda.
            let ds = (obstaculo.s - self.s).abs();
            if ds > 12.0 && ds < pista.comprimento() - 12.0 {
                continue;
            }
            let centro = pista.ponto_lateral(obstaculo.s, obstaculo.lateral);
            let raio = obstaculo.raio + 1.1; // meio carro
            let vetor = self.pos.menos(centro);
            if vetor.comprimento() > raio {
                continue;
            }
            let empurrao = vetor.normalizado().vezes(raio + 0.05);
            self.pos = centro.mais(empurrao);
            self.velocidade *= 0.62;
            self.derrapagem *= 0.5;
            return Some(Evento::Bateu);
        }
        None
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Carro numa reta longa: mede física, não colisão com a borda.
    fn carro_na_reta() -> (Carro, Pista) {
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        // No meio da reta, e não no começo: as pontas da Catmull-Rom perto das
        // curvas de retorno ainda curvam, e o carro saía da pista em linha reta
        // antes de chegar ao topo — o teste então media colisão.
        c.posicionar(&pista, 300.0, 0.0);
        (c, pista)
    }

    /// Carro no circuito real, para os testes que precisam de borda e grama.
    fn carro_na_pista(pista: &Pista) -> Carro {
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(pista, 10.0, 0.0);
        c
    }

    fn simular(carro: &mut Carro, pista: &Pista, entrada: Entrada, segundos: f32) {
        let dt = 1.0 / 75.0;
        let passos = (segundos / dt).round() as u32;
        for _ in 0..passos {
            carro.passo(&entrada, pista, dt);
        }
    }

    #[test]
    fn acelera_ate_perto_do_topo() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 14.0);
        assert!(
            c.velocidade > VEL_MAX * 0.85,
            "velocidade {:.1} m/s depois de 14 s acelerando",
            c.velocidade
        );
        assert!(c.velocidade <= VEL_MAX + 0.5, "passou do máximo sem nitro: {:.1}", c.velocidade);
    }

    #[test]
    fn nao_passa_do_maximo_nem_com_estrada_infinita() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 40.0);
        assert!(c.velocidade <= VEL_MAX + 0.5, "{:.2}", c.velocidade);
    }

    #[test]
    fn sem_acelerar_o_carro_desacelera() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 5.0);
        let antes = c.velocidade;
        simular(&mut c, &pista, Entrada::PARADA, 3.0);
        assert!(c.velocidade < antes * 0.75, "{antes:.1} → {:.1}", c.velocidade);
    }

    #[test]
    fn o_freio_para_o_carro() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 6.0);
        assert!(c.velocidade > 30.0, "não acelerou: {:.1}", c.velocidade);
        let freando = Entrada {
            frear: 1.0,
            ..Entrada::PARADA
        };
        // Tempo de parada a 42 m/s² a partir de ~40 m/s: ~1 s.
        simular(&mut c, &pista, freando, 1.0);
        assert!(c.velocidade.abs() < 6.0, "sobrou {:.1} m/s depois de 1 s freando", c.velocidade);
    }

    #[test]
    fn a_re_e_lenta_e_tem_teto() {
        let (mut c, pista) = carro_na_reta();
        let re = Entrada {
            frear: 1.0,
            ..Entrada::PARADA
        };
        simular(&mut c, &pista, re, 6.0);
        assert!(c.velocidade < 0.0, "a ré não engatou: {:.1}", c.velocidade);
        assert!(c.velocidade >= -8.0, "a ré passou do teto: {:.1}", c.velocidade);
    }

    #[test]
    fn o_nitro_leva_alem_do_maximo_normal() {
        let (mut c, pista) = carro_na_reta();
        let entrada = Entrada {
            acelerar: 1.0,
            nitro: true,
            ..Entrada::PARADA
        };
        // 7 s: tempo de sobra para passar dos 62 m/s e curto o bastante para não
        // chegar à curva de retorno da pista de teste (o teste passaria a medir
        // colisão em vez de nitro).
        simular(&mut c, &pista, entrada, 7.0);
        assert!(
            c.velocidade > VEL_MAX + 3.0,
            "com nitro ficou em {:.1} m/s, abaixo do topo normal {VEL_MAX}",
            c.velocidade
        );
        assert!(c.velocidade <= VEL_NITRO_MAX + 0.5);
    }

    #[test]
    fn o_nitro_acaba_e_volta() {
        let (mut c, pista) = carro_na_reta();
        assert!((c.nitro - 100.0).abs() < 0.01);
        let usando = Entrada {
            acelerar: 1.0,
            nitro: true,
            ..Entrada::PARADA
        };
        simular(&mut c, &pista, usando, 6.0);
        assert!(c.nitro < 5.0, "nitro ainda em {:.1} depois de 6 s", c.nitro);
        simular(&mut c, &pista, Entrada::PARADA, 8.0);
        assert!(c.nitro > 50.0, "nitro só recarregou para {:.1}", c.nitro);
    }

    #[test]
    fn a_direcao_muda_o_rumo() {
        let (mut c, pista) = carro_na_reta();
        let antes = c.angulo;
        let entrada = Entrada {
            acelerar: 1.0,
            virar: 1.0,
            ..Entrada::PARADA
        };
        simular(&mut c, &pista, entrada, 1.0);
        assert!(c.angulo > antes + 0.2, "rumo quase não mudou: {antes:.2} → {:.2}", c.angulo);
    }

    #[test]
    fn o_freio_de_mao_produz_derrapagem() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 4.0);
        let entrada = Entrada {
            acelerar: 0.4,
            virar: 1.0,
            freio_mao: true,
            ..Entrada::PARADA
        };
        let mut derrapou = false;
        let mut pico = 0.0f32;
        for _ in 0..90 {
            if c.passo(&entrada, &pista, 1.0 / 75.0) == Evento::IniciouDrift {
                derrapou = true;
            }
            pico = pico.max(c.derrapagem.abs());
        }
        assert!(derrapou, "o freio de mão não iniciou drift");
        assert!(
            pico > DERRAPAGEM_MIN,
            "a derrapagem chegou só a {pico:.2} m/s: o carro está escorregando de menos"
        );
    }

    #[test]
    fn sem_freio_de_mao_o_carro_volta_a_agarrar() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 4.0);
        let derrapando = Entrada {
            acelerar: 0.4,
            virar: 1.0,
            freio_mao: true,
            ..Entrada::PARADA
        };
        for _ in 0..60 {
            c.passo(&derrapando, &pista, 1.0 / 75.0);
        }
        let solto = Entrada {
            acelerar: 0.4,
            ..Entrada::PARADA
        };
        for _ in 0..90 {
            c.passo(&solto, &pista, 1.0 / 75.0);
        }
        assert!(c.derrapagem.abs() < 1.0, "continuou derrapando: {:.2}", c.derrapagem);
        assert!(!c.derrapando, "o estado de drift não terminou");
    }

    #[test]
    fn fora_do_asfalto_o_carro_perde_velocidade() {
        let pista = Pista::nova();
        // Dois carros iguais, um no asfalto e outro no ombro: comparar o mesmo
        // carro consigo mesmo não serve, porque ele ainda está acelerando.
        let mut na_pista = carro_na_pista(&pista);
        let mut no_ombro = carro_na_pista(&pista);
        simular(&mut na_pista, &pista, Entrada::acelerando(), 6.0);
        simular(&mut no_ombro, &pista, Entrada::acelerando(), 6.0);

        let s = 60.0;
        let meia = pista.meia_largura_em(s);
        no_ombro.pos = pista.ponto_lateral(s, meia + 1.0);
        no_ombro.indice_pista = None;
        na_pista.pos = pista.ponto_lateral(s, 0.0);
        na_pista.indice_pista = None;

        simular(&mut na_pista, &pista, Entrada::acelerando(), 0.5);
        simular(&mut no_ombro, &pista, Entrada::acelerando(), 0.5);
        assert!(no_ombro.fora > 0.1, "o carro devia estar no ombro; fora={:.2}", no_ombro.fora);
        assert!(
            no_ombro.velocidade < na_pista.velocidade - 1.0,
            "ombro {:.1} m/s contra asfalto {:.1} m/s",
            no_ombro.velocidade,
            na_pista.velocidade
        );
    }

    #[test]
    fn a_borda_devolve_o_carro_para_dentro() {
        let pista = Pista::nova();
        let mut c = carro_na_pista(&pista);
        simular(&mut c, &pista, Entrada::acelerando(), 4.0);
        let s = 60.0;
        let meia = pista.meia_largura_em(s);
        c.pos = pista.ponto_lateral(s, meia + 12.0);
        c.indice_pista = None;
        c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0);
        let depois = pista.localizar(c.pos, None);
        assert!(
            depois.distancia <= depois.meia_largura + 2.5,
            "ficou a {:.1} m do centro com meia-largura {:.1}",
            depois.distancia,
            depois.meia_largura
        );
    }

    #[test]
    fn a_marcha_cresce_com_a_velocidade() {
        let (mut c, pista) = carro_na_reta();
        assert_eq!(c.marcha(), 1);
        simular(&mut c, &pista, Entrada::acelerando(), 3.0);
        let terceira = c.marcha();
        simular(&mut c, &pista, Entrada::acelerando(), 10.0);
        assert!(c.marcha() > terceira, "marcha travou em {terceira}");
        assert!(c.marcha() <= 6);
        assert!((0.0..1.0).contains(&c.rpm()));
    }

    #[test]
    fn a_velocidade_em_kmh_bate_com_a_conversao() {
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.velocidade = 30.0;
        assert!((c.kmh() - 108.0).abs() < 0.1);
    }

    #[test]
    fn dt_gigante_nao_atravessa_a_pista() {
        // Um quadro perdido não pode teletransportar o carro para fora do mundo.
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 5.0);
        let antes = c.pos;
        c.passo(&Entrada::acelerando(), &pista, 5.0);
        assert!(
            c.pos.distancia(antes) < 5.0,
            "andou {:.1} m com dt=5 s",
            c.pos.distancia(antes)
        );
    }

    #[test]
    fn drifting_recarrega_nitro() {
        let (mut c, pista) = carro_na_reta();
        simular(&mut c, &pista, Entrada::acelerando(), 4.0);
        // Gasta o nitro até quase zero.
        let usando = Entrada {
            acelerar: 1.0,
            nitro: true,
            ..Entrada::PARADA
        };
        simular(&mut c, &pista, usando, 3.5);
        let baixo = c.nitro;
        let derrapando = Entrada {
            acelerar: 0.3,
            virar: 1.0,
            freio_mao: true,
            ..Entrada::PARADA
        };
        simular(&mut c, &pista, derrapando, 1.5);
        assert!(
            c.nitro > baixo + 5.0,
            "drift não recarregou: {baixo:.1} → {:.1}",
            c.nitro
        );
    }
}
