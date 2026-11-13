//! Descoberta de salas na rede local, sem digitar IP.
//!
//! # Por que existe
//!
//! O `giochi-net` não traz mDNS (a feature puxa `mdns-sd`, que puxa `if-addrs` e
//! `socket2`, e nenhum dos dois está disponível offline — ver `docs/REDE.md` do
//! repositório da engine). Digitar um endereço num controle de d-pad seria pior
//! que não ter multiplayer.
//!
//! Então a descoberta é **própria** e mínima: um datagrama de pergunta e um de
//! resposta, em UDP broadcast. Trinta linhas de protocolo, nenhuma dependência,
//! e testável em loopback.
//!
//! # O protocolo, inteiro
//!
//! ```text
//! cliente → 255.255.255.255:7778   "GIOCHI-TOWER?"      (broadcast + loopback)
//! host    → remetente              "GIOCHI-TOWER! <porta> <nome>"
//! ```
//!
//! A pergunta vai também para `127.0.0.1` porque broadcast em alguns roteadores
//! e em Wi-Fi com isolamento de cliente não volta; o loopback cobre o caso de
//! dois processos na mesma máquina, que é como os testes rodam.
//!
//! # O que ele não faz
//!
//! Não atravessa NAT, não responde fora da sub-rede e não autentica nada. Uma
//! sala encontrada é uma **sugestão de endereço** — quem valida é o handshake do
//! `giochi-net`, que recusa versão diferente.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Marca do protocolo, na pergunta.
const PERGUNTA: &[u8] = b"GIOCHI-TOWER?";
/// Marca do protocolo, na resposta.
const RESPOSTA: &[u8] = b"GIOCHI-TOWER!";

/// Porta de descoberta.
///
/// Separada da porta do jogo (7777) de propósito: quem hospeda não precisa abrir
/// a descoberta, e um host que a abre não deixa de aceitar partida se ela falhar.
pub const PORTA_DESCOBERTA: u16 = 7778;

/// Quanto tempo se espera por respostas, por padrão.
pub const ESPERA_PADRAO_MS: u64 = 700;

/// Uma sala encontrada.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sala {
    /// Endereço do host, pronto para `--entrar`.
    pub endereco: SocketAddr,
    /// Nome curto do host (o que couber no datagrama).
    pub nome: String,
}

impl Sala {
    /// Texto de uma linha, como o menu mostra.
    pub fn rotulo(&self) -> String {
        let ip = self.endereco.ip();
        let porta = self.endereco.port();
        if self.nome.is_empty() {
            format!("{ip}:{porta}")
        } else {
            format!("{}@{ip}:{porta}", self.nome)
        }
    }
}

/// Responde às perguntas de descoberta, em segundo plano.
///
/// A thread é `daemon` por construção: ninguém a junta, e ela morre com o
/// processo. Um `JoinHandle` guardado num campo obrigaria o host a decidir quando
/// parar, e a decisão certa ("quando o processo sair") já é a do SO.
pub struct Anunciante {
    porta_jogo: u16,
    nome: String,
}

impl Anunciante {
    /// Cria o anunciante. A thread **só** começa em [`Self::iniciar`].
    pub fn novo(porta_jogo: u16, nome: impl Into<String>) -> Self {
        Self {
            porta_jogo,
            nome: nome.into(),
        }
    }

    /// Sobe a thread que responde. Falha só se a porta de descoberta estiver
    /// ocupada — e nesse caso o host continua funcionando sem ser encontrado.
    pub fn iniciar(self) -> io::Result<()> {
        self.iniciar_com_porta(PORTA_DESCOBERTA)
    }

    /// Igual a [`Self::iniciar`], numa porta dada.
    ///
    /// Existe para o teste: rodar dois `cargo test` ao mesmo tempo na máquina
    /// disputaria a porta de produção e um dos dois falharia por motivo alheio ao
    /// código.
    pub fn iniciar_com_porta(self, porta_descoberta: u16) -> io::Result<()> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, porta_descoberta))?;
        socket.set_read_timeout(Some(Duration::from_millis(400)))?;
        let porta = self.porta_jogo;
        let nome = self.nome;
        std::thread::Builder::new()
            .name("descoberta".into())
            .spawn(move || {
                let mut buffer = [0u8; 64];
                loop {
                    let Ok((n, origem)) = socket.recv_from(&mut buffer) else {
                        // Timeout não é erro: volta a esperar. Um `break` aqui
                        // desligaria a descoberta em 400 ms de silêncio.
                        continue;
                    };
                    if &buffer[..n] != PERGUNTA {
                        continue;
                    }
                    let resposta = format!("{} {porta} {nome}", String::from_utf8_lossy(RESPOSTA));
                    let _ = socket.send_to(resposta.as_bytes(), origem);
                }
            })?;
        Ok(())
    }
}

/// Procura salas por `espera_ms` e devolve o que responder.
///
/// Ordenado e sem repetição: o mesmo host responde ao broadcast **e** ao
/// loopback quando os dois caminhos funcionam, e duas linhas iguais no menu
/// fariam o jogador achar que há duas partidas.
pub fn procurar(porta_descoberta: u16, espera_ms: u64) -> Vec<Sala> {
    procurar_com_limite(porta_descoberta, espera_ms, 32)
}

/// Igual a [`procurar`], com teto de salas (para o menu não crescer sem fim).
pub fn procurar_com_limite(porta_descoberta: u16, espera_ms: u64, limite: usize) -> Vec<Sala> {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) else {
        return Vec::new();
    };
    // Broadcast pode falhar em rede sem permissão (Android, alguns contêineres).
    // O loopback abaixo continua valendo, então o erro não aborta a procura.
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(120)));

    let destinos = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), porta_descoberta),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), porta_descoberta),
    ];
    for destino in destinos {
        let _ = socket.send_to(PERGUNTA, destino);
    }

    let mut salas: Vec<Sala> = Vec::new();
    let prazo = Instant::now() + Duration::from_millis(espera_ms);
    let mut buffer = [0u8; 128];
    while Instant::now() < prazo && salas.len() < limite {
        let Ok((n, origem)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        if let Some(sala) = interpretar(&buffer[..n], origem.ip()) {
            if !salas.iter().any(|s| s.endereco == sala.endereco) {
                salas.push(sala);
            }
        }
    }
    salas.sort();
    salas
}

/// Interpreta uma resposta. `None` para lixo — quem procura não pode confiar em
/// pacote de terceiro.
fn interpretar(bytes: &[u8], origem: IpAddr) -> Option<Sala> {
    // O IP vem do **remetente**, não do corpo: um datagrama pode dizer qualquer
    // coisa, e aceitar o endereço de dentro é entregar o menu a quem quiser
    // apontar o jogador para outro lugar.
    let texto = std::str::from_utf8(bytes).ok()?;
    let resto = texto.strip_prefix(std::str::from_utf8(RESPOSTA).ok()?)?;
    let mut campos = resto.trim().splitn(2, ' ');
    let porta: u16 = campos.next()?.parse().ok()?;
    if porta == 0 {
        return None;
    }
    let nome = campos.next().unwrap_or("").trim().to_string();
    Some(Sala {
        endereco: SocketAddr::new(origem, porta),
        nome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Porta alta e fixa para o teste: a de produção pode estar em uso na máquina
    // de quem roda a suíte (dois `cargo test` ao mesmo tempo brigariam).
    const PORTA_TESTE: u16 = 47831;

    #[test]
    fn a_resposta_vira_sala_com_o_ip_de_quem_respondeu() {
        let sala = interpretar(b"GIOCHI-TOWER! 7777 console", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)))
            .expect("resposta válida");
        assert_eq!(sala.endereco.to_string(), "10.0.0.2:7777");
        assert_eq!(sala.nome, "console");
        assert_eq!(sala.rotulo(), "console@10.0.0.2:7777");
    }

    #[test]
    fn lixo_e_porta_zero_sao_recusados() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(interpretar(b"qualquer coisa", ip).is_none());
        assert!(interpretar(b"GIOCHI-TOWER!", ip).is_none(), "sem porta");
        assert!(interpretar(b"GIOCHI-TOWER! 0 sem-porta", ip).is_none());
        assert!(interpretar(b"GIOCHI-TOWER! abc nome", ip).is_none());
        // Sem nome é resposta válida: o rótulo cai no endereço.
        let sala = interpretar(b"GIOCHI-TOWER! 7777", ip).expect("sem nome");
        assert_eq!(sala.rotulo(), "127.0.0.1:7777");
    }

    #[test]
    fn o_endereco_vem_do_remetente_e_nao_do_corpo() {
        // Um pacote que se diz noutro IP: se o corpo fosse aceito, qualquer um na
        // rede apontaria o menu para outro host.
        let sala = interpretar(
            b"GIOCHI-TOWER! 7777 192.168.9.9",
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        )
        .expect("válida");
        assert_eq!(sala.endereco.ip().to_string(), "10.0.0.2");
        // E o "192.168.9.9" sobra como parte do nome, que é o mal menor.
        assert!(sala.nome.contains("192.168.9.9"));
    }

    #[test]
    fn a_procura_acha_um_anunciante_no_loopback() {
        // O caminho inteiro, com dois sockets de verdade: pergunta, thread
        // respondendo, resposta interpretada. Sem isto, o teste acima provaria só
        // o parser — e o protocolo poderia estar quebrado dos dois lados.
        Anunciante::novo(7777, "teste")
            .iniciar_com_porta(PORTA_TESTE)
            .expect("anunciante");
        let salas = procurar_com_limite(PORTA_TESTE, 600, 4);
        assert!(
            salas.iter().any(|s| s.endereco.port() == 7777),
            "não achei a sala: {salas:?}"
        );
        let sala = salas.iter().find(|s| s.endereco.port() == 7777).expect("sala");
        assert_eq!(sala.nome, "teste");
    }

    #[test]
    fn sem_anunciante_a_procura_termina_vazia_e_rapido() {
        // Porta onde não há ninguém: a função tem de devolver em ~espera_ms, não
        // travar o menu esperando um pacote que não vem.
        let inicio = Instant::now();
        let salas = procurar_com_limite(47999, 200, 4);
        let gasto = inicio.elapsed();
        assert!(salas.is_empty(), "{salas:?}");
        assert!(gasto < Duration::from_secs(2), "levou {gasto:?}");
    }
}
