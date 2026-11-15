//! Preferências que sobrevivem ao boot: **idioma** e **tema**.
//!
//! ## Por que em arquivo, e não no estado do processo
//!
//! O launcher é iniciado pelo `giochi-init` e o processo termina a cada app que
//! roda. Tudo que não for gravado volta ao padrão de código no próximo boot — e
//! o usuário conclui que a escolha "não pegou". A gravação segue o mesmo
//! caminho do brilho (`/user/.config/giochi/brightness`), que é o arquivo que o
//! próprio `giochi-init` relê.
//!
//! ## Dois arquivos, um valor em cada
//!
//! `/user/.config/giochi/language` e `/user/.config/giochi/theme` guardam um
//! código curto (`en`, `pt`, `giochi-dark`). Um arquivo só com JSON exigiria um
//! leitor de JSON que não falha — e não há: `serde_json` transforma um arquivo
//! truncado por queda de energia em erro de parse. Um código de duas letras
//! lido com `trim` não tem esse problema, e é o mesmo formato dos outros três
//! arquivos da pasta.
//!
//! ## `GIOCHI_IDIOMA` existe para diagnóstico
//!
//! Igual a `GIOCHI_HORA_FIXA`: permite abrir o launcher num idioma sem tocar em
//! `/user` — é assim que se fotografa a mesma tela em dez idiomas no PC. A
//! variável **vence o arquivo**, porque é diagnóstico e não escolha; quem quer
//! a escolha apaga a variável.
//!
//! ## Gravar com `rename`, não com `>` direto
//!
//! Um corte de energia no meio da escrita deixaria o arquivo truncado — e um
//! arquivo truncado não é "sem preferência", é uma preferência que não parseia.
//! Grava no `.tmp` e move: o `rename` no mesmo sistema de arquivos é atômico.

use std::fs;
use std::path::{Path, PathBuf};

use giochi_launcher_core::idioma::Idioma;
use giochi_launcher_core::tema::Tema;

/// Escolha do idioma da interface.
pub const ARQUIVO_IDIOMA: &str = "/user/.config/giochi/language";
/// Escolha do tema visual.
pub const ARQUIVO_TEMA: &str = "/user/.config/giochi/theme";
/// Sobrepõe o arquivo — diagnóstico, não escolha do usuário.
pub const VAR_IDIOMA: &str = "GIOCHI_IDIOMA";

/// Interpreta o conteúdo de um arquivo de preferência.
///
/// Pura de propósito: é a única parte desta camada que pode ser testada sem
/// `/user`. Aceita o que aparece na prática — `pt`, `pt-BR`, linha terminada em
/// `\n`, espaço sobrando, arquivo escrito à mão.
pub fn ler_codigo(conteudo: &str) -> Option<&str> {
    let linha = conteudo.lines().next().unwrap_or("").trim();
    (!linha.is_empty()).then_some(linha)
}

fn ler(caminho: &str) -> Option<String> {
    fs::read_to_string(caminho).ok()
}

fn gravar(caminho: &str, valor: &str) -> Result<(), String> {
    let destino = Path::new(caminho);
    if let Some(dir) = destino.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("criar {}: {e}", dir.display()))?;
    }
    let mut tmp: PathBuf = destino.to_path_buf();
    tmp.set_extension("tmp");
    // Uma linha só, terminada em `\n`: o arquivo continua legível com `cat` e
    // com `echo` de qualquer editor de texto do aparelho.
    fs::write(&tmp, format!("{valor}\n")).map_err(|e| format!("gravar {}: {e}", tmp.display()))?;
    fs::rename(&tmp, destino).map_err(|e| format!("mover para {}: {e}", destino.display()))?;
    Ok(())
}

/// Idioma efetivo: `GIOCHI_IDIOMA` primeiro, depois o arquivo.
///
/// `None` significa **primeira abertura** — é o que autoriza a tela de
/// boas-vindas. Não devolver o padrão aqui é deliberado: "não escolhido" e
/// "escolhido como inglês" são fatos diferentes, e só o primeiro deve perguntar.
pub fn carregar_idioma() -> Option<Idioma> {
    if let Ok(v) = std::env::var(VAR_IDIOMA) {
        if let Some(idioma) = Idioma::de_codigo(&v) {
            eprintln!("idioma: {VAR_IDIOMA}={} (diagnóstico: vence o arquivo)", idioma.codigo());
            return Some(idioma);
        }
        if !v.trim().is_empty() {
            eprintln!(
                "aviso: {VAR_IDIOMA}={v:?} não é um dos dez idiomas; ignorado \
                 (aceitos: en, zh, hi, es, fr, ar, bn, pt, ru, ur)"
            );
        }
    }
    let conteudo = ler(ARQUIVO_IDIOMA)?;
    match ler_codigo(&conteudo).and_then(Idioma::de_codigo) {
        Some(idioma) => Some(idioma),
        None => {
            // Arquivo existe e não parseia: isso **não** é primeira abertura (o
            // arquivo foi criado por alguém), então não se pergunta de novo — e
            // o aviso diz o que corrigir.
            eprintln!(
                "aviso: {} tem {conteudo:?}, que não é um idioma; usando o padrão",
                ARQUIVO_IDIOMA
            );
            None
        }
    }
}

pub fn salvar_idioma(idioma: Idioma) -> Result<(), String> {
    gravar(ARQUIVO_IDIOMA, idioma.codigo())
}

/// Tema gravado, se houver um válido.
pub fn carregar_tema() -> Option<Tema> {
    let conteudo = ler(ARQUIVO_TEMA)?;
    let codigo = ler_codigo(&conteudo)?;
    let tema = Tema::de_id(codigo);
    if tema.is_none() {
        eprintln!("aviso: {ARQUIVO_TEMA} tem {codigo:?}, que não é um tema; usando o padrão");
    }
    tema
}

pub fn salvar_tema(tema: Tema) -> Result<(), String> {
    gravar(ARQUIVO_TEMA, tema.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O que aparece de verdade nos arquivos de `/user/.config/giochi`.
    ///
    /// Cada caso é um formato que já foi escrito à mão ou por `echo`: uma linha
    /// com `\n` final, espaço sobrando, a etiqueta BCP-47 inteira, e o arquivo
    /// vazio (que **não** pode virar idioma nenhum).
    #[test]
    fn le_o_que_aparece_no_arquivo() {
        assert_eq!(ler_codigo("pt\n"), Some("pt"));
        assert_eq!(ler_codigo("  en  \n"), Some("en"));
        assert_eq!(ler_codigo("pt-BR\n"), Some("pt-BR"));
        assert_eq!(ler_codigo(""), None);
        assert_eq!(ler_codigo("\n\n"), None);
        assert_eq!(ler_codigo("   "), None);
    }

    #[test]
    fn o_codigo_lido_volta_ao_idioma() {
        let de = |c: &str| ler_codigo(c).and_then(Idioma::de_codigo);
        assert_eq!(de("pt\n"), Some(Idioma::Pt));
        assert_eq!(de("zh-CN\n"), Some(Idioma::Zh));
        assert_eq!(de(" ur "), Some(Idioma::Ur));
        assert_eq!(de("tlh"), None);
    }

    #[test]
    fn o_codigo_do_tema_volta_ao_tema() {
        let de = |c: &str| ler_codigo(c).and_then(Tema::de_id);
        assert_eq!(de("giochi-dark\n"), Some(Tema::GiochiDark));
        assert_eq!(de("neon"), None);
    }

    /// Gravar e reler dá o mesmo valor, e o arquivo termina em `\n`.
    ///
    /// Usa um diretório temporário porque `/user` não existe no PC: o caminho é
    /// parâmetro das funções privadas justamente para isto.
    #[test]
    fn grava_e_le_de_volta() {
        let dir = std::env::temp_dir().join(format!("giochi-pref-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let arquivo = dir.join("language");
        let caminho = arquivo.to_str().unwrap();

        gravar(caminho, "ru").expect("grava");
        assert_eq!(fs::read_to_string(caminho).unwrap(), "ru\n");
        assert_eq!(
            ler_codigo(&fs::read_to_string(caminho).unwrap()).and_then(Idioma::de_codigo),
            Some(Idioma::Ru)
        );

        // Sobrescrever não deixa lixo do anterior.
        gravar(caminho, "bn").expect("regrava");
        assert_eq!(
            ler_codigo(&fs::read_to_string(caminho).unwrap()).and_then(Idioma::de_codigo),
            Some(Idioma::Bn)
        );
        // E o `.tmp` do `rename` não fica para trás.
        assert!(!dir.join("language.tmp").exists(), "o temporário tem de sumir");

        let _ = fs::remove_dir_all(&dir);
    }
}
