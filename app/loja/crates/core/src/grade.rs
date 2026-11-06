//! Geometria da grade de cartões.
//!
//! # Por que isto é do núcleo, e não da tela
//!
//! Porque **navegar e desenhar têm de concordar**. Num teste manual, este
//! projeto já teve um bug exato assim no launcher: a navegação supunha 4
//! colunas fixas enquanto a tela calculava as colunas pelo espaço disponível. O
//! resultado era a seta para baixo não ir a lugar nenhum e a seta para a direita
//! pular na diagonal — sem erro, sem aviso.
//!
//! A defesa é ter **uma** função que decide o número de colunas
//! (`grade_para`), usada pelo desenho e pela navegação. Ela é pura, mora aqui e
//! tem teste.
//!
//! # Por que não largura fixa de cartão
//!
//! A loja roda numa janela de 1180×760, mas o usuário redimensiona. Cartão de
//! largura fixa produziria uma coluna de 400 px de sobra à direita em tela
//! grande, ou corte em tela pequena. `grade_para` distribui o que existe.

/// Quantas colunas e quantos itens por página cabem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grade {
    pub colunas: usize,
    pub por_pagina: usize,
}

impl Grade {
    /// A linha (0-based) de um índice.
    pub fn linha(&self, indice: usize) -> usize {
        if self.colunas == 0 {
            return 0;
        }
        indice / self.colunas
    }

    /// A coluna (0-based) de um índice.
    pub fn coluna(&self, indice: usize) -> usize {
        if self.colunas == 0 {
            return 0;
        }
        indice % self.colunas
    }
}

/// Calcula a grade a partir do espaço disponível.
///
/// `espaco_x`/`espaco_y` são o que sobra depois da barra lateral e do
/// cabeçalho; `cartao_x`/`cartao_y` incluem o espaçamento entre cartões — quem
/// chama já somou a calha, porque só ele sabe a calha que desenhou.
///
/// Devolve pelo menos **1 coluna** e **1 linha**, mesmo com espaço insuficiente:
/// grade de zero colunas faria a navegação dividir por zero, e a tela ficaria
/// sem item nenhum em vez de mostrar um com corte.
pub fn grade_para(espaco_x: i32, espaco_y: i32, cartao_x: i32, cartao_y: i32) -> Grade {
    let colunas = if cartao_x <= 0 || espaco_x <= 0 {
        1
    } else {
        (espaco_x / cartao_x).max(1) as usize
    };
    let linhas = if cartao_y <= 0 || espaco_y <= 0 {
        1
    } else {
        (espaco_y / cartao_y).max(1) as usize
    };
    Grade {
        colunas,
        por_pagina: colunas * linhas,
    }
}

/// Para onde a seta leva, dado o índice atual, o total e a grade.
///
/// Devolve `None` quando não há para onde ir — e isso é informação, não erro: a
/// tela usa `None` para não redesenhar à toa, e o teste usa para provar que a
/// seta não pula na diagonal.
///
/// ## As regras, e por que cada uma
///
/// | tecla | regra | por quê |
/// | --- | --- | --- |
/// | ← | recua 1, parando no começo da linha | pular para o fim da linha anterior confunde: a linha muda sem o usuário pedir |
/// | → | avança 1, parando no fim da linha | idem, e o último item da linha pode não existir |
/// | ↑ | recua uma **linha** inteira | em grade, "cima" é a linha de cima, não o item anterior |
/// | ↓ | avança uma linha inteira | idem |
///
/// O `↓` é o caso que quebrou no launcher: recuar 1 em vez de `colunas` faz a
/// seta andar na horizontal com o nome de vertical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passo {
    Esquerda,
    Direita,
    Cima,
    Baixo,
}

/// O próximo índice, ou `None` se a seta não leva a lugar nenhum.
pub fn mover(atual: usize, total: usize, grade: Grade, passo: Passo) -> Option<usize> {
    if total == 0 {
        return None;
    }
    let cols = grade.colunas.max(1);
    let na_linha = atual % cols;
    let novo = match passo {
        Passo::Esquerda => {
            if na_linha == 0 {
                return None;
            }
            atual - 1
        }
        Passo::Direita => {
            let prox = atual + 1;
            // `prox / cols == atual / cols` = continua na mesma linha. É isto
            // que impede a seta de "descer" ao passar do fim da linha.
            if prox >= total || prox / cols != atual / cols {
                return None;
            }
            prox
        }
        Passo::Cima => {
            if atual < cols {
                return None;
            }
            atual - cols
        }
        Passo::Baixo => {
            let prox = atual + cols;
            if prox >= total {
                return None;
            }
            prox
        }
    };
    if novo == atual {
        None
    } else {
        Some(novo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn largura_de_1180_com_barra_da_4_colunas() {
        // 1180 - 200 (barra) - 40 (margens) = 940; cartão de 215 + calha de 20.
        let g = grade_para(940, 600, 235, 250);
        assert_eq!(g.colunas, 4);
        assert_eq!(g.por_pagina, 8); // 4 x 2 linhas
    }

    #[test]
    fn janela_estreita_reduz_colunas_e_nao_zera() {
        let g = grade_para(400, 300, 235, 250);
        assert_eq!(g.colunas, 1, "tela estreita tem de mostrar 1 coluna, não 0");
        assert!(g.por_pagina >= 1);
    }

    #[test]
    fn espaco_insuficiente_devolve_grade_minima_em_vez_de_zero() {
        // Zero colunas faria `indice % cols` dividir por zero na navegação.
        let g = grade_para(10, 10, 500, 500);
        assert_eq!(g.colunas, 1);
        assert_eq!(g.por_pagina, 1);
        let g2 = grade_para(0, 0, 0, 0);
        assert_eq!(g2.colunas, 1);
    }

    #[test]
    fn o_baixo_desce_uma_linha_e_nao_um_item() {
        // O bug do launcher: `↓` recuando 1 anda na horizontal com nome de
        // vertical. Em grade de 4, de 0 tem de ir a 4.
        let g = Grade { colunas: 4, por_pagina: 8 };
        assert_eq!(mover(0, 12, g, Passo::Baixo), Some(4));
        assert_eq!(mover(2, 12, g, Passo::Baixo), Some(6));
    }

    #[test]
    fn a_direita_para_no_fim_da_linha() {
        let g = Grade { colunas: 4, por_pagina: 8 };
        assert_eq!(mover(2, 12, g, Passo::Direita), Some(3));
        // Do fim da linha não vai para a próxima linha: isso seria "descer".
        assert_eq!(mover(3, 12, g, Passo::Direita), None);
    }

    #[test]
    fn a_esquerda_para_no_comeco_da_linha() {
        let g = Grade { colunas: 4, por_pagina: 8 };
        assert_eq!(mover(5, 12, g, Passo::Esquerda), Some(4));
        assert_eq!(mover(4, 12, g, Passo::Esquerda), None);
    }

    #[test]
    fn as_setas_verticais_nao_saem_da_lista() {
        let g = Grade { colunas: 4, por_pagina: 8 };
        assert_eq!(mover(1, 12, g, Passo::Cima), None);
        assert_eq!(mover(11, 12, g, Passo::Baixo), None, "passou do último");
        // 9 + 4 = 13 >= 12: para.
        assert_eq!(mover(9, 12, g, Passo::Baixo), None);
        assert_eq!(mover(8, 12, g, Passo::Baixo), None);
        // De 7 **existe** para onde ir: 7 + 4 = 11, e 11 está na lista. Escrevi
        // `None` aqui na primeira versão — a asserção é que estava errada, e o
        // teste reprovou o código por um defeito meu.
        assert_eq!(mover(7, 12, g, Passo::Baixo), Some(11));
    }

    #[test]
    fn lista_vazia_nao_move_e_nao_estoura() {
        let g = Grade { colunas: 3, por_pagina: 9 };
        for p in [Passo::Esquerda, Passo::Direita, Passo::Cima, Passo::Baixo] {
            assert_eq!(mover(0, 0, g, p), None);
        }
    }

    #[test]
    fn ultima_linha_incompleta_nao_pula_para_o_vazio() {
        // 10 itens em 4 colunas: a última linha tem 2. De 8, `→` vai a 9; de 9,
        // para — e `↓` de 6 vai a 9? Não: 6+4=10 não existe, então para.
        let g = Grade { colunas: 4, por_pagina: 8 };
        assert_eq!(mover(8, 10, g, Passo::Direita), Some(9));
        assert_eq!(mover(9, 10, g, Passo::Direita), None);
        assert_eq!(mover(6, 10, g, Passo::Baixo), None);
        assert_eq!(mover(5, 10, g, Passo::Baixo), Some(9));
    }

    #[test]
    fn linha_e_coluna_de_cada_indice() {
        let g = Grade { colunas: 3, por_pagina: 9 };
        assert_eq!((g.linha(0), g.coluna(0)), (0, 0));
        assert_eq!((g.linha(4), g.coluna(4)), (1, 1));
        assert_eq!((g.linha(8), g.coluna(8)), (2, 2));
    }

    #[test]
    fn toda_seta_leva_a_um_indice_valido() {
        // Propriedade, não exemplo: nenhuma tecla pode produzir índice fora da
        // lista, em nenhuma grade, com qualquer total.
        for total in 1..=17 {
            for cols in 1..=5 {
                let g = Grade { colunas: cols, por_pagina: cols * 3 };
                for atual in 0..total {
                    for p in [Passo::Esquerda, Passo::Direita, Passo::Cima, Passo::Baixo] {
                        if let Some(novo) = mover(atual, total, g, p) {
                            assert!(novo < total, "total={total} cols={cols} de {atual} para {novo}");
                            assert_ne!(novo, atual);
                        }
                    }
                }
            }
        }
    }
}
