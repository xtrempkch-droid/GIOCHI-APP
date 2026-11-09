//! Formatação do HUD.
//!
//! ## Por que formatar no núcleo
//!
//! O texto do HUD é **informação**, não desenho: "1:05.3" e "223" carregam
//! regras (quantas casas, quando arredonda, o que mostrar quando o valor é
//! desconhecido) que dão para testar sem abrir janela. Foi assim que o launcher
//! descobriu que `year_label()` mostrava o **id** do cartão em vez do ano — o
//! texto estava legível e errado.
//!
//! O desenho recebe `String` e desenha; não decide nada.

use crate::fonte::largura_texto;

/// Tempo no formato `M:SS.d` (ex.: `1:05.3`).
///
/// Uma casa decimal e não duas: a 75 Hz o cronômetro muda 75 vezes por segundo
/// e o dígito final viraria um borrão. Uma casa é o que dá para ler.
pub fn tempo(segundos: f32) -> String {
    let total = segundos.max(0.0);
    let minutos = (total / 60.0).floor() as u32;
    let resto = total - minutos as f32 * 60.0;
    // `floor` e não `round` na décima: arredondar para cima faria o relógio
    // mostrar `1:00.0` antes de o minuto virar.
    let decimos = ((resto * 10.0).floor() as u32).min(599);
    let seg = decimos / 10;
    let dec = decimos % 10;
    format!("{minutos}:{seg:02}.{dec}")
}

/// Velocidade em km/h, sem casas decimais.
///
/// `abs()` porque a ré existe: o velocímetro mostra o módulo e a marcha diz o
/// sentido, como num carro de verdade.
pub fn velocidade(kmh: f32) -> String {
    format!("{}", kmh.abs().round().clamp(0.0, 999.0) as u32)
}

/// Pontos de drift, com separador de milhar por espaço — o glifo de espaço
/// existe e lê melhor que `12340`.
pub fn pontos(valor: u32) -> String {
    let s = valor.to_string();
    let mut saida = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            saida.push(' ');
        }
        saida.push(c);
    }
    saida
}

/// `VOLTA 1/2`.
pub fn volta(atual: u32, total: u32) -> String {
    let atual = atual.min(total);
    format!("VOLTA {atual}/{total}")
}

/// `POS 1/4`.
pub fn posicao(posicao: usize, total: usize) -> String {
    format!("POS {posicao}/{total}")
}

/// Multiplicador de combo: `x3`.
pub fn combo(multiplicador: u32) -> String {
    format!("x{multiplicador}")
}

/// Nitro em porcentagem: `72%`.
pub fn porcentagem(fracao: f32) -> String {
    let p = (fracao.clamp(0.0, 1.0) * 100.0).round() as u32;
    format!("{p}%")
}

/// Largura em pixels do texto, para centralizar sem medir no desenho.
pub fn largura_em_pixels(texto: &str, escala: f32) -> f32 {
    largura_texto(texto) as f32 * escala
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tempo_no_formato_de_corrida() {
        assert_eq!(tempo(0.0), "0:00.0");
        assert_eq!(tempo(5.9), "0:05.9");
        assert_eq!(tempo(59.99), "0:59.9");
        assert_eq!(tempo(60.0), "1:00.0");
        assert_eq!(tempo(65.34), "1:05.3");
        assert_eq!(tempo(125.99), "2:05.9");
    }

    #[test]
    fn o_tempo_nao_mostra_60_segundos() {
        // O erro clássico do relógio de corrida: 59,98 arredondado vira
        // "0:60.0". `floor` na décima é o que impede.
        assert_eq!(tempo(59.98), "0:59.9");
        assert_eq!(tempo(119.999), "1:59.9");
    }

    #[test]
    fn tempo_negativo_vira_zero() {
        assert_eq!(tempo(-3.0), "0:00.0");
    }

    #[test]
    fn velocidade_arredonda_e_ignora_o_sinal() {
        assert_eq!(velocidade(0.0), "0");
        assert_eq!(velocidade(99.6), "100");
        assert_eq!(velocidade(-30.0), "30");
        assert_eq!(velocidade(223.4), "223");
        assert_eq!(velocidade(5000.0), "999", "o velocímetro tem três dígitos");
    }

    #[test]
    fn pontos_com_separador_de_milhar() {
        assert_eq!(pontos(0), "0");
        assert_eq!(pontos(999), "999");
        assert_eq!(pontos(1000), "1 000");
        assert_eq!(pontos(12340), "12 340");
        assert_eq!(pontos(1234567), "1 234 567");
    }

    #[test]
    fn volta_e_posicao_nao_passam_do_total() {
        assert_eq!(volta(1, 2), "VOLTA 1/2");
        assert_eq!(volta(3, 2), "VOLTA 2/2");
        assert_eq!(posicao(1, 4), "POS 1/4");
    }

    #[test]
    fn porcentagem_do_nitro() {
        assert_eq!(porcentagem(0.0), "0%");
        assert_eq!(porcentagem(0.5), "50%");
        assert_eq!(porcentagem(1.0), "100%");
        assert_eq!(porcentagem(2.0), "100%");
    }

    #[test]
    fn combo_tem_o_x() {
        assert_eq!(combo(1), "x1");
        assert_eq!(combo(5), "x5");
    }

    #[test]
    fn todo_texto_do_hud_usa_glifos_existentes() {
        // O HUD desenha com a fonte de 5×7; um caractere fora da tabela vira
        // buraco silencioso na tela.
        let amostras = [
            tempo(65.3),
            velocidade(223.0),
            pontos(12340),
            volta(1, 2),
            posicao(1, 4),
            combo(3),
            porcentagem(0.72),
            "DRIFT".to_string(),
            "NITRO".to_string(),
            "JOGAR".to_string(),
            "REINICIAR".to_string(),
            "NEON DRIFT".to_string(),
            "WASD OU SETAS".to_string(),
        ];
        for texto in amostras {
            for c in texto.chars() {
                assert!(
                    crate::fonte::glifo(c).is_some(),
                    "{c:?} de {texto:?} não existe na fonte"
                );
            }
        }
    }
}
