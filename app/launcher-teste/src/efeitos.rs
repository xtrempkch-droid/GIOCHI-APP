//! Efeitos visuais do launcher-teste — a camada que a engine desenha por cima
//! das telas reais.
//!
//! ## Por que são **opcionais**, e ligados por flag
//!
//! Este app existe para uma pergunta só: *o launcher desenhado sobre a
//! GIOCHI-ENGINE sai igual ao de produção?* A resposta é medida por comparação
//! pixel a pixel (`comparar-launcher-teste.sh`, `aparelho.sh verificar-teste`) e
//! hoje dá **100,0 %** em quatro tamanhos. Um efeito desenhado por padrão
//! destruiria essa medida — e a medida é a razão de o app existir.
//!
//! Então o desenho das telas **não muda uma linha**. O que entra é uma camada
//! separada, ligada por `--efeitos` (ou `GIOCHI_EFEITOS=1`), desenhada depois do
//! `hud()` e antes do `present()`. Sem a flag, o binário desenha exatamente o
//! que desenhava.
//!
//! ## Por que o booleano não basta como "modo demo"
//!
//! `--demo` troca o **catálogo** (quatro jogos de mentira) para a tela ter
//! conteúdo. Isso muda o desenho e por isso ele também é comparação só nos
//! casos em que os dois lados usam o mesmo catálogo. Efeito é outra dimensão: é
//! sobre *o que a engine consegue desenhar*, não sobre *o que a tela mostra*.
//!
//! ## Determinismo
//!
//! Nada aqui lê relógio. A animação avança por **contagem de quadros**
//! (`tick`), então `--efeitos --dump-frame` produz o mesmo BMP em toda execução
//! — sem isso, "o efeito ficou bom" não seria comparável e o teste de regressão
//! de imagem não existiria.
//!
//! ## O que cada efeito exercita na engine
//!
//! | Efeito | Primitiva |
//! | --- | --- |
//! | brasas subindo | `fill_circle` → `fill_rects` (lote de varreduras) |
//! | varredura de luz | `draw_rect` em bandas |
//! | vinheta | `draw_rect` em bandas de alfa |
//! | scanlines | **um** `fill_rects` com centenas de retângulos |
//! | halo do foco | `stroke_round_rect` pulsante |
//! | rastro de comando | `draw_line` |
//! | pulso da borda | `draw_rect` em quatro tiras |
//!
//! O `fill_rects` das scanlines é o caso mais interessante: são ~160
//! retângulos de 1 px que sairiam em 160 chamadas se cada um fosse um
//! `draw_rect`. Em lote é **uma**.

use giochi_launcher_core::state::{State, Tab};
use giochi_launcher_core::Zone;
use sdl2::pixels::Color;

use crate::draw::Painter;
use crate::theme;
use crate::views;

/// Uma brasa (partícula de luz) subindo.
#[derive(Clone, Copy)]
struct Brasa {
    x: f32,
    y: f32,
    /// Velocidade vertical, em pixels de projeto por quadro. Negativa = sobe.
    vy: f32,
    /// Deriva lateral, para não subirem em coluna.
    vx: f32,
    raio: i32,
    vida: f32,
    vida_max: f32,
}

/// Um brilho curto: ponto claro que acende e apaga em poucos quadros.
///
/// Diferente da brasa (que sobe devagar), o brilho é a "faísca" da cena: dá a
/// sensação de que a tela está viva sem ocupar área nem chamar atenção.
#[derive(Clone, Copy)]
struct Brilho {
    x: f32,
    y: f32,
    raio: i32,
    vida: f32,
    vida_max: f32,
}

/// Um risco horizontal de "passou rápido" — sai do centro para os lados.
#[derive(Clone, Copy)]
struct Risco {
    x: f32,
    y: f32,
    comprimento: f32,
    velocidade: f32,
    vida: f32,
}

/// A camada de efeitos inteira.
pub struct Efeitos {
    /// Quadros desde o início. Toda animação deriva daqui, nunca do relógio.
    tick: u32,
    /// Sorteio determinístico (xorshift32). `rand` traria dependência e
    /// reprodutibilidade pior: a semente é nossa.
    semente: u32,
    brasas: Vec<Brasa>,
    brilhos: Vec<Brilho>,
    riscos: Vec<Risco>,
    /// Chamadas de desenho do último quadro, para o diagnóstico.
    desenhos: u32,
    /// Retângulos reutilizados a cada quadro (sem alocar por quadro).
    lote: Vec<(i32, i32, i32, i32)>,
    /// `true` quando a camada está ligada.
    ativo: bool,
    /// Quando não vazio, **só** estes efeitos são desenhados.
    ///
    /// Existe pela mesma razão do `--so` do neon-drift: "o quadro está estranho"
    /// não diz qual efeito, e "ficou pesado" não diz quanto cada um custa. Com a
    /// lista, cada um pode ser medido sozinho no aparelho.
    so: Option<String>,
}

impl Efeitos {
    /// Cria a camada. `ativo` é o que `--efeitos` liga.
    pub fn novo(ativo: bool) -> Self {
        Self {
            tick: 0,
            semente: 0x9e37_79b9,
            brasas: Vec::with_capacity(160),
            brilhos: Vec::with_capacity(32),
            riscos: Vec::with_capacity(24),
            desenhos: 0,
            lote: Vec::with_capacity(256),
            ativo,
            so: std::env::var("GIOCHI_EFEITOS_SO").ok().filter(|v| !v.is_empty()),
        }
    }

    /// `true` quando os efeitos estão ligados.
    pub fn ativo(&self) -> bool {
        self.ativo
    }

    /// `true` quando o efeito entra no quadro.
    fn quero(&self, nome: &str) -> bool {
        match &self.so {
            None => true,
            Some(filtro) => filtro.split(',').any(|p| p.trim() == nome),
        }
    }

    /// Chamadas de desenho do último quadro desenhado.
    pub fn desenhos(&self) -> u32 {
        self.desenhos
    }

    /// Quantas brasas vivas (diagnóstico).
    pub fn brasas(&self) -> usize {
        self.brasas.len()
    }

    /// Quantos brilhos vivos (diagnóstico).
    pub fn brilhos(&self) -> usize {
        self.brilhos.len()
    }

    /// Nome dos efeitos ligados, para o log de abertura.
    pub fn lista() -> &'static str {
        "brasas, brilho, varredura, vinheta, scanlines, halo do foco, rastro, borda"
    }

    // ── sorteio ──────────────────────────────────────────────

    fn sortear(&mut self) -> f32 {
        let mut x = self.semente;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.semente = x;
        (x >> 8) as f32 / 16_777_216.0
    }

    fn entre(&mut self, a: f32, b: f32) -> f32 {
        a + self.sortear() * (b - a)
    }

    // ── avanço ───────────────────────────────────────────────

    /// Adianta a animação **sem desenhar**.
    ///
    /// É o que torna `--dump-frame --efeitos` reprodutível: o caminho de captura
    /// avança um número fixo de quadros antes do desenho final, então o BMP não
    /// depende de quanto tempo o processo levou para abrir a janela.
    pub fn avancar(&mut self, quadros: u32, largura: i32, altura: i32, cor: Color) {
        for _ in 0..quadros {
            self.tick = self.tick.wrapping_add(1);
            self.passo(largura, altura, cor);
        }
    }

    /// Enche a tela de brasas de uma vez.
    ///
    /// Sem isto, `--dump-frame` mostrava brasas só no rodapé: elas nascem na
    /// base e sobem 0,25..1,1 px por quadro, então em 90 quadros andaram menos de
    /// 100 px. O efeito existia e o quadro não o mostrava — que é a mesma
    /// armadilha do `--teclas` que imprimia estado sem ter mudado nada.
    pub fn semear(&mut self, largura: i32, altura: i32) {
        let (l, a) = (largura.max(1) as f32, altura.max(1) as f32);
        for _ in 0..110 {
            let vida = self.entre(60.0, 260.0);
            let x = self.entre(0.0, l);
            let y = self.entre(0.0, a);
            let vy = -self.entre(0.25, 1.1);
            let vx = self.entre(-0.25, 0.25);
            let raio = self.entre(1.0, 3.6).round() as i32;
            self.brasas.push(Brasa {
                x,
                y,
                vy,
                vx,
                raio,
                vida,
                vida_max: vida,
            });
        }
    }

    /// Um quadro de simulação dos efeitos (depende do tamanho: as brasas nascem
    /// na base da tela).
    fn passo(&mut self, largura: i32, altura: i32, cor: Color) {
        let _ = cor;
        let (l, a) = (largura.max(1) as f32, altura.max(1) as f32);

        // ── brasas ───────────────────────────────────────────
        for b in self.brasas.iter_mut() {
            b.vida -= 1.0;
            b.x += b.vx;
            b.y += b.vy;
            // Some ao encostar no topo da área de conteúdo, senão a brasa
            // atravessa a barra superior e "suja" um lugar onde ela não existe.
            if b.y < -8.0 {
                b.vida = 0.0;
            }
        }
        self.brasas.retain(|b| b.vida > 0.0);

        // Reposição: uma brasa por quadro enquanto houver espaço. Com 90 no
        // teto e vida média de ~150 quadros, o regime é ~35 vivas — barato o
        // bastante para caber num quadro de 640×480 sem custar pixel.
        if self.brasas.len() < 130 {
            let x = self.entre(0.0, l);
            let vy = -self.entre(0.25, 1.1);
            let vx = self.entre(-0.25, 0.25);
            let raio = self.entre(1.0, 3.6).round() as i32;
            let vida = self.entre(110.0, 260.0);
            let y = a + self.entre(0.0, 40.0);
            self.brasas.push(Brasa {
                x,
                y,
                vy,
                vx,
                raio,
                vida,
                vida_max: vida,
            });
        }

        // ── brilhos ──────────────────────────────────────────
        for b in self.brilhos.iter_mut() {
            b.vida -= 1.0;
        }
        self.brilhos.retain(|b| b.vida > 0.0);
        // Um brilho a cada 6 quadros: frequente o bastante para a tela nunca
        // ficar parada, raro o bastante para não competir com o conteúdo.
        if self.tick % 3 == 0 && self.brilhos.len() < 40 {
            let x = self.entre(0.0, l);
            let y = self.entre(a * 0.05, a * 0.95);
            let raio = self.entre(1.0, 2.0).round() as i32;
            let vida = self.entre(18.0, 46.0);
            self.brilhos.push(Brilho {
                x,
                y,
                raio,
                vida,
                vida_max: vida,
            });
        }

        // ── riscos de comando ────────────────────────────────
        for r in self.riscos.iter_mut() {
            r.vida -= 1.0;
            r.x += r.velocidade;
            r.comprimento *= 0.94;
        }
        self.riscos.retain(|r| r.vida > 0.0);
    }

    /// Dispara uma rajada de riscos: a resposta visual a um comando.
    ///
    /// Chamado pelo `--teclas` e por quem tiver o comando na mão. Não é
    /// detectado lendo o estado — quem sabe que uma tecla chegou é quem a
    /// processou, e adivinhar comparando estados erraria em `Back` sobre
    /// overlay, que muda de tela sem mudar o índice.
    pub fn rajada(&mut self, largura: i32, altura: i32) {
        if !self.ativo {
            return;
        }
        let (l, a) = (largura as f32, altura as f32);
        for _ in 0..10 {
            let y = self.entre(a * 0.18, a * 0.86);
            let para_direita = self.sortear() > 0.5;
            let x = if para_direita {
                self.entre(-l * 0.1, l * 0.6)
            } else {
                self.entre(l * 0.4, l * 1.1)
            };
            let velocidade = if para_direita {
                self.entre(14.0, 34.0)
            } else {
                -self.entre(14.0, 34.0)
            };
            let comprimento = self.entre(20.0, 90.0);
            self.riscos.push(Risco {
                x,
                y,
                comprimento,
                velocidade,
                vida: 14.0,
            });
        }
    }

    // ── desenho ──────────────────────────────────────────────

    /// Desenha a camada inteira. Sem `--efeitos`, não toca em nada.
    pub fn desenhar(&mut self, p: &mut Painter, state: &State, layout: &views::Layout) {
        if !self.ativo {
            self.desenhos = 0;
            return;
        }
        let antes = p.desenhos();
        let (largura, altura) = p.logical_size();
        let (l, a) = (largura as i32, altura as i32);
        let cor = destaque(state);

        self.tick = self.tick.wrapping_add(1);
        self.passo(l, a, cor);

        // Da camada mais profunda para a mais superficial: vinheta antes de
        // tudo (ela escurece o fundo), scanlines depois (elas são textura) e
        // brasas por cima (elas são luz).
        if self.quero("vinheta") {
            self.vinheta(p, l, a);
        }
        if self.quero("scanlines") {
            self.scanlines(p, l, a);
        }
        if self.quero("varredura") {
            self.varredura(p, l, a, cor);
        }
        if self.quero("brasas") {
            self.brasas_subindo(p, l, a, cor);
        }
        if self.quero("brilho") {
            self.brilhos_piscando(p, cor);
        }
        if self.quero("rastro") {
            self.riscos_de_comando(p, cor);
        }
        if self.quero("halo") {
            self.halo_do_foco(p, layout, state, cor);
        }
        if self.quero("borda") {
            self.borda_pulsante(p, l, a, cor);
        }

        // O número que importa é o de chamadas **ao backend**, medido onde elas
        // acontecem. Contar aqui dentro media a intenção: uma mutação que
        // desfazia a batelada continuava imprimindo "25 desenhos".
        self.desenhos = p.desenhos().saturating_sub(antes);
    }

    /// Vinheta: escurece o topo e a base em bandas.
    ///
    /// Bandas e não gradiente por pixel porque o custo é por **área em tela**:
    /// 10 bandas de 4 % da altura custam 20 % do quadro, um degradê real custaria
    /// 100 %. E bandas de alfa **fixo** deixam o desenho sair em 5 chamadas em
    /// vez de uma por banda: `fill_rects` recebe uma cor para o lote.
    fn vinheta(&mut self, p: &mut Painter, l: i32, a: i32) {
        const ALFAS: [u8; 5] = [14, 11, 8, 5, 2];
        let faixa = (a / 30).max(2);
        for (i, alfa) in ALFAS.iter().enumerate() {
            let y = i as i32 * faixa;
            self.lote.clear();
            self.lote.push((0, y, l, faixa));
            self.lote.push((0, a - y - faixa, l, faixa));
            p.fill_rects(&self.lote, theme::with_alpha(Color::RGB(0, 0, 0), *alfa));
        }
    }

    /// Scanlines horizontais: a textura de CRT, em **um** lote.
    ///
    /// O passo acompanha o painel: 3 px num 640×480 dá 160 linhas; num 1280×720
    /// daria 240. Fixar 2 px enchia a tela de linhas e apagava o texto num
    /// painel pequeno.
    fn scanlines(&mut self, p: &mut Painter, l: i32, a: i32) {
        // Passo 2 é o período que **briga** com a grade do painel: a 640×480 a
        // linha cai ora no pixel, ora entre dois, e o resultado cintila quando a
        // tela se mexe. E é a 1/2 da tela em área: medido no X35H, a versão de
        // passo 3 custava +1,4 ms — o efeito mais caro da camada, pelo efeito
        // menos legível num painel de LCD. Com passo 4 a área cai para 1/4.
        let passo = (a / 120).max(4);
        self.lote.clear();
        let mut y = 0;
        while y < a {
            self.lote.push((0, y, l, 1));
            y += passo;
        }
        p.fill_rects(&self.lote, theme::with_alpha(Color::RGB(0, 0, 0), 18));
    }

    /// Varredura: uma banda de luz descendo, uma vez a cada ~4 s.
    ///
    /// Três sub-bandas de alfa crescente no meio: sem elas a borda é dura e lê
    /// como retângulo, não como luz. Três e não cinco porque cada alfa distinto é
    /// uma chamada, e a diferença visual entre 3 e 5 não se vê num painel de
    /// 640×480.
    fn varredura(&mut self, p: &mut Painter, l: i32, a: i32, cor: Color) {
        let periodo = 300u32;
        let fase = (self.tick % periodo) as f32 / periodo as f32;
        let altura_banda = (a / 9).max(12);
        // Começa acima do topo e sai abaixo da base: a banda entra e sai.
        let y0 = (fase * (a as f32 + altura_banda as f32 * 2.0)) as i32 - altura_banda * 2;
        const ALFAS: [u8; 3] = [24, 42, 14];
        let parte = (altura_banda / 3).max(1);
        for (i, alfa) in ALFAS.iter().enumerate() {
            let y = y0 + i as i32 * parte;
            if y + parte < 0 || y > a {
                continue;
            }
            self.lote.clear();
            self.lote.push((0, y, l, parte + 1));
            p.fill_rects(&self.lote, theme::with_alpha(cor, *alfa));
        }
    }

    /// Brasas: partículas de luz subindo, na cor do jogo em foco.
    ///
    /// Agrupadas em **faixas de alfa** e desenhadas com um `fill_circles` por
    /// faixa. Uma chamada por brasa custava 129 chamadas por quadro — medido no
    /// X35H, +2,9 ms. Oito faixas dão 8 chamadas e a diferença de alfa entre duas
    /// brasas vizinhas é invisível.
    fn brasas_subindo(&mut self, p: &mut Painter, l: i32, a: i32, cor: Color) {
        let _ = (l, a);
        let brasas = std::mem::take(&mut self.brasas);
        // Índice 0..7, 32 de alfa por faixa.
        let mut faixas: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); 8];
        for b in &brasas {
            let vida = (b.vida / b.vida_max).clamp(0.0, 1.0);
            // Entra e sai: sem o pico no meio, a brasa aparece e some de repente.
            let pico = (vida * 3.0).min(1.0) * (1.0 - (vida - 0.7).max(0.0) / 0.3);
            let alfa = (pico.clamp(0.0, 1.0) * 252.0) as u8;
            if alfa < 6 {
                continue;
            }
            let raio = ((b.raio as f32) * (0.5 + vida * 0.9)).round() as i32;
            let faixa = (alfa as usize / 32).min(7);
            faixas[faixa].push((b.x as i32, b.y as i32, raio.max(1)));
        }
        for (i, lista) in faixas.iter().enumerate() {
            if lista.is_empty() {
                continue;
            }
            // Meio da faixa: 16, 48, 80…
            let alfa = (i as u8 * 32 + 16).min(255);
            p.fill_circles(lista, theme::with_alpha(cor, alfa));
        }
        self.brasas = brasas;
    }

    /// Brilhos: estrelinhas de quatro pontas que acendem e apagam.
    ///
    /// Não é um círculo de 2 px: medido, um círculo desse tamanho mudava **84
    /// pixels** do quadro e a camada parecia não ter o efeito. Duas hastes
    /// cruzadas leem como brilho e cobrem 5× mais área, custando dois retângulos
    /// em vez de uma varredura por linha.
    fn brilhos_piscando(&mut self, p: &mut Painter, cor: Color) {
        let brilhos = std::mem::take(&mut self.brilhos);
        // Cor quase branca com um toque do destaque: brilho na cor do jogo lê
        // como mancha, e branco puro lê como defeito de pixel.
        let quase_branco = theme::mix(cor, Color::RGB(255, 255, 255), 0.55);
        let mut faixas: [Vec<(i32, i32, i32, i32)>; 8] = Default::default();
        for b in &brilhos {
            let vida = (b.vida / b.vida_max).clamp(0.0, 1.0);
            // Triangular: acende no primeiro terço e apaga no resto.
            let forca = if vida > 0.66 {
                (1.0 - vida) * 3.0
            } else {
                vida * 1.5
            };
            let alfa = (forca.clamp(0.0, 1.0) * 252.0) as u8;
            if alfa < 10 {
                continue;
            }
            let (x, y) = (b.x as i32, b.y as i32);
            let r = b.raio.max(2);
            let faixa = (alfa as usize / 32).min(7);
            // Haste longa e haste curta: uma cruz de braços iguais parece um "+"
            // de sinalização; com o braço horizontal menor ela lê como brilho.
            faixas[faixa].push((x - r, y, r * 2 + 1, 1));
            faixas[faixa].push((x, y - r * 2, 1, r * 4 + 1));
        }
        for (i, lista) in faixas.iter().enumerate() {
            if lista.is_empty() {
                continue;
            }
            let alfa = (i as u8 * 32 + 16).min(255);
            p.fill_rects(lista, theme::with_alpha(quase_branco, alfa));
        }
        self.brilhos = brilhos;
    }

    /// Riscos de comando: a resposta imediata a uma tecla.
    fn riscos_de_comando(&mut self, p: &mut Painter, cor: Color) {
        let riscos = std::mem::take(&mut self.riscos);
        for r in &riscos {
            let alfa = ((r.vida / 14.0).clamp(0.0, 1.0) * 170.0) as u8;
            if alfa < 8 {
                continue;
            }
            let x0 = r.x;
            let x1 = r.x + r.comprimento * r.velocidade.signum();
            let y = r.y as i32;
            p.line(
                (x0 as i32, y),
                (x1 as i32, y),
                theme::with_alpha(cor, alfa),
            );
        }
        self.riscos = riscos;
    }

    /// Halo pulsante em volta do item em foco.
    ///
    /// A geometria vem de `layout.hits` — o retângulo que a tela **publicou** ao
    /// desenhar. Recalcular a posição aqui seria a segunda verdade que já
    /// desalinhou o carrossel neste projeto; ler o que a tela publicou é a mesma
    /// regra da `grade_biblioteca`.
    fn halo_do_foco(
        &mut self,
        p: &mut Painter,
        layout: &views::Layout,
        state: &State,
        cor: Color,
    ) {
        let Some(rect) = retangulo_do_foco(layout, state) else {
            return;
        };
        let pulso = 0.55 + 0.45 * ((self.tick as f32) * 0.10).sin();
        // Faixas **preenchidas** em volta do cartão, não `stroke_round_rect`.
        //
        // O `stroke_round_rect` desenha 4 arestas e 4 arcos; cada arco é uma
        // polilinha de ~30 `draw_line`, e cada linha é uma chamada de backend.
        // Medido no X35H: **+6,9 ms por quadro** para 3 contornos — o efeito
        // mais caro de todos, por uma margem de 4×. Quatro bandas por camada, num
        // `fill_rects` por camada, custam uma fração e leem igual: é luz difusa,
        // não uma borda precisa.
        let (x, y, w, h) = (rect[0], rect[1], rect[2], rect[3]);
        for (delta, alfa) in [(10, 26.0f32), (6, 50.0), (3, 96.0)] {
            let a = (alfa * pulso) as u8;
            self.lote.clear();
            self.lote.push((x - delta, y - delta, w + delta * 2, delta));
            self.lote.push((x - delta, y + h, w + delta * 2, delta));
            self.lote.push((x - delta, y, delta, h));
            self.lote.push((x + w, y, delta, h));
            p.fill_rects(&self.lote, theme::with_alpha(cor, a));
        }
    }

    /// Tira de luz na borda do painel, respirando.
    ///
    /// Quatro retângulos de 2 px: mostra que a tela ocupa o painel inteiro e dá
    /// o "ligado" que uma UI chapada não tem.
    fn borda_pulsante(&mut self, p: &mut Painter, l: i32, a: i32, cor: Color) {
        let pulso = 0.5 + 0.5 * ((self.tick as f32) * 0.055).sin();
        let alfa = (55.0 + 90.0 * pulso) as u8;
        let esp = (a / 300).max(2);
        self.lote.clear();
        self.lote.push((0, 0, l, esp));
        self.lote.push((0, a - esp, l, esp));
        self.lote.push((0, 0, esp, a));
        self.lote.push((l - esp, 0, esp, a));
        p.fill_rects(&self.lote, theme::with_alpha(cor, alfa));
    }

}

/// Cor de destaque do quadro: a do jogo em foco na Home, o ACCENT nas outras.
fn destaque(state: &State) -> Color {
    if state.tab == Tab::Home {
        if let Some(game) = state.focused_game() {
            return theme::game_accent(game.id);
        }
    }
    theme::ACCENT
}

/// Converte a zona em foco no `Hit` correspondente, e o índice em posição.
///
/// A tabela existe porque o núcleo guarda **zona + índice** e a tela publica
/// **retângulos + tipo de alvo**; alguém tem de casar os dois vocabulários, e
/// casar aqui é melhor que fazer cada tela publicar o retângulo de foco (seria
/// uma linha nova em oito telas, para uma camada opcional).
fn alvo_em_foco(zone: Zone, indice: usize) -> views::Hit {
    match zone {
        Zone::HomeCarousel => views::Hit::Carousel(indice),
        Zone::LibraryFilters => views::Hit::Filter(
            giochi_launcher_core::state::Filter::ALL
                [indice.min(giochi_launcher_core::state::Filter::ALL.len() - 1)],
        ),
        Zone::LibraryGrid => views::Hit::Game(indice),
        Zone::AppsGrid => views::Hit::App(indice),
        Zone::SettingsSidebar => views::Hit::SettingsCat(indice),
        Zone::SettingsContent => views::Hit::SettingsRow(indice),
        Zone::WifiNetworks => views::Hit::WifiNet(indice),
        Zone::PasswordKeys => views::Hit::PasswordKey(indice),
        Zone::BtDevices => views::Hit::BtDevice(indice),
        Zone::SearchKeyboard => views::Hit::Key(indice),
        Zone::SearchResults => views::Hit::Result(indice),
        Zone::QuickSettings => views::Hit::QsRow(indice),
        Zone::GameDetailActions => views::Hit::DetailButton(indice),
        Zone::PowerActions => views::Hit::PowerButton(indice),
    }
}

/// Retângulo publicado pela tela para o item em foco, se houver.
///
/// O primeiro casamento é por igualdade. Nas grades paginadas o índice é
/// **global** e o retângulo é da página, então há um segundo passo que aceita
/// qualquer retângulo do mesmo tipo cuja posição na página seja a do índice —
/// sem ele o halo sumia justamente na página 2.
fn retangulo_do_foco(layout: &views::Layout, state: &State) -> Option<[i32; 4]> {
    let indice = state.focus.index;
    let alvo = alvo_em_foco(state.focus.zone, indice);
    let achado = layout
        .hits
        .iter()
        .find(|(_, hit)| *hit == alvo)
        .or_else(|| {
            let pagina = if state.focus.zone == Zone::LibraryGrid {
                page_size(&state.grade_biblioteca)
            } else if state.focus.zone == Zone::AppsGrid {
                page_size(&state.grade_apps)
            } else {
                return None;
            };
            let posicao = indice.checked_rem(pagina)?;
            layout.hits.iter().find(|(_, hit)| {
                mesmo_tipo(*hit, alvo) && indice_do_hit(*hit) == Some(posicao)
            })
        })?;
    let r = achado.0;
    Some([r.x(), r.y(), r.width() as i32, r.height() as i32])
}

fn page_size(grade: &giochi_launcher_core::Grade) -> usize {
    grade.por_pagina.max(1)
}

fn indice_do_hit(hit: views::Hit) -> Option<usize> {
    match hit {
        views::Hit::Carousel(i)
        | views::Hit::Game(i)
        | views::Hit::App(i)
        | views::Hit::SettingsCat(i)
        | views::Hit::SettingsRow(i)
        | views::Hit::WifiNet(i)
        | views::Hit::PasswordKey(i)
        | views::Hit::BtDevice(i)
        | views::Hit::Key(i)
        | views::Hit::Result(i)
        | views::Hit::QsRow(i)
        | views::Hit::DetailButton(i)
        | views::Hit::PowerButton(i) => Some(i),
        _ => None,
    }
}

fn mesmo_tipo(a: views::Hit, b: views::Hit) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

#[cfg(test)]
mod testes {
    use super::*;
    use giochi_launcher_core::state::{App as AppEntry, Game, Filter};
    use sdl2::rect::Rect;

    fn estado_home() -> State {
        let mut s = State::new(
            vec![
                Game::new(0, "Um", "Ação", 0.0, "nunca", true),
                Game::new(1, "Dois", "Arcade", 0.0, "nunca", false),
            ],
            vec![AppEntry::new("a", "A", "x", "Mídia")],
        );
        s.tab = Tab::Home;
        s
    }

    fn layout_com_alvos() -> views::Layout {
        let mut l = views::Layout::default();
        l.hits.push((Rect::new(10, 20, 100, 50), views::Hit::Carousel(0)));
        l.hits.push((Rect::new(120, 20, 100, 50), views::Hit::Carousel(1)));
        l.hits.push((Rect::new(5, 5, 40, 40), views::Hit::Game(0)));
        l.hits.push((Rect::new(50, 5, 40, 40), views::Hit::Game(1)));
        l
    }

    #[test]
    fn desligado_nao_muda_estado_nem_conta_desenho() {
        let mut e = Efeitos::novo(false);
        assert!(!e.ativo());
        // Sem `desenhar` a camada não faz nada; o que dá para provar sem GL é
        // que ela se declara inativa e não acumula partícula.
        e.rajada(640, 480);
        e.avancar(10, 640, 480, theme::ACCENT);
        assert_eq!(e.desenhos(), 0);
        assert!(e.riscos.is_empty(), "rajada desligada não pode criar risco");
    }

    #[test]
    fn o_sorteio_e_deterministico() {
        // Sem isso `--dump-frame` não seria reprodutível e não haveria teste de
        // regressão de imagem para a camada.
        let a: Vec<f32> = {
            let mut e = Efeitos::novo(true);
            (0..10).map(|_| e.sortear()).collect()
        };
        let b: Vec<f32> = {
            let mut e = Efeitos::novo(true);
            (0..10).map(|_| e.sortear()).collect()
        };
        assert_eq!(a, b);
        assert!(a.windows(2).any(|w| w[0] != w[1]), "sequência constante");
        assert!(a.iter().all(|v| (0.0..1.0).contains(v)), "fora de [0,1)");
    }

    #[test]
    fn semear_espalha_brasas_pela_altura_toda() {
        // Antes do `semear` as brasas nasciam só na base: em 90 quadros andavam
        // menos de 100 px e o `--dump-frame` mostrava um rodapé com partículas e
        // uma tela vazia em cima.
        let mut e = Efeitos::novo(true);
        e.semear(640, 480);
        assert!(e.brasas() >= 100, "poucas brasas: {}", e.brasas());
        let em_cima = e.brasas.iter().filter(|b| b.y < 240.0).count();
        let embaixo = e.brasas.iter().filter(|b| b.y >= 240.0).count();
        assert!(em_cima > 20, "quase nada na metade de cima: {em_cima}");
        assert!(embaixo > 20, "quase nada na metade de baixo: {embaixo}");
    }

    #[test]
    fn avancar_nao_estoura_o_teto_de_brasas() {
        let mut e = Efeitos::novo(true);
        e.semear(640, 480);
        for _ in 0..2000 {
            e.avancar(1, 640, 480, theme::ACCENT);
        }
        assert!(e.brasas() <= 130, "sem teto: {}", e.brasas());
    }

    #[test]
    fn a_rajada_cria_riscos_e_eles_morrem() {
        let mut e = Efeitos::novo(true);
        e.rajada(640, 480);
        assert_eq!(e.riscos.len(), 10, "a rajada é de 10 riscos");
        for _ in 0..20 {
            e.avancar(1, 640, 480, theme::ACCENT);
        }
        assert!(e.riscos.is_empty(), "risco sobreviveu à vida de 14 quadros");
    }

    #[test]
    fn a_zona_em_foco_vira_o_alvo_publicado() {
        // O casamento entre o vocabulário do núcleo (zona + índice) e o da tela
        // (retângulo + tipo de alvo) é o que permite o halo ler a geometria
        // publicada em vez de recalculá-la.
        assert_eq!(
            alvo_em_foco(Zone::HomeCarousel, 1),
            views::Hit::Carousel(1)
        );
        assert_eq!(alvo_em_foco(Zone::LibraryGrid, 3), views::Hit::Game(3));
        assert_eq!(alvo_em_foco(Zone::AppsGrid, 2), views::Hit::App(2));
        assert_eq!(
            alvo_em_foco(Zone::LibraryFilters, 0),
            views::Hit::Filter(Filter::All)
        );
    }

    #[test]
    fn o_halo_le_o_retangulo_que_a_tela_publicou() {
        let mut s = estado_home();
        s.focus.zone = Zone::HomeCarousel;
        s.focus.index = 1;
        let r = retangulo_do_foco(&layout_com_alvos(), &s).expect("devia achar o cartão 1");
        assert_eq!(r, [120, 20, 100, 50]);
    }

    #[test]
    fn o_halo_some_quando_o_alvo_nao_foi_publicado() {
        // Greide vazia (ou overlay aberto) não publica retângulo: o halo tem de
        // desaparecer, e não cair no cartão 0 — halo no lugar errado é pior que
        // halo nenhum.
        let mut s = estado_home();
        s.focus.zone = Zone::HomeCarousel;
        s.focus.index = 7;
        assert!(retangulo_do_foco(&layout_com_alvos(), &s).is_none());
    }

    #[test]
    fn zona_sem_retangulo_nao_estoura_o_indice() {
        // `SettingsContent` e as listas usam índice que pode estar fora do que a
        // tela publicou (categoria sem linhas). O `checked_rem` e o `min` são o
        // que impede o pânico.
        let s = estado_home();
        let alvo = alvo_em_foco(Zone::LibraryFilters, 99);
        assert!(matches!(alvo, views::Hit::Filter(_)));
    }

    #[test]
    fn indices_de_alvo_mapeiam_ida_e_volta() {
        for i in 0..4 {
            assert_eq!(indice_do_hit(views::Hit::Game(i)), Some(i));
            assert_eq!(indice_do_hit(views::Hit::Carousel(i)), Some(i));
            assert_eq!(indice_do_hit(views::Hit::Key(i)), Some(i));
        }
        // Alvos sem índice não podem devolver um número inventado.
        assert_eq!(indice_do_hit(views::Hit::PowerTap), None);
        assert_eq!(indice_do_hit(views::Hit::Tab(Tab::Home)), None);
    }

    #[test]
    fn mesmo_tipo_distingue_variantes() {
        assert!(mesmo_tipo(views::Hit::Game(0), views::Hit::Game(5)));
        assert!(!mesmo_tipo(views::Hit::Game(0), views::Hit::App(0)));
    }
}
