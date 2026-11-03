# Mensagem para a equipe do Giochi System

Documento de handoff entre as duas equipes. Serve para ser enviado a cada rodada
de entrega de imagem, com as seções de estado atualizadas.

**Como usar:** copie o bloco abaixo. O que muda a cada rodada é a seção 1 (estado)
e a seção 6 (o que pedimos de informação). O resto é contrato e tende a ser
estável.

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
nunca sobe no boot — só por SSH. E `respawn` não é detalhe: o launcher usa `exec`
para abrir jogo, então sem ele não sobra processo para redesenhar a interface
quando o jogo termina.

Como saber que resolveu: **ligar o aparelho e a interface aparecer, sem SSH.**
É o teste mais importante da rodada, e é o único que não temos como simular.

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
dois tem de ceder. Nossa proposta: o sistema guarda, o app só aplica a intenção do
usuário e lê o valor real do mixer. Digam se preferem assim.

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
