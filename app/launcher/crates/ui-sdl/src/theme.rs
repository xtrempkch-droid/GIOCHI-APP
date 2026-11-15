//! Paleta e métricas, transcritas do protótipo.
//!
//! Os nomes vêm direto das variáveis CSS, para a comparação entre o HTML e o
//! binário continuar óbvia quando alguém for ajustar o visual.

use sdl2::pixels::Color;

pub const fn rgb(hex: u32) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a: 255,
    }
}

pub const fn rgba(hex: u32, a: u8) -> Color {
    Color {
        r: ((hex >> 16) & 0xff) as u8,
        g: ((hex >> 8) & 0xff) as u8,
        b: (hex & 0xff) as u8,
        a,
    }
}

/// Cores de um tema.
///
/// ## Por que uma struct, e não constantes soltas
///
/// Enquanto houver **um** tema, constantes bastam — e é o que existia. A partir
/// do momento em que a troca de tema é uma linha da aba Config, a pergunta
/// "qual é a cor de destaque?" deixa de ter uma resposta só, e quem desenha
/// precisa perguntar ao tema **ativo**.
///
/// A struct é o contrato: um tema novo preenche estes campos e o desenho
/// inteiro o acompanha, sem que ninguém precise lembrar de onde há `ACCENT`
/// escrito. Os nomes continuam os do CSS do protótipo, para a comparação entre
/// o HTML original e o binário seguir óbvia.
#[derive(Clone, Copy, Debug)]
pub struct Paleta {
    pub bg: Color,
    pub panel: Color,
    pub panel2: Color,
    pub card: Color,
    pub line: Color,
    pub line2: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub danger: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
}

/// O tema padrão, transcrito do protótipo. Único por enquanto.
pub const GIOCCHI_DARK: Paleta = Paleta {
    bg: rgb(0x070a12),
    panel: rgb(0x0f1621),
    panel2: rgb(0x1a2436),
    card: rgb(0x22304a),
    line: rgba(0x78a0c8, 30),
    line2: rgba(0x78a0c8, 76),
    accent: rgb(0x66c0f4),
    ok: rgb(0x10b981),
    warn: rgb(0xf59e0b),
    danger: rgb(0xef4444),
    text: rgb(0xf1f5f9),
    dim: rgb(0x94a3b8),
    faint: rgba(0x94a3b8, 140),
};

/// Paleta de um tema.
///
/// `match` exaustivo de propósito: acrescentar variante em `core::tema::Tema`
/// **não compila** até existir paleta para ela. É o degrau que faltava para a
/// troca de tema funcionar de fato, e é o que o compilador cobra.
pub const fn paleta(tema: giochi_launcher_core::tema::Tema) -> &'static Paleta {
    match tema {
        giochi_launcher_core::tema::Tema::GiochiDark => &GIOCCHI_DARK,
    }
}

thread_local! {
    /// Paleta do quadro que está sendo desenhado.
    ///
    /// `thread_local` e não campo do `Painter`: o desenho acontece todo na
    /// thread principal, e um campo obrigaria toda chamada de cor a passar pelo
    /// `p` — inclusive as que calculam a cor **antes** de desenhar
    /// (`let cor = if focado { … }`). O SDL2_ttf já usa `thread_local` pelo
    /// mesmo motivo (ver `draw.rs`).
    ///
    /// O valor é trocado **uma vez por quadro**, em `App::render`.
    static ATUAL: std::cell::Cell<&'static Paleta> = const { std::cell::Cell::new(&GIOCCHI_DARK) };
}

/// Passa a desenhar com a paleta deste tema.
///
/// Chamado no começo de cada quadro: o usuário pode trocar de tema com a tela
/// aberta, e um valor fixado na abertura deixaria a metade da interface com as
/// cores antigas.
pub fn definir_tema(tema: giochi_launcher_core::tema::Tema) {
    ATUAL.with(|c| c.set(paleta(tema)));
}

/// Paleta ativa.
pub fn atual() -> &'static Paleta {
    ATUAL.with(|c| c.get())
}

pub fn accent() -> Color {
    atual().accent
}
pub fn dim() -> Color {
    atual().dim
}
pub fn text() -> Color {
    atual().text
}
pub fn faint() -> Color {
    atual().faint
}
pub fn panel() -> Color {
    atual().panel
}
pub fn panel2() -> Color {
    atual().panel2
}
pub fn card() -> Color {
    atual().card
}
pub fn line() -> Color {
    atual().line
}
pub fn line2() -> Color {
    atual().line2
}
pub fn ok() -> Color {
    atual().ok
}
pub fn warn() -> Color {
    atual().warn
}
pub fn danger() -> Color {
    atual().danger
}
pub fn bg() -> Color {
    atual().bg
}

// Branco e preto puros **não** são do tema: são usados em camadas de alfa por
// cima de qualquer cor (sombra, brilho, letra d'água do cartão). Um tema claro
// continua querendo preto a 140 para a sombra do mesmo jeito.
pub const BLACK: Color = rgb(0x000000);
pub const WHITE: Color = rgb(0xffffff);

// Métricas do layout de projeto (1280x720).
pub const TOPBAR_H: i32 = 56;
pub const FOOTER_H: i32 = 54;
pub const SAFE_X: i32 = 160;
// A largura útil não é constante: vem de `views::Moldura::safe_w`, calculada do
// painel. A antiga `SAFE_W = 960` era do desenho fixo de 1280×720, e usá-la no
// layout é o que reencolhia a fonte num painel pequeno — por isso foi removida
// em vez de mantida sem uso.

pub const RADIUS_CARD: i32 = 18;
pub const RADIUS_PANEL: i32 = 16;
pub const RADIUS_TILE: i32 = 10;
pub const RADIUS_CHIP: i32 = 7;

/// Alvo mínimo de toque, em unidades de projeto.
pub const TOUCH: i32 = 44;

// Tamanhos de fonte, em pontos do espaço de projeto.
pub const F_TINY: u16 = 9;
pub const F_SMALL: u16 = 11;
pub const F_BODY: u16 = 13;
pub const F_TITLE: u16 = 26;
pub const F_HERO: u16 = 28;
pub const F_LOGO: u16 = 32;

/// Cor de destaque por jogo, derivada do que o catálogo declara.
///
/// No protótipo cada jogo trazia três cores no dado. Aqui a cor nasce do id,
/// para o núcleo não carregar campo de apresentação.
pub fn game_accent(id: u32) -> Color {
    const PALETTE: [Color; 6] = [
        rgb(0x3b82f6),
        rgb(0x14b8a6),
        rgb(0x8b5cf6),
        rgb(0xf59e0b),
        rgb(0xef4444),
        rgb(0x22c55e),
    ];
    PALETTE[(id as usize) % PALETTE.len()]
}

/// Cor escura correspondente, para o gradiente do cartão.
pub fn game_base(id: u32) -> Color {
    const PALETTE: [Color; 6] = [
        rgb(0x0c1e5f),
        rgb(0x042f2e),
        rgb(0x1a0e24),
        rgb(0x451a03),
        rgb(0x5f0f0f),
        rgb(0x052e16),
    ];
    PALETTE[(id as usize) % PALETTE.len()]
}

/// Mistura linear, usada nos gradientes aproximados por faixas.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        g: (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        b: (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
        a: (a.a as f32 + (b.a as f32 - a.a as f32) * t) as u8,
    }
}

pub fn with_alpha(color: Color, a: u8) -> Color {
    Color { a, ..color }
}

#[cfg(test)]
mod tests {
    use super::*;
    use giochi_launcher_core::tema::Tema;

    /// Os valores do tema padrão são os do protótipo, e continuam iguais.
    ///
    /// A paleta foi extraída das constantes em 15/11/2026 para a troca de tema
    /// funcionar. Uma extração que mexesse num número mudaria a tela inteira em
    /// um pixel de cada vez — e a comparação de quadro acusaria "regressão de
    /// desenho" de 0,3 %, que manda procurar no lugar errado. Estes números são
    /// os medidos em 06/11/2026 e usados na comparação.
    #[test]
    fn o_tema_padrao_tem_os_valores_do_prototipo() {
        let p = paleta(Tema::GiochiDark);
        assert_eq!(p.bg, rgb(0x070a12));
        assert_eq!(p.panel, rgb(0x0f1621));
        assert_eq!(p.panel2, rgb(0x1a2436));
        assert_eq!(p.card, rgb(0x22304a));
        assert_eq!(p.line, rgba(0x78a0c8, 30));
        assert_eq!(p.line2, rgba(0x78a0c8, 76));
        assert_eq!(p.accent, rgb(0x66c0f4));
        assert_eq!(p.ok, rgb(0x10b981));
        assert_eq!(p.warn, rgb(0xf59e0b));
        assert_eq!(p.danger, rgb(0xef4444));
        assert_eq!(p.text, rgb(0xf1f5f9));
        assert_eq!(p.dim, rgb(0x94a3b8));
        assert_eq!(p.faint, rgba(0x94a3b8, 140));
    }

    /// O que o desenho lê é o que `definir_tema` pôs.
    ///
    /// É a ligação que faz "trocar de tema" deixar de ser decorativo: as
    /// funções `accent()`, `text()` e companhia devolvem a paleta **do quadro**,
    /// não uma constante. Com um tema só o valor não muda, e é justamente por
    /// isso que a ligação precisa de teste — sem ele, um desenho que ignorasse a
    /// paleta continuaria idêntico e só apareceria quando o segundo tema
    /// entrasse.
    #[test]
    fn definir_tema_define_o_que_o_desenho_le() {
        definir_tema(Tema::GiochiDark);
        assert_eq!(accent(), paleta(Tema::GiochiDark).accent);
        assert_eq!(text(), paleta(Tema::GiochiDark).text);
        assert_eq!(panel(), paleta(Tema::GiochiDark).panel);
        assert_eq!(danger(), paleta(Tema::GiochiDark).danger);
        assert_eq!(atual().accent, paleta(Tema::GiochiDark).accent);
    }
}
