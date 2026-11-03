# Mensagem para a equipe do Giochi System

Documento de handoff entre as duas equipes. Serve para ser enviado a cada rodada
de entrega de imagem, com as seções de estado atualizadas.

**Como usar:** há duas versões do mesmo conteúdo.

| Versão | Onde | Serve para |
| --- | --- | --- |
| longa | `## Mensagem`, §1–§10 | e-mail, documento anexado, ou quando o detalhe importa |
| curta | `## Versão curta` | WhatsApp, Telegram, chat |

O que muda a cada rodada é a §1 (estado), a §7 (o que pedimos) e a §10 (prazo).
O resto é contrato e tende a ser estável. **Ao atualizar, mexer nas duas versões**
— divergir faz as equipes passarem a ter combinados diferentes.

---

## Mensagem

**De:** equipe do **Giochi app** (apps, launcher, ferramentas de teste)
**Para:** equipe do **Giochi System** (imagem, kernel, drivers, init)
**Assunto:** contrato app × sistema, o que falta na imagem e o que precisamos saber na próxima entrega

### 1. Onde estamos

O launcher roda no X55 há algumas rodadas: compila para aarch64, abre o KMSDRM,
desenha, lê os botões, controla brilho e volume no hardware, e desliga de verdade.
Texto é o único bloqueio funcional.

Recebemos a notícia de que existe uma **imagem nova** e um **segundo aparelho
(r35h, resolução menor)**. **Não testamos nenhum dos dois** — entendemos que a
validação está com vocês e o hardware está em uso. Fica na nossa fila, e não
vamos escrever conclusão sobre o r35h antes de rodar.

### 2. O que o app precisa da imagem

São **dois itens**. Nada além disso.

**(a) `::respawn:/usr/bin/giochi-init` no `/etc/inittab`**

`/usr/bin/giochi-init` existe e está correto, mas hoje nada o chama. O launcher
nunca sobe no boot — só por SSH.

Pedimos **`respawn`** e não `once` por dois motivos:

1. **Recuperação.** Se o launcher terminar — panic, falta de memória, erro ao
   abrir o DRM —, `respawn` traz a interface de volta. Com `once`, o aparelho fica
   numa tela preta sem saída que não seja SSH. Num handheld, isso é tijolo.

2. **Pré-requisito do lançamento de jogos.** A decisão de arquitetura registrada
   em `docs/integracao.md` é que o launcher **cede o processo** ao jogo (`exec`)
   em vez de criar filho, porque só um processo pode ter o DRM master. Quando o
   jogo termina, quem reinicia o launcher é o init. **Isso ainda não está
   implementado** — hoje o launcher só chama `poweroff`/`reboot` com processo
   filho e espera —, mas o `respawn` precisa existir antes de o lançamento
   entrar.

Distinguimos os dois de propósito: o item 1 é o que vale hoje; o item 2 é o que
a imagem precisa ter para a etapa seguinte não travar.

Como saber que resolveu: **ligar o aparelho e a interface aparecer, sem SSH** — é
o teste mais importante da rodada, e o único que não temos como simular. Depois,
matar o processo e ver se ele volta sozinho.

**(b) `SDL2_ttf` + `freetype` + `harfbuzz` + uma fonte TTF**

Quatro coisas, juntas. `SDL2_ttf` sozinho não resolve: ele carrega freetype e
harfbuzz em tempo de execução.

Buildroot, no `Config.in` de vocês:

```text
BR2_PACKAGE_SDL2=y
BR2_PACKAGE_SDL2_TTF=y
```

`BR2_PACKAGE_SDL2_TTF` puxa freetype e harfbuzz na versão de Buildroot em uso —
mas vale conferir, porque foi exatamente essa a suposição que já nos custou uma
rodada no passado.

Fonte: **DejaVu Sans**, licença permissiva, ~757 KB. Ou `BR2_PACKAGE_DEJAVU=y`, ou
nós a embarcamos dentro do pacote do app (`assets/fonts/`). **Preferimos a segunda**
— assim a imagem não depende de pacote de fonte, e o app fica autossuficiente.
Se vocês preferirem a primeira, funciona igual.

### 3. O que o app lê do sistema — e o pedido de aviso

Isto é um contrato. Se qualquer um destes mudar de forma ou de semântica,
**avise antes**: o app não tem como detectar a mudança, e o sintoma vai parecer
bug nosso.

| O que o app lê | Onde | O que espera |
| --- | --- | --- |
| orientação do painel | `/sys/class/graphics/fbcon/rotate` (de `fbcon=rotate:N`) | `0`, `1` ou `3` |
| brilho | `/sys/class/backlight/*/brightness` + `max_brightness` | escala do hardware |
| mapa de botões | `/etc/giochi/buttons.map` | `<nome> = <SÍMBOLO_EVDEV>`, `#` permitido |
| capacidades de entrada | `/proc/bus/input/devices` (`B: KEY=`) | bitmask padrão do kernel |
| dispositivos de entrada | `/dev/input/event*` | evdev |
| tela | `/dev/dri/card0` | KMSDRM funciona |
| volume | mixer ALSA em `/dev/snd/controlC0` | qualquer elemento com playback volume |

**Sobre a rotação:** `fbcon=rotate:2` (180°) **não é suportado** pelo app — ele
avisa e pede configuração manual. Se algum aparelho precisar de 180°, isso é
decisão a conversar, não algo que o app absorve sozinho. Preferimos `0`/`1`/`3`.

**Sobre o `buttons.map`:** o formato está bom e nós o seguimos. Um pedido: símbolo
desconhecido hoje **não** é silencioso — o app avisa
`símbolo(s) do mapa não reconhecido(s)`. Foi assim que descobrimos que estávamos
descartando 12 de 13 botões. Se vocês adicionarem um símbolo novo, o aviso aparece
e é fácil de rastrear.

### 4. O que o app escreve no sistema

Três arquivos em `/user/.config/giochi/`. **O app é o dono deste estado**, porque
a imagem não persiste nenhum deles.

| Arquivo | Conteúdo | Quem deveria ler |
| --- | --- | --- |
| `brightness` | inteiro na escala do hardware (`0..max_brightness`) | `giochi-init` no boot |
| `volume` | 0..100 | — (só o app lê) |
| `rotation` | `0`, `90`, `270` ou `ccw` | — (usuário/sistema pode escrever) |

Hoje o `giochi-init` não roda, então o app **também** reaplica brilho e volume na
abertura. É idempotente: quando o init voltar a rodar, os dois aplicam o mesmo
valor e não há conflito.

**Mas há um caso que precisamos combinar com vocês:** se a imagem nova passar a
persistir estado ALSA (`alsactl`), o app e o sistema vão disputar o mixer. Um dos
dois tem de ceder.

> **Isto é proposta nossa, não decisão tomada.** Uma divisão possível: o sistema
> guarda o estado, e o app apenas aplica a intenção do usuário e lê o valor real
> do mixer. Se preferirem o contrário — o app é o dono e o sistema não toca em
> ALSA — funciona igual, e nesse caso pedimos que o `alsactl` fique fora do init.
> O que não funciona é os dois escreverem sem saber um do outro.

### 5. O que já sabemos do X55 e vale para vocês

Fatos medidos, não deduzidos.

**`poweroff` corta a energia de verdade.** Confirmado no hardware: o aparelho
precisou ser religado pelo botão. Isso contraria o que a documentação do kernel
sugere para este device tree — e nós mesmos já havíamos escrito a conclusão
errada antes de testar. Registro aqui porque vocês podem tropeçar no mesmo ponto.

**Os códigos de tecla, decodificados do `B: KEY=`.** Atenção ao formato: o kernel
lista as palavras de 64 bits em ordem **decrescente**, a última coluna é a
palavra 0. Ler da esquerda desloca tudo em múltiplos de 64.

| Dispositivo | `eventN` | Códigos |
| --- | --- | --- |
| `rk805 pwrkey` | event0 | 116 `KEY_POWER` |
| `gpio-keys-control` | event3 | 304-318 (13 botões) + 544-547 `BTN_DPAD_*` |
| `gpio-keys-vol` | event4 | 114, 115 `KEY_VOLUMEDOWN/UP` |

São **17 botões** no controle — o número que o próprio cabeçalho do `buttons.map`
cita. Os 4 do direcional ficam na faixa `BTN_` (0x220), não `KEY_`, e é por isso
que parecem códigos sem nome.

`evtest --query` **não serve** nesta build: devolve 0 para qualquer código,
inclusive um que o dispositivo não declara. O bitmask do `/proc` é a única fonte.

### 6. Confirmando o achado de vocês — e o que fizemos do nosso lado

O `readelf -d` no `SDL2_ttf` para provar as `DT_NEEDED` foi a decisão certa, e
resolveria um falso positivo que já nos custou tempo:

> **Biblioteca presente não prova uso.**

Chegamos na mesma regra por outro caminho, com entrada em vez de biblioteca: o
dispositivo **declarar** um código de tecla e o kernel **entregar** o evento são
fatos diferentes. Confundir os dois foi o que manteve `KEY_POWER` e as teclas de
volume sem função por várias rodadas.

Do nosso lado, cada item do contrato tem ferramenta que responde a pergunta
correspondente — `bash scripts/aparelho.sh capacidades` lê o `B: KEY=`,
`aparelho.sh input` mostra o que o kernel entregou, `aparelho.sh verificar`
compara o quadro renderizado com uma referência e distingue "desenhou" de
"desenhou de cabeça para baixo" (99,9 % × 10,9 %). Se for útil para vocês, é
público no repositório do app.

Sobre o `usb0` em `10.55.0.2`: o nosso toolkit já tenta esse endereço **antes** de
varrer a rede, e aceita senha. Não é preciso tirar o cartão para nada.

Sobre o `/root/giochi-boot.log`: virou comando (`aparelho.sh boot-log`), que salva
em `dist/` com data no nome antes de qualquer regravação. **Duas perguntas:**
o caminho é estável? Existem outros logs de boot que devemos recolher antes de
regravar? Hoje só sabemos desse.

### 7. O que precisamos saber na próxima entrega

Para não medirmos a coisa errada, precisamos de:

1. **O que mudou na imagem** — kernel, Buildroot, pacotes, `inittab`, device tree.
   Um changelog curto resolve.
2. **O `inittab` tem `::respawn:`?** Sim ou não, direto.
3. **Onde o texto parou** — `giochi-selftest` responde, mas se vocês já rodaram,
   o resultado poupa uma rodada.
4. **Se a rotação mudou de declaração** — algum `fbcon=rotate:` diferente do `3`.
5. **Se o `buttons.map` mudou de formato ou de símbolos.**
6. **Se o estado ALSA passou a ser persistido pelo sistema** (a disputa da §4).
7. **Do r35h:** resolução do painel, orientação declarada, e **qual driver de
   analógico** ele usa — `adc-joystick` (mainline, faixa 15..1023) ou
   `rocknix-joypad` (DTS do projeto, já normalizado). Isso muda a normalização por
   completo, e não temos como descobrir pelo app.

### 8. O que vocês precisam de nós

Pedimos que respondam no mesmo formato — **o que o sistema lê do app, o que o
sistema escreve, e o que precisam saber de nós**. Especificamente:

- o app escreve só aqueles três arquivos em `/user/.config/giochi/`; precisam de
  mais algum?
- algum formato que preferem para `brightness`/`volume`/`rotation`?
- o `giochi-init` deve aplicar algo além de brilho no boot?
- há alguma informação que o app poderia expor e que ajudaria o diagnóstico de
  vocês (versão, estado, último erro)?

### 9. Resumo em uma linha

> A interface não sobe sozinha e não tem texto. O resto funciona, e o que
> funciona está medido no hardware — não deduzido.

### 10. Prazo — e o que fazemos enquanto isso

Não temos data para parar o desenvolvimento, então a mensagem não tem prazo duro.
O que ajuda de verdade é o inverso: **sabermos o que não vai entrar**, para parar
de esperar.

| Item | Se entrar em breve | Se ficar para depois |
| --- | --- | --- |
| `::respawn:` no `inittab` | testamos boot completo no r35h | seguimos subindo por SSH |
| texto (`SDL2_ttf` etc.) | UI fica utilizável; testes de layout começam | UI continua sem rótulo |
| changelog da imagem | uma rodada de teste direta | uma rodada só para descobrir o que mudou |

**Pedido concreto:** uma frase de resposta por item — "entra na próxima", "não
entra" ou "não sabemos". As três respostas nos servem; a ausência de resposta é o
que custa uma rodada.

Enquanto isso não voltamos a pedir nada: o app continua rodando com o que a
imagem de hoje oferece, sem exigir mudança de vocês.

---

## Versão curta (para WhatsApp, Telegram ou chat)

> Copie daqui até o fim da linha horizontal. Mesmo conteúdo da mensagem longa,
> sem as tabelas — o detalhe fica no documento, que pode ir anexado ou por link.

---

**Giochi app → Giochi System**

Sobre a imagem nova e o r35h: **não testamos nenhum dos dois**, porque entendemos
que a validação está com vocês. Fica na nossa fila — e não vamos concluir nada
sobre o r35h antes de rodar.

Dois pedidos, e são os dois únicos que bloqueiam:

**1. `::respawn:/usr/bin/giochi-init` no `/etc/inittab`.**
O script existe e está correto, mas nada o chama — o launcher nunca sobe no boot,
só por SSH. Dois motivos para ser `respawn` e não `once`: se o launcher terminar
(panic, memória, erro no DRM), ele volta sozinho em vez de deixar tela preta sem
saída além de SSH; e é pré-requisito do lançamento de jogos — o projeto é o
launcher ceder o processo ao jogo com `exec`, porque só um processo pode ter o
DRM master, e quem o reinicia depois é o init. (O lançamento ainda não está
implementado; hoje o launcher só chama `poweroff`/`reboot` e espera.)

**2. `SDL2_ttf` + `freetype` + `harfbuzz` + uma fonte TTF.**
As quatro juntas — `SDL2_ttf` sozinho não desenha nada. A fonte pode ir dentro do
nosso pacote (`assets/fonts/`); preferimos assim, mas funciona de qualquer jeito.

Confirmando o achado de vocês: o `readelf -d` no `SDL2_ttf` foi a decisão certa.
Biblioteca presente não prova uso — chegamos na mesma regra por outro caminho,
com os botões: o dispositivo *declarar* um código e o kernel *entregar* o evento
são fatos diferentes. Foi o que manteve o botão de energia sem função por
rodadas.

Do nosso lado já está pronto: `aparelho.sh capacidades` lê o bitmask do `/proc`,
`aparelho.sh input` mostra o que o kernel entrega, `aparelho.sh boot-log` salva o
log **antes** de regravar o cartão, e a rota `usb0` (10.55.0.2, senha) já é
reconhecida pelo toolkit.

Uma coisa que precisamos combinar: se a imagem passar a persistir ALSA, app e
sistema disputam o mixer. É proposta nossa que o sistema guarde e o app só
aplique — mas o contrário funciona igual, desde que só um escreva.

E um pedido que vale mais que os outros na prática: **um changelog curto do que
mudou** (kernel, pacotes, inittab, device tree, `buttons.map`). Sem isso gastamos
uma rodada descobrindo o que mudou antes de testar o que interessa.

Do r35h queremos saber, quando liberarem: resolução do painel, orientação
declarada e **qual driver de analógico** — `adc-joystick` ou `rocknix-joypad`.
Isso muda a normalização por completo e não temos como descobrir pelo app.

Respondemos no mesmo formato sobre o que vocês precisam de nós: o app escreve só
três arquivos em `/user/.config/giochi/` (`brightness`, `volume`, `rotation`).

**E o que mais ajuda:** uma frase por item — "entra na próxima", "não entra" ou
"não sabemos". As três nos servem; o silêncio é o que custa uma rodada.

---

## Notas internas (não enviar)

- A mensagem é deliberadamente curta em pedidos: **dois itens**. Listas longas de
  pedido viram nenhuma prioridade.
- O tom reconhece o achado deles (`readelf`) antes de listar pedidos. Intercâmbio
  entre equipes funciona melhor quando a primeira coisa é "você está certo".
- A seção 5 existe para **dar**, não pedir. Fatos medidos do hardware deles são o
  que compra credibilidade para os pedidos da seção 2.
- A §7 é o pedido mais importante na prática: sem changelog, gastamos uma rodada
  descobrindo o que mudou antes de conseguir testar o que interessa.
- **A versão curta não é resumo da longa, é a mesma mensagem sem tabela.** Manter
  as duas em sincronia ao atualizar: o que entra numa tem de entrar na outra, senão
  as duas equipes passam a ter versões diferentes do combinado.
- **O ALSA está marcado como proposta** (§4) porque ainda não é acordo. Se eles
  aceitarem, trocar o bloco por "acordado: o sistema guarda, o app aplica" — e
  então vira fato, não sugestão.
- **A admissão do erro do `poweroff`** (§5) é mantida de propósito: custa nada e
  evita que eles tropecem no mesmo ponto. Se o objetivo for só assertividade, é a
  primeira coisa a cortar.
- **A §2(a) distingue "vale hoje" de "pré-requisito de projeto" de propósito.** A
  primeira versão dizia que "o launcher usa `exec` para abrir jogo" — isso é
  decisão registrada em `docs/integracao.md`, **não** código: o lançamento de jogo
  não está implementado, e o único uso de processo hoje é `Command::status()` em
  `poweroff`/`reboot`, que é filho e espera. Afirmar comportamento inexistente
  para a outra equipe é o jeito mais rápido de perder credibilidade, e o
  argumento verdadeiro (recuperação de crash + pré-requisito) é mais forte.
- **O prazo é pedido em forma de "o que não vai entrar"**, não de data. Prazo duro
  entre equipes sem hierarquia comum costuma ser ignorado; "não vai entrar" é uma
  resposta que também libera nosso planejamento.

---

## 03/11/2026 (2) — Giochi app → Giochi System

**Assunto:** podem gravar; nós reinjetamos. E uma verificação no artefato de vocês
que muda o que esperar do boot.

### 1. Sobre regravar: **gravem, nós reinjetamos**

Resposta à pergunta de vocês sobre esperar: **pode gravar.** Reinjetar o launcher
do nosso lado é um comando (`bash scripts/aparelho.sh instalar`), e depende só do
aparelho estar na rede — não precisamos do conteúdo antigo do cartão.

O backup de vocês em `out/app-team-backup-20261103/` está correto e completo para
o que importa: `giochi-launcher` (511984 B), `buttons.map`, `brightness` (1332) e
`volume` (80). Conferi os quatro. Não é preciso devolver nada ao cartão: o
`brightness` e o `volume` são estado do usuário e o launcher reescreve os dois na
primeira abertura.

Uma observação sobre o `1332`: confere com o que medimos. O painel do X55 vai de
0 a 1666 e o padrão de fábrica é 833; 1332 é alguém tendo subido o brilho pela
UI — ou seja, o caminho `slider → sysfs → arquivo` funcionou no hardware.

### 2. Verificamos a imagem de vocês no artefato, não na documentação

Antes de responder, extraímos o rootfs de
`out/powkiddy-x35h/giochi-system-powkiddy-x35h-sd.img` (partição 2) e conferimos
o que está lá dentro. **Resultado: o texto está confirmado; o `respawn` não está.**

| Afirmação | Verificado no rootfs | Veredito |
| --- | --- | --- |
| as cinco `.so` presentes | todas em `/usr/lib` | **confere** |
| `SDL2_ttf` usa harfbuzz | `NEEDED libharfbuzz.so.0` no `.so` real | **confere** |
| DejaVu Sans na imagem | `/usr/share/fonts/dejavu/DejaVuSans.ttf`, 757076 B | **confere** |
| dropbear removido, só OpenSSH | `/usr/sbin/dropbear` ausente, `sshd` presente | **confere** |
| `usb0` e bootlog | `S30usbnet`, `S10bootlog`, `S96bootlog` | **confere** |
| `::respawn:/usr/bin/giochi-init` | **não está no `/etc/inittab`** | **falta** |

A causa não é a correção — é o horário. O `inittab` da imagem não tem a linha, e o
`build-info.txt` diz `built_at_utc=2026-11-03T22:17:56Z` (19:17 local), enquanto o
`scripts/post-build.sh` foi alterado às 19:32. A imagem foi construída **antes** da
correção. Confirmei também no commit `9bc36a3`, que é o que o `build-info` cita: ali
o script ainda tinha `if [ ! -x "$LAUNCHER" ] && [ ! -f "$FORCE_MARKER" ]`.

Ou seja: **a correção está certa e a imagem não a contém.** Rebuild resolve. Vale a
pena conferir com um comando antes de gravar:

```bash
debugfs -R "cat /etc/inittab" rootfs.ext4 | grep giochi-init
```

Sem essa linha a interface não sobe no boot, e é exatamente o teste que não dávamos.

### 3. Só existe imagem do X35H em `out/`

`out/powkiddy-x55/` está **vazia**. Então, quando vocês falarem em "regravar o
cartão com a de hoje": se for o X55, a imagem precisa ser construída primeiro; se
for o X35H, ela existe mas é a de antes do `respawn`. Perguntamos porque a
diferença muda o que vamos medir (720×1280 com `rotate:3` × 640×480 com `rotate:0`).

### 4. Aceitamos os quatro pontos, e já implementamos um

**`OPCIONAL`** — implementado, com testes sobre os mapas reais dos dois aparelhos.
Detalhe que só apareceu ao verificar: o mapa do **X35H não tem `home` de forma
alguma** (não é marcado `OPCIONAL`, simplesmente não declara `BTN_MODE`). Então
tratar só o marcador não bastaria — `home` saiu da lista de botões exigidos, e o
marcador protege qualquer outro que venha a variar. Botão de ação ausente
(`a`, `start`) continua reprovando: é o falso negativo que o aviso existe para
pegar.

**Rotação** — `fbconRotate` vindo de `device.json` em vez de texto solto no script
é o que o app já esperava: ele lê `/sys/class/graphics/fbcon/rotate` justamente
para não ter sentido fixo. X35H com `0` cai no caminho sem giro, sem código novo.

**`adc-joystick`, não `rocknix-joypad`** — corrigimos a documentação, que ainda
tratava `rocknix-joypad` como hipótese viva. E o mais importante: o `abs-range` do
Y e do RY vem com `min > max` no DTB, então **o kernel já entrega invertido**. Não
vamos inverter de novo. Registrado para quem implementar a leitura do eixo, porque
inverter duas vezes dá eixo ao contrário e parece problema de calibração.

**ALSA** — "o app é o dono" era a nossa proposta preferida, então está fechado: o
app persiste `/user/.config/giochi/volume` e escreve no mixer. Não pedimos
`alsactl store` no init. Se um dia precisarem restaurar mixer por outro motivo,
combinamos antes.

### 5. X35H — o que já sabemos e o que vamos medir

Agradecemos a correção do nome (**X35H**, não "r35h"). A confusão X35H/X35S
(RK3566) × R35S/R36S (RK3326) é do tipo que custa uma rodada inteira, e nós a
tínhamos registrado errado.

Do `device.json` + DTB de vocês já sabemos: 640×480, `rotate:0`, 16 botões sem
`BTN_MODE`, `adc-joystick` com faixa 15..1023 e Y/RY invertidos, `abs-flat`
30. **Não precisamos perguntar mais nada antes de testar.**

Uma previsão que registramos agora, para poder ser falsificada depois: 640×480 é
4:3 e a UI é projetada em 1280×720 (16:9). A escala vem da largura
(`640 / 1280 = 0,5`), a UI ocupa `720 × 0,5 = 360` px de altura e sobram
`480 − 360 = 120` px — ou seja **25 % da altura em letterbox**, 60 px em cima e
60 embaixo. Se for isso, é o comportamento correto do `set_logical_size`; se a
tela encaixar sem faixa, é porque algo redimensionou — e aí queremos saber o quê.

### 6. Uma frase que corrigimos do nosso lado, e que está no `post-build.sh` de vocês

O comentário da linha 62 do `scripts/post-build.sh` diz:

> `respawn` é o que mantém o launcher vivo quando um jogo termina (o launcher faz
> exec, não fork, então ninguém sobra para redesenhar)

O **argumento está certo**, mas o fato não: o launcher **não** faz `exec` hoje.
Não há `exec` em lugar nenhum do código; o único uso de processo é
`Command::status()` em `poweroff`/`reboot`, que cria filho e espera. O `exec` é
decisão registrada no nosso `docs/integracao.md` para quando o lançamento de jogos
entrar (ceder o processo para não disputar o DRM master), e **não está
implementado**.

Não muda a conclusão — `respawn` é necessário de qualquer forma, por dois motivos
que valem hoje: recuperação se o launcher morrer, e pré-requisito para o `exec`
futuro. Só vale corrigir o comentário para a próxima pessoa não procurar um `exec`
que não existe. Nós tínhamos escrito isso errado e corrigimos nos três documentos
que repetiam.

### 7. O que fazemos agora

**Não tocamos no aparelho** enquanto vocês estiverem gravando. Avisem quando o
cartão estiver com a imagem que vocês querem validar, e nós:

```bash
bash scripts/aparelho.sh opcoes        # rede ou usb0, e autenticação
bash scripts/aparelho.sh instalar      # compila aarch64 e instala o launcher
bash scripts/aparelho.sh diagnostico   # resolução, rotação, PMIC, botões, libs
bash scripts/aparelho.sh verificar     # frame comparado com a referência do PC
bash scripts/aparelho.sh capacidades   # o que cada /dev/input declara
```

O `verificar` agora compara de verdade: captura o quadro, desenha o mesmo no PC e
compara pixel a pixel. No X55 dá 99,9 % na rotação certa e 10,9 % na oposta — é a
diferença que distingue "desenhou" de "desenhou de cabeça para baixo". No X35H a
referência precisa ser gerada com a mesma build da feature `texto`, senão o
letterbox e os glifos contam como diferença; já está previsto no nosso checklist.

**Pergunta única, e é bloqueante para nós:** a imagem que vocês vão gravar no
X55 será construída com o `post-build.sh` corrigido? Se sim, esperamos e testamos
o boot completo. Se a do X55 não sair nesta rodada, testamos o X35H primeiro e
deixamos o X55 para depois — mas queremos saber qual, para não medir o aparelho
errado.
