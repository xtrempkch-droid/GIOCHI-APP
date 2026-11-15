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

use giochi_launcher_core::deteccao::{juntar, Tipo};
use giochi_launcher_core::{App, Detectado, Game};

/// Caminho publicado pelo `Giochi System`.
pub const CAMINHO: &str = "/opt/giochi/apps/index.json";

/// Prefixo dos apps de interface — os **temas** do launcher.
///
/// ## Por que o app precisa saber disto
///
/// O gerador do PC (`scripts/lib/catalogo.mjs`) já mantém tema fora do índice.
/// Mas ele não é o único caminho: `adicionar_ao_indice` deixa o **próprio
/// aparelho** registrar um pacote recém-instalado, sem Node e sem SSH. Instalar
/// um `launcher-neon.giochi` por ali criava um cartão que, apertado, abria um
/// SEGUNDO launcher — que perde o DRM (exclusivo) e morre com
/// `kmsdrm not available`. A tela volta para a Home e nada explica o motivo.
///
/// É a mesma razão de `podar_sem_binario`: o PC que gera o índice não sabe o
/// que estará no aparelho. Quem sabe é o aparelho.
pub const PREFIXO_TEMA: &str = "launcher";

/// `true` quando o id é de um app de interface (tema do launcher).
///
/// O hífen é exigido: sem ele `launchertool` viraria tema e o app sumiria do
/// catálogo em silêncio. A contraprova está nos testes.
pub fn eh_tema(id: &str) -> bool {
    id == PREFIXO_TEMA
        || id
            .strip_prefix(PREFIXO_TEMA)
            .is_some_and(|resto| resto.starts_with('-'))
}

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
    /// Jogo ou app. Decide a aba e **qual lista** o índice do pedido percorre.
    pub tipo: Tipo,
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
    /// Apps da aba Apps, detectados do disco (e do índice, para o que estiver
    /// fora das raízes).
    ///
    /// Lista separada de `games` porque a aba é outra: antes ela desenhava oito
    /// apps escritos no código e nenhum existia no aparelho.
    pub apps: Vec<App>,
    /// Alinhado por posição com `games`: `alvos[i]` descreve `games[i]`.
    ///
    /// `Game.id` é essa posição. O id do domínio é o *slug*, que não cabe em
    /// `u32`; a posição é só a chave interna de cor e favorito — nunca aparece
    /// na tela (ver "índice interno não é dado do jogo" no `AGENTS.md`).
    pub alvos: Vec<Alvo>,
    /// Alinhado por posição com `apps`, como `alvos` é com `games`.
    pub alvos_apps: Vec<Alvo>,
}

impl Catalogo {
    /// Lê **só o índice**, a partir do texto.
    ///
    /// `#[cfg(test)]` e não `pub`: quem abre o aparelho usa
    /// [`Catalogo::detectar`], que junta índice e disco. Este atalho sobra para
    /// os testes da leitura do `index.json` — precisar de um `Path` e de um
    /// sistema de arquivos para testar o parser seria pedir mais do ambiente do
    /// que o teste precisa.
    ///
    /// Deletá-lo junto com o `carregar` que existia aqui foi a escolha certa:
    /// uma função pública documentada e sem chamador é a segunda porta de
    /// entrada que envelhece sem ninguém notar.
    #[cfg(test)]
    pub fn do_texto(texto: &str) -> Result<Self, String> {
        let detectados = ler_indice(texto)?;
        Ok(Self::dos_detectados(detectados))
    }

    /// **A detecção**: o que está instalado, agora.
    ///
    /// ## O que mudou, e por quê
    ///
    /// Antes o launcher só sabia o que o `index.json` dizia, e a aba Apps nem
    /// isso — desenhava oito apps escritos no código. Consequência medida: um
    /// app copiado para `/user/apps` **não aparecia** até alguém rodar um script
    /// de Node no PC e publicar o índice de novo. "Não aparece" e "não existe"
    /// ficavam indistinguíveis.
    ///
    /// Agora a varredura do disco é a fonte principal e o índice é complemento:
    ///
    /// 1. varre as quatro raízes (`/opt/giochi/{games,apps}`, `/user/{games,apps}`)
    ///    procurando `<pasta>/metadata.json`;
    /// 2. lê `kind` do pacote para decidir a aba (ausente = jogo);
    /// 3. junta com o índice, **disco vencendo** ([`juntar`]);
    /// 4. poda tema e binário ausente, como antes.
    ///
    /// O índice não sumiu do caminho por dois motivos: ele é o que lista conteúdo
    /// fora das raízes, e é o que a imagem publica — apagar a leitura deixaria um
    /// arquivo documentado sem leitor.
    pub fn detectar() -> Result<Self, String> {
        let do_indice = match std::fs::read_to_string(caminho()) {
            Ok(texto) => match ler_indice(&texto) {
                Ok(lista) => lista,
                Err(e) => {
                    eprintln!("catálogo: índice inválido ({e}); usando só a varredura do disco");
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };

        let varridas = varrer(&raizes());
        if do_indice.is_empty() && varridas.is_empty() {
            // `Err` aqui mantém o contrato antigo de "não há catálogo": quem chama
            // decide o que dizer na tela. O motivo tem de citar as raízes, senão
            // o usuário procura no lugar errado.
            return Err(format!(
                "nenhum app encontrado em {} (nem catálogo em {})",
                Tipo::todas_as_raizes().join(", "),
                caminho().display()
            ));
        }

        let juntos = juntar(do_indice, varridas);
        Ok(Self::dos_detectados(juntos))
    }

    /// Um item varrido vira `Game` ou `App`, nunca os dois.
    ///
    /// A partição acontece **aqui e uma vez só**: duas listas construídas em
    /// lugares diferentes envelheceriam diferente — é como o carrossel do foco
    /// ficou dessincronizado da tela neste projeto.
    fn dos_detectados(detectados: Vec<Detectado>) -> Self {
        let mut catalogo = Self::default();
        // Todo caminho passa por `juntar`, inclusive o que não tem índice: a
        // deduplicação por slug é **uma** regra, e uma cópia aqui envelheceria
        // diferente dela. Sem isto, duas cópias em raízes diferentes virariam
        // dois cartões que abrem o mesmo app.
        for item in juntar(Vec::new(), detectados) {
            let alvo = Alvo {
                slug: item.slug,
                dir: item.dir,
                entrypoint: item.entrypoint,
                tipo: item.tipo,
            };
            match item.tipo {
                Tipo::Jogo => {
                    let id = catalogo.games.len() as u32;
                    catalogo.games.push(Game {
                        id,
                        title: item.titulo,
                        category: item.categoria,
                        year: item.ano,
                        hours: 0.0,
                        last_played: "nunca".to_string(),
                        favorite: false,
                    });
                    catalogo.alvos.push(alvo);
                }
                Tipo::App => {
                    catalogo.apps.push(App::new(
                        &alvo.slug,
                        &item.titulo,
                        // O ícone é do tema e a detecção não tem como adivinhar
                        // um glifo por app; a aba desenha monograma quando o
                        // nome não está na tabela (ver `views::apps`).
                        "apps",
                        &item.categoria,
                    ));
                    catalogo.alvos_apps.push(alvo);
                }
            }
        }
        catalogo
    }


    /// Alvo de jogo, a partir do `Game.id`.
    pub fn alvo(&self, id: u32) -> Option<&Alvo> {
        self.alvos.get(id as usize)
    }

    /// Alvo de app, a partir da posição na aba Apps.
    pub fn alvo_app(&self, indice: u32) -> Option<&Alvo> {
        self.alvos_apps.get(indice as usize)
    }

    /// `true` quando não há nem jogo nem app — a tela diz isso.
    pub fn sem_nada(&self) -> bool {
        self.games.is_empty() && self.apps.is_empty()
    }

    #[cfg(test)]
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
    pub fn podar_sem_binario(self, mut manter: impl FnMut(&str, &str)) -> Self {
        self.reter(|alvo| {
            let executavel = alvo.executavel();
            if std::path::Path::new(&executavel).is_file() {
                true
            } else {
                manter(&alvo.slug, &executavel);
                false
            }
        })
    }

    /// Descarta cartões de **tema** (prefixo `launcher`) do catálogo.
    ///
    /// ## Por que isto existe
    ///
    /// Um cartão de tema não abre um app: abre um segundo launcher, que disputa
    /// o DRM master com o primeiro (o dispositivo é exclusivo) e falha com
    /// `kmsdrm not available`. O usuário vê a Home voltar ao ponto de partida,
    /// sem nenhuma pista — o mesmo "cartão que abre nada" de
    /// `podar_sem_binario`, só que pior, porque derruba a interface.
    ///
    /// O gerador do PC já filtra, mas o índice pode chegar ao aparelho por
    /// outros caminhos: `adicionar_ao_indice` numa versão anterior, um índice
    /// escrito à mão no diagnóstico, ou um `index.json` que veio de imagem
    /// antiga. A poda em tempo de execução fecha os que sobram.
    pub fn podar_temas(self, mut manter: impl FnMut(&str, &str)) -> Self {
        self.reter(|alvo| {
            if eh_tema(&alvo.slug) {
                manter(&alvo.slug, PREFIXO_TEMA);
                false
            } else {
                true
            }
        })
    }

    /// O único lugar que remove entradas do catálogo.
    ///
    /// `Game.id` é a **posição**: renumerar mantém a chave consistente com
    /// `alvos`, e essa chave nunca aparece na tela (ver "índice interno não é
    /// dado do jogo"). Uma segunda cópia desta renumeração envelheceria
    /// diferente da primeira — foi assim que o carrossel do foco ficou
    /// dessincronizado da tela.
    fn reter(mut self, mut fica: impl FnMut(&Alvo) -> bool) -> Self {
        let mut games = Vec::with_capacity(self.games.len());
        let mut alvos = Vec::with_capacity(self.alvos.len());
        for (game, alvo) in self.games.into_iter().zip(self.alvos) {
            if fica(&alvo) {
                games.push(game);
                alvos.push(alvo);
            }
        }
        for (i, g) in games.iter_mut().enumerate() {
            g.id = i as u32;
        }
        self.games = games;
        self.alvos = alvos;

        // As duas listas passam pela **mesma** poda: uma segunda cópia disto
        // deixaria a aba Apps mostrando cartão sem binário, que é exatamente o
        // defeito que a poda existe para impedir.
        let apps = std::mem::take(&mut self.apps);
        let alvos_apps = std::mem::take(&mut self.alvos_apps);
        let mut mantidos = Vec::with_capacity(alvos_apps.len());
        for (app, alvo) in apps.into_iter().zip(alvos_apps) {
            if fica(&alvo) {
                mantidos.push((app, alvo));
            }
        }
        for (app, alvo) in mantidos {
            self.apps.push(app);
            self.alvos_apps.push(alvo);
        }
        self
    }
}

/// Lê o índice publicado e devolve o que ele declara, **sem** conferir disco.
///
/// Devolve `Detectado` — e não `Alvo` — porque a junção com a varredura
/// ([`juntar`]) precisa do tipo e do título dos dois lados.
fn ler_indice(texto: &str) -> Result<Vec<Detectado>, String> {
    let raiz: serde_json::Value =
        serde_json::from_str(texto).map_err(|e| format!("JSON inválido: {e}"))?;

    let lista = raiz
        .get("apps")
        .and_then(|v| v.as_array())
        .ok_or("catálogo sem a lista \"apps\"")?;

    let mut detectados = Vec::with_capacity(lista.len());
    for entrada in lista {
        // Uma entrada ruim não derruba o catálogo inteiro: o aparelho continua
        // utilizável com os apps que estão certos. O aviso fica no log, que é
        // onde se procura quando um app "não aparece".
        let Some(mut item) = entrada_do_indice(entrada) else {
            let slug = entrada
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("<sem id>");
            eprintln!("catálogo: entrada ignorada ({slug})");
            continue;
        };
        // O índice é fato do sistema instalado, não um pacote em disco: quem
        // vence na junção é a entrada varrida.
        item.do_disco = false;
        detectados.push(item);
    }
    Ok(detectados)
}

/// Uma entrada do `index.json` vira `Detectado`, com as mesmas recusas de antes.
fn entrada_do_indice(entrada: &serde_json::Value) -> Option<Detectado> {
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

    Some(Detectado {
        slug,
        titulo: entrada
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        dir,
        entrypoint,
        categoria: entrada
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("Aplicativo")
            .to_string(),
        ano: entrada
            .get("year")
            .and_then(|v| v.as_u64())
            .filter(|a| (1900..=2199).contains(a))
            .unwrap_or(0) as u16,
        tipo: Tipo::de_kind(entrada.get("kind").and_then(|v| v.as_str())),
        do_disco: false,
    })
}

/// Raízes a varrer, na ordem de precedência.
///
/// `GIOCHI_RAIZES` (separado por `:`) sobrepõe as quatro de uma vez. Existe pelo
/// mesmo motivo de `GIOCHI_CATALOGO`: no PC não há `/opt/giochi`, e sem a
/// sobreposição não haveria como testar a detecção sem root nem como apontar o
/// launcher para uma pasta qualquer.
pub fn raizes() -> Vec<String> {
    match std::env::var("GIOCHI_RAIZES") {
        Ok(v) if !v.trim().is_empty() => v
            .split(':')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Tipo::todas_as_raizes().iter().map(|p| (*p).to_string()).collect(),
    }
}

/// Varre as raízes e devolve o que encontrou de fato no disco.
///
/// Cada raiz é lida **um nível**: `<raiz>/<slug>/metadata.json`. É a mesma
/// profundidade do validador de apps, e a razão é a mesma — grupo dentro de
/// raiz de instalação não existe, e aceitar profundidades variáveis faria um
/// `assets/` com `metadata.json` virar app.
fn varrer(raizes: &[String]) -> Vec<Detectado> {
    let mut achados = Vec::new();
    for raiz in raizes {
        let dir = std::path::Path::new(raiz);
        let Ok(entradas) = std::fs::read_dir(dir) else {
            continue;
        };
        // Ordem estável: `read_dir` não promete nenhuma, e a precedência entre
        // duas cópias do mesmo slug é "a primeira raiz que o tem".
        let mut pastas: Vec<String> = entradas
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        pastas.sort();

        for pasta in pastas {
            let caminho_pasta = format!("{}/{}", raiz.trim_end_matches('/'), pasta);
            match ler_pacote(&caminho_pasta) {
                Ok(Some(item)) => achados.push(item),
                // `Ok(None)`: pasta sem `metadata.json` (ex.: `bin/`) ou com
                // JSON ilegível que já foi avisado. Não é erro do catálogo.
                Ok(None) => {}
                Err(aviso) => eprintln!("catálogo: {aviso}"),
            }
        }
    }
    achados
}

/// Decide se o pacote merece o aviso de lugar errado.
///
/// Separado do `eprintln!` para ser testável: um aviso que só existe no `stderr`
/// não tem como reprovar em teste, e foi justamente um aviso errado que mandou
/// mover o launcher para `/user/games` no R35S.
fn avisar_lugar(slug: &str, tipo: Tipo, dir: &str, kind: Option<&str>) -> Option<String> {
    if eh_tema(slug) {
        return None;
    }
    giochi_launcher_core::deteccao::aviso_de_lugar(tipo, dir, kind)
}

/// Lê `<dir>/metadata.json` e monta o item, com as conferências que importam.
///
/// `Ok(None)` = não é pacote (sem metadata). `Err` = é pacote e está errado — o
/// texto vai para o log, porque um pacote que existe e não aparece é
/// indistinguível de um pacote que não existe.
fn ler_pacote(dir: &str) -> Result<Option<Detectado>, String> {
    let caminho = format!("{dir}/metadata.json");
    let texto = match std::fs::read_to_string(&caminho) {
        Ok(t) => t,
        // `NotFound` é o caso normal de uma pasta que não é pacote.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Ok(Some(erro_de_pacote(dir, "não consegui ler o metadata.json", &e.to_string()))),
    };

    let meta: serde_json::Value = match serde_json::from_str(&texto) {
        Ok(v) => v,
        Err(e) => return Ok(Some(erro_de_pacote(dir, "metadata.json não é JSON válido", &e.to_string()))),
    };

    let entrypoint = meta
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if !entrypoint.starts_with("bin/") || entrypoint.contains("..") || entrypoint.len() <= 4 {
        return Ok(Some(erro_de_pacote(
            dir,
            "entrypoint inválido no metadata.json",
            &format!("`{entrypoint}` (precisa começar em bin/)"),
        )));
    }

    let kind = meta.get("kind").and_then(|v| v.as_str());
    if !Tipo::kind_reconhecido(kind) {
        // Grafia desconhecida cai no padrão (jogo), e isso é o comportamento
        // certo — mas dizer é o que impede `"kind": "jogo"` de virar campo
        // decorativo.
        eprintln!(
            "catálogo: {dir}: kind `{}` não é conhecido (use \"game\" ou \"app\"); tratando como jogo",
            kind.unwrap_or_default()
        );
    }
    let tipo = Tipo::de_kind(kind);

    let slug = slug_de(dir, &meta);
    // **Tema não recebe aviso de lugar.** Ele não entra em catálogo nenhum, então
    // o lugar onde está não muda cartão nenhum — e o aviso mandaria mover o
    // próprio executável de inicialização para a pasta de jogos. Medido no R35S
    // em 15/11/2026, antes desta guarda:
    //   `catálogo: launcher (/opt/giochi/apps/launcher) está em /opt/giochi/apps,
    //    que é raiz de outro tipo; o lugar de um jogo é /user/games`
    // A pasta estava certa; o campo `kind` é que não existe, e não precisa.
    if let Some(aviso) = avisar_lugar(&slug, tipo, dir, kind) {
        // O pacote diz o que é; o lugar está errado para ele. Aparecer na aba
        // certa com uma linha de log é melhor que sumir em silêncio.
        eprintln!("catálogo: {slug} ({dir}) {aviso}");
    }
    Ok(Some(Detectado {
        titulo: meta
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&slug)
            .to_string(),
        slug,
        dir: dir.to_string(),
        entrypoint,
        categoria: meta
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("Aplicativo")
            .to_string(),
        ano: meta
            .get("releaseYear")
            .and_then(|v| v.as_u64())
            .filter(|a| (1900..=2199).contains(a))
            .unwrap_or(0) as u16,
        tipo,
        do_disco: true,
    }))
}

/// Slug do pacote: `metadata.slug`, senão o nome da pasta.
///
/// A mesma ordem do validador e do `adicionar_ao_indice`. Derivar do título
/// moveria o app de lugar ao renomear — o defeito que já instalou em
/// `/opt/giochi/apps/pac-man` enquanto o catálogo apontava para `pacman`.
fn slug_de(dir: &str, meta: &serde_json::Value) -> String {
    meta.get("slug")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            std::path::Path::new(dir)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "desconhecido".to_string())
}

/// Pacote que existe e está errado: aparece no log e **não** vira cartão.
fn erro_de_pacote(dir: &str, motivo: &str, detalhe: &str) -> Detectado {
    eprintln!("catálogo: {dir}: {motivo} ({detalhe}) — fora do catálogo");
    Detectado {
        slug: dir.to_string(),
        titulo: String::new(),
        dir: dir.to_string(),
        // `entrypoint` vazio nunca é `bin/…`, então `podar_sem_binario` o
        // descarta — a mesma peneira de todo pacote.
        entrypoint: String::new(),
        categoria: "Aplicativo".to_string(),
        ano: 0,
        tipo: Tipo::Jogo,
        do_disco: true,
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

    // Instalar um `launcher-*.giochi` no aparelho e registrá-lo aqui criaria o
    // cartão que abre um segundo launcher e derruba o DRM. Recusar com o motivo
    // é melhor que gravar e deixar o usuário descobrir apertando A: o índice
    // fica sem uma entrada que nunca deveria existir, e o instalador diz por quê.
    if eh_tema(&id) {
        return Err(format!(
            "{id} é tema do launcher (prefixo \"{PREFIXO_TEMA}\"): não vira cartão \
             — um cartão de tema abriria um segundo launcher e perderia o DRM"
        ));
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

    /// O prefixo é do **começo** e com hífen.
    ///
    /// Sem o hífen, `launchertool` viraria tema e o app sumiria do catálogo em
    /// silêncio — o erro que a regra existe para evitar, invertido. Sem a âncora
    /// no começo, `my-launcher-tool` sumiria também.
    #[test]
    fn eh_tema_exige_o_prefixo_no_comeco_e_o_hifen() {
        for id in ["launcher", "launcher-teste", "launcher-star-dust", "launcher-neon"] {
            assert!(eh_tema(id), "{id} é tema");
        }
        for id in ["launchertool", "my-launcher-tool", "relancher", "jogo", ""] {
            assert!(!eh_tema(id), "{id} NÃO é tema");
        }
    }

    fn com_ids(ids: &[&str]) -> String {
        let apps: Vec<String> = ids
            .iter()
            .map(|id| {
                format!(
                    r#"{{ "id": "{id}", "version": "1.0.0", "entrypoint": "bin/env",
                        "dir": "/usr", "title": "T-{id}", "developer": "g" }}"#
                )
            })
            .collect();
        format!(r#"{{"schemaVersion":1,"apps":[{}]}}"#, apps.join(","))
    }

    /// Tema no índice não vira cartão.
    ///
    /// O cartão de um tema não abre um app: abre um segundo launcher, que perde
    /// o DRM master (exclusivo) e morre com `kmsdrm not available`. A tela volta
    /// para a Home e nada explica — pior que o "cartão que abre nada", porque
    /// derruba a interface junto.
    ///
    /// Aqui o binário **existe** de propósito (`bin/env` em `/usr`), para provar
    /// que é a poda de tema que tirou o cartão e não a de binário.
    #[test]
    fn poda_tema_mesmo_com_binario_presente() {
        let texto = com_ids(&["launcher-neon", "jogo", "launcher"]);
        let c = Catalogo::do_texto(&texto).unwrap();
        assert_eq!(c.games.len(), 3, "o catálogo lê os três");

        let mut podados = Vec::new();
        let c = c.podar_temas(|slug, motivo| podados.push((slug.to_string(), motivo.to_string())));

        assert_eq!(c.games.len(), 1, "só o app comum fica");
        assert_eq!(c.games[0].title, "T-jogo");
        assert_eq!(c.games[0].id, 0, "o id é a posição, e renumerar mantém consistente");
        assert_eq!(c.alvos.len(), 1, "games e alvos têm de ficar alinhados");
        assert_eq!(podados.len(), 2, "a poda precisa dizer o que tirou");
        // A ordem do relatório segue a **ordem da lista**, que é por título —
        // não a ordem do `index.json`. Afirmar a posição aqui já custou um teste
        // vermelho quando a ordenação passou a valer também para o índice.
        let mut nomes: Vec<&str> = podados.iter().map(|(s, _)| s.as_str()).collect();
        nomes.sort_unstable();
        assert_eq!(nomes, vec!["launcher", "launcher-neon"]);
    }

    #[test]
    fn poda_temas_sem_nada_a_podar_nao_muda_nada() {
        let c = Catalogo::do_texto(&com_ids(&["jogo", "my-launcher-tool"])).unwrap();
        let c = c.podar_temas(|slug, _| panic!("não devia podar {slug}"));
        assert_eq!(c.games.len(), 2);
    }

    /// Instalar um tema no aparelho é recusado com o motivo, não em silêncio.
    ///
    /// `adicionar_ao_indice` é o caminho do **próprio aparelho** para o índice,
    /// e não passa pelo gerador do PC. Sem esta guarda, instalar um
    /// `launcher-neon.giochi` criaria o cartão que derruba o DRM.
    #[test]
    fn instalar_tema_e_recusado_com_motivo() {
        let e = adicionar_ao_indice(EXEMPLO, "/user/apps/launcher-neon", &meta("launcher-neon"))
            .expect_err("tema não deveria entrar no índice");
        assert!(e.contains("tema"), "o erro tem de dizer o motivo: {e}");
        assert!(e.contains("DRM"), "o erro tem de dizer a consequência: {e}");

        // A contraprova: um app com "launcher" no meio continua entrando.
        assert!(adicionar_ao_indice(EXEMPLO, "/user/apps/my-launcher-tool", &meta("my-launcher-tool")).is_ok());
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
        let com_licenca = EXEMPLO.replace(r#""developer": "Giochi","#, r#""developer": "Giochi", "license": "Proprietary","#);
        let meta1 = serde_json::json!({
            "slug": "volley", "title": "Volley", "developer": "Giochi", "version": "0.2.0",
            "arch": "aarch64", "runtime": "native", "entrypoint": "bin/giochi-volley"
        });
        let texto = adicionar_ao_indice(&com_licenca, "/user/apps/volley", &meta1).unwrap();
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

    // ── detecção do disco ───────────────────────────────────
    //
    // Estes testes montam uma árvore de verdade num diretório temporário e
    // chamam `varrer`, que recebe as raízes por parâmetro. A variável
    // `GIOCHI_RAIZES` fica de fora de propósito: variável de ambiente é global
    // ao processo, e dois testes em paralelo se atropelariam — o teste passaria
    // ou falharia conforme a ordem de execução, que é o pior tipo de teste.

    use std::fs;

    /// Cria `<raiz>/<slug>/metadata.json` e o binário do entrypoint.
    fn plantar(raiz: &std::path::Path, slug: &str, kind: Option<&str>, com_binario: bool) -> String {
        let dir = raiz.join(slug);
        fs::create_dir_all(dir.join("bin")).expect("mkdir");
        let kind = kind.map(|k| format!(r#", "kind": "{k}""#)).unwrap_or_default();
        fs::write(
            dir.join("metadata.json"),
            format!(
                r#"{{"title": "Título {slug}", "slug": "{slug}", "developer": "g",
                    "version": "1.0.0", "arch": "aarch64", "runtime": "native",
                    "entrypoint": "bin/{slug}"{kind}}}"#
            ),
        )
        .expect("metadata");
        if com_binario {
            fs::write(dir.join("bin").join(slug), b"#!/bin/sh\n").expect("bin");
        }
        dir.to_string_lossy().into_owned()
    }

    fn temp(nome: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "giochi-cat-{nome}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp");
        dir
    }

    #[test]
    fn deteccao_encontra_pacote_sem_indice_nenhum() {
        // O caso que motivou o pedido: copiar a pasta para o aparelho tem de
        // bastar. Antes, sem `index.json` atualizado no PC, o app não aparecia.
        let raiz = temp("sem-indice");
        plantar(&raiz, "musica", Some("app"), true);

        let achados = varrer(&[raiz.to_string_lossy().into_owned()]);

        assert_eq!(achados.len(), 1);
        assert_eq!(achados[0].slug, "musica");
        assert_eq!(achados[0].tipo, Tipo::App);
        assert!(achados[0].do_disco);
        assert_eq!(achados[0].titulo, "Título musica");
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn tema_nao_recebe_aviso_de_lugar() {
        // Medido no R35S em 15/11/2026: o launcher mora em `/opt/giochi/apps`,
        // não declara `kind` (é tema) e por isso caía no padrão jogo — o log
        // dizia "o lugar de um jogo é /user/games", que é onde o executável de
        // inicialização **não** pode ficar. Um tema não entra em catálogo, então
        // o lugar dele não muda cartão nenhum: silêncio é a resposta certa.
        assert_eq!(avisar_lugar("launcher", Tipo::Jogo, "/opt/giochi/apps/launcher", None), None);
        assert_eq!(
            avisar_lugar("launcher-star-dust", Tipo::Jogo, "/opt/giochi/apps/launcher-star-dust", None),
            None
        );
    }

    #[test]
    fn app_em_raiz_de_jogo_continua_avisando() {
        // A guarda de tema não pode engolir o aviso que existe por um motivo:
        // um pacote classificado como app dentro de `/user/games` apareceria na
        // aba errada sem uma linha de explicação.
        let aviso = avisar_lugar("musica", Tipo::App, "/user/games/musica", Some("app"))
            .expect("aviso");
        assert!(aviso.contains("/user/games"), "{aviso}");
        assert!(aviso.contains("/user/apps"), "{aviso}");
    }

    #[test]
    fn sem_kind_o_pacote_e_jogo() {
        // Compatibilidade: os três jogos do R35S foram instalados antes de o
        // campo existir. Se o padrão fosse app, a Home apareceria vazia num
        // aparelho cheio.
        let raiz = temp("sem-kind");
        plantar(&raiz, "cobrinha", None, true);

        let achados = varrer(&[raiz.to_string_lossy().into_owned()]);

        assert_eq!(achados[0].tipo, Tipo::Jogo);
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn tipo_separa_as_duas_listas_e_nenhum_item_aparece_nas_duas() {
        let raiz = temp("separacao");
        plantar(&raiz, "jogo1", Some("game"), true);
        plantar(&raiz, "app1", Some("app"), true);

        let catalogo = Catalogo::dos_detectados(varrer(&[raiz.to_string_lossy().into_owned()]));

        assert_eq!(catalogo.games.len(), 1);
        assert_eq!(catalogo.apps.len(), 1);
        assert_eq!(catalogo.games[0].title, "Título jogo1");
        assert_eq!(catalogo.apps[0].title, "Título app1");
        // E os alvos acompanham a posição de cada lista.
        assert_eq!(catalogo.alvo(0).map(|a| a.slug.as_str()), Some("jogo1"));
        assert_eq!(catalogo.alvo_app(0).map(|a| a.slug.as_str()), Some("app1"));
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn pasta_sem_metadata_nao_vira_pacote() {
        // `bin/` e `assets/` moram dentro de uma raiz e não têm metadata: se
        // qualquer pasta virasse app, a raiz inteira viraria catálogo.
        let raiz = temp("sem-metadata");
        fs::create_dir_all(raiz.join("bin")).expect("bin");
        fs::create_dir_all(raiz.join("uma-pasta")).expect("pasta");

        assert!(varrer(&[raiz.to_string_lossy().into_owned()]).is_empty());
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn pacote_com_metadata_quebrado_nao_derruba_a_varredura() {
        let raiz = temp("quebrado");
        let quebrado = raiz.join("quebrado");
        fs::create_dir_all(&quebrado).expect("dir");
        fs::write(quebrado.join("metadata.json"), "{lixo").expect("metadata");
        plantar(&raiz, "bom", Some("app"), true);

        let achados = varrer(&[raiz.to_string_lossy().into_owned()]);

        // O quebrado entra com `entrypoint` vazio e é descartado pela poda de
        // binário; o bom continua. Um pacote estragado não pode esconder os
        // outros — foi assim que um `index.json` inválido apagou o catálogo.
        let catalogo = Catalogo::dos_detectados(achados)
            .podar_sem_binario(|_, _| {});
        assert_eq!(catalogo.apps.len(), 1);
        assert_eq!(catalogo.apps[0].id, "bom");
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn entrypoint_fora_de_bin_nao_vira_cartao() {
        let raiz = temp("entrypoint");
        let dir = raiz.join("mau");
        fs::create_dir_all(dir.join("bin")).expect("dir");
        fs::write(
            dir.join("metadata.json"),
            r#"{"title":"Mau","slug":"mau","entrypoint":"../fora"}"#,
        )
        .expect("metadata");

        let catalogo = Catalogo::dos_detectados(varrer(&[raiz.to_string_lossy().into_owned()]))
            .podar_sem_binario(|_, _| {});
        assert!(catalogo.sem_nada(), "entrypoint fora de bin/ virou cartão");
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn imagem_antes_de_usuario_na_mesma_raiz_nao_duplica() {
        // Duas cópias do mesmo slug (imagem e usuário): a primeira raiz vence,
        // e o cartão é um só.
        let imagem = temp("raiz-imagem");
        let usuario = temp("raiz-usuario");
        plantar(&imagem, "x", Some("game"), true);
        plantar(&usuario, "x", Some("game"), true);

        let catalogo = Catalogo::dos_detectados(varrer(&[
            imagem.to_string_lossy().into_owned(),
            usuario.to_string_lossy().into_owned(),
        ]));

        assert_eq!(catalogo.games.len(), 1);
        assert_eq!(
            catalogo.alvo(0).map(|a| a.dir.clone()),
            Some(imagem.join("x").to_string_lossy().into_owned())
        );
        let _ = fs::remove_dir_all(&imagem);
        let _ = fs::remove_dir_all(&usuario);
    }

    #[test]
    fn slug_vem_do_metadata_e_o_nome_da_pasta_e_so_o_ultimo_recurso() {
        let raiz = temp("slug");
        let dir = raiz.join("pasta-qualquer");
        fs::create_dir_all(dir.join("bin")).expect("dir");
        fs::write(
            dir.join("metadata.json"),
            r#"{"title":"T","slug":"nome-estavel","entrypoint":"bin/x"}"#,
        )
        .expect("meta");
        let achados = varrer(&[raiz.to_string_lossy().into_owned()]);
        assert_eq!(achados[0].slug, "nome-estavel");
        let _ = fs::remove_dir_all(&raiz);
    }

    #[test]
    fn raiz_inexistente_nao_e_erro() {
        // `/opt/giochi/games` não existe em nenhuma imagem até hoje. A varredura
        // tem de simplesmente não achar nada ali, e não cair.
        assert!(varrer(&["/caminho/que/nao/existe".to_string()]).is_empty());
    }
}
