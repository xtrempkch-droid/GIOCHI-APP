//! Rede: fachada, protocolo, AOI e o stub de loopback.
//!
//! # Duas metades
//!
//! | Camada | Quem faz | Onde |
//! |---|---|---|
//! | transporte, filtro por AOI, handshake, ping | `giochi-net` (crate da engine) | `vendor/giochi-net` |
//! | payload de regra (input, onda, torre, loja, nemesis) | este jogo | [`protocol`] |
//!
//! A separação é a do próprio crate: ele carrega bytes opacos de regra e tipa só
//! o que precisa para rotear. O snapshot de inimigos usa
//! `giochi_net::wire::SnapshotInimigo` — um único formato para o dado.
//!
//! # Por que os traits existem
//!
//! O prompt exige que trocar o stub pela implementação real **não toque no jogo**.
//! [`ClientBackend`] e [`HostBackend`] são essa costura: o M4 implementa os dois
//! sobre `giochi_net::client::Cliente` e `giochi_net::host::Host`, e `main.rs`
//! continua chamando os mesmos métodos.
//!
//! # O que o stub **não** faz
//!
//! Não abre socket, não descobre ninguém por mDNS, não faz predição nem
//! reconciliação. Tudo isso é M4 e vive do outro lado da mesma costura.

pub mod aoi;
pub mod descoberta;
pub mod protocol;

#[cfg(feature = "stub-net")]
pub mod stub;

// M4: transporte real sobre o `giochi-net`. Sempre compilado desde 13/11/2026 —
// ver a nota em `Cargo.toml` sobre por que a feature saiu.
pub mod client;
pub mod host;
pub mod sessao;

pub use descoberta::{procurar, Sala};
pub use sessao::SessaoJogo;

pub use protocol::{
    action_flags, GameMsg, NetMode, ACTION_ABILITY1, ACTION_ABILITY2, ACTION_ATTACK, ACTION_ITEM,
    ACTION_PAUSE, ACTION_SHOP, DATAGRAM_MAX, PROTOCOL_VERSION, SNAPSHOT_HZ,
};

use giochi_net::wire::SnapshotInimigo;

/// Papel desta instância na partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Simula o mundo.
    Host,
    /// Manda input e desenha o que recebe.
    Client,
}

/// Lado cliente da costura de rede.
pub trait ClientBackend {
    /// Id atribuído na entrada (0 = primeiro cliente, nunca o host).
    fn player_id(&self) -> u8;

    /// Envia a intenção do jogador.
    fn send_input(&mut self, input: GameMsg);

    /// Informa a posição da câmera (é o que define a AOI deste cliente).
    fn set_camera_pos(&mut self, pos: [f32; 2]);

    /// Drena até `budget` mensagens de broadcast. Nunca bloqueia.
    fn poll(&mut self, budget: usize) -> Vec<GameMsg>;

    /// Último snapshot de inimigos visível para este cliente.
    fn take_snapshot(&mut self) -> Option<Vec<SnapshotInimigo>>;
}

/// Lado host da costura de rede.
pub trait HostBackend {
    /// Quantos clientes estão na sala.
    fn client_count(&self) -> usize;

    /// Drena até `budget` inputs, com o id de quem mandou.
    fn poll_input(&mut self, budget: usize) -> Vec<(u8, GameMsg)>;

    /// Manda uma mensagem para todos.
    fn broadcast(&mut self, msg: GameMsg);

    /// Publica o snapshot de inimigos (o host filtra por AOI).
    fn publish_snapshot(&mut self, enemies: &[SnapshotInimigo]);
}

#[cfg(feature = "stub-net")]
impl ClientBackend for stub::StubClient {
    fn player_id(&self) -> u8 {
        self.player_id()
    }

    fn send_input(&mut self, input: GameMsg) {
        stub::net_send_input(self, input);
    }

    fn set_camera_pos(&mut self, pos: [f32; 2]) {
        stub::net_set_camera_pos(self, pos);
    }

    fn poll(&mut self, budget: usize) -> Vec<GameMsg> {
        stub::net_client_poll(self, budget)
    }

    fn take_snapshot(&mut self) -> Option<Vec<SnapshotInimigo>> {
        stub::net_poll_snapshot(self)
    }
}

#[cfg(feature = "stub-net")]
impl HostBackend for stub::StubHost {
    fn client_count(&self) -> usize {
        self.clients()
    }

    fn poll_input(&mut self, budget: usize) -> Vec<(u8, GameMsg)> {
        stub::net_host_poll_input(self, budget)
    }

    fn broadcast(&mut self, msg: GameMsg) {
        stub::net_host_broadcast(self, msg);
    }

    fn publish_snapshot(&mut self, enemies: &[SnapshotInimigo]) {
        stub::net_host_publish_snapshot(self, enemies);
    }
}

/// Sessão local: um host e um cliente de loopback no mesmo processo.
///
/// É o modo single-player do M2. Com `--no-default-features` (sem `stub-net`) não
/// existe sessão, e o jogo desenha direto o estado do host.
#[cfg(feature = "stub-net")]
pub fn local_session() -> Option<(stub::StubHost, stub::StubClient)> {
    let host = stub::net_host_create();
    let client = stub::net_client_join(&host)?;
    log::info!("rede: stub local — jogador {}", client.player_id());
    Some((host, client))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "stub-net")]
    #[test]
    fn the_traits_are_implemented_by_the_stub() {
        let host = stub::net_host_create();
        let mut client = stub::net_client_join(&host).expect("entra");
        assert_eq!(ClientBackend::player_id(&client), 0);
        ClientBackend::set_camera_pos(&mut client, [10.0, 10.0]);
        ClientBackend::send_input(
            &mut client,
            GameMsg::Ping { timestamp: 1 },
        );

        let mut host = host;
        let inputs = HostBackend::poll_input(&mut host, 4);
        assert_eq!(inputs.len(), 1);

        HostBackend::publish_snapshot(&mut host, &[]);
        assert!(HostBackend::client_count(&host) == 1);
        assert!(ClientBackend::take_snapshot(&mut client).is_some());
    }
}
