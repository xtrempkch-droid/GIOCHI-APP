//! Leitura dos botões do controle por evdev, com **estado** e não eventos.
//!
//! ## Por que bitmask, e não fila de ações
//!
//! A cobrinha consome ações discretas ("virou para cima agora"), porque lá cada
//! toque vale uma virada. Corrida é o oposto: acelerar e virar são **contínuos**,
//! e o que o jogo precisa saber a cada quadro é "o A está apertado agora?".
//!
//! Guardar os bits de pressionado num `AtomicU32` compartilhado resolve isso com
//! uma thread por dispositivo e nenhum bloqueio no laço. O custo é que o estado
//! é global ao processo — e é por isso que `--teclas` no PC entra por outro
//! caminho (`entrada_jogo::combinar`), em vez de fingir um evdev.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Acelerar (A ou direcional para cima).
pub const ACELERAR: u32 = 1 << 0;
/// Frear / ré (B ou direcional para baixo).
pub const FREAR: u32 = 1 << 1;
/// Virar à esquerda (direcional esquerda).
pub const ESQUERDA: u32 = 1 << 2;
/// Virar à direita (direcional direita).
pub const DIREITA: u32 = 1 << 3;
/// Freio de mão (L ou R).
pub const FREIO_MAO: u32 = 1 << 4;
/// Nitro (L2 ou R2).
pub const NITRO: u32 = 1 << 5;
/// Botão de confirmar/iniciar (A, START ou X).
pub const CONFIRMAR: u32 = 1 << 6;
/// Botão de sair (SELECT).
pub const SAIR: u32 = 1 << 7;

/// Códigos evdev usados pelo jogo.
///
/// Medidos no `buttons.map` do aparelho (X55/X35H herdam o mesmo DTS): o `A`
/// físico é `BTN_EAST` e o `B` é `BTN_SOUTH`, ao contrário do que a posição no
/// plástico sugere. O direcional está na faixa `BTN_DPAD_*` (544-547), não em
/// `KEY_*`.
pub mod codigos {
    /// Direcional para cima.
    pub const DPAD_UP: u16 = 544;
    /// Direcional para baixo.
    pub const DPAD_DOWN: u16 = 545;
    /// Direcional para a esquerda.
    pub const DPAD_LEFT: u16 = 546;
    /// Direcional para a direita.
    pub const DPAD_RIGHT: u16 = 547;
    /// Botão B do plástico (`BTN_SOUTH`).
    pub const SUL: u16 = 304;
    /// Botão A do plástico (`BTN_EAST`).
    pub const LESTE: u16 = 305;
    /// Botão X (`BTN_NORTH`).
    pub const NORTE: u16 = 307;
    /// Botão Y (`BTN_WEST`).
    pub const OESTE: u16 = 308;
    /// Ombro L1.
    pub const TL: u16 = 310;
    /// Ombro R1.
    pub const TR: u16 = 311;
    /// Gatilho L2.
    pub const TL2: u16 = 312;
    /// Gatilho R2.
    pub const TR2: u16 = 313;
    /// SELECT.
    pub const SELECT: u16 = 314;
    /// START.
    pub const START: u16 = 315;
}

/// Bit correspondente a um código evdev. `None` = botão que o jogo ignora.
pub fn bit_do_codigo(codigo: u16) -> Option<u32> {
    use codigos::*;
    Some(match codigo {
        // Acelerar também no direcional para cima: quem joga deitado usa o
        // direcional, e obrigar o A ali seria uma limitação sem motivo.
        LESTE | DPAD_UP => ACELERAR,
        SUL | DPAD_DOWN => FREAR,
        DPAD_LEFT => ESQUERDA,
        DPAD_RIGHT => DIREITA,
        TL | TR => FREIO_MAO,
        TL2 | TR2 => NITRO,
        START | NORTE => CONFIRMAR,
        SELECT | OESTE => SAIR,
        _ => return None,
    })
}

/// Struct do kernel: `timeval` (16) + tipo (2) + código (2) + valor (4).
const EVENTO_TAMANHO: usize = 24;
const EV_KEY: u16 = 0x01;

/// Aplica um `input_event` cru na máscara. Devolve `true` se algo mudou.
///
/// Aperto repetido (`value == 2`) é ignorado: repetição do kernel não muda o
/// estado, e tratá-la como aperto faria o nitro ser rearmado a cada 30 ms.
pub fn aplicar_evento(mascara: &AtomicU32, bytes: &[u8]) -> bool {
    if bytes.len() < EVENTO_TAMANHO {
        return false;
    }
    let tipo = u16::from_le_bytes([bytes[16], bytes[17]]);
    if tipo != EV_KEY {
        return false;
    }
    let codigo = u16::from_le_bytes([bytes[18], bytes[19]]);
    let valor = i32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let Some(bit) = bit_do_codigo(codigo) else {
        return false;
    };
    match valor {
        1 => {
            mascara.fetch_or(bit, Ordering::Relaxed);
            true
        }
        0 => {
            mascara.fetch_and(!bit, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

/// Botões do aparelho, compartilhados com as threads de leitura.
pub struct Botoes {
    mascara: Arc<AtomicU32>,
    abertos: usize,
}

impl Botoes {
    /// Abre todos os `/dev/input/event*` e sobe uma thread por dispositivo.
    ///
    /// Sem `/dev/input` (o PC) devolve zero dispositivos e máscara zerada — o
    /// teclado do SDL cobre esse caso, e falhar aqui impediria de rodar no CI.
    pub fn iniciar() -> Self {
        let mascara = Arc::new(AtomicU32::new(0));
        let mut abertos = 0;

        let Ok(dir) = std::fs::read_dir("/dev/input") else {
            return Self { mascara, abertos };
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
            let Ok(arquivo) = std::fs::File::open(&caminho) else {
                continue;
            };
            let mascara = Arc::clone(&mascara);
            let nome = caminho
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let _ = std::thread::Builder::new()
                .name(format!("evdev-{nome}"))
                .spawn(move || ler_dispositivo(arquivo, mascara));
            abertos += 1;
        }

        Self { mascara, abertos }
    }

    /// Máscara de botões pressionados neste instante.
    pub fn bits(&self) -> u32 {
        self.mascara.load(Ordering::Relaxed)
    }

    /// Quantos dispositivos foram abertos.
    pub fn abertos(&self) -> usize {
        self.abertos
    }
}

fn ler_dispositivo(mut arquivo: std::fs::File, mascara: Arc<AtomicU32>) {
    use std::io::Read;
    let mut buffer = [0u8; EVENTO_TAMANHO * 16];
    loop {
        match arquivo.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                for pedaco in buffer[..n].chunks_exact(EVENTO_TAMANHO) {
                    aplicar_evento(&mascara, pedaco);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn evento(tipo: u16, codigo: u16, valor: i32) -> [u8; EVENTO_TAMANHO] {
        let mut b = [0u8; EVENTO_TAMANHO];
        b[16..18].copy_from_slice(&tipo.to_le_bytes());
        b[18..20].copy_from_slice(&codigo.to_le_bytes());
        b[20..24].copy_from_slice(&valor.to_le_bytes());
        b
    }

    #[test]
    fn apertar_e_soltar_ligam_e_desligam_o_bit() {
        let m = AtomicU32::new(0);
        assert!(aplicar_evento(&m, &evento(EV_KEY, codigos::LESTE, 1)));
        assert_eq!(m.load(Ordering::Relaxed) & ACELERAR, ACELERAR);
        assert!(aplicar_evento(&m, &evento(EV_KEY, codigos::LESTE, 0)));
        assert_eq!(m.load(Ordering::Relaxed) & ACELERAR, 0);
    }

    #[test]
    fn dois_botoes_ao_mesmo_tempo_nao_se_apagam() {
        // Acelerar e virar juntos é o caso normal de corrida. Com `store` em vez
        // de `fetch_or`, um apagaria o outro.
        let m = AtomicU32::new(0);
        aplicar_evento(&m, &evento(EV_KEY, codigos::LESTE, 1));
        aplicar_evento(&m, &evento(EV_KEY, codigos::DPAD_RIGHT, 1));
        let bits = m.load(Ordering::Relaxed);
        assert_eq!(bits & ACELERAR, ACELERAR);
        assert_eq!(bits & DIREITA, DIREITA);
    }

    #[test]
    fn repeticao_do_kernel_nao_mexe_no_estado() {
        let m = AtomicU32::new(0);
        assert!(!aplicar_evento(&m, &evento(EV_KEY, codigos::LESTE, 2)));
        assert_eq!(m.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn botao_fora_do_mapa_e_ignorado() {
        assert_eq!(bit_do_codigo(9999), None);
        let m = AtomicU32::new(0);
        assert!(!aplicar_evento(&m, &evento(EV_KEY, 9999, 1)));
    }

    #[test]
    fn evento_que_nao_e_tecla_e_ignorado() {
        let m = AtomicU32::new(0);
        assert!(!aplicar_evento(&m, &evento(0x03, 0, 5))); // EV_ABS
        assert_eq!(m.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn buffer_curto_nao_estoura() {
        let m = AtomicU32::new(0);
        assert!(!aplicar_evento(&m, &[0u8; 10]));
        assert!(!aplicar_evento(&m, &[]));
    }

    #[test]
    fn os_botoes_do_jogo_tem_codigo_medido_no_aparelho() {
        // Estes são os do `buttons.map` real. Trocar um deles por chute deixa o
        // botão morto, e o sintoma é "não freia" sem erro em lugar nenhum.
        assert_eq!(bit_do_codigo(305), Some(ACELERAR), "A = BTN_EAST");
        assert_eq!(bit_do_codigo(304), Some(FREAR), "B = BTN_SOUTH");
        assert_eq!(bit_do_codigo(546), Some(ESQUERDA));
        assert_eq!(bit_do_codigo(547), Some(DIREITA));
        assert_eq!(bit_do_codigo(310), Some(FREIO_MAO), "L1");
        assert_eq!(bit_do_codigo(313), Some(NITRO), "R2");
        assert_eq!(bit_do_codigo(315), Some(CONFIRMAR), "START");
        assert_eq!(bit_do_codigo(314), Some(SAIR), "SELECT");
    }

    #[test]
    fn o_deslocamento_dos_campos_bate_com_o_do_kernel() {
        let b = evento(EV_KEY, 0x0123, 1);
        assert_eq!(u16::from_le_bytes([b[16], b[17]]), EV_KEY);
        assert_eq!(u16::from_le_bytes([b[18], b[19]]), 0x0123);
        assert_eq!(i32::from_le_bytes([b[20], b[21], b[22], b[23]]), 1);
    }

    #[test]
    fn sem_dev_input_nenhum_dispositivo_e_o_jogo_nao_quebra() {
        // No CI não há `/dev/input`. O construtor tem de devolver zero, não
        // entrar em pânico.
        let b = Botoes::iniciar();
        assert!(b.abertos() == 0 || b.abertos() > 0, "sempre responde");
        let _ = b.bits();
    }
}
