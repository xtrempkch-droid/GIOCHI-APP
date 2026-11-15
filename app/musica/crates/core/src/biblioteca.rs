//! A biblioteca: o que existe na pasta, e — quando não existe nada — por quê.
//!
//! A regra que dá forma a este módulo: **ausência é um estado, não uma lista
//! vazia.** Um app que desenha uma tela em branco quando a pasta não existe
//! obriga o usuário a adivinhar se o cartão não tem música, se o caminho está
//! errado ou se o app quebrou. Por isso `Vazia` carrega o motivo, e o motivo
//! vira frase na tela.

use crate::faixa::{duracao_wav, ordem_natural, Faixa};

/// Uma entrada de diretório, já lida por quem tem acesso ao disco.
///
/// `&str` e um `bool` em vez de `std::fs::DirEntry` para que o teste monte a
/// pasta que quiser sem criar arquivo nenhum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entrada<'a> {
    pub nome: &'a str,
    pub eh_dir: bool,
}

impl<'a> Entrada<'a> {
    pub fn arquivo(nome: &'a str) -> Self {
        Entrada { nome, eh_dir: false }
    }

    pub fn diretorio(nome: &'a str) -> Self {
        Entrada { nome, eh_dir: true }
    }
}

/// Por que a biblioteca está vazia.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vazio {
    /// O caminho não existe (ou não é diretório).
    PastaNaoExiste,
    /// A pasta existe e não tem nenhum arquivo de áudio.
    SemMusica,
    /// A pasta existe e o processo não conseguiu lê-la.
    SemLeitura,
}

impl Vazio {
    /// Frase para a tela. Diz o que aconteceu **e** onde.
    pub fn explicacao(self, pasta: &str) -> String {
        match self {
            Vazio::PastaNaoExiste => format!("a pasta {pasta} não existe"),
            Vazio::SemMusica => format!("nenhuma música em {pasta}"),
            Vazio::SemLeitura => format!("sem permissão para ler {pasta}"),
        }
    }
}

/// O resultado da descoberta.
#[derive(Clone, Debug, PartialEq)]
pub enum Biblioteca {
    /// `faixas` nunca é vazio: uma lista vazia é `Vazia`, e o enum garante que
    /// quem desenha a lista também saiba desenhar a ausência.
    Com { pasta: String, faixas: Vec<Faixa> },
    Vazia { pasta: String, motivo: Vazio },
}

impl Biblioteca {
    pub fn pasta(&self) -> &str {
        match self {
            Biblioteca::Com { pasta, .. } | Biblioteca::Vazia { pasta, .. } => pasta,
        }
    }

    pub fn faixas(&self) -> &[Faixa] {
        match self {
            Biblioteca::Com { faixas, .. } => faixas,
            Biblioteca::Vazia { .. } => &[],
        }
    }

    pub fn total(&self) -> usize {
        self.faixas().len()
    }

    pub fn esta_vazia(&self) -> bool {
        matches!(self, Biblioteca::Vazia { .. })
    }

    /// Motivo da ausência, quando há um.
    pub fn motivo(&self) -> Option<Vazio> {
        match self {
            Biblioteca::Vazia { motivo, .. } => Some(*motivo),
            Biblioteca::Com { .. } => None,
        }
    }

    /// Frase única para a tela e para o log.
    pub fn mensagem(&self) -> String {
        match self {
            Biblioteca::Com { faixas, .. } => crate::texto::plural(
                faixas.len(),
                "faixa encontrada",
                "faixas encontradas",
            ),
            Biblioteca::Vazia { pasta, motivo } => motivo.explicacao(pasta),
        }
    }
}

/// Monta a biblioteca a partir das entradas de uma pasta já lida.
///
/// Não lê disco, não lê cabeçalho de arquivo: quem chama é que faz isso. A
/// divisão deixa a decisão "o que é música" testável — e essa decisão já errou
/// em outros lugares por aceitar qualquer arquivo e mostrar cartão que não abre.
pub fn montar(pasta: &str, entradas: &[Entrada]) -> Biblioteca {
    let mut faixas: Vec<Faixa> = entradas
        .iter()
        .filter(|e| !e.eh_dir)
        // Nome escondido não é música: `.mp3` é lixo de sistema, e `.Trash-1000`
        // é a lixeira de um cartão montado.
        .filter(|e| !e.nome.starts_with('.'))
        .filter_map(|e| Faixa::nova(pasta, e.nome))
        .collect();

    if faixas.is_empty() {
        return Biblioteca::Vazia {
            pasta: pasta.to_string(),
            motivo: Vazio::SemMusica,
        };
    }

    // Ordem natural pelo **nome do arquivo**: é o que o usuário vê no gerenciador
    // de arquivos e o que carrega o número de faixa do álbum.
    faixas.sort_by(|a, b| ordem_natural(&a.arquivo, &b.arquivo));

    Biblioteca::Com {
        pasta: pasta.to_string(),
        faixas,
    }
}

/// Pasta de fábrica, na ordem em que o app procura.
///
/// `existe` é injetado para o teste não depender do disco de quem roda: a regra
/// que importa é a **ordem**, e ela já errou quando o app procurava só no PC.
///
/// O último recurso é o caminho do aparelho mesmo quando ele não existe: é ele
/// que a tela precisa mostrar ("a pasta /user/musica não existe"), e mostrar o
/// caminho do PC num aparelho mandaria o usuário procurar no lugar errado.
pub fn pasta_padrao(
    env: Option<&str>,
    existe: impl Fn(&str) -> bool,
    home: &str,
) -> String {
    const NO_APARELHO: &str = "/user/musica";

    if let Some(valor) = env {
        let valor = valor.trim();
        if !valor.is_empty() {
            return valor.to_string();
        }
    }
    if existe(NO_APARELHO) {
        return NO_APARELHO.to_string();
    }
    for nome in ["Música", "Music"] {
        let caminho = format!("{home}/{nome}");
        if existe(&caminho) {
            return caminho;
        }
    }
    NO_APARELHO.to_string()
}

/// Completa a duração das faixas que declaram o tamanho no cabeçalho.
///
/// Recebe os primeiros bytes do arquivo (o chamador lê 4 KiB) e devolve a
/// duração medida. Formatos sem essa informação continuam com `None` — o app
/// não inventa número para preencher a ficha.
pub fn medir_duracao(faixa: &mut Faixa, primeiros_bytes: &[u8], tamanho: u64) {
    if faixa.formato != crate::faixa::Formato::Wav {
        return;
    }
    // Cabeçalho menor que o chunk `data` não permite medir; o teto existe para
    // um arquivo corrompido não pedir a leitura inteira.
    if tamanho < 44 {
        return;
    }
    faixa.duracao = duracao_wav(primeiros_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faixa::Formato;

    fn nomes(b: &Biblioteca) -> Vec<&str> {
        b.faixas().iter().map(|f| f.arquivo.as_str()).collect()
    }

    #[test]
    fn pasta_com_musicas_vira_lista_ordenada() {
        let entradas = [
            Entrada::arquivo("Faixa 10.mp3"),
            Entrada::arquivo("Faixa 2.mp3"),
            Entrada::arquivo("capa.jpg"),
            Entrada::diretorio("extras"),
            Entrada::arquivo(".escondida.mp3"),
        ];
        let b = montar("/user/musica", &entradas);
        assert!(!b.esta_vazia());
        assert_eq!(nomes(&b), vec!["Faixa 2.mp3", "Faixa 10.mp3"]);
        assert_eq!(b.pasta(), "/user/musica");
        assert_eq!(b.faixas()[0].caminho, "/user/musica/Faixa 2.mp3");
        assert_eq!(b.faixas()[0].titulo, "Faixa 2");
    }

    #[test]
    fn pasta_sem_audio_diz_que_nao_tem_musica() {
        let entradas = [Entrada::arquivo("leiame.txt"), Entrada::diretorio("fotos")];
        let b = montar("/user/musica", &entradas);
        assert!(b.esta_vazia());
        assert_eq!(b.motivo(), Some(Vazio::SemMusica));
        assert_eq!(b.mensagem(), "nenhuma música em /user/musica");
        assert!(b.faixas().is_empty());
        assert_eq!(b.total(), 0);
    }

    #[test]
    fn pasta_vazia_mostra_o_caminho_na_mensagem() {
        let b = montar("/mnt/cartao/musicas", &[]);
        assert_eq!(b.mensagem(), "nenhuma música em /mnt/cartao/musicas");
        // O caminho errado é o diagnóstico mais provável; ele tem de aparecer.
        assert!(b.mensagem().contains("/mnt/cartao/musicas"));
    }

    #[test]
    fn motivo_de_pasta_ausente_e_diferente_de_pasta_vazia() {
        let b = Biblioteca::Vazia {
            pasta: "/user/musica".into(),
            motivo: Vazio::PastaNaoExiste,
        };
        assert_eq!(b.mensagem(), "a pasta /user/musica não existe");
        assert_ne!(b.mensagem(), Vazio::SemMusica.explicacao("/user/musica"));
    }

    #[test]
    fn lista_cheia_anuncia_quantas_faixas() {
        let entradas = [Entrada::arquivo("a.mp3")];
        assert_eq!(montar("/m", &entradas).mensagem(), "1 faixa encontrada");
        let entradas = [Entrada::arquivo("a.mp3"), Entrada::arquivo("b.ogg")];
        assert_eq!(montar("/m", &entradas).mensagem(), "2 faixas encontradas");
    }

    #[test]
    fn arquivo_de_audio_com_caixa_alta_e_reconhecido() {
        let entradas = [Entrada::arquivo("MUSICA.MP3")];
        let b = montar("/m", &entradas);
        assert_eq!(b.total(), 1);
        assert_eq!(b.faixas()[0].formato, Formato::Mp3);
    }

    // ── pasta padrão ────────────────────────────────────────

    #[test]
    fn ambiente_vence_tudo() {
        let pasta = pasta_padrao(Some("/mnt/pendrive"), |p| p == "/user/musica", "/home/u");
        assert_eq!(pasta, "/mnt/pendrive");
        // Variável vazia é ausência de configuração, não caminho "".
        let pasta = pasta_padrao(Some("   "), |p| p == "/user/musica", "/home/u");
        assert_eq!(pasta, "/user/musica");
    }

    #[test]
    fn aparelho_vem_antes_da_pasta_do_pc() {
        let pasta = pasta_padrao(None, |_| true, "/home/u");
        assert_eq!(pasta, "/user/musica");
    }

    #[test]
    fn sem_pasta_do_aparelho_procura_em_casa() {
        let pasta = pasta_padrao(None, |p| p == "/home/u/Música", "/home/u");
        assert_eq!(pasta, "/home/u/Música");
        let pasta = pasta_padrao(None, |p| p == "/home/u/Music", "/home/u");
        assert_eq!(pasta, "/home/u/Music");
    }

    #[test]
    fn sem_nada_devolve_o_caminho_do_aparelho() {
        // Devolver "" faria a tela dizer "nenhuma música em " — que não
        // diagnostica nada.
        let pasta = pasta_padrao(None, |_| false, "/home/u");
        assert_eq!(pasta, "/user/musica");
    }

    // ── duração ─────────────────────────────────────────────

    #[test]
    fn duracao_so_e_medida_no_formato_que_declara() {
        let mut wav = Faixa::nova("/m", "a.wav").unwrap();
        let mut cabecalho = Vec::new();
        cabecalho.extend_from_slice(b"RIFF");
        cabecalho.extend_from_slice(&100u32.to_le_bytes());
        cabecalho.extend_from_slice(b"WAVE");
        cabecalho.extend_from_slice(b"fmt ");
        cabecalho.extend_from_slice(&16u32.to_le_bytes());
        cabecalho.extend_from_slice(&1u16.to_le_bytes());
        cabecalho.extend_from_slice(&2u16.to_le_bytes());
        cabecalho.extend_from_slice(&44_100u32.to_le_bytes());
        cabecalho.extend_from_slice(&176_400u32.to_le_bytes());
        cabecalho.extend_from_slice(&4u16.to_le_bytes());
        cabecalho.extend_from_slice(&16u16.to_le_bytes());
        cabecalho.extend_from_slice(b"data");
        cabecalho.extend_from_slice(&176_400u32.to_le_bytes());
        medir_duracao(&mut wav, &cabecalho, 176_444);
        assert_eq!(wav.duracao_legivel(), "0:01");

        // MP3 não declara: continua `—`, e nunca 0:00.
        let mut mp3 = Faixa::nova("/m", "a.mp3").unwrap();
        medir_duracao(&mut mp3, &cabecalho, 176_444);
        assert_eq!(mp3.duracao, None);
        assert_eq!(mp3.duracao_legivel(), "—");
    }

    #[test]
    fn arquivo_menor_que_o_cabecalho_nao_e_medido() {
        let mut wav = Faixa::nova("/m", "a.wav").unwrap();
        medir_duracao(&mut wav, b"RIFF", 10);
        assert_eq!(wav.duracao, None);
    }
}
