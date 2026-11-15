//! Layout adaptativo: tudo é proporção do painel, nada é 1280×720 fixo.
//!
//! O launcher já pagou por esta lição: com projeto fixo, um painel 640×480 saía
//! com escala 0,5, a fonte `F_TINY` de 9 px virava 4,5 px no dispositivo e
//! sobrava 25% de letterbox. Aqui o layout recebe o tamanho do espaço de projeto
//! e devolve margens, barras, tamanho de capa e corpos de fonte — sempre com um
//! piso de fonte legível, porque texto de 5 px não é texto pequeno, é mancha.

/// Medidas de uma tela, derivadas do espaço de projeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub largura: i32,
    pub altura: i32,
    /// Margem lateral.
    pub margem: i32,
    /// Altura da barra de título.
    pub topo: i32,
    /// Altura da faixa de dicas.
    pub rodape: i32,
    /// Altura da barra de transporte (estado, volume, duração).
    pub barra: i32,
    /// Tamanho da capa em foco.
    pub capa: i32,
    /// Distância horizontal entre a capa em foco e a vizinha.
    pub passo: i32,
    /// Centro vertical das capas.
    pub centro_y: i32,
    /// Corpo de fonte do título da tela.
    pub fonte_titulo: u16,
    /// Corpo de fonte do nome da faixa em foco.
    pub fonte_nome: u16,
    /// Corpo de fonte do texto corrido.
    pub fonte_corpo: u16,
    /// Corpo de fonte de rodapé/rótulos.
    pub fonte_mini: u16,
}

impl Layout {
    /// Fonte de rodapé nunca abaixo de 9 px: abaixo disso não se lê no painel
    /// de 640×480 e a dica deixa de ser dica.
    pub const FONTE_MINIMA: u16 = 9;

    pub fn de(largura: i32, altura: i32) -> Layout {
        let largura = largura.max(1);
        let altura = altura.max(1);

        let margem = (altura / 28).clamp(8, 28);
        let topo = (altura / 8).clamp(36, 104);
        let rodape = (altura / 9).clamp(30, 76);
        let barra = (altura / 7).clamp(40, 120);
        let livre = (altura - topo - rodape - barra).max(48);

        // A capa cabe na altura livre e na largura com margem dos dois lados.
        let capa = (livre * 3 / 4)
            .min(largura - 2 * margem)
            .clamp(32, 420);
        let passo = (capa * 62 / 100).max(8);

        Layout {
            largura,
            altura,
            margem,
            topo,
            rodape,
            barra,
            capa,
            passo,
            centro_y: topo + livre / 2,
            fonte_titulo: (altura / 15).clamp(16, 40) as u16,
            fonte_nome: (altura / 22).clamp(13, 30) as u16,
            fonte_corpo: (altura / 30).clamp(11, 26) as u16,
            fonte_mini: (altura / 48).clamp(Layout::FONTE_MINIMA as i32, 16) as u16,
        }
    }

    /// Área onde as capas podem ser desenhadas: abaixo do topo, acima da barra
    /// de transporte.
    pub fn area_do_carrossel(&self) -> (i32, i32, i32, i32) {
        (
            0,
            self.topo,
            self.largura,
            (self.altura - self.topo - self.rodape - self.barra).max(0),
        )
    }

    /// Y do topo da barra de transporte.
    pub fn barra_y(&self) -> i32 {
        self.altura - self.rodape - self.barra
    }

    /// Y do topo da faixa de dicas.
    pub fn rodape_y(&self) -> i32 {
        self.altura - self.rodape
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAINEIS: [(i32, i32); 5] = [
        (640, 480),   // X35H e R35S
        (1280, 720),  // X55 girado (o espaço de projeto)
        (1920, 1080), // monitor de PC
        (320, 240),   // piso: painel pequeno de janela
        (720, 1280),  // retrato pedido direto, sem giro
    ];

    #[test]
    fn toda_medida_e_positiva_em_qualquer_painel() {
        for (l, a) in PAINEIS {
            let x = Layout::de(l, a);
            for (nome, valor) in [
                ("margem", x.margem),
                ("topo", x.topo),
                ("rodape", x.rodape),
                ("barra", x.barra),
                ("capa", x.capa),
                ("passo", x.passo),
                ("centro_y", x.centro_y),
            ] {
                assert!(valor > 0, "{l}x{a}: {nome} = {valor}");
            }
            assert!(x.fonte_mini >= Layout::FONTE_MINIMA);
            assert!(x.fonte_mini <= x.fonte_corpo);
            assert!(x.fonte_corpo <= x.fonte_nome);
            assert!(x.fonte_nome <= x.fonte_titulo);
        }
    }

    #[test]
    fn as_barras_cabem_na_altura() {
        for (l, a) in PAINEIS {
            let x = Layout::de(l, a);
            assert!(
                x.topo + x.rodape + x.barra < a,
                "{l}x{a}: barras somam {} de {a}",
                x.topo + x.rodape + x.barra
            );
            assert!(x.rodape_y() > x.barra_y(), "{l}x{a}: rodapé acima da barra");
        }
    }

    #[test]
    fn a_capa_cabe_na_area_do_carrossel() {
        for (l, a) in PAINEIS {
            let x = Layout::de(l, a);
            let (_, topo, _, altura) = x.area_do_carrossel();
            assert!(x.capa <= altura, "{l}x{a}: capa {} > área {altura}", x.capa);
            assert!(x.capa <= l - 2 * x.margem, "{l}x{a}: capa mais larga que a margem");
            assert!(x.centro_y - x.capa / 2 >= topo, "{l}x{a}: capa invade o topo");
            assert!(
                x.centro_y + x.capa / 2 <= x.barra_y(),
                "{l}x{a}: capa invade a barra"
            );
        }
    }

    #[test]
    fn painel_maior_nao_diminui_fonte() {
        let pequeno = Layout::de(640, 480);
        let medio = Layout::de(1280, 720);
        let grande = Layout::de(1920, 1080);
        assert!(pequeno.fonte_mini <= medio.fonte_mini);
        assert!(medio.fonte_mini <= grande.fonte_mini);
        assert!(pequeno.fonte_titulo <= medio.fonte_titulo);
        assert!(medio.fonte_titulo <= grande.fonte_titulo);
        assert!(pequeno.capa <= medio.capa);
        assert!(medio.capa <= grande.capa);
    }

    #[test]
    fn painel_de_640_480_nao_tem_fonte_de_mancha() {
        // Era este o defeito do projeto fixo: 9 px de fonte virando 4,5 px.
        let x = Layout::de(640, 480);
        assert!(x.fonte_mini >= 9, "rodapé com {} px", x.fonte_mini);
        assert!(x.fonte_corpo >= 11);
    }

    #[test]
    fn painel_degenerado_nao_estoura() {
        // Janela de 1x1 acontece em teste automatizado e em driver que ignora o
        // pedido de tamanho; dividir por zero ali derrubaria o app.
        let x = Layout::de(1, 1);
        assert!(x.largura >= 1 && x.altura >= 1);
        assert!(x.capa > 0 && x.fonte_mini >= Layout::FONTE_MINIMA);
        let x = Layout::de(0, 0);
        assert!(x.capa > 0);
    }

    #[test]
    fn rodape_e_barra_terminam_no_fim_da_tela() {
        for (l, a) in PAINEIS {
            let x = Layout::de(l, a);
            assert_eq!(x.rodape_y(), x.barra_y() + x.barra);
            assert_eq!(x.barra_y(), x.altura - x.rodape - x.barra);
        }
    }
}
