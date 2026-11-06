//! Conferir um `.giochi` já baixado: hash, assinatura e conteúdo.
//!
//! # O que a assinatura do pacote prova, e o que ela não prova
//!
//! Prova que **foi a Giochi** (ou um terceiro aprovado, pela chave do item no
//! catálogo) que publicou aquele arquivo exato. **Não** prova que o usuário
//! pagou — isso é a licença, e é outro módulo, de propósito.
//!
//! Quem confundir as duas coisas terá uma loja em que um único `.giochi` copiado
//! serve para todo mundo.
//!
//! # Por que ler o tar à mão
//!
//! O pacote é ustar escrito por nós, com um layout fixo. Uma crate de tar traria
//! suporte a formatos que não usamos, e o que importa aqui é conferir **a ordem
//! dos membros** — que é contrato do extrator no aparelho (o `metadata.json` vem
//! primeiro porque ele é lido em streaming). Ler à mão deixa essa checagem
//! explícita em vez de escondida atrás de uma API de conveniência.
//!
//! O parser é deliberadamente estrito: qualquer coisa fora do ustar mínimo é
//! `Erro`, não "ignora e segue".

use sha2::{Digest, Sha256};

use crate::chave::ChavePublica;

/// Cabeçalho de 512 bytes por membro, mais o bloco de conteúdo alinhado.
const BLOCO: usize = 512;

/// Nome máximo em ustar simples (`name` 100 + `prefix` 155, sem prefixo aqui).
const NOME_MAX: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membro {
    pub caminho: String,
    pub tamanho: usize,
    pub modo: u32,
    /// Deslocamento do conteúdo dentro do arquivo do pacote.
    pub inicio: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroPacote {
    MuitoCurto,
    CabecalhoInvalido { em: usize, motivo: String },
    SemMetadata,
    MetadataInvalido(String),
    OrdemInvalida { primeiro: String },
    HashNaoConfere { esperado: String, obtido: String },
    AssinaturaInvalida,
    AssinaturaComTamanhoErrado(usize),
    UrlInvalida(String),
}

impl std::fmt::Display for ErroPacote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MuitoCurto => write!(f, "arquivo curto demais para ser um .giochi"),
            Self::CabecalhoInvalido { em, motivo } => {
                write!(f, "cabeçalho inválido em {em}: {motivo}")
            }
            Self::SemMetadata => write!(f, "pacote sem metadata.json"),
            Self::MetadataInvalido(e) => write!(f, "metadata.json inválido: {e}"),
            Self::OrdemInvalida { primeiro } => write!(
                f,
                "primeiro membro é '{primeiro}'; o extrator no aparelho lê \
                 metadata.json em streaming, então ele tem de ser o primeiro"
            ),
            Self::HashNaoConfere { esperado, obtido } => {
                write!(f, "sha256 não confere: catálogo {esperado}, arquivo {obtido}")
            }
            Self::AssinaturaInvalida => write!(
                f,
                "assinatura do pacote não confere com a chave do catálogo"
            ),
            Self::AssinaturaComTamanhoErrado(n) => {
                write!(f, "assinatura com {n} bytes; Ed25519 tem 64")
            }
            Self::UrlInvalida(u) => write!(f, "URL não é https: {u}"),
        }
    }
}

impl std::error::Error for ErroPacote {}

fn campo_octal(bloco: &[u8], inicio: usize, tamanho: usize) -> Option<u64> {
    let campo = &bloco[inicio..inicio + tamanho];
    // ustar termina o número com NUL ou espaço; os dois aparecem na prática.
    let texto = String::from_utf8_lossy(campo);
    let limpo = texto.trim_matches(|c: char| c == '\0' || c == ' ');
    if limpo.is_empty() {
        return None;
    }
    u64::from_str_radix(limpo, 8).ok()
}

/// Lê os membros do ustar, na ordem em que aparecem.
pub fn membros(arquivo: &[u8]) -> Result<Vec<Membro>, ErroPacote> {
    if arquivo.len() < BLOCO * 2 {
        return Err(ErroPacote::MuitoCurto);
    }
    let mut saida = Vec::new();
    let mut pos = 0usize;

    while pos + BLOCO <= arquivo.len() {
        let bloco = &arquivo[pos..pos + BLOCO];
        // Dois blocos de zero = fim do arquivo. O ustar padroniza assim.
        if bloco.iter().all(|b| *b == 0) {
            break;
        }
        // O "checksum" do ustar: soma dos bytes com o campo tratado como
        // espaços. Conferir pega lixo que por acaso tem NUL no lugar certo.
        let soma_esperada = campo_octal(bloco, 148, 8).ok_or_else(|| ErroPacote::CabecalhoInvalido {
            em: pos,
            motivo: "checksum ilegível".into(),
        })?;
        let soma: u64 = bloco
            .iter()
            .enumerate()
            .map(|(i, b)| if (148..156).contains(&i) { 32 } else { *b as u64 })
            .sum();
        if soma != soma_esperada {
            return Err(ErroPacote::CabecalhoInvalido {
                em: pos,
                motivo: format!("checksum {soma_esperada} != {soma}"),
            });
        }

        let nome_bruto = &bloco[0..NOME_MAX];
        let fim_nome = nome_bruto.iter().position(|b| *b == 0).unwrap_or(NOME_MAX);
        let caminho = String::from_utf8_lossy(&nome_bruto[..fim_nome]).to_string();
        let tamanho = campo_octal(bloco, 124, 12).unwrap_or(0) as usize;
        let modo = campo_octal(bloco, 100, 8).unwrap_or(0o644) as u32;

        let inicio = pos + BLOCO;
        if inicio + tamanho > arquivo.len() {
            return Err(ErroPacote::CabecalhoInvalido {
                em: pos,
                motivo: format!("'{caminho}' declara {tamanho} bytes e o arquivo acaba antes"),
            });
        }
        saida.push(Membro { caminho, tamanho, modo, inicio });

        // Cada membro ocupa o conteúdo arredondado para cima em blocos de 512.
        let ocupado = tamanho.div_ceil(BLOCO) * BLOCO;
        pos = inicio + ocupado;
    }

    if saida.is_empty() {
        return Err(ErroPacote::MuitoCurto);
    }
    Ok(saida)
}

pub fn sha256_hex(arquivo: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(arquivo);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Um pacote que passou por todas as conferências.
///
/// Existir como tipo é o ponto: quem recebe um `PacoteVerificado` não precisa
/// lembrar de checar nada, e o compilador impede usar bytes não verificados como
/// se fossem verificados.
#[derive(Debug, Clone)]
pub struct PacoteVerificado {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub slug: String,
    pub titulo: String,
    pub versao: String,
    pub entrypoint: String,
}

impl PacoteVerificado {
    /// Confere o arquivo contra o que o catálogo dizia, e contra a assinatura.
    ///
    /// A ordem é hash → assinatura → conteúdo. O hash é barato e pega download
    /// truncado; a assinatura é o que vale; o conteúdo só faz sentido depois.
    pub fn conferir(
        bytes: Vec<u8>,
        sha256_esperado: &str,
        assinatura_em_base64: &str,
        chave_do_pacote: &ChavePublica,
    ) -> Result<Self, ErroPacote> {
        let sha256 = sha256_hex(&bytes);
        if !sha256_esperado.is_empty() && !sha256.eq_ignore_ascii_case(sha256_esperado) {
            return Err(ErroPacote::HashNaoConfere {
                esperado: sha256_esperado.to_string(),
                obtido: sha256,
            });
        }

        let assinatura = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            assinatura_em_base64.trim(),
        )
        .map_err(|e| ErroPacote::AssinaturaComTamanhoErrado(e.to_string().len()))?;
        if assinatura.len() != crate::chave::TAMANHO_ASSINATURA {
            return Err(ErroPacote::AssinaturaComTamanhoErrado(assinatura.len()));
        }
        if !chave_do_pacote.verifica(&bytes, &assinatura) {
            return Err(ErroPacote::AssinaturaInvalida);
        }

        let lista = membros(&bytes)?;
        // O contrato com o extrator do aparelho. Um pacote fora de ordem
        // instala pela metade lá e não dá erro aqui — por isso é checado aqui.
        if lista[0].caminho != "metadata.json" {
            return Err(ErroPacote::OrdemInvalida {
                primeiro: lista[0].caminho.clone(),
            });
        }
        let meta = lista
            .iter()
            .find(|m| m.caminho == "metadata.json")
            .ok_or(ErroPacote::SemMetadata)?;
        let texto = std::str::from_utf8(&bytes[meta.inicio..meta.inicio + meta.tamanho])
            .map_err(|e| ErroPacote::MetadataInvalido(e.to_string()))?;
        let j: serde_json::Value =
            serde_json::from_str(texto).map_err(|e| ErroPacote::MetadataInvalido(e.to_string()))?;

        let texto_de = |campo: &str| -> Result<String, ErroPacote> {
            j.get(campo)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| ErroPacote::MetadataInvalido(format!("sem '{campo}'")))
        };

        let entrypoint = texto_de("entrypoint")?;
        if !entrypoint.starts_with("bin/") || entrypoint.contains("..") {
            return Err(ErroPacote::MetadataInvalido(format!(
                "entrypoint fora de bin/ ou com '..': {entrypoint}"
            )));
        }
        // O entrypoint declarado tem de existir: é o erro que aparece no
        // aparelho como "não abre" e não aponta a causa.
        if !lista.iter().any(|m| m.caminho == entrypoint) {
            return Err(ErroPacote::MetadataInvalido(format!(
                "entrypoint '{entrypoint}' não está no pacote"
            )));
        }

        Ok(Self {
            slug: texto_de("slug").unwrap_or_else(|_| {
                j.get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("app")
                    .to_lowercase()
                    .replace(' ', "-")
            }),
            titulo: texto_de("title").unwrap_or_else(|_| "app".into()),
            versao: texto_de("version").unwrap_or_else(|_| "0.0.0".into()),
            entrypoint,
            sha256,
            bytes,
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::chave::ChavePublica;
    use ed25519_dalek::{Signer, SigningKey};

    /// Escreve um ustar mínimo, com checksum correto.
    fn tar(membros: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut saida = Vec::new();
        for (nome, conteudo, modo) in membros {
            let mut bloco = [0u8; BLOCO];
            bloco[..nome.len()].copy_from_slice(nome.as_bytes());
            bloco[100..108].copy_from_slice(format!("{modo:07o}\0").as_bytes());
            bloco[108..116].copy_from_slice(b"0000000\0");
            bloco[116..124].copy_from_slice(b"0000000\0");
            bloco[124..136].copy_from_slice(format!("{:011o}\0", conteudo.len()).as_bytes());
            bloco[136..148].copy_from_slice(b"00000000000\0");
            // O checksum é calculado com o próprio campo em branco (espaços).
            bloco[148..156].copy_from_slice(b"        ");
            let soma: u64 = bloco.iter().map(|b| *b as u64).sum();
            bloco[148..156].copy_from_slice(format!("{soma:06o}\0 ").as_bytes());

            saida.extend_from_slice(&bloco);
            saida.extend_from_slice(conteudo);
            let resto = conteudo.len().div_ceil(BLOCO) * BLOCO - conteudo.len();
            saida.extend(std::iter::repeat_n(0u8, resto));
        }
        saida.extend(std::iter::repeat_n(0u8, BLOCO * 2));
        saida
    }

    fn metadata(slug: &str) -> Vec<u8> {
        format!(
            r#"{{"title":"Jogo","slug":"{slug}","version":"1.0.0","entrypoint":"bin/{slug}","arch":"aarch64","runtime":"native","developer":"Giochi"}}"#
        )
        .into_bytes()
    }

    fn pacote() -> Vec<u8> {
        let m = metadata("pacman");
        tar(&[
            ("metadata.json", &m, 0o644),
            ("capa.png", b"\x89PNG", 0o644),
            ("bin/pacman", b"#!/bin/sh\necho oi\n", 0o755),
        ])
    }

    fn chave() -> SigningKey {
        SigningKey::from_bytes(&[21u8; 32])
    }
    fn publica_de(k: &SigningKey) -> ChavePublica {
        ChavePublica::de_bytes(&k.verifying_key().to_bytes()).unwrap()
    }
    fn assinar(bytes: &[u8]) -> String {
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            chave().sign(bytes).to_bytes(),
        )
    }

    #[test]
    fn pacote_legitimo_e_aceito_e_traz_o_metadata() {
        let b = pacote();
        let p = PacoteVerificado::conferir(b.clone(), &sha256_hex(&b), &assinar(&b), &publica_de(&chave()))
            .unwrap();
        assert_eq!(p.slug, "pacman");
        assert_eq!(p.versao, "1.0.0");
        assert_eq!(p.entrypoint, "bin/pacman");
    }

    #[test]
    fn um_byte_a_mais_invalida_hash_e_assinatura() {
        let mut b = pacote();
        b.push(0);
        let r = PacoteVerificado::conferir(b.clone(), &sha256_hex(&pacote()), &assinar(&pacote()), &publica_de(&chave()));
        assert!(matches!(r, Err(ErroPacote::HashNaoConfere { .. })), "{r:?}");
    }

    #[test]
    fn hash_certo_com_assinatura_de_outro_pacote_e_recusado() {
        // O caso que o hash não pega: pacote trocado inteiro, com o manifesto
        // do original (que o atacante que escreve no cartão pode reescrever).
        let b = pacote();
        let outro = tar(&[("metadata.json", &metadata("outro"), 0o644)]);
        let r = PacoteVerificado::conferir(
            b.clone(),
            &sha256_hex(&b),
            &assinar(&outro),
            &publica_de(&chave()),
        );
        assert_eq!(r.err(), Some(ErroPacote::AssinaturaInvalida));
    }

    #[test]
    fn assinatura_em_base64_duplo_tem_mensagem_propria() {
        let b = pacote();
        let duplo = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            assinar(&b).as_bytes(),
        );
        let r = PacoteVerificado::conferir(b.clone(), &sha256_hex(&b), &duplo, &publica_de(&chave()));
        assert!(matches!(r, Err(ErroPacote::AssinaturaComTamanhoErrado(_))), "{r:?}");
    }

    #[test]
    fn pacote_que_nao_comeca_por_metadata_e_recusado() {
        // Contrato do extrator em streaming no aparelho.
        let m = metadata("x");
        let b = tar(&[
            ("capa.png", b"\x89PNG", 0o644),
            ("metadata.json", &m, 0o644),
            ("bin/x", b"x", 0o755),
        ]);
        let r = PacoteVerificado::conferir(b.clone(), &sha256_hex(&b), &assinar(&b), &publica_de(&chave()));
        assert!(matches!(r, Err(ErroPacote::OrdemInvalida { .. })), "{r:?}");
    }

    #[test]
    fn entrypoint_declarado_e_ausente_e_recusado() {
        let m = metadata("fantasma");
        let b = tar(&[("metadata.json", &m, 0o644)]);
        let r = PacoteVerificado::conferir(b.clone(), &sha256_hex(&b), &assinar(&b), &publica_de(&chave()));
        assert!(matches!(r, Err(ErroPacote::MetadataInvalido(_))), "{r:?}");
    }

    #[test]
    fn entrypoint_com_dois_pontos_pontos_e_recusado() {
        let m = br#"{"title":"X","slug":"x","version":"1.0.0","entrypoint":"bin/../../etc/passwd"}"#.to_vec();
        let b = tar(&[("metadata.json", &m, 0o644)]);
        let r = PacoteVerificado::conferir(b.clone(), &sha256_hex(&b), &assinar(&b), &publica_de(&chave()));
        assert!(matches!(r, Err(ErroPacote::MetadataInvalido(_))), "{r:?}");
    }

    #[test]
    fn arquivo_truncado_nao_e_lido_como_bom() {
        let b = pacote();
        let cortado = b[..300].to_vec();
        assert!(matches!(membros(&cortado), Err(ErroPacote::MuitoCurto)));
    }

    #[test]
    fn checksum_errado_no_cabecalho_e_pego() {
        let mut b = pacote();
        b[508] = b'X'; // dentro do bloco de padding, depois do checksum
        b[0] = b'Z'; // nome alterado invalida o checksum do primeiro bloco
        let r = membros(&b);
        assert!(matches!(r, Err(ErroPacote::CabecalhoInvalido { .. })), "{r:?}");
    }

    #[test]
    fn sha256_do_pacote_e_estavel() {
        let b = pacote();
        assert_eq!(sha256_hex(&b), sha256_hex(&b.clone()));
        assert_eq!(sha256_hex(&b).len(), 64);
    }
}
