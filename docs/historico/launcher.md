# O launcher — histórico medido

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**, quando o arquivo passou
> de 2.100 linhas. O resumo e a porta de entrada ficam em
> [`AGENTS.md`](../../AGENTS.md) §6; aqui está o texto completo, com as
> medições e o porquê de cada decisão. Nada foi reescrito na mudança.

---

## 6. O launcher — o app central

Rust + SDL2, **renderer próprio**. Alternativas descartadas: `egui` (desenho
custom limitado) e Slint (DSL + runtime grande para 1 GB de RAM). SDL2 já está no
Buildroot e o KMSDRM dispensa compositor.

| Peça | Onde |
| --- | --- |
| estado, foco, reducer, grade | `crates/core` (117 testes, sem SDL) |
| janela, desenho, rotação, evdev, catálogo | `crates/ui-sdl` (136 testes) |

### 6.1 Rotação — a orientação é fato do sistema

O app **não contém nome de aparelho nem sentido fixo**. Precedência
(`crates/ui-sdl/src/display.rs`), e o log diz a fonte:

| Fonte | Quem usa | Recarrega ao vivo |
| --- | --- | --- |
| `GIOCHI_ROTATE` | depuração | não (é ambiente de processo) |
| `/user/.config/giochi/rotation` | usuário | **sim**, a cada 1 s |
| `/sys/class/graphics/fbcon/rotate` | **sistema** | sim |
| `/proc/cmdline` (`fbcon=rotate:N`) | sistema, quando o sysfs diz `0` | não |
| formato do output | último recurso (marca `Deduzido`) | — |

O cruzamento com o `cmdline` existe por medição: no X55 o sysfs respondia `0`
enquanto o `cmdline` pedia `fbcon=rotate:3`, e o launcher desenhava projeto
720×1280 num painel retrato. O sysfs **vence** quando diz algo diferente de zero
(é o estado de execução); só o `0` é desempatado. `fbcon=rotate:2` (180°) não é
suportado — o app avisa e pede configuração manual em vez de fingir.

Cada primitiva pública mapeia a coordenada **uma vez** e delega para uma versão
`*_dev` em espaço de dispositivo. Sem essa separação `stroke_round_rect` chamaria
`fill_rect` e giraria duas vezes. O toque desfaz em `unmap_point`.

A matemática (`rotate_point`/`rotate_rect`/`unrotate_point`) é pura e testada sem
SDL, incluindo ida-e-volta e a relação de 180° entre os sentidos. Errar o sentido
dá exatamente "de ponta a cabeça": horário e anti-horário diferem 180°, sem
meio-termo.

### 6.2 Layout adaptativo — escala 1,0 em qualquer painel

`draw::projecao(rot, output)` devolve o espaço de projeto: sem giro, projeto =
painel; com giro, projeto é paisagem com medidas trocadas. O espaço lógico
continua sendo o do dispositivo, então a escala é **1,0 sempre** e a fonte nunca
encolhe. Em troca, o layout é proporcional: `views::Moldura` calcula margens,
barras e espaçamentos a partir do tamanho do painel, usando 1280×720 só como
proporção de referência.

Antes disso o projeto era fixo em 1280×720: num painel 640×480 a escala era 0,5,
`F_TINY` (9 px) virava 4,5 px de device e sobrava 25 % de letterbox.

### 6.3 Entrada por evdev, não por SDL gamepad

`crates/ui-sdl/src/input_evdev.rs` lê `/dev/input/event*` direto. Motivo medido:
os botões dos handhelds vêm de driver próprio (`rocknix-joypad`) e o
`SDL_GameControllerDB` não tem mapeamento para aparelho custom; além disso o
`buttons.map` mapeia **por código evdev** e o SDL entrega índice. O SDL ficou só
com janela e desenho. Uma thread de leitura por `event*`, estado é a soma de
todos. O analógico **não é lido**: normalizar exige `absinfo` por ioctl, e a
calibração (`axis_*`) já é parseada para quando isso entrar.

**A releitura é obrigatória.** O nó `event*` de um controle Bluetooth só existe
**depois** que ele conecta. Até 13/11/2026 os apps liam `/dev/input` uma vez na
abertura: um controle pareado em execução nunca era visto. Hoje há varredura a
cada **2 s** (`REVISAO_MS`), idempotente por `HashSet`, com uma linha de log
quando acha um dispositivo novo.

`ButtonsMap::parse` acumula `desconhecidos` e `load` avisa **uma vez**:

```text
aviso: 9 símbolo(s) do mapa não reconhecido(s): axis_x_min = 15, ...
```

O mapa é do **sistema**; o tradutor de símbolos é do app. Um símbolo desconhecido
era descartado em silêncio e o botão simplesmente não funcionava — aconteceu
**duas vezes**: com comentário no fim da linha (`up = BTN_DPAD_UP  # gpio4 PA0`,
que deixava 1 de 13 botões) e com `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`. Foi esse aviso
que revelou que as chaves `axis_*` eram declaradas pelo sistema e ignoradas pelo
app. **O que foi ignorado tem de ser dito.**

`scripts/verificar-mapa.mjs` responde "o app conhece este mapa?" lendo a lista de
símbolos do **binário** (`--simbolos`), não de uma cópia: um símbolo desconhecido
vira botão morto em silêncio. Um teste Rust
(`lista_de_simbolos_bate_com_o_tradutor`) impede a lista de divergir do `code_for`
— é o que permite verificar um aparelho **que não está na mão**.

**Pendência medida em 18/11/2026 — o sistema publica um controle virtual.**
O Giochi System passou a expor `Giochi Gamepad`: **17 botões e 4 eixos
(`-32767..32767`) num nó só**, traduzindo o hardware de cada placa. Guia em
`docs/entrada-para-equipes.md` e `contrato-de-entrada.md` naquele repositório
(somente leitura, §1.1). **Nada deste lado mudou** — os quatro consumidores
(launcher, Star Dust, launcher-teste, `musica`) continuam abrindo todos os
`event*` pelo **mesmo** `input_evdev.rs`, via `#[path]`, então a mudança é uma
só. Três armadilhas ficam registradas: (1) o virtual duplica os mesmos `BTN_*`
do nó físico, e `pressed` é `HashMap<u16, bool>` indexado **só pelo código** —
somar as duas fontes faz a última escrita vencer; (2) volume e `KEY_POWER`
continuam **fora** do virtual, então abrir só ele perde dormir e volume;
(3) imagens anteriores a 18/11 têm o nó **sem `SYN_REPORT`** — o `giochi-selftest`
dá `PASS` no nó e o app não recebe evento, então rebuild e regravação vêm antes
de acusar o leitor. O virtual também remove o bloqueio do `absinfo` **para os
eixos dele** (a faixa já vem normalizada e desinvertida). Plano completo,
decisões abertas e como verificar: [`docs/controle-virtual.md`](docs/controle-virtual.md).

### 6.4 O que o kernel promete × o que ele entrega

| Pergunta | Ferramenta | Responde |
| --- | --- | --- |
| o evento chegou? | `aparelho.sh input` | lê `/dev/input` cru, sem mapa |
| o dispositivo declara esse código? | `aparelho.sh capacidades` | lê `B: KEY=` do `/proc` |

| declara? | entrega? | Conclusão |
| --- | --- | --- |
| sim | sim | kernel ok; o problema é `buttons.map` ou `code_for` |
| sim | não | driver ou DTS não registra o evento |
| não | sim | bitmask incompleto; aviso, não veredito |
| não | não | o botão não existe neste hardware |

**O formato do bitmask engana.** O kernel lista as palavras de 64 bits em ordem
**decrescente**, então a última coluna é a palavra 0. Ler da esquerda desloca
todos os códigos em múltiplos de 64 e produz um mapa plausível e errado:

```text
B: KEY=10000000000000 0    → palavra 1, bit 52 → 64+52 = 116 = KEY_POWER
B: KEY=c000000000000 0     → palavra 1, bits 50-51 → 114 e 115 = VOLUMEDOWN/UP
```

Mapa real do X55: `rk805 pwrkey` 116 · `gpio-keys-control` 304-305, 307-308,
310-318 (13 botões) e 544-547 (direcional, que são `BTN_DPAD_*`, faixa `BTN_` e
não `KEY_`, e por isso parecem códigos sem nome) · `gpio-keys-vol` 114-115.
Somando: **20 códigos, todos com entrada em `code_for` e no `buttons.map`**.

`evtest --query` não ajuda: **devolve 0 para qualquer código** nesta imagem. O
`/proc` é a única fonte. `crates/core/src/capacidades.rs` decodifica
`/sys/class/input/event*/device/capabilities/key` com testes usando as três
strings **reais** medidas em aparelho, e cada arquivo é decodificado
separadamente (a ordem decrescente vale por arquivo, e concatenar todos antes
deslocaria os códigos dos seguintes).

`None` (não medido) ≠ lista vazia (medido, não declara nada): no PC não há
`/sys/class/input/event*` e o launcher **não avisa**. Ele só avisa quando mediu e
o código não está lá — é o caso do R35S, que não declara `KEY_POWER` e onde o
toque curto para dormir não existe (o menu de energia continua no START).

### 6.5 Energia, brilho, volume e sono

Cinco regras, todas nascidas de bug real no aparelho:

1. **Qualquer entrada de pessoa acorda.** `Cmd::is_user_input` separa "um dedo
   apertou" de "o sistema reportou" (bateria, HUD, presença de controle). O
   reducer intercepta em `Sleeping`/`Shutdown` **antes** do `match`, e a entrada
   é consumida pelo despertar — o botão que acorda não age.
2. **O botão segurado não pode vazar.** `poll_gamepad` roda `mapper.update`
   mesmo dormindo (senão borda/repetição envelhecem e sai um `Confirm` fantasma
   logo depois de acordar) e **descarta o resto do quadro** quando estava travado.
3. **Dormir apaga o backlight.** `backlight.rs` escreve em
   `/sys/class/backlight/*/brightness`. `apagar` é idempotente (dormir duas vezes
   não salva 0 como "anterior") e na abertura `destravar_tela_apagada` reacende se
   o valor veio 0 — proteção para o caso de o processo morrer dormindo.
4. **O botão de energia físico funciona.** `rk805 pwrkey` emite `KEY_POWER`
   (116). Toque curto dorme; segurar 1 s abre o menu de energia **ao atingir o
   limiar**, não ao soltar (senão o usuário segura sem nenhum sinal). A lógica
   pura vive em `power.rs` (7 testes) e deixa rastro no log:
   `botão de energia: segurado 1.0s, abrindo menu de energia`.
5. **As teclas de volume controlam o mixer ALSA de verdade.** `Master Playback
   Volume` via libasound, passo de 5 %, repetição ao segurar (400 ms, depois
   120 ms). O slider de brilho também chega no painel e **grava**
   `/user/.config/giochi/brightness`, o arquivo que o `giochi-init` lê no boot —
   sem isso a escolha do usuário sumia a cada reinício.

**`libasound` é resolvida por `dlopen`, não por link.** O R35S tem `/dev/snd` e
drivers de áudio, mas **não tem a biblioteca** — e biblioteca ausente é falha de
carga: o processo não sobe "sem som", ele não sobe. Uma feature de build por
aparelho quebrava o X55 (que tem a lib e perdia o mixer). Hoje `volume.rs`
resolve os símbolos em tempo de execução: achando, controla; não achando,
`Ausente` e o volume mostra `—`. **Um binário, sem feature**, e `libasound` não
aparece no `NEEDED`.

Diagnóstico de campo, no próprio aparelho:

```bash
giochi-launcher --testar-backlight   # apaga 2 s e restaura (medido: 833 → 0 → 833)
giochi-launcher --brilho 40          # mesmo caminho do slider, e persiste
giochi-launcher --mostrar-volume     # o que o mixer diz agora
giochi-launcher --volume 30          # aplica pelo mesmo caminho da UI
giochi-launcher --espiar-input 20    # cada tecla crua do kernel, sem mapa
```

`--espiar-input` é o que separa "o evento não chega ao sistema" de "chega e o app
ignora".

Bateria e temperatura vêm de `/sys/class/power_supply` e `/sys/class/thermal`
(`energia.rs`), despachadas a cada **5 s** — ler 4 arquivos de sysfs a 60 Hz
seria desperdício e nenhum dos valores muda entre dois quadros. Os dois saem
juntos no log quando qualquer um muda: uma leitura que falha não pode virar `—`
sem explicação.

**E o "quando muda" já esteve errado de um jeito que nenhum teste pegava.**
`hud.temp_c` é `u16` e `0` é o sentinela de "desconhecido"; a comparação era
`temperatura.map(|t| t as u16) != Some(hud.temp_c)`, e num aparelho **sem
sensor** o lado esquerdo é `None` — `None != Some(0)` é sempre verdadeiro. O
resultado medido no R35S: `energia: bateria sem medidor · temperatura sem sensor`
a cada 5 s no console **serial**, que é o único canal quando o USB não sobe.

Hoje a decisão é uma função pura, `energia_deve_registrar`, que compara `None`
com o **mesmo sentinela** que a HUD usa e registra a primeira linha sempre (é ela
que diz "aqui não há medidor nem sensor"). Seis testes cobrem os casos, e reverter
para a expressão antiga reprova dois: `sem_sensor_nao_registra_a_cada_leitura` e
`sensor_que_desaparece_registra_uma_vez`. A mesma função foi para o Star Dust.

**Um número no sysfs não prova que o hardware existe.** O X55 publica
`bateria 0% (Not charging)` porque o nó do PMIC `rk817` existe — a célula foi
removida fisicamente para dar acesso à serial. Esse módulo não pode ser julgado no
X55; quem valida bateria é o aparelho que tem célula.

### 6.6 Wi-Fi e Bluetooth

`semfio.rs` chama `wpa_cli` e `bluetoothctl` e publica `Option<bool>`:
`None` = não sondado, `Some(false)` = medido que não tem. **`None` não é
ausência**: o R35S não tem rádio e a tela diz "sem rádio neste aparelho", com o
"procurar de novo" inerte — sem essa guarda, apertar `A` ligaria "procurando…"
para sempre.

Três armadilhas de ambiente, todas medidas:

- **Não existe `timeout` na imagem.** As janelas usam `--timeout` do próprio
  `bluetoothctl` e espera fixa no `wpa_cli`. Um comando que não existe falha em
  silêncio se ninguém olhar.
- **`wlan0` sobe DOWN.** O `S40wifi` do sistema não levanta a interface quando o
  `wpa_supplicant.conf` está vazio, e `wpa_cli scan` responde `FAIL-BUSY` com o
  rádio parado. O sintoma é "não acha nenhuma rede", não "erro" — então o app
  chama `ip link set up` antes de varrer.
- **`wpa_cli` está em `/usr/sbin` e `ip` em `/sbin`.** `programa()` procura nos
  diretórios padrão antes de desistir, porque o launcher também é iniciado à mão
  por SSH no diagnóstico.

**`update_config=1` é o que separa "conectou" de "vai lembrar".** Medido no X55
em 15/11/2026, conexão ponta a ponta com a rede real:

```text
wi-fi: conectando a Kill.exe em wlan0 (senha: 10 caractere(s))
wi-fi: interface wlan0 levantada
wpa_state=COMPLETED   inet 192.168.1.7/24   ping 192.168.1.1: 0% perda
DNS resolve (github.com)   HTTP ok
```

Mas o `save_config` respondia **`FAIL`**, em silêncio, e a rede sumia no reboot.
A causa foi medida, não deduzida: `/etc/wpa_supplicant.conf` **não tinha
`update_config=1`**, e essa opção só é lida na **partida** do supplicant.

| Passo | Resultado |
| --- | --- |
| sem `update_config=1` | `save_config` → `FAIL` |
| com a linha, sem reiniciar o processo | `save_config` → `FAIL` (a flag é lida no boot dele) |
| com a linha e supplicant reiniciado | `save_config` → **`OK`**, e o `ssid=` aparece no arquivo |
| matar e subir o supplicant de novo | reconecta **sozinho** a `Kill.exe` (`wpa_state=COMPLETED`) |
| **reboot do aparelho** | subiu, associou a `Kill.exe` e pediu DHCP **sem ninguém tocar em nada** |

A última linha é a que fecha o caso: `uptime` de 507 s depois de uma queda de
SSH, com a rede já `COMPLETED` e IP `192.168.1.7`. Ninguém digitou senha. `/` é
`ext4 (rw)`, então o arquivo sobrevive a reboot de verdade.

Duas consequências no código:

1. `save_config` deixou de ser silencioso. Quando falha, o `conectar()` devolve
   `Cmd::ConectarSemSalvar(ssid)` e a tela diz *"Conectado a X — a rede não foi
   salva: vai pedir a senha de novo"*. Antes ia só para o `stderr`, que ninguém
   lê no aparelho: o usuário reiniciava e encontrava o console desconectado sem
   nenhuma pista da causa.
2. A decisão virou função pura, `resultado_da_conexao(salvou, ssid)`, porque o
   resto do `conectar()` chama `wpa_cli` e não se testa sem aparelho.

**A linha é da imagem, não do app.** Foi aplicada no X55 desta rodada para o
recurso funcionar hoje; a próxima imagem precisa trazê-la (§17).

**Esperar a associação é o que separa "aceitou" de "conectou".**
`select_network` responde OK antes de qualquer associação; sem esperar
`wpa_state=COMPLETED` a tela diria "Conectado" para uma rede que nunca conectou.
A espera tem 12 s e, ao falhar, devolve **o último estado do supplicant** — não
"deu errado". `4-WAY_HANDSHAKE` é senha recusada; `SCANNING` é rede fora de
alcance; são ações diferentes do usuário. `save_config` **não roda** com senha
errada: o aparelho não tenta sozinho no próximo boot e falha sem ninguém entender.

**Pareamento Bluetooth precisa de scan.** O `0x04` do BlueZ é *page timeout*: o
controlador só responde enquanto está anunciando, e um scan de 5 s antes do
`pair` é o que separa "pareou" de `ConnectionAttemptFailed`. O agente
(`NoInputNoOutput`) tem de estar no **mesmo processo** do `pair`; o veredito sai
de `info` (`Paired: yes`), não do texto. `motivo_do_pareamento` traduz
`ConnectionAttemptFailed`/`NotAvailable`/`AuthenticationFailed`/`NoAgentAvailable`
em instruções — e há teste exigindo que o código bruto do BlueZ nunca chegue ao
usuário.

**Bluetooth como entrada: o kernel está completo para clássico, falta `UHID`, e
o ponta a ponta nunca foi medido.** Esta seção já afirmou o contrário — que
faltava "o transporte HID do kernel" — com três provas que **eram falsas**. Ficam
registradas porque cada uma é uma armadilha reutilizável (§13).

Medido no R35S em 15/11/2026 (`/proc/config.gz`, que é a fonte real):

```text
CONFIG_HID=y            # já existe
CONFIG_HID_GENERIC=y    # já existe — é ele que trata o relatório
CONFIG_BT=m
CONFIG_BT_HIDP=m        # já existe
# CONFIG_UHID is not set ← o único que falta
```

| Prova antiga | Por que era falsa |
| --- | --- |
| `modinfo hidp` mostra `depends: bluetooth` sem `hid` | **`modinfo` não existe nesta imagem** (`sh: modinfo: not found`). A "medição" era memória, não comando. O equivalente real é `grep hidp modules.dep`, e ele não lista `hid.ko` porque `CONFIG_HID=y` está **embutido no vmlinux** — não há `.ko` para depender |
| `strings hidp.ko \| grep hid_register_driver` devolve vazio | o símbolo no 6.12 é `__hid_register_driver` (macro `hid_register_driver`). E **não há driver a registrar**: `hidp` no 6.12 é **transport driver**, e o `.ko` referencia `hid_allocate_device`/`hid_parse_report`/`hid_add_device`/`hid_input_report` — o conjunto exato de quem cria `hid_device` e entrega ao `hid-generic` |
| `/sys/bus/hid/drivers/hidp` não existe | **é o esperado.** Só um `hid_driver` aparece ali. Um transport não cria diretório de driver. Medido: `modprobe hidp` → `rc=0`, e o log diz `HIDP socket layer initialized`, que só é impresso **depois** da inicialização bem-sucedida |

A lição vale mais que o achado: **duas das três provas mediam a intenção**
("o módulo parece incompleto") em vez do efeito (o módulo carrega e inicializa?),
e a terceira usou uma ferramenta que não está instalada. Foi assim que um pedido
foi para o canal pedindo `CONFIG_HID=y` para uma imagem que **já tinha**
`CONFIG_HID=y`.

**O que falta de verdade:** `CONFIG_UHID` (`# CONFIG_UHID is not set`), que é o
que permite o BlueZ criar o dispositivo HID de um controle **BLE/HOGP**
(`/dev/uhid` não existe). E `hidp` **não está no autoload**: nem `/etc/modules`
nem `/etc/modules-load.d/` o citam (medido), então o módulo `=m` só carrega por
`modprobe` manual. Para HID clássico o kernel está completo — o próximo passo é
**medir**, não configurar.

**O que segue não medido:** o `disconnected with reason 2` observado no X55 em
sessão anterior nunca foi reproduzido com o kernel analisado aqui. Medir exige um
aparelho com rádio — o R35S não tem, e o X55 não estava na rede em 15/11/2026
(`aparelho.sh procurar` achou só o R35S). **Não repita "o transporte HID falta"
como fato**: hoje é hipótese sem prova, e a prova anterior foi retirada.

```bash
bash scripts/aparelho.sh bluetooth      # a pilha inteira, com veredito
bash scripts/aparelho.sh parear <MAC>   # pareia/conecta com a sequência certa
```

#### O teclado de senha, e a tela que não tinha saída

Dois relatos do dono em 14/11/2026, e os dois eram de verdade:

1. *"teclado modo qwert"* — a grade era `ABCDEFG / HIJKLMN / OPQRSTU / VWXYZ`,
   isto é, **alfabética**. Quem digita no PC não procura a letra pelo alfabeto: a
   mão vai direto onde o QWERTY põe a tecla. Hoje a ordem é
   `QWERTYUIOPASDFGHJKLZXCVBNM` em leitura, e `state::QWERTY` é o literal que
   `o_teclado_esta_em_qwerty` compara — reverter para alfabético reprova.
2. *"quando se digita a senha não há como confirmar"* — literal. `A` digitava,
   `B` descartava, `X` não fazia nada, e **nenhuma tecla emitia
   `Cmd::ConectarSenha`**. O comando existia no enum e o `ui-sdl` sabia
   executá-lo; faltava quem o emitisse. A senha nunca entrava em vigor.

O que a correção exigiu, e cada peça tem teste:

| Peça | Por quê |
| --- | --- |
| zona `PasswordActions` | um botão **Conectar** abaixo da grade; `↓` da última linha chega nele. Célula da grade que não digitasse quebraria a regra das 7 colunas |
| `Cmd::pede_conexao(&state)` | o `ui-sdl` roda `executar_pedido` **antes** do reducer e só vê `Confirm`; o reducer traduz em `ConectarSenha` depois. Sem esta função a tela fechava, o toast prometia "Conectando…" e **nenhum `wpa_cli` saía** |
| `X` → `MostrarSenha` | o rodapé já anunciava "X Mostrar" e a tecla chamava `open_detail`, que com overlay devolve o estado. Promessa vazia |
| `push_char` no campo visível | com a senha aberta, uma letra ia para a busca, que não está na tela |

A regra de `pede_conexao` fica em **uma definição**, consultada pelo reducer e
pelo `ui-sdl` — duas cópias da condição envelheceriam diferente, que é o mesmo
motivo pelo qual `settings_ajustavel` é consultada pelo desenho e pelo reducer.

#### O núcleo do Star Dust era uma cópia, e ficou para trás

`app/launcher-star-dust/crates/core/` era uma **cópia** do núcleo do launcher:
4244 linhas contra 4631, sem `pede_conexao`, sem a zona `PasswordActions` e sem a
navegação do teclado de senha. O efeito era concreto — o botão "Conectar"
aparecia na tela do Star Dust e não funcionava.

Hoje o `Cargo.toml` aponta por caminho (`../../../launcher/crates/core`), como o
`launcher-teste` já fazia, e o compilador passou a apontar sozinho o que faltava
(`efeitos.rs` não cobria a zona nova). **Divergência impossível, não improvável.**

### 6.7 Feature `texto`

O crate `ui-sdl` compila em dois modos:

| Build | Feature | Linka contra |
| --- | --- | --- |
| padrão | `texto` | `libSDL2` + `libSDL2_ttf` |
| enxuto | `--no-default-features` | só `libSDL2` |

Sem `texto`, `text_size` devolve altura aproximada para o layout não colapsar e
`text` não desenha nada. O CI constrói **as duas** combinações: um `cfg` que
nunca é compilado é código que não existe, e o caminho "aparelho sem SDL2_ttf"
ficou quebrado por semanas porque só o build padrão rodava.

`ttf::init()` **não pode ser chamado duas vezes** — a segunda devolve
`SDL2_TTF has already been initialized` e o launcher volta do app **sem nenhum
texto**. O contexto é um por thread, guardado em `thread_local` (`draw.rs`), para
não exigir `Send`/`Sync` de um tipo que guarda ponteiro cru do SDL.

### 6.8 Catálogo e lançamento

`catalogo.rs` monta as duas listas e resolve `id → dir + entrypoint`: varre as
quatro raízes e junta com `/opt/giochi/apps/index.json`, com o disco vencendo
(§6.9). `GIOCHI_RAIZES` sobrepõe as raízes varridas e `GIOCHI_CATALOGO=<arquivo>`
aponta outro índice — os dois existem porque no PC não há `/opt/giochi`. Nem
`raízes` nem índice desligam o outro: testar com `GIOCHI_RAIZES` num `/tmp` ainda
soma o índice real do aparelho, então isolar exige apontar `GIOCHI_CATALOGO` para
um índice vazio.

`--catalogo-adicionar` é como o app registra um pacote recém-instalado — não por
`sed`, que produziria JSON inválido no primeiro campo com vírgula.

**O display precisa ser solto, e isso é medido.** Com o launcher vivo segurando o
DRM master, um segundo processo recebe `kmsdrm not available`. Por isso
`App::encerrar()` **consome** o `App`: destruir o `Painter` destrói o
`WindowCanvas`, que é quem segura o master. Não há como "pausar" o desenho e
liberar o dispositivo — só fechar. O ciclo é `fork`+`wait`; `--lancar <id>`
escreve em `state.lancar`, o **mesmo** campo que o botão preenche.

**O contrato com o `giochi-init` é `fork`+`wait`, não `exec`.** Medido nos dois
aparelhos: o PID do launcher não muda enquanto o app roda. Com `fork`+`wait` o
launcher nunca sai quando um app roda, então o `giochi-init` fica bloqueado nele e
a cadeia "app termina → init reinicia o launcher" não acontece — não precisa
acontecer. Um watchdog escrito sobre a premissa do `exec` mataria um launcher
saudável no meio da execução. **Não trocar para `exec` sem confirmar a linha do
init em campo.**

**Sair de um app que não responde:** START+B segurados 1,5 s → `SIGKILL` no
filho. `SIGKILL` e não `SIGTERM` porque o SDL só posta `SDL_QUIT` na fila e um
app que não bombeia eventos segue vivo com o KMSDRM. Sem `start`/`b` no mapa, o
launcher **avisa** e segue sem o gesto: adivinhar o código seria pior que não
oferecer a saída.

**O catálogo é podado em tempo de execução** (`poda_sem_binario`), não na
geração: o PC que gera o índice não sabe se um cartão removível estará lá.

### 6.9 As duas abas **detectam** — decisão do dono, 15/11/2026

*"a aba apps do launcher precisa ser do tipo detectar, assim como a aba jogos.
os apps deverão ficar em locais diferentes dos jogos para que a detecção não se
confunda."*

| | antes | agora |
| --- | --- | --- |
| aba Jogos | `index.json` + poda em execução | **varredura do disco** + `index.json` de complemento + poda |
| aba Apps | **oito apps escritos no código** (`demo_apps()`) | varredura de `/opt|/user/apps`, com `kind` |
| `A` num app | `toast("Abrindo X...")` e nada abria | `state.lancar = Some(Pedido::app(i))` — abre de verdade |

**As raízes são quatro, e nenhuma é compartilhada** (`deteccao::Tipo`):

```
jogos:  /opt/giochi/games   /user/games
apps:   /opt/giochi/apps    /user/apps
```

`GIOCHI_RAIZES` (separado por `:`) sobrepõe as raízes **varridas** — é o que
permite testar a detecção no PC, onde `/opt/giochi` não existe. Ela não desliga o
índice: no aparelho, `index.json` continua entrando. Medido no R35S em
15/11/2026, com duas raízes de teste num `/tmp` e uma de cada tipo:
`catálogo: 4 jogo(s) e 2 app(s)` — os 3 jogos e o `musica` vinham do índice real.
Para isolar de verdade, aponte **`GIOCHI_CATALOGO`** para um índice vazio; com os
dois, a conta fecha em `1 jogo(s) e 1 app(s)`.

**A varredura é conjunta e a classificação é pelo `kind` do pacote**, não pela
pasta. Isso é deliberado e tem uma medição atrás: os três jogos do R35S estavam
em `/user/apps` (instalados antes da separação) e **não declaram `kind`**. Varrer
só a raiz do próprio tipo os faria **sumir** da Home; classificar por pasta os
faria aparecer na aba de apps. Com `kind` (ausente = jogo) eles continuam jogos,
e o log diz que o lugar está errado:

```text
catálogo: teste-jogo (/user/apps/teste-jogo) está em /user/apps, que é raiz de outro
  tipo; o lugar de um jogo é /user/games (ou declare "kind": "app" no metadata.json,
  se for um app)
```

Medido no R35S em 15/11/2026 com um pacote sem `kind` plantado em `/user/apps`. A
segunda metade da frase existe porque **o campo ausente é metade da causa**: o
padrão é jogo (§3), então um app sem `kind` "está no lugar errado" por omissão de
um campo. Antes dela o aviso só mandava mover, e mover era a ação errada.

Depois da migração, medido no R35S em 15/11/2026:

```text
catálogo: 3 jogo(s) e 1 app(s) detectado(s) em /opt/giochi/games, /user/games, /opt/giochi/apps, /user/apps
```

**O índice não sumiu do caminho.** Ele continua sendo lido porque é o que lista
conteúdo **fora** das raízes (cartão montado noutro caminho) e porque é o
contrato de `docs/integracao.md` §2.1. Mas o **disco vence** na junção: um
pacote varrido substitui a entrada do índice para o mesmo slug, com o `dir` onde
ele está de fato. Foi a lição da loja, que apontava para `/opt/giochi/apps/x` com
o pacote em `/user/apps/x`.

**O pedido de lançamento carrega o tipo** (`state::Pedido { tipo, indice }`). Os
dois índices valem 0 ao mesmo tempo em qualquer aparelho com um jogo e um app;
um pedido que levasse só o número abriria o item errado, e o sintoma ("abriu
outra coisa") aponta para longe da causa. É a mesma razão pela qual a loja
separa `PacoteVerificado` de `LicencaAssinada` por tipo.

**Detectar exige pacote com identidade no disco.** Medido: a primeira instalação
do `musica` no R35S copiou **só o binário**, sem `metadata.json`. A pasta existia,
tinha executável, e não havia como saber que ali havia um *app* — ele apareceu
como **jogo**, no carrossel da Home. Hoje `compilar-musica-arm64.sh --instalar`
copia `metadata.json` (e `capa.png`) junto com o binário.

**O aviso de lugar existe em duas linguagens e diz a mesma frase.** O gerador do
PC (`scripts/lib/catalogo.mjs`, `LUGAR_ERRADO`) e o detector do aparelho
(`deteccao::aviso_de_lugar`) não compartilham código — um roda em Node, o outro
no binário. O que compartilham é o **texto**: `lugar de um <tipo> é <raiz>`. É
de propósito: `grep "lugar de um"` acha os dois lados, e a mensagem não muda de
significado conforme quem reclamou.

**Tema não recebe esse aviso**, e a guarda existe por medição: o launcher mora em
`/opt/giochi/apps`, não declara `kind` (é tema) e caía no padrão jogo — o log
dizia *"o lugar de um jogo é `/user/games`"*, que é onde o executável de
inicialização **não** pode ficar. Seguir o aviso quebraria o boot.

**Consequências para quem instala:**

| caminho | quem escolhe a raiz |
| --- | --- |
| `instalar-do-cartao.sh` (no aparelho) | `kind` do `metadata.json` dentro do `.giochi` (ausente = `/user/games`) |
| `aparelho.sh catalogo` | colhe as quatro raízes e o `mesclarInstalados` avisa quando o `kind` não bate com a pasta |
| `empacotar.mjs` | não muda: o `kind` viaja dentro do pacote |
| `--catalogo-adicionar` (o próprio aparelho) | recebe o `dir` de quem chama e grava o `kind` junto |

### 6.10 Idioma e tema — os dez do site, e o degrau que faltava no tema

Pedido do dono em 15/11/2026: *"o site já está no ar (fonte giochi-os) lá está
traduzido para 10 idiomas e sua missão é fazer o launcher multi idiomas. lá onde
troca o tema terá a opção de trocar o idioma… o tema inicial será inglês (padrão
comercial) e a troca de tema tem que deixar a base pronta para funcionar."*

Status: os dez existem (§`docs/idiomas.md`), o tema está **ligado ao desenho** e
tem **um** tema registrado.

#### Os dez são os do site, na ordem do site

`en`, `zh`, `hi`, `es`, `fr`, `ar`, `bn`, `pt`, `ru`, `ur` — os mais falados por
número total de falantes (Ethnologue), com os mesmos `nome_proprio` do
`GIOCHI-OS/scripts/lib/i18n.mjs`. Lista diferente obrigaria a mesma pessoa a
aprender duas, e o idioma escolhido no site não estaria no aparelho.

#### O padrão é inglês, e "não escolhido" ≠ "escolhido como inglês"

`Idioma::PADRAO` é `En`; `Settings::idioma` nasce nele. Mas quem decide a tela de
boas-vindas é `State::idioma_definido`, e não o idioma em si: um aparelho novo e
um aparelho configurado em inglês têm o **mesmo** `settings.idioma` e vidas
diferentes. Devolver o padrão em `preferencias::carregar_idioma()` apagaria essa
diferença e a boas-vindas nunca apareceria — ou apareceria sempre, que é a mesma
cegueira com outro sintoma.

#### A tabela fica no núcleo; o desenho pede a chave

Uma linha por chave, dez colunas, em `core/src/idioma.rs`:

```rust
("set.language", ["Language", "语言", "भाषा", "Idioma", "Langue",
                  "اللغة", "ভাষা", "Idioma", "Язык", "زبان"]),
```

`Chave` é `#[repr(usize)]`, então `chave.indice()` é o próprio discriminante e a
consulta é `O(1)` — e a linha `i` da tabela **tem** de ser a chave `i` do enum. O
teste `toda_chave_da_tabela_bate_com_o_enum` existe porque consulta por índice
fora de ordem traduziria a frase errada em silêncio: `Ajustes` aparecendo onde
devia estar `Idioma`.

A tradução **não** é um `match` por idioma espalhado nas telas: `views` chama
`t(state, Chave::X)`, e nada mais. É o que permite testar a tabela sem SDL (e é
o que impede a décima-primeira tradução de nascer num arquivo só).

`Perf::label`, `Filter::label` e `Tab::label` passaram a receber o idioma. Sem
isso, a aba Jogos ficava com três palavras em português no meio de uma interface
em chinês — e o rodapé mostrava `Equilibrado` num aparelho em russo.

#### O idioma mora onde se troca o tema, e é dado como o volume

`Config › Aparência` ganhou a linha `Idioma`, entre `Tema` e `Escala`:

| Linha | Ajustável? | O que a seta faz |
| --- | --- | --- |
| 0 Tema | **sim** | percorre `Tema::TODOS`; com um tema só, **avisa** que há um |
| 1 Idioma | **sim** | percorre os dez, com volta, e a interface muda na hora |
| 2 Escala | não | informa que acompanha o painel |

`settings_rows("appearance")` foi de 2 para 3 e `settings_ajustavel` cobre as
linhas 0 e 1 — as duas listas continuam sendo consultadas pelo desenho e pelo
reducer, e o teste `foco_e_desenho_concordam_no_numero_de_linhas` reprova se
divergirem.

#### A vista de quadros passou a ter **quatro** coisas para congelar

O idioma é a quarta, junto de relógio, bateria e temperatura (§11). Um PC sem
`/user/.config/giochi/language` desenha a **tela de boas-vindas**; um aparelho
que já escolheu desenha a Home. Comparar os dois daria uns 60 % e mandaria
procurar defeito de desenho — exatamente a família de erro que já custou três
rodadas (§13, "a referência do `verificar` vinha de um binário velho").

Por isso `aparelho.sh` (captura **e** referência), `comparar-launcher-teste.sh`,
`verificar-star-dust.sh` e `verificar-efeitos.sh` passam `--idioma en` dos dois
lados. `--idioma` **não grava** nada: se gravasse, cada foto mudaria o estado do
aparelho.

#### Uma fonte não cobre dez idiomas — e a escolha é por string

O `DejaVuSans.ttf` do aparelho cobre latim, cirílico e parte do árabe; **não**
cobre chinês, devanágari nem bengali. Trocar a fonte do app por idioma mudaria
também título de jogo, relógio e número. A saída foi escolher a fonte **por
texto**: `Painter::fonte_para` usa a primeira da cadeia que tem glifo para todos
os caracteres, com a resposta memorizada por `(fonte, caractere)` —
`TTF_GlyphIsProvided` a cada texto, a cada quadro, seria trabalho repetido.

Duas medições mandaram no desenho:

1. **A cadeia tem de incluir as fontes dos dez**, e não só a do idioma escolhido.
   Medido no PC em 15/11/2026 com a cadeia só do inglês:
   `aviso: nenhuma das 1 fonte(s) cobre "中文"`. A lista de idiomas desenha os dez
   nomes nativos o tempo todo — nas boas-vindas e em Config › Aparência —, e é
   justamente a linha que a pessoa precisa escolher que saía como quadradinhos.
2. **A principal continua na frente.** Ela cobre o latim, então o título do jogo
   e o relógio saem exatamente como antes; a fonte da escrita entra só para o
   texto que a principal não cobre. Sem isso, entrar em chinês mudaria o desenho
   de tudo que é latino.

Fonte ausente **degrada e avisa**, não impede o app de abrir, e `--fontes` mostra
a cadeia resolvida com `[NÃO EXISTE]` no que o idioma pede e a imagem não tem.

#### `ar` e `ur`: shaping **funciona**, bidi de parágrafo não

> **Esta seção afirmou o contrário e está corrigida.** Dizia "`SDL2_ttf` sem
> HarfBuzz, letras isoladas". Era **suposição** — a mesma família das três provas
> falsas do §13: ninguém tinha rodado `readelf` nem comparado o quadro.

Medido no R35S em 15/11/2026:

```console
$ readelf -d /usr/lib/libSDL2_ttf-2.0.so.0 | grep -i needed
 (NEEDED)  Shared library: [libharfbuzz.so.0]
```

O `defconfig` do R35S liga `BR2_PACKAGE_HARFBUZZ=y` (e o comentário lá já explica
por quê), e o `SDL2_ttf` o puxa de fato. A contraprova visual: o mesmo texto
árabe no aparelho por KMSDRM e no PC dá **99,8%** de identidade — se um dos dois
não fizesse shaping, as formas isoladas contra as conectadas derrubariam esse
número como a rotação oposta derruba para 10,9%.

| O que | Estado |
| --- | --- |
| shaping (`ar`/`ur`): ligaduras e formas inicial/medial/final | **funciona** |
| bidi de parágrafo misto (árabe + número + latino na mesma linha) | **não existe** — o `SDL2_ttf` faz shaping por *run*, não reordena parágrafo |
| layout espelhado (margens, âncoras) | **não existe** — `Idioma::dir()` devolve `Rtl` e nenhuma tela inverte |

A distinção importa: rótulo curto puramente árabe sai correto; `launcher 0.1.0`
dentro de uma linha árabe pode sair com os pedaços fora de ordem, e **fonte não
conserta bidi** — quem consertaria é o layout honrando `dir`. O `App` avisa no log
ao abrir num idioma RTL. **Não** tratar isso como regressão de desenho.

#### Tema: a base existe, e é o compilador que cobra o resto

Até 15/11/2026 a linha `Tema` dizia `Giochi Dark (único)` e a seta não fazia
nada — não por bug, mas porque **nada havia para trocar**. O texto "único" era o
substituto honesto de uma lista inexistente.

Hoje:

| Peça | Onde |
| --- | --- |
| escolha | `core::tema::Tema` (`TODOS`, `proximo`, `anterior`, `ha_escolha`), em `Settings::tema` |
| paleta | `ui-sdl::theme::Paleta` + `paleta(Tema)` com `match` **exaustivo** |
| desenho | `theme::accent()`, `theme::text()`, … leem a paleta do quadro (`thread_local`, trocada uma vez por quadro em `App::render`) |
| persistência | `/user/.config/giochi/theme`, por diferença de estado (`App::sync_tema`) |

Acrescentar um tema agora é: variante em `Tema` + entrada em `TODOS` + um
`Paleta` novo em `theme.rs`. **Nada mais** — e o `match` de `paleta()` não deixa
compilar sem a paleta. Foi por isso que as 39 cores de `views.rs` e as de
`efeitos.rs` deixaram de ser constantes: com uma constante, um segundo tema
mudaria "algumas" cores, que é pior que nenhuma.

O que **não** é do tema continua constante: `WHITE` e `BLACK` puros (camadas de
alafa por cima de qualquer cor) e as métricas.

#### Medido no R35S em 15/11/2026, imagem `built_at_utc=2026-11-15T21:00:56Z`

| O que | Resultado |
| --- | --- |
| primeira abertura | boas-vindas com **inglês em foco**, sem arquivo de idioma |
| escolher português (`↓`×7 + `A`) | `idioma: en -> pt` e `/user/.config/giochi/language` = `pt` |
| segunda abertura | `Portuguese (pt)`, **sem** boas-vindas |
| Home `pt` por KMSDRM × PC | **99,6%** (antialiasing de glifo) |
| Config `pt` × PC | **99,7%** |
| quadro `en` × `pt` no aparelho | **98,4%** — a tradução chega à tela |
| `aparelho.sh verificar` | **100,0%** |
| as dez abrem | 10/10 |
| escolher com **botão físico** (não `--teclas`) | a boas-vindas ficou na tela real e um toque no controle escolheu a linha em foco (inglês) e gravou `en\n`; a execução seguinte abriu sem boas-vindas |
| trocar idioma com **botão físico** | `GIOCHI_DEBUG_CMDS=1` registrou 8× `Cmd::Nav(Left)` de `evdev` e o arquivo andou `zh`→`en`→`ur`→`ru`→`pt`→`bn`→`ar`→`fr`→`es` — cada troca tem uma tecla como causa |
| nenhuma escrita espontânea | 15 s de amostragem sem entrada ⇒ arquivo parado; 10 s de trace ⇒ nenhum `TrocarIdioma` sem `Nav`. `sync_idioma` só grava quando o valor muda de fato, que é o que poupa o cartão SD |

**A imagem não tem fonte para três dos dez idiomas.** `find /usr/share/fonts
-type f | wc -l` = **21**, todas DejaVu, nenhuma Noto/WenQuanYi. O `--fontes`
mede (não deduz) a cobertura por `TTF_GlyphIsProvided`:

```text
en/es/fr/pt 15/15 · ru 10/10 · ar 10/10 · zh 5/8 · hi 1/11 · bn 1/10
```

Chinês, hindi e bengali saem com quadradinho **no R35S de hoje**. O app degrada e
avisa; o pedido de fonte para a imagem é a issue nº 5 de
`docs/issues-para-outros-repos.md`. A lista de idiomas continua utilizável porque
cada linha mostra o nome nativo **e** o nome em inglês à direita — onde a escrita
não tem glifo, a coluna da direita identifica a linha.

`--fontes` também já esteve errado de dois jeitos, e os dois são reutilizáveis:
**deduzia** cobertura pelo tamanho da cadeia (dava "sem fonte" para o português) e
**pedia dispositivo de vídeo** antes de imprimir (`Error: "No available video
device"` por SSH — a ferramenta se recusava a rodar onde o problema aparece). Hoje
ele sai antes de `video.window(...)` e mede o glifo.

#### O teclado de senha é QWERTY em qualquer idioma

`KEYBOARD` tem 26 letras latinas fixas. Uma senha de rede em cirílico ou árabe
não é digitável no aparelho hoje — e o teclado de senha **não pode** virar o
teclado do idioma por dedução, porque a senha é da rede e não da interface.
Registrado como limite em `docs/idiomas.md`; a saída é mapear `KEYBOARD` por
idioma quando houver aparelho com essas redes.
