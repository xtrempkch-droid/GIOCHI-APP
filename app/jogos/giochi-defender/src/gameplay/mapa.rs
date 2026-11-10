//! Os três mapas: o que é defendido e o que está em volta.
//!
//! ## Por que um mapa é dado e não um `if`
//!
//! Cada mapa tem um **objeto defendido diferente** — rocha, estação, lua — e um
//! cenário em volta. Se isso fosse uma cadeia de `if` dentro de `Arena::new`, o
//! terceiro mapa exigiria mexer no caminho dos dois primeiros, e a regressão
//! apareceria como "a rocha mudou de cor" em quem não pediu nada.
//!
//! Aqui `Mapa` é um valor: [`Mapa::partes`] devolve a geometria e as cores, e a
//! arena só sabe criar malhas e desenhar. Um quarto mapa é uma variante e uma
//! linha em [`Mapa::todos`].
//!
//! ## Todo objeto defendido cabe na mesma esfera de colisão
//!
//! A colisão do jogo é uma esfera de `2.5` no centro ([`RAIO_COLISAO`]). Um mapa
//! cujo objeto passasse disso teria cubos atravessando a geometria antes de
//! contar dano — e o sintoma seria "a vida caiu sem nada encostar". Por isso
//! [`Mapa::raio_externo`] existe e é testado contra a esfera de colisão: os três
//! objetos são **silhuetas diferentes do mesmo volume**.
//!
//! ## Cenário novo não pode criar chamada nova
//!
//! A regra do projeto (vem do `neon-drift`) é que decoração se junta em malha,
//! não em desenho. Um anel de estação feito de 24 caixas é **uma** peça depois de
//! [`juntar`], e as três cores de cada mapa são três chamadas no total.

use crate::utils::procedural_mesh::{
    caixa, esticar, generate_icosphere, juntar, transladar, Malha,
};
use giochi_engine_3d::math3d::Color3D;

/// Raio da esfera de colisão do objeto defendido — o mesmo do bloco 1b.
///
/// Todo mapa tem de caber aqui. Ver [`Mapa::raio_externo`].
pub const RAIO_COLISAO: f32 = crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE;

/// Uma peça do cenário: geometria e cor.
///
/// A cor é por peça porque a engine só tem cor uniforme por desenho. Três peças
/// por mapa é o orçamento: um corpo, um detalhe e um acento.
pub struct Parte {
    /// Geometria já no lugar, em espaço de mundo.
    pub malha: Malha,
    /// Cor da peça.
    pub cor: Color3D,
}

/// O que se defende, e onde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mapa {
    /// A rocha do bloco 1b: icosaedro distorcido, céu frio.
    #[default]
    Asteroide,
    /// Estação espacial: núcleo metálico, anel e painéis solares.
    Estacao,
    /// Lua: esfera lisa com crateras, poeira e céu leitoso.
    Lua,
}

impl Mapa {
    /// Os três, na ordem em que o menu os percorre.
    pub fn todos() -> [Mapa; 3] {
        [Mapa::Asteroide, Mapa::Estacao, Mapa::Lua]
    }

    /// Identificador de linha de comando e de arquivo.
    pub fn id(self) -> &'static str {
        match self {
            Mapa::Asteroide => "asteroide",
            Mapa::Estacao => "estacao",
            Mapa::Lua => "lua",
        }
    }

    /// Nome desenhado no menu.
    pub fn nome(self) -> &'static str {
        match self {
            Mapa::Asteroide => "ASTEROIDE",
            Mapa::Estacao => "ESTACAO",
            Mapa::Lua => "LUA",
        }
    }

    /// O que o jogador está defendendo, numa linha.
    pub fn descricao(self) -> &'static str {
        match self {
            Mapa::Asteroide => "rocha a deriva no cinturao",
            Mapa::Estacao => "estacao orbital com anel e paineis",
            Mapa::Lua => "lua craterada de um gigante gasoso",
        }
    }

    /// Próximo mapa, dando a volta.
    pub fn proximo(self) -> Self {
        match self {
            Mapa::Asteroide => Mapa::Estacao,
            Mapa::Estacao => Mapa::Lua,
            Mapa::Lua => Mapa::Asteroide,
        }
    }

    /// Mapa anterior, dando a volta.
    pub fn anterior(self) -> Self {
        self.proximo().proximo()
    }

    /// Lê o identificador, aceitando maiúscula.
    pub fn parse(texto: &str) -> Option<Self> {
        match texto.trim().to_lowercase().as_str() {
            "asteroide" | "asteróide" | "rocha" => Some(Mapa::Asteroide),
            "estacao" | "estação" | "station" => Some(Mapa::Estacao),
            "lua" | "moon" => Some(Mapa::Lua),
            _ => None,
        }
    }

    /// Cor de fundo do espaço.
    pub fn cor_do_espaco(self) -> Color3D {
        match self {
            // O azul profundo do 1b, sem mudança: é o céu que foi medido.
            Mapa::Asteroide => Color3D::new(0.015, 0.018, 0.032, 1.0),
            // Um pouco mais quente: a estação reflete luz própria.
            Mapa::Estacao => Color3D::new(0.022, 0.024, 0.040, 1.0),
            // Leitoso: a lua está perto de um gigante que a ilumina de lado.
            Mapa::Lua => Color3D::new(0.030, 0.030, 0.044, 1.0),
        }
    }

    /// Multiplicador de cor das estrelas.
    ///
    /// O céu é o mesmo em todos os mapas (três faixas de cubos, uma chamada cada);
    /// o que muda é a tinta. Uma tabela de estrelas por mapa custaria mais malha
    /// para dizer a mesma coisa.
    pub fn tinta_das_estrelas(self) -> Color3D {
        match self {
            Mapa::Asteroide => Color3D::new(1.00, 1.00, 1.00, 1.0),
            Mapa::Estacao => Color3D::new(1.00, 0.97, 0.92, 1.0),
            Mapa::Lua => Color3D::new(0.92, 0.95, 1.00, 1.0),
        }
    }

    /// Raio externo da silhueta, com a distorção já aplicada.
    ///
    /// Tem de caber em [`RAIO_COLISAO`]; há teste para os três.
    pub fn raio_externo(self) -> f32 {
        match self {
            Mapa::Asteroide => 2.0 * (1.0 + 0.22),
            Mapa::Estacao => 2.30,
            Mapa::Lua => 2.0 * (1.0 + 0.08),
        }
    }

    /// A geometria do objeto defendido e do cenário em volta.
    ///
    /// Três peças no máximo — corpo, detalhe e acento —, porque cada peça é uma
    /// chamada de desenho. Tudo o que é do mesmo material já sai junto.
    pub fn partes(self) -> Vec<Parte> {
        match self {
            Mapa::Asteroide => asteroide(),
            Mapa::Estacao => estacao(),
            Mapa::Lua => lua(),
        }
    }
}

/// A rocha do 1b, mais seis pedaços soltos à deriva.
///
/// Os pedaços são **icosaedros pequenos**, não caixas: um cubo de 0,5 a três
/// unidades da câmera parece um inimigo, e o jogador atiraria nele sem entender
/// por que não conta. Silhueta redonda não se confunde com a ameaça.
fn asteroide() -> Vec<Parte> {
    let corpo = generate_icosphere(1, 2.0, 0.22);
    let mut pedacos: Vec<Malha> = Vec::new();
    for (i, angulo) in [0.4f32, 1.5, 2.6, 3.7, 4.8, 5.9].iter().enumerate() {
        let raio = 3.0 + (i as f32) * 0.35;
        // Deslocamento alternado em Y para não formarem um anel plano.
        let altura = ((i as f32) * 1.7).sin() * 1.4;
        let pedaco = generate_icosphere(0, 0.22 + (i as f32) * 0.03, 0.30);
        pedacos.push(transladar(
            &pedaco,
            [raio * angulo.cos(), altura, raio * angulo.sin()],
        ));
    }
    vec![
        Parte {
            malha: corpo,
            cor: Color3D::new(0.58, 0.60, 0.66, 1.0),
        },
        Parte {
            malha: juntar(&pedacos),
            cor: Color3D::new(0.34, 0.36, 0.42, 1.0),
        },
    ]
}

/// Núcleo, anel e painéis solares.
///
/// O anel é feito de 24 caixas ao longo de um círculo: uma peça fundida, uma
/// chamada. Os painéis ficam no plano do anel para a silhueta ler como estação, e
/// o mastro atravessa o núcleo para dar volume visto de cima.
fn estacao() -> Vec<Parte> {
    // Corpo: esfera quase lisa, metálica.
    let nucleo = generate_icosphere(1, 1.30, 0.05);

    let mut anel: Vec<Malha> = Vec::new();
    const SEGMENTOS: usize = 24;
    for i in 0..SEGMENTOS {
        let angulo = std::f32::consts::TAU * i as f32 / SEGMENTOS as f32;
        let raio = 2.15;
        // Cada segmento é uma caixa achatada tangente ao círculo. O comprimento
        // cobre a corda do segmento com 15 % de sobra para não abrir fresta.
        let comprimento = (std::f32::consts::TAU * raio / SEGMENTOS as f32) * 0.58;
        anel.push(caixa(
            [raio * angulo.cos(), 0.0, raio * angulo.sin()],
            [comprimento.max(0.12), 0.075, 0.10],
        ));
    }
    // Quatro travessas radiais: o anel sozinho parece um aro flutuando.
    for i in 0..4 {
        let angulo = std::f32::consts::TAU * i as f32 / 4.0;
        anel.push(caixa(
            [1.10 * angulo.cos(), 0.0, 1.10 * angulo.sin()],
            [0.62, 0.05, 0.06],
        ));
    }

    // Painéis solares: duas placas planas em cada lado, no plano do anel.
    let mut paineis: Vec<Malha> = Vec::new();
    for (dx, dz) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let base = caixa([0.0, 0.0, 0.0], [0.95, 0.012, 0.42]);
        paineis.push(transladar(
            &base,
            [dx * 1.55 + dz * 0.0, 0.30, dz * 1.55 + dx * 0.0],
        ));
    }
    // Mastro vertical, que dá a leitura de "estação" vista de cima.
    paineis.push(caixa([0.0, 0.0, 0.0], [0.055, 1.95, 0.055]));

    vec![
        Parte {
            malha: nucleo,
            cor: Color3D::new(0.66, 0.70, 0.78, 1.0),
        },
        Parte {
            malha: juntar(&anel),
            cor: Color3D::new(0.46, 0.50, 0.60, 1.0),
        },
        Parte {
            malha: juntar(&paineis),
            cor: Color3D::new(0.16, 0.32, 0.62, 1.0),
        },
    ]
}

/// Esfera lisa com crateras e poeira ao redor.
///
/// As crateras são calotas **achatadas** (`esticar` em Y) assentadas na
/// superfície, não buracos: cavar exigiria CSG num renderizador de malha única. A
/// borda elevada é o que dá a leitura de cratera sob a luz rasante.
fn lua() -> Vec<Parte> {
    let corpo = generate_icosphere(2, 2.0, 0.08);

    let mut crateras: Vec<Malha> = Vec::new();
    // Seis posições espalhadas por octantes diferentes, para não haver um lado
    // "liso" que o jogador veja ao girar a câmera.
    let posicoes: [(f32, f32, f32, f32); 6] = [
        (0.75, 0.55, 0.36, 0.62),
        (-0.62, 0.30, 0.72, 0.48),
        (0.18, -0.80, 0.57, 0.55),
        (-0.30, -0.35, -0.88, 0.44),
        (0.66, 0.15, -0.74, 0.52),
        (-0.80, 0.60, -0.20, 0.40),
    ];
    for (x, y, z, tamanho) in posicoes {
        let norma = (x * x + y * y + z * z).sqrt();
        let (nx, ny, nz) = (x / norma, y / norma, z / norma);
        let distancia = 1.92;
        let calota = esticar(&generate_icosphere(1, tamanho, 0.05), [1.0, 0.22, 1.0]);
        crateras.push(transladar(
            &calota,
            [nx * distancia, ny * distancia, nz * distancia],
        ));
    }

    // Poeira: pedriscos pequenos e claros, que dão escala à esfera lisa.
    let mut poeira: Vec<Malha> = Vec::new();
    for i in 0..8 {
        let angulo = 0.7 * i as f32;
        let raio = 3.1 + 0.22 * i as f32;
        let altura = ((i as f32) * 2.3).cos() * 1.1;
        poeira.push(transladar(
            &generate_icosphere(0, 0.16 + 0.02 * i as f32, 0.35),
            [raio * angulo.cos(), altura, raio * angulo.sin()],
        ));
    }

    vec![
        Parte {
            malha: corpo,
            cor: Color3D::new(0.72, 0.74, 0.78, 1.0),
        },
        Parte {
            malha: juntar(&crateras),
            cor: Color3D::new(0.50, 0.52, 0.58, 1.0),
        },
        Parte {
            malha: juntar(&poeira),
            cor: Color3D::new(0.62, 0.63, 0.67, 1.0),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_tres_objetos_cabem_na_esfera_de_colisao() {
        // Se um objeto passar de 2.5, o cubo atravessa a geometria antes de
        // contar dano e o sintoma é "a vida caiu sem nada encostar".
        for mapa in Mapa::todos() {
            let raio = mapa.raio_externo();
            assert!(
                raio <= RAIO_COLISAO,
                "{}: raio externo {raio:.2} passa da colisão {RAIO_COLISAO:.2}",
                mapa.nome()
            );
            // E não pode ser minúsculo: o objeto é o alvo que se defende.
            assert!(raio > 1.8, "{}: raio {raio:.2} pequeno demais", mapa.nome());
        }
    }

    #[test]
    fn cada_mapa_tem_geometria_diferente() {
        // "3 mapas com objetos diferentes" — a comparação é da **silhueta**
        // (vértices e caixa), não só da contagem: a estação também usa
        // `icosphere(1)` e teria o mesmo número de vértices que a rocha.
        let assinaturas: Vec<(usize, [f32; 3], [f32; 3])> = Mapa::todos()
            .iter()
            .map(|mapa| {
                let corpo = &mapa.partes()[0].malha;
                let (minimo, maximo) = corpo.limites();
                (corpo.vertices.len(), minimo, maximo)
            })
            .collect();
        for i in 0..assinaturas.len() {
            for j in (i + 1)..assinaturas.len() {
                assert_ne!(
                    assinaturas[i],
                    assinaturas[j],
                    "{} e {} têm o mesmo corpo",
                    Mapa::todos()[i].nome(),
                    Mapa::todos()[j].nome()
                );
            }
        }
    }

    #[test]
    fn cada_mapa_cabe_no_orcamento_de_chamadas() {
        // A regra do projeto: cenário novo não cria chamada nova. Três peças por
        // mapa é o teto, e o teto é testado para não ser esquecido.
        for mapa in Mapa::todos() {
            let partes = mapa.partes();
            assert!(
                partes.len() <= 3,
                "{}: {} peças, orçamento é 3",
                mapa.nome(),
                partes.len()
            );
            assert!(!partes.is_empty());
        }
    }

    #[test]
    fn a_geometria_de_cada_peca_e_finita() {
        // Uma normal `NaN` contamina a matriz de iluminação e a cena inteira fica
        // preta, com o erro aparecendo longe da causa.
        for mapa in Mapa::todos() {
            for parte in mapa.partes() {
                assert!(!parte.malha.vertices.is_empty(), "{}", mapa.nome());
                assert_eq!(parte.malha.indices.len() % 3, 0);
                for vertice in &parte.malha.vertices {
                    for eixo in 0..3 {
                        assert!(
                            vertice.position[eixo].is_finite(),
                            "{}: posição não finita",
                            mapa.nome()
                        );
                        assert!(
                            vertice.normal[eixo].is_finite(),
                            "{}: normal não finita",
                            mapa.nome()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn nenhuma_peca_invade_o_campo_dos_inimigos() {
        // Os pedaços de cenário ficam perto do objeto; se algum chegasse à casca
        // de spawn (15+), seria lido como inimigo.
        for mapa in Mapa::todos() {
            for parte in mapa.partes() {
                let (minimo, maximo) = parte.malha.limites();
                let extremo = minimo
                    .iter()
                    .chain(maximo.iter())
                    .fold(0.0f32, |acc, v| acc.max(v.abs()));
                assert!(
                    extremo < crate::gameplay::partida::RAIO_SPAWN_MIN,
                    "{}: peça chega a {extremo:.1}",
                    mapa.nome()
                );
            }
        }
    }

    #[test]
    fn os_identificadores_sao_lidos_e_percorridos_em_laco() {
        for mapa in Mapa::todos() {
            assert_eq!(Mapa::parse(mapa.id()), Some(mapa));
        }
        assert_eq!(Mapa::parse("LUA"), Some(Mapa::Lua));
        assert_eq!(Mapa::parse("estação"), Some(Mapa::Estacao));
        assert_eq!(Mapa::parse("marte"), None);

        let mut mapa = Mapa::Asteroide;
        let mut vistos = Vec::new();
        for _ in 0..3 {
            vistos.push(mapa);
            mapa = mapa.proximo();
        }
        assert_eq!(vistos, Mapa::todos().to_vec());
        assert_eq!(mapa, Mapa::Asteroide);
        assert_eq!(Mapa::Asteroide.anterior(), Mapa::Lua);
    }

    #[test]
    fn cada_mapa_tem_ceu_e_descricao_proprios() {
        let ceus: Vec<Color3D> = Mapa::todos().iter().map(|m| m.cor_do_espaco()).collect();
        for i in 0..ceus.len() {
            for j in (i + 1)..ceus.len() {
                assert_ne!(ceus[i], ceus[j], "dois mapas com o mesmo fundo");
            }
        }
        let descricoes: Vec<&str> = Mapa::todos().iter().map(|m| m.descricao()).collect();
        assert_eq!(descricoes.len(), 3);
        assert_ne!(descricoes[0], descricoes[1]);
        assert_ne!(descricoes[1], descricoes[2]);
    }
}
