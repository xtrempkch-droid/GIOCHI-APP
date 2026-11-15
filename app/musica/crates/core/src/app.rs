//! O reducer do app: recebe uma ação, devolve o que fazer no mundo.
//!
//! A separação é a mesma do launcher — estado é valor, comando entra, estado
//! novo sai — com uma diferença que importa aqui: o reducer devolve um
//! [`Efeito`] dizendo **o que tocar**. Assim a regra "apertar A na faixa em foco
//! troca de música" é testável sem mixer, sem placa de som e sem aparelho, e a
//! camada de áudio só executa o que foi decidido.
//!
//! O app de exemplo misturava as três coisas num `update` só: lia o gamepad,
//! tocava o áudio e animava o carrossel no mesmo lugar. Consequências medidas:
//! segurar o direcional percorria a lista inteira num quadro (a leitura era de
//! nível, não de borda), e não havia teste nenhum possível sem SDL.

use crate::biblioteca::Biblioteca;
use crate::carrossel::Carrossel;
use crate::faixa::Faixa;
use crate::transporte::{EstadoPlay, Transporte};

/// O que o usuário pediu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    /// Anda `passos` no carrossel (negativo = para a esquerda).
    Mover(i8),
    /// Escolhe uma faixa direto pelo índice (toque).
    Focar(usize),
    /// A: toca a faixa em foco, ou pausa/retoma a que já está tocando.
    Alternar,
    /// X: próxima faixa.
    Proxima,
    /// Y: faixa anterior.
    Anterior,
    /// Parar (mantém a seleção).
    Parar,
    /// Sobe/desce o volume em passos de 5.
    Volume(i8),
    /// Relê a pasta (o usuário copiou música com o app aberto).
    Recarregar,
    /// Sair do app.
    Sair,
}

/// O que o app deve fazer no mundo depois da ação.
///
/// `Nada` existe e é explícito: sem ele, todo chamador teria de adivinhar se
/// "sem mudança" é um caso especial ou o resultado normal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Efeito {
    Nada,
    /// Carrega e toca a faixa `usize` (índice na biblioteca).
    Tocar(usize),
    Retomar,
    Pausar,
    Parar,
    /// Aplica o volume no mixer.
    Volume(i32),
    /// Relê a pasta. O app é quem sabe ler disco; o núcleo só pede.
    Recarregar,
    Sair,
}

/// Estado completo do app.
#[derive(Clone, Debug, PartialEq)]
pub struct Estado {
    pub biblioteca: Biblioteca,
    pub carrossel: Carrossel,
    pub transporte: Transporte,
    /// Índice da faixa que o mixer está tocando (ou pausada).
    ///
    /// Invariante: `tocando.is_some()` se e somente se o transporte não está
    /// `Parado`. Todo método daqui mantém isso, e há teste para cada caminho.
    tocando: Option<usize>,
}

impl Estado {
    pub fn novo(biblioteca: Biblioteca) -> Estado {
        let carrossel = Carrossel::novo(biblioteca.total());
        Estado {
            biblioteca,
            carrossel,
            transporte: Transporte::novo(Transporte::VOLUME_PADRAO),
            tocando: None,
        }
    }

    pub fn com_volume(mut self, volume: i32) -> Estado {
        self.transporte = Transporte::novo(volume);
        self
    }

    /// Índice da faixa em foco, se houver biblioteca.
    pub fn faixa_em_foco(&self) -> Option<&Faixa> {
        self.biblioteca.faixas().get(self.carrossel.foco())
    }

    /// Faixa que está no mixer (tocando ou pausada).
    pub fn faixa_tocando(&self) -> Option<&Faixa> {
        self.tocando.and_then(|i| self.biblioteca.faixas().get(i))
    }

    pub fn indice_tocando(&self) -> Option<usize> {
        self.tocando
    }

    /// `true` quando a faixa em foco é a que está no mixer.
    pub fn foco_e_a_que_toca(&self) -> bool {
        self.tocando == Some(self.carrossel.foco()) && self.tocando.is_some()
    }

    /// Troca a biblioteca (releitura da pasta), preservando foco e transporte.
    pub fn trocar_biblioteca(&mut self, biblioteca: Biblioteca) {
        let tocando_antes = self.tocando;
        self.biblioteca = biblioteca;
        self.carrossel.ajustar(self.biblioteca.total());
        if self.biblioteca.esta_vazia() {
            self.parar_tudo();
        } else if let Some(i) = tocando_antes {
            // A lista pode ter mudado de ordem; o índice antigo aponta para
            // outra música. Manter o índice seria tocar a faixa errada.
            if i >= self.biblioteca.total() {
                self.parar_tudo();
            }
        }
    }

    fn parar_tudo(&mut self) {
        self.tocando = None;
        self.transporte.estado = EstadoPlay::Parado;
    }

    /// O mixer avisou que a faixa terminou (ou parou por conta própria).
    pub fn terminou(&mut self) {
        self.parar_tudo();
    }

    /// Aplica uma ação e diz o que fazer no mundo.
    pub fn aplicar(&mut self, acao: Acao) -> Efeito {
        match acao {
            Acao::Mover(passos) => {
                self.carrossel.mover(isize::from(passos));
                Efeito::Nada
            }
            Acao::Focar(indice) => {
                self.carrossel.ir_para(indice);
                Efeito::Nada
            }
            Acao::Alternar => self.alternar(),
            Acao::Proxima => self.pular(1),
            Acao::Anterior => self.pular(-1),
            Acao::Parar => {
                if self.tocando.is_none() && self.transporte.estado == EstadoPlay::Parado {
                    Efeito::Nada
                } else {
                    self.parar_tudo();
                    Efeito::Parar
                }
            }
            Acao::Volume(passos) => {
                if passos == 0 {
                    Efeito::Nada
                } else {
                    Efeito::Volume(self.transporte.ajustar_volume(passos))
                }
            }
            Acao::Recarregar => Efeito::Recarregar,
            Acao::Sair => Efeito::Sair,
        }
    }

    fn alternar(&mut self) -> Efeito {
        if self.biblioteca.esta_vazia() {
            return Efeito::Nada;
        }
        let foco = self.carrossel.foco();

        // Já está nesta faixa: alterna pausa/retomada. É o caminho que o exemplo
        // não tinha — apertar A de novo recomeçava a música do zero.
        if self.tocando == Some(foco) {
            return match self.transporte.estado {
                EstadoPlay::Tocando => {
                    self.transporte.estado = EstadoPlay::Pausado;
                    Efeito::Pausar
                }
                EstadoPlay::Pausado => {
                    self.transporte.estado = EstadoPlay::Tocando;
                    Efeito::Retomar
                }
                // Pausado/parado com `tocando` marcado não deveria existir;
                // recomeçar é a recuperação honesta.
                EstadoPlay::Parado => {
                    self.transporte.estado = EstadoPlay::Tocando;
                    Efeito::Tocar(foco)
                }
            };
        }

        self.tocando = Some(foco);
        self.transporte.estado = EstadoPlay::Tocando;
        Efeito::Tocar(foco)
    }

    fn pular(&mut self, passo: i8) -> Efeito {
        if self.biblioteca.esta_vazia() {
            return Efeito::Nada;
        }
        if !self.carrossel.mover(isize::from(passo)) {
            return Efeito::Nada;
        }
        // Parado, pular só move o foco: o usuário está escolhendo, não
        // navegando uma playlist em execução.
        if self.tocando.is_none() && self.transporte.estado == EstadoPlay::Parado {
            return Efeito::Nada;
        }
        let foco = self.carrossel.foco();
        self.tocando = Some(foco);
        self.transporte.estado = EstadoPlay::Tocando;
        Efeito::Tocar(foco)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biblioteca::{montar, Entrada};

    fn tres() -> Biblioteca {
        montar(
            "/m",
            &[
                Entrada::arquivo("1 - Um.mp3"),
                Entrada::arquivo("2 - Dois.mp3"),
                Entrada::arquivo("3 - Tres.mp3"),
            ],
        )
    }

    fn vazio() -> Biblioteca {
        montar("/m", &[])
    }

    fn estado() -> Estado {
        Estado::novo(tres())
    }

    fn invariante(e: &Estado) {
        assert_eq!(
            e.indice_tocando().is_some(),
            e.transporte.estado != EstadoPlay::Parado,
            "tocando {:?} com transporte {:?}",
            e.indice_tocando(),
            e.transporte.estado
        );
    }

    // ── tocar e pausar ──────────────────────────────────────

    #[test]
    fn a_toca_a_faixa_em_foco() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Alternar), Efeito::Tocar(0));
        assert_eq!(e.transporte.estado, EstadoPlay::Tocando);
        assert_eq!(e.indice_tocando(), Some(0));
        invariante(&e);
    }

    #[test]
    fn a_de_novo_pausa_e_depois_retoma() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        assert_eq!(e.aplicar(Acao::Alternar), Efeito::Pausar);
        assert_eq!(e.transporte.estado, EstadoPlay::Pausado);
        // Continuar é a mesma faixa: o mixer retoma do ponto, não recomeça.
        assert_eq!(e.aplicar(Acao::Alternar), Efeito::Retomar);
        assert_eq!(e.transporte.estado, EstadoPlay::Tocando);
        invariante(&e);
    }

    #[test]
    fn mover_e_apertar_a_troca_de_faixa() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        e.aplicar(Acao::Mover(1));
        assert_eq!(e.aplicar(Acao::Alternar), Efeito::Tocar(1));
        assert_eq!(e.indice_tocando(), Some(1));
        invariante(&e);
    }

    #[test]
    fn mover_o_foco_sozinho_nao_troca_a_musica() {
        // O foco é escolha; a música continua até o usuário confirmar.
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        e.aplicar(Acao::Mover(2));
        assert_eq!(e.indice_tocando(), Some(0));
        assert!(!e.foco_e_a_que_toca());
        assert_eq!(e.faixa_tocando().map(|f| f.titulo.as_str()), Some("Um"));
        assert_eq!(e.faixa_em_foco().map(|f| f.titulo.as_str()), Some("Tres"));
    }

    #[test]
    fn em_biblioteca_vazia_a_nao_faz_nada() {
        let mut e = Estado::novo(vazio());
        assert_eq!(e.aplicar(Acao::Alternar), Efeito::Nada);
        assert_eq!(e.indice_tocando(), None);
        assert_eq!(e.aplicar(Acao::Mover(1)), Efeito::Nada);
        assert_eq!(e.aplicar(Acao::Proxima), Efeito::Nada);
        assert_eq!(e.transporte.estado, EstadoPlay::Parado);
        invariante(&e);
    }

    // ── próxima e anterior ──────────────────────────────────

    #[test]
    fn proxima_parado_so_move_o_foco() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Proxima), Efeito::Nada);
        assert_eq!(e.carrossel.foco(), 1);
        assert_eq!(e.indice_tocando(), None);
        invariante(&e);
    }

    #[test]
    fn proxima_tocando_troca_a_musica() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        assert_eq!(e.aplicar(Acao::Proxima), Efeito::Tocar(1));
        assert_eq!(e.indice_tocando(), Some(1));
        assert_eq!(e.transporte.estado, EstadoPlay::Tocando);
        invariante(&e);
    }

    #[test]
    fn anterior_no_primeiro_da_a_volta_e_toca() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        assert_eq!(e.aplicar(Acao::Anterior), Efeito::Tocar(2));
        assert_eq!(e.carrossel.foco(), 2);
        invariante(&e);
    }

    #[test]
    fn proxima_pausado_toca_a_seguinte() {
        // Com a música pausada, pular tem de retomar a nova — senão a tela diz
        // "Tocando" e não sai som nenhum.
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        e.aplicar(Acao::Alternar);
        assert_eq!(e.transporte.estado, EstadoPlay::Pausado);
        assert_eq!(e.aplicar(Acao::Proxima), Efeito::Tocar(1));
        assert_eq!(e.transporte.estado, EstadoPlay::Tocando);
        invariante(&e);
    }

    // ── parar ───────────────────────────────────────────────

    #[test]
    fn parar_zera_o_que_esta_no_mixer() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        assert_eq!(e.aplicar(Acao::Parar), Efeito::Parar);
        assert_eq!(e.indice_tocando(), None);
        assert_eq!(e.transporte.estado, EstadoPlay::Parado);
        // A seleção continua onde estava: parar não é voltar ao começo.
        assert_eq!(e.carrossel.foco(), 0);
        invariante(&e);
    }

    #[test]
    fn parar_sem_nada_tocando_nao_manda_comando() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Parar), Efeito::Nada);
    }

    #[test]
    fn fim_da_faixa_devolve_o_transporte_ao_parado() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        e.terminou();
        assert_eq!(e.transporte.estado, EstadoPlay::Parado);
        assert_eq!(e.indice_tocando(), None);
        invariante(&e);
    }

    // ── volume ──────────────────────────────────────────────

    #[test]
    fn volume_devolve_o_valor_a_aplicar() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Volume(1)), Efeito::Volume(75));
        assert_eq!(e.aplicar(Acao::Volume(-3)), Efeito::Volume(60));
        assert_eq!(e.aplicar(Acao::Volume(0)), Efeito::Nada);
    }

    #[test]
    fn volume_funciona_sem_biblioteca() {
        // O volume é do mixer, não da faixa: numa pasta vazia o usuário ainda
        // precisa ajustar o som do próximo app.
        let mut e = Estado::novo(vazio());
        assert_eq!(e.aplicar(Acao::Volume(2)), Efeito::Volume(80));
    }

    // ── recarregar ──────────────────────────────────────────

    #[test]
    fn recarregar_pede_releitura_sem_mexer_no_estado() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        let antes = e.clone();
        assert_eq!(e.aplicar(Acao::Recarregar), Efeito::Recarregar);
        // Quem lê a pasta é a camada de disco; o núcleo não pode inventar faixa
        // nem parar a música só porque alguém pediu releitura.
        assert_eq!(e, antes);
    }

    // ── sair ────────────────────────────────────────────────

    #[test]
    fn sair_nao_depende_do_estado() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Sair), Efeito::Sair);
        e.aplicar(Acao::Alternar);
        assert_eq!(e.aplicar(Acao::Sair), Efeito::Sair);
    }

    // ── troca de biblioteca ─────────────────────────────────

    #[test]
    fn biblioteca_vazia_para_o_que_tocava() {
        let mut e = estado();
        e.aplicar(Acao::Alternar);
        e.trocar_biblioteca(vazio());
        assert_eq!(e.indice_tocando(), None);
        assert_eq!(e.transporte.estado, EstadoPlay::Parado);
        invariante(&e);
    }

    #[test]
    fn trocar_biblioteca_mantem_o_foco_quando_cabe() {
        let mut e = estado();
        e.aplicar(Acao::Mover(2));
        e.trocar_biblioteca(montar(
            "/m",
            &[
                Entrada::arquivo("1 - Um.mp3"),
                Entrada::arquivo("2 - Dois.mp3"),
                Entrada::arquivo("3 - Tres.mp3"),
                Entrada::arquivo("4 - Quatro.mp3"),
            ],
        ));
        assert_eq!(e.carrossel.foco(), 2);
        assert_eq!(e.biblioteca.total(), 4);
    }

    #[test]
    fn trocar_biblioteca_menor_para_o_que_nao_existe_mais() {
        let mut e = estado();
        e.aplicar(Acao::Mover(2));
        e.aplicar(Acao::Alternar);
        assert_eq!(e.indice_tocando(), Some(2));
        // A lista encolheu: o índice 2 deixou de existir.
        e.trocar_biblioteca(montar("/m", &[Entrada::arquivo("1 - Um.mp3")]));
        assert_eq!(e.indice_tocando(), None);
        assert_eq!(e.carrossel.foco(), 0);
        invariante(&e);
    }

    // ── foco direto ─────────────────────────────────────────

    #[test]
    fn focar_pelo_indice_respeita_os_limites() {
        let mut e = estado();
        assert_eq!(e.aplicar(Acao::Focar(2)), Efeito::Nada);
        assert_eq!(e.carrossel.foco(), 2);
        e.aplicar(Acao::Focar(9));
        assert_eq!(e.carrossel.foco(), 2, "índice inválido não muda nada");
    }

    #[test]
    fn cada_caminho_mantem_a_invariante_do_transporte() {
        // A varredura de sequências existe porque a invariante é a única coisa
        // que impede a tela de dizer "Tocando" com o mixer parado.
        let sequencias: [&[Acao]; 6] = [
            &[Acao::Alternar],
            &[Acao::Alternar, Acao::Alternar],
            &[Acao::Alternar, Acao::Mover(1), Acao::Alternar],
            &[Acao::Alternar, Acao::Proxima, Acao::Parar],
            &[Acao::Mover(2), Acao::Anterior, Acao::Anterior],
            &[Acao::Alternar, Acao::Anterior, Acao::Parar, Acao::Proxima],
        ];
        for sequencia in sequencias {
            let mut e = estado();
            for acao in sequencia {
                e.aplicar(*acao);
                invariante(&e);
            }
        }
    }
}
