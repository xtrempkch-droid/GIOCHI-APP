//! As telas do jogo e as transições entre elas.
//!
//! ## Por que uma cena separada do laço
//!
//! Até o 2c, "fim de partida" era um aviso desenhado por cima da cena, e o jogo
//! não tinha começo: abria jogando. Isso deixa dois estados impossíveis de
//! distinguir olhando o código (`estado.acabou()` é uma condição, não um lugar) e
//! um terceiro sem tratamento — o **intervalo entre partidas**, que é onde o
//! jogador precisa de um botão.
//!
//! Aqui a tela é um valor explícito ([`Tela`]) e a transição é uma função pura
//! ([`processar`]). O laço pergunta "onde estou" e "o que fazer com esta tecla";
//! nada de `if`s aninhados decidindo se a simulação anda.
//!
//! ## A regra que o `R` precisava
//!
//! Reiniciar não é só `estado.reiniciar()`: a partida tem pool, temporizador de
//! spawn e sorteio próprios. Se só o placar zerasse, a segunda partida começaria
//! com os cubos da primeira no ar — e o jogador veria a vida cair no primeiro
//! segundo de uma partida "nova". [`Reinicio`] é a lista **completa** do que
//! volta ao início, num lugar só.

use crate::state::GameState;

/// Onde o jogo está.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    /// Título: uma opção para começar, uma para sair.
    Menu,
    /// Partida em andamento.
    Jogando,
    /// Partida terminada, com o resultado.
    Fim {
        /// Venceu as waves todas?
        venceu: bool,
    },
}

impl Tela {
    /// A simulação anda nesta tela?
    ///
    /// É o que impede o cubo de continuar andando atrás do menu — e o que o
    /// "[JOGAR] do menu move o jogador" não pode esquecer.
    pub fn simula(self) -> bool {
        matches!(self, Tela::Jogando)
    }

    /// Desenha a cena 3D do jogo?
    ///
    /// O menu também desenha a arena ao fundo: uma tela preta com texto seria
    /// mais barata e diria menos sobre o que o jogo é.
    pub fn desenha_cena(self) -> bool {
        true
    }
}

/// O que o jogador pediu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    /// `A` / `Enter` / clique no item em foco.
    Confirmar,
    /// `B` / `Esc` — sobe um nível; no menu, não faz nada.
    Voltar,
    /// `R` — recomeça a partida.
    Reiniciar,
    /// Nenhuma tecla: o quadro só quer saber se o **estado** mudou de tela.
    ///
    /// Existe porque a partida termina sozinha. Sem esta ação, a tela de fim só
    /// aparecia quando o jogador apertava alguma coisa — e quem perdesse sem
    /// tocar no controle continuava vendo a cena com zero vidas.
    Nenhuma,
}

/// O que fazer depois de uma ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    /// Fica na mesma tela.
    Fica,
    /// Vai para outra tela.
    Vai(Tela),
    /// Fecha o jogo.
    Sair,
}

/// Item do menu em foco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemMenu {
    /// Começar a partida.
    #[default]
    Iniciar,
    /// Fechar o jogo.
    Sair,
}

impl ItemMenu {
    /// Rótulo desenhado.
    pub fn rotulo(self) -> &'static str {
        match self {
            ItemMenu::Iniciar => "INICIAR",
            ItemMenu::Sair => "SAIR",
        }
    }

    /// Próximo item, dando a volta.
    pub fn proximo(self) -> Self {
        match self {
            ItemMenu::Iniciar => ItemMenu::Sair,
            ItemMenu::Sair => ItemMenu::Iniciar,
        }
    }

    /// Item anterior, dando a volta.
    pub fn anterior(self) -> Self {
        // Dois itens: anterior é o mesmo que próximo. Não é preguiça — com dois
        // elementos, emendar a lista é a única definição que não trava.
        self.proximo()
    }
}

/// Aplica `acao` em `tela` e diz para onde ir.
///
/// `estado` entra porque o fim da partida **não** decide sozinho quando chegou:
/// quem sabe se o jogador venceu ou perdeu é o [`GameState`], e ler isso aqui
/// evita a cópia da regra no laço.
pub fn processar(tela: Tela, acao: Acao, estado: &GameState) -> Resultado {
    // A partida em andamento termina **sozinha**, antes de a ação ser olhada:
    // quem decide é o estado, não o jogador. O `R` continua recomeçando direto,
    // sem obrigar a passar pela tela de fim.
    if tela == Tela::Jogando
        && (estado.acabou() || estado.venceu())
        && !matches!(acao, Acao::Reiniciar)
    {
        return Resultado::Vai(Tela::Fim {
            venceu: estado.venceu(),
        });
    }

    match (tela, acao) {
        // ── Menu ────────────────────────────────────────────────
        (Tela::Menu, Acao::Confirmar) => Resultado::Vai(Tela::Jogando),
        (Tela::Menu, Acao::Voltar) => Resultado::Sair,
        (Tela::Menu, Acao::Reiniciar | Acao::Nenhuma) => Resultado::Fica,

        // ── Jogando ─────────────────────────────────────────────
        (Tela::Jogando, Acao::Reiniciar) => Resultado::Vai(Tela::Jogando),
        (Tela::Jogando, Acao::Voltar) => Resultado::Vai(Tela::Menu),
        (Tela::Jogando, Acao::Confirmar | Acao::Nenhuma) => Resultado::Fica,

        // ── Fim ─────────────────────────────────────────────────
        (Tela::Fim { .. }, Acao::Reiniciar | Acao::Confirmar) => Resultado::Vai(Tela::Jogando),
        (Tela::Fim { .. }, Acao::Voltar) => Resultado::Vai(Tela::Menu),
        (Tela::Fim { .. }, Acao::Nenhuma) => Resultado::Fica,
    }
}

/// Tela inicial para uma execução interativa.
pub fn inicial() -> Tela {
    Tela::Menu
}

/// Item do menu em foco — o menu não guarda estado próprio além disto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Menu {
    /// Item em foco.
    pub item: ItemMenu,
}

impl Menu {
    /// Menu novo, com `INICIAR` em foco.
    pub fn novo() -> Self {
        Self::default()
    }

    /// Move o foco.
    pub fn mover(&mut self, para_baixo: bool) {
        self.item = if para_baixo {
            self.item.proximo()
        } else {
            self.item.anterior()
        };
    }

    /// Ação do `A`, resolvida pelo item em foco.
    pub fn confirmar(&self) -> Resultado {
        match self.item {
            ItemMenu::Iniciar => Resultado::Vai(Tela::Jogando),
            ItemMenu::Sair => Resultado::Sair,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jogando() -> GameState {
        GameState::new()
    }

    #[test]
    fn o_menu_comeca_com_iniciar_em_foco() {
        assert_eq!(Menu::novo().item, ItemMenu::Iniciar);
    }

    #[test]
    fn o_foco_do_menu_da_a_volta() {
        let mut menu = Menu::novo();
        menu.mover(true);
        assert_eq!(menu.item, ItemMenu::Sair);
        menu.mover(true);
        assert_eq!(menu.item, ItemMenu::Iniciar, "dois itens, dá a volta");
        menu.mover(false);
        assert_eq!(menu.item, ItemMenu::Sair, "para cima também dá a volta");
    }

    #[test]
    fn o_a_do_menu_respeita_o_item_em_foco() {
        let mut menu = Menu::novo();
        assert_eq!(menu.confirmar(), Resultado::Vai(Tela::Jogando));
        menu.mover(true);
        assert_eq!(
            menu.confirmar(),
            Resultado::Sair,
            "com SAIR em foco, o A fecha o jogo"
        );
    }

    #[test]
    fn o_b_do_menu_sai() {
        assert_eq!(
            processar(Tela::Menu, Acao::Voltar, &jogando()),
            Resultado::Sair
        );
    }

    #[test]
    fn a_partida_termina_sem_tecla_nenhuma() {
        // O caso que o jogador sente: ele perde, tira o dedo do controle, e a
        // tela tem de mudar mesmo assim.
        let mut estado = jogando();
        for _ in 0..5 {
            estado.perdeu_vida();
        }
        assert_eq!(
            processar(Tela::Jogando, Acao::Nenhuma, &estado),
            Resultado::Vai(Tela::Fim { venceu: false })
        );
        // E o `A` do fim ainda recomeça.
        assert_eq!(
            processar(Tela::Fim { venceu: false }, Acao::Nenhuma, &estado),
            Resultado::Fica
        );
    }

    #[test]
    fn a_partida_termina_sozinha_quando_o_estado_acaba() {
        let mut estado = jogando();
        for _ in 0..5 {
            estado.perdeu_vida();
        }
        assert_eq!(
            processar(Tela::Jogando, Acao::Confirmar, &estado),
            Resultado::Vai(Tela::Fim { venceu: false })
        );
    }

    #[test]
    fn a_vitoria_chega_pela_mesma_regra() {
        let mut estado = jogando();
        estado.wave = crate::state::WAVES_TOTAIS + 1;
        assert_eq!(
            processar(Tela::Jogando, Acao::Confirmar, &estado),
            Resultado::Vai(Tela::Fim { venceu: true })
        );
    }

    #[test]
    fn o_menu_nao_pula_para_o_fim_de_um_jogo_ja_perdido() {
        // A guarda existe por isto: o `estado` da partida anterior ainda está
        // com zero vidas quando o jogador volta ao menu. Sem a checagem de
        // "estou jogando", o menu saltaria para FIM no primeiro quadro.
        let mut estado = jogando();
        for _ in 0..5 {
            estado.perdeu_vida();
        }
        assert_eq!(
            processar(Tela::Menu, Acao::Confirmar, &estado),
            Resultado::Vai(Tela::Jogando),
            "o A do menu tem de começar uma partida, não mostrar a anterior"
        );
        assert_eq!(
            processar(Tela::Menu, Acao::Reiniciar, &estado),
            Resultado::Fica
        );
    }

    #[test]
    fn o_r_recomeca_tanto_no_jogo_quanto_no_fim() {
        for tela in [
            Tela::Jogando,
            Tela::Fim { venceu: false },
            Tela::Fim { venceu: true },
        ] {
            assert_eq!(
                processar(tela, Acao::Reiniciar, &jogando()),
                Resultado::Vai(Tela::Jogando),
                "{tela:?}: R tinha de recomeçar"
            );
        }
    }

    #[test]
    fn o_b_do_fim_volta_ao_menu() {
        assert_eq!(
            processar(Tela::Fim { venceu: true }, Acao::Voltar, &jogando()),
            Resultado::Vai(Tela::Menu)
        );
    }

    #[test]
    fn o_a_do_fim_tambem_recomeca() {
        // Quem acabou de perder aperta o botão que já está com o dedo nele.
        // Exigir `R` seria pedir que o jogador decore uma tecla.
        for venceu in [true, false] {
            assert_eq!(
                processar(Tela::Fim { venceu }, Acao::Confirmar, &jogando()),
                Resultado::Vai(Tela::Jogando)
            );
        }
    }

    #[test]
    fn so_a_partida_simula() {
        assert!(Tela::Jogando.simula());
        assert!(!Tela::Menu.simula(), "o cubo não anda atrás do menu");
        assert!(!Tela::Fim { venceu: false }.simula());
        assert!(!Tela::Fim { venceu: true }.simula());
    }

    #[test]
    fn o_menu_desenha_a_arena_ao_fundo() {
        // Uma tela preta com texto diria menos sobre o jogo; a arena é o cartão
        // de visitas.
        assert!(Tela::Menu.desenha_cena());
    }
}
