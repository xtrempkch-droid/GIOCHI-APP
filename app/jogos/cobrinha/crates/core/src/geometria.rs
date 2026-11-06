//! Onde cada célula cai na tela.
//!
//! ## Sem upscaling: a célula é o pixel
//!
//! A tentação num jogo de grade é desenhar num espaço fixo (digamos 640×480) e
//! deixar o SDL esticar para o painel. Isso é **upscaling**, e num RK3326 com
//! Mali-G31 custa duas coisas: banda de memória (cada pixel é escrito duas
//! vezes) e nitidez (célula de 26 px vira 29,25 px — célula com borda serrilhada
//! de tamanho diferente das vizinhas).
//!
//! Aqui a célula é calculada em **pixels inteiros a partir do painel real**, e o
//! desenho acontece em coordenada de dispositivo. Escala é sempre 1,0. Painel
//! maior não estica o jogo: dá células maiores.
//!
//! ## Por que a conta é inteira
//!
//! `celula = min(largura_util / colunas, altura_util / linhas)` com divisão
//! inteira deixaria uma sobra de até `colunas-1` pixels. A sobra é distribuída
//! como margem, então o campo fica **centralizado** em vez de encostado à
//! esquerda. Nenhuma célula é maior que outra — que é o defeito visível de
//! qualquer arredondamento por célula.

/// Quantas colunas e linhas o campo tem. 4:3, como os painéis que existem.
pub const COLUNAS: i32 = 24;
pub const LINHAS: i32 = 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometria {
    /// Lado da célula em pixels de dispositivo. Nunca fracionário.
    pub celula: i32,
    /// Canto superior esquerdo do campo, já centralizado.
    pub campo_x: i32,
    pub campo_y: i32,
    /// Altura reservada para o placar, acima do campo.
    pub hud: i32,
    /// Sobra não usada, para o relatório de layout.
    pub sobra_x: i32,
    pub sobra_y: i32,
    /// Área disponível para o campo em cada eixo, já sem margem e sem HUD.
    /// Exposta porque o teste precisa afirmar "a célula é a maior que cabe", e
    /// recalcular a conta no teste seria duplicar a fórmula que ele vigia.
    pub util_l: i32,
    pub util_a: i32,
}

impl Geometria {
    /// Calcula o layout para um painel de `largura`×`altura` pixels.
    ///
    /// `hud_fracao` é a fração da altura reservada ao placar. Em 640×480 com
    /// 0,10 dá 48 px — espaço para o placar sem roubar o campo.
    pub fn calcular(largura: i32, altura: i32, hud_fracao: f32) -> Self {
        // Margem proporcional ao menor lado, com piso: em 640×480 dá 12 px, em
        // 1280×720 dá 18. Piso de 4 para painel minúsculo não gerar margem zero.
        let margem = (largura.min(altura) / 40).max(4);
        let hud = ((altura as f32 * hud_fracao) as i32).max(16);

        let util_l = (largura - 2 * margem).max(COLUNAS);
        let util_a = (altura - 2 * margem - hud).max(LINHAS);

        // Divisão inteira: o piso é o que garante que todas as células cabem.
        let celula = (util_l / COLUNAS).min(util_a / LINHAS).max(1);

        let campo_w = celula * COLUNAS;
        let campo_h = celula * LINHAS;
        // Sobra dividida ao meio: sobra ímpar deixa 1 px a mais à direita, e
        // isso é melhor que célula de tamanho diferente.
        let sobra_x = util_l - campo_w;
        let sobra_y = util_a - campo_h;

        Geometria {
            celula,
            campo_x: margem + sobra_x / 2,
            campo_y: margem + hud + sobra_y / 2,
            hud,
            sobra_x,
            sobra_y,
            util_l,
            util_a,
        }
    }

    pub fn campo_w(&self) -> i32 {
        self.celula * COLUNAS
    }

    pub fn campo_h(&self) -> i32 {
        self.celula * LINHAS
    }

    /// Canto superior esquerdo da célula `(coluna, linha)`, em pixels.
    pub fn celula_em(&self, coluna: i32, linha: i32) -> (i32, i32) {
        (
            self.campo_x + coluna * self.celula,
            self.campo_y + linha * self.celula,
        )
    }

    /// Amplitude do painel usada pelo campo: separa "cabe" de "ocupa".
    pub fn ocupacao(&self, largura: i32, altura: i32) -> f32 {
        let area_campo = (self.campo_w() * self.campo_h()) as f32;
        (area_campo / (largura * altura) as f32) * 100.0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn celula_e_inteira_e_cabe_no_painel() {
        for (l, a) in [(640, 480), (800, 600), (1280, 720), (720, 1280), (480, 640)] {
            let g = Geometria::calcular(l, a, 0.10);
            assert!(g.celula >= 1, "{l}x{a}: célula {0}", g.celula);
            assert!(
                g.campo_x >= 0 && g.campo_x + g.campo_w() <= l,
                "{l}x{a}: campo horizontal {}+{} estoura {l}",
                g.campo_x,
                g.campo_w()
            );
            assert!(
                g.campo_y >= 0 && g.campo_y + g.campo_h() <= a,
                "{l}x{a}: campo vertical {}+{} estoura {a}",
                g.campo_y,
                g.campo_h()
            );
        }
    }

    #[test]
    fn escala_e_sempre_um_para_um() {
        // A célula vem do painel, não de um espaço de projeto esticado. Este é
        // o teste que reprova se alguém reintroduzir `set_logical_size`.
        let pequeno = Geometria::calcular(640, 480, 0.10);
        let grande = Geometria::calcular(1280, 720, 0.10);
        assert!(
            grande.celula > pequeno.celula,
            "painel maior tem de dar célula maior: {} contra {}",
            grande.celula,
            pequeno.celula
        );
    }

    #[test]
    fn a_celula_e_a_maior_que_cabe() {
        // A propriedade que importa não é "sobra pequena" — no eixo que não
        // limita, sobra grande é esperada. É "não dava para fazer célula maior":
        // uma célula a mais teria de estourar algum dos dois eixos. Sem isto, um
        // erro de arredondamento passaria com célula menor do que o painel
        // permite, e o campo ficaria menor sem ninguém notar.
        for (l, a) in [(640, 480), (800, 600), (1280, 720), (720, 1280)] {
            let g = Geometria::calcular(l, a, 0.10);
            let maior = g.celula + 1;
            let cabe = maior * COLUNAS <= g.util_l && maior * LINHAS <= g.util_a;
            assert!(
                !cabe,
                "{l}x{a}: célula {} mas {} também caberia",
                g.celula, maior
            );
        }
    }

    #[test]
    fn campo_fica_centralizado() {
        let g = Geometria::calcular(640, 480, 0.10);
        let direita = 640 - (g.campo_x + g.campo_w());
        assert!(
            (g.campo_x - direita).abs() <= 1,
            "margem esquerda {} contra direita {direita}",
            g.campo_x
        );
    }

    #[test]
    fn o_eixo_que_limita_e_aproveitado_inteiro() {
        // O campo é 4:3 e ocupa o máximo do eixo que limita — o outro sobra, e
        // isso é esperado (ver o teste seguinte). Aqui se exige que o eixo
        // limitante não desperdice mais de 6%, que é menos de uma célula.
        for (l, a) in [(640, 480), (800, 600), (1280, 720), (720, 1280)] {
            let g = Geometria::calcular(l, a, 0.10);
            let uso_l = g.campo_w() as f32 / g.util_l as f32;
            let uso_a = g.campo_h() as f32 / g.util_a as f32;
            let melhor = uso_l.max(uso_a);
            assert!(melhor > 0.94, "{l}x{a}: eixo limitante só {melhor:.3} de uso");
        }
    }

    #[test]
    fn em_painel_16_por_9_sobra_lateral_e_isso_e_esperado() {
        // Documenta a consequência do campo 4:3 num painel 16:9 (o X55, girado):
        // sobram faixas laterais. Não é defeito — mudar a grade para 24×13
        // mudaria a jogabilidade entre aparelhos, que é pior.
        let g = Geometria::calcular(1280, 720, 0.10);
        let uso_l = g.campo_w() as f32 / g.util_l as f32;
        assert!(uso_l < 0.80, "16:9 deveria sobrar largura, deu {uso_l:.3}");
        assert!(g.campo_h() + g.celula > g.util_a, "altura tem de estar no limite");
    }

    #[test]
    fn painel_degenerado_nao_divide_por_zero() {
        let g = Geometria::calcular(1, 1, 0.10);
        assert_eq!(g.celula, 1);
    }
}
