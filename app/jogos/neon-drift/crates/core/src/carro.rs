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

/// Aceleração da gravidade usada na inclinação, em m/s².
///
/// **Não é 9,81** — é 9,81 × 3,5, e o fator é de arcade com motivo medido: numa
/// rampa de 11 % a gravidade real dá **1,08 m/s²**, que é menos que o atrito de
/// rolagem do próprio modelo. O carro descia a serra exatamente como desce a
/// avenida, e o mapa novo era um circuito plano com o asfalto torto.
///
/// Com o fator, a descida da Boavista empurra ~3,7 m/s² e a subida segura os
/// mesmos 3,7 — números que se sentem no controle e que ainda deixam o carro
/// dirigível (a 11 % de rampa, um carro de verdade sai de 0 a 100 km/h em ~8 s
/// só pela gravidade).
pub const GRAVIDADE: f32 = 9.81 * 3.5;

// ── salto ─────────────────────────────────────────────────────
//
// Uma rampa de salto é o único lugar do jogo em que o carro **sai do chão**.
// Até aqui a altura vinha sempre da pista (`altura = pista.altura_em(s)`), que
// é o que mantém o carro colado no asfalto na serra; enquanto voa, ela passa a
// ser integrada pela gravidade e o piso é só o chão de aterrissagem.

/// Velocidade mínima para decolar de uma rampa, em m/s.
///
/// 14 m/s = 50 km/h. Abaixo disso o carro sobe e desce a rampa sem sair do
/// chão — quem passa devagar não paga o preço de um voo descontrolado.
pub const VEL_MIN_SALTO: f32 = 14.0;

/// Multiplicador da velocidade vertical na decolagem.
///
/// Fisicamente a componente vertical seria `v · inclinação`, e só. A 62 m/s
/// numa rampa de 22 % isso dá 13,6 m/s e um voo de 2,8 s — correto e sem graça.
/// O fator 1,7 é o que faz o salto ser **o** momento do mapa: 23 m/s de subida,
/// ~9 m de altura e quase 5 s de voo.
pub const BONUS_SALTO: f32 = 1.7;

/// Gravidade durante o voo, em m/s².
///
/// Mais fraca que a `GRAVIDADE` da ladeira de propósito: o voo é a parte
/// arcade, e com 34 m/s² o carro voltaria ao chão em meio segundo.
pub const GRAVIDADE_VOO: f32 = 15.0;

/// Queda, em m/s, a partir da qual a aterrissagem conta como impacto forte.
pub const QUEDA_FORTE: f32 = 9.0;
/// Rotação máxima do volante, em rad/s.
pub const GIRO_MAX: f32 = 1.75;
/// Aceleração extra do nitro, em m/s².
pub const NITRO_ACEL: f32 = 26.0;
/// Freio proporcional ao quanto a velocidade passa do topo **sem nitro**.
///
/// O motor zera no topo (`sobra = 0`), mas isso só impede acelerar mais: sem
/// esta parcela nada traz o carro **de volta** ao topo depois que o nitro acaba.
/// Medido no salar, com o acelerador preso: 82 → 62 levava **36 s**, mais que uma
/// volta inteira, e a corrida fechava com média de **78,2 m/s** contra um teto de
/// 82 — o nitro virava velocidade permanente e a recarga deixava de ser economia.
///
/// Não pode entrar no arrasto geral: lá ele também age **abaixo** do topo, e a
/// primeira versão dessa conta deixava o carro em 32,7 m/s em vez dos 62 que o
/// velocímetro promete. Aqui ele é zero em qualquer velocidade legal.
pub const FREIO_EXCESSO: f32 = 1.0;
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
    /// Decolou de uma rampa.
    Decolou,
    /// Voltou ao chão depois de um salto.
    Aterrissou,
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
    /// Altura do carro sobre o nível do mar, em metros.
    ///
    /// Com as rodas no chão vem da pista; **no ar** é integrada pela gravidade
    /// e fica acima do asfalto. É o que faz a câmera subir na serra, descer na
    /// descida e acompanhar o salto.
    pub altura: f32,
    /// `true` enquanto o carro está no ar, depois de sair de uma rampa.
    ///
    /// No ar ele não colide com mureta nem com obstáculo: quem salta passa por
    /// cima. O desenho usa isto para inclinar o carro e soltar poeira na
    /// aterrissagem.
    pub no_ar: bool,
    /// Velocidade vertical em m/s (positiva subindo). Zero com as rodas no chão.
    pub velocidade_vertical: f32,
    /// `s` do passo anterior, para saber se cruzou o topo de uma rampa.
    s_anterior: f32,
    /// `true` no passo em que o carro tocou a mureta, **mesmo de raspão**.
    ///
    /// Separado de `Evento::Bateu` de propósito: raspar é contínuo e não deve
    /// tocar o som de batida nem zerar o combo do drift, mas precisa soltar
    /// faísca. Antes, ou o toque era batida (e punha o carro a 1,3 m/s) ou não
    /// deixava sinal nenhum.
    pub raspando: bool,
    /// Força do último impacto, 0 (raspão) a 1 (de frente a 60 m/s).
    ///
    /// O desenho lê este número para decidir quantos fragmentos saem. Sem ele a
    /// colisão só saberia "bateu", e um encostar de 3 m/s jogaria a mesma
    /// chuva de destroços que uma batida a 200 km/h.
    pub impacto: f32,
    /// Índice da amostra da pista mais próxima (dica para a busca do próximo
    /// quadro — sem ela a localização seria uma varredura da volta inteira).
    pub indice_pista: Option<usize>,
    /// Posição na volta, em metros.
    pub s: f32,
    /// Limite de velocidade, como **fração** do teto (`1,0` = sem limite).
    ///
    /// Existe para o modo `desafiante`, em que o carro do jogador é 5 % mais
    /// lento que os adversários. Fingir isso mexendo só na IA deixaria o
    /// velocímetro da tela mostrando 223 km/h enquanto o carro entrega 212 — e o
    /// jogador culparia a pista.
    pub limitador: f32,
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
            altura: 0.0,
            no_ar: false,
            velocidade_vertical: 0.0,
            s_anterior: 0.0,
            raspando: false,
            impacto: 0.0,
            indice_pista: None,
            limitador: 1.0,
            s: 0.0,
        }
    }

    /// Teto de velocidade com o limitador em vigor.
    ///
    /// `nitro` escolhe entre o topo normal e o do nitro; o limitador entra nos
    /// dois, porque um nitro que passasse do teto limitado daria ao jogador a
    /// velocidade que o resto do carro não sustenta.
    pub fn teto(&self, nitro: bool) -> f32 {
        let base = if nitro { VEL_NITRO_MAX } else { VEL_MAX };
        base * self.limitador.clamp(0.5, 1.0)
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
        self.s_anterior = s;
        self.indice_pista = None;
        // Reposicionar cancela um salto em curso: teletransportar um carro no
        // ar para o chão é o que `posicionar` significa.
        self.no_ar = false;
        self.velocidade_vertical = 0.0;
        // A altura vem da pista **aqui**, e não só no primeiro `passo`.
        //
        // Faltava, e o efeito era grande: no mapa com 74 m de desnível o carro
        // nascia com `altura` 0 enquanto o piso estava a 74 — a câmera do título
        // ficava 74 m **abaixo** do asfalto, olhando o vazio, e no primeiro passo
        // a imagem saltava 74 m. Medido: `--json` dizia `"s":0,"altitude":0` e
        // `"s":1.2,"altitude":73.9`.
        self.altura = pista.altura_em(s);
        self.raspando = false;
        self.impacto = 0.0;
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
        self.raspando = false;
        let vel = self.velocidade;

        // ── motor, freio e arrasto ────────────────────────────────
        let vel_topo = self.teto(nitro_ativo);
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
        // A parcela **constante** é a resistência de rolagem + freio-motor. Ela
        // tem de ficar abaixo da gravidade numa descida de 11 % (3,7 m/s² com o
        // fator de arcade) — com os 4,5 da primeira versão o carro ficava
        // **parado** numa ladeira, e a serra virava um circuito plano.
        //
        // Em alta velocidade quem manda é o termo quadrático, então soltar o
        // acelerador a 150 km/h continua freando forte.
        let arrasto = if entrada.acelerar > 0.05 {
            0.55
        } else {
            1.6 + 0.07 * vel.abs() + 0.004 * vel * vel
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

        // ── gravidade na inclinação ──────────────────────────────
        //
        // É o que faz uma descida ser uma descida. `-g·sen(θ)`, e a inclinação
        // vem da pista em metros por metro — para ângulos pequenos,
        // `sen(θ) ≈ inclinação`, e o erro a 20 % de rampa é 1,3 %.
        //
        // O sinal segue o **sentido do movimento**: subindo de ré, a gravidade
        // empurra para frente; descendo de ré, para trás. Sem isso o carro
        // ganhava velocidade infinita ao descer em marcha à ré.
        let inclinacao = pista.inclinacao_em(self.s).clamp(-0.42, 0.42);
        let gravidade =
            -GRAVIDADE * inclinacao * if self.velocidade >= 0.0 { 1.0 } else { -1.0 };

        // Forças **mótricas**: empurram o carro e podem, sozinhas, tirá-lo do
        // repouso (motor, nitro, gravidade na ladeira e o pé no freio, que nesta
        // física também é a ré).
        let mut forca = motor + extra_nitro + gravidade;

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

        let mut nova = vel + forca * dt;

        // Rolagem, arrasto e atrito de fora **opõem o movimento**, e é isso que
        // eles têm de fazer — aplicados como força assinada (era
        // `forca -= arrasto`), eles empurravam um carro parado para trás:
        // medido, o carro solto na linha de largada chegava a **-8 m/s** e
        // ficava lá, porque `arrasto` cresce com `|v|` e nunca chega a zero.
        // Aqui a resistência só consome velocidade existente, limitada a ela:
        // freia até parar e nunca inverte o sentido.
        // Acima do `vel_topo` o carro freia **decisivamente** de volta a ele. A
        // constante de tempo é `1/FREIO_EXCESSO = 1 s`: 82 → 62 leva ~3 s, que é
        // o que se sente como "o nitro acabou", nem abrupto nem eterno.
        let excesso = (vel - vel_topo).max(0.0) * FREIO_EXCESSO;
        let resistencias = arrasto + atrito_extra + excesso;
        let consumo = (resistencias * dt).min(nova.abs());
        nova -= consumo * nova.signum();

        self.velocidade = nova.clamp(-8.0, self.teto(true));

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

        let altura_pista = pista.altura_em(local.s);
        let mut impacto_pouso = 0.0f32;

        // ── salto ─────────────────────────────────────────────────
        //
        // Três estados, e a ordem importa:
        //
        //   no chão  → pergunta se cruzou o topo de uma rampa rápida o bastante;
        //   no ar    → integra a gravidade e pergunta se voltou ao asfalto;
        //   no chão  → segue a pista, como sempre.
        //
        // A decolagem é decidida pelo **cruzamento do topo da rampa**, não por
        // "o piso caiu": o topo é um fato declarado no mapa, com `s` exato,
        // enquanto o piso é interpolado entre amostras e o cruzamento cairia
        // num passo imprevisível.
        if self.no_ar {
            self.velocidade_vertical -= GRAVIDADE_VOO * dt;
            self.altura += self.velocidade_vertical * dt;
            if self.altura <= altura_pista {
                // Aterrissou. A queda vira impacto: um salto de 9 m não pode
                // custar o mesmo que um degrau de 20 cm.
                let queda = -self.velocidade_vertical;
                self.altura = altura_pista;
                self.no_ar = false;
                self.velocidade_vertical = 0.0;
                evento = Evento::Aterrissou;
                if queda > QUEDA_FORTE {
                    impacto_pouso = (queda / 30.0).clamp(0.0, 1.0);
                }
            }
        } else if let Some(rampa) = pista.rampa_cruzada(self.s_anterior, local.s) {
            if self.velocidade > VEL_MIN_SALTO {
                self.no_ar = true;
                self.velocidade_vertical = self.velocidade * rampa.inclinacao() * BONUS_SALTO;
                self.altura = altura_pista;
                evento = Evento::Decolou;
            }
        }
        if !self.no_ar {
            self.altura = altura_pista;
        }

        // No ar o carro passa por cima de mureta e obstáculo — é o que faz o
        // salto valer a pena e o que impede um "bateu" no meio do voo.
        if !self.no_ar {
            if let Some(evento_novo) = self.resolver_limites(pista, &local, dt) {
                evento = evento_novo;
            }
            if let Some(evento_novo) = self.resolver_obstaculos(pista) {
                evento = evento_novo;
            }
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
        let rapido = self.velocidade > VEL_MIN_DRIFT && !self.no_ar;
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

        // O impacto é um pulso: sem decaimento, o primeiro toque deixaria o
        // carro "em colisão" pelo resto da corrida e cada quadro soltaria
        // destroços.
        self.impacto = (self.impacto - dt * 6.0).max(impacto_pouso);

        self.s_anterior = self.s;
        evento
    }

    /// Empurra o carro de volta para o asfalto quando ele passa pela borda.
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
        // O impacto é o que o jogador **fez**: ângulo contra a parede vezes
        // velocidade. Raspar (de_frente ~0) dá zero; bater de frente a 40 m/s
        // dá 1. É o número que separa "faísca" de "destroço".
        let impacto = (de_frente * (self.velocidade.abs() / 26.0)).clamp(0.0, 1.0);
        self.impacto = self.impacto.max(impacto);
        self.raspando = true;
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
            self.raspando = true;
            // Cone é objeto pequeno e duro: dói mais que a mureta na mesma
            // velocidade, e é o que justifica a chuva de destroços.
            self.impacto = self
                .impacto
                .max((self.velocidade.abs() / 20.0).clamp(0.2, 1.0));
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

    /// O nitro **empresta** velocidade; não a torna permanente.
    ///
    /// Com o acelerador preso e o tanque vazio, o carro tem de voltar ao topo em
    /// poucos segundos. Sem `FREIO_EXCESSO` ele levava **36 s** — medido no
    /// salar, onde a pista larga não obriga a frear e a corrida inteira ficava
    /// acima do topo, com média de 78,2 m/s contra um teto de 82.
    #[test]
    fn o_nitro_nao_deixa_velocidade_permanente() {
        let (mut c, pista) = carro_na_reta();
        // Tanque vazio: sem isso o piloto automático do teste abriria o nitro e
        // o carro nunca sairia de perto do teto de 82, que é outra pergunta.
        c.nitro = 0.0;
        c.velocidade = VEL_NITRO_MAX;
        simular(&mut c, &pista, Entrada::acelerando(), 4.0);
        assert!(
            c.velocidade < VEL_MAX + 1.5,
            "4 s depois de o nitro acabar o carro ainda estava a {:.1} m/s \
             (topo sem nitro = {VEL_MAX})",
            c.velocidade
        );
        assert!(
            c.velocidade > VEL_MAX - 8.0,
            "o carro caiu para {:.1} m/s: o excesso não pode virar freio forte \
             abaixo do topo",
            c.velocidade
        );
    }

    /// E acima do topo ele **não** age: o carro não pode ficar preso em 62 na
    /// descida, que é o que a ladeira usa para acelerar.
    #[test]
    fn abaixo_do_topo_o_freio_de_excesso_nao_aparece() {
        let (mut c, pista) = carro_na_reta();
        c.velocidade = VEL_MAX * 0.5;
        let antes = c.velocidade;
        simular(&mut c, &pista, Entrada::acelerando(), 1.0);
        assert!(
            c.velocidade > antes,
            "de {antes:.1} m/s o carro não acelerou: chegou a {:.1}",
            c.velocidade
        );
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
        // Velocidade **definida**, não alcançada: acelerar 6 s chegava a 58 m/s
        // (e não aos ~40 que o comentário antigo supunha), então o teste media
        // quanto tempo o carro levou para acelerar, não quanto o freio freia.
        c.velocidade = 40.0;
        let freando = Entrada {
            frear: 1.0,
            ..Entrada::PARADA
        };
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
    // ── relevo, raspão e impacto ────────────────────────────

    #[test]
    fn a_descida_acelera_o_carro_sem_acelerador() {
        // O que faz uma descida ser uma descida. Sem gravidade na inclinação, a
        // serra é um circuito plano com o asfalto torto — e o jogador não sente
        // diferença nenhuma entre os dois mapas.
        // Reta de 1000 m com 12 % de descida — a inclinação média da serra.
        // Numa pista com curvas o carro sai da pista em três segundos e o teste
        // passa a medir colisão, não gravidade.
        // 11 % de rampa: 56 m em 547 m, que é a inclinação média da serra.
        let pista = crate::pista::pista_de_teste_com_relevo(&[
            (0.00, 80.0),
            (0.25, 24.0),
            (0.50, 0.0),
            (0.75, 24.0),
            (1.00, 80.0),
        ]);
        let s_inicio = pista.comprimento() * 0.10;
        assert!(
            pista.inclinacao_em(s_inicio) < -0.02,
            "o trecho em {s_inicio:.0} m não desce (inclinação {:.3})",
            pista.inclinacao_em(s_inicio)
        );

        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, s_inicio, 0.0);
        // Solto: nenhum botão apertado.
        for _ in 0..150 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
        }
        assert!(
            c.velocidade > 3.0,
            "o carro solto não ganhou velocidade na descida: {:.2} m/s",
            c.velocidade
        );
    }

    #[test]
    fn a_subida_segura_o_carro() {
        // O espelho do teste acima, e ele é o que impede a "correção" de trocar
        // o sinal: com o sinal invertido, a volta pela encosta aceleraria sozinha
        // e ninguém notaria até a corrida ficar impossível de perder.
        let pista = crate::pista::pista_de_teste_com_relevo(&[
            (0.00, 0.0),
            (0.25, 56.0),
            (0.50, 80.0),
            (0.75, 56.0),
            (1.00, 0.0),
        ]);
        let s_subida = pista.comprimento() * 0.10;
        assert!(
            pista.inclinacao_em(s_subida) > 0.02,
            "o trecho em {s_subida:.0} m não sobe (inclinação {:.3})",
            pista.inclinacao_em(s_subida)
        );

        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, s_subida, 0.0);
        c.velocidade = 30.0;
        for _ in 0..60 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
        }
        assert!(
            c.velocidade < 30.0,
            "subindo solto o carro não perdeu velocidade: {:.2} m/s",
            c.velocidade
        );
    }

    #[test]
    fn a_altura_do_carro_acompanha_a_pista() {
        let pista = crate::mapa::BOAVISTA.pista();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, pista.comprimento() * 0.05, 0.0);
        c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
        let esperado = pista.altura_em(c.s);
        assert!(
            (c.altura - esperado).abs() < 0.5,
            "altura do carro {:.2} contra {esperado:.2} da pista",
            c.altura
        );
        assert!(c.altura > 30.0, "no alto da serra a altura devia passar de 30 m: {:.1}", c.altura);
    }

    #[test]
    fn posicionar_ja_deixa_o_carro_na_altura_da_pista() {
        // Sem isto o carro nasce em `altura` 0 enquanto o piso está a 74 m: a
        // câmera do título fica **abaixo** do asfalto e a imagem salta no
        // primeiro passo. O bug existia com o mapa novo e não aparecia em mapa
        // plano nenhum — por isso o teste usa a serra.
        let pista = crate::mapa::BOAVISTA.pista();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, 0.0, -3.0);
        assert!(
            (c.altura - pista.altura_em(0.0)).abs() < 0.01,
            "altura {:.2} contra {:.2} da pista na largada",
            c.altura,
            pista.altura_em(0.0)
        );
    }

    #[test]
    fn raspar_a_parede_de_lado_solta_faisca_sem_ser_batida() {
        // A queixa que originou isto: encostar na mureta não deixava sinal
        // nenhum — nem faísca, nem som, nem perda perceptível.
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        // Além do limite (meia-largura + 2,2 m): encostado é *fora* da margem,
        // senão o teste mede o carro andando no asfalto e "prova" um raspão que
        // não existe.
        c.posicionar(&pista, 200.0, pista.meia_largura_em(200.0) + 2.35);
        c.velocidade = 40.0;
        // Rumo **paralelo** à pista: é o caso do raspão.
        c.angulo = 0.0;

        let mut raspou = false;
        let mut maior_impacto = 0.0f32;
        for _ in 0..30 {
            c.passo(&Entrada { acelerar: 0.5, ..Entrada::PARADA }, &pista, 1.0 / 75.0);
            raspou |= c.raspando;
            maior_impacto = maior_impacto.max(c.impacto);
        }
        assert!(raspou, "o carro encostado não marcou raspão");
        assert!(
            maior_impacto < 0.5,
            "raspar de lado marcou impacto {maior_impacto:.2}; devia ser quase zero"
        );
        assert!(
            c.velocidade > 20.0,
            "raspar de lado parou o carro: {:.1} m/s",
            c.velocidade
        );
    }

    #[test]
    fn bater_de_frente_marca_impacto_alto() {
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, 200.0, pista.meia_largura_em(200.0) + 1.0);
        c.velocidade = 55.0;
        // Apontado 60° para a mureta: é uma batida, não um raspão.
        c.angulo = 1.05;

        let mut maior = 0.0f32;
        for _ in 0..40 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
            maior = maior.max(c.impacto);
        }
        assert!(maior > 0.6, "batida de frente marcou impacto só {maior:.2}");
    }

    #[test]
    fn parado_e_sem_entrada_o_carro_nao_anda_de_re() {
        // Medido antes da correção: o carro solto na linha de largada chegava a
        // **-8 m/s** e ficava lá. `arrasto` era somado como força assinada
        // (`forca -= arrasto`), e como ele cresce com `|v|` nunca chegava a
        // zero — o carro parado tinha uma aceleração constante para trás de
        // -1,6 m/s², do mesmo tipo de "anda de graça" que o freio-motor existe
        // para evitar, só que ao contrário.
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, 0.0, 0.0);

        let mut menor = 0.0f32;
        for _ in 0..300 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
            menor = menor.min(c.velocidade);
        }
        assert!(
            menor > -1e-3,
            "o carro parado andou de ré até {menor:.2} m/s"
        );
    }

    #[test]
    fn a_resistencia_freia_ate_zero_e_nao_inverte_o_sentido() {
        // A outra metade da mesma regra: com velocidade, a resistência tem de
        // consumir o movimento até parar — e parar, não passar para o outro
        // lado. Antes ela atravessava o zero e o carro terminava em ré.
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, 100.0, 0.0);
        c.velocidade = 3.0;

        for _ in 0..600 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
        }
        assert!(
            c.velocidade.abs() < 1e-3,
            "soltando a 3 m/s o carro terminou em {:.2} m/s",
            c.velocidade
        );
    }

    #[test]
    fn o_impacto_some_depois_da_batida() {
        // Sem decaimento o carro ficaria "em colisão" para sempre e cada quadro
        // soltaria destroços, mesmo andando longe da parede.
        let pista = crate::pista::pista_de_teste();
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.impacto = 1.0;
        for _ in 0..60 {
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
        }
        assert!(c.impacto < 0.01, "o impacto ficou em {:.2}", c.impacto);
    }
    // ── salto ─────────────────────────────────────────────────
    //
    // A pista real do circuito é usada de propósito: uma pista sintética
    // provaria a fórmula e não a integração com o mapa. As rampas de lá têm
    // inclinação declarada, e é ela que define a decolagem.

    fn pista_do_circuito() -> Pista {
        crate::mapa::por_id("circuito").expect("circuito").pista()
    }

    fn carro_na_rampa(pista: &Pista, indice: usize, velocidade: f32) -> Carro {
        let rampa = pista.rampas()[indice];
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        // 70 m antes do pé: dá tempo de o carro se alinhar e chegar ao topo
        // andando reto.
        c.posicionar(pista, rampa.inicio - 70.0, 0.0);
        c.velocidade = velocidade;
        c
    }

    #[test]
    fn o_carro_sai_do_chao_ao_cruzar_o_topo_da_rampa() {
        let pista = pista_do_circuito();
        let mut c = carro_na_rampa(&pista, 1, 60.0);
        let mut decolou = false;
        for _ in 0..200 {
            if c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0) == Evento::Decolou {
                decolou = true;
                break;
            }
        }
        assert!(decolou, "não decolou; s={:.0} de {:.0}", c.s, pista.comprimento());
        assert!(c.no_ar, "marcou Decolou e não está no ar");
        assert!(
            c.velocidade_vertical > 5.0,
            "velocidade vertical {:.1} m/s é fraca para um salto",
            c.velocidade_vertical
        );
    }

    #[test]
    fn devagar_o_carro_nao_decola() {
        // Abaixo de `VEL_MIN_SALTO` o carro sobe e desce a rampa sem sair do
        // chão: quem passa devagar não paga o preço de um voo descontrolado.
        let pista = pista_do_circuito();
        let rampa = pista.rampas()[1];
        // Começa a 6 m do pé já abaixo do mínimo: em 36 m de rampa ele acelera
        // ~4 m/s e chega ao topo ainda devagar.
        let mut c = Carro::novo(Ponto::ZERO, 0.0);
        c.posicionar(&pista, rampa.inicio - 6.0, 0.0);
        c.velocidade = VEL_MIN_SALTO - 6.0;
        let mut chegou = false;
        for _ in 0..300 {
            // A velocidade é **forçada** a cada quadro: sem isso o carro acelera
            // os 36 m de rampa, chega a 31 m/s e decola — o teste mediria a
            // aceleração, não a regra do salto.
            c.velocidade = VEL_MIN_SALTO - 4.0;
            c.passo(&Entrada::PARADA, &pista, 1.0 / 75.0);
            assert!(!c.no_ar, "decolou a {:.1} m/s (mínimo {VEL_MIN_SALTO})", c.velocidade);
            if c.s > rampa.topo {
                chegou = true;
                break;
            }
        }
        assert!(chegou, "nem chegou ao topo: s={:.0} (topo {:.0})", c.s, rampa.topo);
    }

    #[test]
    fn no_ar_a_altura_fica_acima_da_pista_e_volta_ao_chao() {
        let pista = pista_do_circuito();
        let mut c = carro_na_rampa(&pista, 1, 62.0);
        let mut pico = 0.0f32;
        let mut passos_no_ar = 0u32;
        let mut aterrissou = false;
        for _ in 0..400 {
            let ev = c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0);
            if c.no_ar {
                passos_no_ar += 1;
                pico = pico.max(c.altura - pista.altura_em(c.s));
            }
            if ev == Evento::Aterrissou {
                aterrissou = true;
                break;
            }
        }
        assert!(aterrissou, "nunca voltou ao chão");
        assert!(!c.no_ar, "aterrissou e continua no ar");
        assert_eq!(c.velocidade_vertical, 0.0, "sobrou velocidade vertical");
        assert!(
            (c.altura - pista.altura_em(c.s)).abs() < 0.01,
            "altura {:.2} não voltou para a pista {:.2}",
            c.altura,
            pista.altura_em(c.s)
        );
        assert!(pico > 3.0, "o salto só subiu {pico:.1} m — não é um salto");
        assert!(
            passos_no_ar > 60,
            "ficou no ar só {} passos ({:.1} s)",
            passos_no_ar,
            passos_no_ar as f32 / 75.0
        );
    }

    #[test]
    fn o_pouso_forte_solta_impacto_e_o_leve_nao() {
        // O impacto do pouso é o que faz o jogo soltar destroço e tremer a
        // câmera. Sem ele, um salto de 9 m chega ao chão como um degrau.
        let pista = pista_do_circuito();
        let mut c = carro_na_rampa(&pista, 1, 62.0);
        for _ in 0..400 {
            if c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0) == Evento::Aterrissou {
                break;
            }
        }
        assert!(c.impacto > 0.2, "pouso de um salto alto com impacto {:.2}", c.impacto);
    }

    #[test]
    fn no_ar_o_carro_passa_por_cima_da_mureta() {
        // Quem salta passa por cima: sem isto, o carro bate na borda no meio do
        // voo e o salto nunca completa.
        let pista = pista_do_circuito();
        let mut c = carro_na_rampa(&pista, 1, 62.0);
        // Chega ao ar.
        for _ in 0..200 {
            c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0);
            if c.no_ar {
                break;
            }
        }
        assert!(c.no_ar);
        // Empurra para fora da pista enquanto está no ar.
        let lateral = pista.normal_em(c.s);
        c.pos = c.pos.mais(lateral.vezes(24.0));
        let ev = c.passo(&Entrada::acelerando(), &pista, 1.0 / 75.0);
        assert!(
            c.no_ar && !matches!(ev, Evento::Bateu),
            "bateu no ar: fora={:.1} evento={ev:?}",
            c.fora
        );
    }

    #[test]
    fn o_salto_nao_credita_volta_nem_dispara_o_progresso() {
        // No ar o carro atravessa o miolo de uma curva e o `localizar` passa a
        // achar mais perto o trecho do outro lado: medido na rampa 2 do
        // circuito, o `s` saltou **96 m num quadro de 1/75 s**. Isso não é
        // deslocamento, é o localizador trocando de referência — e somar 96 m
        // por quadro creditaria voltas que o carro não deu.
        //
        // A prova é no **progresso da corrida**, não no `s` do carro: é ele que
        // decide volta e posição.
        let mapa = crate::mapa::por_id("circuito").expect("circuito");
        let mut corrida = crate::corrida::Corrida::com_mapa_e_config(mapa, 3, 300.0);
        corrida.comecar();
        corrida.contagem = 0.0;
        let rampa = corrida.pista.rampas()[1];
        corrida.reposicionar_em(rampa.inicio - 70.0);
        corrida.carro.velocidade = 62.0;

        let mut anterior = corrida.progresso_jogador();
        let mut maior_salto = 0.0f32;
        let mut voou = false;
        for _ in 0..400 {
            corrida.passo(&Entrada::acelerando(), 1.0 / 75.0);
            voou |= corrida.carro.no_ar;
            let agora = corrida.progresso_jogador();
            maior_salto = maior_salto.max((agora - anterior).abs());
            anterior = agora;
        }
        assert!(voou, "não voou — o teste não exercita o salto");
        assert!(
            maior_salto < 8.0,
            "o progresso andou {maior_salto:.1} m num quadro de 1/75 s"
        );
        assert_eq!(corrida.volta, 1, "o salto creditou uma volta");
    }

}
