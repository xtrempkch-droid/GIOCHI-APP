//! Estado e telas da loja.
//!
//! Quatro telas, e a regra que vale em todas: **nada de "pago" sem licença
//! assinada**. A tela de pagamento mostra o código PIX e espera — quem confirma
//! é o servidor, e a confirmação chega como `LicencaAssinada`.
//!
//! # Modo de teste
//!
//! Sem backend configurado, a loja **não inventa** um código PIX: mostrar um
//! "copia e cola" falso é a pior falha possível, porque o usuário tenta pagar.
//! Com `--demo`, a tela entra em modo de teste e diz isso em vermelho na cara.

use giochi_loja_core::catalogo::{Catalogo, Item};
use giochi_loja_core::pagamento::{Estado as EstadoPedido, Pedido, Pix};

use crate::desenho::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    Vitrine,
    Ficha,
    Pagamento,
    Biblioteca,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    Cima,
    Baixo,
    Esquerda,
    Direita,
    Confirmar,
    Voltar,
    Sair,
}

pub struct Estado {
    pub tela: Tela,
    pub selecao: usize,
    pub catalogo: Option<Catalogo>,
    pub pedido: Option<Pedido>,
    pub demo: bool,
    pub mensagem: String,
    /// Licenças já obtidas, por slug. Na vida real viria do disco.
    pub biblioteca: Vec<(String, String)>,
}

impl Estado {
    pub fn novo() -> Self {
        Self {
            tela: Tela::Vitrine,
            selecao: 0,
            catalogo: None,
            pedido: None,
            demo: false,
            mensagem: String::new(),
            biblioteca: Vec::new(),
        }
    }

    pub fn com_catalogo(mut self, c: Catalogo) -> Self {
        self.catalogo = Some(c);
        self
    }

    pub fn demo(mut self) -> Self {
        self.demo = true;
        self
    }

    fn itens(&self) -> &[Item] {
        self.catalogo.as_ref().map(|c| c.itens.as_slice()).unwrap_or(&[])
    }

    fn item_atual(&self) -> Option<&Item> {
        self.itens().get(self.selecao)
    }

    /// Trata uma tecla. Devolve `false` quando é hora de sair.
    pub fn tecla(&mut self, t: Tecla) -> bool {
        match t {
            Tecla::Sair => return false,
            Tecla::Voltar => {
                self.tela = match self.tela {
                    Tela::Vitrine => Tela::Vitrine,
                    Tela::Ficha => Tela::Vitrine,
                    Tela::Pagamento => Tela::Ficha,
                    Tela::Biblioteca => Tela::Vitrine,
                };
                self.mensagem.clear();
            }
            Tecla::Esquerda | Tecla::Direita => {
                // Abas na vitrine; nada nas outras telas.
                if self.tela == Tela::Vitrine {
                    self.tela = Tela::Biblioteca;
                } else if self.tela == Tela::Biblioteca {
                    self.tela = Tela::Vitrine;
                }
            }
            Tecla::Cima => {
                if self.selecao > 0 {
                    self.selecao -= 1;
                }
            }
            Tecla::Baixo => {
                if !self.itens().is_empty() && self.selecao + 1 < self.itens().len() {
                    self.selecao += 1;
                }
            }
            Tecla::Confirmar => self.confirmar(),
        }
        true
    }

    fn confirmar(&mut self) {
        match self.tela {
            Tela::Vitrine => {
                if self.item_atual().is_some() {
                    self.tela = Tela::Ficha;
                    self.mensagem.clear();
                }
            }
            Tela::Ficha => {
                let Some(item) = self.item_atual().cloned() else { return };
                let pedido = Pedido::novo(
                    format!("DEMO-{}", self.selecao + 1),
                    item.slug.clone(),
                    item.preco,
                    // O aparelho de destino: na vida real, a chave pública que o
                    // aparelho gerou. Em demo, um valor fixo e visível.
                    "APARELHO-DE-DEMONSTRACAO",
                    0,
                );
                if !self.demo {
                    // Sem provedor não há pedido. Dizer isso é melhor que
                    // mostrar um código PIX que não existe.
                    self.mensagem = "sem provedor de pagamento configurado — use --demo para testar".into();
                    return;
                }
                let pix = Pix {
                    copia_e_cola: format!(
                        "00020126580014BR.GOV.BCB.PIX0136demonstracao-{}-5204000053039865406{:06}5802BR5909GIOCHI6008BRASILIA62070503***6304DEMO",
                        item.slug, item.preco.centavos
                    ),
                    valor: item.preco,
                    expira_em: 0,
                    recebedor: "Giochi (TESTE)".into(),
                };
                self.pedido = pedido.aguardar(Some(pix), None, u64::MAX).ok();
                self.tela = Tela::Pagamento;
                self.mensagem.clear();
            }
            Tela::Pagamento => {
                // Aqui é onde o usuário de verdade não faz nada: quem confirma
                // é o servidor. Em demo, o botão explica isso.
                self.mensagem = "em produção, a confirmação vem do servidor (webhook do PIX) — \
                                 esta tela não marca como pago sozinha"
                    .into();
            }
            Tela::Biblioteca => {}
        }
    }

    pub fn desenhar(&self, p: &mut Pintor) {
        p.limpar(BG);
        self.desenhar_topo(p);
        match self.tela {
            Tela::Vitrine => self.desenhar_vitrine(p),
            Tela::Ficha => self.desenhar_ficha(p),
            Tela::Pagamento => self.desenhar_pagamento(p),
            Tela::Biblioteca => self.desenhar_biblioteca(p),
        }
        self.desenhar_rodape(p);
    }

    fn desenhar_topo(&self, p: &mut Pintor) {
        p.retangulo(0, 0, p.largura, TOPO_H, PANEL);
        p.linha(0, TOPO_H, p.largura, TOPO_H, LINE);

        p.texto(24, 16, "Giochi", 24, ACCENT, Alinhamento::Esquerda);
        let abas = [("Loja", Tela::Vitrine), ("Biblioteca", Tela::Biblioteca)];
        let mut x = 160;
        for (nome, tela) in abas {
            let ativo = self.tela == tela || (tela == Tela::Vitrine && self.tela != Tela::Biblioteca);
            let cor = if ativo { TEXT } else { DIM };
            let w = p.texto(x, 20, nome, 27, cor, Alinhamento::Esquerda);
            if ativo {
                p.retangulo(x, TOPO_H - 3, w, 3, ACCENT);
            }
            x += w + 40;
        }

        if self.demo {
            // Aviso na cara, não em rodapé cinza: em modo de teste o código PIX
            // da tela seguinte não é pagável, e quem não ler vai tentar pagar.
            let rotulo = "MODO DE TESTE — nenhum pagamento é real";
            let (w, _) = p.text_size(rotulo, 22);
            let x = p.largura - w - 32;
            p.arredondado(x - 10, 14, w + 20, 30, 6, DANGER);
            p.texto(x, 18, rotulo, 22, TEXT, Alinhamento::Esquerda);
        } else if let Some(c) = &self.catalogo {
            let s = format!("{} app(s)", c.itens.len());
            p.texto(p.largura - 24, 20, &s, 22, DIM, Alinhamento::Direita);
        }
    }

    fn desenhar_vitrine(&self, p: &mut Pintor) {
        let y0 = TOPO_H + 24;
        if self.itens().is_empty() {
            p.texto(p.largura / 2, y0 + 40, "catálogo vazio", 30, DIM, Alinhamento::Centro);
            p.texto(
                p.largura / 2,
                y0 + 80,
                "passe --catalogo <arquivo.json> para carregar",
                22,
                DIM,
                Alinhamento::Centro,
            );
            return;
        }

        let linha_h = 96;
        for (i, item) in self.itens().iter().enumerate() {
            let y = y0 + i as i32 * (linha_h + 12);
            if y + linha_h > p.altura - RODAPE_H - 12 {
                break;
            }
            let selecionado = i == self.selecao;
            p.arredondado(24, y, p.largura - 48, linha_h, 12, if selecionado { CARD } else { PANEL });
            if selecionado {
                p.retangulo(24, y + 12, 4, linha_h - 24, ACCENT);
            }

            // "Capa" sem asset: um quadrado com a inicial. Sem arquivo binário
            // para carregar, e continua legível se o catálogo mudar.
            p.arredondado(44, y + 16, 64, 64, 10, PANEL2);
            let inicial = item.titulo.chars().next().unwrap_or('?').to_string();
            p.texto(76, y + 32, &inicial, 34, ACCENT, Alinhamento::Centro);

            p.texto_limitado(128, y + 18, &item.titulo, 28, TEXT, p.largura - 340);
            let ano = if item.ano == 0 { "—".to_string() } else { item.ano.to_string() };
            p.texto(128, y + 58, &ano, 22, DIM, Alinhamento::Esquerda);

            let preco = item.preco.to_string();
            let cor_preco = if item.preco.eh_gratis() { OK } else { TEXT };
            p.texto(p.largura - 48, y + 30, &preco, 26, cor_preco, Alinhamento::Direita);
        }
    }

    fn desenhar_ficha(&self, p: &mut Pintor) {
        let Some(item) = self.item_atual() else { return };
        let y = TOPO_H + 28;

        p.texto(40, y, &item.titulo, 40, TEXT, Alinhamento::Esquerda);
        let ano = if item.ano == 0 { "—".to_string() } else { item.ano.to_string() };
        p.texto(40, y + 56, &format!("{ano}  ·  versão {}", item.versao), 24, DIM, Alinhamento::Esquerda);

        p.arredondado(40, y + 100, 200, 200, 14, PANEL2);
        let inicial = item.titulo.chars().next().unwrap_or('?').to_string();
        p.texto(140, y + 170, &inicial, 90, ACCENT, Alinhamento::Centro);

        let x = 280;
        let largura = p.largura - x - 40;
        p.texto_limitado(x, y + 100, &item.descricao, 24, TEXT, largura);

        let mb = item.tamanho_bytes as f64 / 1_048_576.0;
        p.texto(x, y + 190, &format!("{mb:.1} MB"), 22, DIM, Alinhamento::Esquerda);

        // Botão de compra.
        let rotulo = if item.preco.eh_gratis() { "Baixar".to_string() } else { format!("Comprar · {}", item.preco) };
        let (w, _) = p.text_size(&rotulo, 28);
        let bw = w + 64;
        p.arredondado(x, y + 240, bw, 56, 12, ACCENT);
        p.texto(x + bw / 2, y + 255, &rotulo, 28, BG, Alinhamento::Centro);

        if !self.mensagem.is_empty() {
            p.texto_limitado(x, y + 320, &self.mensagem, 22, WARN, largura);
        }
    }

    fn desenhar_pagamento(&self, p: &mut Pintor) {
        let y = TOPO_H + 28;
        let Some(pedido) = &self.pedido else {
            p.texto(p.largura / 2, y + 60, "sem pedido", 30, DIM, Alinhamento::Centro);
            return;
        };
        p.texto(40, y, "Pagamento", 40, TEXT, Alinhamento::Esquerda);
        p.texto(40, y + 56, &format!("pedido {}", pedido.id), 22, DIM, Alinhamento::Esquerda);

        if let EstadoPedido::Aguardando { pix: Some(pix), .. } = pedido.estado() {
            let valor = pix.valor.to_string();
            p.texto(p.largura - 40, y, &valor, 40, TEXT, Alinhamento::Direita);

            p.arredondado(40, y + 110, p.largura - 80, 90, 12, PANEL);
            p.texto(60, y + 124, "PIX copia e cola", 22, DIM, Alinhamento::Esquerda);
            // O código é longo e precisa ser copiável; mostramos o começo e o
            // tamanho, e a tela de verdade oferece "copiar" (área de
            // transferência é trabalho de plataforma, ainda não ligado).
            let visivel: String = pix.copia_e_cola.chars().take(72).collect();
            p.texto(60, y + 156, &format!("{visivel}…"), 20, TEXT, Alinhamento::Esquerda);
            p.texto(
                60,
                y + 182,
                &format!("{} caracteres · recebedor: {}", pix.copia_e_cola.len(), pix.recebedor),
                20,
                DIM,
                Alinhamento::Esquerda,
            );

            if self.demo {
                p.texto(
                    40,
                    y + 226,
                    "CÓDIGO DE TESTE: não é pagável. Em produção vem do provedor de pagamento.",
                    22,
                    DANGER,
                    Alinhamento::Esquerda,
                );
            }
        } else {
            p.texto(40, y + 120, "sem instrução de pagamento", 24, DIM, Alinhamento::Esquerda);
        }

        let rotulo = "Já paguei";
        let (w, _) = p.text_size(rotulo, 28);
        p.arredondado(40, y + 280, w + 64, 56, 12, PANEL2);
        p.texto(40 + (w + 64) / 2, y + 295, rotulo, 28, TEXT, Alinhamento::Centro);

        if !self.mensagem.is_empty() {
            p.texto_limitado(40, y + 356, &self.mensagem, 20, WARN, p.largura - 80);
        }
    }

    fn desenhar_biblioteca(&self, p: &mut Pintor) {
        let y = TOPO_H + 28;
        p.texto(40, y, "Biblioteca", 40, TEXT, Alinhamento::Esquerda);
        if self.biblioteca.is_empty() {
            p.texto(40, y + 80, "nada comprado ainda", 26, DIM, Alinhamento::Esquerda);
            p.texto(
                40,
                y + 118,
                "o app aparece aqui quando o servidor emitir a licença assinada",
                22,
                DIM,
                Alinhamento::Esquerda,
            );
            return;
        }
        for (i, (slug, estado)) in self.biblioteca.iter().enumerate() {
            let yy = y + 70 + i as i32 * 60;
            p.arredondado(40, yy, p.largura - 80, 48, 10, PANEL);
            p.texto(60, yy + 12, slug, 24, TEXT, Alinhamento::Esquerda);
            p.texto(p.largura - 60, yy + 12, estado, 22, DIM, Alinhamento::Direita);
        }
    }

    fn desenhar_rodape(&self, p: &mut Pintor) {
        let y = p.altura - RODAPE_H;
        p.retangulo(0, y, p.largura, RODAPE_H, PANEL);
        p.linha(0, y, p.largura, y, LINE);
        let dica = match self.tela {
            Tela::Vitrine => "↑ ↓ escolher   ·   Enter abrir   ·   ← → biblioteca   ·   Esc sair",
            Tela::Ficha => "Enter comprar   ·   Esc voltar",
            Tela::Pagamento => "Esc voltar ao app",
            Tela::Biblioteca => "← → loja   ·   Esc sair",
        };
        p.texto(24, y + 16, dica, 20, DIM, Alinhamento::Esquerda);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use giochi_loja_core::catalogo::{Item, VERSAO as CATALOGO_V};
    use giochi_loja_core::pagamento::Estado as EstadoPedido;
    use giochi_loja_core::preco::{Moeda, Preco};

    fn item(slug: &str, centavos: u32) -> Item {
        Item {
            slug: slug.into(),
            titulo: slug.into(),
            ano: 2000,
            descricao: "d".into(),
            preco: Preco::new(centavos, Moeda::BRL),
            versao: "1.0.0".into(),
            tamanho_bytes: 1000,
            sha256: String::new(),
            url: format!("https://x/{slug}.giochi"),
            assinatura: String::new(),
            chave_pacote: String::new(),
        }
    }

    fn estado(demo: bool) -> Estado {
        let c = Catalogo { versao: CATALOGO_V, gerado_em: 0, invalida_antes: None,
                           itens: vec![item("a", 0), item("b", 1990), item("c", 990)] };
        let e = Estado::novo().com_catalogo(c);
        if demo { e.demo() } else { e }
    }

    #[test]
    fn setas_nao_saem_da_lista() {
        let mut e = estado(false);
        e.tecla(Tecla::Cima);
        assert_eq!(e.selecao, 0, "subiu acima do primeiro");
        for _ in 0..10 { e.tecla(Tecla::Baixo); }
        assert_eq!(e.selecao, 2, "passou do último");
    }

    #[test]
    fn enter_abre_a_ficha_do_item_selecionado() {
        let mut e = estado(false);
        e.tecla(Tecla::Baixo);
        e.tecla(Tecla::Confirmar);
        assert_eq!(e.tela, Tela::Ficha);
        assert_eq!(e.item_atual().unwrap().slug, "b");
    }

    #[test]
    fn esc_volta_uma_tela_por_vez_e_nao_sai_da_vitrine() {
        let mut e = estado(false);
        e.tecla(Tecla::Confirmar);          // vitrine -> ficha
        e.tecla(Tecla::Voltar);
        assert_eq!(e.tela, Tela::Vitrine);
        assert!(e.tecla(Tecla::Voltar), "Esc na vitrine encerrou o app");
    }

    #[test]
    fn q_sai() {
        let mut e = estado(false);
        assert!(!e.tecla(Tecla::Sair));
    }

    #[test]
    fn comprar_sem_provedor_nao_cria_pedido_e_avisa() {
        // A regra mais importante da tela: não inventar código PIX.
        let mut e = estado(false);
        e.tecla(Tecla::Confirmar);  // ficha
        e.tecla(Tecla::Confirmar);  // comprar
        assert_eq!(e.tela, Tela::Ficha, "abriu o pagamento sem provedor");
        assert!(e.pedido.is_none(), "criou pedido sem provedor");
        assert!(e.mensagem.contains("provedor"), "{}", e.mensagem);
    }

    #[test]
    fn em_demo_o_pedido_existe_e_fica_aguardando() {
        let mut e = estado(true);
        e.tecla(Tecla::Confirmar);
        e.tecla(Tecla::Confirmar);
        assert_eq!(e.tela, Tela::Pagamento);
        let p = e.pedido.as_ref().unwrap();
        assert!(matches!(p.estado(), EstadoPedido::Aguardando { pix: Some(_), .. }), "{:?}", p.estado());
    }

    #[test]
    fn a_tela_de_pagamento_nao_se_declara_paga() {
        // O botão "Já paguei" não pode virar `Pago`: só o servidor faz isso.
        let mut e = estado(true);
        e.tecla(Tecla::Confirmar);
        e.tecla(Tecla::Confirmar);
        // Aperta "Já paguei" dez vezes.
        for _ in 0..10 {
            e.tecla(Tecla::Confirmar);
        }
        let p = e.pedido.as_ref().unwrap();
        assert!(!matches!(p.estado(), EstadoPedido::Pago { .. }), "cliente se declarou pago");
        assert!(p.licenca().is_none());
        assert!(e.mensagem.contains("servidor"), "{}", e.mensagem);
    }

    #[test]
    fn setas_trocam_entre_loja_e_biblioteca() {
        let mut e = estado(false);
        e.tecla(Tecla::Direita);
        assert_eq!(e.tela, Tela::Biblioteca);
        e.tecla(Tecla::Esquerda);
        assert_eq!(e.tela, Tela::Vitrine);
    }

    #[test]
    fn catalogo_vazio_nao_quebra_nem_seleciona() {
        let mut e = Estado::novo().com_catalogo(Catalogo {
            versao: CATALOGO_V, gerado_em: 0, invalida_antes: None, itens: vec![],
        });
        e.tecla(Tecla::Baixo);
        e.tecla(Tecla::Confirmar);
        assert_eq!(e.selecao, 0);
        assert_eq!(e.tela, Tela::Vitrine, "abriu ficha de catálogo vazio");
        assert!(e.item_atual().is_none());
    }
}
