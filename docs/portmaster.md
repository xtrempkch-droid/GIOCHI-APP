# PortMaster — a loja e os ports na aba Apps

> Implementado em 26/11/2026. Responde ao **pedido de PortMaster** registrado
> nas issues #2–#5 deste repositório (as #3, #4 e #5 são duplicatas da #2).
>
> Guia do sistema: `docs/evaluacao-portmaster.md` e `docs/entrada-para-equipes.md`
> no **GIOCHI-SYSTEM** (somente leitura — §1.1 do `AGENTS.md`).

## 0. Créditos — de quem é o quê

**O PortMaster é obra da comunidade [PortsMaster][pm]**, não deste repositório.
A loja, o catálogo de ports, o trabalho de portar cada jogo e os ports em si são
deles; a maior parte é software livre e os ports são **gratuitos**.

O que este repositório escreveu é uma coisa só: o `giochi-ports`, um invólucro de
~370 linhas de `sh` que roda o PortMaster dentro do `bubblewrap` com o rootfs
somente-leitura — porque ele baixa `.zip` **sem assinatura** e executa o `.sh` de
dentro como root. O invólucro não substitui nem reimplementa nada: ele chama o
`pugwash` deles.

O crédito é gravado onde não depende de alguém lembrar:

| Onde | O que diz |
| --- | --- |
| cartão da loja na aba Apps | título **PortMaster** |
| `metadata.json` do cartão | `developer` lido do `gameinfo.xml` do próprio pacote (`Community`, medido no R35S) e `licenseUrl` para o projeto |
| script do cartão | o bloco `CRÉDITOS`, com a URL |
| cartão de cada port | `developer` = **quem portou**, lido do `port.json` |

A URL não é de memória: `github.com/PortsMaster/PortMaster-New` e
`.../PortMaster-GUI` aparecem **dentro do pacote instalado** (medido com `grep`
no R35S em 26/11/2026).

[pm]: https://github.com/PortsMaster/PortMaster-New

---

## 1. O que o PortMaster faz e onde ele instala

O PortMaster é uma loja de ports que roda **no aparelho**, em `/user/ports`:

```text
/user/ports/<nome>.sh   + /user/ports/<nome>/     ← o instalado pela GUI
/user/ports/<dir>/<nome>.sh                       ← o feito à mão
```

O nome do diretório e o do arquivo **não coincidem sempre** (`FORM.sh` mora em
`form/`, `stardewvalley/StardewValley.sh`). A comparação é sem caixa, e é o que
o `giochi-ports --rodar` já faz.

O launcher varre `/opt|/user/{games,apps}` (§6.9 do `AGENTS.md`). Um port
instalado pela GUI ficaria **invisível**: existe no disco e em nenhum cartão.

---

## 2. O que foi implementado

`app/launcher/crates/ui-sdl/src/portmaster.rs`, compartilhado por `#[path]` com
o Star Dust. Na abertura — e antes da detecção — o launcher:

1. lista `/user/ports` (os dois layouts, ignora `PortMaster/` e nomes ocultos);
2. para cada port, escreve um pacote de app em `/user/apps/<slug>/`:

```text
/user/apps/portmaster-form/
├── .portmaster-port          # marca: "FORM" — só o que tem esta marca é podado
├── metadata.json             # kind: app, entrypoint: bin/portmaster-form
└── bin/portmaster-form       # 755, exec giochi-ports --rodar "FORM"
```

3. apaga os cartões marcados cujo port sumiu.

O slug é `portmaster-<nome normalizado>` e **nunca** começa com `launcher`, então
não cai na poda de tema (§8 do `AGENTS.md`). Nome sem nenhum ASCII recebe um
sufixo FNV-1a determinístico.

### Por que no launcher, e não num script do PC

Quem instala o port é a GUI **no aparelho**, sem PC por perto. A GUI roda com o
KMSDRM livre — o launcher não pode estar no ar — e o `giochi-init` reinicia o
launcher quando ela sai. Portanto o launcher é a única peça que está no
aparelho, sobe logo depois da instalação e já sabe lançar um app sem argumentos.

### Por que `entrypoint` em `bin/` e não na raiz

O pedido original sugeria `entrypoint: "<slug>"` na raiz do app. **Não
funciona**: o detector recusa `entrypoint` que não comece em `bin/` — medido no
R35S em 26/11/2026:

```text
catálogo: /user/apps/portmaster-form: entrypoint inválido no metadata.json
          (`portmaster-form` (precisa começar em bin/)) — fora do catálogo
```

O layout que funciona é `bin/<slug>`, e é o que o módulo escreve.

---

## 3. O caminho do lançamento, sem mudar o `Alvo`

`Alvo` continua sem `args`: o launcher executa o `entrypoint` com `cwd` na pasta
do app e **sem argumentos** (`docs/integracao.md` §4). A escolha do port vai
dentro do script gerado:

```sh
#!/bin/sh
exec giochi-ports --rodar "FORM"
```

`giochi-ports` resolve o resto: acha o `.sh` nos dois layouts, confere o DRM,
roda dentro do `bwrap` com o rootfs somente-leitura e devolve ao sair.

**O marcador de DRM não é criado nesse caminho**, e isso é de propósito: o
launcher destrói o `Painter` antes do `spawn` (§6.8), então quando o filho chega
não há processo com `card0` aberto. Criar `/run/giochi/dev-free-console` ali
seria mentir para o `giochi-init` sobre um console que está livre — se o port
morresse com o marcador, o launcher não voltaria.

---

## 3b. Por que o cartão não abria — e a correção de 26/11/2026

A primeira versão **funcionava no papel e não abria o FORM**. Medido no R35S:

```text
sistema: iniciando portmaster-form (/user/apps/portmaster-form/bin/portmaster-form)
giochi-ports: rodando FORM.sh (de /user/ports)
giochi-ports: liberando o DRM: parando o PID 5881 (o launcher) — o supervisor o trará de volta ao sair
```

E o FORM nunca aparecia. A tela ficava onde estava, sem mensagem nenhuma.

### A causa, em três elos

1. **O `giochi-ports` pergunta "o DRM está livre?" lendo `/proc/*/fd`** atrás de
   `/dev/dri/card0` — não pergunta quem tem o *master*. Basta um descritor
   aberto para ele concluir que o launcher está com a tela.
2. **O launcher mantinha `card0` aberto**, mesmo depois de `App::encerrar()`
   soltar o `Painter` e devolver o *master*. Quem segura o descritor é o driver
   gráfico (`libmali`) carregado pelo renderer `opengles2`, e ele só fecha quando
   o processo morre. **Soltar o `VideoSubsystem` do SDL não resolve** — foi a
   primeira correção tentada, e a medida mostrou que o fd continuava:
   `1 descritor(es) ... 3 -> /dev/dri/card0`.
3. **O launcher não morria com `SIGTERM`.** O SDL instala um handler que só
   posta `SDL_QUIT` na fila de eventos, e durante o filho ninguém bombeia essa
   fila. O `giochi-ports` esperava 6 s, desistia, removia o marcador e saía.

O terceiro elo é o que o **próprio `giochi-ports` documenta**: ele manda
`SIGTERM` de propósito porque o `giochi-init` zera a cota de reinício em saída
por sinal, e conta com o supervisor para devolver a interface quando o port
terminar. O lado do app é que não cumpria a parte dele.

### A correção, no app

`system::lancar_com_saida` arma um handler de `SIGTERM` durante a espera do
filho; quando o sinal chega, ele **repõe o handler padrão e se reergue**
(`raise`), morrendo por sinal de verdade — `exit(143)` contaria como queda e
gastaria a cota de cinco. O filho fica órfão e vivo, que é o desejado: o port
assume a tela, e o supervisor devolve o launcher quando o marcador some.

O log diz o que aconteceu, com a medida que explica o pedido:

```text
sistema: SIGTERM durante portmaster-form — saindo para quem pediu devolver a
  interface (1 descritor(es) de /dev/dri neste processo: 3 -> /dev/dri/card0)
```

Fora da janela do filho, o comportamento do SDL continua o de antes: o handler é
restaurado assim que o app termina normalmente.

### Verificação (R35S, 26/11/2026)

| Passo | Resultado |
| --- | --- |
| abre o cartão pelo caminho do botão A | o launcher **sai por `SIGTERM`**, como o `giochi-ports` pede |
| o port | `FORM video: opengles2 renderer, vsync on` — GPU de verdade, dentro do `bwrap` |
| durante o port | `/run/giochi/dev-free-console` presente (o supervisor não devolve no meio) |
| o usuário sai do FORM | `giochi-ports` sai, **o marcador some** |
| depois | o supervisor sobe o launcher, com `card0` aberto — a interface volta sozinha |
| app comum (`musica`) | abre, termina e o launcher volta: a correção não mudou esse caminho |

Teste que prende a correção: `sigterm_arma_no_lancamento_e_e_anotado`, em
`system.rs`. Ele falha se o `sigterm::instalar()` sair de `lancar_com_saida`
(conferido revertendo de propósito), e roda em **um** teste só porque `SIGTERM` é
global ao processo: dois testes que armam e restauram o handler em paralelo fazem
o `raise` de um pegar o handler padrão do outro e **a suíte morre com sinal 15**.

## 3c. A loja do PortMaster também é um app

Antes disto, a GUI do PortMaster só abria por SSH: o usuário do aparelho não
tinha como instalar um port. Agora ela tem cartão próprio na aba Apps.

```text
/user/apps/portmaster/
├── .portmaster-port      # marca: "#loja-portmaster"
├── metadata.json         # kind: app, entrypoint: bin/portmaster, category: Loja
└── bin/portmaster        # 755 — exec giochi-ports --liberar-drm
```

Diferenças em relação ao cartão de um port, e o motivo de cada uma:

| | port | loja |
| --- | --- | --- |
| script | `giochi-ports --rodar "<nome>"` | `giochi-ports --liberar-drm` |
| por quê | rodar um jogo já instalado | abrir a GUI, que precisa pedir a tela |
| marca | o nome do port | `#loja-portmaster` — sentinela, para a poda não confundir com um port |
| contagem no log | entra em `N port(s)` | aparece como `; loja: PortMaster` |

`--liberar-drm` é o modo que o próprio `giochi-ports` documenta ("para o
launcher, abre a GUI, e o devolve"): ele cria o marcador, manda `SIGTERM` no
launcher, espera o `card0` liberar e sobe a GUI; ao sair, remove o marcador e o
`giochi-init` devolve a interface. Só funciona porque o launcher passou a morrer
de `SIGTERM` (§3b).

### Duas coisas que o cartão precisa fazer, e por quê

O `giochi-ports` roda `python3 ./pugwash` direto. O caminho **oficial** do
PortMaster (`PortMaster.sh` → `utils/pm_launch.txt`) faz duas coisas antes, e
sem elas a loja não abre — as duas foram medidas no R35S em 26/11/2026:

| Falta | Sintoma | Por quê |
| --- | --- | --- |
| `PYSDL2_DLL_PATH=/usr/lib` | `SDLError: … No available video device` | sem a variável o `pysdl2` carrega o SDL2 de `exlibs/` do pacote, que não tem KMSDRM. É o `pm_hook_gui_env` deles |
| apagar `.pugwash-reboot` | abre e **fecha na mesma hora**, sem mensagem | o `pugwash` lê esse arquivo **depois** de inicializar o SDL e, se ele existe, pula o `pm.run()` e retorna 0. É o pedido de "reinicie-me" do auto-update. O `pm_launch.txt` apaga antes de cada execução |

O arquivo estava em `/user/ports/PortMaster/.pugwash-reboot` **desde 20/11** (de
uma execução anterior da GUI) e a loja nunca abria. É a mesma família do
marcador de DRM: estado que sobra de uma execução e mente sobre a próxima.

O cartão gerado faz as duas coisas antes do `exec`. O laço de reexecução do
`pm_launch.txt` (para quando o `pugwash` pede reinício de verdade) **não** foi
reproduzido: aqui, um pedido de reinício exige que o usuário feche e abra a loja
de novo — um toque, e evita repetir a dança do DRM no meio de um app.

**As duas passaram a existir também no `/usr/bin/giochi-ports`** — o pedido 6 de
[`issues-para-outros-repos.md`](issues-para-outros-repos.md) foi atendido, medido
em 27/11/2026 na imagem `main-37e08b19` (`PYSDL2_DLL_PATH=/usr/lib` na linha 235,
`rm -f "$PUGWASH_REBOOT"` na 254). O cartão **mantém a cópia** e as duas são
idempotentes (`VAR=valor` não acumula, `rm -f` de arquivo ausente é no-op): é o
que faz o cartão abrir numa imagem anterior à correção, sem que ninguém precise
saber qual imagem está no cartão. Não é contorno de defeito vivo — se um dia
divergirem, quem manda é o wrapper, e o teste do cartão é o que avisa.

### Medido no R35S em 26/11/2026

```text
catálogo: 4 jogo(s) e 3 app(s) detectado(s) em ...   ← era 2, agora inclui a loja
sistema: iniciando portmaster (/user/apps/portmaster/bin/portmaster)
giochi-ports: liberando o DRM: parando o PID 1557 (o launcher) — o supervisor o trará de volta ao sair
sistema: SIGTERM durante portmaster — saindo para quem pediu devolver a interface
```

Com as duas correções, amostrado com relógio de verdade: a GUI ficou **de t=15 s
a t=99 s** no ar (a janela inteira do teste), com o marcador do supervisor
presente, o launcher morto, `pugwash` vivo e **zero** erro de vídeo. Antes:
`No available video device` (sem a variável) e saída imediata (com o marcador
velho).

Quando a GUI termina, `giochi-ports` encerra, o marcador some e o supervisor
devolve o launcher com o DRM — medido: `19698 ppid=4177` com `card0` aberto. As
passadas seguintes de `--sincronizar-ports` dizem `2 igual(es)` — não reescreve,
que é o comportamento certo num cartão SD.

## 4. Medido no R35S em 26/11/2026

Imagem `e0d74e6` (com `GIOCHI_WITH_PORTMASTER=1`), port `FORM` instalado:

| Passo | Resultado |
| --- | --- |
| `--sincronizar-ports` com um port | `1 novo(s)` |
| segunda passada | `1 igual(es)` — não reescreve (é cartão SD) |
| port removido do disco | `1 removido(s)` e o cartão some |
| port devolvido | `1 novo(s)` de novo |
| detecção com o cartão | `4 jogo(s) e 2 app(s)` |
| `--lancar portmaster-form` (o mesmo campo do botão A) | `sistema: iniciando portmaster-form (...)`, `giochi-ports: rodando FORM.sh (de /user/ports)` |
| o port dentro do sandbox | `FORM 5 s: chip 52.9 C, frames 13.5 ms, held buttons 00000` |
| `aparelho.sh verificar` com o cartão | **99.5%** — e **99.5%** sem ele (a diferença de 0,5% é anterior e não é do desenho) |

O port rodou com `renderer: software` porque o teste subiu o launcher com
`SDL_VIDEODRIVER=dummy`, e a variável foi herdada. Na produção o launcher roda
em KMSDRM e o port pega `opengles2` — foi o que o Giochi System mediu (60 fps).

---

## 5. A referência do `verificar` passou a incluir o disco

O `gerar_referencia` montava o catálogo do PC **só** a partir de
`/opt/giochi/apps/index.json`. Desde 15/11/2026 a detecção é disco-primeiro
(§6.9), então todo pacote instalado fora do índice — um app da loja, o cartão de
um port — existia no aparelho e não na referência.

Medido: **98,6%** com o cartão de port, e **99,5%** depois de somar a colheita
das quatro raízes (`colher_instalados`, a mesma de
`aparelho.sh catalogo`). O disco vence o índice, como no aparelho.

---

## 6. Como verificar

```bash
# no PC: exercita o módulo sem tocar no aparelho
GIOCHI_PORTS=/tmp/p/ports GIOCHI_APPS=/tmp/p/apps \
  ./app/launcher/target/debug/giochi-launcher --sincronizar-ports

# no aparelho, sem abrir a interface (abrir custa uma volta do giochi-init)
ssh root@<ip> '/opt/giochi/apps/launcher/bin/giochi-launcher --sincronizar-ports'

# o cartão e o script gerados
ssh root@<ip> 'cat /user/apps/portmaster-*/metadata.json'
ssh root@<ip> 'cat /user/apps/portmaster-*/bin/*'

# o caminho de lançamento de produção (o mesmo do botão A)
ssh root@<ip> 'SDL_VIDEODRIVER=dummy /opt/giochi/apps/launcher/bin/giochi-launcher --lancar portmaster-form'
```

---

## 7. O que **não** foi feito, e por quê

| Pedido da issue #2 | Estado |
| --- | --- |
| ports listados na aba Apps | **feito** — cartão por port, detectado do disco |
| formato do alvo sem mudar o launcher | **feito** — wrapper em `bin/`, sem `args` no `Alvo` |
| aba "Ports" própria e `args` no `Alvo` | **não feito** — é UX; a aba Apps já mostra os ports, e `args` mexe no contrato do `Alvo` sem necessidade hoje |
| a GUI do PortMaster na aba Apps | **feito** (§3c) — cartão próprio, com crédito, aberto por `giochi-ports --liberar-drm` |
| PortMaster como item da **loja do PC** (`app/store`) | **não feito** — o pacote não é assinado (GPL-2.0/LGPL-2.1 dentro) e teria de aparecer como **sideload**. Registrado em `AGENTS.md` §16 |
| assinatura dos ports | **não existe, de propósito** — port é código de terceiro; o que o sistema garante é o sandbox, não o conteúdo |

Dois detalhes do sistema que quem for instalar por outro caminho precisa
repetir, medidos pela equipe do System:

1. O `gamecontrollerdb.txt` do PortMaster tem 472 KB e o `control.txt` o exporta
   inteiro em `SDL_GAMECONTROLLERCONFIG`; `MAX_ARG_STRLEN` é 128 KB e **todo
   `exec`** falha com `Argument list too long`. O `giochi-ports` monta o nosso
   DB (2 KB) por cima.
2. O zip do PortMaster sai com modo `600` sob `umask 077`; `entrypoint` `.sh`
   precisa de `+x` e shebang.
