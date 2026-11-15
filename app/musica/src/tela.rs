//! O desenho da tela do tocador.
//!
//! Três telas e um rodapé: o carrossel de capas, a barra de transporte, a faixa
//! de dicas — e, quando não há música, uma tela que **diz por quê** e onde o app
//! procurou.
//!
//! ## O que este arquivo não faz
//!
//! Não decide nada: quem escolhe a faixa é o reducer do núcleo, quem toca é o
//! mixer. Aqui só se transforma `Estado` em pixels, com o layout vindo do
//! núcleo (`Layout`), a geometria vinda do núcleo (`carrossel::geometria`) e a
//! capa vinda do núcleo (`capa::capa_de` + `arte::pintar`). Um número de pixel
//! escrito à mão aqui seria a segunda verdade que já desalinhou o carrossel do
//! foco no launcher.

use std::collections::{HashMap, VecDeque};

use giochi_engine::TextureHandle;
use giochi_musica_core::carrossel::geometria;
use giochi_musica_core::capa::capa_de;
use giochi_musica_core::faixa::{extensoes_aceitas, Faixa};
use giochi_musica_core::layout::Layout;
use giochi_musica_core::texto::{dicas, encurtar, plural};
use giochi_musica_core::{Estado, EstadoPlay};

use crate::arte;
use crate::audio::Som;
use crate::pintor::Pintor;
use crate::theme;

/// Quantas capas cabem de cada lado do foco.
const RAIO: usize = 3;
/// Quantas texturas de capa ficam em cache.
///
/// 16 × 96×96×4 = 590 KB de VRAM. Sem teto, percorrer 500 faixas deixaria 500
/// texturas abertas (18 MB) num aparelho de 1 GB — e o `destroy_texture` nunca
/// seria chamado.
const CACHE_CAPAS: usize = 16;

pub struct Telas {
    capas: HashMap<usize, TextureHandle>,
    ordem: VecDeque<usize>,
    pub quadro: u64,
}

impl Default for Telas {
    fn default() -> Self {
        Telas::nova()
    }
}

impl Telas {
    pub fn nova() -> Telas {
        Telas {
            capas: HashMap::new(),
            ordem: VecDeque::new(),
            quadro: 0,
        }
    }

    /// Solta todas as texturas de capa.
    pub fn liberar(&mut self, pintor: &mut Pintor) {
        for (_, textura) in self.capas.drain() {
            pintor.liberar_textura(textura);
        }
        self.ordem.clear();
    }

    fn capa(&mut self, pintor: &mut Pintor, indice: usize, faixa: &Faixa) -> Option<TextureHandle> {
        if let Some(textura) = self.capas.get(&indice) {
            // Move para o fim da fila: o mais antigo sai primeiro.
            if let Some(pos) = self.ordem.iter().position(|i| *i == indice) {
                self.ordem.remove(pos);
            }
            self.ordem.push_back(indice);
            return Some(*textura);
        }

        let pixels = arte::pintar(&capa_de(&faixa.arquivo));
        let textura = pintor
            .criar_textura(arte::LADO, arte::LADO, &pixels)
            .map_err(|e| eprintln!("aviso: capa de {} falhou: {e}", faixa.titulo))
            .ok()?;

        self.capas.insert(indice, textura);
        self.ordem.push_back(indice);
        while self.ordem.len() > CACHE_CAPAS {
            if let Some(velho) = self.ordem.pop_front() {
                if let Some(t) = self.capas.remove(&velho) {
                    pintor.liberar_textura(t);
                }
            }
        }
        Some(textura)
    }

    /// Desenha um quadro completo.
    pub fn desenhar(
        &mut self,
        pintor: &mut Pintor,
        estado: &Estado,
        som: &Som,
        recado: Option<&str>,
    ) {
        let (w, h) = pintor.projeto();
        let l = Layout::de(w, h);
        self.quadro = self.quadro.wrapping_add(1);

        pintor.clear(theme::bg());
        fundo(pintor, &l);

        topo(pintor, &l, estado);

        if estado.biblioteca.esta_vazia() {
            tela_vazia(pintor, &l, estado);
        } else {
            self.carrossel(pintor, &l, estado);
        }

        transporte(pintor, &l, estado, som, recado, self.quadro);
        rodape(pintor, &l, estado);
    }

    fn carrossel(&mut self, pintor: &mut Pintor, l: &Layout, estado: &Estado) {
        let mut visiveis = estado.carrossel.visiveis(RAIO);
        // Mais longe primeiro: o foco é desenhado por último e fica na frente.
        visiveis.sort_by_key(|(_, p)| -p.abs());

        let faixas = estado.biblioteca.faixas();

        for (indice, posicao) in visiveis {
            let Some(faixa) = faixas.get(indice) else {
                continue;
            };
            let g = geometria(posicao, l.largura / 2, l.passo);
            let tam = ((l.capa as f32) * g.escala).round() as i32;
            if tam < 8 {
                continue;
            }
            let x = g.x - tam / 2;
            let y = l.centro_y - tam / 2;

            // Fora da tela não se desenha nem se cria textura: uma capa a 200 px
            // da borda custaria 36 KB de upload por quadro para nenhum pixel
            // visível.
            if x + tam <= 0 || x >= l.largura {
                continue;
            }

            if let Some(textura) = self.capa(pintor, indice, faixa) {
                let _ = pintor.desenhar_textura(textura, x, y, tam, tam);
            } else {
                pintor.fill_rect(x, y, tam, tam, theme::panel2());
            }

            // Escurecimento por profundidade: `draw_texture_ex` não aceita
            // tint, e um retângulo preto por cima custa 1 desenho.
            if g.alfa < 255 {
                pintor.fill_rect(x, y, tam, tam, theme::with_alpha(theme::BLACK, 255 - g.alfa));
            }

            if posicao == 0 {
                let e = (l.capa / 64).max(2);
                pintor.rect_border(x - e, y - e, tam + 2 * e, tam + 2 * e, e, theme::accent());
            }

            // Selo de "esta é a que está no mixer": separado do foco, porque
            // foco é escolha e selo é reprodução.
            if estado.indice_tocando() == Some(indice) {
                let raio = (tam / 7).max(9);
                let (cx, cy) = (x + raio + 4, y + raio + 4);
                pintor.fill_rect(cx - raio, cy - raio, raio * 2, raio * 2, theme::ok());
                pintor.icon("play", cx, cy, raio + 2, theme::BLACK);
            }
        }

        // Título e ficha da faixa em foco, logo abaixo das capas.
        let Some(faixa) = estado.faixa_em_foco() else {
            return;
        };
        let y = (l.centro_y + l.capa / 2 + l.altura / 40).min(l.barra_y() - l.fonte_nome as i32 * 2);
        let largura = l.largura - 2 * l.margem;
        let titulo = encurtar(&faixa.titulo, largura, |t| {
            pintor.text_size(t, l.fonte_nome).0
        });
        pintor.text_center(l.largura / 2, y, &titulo, l.fonte_nome, theme::text());

        let ficha = format!("{} · {}", faixa.formato.rotulo(), faixa.duracao_legivel());
        pintor.text_center(l.largura / 2, y + l.fonte_nome as i32 + 4, &ficha, l.fonte_corpo, theme::dim());
    }
}

/// Fundo: gradiente em bandas (uma chamada) e brilho no topo.
fn fundo(pintor: &mut Pintor, l: &Layout) {
    let bandas = 12;
    let altura = (l.altura / bandas).max(1);
    let mut faixas = Vec::with_capacity(bandas as usize);
    for i in 0..bandas {
        faixas.push((0, i * altura, l.largura, altura));
    }
    // Uma única chamada: `draw_rect` é chamada de backend por retângulo, e 12
    // retângulos separados custariam 12 — a lição medida no Star Dust.
    pintor.fill_rects(&faixas, theme::panel());
    pintor.fill_rect(0, 0, l.largura, altura * 2, theme::with_alpha(theme::accent(), 10));
    pintor.fill_rect(0, 0, l.largura, altura, theme::with_alpha(theme::accent(), 8));
}

fn topo(pintor: &mut Pintor, l: &Layout, estado: &Estado) {
    let y = (l.topo - pintor.text_size("Ag", l.fonte_titulo).1) / 2;
    let icone = l.fonte_titulo as i32 + 6;

    pintor.icon("nota", l.margem + icone / 2, l.topo / 2, icone, theme::accent());
    pintor.text_left(
        l.margem + icone + 8,
        y,
        "Música",
        l.fonte_titulo,
        theme::text(),
    );

    // À direita, a pasta e a contagem: o dado que responde "de onde vêm essas
    // músicas?" sem o usuário abrir um menu.
    let pasta = encurtar(estado.biblioteca.pasta(), l.largura / 2, |t| {
        pintor.text_size(t, l.fonte_mini).0
    });
    let altura_mini = pintor.text_size("Ag", l.fonte_mini).1;
    pintor.text_right(
        l.largura - l.margem,
        (l.topo - altura_mini) / 2 - l.fonte_mini as i32 - 2,
        &pasta,
        l.fonte_mini,
        theme::dim(),
    );

    let contagem = match estado.biblioteca.esta_vazia() {
        true => "sem faixas".to_string(),
        false => plural(estado.biblioteca.total(), "faixa", "faixas"),
    };
    pintor.text_right(
        l.largura - l.margem,
        (l.topo - altura_mini) / 2 + l.fonte_mini as i32,
        &contagem,
        l.fonte_mini,
        theme::faint(),
    );

    pintor.fill_rect(0, l.topo - 1, l.largura, 1, theme::line());
}

/// Tela de ausência: o motivo, a pasta e o que fazer.
fn tela_vazia(pintor: &mut Pintor, l: &Layout, estado: &Estado) {
    let centro = l.largura / 2;
    let y0 = l.topo + (l.barra_y() - l.topo) / 3;

    pintor.icon("search", centro, y0, (l.altura / 7).clamp(32, 96), theme::faint());

    let largura = l.largura - 2 * l.margem;
    let mensagem = encurtar(&estado.biblioteca.mensagem(), largura, |t| {
        pintor.text_size(t, l.fonte_nome).0
    });
    let y = y0 + (l.altura / 7).clamp(32, 96) / 2 + l.altura / 30;
    pintor.text_center(centro, y, &mensagem, l.fonte_nome, theme::text());

    let formatos = format!("formatos: {}", extensoes_aceitas());
    let formatos = encurtar(&formatos, largura, |t| pintor.text_size(t, l.fonte_mini).0);
    pintor.text_center(centro, y + l.fonte_nome as i32 + 6, &formatos, l.fonte_mini, theme::dim());

    let dica = "copie as músicas para essa pasta · SELECT Recarregar";
    let dica = encurtar(dica, largura, |t| pintor.text_size(t, l.fonte_corpo).0);
    pintor.text_center(
        centro,
        y + l.fonte_nome as i32 + l.fonte_mini as i32 + 14,
        &dica,
        l.fonte_corpo,
        theme::faint(),
    );
}

fn transporte(
    pintor: &mut Pintor,
    l: &Layout,
    estado: &Estado,
    som: &Som,
    recado: Option<&str>,
    quadro: u64,
) {
    let y0 = l.barra_y();
    pintor.fill_rect(0, y0, l.largura, l.barra, theme::panel2());
    pintor.fill_rect(0, y0, l.largura, 1, theme::line2());

    let meio = y0 + l.barra / 2;
    let altura_texto = pintor.text_size("Ag", l.fonte_corpo).1;
    let base = meio - altura_texto / 2;

    // Estado da reprodução, com a cor dizendo o mesmo que a palavra.
    let (rotulo, cor) = match estado.transporte.estado {
        EstadoPlay::Tocando => ("Tocando", theme::ok()),
        EstadoPlay::Pausado => ("Pausado", theme::warn()),
        EstadoPlay::Parado => ("Parado", theme::dim()),
    };
    pintor.fill_rect(l.margem, meio - altura_texto / 2, 3, altura_texto, cor);
    pintor.text_left(l.margem + 10, base, rotulo, l.fonte_corpo, cor);

    let x_estado = l.margem + 14 + pintor.text_size(rotulo, l.fonte_corpo).0;

    // Equalizador: só quando há som de verdade. Quatro retângulos num lote.
    if estado.transporte.estado.emitindo() && !estado.biblioteca.esta_vazia() {
        let semente = giochi_musica_core::capa::fnv1a64(
            estado
                .faixa_tocando()
                .map(|f| f.caminho.as_str())
                .unwrap_or("")
                .as_bytes(),
        );
        let maximo = (l.fonte_corpo as i32 * 2 / 3).max(3);
        let largura_barra = 3;
        let mut barras = Vec::with_capacity(4);
        for b in 0..4 {
            let altura = arte::altura_do_vu(quadro, b, semente, maximo).max(2);
            let x = x_estado + 10 + b as i32 * (largura_barra + 2);
            barras.push((x, meio - altura / 2, largura_barra, altura));
        }
        pintor.fill_rects(&barras, theme::ok());
    }

    // O que está no mixer — ou o motivo de nada estar.
    let x_texto = x_estado + 40;
    let espaco = (l.largura / 2 - x_texto).max(40);
    let (texto, cor_texto) = match (recado, estado.faixa_tocando()) {
        (Some(erro), _) => (erro.to_string(), theme::warn()),
        (None, Some(faixa)) => (faixa.titulo.clone(), theme::text()),
        (None, None) => match som.aviso() {
            Some(motivo) => (motivo.to_string(), theme::warn()),
            None => ("—".to_string(), theme::faint()),
        },
    };
    let texto = encurtar(&texto, espaco, |t| pintor.text_size(t, l.fonte_corpo).0);
    pintor.text_left(x_texto, base, &texto, l.fonte_corpo, cor_texto);

    // Volume, à direita: rótulo, barra e porcentagem.
    let porcento = format!("{}%", estado.transporte.volume);
    let largura_porcento = pintor.text_size(&porcento, l.fonte_mini).0;
    let barra_w = (l.largura / 6).clamp(40, 160);
    let barra_x = l.largura - l.margem - largura_porcento - 8 - barra_w;

    pintor.icon("vol", barra_x - 12, meio, l.fonte_corpo as i32 + 2, theme::dim());
    let altura_barra = (l.fonte_mini as i32 / 2).max(4);
    pintor.fill_rect(barra_x, meio - altura_barra / 2, barra_w, altura_barra, theme::card());
    let preenchido = (barra_w as f32 * estado.transporte.fracao_do_volume()).round() as i32;
    if preenchido > 0 {
        pintor.fill_rect(barra_x, meio - altura_barra / 2, preenchido, altura_barra, theme::accent());
    }
    pintor.text_right(
        l.largura - l.margem,
        base,
        &porcento,
        l.fonte_mini,
        theme::dim(),
    );
}

fn rodape(pintor: &mut Pintor, l: &Layout, estado: &Estado) {
    let y0 = l.rodape_y();
    pintor.fill_rect(0, y0, l.largura, l.rodape, theme::panel());
    pintor.fill_rect(0, y0, l.largura, 1, theme::line());

    let mut lista = dicas(l.largura - 2 * l.margem, |t| pintor.text_size(t, l.fonte_mini).0);
    // Na tela vazia a única ação que interessa é reler a pasta.
    if estado.biblioteca.esta_vazia() {
        lista = vec![
            giochi_musica_core::Dica { tecla: "SELECT", rotulo: "Recarregar" },
            giochi_musica_core::Dica { tecla: "START", rotulo: "Sair" },
        ];
    }

    let larguras: Vec<i32> = lista
        .iter()
        .map(|d| {
            pintor.text_size(d.tecla, l.fonte_mini).0
                + 4
                + pintor.text_size(d.rotulo, l.fonte_mini).0
        })
        .collect();
    let total: i32 = larguras.iter().sum::<i32>() + 18 * lista.len().saturating_sub(1) as i32;
    let mut x = ((l.largura - total) / 2).max(l.margem);
    let y = y0 + (l.rodape - pintor.text_size("Ag", l.fonte_mini).1) / 2;

    for (dica, largura) in lista.iter().zip(larguras) {
        pintor.text_left(x, y, dica.tecla, l.fonte_mini, theme::accent());
        let x_rotulo = x + pintor.text_size(dica.tecla, l.fonte_mini).0 + 4;
        pintor.text_left(x_rotulo, y, dica.rotulo, l.fonte_mini, theme::dim());
        x += largura + 18;
    }
}
