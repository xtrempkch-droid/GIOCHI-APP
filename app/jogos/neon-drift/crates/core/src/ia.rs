//! Adversários: IA simples que percorre a pista.
//!
//! ## O que a IA **não** faz, e por quê
//!
//! Ela não calcula trajetória, não negocia curva com o jogador e não tem modelo
//! físico próprio: anda sobre a linha central, com um deslocamento lateral que
//! oscila devagar, e escolhe a velocidade olhando a curvatura à frente. Isso é
//! suficiente para parecer corrida e barato o suficiente para rodar junto com a
//! renderização num RK3326.
//!
//! O que ela faz de propósito:
//!
//! * **freia na curva** — adversário que faz a curva em linha reta parece
//!   trenzinho, e o jogador aprende a lição errada (a de que a pista não importa);
//! * **varia de linha** — três carros idênticos em fila indiana não são corrida;
//! * **fecha a porta devagar** — o deslocamento lateral é lento, então dar o golpe
//!   é possível, e não aleatório.

use crate::pista::Ponto;
use crate::pista::Pista;

/// Velocidade-base de um adversário, em m/s.
///
/// Era 48 e a corrida ficou **impossível** — mas o culpado não era o número: a
/// grade nascia antes da linha de largada e o `progresso` creditava uma volta
/// inteira a quem andasse 8 m. Corrigida a grade, 46 deixa a IA ~9 s atrás do
/// piloto artificial em duas voltas: pressiona quem dirige bem, sem exigir
/// perfeição. O máximo do jogador é 62 m/s (82 com nitro).
pub const VEL_BASE: f32 = 46.0;
/// Aceleração do adversário, em m/s². Menor que a do jogador: ele é o herói.
pub const ACEL: f32 = 11.0;
/// Metros à frente em que a IA "enxerga" a curva.
pub const OLHAR: f32 = 30.0;

/// Um piloto adversário.
#[derive(Debug, Clone)]
pub struct Adversario {
    /// Nome mostrado no HUD/resultado.
    pub nome: &'static str,
    /// Posição na volta, em metros.
    pub s: f32,
    /// Volta em que está (1 = primeira).
    pub volta: u32,
    /// Deslocamento lateral atual, em metros.
    pub lateral: f32,
    /// Velocidade atual, em m/s.
    pub velocidade: f32,
    /// Fase da oscilação lateral: dá linha própria a cada piloto.
    pub fase: f32,
    /// Fração da velocidade-base que este piloto consegue (0,8..0,95).
    pub habilidade: f32,
    /// Índice de cor na paleta (o desenho escolhe).
    pub cor: usize,
}

impl Adversario {
    /// Novo piloto, já posicionado em `s`.
    pub fn novo(nome: &'static str, s: f32, habilidade: f32, fase: f32, cor: usize) -> Self {
        Self {
            nome,
            s,
            volta: 1,
            lateral: 0.0,
            velocidade: 0.0,
            fase,
            habilidade: habilidade.clamp(0.72, 0.98),
            cor,
        }
    }

    /// Avança a IA um passo.
    pub fn passo(&mut self, pista: &Pista, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return;
        }

        // Curvatura à frente, em radianos por metro. Um S de 30 m de raio dá
        // ~0,033 rad/m; uma reta dá ~0.
        let aqui = pista.direcao_em(self.s);
        let frente = pista.direcao_em(self.s + OLHAR);
        let angulo = aqui.ponto(frente).clamp(-1.0, 1.0).acos();
        let curvatura = (angulo / OLHAR).clamp(0.0, 0.06);

        // Curva forte reduz o alvo até ~55 % da base. O `.max` existe para o
        // carro nunca parar dentro da curva por causa da conta.
        let fator_curva = (1.0 - curvatura / 0.06 * 0.45).clamp(0.55, 1.0);
        let alvo = (VEL_BASE * self.habilidade * fator_curva).max(12.0);

        let diferenca = alvo - self.velocidade;
        let aceleracao = if diferenca > 0.0 { ACEL } else { ACEL * 2.4 };
        let passo_vel = aceleracao * dt;
        self.velocidade += diferenca.clamp(-passo_vel, passo_vel);

        self.s += self.velocidade * dt;
        let comprimento = pista.comprimento();
        if self.s >= comprimento {
            self.s -= comprimento;
            self.volta = self.volta.saturating_add(1);
        }

        // Linha própria, mas nunca encostando na borda: `0,30` da meia-largura
        // deixa espaço para o jogador passar por dentro ou por fora.
        let meia = pista.meia_largura_em(self.s);
        let amplitude = (meia * 0.30).max(0.5);
        let alvo_lateral = (self.s * 0.021 + self.fase).sin() * amplitude;
        self.lateral += (alvo_lateral - self.lateral) * (1.0 - (-1.4 * dt).exp());
    }

    /// Posição no mundo.
    pub fn posicao(&self, pista: &Pista) -> Ponto {
        pista.ponto_lateral(self.s, self.lateral)
    }

    /// Rumo em radianos, alinhado com a pista naquele ponto.
    pub fn angulo(&self, pista: &Pista) -> f32 {
        let d = pista.direcao_em(self.s);
        d.x.atan2(d.z)
    }

    /// Progresso total na corrida, em metros (para o ranking).
    pub fn progresso(&self, pista: &Pista) -> f32 {
        (self.volta.saturating_sub(1)) as f32 * pista.comprimento() + self.s
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn anda_para_frente_e_da_volta() {
        let pista = Pista::nova();
        let mut a = Adversario::novo("TESTE", 0.0, 0.9, 0.0, 0);
        for _ in 0..(30.0 / (1.0 / 75.0)) as u32 {
            a.passo(&pista, 1.0 / 75.0);
        }
        assert!(a.s > 100.0, "andou só {:.1} m em 30 s", a.s);
        assert!(a.velocidade > 20.0, "velocidade {:.1}", a.velocidade);
    }

    #[test]
    fn freia_na_curva_fechada() {
        let pista = Pista::nova();
        // A curva fechada é achada pela própria medida da pista: escrever
        // `s = 455` à mão já esteve errado quando o traçado mudou.
        let grampo = pista.s_de_raio_minimo();
        let mut na_reta = Adversario::novo("RETA", 40.0, 0.9, 0.0, 0);
        na_reta.velocidade = 50.0;
        let mut na_curva = Adversario::novo("CURVA", grampo - 20.0, 0.9, 0.0, 0);
        na_curva.velocidade = 50.0;
        for _ in 0..60 {
            na_reta.passo(&pista, 1.0 / 75.0);
            na_curva.passo(&pista, 1.0 / 75.0);
        }
        assert!(
            na_curva.velocidade < na_reta.velocidade - 2.0,
            "na curva {:.1} m/s contra {:.1} m/s na reta: a IA não está vendo a curva",
            na_curva.velocidade,
            na_reta.velocidade
        );
    }

    #[test]
    fn a_linha_lateral_fica_dentro_do_asfalto() {
        let pista = Pista::nova();
        let mut a = Adversario::novo("LINHA", 0.0, 0.88, 1.7, 1);
        for _ in 0..(60.0 / (1.0 / 75.0)) as u32 {
            a.passo(&pista, 1.0 / 75.0);
            let meia = pista.meia_largura_em(a.s);
            assert!(
                a.lateral.abs() <= meia.max(1.0),
                "lateral {:.1} maior que a meia-largura {:.1} em s={:.0}",
                a.lateral,
                meia,
                a.s
            );
        }
    }

    #[test]
    fn habilidade_maior_anda_mais() {
        let pista = Pista::nova();
        let mut bom = Adversario::novo("BOM", 0.0, 0.95, 0.0, 0);
        let mut fraco = Adversario::novo("FRACO", 0.0, 0.75, 0.0, 1);
        for _ in 0..(20.0 / (1.0 / 75.0)) as u32 {
            bom.passo(&pista, 1.0 / 75.0);
            fraco.passo(&pista, 1.0 / 75.0);
        }
        assert!(
            bom.progresso(&pista) > fraco.progresso(&pista) + 20.0,
            "os dois pilotos andam quase igual: {:.0} contra {:.0}",
            bom.progresso(&pista),
            fraco.progresso(&pista)
        );
    }

    #[test]
    fn a_volta_e_contada_quando_o_s_da_a_volta() {
        let pista = Pista::nova();
        let mut a = Adversario::novo("VOLTA", pista.comprimento() - 5.0, 0.9, 0.0, 0);
        a.velocidade = 50.0;
        for _ in 0..60 {
            a.passo(&pista, 1.0 / 75.0);
        }
        assert_eq!(a.volta, 2, "não contou a volta; s={:.1}", a.s);
        // 0,8 s a 50 m/s são ~40 m: o que importa é que o `s` deu a volta, não
        // que ele esteja perto de zero.
        assert!(a.s < 45.0, "s não voltou ao começo: {:.1}", a.s);
    }

    #[test]
    fn o_progresso_soma_voltas() {
        let pista = Pista::nova();
        let mut a = Adversario::novo("P", 100.0, 0.9, 0.0, 0);
        a.volta = 2;
        let esperado = pista.comprimento() + 100.0;
        assert!((a.progresso(&pista) - esperado).abs() < 0.01);
    }
}
