# Controles

Como o launcher recebe input e como o usuário gerencia os controles.

---

## 1. Entrada: evdev direto

**Decisão (02/11/2026): o launcher lê `/dev/input/event*` diretamente. O SDL fica só
para desenhar.**

O mapa por aparelho é `device/<pasta>/buttons.map` no **Giochi System**, derivado do
DTS. Como extrair, o que já foi descoberto e por que os bancos públicos não servem
estão em `docs/button-maps.md` **no repositório Giochi System** — não há link
relativo aqui de propósito: os repositórios são independentes, e caminho relativo
entre eles quebraria em qualquer clone isolado.

### Por que não a API de gamepad do SDL

Os botões desses handhelds não passam por driver de gamepad padrão: quem os
registra é `gpio-keys` no device tree, e o analógico é o `adc-joystick` do
mainline — **nos dois aparelhos** (X55 e X35H), confirmado em 03/11/2026 pelo DTB
compilado e pelo mapa que o sistema entrega. `rocknix-joypad` (fora da árvore) era
a hipótese alternativa do antecessor e **não** é o que roda.

O SDL só reconhece um controle se houver entrada no `SDL_GameControllerDB` — que
não existe para aparelho custom. Resultado: **no PC o SDL funciona, no aparelho
não**.

Segundo motivo, mais sutil: `buttons.map` mapeia por **código evdev**
(`BTN_EAST`), mas o SDL entrega **índice de botão** (0, 1, 2…). Os dois não
conversam. Ou traduzimos índice→nome em código, ou lemos evdev e usamos o mapa do
sistema como ele já é.

### Múltiplos controles

Evdev é naturalmente melhor aqui que o SDL:

| | evdev | SDL gamepad |
| --- | --- | --- |
| Vários controles | um `fd` por dispositivo | índice global |
| Controle interno + USB | ambos abertos, sem conflito | depende de mapping |
| Remover/plugar | só fechar/abrir o `fd` | precisa tratar evento |
| Identificar o aparelho | nome em `/sys/class/input/eventN/device/name` | nem sempre |

**Como vários controles são combinados:** o estado é a **soma** de todos. Se qualquer
dispositivo manda "A pressionado", a ação dispara. Botão repetido entre dois
controles não dispara duas vezes, porque a borda é calculada sobre o estado mesclado.

Efeito colateral aceito: se dois controles mandam direções opostas ao mesmo tempo, a
última lida vence. Ninguém joga assim.

### O que o leitor faz

```text
/dev/input/event0 ─┐
/dev/input/event3 ─┼─▶ threads de leitura ─▶ canal ─▶ estado mesclado ─▶ Mapper ─▶ Cmd
/dev/input/event5 ─┘
```

- Uma thread por dispositivo, leitura bloqueante. Sem `libc`, sem `fcntl`: thread
  custa menos que reescrever o leitor.
- Cada thread lê `input_event` de 24 bytes (aarch64) e manda `(type, code, value)`.
- O laço principal drena o canal a cada quadro e monta o `GamepadFrame` do núcleo.
- `buttons.map` traduz código evdev → nome lógico.

### Limite desta versão

Suporta **botões e hat digital** (`BTN_DPAD_*`, `ABS_HAT0X/Y`).

**Analógico não está suportado ainda:** normalizar `ABS_X/ABS_Y` exige `absinfo`
(ioctl `EVIOCGABS`), que precisa de `libc`. Fica como próximo passo, com o range lido
do dispositivo em vez de assumido.

---

## 2. Botão de energia

Não é botão de controle: é do PMIC (`rk805 pwrkey`), existe para ligar e
desligar o aparelho. Por isso **não passa pelo `buttons.map`** — um mapa por
aparelho não deveria precisar declarar que o botão de energia existe.

| Gesto | Ação |
| --- | --- |
| Toque curto (solta antes de 1 s) | dorme; backlight apaga |
| Segurar 1 s ou mais | abre o menu de energia |
| Qualquer gesto dormindo/desligado | acorda (o toque é consumido) |

O menu abre **ao atingir o limiar**, não ao soltar. Esperar a soltura faria o
usuário segurar sem nenhum sinal de que algo aconteceu — que foi exatamente a
queixa original ("apertei e não aconteceu nada").

O limiar é `PowerWatch::LIMIAR_PADRAO` em `crates/ui-sdl/src/power.rs`, com
teste cobrindo cada caso (toque, segurar, soltar depois do menu, consumir ao
acordar, quadro lento).

### O marcador `OPCIONAL` no mapa

Um botão pode estar no device tree e **não existir** no gabarito físico. É o caso
do `F` (`BTN_MODE`) da X55: o nó continua no DTS — revisões que o têm funcionam sem
rebuild — mas o aparelho testado tem 16 botões, não 17.

O sistema marca a linha no comentário, e o app entende:

```text
home    = BTN_MODE           # gpio3 PB7 — OPCIONAL: nao existe fisicamente
```

Regras implementadas (`ButtonsMap::parse` / `is_complete`):

| Situação | Antes | Agora |
| --- | --- | --- |
| botão marcado `OPCIONAL` ausente | mapa "incompleto" para sempre | não cobra |
| `home` ausente **sem** marcador | mapa "incompleto" | não cobra — `home` varia por hardware |
| botão de ação ausente (`a`, `start`…) | "incompleto" | **continua cobrando** |

`home` saiu da lista de exigidos mesmo sem marcador porque o X35H simplesmente não
declara `BTN_MODE`. Aviso permanente é pior que nenhum: esconde o dia em que um
botão de verdade quebrar — que é justamente o que o aviso existe para pegar.

O código continua mapeado quando existe: em revisão que tenha o botão, ele
funciona sem recompilar.

### Como o código descobre o botão

O código evdev vem do hardware, não de configuração. No X55:

```text
/sys/class/input/event0/device/capabilities/key  →  10000000000000 0
```

O kernel imprime as palavras de bits da **mais alta para a mais baixa**; o valor
não-zero é a palavra 1, bit 52 → `64 + 52 = 116` = `KEY_POWER`. Conferido com o
`gpio-keys-vol`, que declara `c000000000000` → bits 50/51 da palavra 1 → 114 e
115 = `KEY_VOLUMEDOWN`/`KEY_VOLUMEUP`.

> `evtest --query` **não serve** para isso: nesta build ele devolve 0 para
> qualquer código, inclusive um que o dispositivo não declara. Use o bitmask.

Não faça essa conta à mão. Duas ferramentas respondem, e é o par que separa
"driver quebrado" de "app que ignora":

```bash
bash scripts/aparelho.sh capacidades   # o que o kernel PROMETE emitir
bash scripts/aparelho.sh input 20      # o que o kernel ENTREGOU
```

O mapa completo do X55, decodificado:

| Dispositivo | `eventN` | Códigos declarados |
| --- | --- | --- |
| `rk805 pwrkey` | event0 | 116 `KEY_POWER` |
| `gpio-keys-control` | event3 | 304-318 (13 botões) + 544-547 `BTN_DPAD_*` |
| `gpio-keys-vol` | event4 | 114, 115 |

**São 17 botões no controle**, que é exatamente o número que o cabeçalho do
`buttons.map` do sistema cita. O direcional fica na faixa `BTN_` (0x220), não na
faixa `KEY_` — e é por isso que `544..547` parecem códigos sem nome.

Somando os três dispositivos: **20 códigos, todos com entrada em `code_for` e no
`buttons.map`**. A cobertura é verificável em segundos; ver
[ferramentas.md](ferramentas.md).

O `KEY_POWER` também abre o menu pelo botão **START**, por decisão anterior;
os dois caminhos coexistem.

## 2b. Analógico — o que o DTB declara

Medido no DTB compilado do **X35H** pela equipe do System em 03/11/2026, e igual
no X55 (mesmo driver, mesmo nó):

```text
/adc-joystick          poll-interval = 60 ms
  axis@0  ABS_X   (0)  abs-range 15 .. 1023
  axis@1  ABS_Y   (1)  abs-range 1023 .. 15      ← invertido no DTB
  axis@2  ABS_RX  (3)  abs-range 15 .. 1023
  axis@3  ABS_RY  (4)  abs-range 1023 .. 15      ← invertido no DTB
  abs-flat = 30 (deadzone)     abs-fuzz = 20
```

Três consequências que evitam erro:

1. **Não inverter Y nem RY no app.** `min` maior que `max` no device tree significa
   que o kernel **já entrega invertido**. Inverter de novo inverte duas vezes, e o
   sintoma (eixo ao contrário) parece bug de calibração.
2. **A deadzone é do sistema**, `abs-flat = 30`. Não é constante do app.
3. **O centro cai de `(15 + 1023) / 2 = 519`**, e a meia-escala é `504`. A
   normalização é `(valor - 519) / 504`, **sem** inversão.

A calibração já é lida do mapa do sistema (`axis_*` → `ButtonsMap::calibragem`,
com teste sobre o mapa real do X35H). O que falta é abrir o `absinfo` por ioctl —
o número acima é do DTB, não de medição em execução.

> O valor de `abs-flat` é declarado pelo DTS. Se a imagem mudar o nó, muda junto —
> por isso ele é lido do mapa, e não fixado no código.

## 3. Teclas de volume

`gpio-keys-vol` (event4) emite `KEY_VOLUMEDOWN` (114) e `KEY_VOLUMEUP` (115).
Diferente do botão de energia, **estas passam pelo `buttons.map`** — o sistema as
declara como `volup` / `voldown`, e o app só consulta.

| Gesto | Ação |
| --- | --- |
| Toque | um passo de 5 % |
| Segurar | repete: 1 passo imediato, 400 ms de atraso, depois a cada 120 ms |

O volume vai ao mixer de verdade: `Master Playback Volume` via libasound. Não é
número de UI.

### Por que o app controla ALSA

O mixer é interface **padrão** do Linux (`/dev/snd/controlC0`), do mesmo jeito que
o backlight é `/sys/class/backlight`. O elemento é **descoberto** (prefere
`Master`, depois `PCM`, depois `Playback`, senão o primeiro com volume de
reprodução) — fixar um nome seria remenda, porque o nome é decisão do codec.

### Persistência

O sistema **não** guarda estado ALSA: não há `alsactl` no init, `/var/lib/alsa`
está vazio e não existe `/etc/asound.conf`. Então o volume voltaria ao padrão do
codec a cada boot.

O app grava `/user/.config/giochi/volume` e reaplica na abertura (mesmo contrato
do brilho).

### Diagnóstico

```sh
giochi-launcher --mostrar-volume    # o que o mixer diz agora
giochi-launcher --volume 30         # aplica, pelo mesmo caminho da UI
giochi-launcher --espiar-input 20   # mostra cada tecla que chega do kernel
```

O espião é o que separa "não chega ao sistema" de "chega e o app ignora": ele lê
`/dev/input/event*` cru, sem mapa e sem tradução. A saída nomeia cada código, e
no fim diz quantos eventos vieram.

## 4. Desligar e reiniciar

O app executa `poweroff` / `reboot` do BusyBox quando o estado vira
`Screen::Shutdown` / `Screen::Reboot`. Verificado no X55: a energia é cortada de
fato.

Antes de executar, o app desenha um quadro ("Sistema desligado." / "Reiniciando..."),
porque `poweroff` bloqueia até o sistema parar — sem isso a tela congelaria no
menu de energia e pareceria travamento.

Se o comando falhar, a mensagem vai para a tela e a tentativa não se repete a
cada quadro. Em `Reboot`, qualquer botão devolve a interface, para o usuário não
ficar preso em "Reiniciando...".

Diagnóstico que **não** desliga:

```sh
giochi-launcher --checar-energia
```

Ele mostra o que o device tree declara sobre energia e avisa que isso **não prevê**
o comportamento do `poweroff` — no X55 a propriedade
`rockchip,system-power-controller` está ausente e o aparelho desliga normalmente.

## 5. O programa de gerenciamento

Pedido: "vamos precisar de alguma aplicação para gerenciar isso, mesmo que seja algo
adicional".

**Sim, e por um motivo específico: o problema do ovo e da galinha.**

### O problema que obriga a existir um programa separado

Se o mapeamento estiver errado — A e B trocados, por exemplo — o usuário **não
consegue navegar** até a tela de configuração para corrigir. O próprio ato de
consertar exige o controle que está quebrado.

Isso elimina a opção de colocar o gerenciador dentro de Configurações do launcher:
ele estaria atrás de uma porta que o usuário não consegue abrir.

### Desenho proposto

Um binário separado, `giochi-controllers`, iniciado pelo `giochi-init` **antes** do
launcher quando existir um sinalizador:

```text
giochi-init
   │
   ├── existe /user/.config/giochi/controller-setup ?
   │        │
   │        ├── sim ──▶ giochi-controllers ──▶ sai ──▶ launcher
   │        └── não ──▶ launcher
```

O sinalizador é criado de três formas, todas independentes do controle:

| Como ativar | Quando serve |
| --- | --- |
| Botão `home` segurado no boot | controle funciona, mas mapeamento está errado |
| Arquivo criado pelo PC (cartão no leitor) | controle não funciona de jeito nenhum |
| Primeiro boot, se nenhum evento chegou em 10 s | aparelho novo, mapa errado |

A terceira é a que resolve o caso geral: se o launcher não recebeu **nenhum** evento
em 10 s, alguma coisa está errada e ele mesmo pede socorro.

### O que o programa faz

1. **Lista dispositivos** — nome de `/sys/class/input/eventN/device/name`
2. **Mostra os códigos que chegam** — aperta um botão, vê `BTN_SOUTH` na tela
3. **Atribui nome lógico** — "esse é o A", "esse é o B"
4. **Testa os 13 nomes obrigatórios** e aponta os que faltam
5. **Salva** em `/user/.config/giochi/buttons.map`
6. **Restaura padrão** — sempre disponível

Operação: **só botões**. Não depende de analógico, não depende de mapa correto — só
de o dispositivo emitir eventos. É o mínimo que funciona quando tudo o mais falhou.

### Onde ele mora

Esta é a parte que precisa de decisão, porque afeta o modelo de três repositórios.

| Opção | A favor | Contra |
| --- | --- | --- |
| App em `Giochi app`, `distribution: image` | fica junto do launcher, que é o consumidor do mapa | mais um binário na imagem |
| Ferramenta do `Giochi System` | mexe com hardware, que é o domínio dele | duplica lógica de leitura de input |
| App pequeno, separado, no `Giochi app` | isolado; falha sem derrubar o launcher | um app a mais no catálogo |

**Recomendação: app pequeno no `Giochi app`, `distribution: image`,** porque ele
lê input exatamente como o launcher — o código de evdev é compartilhado, não
duplicado. E, se quebrar, o launcher continua funcionando.

### O que o sistema precisa entregar

| Item | Caminho | Estado em 04/11/2026 |
| --- | --- | --- |
| Mapa padrão do aparelho | `/etc/giochi/buttons.map` | **existe** — 103 linhas, `0644`, copiado do staging |
| O launcher e o gerenciador leem esse arquivo | — | o launcher lê; o gerenciador não existe ainda |
| Sinalizador de setup | `/user/.config/giochi/controller-setup` | **não existe**; e **quem cria é o launcher** (§ abaixo) |
| `giochi-init` que decide o que iniciar | `/usr/bin/giochi-init` | **existe**, com `::respawn:` no `inittab` das duas imagens de 04/11 |

O mapa padrão é **resolvido pelo sistema** para o aparelho (sem seções
`[default]`/`[x35s]`), porque quem sabe qual aparelho é esse é o `Giochi System`. O
launcher lê um mapa plano e não precisa conhecer modelo de hardware.

**Como conferi:** `debugfs` direto no `rootfs.ext4` de `out/powkiddy-x55` e
`out/powkiddy-x35h`. A linha `::respawn:/usr/bin/giochi-init` está no
`/etc/inittab` dos dois, depois do `rcS` e antes do getty. Fonte:
`Giochi System/scripts/post-build.sh`.

### Gatilho do sinalizador: é do launcher

O desenho lista três formas de criar `controller-setup`, e a terceira — "nenhum
evento em 10 s" — depende do launcher, não do sistema. É a que resolve o caso
geral: aparelho novo com mapa errado, em que o usuário não consegue navegar até
tela nenhuma.

**Estado: não implementado.** O launcher hoje avisa no log quando não há
`/dev/input`, mas não cria o sinalizador.

### O contrato do `giochi-controllers`, do lado do `giochi-init`

| Item | Valor |
| --- | --- |
| Executável | `$GIOCHI_CONTROLLERS`, padrão `/opt/giochi/apps/controllers/bin/giochi-controllers` |
| `argv` | nenhum — o init chama `"$CONTROLLERS"` seco |
| Quando roda | **antes** do launcher, e só se `/user/.config/giochi/controller-setup` existir |
| Saída 0 | mapeamento salvo |
| Saída ≠ 0 | falha — o sinalizador **permanece** (fonte de 04/11; a imagem ainda tem a versão que apagava) |

> **Corrigido no fonte em 04/11/2026, pendente na imagem.** A versão anterior
> apagava `$SETUP_FLAG` independente do código de saída: um configurador que
> falhasse perdia o pedido de socorro, e o usuário ficava com o mapa errado sem
> segunda chance e sem SSH. O `giochi-init` da árvore agora testa `$status` e
> mantém o sinalizador em falha — o contrato acima é o que está implementado.
>
> A imagem de 04/11 ainda tem a versão antiga: até ela ser regravada, o
> `giochi-controllers` não deve **depender** de o sinalizador sobreviver a uma
> falha. Na dúvida, saia com `0` e recrie o sinalizador você mesmo.

---

## 6. Em aberto

| # | Assunto |
| --- | --- |
| 1 | Onde o gerenciador mora — recomendação acima, precisa confirmação |
| 2 | Como o `home` segurado é detectado antes do mapeamento estar válido |
| 3 | Analógico: precisa de `absinfo` (ioctl) para normalizar; a calibração `axis_*` já é parseada, os eixos não são lidos |
| 4 | Mapa plano resolvido pelo sistema, ou seções lidas pelo launcher |
| 5 | Limite de 10 s sem evento — valor certo? |
| 6 | ~~Qual driver a imagem usa~~ — **respondido em 03/11/2026: `adc-joystick`, mainline, nos dois aparelhos.** Ver a seção de analógico abaixo |
