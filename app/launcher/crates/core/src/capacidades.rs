//! Decodificar o que o kernel **promete** emitir.
//!
//! # A pergunta, e a armadilha
//!
//! `/sys/class/input/event*/device/capabilities/key` lista máscaras de bits em
//! hexadecimal. A armadilha é a ordem: **as palavras de 64 bits vêm em ordem
//! DECRESCENTE**, então a **última coluna é a palavra 0**. Ler da esquerda
//! desloca todos os códigos em múltiplos de 64 e produz um mapa plausível e
//! errado — que é o pior tipo de erro, porque não parece erro.
//!
//! Os três exemplos abaixo são **medidos em aparelho**, não inventados:
//!
//! ```text
//! X55   gpio-keys-vol        c000000000000 0                  → 114 e 115
//! X55   rk805 pwrkey        10000000000000 0                  → 116
//! R35S  r36s_Gamepad        f00000000 0 0 0 7fdb000000000000 0 0 0 0
//!                                                             → 17 botões
//! ```
//!
//! # Por que isto existe no app
//!
//! Porque "o botão não funciona" tem duas causas que se confundem: o **driver**
//! não declara o código, ou o app não o mapeia. Saber o que o kernel declara
//! separa as duas sem aparelho na mão.
//!
//! E porque há uma diferença real entre aparelhos que o app precisa conhecer: o
//! **R35S não declara `KEY_POWER`** — o botão de energia dele não é dispositivo
//! de entrada. O launcher oferecia "toque curto para dormir" em todo aparelho.

/// Um código de tecla do kernel (`input-event-codes.h`).
pub const KEY_POWER: u16 = 116;

/// Decodifica a linha hexadecimal, respeitando a ordem decrescente.
///
/// Ignora colunas vazias e aceita tanto espaço quanto tabulação, porque o
/// arquivo é lido cru do `/sys`.
pub fn decodificar(linha: &str) -> Vec<u16> {
    let palavras: Vec<&str> = linha.split_whitespace().collect();
    let total = palavras.len();
    let mut codigos = Vec::new();

    for (posicao, palavra) in palavras.iter().enumerate() {
        // A palavra 0 é a **última** da linha. Este é o deslocamento que, se
        // estiver errado, produz um mapa plausível e errado.
        let indice = (total - 1 - posicao) as u32;
        let valor = match u64::from_str_radix(palavra.trim_start_matches("0x"), 16) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for bit in 0..64u32 {
            if valor & (1u64 << bit) != 0 {
                let codigo = indice * 64 + bit;
                // `u16` cobre a faixa do kernel; acima disso é máscara absurda.
                if let Ok(c) = u16::try_from(codigo) {
                    codigos.push(c);
                }
            }
        }
    }
    codigos.sort_unstable();
    codigos
}

/// Se algum dos códigos declarados é o botão de energia.
pub fn tem_power(codigos: &[u16]) -> bool {
    codigos.contains(&KEY_POWER)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn gpio_keys_vol_do_x55_da_114_e_115() {
        // Medido no X55: `c000000000000 0`. `c` = dois bits no topo da palavra
        // 1 (a primeira da linha), que é 64+50 e 64+51.
        let c = decodificar("c000000000000 0");
        assert_eq!(c, vec![114, 115], "esperava VOLUMEDOWN/UP");
    }

    #[test]
    fn pwrkey_do_x55_da_116() {
        // A regra das palavras, no caso mais simples: `1` seguido de 13 zeros =
        // bit 52 da palavra 1.
        assert_eq!(decodificar("10000000000000 0"), vec![116]);
        assert!(tem_power(&decodificar("10000000000000 0")));
    }

    #[test]
    fn gamepad_do_r35s_da_os_17_botoes() {
        // Medido no R35S em 06/11/2026, via TTL. Nove palavras: `mmap` do
        // gamepad inteiro, com o D-pad na faixa `BTN_` (544-547) e o resto em
        // `0x136`-`0x13e`.
        let linha = "f00000000 0 0 0 7fdb000000000000 0 0 0 0";
        let c = decodificar(linha);
        let esperado: Vec<u16> = vec![
            304, 305, 307, 308, // B, A, X, Y
            310, 311, 312, 313, // L, R, L2, R2
            314, 315, 316, 317, 318, // SELECT, START, MODE, THUMBL, THUMBR
            544, 545, 546, 547, // D-pad
        ];
        assert_eq!(c, esperado);
        assert_eq!(c.len(), 17);
        // E o R35S **não** declara o botão de energia.
        assert!(!tem_power(&c), "o R35S não tem KEY_POWER como entrada");
    }

    #[test]
    fn ler_da_esquerda_daria_tudo_errado() {
        // A prova de que a ordem importa: se a linha fosse lida da esquerda, o
        // pwrkey do X55 daria 52 em vez de 116. Um mapeamento plausível (52 não
        // existe como tecla comum) e errado.
        let esquerda = decodificar("10000000000000 0");
        assert_eq!(esquerda, vec![116]);
        assert_ne!(esquerda, vec![52]);
    }

    #[test]
    fn linha_vazia_ou_lixo_nao_estoura() {
        assert!(decodificar("").is_empty());
        assert!(decodificar("   ").is_empty());
        assert!(decodificar("zzz 0").is_empty());
        // Zero explícito não declara nada.
        assert!(decodificar("0 0").is_empty());
    }

    #[test]
    fn aceita_prefixo_0x() {
        // Alguns drivers escrevem com `0x`; o `/sys` costuma não escrever.
        assert_eq!(decodificar("0x10000000000000 0"), vec![116]);
    }

    #[test]
    fn os_codigos_saem_em_ordem_e_sem_repeticao() {
        let c = decodificar("3 0");
        // `3` na palavra 1 = bits 0 e 1 = 64 e 65.
        assert_eq!(c, vec![64, 65]);
    }
}
