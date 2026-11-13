//! O que cada coisa do mundo **parece**: modelo, altura e cor.
//!
//! # Por que isto é um módulo e não um `match` no laço de desenho
//!
//! São duas perguntas diferentes: "qual é o estado do mundo" (`game/`) e "com que
//! forma ele aparece" (aqui). Misturadas, a tabela de aparências ficava enterrada
//! no meio de 40 linhas de chamadas de desenho, e ninguém conseguia responder
//! "todos os tipos de inimigo têm modelo?" sem ler o laço inteiro.
//!
//! Aqui é **dado puro**, com teste: cada tipo de inimigo, cada classe e cada boss
//! tem de apontar para um modelo que existe no catálogo. Um tipo novo sem entrada
//! cai no `Grunt` em silêncio — e é o teste que impede o silêncio.
//!
//! # A horda sai agrupada
//!
//! [`agrupar`] transforma a lista de inimigos visíveis em **um lote por modelo**.
//! Uma chamada de desenho por inimigo custaria ~1,5 µs de CPU cada só para
//! submeter no Cortex-A35; com 60 inimigos vivos isso é 90 µs por quadro que não
//! desenham nada. Uma chamada por tipo são cinco.

use std::collections::BTreeMap;

use glam::Vec3;
use giochi_engine_3d::math3d::{Mat4, Quat, Transform3D, Vec3 as EngineVec3};

use crate::config::{BossKind, ClassId, EnemyKind};
use crate::game::state::GameState;

/// Como uma entidade aparece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aparencia {
    /// Nome do modelo no catálogo (`assets/modelos/<nome>.obj`).
    pub modelo: &'static str,
    /// Altura em metros — o que a jogabilidade ocupa na tela.
    pub altura: f32,
    /// Cor de tingimento multiplicada sobre a textura.
    pub cor: [f32; 4],
    /// Tamanho do volume de colisão, para o teste de coerência.
    pub volume: f32,
}

/// Aparência de cada classe de jogador (GDD §7).
///
/// A altura é a do **modelo**, não a de um cubo: 1,8 m é um humanoide adulto, e é
/// o que faz a escala do mapa (160 m) fazer sentido na tela.
pub fn da_classe(class: ClassId) -> Aparencia {
    match class {
        ClassId::Soldier => Aparencia { modelo: "soldado", altura: 1.85, cor: [1.0, 1.0, 1.0, 1.0], volume: 0.45 },
        ClassId::Warrior => Aparencia { modelo: "guerreiro", altura: 2.0, cor: [1.0, 0.96, 0.92, 1.0], volume: 0.5 },
        ClassId::Sniper => Aparencia { modelo: "sniper", altura: 1.85, cor: [0.94, 1.0, 0.96, 1.0], volume: 0.42 },
        ClassId::Medic => Aparencia { modelo: "medico", altura: 1.8, cor: [1.0, 1.0, 1.0, 1.0], volume: 0.45 },
    }
}

/// Aparência de cada tipo de inimigo (GDD §8).
///
/// # Por que a cor não é branca
///
/// Boss e Coronel saem **texturizados** (um por vez, são poucos), e aí o branco
/// é o neutro que mostra o arquivo. A horda comum sai **instanciada**, e o
/// caminho instanciado não amostra textura — só multiplica a cor. Com branco, os
/// cinco tipos de bug apareciam brancos: a mesma silhueta pintada de giz.
///
/// Estas são as cores de quitina que o lote usa. Elas não são um segundo
/// conjunto de tons "de mentira": são o marrom-ocre real do filme, e é o que faz
/// a horda ler como praga em vez de estátua.
pub fn do_inimigo(kind: EnemyKind) -> Aparencia {
    match kind {
        EnemyKind::Grunt => Aparencia { modelo: "grunt", altura: 1.65, cor: [0.66, 0.53, 0.36, 1.0], volume: 0.5 },
        EnemyKind::Tank => Aparencia { modelo: "tank", altura: 1.7, cor: [0.54, 0.49, 0.40, 1.0], volume: 0.9 },
        EnemyKind::Ranged => Aparencia { modelo: "ranged", altura: 1.95, cor: [0.42, 0.55, 0.58, 1.0], volume: 0.45 },
        EnemyKind::Swarm => Aparencia { modelo: "swarm", altura: 0.95, cor: [0.72, 0.56, 0.36, 1.0], volume: 0.4 },
        EnemyKind::Flying => Aparencia { modelo: "flying", altura: 1.55, cor: [0.62, 0.46, 0.52, 1.0], volume: 0.5 },
    }
}

/// Aparência de cada boss (GDD §9).
pub fn do_boss(kind: BossKind) -> Aparencia {
    match kind {
        BossKind::Tanker => Aparencia { modelo: "tanker", altura: 3.0, cor: [1.0, 1.0, 1.0, 1.0], volume: 1.4 },
        BossKind::Plasma => Aparencia { modelo: "plasma", altura: 2.8, cor: [1.0, 1.0, 1.0, 1.0], volume: 1.1 },
        BossKind::Brain => Aparencia { modelo: "brain", altura: 3.2, cor: [1.0, 1.0, 1.0, 1.0], volume: 1.6 },
        BossKind::Queen => Aparencia { modelo: "queen", altura: 3.4, cor: [1.0, 1.0, 1.0, 1.0], volume: 1.5 },
    }
}

/// O Coronel, com o tingimento que o distingue de um boss comum.
pub fn do_coronel() -> Aparencia {
    Aparencia { modelo: "coronel", altura: 2.6, cor: [1.0, 0.82, 0.9, 1.0], volume: 0.9 }
}

/// A torre (GDD §11). Escala com o HP máximo: uma torre "upgrade" fica maior.
pub fn da_torre(max_hp: f32) -> Aparencia {
    // 5 000 HP é a base (5,9 m de modelo ⇒ 1,0 de escala); 8 000 é o teto da loja.
    let escala = (max_hp / 5000.0).clamp(1.0, 1.6);
    Aparencia {
        modelo: "torre",
        altura: 5.9 * escala,
        cor: [1.0, 1.0, 1.0, 1.0],
        volume: 2.8 * escala,
    }
}

/// Como o inimigo `tipo` (do snapshot) aparece: modelo, altura, cor e giro.
///
/// Os bytes 9 e 10 são boss e Coronel — o protocolo os usa em vez de um id de
/// tipo porque eles não são "mais um inimigo": têm barra na HUD e não morrem por
/// dano comum.
pub fn do_byte_de_inimigo(tipo: u8, boss_kind: Option<BossKind>) -> Aparencia {
    match tipo {
        9 => boss_kind.map(do_boss).unwrap_or_else(|| do_boss(BossKind::Tanker)),
        10 => do_coronel(),
        outro => do_inimigo(GameState::enemy_kind_of_byte(outro)),
    }
}

/// Um lote de cópias do mesmo modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Lote {
    /// Nome do modelo.
    pub modelo: &'static str,
    /// Cor com que o lote inteiro é desenhado.
    pub cor: [f32; 4],
    /// Uma matriz por cópia.
    pub matrizes: Vec<Mat4>,
}

/// Uma cópia a desenhar: modelo, posição, altura e giro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Copia {
    /// Nome do modelo.
    pub modelo: &'static str,
    /// Base (o pé fica aqui).
    pub pos: Vec3,
    /// Altura em metros.
    pub altura: f32,
    /// Giro em torno de Y.
    pub giro: f32,
    /// Cor.
    pub cor: [f32; 4],
}

/// Altura medida de cada modelo, para a matriz de instância.
///
/// A matriz precisa da **escala** (`altura desejada / altura do arquivo`), e a
/// altura do arquivo quem sabe é o catálogo — que precisa de GPU. Por isso esta
/// tabela existe: as medidas vêm do gerador (`scripts/gerar-modelos.mjs`), e o
/// teste `as_alturas_da_tabela_batem_com_os_arquivos` compara com os OBJ reais,
/// então ela não pode envelhecer sozinha.
pub fn altura_do_arquivo(modelo: &str) -> f32 {
    match modelo {
        "soldado" => 1.931,
        "guerreiro" => 2.125,
        "sniper" => 1.926,
        "medico" => 1.883,
        "grunt" => 1.288,
        "tank" => 1.312,
        "ranged" => 2.154,
        "swarm" => 0.708,
        "flying" => 1.533,
        "tanker" => 2.551,
        "plasma" => 3.213,
        "brain" => 3.090,
        "queen" => 2.972,
        "coronel" => 2.648,
        "torre" => 5.900,
        "rocha" => 1.000,
        "espigao" => 3.000,
        "cratera" => 0.060,
        "mesa" => 1.000,
        "destroco" => 1.642,
        _ => 1.8,
    }
}

/// Agrupa as cópias por modelo, para uma chamada de desenho por modelo.
///
/// A ordem de saída é estável (`BTreeMap` por nome), então dois quadros com os
/// mesmos inimigos produzem a mesma sequência de desenhos — é o que permite
/// comparar quadro a quadro sem ruído de ordenação.
pub fn agrupar(copias: &[Copia]) -> Vec<Lote> {
    let mut por_modelo: BTreeMap<&'static str, Lote> = BTreeMap::new();
    for copia in copias {
        let escala = if altura_do_arquivo(copia.modelo) > 0.01 {
            copia.altura / altura_do_arquivo(copia.modelo)
        } else {
            1.0
        };
        // `Mat4` é o tipo da engine (o renderer recebe `&[Mat4]`); montar em glam
        // e converter exigiria a mesma conta duas vezes. `Transform3D` é T·R·S.
        let matriz = Transform3D::new(
            EngineVec3::from_glam(copia.pos),
            Quat::from_axis_angle(EngineVec3::UP, copia.giro),
            EngineVec3::new(escala, escala, escala),
        )
        .matrix();
        por_modelo
            .entry(copia.modelo)
            .or_insert_with(|| Lote {
                modelo: copia.modelo,
                cor: copia.cor,
                matrizes: Vec::new(),
            })
            .matrizes
            .push(matriz);
    }
    por_modelo.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os personagens, bosses e a torre (`engine_bridge::modelos::NOMES`).
    const MODELOS: [&str; 15] = [
        "soldado", "guerreiro", "sniper", "medico", "grunt", "tank", "ranged", "swarm", "flying",
        "tanker", "plasma", "brain", "queen", "coronel", "torre",
    ];

    /// O cenário (`engine_bridge::modelos::CENARIO`).
    const CENARIO: [&str; 5] = ["rocha", "espigao", "cratera", "mesa", "destroco"];

    const CLASSES: [ClassId; 4] = [ClassId::Soldier, ClassId::Warrior, ClassId::Sniper, ClassId::Medic];
    const INIMIGOS: [EnemyKind; 5] = [
        EnemyKind::Grunt,
        EnemyKind::Tank,
        EnemyKind::Ranged,
        EnemyKind::Swarm,
        EnemyKind::Flying,
    ];
    const BOSSES: [BossKind; 4] = [BossKind::Tanker, BossKind::Plasma, BossKind::Brain, BossKind::Queen];

    #[test]
    fn todo_tipo_aponta_para_um_modelo_que_existe() {
        let mut usados = Vec::new();
        for class in CLASSES {
            usados.push(da_classe(class).modelo);
        }
        for kind in INIMIGOS {
            usados.push(do_inimigo(kind).modelo);
        }
        for boss in BOSSES {
            usados.push(do_boss(boss).modelo);
        }
        usados.push(do_coronel().modelo);
        usados.push(da_torre(5000.0).modelo);

        for modelo in usados {
            assert!(MODELOS.contains(&modelo), "modelo '{modelo}' não está no catálogo");
        }
    }

    #[test]
    fn os_bosses_sao_maiores_que_os_inimigos_comuns() {
        let maior_inimigo = INIMIGOS
            .iter()
            .map(|k| do_inimigo(*k).altura)
            .fold(0.0_f32, f32::max);
        for boss in BOSSES {
            let altura = do_boss(boss).altura;
            assert!(
                altura > maior_inimigo,
                "{:?} tem {altura:.2} m e o maior inimigo comum {maior_inimigo:.2} m",
                boss
            );
        }
    }

    #[test]
    fn o_coronel_nao_e_confundivel_com_um_boss_de_onda() {
        // O GDD §10 faz do Coronel uma ameaça que atravessa ondas. Se ele tiver o
        // mesmo modelo de um boss, o jogador não sabe se pode respirar quando a
        // barra de boss some.
        for boss in BOSSES {
            assert_ne!(do_coronel().modelo, do_boss(boss).modelo);
        }
        // E a cor dele é distinta: o tingimento é o que o diferencia à distância.
        assert!(do_coronel().cor[2] > do_coronel().cor[0] || do_coronel().cor[1] < 0.95);
    }

    #[test]
    fn a_torre_cresce_com_o_upgrade_mas_tem_teto() {
        let base = da_torre(5000.0);
        let topo = da_torre(8000.0);
        assert!(topo.altura > base.altura, "{} × {}", topo.altura, base.altura);
        // Um HP absurdo (config quebrada) não pode virar uma torre de 50 m.
        assert!(da_torre(50_000.0).altura <= 5.9 * 1.6 + 0.01);
        // E zero HP não encolhe a torre até sumir.
        assert!(da_torre(0.0).altura >= 5.9);
    }

    #[test]
    fn o_volume_de_colisao_segue_a_silhueta_de_cada_inimigo() {
        // Uma faixa numérica arbitrária ("altura/volume entre 2 e 5") não mede
        // nada: o Ranged tem uma antena e o Tank é baixo e largo, e os dois são
        // legítimos. O que importa é a **ordem**: o Tank é o mais largo e o Swarm
        // o menor, e todos cabem entre 0,3 m e 1,1 m de raio.
        let volumes: Vec<(EnemyKind, f32)> = INIMIGOS.iter().map(|k| (*k, do_inimigo(*k).volume)).collect();
        let maior = volumes.iter().fold((EnemyKind::Grunt, 0.0_f32), |acc, (k, v)| {
            if *v > acc.1 { (*k, *v) } else { acc }
        });
        let menor = volumes.iter().fold((EnemyKind::Grunt, f32::INFINITY), |acc, (k, v)| {
            if *v < acc.1 { (*k, *v) } else { acc }
        });
        assert_eq!(maior.0, EnemyKind::Tank, "o mais largo tem de ser o Tank");
        assert_eq!(menor.0, EnemyKind::Swarm, "o menor tem de ser o Swarm");
        for (kind, volume) in volumes {
            assert!((0.3..=1.1).contains(&volume), "{kind:?}: volume {volume:.2} m");
        }
    }

    #[test]
    fn as_alturas_da_tabela_batem_com_os_arquivos() {
        // A tabela existe porque a matriz de instância precisa da escala sem GPU.
        // Uma tabela que ninguém confere é a que envelhece: este teste a compara
        // com os OBJ que o jogo carrega.
        let dir = crate::engine_bridge::modelos::achar_diretorio().expect("assets/");
        for nome in MODELOS.into_iter().chain(CENARIO) {
            let (vertices, _) = giochi_engine_3d::mesh::load_obj(&dir.join("modelos").join(format!("{nome}.obj")))
                .unwrap_or_else(|e| panic!("{nome}.obj: {e}"));
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for v in &vertices {
                min = min.min(v.position[1]);
                max = max.max(v.position[1]);
            }
            let medida = max - min;
            let tabela = altura_do_arquivo(nome);
            assert!(
                (medida - tabela).abs() < 0.02,
                "{nome}: arquivo mede {medida:.3} m e a tabela diz {tabela:.3} m — rode scripts/gerar-modelos.mjs e atualize a tabela"
            );
        }
    }

    #[test]
    fn agrupar_faz_um_lote_por_modelo_e_preserva_a_contagem() {
        let copias: Vec<Copia> = (0..10)
            .map(|i| Copia {
                modelo: if i % 2 == 0 { "grunt" } else { "tank" },
                pos: Vec3::new(i as f32, 0.0, 0.0),
                altura: 1.6,
                giro: 0.0,
                cor: [1.0, 1.0, 1.0, 1.0],
            })
            .collect();
        let lotes = agrupar(&copias);
        assert_eq!(lotes.len(), 2, "dois modelos, dois lotes");
        assert_eq!(lotes.iter().map(|l| l.matrizes.len()).sum::<usize>(), 10);
        // Ordem estável: `grunt` antes de `tank`, sempre.
        assert_eq!(lotes[0].modelo, "grunt");
        assert_eq!(lotes[1].modelo, "tank");
    }

    #[test]
    fn a_matriz_do_lote_aplica_a_escala_da_altura_pedida() {
        // Um Grunt pedido com 3,2 m (o dobro) tem de sair com escala 2× a altura
        // do arquivo — senão o modelo aparece sempre do mesmo tamanho e a altura
        // por tipo vira decoração.
        let copia = Copia {
            modelo: "grunt",
            pos: Vec3::ZERO,
            altura: altura_do_arquivo("grunt") * 2.0,
            giro: 0.0,
            cor: [1.0, 1.0, 1.0, 1.0],
        };
        let lote = &agrupar(&[copia])[0];
        // A escala é lida aplicando a matriz a um ponto unitário: y = 1 tem de
        // virar y = 2, e é exatamente o que o shader instanciado faz com o vértice.
        let ponto = lote.matrizes[0].transform_point(EngineVec3::new(0.0, 1.0, 0.0));
        assert!((ponto.y - 2.0).abs() < 1e-4, "y = {}", ponto.y);
    }

    #[test]
    fn um_tipo_desconhecido_cai_no_grunt_em_vez_de_sumir() {
        // Um byte novo no protocolo (tipo 7, por exemplo) não pode fazer o inimigo
        // aparecer do nada nem desaparecer: ele aparece como Grunt, e o teste fixa
        // essa degradação.
        assert_eq!(do_byte_de_inimigo(7, None).modelo, do_inimigo(EnemyKind::Grunt).modelo);
        // E o byte 9 sem kind conhecido vira o Tanker, não um Grunt gigante.
        assert_eq!(do_byte_de_inimigo(9, None).modelo, do_boss(BossKind::Tanker).modelo);
        assert_eq!(do_byte_de_inimigo(10, None).modelo, do_coronel().modelo);
    }
}
