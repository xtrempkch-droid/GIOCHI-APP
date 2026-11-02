# Integração — o que o launcher lê do sistema

Contrato entre o **launcher** (`Giochi app`) e o **sistema** (`Giochi System`).

Decisão de 02/11/2026: **ler direto do sistema de arquivos, sem daemon.**

---

## 1. Por que sem daemon

Um daemon significaria: mais um processo no boot, mais um protocolo para versionar,
mais um ponto que pode travar e deixar o console numa tela preta.

Para um launcher, ler um arquivo de `/sys` custa microssegundos. Não há motivo
para intermediar.

| | Ler direto | Daemon |
| --- | --- | --- |
| Peças no boot | 1 (launcher) | 2 |
| Protocolo | nenhum — caminhos de arquivo | precisa versionar |
| Se a fonte falha | valor `None`, UI mostra `—` | daemon morto = tela sem dado |
| Bloqueio de quadro | possível, se o `read` travar | não |

O único risco real é **`read()` bloquear**. Resolvido com cache e com leitura fora
do caminho crítico do desenho (§4).

---

## 2. Fontes

### 2.1 Catálogo de apps

**De onde:** `/opt/giochi/apps/index.json`

Formato, gerado pela fase `apps` do `Giochi os`:

```json
{
  "schemaVersion": 1,
  "apps": [
    { "id": "launcher", "version": "0.1.0", "entrypoint": "bin/giochi-launcher", "dir": "/opt/giochi/apps/launcher" }
  ]
}
```

**Fallback:** arquivo ausente → grade de apps vazia com mensagem. Nunca quebra.

**Cuidado:** `dir` vem do manifesto. O launcher deve resolver `entrypoint` **relativo
a `dir`** e recusar caminho que escape (`..`, absoluto). Um manifesto corrompido não
pode virar execução de binário arbitrário.

### 2.2 Identidade do aparelho

**De onde, em ordem:**

1. `/proc/device-tree/model` — string do device tree, com byte nulo no fim
2. `/etc/giochi-system-release` — `NAME=`, `VERSION=`
3. `/opt/giochi/device.json` — copiado pelo sistema, se existir

**Fallback:** "Giochi" genérico.

**Detalhe:** `/proc/device-tree/model` termina em `\0`. Ler bytes e cortar no
primeiro zero — não decodificar como UTF-8 direto, senão o nome sai com lixo no fim.

### 2.3 Bateria

**De onde:** varrer `/sys/class/power_supply/*` (o nome varia com o fuel gauge:
`BAT0`, `battery`, `rk817-battery`).

Para cada entrada, ler:

| Arquivo | Conteúdo | Uso |
| --- | --- | --- |
| `type` | `Battery` ou `Mains` | filtrar |
| `capacity` | 0–100 | percentual |
| `status` | `Charging`, `Discharging`, `Full`, `Not charging` | ícone |

**Não** escolher pelo nome da pasta: em RK3566 o gauge pode ser `battery`, em outro
aparelho `BAT0`. Filtrar por `type` é o que funciona nos dois.

**Fallback:** nenhum `Battery` encontrado → `None` → UI mostra `—`.

### 2.4 Temperatura

**De onde:** `/sys/class/thermal/thermal_zone*`

Para cada zona, ler `type` e `temp`. Escolher a zona cujo `type` contém `soc` ou
`gpu`; se não houver, usar a primeira.

**Unidade:** `temp` vem em **miligraus Celsius**. Dividir por 1000.

**Fallback:** sem zona → `None`.

### 2.5 TDP / consumo

**Não existe leitura direta hoje.** Não há arquivo padrão no sysfs que exponha watts
nesses SoCs; o consumo real depende do PMIC e do regulador, com leitura por I²C
específica do aparelho.

**Decisão (02/11/2026): o campo continua na HUD, mostrando `—`.**

Motivo: quando o suporte existir no `Giochi System`, o campo já está lá e passa a
mostrar número sem mudar a UI. Remover agora só criaria trabalho de recolocar
depois.

**Caminho previsto para quando houver suporte** — em ordem de probabilidade:

| Fonte | Como |
| --- | --- |
| Regulador do PMIC | `iio` em `/sys/bus/iio/devices/*/in_voltage*` × corrente |
| Driver de bateria | `power_now` ou `current_now` × `voltage_now` em `/sys/class/power_supply/*` |
| Específico do aparelho | leitura I²C no `device.json` do device |

Se `power_now` existir no fuel gauge, o TDP sai de graça com a leitura de bateria
que já vamos fazer. Vale checar no primeiro boot real: é o cenário mais barato.

**Regra:** enquanto não houver número medido, `—`. Nunca estimar.

### 2.6 Wi-Fi

**De onde:** varredura de `/sys/class/net/*/wireless` para achar a interface, depois
`operstate`.

| O que dá para saber | O que **não** dá |
| --- | --- |
| interface existe | SSID conectado |
| `operstate` = `up` / `down` | qualidade do sinal |
| endereço MAC | se está associada |

`operstate` diz que a interface está ativa, **não** que há conexão. Para SSID seria
preciso `nl80211` (socket, não arquivo).

**Decisão:** mostrar "ligado/desligado", nunca o nome da rede. Rótulo honesto:
`Wi-Fi` com estado da interface.

### 2.7 Bluetooth

**De onde:** presença de `/sys/class/bluetooth/`.

Mesma limitação do Wi-Fi: presença de adaptador não é o mesmo que ligado. Estado
real exige `rfkill`, que é `/dev/rfkill` — dispositivo de caractere, não arquivo.

**Decisão:** mostrar "adaptador presente / ausente". Não prometer mais que isso.

### 2.8 Brilho — por Fn + e Fn −

**Decisão (02/11/2026): o brilho é controlado pelo combo Fn + / Fn −**, não pelo
slider do painel de ajustes rápidos.

**De onde, para escrever:** `/sys/class/backlight/*/brightness`

- Filtrar pelo diretório que tenha `brightness` **e** `max_brightness`.
- Escrever valor entre 0 e `max_brightness`.
- Passo de **10%** por toque; segurar repete (o mesmo `Mapper` de repetição serve).

**Brilho é o único recurso deste contrato que o launcher realmente controla.** É
arquivo de texto, lê e escreve sem biblioteca.

#### O problema do Fn

"Fn" não é uma tecla que o Linux vê. Em handhelds, o combo Fn + algo costuma ser
resolvido pelo **controlador de teclado** (EC), que emite um scancode próprio — ou
pelo kernel, mapeado para `KEY_BRIGHTNESSDOWN`/`KEY_BRIGHTNESSUP`.

Não temos o hardware para saber qual dos dois acontece aqui. Então:

**Decisão: a tecla vira dado, não código.** O mapeamento `scancode → ação` fica em
`buttons.map`, e o launcher só consulta.

```json
{
  "brightness_down": { "key": "KEY_BRIGHTNESSDOWN" },
  "brightness_up":   { "key": "KEY_BRIGHTNESSUP" }
}
```

Consequência: **o `buttons.map` do antecessor precisa ser revisto antes de migrar**
(D5). É ele que traz o mapeamento real do aparelho. Sem isso, o código teria que
adivinhar dois scancodes.

**Se a tecla não chegar ao SDL**, o caminho alternativo é ler
`/dev/input/event*` direto para a tecla, ou — último recurso — um `udev`-like no
`giochi-init`. Fica registrado como plano B, não como desenho.

**Persistência:** guardar o último valor em `/user/.config/giochi/brightness` e
restaurar no boot. Sem isso, o aparelho volta ao brilho máximo a cada reinício — e
em handheld isso incomoda de verdade.

### 2.9 Som — ALSA direto

**Decisão: ALSA direto, sem servidor de som.** O sistema é Buildroot mínimo: não há
PulseAudio nem PipeWire, e não deve haver. Um servidor de som num console é mais
uma peça no boot e mais um lugar para o áudio sumir em silêncio.

#### O que já sabemos do antecessor (e é a armadilha principal)

O RK3566 usa codec **RK817** em I²C `0x20`, com `i2s1_8ch`, card chamado
`rk817_ext`, `mclk-fs = 256`, `hp-det-gpio = <&gpio4 RK_PC6>` e
`spk_amp enable-gpios = <&gpio4 RK_PC2>`.

E a armadilha, medida em hardware:

> **O `Playback Mux` do RK817 tem de ficar em `HP`.** O amplificador do alto-falante
> é alimentado por HPOL/HPOR; em `SPK` o aparelho fica **mudo**.

Isso não é bug de driver: é o roteamento da placa. Precisa vir configurado no
`Giochi System` (via `post-build`/`alsactl`) ou o áudio não sai do alto-falante.

#### Como o launcher controla o volume

Três caminhos, com custos diferentes:

| Caminho | A favor | Contra |
| --- | --- | --- |
| `alsa` crate (bindings libasound) | controle real, sem processo filho | adiciona dependência C ao binário |
| `amixer` por processo | sem dependência de link | exige `alsa-utils` na imagem; `fork` a cada ajuste; parsing de saída de texto |
| ioctl em `/dev/snd/controlC0` | sem extra | reimplementar parte do cliente ALSA; não vale |

**Recomendação: `alsa` crate.** O `amixer` por processo é frágil — depende de
`alsa-utils` estar na imagem, do formato da saída em texto, e cria um `fork` a cada
passo do slider. Vínculo com libasound é dependência pequena e estável, e o
Buildroot já empacota.

**Pendente de decisão:** o volume continua no painel de ajustes rápidos (como está),
ou também ganha combo Fn, igual ao brilho? Consistência sugere Fn; o protótipo já
tem o slider.

#### Controle exato

| Item | Valor |
| --- | --- |
| Card | `rk817_ext` (nome fixo; se mudar, falhar com mensagem clara) |
| Controle | `Playback` (verificar com `amixer -c rk817_ext scontrols` no hardware) |
| Faixa | 0–100% do controle, não dB |
| Passo | 5% por toque, repetição ao segurar |
| Mute | controle `Playback` no 0, ou `Playback Switch` se existir |

**Nada disso foi verificado em hardware.** Os nomes de controle são suposição
baseada no codec; o `amixer` real precisa ser lido no aparelho.

---

### 2.10 NPU — descartado como upscaler; e a ideia de "coprocessador"

**Decisão (02/11/2026): upscaling de áudio por NPU está descartado.** Motivo: exige
kernel BSP, exige camada de áudio, e disputa o acelerador com a IA dos jogos num
orçamento de 0,8 TOPS. Custo alto, resultado incerto.

O raciocínio completo fica registrado abaixo para não ser reproposto sem contexto.

#### Qualquer uso do NPU custa o mesmo

O preço não depende do que o NPU faz — depende de **existir**. Não há driver no
mainline; a pilha é `rknpu2` + `rknn-toolkit2` sobre kernel **BSP 6.1**
([rknpu2](https://gitcode.com/open3588/rknpu2), [rknn-toolkit2](https://gitcode.com/open3588/rknn-toolkit2)).

Então a pergunta nunca é "o NPU ajuda nesta tarefa?". É:

> Vale trocar o kernel mainline 6.12 — o que o antecessor validou em hardware —
> pelo BSP 6.1?

Enquanto a resposta for não, **nenhum uso do NPU se paga**, por melhor que seja a
tarefa.

#### "Processador de validação" — correção de categoria

Aqui há uma confusão que vale desfazer: **validação de integridade não é trabalho
de NPU.**

| O que é "validar" | Ferramenta certa | NPU serve? |
| --- | --- | --- |
| Conferir SHA-256 de uma imagem | CPU, ou o **motor de cripto** do SoC | **não** |
| Verificar assinatura Ed25519 | CPU, ou motor de cripto | **não** |
| Checar se um pacote é compatível | comparação de campos JSON | não |
| Detectar anomalia em build | fora do aparelho | não |

NPU faz **multiplicação de matrizes em int8**. Rodar SHA-256 nela é pior que na
CPU: hash é operação sequencial e dependente de estado, o oposto do paralelismo que
o acelerador oferece.

Se a ideia é **acelerar validação**, o componente certo é o **motor de criptografia**
do SoC (`rk_crypto`), que faz SHA/AES/RSA em hardware. Mas ele também é driver fora
da árvore — e o ganho sobre CPU, para conferir um checksum no boot, é irrelevante.

#### Usos que fariam sentido — se o BSP já estivesse decidido

| Uso | Encaixe | Comentário |
| --- | --- | --- |
| IA dos jogos | bom | é escolha do jogo, não do sistema |
| Supressão de ruído do microfone | bom | se houver microfone |
| Palavra de ativação (wake word) | bom | útil em portátil, exige mic |
| Upscaling de **vídeo** | fraco | GPU/FSR faz melhor com menos energia |
| **Validação / integridade** | **nenhum** | categoria errada |

Nenhum deles justifica trocar o kernel sozinho.

#### Recomendação

**Estacionar o NPU.** Não é "nunca" — é "não agora, e não por este preço".

Reabrir se:

1. aparecer driver de NPU no mainline para RK3566; **ou**
2. houver outro motivo para adotar o BSP (e aí o NPU vem de graça); **ou**
3. a IA dos jogos se tornar o argumento central do produto.

Estacionar agora **não custa nada**: nenhuma linha de código depende disso.

**E RK3326 continua fora** em qualquer cenário — não tem NPU.

---

### 2.11 Compactar/descompactar vídeo e áudio em tempo real — não é NPU

Pergunta justa, e a resposta é não — por dois motivos independentes.

#### 1. Codec é padrão bit-exato; NPU é aproximação

H.264, H.265 e AAC não são "tarefas parecidas com compressão". São **normas com
bitstream definido**: o decodificador do outro lado precisa aceitar exatamente
aqueles bytes.

Rede neural faz o contrário disso — aproxima, e o erro é aceitável porque o
resultado é uma imagem. Compressão neural existe em pesquisa, mas não produz
bitstream compatível com player nenhum, não é mais rápida que hardware dedicado, e
não é tempo real em 0,8 TOPS.

**É a segunda categoria errada da mesma família:** NPU não valida integridade
(§2.10) e não faz codec. Ela multiplica matrizes em int8.

#### 2. O SoC já tem o componente certo — e não é o NPU

| Tarefa | Componente correto | Onde |
| --- | --- | --- |
| Decodificar H.264/H.265 | **VPU** (decodificador de vídeo) | RK3566 tem, até 4K |
| Codificar H.264 | **VPU** (codificador) | RK3566 tem, 1080p |
| JPEG | **codec JPEG** dedicado | ambos |
| Escalar, converter formato, rotacionar, compor | **RGA** (acelerador 2D) | ambos |
| AAC, Opus, MP3, Vorbis | **CPU** | software basta |

Áudio nem precisa de acelerador: decodificar Opus custa poucos por cento de um
núcleo Cortex-A55.

#### O impasse do driver é o mesmo

O VPU do RK3566 é geração mais nova que a cobertura do `hantro`/`rkvdec` no
mainline, e os formatos suportados variam por SoC. Ou seja: **codec por hardware tem
o mesmo impasse do NPU** — funciona no BSP, incerto no mainline. Com um agravante:
aqui o ganho é grande, porque decodificar vídeo em CPU devora bateria.

**Se player de vídeo ou streaming entrarem no escopo, a pergunta "mainline ou BSP?"
volta — e é decisão de produto, não de implementação.**

#### Onde codec importa — e onde não

| Cenário | Precisa? |
| --- | --- |
| Launcher (UI, cartões, texto) | **não** — é desenho |
| Jogos nativos | **não** — renderizam |
| Player de música | não — CPU resolve |
| Player de vídeo | **sim** |
| Streaming de jogo (tipo Moonlight) | **sim** |
| Gravar tela / transmitir | **sim** |

**Decisão:** o launcher não usa codec.

#### RGA — o acelerador que ajudaria a UI

**RGA** faz escala, conversão de formato, rotação e composição em hardware. É o que
um launcher usa para desenhar num espaço e apresentar em 640×480 ou 1280×720 sem
custo de CPU. Mesma ressalva de driver, e o ganho é pequeno — redimensionar UI
estática é barato em qualquer GPU. **Não justifica trocar o kernel.**

---

## 3. Como é lido

Cada fonte vira uma struct com valor opcional e o instante da última leitura:

```rust
struct Source<T> {
    value: Option<T>,
    last_read: Instant,
}
```

Regras:

1. **TTL de 1 segundo** por fonte. Bateria e temperatura não mudam mais rápido que
   isso; ler a 60 Hz é desperdício puro.
2. **Nunca ler no caminho do desenho.** Ler no `tick`, antes do `render`.
3. **Falha é valor, não exceção.** Arquivo ausente ou conteúdo estranho → `None`.
   Nada de `unwrap`.
4. **UI nunca inventa.** `None` desenha `—`, não zero.

TTL por fonte:

| Fonte | TTL | Motivo |
| --- | --- | --- |
| Bateria | 2 s | muda devagar, e cada leitura são 2 arquivos |
| Temperatura | 2 s | idem |
| Wi-Fi / BT | 5 s | quase estático |
| Catálogo de apps | 10 s | só muda quando instala algo |
| Identidade | 1× no boot | não muda |

---

## 4. Lançar um app — a parte difícil

Esta é a decisão mais consequente do documento.

### O problema

O launcher tem o **DRM master** (é ele quem desenha na tela). Um jogo que tente
abrir o mesmo dispositivo falha ou herda a tela em estado indefinido.

### A solução: não fazer fork — fazer `exec`

```text
giochi-init  ──▶ launcher  ──exec──▶  jogo
     ▲                                  │
     └────────── jogo termina ──────────┘
                init reinicia o launcher
```

O launcher **substitui o próprio processo** (`exec`) pelo jogo. Não há dois
processos disputando o display: o launcher deixa de existir.

Quando o jogo termina, quem estava esperando é o **`giochi-init`**, que reinicia o
launcher.

### Por que assim

| Alternativa | Problema |
| --- | --- |
| `fork` + esperar | dois processos querendo o DRM; precisa liberar master e recuperar |
| Launcher em background | consome RAM e ainda precisa ceder o display |
| **`exec` + init reinicia** | **nenhuma disputa; um processo por vez** |

O custo é o launcher reiniciar do zero ao voltar do jogo — cerca de 1 s. Aceitável,
e é o que consoles fazem.

### Pré-requisitos no `Giochi System`

O `giochi-init` precisa:

1. Rodar o launcher num ciclo: `while true; do launcher; done`
2. Distinguir "saiu normal" de "crashou", para não entrar em loop de reinício
   infinito se o launcher estiver quebrado (limite de tentativas).
3. Garantir que o `exec` do jogo tenha `PATH`, `HOME` e o locale corretos.

São três itens de shell no repositório do sistema. **Nada disso existe hoje** — o
`Giochi System` ainda não tem `giochi-init`.

### Contrato do `exec`

| Item | Valor |
| --- | --- |
| Executável | `<dir>/<entrypoint>` do `index.json` |
| `cwd` | `<dir>` |
| `argv` | `[entrypoint]` — sem argumentos |
| `env` | herdado, mais `GIOCHI_APP_ID=<id>` |
| Falha ao executar | volta ao launcher e mostra aviso; não reinicia |

---

## 5. O que o launcher não faz

- **Não monta partição, não formata, não atualiza o sistema.**
- **Não gerencia energia** (suspender/desligar de verdade). Os botões existem na
  UI, mas hoje só mudam o estado interno. Comandos reais (`logind`, `systemctl`,
  ou `reboot(2)`) são do sistema.
- **Não instala app.** Instalação é da loja, no PC.
- **Não lê TDP.**
- **Não controla volume** (ver §2.8).

---

## 6. O que o `Giochi System` precisa garantir

Para este contrato funcionar, o sistema deve entregar:

| Item | Caminho | Estado hoje |
| --- | --- | --- |
| Catálogo de apps | `/opt/giochi/apps/index.json` | gerado pelo overlay do `Giochi os` — **nunca testado** |
| Identidade | `/proc/device-tree/model` | existe no kernel — não verificado |
| Bateria | `/sys/class/power_supply/*` | depende do driver do gauge — não verificado |
| Temperatura | `/sys/class/thermal/*` | idem |
| Brilho | `/sys/class/backlight/*` | depende de `BACKLIGHT_*` no kernel |
| `giochi-init` | — | **não existe** |

Nenhum desses caminhos foi verificado em hardware. O contrato está escrito; a
verificação é do `Giochi System`, no primeiro boot real.

---

## 7. Em aberto

| # | Assunto | Situação |
| --- | --- | --- |
| 1 | **Mainline x BSP** | adiado com o NPU. **Volta** se entrar player de vídeo ou streaming |
| 2 | Vídeo por hardware (VPU) | mesma incerteza do NPU no mainline; só importa se houver streaming/player |
| 3 | Volume: slider do painel ou também combo Fn? | brilho já é Fn; consistência sugere Fn |
| 4 | `buttons.map` do antecessor | **precisa ser revisto** — traz os scancodes reais do Fn |
| 5 | Nome do controle ALSA | suposição `Playback` em `rk817_ext`; confirmar com `amixer` no aparelho |
| 6 | `giochi-init` | o ciclo `while true; do launcher; done` não existe |
| 7 | Energia real (suspender/desligar) | exige IPC com o init ou `reboot(2)` |
| 8 | Multi-idioma | confirmado; formato (`locales/`) ainda não revisado |
| 9 | Tentativas de reinício antes de desistir | definir limite |

## 8. Decidido nesta rodada

| Assunto | Decisão |
| --- | --- |
| Leitura de estado | **direto do sistema de arquivos**, sem daemon |
| TDP | campo fica na HUD mostrando `—` até existir suporte |
| Brilho | **combo Fn + / Fn −**, escrevendo em `/sys/class/backlight/*` |
| Som | **ALSA direto**, sem servidor; `alsa` crate preferido ao `amixer` |
| Lançamento de app | **`exec`**, não `fork`; init reinicia o launcher |
| Fonte | TTF DejaVu Sans com cache de textura |
| Splash | RGBA cru, ~0,75 s, some no primeiro input |
| U-Boot splash | não ligar sem hardware |

## 9. Efeito de estacionar o NPU

Boa notícia: **parar o NPU restaura as decisões que ele havia derrubado.**

| Decisão | Com NPU | **Sem NPU (agora)** |
| --- | --- | --- |
| Servidor de som | obrigatório (áudio passa pelo upscaler) | **ALSA direto, sem servidor** |
| Kernel | BSP 6.1 | **mainline 6.12**, o validado em hardware |
| Camada de áudio | necessária | **não existe** |
| RK3326 | fora do recurso | **sem diferença** — nada dependia dele |
| `librknnrt` no rootfs | sim | **não** — imagem menor |

Nenhuma linha foi escrita dependendo do NPU, então descartar não custou retrabalho.

### O que fica registrado para o futuro

Se o NPU voltar, três coisas precisam estar verdadeiras:

1. Driver no mainline, **ou** decisão consciente de trocar o kernel.
2. Um uso que o CPU/GPU não faça bem — IA de jogo é o candidato real.
3. Medição de NPU compartilhado, se houver mais de um consumidor.

E um alerta que continua valendo: **"validação" não é trabalho de NPU.** Hash e
assinatura são CPU ou motor de cripto. Se a ideia era acelerar verificação de
imagem, o ganho é irrelevante — conferir um SHA-256 uma vez no boot não é gargalo
de nada.
