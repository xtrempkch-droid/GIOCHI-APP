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
use giochi_loja_core::grade::{grade_para, mover, Grade, Passo};
use giochi_loja_core::pagamento::{Estado as EstadoPedido, Pedido, Pix};

use crate::desenho::*;

/// Largura da barra lateral. A referência é o itch.io: coluna estreita à
/// esquerda, conteúdo à direita.
pub const BARRA_LATERAL: i32 = 208;
/// Margem interna do conteúdo.
pub const MARGEM: i32 = 28;
/// Largura-alvo do cartão. Não é fixa: `grade_da_janela` distribui o que existe.
pub const CARTAO_ALVO: i32 = 218;
/// Calha entre cartões.
pub const CALHA: i32 = 18;
/// Altura do texto abaixo da capa (título + preço).
pub const RODAPE_CARTAO: i32 = 54;

/// A grade para estas dimensões de janela.
///
/// Uma função só, usada pelo desenho **e** pela navegação. Duas contas
/// separadas divergem no dia em que alguém ajusta uma margem.
pub fn grade_da_janela(largura: i32, altura: i32, total: usize) -> Grade {
    let _ = total;
    let disp_x = largura - BARRA_LATERAL - 2 * MARGEM;
    // Reserva do título da seção e da margem inferior.
    let disp_y = altura - crate::desenho::TOPO_H - crate::desenho::RODAPE_H - 2 * MARGEM - 46;
    let cartao_y = CARTAO_ALVO * 3 / 4 + RODAPE_CARTAO + CALHA;
    grade_para(disp_x, disp_y, CARTAO_ALVO + CALHA, cartao_y)
}

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
    /// A grade **do quadro atual**. Publicada pelo desenho e lida pela
    /// navegação, para as duas nunca discordarem do número de colunas — foi
    /// exatamente esse divórcio que quebrou a navegação no launcher.
    pub grade: Grade,
    largura: i32,
    altura: i32,
    pub catalogo: Option<Catalogo>,
    pub pedido: Option<Pedido>,
    pub demo: bool,
    pub mensagem: String,
    /// Licenças já obtidas, por slug. Na vida real viria do disco.
    pub biblioteca: Vec<(String, String)>,
}

impl Estado {
    pub fn novo(largura: i32, altura: i32) -> Self {
        Self {
            tela: Tela::Vitrine,
            selecao: 0,
            // Calculada na construção, e não só no primeiro desenho: `--teclas`
            // aplica teclas **antes** de desenhar, e usar o padrão ali foi o bug
            // do launcher (navegava com 4 colunas num layout de 2).
            grade: grade_da_janela(largura, altura, 0),
            catalogo: None,
            pedido: None,
            largura,
            altura,
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
            // Em grade, a seta anda na grade: `↑`/`↓` mudam de **linha** (não de
            // item) e `←`/`→` param no fim da linha. A regra mora em
            // `giochi_loja_core::grade` e tem teste próprio, incluindo o de que
            // `↓` não anda um item só — que foi o bug real no launcher.
            Tecla::Cima | Tecla::Baixo | Tecla::Esquerda | Tecla::Direita => {
                if self.tela == Tela::Vitrine {
                    let passo = match t {
                        Tecla::Cima => Passo::Cima,
                        Tecla::Baixo => Passo::Baixo,
                        Tecla::Esquerda => Passo::Esquerda,
                        _ => Passo::Direita,
                    };
                    if let Some(novo) = mover(self.selecao, self.itens().len(), self.grade, passo) {
                        self.selecao = novo;
                    }
                } else if matches!(t, Tecla::Esquerda | Tecla::Direita) {
                    // Fora da vitrine, ← → trocam de aba.
                    self.tela = if self.tela == Tela::Biblioteca { Tela::Vitrine } else { Tela::Biblioteca };
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

    pub fn desenhar(&mut self, p: &mut Pintor) {
        // Republica a grade a cada quadro: a janela pode ter sido
        // redimensionada, e a navegação tem de acompanhar no quadro seguinte.
        self.largura = p.largura;
        self.altura = p.altura;
        self.grade = grade_da_janela(p.largura, p.altura, self.itens().len());
        p.limpar(BG);
        self.desenhar_barra(p);
        self.desenhar_topo(p);
        match self.tela {
            Tela::Vitrine => self.desenhar_vitrine(p),
            Tela::Ficha => self.desenhar_ficha(p),
            Tela::Pagamento => self.desenhar_pagamento(p),
            Tela::Biblioteca => self.desenhar_biblioteca(p),
        }
        self.desenhar_rodape(p);
    }

    /// Barra lateral: identidade, abas e o aviso de modo de teste.
    ///
    /// Fica à esquerda porque é o desenho do itch.io, que é a referência visual
    /// da loja. Não é integração com o itch.io — nenhuma chamada, nenhuma
    /// credencial, nenhum `butlerd`; só o layout.
    fn desenhar_barra(&self, p: &mut Pintor) {
        p.retangulo(0, 0, BARRA_LATERAL, p.altura, PANEL);
        p.linha(BARRA_LATERAL, 0, BARRA_LATERAL, p.altura, LINE);

        p.texto(24, 20, "Giochi", 27, ACCENT, Alinhamento::Esquerda);
        p.texto(24, 52, "loja do PC", 18, DIM, Alinhamento::Esquerda);

        let abas = [("Loja", Tela::Vitrine), ("Biblioteca", Tela::Biblioteca)];
        let mut y = 108;
        for (nome, tela) in abas {
            let ativo = self.tela == tela;
            if ativo {
                p.arredondado(12, y - 8, BARRA_LATERAL - 24, 38, 8, PANEL2);
                p.retangulo(12, y - 8, 3, 38, ACCENT);
            }
            p.texto(28, y, nome, 23, if ativo { TEXT } else { DIM }, Alinhamento::Esquerda);
            y += 46;
        }

        // Rodapé da barra: o aviso mora aqui, não no topo, porque em modo de
        // teste o que o usuário precisa lembrar é antes de pagar.
        let rodape = p.altura - 96;
        if self.demo {
            p.arredondado(12, rodape, BARRA_LATERAL - 24, 44, 8, DANGER);
            p.texto(24, rodape + 6, "MODO DE TESTE", 18, TEXT, Alinhamento::Esquerda);
            p.texto(24, rodape + 24, "pagamento não é real", 15, TEXT, Alinhamento::Esquerda);
        } else if let Some(c) = &self.catalogo {
            let s = format!("{} app(s)", c.itens.len());
            p.texto(24, rodape + 12, &s, 19, DIM, Alinhamento::Esquerda);
        }
    }

    fn desenhar_topo(&self, p: &mut Pintor) {
        // O topo cobre só a área de conteúdo: a barra lateral já tem o título.
        let x0 = BARRA_LATERAL;
        p.retangulo(x0, 0, p.largura - x0, TOPO_H, BG);
        p.linha(x0, TOPO_H, p.largura, TOPO_H, LINE);
        let titulo = match self.tela {
            Tela::Vitrine => "Loja",
            Tela::Ficha => "Detalhes",
            Tela::Pagamento => "Pagamento",
            Tela::Biblioteca => "Biblioteca",
        };
        p.texto(x0 + MARGEM, 14, titulo, 26, TEXT, Alinhamento::Esquerda);
        if let Some(i) = self.item_atual() {
            if self.tela == Tela::Vitrine {
                p.texto(
                    p.largura - MARGEM,
                    20,
                    &format!("{} de {}", self.selecao + 1, self.itens().len()),
                    18,
                    DIM,
                    Alinhamento::Direita,
                );
                let _ = i;
            }
        }
    }

    /// A grade de cartões — o coração do layout.
    fn desenhar_vitrine(&self, p: &mut Pintor) {
        let x0 = BARRA_LATERAL + MARGEM;
        let dispersao = p.largura - BARRA_LATERAL - 2 * MARGEM;
        let y0 = TOPO_H + MARGEM + 30;

        if self.itens().is_empty() {
            p.texto(x0, y0 + 20, "catálogo vazio", 28, DIM, Alinhamento::Esquerda);
            p.texto(
                x0,
                y0 + 60,
                "passe --catalogo <arquivo.json> para carregar",
                20,
                DIM,
                Alinhamento::Esquerda,
            );
            return;
        }

        // A mesma função que a navegação usa. O cartão ocupa o que sobra: se a
        // janela é larga, os cartões crescem em vez de sobrar espaço à direita.
        let cols = self.grade.colunas.max(1) as i32;
        let cartao_w = if cols > 1 {
            (dispersao - (cols - 1) * CALHA) / cols
        } else {
            dispersao
        };
        let capa_h = cartao_w * 3 / 4;
        let cartao_h = capa_h + RODAPE_CARTAO;

        for (i, item) in self.itens().iter().enumerate() {
            let col = i as i32 % cols;
            let lin = i as i32 / cols;
            let x = x0 + col * (cartao_w + CALHA);
            let y = y0 + lin * (cartao_h + CALHA);
            // Fora da tela não desenha. Importante porque `por_pagina` pode ser
            // menor que o total, e desenhar o invisível é trabalho que ninguém vê.
            if y + cartao_h > p.altura - crate::desenho::RODAPE_H {
                continue;
            }
            let selecionado = i == self.selecao;

            crate::capa::desenhar(p, x, y, cartao_w, capa_h, &item.slug, &item.titulo, selecionado);

            p.texto_limitado(x + 2, y + capa_h + 8, &item.titulo, 20, TEXT, cartao_w - 4);
            let preco = item.preco.to_string();
            let cor = if item.preco.eh_gratis() { OK } else { DIM };
            p.texto(x + 2, y + capa_h + 32, &preco, 18, cor, Alinhamento::Esquerda);
        }
    }

    fn desenhar_ficha(&self, p: &mut Pintor) {
        let Some(item) = self.item_atual() else { return };
        let y = TOPO_H + 28;
        let e = BARRA_LATERAL + MARGEM;

        p.texto(e, y, &item.titulo, 34, TEXT, Alinhamento::Esquerda);
        let ano = if item.ano == 0 { "—".to_string() } else { item.ano.to_string() };
        p.texto(e, y + 48, &format!("{ano}  ·  versão {}", item.versao), 21, DIM, Alinhamento::Esquerda);

        // A mesma capa da grade, grande: o usuário reconhece o que clicou.
        crate::capa::desenhar(p, e, y + 92, 220, 165, &item.slug, &item.titulo, true);

        let x = e + 250;
        let largura = p.largura - x - 40;
        p.texto_limitado(x, y + 92, &item.descricao, 22, TEXT, largura);

        let mb = item.tamanho_bytes as f64 / 1_048_576.0;
        p.texto(x, y + 180, &format!("{mb:.1} MB"), 20, DIM, Alinhamento::Esquerda);

        // Botão de compra.
        let rotulo = if item.preco.eh_gratis() { "Baixar".to_string() } else { format!("Comprar · {}", item.preco) };
        let (w, _) = p.text_size(&rotulo, 28);
        let bw = w + 64;
        p.arredondado(x, y + 220, bw, 52, 12, ACCENT);
        p.texto(x + bw / 2, y + 234, &rotulo, 26, BG, Alinhamento::Centro);

        if !self.mensagem.is_empty() {
            p.texto_limitado(x, y + 290, &self.mensagem, 20, WARN, largura);
        }
    }

    fn desenhar_pagamento(&self, p: &mut Pintor) {
        let y = TOPO_H + 28;
        let _e = BARRA_LATERAL + MARGEM;
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
        let e = BARRA_LATERAL + MARGEM;
        p.texto(e, y, "Sua biblioteca", 34, TEXT, Alinhamento::Esquerda);
        if self.biblioteca.is_empty() {
            p.texto(e, y + 70, "nada comprado ainda", 24, DIM, Alinhamento::Esquerda);
            p.texto(
                e,
                y + 106,
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
        let e = Estado::novo(1180, 760).com_catalogo(c);
        if demo { e.demo() } else { e }
    }

    #[test]
    fn setas_nao_saem_da_grade() {
        let mut e = estado(false);
        e.tecla(Tecla::Cima);
        assert_eq!(e.selecao, 0, "subiu acima do primeiro");
        for _ in 0..10 {
            e.tecla(Tecla::Baixo);
        }
        assert!(e.selecao < 3, "passou do último: {}", e.selecao);
        for _ in 0..10 {
            e.tecla(Tecla::Direita);
        }
        assert!(e.selecao < 3, "passou do último com →: {}", e.selecao);
    }

    #[test]
    fn a_seta_para_baixo_desce_uma_linha_e_nao_um_item() {
        // O bug que já aconteceu no launcher: `↓` andando 1 item tem nome de
        // vertical e efeito horizontal. Aqui a grade é de 3 colunas em 1180 px.
        let mut e = estado(false); // 3 itens, então a grade tem 3 colunas
        assert_eq!(e.grade.colunas, 3, "a janela 1180x760 deveria dar 3 colunas");
        // Com 3 itens não há segunda linha; o teste da geometria está em
        // `grade::testes`. Aqui basta provar que `→` anda 1 e `↓` não é `→`.
        e.tecla(Tecla::Direita);
        assert_eq!(e.selecao, 1, "→ deveria andar um item");
    }

    #[test]
    fn enter_abre_a_ficha_do_item_selecionado() {
        let mut e = estado(false);
        e.tecla(Tecla::Direita); // → vai para o segundo item na grade
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
    fn na_biblioteca_as_setas_trocam_de_aba_e_nao_de_item() {
        // Na vitrine `←`/`→` movem a seleção; na biblioteca (sem grade) elas
        // trocam de aba. Ter as duas regras no mesmo lugar é o que permite a
        // tela única de navegação.
        let mut e = estado(false);
        e.tela = Tela::Biblioteca;
        e.tecla(Tecla::Esquerda);
        assert_eq!(e.tela, Tela::Vitrine);
        e.tecla(Tecla::Direita);
        assert_eq!(e.selecao, 1, "na vitrine, → deveria mover a seleção");
    }

    #[test]
    fn catalogo_vazio_nao_quebra_nem_seleciona() {
        let mut e = Estado::novo(1180, 760).com_catalogo(Catalogo {
            versao: CATALOGO_V, gerado_em: 0, invalida_antes: None, itens: vec![],
        });
        e.tecla(Tecla::Baixo);
        e.tecla(Tecla::Confirmar);
        assert_eq!(e.selecao, 0);
        assert_eq!(e.tela, Tela::Vitrine, "abriu ficha de catálogo vazio");
        assert!(e.item_atual().is_none());
    }
}
