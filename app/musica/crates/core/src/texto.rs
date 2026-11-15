//! Texto de interface: encurtar, formatar tempo e montar as dicas do rodapé.
//!
//! A medição de largura é **injetada** (`impl Fn(&str) -> i32`) em vez de vir do
//! `SDL_ttf`. Duas consequências: o CI testa o corte de texto sem fonte, sem SDL
//! e sem GPU; e o desenho passa a mesma função de medir que usa para desenhar,
//! então o corte nunca discorda do que foi desenhado.

/// Encurta `texto` para caber em `largura_max`, com reticências.
///
/// Corta em **caractere**, nunca em byte: `Música` tem 6 caracteres e 7 bytes,
/// e cortar por byte produziria texto inválido (e, no melhor caso, um `?`).
/// Devolve string vazia quando nem as reticências cabem — devolver `…` que
/// estoura a largura seria mentir sobre o próprio contrato.
pub fn encurtar(texto: &str, largura_max: i32, mut medir: impl FnMut(&str) -> i32) -> String {
    if largura_max <= 0 {
        return String::new();
    }
    if medir(texto) <= largura_max {
        return texto.to_string();
    }

    const RETICENCIAS: &str = "…";
    if medir(RETICENCIAS) > largura_max {
        return String::new();
    }

    let mut saida = String::new();
    for c in texto.chars() {
        let mut tentativa = saida.clone();
        tentativa.push(c);
        tentativa.push_str(RETICENCIAS);
        if medir(&tentativa) > largura_max {
            break;
        }
        saida.push(c);
    }
    saida.push_str(RETICENCIAS);
    saida
}

/// Formata segundos como `m:ss` (ou `h:mm:ss` em faixas longas).
///
/// `None` para valor que não é duração: negativo, `NaN` ou infinito. Zero é um
/// valor legítimo (`0:00`) — o que não existe é duração desconhecida, e essa é
/// representada pela ausência do `Option`, nunca por zero.
pub fn formatar_tempo(segundos: f32) -> Option<String> {
    if !segundos.is_finite() || segundos < 0.0 {
        return None;
    }
    let total = segundos.round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    Some(if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    })
}

/// Concorda o número com o substantivo: `1 faixa`, `0 faixas`, `2 faixas`.
pub fn plural(n: usize, singular: &str, muitos: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {muitos}")
    }
}

/// Um par tecla/legenda do rodapé.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dica {
    pub tecla: &'static str,
    pub rotulo: &'static str,
}

impl Dica {
    pub fn texto(&self) -> String {
        format!("{} {}", self.tecla, self.rotulo)
    }
}

/// Dicas do rodapé, completas quando cabem e compactas quando não cabem.
///
/// O rodapé é a única documentação que o usuário tem na mão: prometer uma tecla
/// que não existe é pior que não listar. Por isso a lista encurta em vez de ser
/// cortada pelo desenho — corte de desenho some com a última dica, e foi assim
/// que um rodapé chegou a prometer `A — Jogar` onde `A` abria a ficha.
pub fn dicas(largura: i32, medir: impl FnMut(&str) -> i32) -> Vec<Dica> {
    const COMPLETAS: [Dica; 5] = [
        Dica { tecla: "A", rotulo: "Tocar/Pausar" },
        Dica { tecla: "X", rotulo: "Próxima" },
        Dica { tecla: "Y", rotulo: "Anterior" },
        Dica { tecla: "LB/RB", rotulo: "Volume" },
        Dica { tecla: "START", rotulo: "Sair" },
    ];
    const CURTAS: [Dica; 5] = [
        Dica { tecla: "A", rotulo: "Tocar" },
        Dica { tecla: "X", rotulo: "Próx." },
        Dica { tecla: "Y", rotulo: "Ant." },
        Dica { tecla: "LB/RB", rotulo: "Vol." },
        Dica { tecla: "START", rotulo: "Sair" },
    ];

    fn largura_de(lista: &[Dica], medir: &mut impl FnMut(&str) -> i32) -> i32 {
        lista
            .iter()
            .map(|d| medir(&d.texto()))
            .sum::<i32>()
            + 14 * lista.len().saturating_sub(1) as i32
    }

    let mut medir = medir;
    if largura_de(&COMPLETAS, &mut medir) <= largura {
        COMPLETAS.to_vec()
    } else if largura_de(&CURTAS, &mut medir) <= largura {
        CURTAS.to_vec()
    } else {
        CURTAS[..2].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Medidor de fonte monoespaçada: 10 px por caractere.
    fn mono(texto: &str) -> i32 {
        texto.chars().count() as i32 * 10
    }

    // ── encurtar ────────────────────────────────────────────

    #[test]
    fn texto_que_cabe_nao_muda() {
        assert_eq!(encurtar("ABCDEFGHIJ", 100, mono), "ABCDEFGHIJ");
        assert_eq!(encurtar("ABC", 30, mono), "ABC");
    }

    #[test]
    fn texto_longo_ganha_reticencias_sem_estourar() {
        let curto = encurtar("ABCDEFGHIJ", 55, mono);
        assert_eq!(curto, "ABCD…");
        assert!(mono(&curto) <= 55, "estourou: {curto}");
        // Um caractere a mais estouraria: o corte é o máximo que cabe.
        assert!(mono("ABCDE…") > 55);
    }

    #[test]
    fn encurtar_conta_caracteres_nao_bytes() {
        // "Música" tem 6 caracteres e 7 bytes. Se o corte fosse por byte, o
        // resultado seria inválido ou teria um byte a menos.
        assert_eq!(encurtar("Música Ação", 80, mono), "Música …");
        assert!(encurtar("Música", 60, mono).is_char_boundary(3));
    }

    #[test]
    fn encurtar_sem_espaco_nao_devolve_reticencia_que_estoura() {
        assert_eq!(encurtar("ABC", 5, mono), "");
        assert_eq!(encurtar("ABC", 0, mono), "");
        assert_eq!(encurtar("ABC", -10, mono), "");
        assert_eq!(encurtar("", 100, mono), "");
    }

    #[test]
    fn encurtar_nunca_devolve_texto_maior_que_o_original() {
        for limite in [10, 20, 30, 45, 60, 200] {
            let saida = encurtar("Uma música com nome bem comprido", limite, mono);
            assert!(mono(&saida) <= limite.max(0), "limite {limite}: {saida:?}");
        }
    }

    // ── tempo ───────────────────────────────────────────────

    #[test]
    fn tempo_formata_minuto_e_segundo() {
        assert_eq!(formatar_tempo(0.0).as_deref(), Some("0:00"));
        assert_eq!(formatar_tempo(9.0).as_deref(), Some("0:09"));
        assert_eq!(formatar_tempo(61.0).as_deref(), Some("1:01"));
        assert_eq!(formatar_tempo(222.4).as_deref(), Some("3:42"));
        // Arredonda para o segundo mais próximo: 59,6 s é "1:00", não "0:59".
        assert_eq!(formatar_tempo(59.6).as_deref(), Some("1:00"));
    }

    #[test]
    fn tempo_longo_ganha_hora() {
        assert_eq!(formatar_tempo(3600.0).as_deref(), Some("1:00:00"));
        assert_eq!(formatar_tempo(3725.0).as_deref(), Some("1:02:05"));
    }

    #[test]
    fn tempo_invalido_nao_vira_zero() {
        assert_eq!(formatar_tempo(-1.0), None);
        assert_eq!(formatar_tempo(f32::NAN), None);
        assert_eq!(formatar_tempo(f32::INFINITY), None);
    }

    // ── plural ──────────────────────────────────────────────

    #[test]
    fn plural_concorda_com_o_numero() {
        assert_eq!(plural(0, "faixa", "faixas"), "0 faixas");
        assert_eq!(plural(1, "faixa", "faixas"), "1 faixa");
        assert_eq!(plural(2, "faixa", "faixas"), "2 faixas");
    }

    // ── dicas ───────────────────────────────────────────────

    #[test]
    fn rodape_largo_mostra_todas_as_dicas() {
        let lista = dicas(1280, mono);
        assert_eq!(lista.len(), 5);
        assert_eq!(lista[0].rotulo, "Tocar/Pausar");
    }

    #[test]
    fn rodape_estreito_encurta_em_vez_de_cortar() {
        let lista = dicas(300, mono);
        assert!(!lista.is_empty());
        assert_eq!(lista[0].rotulo, "Tocar");
        let total: i32 = lista.iter().map(|d| mono(&d.texto()) + 14).sum();
        assert!(total <= 300 + 14, "as dicas continuam maiores que o rodapé");
    }

    #[test]
    fn rodape_minusculo_mostra_o_essencial() {
        let lista = dicas(40, mono);
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].tecla, "A");
        assert_eq!(lista[1].tecla, "X");
    }

    #[test]
    fn toda_dica_tem_texto_nao_vazio() {
        for largura in [40, 300, 1280] {
            for dica in dicas(largura, mono) {
                assert!(!dica.tecla.is_empty());
                assert!(!dica.rotulo.is_empty());
            }
        }
    }
}
