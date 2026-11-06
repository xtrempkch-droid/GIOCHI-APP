//! Leitura de `/dev/input` numa thread própria, sem alocar.
//!
//! ## Por que evdev e não o gamepad do SDL
//!
//! Os botões do console vêm de `gpio-keys`/`r36s_Gamepad`, que **não estão no
//! `SDL_GameControllerDB`**. O SDL devolveria `None` para todos eles e o jogo
//! abriria, desenharia e nenhum botão responderia — sem erro nenhum no log. Já
//! aconteceu aqui.
//!
//! ## Por que uma thread
//!
//! `read()` em `/dev/input` **bloqueia**. Ler no laço principal travaria o
//! desenho até o próximo botão. A thread separada lê e publica; o laço consome
//! sem esperar.
//!
//! ## Memória fraca do ARM
//!
//! x86 é TSO: quem escreve e depois lê "vê" na ordem, quase sempre. ARM não. A
//! fila abaixo é SPSC (um produtor, um consumidor) e por isso usa
//! `Acquire`/`Release` explícitos: o `Release` no índice garante que o conteúdo
//! do slot foi escrito antes do índice; o `Acquire` garante que o consumidor só
//! lê o slot depois de ver o índice. Sem isso, o compilador e o Cortex-A35
//! podem reordenar, e o sintoma não é crash — é **botão que às vezes não
//! registra**, o pior tipo de bug para diagnosticar.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

use giochi_cobrinha_core::Acao;

/// Capacidade da fila. 64 ações é mais do que um dedo humano produz entre dois
/// quadros a 45 Hz (22 ms).
pub const CAPACIDADE: usize = 64;

const VAZIO: u8 = 0;

fn codificar(a: Acao) -> u8 {
    match a {
        Acao::Dir(giochi_cobrinha_core::Direcao::Cima) => 1,
        Acao::Dir(giochi_cobrinha_core::Direcao::Baixo) => 2,
        Acao::Dir(giochi_cobrinha_core::Direcao::Esquerda) => 3,
        Acao::Dir(giochi_cobrinha_core::Direcao::Direita) => 4,
        Acao::Confirmar => 5,
        Acao::Sair => 6,
    }
}

fn decodificar(v: u8) -> Option<Acao> {
    use giochi_cobrinha_core::Direcao;
    match v {
        1 => Some(Acao::Dir(Direcao::Cima)),
        2 => Some(Acao::Dir(Direcao::Baixo)),
        3 => Some(Acao::Dir(Direcao::Esquerda)),
        4 => Some(Acao::Dir(Direcao::Direita)),
        5 => Some(Acao::Confirmar),
        6 => Some(Acao::Sair),
        _ => None,
    }
}

/// Fila sem trava entre a thread de entrada e o laço de desenho.
///
/// Não há `unsafe` aqui de propósito: cada slot é atômico, então não existe
/// acesso não sincronizado a memória. Um ring buffer com `UnsafeCell` seria
/// mais rápido por nanossegundos e mais fácil de errar.
pub struct Fila {
    slots: [AtomicU8; CAPACIDADE],
    escrita: AtomicUsize,
    leitura: AtomicUsize,
}

impl Default for Fila {
    fn default() -> Self {
        Self::nova()
    }
}

impl Fila {
    pub fn nova() -> Self {
        Fila {
            slots: std::array::from_fn(|_| AtomicU8::new(VAZIO)),
            escrita: AtomicUsize::new(0),
            leitura: AtomicUsize::new(0),
        }
    }

    /// Produtor. Devolve `false` quando a fila está cheia — descartar é melhor
    /// que bloquear a thread que lê o kernel.
    pub fn empurrar(&self, acao: Acao) -> bool {
        let e = self.escrita.load(Ordering::Relaxed);
        let l = self.leitura.load(Ordering::Acquire);
        if e.wrapping_sub(l) >= CAPACIDADE {
            return false;
        }
        self.slots[e % CAPACIDADE].store(codificar(acao), Ordering::Relaxed);
        // `Release`: o slot tem de estar escrito antes de o índice avançar.
        self.escrita.store(e.wrapping_add(1), Ordering::Release);
        true
    }

    /// Consumidor. `None` quando vazia.
    pub fn puxar(&self) -> Option<Acao> {
        let l = self.leitura.load(Ordering::Relaxed);
        let e = self.escrita.load(Ordering::Acquire);
        if l == e {
            return None;
        }
        let v = self.slots[l % CAPACIDADE].load(Ordering::Relaxed);
        self.leitura.store(l.wrapping_add(1), Ordering::Release);
        decodificar(v)
    }
}

/// Struct do kernel. Em aarch64/x86-64: `timeval` (16) + type (2) + code (2)
/// + value (4) = 24 bytes. Em 32 bits seria diferente — o projeto é aarch64.
const EVENTO_TAMANHO: usize = 24;
const EV_KEY: u16 = 0x01;
const VALOR_SOLTOU: i32 = 0;
const VALOR_APERTOU: i32 = 1;

/// Interpreta um `input_event` cru. `None` quando não é um aperto de botão.
///
/// Aperto repetido (`value == 2`) é ignorado: repetição é do kernel e no jogo
/// ela viraria "virar a direção várias vezes" com um toque só.
pub fn interpretar(bytes: &[u8]) -> Option<Acao> {
    if bytes.len() < EVENTO_TAMANHO {
        return None;
    }
    let tipo = u16::from_le_bytes([bytes[16], bytes[17]]);
    if tipo != EV_KEY {
        return None;
    }
    let codigo = u16::from_le_bytes([bytes[18], bytes[19]]);
    let valor = i32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if valor != VALOR_APERTOU && valor != VALOR_SOLTOU {
        return None;
    }
    if valor == VALOR_SOLTOU {
        return None;
    }
    giochi_cobrinha_core::de_evdev(codigo)
}

/// Quantos dispositivos de entrada foram abertos.
pub struct Entrada {
    fila: Arc<Fila>,
    /// Falso quando não há `/dev/input` — o PC, o CI.
    ativa: Arc<AtomicBool>,
    abertos: usize,
}

impl Entrada {
    pub fn fila(&self) -> &Arc<Fila> {
        &self.fila
    }

    pub fn abertos(&self) -> usize {
        self.abertos
    }

    pub fn ativa(&self) -> bool {
        self.ativa.load(Ordering::Relaxed)
    }

    /// Abre todos os `/dev/input/event*` e sobe uma thread por dispositivo.
    pub fn iniciar() -> Self {
        let fila = Arc::new(Fila::nova());
        let ativa = Arc::new(AtomicBool::new(false));
        let mut abertos = 0;

        let dir = match std::fs::read_dir("/dev/input") {
            Ok(d) => d,
            Err(_) => {
                return Entrada { fila, ativa, abertos: 0 };
            }
        };

        let mut caminhos: Vec<_> = dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("event"))
                    .unwrap_or(false)
            })
            .collect();
        caminhos.sort();

        for caminho in caminhos {
            let arquivo = match std::fs::File::open(&caminho) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let fila = Arc::clone(&fila);
            let ativa = Arc::clone(&ativa);
            let nome = caminho.file_name().unwrap_or_default().to_string_lossy().to_string();
            // `Builder` sem nome: a thread morre junto com o processo.
            let _ = std::thread::Builder::new()
                .name(format!("evdev-{nome}"))
                .spawn(move || ler_dispositivo(arquivo, fila, ativa));
            abertos += 1;
        }

        Entrada { fila, ativa, abertos }
    }
}

fn ler_dispositivo(mut arquivo: std::fs::File, fila: Arc<Fila>, ativa: Arc<AtomicBool>) {
    use std::io::Read;
    let mut buffer = [0u8; EVENTO_TAMANHO * 16];
    loop {
        match arquivo.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                ativa.store(true, Ordering::Relaxed);
                for pedaco in buffer[..n].chunks_exact(EVENTO_TAMANHO) {
                    if let Some(acao) = interpretar(pedaco) {
                        fila.empurrar(acao);
                    }
                }
            }
            // Dispositivo removido (controle por USB) ou interrompido: sai.
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use giochi_cobrinha_core::Direcao;

    /// Monta um `input_event` como o kernel entregaria.
    fn evento(tipo: u16, codigo: u16, valor: i32) -> [u8; EVENTO_TAMANHO] {
        let mut b = [0u8; EVENTO_TAMANHO];
        b[16..18].copy_from_slice(&tipo.to_le_bytes());
        b[18..20].copy_from_slice(&codigo.to_le_bytes());
        b[20..24].copy_from_slice(&valor.to_le_bytes());
        b
    }

    #[test]
    fn aperto_de_direcional_vira_acao() {
        let b = evento(EV_KEY, 544, 1); // BTN_DPAD_UP
        assert_eq!(interpretar(&b), Some(Acao::Dir(Direcao::Cima)));
    }

    #[test]
    fn soltar_nao_vira_acao() {
        // Se soltar gerasse ação, o jogo receberia cada botão duas vezes e as
        // viradas sairiam dobradas — sintoma que ninguém associaria a isto.
        let b = evento(EV_KEY, 544, 0);
        assert_eq!(interpretar(&b), None);
    }

    #[test]
    fn repeticao_do_kernel_e_ignorada() {
        let b = evento(EV_KEY, 546, 2); // valor 2 = autorepeat
        assert_eq!(interpretar(&b), None);
    }

    #[test]
    fn evento_que_nao_e_tecla_e_ignorado() {
        let b = evento(0x03, 0, 5); // EV_ABS
        assert_eq!(interpretar(&b), None);
    }

    #[test]
    fn buffer_curto_nao_estoura() {
        assert_eq!(interpretar(&[0u8; 10]), None);
        assert_eq!(interpretar(&[]), None);
    }

    #[test]
    fn o_deslocamento_dos_campos_e_o_do_kernel() {
        // Um erro de deslocamento aqui lê o tipo no lugar do código e o jogo
        // passa a responder a botões errados — sem erro em lugar nenhum.
        // 24 bytes, timeval de 16: tipo em 16, código em 18, valor em 20.
        let b = evento(EV_KEY, 0x0123, 1);
        assert_eq!(u16::from_le_bytes([b[16], b[17]]), EV_KEY);
        assert_eq!(u16::from_le_bytes([b[18], b[19]]), 0x0123);
        assert_eq!(i32::from_le_bytes([b[20], b[21], b[22], b[23]]), 1);
    }

    #[test]
    fn fila_vazia_devolve_nada() {
        let f = Fila::nova();
        assert_eq!(f.puxar(), None);
    }

    #[test]
    fn fila_preserva_a_ordem() {
        let f = Fila::nova();
        f.empurrar(Acao::Dir(Direcao::Cima));
        f.empurrar(Acao::Dir(Direcao::Esquerda));
        f.empurrar(Acao::Confirmar);
        assert_eq!(f.puxar(), Some(Acao::Dir(Direcao::Cima)));
        assert_eq!(f.puxar(), Some(Acao::Dir(Direcao::Esquerda)));
        assert_eq!(f.puxar(), Some(Acao::Confirmar));
        assert_eq!(f.puxar(), None);
    }

    #[test]
    fn fila_cheia_recusa_sem_bloquear() {
        let f = Fila::nova();
        for _ in 0..CAPACIDADE {
            assert!(f.empurrar(Acao::Confirmar));
        }
        assert!(!f.empurrar(Acao::Confirmar), "fila cheia tem de recusar");
        // E o consumidor continua conseguindo drenar.
        assert_eq!(f.puxar(), Some(Acao::Confirmar));
    }

    #[test]
    fn fila_da_a_volta_sem_perder_posicao() {
        let f = Fila::nova();
        for i in 0..(CAPACIDADE * 3) {
            f.empurrar(Acao::Confirmar);
            assert_eq!(f.puxar(), Some(Acao::Confirmar), "iteração {i}");
        }
    }

    #[test]
    fn produtor_e_consumidor_em_threads_diferentes() {
        // Exercita o caminho real: uma thread empurra, outra puxa. É o teste
        // que pega erro de `Ordering` — em x86 ele passaria com `Relaxed` por
        // acaso; em ARM, não.
        let fila = Arc::new(Fila::nova());
        let produtor = Arc::clone(&fila);
        let t = std::thread::spawn(move || {
            for _ in 0..10_000 {
                while !produtor.empurrar(Acao::Dir(Direcao::Direita)) {
                    std::hint::spin_loop();
                }
            }
        });
        let mut recebidas = 0;
        while recebidas < 10_000 {
            if fila.puxar().is_some() {
                recebidas += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        t.join().unwrap();
        assert_eq!(recebidas, 10_000);
    }
}
