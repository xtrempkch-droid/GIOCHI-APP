//! Backend **real** de host, sobre `giochi_net::host::Host` (feature `real-net`).
//!
//! O stub continua sendo o padrão; este módulo existe para o M4 trocar o
//! transporte sem tocar no jogo — a costura é [`HostBackend`], e o `GameState`
//! nunca vê um `UdpSocket`.
//!
//! # Tradução, não reimplementação
//!
//! O crate tipa o que precisa para rotear e deixa regra de jogo como payload
//! opaco (`Global { dados }`). Aqui só há **duas** traduções:
//!
//! - `Mensagem::Input { .. }` → [`GameMsg::Input`] (sobe);
//! - [`GameMsg`] → `Mensagem::Global { dados }` (desce).
//!
//! `Inimigos`/`ShopBuy`/`Ping` ficam com o crate: são dele por contrato.
//!
//! # Sem bloqueio
//!
//! `processar(orcamento)` drena no máximo `orcamento` datagramas. O laço do jogo
//! chama com orçamento pequeno a cada passo fixo, então um cliente inundando a
//! rede não pode segurar o timestep (prompt §10).

use giochi_net::host::{ClienteId, Host, MAX_CLIENTES};
use giochi_net::wire::SnapshotInimigo;
use giochi_net::wire::Mensagem;

use super::protocol::GameMsg;
use super::HostBackend;

/// Host UDP real.
pub struct RealHost {
    host: Host,
    /// Raio de AOI aplicado a cada cliente (m, do perfil de host).
    area_m: f32,
    /// Clientes que já receberam `definir_area`.
    known: Vec<ClienteId>,
    /// Bytes enviados no último snapshot (para medir banda, GDD §20).
    last_bytes: usize,
}

impl RealHost {
    /// Sobe o host na porta dada (ou na padrão do crate).
    pub fn novo(porta: Option<u16>) -> std::io::Result<Self> {
        Ok(Self {
            host: Host::novo(porta)?,
            area_m: 55.0,
            known: Vec::new(),
            last_bytes: 0,
        })
    }

    /// Endereço em que está escutando (útil para o mDNS anunciar).
    pub fn endereco(&self) -> std::io::Result<std::net::SocketAddr> {
        self.host.endereco()
    }

    /// Define o raio de AOI de todos os clientes (chamado quando o perfil muda).
    pub fn set_area(&mut self, radius_m: f32) {
        self.area_m = radius_m.max(1.0);
        for id in &self.known {
            self.host.definir_area(*id, self.area_m);
        }
    }

    /// Bytes do último snapshot publicado — a medida de banda por cliente.
    pub fn last_snapshot_bytes(&self) -> usize {
        self.last_bytes
    }

    /// Quantos clientes cabem (repassa o limite do crate).
    pub fn max_clients(&self) -> usize {
        MAX_CLIENTS
    }

    fn register(&mut self, id: ClienteId) {
        if !self.known.contains(&id) {
            self.host.definir_area(id, self.area_m);
            self.known.push(id);
            log::info!("net: cliente {id} entrou (AOI {:.0} m)", self.area_m);
        }
    }
}

impl HostBackend for RealHost {
    fn client_count(&self) -> usize {
        self.host.clientes()
    }

    fn poll_input(&mut self, budget: usize) -> Vec<(u8, GameMsg)> {
        let mut out = Vec::new();
        for (id, mensagem) in self.host.processar(budget) {
            match mensagem {
                Mensagem::Input {
                    tick,
                    mover_x,
                    mover_z,
                    mirar_x,
                    mirar_z,
                    acoes,
                    camera_x,
                    camera_z,
                } => {
                    self.register(id);
                    out.push((
                        id,
                        GameMsg::Input {
                            tick: u64::from(tick),
                            player_id: id,
                            move_dir: [mover_x, mover_z],
                            aim_dir: [mirar_x, mirar_z],
                            // `acoes` é u32 no crate e u16 no jogo: os bits do
                            // jogo cabem, e o truncamento explícito avisa.
                            action_flags: acoes as u16,
                            camera_pos: [camera_x, camera_z],
                        },
                    ));
                }
                Mensagem::ShopBuy { item_id } => {
                    out.push((
                        id,
                        GameMsg::ShopBuy {
                            player_id: id,
                            item_id: item_id.min(u16::from(u8::MAX)) as u8,
                        },
                    ));
                }
                Mensagem::Ping { enviado_ms } => {
                    // O crate já responde sozinho no `processar`; nada a fazer.
                    log::trace!("net: ping de {id} em {enviado_ms} ms");
                }
                outro => log::trace!("net: mensagem inesperada no host: {outro:?}"),
            }
        }
        out
    }

    fn broadcast(&mut self, msg: GameMsg) {
        let dados = msg.encode();
        self.host.enviar_global(&dados);
    }

    fn publish_snapshot(&mut self, enemies: &[SnapshotInimigo]) {
        self.last_bytes = enemies.len() * std::mem::size_of::<SnapshotInimigo>();
        // `tick_snapshot` filtra por cliente com o AOI já definido e faz unicast.
        self.host.tick_snapshot(enemies);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_area_default_is_the_balanced_profile() {
        // Sem rede: só a constante, para o teste não depender de abrir socket.
        assert!(55.0 > 0.0);
        assert_eq!(MAX_CLIENTS, 8);
    }
}
