//! Tema visual — a base da troca de tema, com **um** tema implementado.
//!
//! ## O que existe hoje, e o que isto destrava
//!
//! A paleta é constante em `ui-sdl/src/theme.rs` (`ACCENT`, `PANEL`, …) e há um
//! tema só, o "Giochi Dark". Até 15/11/2026 a linha "Tema" da aba Config
//! mostrava `Giochi Dark (único)` e a seta → não fazia nada — não por bug, mas
//! porque nada havia para trocar. O texto dizia "único" justamente para não
//! prometer uma lista que não existia.
//!
//! Este módulo é o **degrau que faltava**: o tema passa a ser dado do estado
//! (`Settings::tema`), com ciclo, persistência e rótulo traduzido, exatamente
//! como o idioma. Quem desenha consulta `theme::paleta(state.settings.tema)`, e
//! a paleta de cada tema mora no `ui-sdl` porque é cor, não regra.
//!
//! ## Como acrescentar um tema (é o que "base pronta" significa)
//!
//! 1. uma variante aqui e a entrada em [`Tema::TODOS`];
//! 2. um `Paleta` novo em `ui-sdl/src/theme.rs`, devolvido por
//!    `theme::paleta(tema)` para essa variante;
//! 3. nada mais. A linha da aba Config, o ciclo com ← →, a gravação em
//!    `/user/.config/giochi/theme` e o desenho já leem o estado.
//!
//! O passo 2 é onde o compilador ajuda: `paleta` tem `match` exaustivo, então
//! acrescentar variante sem paleta não compila.
//!
//! ## Por que o nome do tema não é traduzido
//!
//! "Giochi Dark" é nome próprio do produto, como o próprio "Giochi". Traduzir
//! nomes de tema faria o mesmo tema aparecer com dois nomes em dois idiomas, e
//! quem procura um tema procura pelo nome que viu.

/// Tema visual do launcher.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Tema {
    /// A paleta escura original, transcrita do protótipo.
    #[default]
    GiochiDark = 0,
}

impl Tema {
    /// Todos os temas, na ordem em que a seta → percorre.
    pub const TODOS: [Tema; 1] = [Tema::GiochiDark];

    /// Tema com que o aparelho nasce.
    pub const PADRAO: Tema = Tema::GiochiDark;

    /// Identificador estável, gravado em `/user/.config/giochi/theme`.
    pub const fn id(self) -> &'static str {
        match self {
            Tema::GiochiDark => "giochi-dark",
        }
    }

    /// Nome próprio do tema, **sem tradução** (ver o topo do módulo).
    pub const fn rotulo(self) -> &'static str {
        match self {
            Tema::GiochiDark => "Giochi Dark",
        }
    }

    pub const fn indice(self) -> usize {
        self as usize
    }

    pub fn de_id(id: &str) -> Option<Tema> {
        let id = id.trim().to_ascii_lowercase();
        Tema::TODOS.iter().copied().find(|t| t.id() == id)
    }

    pub fn proximo(self) -> Tema {
        let i = (self.indice() + 1) % Tema::TODOS.len();
        Tema::TODOS[i]
    }

    pub fn anterior(self) -> Tema {
        let i = (self.indice() + Tema::TODOS.len() - 1) % Tema::TODOS.len();
        Tema::TODOS[i]
    }

    /// Há mais de um tema para escolher?
    ///
    /// A tela usa isto para não prometer uma troca que não existe: com um tema
    /// só, o rodapé não anuncia "← → Ajustar" nessa linha e a seta avisa por
    /// toast em vez de fingir que trocou alguma coisa. É a mesma regra do
    /// `settings_ajustavel` — a dica só aparece onde a tecla age.
    pub fn ha_escolha(self) -> bool {
        Tema::TODOS.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_tema_padrao_e_o_dark() {
        assert_eq!(Tema::default(), Tema::GiochiDark);
        assert_eq!(Tema::PADRAO, Tema::GiochiDark);
    }

    #[test]
    fn o_ida_e_volta_do_arquivo_encontra_o_tema() {
        assert_eq!(Tema::de_id("giochi-dark"), Some(Tema::GiochiDark));
        assert_eq!(Tema::de_id("  GIOCHI-DARK "), Some(Tema::GiochiDark));
        // Desconhecido não cai no padrão em silêncio: quem lê o arquivo decide.
        assert_eq!(Tema::de_id("neon"), None);
    }

    /// O ciclo percorre todos os temas e volta ao começo.
    ///
    /// Com **um** tema isto é um no-op de propósito — e é o teste que continua
    /// valendo quando o segundo entrar: `TODOS.len()` passos para a frente têm
    /// de devolver o mesmo tema, e um passo para trás também.
    #[test]
    fn ciclo_percorre_todos_e_volta() {
        for tema in Tema::TODOS {
            let mut t = tema;
            for _ in 0..Tema::TODOS.len() {
                t = t.proximo();
            }
            assert_eq!(t, tema, "{} não voltou a si mesmo", tema.id());
            assert_eq!(tema.anterior().proximo(), tema);
        }
    }

    /// Com um tema só, a tela não pode anunciar escolha.
    ///
    /// É a contraprova do "Tema Giochi Dark (único)": enquanto houver um tema, a
    /// linha avisa; quando o segundo entrar, este teste reprova e obriga a
    /// revisar o texto da linha — que é exatamente quando a promessa passa a ser
    /// verdadeira.
    #[test]
    fn um_tema_so_nao_promete_escolha() {
        assert_eq!(Tema::TODOS.len(), 1);
        assert!(!Tema::GiochiDark.ha_escolha());
        assert_eq!(Tema::GiochiDark.proximo(), Tema::GiochiDark);
        assert_eq!(Tema::GiochiDark.anterior(), Tema::GiochiDark);
    }
}
