//! Layout da HUD em **pixels** e o estado dos efeitos.
//!
//! ## Por que em pixels e não em unidades de HUD
//!
//! A conversão para unidades da câmera de 90° ([`crate::ui::reticle`]) é uma
//! linha e acontece no desenho. O layout fica em pixels porque é nessa unidade
//! que as perguntas reais se fazem: "o placar de seis dígitos cabe ao lado das
//! vidas?", "o `+100` sai pela borda de cima?". Um layout em unidades relativas
//! responderia as duas, mas ninguém as escreveria assim.
//!
//! ## As três faixas do topo não podem se cruzar
//!
//! Vidas à esquerda, onda ao centro, pontos à direita — três blocos que crescem
//! com o conteúdo. No `neon-drift` este projeto já teve o rodapé com velocidade,
//! marcha e barra empilhados sobre a mesma faixa de 40 px, e o resultado foi
//! texto ilegível numa tela de 3,5". Aqui há um teste que mede a **caixa de cada
//! bloco** em quatro tamanhos de painel e reprova se elas se tocarem.

use crate::state::{GameState, WAVES_TOTAIS};
use crate::ui::font;

/// Texto neutro da HUD.
pub const COR_TEXTO: [f32; 4] = [0.86, 0.93, 1.0, 1.0];
/// Números e destaques: o ciano do asteroide.
pub const COR_DESTAQUE: [f32; 4] = [0.36, 0.92, 1.0, 1.0];
/// Dano, derrota e o que precisa doer.
pub const COR_ALERTA: [f32; 4] = [1.0, 0.34, 0.28, 1.0];
/// Vida perdida: presente no desenho, ausente na leitura.
pub const COR_APAGADA: [f32; 4] = [0.24, 0.28, 0.36, 1.0];

/// Quanto tempo o `+100` fica na tela, em segundos.
pub const VIDA_FLUTUANTE: f32 = 0.9;
/// Quanto tempo o clarão de dano dura.
pub const VIDA_DANO: f32 = 0.45;
/// Quanto tempo o aviso de onda fica.
pub const VIDA_AVISO: f32 = 1.6;
/// Quantos `+100` simultâneos o estado guarda. Mais que isto é oito abates em
/// menos de um segundo, e o nono empurraria o primeiro para fora da tela.
const TETO_FLUTUANTES: usize = 8;

/// Tamanho da tela em pixels, como o desenho o vê.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tela {
    /// Largura desenhável.
    pub largura: f32,
    /// Altura desenhável.
    pub altura: f32,
}

impl Tela {
    /// Constrói a partir do tamanho do drawable.
    pub fn new(largura: u32, altura: u32) -> Self {
        Self {
            largura: largura.max(1) as f32,
            altura: altura.max(1) as f32,
        }
    }
}

/// Um texto posicionado, com o canto superior esquerdo em pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Texto {
    /// O que escrever. Só caixa alta e a pontuação da [`font`].
    pub conteudo: String,
    /// Canto esquerdo, em pixels.
    pub x: f32,
    /// Topo, em pixels.
    pub y: f32,
    /// Pixels por ponto do glifo.
    pub escala: f32,
    /// Cor com alfa.
    pub cor: [f32; 4],
}

impl Texto {
    /// Largura ocupada, em pixels.
    pub fn largura(&self) -> f32 {
        font::largura_texto(&self.conteudo) as f32 * self.escala
    }

    /// Altura ocupada, em pixels.
    pub fn altura(&self) -> f32 {
        font::ALTURA as f32 * self.escala
    }

    /// Retângulo ocupado: `(x, y, largura, altura)`.
    pub fn caixa(&self) -> (f32, f32, f32, f32) {
        (self.x, self.y, self.largura(), self.altura())
    }

    /// Direita do texto.
    pub fn direita(&self) -> f32 {
        self.x + self.largura()
    }

    /// Base do texto.
    pub fn base(&self) -> f32 {
        self.y + self.altura()
    }
}

/// Um retângulo cheio — vida, faixa de dano, fundo de aviso.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    /// Canto esquerdo.
    pub x: f32,
    /// Topo.
    pub y: f32,
    /// Largura.
    pub w: f32,
    /// Altura.
    pub h: f32,
    /// Cor com alfa.
    pub cor: [f32; 4],
}

/// Tudo o que a HUD desenha num quadro.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Cena {
    /// Textos, em ordem de desenho.
    pub textos: Vec<Texto>,
    /// Retângulos, desenhados antes dos textos.
    pub retangulos: Vec<Retangulo>,
}

impl Cena {
    /// Acrescenta outra cena a esta.
    pub fn juntar(&mut self, outra: Cena) {
        self.retangulos.extend(outra.retangulos);
        self.textos.extend(outra.textos);
    }
}

/// O menor lado da tela.
///
/// É dele que a HUD tira o tamanho, e **não** da altura. Num painel retrato
/// (720×1280) escalar pela altura daria letra de 37 px e os três blocos do topo
/// se cruzariam — medido: o bloco das vidas terminava em 448 px e o `ONDA`
/// começava em 237. O menor lado é o que de fato limita o espaço horizontal.
fn menor_lado(tela: Tela) -> f32 {
    tela.largura.min(tela.altura).max(1.0)
}

/// Escala do HUD: pixels por ponto do glifo.
///
/// Proporcional ao menor lado, com piso 1,0 — a 480p dá 2,0 (letra de 10×14 px),
/// a 720p dá 3,0. O piso existe porque num painel minúsculo a letra viraria pó
/// em vez de continuar legível.
pub fn escala(tela: Tela) -> f32 {
    (menor_lado(tela) / 240.0).max(1.0)
}

/// Margem das bordas, em pixels.
pub fn margem(tela: Tela) -> f32 {
    (menor_lado(tela) * 0.045).round().max(6.0)
}

/// Lado de um marcador de vida, em pixels.
pub fn lado_da_vida(tela: Tela) -> f32 {
    (font::ALTURA as f32 * escala(tela)).round()
}

/// Um `+100` subindo e sumindo.
#[derive(Debug, Clone, PartialEq)]
struct Flutuante {
    x: f32,
    y: f32,
    pontos: u32,
    idade: f32,
}

/// Um aviso grande no centro: a onda que começou, ou o fim da partida.
#[derive(Debug, Clone, PartialEq)]
struct Aviso {
    texto: String,
    idade: f32,
    vida: f32,
}

/// O que a HUD guarda entre quadros: só os efeitos.
///
/// O placar e as vidas **não** estão aqui — eles são o [`GameState`], a única
/// fonte da verdade. Duplicar o número criaria dois lugares para ele envelhecer
/// diferente, que é o defeito que o `year_label` do launcher já pagou caro.
#[derive(Debug, Default, Clone)]
pub struct Hud {
    flutuantes: Vec<Flutuante>,
    dano: f32,
    aviso: Option<Aviso>,
    /// Wave anunciada pela última vez, para o aviso disparar uma vez por onda.
    onda_anunciada: u32,
}

impl Hud {
    /// HUD nova, sem efeito nenhum.
    pub fn new() -> Self {
        Self {
            flutuantes: Vec::new(),
            dano: 0.0,
            aviso: None,
            onda_anunciada: 1,
        }
    }

    /// Registra um abate: o `+100` nasce na posição de tela do alvo.
    ///
    /// A posição vem em pixels porque quem tem a câmera é o chamador — e porque
    /// projetar aqui exigiria a matriz, que não pertence ao layout.
    pub fn abateu(&mut self, x: f32, y: f32, pontos: u32) {
        if self.flutuantes.len() >= TETO_FLUTUANTES {
            self.flutuantes.remove(0);
        }
        self.flutuantes.push(Flutuante {
            x,
            y,
            pontos,
            idade: 0.0,
        });
    }

    /// Registra dano no asteroide: o clarão vermelho na borda.
    pub fn levou_dano(&mut self) {
        self.dano = VIDA_DANO;
    }

    /// Anuncia uma onda, uma vez por valor.
    pub fn anunciar_onda(&mut self, onda: u32) {
        if onda == self.onda_anunciada {
            return;
        }
        self.onda_anunciada = onda;
        self.aviso = Some(Aviso {
            texto: format!("ONDA {onda}"),
            idade: 0.0,
            vida: VIDA_AVISO,
        });
    }

    /// Anuncia o fim da partida (vitória ou derrota).
    ///
    /// **Idempotente**, e isso não é elegância: o laço chama isto todo quadro
    /// enquanto a partida está acabada, e recriar o aviso a cada chamada zerava a
    /// idade — a animação de entrada nunca terminava e o aviso ficava com alfa 0,
    /// ou seja, invisível. O sintoma era "o jogo acaba e a tela não diz nada".
    pub fn anunciar_fim(&mut self, venceu: bool) {
        let texto = if venceu { "VITORIA" } else { "FIM DE JOGO" };
        if self.aviso.as_ref().is_some_and(|a| a.texto == texto) {
            return;
        }
        // Um aviso de fim não expira: quem decide voltar é o jogador, e a tela
        // não pode ficar em silêncio esperando.
        self.aviso = Some(Aviso {
            texto: texto.to_string(),
            idade: 0.0,
            vida: f32::INFINITY,
        });
    }

    /// Faz o tempo passar nos efeitos.
    pub fn atualizar(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.flutuantes.retain_mut(|f| {
            f.idade += dt;
            f.idade < VIDA_FLUTUANTE
        });
        self.dano = (self.dano - dt).max(0.0);
        if let Some(aviso) = &mut self.aviso {
            aviso.idade += dt;
            if aviso.idade >= aviso.vida {
                self.aviso = None;
            }
        }
    }

    /// Quantos `+100` estão vivos agora — medida de teste e de diagnóstico.
    pub fn flutuantes(&self) -> usize {
        self.flutuantes.len()
    }

    /// O clarão de dano está aceso?
    pub fn com_dano(&self) -> bool {
        self.dano > 0.0
    }

    /// Zera tudo — partida nova.
    pub fn limpar(&mut self) {
        *self = Self::new();
    }
}

/// Monta a cena inteira da HUD para um quadro.
pub fn cena(estado: &GameState, hud: &Hud, tela: Tela) -> Cena {
    let mut cena = fixos(estado, tela);
    cena.juntar(efeitos(estado, hud, tela));
    cena
}

/// Os blocos permanentes: vidas, onda e pontos.
pub fn fixos(estado: &GameState, tela: Tela) -> Cena {
    let esc = escala(tela);
    let m = margem(tela);
    let mut cena = Cena::default();

    // ── esquerda: VIDA + marcadores ──────────────────────────────
    let rotulo = Texto {
        conteudo: "VIDA".to_string(),
        x: m,
        y: m,
        escala: esc,
        cor: COR_TEXTO,
    };
    let lado = lado_da_vida(tela);
    let vao = (lado * 0.42).round();
    let x_marcadores = rotulo.direita() + lado * 0.9;
    let y_marcadores = m + (rotulo.altura() - lado).max(0.0) * 0.5;
    cena.textos.push(rotulo);

    let total = crate::state::VIDAS_INICIAIS.max(1);
    for i in 0..total {
        let viva = i < estado.lives;
        cena.retangulos.push(Retangulo {
            x: x_marcadores + i as f32 * (lado + vao),
            y: y_marcadores,
            w: lado,
            h: lado,
            cor: if viva { COR_DESTAQUE } else { COR_APAGADA },
        });
    }

    // ── centro: ONDA n/3 ─────────────────────────────────────────
    let onda = Texto {
        conteudo: format!("ONDA {}/{}", estado.wave.min(WAVES_TOTAIS), WAVES_TOTAIS),
        x: 0.0,
        y: m,
        escala: esc,
        cor: COR_TEXTO,
    };
    let onda = Texto {
        x: ((tela.largura - onda.largura()) * 0.5).round(),
        ..onda
    };
    cena.textos.push(onda);

    // ── direita: PONTOS 000000 ──────────────────────────────────
    let pontos = Texto {
        conteudo: format!("PONTOS {:06}", estado.score.min(999_999)),
        x: 0.0,
        y: m,
        escala: esc,
        cor: COR_DESTAQUE,
    };
    let pontos = Texto {
        x: (tela.largura - m - pontos.largura()).round(),
        ..pontos
    };
    cena.textos.push(pontos);

    cena
}

/// Os efeitos: `+100`, clarão de dano, aviso central.
pub fn efeitos(estado: &GameState, hud: &Hud, tela: Tela) -> Cena {
    let esc = escala(tela);
    let mut cena = Cena::default();

    if hud.com_dano() {
        cena.juntar(clarao(hud.dano / VIDA_DANO, tela));
    }

    for f in &hud.flutuantes {
        let t = (f.idade / VIDA_FLUTUANTE).clamp(0.0, 1.0);
        // Sobe `12·escala` no total e some na última metade da vida. O texto
        // encolhe um pouco ao subir: parado, ele briga com o alvo.
        let texto = Texto {
            conteudo: format!("+{}", f.pontos),
            x: f.x,
            y: f.y - 12.0 * esc * t,
            escala: esc * (1.0 - 0.25 * t),
            cor: [COR_DESTAQUE[0], COR_DESTAQUE[1], COR_DESTAQUE[2], 1.0 - t],
        };
        // Centraliza no ponto do abate.
        cena.textos.push(Texto {
            x: f.x - texto.largura() * 0.5,
            ..texto
        });
    }

    if let Some(aviso) = &hud.aviso {
        cena.juntar(distintivo(aviso, estado, tela));
    }

    cena
}

/// Clarão vermelho nas quatro bordas quando o asteroide leva dano.
///
/// Quatro faixas em vez de um retângulo de tela cheia com alfa: um véu sobre a
/// cena inteira escurece o que o jogador precisa ver para continuar jogando.
fn clarao(fracao: f32, tela: Tela) -> Cena {
    let f = fracao.clamp(0.0, 1.0);
    let espessura = (tela.altura * 0.10 * f).round().max(1.0);
    let cor = [COR_ALERTA[0], COR_ALERTA[1], COR_ALERTA[2], 0.55 * f];
    let (w, h) = (tela.largura, tela.altura);
    Cena {
        textos: Vec::new(),
        retangulos: vec![
            Retangulo {
                x: 0.0,
                y: 0.0,
                w,
                h: espessura,
                cor,
            },
            Retangulo {
                x: 0.0,
                y: h - espessura,
                w,
                h: espessura,
                cor,
            },
            Retangulo {
                x: 0.0,
                y: espessura,
                w: espessura,
                h: h - 2.0 * espessura,
                cor,
            },
            Retangulo {
                x: w - espessura,
                y: espessura,
                w: espessura,
                h: h - 2.0 * espessura,
                cor,
            },
        ],
    }
}

/// O texto central com fundo — aviso de onda e fim de partida.
fn distintivo(aviso: &Aviso, estado: &GameState, tela: Tela) -> Cena {
    let esc = escala(tela) * 1.6;
    // Entra em 0,25 s e sai nos últimos 0,35 s; o aviso de fim não sai.
    let entrando = (aviso.idade / 0.25).clamp(0.0, 1.0);
    let saindo = if aviso.vida.is_finite() {
        ((aviso.vida - aviso.idade) / 0.35).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let alfa = entrando.min(saindo);

    let mut textos = Vec::new();
    let mut retangulos = Vec::new();

    let titulo = Texto {
        conteudo: aviso.texto.clone(),
        x: 0.0,
        y: tela.altura * 0.40,
        escala: esc * (0.85 + 0.15 * entrando),
        cor: [COR_TEXTO[0], COR_TEXTO[1], COR_TEXTO[2], alfa],
    };
    let titulo = Texto {
        x: ((tela.largura - titulo.largura()) * 0.5).round(),
        ..titulo
    };

    let mut bloco = Cena::default();
    // Fundo escuro atrás da faixa, do tamanho do título com folga.
    let folga = esc * 4.0;
    bloco.retangulos.push(Retangulo {
        x: titulo.x - folga,
        y: titulo.y - folga * 0.55,
        w: titulo.largura() + 2.0 * folga,
        h: titulo.altura() + folga * 1.1,
        cor: [0.02, 0.03, 0.06, 0.72 * alfa],
    });
    let linha = bloco.retangulos[0];
    let sublinha = Texto {
        conteudo: format!("PONTOS {:06}", estado.score.min(999_999)),
        x: 0.0,
        y: linha.y + linha.h + esc * 1.4,
        escala: esc * 0.5,
        cor: [COR_DESTAQUE[0], COR_DESTAQUE[1], COR_DESTAQUE[2], alfa],
    };
    let sublinha = Texto {
        x: ((tela.largura - sublinha.largura()) * 0.5).round(),
        ..sublinha
    };

    textos.push(titulo);
    // O placar embaixo do aviso só faz sentido no fim da partida; numa onda
    // nova ele repetiria o canto superior direito e o aviso viraria ruído.
    if !aviso.vida.is_finite() {
        textos.push(sublinha);
    }
    retangulos.extend(bloco.retangulos);

    Cena { textos, retangulos }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TELAS: [(u32, u32); 4] = [(640, 480), (800, 600), (1280, 720), (720, 1280)];

    fn estado_com(score: u32, lives: i32, wave: u32) -> GameState {
        GameState {
            score,
            lives,
            wave,
            paused: false,
        }
    }

    /// Os textos que não são efeito: vidas, onda e pontos.
    fn blocos(estado: &GameState, tela: Tela) -> Vec<Texto> {
        fixos(estado, tela).textos
    }

    #[test]
    fn a_hud_cabe_na_tela_em_todos_os_paineis() {
        for (w, h) in TELAS {
            let tela = Tela::new(w, h);
            let cena = cena(&estado_com(999_999, 5, 3), &Hud::new(), tela);
            for t in &cena.textos {
                let (x, y, l, a) = t.caixa();
                assert!(
                    x >= 0.0 && y >= 0.0 && x + l <= tela.largura && y + a <= tela.altura,
                    "{w}x{h}: texto {:?} em ({x:.0},{y:.0},{l:.0},{a:.0}) sai da tela",
                    t.conteudo
                );
            }
            for r in &cena.retangulos {
                assert!(
                    r.x >= -0.5
                        && r.y >= -0.5
                        && r.x + r.w <= tela.largura + 0.5
                        && r.y + r.h <= tela.altura + 0.5,
                    "{w}x{h}: retângulo em ({:.0},{:.0},{:.0},{:.0}) sai da tela",
                    r.x,
                    r.y,
                    r.w,
                    r.h
                );
            }
        }
    }

    #[test]
    fn os_tres_blocos_do_topo_nao_se_sobrepoem() {
        // A regressão que este teste existe para pegar: vidas, onda e pontos
        // crescendo lado a lado até se cruzarem numa tela estreita. Medido com o
        // pior caso de cada um (5 vidas, wave 3, seis dígitos de placar).
        for (w, h) in TELAS {
            let tela = Tela::new(w, h);
            let textos = blocos(&estado_com(999_999, 5, 3), tela);
            assert_eq!(textos.len(), 3, "esperava VIDA, ONDA e PONTOS");

            let (vida, onda, pontos) = (&textos[0], &textos[1], &textos[2]);
            let vidas = vida.caixa();
            // A caixa das vidas inclui os marcadores, que ficam à direita do
            // rótulo — o rótulo sozinho não é a largura do bloco.
            let marcadores = lado_da_vida(tela) * 5.0 + (lado_da_vida(tela) * 0.42).round() * 4.0;
            let caixa_vidas = (vidas.0, vidas.1, vida.largura() + marcadores, vidas.3);

            for (a, b, nome) in [
                (caixa_vidas, onda.caixa(), "vidas × onda"),
                (onda.caixa(), pontos.caixa(), "onda × pontos"),
                (caixa_vidas, pontos.caixa(), "vidas × pontos"),
            ] {
                let (ax, ay, aw, ah) = a;
                let (bx, by, bw, bh) = b;
                let cruza = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
                assert!(!cruza, "{w}x{h}: {nome} se cruzam");
                // Não basta não cruzar: encostado lê como um bloco só. A folga
                // exigida é de uma letra de altura.
                let folga = if ax < bx {
                    bx - (ax + aw)
                } else if bx < ax {
                    ax - (bx + bw)
                } else {
                    f32::INFINITY
                };
                let minimo = font::ALTURA as f32 * escala(tela);
                assert!(
                    folga >= minimo,
                    "{w}x{h}: {nome} encostam (folga {folga:.0} px, mínimo {minimo:.0})"
                );
            }
        }
    }

    #[test]
    fn as_vidas_mostram_uma_caixa_por_vida() {
        for lives in 0..=crate::state::VIDAS_INICIAIS {
            let tela = Tela::new(640, 480);
            let cena = fixos(&estado_com(0, lives, 1), tela);
            let marcadores: Vec<_> = cena
                .retangulos
                .iter()
                .filter(|r| r.w == lado_da_vida(tela))
                .collect();
            assert_eq!(
                marcadores.len(),
                crate::state::VIDAS_INICIAIS as usize,
                "lives={lives}"
            );
            let acesas = marcadores.iter().filter(|r| r.cor == COR_DESTAQUE).count();
            assert_eq!(acesas, lives.max(0) as usize, "lives={lives}");
        }
    }

    #[test]
    fn o_placar_tem_seis_digitos_e_nao_estoura_a_caixa() {
        let tela = Tela::new(640, 480);
        for (score, esperado) in [
            (0, "PONTOS 000000"),
            (7, "PONTOS 000007"),
            (99, "PONTOS 000099"),
            (1_200, "PONTOS 001200"),
            (999_999, "PONTOS 999999"),
            // Acima de seis dígitos o número é **limitado**, não cortado: sete
            // dígitos empurrariam o bloco para cima do `ONDA`.
            (1_000_000, "PONTOS 999999"),
            (u32::MAX, "PONTOS 999999"),
        ] {
            let cena = fixos(&estado_com(score, 5, 1), tela);
            let pontos = cena.textos.last().expect("placar");
            assert_eq!(pontos.conteudo, esperado, "score {score}");
        }
    }

    #[test]
    fn todo_texto_da_hud_usa_caracteres_que_a_fonte_tem() {
        // Um `Ç` ou uma minúscula esquecida vira buraco na tela, sem erro. Este
        // teste é o que transforma o buraco em falha de build.
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.anunciar_fim(false);
        hud.abateu(100.0, 100.0, 100);
        let cena = cena(&estado_com(1_200, 2, 3), &hud, tela);
        assert!(!cena.textos.is_empty());
        for t in &cena.textos {
            for c in t.conteudo.chars() {
                assert!(
                    font::glifo(c).is_some(),
                    "texto {:?} usa {c:?}, que a fonte não tem",
                    t.conteudo
                );
            }
        }
    }

    #[test]
    fn o_correio_do_abate_sobe_e_some() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.abateu(320.0, 240.0, 100);
        assert_eq!(hud.flutuantes(), 1);

        let inicio = efeitos(&estado_com(100, 5, 1), &hud, tela).textos;
        assert_eq!(inicio.len(), 1);
        assert_eq!(inicio[0].conteudo, "+100");
        assert_eq!(inicio[0].cor[3], 1.0, "o +100 nasce opaco");
        let y0 = inicio[0].y;

        hud.atualizar(0.45);
        let meio = efeitos(&estado_com(100, 5, 1), &hud, tela).textos;
        assert!(meio[0].y < y0, "o +100 tinha de subir");
        assert!(meio[0].cor[3] < 1.0, "o +100 tinha de desbotar");

        hud.atualizar(VIDA_FLUTUANTE);
        assert_eq!(hud.flutuantes(), 0, "o +100 tinha de expirar");
        assert!(efeitos(&estado_com(100, 5, 1), &hud, tela)
            .textos
            .is_empty());
    }

    #[test]
    fn o_correio_fica_centralizado_no_ponto_do_abate() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.abateu(320.0, 200.0, 100);
        let texto = &efeitos(&estado_com(100, 5, 1), &hud, tela).textos[0];
        let centro = texto.x + texto.largura() * 0.5;
        assert!(
            (centro - 320.0).abs() < 1.0,
            "centro do +100 em {centro:.1}, esperado 320"
        );
    }

    #[test]
    fn o_teto_de_flutuantes_empurra_o_mais_velho() {
        let mut hud = Hud::new();
        for i in 0..TETO_FLUTUANTES + 4 {
            hud.abateu(10.0, 10.0, i as u32);
        }
        assert_eq!(hud.flutuantes(), TETO_FLUTUANTES);
    }

    #[test]
    fn o_clarao_de_dano_apaga_sozinho() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        assert!(!hud.com_dano());
        assert!(efeitos(&estado_com(0, 5, 1), &hud, tela)
            .retangulos
            .is_empty());

        hud.levou_dano();
        let cheio = efeitos(&estado_com(0, 4, 1), &hud, tela).retangulos;
        assert_eq!(cheio.len(), 4, "quatro faixas de borda");
        let espessura_cheia = cheio[0].h;

        hud.atualizar(VIDA_DANO * 0.5);
        let meio = efeitos(&estado_com(0, 4, 1), &hud, tela).retangulos;
        assert!(
            meio[0].h < espessura_cheia,
            "o clarão tinha de afinar: {:.1} → {:.1}",
            espessura_cheia,
            meio[0].h
        );

        hud.atualizar(VIDA_DANO);
        assert!(!hud.com_dano());
        assert!(efeitos(&estado_com(0, 4, 1), &hud, tela)
            .retangulos
            .is_empty());
    }

    #[test]
    fn o_aviso_de_onda_dispara_uma_vez_por_onda() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.anunciar_onda(1);
        assert!(
            hud.aviso.is_none(),
            "a wave 1 é onde a partida começa; anunciá-la é ruído"
        );

        hud.anunciar_onda(2);
        let cena = efeitos(&estado_com(0, 5, 2), &hud, tela);
        assert_eq!(cena.textos[0].conteudo, "ONDA 2");
        // Uma onda sem aviso de fim não mostra o placar: ele já está no canto.
        assert_eq!(cena.textos.len(), 1);

        // Repetir a mesma onda não reanima o aviso.
        hud.atualizar(VIDA_AVISO);
        assert!(hud.aviso.is_none());
        hud.anunciar_onda(2);
        assert!(hud.aviso.is_none(), "wave repetida não anuncia de novo");
    }

    #[test]
    fn o_aviso_de_fim_nao_expira_e_mostra_o_placar() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.anunciar_fim(false);
        hud.atualizar(60.0);
        let cena = efeitos(&estado_com(1_700, 0, 2), &hud, tela);
        assert_eq!(cena.textos[0].conteudo, "FIM DE JOGO");
        assert_eq!(cena.textos[1].conteudo, "PONTOS 001700");
        assert_eq!(cena.textos[0].cor[3], 1.0, "o fim fica opaco");
    }

    #[test]
    fn o_aviso_de_fim_nao_reinicia_quando_repetido() {
        // O laço chama `anunciar_fim` a cada quadro enquanto a partida está
        // acabada. Recriar o aviso zerava a idade e a animação de entrada nunca
        // passava de alfa 0 — o aviso existia e não aparecia.
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.anunciar_fim(false);
        hud.atualizar(0.5);
        hud.anunciar_fim(false);
        hud.anunciar_fim(false);
        let cena = efeitos(&estado_com(100, 0, 1), &hud, tela);
        assert!(
            cena.textos[0].cor[3] > 0.9,
            "alfa {:.2} depois de três anúncios — o aviso reiniciou",
            cena.textos[0].cor[3]
        );
    }

    #[test]
    fn a_vitoria_tem_texto_proprio() {
        let tela = Tela::new(640, 480);
        let mut hud = Hud::new();
        hud.anunciar_fim(true);
        let cena = efeitos(&estado_com(3_000, 3, WAVES_TOTAIS + 1), &hud, tela);
        assert_eq!(cena.textos[0].conteudo, "VITORIA");
    }

    #[test]
    fn a_onda_mostrada_nao_passa_da_ultima() {
        let tela = Tela::new(640, 480);
        // Depois de vencer, `wave` é `WAVES_TOTAIS + 1`: a HUD não pode dizer 4/3.
        let textos = blocos(&estado_com(0, 3, WAVES_TOTAIS + 1), tela);
        assert_eq!(
            textos[1].conteudo,
            format!("ONDA {WAVES_TOTAIS}/{WAVES_TOTAIS}")
        );
    }

    #[test]
    fn o_tempo_negativo_nao_reanima_nem_apaga_efeito() {
        let mut hud = Hud::new();
        hud.levou_dano();
        hud.abateu(10.0, 10.0, 100);
        hud.atualizar(-5.0);
        hud.atualizar(f32::NAN);
        assert!(hud.com_dano(), "dt inválido não pode apagar o clarão");
        assert_eq!(hud.flutuantes(), 1);
    }

    #[test]
    fn limpar_devolve_a_hud_ao_estado_inicial() {
        let mut hud = Hud::new();
        hud.levou_dano();
        hud.abateu(1.0, 2.0, 100);
        hud.anunciar_onda(3);
        hud.limpar();
        assert!(!hud.com_dano());
        assert_eq!(hud.flutuantes(), 0);
        assert!(hud.aviso.is_none());
    }

    #[test]
    fn a_escala_cresce_com_o_painel_e_tem_piso() {
        assert_eq!(escala(Tela::new(640, 480)), 2.0);
        assert_eq!(escala(Tela::new(800, 600)), 2.5);
        assert_eq!(escala(Tela::new(1280, 720)), 3.0);
        assert_eq!(
            escala(Tela::new(320, 240)),
            1.0,
            "num painel minúsculo a letra não pode virar pó"
        );
    }

    #[test]
    fn a_hud_do_aparelho_e_legivel() {
        // 480p é o painel dos dois aparelhos: a letra tem de passar de 10 px de
        // altura, que é o piso prático num painel de 3,5".
        let tela = Tela::new(640, 480);
        let altura_da_letra = font::ALTURA as f32 * escala(tela);
        assert!(
            altura_da_letra >= 10.0,
            "letra de {altura_da_letra} px em 480p"
        );
    }
}
