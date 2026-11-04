//! Leitura do catálogo de apps que o sistema publica.
//!
//! ## O que isto resolve
//!
//! Até aqui o launcher desenhava `demo_games()` do próprio binário: quatro
//! títulos escritos no código. Qualquer app instalado era invisível, e o botão
//! "Jogar" não tinha para onde apontar — nem havia o que apontar.
//!
//! O contrato está em `docs/integracao.md` §2.1. Este módulo é o lado do app.
//!
//! ## Regra de segurança que vale a pena manter
//!
//! `dir` e `entrypoint` vêm de um arquivo que o sistema escreve, não de código
//! nosso. Um manifesto corrompido não pode virar execução de binário
//! arbitrário, então o caminho é validado aqui: `entrypoint` precisa começar em
//! `bin/` e nenhum dos dois pode conter `..` nem ser relativo. Recusar é melhor
//! que sanear — sanear esconde que o arquivo está errado.

use std::path::Path;

use giochi_launcher_core::Game;

/// Caminho publicado pelo `Giochi System`.
pub const CAMINHO: &str = "/opt/giochi/apps/index.json";

/// Variável que sobrepõe o caminho.
///
/// Existe pelo mesmo motivo que `GIOCHI_ROTATE`: no PC não há
/// `/opt/giochi/apps/`, e testar a leitura exigiria escrever em `/opt` com
/// root ou então não testar. Um catálogo de mentira num diretório temporário
/// exercita o mesmo código.
pub const VAR_CAMINHO: &str = "GIOCHI_CATALOGO";

/// Caminho efetivo do catálogo.
pub fn caminho() -> std::path::PathBuf {
    match std::env::var(VAR_CAMINHO) {
        Ok(v) if !v.is_empty() => std::path::PathBuf::from(v),
        _ => std::path::PathBuf::from(CAMINHO),
    }
}

/// Um app do catálogo, com o que é preciso para executá-lo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alvo {
    /// Slug estável (`launcher`, `volley`). É o `GIOCHI_APP_ID` do processo.
    pub slug: String,
    pub dir: String,
    pub entrypoint: String,
}

impl Alvo {
    /// Caminho absoluto do executável.
    pub fn executavel(&self) -> String {
        format!("{}/{}", self.dir.trim_end_matches('/'), self.entrypoint)
    }
}

/// O que o catálogo entrega ao launcher.
#[derive(Clone, Debug, Default)]
pub struct Catalogo {
    pub games: Vec<Game>,
    /// Alinhado por posição com `games`: `alvos[i]` descreve `games[i]`.
    ///
    /// `Game.id` é essa posição. O id do domínio é o *slug*, que não cabe em
    /// `u32`; a posição é só a chave interna de cor e favorito — nunca aparece
    /// na tela (ver "índice interno não é dado do jogo" no `AGENTS.md`).
    pub alvos: Vec<Alvo>,
}

impl Catalogo {
    /// Lê o catálogo. Ausente ou inválido devolve `Err` — quem chama decide o
    /// fallback, e o padrão é o catálogo de demonstração.
    pub fn carregar(caminho: &Path) -> Result<Self, String> {
        let texto = std::fs::read_to_string(caminho)
            .map_err(|e| format!("{}: {e}", caminho.display()))?;
        Self::do_texto(&texto)
    }

    /// Mesma leitura, a partir do texto. Separado para poder testar sem disco.
    pub fn do_texto(texto: &str) -> Result<Self, String> {
        let raiz: serde_json::Value =
            serde_json::from_str(texto).map_err(|e| format!("JSON inválido: {e}"))?;

        let lista = raiz
            .get("apps")
            .and_then(|v| v.as_array())
            .ok_or("catálogo sem a lista \"apps\"")?;

        let mut catalogo = Self::default();
        for entrada in lista {
            // Uma entrada ruim não derruba o catálogo inteiro: o aparelho
            // continua utilizável com os apps que estão certos. O aviso fica no
            // log, que é onde se procura quando um app "não aparece".
            let Some(alvo) = Self::ler_alvo(entrada) else {
                let slug = entrada
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<sem id>");
                eprintln!("catálogo: entrada ignorada ({slug})");
                continue;
            };

            let id = catalogo.games.len() as u32;
            let titulo = entrada
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&alvo.slug)
                .to_string();
            let categoria = entrada
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("Aplicativo")
                .to_string();
            let ano = entrada
                .get("year")
                .and_then(|v| v.as_u64())
                .filter(|a| (1900..=2199).contains(a))
                .unwrap_or(0) as u16;

            catalogo.games.push(Game {
                id,
                title: titulo,
                category: categoria,
                year: ano,
                hours: 0.0,
                last_played: "nunca".to_string(),
                favorite: false,
            });
            catalogo.alvos.push(alvo);
        }

        Ok(catalogo)
    }

    fn ler_alvo(entrada: &serde_json::Value) -> Option<Alvo> {
        let slug = entrada.get("id")?.as_str()?.to_string();
        let dir = entrada.get("dir")?.as_str()?.to_string();
        let entrypoint = entrada.get("entrypoint")?.as_str()?.to_string();

        // `..` em qualquer posição e caminho não-absoluto são recusados. O
        // `entrypoint` ainda precisa ficar dentro de `bin/`, como no
        // `metadata.json`.
        if dir.contains("..") || !dir.starts_with('/') {
            return None;
        }
        if entrypoint.contains("..") || entrypoint.starts_with('/') {
            return None;
        }
        if !entrypoint.starts_with("bin/") || entrypoint.len() <= 4 {
            return None;
        }

        Some(Alvo {
            slug,
            dir,
            entrypoint,
        })
    }

    /// Alvo para executar, a partir do `Game.id`.
    pub fn alvo(&self, id: u32) -> Option<&Alvo> {
        self.alvos.get(id as usize)
    }

    pub fn vazio(&self) -> bool {
        self.games.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXEMPLO: &str = r#"{
        "schemaVersion": 1,
        "apps": [
            { "id": "volley", "version": "0.1.0", "entrypoint": "bin/giochi-volley",
              "dir": "/opt/giochi/apps/volley", "title": "Volley",
              "developer": "Giochi", "year": 2026, "category": "Esporte" }
        ]
    }"#;

    #[test]
    fn le_um_app_completo() {
        let c = Catalogo::do_texto(EXEMPLO).expect("deveria ler");
        assert_eq!(c.games.len(), 1);
        assert_eq!(c.games[0].title, "Volley");
        assert_eq!(c.games[0].year, 2026);
        assert_eq!(c.games[0].category, "Esporte");
        assert_eq!(
            c.alvo(0).map(|a| a.executavel()),
            Some("/opt/giochi/apps/volley/bin/giochi-volley".to_string())
        );
    }

    /// O ano do catálogo é opcional. Ausente vira `0` — "desconhecido" — e a UI
    /// mostra `—`. O que não pode é virar o índice do app na tela.
    #[test]
    fn ano_ausente_fica_desconhecido() {
        let texto = EXEMPLO.replace(r#", "year": 2026"#, "");
        let c = Catalogo::do_texto(&texto).expect("deveria ler");
        assert_eq!(c.games[0].year, 0);
    }

    #[test]
    fn ano_fora_de_faixa_e_ignorado() {
        for ruim in ["0", "42", "1899", "2200"] {
            let texto = EXEMPLO.replace("2026", ruim);
            let c = Catalogo::do_texto(&texto).expect("deveria ler");
            assert_eq!(c.games[0].year, 0, "ano {ruim} deveria ser descartado");
        }
    }

    /// Entrada com caminho que escapa é descartada, não "corrigida".
    #[test]
    fn recusa_caminho_que_escapa() {
        for ruim in [
            r#""dir": "/opt/giochi/apps/../../../etc""#,
            r#""dir": "opt/giochi/apps/volley""#,
            r#""entrypoint": "bin/../../etc/passwd""#,
            r#""entrypoint": "/bin/sh""#,
            r#""entrypoint": "sh""#,
        ] {
            let mut texto = EXEMPLO.to_string();
            // Troca o campo correspondente pelo valor ruim.
            if ruim.contains("\"dir\"") {
                texto = texto.replace(
                    r#""dir": "/opt/giochi/apps/volley""#,
                    ruim,
                );
            } else {
                texto = texto.replace(
                    r#""entrypoint": "bin/giochi-volley""#,
                    ruim,
                );
            }
            let c = Catalogo::do_texto(&texto).expect("JSON continua válido");
            assert!(c.vazio(), "{ruim} deveria ser recusado");
        }
    }

    /// Catálogo vazio é válido: aparelho sem app nenhum não pode quebrar.
    #[test]
    fn catalogo_vazio_e_valido() {
        let c = Catalogo::do_texto(r#"{"schemaVersion":1,"apps":[]}"#).expect("deveria ler");
        assert!(c.vazio());
    }

    #[test]
    fn json_invalido_e_erro() {
        assert!(Catalogo::do_texto("não é json").is_err());
        assert!(Catalogo::do_texto("{}").is_err(), "sem a lista apps");
    }
}
