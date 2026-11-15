//! Detecção: o que é jogo, o que é app, e **onde** cada um mora.
//!
//! ## O problema que isto resolve
//!
//! A aba Apps do launcher desenhava oito apps escritos no código
//! (`demo_apps()`): Música, RetroArch, Arquivos, Terminal, Navegador, Capturas,
//! Loja, Sincronizar. Nenhum deles existia no aparelho, e apertar `A` só mostrava
//! um aviso. É o mesmo defeito que a Home já teve com quatro jogos de
//! demonstração, e o mesmo que o catálogo escrito à mão produzia: **cartão que
//! aparece e não abre**.
//!
//! A aba Jogos, em contraste, lê o que o sistema publica e poda o que não tem
//! binário. A aba Apps passa a fazer o mesmo: detecta.
//!
//! ## Por que os dois têm raízes diferentes
//!
//! "Apps deverão ficar em locais diferentes dos jogos para que a detecção não se
//! confunda" — decisão do dono, 15/11/2026. Sem separação, a única forma de
//! distinguir seria um campo declarativo, e aí um pacote sem o campo cairia no
//! lugar errado **em silêncio**. Com raízes distintas, o lugar é a resposta
//! padrão e a declaração é a exceção que se pode conferir.
//!
//! | tipo | imagem | usuário / loja |
//! | --- | --- | --- |
//! | jogo | `/opt/giochi/games` | `/user/games` |
//! | app | `/opt/giochi/apps` | `/user/apps` |
//!
//! ## A declaração ainda existe, e não é redundância
//!
//! O `.giochi` precisa ser auto-descritivo: um pacote copiado para um cartão tem
//! de chegar ao lugar certo **sem** um PC decidindo por ele. `metadata.json`
//! ganhou `kind` (`game` ou `app`), que é o que o instalador lê para escolher a
//! raiz.
//!
//! Quando os dois discordam — `kind: "app"` dentro de `/user/games` — a
//! **declaração vence** (o pacote diz o que é) e o detector **avisa** onde ele
//! está. É o contrário de silêncio: um app que aparece em lugar nenhum é pior
//! que um app na aba certa com uma linha de log dizendo que está na pasta errada.

/// O que a aba de destino mostra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Jogo,
    App,
}

impl Tipo {
    /// Interpreta o `kind` do `metadata.json`.
    ///
    /// **Ausente vira jogo**, e isso é compatibilidade, não preferência: antes
    /// desta separação todo pacote do aparelho era jogo. Um app novo declara
    /// `"kind": "app"`; o que já está instalado continua onde estava, sem
    /// precisar ser reempacotado.
    pub fn de_kind(kind: Option<&str>) -> Tipo {
        match kind.map(str::trim) {
            Some("app") => Tipo::App,
            Some("apps") => Tipo::App,
            _ => Tipo::Jogo,
        }
    }

    /// `true` quando o `kind` era um valor conhecido (ou ausente).
    ///
    /// Existe para o validador e para o detector avisarem sobre `"kind": "jogo"`
    /// — que é o nome em português, plausível de escrever e **silenciosamente**
    /// tratado como jogo hoje. O aviso é o que impede a grafia de virar um
    /// campo decorativo.
    pub fn kind_reconhecido(kind: Option<&str>) -> bool {
        matches!(kind.map(str::trim), None | Some("") | Some("game") | Some("app") | Some("apps"))
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Jogo => "jogo",
            Tipo::App => "app",
        }
    }

    /// Valor canônico para gravar em `metadata.json`.
    pub fn kind(self) -> &'static str {
        match self {
            Tipo::Jogo => "game",
            Tipo::App => "app",
        }
    }

    /// Raízes onde este tipo é procurado, na ordem de precedência.
    ///
    /// A ordem importa: a primeira que contém o slug vence. Imagem antes de
    /// usuário porque o que veio na imagem é o que o sistema assinou; um pacote
    /// do usuário com o mesmo slug é uma substituição — e é o `/user` que o
    /// mecanismo de licença permite trocar (§9 do `AGENTS.md`).
    pub fn raizes(self) -> [&'static str; 2] {
        match self {
            Tipo::Jogo => RAIZES_JOGO,
            Tipo::App => RAIZES_APP,
        }
    }

    /// Raiz de instalação para pacotes que chegam pela loja ou pelo cartão.
    pub fn raiz_de_instalacao(self) -> &'static str {
        match self {
            Tipo::Jogo => RAIZ_JOGO_USUARIO,
            Tipo::App => RAIZ_APP_USUARIO,
        }
    }

    /// Todas as raízes dos dois tipos — o que a detecção varre.
    ///
    /// A varredura é **conjunta** de propósito: um jogo instalado em
    /// `/user/apps` (como os três da imagem de 15/11/2026, anteriores a esta
    /// separação) continua sendo jogo, porque o `kind` ausente vale `game`. Varrer
    /// só a raiz do próprio tipo faria esses três **sumirem** da Home.
    pub fn todas_as_raizes() -> [&'static str; 4] {
        [
            RAIZ_JOGO_IMAGEM,
            RAIZ_JOGO_USUARIO,
            RAIZ_APP_IMAGEM,
            RAIZ_APP_USUARIO,
        ]
    }
}

/// Jogos que vêm na imagem assinada.
pub const RAIZ_JOGO_IMAGEM: &str = "/opt/giochi/games";
/// Jogos que chegam pela loja ou pelo cartão.
pub const RAIZ_JOGO_USUARIO: &str = "/user/games";
/// Apps que vêm na imagem assinada.
pub const RAIZ_APP_IMAGEM: &str = "/opt/giochi/apps";
/// Apps que chegam pela loja ou pelo cartão.
pub const RAIZ_APP_USUARIO: &str = "/user/apps";

pub const RAIZES_JOGO: [&str; 2] = [RAIZ_JOGO_IMAGEM, RAIZ_JOGO_USUARIO];
pub const RAIZES_APP: [&str; 2] = [RAIZ_APP_IMAGEM, RAIZ_APP_USUARIO];

/// Pra onde o instalador deve mandar um pacote deste tipo.
pub fn raiz_de_instalacao(kind: Option<&str>) -> &'static str {
    Tipo::de_kind(kind).raiz_de_instalacao()
}

/// Aviso quando o pacote está numa raiz que não é a do próprio tipo.
///
/// Devolve `None` quando o lugar está certo — inclusive para pacote em raiz
/// desconhecida, porque aí não há o que afirmar.
pub fn aviso_de_lugar(tipo: Tipo, dir: &str, kind_declarado: Option<&str>) -> Option<String> {
    let das_do_tipo = tipo.raizes().iter().any(|raiz| dentro_de(dir, raiz));
    if das_do_tipo {
        return None;
    }
    let onde = Tipo::todas_as_raizes()
        .iter()
        .find(|raiz| dentro_de(dir, raiz))
        .copied();
    let onde = onde?;
    let mut texto = format!(
        "está em {onde}, que é raiz de outro tipo; o lugar de um {} é {}",
        tipo.nome(),
        tipo.raiz_de_instalacao()
    );
    // O `kind` ausente **cria** este aviso: sem ele o padrão é jogo (§3), e um
    // app em `/opt/giochi/apps` passa a "estar no lugar errado" por omissão de um
    // campo. Medido no R35S em 15/11/2026: o launcher recebeu "o lugar de um jogo
    // é /user/games" — instrução que, seguida, põe o executável de inicialização
    // dentro da pasta de jogos. Dizer as duas leituras custa uma linha.
    if kind_declarado.is_none() {
        texto.push_str(
            " (ou declare \"kind\": \"app\" no metadata.json, se for um app)",
        );
    }
    Some(texto)
}

/// `dir` está dentro de `raiz` (comparação por componente, não por prefixo de
/// texto: `/user/apps2` não está dentro de `/user/apps`).
fn dentro_de(dir: &str, raiz: &str) -> bool {
    let raiz = raiz.trim_end_matches('/');
    dir == raiz
        || dir
            .strip_prefix(raiz)
            .is_some_and(|resto| resto.starts_with('/'))
}

/// Um pacote detectado no disco, antes de virar cartão.
///
/// `Clone` e `PartialEq` porque a junção com o índice compara por slug e a
/// ordenação é estável por título.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detectado {
    pub slug: String,
    pub titulo: String,
    pub dir: String,
    pub entrypoint: String,
    pub categoria: String,
    /// `0` = desconhecido. Nunca derivado de índice (ver `AGENTS.md` §3).
    pub ano: u16,
    pub tipo: Tipo,
    /// De onde veio: `true` quando o pacote foi lido do disco.
    ///
    /// A diferença decide a precedência da junção e o que o log diz. Um índice
    /// que aponta para um diretório ausente não pode vencer um pacote que está
    /// ali de fato.
    pub do_disco: bool,
}

impl Detectado {
    pub fn executavel(&self) -> String {
        format!("{}/{}", self.dir.trim_end_matches('/'), self.entrypoint)
    }
}

/// Junta o que veio do índice com o que a varredura achou.
///
/// ## A regra
///
/// **O disco vence.** Se o mesmo slug aparece nos dois, a entrada varrida fica:
/// ela é a que existe de fato, com o `dir` onde está. Foi a lição da loja — o
/// índice apontava para `/opt/giochi/apps/x` e o pacote estava em `/user/apps/x`.
///
/// Entrada que só existe no índice é **mantida**: é o caso de conteúdo que não
/// mora nas raízes (cartão montado fora delas), e descartá-la apagaria um jogo
/// que abre.
///
/// O `tipo` do varrido vem do `kind` do próprio pacote; o do índice, do campo
/// `kind` da entrada (ausente = jogo).
pub fn juntar(do_indice: Vec<Detectado>, do_disco: Vec<Detectado>) -> Vec<Detectado> {
    let mut juntos: Vec<Detectado> = Vec::with_capacity(do_indice.len() + do_disco.len());
    let mut vistos: Vec<String> = Vec::with_capacity(do_disco.len());

    for entrada in do_disco {
        if vistos.iter().any(|s| s == &entrada.slug) {
            // Duas cópias do mesmo slug em raízes diferentes: a primeira vence,
            // e a ordem de `todas_as_raizes` é a precedência (imagem antes de
            // usuário). Duplicar produziria dois cartões que abrem o mesmo app.
            continue;
        }
        vistos.push(entrada.slug.clone());
        juntos.push(entrada);
    }

    for entrada in do_indice {
        if vistos.iter().any(|s| s == &entrada.slug) {
            continue;
        }
        vistos.push(entrada.slug.clone());
        juntos.push(entrada);
    }

    // Ordem estável por título (mesma regra do gerador do PC): sem ela a Home
    // mudaria de conteúdo conforme a ordem de leitura do diretório.
    juntos.sort_by_key(|d| d.titulo.to_lowercase());
    juntos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pacote(slug: &str, dir: &str, tipo: Tipo, do_disco: bool) -> Detectado {
        Detectado {
            slug: slug.to_string(),
            titulo: slug.to_string(),
            dir: dir.to_string(),
            entrypoint: "bin/x".to_string(),
            categoria: "Aplicativo".to_string(),
            ano: 0,
            tipo,
            do_disco,
        }
    }

    // ── tipo ────────────────────────────────────────────────

    #[test]
    fn kind_ausente_vale_jogo() {
        // Compatibilidade: todo pacote anterior a esta separação era jogo. Se o
        // padrão fosse `app`, os três jogos do R35S virariam apps da noite para
        // o dia — e a Home apareceria vazia num aparelho cheio.
        assert_eq!(Tipo::de_kind(None), Tipo::Jogo);
        assert_eq!(Tipo::de_kind(Some("")), Tipo::Jogo);
        assert_eq!(Tipo::de_kind(Some("game")), Tipo::Jogo);
    }

    #[test]
    fn kind_app_declara_app() {
        assert_eq!(Tipo::de_kind(Some("app")), Tipo::App);
        assert_eq!(Tipo::de_kind(Some(" apps ")), Tipo::App);
    }

    #[test]
    fn kind_com_grafia_desconhecida_vira_jogo_e_e_reportado() {
        // `"kind": "jogo"` é o nome em português e o usuário vai escrever isso.
        // Tratar como jogo é o comportamento certo (é o padrão), mas tem de ser
        // **dito** — senão a grafia vira campo decorativo.
        assert_eq!(Tipo::de_kind(Some("jogo")), Tipo::Jogo);
        assert!(!Tipo::kind_reconhecido(Some("jogo")));
        assert!(Tipo::kind_reconhecido(Some("app")));
        assert!(Tipo::kind_reconhecido(Some("game")));
        assert!(Tipo::kind_reconhecido(None));
    }

    #[test]
    fn tipo_e_kind_sao_o_mesmo_par() {
        // O que se lê do metadata e o que se grava nele não podem divergir: o
        // instalador grava `kind()` e o detector lê com `de_kind`.
        for tipo in [Tipo::Jogo, Tipo::App] {
            assert_eq!(Tipo::de_kind(Some(tipo.kind())), tipo);
        }
    }

    // ── raízes ──────────────────────────────────────────────

    #[test]
    fn cada_tipo_tem_raizes_distintas() {
        assert_eq!(Tipo::Jogo.raizes(), [RAIZ_JOGO_IMAGEM, RAIZ_JOGO_USUARIO]);
        assert_eq!(Tipo::App.raizes(), [RAIZ_APP_IMAGEM, RAIZ_APP_USUARIO]);
        // A separação é o ponto do pedido: nenhuma raiz é compartilhada.
        for raiz in Tipo::Jogo.raizes() {
            assert!(!Tipo::App.raizes().contains(&raiz), "{raiz} nos dois tipos");
        }
    }

    #[test]
    fn instalacao_do_app_vai_para_o_user_de_apps() {
        assert_eq!(raiz_de_instalacao(Some("app")), RAIZ_APP_USUARIO);
        assert_eq!(raiz_de_instalacao(Some("game")), RAIZ_JOGO_USUARIO);
        assert_eq!(raiz_de_instalacao(None), RAIZ_JOGO_USUARIO);
    }

    #[test]
    fn varredura_cobre_as_quatro_raizes() {
        let todas = Tipo::todas_as_raizes();
        assert_eq!(todas.len(), 4);
        for raiz in Tipo::Jogo.raizes().into_iter().chain(Tipo::App.raizes()) {
            assert!(todas.contains(&raiz), "{raiz} fora da varredura");
        }
    }

    #[test]
    fn dentro_de_compara_componente_nao_texto() {
        assert!(dentro_de("/user/apps/musica", "/user/apps"));
        assert!(dentro_de("/user/apps", "/user/apps"));
        assert!(dentro_de("/user/apps/", "/user/apps"));
        // O caso que um `starts_with` de texto erraria: `apps2` não é `apps`.
        assert!(!dentro_de("/user/apps2/musica", "/user/apps"));
        assert!(!dentro_de("/user/ap", "/user/apps"));
        assert!(!dentro_de("/outro/lugar", "/user/apps"));
    }

    // ── aviso de lugar ──────────────────────────────────────

    #[test]
    fn lugar_certo_nao_gera_aviso() {
        assert_eq!(aviso_de_lugar(Tipo::App, "/user/apps/musica", Some("app")), None);
        assert_eq!(aviso_de_lugar(Tipo::App, "/opt/giochi/apps/musica", Some("app")), None);
        assert_eq!(aviso_de_lugar(Tipo::Jogo, "/user/games/volley", Some("game")), None);
    }

    #[test]
    fn lugar_trocado_gera_aviso_com_o_destino() {
        let aviso = aviso_de_lugar(Tipo::App, "/user/games/musica", Some("app")).expect("aviso");
        assert!(aviso.contains("/user/games"), "não diz onde está: {aviso}");
        assert!(aviso.contains("/user/apps"), "não diz para onde vai: {aviso}");

        let aviso = aviso_de_lugar(Tipo::Jogo, "/user/apps/cobrinha", Some("game")).expect("aviso");
        assert!(aviso.contains("/user/games"));
    }

    #[test]
    fn diretorio_fora_das_raizes_nao_afirma_nada() {
        // Cartão montado fora das raízes é legítimo (ver docs/cartao-sd.md).
        // Avisar ali seria ruído que ensina a ignorar avisos.
        assert_eq!(aviso_de_lugar(Tipo::Jogo, "/mnt/cartao/volley", None), None);
        assert_eq!(aviso_de_lugar(Tipo::App, "/mnt/cartao/app", None), None);
    }

    #[test]
    fn kind_ausente_oferece_as_duas_leituras_do_mesmo_sintoma() {
        // Medido no R35S em 15/11/2026: o launcher mora em `/opt/giochi/apps` e
        // não declara `kind` (é tema), então caía no padrão jogo e recebia
        // "o lugar de um jogo é /user/games". Seguir esse aviso moveria o
        // executável de inicialização para dentro da pasta de jogos.
        let sem_kind = aviso_de_lugar(Tipo::Jogo, "/opt/giochi/apps/launcher", None)
            .expect("aviso");
        assert!(sem_kind.contains("/user/games"), "{sem_kind}");
        assert!(
            sem_kind.contains(r#""kind": "app""#),
            "não diz que a causa pode ser o campo ausente: {sem_kind}"
        );

        // Com `kind` declarado o aviso continua existindo, e sem o palpite.
        let com_kind = aviso_de_lugar(Tipo::Jogo, "/user/apps/cobrinha", Some("game"))
            .expect("aviso");
        assert!(!com_kind.contains("kind"), "palpite à toa: {com_kind}");
    }

    // ── junção ──────────────────────────────────────────────

    #[test]
    fn disco_vence_o_indice_para_o_mesmo_slug() {
        let indice = vec![pacote("cobrinha", "/opt/giochi/apps/cobrinha", Tipo::Jogo, false)];
        let disco = vec![pacote("cobrinha", "/user/apps/cobrinha", Tipo::Jogo, true)];
        let juntos = juntar(indice, disco);
        assert_eq!(juntos.len(), 1, "duas cópias do mesmo slug viram dois cartões");
        assert_eq!(juntos[0].dir, "/user/apps/cobrinha", "o índice venceu o disco");
        assert!(juntos[0].do_disco);
    }

    #[test]
    fn entrada_que_so_existe_no_indice_fica() {
        // Cartão montado fora das raízes: a varredura não o vê, e descartá-lo
        // apagaria um jogo que abre.
        let indice = vec![pacote("volley", "/mnt/cartao/volley", Tipo::Jogo, false)];
        let juntos = juntar(indice, vec![]);
        assert_eq!(juntos.len(), 1);
        assert_eq!(juntos[0].dir, "/mnt/cartao/volley");
        assert!(!juntos[0].do_disco);
    }

    #[test]
    fn dois_pacotes_com_o_mesmo_slug_no_disco_nao_duplicam() {
        // `/opt` antes de `/user` na ordem de varredura: o da imagem vence.
        let disco = vec![
            pacote("x", RAIZ_JOGO_IMAGEM, Tipo::Jogo, true),
            pacote("x", RAIZ_JOGO_USUARIO, Tipo::Jogo, true),
        ];
        let juntos = juntar(vec![], disco);
        assert_eq!(juntos.len(), 1);
        assert_eq!(juntos[0].dir, RAIZ_JOGO_IMAGEM);
    }

    #[test]
    fn tipos_diferentes_convivem_e_ficam_filtraveis() {
        let disco = vec![
            pacote("musica", "/user/apps/musica", Tipo::App, true),
            pacote("cobrinha", "/user/apps/cobrinha", Tipo::Jogo, true),
            pacote("volley", "/user/games/volley", Tipo::Jogo, true),
        ];
        let juntos = juntar(vec![], disco);
        let apps: Vec<&str> = juntos.iter().filter(|d| d.tipo == Tipo::App).map(|d| d.slug.as_str()).collect();
        let jogos: Vec<&str> = juntos.iter().filter(|d| d.tipo == Tipo::Jogo).map(|d| d.slug.as_str()).collect();
        assert_eq!(apps, vec!["musica"], "o app não pode cair na Home");
        assert_eq!(jogos, vec!["cobrinha", "volley"], "jogo na aba de apps");
    }

    #[test]
    fn ordem_e_por_titulo_e_nao_depende_da_leitura_do_diretorio() {
        let disco = vec![
            Detectado { titulo: "Zelda".into(), ..pacote("z", "/user/games/z", Tipo::Jogo, true) },
            Detectado { titulo: "a-máquina".into(), ..pacote("a", "/user/games/a", Tipo::Jogo, true) },
            Detectado { titulo: "Metroid".into(), ..pacote("m", "/user/games/m", Tipo::Jogo, true) },
        ];
        let juntos = juntar(vec![], disco);
        // Insensível à caixa: `a-máquina` vem antes de `Metroid`, que vem antes
        // de `Zelda` — a ordem que o usuário espera de uma lista de títulos.
        let titulos: Vec<&str> = juntos.iter().map(|d| d.titulo.as_str()).collect();
        assert_eq!(titulos, vec!["a-máquina", "Metroid", "Zelda"]);
    }

    #[test]
    fn executavel_junta_sem_barra_dupla() {
        let mut p = pacote("x", "/user/games/x/", Tipo::Jogo, true);
        assert_eq!(p.executavel(), "/user/games/x/bin/x");
        p.dir = "/user/games/x".into();
        assert_eq!(p.executavel(), "/user/games/x/bin/x");
    }
}
