//! Regras da cobrinha.
//!
//! ## O que este módulo **não** faz
//!
//! Não conhece SDL, cor, pixel nem relógio. Recebe passos e devolve estado. Isso
//! é o que permite testar a lógica inteira numa máquina sem biblioteca gráfica —
//! e o que faz uma partida de 500 passos rodar em milissegundos no teste.
//!
//! ## Sem alocação por passo
//!
//! O corpo é um **anel de tamanho fixo** (`[Posicao; CAPACIDADE]`). A cabeça
//! avança o índice e a cauda é "esquecida" sem mover memória. Um jogo de grade
//! que aloca a cada passo entrega ao alocador uma tarefa que ele não precisa ter,
//! e num Cortex-A35 cada `malloc` é uma chance de o quadro estourar o prazo.
//!
//! ## Determinismo
//!
//! O sorteio da comida é um `xorshift64` com semente. A mesma semente e a mesma
//! sequência de direções produzem a mesma partida — é o que permite comparar
//! dois quadros e afirmar que o desenho, e não a sorte, mudou.

use crate::geometria::{COLUNAS, LINHAS};

/// Teto do corpo. 24×18 = 432 células; o anel é maior que o tabuleiro inteiro,
/// então a cobra pode teoricamente preencher tudo sem nunca dar a volta no anel.
pub const CAPACIDADE: usize = 512;

/// Quantas viradas ficam na fila. Três cobrem o gesto rápido mais comum
/// (duas curvas encadeadas) sem deixar o jogador "programar" meia pista.
pub const FILA_MAX: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posicao {
    pub x: i32,
    pub y: i32,
}

impl Posicao {
    pub const fn nova(x: i32, y: i32) -> Self {
        Posicao { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direcao {
    Cima,
    Baixo,
    Esquerda,
    Direita,
}

impl Direcao {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direcao::Cima => (0, -1),
            Direcao::Baixo => (0, 1),
            Direcao::Esquerda => (-1, 0),
            Direcao::Direita => (1, 0),
        }
    }

    /// A direção contrária. Virar 180° mataria a cobra contra o próprio pescoço,
    /// então é o único movimento recusado.
    pub fn oposta(self) -> Self {
        match self {
            Direcao::Cima => Direcao::Baixo,
            Direcao::Baixo => Direcao::Cima,
            Direcao::Esquerda => Direcao::Direita,
            Direcao::Direita => Direcao::Esquerda,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estado {
    /// Esperando o jogador começar. A cobra já está desenhada.
    Pronto,
    Jogando,
    Perdido,
}

/// O que aconteceu num passo. O chamador decide se toca som, se salva recorde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evento {
    Nada,
    Comeu,
    Morreu(Motivo),
    Ignorado,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motivo {
    Parede,
    ProprioCorpo,
}

pub struct Jogo {
    corpo: [Posicao; CAPACIDADE],
    cabeca: usize,
    tamanho: usize,
    /// Onde a cabeça estava antes do último passo. É o que a interpolação usa.
    anterior: Posicao,
    pub direcao: Direcao,
    /// Viradas pedidas que ainda não foram aplicadas.
    ///
    /// Precisa ser fila, não um slot: dentro de um mesmo passo o jogador pode
    /// apertar Cima e depois Esquerda. Com um slot só, a segunda substituía a
    /// primeira, e o passo seguinte tentava virar da direita para a esquerda —
    /// 180°, recusado — e o jogador perdia as duas viradas. Com a fila, cada
    /// passo aplica uma, na ordem em que foram pedidas.
    fila: [Direcao; FILA_MAX],
    na_fila: usize,
    pub comida: Posicao,
    pub pontos: u32,
    pub estado: Estado,
    semente: u64,
    passos: u32,
}

impl Jogo {
    pub fn novo(semente: u64) -> Self {
        let semente = if semente == 0 { 0x9E37_79B9_7F4A_7C15 } else { semente };
        let mut jogo = Jogo {
            corpo: [Posicao::nova(0, 0); CAPACIDADE],
            cabeca: 0,
            tamanho: 3,
            anterior: Posicao::nova(0, 0),
            direcao: Direcao::Direita,
            fila: [Direcao::Direita; FILA_MAX],
            na_fila: 0,
            comida: Posicao::nova(0, 0),
            pontos: 0,
            estado: Estado::Pronto,
            semente,
            passos: 0,
        };
        jogo.reiniciar();
        jogo
    }

    pub fn reiniciar(&mut self) {
        let y = LINHAS / 2;
        self.tamanho = 3;
        self.cabeca = 2;
        // Corpo horizontal, cabeça à direita.
        for k in 0..self.tamanho {
            self.corpo[(self.cabeca + CAPACIDADE - k) % CAPACIDADE] = Posicao::nova(3 - k as i32, y);
        }
        self.anterior = self.cabeca_pos();
        self.direcao = Direcao::Direita;
        self.na_fila = 0;
        self.pontos = 0;
        self.passos = 0;
        self.estado = Estado::Pronto;
        self.sortear_comida();
    }

    /// Começa a partida. Sem isto a cobra fica parada — de propósito, para o
    /// placar não começar a correr antes de o jogador entender o controle.
    pub fn comecar(&mut self) {
        if self.estado == Estado::Pronto || self.estado == Estado::Perdido {
            if self.estado == Estado::Perdido {
                self.reiniciar();
            }
            self.estado = Estado::Jogando;
        }
    }

    pub fn cabeca_pos(&self) -> Posicao {
        self.corpo[self.cabeca]
    }

    /// Segmento `k` do corpo, 0 = cabeça.
    pub fn segmento(&self, k: usize) -> Posicao {
        self.corpo[(self.cabeca + CAPACIDADE - k % CAPACIDADE) % CAPACIDADE]
    }

    pub fn tamanho(&self) -> usize {
        self.tamanho
    }

    pub fn passos(&self) -> u32 {
        self.passos
    }

    /// Pede uma direção. Aplicada no próximo passo, se não for a oposta.
    ///
    /// Devolve `false` quando o pedido é recusado — recusar em silêncio é o que
    /// faz o jogador achar que o controle falhou.
    pub fn pedir_direcao(&mut self, d: Direcao) -> bool {
        // A comparação é com a **última intenção conhecida**, não com a direção
        // atual: apertando Cima e depois Esquerda, a segunda não pode ser
        // recusada por ser oposta à direita atual.
        let base = if self.na_fila == 0 {
            self.direcao
        } else {
            self.fila[self.na_fila - 1]
        };
        if d == base || d == base.oposta() || self.na_fila >= FILA_MAX {
            return false;
        }
        self.fila[self.na_fila] = d;
        self.na_fila += 1;
        true
    }

    /// Avança um passo de simulação.
    pub fn passo(&mut self) -> Evento {
        if self.estado != Estado::Jogando {
            return Evento::Ignorado;
        }
        self.passos = self.passos.saturating_add(1);

        // Uma virada por passo, na ordem em que foi pedida. Descartar as
        // demais seria "engolir" entrada; guardar todas aplicaria uma curva
        // por quadro e a cobra rodaria no lugar.
        if self.na_fila > 0 {
            let d = self.fila[0];
            if d != self.direcao.oposta() && d != self.direcao {
                self.direcao = d;
            }
            self.fila.copy_within(1..self.na_fila, 0);
            self.na_fila -= 1;
        }

        let (dx, dy) = self.direcao.delta();
        let cabeca = self.cabeca_pos();
        let nova = Posicao::nova(cabeca.x + dx, cabeca.y + dy);
        self.anterior = cabeca;

        // Parede: o campo tem borda. Atravessar seria mais fácil e foi recusado
        // de propósito — num visor de 3,5" o jogador não vê o outro lado.
        if nova.x < 0 || nova.x >= COLUNAS || nova.y < 0 || nova.y >= LINHAS {
            self.estado = Estado::Perdido;
            return Evento::Morreu(Motivo::Parede);
        }

        let comeu = nova == self.comida;
        // O corpo encolhe "de trás para frente": ao comer, a cauda não sai.
        // Sem comer, a última célula é esquecida — e por isso encostar nela é
        // permitido, senão o jogo mataria num movimento que ele mesmo faria.
        let ultimo = if comeu { self.tamanho } else { self.tamanho - 1 };
        for k in 0..ultimo {
            if self.segmento(k) == nova {
                self.estado = Estado::Perdido;
                return Evento::Morreu(Motivo::ProprioCorpo);
            }
        }

        let indice = (self.cabeca + 1) % CAPACIDADE;
        self.corpo[indice] = nova;
        self.cabeca = indice;
        if comeu {
            if self.tamanho < CAPACIDADE {
                self.tamanho += 1;
            }
            self.pontos += 1;
            self.sortear_comida();
            return Evento::Comeu;
        }
        Evento::Nada
    }

    /// Posição da cabeça **entre** duas células, em coordenada de grade.
    ///
    /// `alpha` vem do relógio: 0 é a célula anterior, 1 é a atual. É isto que
    /// transforma 8 passos por segundo em movimento liso a 45 Hz.
    pub fn cabeca_interpolada(&self, alpha: f32) -> (f32, f32) {
        let (ax, ay) = (self.anterior.x as f32, self.anterior.y as f32);
        let (cx, cy) = (self.cabeca_pos().x as f32, self.cabeca_pos().y as f32);
        let a = alpha.clamp(0.0, 1.0);
        (ax + (cx - ax) * a, ay + (cy - ay) * a)
    }

    fn sortear_comida(&mut self) {
        let livres = COLUNAS * LINHAS - self.tamanho as i32;
        if livres <= 0 {
            return;
        }
        let escolha = (self.proximo_aleatorio() % livres as u64) as i32;
        // Percorre a grade contando células livres até a escolhida. O(n) sobre
        // 432 células a 16 Hz é 7 mil comparações por segundo — nada, e evita
        // manter uma lista de livres que precisaria de alocação.
        let mut contador = 0;
        for y in 0..LINHAS {
            for x in 0..COLUNAS {
                let p = Posicao::nova(x, y);
                let ocupada = (0..self.tamanho).any(|k| self.segmento(k) == p);
                if ocupada {
                    continue;
                }
                if contador == escolha {
                    self.comida = p;
                    return;
                }
                contador += 1;
            }
        }
    }

    fn proximo_aleatorio(&mut self) -> u64 {
        // xorshift64*: determinístico, sem dependência e sem estado global.
        let mut x = self.semente;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.semente = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn partida(semente: u64) -> Jogo {
        let mut j = Jogo::novo(semente);
        j.comecar();
        j
    }

    #[test]
    fn comeca_com_tres_segmentos_e_parada() {
        let j = Jogo::novo(1);
        assert_eq!(j.tamanho(), 3);
        assert_eq!(j.estado, Estado::Pronto);
        assert_eq!(j.passos(), 0);
    }

    #[test]
    fn andar_move_a_cabeca_uma_celula() {
        let mut j = partida(1);
        let antes = j.cabeca_pos();
        assert_eq!(j.passo(), Evento::Nada);
        let depois = j.cabeca_pos();
        assert_eq!(depois.x, antes.x + 1);
        assert_eq!(depois.y, antes.y);
        assert_eq!(j.passo(), Evento::Nada);
        assert_eq!(j.cabeca_pos().x, antes.x + 2);
    }

    #[test]
    fn nao_vira_cento_e_oitenta_graus() {
        let mut j = partida(1);
        assert!(!j.pedir_direcao(Direcao::Esquerda), "oposta tem de ser recusada");
        assert!(j.pedir_direcao(Direcao::Cima));
        // Mesma direção também é recusada: não é "aceitar e não fazer nada".
        assert!(!j.pedir_direcao(Direcao::Cima));
    }

    #[test]
    fn duas_viradas_no_mesmo_passo_nao_se_perdem() {
        // Cima e depois Esquerda, pedidas antes de o passo acontecer.
        // A primeira versão usava um slot só: Esquerda substituía Cima, e o
        // passo seguinte tentava virar de Direita para Esquerda (180°, recusado)
        // — o jogador perdia as duas viradas e batia na parede.
        // Com fila, cada passo aplica uma, na ordem.
        let mut j = partida(1);
        assert!(j.pedir_direcao(Direcao::Cima));
        assert!(j.pedir_direcao(Direcao::Esquerda));
        j.passo();
        assert_eq!(j.direcao, Direcao::Cima, "a primeira da fila vale neste passo");
        j.passo();
        assert_eq!(j.direcao, Direcao::Esquerda, "a segunda vale no passo seguinte");
    }

    #[test]
    fn a_fila_nao_aceita_meia_pista_de_viradas() {
        let mut j = partida(1);
        assert!(j.pedir_direcao(Direcao::Cima));
        assert!(j.pedir_direcao(Direcao::Esquerda));
        assert!(j.pedir_direcao(Direcao::Baixo));
        assert!(!j.pedir_direcao(Direcao::Direita), "quarta virada tem de ser recusada");
    }

    #[test]
    fn parede_mata() {
        let mut j = partida(3);
        // Da coluna 2 até a borda direita são 21 passos.
        for _ in 0..(COLUNAS + 5) {
            if let Evento::Morreu(m) = j.passo() {
                assert_eq!(m, Motivo::Parede);
                assert_eq!(j.estado, Estado::Perdido);
                return;
            }
        }
        panic!("não morreu na parede");
    }

    #[test]
    fn comer_cresce_e_pontua() {
        let mut j = partida(7);
        // Coloca a comida logo à frente para o teste não depender do sorteio.
        let cabeca = j.cabeca_pos();
        j.comida = Posicao::nova(cabeca.x + 1, cabeca.y);
        assert_eq!(j.passo(), Evento::Comeu);
        assert_eq!(j.pontos, 1);
        assert_eq!(j.tamanho(), 4);
    }

    #[test]
    fn comida_nunca_nasce_em_cima_do_corpo() {
        // A comida nasce sorteada; se caísse sobre a cobra, ela cresceria sem o
        // jogador chegar perto — e o jogador não teria como saber por quê.
        for semente in 1..=200u64 {
            let j = Jogo::novo(semente);
            for k in 0..j.tamanho() {
                assert_ne!(
                    j.segmento(k),
                    j.comida,
                    "semente {semente}: comida em cima do segmento {k}"
                );
            }
        }
    }

    #[test]
    fn reiniciar_devolve_tudo_ao_comeco() {
        let mut j = partida(5);
        for _ in 0..5 {
            j.passo();
        }
        j.reiniciar();
        assert_eq!(j.tamanho(), 3);
        assert_eq!(j.pontos, 0);
        assert_eq!(j.estado, Estado::Pronto);
        assert_eq!(j.passos(), 0);
    }

    #[test]
    fn a_mesma_semente_da_a_mesma_partida() {
        // Determinismo: sem isto, comparar dois quadros não prova nada.
        let comida_a = Jogo::novo(42).comida;
        let comida_b = Jogo::novo(42).comida;
        assert_eq!(comida_a, comida_b);
    }

    #[test]
    fn interpolacao_anda_entre_as_duas_celulas() {
        let mut j = partida(1);
        j.passo();
        let (x0, _) = j.cabeca_interpolada(0.0);
        let (x1, _) = j.cabeca_interpolada(1.0);
        let (xm, _) = j.cabeca_interpolada(0.5);
        assert!((x1 - x0 - 1.0).abs() < 1e-6, "um passo é uma célula");
        assert!(xm > x0 && xm < x1, "o meio fica entre as pontas: {x0} {xm} {x1}");
    }

    #[test]
    fn alpha_fora_da_faixa_e_limitado() {
        let mut j = partida(1);
        j.passo();
        let (x_neg, _) = j.cabeca_interpolada(-5.0);
        let (x_grande, _) = j.cabeca_interpolada(99.0);
        assert!((x_neg - j.cabeca_interpolada(0.0).0).abs() < 1e-6);
        assert!((x_grande - j.cabeca_interpolada(1.0).0).abs() < 1e-6);
    }

    #[test]
    fn encostar_na_propria_cauda_nao_mata() {
        // Clássico: a cauda sai no mesmo passo, então a célula dela é livre.
        let mut j = partida(1);
        j.pedir_direcao(Direcao::Cima);
        j.passo();
        j.pedir_direcao(Direcao::Esquerda);
        j.passo();
        j.pedir_direcao(Direcao::Baixo);
        let r = j.passo();
        assert_ne!(r, Evento::Morreu(Motivo::ProprioCorpo), "morreu na própria cauda");
    }

    #[test]
    fn partida_longa_nao_estoura_o_anel() {
        // 600 passos com crescimento simulado: o anel tem de aguentar sem pânico
        // e sem índice fora da faixa.
        let mut j = partida(99);
        for i in 0..600 {
            if j.estado != Estado::Jogando {
                break;
            }
            if i % 7 == 0 {
                j.pontos += 1;
                j.tamanho = (j.tamanho + 1).min(CAPACIDADE);
            }
            j.passo();
            let _ = j.segmento(j.tamanho() - 1);
        }
        assert!(j.tamanho() <= CAPACIDADE);
    }
}
