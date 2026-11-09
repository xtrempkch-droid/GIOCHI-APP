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
};

/// Todos os mapas, na ordem em que aparecem no jogo.
pub const TODOS: [&Mapa; 2] = [&AVENIDA, &BOAVISTA];

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
            assert!(
                mais_fechada > 1.2,
                "{}: a curva mais fechada muda só {:.2} rad em 45 m",
                mapa.id,
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
