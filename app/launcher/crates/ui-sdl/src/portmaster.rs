//! Ports do PortMaster viram cartões na aba Apps.
//!
//! ## O problema que isto resolve
//!
//! O PortMaster instala em `/user/ports` — `<nome>.sh` ao lado de `<nome>/`, ou
//! `<nome>/<nome>.sh` — e o launcher varre `/user/apps`. Um port instalado pela
//! GUI do PortMaster ficaria **invisível**: existe no disco e em nenhum cartão,
//! que é exatamente o "cartão que não existe" que a detecção veio resolver.
//!
//! ## Por que materializar no launcher, e não num script no PC
//!
//! Quem instala um port é a GUI do PortMaster, **no aparelho**, sem PC por
//! perto. Um gerador rodado por SSH deixaria o port recém-instalado invisível
//! até alguém lembrar de rodá-lo de novo. O launcher é a única peça que está no
//! aparelho, sobe depois da GUI sair (o `giochi-init` o reinicia) e já sabe
//! lançar um app sem argumentos.
//!
//! ## O que foi medido no R35S em 26/11/2026
//!
//! | Experimento | Resultado |
//! | --- | --- |
//! | cartão em `/user/apps/<slug>/` com `"entrypoint": "bin/<slug>"` | aparece: `4 jogo(s) e 2 app(s)` |
//! | o mesmo com `"entrypoint": "<slug>"` (na raiz) | **recusado**: `entrypoint inválido ... (precisa começar em bin/)` |
//! | `giochi-ports --rodar <nome>` | acha os dois layouts, sem diferenciar caixa |
//! | marcador de DRM do `giochi-ports` | **não** é criado quando ninguém tem o card — é o caso do filho do launcher, que já destruiu o `Painter` |
//!
//! A segunda linha é a que importa para quem ler o pedido original: o layout
//! sugerido lá (executável na raiz do app, `entrypoint: "<slug>"`) **não**
//! funciona neste launcher. O `entrypoint` precisa começar em `bin/`.
//!
//! ## O que este módulo **não** faz
//!
//! - Não assina nada. Port é código de terceiro sem assinatura; o que o sistema
//!   garante é o sandbox do `giochi-ports` (`bwrap`, rootfs somente-leitura).
//! - Não abre a GUI do PortMaster. Instalar port é a loja (host) ou a própria
//!   GUI; aqui só se transforma port instalado em cartão.
//! - Não toca em diretório de `/user/apps` que não tenha a marca [`MARCA`].

use std::fs;
use std::path::{Path, PathBuf};

/// Onde o PortMaster instala.
pub const RAIZ_PORTS: &str = "/user/ports";
/// Onde o launcher procura apps do usuário.
pub const RAIZ_APPS: &str = "/user/apps";

/// Raiz de ports em uso, com sobreposição por ambiente.
///
/// `GIOCHI_PORTS` existe pela mesma razão do `GIOCHI_RAIZES` do catálogo: testar
/// a sincronização no PC, onde `/user/ports` não existe, sem precisar de
/// aparelho. No aparelho a variável não é definida e vale a constante.
pub fn raiz_ports() -> PathBuf {
    std::env::var_os("GIOCHI_PORTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(RAIZ_PORTS))
}

/// Raiz de apps em uso, com sobreposição por ambiente (`GIOCHI_APPS`).
pub fn raiz_apps() -> PathBuf {
    std::env::var_os("GIOCHI_APPS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(RAIZ_APPS))
}

/// Arquivo que marca um diretório como gerado por este módulo.
///
/// Guarda o **nome do port**, não o slug: é o que reencontra o cartão quando o
/// slug mudar de forma (renomeação do `.sh`). Só diretório com esta marca pode
/// ser apagado pela poda — o resto de `/user/apps` não é nosso.
const MARCA: &str = ".portmaster-port";

/// Prefixo do slug. `portmaster-` nunca casa com `ehTema` (que poda `launcher*`),
/// então um port jamais abre um segundo launcher.
const PREFIXO: &str = "portmaster-";

/// Um port instalado, como o launcher precisa vê-lo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub nome: String,
    pub titulo: String,
    pub porter: Option<String>,
}

/// O que a sincronização fez, para o log dizer — e para o teste conferir.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Relatorio {
    /// Raiz efetivamente varrida (a constante, ou `GIOCHI_PORTS`).
    pub raiz: String,
    /// `false` quando `/user/ports` não existe (o caso do PC). Nada é criado.
    pub existe: bool,
    pub ports: usize,
    pub criados: usize,
    pub atualizados: usize,
    pub iguais: usize,
    pub removidos: usize,
    pub avisos: Vec<String>,
}

impl Relatorio {
    /// Uma linha para o log, ou `None` quando não há por que falar.
    ///
    /// Silêncio no PC é de propósito: `ports: /user/ports não existe` em toda
    /// abertura seria ruído que ensina a ignorar o log.
    pub fn resumo(&self) -> Option<String> {
        if !self.existe {
            return None;
        }
        Some(format!(
            "ports: {} port(s) em {} — {} cartão(ões) ({} novo(s), {} atualizado(s), {} igual(es), {} removido(s))",
            self.ports,
            self.raiz,
            self.ports,
            self.criados,
            self.atualizados,
            self.iguais,
            self.removidos
        ))
    }
}

/// Lista os ports instalados, resolvendo os dois layouts do PortMaster.
///
/// ```text
/// /user/ports/<nome>.sh          + /user/ports/<nome>/     (instalado pela GUI)
/// /user/ports/<dir>/<nome>.sh                              (feito à mão)
/// ```
///
/// O nome é o **do arquivo `.sh`**, sem extensão, porque é assim que o
/// `giochi-ports --rodar` compara (base do arquivo ou nome da pasta, sem caixa).
/// A pasta `PortMaster/` fica de fora: ela é o instalador, não um port.
pub fn listar(raiz: &Path) -> Vec<Port> {
    let mut candidatos: Vec<(String, PathBuf)> = Vec::new();
    let Ok(entradas) = fs::read_dir(raiz) else {
        return Vec::new();
    };
    let mut itens: Vec<PathBuf> = entradas.flatten().map(|e| e.path()).collect();
    itens.sort();

    for caminho in itens {
        let Some(nome_arquivo) = caminho.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if nome_arquivo.starts_with('.') {
            continue;
        }
        if caminho.is_dir() {
            // O controlfolder do PortMaster tem `PortMaster.sh` dentro; listá-lo
            // criaria um cartão que abre o instalador como se fosse jogo.
            if nome_arquivo.eq_ignore_ascii_case("PortMaster") {
                continue;
            }
            let Ok(sub) = fs::read_dir(&caminho) else {
                continue;
            };
            let mut scripts: Vec<PathBuf> = sub
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && eh_sh(p))
                .collect();
            scripts.sort();
            for script in scripts {
                if let Some(nome) = nome_do_script(&script) {
                    let dir = script.parent().map(Path::to_path_buf).unwrap_or_default();
                    candidatos.push((nome, dir));
                }
            }
        } else if eh_sh(&caminho) {
            if let Some(nome) = nome_do_script(&caminho) {
                candidatos.push((nome, raiz.to_path_buf()));
            }
        }
    }

    candidatos.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(a.0.cmp(&b.0)));
    let mut vistos: Vec<String> = Vec::new();
    let mut ports = Vec::new();
    for (nome, dir_do_script) in candidatos {
        let chave = nome.to_lowercase();
        if vistos.contains(&chave) {
            continue;
        }
        vistos.push(chave);
        let (titulo, porter) = ler_port_json(raiz, &nome, &dir_do_script);
        ports.push(Port {
            titulo: titulo.unwrap_or_else(|| nome.clone()),
            nome,
            porter,
        });
    }
    ports
}

fn eh_sh(caminho: &Path) -> bool {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sh"))
}

fn nome_do_script(caminho: &Path) -> Option<String> {
    let nome = caminho.file_stem()?.to_str()?.trim();
    if nome.is_empty() {
        None
    } else {
        Some(nome.to_string())
    }
}

/// Título e autor do `port.json` do PortMaster, quando existir.
///
/// O arquivo é onde o PortMaster guarda o nome de exibição (`attr.title`) e quem
/// portou (`attr.porter`). Sem ele, o título é o nome do `.sh` — o que o cartão
/// mostraria de qualquer forma.
fn ler_port_json(raiz: &Path, nome: &str, dir_do_script: &Path) -> (Option<String>, Option<String>) {
    // Layout `<nome>.sh` na raiz: o `port.json` mora na pasta do port, que pode
    // ter outra caixa (`FORM.sh` + `form/`, medido no R35S).
    let dir = if dir_do_script == raiz {
        achar_pasta(raiz, nome).unwrap_or_else(|| raiz.join(nome))
    } else {
        dir_do_script.to_path_buf()
    };
    let Ok(texto) = fs::read_to_string(dir.join("port.json")) else {
        return (None, None);
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&texto) else {
        return (None, None);
    };
    let attr = json.get("attr");
    let titulo = attr
        .and_then(|a| a.get("title"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string);
    let porter = attr
        .and_then(|a| a.get("porter"))
        .and_then(|v| v.as_array())
        .and_then(|lista| lista.first())
        .and_then(|v| v.as_str())
        .map(str::to_string);
    (titulo, porter)
}

/// Procura `<raiz>/<nome>` sem diferenciar caixa, sem varrer o disco inteiro.
fn achar_pasta(raiz: &Path, nome: &str) -> Option<PathBuf> {
    let entradas = fs::read_dir(raiz).ok()?;
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir()
            && caminho
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(nome))
        {
            return Some(caminho);
        }
    }
    None
}

/// Slug estável do cartão: `portmaster-<nome normalizado>`.
///
/// Normaliza para ASCII minúsculo porque o slug é nome de pasta e identidade do
/// cartão. Um nome sem nenhum caractere ASCII (port com título japonês, por
/// exemplo) recebe um sufixo de hash determinístico — FNV-1a, e **não**
/// `DefaultHasher`, que não promete estabilidade entre versões do compilador.
pub fn slug_do_port(nome: &str) -> String {
    let mut slug = String::from(PREFIXO);
    let mut ultimo_hifen = false;
    for c in nome.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            ultimo_hifen = false;
        } else if !ultimo_hifen {
            slug.push('-');
            ultimo_hifen = true;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    // `PREFIXO` tem hífen no fim; o `trim` acima pode deixar só `portmaster`.
    if slug == PREFIXO.trim_end_matches('-') {
        format!("{PREFIXO}{}", hash_curto(nome))
    } else {
        slug
    }
}

fn hash_curto(texto: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in texto.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// Conteúdo do executável do cartão.
///
/// Sem argumentos no `Alvo` do launcher (`Command::new(...).spawn()`), o cartão
/// só pode carregar a escolha do port dentro dele. Aspas duplas com escape, e
/// não simples: um nome com `'` quebraria o script em silêncio.
fn script_do_port(port: &Port) -> String {
    format!(
        "#!/bin/sh\n\
         # Gerado pelo launcher do Giochi — port \"{}\" (PortMaster).\n\
         # Não edite: este arquivo é reescrito quando a lista de ports muda.\n\
         exec giochi-ports --rodar \"{}\"\n",
        port.titulo.replace(['\n', '\r'], " "),
        escapar_aspas(&port.nome)
    )
}

fn escapar_aspas(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '\\' | '"' | '$' | '`' => {
                saida.push('\\');
                saida.push(c);
            }
            _ => saida.push(c),
        }
    }
    saida
}

fn metadata_do_port(port: &Port, slug: &str) -> String {
    let json = serde_json::json!({
        "title": port.titulo,
        "slug": slug,
        "developer": port.porter.clone().unwrap_or_else(|| "PortMaster".into()),
        "version": "1.0.0",
        "arch": "aarch64",
        "runtime": "native",
        "entrypoint": format!("bin/{slug}"),
        "kind": "app",
        "category": "PortMaster",
    });
    // `to_string_pretty` + `\n`: o arquivo é lido por humano no diagnóstico, e
    // sem a quebra final o `cat` cola o shell prompt na última chave.
    let mut texto = serde_json::to_string_pretty(&json).unwrap_or_default();
    texto.push('\n');
    texto
}

/// Sincroniza `/user/ports` → cartões em `/user/apps`.
///
/// Nunca falha a abertura do launcher: o pior caso é um port que não aparece e
/// uma linha `aviso:` no log. Apagar e criar arquivos no `/user` não pode ser
/// motivo para a interface não subir.
pub fn sincronizar(raiz_ports: &Path, raiz_apps: &Path) -> Relatorio {
    let mut rel = Relatorio {
        raiz: raiz_ports.display().to_string(),
        ..Relatorio::default()
    };
    if !raiz_ports.is_dir() {
        return rel;
    }
    rel.existe = true;
    let ports = listar(raiz_ports);
    rel.ports = ports.len();

    let mut usados: Vec<String> = Vec::new();
    let mut vivos: Vec<String> = Vec::new();
    for port in &ports {
        let mut slug = slug_do_port(&port.nome);
        if usados.contains(&slug) {
            // Dois nomes que normalizam para o mesmo slug (`A B` e `A-B`). O
            // segundo ganha sufixo numérico, em ordem determinística.
            let mut n = 2;
            while usados.contains(&format!("{slug}-{n}")) {
                n += 1;
            }
            slug = format!("{slug}-{n}");
        }
        usados.push(slug.clone());
        vivos.push(port.nome.clone());
        sincronizar_cartao(raiz_apps, port, &slug, &mut rel);
    }
    podar(raiz_apps, &vivos, &mut rel);
    rel
}

fn sincronizar_cartao(raiz_apps: &Path, port: &Port, slug: &str, rel: &mut Relatorio) {
    let dir = raiz_apps.join(slug);
    let marca = dir.join(MARCA);
    if dir.exists() && !marca.exists() {
        rel.avisos.push(format!(
            "{} já existe e não veio do PortMaster; pulando o port {}",
            dir.display(),
            port.nome
        ));
        return;
    }
    if let Ok(anterior) = fs::read_to_string(&marca) {
        if anterior.trim() != port.nome {
            rel.avisos.push(format!(
                "{} pertence ao port {} (marca); pulando {}",
                dir.display(),
                anterior.trim(),
                port.nome
            ));
            return;
        }
    }

    let bin = dir.join("bin").join(slug);
    let metadados = dir.join("metadata.json");
    // A criação é decidida **antes** de escrever: depois do `fs::write` o
    // arquivo existe, e `exists()` não distingue mais "criei" de "atualizei".
    let existia = metadados.is_file();
    let mut mudou = false;
    match escrever(&bin, &script_do_port(port)) {
        Ok(m) => mudou |= m,
        Err(e) => {
            rel.avisos.push(format!("não consegui escrever {}: {e}", bin.display()));
            return;
        }
    }
    // O launcher lança o entrypoint direto (`Command::new`): sem `+x` o cartão
    // aparece e não abre. O umask do aparelho é 077 (§13), então o modo é
    // explícito — mesma família do `tar --mode` que mascara permissão.
    if let Err(e) = dar_execucao(&bin) {
        rel.avisos.push(format!("não consegui dar +x em {}: {e}", bin.display()));
    }
    match escrever(&metadados, &metadata_do_port(port, slug)) {
        Ok(m) => mudou |= m,
        Err(e) => {
            rel.avisos
                .push(format!("não consegui escrever {}: {e}", metadados.display()));
            return;
        }
    }
    // A marca vai por último: se algo falhar no meio, sobra um diretório sem
    // marca — que a poda não apaga e o launcher ignora — em vez de um diretório
    // marcado e quebrado.
    match escrever(&marca, &format!("{}\n", port.nome)) {
        Ok(m) => mudou |= m,
        Err(e) => {
            rel.avisos
                .push(format!("não consegui escrever {}: {e}", marca.display()));
            return;
        }
    }
    if mudou {
        if existia {
            rel.atualizados += 1;
        } else {
            rel.criados += 1;
        }
    } else {
        rel.iguais += 1;
    }
}

fn escrever(caminho: &Path, conteudo: &str) -> Result<bool, String> {
    if let Ok(atual) = fs::read_to_string(caminho) {
        if atual == conteudo {
            return Ok(false);
        }
    }
    if let Some(pai) = caminho.parent() {
        fs::create_dir_all(pai).map_err(|e| e.to_string())?;
    }
    fs::write(caminho, conteudo).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(unix)]
fn dar_execucao(caminho: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let atual = fs::metadata(caminho).map_err(|e| e.to_string())?.permissions();
    let modo = atual.mode() & 0o777;
    if modo != 0o755 {
        fs::set_permissions(caminho, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Apaga os cartões gerados cujo port não existe mais.
///
/// Só diretório com [`MARCA`] é candidato: o resto de `/user/apps` é de outros
/// donos. Sem a poda, desinstalar um port deixaria um cartão que abre e falha
/// com `port 'x' nao encontrado` — "cartão que aparece e não abre", o defeito
/// que a detecção existe para evitar.
fn podar(raiz_apps: &Path, vivos: &[String], rel: &mut Relatorio) {
    let Ok(entradas) = fs::read_dir(raiz_apps) else {
        return;
    };
    let mut itens: Vec<PathBuf> = entradas.flatten().map(|e| e.path()).collect();
    itens.sort();
    for dir in itens {
        if !dir.is_dir() {
            continue;
        }
        let Ok(marca) = fs::read_to_string(dir.join(MARCA)) else {
            continue;
        };
        let nome = marca.trim();
        if vivos.iter().any(|v| v == nome) {
            continue;
        }
        match fs::remove_dir_all(&dir) {
            Ok(()) => rel.removidos += 1,
            Err(e) => rel
                .avisos
                .push(format!("não consegui remover {}: {e}", dir.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(nome: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "giochi-ports-{nome}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp");
        dir
    }

    fn plantar_port(raiz_ports: &Path, nome: &str, titulo: &str) {
        let dir = raiz_ports.join(name_lower(nome));
        fs::create_dir_all(&dir).expect("dir do port");
        fs::write(dir.join(format!("{nome}.sh")), "#!/bin/sh\n").expect("sh");
        fs::write(
            dir.join("port.json"),
            format!(r#"{{"attr": {{"title": "{titulo}", "porter": ["quem"]}}}}"#),
        )
        .expect("port.json");
    }

    fn name_lower(nome: &str) -> String {
        nome.to_lowercase()
    }

    #[test]
    fn lista_os_dois_layouts_e_ignora_o_instalador() {
        let raiz = temp("layouts");
        fs::write(raiz.join("FORM.sh"), "#!/bin/sh\n").expect("sh na raiz");
        plantar_port(&raiz, "2048", "2048");
        let stardew = raiz.join("stardewvalley");
        fs::create_dir_all(&stardew).expect("dir");
        fs::write(stardew.join("StardewValley.sh"), "#!/bin/sh\n").expect("sh");
        let pm = raiz.join("PortMaster");
        fs::create_dir_all(&pm).expect("pm");
        fs::write(pm.join("PortMaster.sh"), "#!/bin/sh\n").expect("pm.sh");
        fs::write(raiz.join(".oculto.sh"), "#!/bin/sh\n").expect("oculto");

        let ports = listar(&raiz);
        let nomes: Vec<&str> = ports.iter().map(|p| p.nome.as_str()).collect();

        assert_eq!(nomes, ["2048", "FORM", "StardewValley"]);
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn titulo_vem_do_port_json_mesmo_com_caixa_diferente() {
        let raiz = temp("titulo");
        fs::write(raiz.join("FORM.sh"), "#!/bin/sh\n").expect("sh");
        let form = raiz.join("form");
        fs::create_dir_all(&form).expect("dir");
        fs::write(
            form.join("port.json"),
            r#"{"attr": {"title": "FORM — escultura", "porter": ["deadquiet42"]}}"#,
        )
        .expect("json");

        let ports = listar(&raiz);

        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].titulo, "FORM — escultura");
        assert_eq!(ports[0].porter.as_deref(), Some("deadquiet42"));
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn slug_nao_parece_tema_e_e_estavel() {
        assert_eq!(slug_do_port("Stardew Valley"), "portmaster-stardew-valley");
        assert_eq!(slug_do_port("Stardew Valley"), slug_do_port("Stardew Valley"));
        assert!(!slug_do_port("launcher-neon").starts_with("launcher"));
        // Nome sem ASCII cai no hash determinístico, e não num slug vazio.
        let japones = slug_do_port("日本語");
        assert!(japones.starts_with("portmaster-"));
        assert_eq!(japones, slug_do_port("日本語"));
        assert_ne!(japones, slug_do_port("中文"));
    }

    #[test]
    fn sincronizar_cria_cartao_que_o_catalogo_aceita() {
        let ports = temp("cria-ports");
        let apps = ports.join("apps");
        plantar_port(&ports, "2048", "2048");

        let rel = sincronizar(&ports, &apps);

        assert_eq!(rel.ports, 1);
        assert_eq!(rel.criados, 1, "{rel:?}");
        // A forma que o detector do launcher exige: `entrypoint` começando em
        // `bin/`, e o arquivo existindo com +x.
        let dir = apps.join("portmaster-2048");
        let meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("metadata.json")).expect("metadata"))
                .expect("json");
        assert_eq!(meta["kind"], "app");
        assert_eq!(meta["entrypoint"], "bin/portmaster-2048");
        assert_eq!(meta["title"], "2048");
        let bin = dir.join("bin/portmaster-2048");
        let texto = fs::read_to_string(&bin).expect("script");
        assert!(texto.starts_with("#!/bin/sh\n"), "{texto}");
        assert!(texto.contains("--rodar \"2048\""), "{texto}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = fs::metadata(&bin).expect("meta").permissions().mode() & 0o777;
            assert_eq!(modo, 0o755, "entrypoint sem +x não abre");
        }
        assert_eq!(
            fs::read_to_string(dir.join(MARCA)).expect("marca").trim(),
            "2048"
        );
        let _ = fs::remove_dir_all(&ports);
    }

    #[test]
    fn segunda_passada_nao_reescreve_nada() {
        let ports = temp("idem-ports");
        let apps = ports.join("apps");
        plantar_port(&ports, "2048", "2048");
        let primeiro = sincronizar(&ports, &apps);
        assert_eq!(primeiro.criados, 1);

        let antes = fs::metadata(apps.join("portmaster-2048/metadata.json"))
            .and_then(|m| m.modified())
            .ok();
        let segundo = sincronizar(&ports, &apps);

        assert_eq!(segundo.criados, 0);
        assert_eq!(segundo.atualizados, 0);
        assert_eq!(segundo.iguais, 1);
        assert_eq!(
            fs::metadata(apps.join("portmaster-2048/metadata.json"))
                .and_then(|m| m.modified())
                .ok(),
            antes,
            "conteúdo igual não pode reescrever: é cartão SD"
        );
        let _ = fs::remove_dir_all(&ports);
    }

    #[test]
    fn port_removido_apaga_o_cartao() {
        let ports = temp("poda-ports");
        let apps = ports.join("apps");
        plantar_port(&ports, "2048", "2048");
        sincronizar(&ports, &apps);
        assert!(apps.join("portmaster-2048").is_dir());

        fs::remove_dir_all(ports.join("2048")).expect("remover port");
        let rel = sincronizar(&ports, &apps);

        assert_eq!(rel.removidos, 1, "{rel:?}");
        assert!(!apps.join("portmaster-2048").exists());
        let _ = fs::remove_dir_all(&ports);
    }

    #[test]
    fn app_alheio_com_o_mesmo_slug_nao_e_tocado() {
        let ports = temp("alheio-ports");
        let apps = ports.join("apps");
        plantar_port(&ports, "2048", "2048");
        let alheio = apps.join("portmaster-2048");
        fs::create_dir_all(&alheio).expect("dir");
        fs::write(alheio.join("metadata.json"), r#"{"title": "meu"}"#).expect("meta");

        let rel = sincronizar(&ports, &apps);

        assert_eq!(rel.criados, 0);
        assert_eq!(
            fs::read_to_string(alheio.join("metadata.json")).expect("meta"),
            r#"{"title": "meu"}"#
        );
        assert!(!rel.avisos.is_empty(), "o pulo tem de ser dito");
        let _ = fs::remove_dir_all(&ports);
    }

    #[test]
    fn sem_ports_nao_cria_nada() {
        let base = temp("sem-ports");
        let apps = base.join("apps");
        let rel = sincronizar(&base.join("nao-existe"), &apps);

        assert!(!rel.existe);
        assert!(rel.resumo().is_none());
        assert!(!apps.exists(), "PC não pode ganhar /user/apps do nada");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn dois_nomes_com_o_mesmo_slug_nao_se_sobrescrevem() {
        let ports = temp("colisao");
        let apps = ports.join("apps");
        let a = ports.join("a");
        fs::create_dir_all(&a).expect("a");
        fs::write(a.join("A B.sh"), "#!/bin/sh\n").expect("a b");
        let b = ports.join("b");
        fs::create_dir_all(&b).expect("b");
        fs::write(b.join("A-B.sh"), "#!/bin/sh\n").expect("a-b");

        let rel = sincronizar(&ports, &apps);

        assert_eq!(rel.ports, 2);
        assert_eq!(rel.criados, 2, "{rel:?}");
        let textos: Vec<String> = ["portmaster-a-b", "portmaster-a-b-2"]
            .iter()
            .map(|s| fs::read_to_string(apps.join(s).join("bin").join(s)).expect("script"))
            .collect();
        assert!(textos.iter().any(|t| t.contains("--rodar \"A B\"")));
        assert!(textos.iter().any(|t| t.contains("--rodar \"A-B\"")));
        let _ = fs::remove_dir_all(&ports);
    }
}
