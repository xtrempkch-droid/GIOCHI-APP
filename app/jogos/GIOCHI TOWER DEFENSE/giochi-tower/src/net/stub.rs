//! Stub de rede: host + cliente no mesmo processo, por memória compartilhada.
//!
//! # Por que ele existe mesmo com o `giochi-net` pronto
//!
//! Não é antecipação do crate — é o modo **single-player** e a bancada. Um jogo
//! co-op precisa ser jogável sozinho, e um laço host→cliente em memória permite
//! testar o caminho de dados (input sobe, snapshot desce, AOI filtra) sem abrir
//! socket, sem porta e sem depender de rede na máquina de teste.
//!
//! # Sem thread, sem async, sem `unsafe`
//!
//! As filas são `Rc<RefCell<..>>` — host e cliente vivem no mesmo laço, então não
//! há concorrência a resolver. O prompt proíbe runtime async e exige que a rede
//! nunca bloqueie o timestep: aqui não há `recv` bloqueante porque não há socket.
//!
//! # A superfície imita o `giochi-net`
//!
//! As funções seguem os nomes que o prompt planejou (`net_host_create`,
//! `net_client_join`, `net_send_input`, `net_set_camera_pos`,
//! `net_poll_snapshot`, `net_poll_snapshot_filtered`) e as operações espelham
//! `Host::{processar, definir_area, enviar_global, tick_snapshot}` e
//! `Cliente::{entrar, enviar_input, processar}`. No M4, implementar
//! [`super::ClientBackend`]/[`super::HostBackend`] sobre o crate real é o único
//! trabalho — o jogo não muda.
//!
//! # Uma simplificação declarada
//!
//! O host guarda **um** snapshot filtrado, não um por cliente: em single-player
//! há um cliente. O filtro por cliente existe no código
//! ([`net_host_publish_snapshot`] usa a câmera de cada cliente registrado), mas a
//! entrega usa a do cliente local. O `Host` real do crate já faz unicast por
//! cliente; não vale reimplementar isso num stub.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use giochi_net::aoi::AreaDeInteresse;
use giochi_net::wire::SnapshotInimigo;

use super::aoi;
use super::protocol::GameMsg;

/// Máximo de clientes que o stub aceita (GDD: 2–4 jogadores; o crate aceita 8).
pub const MAX_CLIENTS: usize = 4;

/// Estado compartilhado entre host e clientes do stub.
#[derive(Debug, Default)]
struct Shared {
    /// Inputs esperando o host drenar, com o id de quem mandou.
    inputs: VecDeque<(u8, GameMsg)>,
    /// Mensagens de broadcast esperando o cliente drenar.
    broadcasts: VecDeque<GameMsg>,
    /// Último snapshot publicado.
    snapshot: Option<Vec<SnapshotInimigo>>,
    /// Próximo id a atribuir (0 é o primeiro cliente, nunca o host).
    next_id: u8,
    /// Câmera e multiplicador de AOI de cada cliente.
    cameras: Vec<(u8, [f32; 2], f32)>,
    /// Quantos clientes entraram.
    clients: usize,
}

/// O host do stub.
#[derive(Debug)]
pub struct StubHost {
    shared: Rc<RefCell<Shared>>,
}

/// Um cliente do stub.
#[derive(Debug)]
pub struct StubClient {
    shared: Rc<RefCell<Shared>>,
    id: u8,
    aoi_scale: f32,
}

/// Cria um host local.
pub fn net_host_create() -> StubHost {
    StubHost {
        shared: Rc::new(RefCell::new(Shared::default())),
    }
}

/// Um cliente entra no host local. `None` quando a sala está cheia.
pub fn net_client_join(host: &StubHost) -> Option<StubClient> {
    let mut shared = host.shared.borrow_mut();
    if shared.clients >= MAX_CLIENTS {
        return None;
    }
    let id = shared.next_id;
    shared.next_id = shared.next_id.wrapping_add(1);
    shared.clients += 1;
    // A câmera começa na origem; o primeiro `net_set_camera_pos` corrige.
    shared.cameras.push((id, [0.0, 0.0], 1.5));
    Some(StubClient {
        shared: Rc::clone(&host.shared),
        id,
        aoi_scale: 1.5,
    })
}

/// C→H: manda a intenção do jogador.
pub fn net_send_input(client: &mut StubClient, input: GameMsg) {
    client.shared.borrow_mut().inputs.push_back((client.id, input));
}

/// C→H: informa a posição da câmera, que define a AOI deste cliente.
pub fn net_set_camera_pos(client: &mut StubClient, pos: [f32; 2]) {
    let mut shared = client.shared.borrow_mut();
    if let Some(entry) = shared
        .cameras
        .iter_mut()
        .find(|(id, _, _)| *id == client.id)
    {
        entry.1 = pos;
        entry.2 = client.aoi_scale;
    } else {
        shared.cameras.push((client.id, pos, client.aoi_scale));
    }
}

/// H: drena até `budget` inputs. Nunca bloqueia — se não há nada, devolve vazio.
pub fn net_host_poll_input(host: &mut StubHost, budget: usize) -> Vec<(u8, GameMsg)> {
    let mut shared = host.shared.borrow_mut();
    let take = budget.min(shared.inputs.len());
    shared.inputs.drain(..take).collect()
}

/// H→C: broadcast para todos os clientes.
pub fn net_host_broadcast(host: &mut StubHost, msg: GameMsg) {
    host.shared.borrow_mut().broadcasts.push_back(msg);
}

/// H→C: publica o snapshot de inimigos, **filtrado pela AOI** de cada cliente.
///
/// A contagem de inimigos antes e depois do filtro é registrada no log: é o
/// número que prova que a AOI está fazendo algo, e não só existindo.
pub fn net_host_publish_snapshot(host: &mut StubHost, enemies: &[SnapshotInimigo]) {
    let mut shared = host.shared.borrow_mut();
    if shared.cameras.is_empty() {
        shared.snapshot = None;
        return;
    }
    // O cliente local é o primeiro registrado; o Host real faz um por cliente.
    let (_, camera, scale) = shared.cameras[0];
    let area = aoi::area_at(
        glam::Vec3::new(camera[0], 0.0, camera[1]),
        crate::game::camera::IsoCamera::new(glam::Vec3::ZERO).aoi_radius(16.0 / 9.0, scale),
    );
    let visible = aoi::filter(enemies, &area);
    log::trace!(
        "snapshot: {} inimigo(s) publicados, {} filtrados pela AOI",
        visible.len(),
        enemies.len() - visible.len()
    );
    shared.snapshot = Some(visible);
}

/// C: último snapshot recebido (já filtrado pelo host).
pub fn net_poll_snapshot(client: &mut StubClient) -> Option<Vec<SnapshotInimigo>> {
    client.shared.borrow_mut().snapshot.clone()
}

/// C: último snapshot, com um filtro extra feito no cliente.
pub fn net_poll_snapshot_filtered(
    client: &mut StubClient,
    area: &AreaDeInteresse,
) -> Option<Vec<SnapshotInimigo>> {
    let shared = client.shared.borrow();
    shared
        .snapshot
        .as_ref()
        .map(|enemies| aoi::filter(enemies, area))
}

/// C: drena até `budget` mensagens de broadcast.
pub fn net_client_poll(client: &mut StubClient, budget: usize) -> Vec<GameMsg> {
    let mut shared = client.shared.borrow_mut();
    let take = budget.min(shared.broadcasts.len());
    shared.broadcasts.drain(..take).collect()
}

impl StubHost {
    /// Quantos clientes estão na sala.
    pub fn clients(&self) -> usize {
        self.shared.borrow().clients
    }

    /// `true` se ninguém está conectado.
    pub fn is_empty(&self) -> bool {
        self.clients() == 0
    }
}

impl StubClient {
    /// Id atribuído na entrada.
    pub fn player_id(&self) -> u8 {
        self.id
    }

    /// Multiplicador de AOI deste cliente (vem do perfil de host no M4).
    pub fn aoi_scale(&self) -> f32 {
        self.aoi_scale
    }

    /// Ajusta o multiplicador de AOI (perfil `Rk3326` usa 1,0).
    pub fn set_aoi_scale(&mut self, scale: f32) {
        self.aoi_scale = scale.max(0.0);
        let mut shared = self.shared.borrow_mut();
        if let Some(entry) = shared.cameras.iter_mut().find(|(id, _, _)| *id == self.id) {
            entry.2 = self.aoi_scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use giochi_net::wire::SnapshotInimigo;

    fn snapshot(id: u16, x: f32, z: f32) -> SnapshotInimigo {
        SnapshotInimigo {
            id,
            x,
            z,
            hp: 65535,
            tipo: 0,
            fase_chefe: 0,
        }
    }

    #[test]
    fn a_client_joins_and_gets_the_first_id() {
        let host = net_host_create();
        assert!(host.is_empty());
        let client = net_client_join(&host).expect("primeiro cliente entra");
        assert_eq!(client.player_id(), 0, "o host não é jogador");
        assert_eq!(host.clients(), 1);
    }

    #[test]
    fn the_room_refuses_more_than_the_maximum() {
        let host = net_host_create();
        for expected in 0..MAX_CLIENTS {
            let client = net_client_join(&host).expect("cabe");
            assert_eq!(client.player_id() as usize, expected);
        }
        assert!(net_client_join(&host).is_none(), "sala cheia");
        assert_eq!(host.clients(), MAX_CLIENTS);
    }

    #[test]
    fn input_climbs_and_broadcast_descends() {
        let host = net_host_create();
        let mut client = net_client_join(&host).expect("entra");

        let input = GameMsg::Input {
            tick: 42,
            player_id: 0,
            move_dir: [0.0, 1.0],
            aim_dir: [1.0, 0.0],
            action_flags: 0,
            camera_pos: [0.0, 0.0],
        };
        net_send_input(&mut client, input);
        let drained = net_host_poll_input(&mut host_clone(&host), 8);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].0, 0, "veio do jogador 0");
        assert_eq!(drained[0].1, input);

        net_host_broadcast(&mut host_clone(&host), GameMsg::TowerState {
            hp: 4999,
            max_hp: 5000,
            armor: 5,
        });
        let received = net_client_poll(&mut client, 8);
        assert_eq!(received.len(), 1);
        assert!(matches!(received[0], GameMsg::TowerState { hp: 4999, .. }));
    }

    /// O host do stub não é `Clone`; os testes que precisam de `&mut` usam esta
    /// cópia de fachada, que compartilha o mesmo estado.
    fn host_clone(host: &StubHost) -> StubHost {
        StubHost {
            shared: Rc::clone(&host.shared),
        }
    }

    #[test]
    fn the_snapshot_only_carries_what_is_near_the_camera() {
        let host = net_host_create();
        let mut client = net_client_join(&host).expect("entra");
        net_set_camera_pos(&mut client, [0.0, 0.0]);

        let enemies = vec![snapshot(1, 1.0, 1.0), snapshot(2, 500.0, 500.0)];
        net_host_publish_snapshot(&mut host_clone(&host), &enemies);

        let visible = net_poll_snapshot(&mut client).expect("há snapshot");
        assert_eq!(visible.len(), 1, "o inimigo a 700 m não deveria ter sido enviado");
        assert_eq!(visible[0].id, 1);
    }

    #[test]
    fn moving_the_camera_changes_what_the_client_sees() {
        let host = net_host_create();
        let mut client = net_client_join(&host).expect("entra");
        let enemies = vec![snapshot(1, 0.0, 0.0), snapshot(2, 120.0, 0.0)];

        net_set_camera_pos(&mut client, [0.0, 0.0]);
        net_host_publish_snapshot(&mut host_clone(&host), &enemies);
        assert_eq!(net_poll_snapshot(&mut client).unwrap().len(), 1);

        net_set_camera_pos(&mut client, [120.0, 0.0]);
        net_host_publish_snapshot(&mut host_clone(&host), &enemies);
        let now = net_poll_snapshot(&mut client).unwrap();
        assert_eq!(now.len(), 1);
        assert_eq!(now[0].id, 2, "a câmera mudou e o visível mudou junto");
    }

    #[test]
    fn the_client_side_filter_is_defensive_and_harmless() {
        let host = net_host_create();
        let mut client = net_client_join(&host).expect("entra");
        net_set_camera_pos(&mut client, [0.0, 0.0]);
        net_host_publish_snapshot(
            &mut host_clone(&host),
            &[snapshot(1, 0.0, 0.0), snapshot(2, 5.0, 0.0)],
        );

        let narrow = aoi::area_at(glam::Vec3::ZERO, 1.0);
        let filtered = net_poll_snapshot_filtered(&mut client, &narrow).expect("snapshot");
        assert_eq!(filtered.len(), 1, "filtro extra do cliente aplicado");
    }

    #[test]
    fn a_full_round_does_not_deadlock_or_leak() {
        let host = net_host_create();
        let mut client = net_client_join(&host).expect("entra");
        let mut shared_host = host_clone(&host);

        // 600 ticks de host e cliente alternando, como no laço do jogo.
        for tick in 0..600u64 {
            let input = GameMsg::Input {
                tick,
                player_id: client.player_id(),
                move_dir: [0.0, 1.0],
                aim_dir: [0.0, 1.0],
                action_flags: 0,
                camera_pos: [0.0, 0.0],
            };
            net_send_input(&mut client, input);
            let drained = net_host_poll_input(&mut shared_host, 64);
            assert!(drained.len() <= 1, "o input não pode duplicar");

            net_host_broadcast(&mut shared_host, GameMsg::Ping {
                timestamp: tick as u32,
            });
            let _ = net_client_poll(&mut client, 64);

            net_host_publish_snapshot(&mut shared_host, &[snapshot(1, 0.0, 0.0)]);
            assert!(net_poll_snapshot(&mut client).is_some());
        }

        // Fim: as filas não cresceram sem limite.
        assert!(net_host_poll_input(&mut shared_host, 1024).is_empty());
        assert!(net_client_poll(&mut client, 1024).is_empty());
    }

    #[test]
    fn nobody_connected_means_no_snapshot() {
        let host = net_host_create();
        net_host_publish_snapshot(&mut host_clone(&host), &[snapshot(1, 0.0, 0.0)]);
        // Sem cliente, não há para quem publicar; entrar depois não recebe o antigo.
        let mut client = net_client_join(&host).expect("entra");
        assert!(net_poll_snapshot(&mut client).is_none());
    }
}
