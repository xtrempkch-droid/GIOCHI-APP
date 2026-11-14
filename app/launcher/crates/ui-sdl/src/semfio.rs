//! Wi-Fi e Bluetooth de verdade: `wpa_cli` e `bluetoothctl`.
//!
//! ## Por que uma thread
//!
//! Uma varredura de Wi-Fi leva segundos e a de Bluetooth é uma janela fixa de
//! 8 s. Rodar isso no laço de desenho congelaria a interface — e a tela que
//! deveria mostrar "procurando…" ficaria parada justamente nesse período.
//!
//! Então o padrão é o mesmo da entrada por evdev: a UI **pede** por um canal e
//! recebe `Cmd` de volta por outro. O núcleo continua puro; quem fala com o
//! sistema é esta camada.
//!
//! ## O que foi medido no X55 (06/11/2026)
//!
//! - `wpa_supplicant` (2.10) roda no boot e o socket `/var/run/wpa_supplicant/wlan0`
//!   existe, mas a interface sobe **DOWN** — o `S40wifi` do sistema não a levanta
//!   quando o `wpa_supplicant.conf` está vazio. Então é este módulo quem chama
//!   `ip link set up` antes de varrer.
//! - `bluetoothctl` 5.72, controlador presente (`Powered: yes`, `Pairable: no`).
//! - **Não existe `timeout` na imagem**. Por isso as janelas usam o `--timeout`
//!   do próprio `bluetoothctl` e uma espera fixa no `wpa_cli`, nunca o utilitário
//!   externo — um comando que não existe falha em silêncio se alguém não olhar.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use giochi_launcher_core::{Cmd, DispositivoBt, Rede};

/// Pedido da UI para o mundo lá fora.
#[derive(Clone, Debug)]
pub enum Pedido {
    EscanearWifi,
    /// `senha` vazia = rede aberta ou já salva.
    Conectar { ssid: String, senha: String },
    EscanearBt,
    Parear { mac: String },
}

/// Janela de varredura do Wi-Fi.
///
/// `wpa_cli scan` responde `OK` antes de ter resultado, e não há evento de
/// "terminou". Quatro segundos é o tempo típico de um `ap_scan=1` com poucas
/// redes; esperar mais só piora a sensação de travado.
const ESPERA_VARREDURA_MS: u64 = 4_000;

/// Janela de descoberta do Bluetooth, em segundos.
const JANELA_BT_S: u32 = 8;

/// Janela de "acordar" o aparelho imediatamente antes de parear.
///
/// Existe porque `pair` sem descoberta recente resulta em page timeout
/// (`ConnectionAttemptFailed`) mesmo com o controle a um metro — ele só responde
/// enquanto está anunciando. Curta de propósito: é um empurrão, não uma varredura.
const JANELA_ACORDAR_S: u32 = 5;

/// Janela do próprio `pair`, em segundos (aceito como texto pelo `bluetoothctl`).
const JANELA_PAREAR_S: &str = "25";

pub struct SemFio {
    pedidos: Sender<Pedido>,
    respostas: Receiver<Cmd>,
    tem_wifi: bool,
    tem_bt: bool,
}

impl SemFio {
    pub fn novo() -> Self {
        let (tx_pedido, rx_pedido) = channel::<Pedido>();
        let (tx_cmd, rx_cmd) = channel::<Cmd>();
        let iface = interface_wlan();

        // O nome da interface vai para o log na abertura: se o aparelho não
        // tiver rádio, "nenhuma rede" e "não há interface" têm causas
        // diferentes, e só a linha de log separa as duas.
        // A frase muda quando **não há** rádio: num aparelho sem nenhum (R35S)
        // dizer "interface ausente" soa a defeito, e não é — é o hardware.
        eprintln!(
            "sem fio: wi-fi {}, bluetooth {}",
            iface.as_deref().unwrap_or("ausente"),
            if tem_bluetooth() { "disponível" } else { "não disponível" }
        );

        let tem_wifi = iface.is_some();
        let tem_bt = tem_bluetooth();
        let iface_thread = iface;
        thread::Builder::new()
            .name("giochi-semfio".into())
            .spawn(move || trabalhador(rx_pedido, tx_cmd, iface_thread))
            .ok();

        Self {
            pedidos: tx_pedido,
            respostas: rx_cmd,
            tem_wifi,
            tem_bt,
        }
    }

    /// O que este aparelho tem de rádio, para o núcleo guardar como fato.
    pub fn capacidades(&self) -> Cmd {
        Cmd::CapacidadesSemFio {
            wifi: self.tem_wifi,
            bluetooth: self.tem_bt,
        }
    }

    pub fn pedir(&self, pedido: Pedido) {
        // Se a thread não subiu, o pedido some — mas o overlay já mostrou
        // "procurando…" antes, então o usuário vê que nada voltou em vez de
        // ver uma lista vazia que parece "nenhuma rede".
        if self.pedidos.send(pedido).is_err() {
            eprintln!("aviso: thread de wi-fi/bluetooth não está viva");
        }
    }

    /// Comandos que chegaram desde o último quadro.
    pub fn colher(&self) -> Vec<Cmd> {
        self.respostas.try_iter().collect()
    }
}

fn trabalhador(pedidos: Receiver<Pedido>, respostas: Sender<Cmd>, iface: Option<String>) {
    for pedido in pedidos {
        let saida = match pedido {
            Pedido::EscanearWifi => {
                let _ = respostas.send(Cmd::EscaneandoWifi(true));
                escanear_wifi(iface.as_deref())
            }
            Pedido::Conectar { ssid, senha } => conectar(iface.as_deref(), &ssid, &senha),
            Pedido::EscanearBt => {
                let _ = respostas.send(Cmd::EscaneandoBt(true));
                escanear_bt()
            }
            Pedido::Parear { mac } => parear(&mac),
        };
        if respostas.send(saida).is_err() {
            return; // a janela fechou
        }
    }
}

// ── Wi-Fi ──────────────────────────────────────────────────

/// Existe controlador Bluetooth neste aparelho?
///
/// Dois sinais, nesta ordem: adaptador `hci*` em `/sys/class/bluetooth` (prova de
/// hardware) e o utilitário presente (prova de que dá para operar). Um aparelho
/// sem adaptador — o R35S — falha no primeiro e a tela diz "não tem rádio" em vez
/// de "procurando…".
fn tem_bluetooth() -> bool {
    let tem_adapter = std::fs::read_dir("/sys/class/bluetooth")
        .map(|d| d.filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with("hci")))
        .unwrap_or(false);
    tem_adapter && programa("bluetoothctl").exists()
}

/// Caminho de um utilitário, com os diretórios padrão como reserva.
///
/// No X55 os três vivem fora do PATH comum: `wpa_cli` em `/usr/sbin`, `ip` em
/// `/sbin`. O PATH do `giochi-init` cobre os dois (medido:
/// `/bin:/sbin:/usr/bin:/usr/sbin`), mas o launcher também é iniciado à mão por
/// SSH durante o diagnóstico, e aí o PATH é outro. Procurar nos diretórios
/// padrão custa uma chamada e evita um "não existe nesta imagem" que seria falso.
fn programa(nome: &str) -> PathBuf {
    for dir in ["/usr/sbin", "/sbin", "/usr/bin", "/bin"] {
        let caminho = Path::new(dir).join(nome);
        if caminho.exists() {
            return caminho;
        }
    }
    PathBuf::from(nome)
}

/// Primeira interface `wl*` do sistema.
///
/// Descoberta em vez de fixa: o mesmo binário roda em aparelhos diferentes, e
/// um `wlan0` escrito à mão é a classe de bug que só aparece no segundo modelo.
fn interface_wlan() -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/net").ok()?;
    let mut nomes: Vec<String> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("wl"))
        .collect();
    nomes.sort();
    nomes.into_iter().next()
}

fn wpa(iface: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(programa("wpa_cli"));
    cmd.arg("-i").arg(iface);
    cmd.args(args);
    let saida = cmd.output().map_err(|e| format!("wpa_cli: {e}"))?;
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    if texto.contains("Failed to connect") {
        return Err("wpa_supplicant não está respondendo".into());
    }
    Ok(texto)
}

/// Sobe a interface se estiver parada.
///
/// Medido: no X55 ela nasce DOWN e o `wpa_cli scan` responde `FAIL-BUSY` com o
/// rádio desligado — o sintoma é "não acha nenhuma rede", não "erro".
fn ligar_interface(iface: &str) {
    let estado = std::fs::read_to_string(format!("/sys/class/net/{iface}/operstate"))
        .unwrap_or_default();
    if estado.trim() == "up" {
        return;
    }
    match Command::new(programa("ip")).args(["link", "set", iface, "up"]).output() {
        Ok(s) if s.status.success() => eprintln!("wi-fi: interface {iface} levantada"),
        Ok(s) => eprintln!(
            "aviso: ip link set {iface} up falhou ({})",
            String::from_utf8_lossy(&s.stderr).trim()
        ),
        Err(e) => eprintln!("aviso: ip indisponível ({e})"),
    }
}

fn escanear_wifi(iface: Option<&str>) -> Cmd {
    let Some(iface) = iface else {
        return Cmd::RedesWifi(Vec::new(), None).com_erro("nenhuma interface sem fio");
    };
    ligar_interface(iface);

    // `FAIL-BUSY` só quer dizer que já havia varredura em curso — não é erro.
    if let Err(e) = wpa(iface, &["scan"]) {
        return Cmd::RedesWifi(Vec::new(), None).com_erro(&e);
    }
    thread::sleep(std::time::Duration::from_millis(ESPERA_VARREDURA_MS));

    let brutos = match wpa(iface, &["scan_results"]) {
        Ok(t) => t,
        Err(e) => return Cmd::RedesWifi(Vec::new(), None).com_erro(&e),
    };
    let salvas = wpa(iface, &["list_networks"]).unwrap_or_default();
    let conectado = wpa(iface, &["status"])
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("ssid=").map(|s| s.to_string()))
        .filter(|s| !s.is_empty());

    let redes = juntar_redes(&brutos, &salvas, conectado.as_deref());
    eprintln!(
        "wi-fi: {} rede(s) em {iface}{}",
        redes.len(),
        match &conectado {
            Some(s) => format!(", conectado em {s}"),
            None => String::new(),
        }
    );
    Cmd::RedesWifi(redes, conectado)
}

/// Converte o dBm do `wpa_cli` em 0..=100.
///
/// O `scan_results` informa potência em dBm (negativo, perto de −30 é ótimo e
/// −90 é o limite do inútil). A UI guarda porcentagem para poder desenhar sem
/// saber de rádio; a conversão é `2·(dBm + 100)`, a mesma dos clientes
/// usuais — −50 dBm vira 100 e −100 dBm vira 0.
fn forca_do_sinal(dbm: &str) -> u8 {
    let dbm: i32 = dbm.parse().unwrap_or(-100);
    (2 * (dbm + 100)).clamp(0, 100) as u8
}

/// Transforma a saída de `scan_results` em lista única por SSID.
///
/// Regras que o formato bruto exige:
/// - as colunas são separadas por **tabulação**, e o SSID pode conter espaço;
/// - redes ocultas vêm com SSID vazio — não há como escolhê-las pelo nome;
/// - o mesmo SSID aparece uma vez por BSSID (2,4 e 5 GHz, repetidores), então
///   a lista teria a mesma rede quatro vezes. Fica a de sinal melhor.
fn juntar_redes(bruto: &str, salvas: &str, conectado: Option<&str>) -> Vec<Rede> {
    let nomes_salvos: Vec<String> = salvas
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').nth(1))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut redes: Vec<Rede> = Vec::new();
    for linha in bruto.lines().skip(1) {
        let cols: Vec<&str> = linha.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let ssid = cols[4].trim();
        if ssid.is_empty() {
            continue;
        }
        let sinal = forca_do_sinal(cols[2].trim());
        let flags = cols[3];
        let protegida = ["PSK", "SAE", "EAP", "WEP", "IEEE8021X"]
            .iter()
            .any(|m| flags.contains(m));

        match redes.iter_mut().find(|r| r.ssid == ssid) {
            Some(existente) => {
                if sinal > existente.sinal {
                    existente.sinal = sinal;
                    existente.protegida = protegida;
                }
            }
            None => redes.push(Rede {
                ssid: ssid.to_string(),
                sinal,
                protegida,
                salva: nomes_salvos.iter().any(|n| n == ssid),
            }),
        }
    }

    // Sinal melhor primeiro, como qualquer lista de redes faz.
    redes.sort_by(|a, b| b.sinal.cmp(&a.sinal));
    // A conectada no topo, mesmo com sinal pior: é a que o usuário quer ver.
    if let Some(atual) = conectado {
        if let Some(pos) = redes.iter().position(|r| r.ssid == atual) {
            redes.swap(0, pos);
        }
    }
    redes
}

fn conectar(iface: Option<&str>, ssid: &str, senha: &str) -> Cmd {
    let Some(iface) = iface else {
        return Cmd::ResultadoWifi(Some("nenhuma interface sem fio".into()));
    };
    ligar_interface(iface);

    // Rede já salva: basta selecionar. Reescrever a senha a cada conexão
    // encheria o `wpa_supplicant.conf` de entradas repetidas.
    let salvas = wpa(iface, &["list_networks"]).unwrap_or_default();
    if let Some(id) = salvas
        .lines()
        .skip(1)
        .find_map(|l| {
            let mut c = l.split('\t');
            let id = c.next()?;
            let nome = c.next()?;
            (nome == ssid).then(|| id.to_string())
        })
    {
        return match wpa(iface, &["select_network", &id]) {
            Ok(t) if t.contains("OK") => {
                eprintln!("wi-fi: selecionada a rede salva {ssid} (id {id})");
                Cmd::ResultadoWifi(None)
            }
            Ok(t) => Cmd::ResultadoWifi(Some(limpar(&t))),
            Err(e) => Cmd::ResultadoWifi(Some(e)),
        };
    }

    let passo = |args: &[&str]| -> Result<(), String> {
        match wpa(iface, args) {
            Ok(t) if t.contains("OK") || t.trim().parse::<usize>().is_ok() => Ok(()),
            Ok(t) => Err(format!("{}: {}", args[0], limpar(&t))),
            Err(e) => Err(e),
        }
    };

    if let Err(e) = passo(&["add_network"]) {
        return Cmd::ResultadoWifi(Some(e));
    }
    // `add_network` devolve o id como número; descobrimos qual é pela lista.
    let id = match wpa(iface, &["list_networks"])
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').next())
        .filter_map(|s| s.parse::<usize>().ok())
        .max()
    {
        Some(id) => id.to_string(),
        None => return Cmd::ResultadoWifi(Some("wpa_cli não criou a rede".into())),
    };

    let aspas = |s: &str| format!("\"{s}\"");
    let id_ssid = aspas(ssid);
    let mut definicoes: Vec<Vec<String>> = vec![vec!["set_network".into(), id.clone(), "ssid".into(), id_ssid]];
    if senha.is_empty() {
        // Rede aberta: sem `psk`, com `key_mgmt=NONE`. Mandar psk vazio produz
        // uma entrada que o supplicant aceita e nunca conecta.
        definicoes.push(vec!["set_network".into(), id.clone(), "key_mgmt".into(), "NONE".into()]);
    } else {
        let psk = aspas(senha);
        definicoes.push(vec!["set_network".into(), id.clone(), "psk".into(), psk]);
    }
    definicoes.push(vec!["enable_network".into(), id.clone()]);
    definicoes.push(vec!["select_network".into(), id.clone()]);

    for args in definicoes {
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        if let Err(e) = passo(&refs) {
            return Cmd::ResultadoWifi(Some(e));
        }
    }

    // Esperar a associação é o que separa "os comandos aceitaram" de
    // "conectou". Sem esta espera uma senha errada devolveria sucesso: o
    // `select_network` responde OK e a falha só aparece segundos depois, em
    // silêncio, sem nada na tela.
    let estado = aguardar_conexao(iface);
    if estado != "COMPLETED" {
        return Cmd::ResultadoWifi(Some(descrever_falha(&estado)));
    }

    // Só grava depois de dar certo. Salvar uma senha errada faria o aparelho
    // tentar sozinho no próximo boot e falhar sem ninguém entender por quê.
    match wpa(iface, &["save_config"]) {
        Ok(t) if t.contains("OK") => eprintln!("wi-fi: configuração salva para {ssid}"),
        Ok(t) => eprintln!("aviso: save_config respondeu {}", limpar(&t)),
        Err(e) => eprintln!("aviso: não salvou a rede ({e})"),
    }
    Cmd::ResultadoWifi(None)
}

/// Espera a associação terminar e devolve o `wpa_state` final.
///
/// Doze segundos em passos de meio: uma rede boa completa em 1–3 s; uma senha
/// errada fica em `4-WAY_HANDSHAKE` ou volta a `DISCONNECTED` e não muda mais.
/// Esperar mais só faria o usuário olhar "conectando…" sem motivo.
fn aguardar_conexao(iface: &str) -> String {
    let mut ultimo = String::new();
    for _ in 0..24 {
        thread::sleep(std::time::Duration::from_millis(500));
        let status = wpa(iface, &["status"]).unwrap_or_default();
        let estado = status
            .lines()
            .find_map(|l| l.strip_prefix("wpa_state="))
            .unwrap_or("")
            .trim()
            .to_string();
        if estado.is_empty() {
            continue;
        }
        ultimo = estado.clone();
        if estado == "COMPLETED" || estado == "DISCONNECTED" {
            return estado;
        }
        // `4-WAY_HANDSHAKE` é onde uma senha errada costuma parar.
        if estado == "4-WAY_HANDSHAKE" {
            thread::sleep(std::time::Duration::from_millis(2_000));
            let status = wpa(iface, &["status"]).unwrap_or_default();
            if !status.contains("wpa_state=COMPLETED") {
                return "4-WAY_HANDSHAKE".into();
            }
            return "COMPLETED".into();
        }
    }
    // O último estado vale mais que "deu errado": medido no X55 com senha
    // errada, ele fica em `SCANNING` porque o supplicant desiste da rede e
    // volta a varrer. Sem este texto o diagnóstico começa do zero.
    if ultimo.is_empty() {
        "sem resposta".into()
    } else {
        ultimo
    }
}

/// Traduz o estado do supplicant em algo que o usuário possa usar.
///
/// "Falhou" não ajuda: `4-WAY_HANDSHAKE` é senha errada, `SCANNING` é a rede
/// fora de alcance depois de aceitar a senha. São ações diferentes.
fn descrever_falha(estado: &str) -> String {
    match estado {
        "4-WAY_HANDSHAKE" => "senha recusada pela rede".into(),
        "DISCONNECTED" => "não conectou: rede fora de alcance?".into(),
        "SCANNING" => "a rede não respondeu (senha errada ou fora de alcance)".into(),
        "ASSOCIATING" | "ASSOCIATED" => "associou mas não autenticou (senha?)".into(),
        "sem resposta" => "o supplicant não respondeu".into(),
        outro if outro.is_empty() => "não conectou".into(),
        outro => format!("não conectou ({outro})"),
    }
}

/// Primeira linha não vazia, sem os `FAIL`/`OK` soltos do `wpa_cli`.
fn limpar(saida: &str) -> String {
    saida
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("falhou")
        .to_string()
}

// ── Bluetooth ──────────────────────────────────────────────

fn bt(args: &[&str]) -> Result<String, String> {
    match Command::new(programa("bluetoothctl")).args(args).output() {
        Ok(s) => Ok(String::from_utf8_lossy(&s.stdout).into_owned()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err("bluetoothctl não existe nesta imagem".into())
        }
        Err(e) => Err(format!("bluetoothctl: {e}")),
    }
}

/// Lista os aparelhos conhecidos, marcando pareados e conectados.
fn listar_bt() -> Vec<DispositivoBt> {
    let Ok(texto) = bt(&["devices"]) else {
        return Vec::new();
    };
    texto
        .lines()
        .filter_map(|l| {
            // "Device AA:BB:CC:DD:EE:FF nome"
            let resto = l.strip_prefix("Device ")?;
            let (mac, nome) = resto.split_once(' ').unwrap_or((resto, ""));
            let mac = mac.trim();
            if mac.is_empty() {
                return None;
            }
            // Sem nome anunciado, o `bluetoothctl` repete o MAC com hífens.
            // Mostrar isso como se fosse nome confunde: vira "aparelho" mesmo.
            let nome = nome.trim();
            let nome = if nome.is_empty() || nome.replace('-', ":").eq_ignore_ascii_case(mac) {
                "aparelho sem nome".to_string()
            } else {
                nome.to_string()
            };
            let info = bt(&["info", mac]).unwrap_or_default();
            Some(DispositivoBt {
                mac: mac.to_string(),
                nome,
                pareado: info.contains("Paired: yes"),
            })
        })
        .collect()
}

fn escanear_bt() -> Cmd {
    if let Err(e) = bt(&["power", "on"]) {
        return Cmd::DispositivosBt(Vec::new(), None).com_erro(&e);
    }
    // `Pairable: no` é o padrão do controlador. Sem isto o pedido de pareamento
    // é recusado pelo próprio aparelho, e a mensagem que chega é genérica.
    let _ = bt(&["pairable", "on"]);
    let _ = bt(&["agent", "KeyboardDisplay"]);

    // `--timeout` é do bluetoothctl, não do sistema: a imagem **não tem** o
    // utilitário `timeout` (medido), e depender dele daria um comando que
    // falha sem avisar.
    let janela = JANELA_BT_S.to_string();
    let _ = Command::new(programa("bluetoothctl"))
        .args(["--timeout", &janela, "scan", "on"])
        .output();

    let dispositivos = listar_bt();
    eprintln!("bluetooth: {} aparelho(s) conhecido(s)", dispositivos.len());
    Cmd::DispositivosBt(dispositivos, None)
}

fn parear(mac: &str) -> Cmd {
    // ── 1. acordar o aparelho ────────────────────────────────────────────
    //
    // Medido no X55 (13/11/2026) com um "Pro Controller" já descoberto:
    //
    //     bluetoothctl --timeout 20 pair 98:B6:E9:9C:46:65
    //     connect failed (status 0x04, Connect Failed)
    //     Failed to pair: org.bluez.Error.ConnectionAttemptFailed
    //
    // `0x04` é **page timeout**: o controle não respondeu. Ele só responde
    // enquanto está anunciando, e a varredura do launcher acabou minutos antes.
    // Um `pair` sem descoberta recente, portanto, falha por motivo que não é o
    // pareamento — e a mensagem manda o usuário procurar defeito no app.
    //
    // Uma janela curta de descoberta imediatamente antes resolve: o aparelho
    // volta a anunciar (é o que o modo de pareamento faz) e o `pair` o alcança.
    let _ = bt(&["power", "on"]);
    let _ = bt(&["pairable", "on"]);
    let janela = JANELA_ACORDAR_S.to_string();
    let _ = Command::new(programa("bluetoothctl"))
        .args(["--timeout", &janela, "scan", "on"])
        .output();

    // ── 2. parear com agente ─────────────────────────────────────────────
    //
    // `--agent` explícito, e não `agent` numa chamada anterior: cada
    // `bluetoothctl` é um processo, e o agente registrado num morre com ele. Um
    // controle é `NoInputNoOutput` (não tem teclado nem tela para confirmar
    // código) — é o Just Works, que é o que ele espera.
    let saida = Command::new(programa("bluetoothctl"))
        .args([
            "--agent",
            "NoInputNoOutput",
            "--timeout",
            JANELA_PAREAR_S,
            "pair",
            mac,
        ])
        .output();
    let texto = match saida {
        Ok(s) => String::from_utf8_lossy(&s.stdout).into_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Cmd::ResultadoBt(Some("bluetoothctl não existe nesta imagem".into()))
        }
        Err(e) => return Cmd::ResultadoBt(Some(format!("bluetoothctl: {e}"))),
    };

    // ── 3. o veredito é o estado, não o texto ────────────────────────────
    //
    // `Pairing successful` já apareceu para aparelho que voltou a `Paired: no`
    // logo depois (o controle cai e o BlueZ desfaz). Quem decide é o `info`.
    let info = bt(&["info", mac]).unwrap_or_default();
    let pareado = info.contains("Paired: yes");

    if pareado || texto.contains("AlreadyExists") {
        // Parear sem confiar exige autorizar de novo a cada conexão; e sem
        // conectar o aparelho fica pareado e inútil.
        let _ = bt(&["trust", mac]);
        if !info.contains("Connected: yes") {
            let _ = bt(&["connect", mac]);
        }
        eprintln!("bluetooth: {mac} pareado e confiado");
        return Cmd::ResultadoBt(None);
    }

    Cmd::ResultadoBt(Some(motivo_do_pareamento(&texto, &info)))
}

/// Traduz a falha do BlueZ em ação para o usuário.
///
/// As três falhas pedem coisas diferentes, e a mensagem crua (`Failed to pair:
/// org.bluez.Error.ConnectionAttemptFailed`) não diz nenhuma delas.
fn motivo_do_pareamento(texto: &str, info: &str) -> String {
    if info.is_empty() {
        return "o aparelho não está mais visível: aproxime e procure de novo".into();
    }
    if texto.contains("ConnectionAttemptFailed") || texto.contains("status 0x04") {
        return "o controle não respondeu: coloque-o em modo de pareamento \
                (segure o botão de sync/par) e tente de novo"
            .into();
    }
    if texto.contains("AuthenticationFailed") || texto.contains("AuthenticationRejected") {
        return "o controle recusou a autenticação: apague-o no controle e pareie de novo".into();
    }
    if texto.contains("AlreadyExists") {
        return "já existe um pareamento com este aparelho; remova e tente de novo".into();
    }
    if texto.contains("NoAgentAvailable") || texto.contains("No agent") {
        return "o BlueZ não tinha agente para confirmar o pareamento".into();
    }
    texto
        .lines()
        .map(|l| l.trim())
        .find(|l| l.contains("Failed") || l.contains("not available"))
        .map(|l| l.to_string())
        .unwrap_or_else(|| "pareamento recusado".into())
}

/// Anexa a mensagem de erro ao comando.
///
/// O erro viaja **no** `Cmd` de resultado para haver uma única rota: se ele
/// fosse um comando separado, a lista e o erro poderiam chegar em ordem
/// trocada e a tela mostraria "nenhuma rede" com um erro pendente.
trait ComErro {
    fn com_erro(self, motivo: &str) -> Cmd;
}

impl ComErro for Cmd {
    fn com_erro(self, motivo: &str) -> Cmd {
        eprintln!("aviso: {motivo}");
        match self {
            Cmd::RedesWifi(redes, _) => Cmd::RedesWifi(redes, Some(motivo.to_string())),
            Cmd::DispositivosBt(d, _) => Cmd::DispositivosBt(d, Some(motivo.to_string())),
            outro => outro,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A falha de page timeout é a que aconteceu no X55 com um controle
    /// **já descoberto**: `pair` sem descoberta recente.
    #[test]
    fn page_timeout_vira_instrucao_e_nao_codigo_do_bluez() {
        let motivo = motivo_do_pareamento(
            "Attempting to pair with 98:B6:E9:9C:46:65\n\
             hci0 98:B6:E9:9C:46:65 type BR/EDR connect failed (status 0x04, Connect Failed)\n\
             Failed to pair: org.bluez.Error.ConnectionAttemptFailed",
            "Device 98:B6:E9:9C:46:65\n  Name: Pro Controller\n  Paired: no",
        );
        assert!(motivo.contains("modo de pareamento"), "{motivo}");
        assert!(!motivo.contains("org.bluez"), "código cru não é instrução: {motivo}");
    }

    #[test]
    fn aparelho_fora_de_alcance_pede_procurar_de_novo() {
        let motivo = motivo_do_pareamento("Failed to pair: not available", "");
        assert!(motivo.contains("não está mais visível"), "{motivo}");
    }

    #[test]
    fn autenticacao_recusada_pede_apagar_no_controle() {
        let motivo = motivo_do_pareamento(
            "Failed to pair: org.bluez.Error.AuthenticationFailed",
            "Device X\n  Paired: no",
        );
        assert!(motivo.contains("apague-o no controle"), "{motivo}");
    }

    /// Sem agente o BlueZ responde `NoAgentAvailable` — e a ação é outra.
    #[test]
    fn falta_de_agente_e_diagnosticada_como_tal() {
        let motivo = motivo_do_pareamento(
            "Failed to pair: org.bluez.Error.NoAgentAvailable",
            "Device X\n  Paired: no",
        );
        assert!(motivo.contains("agente"), "{motivo}");
    }

    /// O que não é reconhecido tem de chegar ao usuário, não sumir.
    #[test]
    fn falha_desconhecida_vira_o_texto_do_bluez() {
        let motivo = motivo_do_pareamento(
            "Failed to pair: org.bluez.Error.InProgress",
            "Device X\n  Paired: no",
        );
        assert!(motivo.contains("InProgress"), "{motivo}");
    }

    const BRUTO: &str = "bssid / frequency / signal level / flags / ssid\n\
e4:be:ed:3e:88:16\t2462\t-70\t[WPA-PSK-CCMP+TKIP][WPA2-PSK-CCMP+TKIP][ESS]\tORLEANS\n\
24:2f:d0:ca:0f:92\t5220\t-23\t[WPA2-PSK-CCMP][ESS]\tKill.exe\n\
24:2f:d0:ca:0f:90\t2427\t-28\t[WPA2-PSK-CCMP][ESS]\tKill.exe\n\
26:2f:d0:4a:0f:90\t2427\t-23\t[WPA2-PSK-CCMP][ESS]\t\n\
aa:bb:cc:dd:ee:ff\t2412\t-55\t[ESS]\tCafe Aberto\n";

    const SALVAS: &str = "network id / ssid / bssid / flags\n0\t\tany\t\n1\tKill.exe\tany\t[DISABLED]\n";

    #[test]
    fn rede_repetida_fica_uma_vez_com_melhor_sinal() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        let kill: Vec<_> = redes.iter().filter(|r| r.ssid == "Kill.exe").collect();
        assert_eq!(kill.len(), 1, "2,4 e 5 GHz são a mesma rede");
        assert_eq!(kill[0].sinal, 100, "-23 dBm é o melhor dos dois");
    }

    #[test]
    fn rede_oculta_nao_entra_na_lista() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        assert!(redes.iter().all(|r| !r.ssid.is_empty()));
        assert!(!redes.iter().any(|r| r.ssid == "26:2f:d0:4a:0f:90"));
    }

    #[test]
    fn ssid_com_espaco_nao_e_cortado() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        assert!(redes.iter().any(|r| r.ssid == "Cafe Aberto"));
    }

    #[test]
    fn rede_aberta_nao_e_marcada_como_protegida() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        let aberta = redes.iter().find(|r| r.ssid == "Cafe Aberto").unwrap();
        assert!(!aberta.protegida);
        let psk = redes.iter().find(|r| r.ssid == "ORLEANS").unwrap();
        assert!(psk.protegida);
    }

    #[test]
    fn rede_salva_e_reconhecida() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        assert!(redes.iter().find(|r| r.ssid == "Kill.exe").unwrap().salva);
        assert!(!redes.iter().find(|r| r.ssid == "ORLEANS").unwrap().salva);
    }

    #[test]
    fn sinal_ordena_do_melhor_para_o_pior() {
        let redes = juntar_redes(BRUTO, SALVAS, None);
        for par in redes.windows(2) {
            assert!(
                par[0].sinal >= par[1].sinal,
                "{} ({}) antes de {} ({})",
                par[0].ssid,
                par[0].sinal,
                par[1].ssid,
                par[1].sinal
            );
        }
    }

    #[test]
    fn conectada_vai_para_o_topo_mesmo_com_sinal_pior() {
        let redes = juntar_redes(BRUTO, SALVAS, Some("ORLEANS"));
        assert_eq!(redes[0].ssid, "ORLEANS", "a conectada tem de aparecer primeiro");
    }

    #[test]
    fn sem_rede_nenhuma_a_lista_e_vazia_e_nao_quebra() {
        assert!(juntar_redes("", "", None).is_empty());
        assert!(juntar_redes("bssid / frequency / signal level / flags / ssid", "", None).is_empty());
    }

    #[test]
    fn falha_de_conexao_diz_o_motivo_e_nao_so_que_falhou() {
        assert_eq!(descrever_falha("4-WAY_HANDSHAKE"), "senha recusada pela rede");
        assert!(descrever_falha("DISCONNECTED").contains("fora de alcance"));
        assert!(descrever_falha("").contains("não conectou"));
        assert!(descrever_falha("DESCONHECIDO").contains("DESCONHECIDO"));
    }

    #[test]
    fn erro_viaja_dentro_do_cmd_de_resultado() {
        match Cmd::RedesWifi(Vec::new(), None).com_erro("sem suporte") {
            Cmd::RedesWifi(_, Some(m)) => assert_eq!(m, "sem suporte"),
            outro => panic!("esperava Rede sem sucesso, veio {outro:?}"),
        }
        match Cmd::DispositivosBt(Vec::new(), None).com_erro("sem hci") {
            Cmd::DispositivosBt(_, Some(m)) => assert_eq!(m, "sem hci"),
            outro => panic!("esperava DispositivosBt, veio {outro:?}"),
        }
    }
}
