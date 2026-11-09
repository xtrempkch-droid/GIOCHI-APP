//! A corrida: voltas, posição, tempo, vitória e derrota.
//!
//! ## Por que a corrida é um tipo, e não variáveis soltas no laço
//!
//! "Quem está em primeiro?" depende de quatro progressos, "acabou?" depende de
//! voltas **e** de tempo, e "venceu?" depende da ordem em que os carros
//! terminaram. Espalhar isso pelo laço de desenho produziria três verdades
//! parecidas e uma delas ficaria errada — foi assim que o carrossel do launcher
//! saiu de sincronia com o foco.
//!
//! Aqui é um tipo só, sem SDL e sem renderização, com teste para cada regra.

use crate::carro::{Carro, Entrada, Evento as EventoCarro, VEL_MAX};
use crate::pista::Ponto;
use crate::drift::Drift;
use crate::ia::Adversario;
use crate::pista::Pista;

/// Voltas padrão de uma corrida.
pub const VOLTAS: u32 = 2;
/// Tempo limite padrão, em segundos.
pub const TEMPO_LIMITE: f32 = 130.0;
/// Duração da contagem regressiva antes da largada.
pub const CONTAGEM: f32 = 3.0;

/// Em que tela o jogo está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    /// Tela inicial: título, controles e "Jogar".
    Titulo,
    /// Correndo (inclui a contagem regressiva).
    Correndo,
    /// Tela de resultado.
    Resultado,
}

/// Como a corrida terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desfecho {
    /// Primeiro lugar.
    Vitoria,
    /// Terminou, mas não em primeiro.
    ChegouDepois,
    /// O tempo limite acabou antes da última volta.
    TempoEsgotado,
}

impl Desfecho {
    /// Texto do resultado.
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Vitoria => "VITORIA",
            Self::ChegouDepois => "DERROTA",
            Self::TempoEsgotado => "TEMPO ESGOTADO",
        }
    }
}

/// Avisos da corrida para o som e para os efeitos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// Nada neste passo.
    Nada,
    /// Bateu (borda ou obstáculo).
    Bateu,
    /// Começou a derrapar.
    IniciouDrift,
    /// Fechou um drift e garantiu os pontos.
    TerminouDrift,
    /// Completou uma volta.
    Volta,
    /// Cruzou a linha final.
    Terminou,
}

/// Estado completo de uma partida.
#[derive(Debug, Clone)]
pub struct Corrida {
    /// Mapa em disputa: define voltas, tempo limite e o cenário desenhado.
    pub mapa: &'static crate::mapa::Mapa,
    /// Pista.
    pub pista: Pista,
    /// Carro do jogador.
    pub carro: Carro,
    /// Adversários.
    pub adversarios: Vec<Adversario>,
    /// Pontuação de drift.
    pub drift: Drift,
    /// Tela atual.
    pub fase: Fase,
    /// Volta atual (1-based; passa de `total_voltas` ao terminar).
    pub volta: u32,
    /// Voltas necessárias para vencer.
    pub total_voltas: u32,
    /// Tempo decorrido desde a largada, em segundos.
    pub tempo: f32,
    /// Tempo limite, em segundos.
    pub tempo_limite: f32,
    /// Segundos restantes de contagem regressiva (0 = largou).
    pub contagem: f32,
    /// Posição do jogador (1 = primeiro).
    pub posicao: usize,
    /// Desfecho, quando a corrida terminou.
    pub desfecho: Option<Desfecho>,
    /// `s` do quadro anterior, para detectar a passagem da linha.
    s_anterior: f32,
}

impl Corrida {
    /// `true` quando a corrida acabou e o resultado já está decidido.
    pub fn terminou(&self) -> bool {
        self.desfecho.is_some()
    }
}

impl Corrida {
    /// Partida nova no mapa padrão, parada na tela de título.
    pub fn nova() -> Self {
        Self::com_mapa(crate::mapa::PADRAO)
    }

    /// Partida nova num mapa, com as voltas e o tempo limite **dele**.
    ///
    /// Voltas e tempo vivem no mapa e não em constantes globais: um circuito de
    /// serra de 1,1 km com duas voltas em 150 s seria impossível, e a diferença
    /// tem de estar onde o mapa está declarado.
    pub fn com_mapa(mapa: &'static crate::mapa::Mapa) -> Self {
        Self::com_mapa_e_config(mapa, mapa.voltas, mapa.tempo_limite)
    }

    /// Partida nova com número de voltas e tempo limite escolhidos (teste).
    pub fn com_config(voltas: u32, tempo_limite: f32) -> Self {
        Self::com_mapa_e_config(crate::mapa::PADRAO, voltas, tempo_limite)
    }

    /// Partida nova num mapa, com voltas e tempo escolhidos (teste).
    pub fn com_mapa_e_config(
        mapa: &'static crate::mapa::Mapa,
        voltas: u32,
        tempo_limite: f32,
    ) -> Self {
        let pista = mapa.pista();
        let mut carro = Carro::novo(Ponto::ZERO, 0.0);
        carro.posicionar(&pista, 0.0, -3.0);

        // Largada em grade: o jogador atrás, três adversários à frente e ao
        // lado. A vantagem é de metros, não de uma volta.
        //
        // Eles já nasceram **antes** da linha (`comprimento - 8`), e isso
        // creditava uma volta inteira a quem andasse 8 m: `progresso` soma
        // `(volta-1)·comprimento + s`, e a volta ia a 2 no primeiro metro.
        // Medido: a IA aparecia 400 m à frente com a mesma velocidade do
        // jogador, e o piloto artificial — que fecha a volta em 25 s — perdia
        // sempre. Nascer depois da linha é o que faz a conta do progresso
        // significar o que ela diz.
        let adversarios = vec![
            Adversario::novo("VULTO", 18.0, 0.93, 0.0, 1),
            Adversario::novo("BRASA", 11.0, 0.89, 2.1, 2),
            Adversario::novo("TURBO", 4.0, 0.85, 4.3, 3),
        ];

        let mut corrida = Self {
            mapa,
            pista,
            carro,
            adversarios,
            drift: Drift::novo(),
            fase: Fase::Titulo,
            volta: 1,
            total_voltas: voltas.max(1),
            tempo: 0.0,
            tempo_limite,
            contagem: CONTAGEM,
            posicao: 1,
            desfecho: None,
            s_anterior: 0.0,
        };
        corrida.reposicionar_adversarios();
        corrida
    }

    /// Volta o jogo para a tela de título.
    pub fn titulo(&mut self) {
        self.fase = Fase::Titulo;
    }

    /// Começa a corrida (botão "Jogar").
    pub fn comecar(&mut self) {
        self.reiniciar();
        self.fase = Fase::Correndo;
    }

    /// Reinicia a corrida mantendo o número de voltas e o tempo limite.
    pub fn reiniciar(&mut self) {
        self.carro.posicionar(&self.pista, 0.0, -3.0);
        self.carro.nitro = 100.0;
        self.drift.reiniciar();
        self.volta = 1;
        self.tempo = 0.0;
        self.contagem = CONTAGEM;
        self.posicao = 1;
        self.desfecho = None;
        self.s_anterior = 0.0;
        self.reposicionar_adversarios();
        self.fase = Fase::Correndo;
    }

    fn reposicionar_adversarios(&mut self) {
        for (i, adversario) in self.adversarios.iter_mut().enumerate() {
            adversario.volta = 1;
            // Depois da linha, nunca antes: ver o comentário da grade.
            adversario.s = 18.0 - i as f32 * 7.0;
            adversario.lateral = if i % 2 == 0 { 3.0 } else { -3.0 };
            adversario.velocidade = 0.0;
        }
    }

    /// Progresso do jogador, em metros desde a largada.
    pub fn progresso_jogador(&self) -> f32 {
        (self.volta.saturating_sub(1)) as f32 * self.pista.comprimento() + self.carro.s
    }

    /// Posição do jogador na corrida, recalculada.
    pub fn calcular_posicao(&mut self) {
        let meu = self.progresso_jogador();
        let atras_de_mim = self
            .adversarios
            .iter()
            .filter(|a| a.progresso(&self.pista) > meu)
            .count();
        self.posicao = atras_de_mim + 1;
    }

    /// Avança a corrida um passo.
    pub fn passo(&mut self, entrada: &Entrada, dt: f32) -> Evento {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 || self.fase != Fase::Correndo {
            return Evento::Nada;
        }

        // Contagem regressiva: os carros ainda não andam, e o tempo não corre.
        // Isso é o que dá a largada justa — sem ela o jogador sairia depois de
        // os adversários já estarem em movimento, porque ele é quem aperta o
        // botão.
        if self.contagem > 0.0 {
            self.contagem = (self.contagem - dt).max(0.0);
            return Evento::Nada;
        }

        self.tempo += dt;

        let evento_carro = self.carro.passo(entrada, &self.pista, dt);
        self.drift
            .passo(self.carro.derrapando, self.carro.velocidade, self.carro.derrapagem, dt);

        let mut evento = match evento_carro {
            EventoCarro::Bateu => {
                self.drift.colidir();
                Evento::Bateu
            }
            EventoCarro::IniciouDrift => Evento::IniciouDrift,
            EventoCarro::TerminouDrift => Evento::TerminouDrift,
            EventoCarro::Nada => Evento::Nada,
        };

        // Passagem da linha: o `s` dá a volta. Testar por "diminuiu muito" e não
        // por "chegou perto de zero" evita perder a volta quando o carro cruza
        // a linha com o `s` já um pouco adiante.
        let comprimento = self.pista.comprimento();
        let s = self.carro.s;
        if self.s_anterior > comprimento * 0.6 && s < comprimento * 0.4 {
            self.volta = self.volta.saturating_add(1);
            if self.volta > self.total_voltas {
                self.terminar(Desfecho::Vitoria, Evento::Terminou);
                return Evento::Terminou;
            }
            if evento == Evento::Nada {
                evento = Evento::Volta;
            }
        }
        self.s_anterior = s;

        for adversario in self.adversarios.iter_mut() {
            adversario.passo(&self.pista, dt);
        }

        self.calcular_posicao();

        // Derrota por tempo: só conta se ainda não terminou.
        if self.tempo >= self.tempo_limite && self.desfecho.is_none() {
            self.terminar(Desfecho::TempoEsgotado, Evento::Terminou);
            return Evento::Terminou;
        }

        evento
    }

    /// Encerra a corrida com o desfecho calculado.
    ///
    /// A posição final não é a do quadro: quem já cruzou a linha conta como à
    /// frente, mesmo que o `s` dele tenha voltado a zero. Sem isso, cruzar a
    /// linha em primeiro e ver "3º" no resultado seria o padrão.
    fn terminar(&mut self, candidato: Desfecho, _evento: Evento) {
        let total = self.total_voltas as f32 * self.pista.comprimento();
        let ja_terminaram = self
            .adversarios
            .iter()
            .filter(|a| a.progresso(&self.pista) >= total)
            .count();
        // O jogador acabou de cruzar; entre os que terminaram, ele é o último.
        let posicao_final = ja_terminaram + 1;

        self.desfecho = Some(match candidato {
            Desfecho::TempoEsgotado => Desfecho::TempoEsgotado,
            _ if posicao_final == 1 => Desfecho::Vitoria,
            _ => Desfecho::ChegouDepois,
        });
        self.posicao = posicao_final;
        self.fase = Fase::Resultado;
    }

    /// Segundos restantes do tempo limite (0 quando estourou).
    pub fn tempo_restante(&self) -> f32 {
        (self.tempo_limite - self.tempo).max(0.0)
    }

    /// Velocidade máxima usada como referência de barra no HUD.
    pub fn velocidade_referencia(&self) -> f32 {
        VEL_MAX
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn dirigir(corrida: &mut Corrida, segundos: f32, entrada: Entrada) {
        let dt = 1.0 / 75.0;
        for _ in 0..(segundos / dt).round() as u32 {
            corrida.passo(&entrada, dt);
        }
    }

    /// Piloto artificial: mira um ponto à frente na pista e freia no que vem.
    ///
    /// Serve para provar que a corrida **termina e é ganhável** sem depender de
    /// perícia humana. Ele não é um jogador bom — é um jogador competente: se
    /// nem ele vence, a corrida está desbalanceada.
    fn piloto_automatico(corrida: &mut Corrida, segundos: f32) {
        let dt = 1.0 / 75.0;
        let passos = (segundos / dt).round() as u32;
        let pista = corrida.pista.clone();
        for _ in 0..passos {
            let s = corrida.carro.s;
            // Ponto de mira: à frente, e não a direção no `s` atual. Mirar no
            // próprio `s` faz o carro sempre atrasado em relação à curva e ele
            // sai pela borda de fora.
            let alvo = pista.ponto_em(s + 14.0);
            let direcao = alvo.menos(corrida.carro.pos);
            let angulo_alvo = direcao.x.atan2(direcao.z);
            let mut erro = angulo_alvo - corrida.carro.angulo;
            while erro > std::f32::consts::PI {
                erro -= std::f32::consts::TAU;
            }
            while erro < -std::f32::consts::PI {
                erro += std::f32::consts::TAU;
            }

            // Curvatura em 45 m de antecedência: é o que decide soltar o
            // acelerador antes da curva em vez de dentro dela.
            let aqui = pista.direcao_em(s + 8.0);
            let frente = pista.direcao_em(s + 53.0);
            let curva = aqui.ponto(frente).clamp(-1.0, 1.0).acos();
            let rapido = corrida.carro.velocidade > 1.0;

            let (acelerar, frear) = if curva > 0.85 && corrida.carro.velocidade > 26.0 {
                (0.0, 1.0)
            } else if curva > 0.55 && corrida.carro.velocidade > 34.0 {
                (0.15, 0.0)
            } else if rapido && corrida.carro.velocidade < 40.0 {
                (1.0, 0.0)
            } else {
                (1.0, 0.0)
            };

            let entrada = Entrada {
                acelerar,
                frear,
                virar: (erro * 3.2).clamp(-1.0, 1.0),
                ..Entrada::PARADA
            };
            corrida.passo(&entrada, dt);
            if corrida.fase == Fase::Resultado {
                return;
            }
        }
    }

    #[test]
    fn comeca_no_titulo_e_nao_anda_antes_de_jogar() {
        let mut c = Corrida::nova();
        assert_eq!(c.fase, Fase::Titulo);
        let antes = c.carro.pos;
        c.passo(&Entrada::acelerando(), 1.0 / 75.0);
        assert_eq!(c.carro.pos, antes, "o carro andou na tela de título");
        assert_eq!(c.tempo, 0.0);
    }

    #[test]
    fn a_contagem_segura_os_carros() {
        let mut c = Corrida::nova();
        c.comecar();
        assert!((c.contagem - CONTAGEM).abs() < 0.01);
        dirigir(&mut c, 1.0, Entrada::acelerando());
        assert!(
            c.carro.velocidade < 0.5,
            "andou durante a contagem: {:.2} m/s",
            c.carro.velocidade
        );
        assert_eq!(c.tempo, 0.0, "o cronômetro não pode correr na contagem");
        dirigir(&mut c, CONTAGEM, Entrada::acelerando());
        assert!(c.carro.velocidade > 1.0, "não largou depois da contagem");
    }

    #[test]
    fn o_jogador_completa_duas_voltas_e_vence() {
        let mut c = Corrida::nova();
        c.comecar();
        dirigir(&mut c, CONTAGEM + 0.1, Entrada::PARADA);
        piloto_automatico(&mut c, 120.0);
        assert_eq!(c.fase, Fase::Resultado, "a corrida não terminou em 2 min");
        assert_eq!(
            c.desfecho,
            Some(Desfecho::Vitoria),
            "terminou em {:?} na posição {} com tempo {:.1}",
            c.desfecho,
            c.posicao,
            c.tempo
        );
    }

    #[test]
    fn o_tempo_limite_derruba_quem_nao_termina() {
        let mut c = Corrida::com_config(2, 6.0);
        c.comecar();
        dirigir(&mut c, CONTAGEM + 7.0, Entrada::PARADA);
        assert_eq!(c.desfecho, Some(Desfecho::TempoEsgotado));
        assert_eq!(c.fase, Fase::Resultado);
    }

    #[test]
    fn a_volta_e_contada_ao_cruzar_a_linha() {
        let mut c = Corrida::nova();
        c.comecar();
        assert_eq!(c.volta, 1);
        c.contagem = 0.0;
        // Põe o carro logo antes da linha e empurra para depois dela.
        c.carro.posicionar(&c.pista, c.pista.comprimento() - 3.0, 0.0);
        c.s_anterior = c.pista.comprimento() - 3.0;
        piloto_automatico(&mut c, 1.0);
        assert_eq!(c.volta, 2, "não contou a volta; s={:.1}", c.carro.s);
    }

    #[test]
    fn reiniciar_devolve_tudo_ao_comeco() {
        let mut c = Corrida::nova();
        c.comecar();
        dirigir(&mut c, CONTAGEM + 5.0, Entrada::acelerando());
        assert!(c.carro.velocidade > 1.0);
        let pontos_antes = c.drift.total();
        c.reiniciar();
        assert_eq!(c.volta, 1);
        assert_eq!(c.tempo, 0.0);
        assert_eq!(c.carro.velocidade, 0.0);
        assert_eq!(c.drift.total(), 0);
        let _ = pontos_antes;
        assert_eq!(c.fase, Fase::Correndo);
        assert_eq!(c.desfecho, None);
    }

    #[test]
    fn a_posicao_e_um_quando_o_jogador_esta_na_frente() {
        let mut c = Corrida::nova();
        c.comecar();
        c.contagem = 0.0;
        // Adversários parados no fim da volta, jogador adiantado.
        for a in c.adversarios.iter_mut() {
            a.s = 5.0;
            a.volta = 1;
        }
        c.carro.posicionar(&c.pista, 200.0, 0.0);
        c.calcular_posicao();
        assert_eq!(c.posicao, 1);
    }

    #[test]
    fn a_posicao_conta_quem_esta_a_frente() {
        let mut c = Corrida::nova();
        c.comecar();
        c.contagem = 0.0;
        c.carro.posicionar(&c.pista, 100.0, 0.0);
        for (i, a) in c.adversarios.iter_mut().enumerate() {
            a.s = 120.0 + i as f32 * 10.0;
            a.volta = 1;
        }
        c.calcular_posicao();
        assert_eq!(c.posicao, 4, "três à frente = quarto");
    }

    #[test]
    fn quem_ja_terminou_fica_a_frente_no_resultado() {
        let mut c = Corrida::com_config(1, 200.0);
        c.comecar();
        c.contagem = 0.0;
        // Um adversário já completou a volta.
        c.adversarios[0].volta = 2;
        c.adversarios[0].s = 1.0;
        c.carro.posicionar(&c.pista, c.pista.comprimento() - 2.0, 0.0);
        c.s_anterior = c.pista.comprimento() - 2.0;
        piloto_automatico(&mut c, 2.0);
        assert_eq!(c.fase, Fase::Resultado);
        assert_eq!(c.posicao, 2, "quem já cruzou tem de ficar à frente");
        assert_eq!(c.desfecho, Some(Desfecho::ChegouDepois));
    }

    #[test]
    fn o_tempo_restante_nunca_e_negativo() {
        let mut c = Corrida::com_config(2, 1.0);
        c.comecar();
        c.contagem = 0.0;
        dirigir(&mut c, 5.0, Entrada::PARADA);
        assert_eq!(c.tempo_restante(), 0.0);
    }

    #[test]
    fn tres_adversarios_na_pista() {
        let c = Corrida::nova();
        assert_eq!(c.adversarios.len(), 3, "o pedido era 2 ou 3 adversários");
        let nomes: Vec<&str> = c.adversarios.iter().map(|a| a.nome).collect();
        assert_eq!(nomes.len(), 3);
        for a in &c.adversarios {
            assert!(!a.nome.is_empty());
        }
    }
}
