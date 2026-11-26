# Armadilhas medidas — histórico

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**. O resumo está em
> [`AGENTS.md`](../../AGENTS.md) §13 (uma linha por armadilha) e §14
> (o que está provado e o que é afirmação). **Leia esta página antes de
> depurar**: cada item custou pelo menos uma rodada.

---

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

### O build `--no-default-features` sobrescreve o binário da comparação

`cargo build --no-default-features` em `app/launcher/crates/ui-sdl` grava em
`target/debug/giochi-launcher` **o mesmo caminho** do build completo. Quem
compilou a combinação do CI por último deixa o binário **sem texto** ali, e
`comparar-launcher-teste.sh` passa a comparar um launcher mudo com um que
desenha — 95,6 % e "linhas 0,11", com cara de regressão de desenho.

Medido em 15/11/2026: os dois quadros pareciam a Home, um sem nenhum texto. O
sintoma está no log, não no número: `aviso: compilado sem a feature 'texto'`.

Depois de exercitar a combinação enxuta, rode `bash scripts/compilar-launcher.sh`
antes de comparar. A mesma família do `--dump-frame` que grava preto (§13): o
binário é o mesmo caminho, e o que muda é o que ele sabe fazer.

### Três provas falsas viraram um pedido errado para outra equipe

Em 15/11/2026 um pedido foi para o canal do Giochi System: *"falta o transporte
HID do kernel (`CONFIG_HID`/`BT_HIDP`), o `hidp.ko` é só a camada de socket"*.
Medido no R35S na mesma data, contra `/proc/config.gz`:

```text
CONFIG_HID=y          # sempre esteve lá
CONFIG_HID_GENERIC=y
CONFIG_BT_HIDP=m
# CONFIG_UHID is not set    ← o único que faltava
```

O módulo **carrega e inicializa** (`modprobe hidp` → `rc=0`,
`HIDP socket layer initialized` no `dmesg`, que só aparece depois do sucesso).
As três provas do pedido eram falsas, cada uma de um jeito diferente:

| Prova | Defeito |
| --- | --- |
| `modinfo hidp` mostra `depends: bluetooth` sem `hid` | **`modinfo` não existe na imagem** (`sh: modinfo: not found`). A linha veio de memória e nunca foi executada |
| `strings hidp.ko \| grep hid_register_driver` vazio | o símbolo no 6.12 é `__hid_register_driver`, gerado por macro. O `grep` procurava o nome errado e o "sinal" era a ausência do que não existe |
| `/sys/bus/hid/drivers/hidp` não existe | **é o correto**: no 6.12 o `hidp` é transport driver, cria `hid_device` e quem fica no barramento é o `hid-generic`. Esse diretório nunca existiria |

Três lições, todas reutilizáveis:

1. **Ferramenta que não está instalada não é ferramenta.** `modinfo`,
   `timeout`, `evtest --query` (§6.6) — o comando ausente devolve vazio, e vazio
   lido como "não tem" é a mesma família de `is_ok()` medindo presença de
   variável em vez de estado.
2. **Nome de símbolo é detalhe de versão do kernel.** `hid_register_driver` é
   macro; o símbolo exportado é `__hid_register_driver`. Procurar o nome
   plausível devolve vazio, e vazio virou diagnóstico.
3. **Pergunte se o estado observado é o esperado antes de chamá-lo de defeito.**
   O jeito de responder isso é o log do próprio módulo (ele diz que inicializou)
   e a config do kernel (ela diz o que foi embutido), não a estrutura de sysfs
   que se *esperava*.

A ferramenta do repo (`scripts/aparelho.sh bluetooth`) repetia o mesmo erro e
ganhou a correção junto: hoje ela prova o núcleo HID por
`/sys/bus/hid/drivers/hid-generic`, lê a config por `zcat /proc/config.gz` e
declara **"entrega ao controle NÃO MEDIDA"** em vez de inventar veredito.

### Comparação exata de quadro acusa glifo que a régua do projeto aprova

`verificar-frame.mjs` compara com **tolerância 12** (soma da diferença RGB por
pixel) e amostra 1 pixel a cada 3×3. Comparar os mesmos dois BMPs com igualdade
**exata** dá outro número, e o outro número parece regressão:

| Quadro (R35S × PC, 640×480) | exato | régua do projeto (tol. 12) |
| --- | --- | --- |
| Home | 98,30 % (5.228 px) | **100,0 %** |
| Apps | 99,39 % (1.887 px) | **100,0 %** |

Medido em 15/11/2026. Os pixels diferentes são **antialiasing de glifo**: mesmo
texto, mesma posição, um pixel de borda com 1–3 níveis de diferença. A causa é o
`FreeType` do Buildroot contra o do PC (o `libSDL2_ttf` é 2.24.0 dos dois lados e
trocar o arquivo `DejaVuSans.ttf` pelo do aparelho **não mudou um pixel** — os
arquivos têm MD5 diferentes e rasterizam igual).

Três consequências:

1. **Não use igualdade exata para julgar desenho.** É régua mais dura que a
   aprovada, e o resultado dela já custou uma rodada de investigação.
2. **A tolerância 12 existe para isso**, não para perdoar layout errado: um
   deslocamento de 1 px, uma cor de tema trocada ou um texto faltando estouram
   muito antes disso (rotação oposta dá 10,9 %, catálogo divergente 92–97 %).
3. **A fonte é ambiente, não dado.** Como relógio, bateria e temperatura (§11),
   ela não se congela por variável de ambiente — está na biblioteca.

Sanidade do comparador, e ela é do mesmo tipo da que já existe para o rotador
(§13, "girar 90° e voltar tem de dar 100,00 %"): **o mesmo BMP contra ele mesmo
tem de dar 100,0 %**, e um BMP contra a própria imagem deslocada 1 px tem de
cair. Se as duas coisas não acontecerem, o número está medindo outra coisa.

### Aviso de compilador que ninguém lê é aviso que não existe

Em 15/11/2026 havia **21 avisos** acumulados entre as quatro crates, e **zero**
no CI: `cargo build` não é `cargo clippy`, e nenhum workflow rodava o segundo.
O inventário diz o que se perde:

| Aviso | O que escondia |
| --- | --- |
| `rotacao_do_sistema` nunca usada | wrapper morto: o caminho vivo é `..._com_fonte` |
| `sort_by(\|a,b\| b.cmp(&a))` | `sort_by_key` + `Reverse`, que é o que se quis escrever |
| `let _ = set_clip_rect(...)` | `set_clip_rect` devolve `()`, e o `let _` mentia sobre isso |
| `loop { match ... Err => break }` | um `while let` — a única saída era a falha de leitura |
| `i as u8 * 32` em 8 faixas | o cast estoura na **nona** faixa; um efeito a mais quebraria em silêncio |
| 3 × "items after a test module" | código de produção depois do `mod tests` |

Nenhum quebrava teste, e por isso nenhum apareceu em lugar nenhum. O CI ganhou um
passo com **`-D warnings`** — rodar clippy sem `-D` só desloca o problema de
lugar, porque o log de um job verde ninguém abre.

`launcher-teste` fica fora desse passo: depende do repositório irmão
`GIOCHI-ENGINE-3D`, que o CI não tem. Localmente ele também está em zero.

E os módulos compartilhados por `#[path]` (`views`, `catalogo`, `system` no
`launcher-teste`) levam `#[allow(dead_code)]` **local ao módulo**: eles trazem
itens que só o launcher de produção usa (desligar, instalar pacote, toque). A
alternativa seria copiar e apagar o que sobra — que é como nasceram as cópias
atrasadas do Star Dust (§7).

### Um `cfg` que nunca é compilado é código que não existe

`cargo build --no-default-features` ficou quebrado por semanas (faltava um campo
no inicializador do `Painter` na ramificação `not(feature = "texto")`) porque só o
build padrão rodava. O CI constrói as duas combinações.

### `--instalar <ip>` sem usuário vira `charles@<ip>`

`ssh root@aparelho` é o único login que existe lá. Um `--instalar 10.55.0.2` sem
`@` faz o `ssh` usar o usuário **deste PC**, e a resposta é

```text
charles@10.55.0.2: Permission denied (publickey,password,keyboard-interactive)
```

que manda procurar chave SSH — e o errado era o nome do usuário. O
`compilar-launcher-teste-arm64.sh` já fazia certo; o
`compilar-launcher-arm64.sh` não fazia, e os dois divergiram. Hoje ambos usam
`INSTALAR="${2#root@}"` no parse e `root@$INSTALAR` nos comandos.

### Dois significados para a mesma variável produzem `root@root@`

`compilar-launcher-arm64.sh` guardava o destino do `--instalar` de duas formas:
`INSTALAR` era **bare** (`${2#root@}`) mas `HOST_SSH` já vinha com `root@`. Quando
`--instalar` não trazia argumento, `INSTALAR="$HOST_SSH"` gravava `root@10.55.0.2`
e os quatro `ssh`/`scp` prefixavam `root@` de novo. Medido em 15/11/2026:

```text
instalando em root@root@10.55.0.2:/opt/giochi/apps/launcher/bin
root@root@10.55.0.2: Permission denied (publickey,password,keyboard-interactive).
```

A mensagem manda procurar chave quando o errado era o alvo — a **mesma** família
do `--instalar ip` que virava `charles@ip`. Corrigir o prefixo num ponto não
adianta: sobra outro lugar que prefixa. Hoje **uma função** decide, e todo
`ssh`/`scp` a consulta:

```bash
alvo_ssh() { case "$INSTALAR" in *@*) printf '%s' "$INSTALAR" ;; *) printf 'root@%s' "$INSTALAR" ;; esac; }
```

`foo@host` passa intacto (usuário explícito é respeitado), `host` ganha `root@`,
e `root@host` não ganha nada. Medido nas três formas mais `--instalar` sozinho.

### A referência do `verificar` vinha de um binário velho

`gerar_referencia` usa `app/launcher/target/debug/giochi-launcher` para desenhar a
Home no PC. Nada garantia que ele fosse atual — e `--testes` compila *harnesses*,
não o binário, enquanto o clippy só faz type-check. Medido em 15/11/2026: o
binário era de 08:19, a poda de tema entrou depois, a referência desenhou 6
cartões e o aparelho 4. **92,4%**, com cara de regressão de desenho e nenhuma
regressão nenhuma. Com o binário atualizado: **99,6%**.

É a terceira vez que a comparação acusa o desenho por uma assimetria de entrada
(as outras: catálogo divergente no X35H, 99,4% → 97,8%; e `dir` do aparelho
inexistente no PC, 36,5%). A diferença é que esta o próprio script podia evitar:
hoje ele procura `*.rs` mais novo que o binário e **reconstrói** — o build é
incremental e custa 0,02 s quando nada mudou. Aviso aqui seria lido por ninguém
e a rodada inteira sairia com o número errado.

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

### A flag de diagnóstico aplicada antes de quem a sobrescreve

`--idioma pt` era lido em `main`, gravado no `State` — e o `App::new`, que roda
depois, lia `/user/.config/giochi/language`, não achava nada e escrevia o padrão
**por cima**. O log da mesma execução dizia as duas coisas, em duas linhas
seguidas:

```text
--idioma: Portuguese (pt) — não grava
idioma: English (en) — primeira abertura, o padrão é inglês
```

E o sintoma que denunciou foi o quadro: `--dump-frame` em `en`, `pt`, `zh` e `ar`
saía **idêntico pixel a pixel** (0 de 307.200). Um número que não muda quando a
entrada muda é a definição de ferramenta que não mede o efeito.

A correção não foi reordenar as escritas: foi tornar a sobreposição **parâmetro**
(`App::new(..., idioma_forcado: Option<Idioma>)`), para não existir ordem em que
o arquivo vença um pedido explícito. Mesma família do `INSTALAR`/`HOST_SSH` com
dois significados (§13): enquanto a informação puder chegar por dois caminhos,
um deles vai vencer sem avisar.

Sanidade que ficou: comparar dois idiomas diferentes **precisa** dar quadros
diferentes — medido em 15/11/2026, `en × pt` = 7.351 px, `en × zh` = 11.293 px,
`en × ar` = 13.993 px na tela de Config. Se der zero, o caminho do idioma
quebrou antes de chegar ao desenho.

### Ferramenta de diagnóstico que não alcança a tela que diz fotografar

`--tela senha` empilhava o overlay à mão (`overlays.push`) sem passar por
`focus_on`, então o foco continuava em `HomeCarousel`: `--teclas down,a`
reportava "foco HomeCarousel 0" e o `↓` caía no carrossel. Uma ferramenta que
declara fotografar uma tela e não chega nela. Hoje `abrir_tela` move o foco para
a zona do overlay, e é isso que permitiu medir o defeito da senha **sem dedo no
aparelho**.

Junto veio `--teclas` aceitando letra solta e `x`. Sem poder digitar, nenhuma
ferramenta exercitava o teclado de senha — e o defeito estava exatamente ali.

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
| senha Wi-Fi: digitar, ver e confirmar | **medido** — o caminho de produção `--conectar` conectou no X55 com a senha real |
| Wi-Fi sobrevive ao reboot | **medido**: aparelho reiniciou e voltou sozinho para `Kill.exe`, com IP, sem digitar senha |
| boot automático (`::respawn:` → init → launcher) | **medido** no X55: `ppid` do launcher é o init, 29 s depois do boot |
| Wi-Fi: varredura e lista | **medido** (`wi-fi: 11 rede(s) em wlan0`) |
| Wi-Fi: conectar com a senha certa | **medido** no X55 (15/11/2026): `COMPLETED`, IP, gateway, DNS e HTTP |
| Bluetooth: parear | **medido** (`Bonded: yes`) |
| Bluetooth: virar entrada | **bloqueado na imagem** (falta transporte HID e `uhid`) |
| Star Dust desligado = launcher idêntico | **medido**, 4 tamanhos, byte a byte |
| launcher-teste 100 % pixel a pixel | **medido**, 4 tamanhos + 6 telas |
| `.pyz` e loja nos três sistemas | **medido** no CI (7 e 4 jobs verdes) |
| assinatura verificada no aparelho | **medido**: 20 MB de RSS, 0,01 s |
| launcher multi-idioma (10 idiomas) | **medido** no R35S em 15/11/2026: boas-vindas na 1ª abertura, `en -> pt` pelo caminho real, 2ª abertura sem boas-vindas, Home `pt` **99,6%** e Config `pt` **99,7%** contra o PC, `verificar` **100,0%**, os dez abrem |
| escolha do idioma por **botão físico** | **medido**: um toque no controle na boas-vindas gravou `en\n` e a execução seguinte abriu sem perguntar — o caminho `input_evdev`, não o `--teclas` |
| cobertura de fonte por idioma no aparelho | **medido** com `--fontes` (`TTF_GlyphIsProvided`): `pt/ru/ar` completos, `zh` 5/8, `hi` 1/11, `bn` 1/10 — falta fonte na imagem (issue nº 5) |

**Regra de honestidade:** nada aqui pode ser declarado "funciona" sem ter sido
executado. "Compila" e "roda" são afirmações diferentes — e o `poweroff` já caiu
por ter sido escrito antes de testado.

**Existir, declarar e ser usado são fatos diferentes.** Uma `.so` no rootfs não
prova que o app a usa (só `readelf -d` mostra as `DT_NEEDED`); um `/dev/input`
declarado no `/proc` não prova que o evento chega; uma variável de ambiente
documentada não prova que o código a lê (§13).
