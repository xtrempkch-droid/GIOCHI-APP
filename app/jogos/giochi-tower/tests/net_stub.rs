//! Testes de integração da rede (M2).
//!
//! O critério de aceitação do prompt é literal: *"host+client trocam
//! Input/EnemySnapshot sem deadlock"*. Aqui isso é exercitado pela API pública do
//! crate do jogo — `giochi_tower::net` — e não pelos detalhes internos do stub:
//! se o M4 trocar o stub pelo `giochi-net` real, estes testes continuam sendo a
//! especificação do que o backend tem de fazer.
//!
//! Não há thread, socket nem `sleep`: um deadlock aqui aparece como teste que não
//! termina, e o orçamento de drenagem (`budget`) é a garantia de que ninguém
//! espera por mensagem que não vem.

use giochi_net::wire::SnapshotInimigo;
use giochi_tower::net::{aoi, protocol::GameMsg, stub, ClientBackend, HostBackend};

fn snapshot(id: u16, x: f32, z: f32) -> SnapshotInimigo {
    SnapshotInimigo {
        id,
        x,
        z,
        hp: 65_535,
        tipo: 0,
        fase_chefe: 0,
    }
}

#[test]
fn host_and_client_exchange_input_and_snapshot_without_deadlock() {
    let mut host = stub::net_host_create();
    let mut client = stub::net_client_join(&host).expect("cabe um cliente");

    // 5 s de jogo a 60 Hz, com o laço na ordem do jogo real.
    for tick in 0..300u64 {
        client.set_camera_pos([0.0, 0.0]);
        client.send_input(GameMsg::input(
            tick,
            client.player_id(),
            &giochi_tower::engine_bridge::InputFrame::default(),
            [0.0, 0.0],
        ));

        let inputs = host.poll_input(8);
        assert_eq!(inputs.len(), 1, "um input por tick, sem duplicar");
        assert_eq!(inputs[0].0, client.player_id());

        host.publish_snapshot(&[snapshot(1, 0.0, 0.0)]);
        host.broadcast(GameMsg::Ping {
            timestamp: tick as u32,
        });

        let messages = client.poll(8);
        assert_eq!(messages.len(), 1, "um broadcast por tick");

        let visible = client.take_snapshot().expect("houve snapshot");
        assert_eq!(visible.len(), 1);
    }

    // Drenar mais do que existe devolve vazio, e não bloqueia.
    assert!(host.poll_input(1_000).is_empty());
    assert!(client.poll(1_000).is_empty());
}

#[test]
fn the_snapshot_respects_the_interest_area() {
    let mut host = stub::net_host_create();
    let mut client = stub::net_client_join(&host).expect("entra");
    client.set_camera_pos([0.0, 0.0]);

    let enemies = vec![
        snapshot(1, 5.0, 0.0),   // perto
        snapshot(2, 0.0, 5.0),   // perto
        snapshot(3, 300.0, 0.0), // longe
        snapshot(4, 0.0, -300.0) // longe
    ];
    host.publish_snapshot(&enemies);

    let visible = client.take_snapshot().expect("snapshot");
    assert_eq!(visible.len(), 2, "só os dois de perto");
    assert!(visible.iter().all(|e| e.id <= 2));
}

#[test]
fn four_clients_fit_and_the_fifth_is_refused() {
    let host = stub::net_host_create();
    let mut ids = Vec::new();
    for _ in 0..stub::MAX_CLIENTS {
        let client = stub::net_client_join(&host).expect("cabe");
        ids.push(client.player_id());
    }
    assert_eq!(ids, vec![0, 1, 2, 3], "GDD §1: 2 a 4 jogadores");
    assert!(stub::net_client_join(&host).is_none());
    assert_eq!(host.client_count(), stub::MAX_CLIENTS);
}

#[test]
fn the_client_side_aoi_is_a_stricter_second_filter() {
    let mut host = stub::net_host_create();
    let mut client = stub::net_client_join(&host).expect("entra");
    client.set_camera_pos([0.0, 0.0]);
    host.publish_snapshot(&[snapshot(1, 0.0, 0.0), snapshot(2, 20.0, 0.0)]);

    let everything = client.take_snapshot().expect("snapshot");
    assert_eq!(everything.len(), 2);

    // Filtro do cliente mais apertado: só o que está a 1 m da origem.
    let narrow = aoi::area_at(glam::Vec3::ZERO, 1.0);
    let filtered = aoi::filter(&everything, &narrow);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, 1);
}

#[test]
fn an_input_survives_the_wire_roundtrip() {
    // O stub entrega em memória, mas a mensagem é codificada e decodificada no
    // caminho do jogo: um campo que não serializa apareceria aqui.
    let original = GameMsg::Input {
        tick: 42,
        player_id: 1,
        move_dir: [0.25, -0.5],
        aim_dir: [1.0, 0.0],
        action_flags: 1,
        camera_pos: [12.0, -30.0],
    };
    let bytes = original.encode();
    assert!(bytes.len() <= 32, "input ocupa {} bytes", bytes.len());
    assert_eq!(GameMsg::decode(&bytes).expect("decodifica"), original);
}
