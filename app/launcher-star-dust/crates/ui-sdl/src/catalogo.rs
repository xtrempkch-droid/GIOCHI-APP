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

    /// Descarta apps cujo executável não está no disco.
    ///
    /// ## Por que isto existe
    ///
    /// Com jogos em cartão removível, o `dir` do catálogo pode apontar para um
    /// caminho que **não existe naquele boot** — o cartão foi tirado, ou montou
    /// em outro lugar. Sem esta poda o cartão aparece na Home e não abre nada:
    /// o pior modo de falha já registrado neste projeto ("cartão que abre nada é
    /// pior que cartão ausente").
    ///
    /// A poda é feita em **tempo de execução**, não na geração do catálogo,
    /// porque o PC que gera o índice não sabe se o cartão estará lá. Quem sabe é
    /// o aparelho, no boot.
    ///
    /// `manter` recebe o motivo para virar uma linha de log — uma poda silenciosa
    /// esconderia um cartão mal montado.
    pub fn podar_sem_binario(mut self, mut manter: impl FnMut(&str, &str)) -> Self {
        let antes = self.games.len();

        let mut games = Vec::with_capacity(antes);
        let mut alvos = Vec::with_capacity(antes);
        for (game, alvo) in self.games.into_iter().zip(self.alvos.into_iter()) {
            let executavel = alvo.executavel();
            if std::path::Path::new(&executavel).is_file() {
                games.push(game);
                alvos.push(alvo);
            } else {
                manter(&alvo.slug, &executavel);
            }
        }

        // `Game.id` é a **posição**: renumerar mantém a chave consistente com
        // `alvos`, e essa chave nunca aparece na tela (ver "índice interno não é
        // dado do jogo").
        for (i, g) in games.iter_mut().enumerate() {
            g.id = i as u32;
        }

        self.games = games;
        self.alvos = alvos;
        self
    }
}

/// Acrescenta ou substitui um app no catálogo publicado.
///
/// ## Por que isto existe
///
/// O catálogo (`/opt/giochi/apps/index.json`) era escrito só do PC. Isso amarra
/// a instalação de um jogo ao SSH e a um computador com Node — e o caminho
/// natural num portátil é outro: copiar o `.giochi` para o cartão e instalar no
/// aparelho.
///
/// Com esta função, o próprio aparelho atualiza o índice **sem Node**: o
/// launcher já tem `serde_json`.
///
/// ## O que ela garante
///
/// - `id` é a chave: se existe, é substituído (reinstalar não duplica);
/// - a ordem por título é restaurada, senão a Home mudaria de conteúdo conforme
///   a ordem de instalação;
/// - `license` do índice anterior é herdada quando o `metadata.json` do pacote
///   não a traz — foi a perda que apareceu na primeira instalação;
/// - nada de `unsafe` ou escrita parcial: quem chama grava em arquivo temporário
///   e move, para um corte de energia não deixar JSON truncado.
pub fn adicionar_ao_indice(texto: &str, dir: &str, metadata: &serde_json::Value) -> Result<String, String> {
    let mut raiz: serde_json::Value = serde_json::from_str(texto)
        .map_err(|e| format!("catálogo atual ilegível: {e}"))?;

    let id = metadata
        .get("slug")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // Sem `slug`, o nome da pasta é a identidade — é o que o
            // `aparelho.sh` usa para instalar, então é a mesma chave.
            std::path::Path::new(dir)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .ok_or("não consegui determinar o id do app")?;

    let entrypoint = metadata
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .ok_or("metadata.json sem entrypoint")?;

    if !entrypoint.starts_with("bin/") || entrypoint.contains("..") {
        return Err(format!("entrypoint inválido: {entrypoint}"));
    }

    let lista = raiz
        .get_mut("apps")
        .and_then(|v| v.as_array_mut())
        .ok_or("catálogo sem a lista \"apps\"")?;

    let anterior = lista
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
        .cloned();

    let mut novo = serde_json::json!({
        "id": id,
        "version": metadata.get("version").cloned().unwrap_or(serde_json::json!("0.0.0")),
        "entrypoint": entrypoint,
        "dir": dir,
        "title": metadata.get("title").cloned().unwrap_or(serde_json::json!(id)),
        "developer": metadata.get("developer").cloned().unwrap_or(serde_json::json!("desconhecido")),
    });

    // Campos opcionais que o catálogo repassa à UI.
    for (campo, destino) in [("releaseYear", "year"), ("category", "category")] {
        if let Some(v) = metadata.get(campo) {
            if !v.is_null() {
                novo[destino] = v.clone();
            }
        }
    }

    // `license` só existe no índice (vem do `apps.json`), nunca no pacote.
    // Trocar a entrada inteira a apagava: o pacman apareceu sem licença.
    if let Some(lic) = anterior.as_ref().and_then(|a| a.get("license")) {
        novo["license"] = lic.clone();
    }

    lista.retain(|a| a.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));
    lista.push(novo);

    // Ordem por título, com `sort_by` estável: duas execuções dão o mesmo
    // arquivo, então o catálogo não muda sozinho entre boots.
    lista.sort_by(|a, b| {
        let ta = a.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let tb = b.get("title").and_then(|v| v.as_str()).unwrap_or("");
        ta.cmp(tb)
    });

    serde_json::to_string_pretty(&raiz)
        .map(|s| s + "\n")
        .map_err(|e| format!("não consegui serializar: {e}"))
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

    fn meta(id: &str) -> serde_json::Value {
        serde_json::json!({
            "slug": id, "title": "Jogo", "developer": "Giochi", "version": "1.0.0",
            "arch": "aarch64", "runtime": "native", "entrypoint": "bin/app"
        })
    }

    fn com_caminhos(exec: &[&str]) -> String {
        let apps: Vec<String> = exec
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    r#"{{ "id": "a{i}", "version": "1.0.0", "entrypoint": "{e}",
                        "dir": "/nao/existe", "title": "T{i}", "developer": "g" }}"#
                )
            })
            .collect();
        format!(r#"{{"schemaVersion":1,"apps":[{}]}}"#, apps.join(","))
    }

    /// Jogo em cartão removível: cartão fora = app fora da Home.
    ///
    /// Sem esta poda o cartão aparece e não abre — o pior modo de falha deste
    /// projeto. O PC que gera o índice não sabe se o cartão estará lá; quem sabe
    /// é o aparelho, no boot.
    #[test]
    fn poda_app_cujo_binario_nao_esta_no_disco() {
        let texto = com_caminhos(&["bin/nao-existe-1", "bin/nao-existe-2"]);
        let c = Catalogo::do_texto(&texto).unwrap();
        assert_eq!(c.games.len(), 2, "o catálogo lê os dois");

        let mut podados = Vec::new();
        let c = c.podar_sem_binario(|slug, caminho| podados.push((slug.to_string(), caminho.to_string())));

        assert!(c.vazio(), "nenhum binário existe, então nenhum cartão");
        assert_eq!(podados.len(), 2, "a poda precisa dizer o que tirou");
        assert_eq!(podados[0].0, "a0");
    }

    /// Binário que existe é preservado, e o `id` continua sendo a posição.
    #[test]
    fn poda_preserva_o_que_existe_e_renumera() {
        // `entrypoint` precisa começar em `bin/` e não pode ter `..` — a
        // checagem de segurança do leitor recusa, e é ela que impede um
        // manifesto corrompido virar execução de binário arbitrário. Então o
        // caminho existente é montado com `dir` + `bin/...`, não com `..`.
        let texto = com_caminhos(&["bin/nao-existe", "bin/env"])
            .replace(r#""dir": "/nao/existe""#, r#""dir": "/usr""#);
        let c = Catalogo::do_texto(&texto).unwrap();

        let c = c.podar_sem_binario(|_, _| {});
        assert_eq!(c.games.len(), 1, "só o que existe fica");
        assert_eq!(c.games[0].id, 0, "o id é a posição, e renumerar mantém consistente");
        assert_eq!(c.alvos.len(), 1, "games e alvos têm de ficar alinhados");
    }

    #[test]
    fn poda_sem_nada_a_podar_nao_muda_nada() {
        let texto = com_caminhos(&["bin/env"]).replace(r#""dir": "/nao/existe""#, r#""dir": "/usr""#);
        let c = Catalogo::do_texto(&texto).unwrap();
        let antes = c.games.len();
        let c = c.podar_sem_binario(|_, _| panic!("não devia podar nada"));
        assert_eq!(c.games.len(), antes);
    }

    #[test]
    fn adiciona_app_novo_ao_indice() {
        let novo = adicionar_ao_indice(EXEMPLO, "/user/apps/pacman", &meta("pacman")).unwrap();
        let c = Catalogo::do_texto(&novo).unwrap();
        assert_eq!(c.games.len(), 2, "o volley continua lá");
        assert!(c.games.iter().any(|g| g.title == "Jogo"));
    }

    /// Reinstalar não pode duplicar o cartão.
    #[test]
    fn substitui_em_vez_de_duplicar() {
        let meta1 = serde_json::json!({
            "slug": "volley", "title": "Volley", "developer": "Giochi", "version": "9.9.9",
            "arch": "aarch64", "runtime": "native", "entrypoint": "bin/giochi-volley"
        });
        let texto = adicionar_ao_indice(EXEMPLO, "/user/apps/volley", &meta1).unwrap();
        let c = Catalogo::do_texto(&texto).unwrap();
        assert_eq!(c.games.len(), 1, "mesmo id não duplica");
        assert_eq!(c.games[0].title, "Volley");
    }

    /// A licença vive só no índice; trocar a entrada a apagava.
    #[test]
    fn preserva_licenca_do_indice() {
        let comLicenca = EXEMPLO.replace(r#""developer": "Giochi","#, r#""developer": "Giochi", "license": "Proprietary","#);
        let meta1 = serde_json::json!({
            "slug": "volley", "title": "Volley", "developer": "Giochi", "version": "0.2.0",
            "arch": "aarch64", "runtime": "native", "entrypoint": "bin/giochi-volley"
        });
        let texto = adicionar_ao_indice(&comLicenca, "/user/apps/volley", &meta1).unwrap();
        assert!(texto.contains("Proprietary"), "licença desapareceu: {texto}");
    }

    #[test]
    fn ordem_por_titulo_nao_depende_da_instalacao() {
        let a = adicionar_ao_indice(EXEMPLO, "/user/apps/aaa", &{
            let mut m = meta("aaa");
            m["title"] = serde_json::json!("AAA");
            m
        })
        .unwrap();
        let b = adicionar_ao_indice(EXEMPLO, "/user/apps/zzz", &{
            let mut m = meta("zzz");
            m["title"] = serde_json::json!("ZZZ");
            m
        })
        .unwrap();

        let titulos = |t: &str| -> Vec<String> {
            Catalogo::do_texto(t).unwrap().games.iter().map(|g| g.title.clone()).collect()
        };
        assert_eq!(titulos(&a), vec!["AAA", "Volley"]);
        assert_eq!(titulos(&b), vec!["Volley", "ZZZ"]);
    }

    #[test]
    fn recusa_entrypoint_que_sai_do_pacote() {
        let mut m = meta("mal");
        m["entrypoint"] = serde_json::json!("bin/../../etc/passwd");
        assert!(adicionar_ao_indice(EXEMPLO, "/user/apps/mal", &m).is_err());
    }

    #[test]
    fn indice_corrompido_da_erro_em_vez_de_zerar() {
        // Sobrescrever um catálogo ilegível com uma lista de um item faria o
        // usuário perder os outros apps — e sem aviso.
        assert!(adicionar_ao_indice("{lixo", "/user/apps/x", &meta("x")).is_err());
    }

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
