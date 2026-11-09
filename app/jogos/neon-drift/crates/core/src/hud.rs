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

// ── geometria do rodapé ───────────────────────────────────────────
//
// A geometria mora aqui porque **sobreposição é um fato aritmético**, não
// visual. O rodapé já saiu errado uma vez: `texto()` recebe o *topo* da linha
// e o código somava deslocamentos fixos (`base_y - 2·escala`) para um glifo de
// `7·escala` de altura. Em 640×480 isso deu velocidade 424..457, marcha
// 439..464 e barra 452..461 — três elementos na mesma faixa de 40 px, que o
// usuário viu como "o velocímetro em cima do nitro".
//
// Nenhum teste pegava porque o desenho estava legível e nada comparava duas
// caixas. Aqui a caixa de cada elemento sai da mesma função que o desenho usa,
// então `caixas_do_rodape` pode afirmar que elas não se tocam.

/// Uma caixa de texto do HUD: canto superior esquerdo e altura de linha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaixaTexto {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

impl CaixaTexto {
    pub fn base(&self) -> f32 {
        self.y + self.altura
    }

    /// Sobreposição de área entre duas caixas. Zero significa que não se tocam.
    pub fn sobreposicao(&self, outra: &CaixaTexto) -> f32 {
        let dx = (self.x + self.largura).min(outra.x + outra.largura) - self.x.max(outra.x);
        let dy = self.base().min(outra.base()) - self.y.max(outra.y);
        if dx > 0.0 && dy > 0.0 {
            dx * dy
        } else {
            0.0
        }
    }
}

/// Um retângulo do HUD (a barra de nitro e o fundo dela).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caixa {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

/// Posições do rodapé esquerdo: velocímetro, unidade, marcha, barra e rótulo.
///
/// A pilha é montada **de baixo para cima** a partir da barra, que é o elemento
/// mais próximo da borda. Cada passo subtrai a altura **real** da linha
/// seguinte (`fonte::ALTURA × escala`), não um deslocamento escolhido a olho.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rodape {
    pub velocidade: CaixaTexto,
    pub unidade: CaixaTexto,
    pub marcha: CaixaTexto,
    pub barra: Caixa,
    pub rotulo: CaixaTexto,
    /// Escala da fonte de cada linha, para o desenho não recalcular.
    pub escala_velocidade: f32,
    pub escala_marcha: f32,
    pub escala_unidade: f32,
    pub escala_rotulo: f32,
}

/// Monta o rodapé para um painel de `largura` × `altura` em espaço de projeto.
///
/// `velocidade` e `marcha` entram como texto porque o `x` da unidade depende da
/// largura do número — que muda de `0` para `999`.
pub fn rodape(largura: f32, altura: f32, velocidade: &str, marcha: &str) -> Rodape {
    let escala = (largura / 220.0).clamp(1.8, 4.2);
    let escala_velocidade = escala * 1.6;
    let escala_marcha = escala * 1.2;
    let escala_unidade = escala * 0.6;
    let escala_rotulo = escala * 0.55;

    let altura_linha = |e: f32| crate::fonte::ALTURA as f32 * e;
    let x = largura * 0.03;
    let barra_largura = largura * 0.30;
    let barra_altura = (escala * 2.2).max(5.0);
    // Respiro até a borda do painel: existe para o texto não encostar no bisel.
    let margem = escala * 2.2;

    let barra_y = altura - margem - barra_altura;
    let marcha_y = barra_y - escala * 1.4 - altura_linha(escala_marcha);
    let velocidade_y = marcha_y - escala - altura_linha(escala_velocidade);

    let velocidade = CaixaTexto {
        x,
        y: velocidade_y,
        largura: largura_em_pixels(velocidade, escala_velocidade),
        altura: altura_linha(escala_velocidade),
    };
    // A unidade é sufoco do número: encosta à direita dele e alinha pela base.
    let unidade = CaixaTexto {
        x: x + velocidade.largura + escala * 1.2,
        y: velocidade.base() - altura_linha(escala_unidade),
        largura: largura_em_pixels("KM/H", escala_unidade),
        altura: altura_linha(escala_unidade),
    };
    let marcha = CaixaTexto {
        x,
        y: marcha_y,
        largura: largura_em_pixels(marcha, escala_marcha),
        altura: altura_linha(escala_marcha),
    };
    let rotulo = CaixaTexto {
        x: x + barra_largura + escala * 1.5,
        y: barra_y + barra_altura - altura_linha(escala_rotulo),
        largura: largura_em_pixels("NITRO", escala_rotulo),
        altura: altura_linha(escala_rotulo),
    };

    Rodape {
        velocidade,
        unidade,
        marcha,
        barra: Caixa {
            x,
            y: barra_y,
            largura: barra_largura,
            altura: barra_altura,
        },
        rotulo,
        escala_velocidade,
        escala_marcha,
        escala_unidade,
        escala_rotulo,
    }
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

    /// Telas que o jogo precisa cobrir: 480p (X35H/R35S), 600p, 720p (X55 em
    /// projeto) e o painel retrato do X55.
    const TELAS: [(f32, f32); 4] = [
        (640.0, 480.0),
        (800.0, 600.0),
        (1280.0, 720.0),
        (720.0, 1280.0),
    ];

    #[test]
    fn os_elementos_do_rodape_nao_se_sobrepoem() {
        // O bug relatado pelo usuário: "o contador de velocidade fica muito
        // próximo ao contador de nitro e acaba sobrepondo". Antes da correção
        // as três caixas dividiam a mesma faixa de 40 px em 640×480.
        for (largura, altura) in TELAS {
            for kmh in ["0", "7", "99", "223"] {
                let r = rodape(largura, altura, kmh, "3");
                let caixas = [
                    ("velocidade", r.velocidade),
                    ("unidade", r.unidade),
                    ("marcha", r.marcha),
                    ("rotulo", r.rotulo),
                ];
                for (i, (nome_a, a)) in caixas.iter().enumerate() {
                    for (nome_b, b) in &caixas[i + 1..] {
                        assert_eq!(
                            a.sobreposicao(b),
                            0.0,
                            "{largura}×{altura} com {kmh} km/h: {nome_a} encosta em {nome_b}"
                        );
                    }
                    let barra = CaixaTexto {
                        x: r.barra.x,
                        y: r.barra.y,
                        largura: r.barra.largura,
                        altura: r.barra.altura,
                    };
                    if *nome_a != "rotulo" {
                        assert_eq!(
                            a.sobreposicao(&barra),
                            0.0,
                            "{largura}×{altura}: {nome_a} encosta na barra de nitro"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn o_velocimetro_fica_acima_e_a_barra_embaixo() {
        // A ordem vertical é a leitura do canto: número grande, marcha, barra.
        let r = rodape(640.0, 480.0, "156", "3");
        assert!(r.velocidade.base() < r.marcha.y, "velocidade acima da marcha");
        assert!(r.marcha.base() < r.barra.y, "marcha acima da barra");
        // E a barra é o elemento mais baixo, com margem até o bisel.
        assert!(r.barra.y + r.barra.altura < 480.0);
        let folga = 480.0 - (r.barra.y + r.barra.altura);
        assert!(folga > 4.0, "folga até a borda era {folga}");
    }

    #[test]
    fn o_rodape_cabe_no_painel() {
        for (largura, altura) in TELAS {
            let r = rodape(largura, altura, "223", "5");
            for (nome, c) in [
                ("velocidade", r.velocidade),
                ("unidade", r.unidade),
                ("marcha", r.marcha),
                ("rotulo", r.rotulo),
            ] {
                assert!(c.x >= 0.0 && c.x + c.largura <= largura, "{nome} sai pela direita em {largura}×{altura}");
                assert!(c.y >= 0.0 && c.base() <= altura, "{nome} sai por baixo em {largura}×{altura}");
            }
            assert!(r.barra.x + r.barra.largura <= largura);
        }
    }

    #[test]
    fn a_unidade_encosta_a_direita_do_numero() {
        // A unidade saía solta no meio do nada porque o `x` somava a própria
        // largura de "KM/H" ao deslocamento. Ela é sufixo: começa depois do
        // número e alinha pela base.
        let r = rodape(640.0, 480.0, "156", "3");
        assert!((r.unidade.x - (r.velocidade.x + r.velocidade.largura)).abs() < 6.0);
        assert!((r.unidade.base() - r.velocidade.base()).abs() < 0.01);
    }

    #[test]
    fn a_escada_do_rodape_e_medida_pela_altura_da_fonte() {
        // A regra que faltava: cada degrau usa `7·escala` da linha que está
        // acima, não uma constante. Se alguém voltar a usar deslocamento fixo,
        // a distância entre a base da velocidade e o topo da marcha deixa de
        // ser `escala`.
        for (largura, altura) in TELAS {
            let r = rodape(largura, altura, "99", "2");
            let escala = (largura / 220.0).clamp(1.8, 4.2);
            let vao_vel_marcha = r.marcha.y - r.velocidade.base();
            assert!(
                (vao_vel_marcha - escala).abs() < 0.01,
                "{largura}×{altura}: vão era {vao_vel_marcha}, esperado {escala}"
            );
        }
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
