//! Menus: principal, multijogador, configurações, pausa e fim de partida.
//!
//! # Por que uma máquina de estados pura
//!
//! Um menu parece trivial até a primeira vez que "voltar" fecha a partida em vez
//! de voltar, ou que "reiniciar" deixa o estado antigo vivo. Aqui a decisão é
//! função pura de `(tela, cursor, entrada)` → `(tela, cursor, ação)`, e o laço em
//! `main.rs` só executa a ação. Assim "reiniciar" e "sair" são testáveis sem
//! abrir janela — e nenhum dos dois pode acontecer por acidente.
//!
//! # Os três desfechos que o usuário pediu
//!
//! | Pedido | Onde |
//! |---|---|
//! | reiniciar | pausa → "Reiniciar partida"; fim → "Jogar de novo" |
//! | sair | principal → "Sair"; pausa → "Sair para o menu" (e o menu tem Sair) |
//! | configurar | principal → "Configurações" (volume, câmera, FPS) |
//!
//! # A entrada é a mesma do jogo
//!
//! `InputFrame` já traz o analógico com zona morta e as bordas de botão. Um menu
//! que lesse o teclado por fora teria o próprio deadzone, a própria repetição e o
//! próprio bug — e no aparelho o analógico é o único caminho.

use crate::engine_bridge::input::{Axis, InputFrame};

/// Quanto tempo o cursor leva para repetir quando a direção fica segurada.
///
/// 0,22 s e não 60 Hz: com repetição por quadro, um toque no direcional atravessa
/// a lista inteira e o item escolhido passa a ser o último da tela.
pub const REPETICAO_S: f32 = 0.22;

/// Quanto a direção precisa passar para contar como "apertou".
///
/// A zona morta do analógico não serve aqui: ela existe para o movimento do
/// personagem, onde 0,3 é "andando devagar". Num menu, qualquer coisa acima de
/// 0,5 é intenção.
pub const LIMIAR_NAVEGACAO: f32 = 0.5;

/// Telas do menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    /// Menu principal, na abertura.
    Principal,
    /// Hospedar, procurar salas.
    Multijogador,
    /// Volume, câmera, FPS.
    Configuracoes,
    /// Pausa durante a partida.
    Pausa,
    /// Vitória ou derrota.
    Fim {
        /// `true` = campanha vencida.
        venceu: bool,
    },
}

impl Tela {
    /// A tela pausa o mundo?
    pub fn pausa_o_jogo(self) -> bool {
        !matches!(self, Tela::Fim { .. })
    }

    /// Nome curto, para o log e para os testes.
    pub fn nome(self) -> &'static str {
        match self {
            Tela::Principal => "principal",
            Tela::Multijogador => "multijogador",
            Tela::Configuracoes => "configuracoes",
            Tela::Pausa => "pausa",
            Tela::Fim { venceu: true } => "fim (vitória)",
            Tela::Fim { venceu: false } => "fim (derrota)",
        }
    }
}

/// Uma linha do menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Texto fixo à esquerda.
    pub rotulo: String,
    /// Valor atual, se a linha for ajustável.
    pub valor: Option<String>,
}

impl Item {
    /// Linha de ação.
    pub fn acao(rotulo: impl Into<String>) -> Self {
        Self {
            rotulo: rotulo.into(),
            valor: None,
        }
    }

    /// Linha ajustável (esquerda/direita mudam o valor).
    pub fn ajustavel(rotulo: impl Into<String>, valor: impl Into<String>) -> Self {
        Self {
            rotulo: rotulo.into(),
            valor: Some(valor.into()),
        }
    }

    /// A linha aceita esquerda/direita?
    pub fn ajustavel_de_fato(&self) -> bool {
        self.valor.is_some()
    }
}

/// O que fazer depois de uma entrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acao {
    /// Nada — a entrada só moveu o cursor.
    Nada,
    /// Abre a partida (do menu principal).
    Jogar,
    /// Reinicia com o mesmo mapa/config.
    Reiniciar,
    /// Volta ao menu principal, encerrando a sessão de rede.
    MenuPrincipal,
    /// Encerra o processo.
    Sair,
    /// Abre uma tela.
    Abrir(Tela),
    /// Volta um nível (do submenu para o principal, da pausa para o jogo).
    Voltar,
    /// Sobe uma partida em rede na porta padrão.
    Hospedar,
    /// Procura salas na rede local.
    ProcurarSalas,
    /// Entra na sala `indice` da última procura.
    EntrarSala(usize),
    /// Abre a pausa durante a partida.
    Pausar,
}

/// Configurações que o jogador muda no menu.
///
/// São três, e todas mudam algo **visível**: volume (o mixer ALSA), distância da
/// câmera (o quanto do mapa se vê) e FPS (o contador). Uma opção que não muda
/// nada é pior que uma opção ausente — é a mentira que este repositório combate
/// desde o "Wi-Fi: Ligado" fixo do launcher.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    /// Volume de 0 a 100.
    pub volume: u8,
    /// Distância da câmera ao foco, em metros.
    pub camera_dist_m: f32,
    /// Mostra o contador de quadros.
    pub mostrar_fps: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume: 70,
            camera_dist_m: 52.0,
            mostrar_fps: false,
        }
    }
}

/// Limites da câmera no menu.
///
/// O teto é o padrão antigo: aproximar é a razão de a opção existir (ver o
/// personagem), e afastar além do padrão só aumentaria o que já se via.
pub const CAMERA_MIN_M: f32 = 24.0;
/// Teto da distância da câmera.
pub const CAMERA_MAX_M: f32 = 60.0;

/// Passo de cada ajuste.
pub const PASSO_VOLUME: u8 = 5;
/// Passo da câmera, em metros.
pub const PASSO_CAMERA_M: f32 = 4.0;

impl Config {
    /// Normaliza valores fora de faixa (arquivo editado à mão ou versão antiga).
    pub fn saneada(self) -> Self {
        Self {
            volume: self.volume.min(100),
            camera_dist_m: if self.camera_dist_m.is_finite() {
                self.camera_dist_m.clamp(CAMERA_MIN_M, CAMERA_MAX_M)
            } else {
                Self::default().camera_dist_m
            },
            mostrar_fps: self.mostrar_fps,
        }
    }

    /// Volume em `0.0..=1.0`, para o mixer.
    pub fn volume_fracao(self) -> f32 {
        f32::from(self.volume.min(100)) / 100.0
    }

    /// Aplica `+1`/`-1` num campo e devolve o rótulo do novo valor.
    ///
    /// `campo` é o índice da linha ajustável dentro da tela de configurações, na
    /// ordem em que [`Menu::itens`] as produz. Índice em vez de enum porque o
    /// menu já trabalha com índice de cursor — duas numerações para a mesma
    /// lista é como o carrossel do launcher dessincronizou.
    pub fn ajustar(&mut self, campo: usize, delta: i32) -> bool {
        match campo {
            0 => {
                let passo = PASSO_VOLUME as i32 * delta;
                let novo = (i32::from(self.volume) + passo).clamp(0, 100) as u8;
                let mudou = novo != self.volume;
                self.volume = novo;
                mudou
            }
            1 => {
                let novo = (self.camera_dist_m + PASSO_CAMERA_M * delta as f32)
                    .clamp(CAMERA_MIN_M, CAMERA_MAX_M);
                let mudou = (novo - self.camera_dist_m).abs() > f32::EPSILON;
                self.camera_dist_m = novo;
                mudou
            }
            2 => {
                self.mostrar_fps = !self.mostrar_fps;
                true
            }
            _ => false,
        }
    }
}

/// Estado do menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    /// Tela atual.
    pub tela: Tela,
    /// Item sob o cursor.
    pub cursor: usize,
    /// Tempo restante até a próxima repetição da direção.
    pub repeticao_s: f32,
    /// Configurações do jogador (o menu é o editor delas).
    pub config: Config,
    /// Salas encontradas na última procura, `nome@endereço`.
    pub salas: Vec<String>,
    /// Último aviso mostrado ao jogador (erro de rede, "procurando…").
    pub aviso: Option<String>,
    /// Há partida em andamento (habilita "Continuar").
    pub em_partida: bool,
}

impl Menu {
    /// Menu principal, na abertura.
    ///
    /// `em_partida` existe para o "Continuar" só aparecer quando há o que
    /// continuar: um item que não faz nada é o defeito que este menu evita.
    pub fn novo(em_partida: bool) -> Self {
        Self {
            tela: Tela::Principal,
            cursor: 0,
            repeticao_s: 0.0,
            config: Config::default(),
            salas: Vec::new(),
            aviso: None,
            em_partida,
        }
    }

    /// Volta o cursor para a primeira linha (ao trocar de tela).
    fn recolocar(&mut self) {
        self.cursor = 0;
        self.repeticao_s = 0.0;
    }

    /// Abre uma tela pelo nome, reiniciando o cursor.
    pub fn abrir(&mut self, tela: Tela) {
        self.tela = tela;
        self.recolocar();
    }

    /// As linhas da tela atual.
    pub fn itens(&self) -> Vec<Item> {
        match self.tela {
            Tela::Principal => {
                let mut itens = Vec::new();
                if self.em_partida {
                    itens.push(Item::acao("Continuar"));
                }
                itens.push(Item::acao("Jogar"));
                itens.push(Item::acao("Multijogador"));
                itens.push(Item::acao("Configurações"));
                itens.push(Item::acao("Sair"));
                itens
            }
            Tela::Multijogador => {
                let mut itens = vec![Item::acao("Hospedar partida"), Item::acao("Procurar salas")];
                for (indice, sala) in self.salas.iter().enumerate() {
                    let _ = indice;
                    itens.push(Item::acao(format!("Entrar: {sala}")));
                }
                itens.push(Item::acao("Voltar"));
                itens
            }
            Tela::Configuracoes => vec![
                Item::ajustavel("Volume", format!("{}%", self.config.volume)),
                Item::ajustavel("Câmera", format!("{:.0} m", self.config.camera_dist_m)),
                Item::ajustavel("Mostrar FPS", if self.config.mostrar_fps { "Sim" } else { "Não" }),
                Item::acao("Voltar"),
            ],
            Tela::Pausa => vec![
                Item::acao("Continuar"),
                Item::acao("Reiniciar partida"),
                Item::acao("Configurações"),
                Item::acao("Sair para o menu"),
            ],
            Tela::Fim { venceu } => {
                let mut itens = vec![Item::acao(if venceu {
                    "Jogar de novo"
                } else {
                    "Tentar de novo"
                })];
                itens.push(Item::acao("Configurações"));
                itens.push(Item::acao("Sair para o menu"));
                itens
            }
        }
    }

    /// A linha sob o cursor.
    pub fn item_atual(&self) -> Option<Item> {
        self.itens().into_iter().nth(self.cursor)
    }

    /// Processa um quadro de entrada e devolve o que fazer.
    ///
    /// A ordem importa: **confirmar** e **voltar** são bordas (acontecem uma vez),
    /// e a navegação tem repetição. Testar a navegação antes de confirmar faria um
    /// analógico torto mover o cursor e confirmar no mesmo quadro.
    pub fn entrada(&mut self, frame: &InputFrame, dt: f32) -> Acao {
        let itens = self.itens();
        if itens.is_empty() {
            return Acao::Nada;
        }
        // Grampeia o cursor: a lista encolheu (sala que sumiu, "Continuar" que
        // desapareceu ao sair da partida) desde o quadro anterior.
        if self.cursor >= itens.len() {
            self.cursor = itens.len() - 1;
        }

        if frame.edges.attack {
            return self.confirmar();
        }
        if frame.edges.ability1 || frame.edges.pause {
            return self.voltar();
        }

        // --- navegação vertical, com repetição -------------------------
        self.repeticao_s -= dt;
        let eixo = frame.move_dir;
        let vertical = if eixo.y.abs() >= LIMIAR_NAVEGACAO {
            Some(eixo.y)
        } else {
            None
        };
        if let Some(y) = vertical {
            if self.repeticao_s <= 0.0 {
                let passo = if y > 0.0 {
                    itens.len() - 1
                } else {
                    itens.len() + 1
                };
                self.cursor = (self.cursor + passo) % itens.len();
                self.repeticao_s = REPETICAO_S;
            }
        } else {
            // Soltar a direção zera a espera: o próximo toque responde na hora, e
            // não depois de 0,22 s que o jogador não pediu.
            self.repeticao_s = 0.0;
        }

        // --- ajuste horizontal nas linhas que têm valor -----------------
        let ajustavel = itens
            .get(self.cursor)
            .map(Item::ajustavel_de_fato)
            .unwrap_or(false);
        if ajustavel && eixo.x.abs() >= LIMIAR_NAVEGACAO {
            // Repetição própria: ajustar volume com a mesma cadência da navegação
            // pularia de 0 a 100 num toque longo.
            if self.repeticao_s <= 0.0 {
                let delta = if eixo.x > 0.0 { 1 } else { -1 };
                self.aplicar_ajuste(delta);
                self.repeticao_s = REPETICAO_S;
            }
        }

        Acao::Nada
    }

    /// Aplica `delta` à linha ajustável sob o cursor, se houver.
    fn aplicar_ajuste(&mut self, delta: i32) -> bool {
        // O índice do **campo** é a posição na lista de ajustáveis, não na de
        // itens: um separador ou um "Voltar" no meio deslocaria tudo em silêncio.
        let itens = self.itens();
        let campo = itens
            .iter()
            .take(self.cursor + 1)
            .filter(|i| i.ajustavel_de_fato())
            .count()
            .saturating_sub(1);
        self.config.ajustar(campo, delta)
    }

    /// Executa a linha sob o cursor.
    pub fn confirmar(&mut self) -> Acao {
        let Some(item) = self.item_atual() else {
            return Acao::Nada;
        };
        match self.tela {
            Tela::Principal => {
                // O primeiro item é "Continuar" só quando há partida.
                let mut indice = 0;
                if self.em_partida {
                    if self.cursor == 0 {
                        return Acao::Voltar;
                    }
                    indice = 1;
                }
                match self.cursor - indice {
                    0 => Acao::Jogar,
                    1 => {
                        self.abrir(Tela::Multijogador);
                        Acao::Nada
                    }
                    2 => {
                        self.abrir(Tela::Configuracoes);
                        Acao::Nada
                    }
                    _ => Acao::Sair,
                }
            }
            Tela::Multijogador => {
                if item.rotulo.starts_with("Hospedar") {
                    return Acao::Hospedar;
                }
                if item.rotulo.starts_with("Procurar") {
                    return Acao::ProcurarSalas;
                }
                if item.rotulo == "Voltar" {
                    self.abrir(Tela::Principal);
                    return Acao::Nada;
                }
                // "Entrar: ..." — o índice da sala é a posição menos as duas
                // primeiras linhas.
                if self.cursor >= 2 {
                    return Acao::EntrarSala(self.cursor - 2);
                }
                Acao::Nada
            }
            Tela::Configuracoes => {
                if self.cursor + 1 == self.itens().len() {
                    self.abrir(Tela::Principal);
                }
                Acao::Nada
            }
            Tela::Pausa => match self.cursor {
                0 => Acao::Voltar,
                1 => Acao::Reiniciar,
                2 => {
                    self.abrir(Tela::Configuracoes);
                    Acao::Nada
                }
                _ => {
                    self.sair_da_partida();
                    Acao::MenuPrincipal
                }
            },
            Tela::Fim { .. } => match self.cursor {
                0 => Acao::Reiniciar,
                1 => {
                    self.abrir(Tela::Configuracoes);
                    Acao::Nada
                }
                _ => {
                    self.sair_da_partida();
                    Acao::MenuPrincipal
                }
            },
        }
    }

    /// Volta ao menu principal **e** larga a partida.
    ///
    /// # O bug que isto conserta (relato do dono, 10/11/2026)
    ///
    /// "Sair para o menu" devolvia `Acao::MenuPrincipal`, e o `main.rs` só zerava
    /// o `modo`. O menu continuava em `Tela::Pausa`: a mesma pausa era desenhada
    /// de novo, com as mesmas quatro linhas, sobre o mesmo mundo congelado. Do
    /// lado de fora, **nada acontecia** — e apertar a linha outra vez repetia o
    /// nada.
    ///
    /// A navegação é do menu; o `main.rs` executa a ação. Deixar a troca de tela
    /// só no `main.rs` é o mesmo erro de contrato do `A — Jogar` na Home do
    /// launcher (`AGENTS.md` §12): a ação existe, o desenho não acompanha.
    fn sair_da_partida(&mut self) {
        self.marcar_partida(false);
        self.abrir(Tela::Principal);
    }

    /// "Voltar" (B): um nível acima, sem nunca encerrar o processo por engano.
    pub fn voltar(&mut self) -> Acao {
        match self.tela {
            // No menu principal, "voltar" **não** sai: sair é uma linha explícita.
            // Um B pressionado por reflexo fechar o jogo é o tipo de perda que o
            // jogador não perdoa.
            Tela::Principal => Acao::Nada,
            Tela::Multijogador => {
                self.abrir(Tela::Principal);
                Acao::Nada
            }
            Tela::Configuracoes => {
                // De onde se veio: da pausa volta para a pausa, do menu para o menu.
                let destino = if self.em_partida {
                    Tela::Pausa
                } else {
                    Tela::Principal
                };
                self.abrir(destino);
                Acao::Nada
            }
            Tela::Pausa => Acao::Voltar,
            Tela::Fim { .. } => {
                self.abrir(Tela::Principal);
                Acao::Nada
            }
        }
    }

    /// Registra as salas encontradas, mantendo o cursor válido.
    pub fn definir_salas(&mut self, salas: Vec<String>) {
        self.salas = salas;
        self.aviso = None;
        if self.tela == Tela::Multijogador {
            let total = self.itens().len();
            if self.cursor >= total {
                self.cursor = total.saturating_sub(1);
            }
        }
    }

    /// Liga o "Continuar" quando uma partida começa (ou desliga ao voltar).
    pub fn marcar_partida(&mut self, em_partida: bool) {
        self.em_partida = em_partida;
        let total = self.itens().len();
        if self.cursor >= total {
            self.cursor = total.saturating_sub(1);
        }
    }
}

/// Direção de navegação, na ordem em que o menu a consome.
///
/// Existe para o teste de propriedade: nenhuma sequência de entradas pode deixar
/// o cursor fora da lista, em nenhuma tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direcao {
    /// Cima.
    Cima,
    /// Baixo.
    Baixo,
    /// Esquerda.
    Esquerda,
    /// Direita.
    Direita,
    /// Confirma.
    Confirma,
    /// Volta.
    Volta,
}

/// Um quadro de entrada sintético, para teste e para `--teclas`.
pub fn quadro(direcao: Direcao) -> InputFrame {
    let mut frame = InputFrame::default();
    match direcao {
        Direcao::Cima => frame.move_dir = Axis::new(0.0, 1.0),
        Direcao::Baixo => frame.move_dir = Axis::new(0.0, -1.0),
        Direcao::Esquerda => frame.move_dir = Axis::new(-1.0, 0.0),
        Direcao::Direita => frame.move_dir = Axis::new(1.0, 0.0),
        Direcao::Confirma => frame.edges.attack = true,
        Direcao::Volta => frame.edges.ability1 = true,
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aplica uma direção com o tempo já decorrido (para a repetição liberar).
    fn passo(menu: &mut Menu, direcao: Direcao) -> Acao {
        menu.entrada(&quadro(direcao), REPETICAO_S + 0.01)
    }

    fn menu_principal() -> Menu {
        Menu::novo(false)
    }

    #[test]
    fn o_menu_principal_tem_jogar_multijogador_configuracoes_e_sair() {
        let menu = menu_principal();
        let rotulos: Vec<String> = menu.itens().into_iter().map(|i| i.rotulo).collect();
        assert_eq!(rotulos, ["Jogar", "Multijogador", "Configurações", "Sair"]);
        // Sem partida em andamento não existe "Continuar": um item que não faz
        // nada é pior que um item ausente.
        assert!(!rotulos.contains(&"Continuar".to_string()));
        assert!(Menu::novo(true).itens()[0].rotulo == "Continuar");
    }

    #[test]
    fn o_cursor_da_a_volta_nas_duas_direcoes() {
        let mut menu = menu_principal();
        let total = menu.itens().len();
        for _ in 0..total {
            passo(&mut menu, Direcao::Baixo);
        }
        assert_eq!(menu.cursor, 0, "descer {total} vezes volta ao começo");
        passo(&mut menu, Direcao::Cima);
        assert_eq!(menu.cursor, total - 1, "subir do começo vai para o fim");
    }

    #[test]
    fn segurar_a_direcao_nao_atravessa_a_lista_em_um_quadro() {
        let mut menu = menu_principal();
        // Dois quadros seguidos, sem tempo para repetir: o cursor anda uma vez.
        menu.entrada(&quadro(Direcao::Baixo), 0.01);
        menu.entrada(&quadro(Direcao::Baixo), 0.01);
        assert_eq!(menu.cursor, 1, "repetiu antes do tempo");
        // E depois do tempo, anda.
        menu.entrada(&quadro(Direcao::Baixo), REPETICAO_S + 0.01);
        assert_eq!(menu.cursor, 2);
    }

    #[test]
    fn soltar_a_direcao_devolve_a_resposta_imediata() {
        let mut menu = menu_principal();
        menu.entrada(&quadro(Direcao::Baixo), 0.01);
        menu.entrada(&quadro(Direcao::Baixo), REPETICAO_S + 0.01);
        let depois_de_segurar = menu.cursor;
        // Solta (sem direção) e aperta de novo no quadro seguinte.
        menu.entrada(&InputFrame::default(), 0.01);
        menu.entrada(&quadro(Direcao::Baixo), 0.01);
        assert_eq!(menu.cursor, depois_de_segurar + 1, "o toque seguinte esperou a repetição");
    }

    #[test]
    fn confirmar_jogar_abre_a_partida_e_nao_o_menu() {
        let mut menu = menu_principal();
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Jogar);
    }

    #[test]
    fn confirmar_sair_e_o_unico_caminho_para_encerrar() {
        let mut menu = menu_principal();
        passo(&mut menu, Direcao::Baixo);
        passo(&mut menu, Direcao::Baixo);
        passo(&mut menu, Direcao::Baixo);
        assert_eq!(menu.item_atual().expect("sair").rotulo, "Sair");
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Sair);
    }

    #[test]
    fn voltar_no_menu_principal_nao_encerra_o_jogo() {
        // B pressionado por reflexo não pode fechar o jogo: sair é uma linha
        // explícita, com o cursor nela.
        let mut menu = menu_principal();
        assert_eq!(passo(&mut menu, Direcao::Volta), Acao::Nada);
        assert_eq!(menu.tela, Tela::Principal);
    }

    #[test]
    fn a_pausa_oferece_continuar_reiniciar_configuracoes_e_sair_para_o_menu() {
        let mut menu = Menu::novo(true);
        menu.abrir(Tela::Pausa);
        let rotulos: Vec<String> = menu.itens().into_iter().map(|i| i.rotulo).collect();
        assert_eq!(
            rotulos,
            ["Continuar", "Reiniciar partida", "Configurações", "Sair para o menu"]
        );
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Voltar);
        passo(&mut menu, Direcao::Baixo);
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Reiniciar);
        passo(&mut menu, Direcao::Baixo);
        passo(&mut menu, Direcao::Baixo);
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::MenuPrincipal);
    }

    #[test]
    fn o_fim_oferece_reiniciar_e_sair_para_o_menu() {
        for venceu in [true, false] {
            let mut menu = Menu::novo(true);
            menu.abrir(Tela::Fim { venceu });
            let primeiro = menu.itens()[0].rotulo.clone();
            assert!(primeiro.contains("de novo"), "{primeiro}");
            assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Reiniciar);
            // E o último é sair para o menu — nunca "Sair" do jogo direto daqui.
            let ultimo = menu.itens().len() - 1;
            menu.cursor = ultimo;
            assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::MenuPrincipal);
        }
    }

    #[test]
    fn sair_para_o_menu_muda_de_tela_e_nao_so_de_estado() {
        // O bug relatado em 11/11/2026: a linha devolvia `Acao::MenuPrincipal` e o
        // `main.rs` só zerava o `modo`. O menu ficava em `Tela::Pausa` e redesenhava
        // a mesma pausa sobre o mesmo mundo congelado — **nada** mudava na tela.
        //
        // Este teste olha a tela, não a ação: era exatamente a diferença que faltava
        // no teste acima, que só comparava `Acao::MenuPrincipal`.
        let mut menu = Menu::novo(true);
        menu.marcar_partida(true);
        menu.abrir(Tela::Pausa);
        menu.cursor = menu.itens().len() - 1;
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::MenuPrincipal);
        assert_eq!(menu.tela, Tela::Principal);
        // Sem partida em curso, "Jogar" é a primeira linha — e não "Continuar".
        assert!(!menu.em_partida);
        assert_eq!(menu.item_atual().expect("linha").rotulo, "Jogar");
        // E apertar de novo começa uma partida nova em vez de repetir o nada.
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Jogar);
    }

    #[test]
    fn sair_para_o_menu_depois_do_fim_tambem_muda_de_tela() {
        for venceu in [true, false] {
            let mut menu = Menu::novo(true);
            menu.abrir(Tela::Fim { venceu });
            menu.cursor = menu.itens().len() - 1;
            assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::MenuPrincipal);
            assert_eq!(menu.tela, Tela::Principal);
            assert_eq!(menu.item_atual().expect("linha").rotulo, "Jogar");
        }
    }

    #[test]
    fn o_volume_ajusta_pelo_teclado_e_respeita_os_limites() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Configuracoes);
        assert_eq!(menu.config.volume, 70);
        passo(&mut menu, Direcao::Esquerda);
        assert_eq!(menu.config.volume, 65);
        passo(&mut menu, Direcao::Direita);
        assert_eq!(menu.config.volume, 70);

        // Segurar à esquerda não passa de zero nem envolve para 100.
        for _ in 0..40 {
            passo(&mut menu, Direcao::Esquerda);
        }
        assert_eq!(menu.config.volume, 0);
        for _ in 0..40 {
            passo(&mut menu, Direcao::Direita);
        }
        assert_eq!(menu.config.volume, 100);
    }

    #[test]
    fn a_camera_aproxima_e_afasta_dentro_da_faixa() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Configuracoes);
        passo(&mut menu, Direcao::Baixo);
        assert_eq!(menu.item_atual().expect("camera").rotulo, "Câmera");
        let inicial = menu.config.camera_dist_m;
        passo(&mut menu, Direcao::Esquerda);
        assert!(menu.config.camera_dist_m < inicial, "esquerda devia aproximar");
        for _ in 0..40 {
            passo(&mut menu, Direcao::Esquerda);
        }
        assert_eq!(menu.config.camera_dist_m, CAMERA_MIN_M);
        for _ in 0..40 {
            passo(&mut menu, Direcao::Direita);
        }
        assert_eq!(menu.config.camera_dist_m, CAMERA_MAX_M);
    }

    #[test]
    fn o_fps_alterna_e_a_linha_mostra_o_estado() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Configuracoes);
        menu.cursor = 2;
        assert_eq!(menu.item_atual().expect("fps").valor.as_deref(), Some("Não"));
        passo(&mut menu, Direcao::Direita);
        assert!(menu.config.mostrar_fps);
        assert_eq!(menu.item_atual().expect("fps").valor.as_deref(), Some("Sim"));
        passo(&mut menu, Direcao::Direita);
        assert!(!menu.config.mostrar_fps, "alternar duas vezes volta ao começo");
    }

    #[test]
    fn a_linha_voltar_nao_responde_a_seta_e_sai_ao_confirmar() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Configuracoes);
        let ultimo = menu.itens().len() - 1;
        menu.cursor = ultimo;
        assert!(!menu.item_atual().expect("voltar").ajustavel_de_fato());
        // Esquerda numa linha sem valor não muda configuração nenhuma.
        let antes = menu.config;
        passo(&mut menu, Direcao::Esquerda);
        assert_eq!(menu.config, antes);
        passo(&mut menu, Direcao::Confirma);
        assert_eq!(menu.tela, Tela::Principal);
    }

    #[test]
    fn as_configuracoes_voltam_para_onde_foram_abertas() {
        // Aberta do menu, volta ao menu; aberta da pausa, volta à pausa. Errar
        // isso joga o jogador para o menu principal no meio de uma partida.
        let mut do_menu = menu_principal();
        do_menu.abrir(Tela::Configuracoes);
        passo(&mut do_menu, Direcao::Volta);
        assert_eq!(do_menu.tela, Tela::Principal);

        let mut da_pausa = Menu::novo(true);
        da_pausa.abrir(Tela::Pausa);
        da_pausa.abrir(Tela::Configuracoes);
        passo(&mut da_pausa, Direcao::Volta);
        assert_eq!(da_pausa.tela, Tela::Pausa);
    }

    #[test]
    fn a_sala_procurada_vira_linha_e_o_indice_do_entrar_esta_certo() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Multijogador);
        menu.definir_salas(vec!["sala-a@10.0.0.2:7777".to_string(), "sala-b@10.0.0.3:7777".to_string()]);
        let rotulos: Vec<String> = menu.itens().into_iter().map(|i| i.rotulo).collect();
        assert_eq!(rotulos.len(), 5, "{rotulos:?}");
        assert!(rotulos[2].starts_with("Entrar"));

        menu.cursor = 2;
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::EntrarSala(0));
        menu.cursor = 3;
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::EntrarSala(1));
        menu.cursor = 4;
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Nada);
        assert_eq!(menu.tela, Tela::Principal, "Voltar devia fechar o submenu");
    }

    #[test]
    fn uma_sala_que_some_nao_deixa_o_cursor_fora_da_lista() {
        let mut menu = menu_principal();
        menu.abrir(Tela::Multijogador);
        menu.definir_salas(vec!["a".into(), "b".into(), "c".into()]);
        menu.cursor = 4;
        // A procura seguinte encontra menos salas: o cursor tem de recuar junto,
        // senão confirmar acerta uma sala que não existe e o host recusa.
        menu.definir_salas(vec!["a".into()]);
        assert!(menu.cursor < menu.itens().len(), "cursor {} de {}", menu.cursor, menu.itens().len());
        // E confirmar continua fazendo sentido.
        let acao = passo(&mut menu, Direcao::Confirma);
        assert!(matches!(acao, Acao::EntrarSala(_) | Acao::Nada), "{acao:?}");
    }

    #[test]
    fn nenhuma_sequencia_de_entradas_deixa_o_cursor_fora_da_lista() {
        // Propriedade, não exemplo: 5 telas × 200 entradas pseudoaleatórias. Um
        // cursor fora da lista estoura no `item_atual()` e o menu inteiro cai — no
        // aparelho, sem terminal para ver.
        let direcoes = [
            Direcao::Cima,
            Direcao::Baixo,
            Direcao::Esquerda,
            Direcao::Direita,
            Direcao::Confirma,
            Direcao::Volta,
        ];
        for tela in [
            Tela::Principal,
            Tela::Multijogador,
            Tela::Configuracoes,
            Tela::Pausa,
            Tela::Fim { venceu: true },
        ] {
            let mut menu = Menu::novo(true);
            menu.abrir(tela);
            let mut estado = 12345u64;
            for _ in 0..200 {
                estado = estado.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let direcao = direcoes[(estado >> 33) as usize % direcoes.len()];
                let _ = menu.entrada(&quadro(direcao), REPETICAO_S + 0.01);
                let total = menu.itens().len();
                assert!(
                    menu.cursor < total,
                    "{}: cursor {} de {} itens",
                    menu.tela.nome(),
                    menu.cursor,
                    total
                );
            }
        }
    }

    #[test]
    fn o_menu_marca_a_partida_e_o_continuar_reaparece_no_lugar_certo() {
        let mut menu = Menu::novo(false);
        menu.abrir(Tela::Pausa);
        menu.marcar_partida(true);
        menu.abrir(Tela::Principal);
        assert_eq!(menu.itens()[0].rotulo, "Continuar");
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Voltar);
        // Sem partida, o primeiro item é Jogar de novo.
        menu.marcar_partida(false);
        assert_eq!(menu.itens()[0].rotulo, "Jogar");
        assert_eq!(passo(&mut menu, Direcao::Confirma), Acao::Jogar);
    }

    #[test]
    fn a_configuracao_saneada_nao_deixa_valor_absurdo_passar() {
        let c = Config {
            volume: 250,
            camera_dist_m: f32::NAN,
            mostrar_fps: true,
        }
        .saneada();
        assert_eq!(c.volume, 100);
        assert_eq!(c.camera_dist_m, Config::default().camera_dist_m);
        assert!(c.mostrar_fps);

        let c = Config { volume: 40, camera_dist_m: 5.0, mostrar_fps: false }.saneada();
        assert_eq!(c.camera_dist_m, CAMERA_MIN_M);
        assert!((c.volume_fracao() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn o_menu_de_pausa_pausa_e_o_de_fim_nao() {
        assert!(Tela::Pausa.pausa_o_jogo());
        assert!(Tela::Configuracoes.pausa_o_jogo());
        assert!(!Tela::Fim { venceu: false }.pausa_o_jogo());
    }
}
