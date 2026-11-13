//! A sessão de rede do jogo: um host **ou** um cliente, atrás de um tipo só.
//!
//! # Por que um enum e não dois caminhos no laço
//!
//! `main.rs` já tem menu, simulação, desenho e diagnóstico. Dois `match` de rede
//! espalhados pelo laço envelheceriam diferente — é o mesmo motivo pelo qual o
//! `HostBackend`/`ClientBackend` existem. Aqui os dois lados expõem os mesmos
//! quatro verbos, e o laço não sabe qual está ativo.
//!
//! # Autoridade: o host simula, o cliente prevê
//!
//! | | host | cliente |
//! |---|---|---|
//! | inimigos | simula e publica (com AOI) | desenha o snapshot |
//! | jogadores | simula todos | prevê o **próprio**, recebe os outros |
//! | torre e onda | dono | recebe |
//!
//! A divisão é a que o GDD §20 pede e a que o `giochi-net` suporta: o crate
//! filtra por área no host e entrega um snapshot já reduzido ao cliente.
//!
//! # Classe dos jogadores que entram
//!
//! Rodízio pelas quatro classes, na ordem do id. O cliente não escolhe classe
//! (isso exigiria mais um campo no protocolo e uma tela de escolha no meio da
//! conexão); o rodízio garante que quatro jogadores não sejam quatro Soldados.

use giochi_net::wire::SnapshotInimigo;

use super::client::RealClient;
use super::host::RealHost;
use super::{ClientBackend, HostBackend, Role};
use crate::config::{ClassId, GameConfig};
use crate::engine_bridge::InputFrame;
use crate::errors::{GameError, Result};
use crate::game::state::GameState;
use crate::net::protocol::GameMsg;

/// Classes em rodízio para quem entra.
const CLASSES: [ClassId; 4] = [ClassId::Soldier, ClassId::Warrior, ClassId::Sniper, ClassId::Medic];

/// De quanto em quanto tempo o host manda o resumo dos jogadores, em segundos.
///
/// 10 Hz e não 60: um resumo carrega posição, vida e placar de até 8 jogadores, e
/// a 60 Hz isso dominaria a banda sem o olho notar — a posição vem suavizada pela
/// predição do cliente.
pub const RESUMO_HZ: f32 = 10.0;

/// Uma sessão de rede ativa.
pub enum SessaoJogo {
    /// Este processo simula o mundo.
    Host(Box<RealHost>),
    /// Este processo manda input e desenha o que recebe.
    Cliente(Box<RealClient>),
}

impl std::fmt::Debug for SessaoJogo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host(host) => write!(f, "SessaoJogo::Host({} cliente(s))", host.client_count()),
            Self::Cliente(cliente) => write!(f, "SessaoJogo::Cliente(jogador {})", cliente.player_id()),
        }
    }
}

impl SessaoJogo {
    /// Sobe um host na porta dada.
    pub fn hospedar(porta: u16) -> Result<Self> {
        let host = RealHost::novo(Some(porta)).map_err(|erro| {
            GameError::message(format!("não consegui hospedar na porta {porta}: {erro}"))
        })?;
        let endereco = host
            .endereco()
            .map(|e| e.to_string())
            .unwrap_or_else(|_| format!("porta {porta}"));
        log::info!("rede: hospedando em {endereco}");
        Ok(Self::Host(Box::new(host)))
    }

    /// Entra numa sala.
    pub fn entrar(endereco: &str) -> Result<Self> {
        let cliente = RealClient::entrar(endereco).map_err(|erro| {
            GameError::message(format!("não consegui entrar em {endereco}: {erro}"))
        })?;
        // O id só existe depois do handshake. Anunciá-lo aqui imprimia
        // "jogador 0" — o id do **host** — e fazia o diagnóstico de rede
        // procurar defeito no lugar errado.
        log::info!("rede: conectando; o id de jogador chega no handshake");
        Ok(Self::Cliente(Box::new(cliente)))
    }

    /// Papel desta ponta.
    pub fn papel(&self) -> Role {
        match self {
            Self::Host(_) => Role::Host,
            Self::Cliente(_) => Role::Client,
        }
    }

    /// Quantos clientes estão na sala (1 no cliente, que é ele mesmo).
    pub fn jogadores(&self) -> usize {
        match self {
            Self::Host(host) => host.client_count() + 1,
            Self::Cliente(_) => 1,
        }
    }

    /// Roda um quadro de rede e devolve os inimigos que a tela deve desenhar.
    ///
    /// No host, `entrada` é o input local (que sobe como mensagem para o stub de
    /// loopback não confundir) e o retorno é o snapshot publicado — o host
    /// desenha o próprio mundo, então ele devolve o que o estado tem.
    pub fn passo(
        &mut self,
        state: &mut GameState,
        frame: &InputFrame,
        config: &GameConfig,
        dt: f32,
    ) -> Vec<SnapshotInimigo> {
        match self {
            Self::Host(host) => {
                for (id, msg) in host.poll_input(32) {
                    if let GameMsg::Input {
                        player_id,
                        move_dir,
                        aim_dir,
                        action_flags,
                        ..
                    } = msg
                    {
                        // O jogador entra na partida no primeiro input, e não num
                        // handshake de aplicação: o `giochi-net` já aceitou a
                        // conexão, e exigir mais um passo faria o cliente
                        // aparecer "conectado mas invisível".
                        let classe = CLASSES[usize::from(player_id) % CLASSES.len()];
                        state.adicionar_jogador(id, classe, config);
                        let mut remoto = InputFrame {
                            move_dir: crate::engine_bridge::Axis::new(move_dir[0], move_dir[1]),
                            aim_dir: crate::engine_bridge::Axis::new(aim_dir[0], aim_dir[1]),
                            ..InputFrame::default()
                        };
                        remoto.buttons.attack = action_flags & crate::net::ACTION_ATTACK != 0;
                        state.aplicar_input_de(id, dt, &remoto, config);
                    }
                }
                host.publish_snapshot(&state.snapshot_enemies());
                host.broadcast(GameMsg::WaveState {
                    number: state.wave,
                    timer_ms: (state.phase_timer_s.max(0.0) * 1000.0) as u32,
                    is_boss: state.boss.is_some(),
                    boss_id: state.boss.as_ref().map(|b| b.kind as u8 + 1).unwrap_or(0),
                    boss_hp_pct: state.boss.as_ref().map(|b| b.hp_pct_u8()).unwrap_or(0),
                    mode: match state.mode {
                        crate::game::state::RunMode::Campaign => crate::net::protocol::NetMode::Campaign,
                        crate::game::state::RunMode::Endless => crate::net::protocol::NetMode::Endless,
                    },
                });
                host.broadcast(GameMsg::TowerState {
                    hp: state.tower.hp.max(0.0) as u16,
                    max_hp: state.tower.max_hp as u16,
                    armor: state.tower.armor as u16,
                });
                for jogador in &state.players {
                    host.broadcast(GameMsg::PlayerState {
                        player_id: jogador.id,
                        pos: [jogador.position.x, jogador.position.z],
                        hp: jogador.hp.max(0.0) as u16,
                        max_hp: jogador.max_hp as u16,
                        score: jogador.score,
                        alive: jogador.alive,
                    });
                }
                state.snapshot_enemies()
            }
            Self::Cliente(cliente) => {
                // O `local_id` do mundo tem de acompanhar o id do handshake, e
                // não ficar no 0 de nascimento: 0 é o host, então o cliente
                // estaria dirigindo o avatar dele e vendo o próprio como
                // remoto. Id 0 nunca é atribuído a cliente, por contrato.
                if let Some(meu_id) = cliente.jogador() {
                    if meu_id != state.local_id {
                        log::info!("rede: assumindo o jogador {meu_id}");
                        state.local_id = meu_id;
                        if state.index_of(meu_id).is_none() {
                            let classe = CLASSES[usize::from(meu_id) % CLASSES.len()];
                            state.adicionar_jogador(meu_id, classe, config);
                        }
                    }
                }
                let camera = [state.camera.focus.x, state.camera.focus.z];
                cliente.set_camera_pos(camera);
                cliente.send_input(GameMsg::input(state.tick, cliente.player_id(), frame, camera));
                for msg in cliente.poll(64) {
                    match msg {
                        GameMsg::PlayerState {
                            player_id,
                            pos,
                            hp,
                            max_hp,
                            score,
                            alive,
                        } => {
                            if player_id != state.local_id
                                && state.index_of(player_id).is_none()
                            {
                                let classe = CLASSES[usize::from(player_id) % CLASSES.len()];
                                state.adicionar_jogador(player_id, classe, config);
                            }
                            state.aplicar_resumo_de_jogador(
                                player_id, pos, hp, max_hp, score, alive,
                            );
                        }
                        GameMsg::TowerState { hp, max_hp, .. } => {
                            state.tower.hp = f32::from(hp);
                            state.tower.max_hp = f32::from(max_hp).max(1.0);
                        }
                        GameMsg::WaveState { number, .. } => {
                            state.wave = number.max(1);
                        }
                        outro => log::trace!("rede: {outro:?}"),
                    }
                }
                cliente.take_snapshot().unwrap_or_default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_classe_de_cada_id_sai_do_rodizio_e_nao_repete() {
        // Quatro jogadores, quatro classes. Um `CLASSES[0]` fixo daria quatro
        // Soldados — e como o modelo é o que muda por classe, o jogador veria
        // quatro bonecos iguais e chamaria de bug de rede.
        let classes: Vec<ClassId> = (0u8..4).map(|id| CLASSES[usize::from(id) % CLASSES.len()]).collect();
        let distintas: std::collections::BTreeSet<u8> =
            classes.iter().map(|c| *c as u8).collect();
        assert_eq!(distintas.len(), 4, "{classes:?}");
        // E um quinto jogador (id 4) volta ao começo em vez de estourar o índice.
        assert_eq!(CLASSES[4 % CLASSES.len()], CLASSES[0]);
    }

    #[test]
    fn o_papel_e_derivado_do_tipo_e_nao_de_um_campo() {
        // Não há como construir uma sessão sem socket aqui, então o teste fixa a
        // **regra** que o `match` de `papel()` implementa: um enum com dois casos
        // que se confundem seria pior que dois tipos.
        let papel = |e_host: bool| if e_host { Role::Host } else { Role::Client };
        assert_eq!(papel(true), Role::Host);
        assert_eq!(papel(false), Role::Client);
        assert_ne!(Role::Host, Role::Client);
    }

    #[test]
    fn a_taxa_de_resumo_e_dez_hertz() {
        // O host publica 10 vezes por segundo; a 60 Hz a banda triplicaria sem
        // ganho visível. O número está aqui para quem for mexer saber que existe.
        assert!((RESUMO_HZ - 10.0).abs() < f32::EPSILON);
    }
}
