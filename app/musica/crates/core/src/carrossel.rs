//! Carrossel de capas: foco com volta e geometria de profundidade.
//!
//! Duas decisões que o app de exemplo errava:
//!
//! 1. **A volta existe.** No exemplo, o foco parava no fim da lista: chegar da
//!    última faixa à primeira exigia voltar tudo com o direcional. Aqui `mover`
//!    dá a volta, e a playlist tem a mesma sensação de um carrossel.
//! 2. **A posição é a distância mais curta no anel.** Com 5 faixas e foco na
//!    primeira, a última está a `-1` e não a `+4` — senão ela apareceria
//!    atravessando a tela por cima de todas as outras, como se o carrossel
//!    girasse ao contrário.
//!
//! A geometria também é pura: ela não conhece painel, fonte nem SDL. Quem
//! decide onde fica o centro é o layout; aqui só se responde "dada a posição
//! relativa, que tamanho e que opacidade".

/// Seleção num anel de `total` faixas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrossel {
    total: usize,
    foco: usize,
}

impl Carrossel {
    pub fn novo(total: usize) -> Self {
        Carrossel { total, foco: 0 }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn foco(&self) -> usize {
        self.foco
    }

    pub fn esta_vazio(&self) -> bool {
        self.total == 0
    }

    /// Reajusta o total, mantendo o foco dentro da lista.
    ///
    /// `clamp` e não `wrap`: quando a pasta muda, o usuário quer continuar perto
    /// de onde estava. Dar a volta ao contrário aqui levaria o foco para o
    /// começo da lista depois de remover uma música do fim.
    pub fn ajustar(&mut self, total: usize) {
        self.total = total;
        if total == 0 {
            self.foco = 0;
        } else if self.foco >= total {
            self.foco = total - 1;
        }
    }

    /// Move o foco, com volta. Devolve `false` quando não há o que mover.
    pub fn mover(&mut self, passo: isize) -> bool {
        if self.total == 0 || passo == 0 {
            return false;
        }
        let n = self.total as isize;
        self.foco = (((self.foco as isize + passo) % n + n) % n) as usize;
        true
    }

    /// Coloca o foco num índice qualquer (toque ou pedido direto).
    pub fn ir_para(&mut self, indice: usize) -> bool {
        if indice >= self.total || indice == self.foco {
            return false;
        }
        self.foco = indice;
        true
    }

    /// Distância assinada até `indice`, pelo caminho mais curto do anel.
    ///
    /// Empate (total par, meia volta exata) vai para o lado positivo, e isso é
    /// escolha: a alternativa seria uma função não determinística, e o carrossel
    /// mudaria de desenho entre dois quadros com a mesma entrada.
    pub fn posicao(&self, indice: usize) -> i32 {
        if self.total == 0 {
            return 0;
        }
        let n = self.total as i32;
        let d = (indice as i32 - self.foco as i32).rem_euclid(n);
        if d * 2 > n {
            d - n
        } else {
            d
        }
    }

    /// Faixas visíveis com a posição relativa, da esquerda para a direita.
    ///
    /// `raio` é quantas capas cabem de cada lado do foco. A lista sai ordenada
    /// pela posição, que é a ordem de desenho: quem está mais longe é desenhado
    /// primeiro e fica atrás.
    pub fn visiveis(&self, raio: usize) -> Vec<(usize, i32)> {
        let mut lista: Vec<(usize, i32)> = (0..self.total)
            .map(|i| (i, self.posicao(i)))
            .filter(|(_, p)| p.unsigned_abs() as usize <= raio)
            .collect();
        lista.sort_by_key(|(_, p)| *p);
        lista
    }
}

/// Onde desenhar uma capa, dada a posição relativa ao foco.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometria {
    pub x: i32,
    pub escala: f32,
    pub alfa: u8,
}

/// Escala de cada passo de distância.
const PASSO_ESCALA: f32 = 0.16;
/// Escala mínima: capa que encolhe até sumir não informa que existe.
const ESCALA_MINIMA: f32 = 0.42;
/// Opacidade perdida por passo.
const PASSO_ALFA: f32 = 46.0;
/// Opacidade mínima: capa quase invisível parece ausência de faixa.
const ALFA_MINIMO: u8 = 70;

/// Geometria de uma capa na posição `posicao` (0 = foco).
///
/// `passo` é a distância horizontal entre o foco e o vizinho imediato. O
/// deslocamento encolhe com a distância (`fator`), o que produz o efeito de
/// perspectiva: as capas de trás ficam mais juntas, como num corredor. Sem
/// isso, um carrossel de 5 faixas fica com a mesma separação em todas e parece
/// uma fileira, não um anel.
pub fn geometria(posicao: i32, centro_x: i32, passo: i32) -> Geometria {
    let d = posicao.unsigned_abs() as f32;
    let escala = (1.0 - PASSO_ESCALA * d).max(ESCALA_MINIMA);
    let alfa = (255.0 - PASSO_ALFA * d).max(ALFA_MINIMO as f32) as u8;
    let fator = 1.0 - 0.12 * d;
    Geometria {
        x: centro_x + (passo as f32 * posicao as f32 * fator).round() as i32,
        escala,
        alfa,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── foco ────────────────────────────────────────────────

    #[test]
    fn carrossel_vazio_nao_move_nada() {
        let mut c = Carrossel::novo(0);
        assert!(!c.mover(1));
        assert!(!c.mover(-1));
        assert_eq!(c.foco(), 0);
        assert!(c.esta_vazio());
        assert!(c.visiveis(3).is_empty());
    }

    #[test]
    fn mover_da_a_volta_nas_duas_pontas() {
        let mut c = Carrossel::novo(5);
        assert!(c.mover(-1));
        assert_eq!(c.foco(), 4, "esquerda no primeiro vai para o último");
        assert!(c.mover(1));
        assert_eq!(c.foco(), 0, "direita no último volta ao primeiro");
    }

    #[test]
    fn mover_atravessa_varias_posicoes_sem_sair_do_intervalo() {
        let mut c = Carrossel::novo(3);
        for passo in [1, 1, 1, -1, -4, 7, -1] {
            c.mover(passo);
            assert!(c.foco() < 3, "foco {} fora da lista", c.foco());
        }
        // 7 passos num anel de 3 é 1 passo.
        let mut a = Carrossel::novo(3);
        let mut b = Carrossel::novo(3);
        a.mover(7);
        b.mover(1);
        assert_eq!(a.foco(), b.foco());
    }

    #[test]
    fn mover_zero_nao_muda_e_diz_que_nao_moveu() {
        let mut c = Carrossel::novo(4);
        c.mover(2);
        let antes = c.foco();
        assert!(!c.mover(0));
        assert_eq!(c.foco(), antes);
    }

    #[test]
    fn ir_para_recusa_indice_fora_da_lista() {
        let mut c = Carrossel::novo(3);
        assert!(c.ir_para(2));
        assert!(!c.ir_para(3));
        assert!(!c.ir_para(2), "ir para o mesmo lugar não muda nada");
        assert_eq!(c.foco(), 2);
    }

    #[test]
    fn ajustar_encolhe_o_foco_para_caber() {
        let mut c = Carrossel::novo(5);
        c.ir_para(4);
        c.ajustar(2);
        assert_eq!(c.foco(), 1, "o foco cai para o último índice válido");
        c.ajustar(0);
        assert_eq!(c.foco(), 0);
    }

    // ── posição no anel ─────────────────────────────────────

    #[test]
    fn ultimo_fica_um_passo_a_esquerda_do_primeiro() {
        let c = Carrossel::novo(5);
        assert_eq!(c.posicao(0), 0);
        assert_eq!(c.posicao(1), 1);
        assert_eq!(c.posicao(4), -1, "o caminho curto é por trás, não por cima");
    }

    #[test]
    fn empate_vai_para_o_lado_positivo_e_e_deterministico() {
        // Total par: a faixa oposta está a meia volta exata.
        let c = Carrossel::novo(4);
        assert_eq!(c.posicao(2), 2);
        assert_eq!(c.posicao(3), -1);
        // Chamar duas vezes tem de dar o mesmo: o desenho não pode tremer.
        assert_eq!(c.posicao(2), c.posicao(2));
    }

    #[test]
    fn carrossel_de_uma_faixa_tem_posicao_zero() {
        let c = Carrossel::novo(1);
        assert_eq!(c.posicao(0), 0);
    }

    #[test]
    fn posicoes_cobrem_o_anel_sem_repetir() {
        for total in 1..=9 {
            let mut c = Carrossel::novo(total);
            c.mover(2);
            let mut pos: Vec<i32> = (0..total).map(|i| c.posicao(i)).collect();
            pos.sort();
            let esperado: Vec<i32> = match total {
                1 => vec![0],
                2 => vec![0, 1],
                n => {
                    let metade = n as i32 / 2;
                    let mut v: Vec<i32> = (-metade..=metade).collect();
                    if n % 2 == 0 {
                        v.remove(0);
                    }
                    v
                }
            };
            assert_eq!(pos, esperado, "total {total}");
        }
    }

    #[test]
    fn visiveis_respeita_o_raio_e_sai_em_ordem_de_desenho() {
        let c = Carrossel::novo(7);
        let v = c.visiveis(2);
        assert_eq!(v.len(), 5);
        assert_eq!(v.iter().map(|(_, p)| *p).collect::<Vec<_>>(), vec![-2, -1, 0, 1, 2]);
        assert_eq!(v[2].0, 0, "o foco é o do meio");

        let v = c.visiveis(0);
        assert_eq!(v, vec![(0, 0)]);

        // Raio maior que a lista devolve a lista inteira, sem repetir.
        let v = c.visiveis(99);
        assert_eq!(v.len(), 7);
    }

    #[test]
    fn visiveis_nao_repete_faixa_quando_a_lista_e_menor_que_o_raio() {
        let c = Carrossel::novo(3);
        let v = c.visiveis(4);
        let mut indices: Vec<usize> = v.iter().map(|(i, _)| *i).collect();
        indices.sort_unstable();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    // ── geometria ───────────────────────────────────────────

    #[test]
    fn foco_fica_no_centro_com_escala_cheia() {
        let g = geometria(0, 300, 100);
        assert_eq!(g.x, 300);
        assert_eq!(g.escala, 1.0);
        assert_eq!(g.alfa, 255);
    }

    #[test]
    fn vizinhos_sao_simetricos() {
        let esq = geometria(-1, 300, 100);
        let dir = geometria(1, 300, 100);
        assert_eq!(esq.escala, dir.escala);
        assert_eq!(esq.alfa, dir.alfa);
        assert_eq!(esq.x - 300, 300 - dir.x);
        assert!(dir.x > 300 && esq.x < 300);
    }

    #[test]
    fn o_anel_aperta_com_a_distancia() {
        // Passo constante daria 100 px entre cada par; a perspectiva tem de
        // produzir um passo menor lá atrás.
        let g1 = geometria(1, 0, 100);
        let g2 = geometria(2, 0, 100);
        let g3 = geometria(3, 0, 100);
        assert!(g2.x - g1.x < g1.x, "segundo passo maior que o primeiro");
        assert!(g3.x - g2.x < g2.x - g1.x);
    }

    #[test]
    fn escala_e_alfa_encolhem_sem_zerar() {
        let mut anterior = geometria(0, 0, 100);
        for p in 1..12 {
            let g = geometria(p, 0, 100);
            assert!(g.escala <= anterior.escala);
            assert!(g.escala >= ESCALA_MINIMA, "escala sumiu em {p}");
            assert!(g.alfa <= anterior.alfa);
            assert!(g.alfa >= ALFA_MINIMO, "alfa sumiu em {p}");
            anterior = g;
        }
    }

    #[test]
    fn geometria_nao_depende_do_numero_de_faixas() {
        // É o que permite desenhar o carrossel a partir de `visiveis` sem que a
        // lista de sete e a de três se comportem diferente no foco.
        for total in [1, 3, 7] {
            let c = Carrossel::novo(total);
            for (_, p) in c.visiveis(total) {
                let g = geometria(p, 500, 80);
                assert!(g.x > 0 && g.x < 1000);
            }
        }
    }
}
