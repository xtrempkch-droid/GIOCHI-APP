//! Ações que mexem no sistema inteiro: desligar e reiniciar de verdade.
//!
//! ## Por que este módulo existe
//!
//! Até aqui, "Desligar" e "Reiniciar" só trocavam a tela. O reducer do núcleo,
//! corretamente, não executa nada — ele é puro e testável sem hardware. Mas
//! alguém precisa executar, e esse alguém é a camada de UI.
//!
//! ## Processo filho, não `exec`
//!
//! O launcher **cria um processo filho** com `Command::status()` e espera por
//! ele. Não usa `exec`, porque `exec` substituiria o próprio launcher: se o
//! comando falhasse, não sobraria processo para mostrar o erro na tela.
//!
//! A contrapartida é que durante o `poweroff` existem dois processos e os dois
//! podem ter o DRM aberto. Na prática dura poucos milissegundos e o aparelho
//! desliga antes de qualquer conflito — mas está aqui registrado, porque já
//! esteve descrito ao contrário neste arquivo.
//!
//! ## Desligar corta a energia neste aparelho
//!
//! Medido no X55 em 03/11/2026: `poweroff` **desliga de fato** — o aparelho
//! precisou ser religado pelo botão físico.
//!
//! A primeira versão deste comentário afirmava o contrário, deduzindo do device
//! tree que, sem `rockchip,system-power-controller`, o hardware continuaria
//! alimentado. A propriedade de fato **não existe** e o desligamento funciona
//! assim mesmo: o driver do PMIC registra o caminho de outra forma. A conclusão
//! foi escrita a partir de documentação, sem teste, e o teste a desmentiu.
//!
//! O app faz a parte dele (chama `poweroff`) e **registra o resultado**.
//! `declaracao_de_energia()` abaixo continua existindo como informação sobre o
//! device tree — nunca como previsão do que o comando fará.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// O que o kernel vai fazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acao {
    Desligar,
    Reiniciar,
}

impl Acao {
    /// Comando que executa a ação.
    ///
    /// `poweroff` e `reboot` do BusyBox fazem a chamada de sistema certa
    /// (`reboot(2)` com o comando apropriado); não é preciso `shutdown`.
    pub fn comando(self) -> &'static str {
        match self {
            Acao::Desligar => "poweroff",
            Acao::Reiniciar => "reboot",
        }
    }
}

/// O device tree declara `system-power-controller` em algum nó?
///
/// **Correção de 03/11/2026:** a primeira versão disto afirmava que, sem essa
/// propriedade, `poweroff` não cortaria a energia. **Está errado e foi medido:**
/// no X55 a propriedade não existe e `poweroff` desliga o aparelho de verdade —
/// o usuário confirmou que precisou religar. O driver do PMIC registra o
/// caminho de desligamento de outra forma.
///
/// Então isto é **informação**, não veredito: serve para comparar aparelhos e
/// detectar mudança no device tree, não para prever o comportamento do
/// `poweroff`. Quem quer saber se desliga precisa desligar.
pub fn declaracao_de_energia() -> bool {
    declaracao_de_energia_em("/proc/device-tree")
}

fn declaracao_de_energia_em(raiz: &str) -> bool {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return false;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.join("system-power-controller").exists() {
            return true;
        }
        if caminho.is_dir() && declaracao_de_energia_em(&caminho.to_string_lossy()) {
            return true;
        }
    }
    false
}

/// Executa a ação. **Não retorna em caso de sucesso.**
///
/// Devolve a mensagem de erro quando o comando não pôde ser executado ou
/// retornou erro — o chamador mostra isso na tela, em vez de fingir que
/// desligou.
pub fn executar(acao: Acao) -> String {
    eprintln!(
        "sistema: executando `{}` ({acao:?})",
        acao.comando()
    );

    match Command::new(acao.comando()).status() {
        Ok(status) if status.success() => {
            // Normalmente não chega aqui: `poweroff` derruba o sistema antes.
            format!(
                "`{}` terminou sem desligar o aparelho",
                acao.comando()
            )
        }
        Ok(status) => format!(
            "`{}` saiu com {} — o aparelho não desligou",
            acao.comando(),
            status.code().map_or("sinal".to_string(), |c| c.to_string())
        ),
        Err(e) => format!("não consegui executar `{}`: {e}", acao.comando()),
    }
}

/// Marca que a falha já foi mostrada, para não repetir a tentativa a cada quadro.
pub struct TentativaUnica {
    ja_tentou: bool,
}

impl TentativaUnica {
    pub fn nova() -> Self {
        Self { ja_tentou: false }
    }

    /// `true` na primeira chamada, `false` depois.
    ///
    /// Sem isso, um `poweroff` que falha (por falta de
    /// `system-power-controller`, por exemplo) seria reexecutado a cada quadro,
    /// enchendo o console e gastando CPU para sempre.
    pub fn primeira_vez(&mut self) -> bool {
        if self.ja_tentou {
            false
        } else {
            self.ja_tentou = true;
            true
        }
    }

    pub fn resetar(&mut self) {
        self.ja_tentou = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comandos_corretos_por_acao() {
        assert_eq!(Acao::Desligar.comando(), "poweroff");
        assert_eq!(Acao::Reiniciar.comando(), "reboot");
    }

    #[test]
    fn tentativa_unica_so_dispara_uma_vez() {
        let mut t = TentativaUnica::nova();
        assert!(t.primeira_vez());
        assert!(!t.primeira_vez());
        assert!(!t.primeira_vez());
    }

    #[test]
    fn resetar_permite_nova_tentativa() {
        let mut t = TentativaUnica::nova();
        t.primeira_vez();
        t.resetar();
        assert!(t.primeira_vez());
    }

    #[test]
    fn detecta_a_propriedade_no_device_tree() {
        let dir = std::env::temp_dir().join(format!("giochi-dt-{}", std::process::id()));
        let pmic = dir.join("i2c").join("pmic@20");
        std::fs::create_dir_all(&pmic).unwrap();

        assert!(!declaracao_de_energia_em(&dir.to_string_lossy()));

        std::fs::write(pmic.join("system-power-controller"), []).unwrap();
        assert!(declaracao_de_energia_em(&dir.to_string_lossy()));
    }

    #[test]
    fn ausencia_da_propriedade_nao_e_veredito_sobre_desligar() {
        // O X55 desliga sem a propriedade. Este teste existe para impedir que
        // alguém reintroduza a conclusão errada de "sem DT, não desliga".
        let dir = std::env::temp_dir().join(format!("giochi-dt2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!declaracao_de_energia_em(&dir.to_string_lossy()));
        // O contrato: `executar` continua sendo a única fonte de verdade.
        assert_eq!(Acao::Desligar.comando(), "poweroff");
    }

    #[test]
    fn comando_inexistente_devolve_erro_em_vez_de_panico() {
        // `Acao` só tem comandos que existem, mas o contrato de `executar` é
        // devolver mensagem — então testamos o caminho de erro pelo ambiente.
        let msg = executar_com("comando-que-nao-existe-giochi", &[]);
        assert!(msg.contains("não consegui executar"), "veio: {msg}");
    }

    /// Igual a [`executar`], mas com o comando parametrizado, para testar o
    /// caminho de falha sem depender de privilégio ou de hardware.
    fn executar_com(programa: &str, args: &[&str]) -> String {
        match Command::new(programa).args(args).status() {
            Ok(s) if s.success() => "terminou sem efeito".to_string(),
            Ok(s) => format!("saiu com {:?}", s.code()),
            Err(e) => format!("não consegui executar `{programa}`: {e}"),
        }
    }
}

/// Resultado de uma tentativa de iniciar um app.
#[derive(Debug, PartialEq, Eq)]
pub enum Lancamento {
    /// O processo rodou e terminou. Devolve o código de saída, se houver —
    /// `None` significa morto por sinal.
    Terminou(Option<i32>),
    /// Encerrado pelo gesto de saída do launcher, não por ele mesmo.
    EncerradoAMao,
}

/// Quanto tempo o gesto de saída precisa ficar apertado.
///
/// Curto o bastante para não prender o jogador, longo o bastante para um toque
/// perdido no START não matar a partida.
pub const SAIDA_SEGURADA: Duration = Duration::from_millis(1500);

/// De quanto em quanto tempo o vigia olha o controle.
const INTERVALO_DE_VIGIA: Duration = Duration::from_millis(30);

/// Decide quando o gesto de saída já durou o bastante.
///
/// Puro, com o instante recebido de fora: a regra é uma **duração**, e testá-la
/// com `sleep` de verdade faria o teste depender do relógio da máquina — o tipo
/// de teste que passa no PC e falha no aparelho.
#[derive(Debug, Default)]
pub struct GestoDeSaida {
    desde: Option<Instant>,
}

impl GestoDeSaida {
    /// Devolve `true` quando o gesto completou [`SAIDA_SEGURADA`].
    pub fn atualizar(&mut self, apertado: bool, agora: Instant) -> bool {
        if !apertado {
            self.desde = None;
            return false;
        }
        let inicio = *self.desde.get_or_insert(agora);
        agora.saturating_duration_since(inicio) >= SAIDA_SEGURADA
    }
}

/// Onde estão os dispositivos de entrada.
///
/// `GIOCHI_INPUT_DIR` existe para o teste poder apontar um FIFO com eventos
/// `input_event` de verdade: sem isso, provar o gesto exigiria um controle e um
/// dedo, que é exatamente o que não existe numa rodada automatizada.
fn diretorio_de_entrada() -> std::path::PathBuf {
    std::env::var_os("GIOCHI_INPUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/dev/input"))
}

/// Lê o controle enquanto um jogo roda e avisa quando o gesto de saída acontece.
///
/// ## Por que existe
///
/// Um jogo que não tem como sair (o `equilibrium`/`ball_balance` da engine é o
/// caso concreto) **prende o aparelho**: o launcher está em `fork`+`wait` e o
/// único caminho seria desligar no botão. O launcher então é quem oferece a
/// saída, e não cada jogo — o defeito de um jogo não deveria custar uma recompila
/// de outro repositório.
///
/// ## Gesto: START + B
///
/// START sozinho é usado para pausar em quase todo jogo; B sozinho é o "voltar".
/// Os dois juntos por 1,5 s não é uma combinação que alguém aperte sem querer, e
/// os dois estão no `/etc/giochi/buttons.map` dos três aparelhos.
struct Vigia {
    pedido: Arc<AtomicBool>,
    parar: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Vigia {
    /// `None` quando não há mapeamento: sem mapa não há como saber qual código é
    /// START, e adivinhar seria pior que não oferecer a saída.
    fn novo(mapa: &crate::input_evdev::ButtonsMap, dir: &Path) -> Option<Self> {
        if mapa.code("start").is_none() || mapa.code("b").is_none() {
            return None;
        }
        let pedido = Arc::new(AtomicBool::new(false));
        let parar = Arc::new(AtomicBool::new(false));
        let mapa = mapa.clone();
        let dir = dir.to_path_buf();
        let pedido_da_thread = Arc::clone(&pedido);
        let parar_da_thread = Arc::clone(&parar);
        let thread = std::thread::spawn(move || {
            let mut entrada = crate::input_evdev::EvdevInput::open(&dir, mapa);
            let mut gesto = GestoDeSaida::default();
            while !parar_da_thread.load(Ordering::Relaxed) {
                entrada.poll();
                // `b` vem do quadro do núcleo (`back`), e o START direto do mapa —
                // é a mesma dupla que o jogador vê no plástico.
                let apertado = entrada.start_pressed() && entrada.frame().back;
                if gesto.atualizar(apertado, Instant::now()) {
                    pedido_da_thread.store(true, Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(INTERVALO_DE_VIGIA);
            }
        });
        Some(Self {
            pedido,
            parar,
            thread: Some(thread),
        })
    }

    fn pediu_saida(&self) -> bool {
        self.pedido.load(Ordering::Relaxed)
    }
}

impl Drop for Vigia {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // A thread dorme no máximo `INTERVALO_DE_VIGIA`: juntar-se a ela é
            // limitado no tempo, e deixá-la solta manteria os descritores de
            // `/dev/input` abertos enquanto o launcher desenha.
            let _ = thread.join();
        }
    }
}

/// Executa o app e espera.
///
/// ## Por que filho, e não `exec`
///
/// `docs/integracao.md` §4 decide por `exec` com o `giochi-init` reiniciando o
/// launcher, e essa continua sendo a decisão **do produto**. Ela não pode ser
/// usada aqui ainda por um motivo medido: o `/etc/inittab` do X35H e do X55
/// **não tem** a linha `::respawn:` do `giochi-init`. Com `exec`, o jogo sairia
/// para uma tela morta, sem ninguém para subir a interface de volta.
///
/// Enquanto o init não existir, o filho é o que dá o mesmo efeito observável:
/// jogar, sair e voltar para o launcher. Quando a linha do init entrar na
/// imagem, trocar isto por `exec` é uma mudança local — o contrato de caminho e
/// ambiente abaixo já é o mesmo.
///
/// ## O display
///
/// Quem chama **precisa** ter destruído o `Painter` antes: com o launcher vivo
/// segurando o DRM master, o filho recebe `kmsdrm not available` (medido). Esta
/// função não tem como liberar o display por conta própria.
/// Trocar `.status()` por `.spawn()` + `try_wait()` é o que permite vigiar o
/// controle enquanto o jogo roda: `.status()` bloqueia e nada mais executa
/// nesse processo — e é justamente nesse intervalo que o gesto de saída
/// precisa ser lido.
pub fn lancar_com_saida(
    alvo: &crate::catalogo::Alvo,
    mapa: Option<&crate::input_evdev::ButtonsMap>,
) -> Result<Lancamento, String> {
    let executavel = alvo.executavel();
    eprintln!("sistema: iniciando {} ({})", alvo.slug, executavel);

    let mut filho = Command::new(&executavel)
        // `cwd` = diretório do app, para ele achar os próprios `assets/`
        // sem precisar de caminho absoluto no código.
        .current_dir(&alvo.dir)
        // O `giochi-init` inicia o launcher sem variáveis. `GIOCHI_APP_ID`
        // permite ao app saber quem é sem passar argumento — o contrato
        // (`docs/integracao.md` §4) diz que `argv` é só o próprio caminho.
        .env("GIOCHI_APP_ID", &alvo.slug)
        .spawn()
        .map_err(|e| format!("não consegui iniciar {}: {e}", alvo.slug))?;

    let vigia = mapa.and_then(|mapa| Vigia::novo(mapa, &diretorio_de_entrada()));
    if vigia.is_none() {
        eprintln!(
            "sistema: sem mapa de botões para START+B — saída de emergência indisponível"
        );
    }

    let codigo = loop {
        match filho.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(e) => return Err(format!("não consegui esperar {}: {e}", alvo.slug)),
        }
        if vigia.as_ref().is_some_and(Vigia::pediu_saida) {
            // `SIGKILL`, não `SIGTERM`: o SDL instala um handler de SIGTERM que
            // só posta `SDL_QUIT` na fila, e um jogo que não bombeia eventos
            // segue vivo segurando o KMSDRM. Já aconteceu com o launcher-teste.
            let _ = filho.kill();
            let _ = filho.wait();
            eprintln!(
                "sistema: saída de emergência — {} encerrado à mão (START+B por {:.1} s)",
                alvo.slug,
                SAIDA_SEGURADA.as_secs_f32()
            );
            drop(vigia);
            return Ok(Lancamento::EncerradoAMao);
        }
        std::thread::sleep(INTERVALO_DE_VIGIA);
    };

    eprintln!(
        "sistema: {} terminou ({})",
        alvo.slug,
        codigo.map_or("sinal".to_string(), |c| format!("código {c}"))
    );
    drop(vigia);
    Ok(Lancamento::Terminou(codigo))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_gesto_so_dispara_depois_da_duracao() {
        let mut gesto = GestoDeSaida::default();
        let t0 = Instant::now();
        assert!(!gesto.atualizar(true, t0), "disparou no primeiro quadro");
        assert!(!gesto.atualizar(
            true,
            t0 + SAIDA_SEGURADA - Duration::from_millis(1)
        ));
        assert!(
            gesto.atualizar(true, t0 + SAIDA_SEGURADA + Duration::from_millis(1)),
            "1,5 s apertado tem de encerrar o jogo"
        );
    }

    /// Escreve um `input_event` de botão no FIFO.
    ///
    /// 24 bytes: `timeval` (16) + `type` (u16) + `code` (u16) + `value` (i32), que é
    /// o layout de `struct input_event` nos dois alvos (x86-64 e aarch64).
    fn evento_de_botao(saida: &mut std::fs::File, code: u16, valor: i32) {
        use std::io::Write;
        let mut bytes = [0u8; 24];
        bytes[16..18].copy_from_slice(&1u16.to_ne_bytes());
        bytes[18..20].copy_from_slice(&code.to_ne_bytes());
        bytes[20..24].copy_from_slice(&valor.to_ne_bytes());
        saida.write_all(&bytes).expect("escrever evento");
        saida.flush().expect("descarregar evento");
    }

    #[test]
    fn start_sozinho_nao_encerra_e_start_mais_b_encerra() {
        // A cadeia inteira: FIFO -> threads de leitura -> `ButtonsMap` -> estado
        // -> gesto -> pedido. É o que permite afirmar que o gesto funciona sem
        // apertar botão nenhum, no aparelho que não tem `/dev/uinput`.
        let dir = std::env::temp_dir().join(format!("giochi-vigia-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar dir");
        let fifo = dir.join("event0");
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo");
        assert!(status.success(), "mkfifo falhou");

        let mapa = crate::input_evdev::ButtonsMap::parse(
            "a = BTN_EAST\nb = BTN_SOUTH\nstart = BTN_START\n",
        );
        let start = mapa.code("start").expect("start no mapa");
        let b = mapa.code("b").expect("b no mapa");

        let vigia = Vigia::novo(&mapa, &dir).expect("vigia");
        // O escritor abre depois do leitor, senão o `open` do FIFO bloqueia.
        let mut saida = std::fs::OpenOptions::new()
            .write(true)
            .open(&fifo)
            .expect("abrir o fifo para escrita");

        evento_de_botao(&mut saida, start, 1);
        std::thread::sleep(Duration::from_millis(1800));
        assert!(
            !vigia.pediu_saida(),
            "START sozinho encerrou o jogo: é a tecla de pausa de quase todo jogo"
        );

        evento_de_botao(&mut saida, b, 1);
        std::thread::sleep(Duration::from_millis(2000));
        assert!(
            vigia.pediu_saida(),
            "START+B por mais de 1,5 s não pediu a saída"
        );

        drop(vigia);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn soltar_reinicia_a_contagem() {
        // Um toque perdido no START não pode acumular com um segundo toque dez
        // segundos depois: o gesto é **segurado**, não "somado".
        let mut gesto = GestoDeSaida::default();
        let t0 = Instant::now();
        assert!(!gesto.atualizar(true, t0));
        assert!(!gesto.atualizar(false, t0 + Duration::from_millis(1200)));
        assert!(!gesto.atualizar(
            true,
            t0 + Duration::from_millis(1300)
        ));
        assert!(
            !gesto.atualizar(true, t0 + Duration::from_millis(1900)),
            "somou o toque anterior: 600 ms não é 1,5 s"
        );
        assert!(gesto.atualizar(true, t0 + Duration::from_millis(3000)));
    }
}
