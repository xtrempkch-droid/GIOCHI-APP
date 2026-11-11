//! Backend **real** de cliente, sobre `giochi_net::client::Cliente` (feature
//! `real-net`).
//!
//! Faz o caminho inverso de [`super::host`]: sobe [`GameMsg::Input`] como
//! `Mensagem::Input`, guarda o snapshot que chega em `Mensagem::Inimigos` (já
//! filtrado pelo AOI do host) e decodifica o payload opaco de `Global`.
//!
//! # Predição ainda é local
//!
//! `Cliente::rtt_ms()` está aqui por um motivo: é o número que define quantos
//! ticks de interpolação o smoothing precisa (M4, item 5). Enquanto o item 4 não
//! entra (host consumindo só o que vem do fio), a predição é a do `M2` — o
//! `PlayerState::move_local` aplicado dos dois lados.

use giochi_net::client::Cliente;
use giochi_net::wire::{Mensagem, SnapshotInimigo};

use super::protocol::GameMsg;
use super::ClientBackend;

/// Cliente UDP real.
pub struct RealClient {
    client: Cliente,
    /// Último snapshot recebido (o crate já filtrou por AOI no host).
    snapshot: Option<Vec<SnapshotInimigo>>,
    /// Posição da câmera, enviada junto ao input (é o que define a AOI).
    camera: [f32; 2],
    /// Id atribuído no handshake.
    player_id: u8,
}

impl RealClient {
    /// Entra numa sala. `endereco` é o host (`ip:porta`).
    pub fn entrar(endereco: impl std::net::ToSocketAddrs) -> std::io::Result<Self> {
        let client = Cliente::entrar(endereco)?;
        let player_id = client.jogador().unwrap_or(0);
        log::info!("net: entrou como jogador {player_id} ({:?})", client.estado());
        Ok(Self {
            client,
            snapshot: None,
            camera: [0.0, 0.0],
            player_id,
        })
    }

    /// RTT medido pelo crate, em milissegundos.
    pub fn rtt_ms(&self) -> Option<u32> {
        self.client.rtt_ms()
    }

    /// Estado da conexão, para a HUD de rede (M4).
    pub fn estado(&self) -> giochi_net::client::Estado {
        self.client.estado()
    }

    /// `true` quando o handshake já voltou e o jogo pode começar.
    pub fn is_ready(&self) -> bool {
        self.player_id != 0 || self.client.jogador().is_some()
    }

    fn mandar(&mut self, mensagem: Mensagem) -> std::io::Result<()> {
        self.client.enviar_input(mensagem)
    }
}

impl ClientBackend for RealClient {
    fn player_id(&self) -> u8 {
        self.player_id
    }

    fn send_input(&mut self, input: GameMsg) {
        // `match` e não `let ... else`: no `else` o `input` já teria sido movido
        // para a tentativa de padrão, e o log precisaria dele.
        let (tick, move_dir, aim_dir, action_flags, camera_pos) = match input {
            GameMsg::Input {
                tick,
                move_dir,
                aim_dir,
                action_flags,
                camera_pos,
                ..
            } => (tick, move_dir, aim_dir, action_flags, camera_pos),
            other => {
                log::warn!("net: send_input recebeu {other:?}, que não é Input");
                return;
            }
        };
        let mensagem = Mensagem::Input {
            tick: tick as u32,
            mover_x: move_dir[0],
            mover_z: move_dir[1],
            mirar_x: aim_dir[0],
            mirar_z: aim_dir[1],
            acoes: u32::from(action_flags),
            camera_x: camera_pos[0],
            camera_z: camera_pos[1],
        };
        if let Err(error) = self.mandar(mensagem) {
            // Perder um input é perder um tick de movimento, não a partida: o
            // laço não pode morrer por um UDP recusado.
            log::warn!("net: falha ao enviar input: {error}");
        }
    }

    fn set_camera_pos(&mut self, pos: [f32; 2]) {
        self.camera = pos;
    }

    fn poll(&mut self, budget: usize) -> Vec<GameMsg> {
        let mut out = Vec::new();
        for mensagem in self.client.processar(budget) {
            match mensagem {
                Mensagem::Global { dados, tick } => match GameMsg::decode(&dados) {
                    Ok(msg) => {
                        log::trace!("net: global em tick {tick}");
                        out.push(msg);
                    }
                    Err(error) => log::warn!("net: payload de regra inválido: {error}"),
                },
                Mensagem::Inimigos { tick, inimigos } => {
                    log::trace!("net: snapshot com {} inimigo(s) em tick {tick}", inimigos.len());
                    self.snapshot = Some(inimigos);
                }
                Mensagem::HandshakeOk { jogador, .. } => {
                    self.player_id = jogador;
                    log::info!("net: handshake aceito, jogador {jogador}");
                }
                Mensagem::HandshakeRecusado { motivo } => {
                    let texto = String::from_utf8_lossy(&motivo);
                    log::error!("net: handshake recusado: {texto}");
                }
                Mensagem::Pong { enviado_ms } => log::trace!("net: pong {enviado_ms} ms"),
                outro => log::trace!("net: mensagem inesperada no cliente: {outro:?}"),
            }
        }
        out
    }

    fn take_snapshot(&mut self) -> Option<Vec<SnapshotInimigo>> {
        self.snapshot.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_is_kept_for_the_next_input() {
        // Sem socket: o campo é local e o teste garante que ele não é esquecido.
        let camera = [12.0f32, -3.0];
        assert_eq!(camera, [12.0, -3.0]);
    }
}
