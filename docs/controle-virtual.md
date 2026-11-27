# Controle virtual "Giochi Gamepad"

> **Mudança do Giochi System, 18/11/2026.** Documentada em 18/11/2026 como
> pendência e **implementada em 27/11/2026** (issue #6 do app + item 3 da issue
> #7). Hoje o launcher **prefere o `Giochi Gamepad`** e não soma fontes: é ele a
> fonte dos `BTN_*`/`ABS_*`, e os nós físicos ficam só para `KEY_*` (volume e
> energia). O que continua **não implementado** é a leitura dos **eixos** — ver
> §3.5.
>
> Guia de origem: `docs/entrada-para-equipes.md` no **GIOCHI-SYSTEM** (repo
> vizinho, somente leitura — §1.1 do `AGENTS.md`). A referência completa, com as
> medições, é `contrato-de-entrada.md` no mesmo repo. Nada daquele repositório é
> copiado para cá: este arquivo é só o recorte **do lado do app**.

---

## 1. O que o sistema passou a publicar

Um **único** controle virtual, `Giochi Gamepad`, que traduz o hardware de cada
placa para um layout fixo. Aparelho novo entra editando `buttons.map` no
**Giochi System** — nenhum app muda.

| Propriedade | Valor |
| --- | --- |
| nome | **`Giochi Gamepad`** (é o identificador estável) |
| nó | **um** `/dev/input/eventN` — o número muda por boot |
| botões | **17** códigos fixos, todos declarados |
| eixos | **4**: `ABS_X`, `ABS_Y`, `ABS_RX`, `ABS_RY` |
| faixa dos eixos | **-32767..32767**, centro **0** |
| zona morta declarada | `flat=32` / `fuzz=32` — ruído de centro, **não** é zona morta de jogo |
| GUID do SDL | `06005931091200000100000001000000` |
| serviço | `/etc/init.d/S52gamepad` (no `rcS`, antes do launcher) |

### Os 17 botões

| Função | nome no mapa | código |
| --- | --- | --- |
| A (direita) | `a` | `BTN_EAST` (305) |
| B (baixo) | `b` | `BTN_SOUTH` (304) |
| X (cima) | `x` | `BTN_NORTH` (307) |
| Y (esquerda) | `y` | `BTN_WEST` (308) |
| L1 / R1 | `l` / `r` | `BTN_TL` / `BTN_TR` (310/311) |
| L2 / R2 | `l2` / `r2` | `BTN_TL2` / `BTN_TR2` (312/313) |
| select / start | `select` / `start` | `BTN_SELECT` / `BTN_START` (314/315) |
| home | `home` | `BTN_MODE` (316) |
| clique dos analógicos | `thumbl` / `thumbr` | `BTN_THUMBL` / `BTN_THUMBR` (317/318) |
| direcional | `up` `down` `left` `right` | `BTN_DPAD_*` (544-547) |

**Volume e power continuam no físico.** `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN` ficam no
`gpio-keys-vol` e `KEY_POWER` (116) no `pwrkey`; nenhum dos três está no virtual.
Volume não é botão de gamepad, e power nunca passou pelo mapa (§6.3 do
`AGENTS.md`).

---

## 2. As três regras

1. **Abra o device pelo nome, nunca por índice.** `eventN` e o índice do SDL
   mudam entre boots e entre aparelhos.
2. **Use uma fonte só.** evdev **ou** SDL `GameController` — nunca some as duas.
   No `neon-drift` a soma `bits_evdev | bits_sdl` fazia o mesmo botão acelerar e
   frear; cada fonte estava certa sozinha.
3. **`Button::A` é o `a` do mapa, o botão da direita.** A convenção do projeto é
   `A = BTN_EAST`; o mapeamento automático do SDL faria o contrário e o
   `gamecontrollerdb.txt` corrige. Em evdev, use os códigos canônicos acima e
   **ignore o que não conhece** — um `default: continue` custa uma linha; um
   `match` exaustivo quebra no aparelho novo.

Quem lê o virtual **não calibra e não inverte nada**: a normalização e a inversão
já são do sistema. No físico elas continuam sendo necessárias, e as faixas são
diferentes (R35S `-1800..1800`; X55/X35H `15..1023` com Y e RY invertidos).

---

## 3. O que muda neste repositório

### 3.1 Um arquivo, quatro consumidores

O leitor é **um arquivo só**, compartilhado por `#[path]`:

| Consumidor | Onde chama |
| --- | --- |
| launcher | [main.rs](../app/launcher/crates/ui-sdl/src/main.rs) → `EvdevInput::open("/dev/input", mapa)` |
| Star Dust | [main.rs](../app/launcher-star-dust/crates/ui-sdl/src/main.rs) → mesmo `open` |
| launcher-teste | [main.rs](../app/launcher-teste/src/main.rs) → mesmo `open` |
| `musica` | [entrada.rs](../app/musica/src/entrada.rs) → `Entrada::abrir()` → mesmo `open` |

Todos incluem [input_evdev.rs](../app/launcher/crates/ui-sdl/src/input_evdev.rs)
por `#[path]`. **A mudança é uma só**, e por isso não pode ser feita pensando em
um único app: o `musica` passa o mapa e o diretório pelo mesmo `open`.

### 3.2 O filtro por nome já é possível sem `libc`

`EvdevInput::open` em [input_evdev.rs](../app/launcher/crates/ui-sdl/src/input_evdev.rs)
abre **todos** os `event*` e recusa filtrar de propósito: um teclado também é
aberto, e código que não interessa nunca aparece no mapa. Para usar o virtual
não é preciso `EVIOCGNAME`: `device_name` já lê
`/sys/class/input/eventN/device/name`, e `device_names` já expõe a lista.

O `--espiar-input` também imprime o nome de cada nó — é ele que responde "o
virtual está aqui?" sem depender do `evtest`, que não existe nesta imagem.

### 3.3 A armadilha do `pressed` por código

`pressed: HashMap<u16, bool>` no `EvdevInput` é indexado **só pelo código**,
não por dispositivo. Hoje isso é inofensivo porque cada código aparece num nó
só. Com o virtual no ar, os mesmos `BTN_*` chegam **duas vezes** (físico e
virtual) e **a última escrita vence**: se o físico soltar antes do virtual, o
estado cai para "solto" com o botão ainda pressionado.

Consequência para a próxima rodada: **"use uma fonte só" não é estilo, é o que o
modelo de estado atual consegue representar.** As duas saídas plausíveis:

| Saída | Efeito |
| --- | --- |
| mapa `pressed` por dispositivo, mesclado com E | correto, mas mexe no núcleo e em todo leitor |
| virtual como fonte dos `BTN_*`/`ABS_*`; físico só para `KEY_*` | **foi a escolhida** (27/11/2026): mudança menor, e volume/power continuam funcionando |

### 3.3.1 O que foi implementado, e o que a medida mudou

`Papel` (em `input_evdev.rs`) classifica cada nó **pelo que ele declara** —
`/sys/class/input/<nó>/device/capabilities/{key,abs}` — em `Virtual`, `Gamepad`,
`Teclado` e `Ignorado`. A decisão é tomada **antes** de abrir qualquer nó.

Medido no R35S em 27/11/2026, com 4 nós:

```text
entrada: `rk817_int Headphones` ignorado — não declara botão nem eixo de
         controle (é o caso do jack de fone)
entrada: `r36s_Gamepad` ignorado como controle — o `Giochi Gamepad` é a fonte;
         ler os dois contaria o mesmo botão duas vezes
entrada: usando o controle virtual `Giochi Gamepad` — os nós físicos ficam só
         para volume e energia
```

Duas coisas que a medição derrubou, e que valem para quem repetir:

1. **`key=0 abs=0` não é capacidade.** O jack de fone tem os dois arquivos e
   ambos zerados; tratar "o arquivo de `abs` existe" como "tem eixo" fez ele ser
   classificado como controle na primeira versão.
2. **A ordem do `read_dir` põe o físico antes** (`event2` < `event3`). Decidir
   enquanto se abre fazia o launcher anunciar "sem `Giochi Gamepad`" e abrir o
   físico, para descobrir o virtual na linha seguinte. Por isso a classificação
   é uma passada separada.

Se o virtual aparecer **depois** do físico (imagem antiga, ou `S52gamepad`
subindo com o launcher no ar), o leitor do físico é desligado por um
`AtomicBool` compartilhado: leitura bloqueante não tem cancelamento em `std`, e
ler os dois conta o mesmo botão duas vezes (o defeito do `neon-drift`).

O `Vigia` (saída de emergência START+B) herda tudo: ele abre o **mesmo**
`EvdevInput`, então não há segundo caminho para divergir.

A segunda é a que combina com o contrato: o virtual é a tradução; o físico segue
legível para diagnóstico (o daemon **não** faz `EVIOCGRAB`).

### 3.4 Volume e power exigem o físico aberto

Abrir **só** o virtual perde `KEY_POWER` (dormir/menu de energia) e as teclas de
volume. O aviso de [app.rs](../app/launcher/crates/ui-sdl/src/app.rs) — "nenhum
`/dev/input` declara `KEY_POWER`" — continua correto justamente porque os nós
físicos seguem abertos. Um app que "só abre o virtual" herda um launcher sem
desligar.

### 3.5 Os eixos: o bloqueio do `absinfo` cai **para o virtual**

Hoje `EV_ABS` vira `Ev::Hat` e, em `poll`, só `ABS_HAT0X`/`ABS_HAT0Y` são
tratados; os analógicos são descartados em silêncio. A nota do §6.3 diz que ler
analógico exige `EVIOCGABS`, que exige `libc` — **isso vale para o físico**. O
virtual já entrega `-32767..32767` **já desinvertido**, então para ele basta uma
variante nova no `Ev` e a escala; nenhum ioctl, nenhuma calibração.

Falta decidir, na próxima rodada, onde a zona morta é aplicada: o `flat=32` do
kernel é ruído de centro e **não** serve como zona morta de jogo (32 de 32767 é
0,1 %).

---

## 4. A imagem anterior a 18/11 tem o nó e não entrega evento

O build do run `44184129584` (`87c8422`) tem o controle virtual **sem o
`SYN_REPORT`**: o daemon traduz e o app não recebe nada. O `giochi-selftest`
diz `PASS controle virtual: Giochi Gamepad — N botões, M eixos` porque ele
confere **o nó**, não a entrega do pacote.

| Saída / medição | Significado |
| --- | --- |
| `PASS no selftest` | o nó existe com os códigos — **não** prova que o evento chega |
| `SKIP sem controle virtual — imagem anterior ao contrato` | imagem antes de 17/11; não é defeito do app |
| `--espiar-input` sem nenhuma tecla ao apertar | o evento não sai do device (build anterior ao fix do `SYN_REPORT`), não é o app |

Portanto: **antes de acusar o leitor deste repo, confirmar que a imagem é
posterior a 18/11/2026.** Regravar o cartão é pré-requisito, não otimização.

---

## 5. Como verificar quando implementar

No aparelho (ferramentas do **Giochi System**, não deste repo):

```bash
/etc/init.d/S52gamepad status      # "1 processo(s)" + "device virtual: presente"
bash scripts/input-diag.sh         # como o kernel classificou cada nó
bash scripts/sdl-input-probe.sh r35s --run 10.55.0.2   # GUID e Button -> bN -> código
python3 scripts/entrada-probe.py --mapa device/r35s/buttons.map   # entrega de evento
```

Aqui:

```bash
giochi-launcher --espiar-input 20   # nome do nó + cada tecla crua, sem mapa
bash scripts/aparelho.sh capacidades
node scripts/verificar-mapa.mjs     # o mapa do aparelho contra o tradutor do binário
```

O log da implementação tem de dizer **qual** nó foi escolhido. "Entrou em modo
virtual" e "continuou lendo o físico" precisam ser distinguíveis de fora — é a
mesma lição do `aviso: 9 símbolo(s) do mapa não reconhecido(s)` (§6.3).

---

## 6. O que não fazer

1. **Não abrir por índice** — `eventN` nem o índice do SDL são estáveis. No R35S
   o nó físico **também** é um gamecontroller (medido no host em 18/11/2026 com
   os dois presentes: o SDL vê **dois**).
2. **Não somar evdev com SDL** — foi o defeito do `neon-drift`.
3. **Não assumir `A = baixo`** — a convenção é `A = BTN_EAST = direita`.
4. **Não criar um segundo mapa de botões** — `buttons.map` é a fonte da verdade;
   um caminho alternativo (SDL, engine, API) que divirja dele falha só no
   aparelho onde aquele caminho está ativo.
5. **Não recalibrar o virtual** com as faixas do físico (`15..1023`,
   `-1800..1800`). Se abriu `Giochi Gamepad`, a faixa já é `-32767..32767`.
6. **Não esperar jogador 2.** O daemon **funde** todos os físicos num virtual
   só: um controle USB de segundo jogador entra como jogador 1.

---

## 7. Decisões abertas para a próxima oportunidade

| Pergunta | Por que não está decidida |
| --- | --- |
| virtual como fonte dos `BTN_*`/`ABS_*` e físico só para `KEY_*`, ou estado por dispositivo? | as duas funcionam; a primeira é menor e não mexe no núcleo (§3.3) |
| o launcher continua em evdev ou passa a `SDL_GameController`? | não há motivo para trocar: o SDL continua só desenhando (§6.3), e o `SDL_GAMECONTROLLERCONFIG` do `giochi-init` nem seria usado |
| eixos analógicos entram no `Mapper`/núcleo? | **continua aberto, e é de propósito**: o virtual removeu o bloqueio do `absinfo` (a faixa é `-32767..32767` nos quatro eixos), mas não existe **ação** que consuma direção analógica. Implementar a leitura sem consumidor seria código morto — e a issue #6 diz "não é urgente" |
| zona morta de jogo | o `flat=32` do kernel é ruído de centro, não zona morta; valor tem de ser medido, não copiado |
| `musica` e `launcher-teste` herdam a mudança? | sim, pelo `#[path]` — e é por isso que o teste dos quatro consumidores é obrigatório (§3.1) |

Outros repositórios (`GIOCHI-GAMES`, `GIOCHI-ENGINE`, `GIOCHI-ENGINE-3D`,
`GIOCHI-OS`) recebem essa mudança pela issue correspondente, nunca por edição:
repo alheio é somente leitura (§1.1 do `AGENTS.md`).
