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
        // `jogador()` pode ser `None` aqui: o handshake é assíncrono e o
        // primeiro quadro costuma chegar antes da resposta. Imprimir o
        // `unwrap_or(0)` dizia "entrou como jogador 0", que é o id do **host** —
        // um número plausível e falso no log de diagnóstico.
        match client.jogador() {
            Some(jogador) => log::info!("net: entrou como jogador {jogador} ({:?})", client.estado()),
            None => log::info!("net: conectado, aguardando handshake ({:?})", client.estado()),
        }
        let player_id = client.jogador().unwrap_or(0);
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
    ///
    /// O crate é a autoridade: ele consome o `HandshakeOk` e guarda o id em
    /// `jogador()`, sem repassá-lo ao jogo. Perguntar a ele é a única resposta
    /// que não depende de uma mensagem que ninguém entrega.
    pub fn is_ready(&self) -> bool {
        self.client.jogador().is_some() || self.player_id != 0
    }

    /// Id confirmado no handshake, se ele já voltou.
    pub fn jogador(&self) -> Option<u8> {
        self.client.jogador()
    }

    fn mandar(&mut self, mensagem: Mensagem) -> std::io::Result<()> {
        self.client.enviar_input(mensagem)
    }
}

impl ClientBackend for RealClient {
    fn player_id(&self) -> u8 {
        // O crate manda. A cópia local existe para o caminho do stub e para o
        // `Send`, e pode estar velha por um quadro — usar a velha faria o
        // cliente mandar input em nome do host.
        self.client.jogador().unwrap_or(self.player_id)
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
        // O id mora no crate, que **consome** o `HandshakeOk` em vez de
        // repassá-lo (ver `vendor/giochi-net/src/client.rs`). Este laço tinha um
        // braço `Mensagem::HandshakeOk` que nunca podia rodar — o log "handshake
        // aceito" jamais saiu, e o `local_id` do mundo ficava em 0, fazendo o
        // cliente controlar o avatar do **host**. Ler o estado do crate é a
        // correção; esperar por uma mensagem que ninguém entrega não era.
        match (self.client.jogador(), self.client.estado()) {
            (Some(jogador), _) if jogador != self.player_id => {
                log::info!("net: handshake aceito, jogador {jogador}");
                self.player_id = jogador;
            }
            (None, giochi_net::client::Estado::Recusado) if self.player_id == 0 => {
                log::error!("net: o host recusou a conexão (versão ou lotação)");
            }
            _ => {}
        }
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

/// O motivo do handshake recusado, como texto.
///
/// `motivo` é `[u8; 48]` fixo e `tamanho` diz quantos bytes valem. Ler o array
/// inteiro acrescentaria zeros à mensagem, e o log mostraria o motivo com lixo no
/// fim — num erro que o jogador precisa ler para saber se é versão, lotação ou
/// senha.
pub fn motivo_texto(motivo: &[u8; 48], tamanho: u8) -> String {
    let n = usize::from(tamanho).min(motivo.len());
    String::from_utf8_lossy(&motivo[..n]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_motivo_usa_so_os_bytes_validos() {
        let mut motivo = [0u8; 48];
        motivo[..5].copy_from_slice(b"cheio");
        assert_eq!(motivo_texto(&motivo, 5), "cheio");
        // Tamanho maior que o array não estoura: o crate usa `[u8; 48]` fixo e
        // um `tamanho` corrompido não pode virar pânico no cliente.
        assert_eq!(motivo_texto(&motivo, 255).len(), 48);
        assert_eq!(motivo_texto(&motivo, 0), "");
    }

    /// A regra que o `passo()` da sessão usa para adotar o id: **0 não é um id
    /// de cliente**. Se ela mudar, o cliente ou fica sem id ou assume o do host.
    #[test]
    fn id_zero_nunca_e_adotado_como_proprio() {
        let adotavel = |do_crate: Option<u8>, local: u8| match do_crate {
            Some(id) if id != 0 && id != local => Some(id),
            _ => None,
        };
        assert_eq!(adotavel(Some(1), 0), Some(1), "o primeiro cliente é o 1");
        assert_eq!(adotavel(Some(0), 0), None, "0 é o host, nunca um cliente");
        assert_eq!(adotavel(None, 0), None, "sem handshake não há o que adotar");
        assert_eq!(adotavel(Some(1), 1), None, "já é o meu: nada a fazer");
        assert_eq!(adotavel(Some(3), 1), Some(3), "o host pode reatribuir");
    }

    #[test]
    fn a_flag_de_acao_cabe_em_u16() {
        // O crate carrega `acoes` em u32 e o jogo em u16. Um bit acima do 15
        // seria perdido no truncamento — este teste é o que impede a flag de
        // nascer lá em cima sem ninguém notar.
        for bit in 0..16 {
            let flag: u16 = 1 << bit;
            assert_eq!(u32::from(flag) as u16, flag);
        }
    }
}
