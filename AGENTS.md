# AGENTS.md — Giochi app

> Handoff técnico deste repositório. Leia antes de continuar o trabalho.
> Atualizado em: 14/11/2026 — **os jogos saíram daqui** (foram para um
> repositório próprio). Este repositório é só de **apps do sistema**.

## 0. Estado em uma tela

| | |
| --- | --- |
| apps em `app/apps.json` | **7**: `launcher`, `launcher-teste`, `launcher-star-dust` (image) + `forge`, `store`, `painel`, `loja` (host) |
| `validate-app.mjs --all` | **7 válidos** (1 em scaffold: `launcher-teste`) |
| `tests/run-all.mjs` | **199/199** JS |
| `compilar-launcher.sh --testes` | **241** Rust (134 ui-sdl + 107 core) |
| `compilar-loja.sh --testes` | **116** Rust (100 núcleo + 16 interface) |
| `testar-apps-pc.sh` | **90** Python (58 forge + 32 painel) |
| apps no catálogo do aparelho | **0** — os sete são host ou shell, e o catálogo exclui os dois (§8) |

O que roda no aparelho hoje é **o launcher**. Com catálogo vazio ele desenha
"Nenhum jogo instalado." e continua navegável — catalogar ausência como ausência
é o comportamento certo, e já houve o oposto aqui (quatro jogos de demonstração
que não existiam no aparelho e davam erro ao apertar `A`).

## 1. Objetivo e fronteira

| Repositório | Função |
| --- | --- |
| **Giochi app** (este) | Apps do sistema, ferramentas de PC, arte e manifesto de requisitos |
| **Giochi System** | Sistema Linux limpo: kernel, rootfs, boot, drivers, painel |
| **Giochi os** | Junta sistema + apps e gera a imagem final por aparelho |
| **repositório de jogos** | Os jogos 3D que moravam em `app/jogos/` até 14/11/2026 |

Cada pasta é **repositório Git individual**. A comunicação é sempre por
**artefato declarado explicitamente**, nunca por caminho relativo presumido.

Localização: `C:\Users\Jussara dias\Desktop\plataforma sistema\Giochi app`
(Windows) e `/home/charles/Giochi/GIOCHI-APP` (Linux). GitHub:
`https://github.com/xtrempkch-droid/GIOCHI-APP`.

## 2. Estrutura

```text
app/
├── apps.json              # fonte da verdade: id, folder, runtime, status, distribution, license, role
├── _template/             # modelo para app novo
├── launcher/              # Rust + SDL2 — a interface do aparelho
│   ├── Cargo.toml         # workspace: crates/core + crates/ui-sdl
│   ├── crates/core/       # 8 módulos, 107 testes, sem SDL
│   └── crates/ui-sdl/     # 15 módulos, 134 testes, KMSDRM + evdev + catálogo
├── launcher-teste/        # o mesmo launcher sobre a GIOCHI-ENGINE (scaffold)
├── launcher-star-dust/    # o launcher + camada de efeitos (ready)
├── forge/                 # empacotador/classificador (host, Python)
├── store/                 # baixa/paga e envia ao aparelho (host, Python)
├── painel/                # interface web local (host, Python)
└── loja/                  # loja gráfica (host, Rust + SDL2)
schemas/                   # metadata.schema.json + forge.schema.json
scripts/                   # validador, empacotadores, ferramentas do aparelho
tests/                     # 199 testes JS + suites Python de forge e painel
docs/                      # 20 páginas
.github/workflows/         # validate.yml, pc-apps.yml, loja.yml
```

### Anatomia de um app

```text
app/<slug>/
├── metadata.json     # identidade do pacote (obrigatório)
├── forge.json        # requisitos de hardware (opcional)
├── capa.png          # 320x320, capa da loja
├── src/              # CÓDIGO-FONTE
├── bin/              # O QUE VAI PARA O APARELHO
├── assets/           # ARTE-FONTE: logos, fontes, arquivos editáveis
└── README.md
```

**A distinção `src/` × `bin/` × `assets/` é a decisão estrutural mais importante.**
`src/` é o que se edita, `bin/` é o que o aparelho executa, `assets/` é material
de trabalho que não viaja. Confundir isso gera pacote de centenas de megabytes com
arquivos editáveis dentro.

**Grupos** (`app/<grupo>/<slug>`) continuam suportados pelo validador e não são
usados hoje. O que distingue grupo de app é o `metadata.json` — pasta com o
arquivo é app, pasta sem ele é grupo, e a varredura desce um nível. Profundidade
máxima: 2.

## 3. Contratos

### `app/apps.json`

```json
{ "id": "launcher", "folder": "launcher", "runtime": "native",
  "status": "ready", "distribution": "image",
  "license": "Proprietary", "role": "shell" }
```

| Campo | Valores | Observação |
| --- | --- | --- |
| `id` | slug kebab | identidade estável; é o nome da pasta no aparelho |
| `folder` | caminho em `app/` | pode ter um nível de grupo |
| `runtime` | `godot`, `retroarch`, `proton`, `native` | `native` cobre Rust, C++, Python |
| `status` | `scaffold`, `ready` | `scaffold` rebaixa `entrypoint` ausente a **aviso** |
| `distribution` | `image`, `store`, `host` | decide destino **e** licença |
| `license` | SPDX ou `Proprietary` | precisa bater com o `metadata.json` |
| `role` | `shell` | opcional; fica fora do catálogo |

`role: shell` existe porque o launcher tem `status: ready`, `distribution: image`
e binário no lugar — entrava por todos os critérios, e o cartão "Launcher"
abriria um **segundo** launcher, que perde o DRM (exclusivo) e morre com
`kmsdrm not available`. O papel é **declarado**, não inferido do nome: fixar
`id === "launcher"` funcionaria até alguém renomear a pasta. Hoje vale para os
três apps de interface.

Pasta em `app/` fora do `apps.json` reprova com `app_nao_registrado`: um app
invisível para a loja é pior que um app quebrado.

### `distribution` decide o destino e a licença

| Valor | Onde roda | Efeito no licenciamento |
| --- | --- | --- |
| `image` | dentro da imagem assinada | exige licença permissiva |
| `store` | instalado em `/user/apps`, enviado pela loja | aceita GPL |
| `host` | no **computador do usuário** | não vai para o aparelho |

O validador **recusa mecanicamente** GPL/AGPL/LGPL com `distribution: "image"`
(código `licenca_gpl_na_imagem`). É o que permite vender app GPL: o pacote
baixado não faz parte da imagem assinada e o comprador pode substituí-lo em
`/user` — exatamente o procedimento sem chave que a GPL-3.0 §6 exige.

`arch` é validado **em par com `distribution`**: `aarch64` para `image`/`store`,
`x86_64`/`arm64`/`any` para `host`. Códigos
`arquitetura_de_console_em_app_de_pc` e `arquitetura_de_pc_em_app_do_aparelho`.
Só ampliar o enum teria deixado o campo voltar a não significar nada.

### `metadata.json`

Obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`, `entrypoint`.
Opcionais: `graphics_api`, `buttons_map`, `license`, `licenseUrl`, `releaseYear`,
`category`, `slug`. `additionalProperties: false` — campo desconhecido reprova.
Semver obrigatório (`1.0.0`, não `1.0`).

- **`slug`** — identidade do pacote. Derivar do **título** faria "Editor" →
  "Editor Pro" mover o app de lugar, quebrando catálogo e favorito. Ordem:
  `metadata.slug` → `id` do `apps.json` → título (último recurso, e o pacote diz
  que usou). Um `slug` derivado do título já instalou em
  `/opt/giochi/apps/pac-man` enquanto o catálogo apontava para `pacman`.
- **`releaseYear`** e **`category`** viajam numa cadeia de três elos —
  `metadata.json` → índice da loja → catálogo do launcher. `0`/ausente é
  *desconhecido* e a UI mostra `—`. **Nunca** derivar do índice do app.

**Substituto de dado ausente precisa ser declarado ausente, não preenchido com
número interno.** A Home já mostrou `Spacewar • Ação • #002`, que parecia
informação do jogo e era o índice do cartão.

### `forge.json`

Requisitos de capacidade — todas opcionais: `arch`, `runtime`,
`graphics_api_min`, `graphics_api_fallback`, `godot_version`, `ram_min_mb`,
`vram_min_mb`, `display_min_width/height`, `touchscreen_required`.

**Nunca declara `BR2_PACKAGE_*`.** O Buildroot descarta símbolo inexistente em
silêncio; um app que declarasse pacote produziria build verde com biblioteca
ausente. As dependências ficam no `Config.in` do **Giochi System**.

`graphics_api_min: "none"` existe para app que roda em software (SDL2 puro). O
**default continua `gles3`**: omitir num app gráfico tem de ser conservador —
`none` é declaração, não esquecimento. A cadeia de checagem tem `else` que
reprova valor desconhecido (`gles4`, erro de digitação).

## 4. Comandos

```bash
node scripts/validate-app.mjs --all         # valida todos
node scripts/validate-app.mjs app/loja      # valida um
node tests/run-all.mjs                      # 199 testes JS (lista vem de readdir)
bash scripts/testar-apps-pc.sh              # 90 testes Python (forge + painel)

bash scripts/compilar-launcher.sh           # build x86-64, resolve o SDL2
bash scripts/compilar-launcher.sh --testes  # 241 testes
bash scripts/compilar-loja.sh --testes      # 116 testes

bash scripts/compilar-launcher-arm64.sh                       # cross-compile aarch64
bash scripts/compilar-launcher-arm64.sh --app launcher-star-dust --instalar
bash scripts/compilar-launcher-teste-arm64.sh --instalar
node scripts/empacotar.mjs app/launcher --saida dist/pacotes  # .giochi assinado
node scripts/verificar-pacote.mjs dist/pacotes/launcher.giochi
```

**Não rode `cargo test --workspace` direto em `app/launcher`.** O crate linka com
`rust-lld`, que ignora `LIBRARY_PATH`; sem
`RUSTFLAGS=-L .../giochi-sdl2/root/usr/lib/x86_64-linux-gnu` o link falha com
`unable to find library -lSDL2` **mesmo com o build passando**. Os dois scripts
já exportam isso — use `--testes`.

**Não use `node --test tests/`**: o runner spawna um processo por arquivo e sob
sandbox falha com `spawn EPERM`. O runner próprio existe por isso, e desde
14/11/2026 ele **descobre** os `.test.mjs` por `readdir` em vez de uma lista
escrita à mão — a lista fixa deixou um arquivo novo de fora e o resultado saiu
"164/164" com menos cobertura.

## 5. Decisões que não se reabrem sem o dono pedir

- **Pasta `app/` (singular)** — escolha do usuário.
- **`runtime: "native"`** no enum — sem ele, ferramentas em Python não tinham
  como se declarar.
- **`status: scaffold`** — espelha o padrão `draft` do Giochi System e evita CI
  vermelho permanente durante migração. Todo o resto continua sendo erro.
- **Licença do repositório: MIT.** Exclusividade sobre o sistema foi descartada
  conscientemente.
- **Licença dos apps próprios: `Proprietary`.** O comprador não pode
  redistribuir; é o que dá base para cobrar pelo binário.
- **A loja roda no PC do usuário**, não no aparelho.
- **Migração por script, não por cópia manual** — projetos com assets binários
  copiados por API produzem árvore incompleta e sem histórico.

## 6. O launcher — o app central

Rust + SDL2, **renderer próprio**. Alternativas descartadas: `egui` (desenho
custom limitado) e Slint (DSL + runtime grande para 1 GB de RAM). SDL2 já está no
Buildroot e o KMSDRM dispensa compositor.

| Peça | Onde |
| --- | --- |
| estado, foco, reducer, grade | `crates/core` (107 testes, sem SDL) |
| janela, desenho, rotação, evdev, catálogo | `crates/ui-sdl` (134 testes) |

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

**O sistema tem HCI; o que falta é o transporte HID do kernel.** Medido no X55:
`hci0`, rfkill liberado, firmware `rtl8821cs` carregado, `bluetoothd` no ar,
BlueZ 5.72. Mas `modinfo hidp` mostra `depends: bluetooth` (sem `hid`),
`strings hidp.ko | grep hid_register_driver` não devolve nada e
`/sys/bus/hid/drivers/hidp` não existe: o módulo é **só a camada de socket**. Com
o controle ao lado ele conecta e cai — `disconnected with reason 2` — sem criar
`/dev/input/event*`. Falta também `/dev/uhid` para controle BLE. **É da imagem,
não do app.**

```bash
bash scripts/aparelho.sh bluetooth      # a pilha inteira, com veredito
bash scripts/aparelho.sh parear <MAC>   # pareia/conecta com a sequência certa
```

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

`catalogo.rs` lê `/opt/giochi/apps/index.json` e resolve `id → dir + entrypoint`.
`GIOCHI_CATALOGO=<arquivo>` aponta outro catálogo (existe porque no PC não há
`/opt/giochi/apps/`). `--catalogo-adicionar` é como o app registra um pacote
recém-instalado — não por `sed`, que produziria JSON inválido no primeiro campo
com vírgula.

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

## 7. Launcher-teste e Star Dust

### `launcher-teste` (scaffold)

O **mesmo desenho sobre a GIOCHI-ENGINE**, em app separado para não arriscar o de
produção. Reusa o crate `core` e as telas reais por `#[path]`; foi ele que provou
o porte: **100,0 % pixel a pixel** com o launcher em 640×480, 800×600, 1280×720 e
720×1280 girado, e nas seis telas.

Depende de `../../../GIOCHI-ENGINE-3D/giochi-engine` — **repositório irmão**. Sem
ele o build falha; é a única dependência fora deste repo.

Segue `scaffold` porque não tem sono, brilho, volume, energia nem Wi-Fi/BT: é um
banco de prova de desenho, e chamá-lo de `ready` prometeria o que ele não faz.

### `launcher-star-dust` (ready)

O launcher de produção **mais** a camada de efeitos. Ligada por padrão
(`--sem-efeitos` ou `GIOCHI_EFEITOS=0` desliga); desligada, o quadro é
**idêntico byte a byte** ao do launcher — propriedade medida em quatro tamanhos.

Oito efeitos (`efeitos.rs`): `brasas`, `brilho`, `varredura`, `vinheta`,
`scanlines`, `halo` do foco, `rastro` e `borda` pulsante. A camada inteira custa
**29 chamadas de desenho**.

O que a medição ensinou, e que vale para qualquer efeito novo:

| Versão | Custo | Causa |
| --- | --- | --- |
| uma chamada por brasa + `stroke_round_rect` no halo | +7,3 ms | 162 chamadas; o halo sozinho +6,9 ms |
| batelada por faixa de alfa | **+3,1 ms** | 27 chamadas |

**`draw_rect` é uma chamada de backend por retângulo.** O halo com
`stroke_round_rect` desenhava 4 arestas e 4 arcos de ~30 `draw_line` cada para
uma borda que ninguém vê; quatro bandas preenchidas num `fill_rects` leem igual e
custam 4× menos. A camada acrescenta **zero** divergência entre PC e aparelho
(98,10 % sem efeitos × 98,15 % com) — se introduzisse desenho dependente de GPU,
esse número teria caído.

O `halo` lê a geometria que a **tela publicou** (`layout.hits`), não recalcula:
recalcular seria a segunda verdade que já desalinhou o carrossel do foco.

A cadência de animação é própria (`GIOCHI_EFEITOS_FPS`, padrão **30**): a UI só
redesenha quando algo muda, e um efeito animado muda sempre. Não é 60 porque a
diferença visual em brasas de 1–3 px é nula e animação é contínua — interação é
que precisa de resposta imediata. Valor inválido cai no padrão; número que não
parseia não pode zerar a animação em silêncio.

O `rastro` é disparado por `cmd.is_user_input()`, **não** por "o estado mudou":
com o gatilho errado ele custava 20 chamadas num quadro em que ninguém apertou
nada, e o efeito perdia o significado ("algo aconteceu") virando ruído
permanente. A camada é opcional no `launcher-teste` justamente porque lá a medida
é a fidelidade de desenho; aqui ela **é** o motivo do app.

```bash
bash scripts/verificar-star-dust.sh              # 640x480
bash scripts/verificar-star-dust.sh --tamanho 1280x720
bash scripts/verificar-efeitos.sh                # 4 tamanhos + 1 rotação
```

Cinco perguntas em cada: desenhou? é determinística? some quando desligado? cabe
no orçamento de chamadas? desligada é idêntica à produção? O determinismo vem de
animar por **contagem de quadros**, nunca pelo relógio. E `verificar-efeitos.sh`
tem contador e `exit 1`: a primeira versão imprimiu "ok camada verificada" depois
de um FALHA, porque o resumo não olhava o que tinha acontecido. **Um resumo que
ignora falha é pior que nenhum.**

## 8. O catálogo do aparelho

**Catálogo é projeção, não segunda fonte da verdade.** Sai de `app/apps.json` +
os `metadata.json`:

```bash
bash scripts/aparelho.sh catalogo        # gera, confere no aparelho e publica
node scripts/gerar-catalogo.mjs --lista  # quem entra e quem fica de fora
```

Antes ele era escrito à mão e o aparelho mostrou **um** cartão com dois apps
instalados — e um app fora do JSON ficava invisível sem erro em lugar nenhum: nem
no app, nem no log, nem na tela.

| Regra | Por quê |
| --- | --- |
| `status: scaffold` fica fora | `bin/` vazio; cartão que não abre é pior que cartão ausente |
| `distribution: host` fica fora | roda no PC |
| `distribution: store` fica fora | é instalado em `/user/apps` pela loja |
| `role: shell` fica fora | senão o launcher **lista a si mesmo** |
| `entrypoint` precisa existir | idem scaffold, mas medido no arquivo |

**Hoje o catálogo sai vazio (0 apps)** — os sete apps são host ou shell. Não é
defeito: é o resultado correto das regras de projeção. Assim que um app `image`
sem `role: shell` existir, ou um pacote da loja for instalado, ele aparece.

**A conferência que faltava é no aparelho, não no repo.** O gerador checa o
binário na árvore; quem importa é o aparelho. `aparelho.sh catalogo` percorre cada
`dir/entrypoint` por SSH e **reduz** o catálogo antes de publicar, dizendo o que
tirou e como instalar. Medido: remover o binário do aparelho fez `gerado: 3`
virar `catálogo reduzido para 2`. **Projeção de verdade reflete remoção, não só
adição.**

**O `dir` do aparelho não é derivável do repo.** `<grupo>/<slug>` no repositório é
`/opt/giochi/apps/<id>` no aparelho. É convenção do `Giochi System` e a única
informação do catálogo que não vem do `apps.json` nem do `metadata.json` — por
isso está explícita em `scripts/lib/catalogo.mjs` e tem teste.

**O catálogo inclui o que veio da loja.** Um `.giochi` não está no `apps.json`; a
colheita lê `/opt/giochi/apps/*/metadata.json` **e** `/user/apps/*/metadata.json`
e o disco vence. E a mesclagem **herda** `license` do `apps.json` — substituir a
entrada inteira apagava a licença.

**Vigiar: foco e desenho descrevem a mesma lista.** `settings_rows()` no núcleo
decide até onde o foco anda; `views::linhas_da_categoria` monta o que é desenhado.
Dois lugares descrevendo a mesma lista foi exatamente como o carrossel ficou
dessincronizado do foco; há teste reprovando se divergirem.

## 9. Os apps de PC

### forge e store — Python, biblioteca padrão apenas

Decisão de projeto: a loja tem de funcionar numa máquina limpa. Isso torna o
`.pyz` possível — **um** arquivo, idêntico nos três sistemas — e o PyInstaller
desnecessário para quem já tem Python 3.9+.

| | `.pyz` | PyInstaller |
| --- | --- | --- |
| artefatos | um | um por sistema e por arch |
| determinístico | **sim** | não |
| tamanho | ~14 KB | ~10 MB |
| precisa de Python | **sim** (3.9+) | não |

`scripts/lib/zip.mjs` escreve o ZIP à mão pela mesma razão do `tar.mjs`: o
`zipapp` oficial grava o `mtime` de origem e dois empacotamentos do mesmo código
dariam SHA-256 diferentes — o checksum deixaria de identificar conteúdo. Medido:
mesmo SHA-256 em duas rodadas.

```bash
bash scripts/testar-apps-pc.sh            # 90 testes
node scripts/empacotar-pc.mjs             # .pyz + .sh + .cmd + SHA256SUMS
app/painel/bin/giochi-painel              # http://127.0.0.1:8710
```

**`bin/` tem lançador POSIX** que roda o módulo de `../src`: o repositório clonado
funciona sem instalar nada. **Windows não executa `sh`** — ali o caminho é o
`.cmd` da release, e isso está na documentação em vez de ser escondido.

**O erro clássico de batch:** `py ... %* && exit /b %ERRORLEVEL%` não funciona —
em `.cmd` o `%VAR%` é expandido quando a linha é **lida**, então `%ERRORLEVEL%`
vale o valor de antes. O atalho usa `where py` numa linha e `if %ERRORLEVEL%==0`
na seguinte, e diz o que instalar se nenhum Python existir. Duplo-clique que abre
e fecha sem texto é o pior resultado possível.

O `sdl2` 0.37 tem `default = []`, sem `use-vcpkg`: o `sdl2-sys` emite só `-l SDL2`
e deixa o linker procurar, então exportar `SDL2_DIR` não faz nada. O que funciona
no MSVC é `LIB`, com os zips oficiais "VC" (`include/` + `lib/x64/`).

### forge — classificação

Perfis medidos em `device.conf` do Giochi System: `x55`, `x35h`, `x35s`, `r35s`
e três genéricos por chip (`rk3326-1gb`, `rk3326-2gb`, `rk3566-2gb`).

Três regras que só existem porque a medição derrubou a crença anterior:

1. **O painel é do aparelho, não do SoC.** X55 é 720×1280 e X35H é 640×480, os
   dois com RK3566. Um perfil por chip reprovava no X55 um app que roda lá.
2. **A tela é comparada com o espaço de projeto, não com o painel cru.**
   `usable_display()` = maior lado × menor lado. O X55 é retrato, mas nenhum app
   desenha em retrato: 720×1280 → 1280×720, e um app 1280×720 **cabe**.
3. **`godot_runtime` tem três estados.** `False` = medido que não tem (X55, X35H,
   X35S), `True` = medido que tem, `None` = não medido (perfis genéricos). Só
   `False` reprova: **não medir não é reprovar**. Nenhum build oficial do Godot
   tem KMSDRM, então `runtime: godot` é **Incompatível** — antes era classificado
   como compatível, o falso verde que este repositório combate.

A RAM dos perfis é a **medida** (`/proc/meminfo`: 1968 MB no X55, 964 MB no
X35H), não o valor nominal do SoC — o firmware reserva memória para a GPU, e um
app que declara "preciso de 2048 MB" passaria no número redondo e morreria no
real.

E o rootfs é **glibc 2.38**, não musl. "Recompilar contra musl" não resolveria
nada: o bloqueio de Godot é KMSDRM, não a libc.

### painel — a interface web dos apps de PC

Serve `127.0.0.1:8710`, mostra os aparelhos reais, classifica um app e empacota o
`.giochi`. **Chama o forge e o store; não os copia** — uma segunda implementação
da regra de compatibilidade envelheceria diferente da primeira, e a primeira é a
que decide o que o usuário pode comprar. A separação `acoes` × `servidor` é o que
permite testar sem abrir socket.

**`importlib.resources`, não `Path(__file__)`.** Dentro de um `.pyz` não existe
diretório ao lado do módulo: `__file__` aponta para dentro do zip e
`Path.exists()` devolve `False`. Com `Path`, o painel **empacotado** serve 404 na
própria página. E `__package__` é `None` no 3.9 dentro de um zip (o `zipimport`
legado de lá não preenche), então `resources.files(None)` estoura com
`AttributeError` — só no `.pyz`, só no piso. A guarda tem de **executar o zip**, e
ela roda sob `python3.9` quando ele existe.

**Loopback não basta.** O servidor escuta só `127.0.0.1` **e confere o cabeçalho
`Host`** contra `127.0.0.1`, `localhost` e `[::1]`. Sem isso, uma página na
internet resolve um domínio para o loopback e fala com o painel (*DNS
rebinding*): a origem da requisição não protege nada nesse cenário. Caminhos
estáticos recusam `..` antes de tocar no disco.

Ele não instala no aparelho (isso é `aparelho.sh instalar-pacote`, por SSH) e não
guarda chave nenhuma: recebe o caminho de uma chave existente e repassa ao
`sign_package`.

### loja — Rust + SDL2, três sistemas

Resposta à pergunta que originou tudo — *"não quero que usem a chave para roubar
os apps"*: **uma chave pública não permite nada além de conferir assinaturas.**
O risco real nunca foi ela.

| Risco | Mitigação |
| --- | --- |
| chave **privada** vaza | `invalida_antes` no catálogo: o que foi assinado antes do corte não vale |
| **"autêntico" tratado como "comprado"** | licença separada, amarrada ao aparelho |
| catálogo trocado na rede | catálogo assinado; item sem assinatura não entra |
| servidor de pacote comprometido | sha256 + assinatura do `.giochi` |

Daí **três chaves e três perguntas**:

| Pergunta | Responde | Privada vive em |
| --- | --- | --- |
| "veio da Giochi?" | assinatura do pacote | servidor |
| "este aparelho pagou?" | **licença** (chave separada) | servidor |
| "é este aparelho?" | par gerado **no aparelho** | aparelho |

No código são **tipos diferentes**: `PacoteVerificado` prova origem e nada mais;
`LicencaAssinada` prova compra e exige a chave do aparelho. Confundir os dois é o
que faria um único pacote copiado servir para todos.

O que ela **não** faz, de propósito:

- **Não vê cartão.** Checkout hospedado no navegador (PCI-DSS SAQ A).
- **Não se declara paga.** `Estado::Pago` carrega `LicencaAssinada`, e a chave de
  licença não está no cliente. Não é disciplina, é o sistema de tipos — o teste
  aperta "Já paguei" dez vezes e exige que continue `Aguardando`.
- **Não inventa código PIX.** Sem provedor, a tela diz isso. Um "copia e cola"
  falso é a pior falha possível: o usuário tenta pagar.

**`butlerd` foi decidido não usar, com número:** é um daemon JSON-RPC de download
incremental para builds de dezenas de GB; um pacote de 81 KB custaria um processo
a supervisionar para economizar zero. **Regra para reconsiderar: pacotes acima de
~500 MB.** Antes disso, `curl -C -` resolve e já existe nos três sistemas.

**A mensagem assinada é montada em Rust (verifica) e em JS (assina).** Ela já
divergiu por um caractere (`item=<slug>|` contra `item|<slug>|`) e o catálogo
assinado **nunca** verificava, com a mensagem "assinatura inválida, não foi a
Giochi" mandando procurar no lugar errado. Hoje os dois lados têm o mesmo literal
com teste, e `giochi-loja --mensagem-catalogo` imprime os bytes.

**O layout é emprestado, não integrado.** Inspiração na forma de grade de cartões;
**zero** chamadas a serviços externos. A lista vem do catálogo da Giochi. E a
arquitetura de provedor é explícita: o provedor faz o checkout e **conta as
instalações** (`activation_limit` fica no servidor dele), o que resolve sem
backend nosso. Credencial de conta de usuário (API key) prova posse, não vínculo
com o aparelho — não serve para vender.

**Uma função decide a grade; desenho e navegação leem dela.**
`giochi_loja_core::grade::grade_para` + `mover`, com 11 testes incluindo um de
**propriedade** (nenhuma tecla produz índice fora da lista, em nenhuma grade, com
qualquer total). No launcher este projeto já teve o bug oposto — navegação
supondo 4 colunas com a tela calculando 2 — e o sintoma foi "a seta para baixo não
faz nada".

**Capas são desenhadas, porque o PNG só existe depois do download.** FNV-1a
(**não** `DefaultHasher`, que não é estável entre versões do compilador) → matiz,
matiz do detalhe e deslocamento da faixa. A cor **colidiu de verdade** (dois apps
na mesma matiz de 8 bits em 12 itens), então ela não é o único diferenciador e o
teste exige que o **conjunto** seja único. Um teste separado *documenta* a
colisão da matiz sozinha, para ninguém "consertar" achando que é bug.

## 10. O pacote `.giochi`

```text
app/<slug> ──empacotar──▶ <slug>.giochi (+.sig +.manifest.json)
            ──instalar-pacote──▶ /user/apps/<slug>
            ──catálogo──▶ cartão na tela ──▶ o app
```

**O `tar` é escrito à mão** (`scripts/lib/tar.mjs`) por três motivos medidos:

1. **Ordem** — o `tar` do sistema usa a ordem do `readdir`; aqui a ordem é
   contrato e a montagem é uma função só, testável.
2. **Reprodutibilidade** — `mtime`/`uid`/`gid` fixos. Dois empacotamentos do mesmo
   conteúdo dão **o mesmo SHA-256** (conferido). Se mudasse, a assinatura deixaria
   de identificar o conteúdo.
3. **Bit de execução** — `tar --mode` mascara permissão, e o erro só apareceria no
   aparelho como "entrypoint não existe".

Verificado contra o GNU tar: ele lê o ustar escrito à mão, extrai com as
permissões certas e `test -x` passa. Sem essa conferência seria o meu parser
concordando com o meu escritor.

**O extrator é script copiado, não `sh -c` aninhado.** A primeira versão montava o
comando com aspas dentro de aspas dentro de `ssh` e produziu
`sh: "bin/giochi-pacman": unknown operand`. Aninhar três níveis de aspas para
falar com outra máquina não é difícil de escrever — é difícil de **ler quando
falha**. Hoje é `scripts/lib/extrair-pacote.sh`, copiado e executado.

**App da loja vai para `/user/apps`, não para `/opt`.** `/opt/giochi/apps` é o
mesmo lugar dos apps **da imagem**, e gravar ali destrói o argumento que justifica
a loja existir: `distribution: store` significa "não faz parte da imagem
assinada", e é isso que permite vender app GPL.

### A assinatura tem um formato só, e é o do antecessor

| Guarda | Onde |
| --- | --- |
| `manifesto.signature` aponta o arquivo `.sig` | `assinar()` + teste JS |
| assinatura com tamanho diferente de 64 é recusada | `verificar()` + teste JS |
| o `.sig` tem 64 bytes e nenhum `\n` | `tests/painel/test_acoes.py` |

Os dois lados já divergiram: o Python gravava 64 bytes crus e o JS gravava base64
**em texto** (89 bytes com `\n`). Os testes JS assinavam e verificavam com a mesma
biblioteca, então passavam com qualquer formato. A prova que faltava é
**ferramenta externa**:

```text
openssl pkeyutl -verify -pubin -inkey chave-publica.pem \
  -rawin -in app.giochi -sigfile app.giochi.sig
Signature Verified Successfully     ← tanto o .sig do Python quanto o do JS
```

### A verificação no aparelho existe

`verificador-ed25519` (8.912 bytes, `EVP_DigestVerify`, ligado a `libcrypto.so.3`)
roda no aparelho: **20 MB de RSS, 0,01 s**. Ele confere o **tamanho** antes de
verificar (um `.sig` de 89 bytes é base64 gravado como texto, e o
`EVP_DigestVerify` recusaria com uma mensagem que não aponta a causa) e confere o
**tipo de chave** (uma chave RSA falharia na verificação e o veredito sairia
"assinatura não confere", culpando o pacote por erro de configuração).

`instalar-do-cartao.sh` recusa antes de extrair e **avisa** quando não tem
verificador ou chave. A primeira versão verificava em silêncio — o que torna
"verificou e passou" indistinguível de "não verificou". **Silêncio no sucesso é o
defeito espelhado do silêncio no fracasso.**

**Integridade não é autenticidade.** SHA-256 pega cópia incompleta — o erro real
de quem copia pelo leitor de cartão. Quem escreve no cartão reescreve o `.giochi`
**e** o manifesto, e o hash confere.

## 11. Verificação no aparelho

```bash
bash scripts/aparelho.sh procurar     # 10.55.0.2 (usb0), depois varre a /24
bash scripts/aparelho.sh diagnostico  # sistema, tela, áudio, entrada, libs
bash scripts/aparelho.sh bluetooth    # a pilha inteira, com veredito
bash scripts/aparelho.sh instalar     # compila, instala, confere sha256
bash scripts/aparelho.sh subir        # inicia o launcher (o init não faz sozinho)
bash scripts/aparelho.sh verificar    # processo + DRM + frame comparado
bash scripts/aparelho.sh capacidades  # códigos que cada /dev/input declara
bash scripts/aparelho.sh catalogo     # gera e publica o índice de apps
bash scripts/aparelho.sh boot-log     # salva o log ANTES de regravar o cartão
```

**`verificar` é o que importa**: para o app (o KMSDRM é exclusivo), captura um
quadro, desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`, des-rotaciona pela
orientação que o sistema declara e compara pixel a pixel. No X55 dá **98,9 %** e
no X35H **99,9 %**; com a rotação oposta cai para **10,9 %**. É essa diferença que
distingue "desenhou" de "desenhou de ponta a cabeça".

A comparação só vale com a **mesma entrada dos dois lados**: catálogo igual e
referência no espaço de projeto. As duas coisas já faltaram, e das duas vezes o
launcher pareceu pior sem ter regredido.

**Três coisas do ambiente mudam e precisam ser congeladas dos dois lados**, senão
a comparação reprova por hardware: relógio (aparelho sem NTP), bateria e
temperatura.

| Variável | Onde | Efeito |
| --- | --- | --- |
| `GIOCHI_HORA_FIXA` | `views::current_clock` | desenha o texto dado |
| `GIOCHI_BATERIA_FIXA` | `app::tick_energia` | nível fixo |
| `GIOCHI_TEMPERATURA_FIXA` | idem | °C fixo |

Antes delas cada uma derrubava a comparação para 99,5–99,9 % e nenhuma era
regressão de desenho. Depois: **100,0 %** nos dois aparelhos.

O `retry` de 99,5 % também já esteve **invertido**:
`awk 'exit !(p < 99.5)' && break` quebrava o laço quando o resultado era **bom**,
então a segunda tentativa nunca acontecia justamente no caso que ela cobre (o
minuto virando entre as duas capturas).

## 12. Ambientes medidos

| | X55 | X35H | R35S |
| --- | --- | --- | --- |
| SoC / GPU | RK3566 / Mali-G52 | RK3566 / Mali-G52 | RK3326 / Mali-G31 |
| painel | 720×1280 | 640×480 | 640×480 |
| `fbcon/rotate` | `0` + cmdline `3` → **Ccw90** | `0` → None | `0` → None |
| RAM | 1968 MB | 964 MB | 964 MB |
| acesso | `usb0` `10.55.0.2` + Wi-Fi | `usb0` + rede | `usb0` ou serial |
| `KEY_POWER` | sim (`rk805`) | sim | **não** |
| Wi-Fi / BT | sim | sim | **sem rádio** |
| `libasound` | sim | sim | **não** → volume `—` |
| `SDL2_ttf` | 2.22 | 2.22 | presente |

**Não confie no nome do aparelho nem no IP.** Leia
`/etc/giochi/build-info.txt`; o `10.55.0.2` já foi X35H e R35S no mesmo dia, e a
host key muda ao regravar (`ssh-keygen -R 10.55.0.2` — a mensagem fala de ataque
MITM e não é).

O R35S não tem rádio e o `dwc2` nasce como *host*: `aparelho.sh` (feito de
`ssh`/`scp`) não o alcança sem `usb0`. Quando o USB não sobe, a console serial
(`ttyS2` @ 1500000) é o único caminho:

```bash
bash scripts/console-r35s.sh "uname -r"      # um comando
bash scripts/console-r35s.sh --escutar 12    # só ouve (boot, panic)
```

Por dentro é um container `--privileged -v /dev:/dev` com a imagem do System: o
nó é `root:dialout` e o CH341 **cai do barramento**, então `-v /dev:/dev`
sobrevive e `--device=` fixado aborta quando o nó some. Leitores órfãos seguram a
porta e o erro aparece como `returned no data`, que manda procurar defeito no
cabo.

## 13. Armadilhas medidas

Cada uma custou pelo menos uma rodada. Nenhuma é hipótese.

### Ferramenta que mede a intenção em vez do efeito

Quatro ocorrências, e o padrão é sempre o mesmo: um número que parece medir o
efeito e mede o pedido.

| Onde | O que acontecia |
| --- | --- |
| contador de efeitos do `launcher-teste` | contava chamadas **dentro** do módulo; imprimia "25" com 182 saindo |
| `--teclas` do launcher | aplicava antes de `Screen::Running`; imprimia estado inicial, plausível e inalterado |
| `--tamanho` num painel real | projeto 640×480 num drawable 720×1280; o BMP saía com o conteúdo no canto |
| `verificar-efeitos.sh` | imprimia "ok camada verificada" depois de um FALHA |

Os contadores passaram para o ponto onde o backend é chamado
(`Painter::desenhos`), o `--teclas` passou a avançar o boot antes de aplicar, o
`--tamanho` compara com `janela.drawable_size()` e **avisa** quando o driver
ignorou o pedido (comparação com o drawable de verdade, não com o nome do driver,
então vale para qualquer backend), e o script ganhou contador e `exit 1`.

**Um teste que nunca falhou não provou nada.** Todo teste novo desta casa é
conferido revertendo a correção de propósito.

### Processo que procura a si mesmo

`case "$c" in *giochi-init*)`, `*giochi-launcher*` e `*bluetoothd*` casam com o
próprio `sh -c` que executa a busca, porque o padrão está no `argv`. Três
ocorrências. A comparação passou a ser por caminho **exato** ou por
`readlink -f /proc/PID/exe`.

E `pkill -f giochi-launcher` **mata a própria sessão SSH**. Use `pidof` + `kill`.

### `ps`, `pidof` e o descritor de DRM

- **`ps` mente nesta imagem:** listou 2 de 94 processos, e entre os invisíveis
  estavam `init`, `sshd`, `giochi-init` **e o launcher**. Leia `/proc/*/cmdline`.
- **`pidof` não acha script `sh`** (casa por `comm`, truncado em 15 caracteres):
  `pidof giochi-init` devolve vazio com o processo vivo.
- **"Vivo" não é "desenhando".** Um launcher pode estar vivo **sem** os descritores
  de `card0`/`renderD` — nunca respawnado, nunca desenhando. A pergunta é "quem
  segura o DRM".
- **O dono do DRM pode ser o app.** Com o launcher em `fork`+`wait` sem DRM, o app
  é o dono; matar o launcher ali deixa o app **órfão com o dispositivo**.

### O `giochi-init` desiste, e a cota é pequena

`RESTART_MAX=5` quedas em 60 s e ele **para de reiniciar**, dorme em
`sleep 3600` e grava `/run/giochi/launcher-parado`. Cada `verificar` derruba o
launcher uma vez — **seis rodadas seguidas deixam o aparelho sem interface**.

`subir` e `verificar` tratam isso: se o launcher não aparece, matam o
`giochi-init` (o `::respawn:` do inittab o reinicia com `restarts=0`) e esperam
mais. Três defeitos foram corrigidos nesse caminho: `SEM_DRM` era aviso e não
falha (o caminho de recuperação nunca era alcançado);
`/run/giochi/launcher-parado` nunca era apagado (o supervisor novo herdava a
desistência); e `pid=$(pids_launcher)` devolvia **dois** pids quando havia um
travado em estado `D`, fazendo o script anunciar tela preta com a tela
funcionando.

### `SIGTERM` não mata o laço

O SDL instala handler que só posta `SDL_QUIT` na fila. Um app que não bombeia
eventos (o `launcher-teste`) segue vivo segurando o KMSDRM. **`kill -9`**, e
esperar o descritor liberar. Mesma família:
`timeout 14 launcher --lancar X` **não termina** (o launcher está em
`fork`+`wait`) — a sessão SSH pendura e, quando ela morre, o filho fica órfão dono
do DRM.

E `timeout` na ponta do **host** não mata o processo remoto: matar o `ssh` deixa o
comando rodando no aparelho. O `timeout` tem de estar dentro do comando remoto, ou
não se usa.

### BMP: 32 bpp, e o swap

- O launcher escreve BMP de **32 bpp**. Qualquer script de análise tem de checar
  `bytesPorPixel` no cabeçalho e calcular o *stride* como
  `ceil(largura * bpp / 4) * 4`; ler como 24 bpp produziu uma rodada inteira de
  medições inválidas. Sanidade: girar 90° e voltar tem de dar **100,00 %**.
- **`--dump-frame` com backend acelerado grava preto**: o swap esvazia o buffer.
  Use `GIOCHI_GLES=0` (força `software`) na captura. Terceira vez que apareceu.
- **A inversa de `girar_rect(Ccw90)` não é "girar para o outro lado"** — as
  dimensões trocam junto. Errar produz uma **transposição**, invisível em quadros
  quase simétricos.

### `ssh` dentro de `while read` come o stdin

`no_aparelho` chama `ssh`, que lê stdin e o repassa ao shell remoto. Num
`while read ... do no_aparelho ... done <<< "$lista"` só a **primeira** linha é
processada — e foi assim que a conferência do catálogo examinou **só o primeiro
app** e publicou três cartões mortos sem uma linha de aviso. Duas trancas:
`</dev/null` no `no_aparelho` e `read <&3` com `3<<<` no laço.

### `sh -c` remoto com aspas duplas expande no PC

Corpo remoto entre **aspas duplas** faz o shell local expandir `$e`/`$n` antes de
enviar; com `set -u` isso vira `e: variável não associada`. Use **aspas simples**
com marcador (`__MAC__`) trocado no PC.

### Um `cfg` que nunca é compilado é código que não existe

`cargo build --no-default-features` ficou quebrado por semanas (faltava um campo
no inicializador do `Painter` na ramificação `not(feature = "texto")`) porque só o
build padrão rodava. O CI constrói as duas combinações.

### Ferramenta de build que sai calada

`INSTALAR="${2:-}"; shift 2` com um argumento só: o `shift 2` falha e `set -e`
mata o script **sem imprimir nada**. Um script de build que sai calado é
indistinguível de um build que não rodou. E `local` fora de função
(`local: somente pode ser usado em uma função`) passa pelo `bash -n`.

### Um `u16` de sentinela comparado com `Option` é sempre verdadeiro

`hud.temp_c: u16` usa `0` para "desconhecido" — não há valor que signifique "sem
leitura" num inteiro. Comparar `Option<u16>` com `Some(temp_c)` faz
`None != Some(0)`, que é **verdadeiro para sempre**, e a linha de log saía a cada
5 s num aparelho sem sensor (medido no R35S, 06/11/2026; corrigido em 14/11).

Duas lições juntas: **sentinela num tipo que não tem "ausente" convida a esse
erro**, e a correção certa é converter para o sentinela (`unwrap_or(0)`) em vez de
comparar tipos diferentes. O teste que fecha é o do caso silencioso — "sem
sensor, não registra de novo" — porque o caso barulhento já aparecia na tela.

### `is_ok()` é "a variável existe", não "está ligada"

`GIOCHI_GLES` era testada por presença, então `GIOCHI_GLES=0` **ligava** o GLES, e
a documentação dizia o oposto do que o código fazia. Pior: a variável era inútil
na direção documentada — sem ela o SDL já escolhe `opengles2` no KMSDRM. Hoje os
três estados fazem coisas diferentes (ausente = deixa o SDL escolher, `1` = pede
`opengles2`, `0` = pede `software`) e `Painter::new` **anuncia** qual backend
abriu de fato, porque `SDL_RENDER_DRIVER` inválido cai em silêncio para outro
driver — pedir não prova nada.

### CI: `cancelled` sem passos é infraestrutura

O job de macOS era **cancelado em 1 s sem passo nenhum**: `macos-14` foi
aposentado (`actions/runner-images#15226`). Virou `macos-15`.

**Verificação de CI não lê texto acentuado**: `-match "seleção: 2"` falha no
PowerShell 5.1 (code page OEM), e `$array -notmatch "x"` **filtra** em vez de
testar, então a condição fica sempre verdadeira. Daí `--json` com saída ASCII pura
e diagnóstico no stderr para o stdout ser só dado.

`exit 0` no fim de um passo `pwsh` que testa código de saída diferente de zero não
é redundância: o runner **anexa** `exit $LASTEXITCODE`, e o passo termina vermelho
por um sucesso que ele mesmo provocou.

E **filtro de `paths` faz parte do teste**: `app/painel/**` não estava no filtro de
`validate.yml`, então a correção do 3.9 não acionou o workflow. App fora do filtro
é app sem CI, e o sintoma é silêncio.

## 14. O que está provado e o que é afirmação

| Alegação | Como |
| --- | --- |
| launcher roda no aparelho | **medido**: KMSDRM, 100,0 % contra a referência do PC |
| rotação segue o sistema | **medido** nos três aparelhos, com o `cmdline` desempatando o `0` |
| botões do controle embutido | **medido**: cobertura completa dos 20 códigos do X55 |
| brilho, volume, dormir, desligar, reiniciar | **medido** no X55 |
| boot automático (`::respawn:` → init → launcher) | **medido** no X55: `ppid` do launcher é o init, 29 s depois do boot |
| Wi-Fi: varredura e lista | **medido** (`wi-fi: 11 rede(s) em wlan0`) |
| Wi-Fi: conectar com a senha certa | **não testado** — falta a senha |
| Bluetooth: parear | **medido** (`Bonded: yes`) |
| Bluetooth: virar entrada | **bloqueado na imagem** (falta transporte HID e `uhid`) |
| Star Dust desligado = launcher idêntico | **medido**, 4 tamanhos, byte a byte |
| launcher-teste 100 % pixel a pixel | **medido**, 4 tamanhos + 6 telas |
| `.pyz` e loja nos três sistemas | **medido** no CI (7 e 4 jobs verdes) |
| assinatura verificada no aparelho | **medido**: 20 MB de RSS, 0,01 s |

**Regra de honestidade:** nada aqui pode ser declarado "funciona" sem ter sido
executado. "Compila" e "roda" são afirmações diferentes — e o `poweroff` já caiu
por ter sido escrito antes de testado.

**Existir, declarar e ser usado são fatos diferentes.** Uma `.so` no rootfs não
prova que o app a usa (só `readelf -d` mostra as `DT_NEEDED`); um `/dev/input`
declarado no `/proc` não prova que o evento chega; uma variável de ambiente
documentada não prova que o código a lê (§13).

## 15. Arquivos que não devem ser commitados

`dist/`, `out/`, `*.giochi`, `node_modules/`, `.godot/`, `.DS_Store`, `.vscode/`,
`.idea/`, `target/` e `__pycache__/`.

**`bin/` deve ser commitado** — é exatamente o que vai para o aparelho.
**`app/launcher/Cargo.lock` também** — o launcher é binário e o lock fixa as
versões que foram de fato compiladas; o CI usa `--locked`.

## 16. O que falta

| Item | Estado |
| --- | --- |
| app `image` que não seja shell | **não existe** — por isso o catálogo sai vazio (§8) |
| licença consultada antes de lançar | **não feito** — o verificador existe, o launcher ainda não pergunta ao servidor |
| conexão Wi-Fi real | **não testada** — falta a senha de uma rede |
| pareamento BT virar entrada | **bloqueado na imagem** — pedido no canal (§17) |
| analógico | **não lido** — normalizar exige `absinfo` por ioctl |
| tema em tempo de execução | **não existe** — `theme::ACCENT` é `const` em centenas de lugares, e há **um** tema |
| `bash -n` no `build-device.sh` | fica no **Giochi System**, outro repositório |

## 17. Canal com o Giochi System

A conversa entre as equipes vive em `/home/charles/Giochi/chat_de_equipe.txt` —
**fora dos dois repositórios**, porque nenhum dos dois é visível para o outro lado.
Aqui há só um ponteiro: `docs/mensagem-giochi-system.md`.

**Janela: 08:00–18:50, horário de Brasília.** Fora dela não se responde nem se
inicia rodada. `bash scripts/canal.sh` responde, e `--exigir` sai com 1 para
encadear em script.

Pedido aberto (14/11/2026): **`CONFIG_HID=y` com o transporte BT-HIDP no
`hidp.ko`**, **`CONFIG_UHID=y`** e `hidp` carregado no boot. Verificação
objetiva: `modinfo hidp` lista `hid` em `depends`, e `/sys/bus/hid/drivers/hidp`
existe.
