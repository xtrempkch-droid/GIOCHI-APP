//! Piloto artificial: uma entrada de controle calculada a partir da pista.
//!
//! ## Por que isto vive no núcleo, e não num teste
//!
//! Nasceu como ajudante de teste, para provar que a corrida é ganhável sem dedo
//! humano. Só que ele resolve um problema de **diagnóstico**: sem ninguém
//! dirigindo, não há como capturar um quadro no meio da pista, na curva fechada,
//! dentro do túnel ou derrapando. O `--passos` do binário, sozinho, só levava o
//! carro de encontro ao primeiro muro — medido: 900 passos, 2,6 m/s, encostado na
//! borda, e o quadro mostrava a linha de largada como se a corrida não tivesse
//! começado.
//!
//! Aqui ele é código de produção, com teste, e o binário o usa por `--piloto`.
//!
//! ## O que ele **não** é
//!
//! Um jogador bom. É um jogador competente: mira à frente, freia antes da curva
//! e acelera na saída. Se nem ele vence, a corrida está desbalanceada — e é
//! exatamente por isso que ele serve de referência.

use crate::carro::Entrada;
use crate::corrida::Corrida;

/// Segundos à frente em que o piloto mira.
const MIRA: f32 = 14.0;
/// Antecedência da leitura de curvatura, em metros.
const CURVATURA_AQUI: f32 = 8.0;
/// Antecedência mínima da leitura, em metros.
///
/// **Ela cresce com a velocidade**: a 75 m/s o carro anda 75 m por segundo, e
/// olhar 53 m à frente é olhar 0,7 s — tarde demais para frear de 75 a 30 m/s,
/// que precisa de ~65 m. Medido antes desta correção: o piloto chegava na curva
/// fechada a **75,5 m/s** e saía pela borda de fora.
const CURVATURA_FRENTE_BASE: f32 = 26.0;
/// Metros de antecedência por m/s de velocidade.
const CURVATURA_FRENTE_POR_VELOCIDADE: f32 = 1.1;
/// Curvatura (radianos de diferença de rumo) que faz soltar o acelerador.
const CURVA_FORTE: f32 = 0.85;
/// Curvatura que caracteriza a curva fechada, onde o freio de mão entra.
const CURVA_FECHADA: f32 = 1.25;
/// Abaixo disto a pista é reta o suficiente para o nitro.
const RETA: f32 = 0.20;

/// Entrada de controle para o quadro atual.
///
/// O `s` vem do carro (posição na volta, em metros) e a direção da pista não é
/// consultada no `s` atual, e sim no ponto de mira: mirar no próprio `s` deixa o
/// carro sempre atrasado em relação à curva, e ele sai pela borda de fora.
pub fn entrada(corrida: &Corrida) -> Entrada {
    let pista = &corrida.pista;
    let carro = &corrida.carro;

    let alvo = pista.ponto_em(carro.s + MIRA);
    let direcao = alvo.menos(carro.pos);
    let angulo_alvo = direcao.x.atan2(direcao.z);
    let mut erro = angulo_alvo - carro.angulo;
    // Normaliza para [-π, π]: sem isto o erro de 350° vira "vire quase uma volta".
    while erro > std::f32::consts::PI {
        erro -= std::f32::consts::TAU;
    }
    while erro < -std::f32::consts::PI {
        erro += std::f32::consts::TAU;
    }

    let aqui = pista.direcao_em(carro.s + CURVATURA_AQUI);
    let olhar = CURVATURA_FRENTE_BASE + carro.velocidade * CURVATURA_FRENTE_POR_VELOCIDADE;
    let frente = pista.direcao_em(carro.s + olhar);
    let curva = aqui.ponto(frente).clamp(-1.0, 1.0).acos();

    let (acelerar, frear) = if curva > CURVA_FORTE && carro.velocidade > 26.0 {
        (0.0, 1.0)
    } else if curva > 0.55 && carro.velocidade > 34.0 {
        (0.15, 0.0)
    } else {
        (1.0, 0.0)
    };

    // Freio de mão só na curva fechada e já em velocidade: é o que gera o drift
    // e a fumaça. Cedo demais, ele transforma toda curva num pião.
    let freio_mao = curva > CURVA_FECHADA && carro.velocidade > 30.0;

    // Nitro na reta e com tanque. `carro.nitro_usando` avisa que o passo anterior
    // já estava gastando — manter o pedal evita ligar/desligar a cada quadro.
    let nitro = (curva < RETA || carro.nitro_usando) && carro.nitro > 15.0;

    Entrada {
        acelerar,
        frear,
        virar: (erro * 3.2).clamp(-1.0, 1.0),
        freio_mao,
        nitro,
    }
}

/// Dirige por `segundos`, em passos de 75 Hz, e devolve quantos passos deu.
///
/// Usado pelos testes do núcleo e pelo `--passos` do binário; parar quando a
/// corrida termina é o que impede um laço que "roda para sempre" num teste.
pub fn dirigir(corrida: &mut Corrida, segundos: f32) -> u32 {
    let dt = 1.0 / crate::relogio::ALVO_FPS as f32;
    let passos = (segundos / dt).round().max(0.0) as u32;
    for i in 0..passos {
        let entrada = entrada(corrida);
        corrida.passo(&entrada, dt);
        if corrida.terminou() {
            return i + 1;
        }
    }
    passos
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::corrida::Fase;

    #[test]
    fn o_piloto_sai_da_largada_e_nao_fica_parado() {
        let mut c = Corrida::nova();
        c.comecar();
        // A contagem regressiva segura os carros por 3 s; medir antes disso
        // mediria o freio de estacionamento, não o piloto.
        dirigir(&mut c, 4.5);
        assert!(
            c.carro.velocidade > 10.0,
            "depois de 4,5 s o carro está a {:.1} m/s",
            c.carro.velocidade
        );
    }

    #[test]
    fn atravessar_a_linha_nao_trava_a_localizacao() {
        // A janela do `localizar` era linear e prendia a dica no fim do vetor:
        // ao cruzar a linha o carro era localizado 2 m depois dela com 12 m de
        // desvio lateral, e ficava a 2,7 m/s empurrando a mureta para sempre.
        let mut c = Corrida::nova();
        c.comecar();
        let comprimento = c.pista.comprimento();
        // Põe o carro 12 m antes da linha, já em velocidade de corrida.
        c.carro.posicionar(&c.pista, comprimento - 12.0, 0.0);
        c.carro.velocidade = 40.0;
        let mut cruzou = false;
        let mut avanco_maximo = 0.0f32;
        for _ in 0..(4.0 * 75.0) as u32 {
            let antes = c.carro.s;
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            if antes > comprimento * 0.8 && c.carro.s < comprimento * 0.2 {
                cruzou = true;
            }
            if cruzou {
                avanco_maximo = avanco_maximo.max(c.carro.s);
            }
        }
        assert!(cruzou, "o carro não cruzou a linha");
        assert!(
            avanco_maximo > 40.0,
            "depois da linha o carro só avançou {avanco_maximo:.1} m em 4 s — travou"
        );
        assert!(c.carro.fora < 1.0, "saiu da pista logo depois da linha");
    }

    #[test]
    fn o_piloto_completa_uma_volta_sem_sair_da_pista() {
        let mut c = Corrida::nova();
        c.comecar();
        let mut pior_fora = 0.0f32;
        for _ in 0..(60.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            pior_fora = pior_fora.max(c.carro.fora);
            if c.volta > 1 || c.terminou() {
                break;
            }
        }
        assert!(
            c.volta > 1 || c.terminou(),
            "uma volta em 60 s: parou na volta {} a {:.0} m",
            c.volta,
            c.carro.s
        );
        assert!(
            pior_fora < 3.0,
            "o piloto saiu {pior_fora:.1} m da pista — ele é a referência de que a corrida é ganhável"
        );
    }

    #[test]
    fn o_piloto_vence_a_corrida() {
        let mut c = Corrida::nova();
        c.comecar();
        // 150 s é mais que o dobro do limite; se não venceu, não é o relógio.
        for _ in 0..(150.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            if c.terminou() {
                break;
            }
        }
        assert_eq!(c.fase, Fase::Resultado, "a corrida não terminou");
        assert!(
            c.volta > c.total_voltas,
            "terminou na volta {} de {}",
            c.volta,
            c.total_voltas
        );
    }

    #[test]
    fn o_piloto_usa_nitro_na_reta_e_gasta_o_tanque() {
        let mut c = Corrida::nova();
        c.comecar();
        let mut usou = false;
        for _ in 0..(20.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            usou |= c.carro.nitro_usando;
            if usou {
                break;
            }
        }
        assert!(usou, "20 s de corrida e o piloto nunca abriu o nitro");
    }

    #[test]
    fn o_piloto_derruba_a_traseira_na_curva_fechada() {
        let mut c = Corrida::nova();
        c.comecar();
        let mut derrapou = false;
        for _ in 0..(90.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            if c.carro.derrapando {
                derrapou = true;
                break;
            }
            if c.terminou() {
                break;
            }
        }
        assert!(
            derrapou,
            "o piloto não derrapou em 90 s; o drift é a mecânica central do jogo"
        );
    }

    #[test]
    fn a_vitoria_nao_e_facil_demais_nem_impossivel() {
        // A margem é o que separa "corrida" de "passeio" e de "parede". O piloto
        // é um jogador competente, não um recordista: se ele ganha por 30 s, a
        // IA não está correndo; se perde, a corrida não é ganhável.
        let mut c = Corrida::nova();
        c.comecar();
        for _ in 0..(150.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            if c.terminou() {
                break;
            }
        }
        assert_eq!(c.fase, Fase::Resultado, "a corrida não terminou");
        let total = c.total_voltas as f32 * c.pista.comprimento();
        let melhor_adversario = c
            .adversarios
            .iter()
            .map(|a| a.progresso(&c.pista))
            .fold(0.0f32, f32::max);
        let metros_de_folga = total - melhor_adversario;
        let segundos = metros_de_folga / 40.0;
        assert!(
            (1.0..25.0).contains(&segundos),
            "folga de {metros_de_folga:.0} m (~{segundos:.0} s): fora da faixa 1..25 s"
        );
    }

    #[test]
    fn o_piloto_freia_antes_da_curva_e_nao_dentro_dela() {
        // Medir a velocidade máxima "90 m antes" não diz nada: é a velocidade de
        // chegada, e chegar rápido e frear é o certo. O que prova a frenagem é
        // a ordem dos fatos: entra rápido, passa no ápice devagar.
        let mut c = Corrida::nova();
        c.comecar();
        let alvo = c.pista.s_de_raio_minimo();
        let mut na_chegada: Option<f32> = None;
        let mut no_apice: Option<f32> = None;
        for _ in 0..(90.0 * 75.0) as u32 {
            let entrada = entrada(&c);
            c.passo(&entrada, 1.0 / 75.0);
            let s = c.carro.s;
            let distancia = (s - alvo).abs();
            if (distancia - 80.0).abs() < 1.5 {
                na_chegada = Some(c.carro.velocidade);
            }
            if distancia < 6.0 {
                no_apice = Some(c.carro.velocidade);
                break;
            }
            if c.terminou() || (s > alvo + 60.0 && alvo < 60.0) {
                break;
            }
        }
        let chegada = na_chegada.expect("nunca passou a 80 m do ápice");
        let apice = no_apice.expect("nunca passou perto do ápice");
        assert!(
            apice < 42.0,
            "chegou no ápice a {apice:.1} m/s — a curva fechada não é fechada para ele"
        );
        assert!(
            apice < chegada - 8.0,
            "chegou a {chegada:.1} m/s e passou no ápice a {apice:.1} m/s: quase não freou"
        );
    }
}


