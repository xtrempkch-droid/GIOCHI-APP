//! Os mapas do jogo: traçado, larguras, túneis, relevo e cenário.
//!
//! ## Por que um mapa é **dado**, e não código
//!
//! A primeira versão tinha o traçado em constantes dentro de `pista.rs` e o
//! cenário em funções dentro de `cena3d.rs`. Acrescentar um mapa assim significa
//! editar dois arquivos em dois crates, e o segundo é o que desenha — ou seja, o
//! lugar onde um erro aparece como "a pista está estranha" e não como erro.
//!
//! Aqui o mapa é uma estrutura de dados: traçado, larguras, túneis, **relevo** e
//! o tipo de cenário. `Pista::do_mapa` consome o traçado; `Cena3D` consome o tipo
//! de cenário. Nenhum dos dois sabe o nome de um mapa específico.
//!
//! ## Relevo: por que a descida precisa de altura de verdade
//!
//! "Descida do Alto da Boavista" sem descida é um circuito plano com nome
//! bonito. O relevo entra como uma lista de pares `(fração da volta, metros)`
//! interpolada no comprimento de arco, e o que ele muda é:
//!
//! * a geometria — o asfalto sobe e desce, e a câmera também;
//! * a física — a gravidade empurra o carro na descida e segura na subida;
//! * o desenho — a cidade fica **abaixo** do jogador, que é o que dá a sensação
//!   de altura.
//!
//! O perfil é periódico (a fração 1,0 tem de casar com a 0,0), senão o carro
//! levaria um degrau na linha de chegada. Há teste para isso.

use crate::pista::{Ponto, Pista};

/// Como o cenário em volta da pista é construído.
///
/// O tipo **não** é o nome do mapa: dois mapas podem ser `Avenida` com traçados
/// diferentes. É a decisão de desenho que o `Cena3D` consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cenario {
    /// Cidade noturna: prédios, janelas, outdoors, túnel de concreto.
    Avenida,
    /// Serra: mata dos dois lados, rocha do lado de dentro da curva, guarda-corpo
    /// e a cidade **lá embaixo**, vista de cima.
    Descida,
    /// Pista de teste fechada: barreiras listradas, torres de luz e arquibancada.
    /// É onde as rampas ficam, porque um circuito de verdade não tem salto.
    Circuito,
    /// Orla à beira d'água: contêineres empilhados, guindastes e o mar escuro.
    Porto,
    /// Planície de sal com pista larga e referências distantes — o mapa mais
    /// rápido, onde o nitro é a única curva.
    Salar,
}

/// Uma rampa de salto: o carro sobe, decola no topo e voa até cair.
///
/// ## Por que uma rampa é dado declarado, e não um morrinho no relevo
///
/// Um "morrinho" no perfil de elevação é uma ladeira suave: o carro desce
/// acompanhando o chão, porque a física gruda o carro no asfalto. Para o carro
/// **sair do chão** é preciso que o piso caia mais rápido do que a gravidade
/// conseguiria trazê-lo de volta — ou seja, um corte seco depois da subida.
///
/// Isso não dá para expressar num perfil de `(fração, altura)`: o perfil liga
/// dois pontos com uma reta, e "topo em 4 m e logo abaixo 0 m" só existiria se
/// os dois pontos estivessem a centímetros um do outro. Declarar a rampa
/// separada deixa o **topo** ser um fato do mapa, com `s` exato, e a física
/// pergunta exatamente isso: "cruzei o topo de uma rampa neste passo?".
#[derive(Debug, Clone, Copy)]
pub struct Rampa {
    /// Onde o pé da rampa começa, como fração da volta (0..1).
    pub fracao: f32,
    /// Comprimento da subida, em metros.
    pub comprimento: f32,
    /// Altura do topo acima do asfalto, em metros.
    pub altura: f32,
}

/// Um mapa jogável.
#[derive(Debug, Clone, Copy)]
pub struct Mapa {
    /// Identidade estável, usada na CLI e no catálogo.
    pub id: &'static str,
    /// Nome que aparece na tela.
    pub nome: &'static str,
    /// Onde ele fica, para a ficha.
    pub local: &'static str,
    /// Uma linha sobre o que o mapa pede do jogador.
    pub descricao: &'static str,
    /// Como o cenário é construído.
    pub cenario: Cenario,
    /// Voltas para vencer.
    pub voltas: u32,
    /// Tempo limite, em segundos.
    pub tempo_limite: f32,
    /// Dificuldade de 1 a 5, para a ficha.
    pub dificuldade: u8,
    /// Pontos de controle do traçado.
    pub controles: &'static [Ponto],
    /// Meia-largura no trecho que **começa** em cada ponto de controle.
    pub larguras: &'static [f32],
    /// Índices de ponto de controle onde o trecho é túnel.
    pub tuneis: &'static [usize],
    /// Perfil de elevação: `(fração da volta 0..1, altura em metros)`.
    ///
    /// Vazio significa plano. Ordenado por fração; a última fração tem de ser
    /// `1.0` com a mesma altura da primeira.
    pub relevo: &'static [(f32, f32)],
    /// Rampas de salto. Vazio significa nenhuma.
    ///
    /// Elas são somadas **em cima** do perfil de elevação: um mapa pode ter
    /// serra e rampa no meio dela.
    pub rampas: &'static [Rampa],
}

impl Mapa {
    /// Constrói a pista deste mapa.
    pub fn pista(&self) -> Pista {
        Pista::do_mapa(self)
    }

    /// Altura máxima do perfil, em metros (0 num mapa plano).
    pub fn desnivel(&self) -> f32 {
        let mut maior = 0.0f32;
        let mut menor = 0.0f32;
        for (_, h) in self.relevo {
            maior = maior.max(*h);
            menor = menor.min(*h);
        }
        maior - menor
    }

    /// Altura no ponto da volta indicado por `fracao` (0..1).
    ///
    /// Interpolação linear entre os pares do perfil. Fora de 0..1 a fração é
    /// normalizada — o perfil é periódico, e um `s` negativo (que acontece na
    /// primeira amostra da volta) tem de devolver a altura do fim, não a do
    /// começo em linha reta.
    pub fn altura_em(&self, fracao: f32) -> f32 {
        if self.relevo.is_empty() {
            return 0.0;
        }
        let f = fracao - fracao.floor();
        for par in self.relevo.windows(2) {
            let (a, ha) = par[0];
            let (b, hb) = par[1];
            if f >= a && f <= b {
                let t = if (b - a).abs() < 1e-6 {
                    0.0
                } else {
                    (f - a) / (b - a)
                };
                return ha + (hb - ha) * t;
            }
        }
        // Antes do primeiro par: a altura do primeiro. Não pode acontecer com um
        // perfil que começa em 0,0, mas devolver 0 é melhor que `NaN`.
        self.relevo.first().map(|(_, h)| *h).unwrap_or(0.0)
    }
}

// ════════════════════════════════════════════════════════════
// AVENIDA — o circuito original
// ════════════════════════════════════════════════════════════

/// Avenida noturna: reta longa para o nitro, praça larga de drift, túnel na
/// diagonal e uma curva fechada no retorno. Plana.
pub const AVENIDA: Mapa = Mapa {
    id: "avenida",
    nome: "Avenida Central",
    local: "cidade fictícia",
    descricao: "Reta de 170 m para o nitro, praça de drift e curva fechada.",
    cenario: Cenario::Avenida,
    voltas: 2,
    tempo_limite: 150.0,
    dificuldade: 2,
    controles: &[
        Ponto::new(0.0, 0.0),
        Ponto::new(0.0, 120.0),
        Ponto::new(0.0, 218.0),
        Ponto::new(24.0, 268.0),
        Ponto::new(86.0, 292.0),
        Ponto::new(152.0, 284.0),
        Ponto::new(206.0, 240.0),
        Ponto::new(232.0, 178.0),
        Ponto::new(238.0, 112.0),
        Ponto::new(230.0, 62.0),
        Ponto::new(206.0, 30.0),
        Ponto::new(172.0, 20.0),
        Ponto::new(138.0, 26.0),
        Ponto::new(70.0, 14.0),
        Ponto::new(6.0, -34.0),
    ],
    larguras: &[
        9.5, 9.5, 10.0, 12.0, 19.0, 17.0, 11.0, 10.0, 8.5, 8.5, 8.0, 8.0, 9.0, 11.0, 10.0,
    ],
    tuneis: &[7, 8, 9],
    relevo: &[],
    rampas: &[],
};

// ════════════════════════════════════════════════════════════
// BOAVISTA — a descida
// ════════════════════════════════════════════════════════════
//
// Referência: a descida do Alto da Boavista, no Rio. O que a define não é o
// traçado exato (não há como copiar uma via real e continuar fechando uma volta)
// e sim o **caráter**: sai de cima da serra, desce por curvas encadeadas com a
// cidade aparecendo embaixo, atravessa um túnel de pedra e volta pela encosta.
//
// O traçado tem três trechos distintos, e cada um pede uma coisa do jogador:
//
//   1. **a descida** (0 a ~45 %): curvas de raio médio encadeadas, sempre para o
//      mesmo lado, com o carro ganhando velocidade sozinho pela gravidade;
//   2. **o túnel e a cidade** (~45 a 62 %): reta no fundo do vale, onde o nitro
//      serve, terminando numa curva fechada de verdade;
//   3. **a subida** (~62 a 100 %): a volta pela encosta, estreita, com grampos
//      apertados — é onde o nitro faz falta e onde quem gastou tudo na descida
//      perde a corrida.
//
// O relevo desce 74 m e volta. A subida é mais curta que a descida de propósito:
// 74 m em 560 m de descida dá 13 % de inclinação, e 74 m em 380 m de subida dá
// 19 % — arcade, mas ainda dirigível, e é o que faz a volta fechar.
/// A descida da serra: 74 m de desnível, curvas encadeadas, túnel de pedra e a
/// cidade vista de cima.
pub const BOAVISTA: Mapa = Mapa {
    id: "boavista",
    nome: "Descida da Boavista",
    local: "Rio de Janeiro",
    descricao: "Descida de serra: 74 m de queda, túnel de pedra e subida estreita.",
    cenario: Cenario::Descida,
    voltas: 2,
    tempo_limite: 185.0,
    dificuldade: 4,
    controles: &[
        // ── alto da serra: a largada é no topo ──────────────────
        Ponto::new(0.0, 0.0),
        Ponto::new(-14.0, 78.0),
        // ── descida: curvas encadeadas para a direita ──────────
        Ponto::new(-62.0, 130.0),
        Ponto::new(-142.0, 152.0),
        Ponto::new(-214.0, 128.0),
        Ponto::new(-262.0, 74.0),
        Ponto::new(-282.0, 6.0),
        // ── o vale: reta longa e o túnel ───────────────────────
        Ponto::new(-268.0, -68.0),
        Ponto::new(-222.0, -128.0),
        Ponto::new(-152.0, -168.0),
        // ── curva fechada no fundo ─────────────────────────────
        Ponto::new(-72.0, -186.0),
        Ponto::new(-6.0, -164.0),
        // ── subida: grampos na encosta ─────────────────────────
        Ponto::new(30.0, -108.0),
        Ponto::new(34.0, -44.0),
        Ponto::new(68.0, 6.0),
        Ponto::new(108.0, 44.0),
        Ponto::new(116.0, 96.0),
        Ponto::new(76.0, 92.0),
        Ponto::new(44.0, 52.0),
    ],
    larguras: &[
        // Largada no topo: larga, para caber a grade.
        11.0, 10.0, 9.0, 9.0, 8.5, 8.0, 7.5, 8.0, 9.0, 10.5, 12.0, 10.0, 8.0, 7.0, 7.0, 7.5,
        8.0, 8.5, 9.0,
    ],
    // O túnel de pedra corta o miolo do vale. São os trechos 8 e 9.
    tuneis: &[8, 9],
    // Perfil periódico: a fração 1,0 casa com a 0,0.
    relevo: &[
        (0.00, 74.0),
        (0.08, 68.0),
        (0.18, 56.0),
        (0.28, 40.0),
        (0.38, 24.0),
        (0.46, 12.0),
        (0.54, 4.0),
        (0.62, 0.0),
        (0.70, 3.0),
        (0.78, 14.0),
        (0.86, 32.0),
        (0.93, 52.0),
        (0.97, 66.0),
        (1.00, 74.0),
    ],
    rampas: &[],
};

// ════════════════════════════════════════════════════════════
// CIRCUITO — o mapa das rampas, com o dobro do tamanho
// ════════════════════════════════════════════════════════════
//
// Pedido do dono: "mapa com rampas de salto e com o dobro do tamanho".
//
// Dobro de quê? Da avenida, que é o mapa padrão e a referência de "uma volta
// normal": 962 m × 2 ≈ 1 900 m. O traçado abaixo foi medido, não estimado — a
// primeira versão fechava em 1 537 m e foi esticada até passar de 1 900.
//
// O desenho tem três partes, e cada uma existe por um motivo:
//
//   1. **a reta de largada** (~400 m): a mais longa do jogo. É onde o nitro
//      faz diferença e onde os três adversários se espalham antes da primeira
//      curva — numa pista curta eles ficam todos juntos e a corrida vira
//      empurra-empurra;
//   2. **as três rampas**, na segunda metade. Duas alinhadas com o eixo da
//      pista (salto limpo, ~9 m de altura) e uma em curva fechada (salto
//      torto, cai de lado) — a segunda ensina que decolar não é sempre bom;
//   3. **o hairpin** antes da reta final, de raio pequeno: quem gastou o nitro
//      nos saltos chega aqui sem nada e perde a posição.
//
// As rampas são `Rampa` declarada, não relevo: ver o comentário do tipo. A
// inclinação de cada uma é `altura / comprimento`, e é ela que define a
// velocidade vertical na decolagem.
/// O circuito de teste com três rampas de salto: 1,9 km, o dobro da avenida.
pub const CIRCUITO: Mapa = Mapa {
    id: "circuito",
    nome: "Circuito das Rampas",
    local: "Proving ground",
    descricao: "Duas vezes a avenida, reta de 400 m e três rampas: duas limpas, uma em curva.",
    cenario: Cenario::Circuito,
    voltas: 2,
    tempo_limite: 260.0,
    dificuldade: 5,
    controles: &[
        // ── reta de largada: 400 m ─────────────────────────────
        Ponto::new(0.0, 0.0),
        Ponto::new(0.0, 200.0),
        Ponto::new(0.0, 400.0),
        // ── curva 1: ampla, para a direita ─────────────────────
        Ponto::new(40.0, 520.0),
        Ponto::new(150.0, 590.0),
        Ponto::new(300.0, 610.0),
        // ── curva 2: a segunda metade do arco ──────────────────
        Ponto::new(420.0, 550.0),
        Ponto::new(480.0, 430.0),
        // ── reta de trás: 170 m, onde ficam as rampas 1 e 2 ────
        Ponto::new(480.0, 260.0),
        Ponto::new(470.0, 150.0),
        // ── curva 3: fechada à esquerda (a rampa 3) ─────────────
        Ponto::new(400.0, 60.0),
        Ponto::new(300.0, 20.0),
        // ── hairpin: raio pequeno, o castigo de quem gastou tudo ─
        Ponto::new(190.0, 30.0),
        Ponto::new(120.0, 90.0),
        // ── volta para a reta ──────────────────────────────────
        Ponto::new(60.0, 170.0),
        Ponto::new(25.0, 90.0),
    ],
    larguras: &[
        // Largada larga, para a grade caber; depois estreita nas curvas e abre
        // na praça de drift (a curva 2, de 20 m de meia-largura).
        13.0, 13.0, 13.0, 12.0, 11.0, 11.0, 11.0, 10.0, 10.0, 10.0, 12.0, 20.0, 12.0, 10.0,
        11.0, 12.0,
    ],
    tuneis: &[],
    relevo: &[],
    // Três rampas. As duas primeiras ficam na reta de trás (a fração 0,5 a 0,6
    // da volta, depois de as curvas 1 e 2 caberem nos primeiros 45 %); a
    // terceira cai dentro da curva 3, de propósito.
    //
    // `comprimento` entre 26 e 34 m: mais curto que isso e a inclinação passa
    // de 15 %, o carro perde velocidade subindo e o salto vira um degrau.
    rampas: &[
        // Rampa 1 na saída da curva 2 (raio ~150 m): decola em curva, cai em
        // ~100 m. É a que ensina que o salto não precisa de reta.
        Rampa { fracao: 0.50, comprimento: 30.0, altura: 3.4 },
        // Rampa 2 no meio da reta de trás (raio > 500 m): a limpa, a que dá
        // para encarar com nitro.
        Rampa { fracao: 0.60, comprimento: 26.0, altura: 3.0 },
        // Rampa 3 dentro da curva 3 (raio ~70 m): a torta. Decolar aqui joga o
        // carro para a grama se ele entrar rápido demais.
        Rampa { fracao: 0.682, comprimento: 30.0, altura: 3.6 },
    ],
};

// ════════════════════════════════════════════════════════════
// PORTO — a orla
// ════════════════════════════════════════════════════════════
//
// O mapa de curvas de 90°: um cais com quatro cantos retos, contêineres
// empilhados nas laterais e o mar do lado de fora. Onde o circuito é rápido e
// reto, aqui a pista é uma sucessão de conversões — e conversão de 90° é o que
// castiga quem entra acelerando.
//
// 1 240 m, três voltas curtas: é o mapa mais "técnico" dos cinco, e o único em
// que a praça de drift é um canto do cais em vez de um trecho de reta.
/// Porto Noturno: cais estreito, hairpin de raio 22 m e três voltas curtas.
pub const PORTO: Mapa = Mapa {
    id: "porto",
    nome: "Porto Noturno",
    local: "Cais",
    descricao: "Quatro cantos de 90°, contêineres e o mar: curvas, não velocidade.",
    cenario: Cenario::Porto,
    voltas: 3,
    tempo_limite: 210.0,
    dificuldade: 3,
    controles: &[
        // ── reta do cais ───────────────────────────────────────
        Ponto::new(0.0, 0.0),
        Ponto::new(0.0, 170.0),
        // ── canto 1: 90° para a direita ────────────────────────
        Ponto::new(45.0, 220.0),
        Ponto::new(200.0, 235.0),
        // ── canto 2: 90° e desce pelo lado do mar ──────────────
        Ponto::new(255.0, 190.0),
        Ponto::new(265.0, 130.0),
        // ── hairpin: 180° em torno de um contêiner ─────────────
        // Os três pontos são o que dá raio ~22 m: é a curva que a
        // `cada_mapa_constroi_uma_pista_com_comprimento_utilizavel` exige, e a
        // que faz o porto ser "técnico" em vez de só quatro cantos.
        Ponto::new(240.0, 85.0),
        Ponto::new(196.0, 78.0),
        Ponto::new(160.0, 118.0),
        // ── volta subindo pelo meio do cais ────────────────────
        Ponto::new(140.0, 170.0),
        Ponto::new(95.0, 195.0),
        Ponto::new(35.0, 150.0),
        Ponto::new(0.0, 80.0),
    ],
    larguras: &[
        13.0, 11.0, 10.0, 10.0, 10.0, 9.0, 9.0, 9.0, 10.0, 11.0, 19.0, 11.0, 12.0,
    ],
    tuneis: &[],
    relevo: &[],
    rampas: &[],
};

// ════════════════════════════════════════════════════════════
// SALAR — a planície de sal
// ════════════════════════════════════════════════════════════
//
// O oposto do porto: pista larga, curvas de raio enorme e uma rampa no meio de
// uma reta de 500 m. É o mapa do nitro — quase não há curva para frear, então
// a corrida é decidida por quem administra a recarga e por quem aceita o salto.
//
// Referência de *forma*: os salares do Atacama, planície branca com montanhas
// ao fundo. Nada é copiado: o traçado é invenção e o cenário é aritmética.
/// Salar: 2 188 m de planície branca, pista larga e **uma** rampa na reta de 500 m.
pub const SALAR: Mapa = Mapa {
    id: "salar",
    nome: "Salar",
    local: "Planície de sal",
    descricao: "Pista larga e reta de 500 m com uma rampa: o mapa do nitro.",
    cenario: Cenario::Salar,
    voltas: 2,
    tempo_limite: 230.0,
    dificuldade: 2,
    controles: &[
        Ponto::new(0.0, 0.0),
        Ponto::new(0.0, 250.0),
        Ponto::new(0.0, 500.0),
        Ponto::new(80.0, 640.0),
        Ponto::new(260.0, 700.0),
        Ponto::new(440.0, 640.0),
        Ponto::new(520.0, 440.0),
        Ponto::new(520.0, 200.0),
        Ponto::new(440.0, 20.0),
        Ponto::new(240.0, -30.0),
        Ponto::new(90.0, -60.0),
    ],
    larguras: &[16.0, 16.0, 16.0, 15.0, 14.0, 14.0, 14.0, 15.0, 16.0, 16.0, 16.0],
    tuneis: &[],
    relevo: &[],
    // Na reta de 500 m, onde o carro chega a 62 m/s: o salto mais alto do jogo.
    rampas: &[Rampa { fracao: 0.15, comprimento: 34.0, altura: 3.2 }],
};

/// Todos os mapas, na ordem em que aparecem no jogo.
pub const TODOS: &[&Mapa] = &[&AVENIDA, &BOAVISTA, &CIRCUITO, &PORTO, &SALAR];

/// O mapa padrão.
pub const PADRAO: &Mapa = &AVENIDA;

/// Procura um mapa pela identidade. Aceita prefixo único ("boav" → boavista).
pub fn por_id(id: &str) -> Option<&'static Mapa> {
    let alvo = id.trim().to_ascii_lowercase();
    if alvo.is_empty() {
        return None;
    }
    // Igualdade primeiro: `avenida` não pode casar com nada além dela mesma.
    if let Some(m) = TODOS.iter().find(|m| m.id == alvo) {
        return Some(m);
    }
    // Prefixo só se for único, senão "a" escolheria um mapa por sorte de ordem.
    let mut achados = TODOS.iter().filter(|m| m.id.starts_with(&alvo));
    let primeiro = achados.next()?;
    if achados.next().is_some() {
        return None;
    }
    Some(primeiro)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_circuito_tem_o_dobro_do_tamanho_da_avenida() {
        // O pedido foi literal: "mapa com rampas de salto e com o dobro do
        // tamanho". A avenida é a referência de "uma volta normal" — 962 m —,
        // então o dobro é ~1 924 m. Medido: 2 081 m.
        let dobro = AVENIDA.pista().comprimento() * 2.0;
        let circuito = CIRCUITO.pista().comprimento();
        assert!(
            circuito >= dobro,
            "o circuito tem {circuito:.0} m; o dobro da avenida é {dobro:.0} m"
        );
        assert!(
            circuito <= dobro * 1.35,
            "o circuito tem {circuito:.0} m — passou de 35 % acima do dobro, e o \
             nome 'o dobro' deixa de valer"
        );
    }

    #[test]
    fn o_circuito_tem_as_tres_rampas_e_elas_cabem_na_volta() {
        let pista = CIRCUITO.pista();
        assert_eq!(pista.rampas().len(), 3, "o mapa das rampas tem de ter rampas");
        let total = pista.comprimento();
        for (i, r) in pista.rampas().iter().enumerate() {
            assert!(r.inicio > 0.0 && r.topo < total, "rampa {i} fora da volta");
            assert!(
                r.altura >= 2.0 && r.altura <= 6.0,
                "rampa {i} com {:.1} m de altura: ou é um degrau ou é um morro",
                r.altura
            );
            let inclinacao = r.inclinacao();
            assert!(
                inclinacao > 0.06 && inclinacao < 0.25,
                "rampa {i}: inclinação {:.0}% — abaixo de 6% o carro não decola, \
                 acima de 25% ele perde velocidade subindo",
                inclinacao * 100.0
            );
        }
        // Não podem se sobrepor: o carro voa ~100 m, e duas rampas coladas
        // fariam ele pousar em cima da segunda.
        let mut anterior = 0.0;
        for r in pista.rampas() {
            assert!(
                r.inicio - anterior > 120.0,
                "rampas a {:.0} m uma da outra; o voo é maior que isso",
                r.inicio - anterior
            );
            anterior = r.topo;
        }
    }

    #[test]
    fn rampa_declarada_vira_degrau_no_asfalto() {
        // A rampa tem de estar **na geometria**, não só na física: um salto
        // invisível é pior que nenhum salto.
        let pista = CIRCUITO.pista();
        let r = pista.rampas()[1];
        let pe = pista.altura_em(r.inicio - 8.0);
        let meio = pista.altura_em((r.inicio + r.topo) * 0.5);
        let topo = pista.altura_em(r.topo - 1.0);
        assert!(meio > pe + 0.5, "o meio da rampa ({meio:.2}) não subiu do pé ({pe:.2})");
        assert!(
            topo > meio,
            "o topo ({topo:.2}) não está acima do meio ({meio:.2})"
        );
        // E depois do topo o piso volta ao nível do pé: é o corte que faz o
        // carro sair do chão.
        let depois = pista.altura_em(r.topo + 6.0);
        assert!(
            depois < topo - 1.5,
            "depois do topo o piso devia cair; topo {topo:.2}, depois {depois:.2}"
        );
    }

    #[test]
    fn todo_mapa_tem_largura_por_ponto_de_controle() {
        for mapa in TODOS {
            assert!(
                mapa.controles.len() >= 4,
                "{}: um laço fechado precisa de 4 pontos",
                mapa.id
            );
            assert_eq!(
                mapa.larguras.len(),
                mapa.controles.len(),
                "{}: uma largura por ponto de controle",
                mapa.id
            );
        }
    }

    #[test]
    fn o_relevo_e_periodico() {
        // Sem isto o carro leva um degrau na linha de chegada: a última amostra
        // fica na altura do fim do perfil e a primeira na do começo.
        for mapa in TODOS {
            if mapa.relevo.is_empty() {
                continue;
            }
            let inicio = mapa.altura_em(0.0);
            let fim = mapa.altura_em(1.0);
            assert!(
                (inicio - fim).abs() < 0.01,
                "{}: relevo não fecha ({inicio} contra {fim})",
                mapa.id
            );
        }
    }

    #[test]
    fn o_relevo_e_ordenado_e_comeca_em_zero() {
        for mapa in TODOS {
            if mapa.relevo.is_empty() {
                continue;
            }
            assert_eq!(mapa.relevo[0].0, 0.0, "{}: o perfil tem de começar em 0", mapa.id);
            for par in mapa.relevo.windows(2) {
                assert!(
                    par[1].0 > par[0].0,
                    "{}: frações fora de ordem ({} depois de {})",
                    mapa.id,
                    par[1].0,
                    par[0].0
                );
            }
            let ultimo = mapa.relevo.last().unwrap();
            assert!((ultimo.0 - 1.0).abs() < 1e-6, "{}: o perfil tem de terminar em 1", mapa.id);
        }
    }

    #[test]
    fn a_boavista_desce_e_sobe() {
        let topo = BOAVISTA.altura_em(0.0);
        let fundo = BOAVISTA.altura_em(0.62);
        assert!(fundo < 1.0, "o fundo do vale devia estar no nível do mar: {fundo}");
        assert!(topo > 60.0, "o alto devia passar de 60 m: {topo}");
        assert!(
            BOAVISTA.desnivel() > 60.0,
            "o desnível medido é {:.1} m",
            BOAVISTA.desnivel()
        );
    }

    #[test]
    fn a_avenida_e_plana() {
        // Um perfil vazio tem de devolver zero em qualquer ponto: se devolvesse
        // outra coisa, a pista plana ganharia inclinação invisível e o carro
        // aceleraria sozinho.
        assert_eq!(AVENIDA.altura_em(0.0), 0.0);
        assert_eq!(AVENIDA.altura_em(0.37), 0.0);
        assert_eq!(AVENIDA.altura_em(1.0), 0.0);
        assert_eq!(AVENIDA.desnivel(), 0.0);
    }

    #[test]
    fn a_altura_aceita_fracao_fora_de_zero_a_um() {
        // O `s` da primeira amostra da volta pode chegar negativo; normalizar é o
        // que impede o carro de cair para o nível do mar no primeiro metro.
        let a = BOAVISTA.altura_em(0.25);
        assert!((BOAVISTA.altura_em(1.25) - a).abs() < 1e-3);
        assert!((BOAVISTA.altura_em(-0.75) - a).abs() < 1e-3);
    }

    #[test]
    fn por_id_aceita_nome_exato_e_prefixo_unico() {
        assert_eq!(por_id("boavista").map(|m| m.id), Some("boavista"));
        assert_eq!(por_id("BOAV").map(|m| m.id), Some("boavista"));
        assert_eq!(por_id("avenida").map(|m| m.id), Some("avenida"));
        assert!(por_id("").is_none());
        assert!(por_id("zzz").is_none());
    }

    #[test]
    fn cada_mapa_constroi_uma_pista_com_comprimento_utilizavel() {
        for mapa in TODOS {
            let pista = mapa.pista();
            assert!(
                pista.comprimento() > 400.0,
                "{}: {:.0} m é curto demais para duas voltas",
                mapa.id,
                pista.comprimento()
            );
            // `raio_minimo()` olha três amostras (~6 m) e é dominado pela
            // junção entre segmentos da Catmull-Rom, não pela curva que o
            // jogador sente — a `avenida` mede 4,2 m ali desde a primeira
            // versão e sempre foi dirigível. O que se exige aqui é só que o
            // traçado não tenha uma quina impossível.
            assert!(
                pista.raio_minimo() > 3.0,
                "{}: raio mínimo {:.1} m — quina impossível de contornar",
                mapa.id,
                pista.raio_minimo()
            );
            // E a curva que importa: 45 m de varredura com mudança de rumo
            // suficiente para exigir largar o acelerador.
            let mut mais_fechada = 0.0f32;
            let mut s = 0.0;
            while s < pista.comprimento() {
                mais_fechada = mais_fechada.max(curvatura_em(&pista, s, 45.0));
                s += 5.0;
            }
            // O piso vem da **dificuldade declarada**, e não de uma lista de
            // exceções: mapa de fluxo (1–2) pode ser rápido e largo, mapa que
            // pede frear (3–5) tem de ter uma curva que obrigue a tirar o pé.
            //
            // Sem isso o `salar` — que promete "pista larga e reta de 500 m" —
            // precisaria de um hairpin, contradizendo a própria descrição. E
            // uma lista de "mapas que não contam" envelheceria: o próximo mapa
            // fácil entraria sem ninguém saber por quê.
            let piso = if mapa.dificuldade >= 3 { 1.2 } else { 0.4 };
            assert!(
                mais_fechada > piso,
                "{} (dificuldade {}): a curva mais fechada muda só {:.2} rad em 45 m (piso {piso})",
                mapa.id,
                mapa.dificuldade,
                mais_fechada
            );
        }
    }

    /// Curvatura acumulada numa varredura de `janela` metros a partir de `s`.
    fn curvatura_em(pista: &Pista, s: f32, janela: f32) -> f32 {
        let a = pista.direcao_em(s);
        let b = pista.direcao_em(s + janela);
        a.ponto(b).clamp(-1.0, 1.0).acos()
    }
}
