# Issues para os outros repositórios

Este repositório **não escreve** nos repos vizinhos — regra do dono do projeto em
15/11/2026, registrada em [AGENTS.md](../AGENTS.md) §1.1. Quando o diagnóstico
conclui que a correção mora em repo alheio, ela vira **issue**, com o medido, o
arquivo e a linha, a consequência e o que a correção provavelmente é.

**Estado em 15/11/2026, medido por `list_issues`:** **uma** das cinco existe —
o pedido **5** (fontes), no `GIOCHI-SYSTEM`, como
[issue #5](https://github.com/xtrempkch-droid/GIOCHI-SYSTEM/issues/5). As outras
quatro continuam só aqui.

E o repositório vizinho tem **quatro** issues que este documento não numerou:

| Issue | É o pedido | Situação |
| --- | --- | --- |
| `GIOCHI-SYSTEM#1`, `#2`, `#3` | **1** (Bluetooth), em três cópias | corpo **antigo e errado**: pede `CONFIG_HID`/`CONFIG_BT_HIDP`, que já estão na imagem (§6.6 do `AGENTS.md`) |
| `GIOCHI-SYSTEM#4` | **5** (fontes) | **duplicata byte a byte** da #5 |
| `GIOCHI-SYSTEM#5` | **5** (fontes) | canônica |

**`#N` neste documento é sempre do GitHub; o pedido do arquivo é "pedido N".** A
numeração dos dois lados colide de propósito nenhum e já produziu confusão: a #4
do GitHub é a duplicata do pedido 5, e a #4 daqui é o pedido do `GIOCHI-OS`.

A duplicata foi **erro do agente**, e a causa é reutilizável: o `list_issues` de
conferência rodou antes de o segundo clique chegar (a #4 nasceu `22:48:11`, a #5
`22:48:33`), então "a issue não existe" foi dito com as duas já criadas. **O
resultado de um formulário assíncrono não se confere antes do clique** — a leitura
devolve o passado.

O caminho de escrita pelo MCP autentica (`POST` em issue inexistente responde
404, não 401) mas **não cria sozinho**: o `issue_write` renderiza um formulário de
confirmação no VS Code e espera um clique que o agente não dá. Pior: o formulário
**não é garantia de que aparece** — o dono do projeto relatou *"dessa vez não
apareceu o pedido para eu clicar"* sobre uma submissão que, afinal, tinha passado.

Por isso o caminho agora é outro, e não depende de extensão nenhuma:

```bash
node scripts/abrir-issue.mjs --lista        # os cinco pedidos
node scripts/abrir-issue.mjs 5 --abrir      # URL do issues/new, já preenchida
node scripts/abrir-issue.mjs 5 --corpo      # só o corpo, para colar à mão
```

O GitHub aceita `?title=` e `?body=` no `issues/new`, então o texto que está
abaixo vira um link: quem clica é a pessoa, mas clica no formulário **do GitHub**,
que ela já conhece. O corpo sai verbatim daqui (uma segunda cópia envelheceria
diferente da primeira) e os links relativos são reescritos para absolutos — um
`idiomas.md` solto numa issue aponta para 404, porque o corpo deixa este
diretório.

Sem atalho por linha de comando: `gh` não está instalado e não há `GITHUB_TOKEN`
no ambiente.

| # | Repositório | Assunto | Gravidade |
| --- | --- | --- | --- |
| 1 | `GIOCHI-SYSTEM` | `CONFIG_UHID` para BLE + `hidp` no boot | média (só BLE) |
| 2 | `GIOCHI-SYSTEM` | Wi-Fi não sobrevive ao reboot | **defeito visível ao usuário** |
| 3 | `GIOCHI-SYSTEM` | Raízes `/opt|/user/games` e o `giochi-selftest` | aviso falso no aparelho |
| 4 | `GIOCHI-OS` | `runAppsPhase` grava todo app em `/opt/giochi/apps` | **latente** |
| 5 | `GIOCHI-SYSTEM` | fontes das escritas do launcher (chinês, hindi, bengali) | **o launcher multi-idioma só desenha 7 dos 10** |

A ordem é por gravidade: a **#2** é a mais grave (a documentação atual deixa o
aparelho pior do que estava), a #1 cobre só BLE depois da correção, e a #4 hoje
acerta por acidente (só há um app `image`) e passa a errar quando houver o
segundo.

### Os quatro pedidos de PortMaster **neste** repositório (26/11/2026)

Antes da tabela acima, este repositório recebeu **quatro** issues iguais sobre o
PortMaster — `GIOCHI-APP#2`, `#3`, `#4` e `#5`, todas com o título *"PortMaster:
listar os ports no launcher e oferecer como app de loja (sideload)"*. Nasceram do
mesmo defeito de mecanismo descrito acima (a conferência antes do clique devolveu
o passado), e a **#2 é a canônica**.

Elas são de **outro assunto**: não são pedidos para repo alheio, são pedidos para
**este** repositório, e foram respondidas em
[`docs/portmaster.md`](portmaster.md):

| Pedido da #2 | Estado |
| --- | --- |
| ports na aba Apps, sem mudar o launcher | **feito** — cartão em `/user/apps/portmaster-<nome>` (26/11/2026) |
| formato do alvo sem `args` | **feito** — o port viaja dentro do script gerado |
| aba "Ports" própria, `args` no `Alvo` | **não feito** — é UX; registrado em §16 do `AGENTS.md` |
| PortMaster como item da loja, declarado sideload | **não feito** — o pacote não é assinado (GPL/LGPL dentro) |

O layout que a #2 sugeria (`entrypoint` na raiz do app) **não funciona**: o
detector exige `bin/` e recusa o resto com `entrypoint inválido`. Medido no R35S.

**As #3, #4 e #5 devem ser fechadas como duplicatas da #2.** O fechamento depende
do mesmo clique de formulário — foi pedido, e o registro de qual foi fechada fica
aqui quando o clique acontecer.


A **#5** é a mais visível para o usuário final: três dos dez idiomas que o
launcher oferece não têm fonte na imagem, e o sintoma é uma tela de quadradinhos.
Ela também é a **mais barata** de corrigir: metade do pedido é ligar um pacote
(`BR2_PACKAGE_WQY_ZENHEI`) que **já está na árvore** e só está desligado.

**Duas destas issues corrigem pedidos anteriores deste repositório**, e isso está
dito dentro de cada uma: a #1 pedia `CONFIG_HID=y` (que já existia, com três
provas falsas), e a #2 pedia `update_config=1` "na imagem" quando o defeito está
na receita de provisionamento. Pedido errado custa a mesma rodada que defeito não
relatado — e a lição está em [AGENTS.md](../AGENTS.md) §13.

---

## 1. `GIOCHI-SYSTEM` — Bluetooth: só falta `CONFIG_UHID`

**Título:** Bluetooth: kernel está completo para HID clássico; falta `CONFIG_UHID` (BLE/HOGP) e `hidp` não sobe no boot

> **Esta issue já foi escrita errada e está corrigida.** A primeira versão pedia
> "o transporte HID do kernel (`CONFIG_HID`/`BT_HIDP`)" e trazia três provas —
> **as três eram falsas**. As provas e o motivo estão no fim, porque a armadilha
> vale mais que o pedido: elas foram para o canal e viraam pedido de mudança de
> imagem para um kernel que já estava certo.
>
> **Ela está aberta três vezes, com o corpo velho:** `GIOCHI-SYSTEM#1`, `#2` e
> `#3`. Este texto é o que deve ser colado **como comentário nas três** — reescrever
> a issue não é possível por aqui (§1.1), e fechá-las sem a correção deixaria o
> pedido errado como última palavra do fio.

### Medido no R35S em 15/11/2026 — `/proc/config.gz`, a fonte que não mente

```console
$ zcat /proc/config.gz | grep -E '^CONFIG_(HID|HID_GENERIC|BT|BT_HIDP|UHID)='
CONFIG_HID=y
CONFIG_HID_GENERIC=y
CONFIG_BT=m
CONFIG_BT_HIDP=m
$ zcat /proc/config.gz | grep UHID
# CONFIG_UHID is not set          <- o único que falta
```

E o transporte funciona:

```console
$ modprobe hidp ; echo rc=$?
rc=0
$ dmesg | grep -i hidp
Bluetooth: HIDP (Human Interface Emulation) ver 1.2
Bluetooth: HIDP socket layer initialized
$ grep -E '^(hidp|bluetooth) ' /proc/modules
hidp 28672 0 - Live ...
bluetooth 507904 1 hidp, Live ...
```

A segunda linha do `dmesg` só é impressa **depois** de a inicialização dar certo.

### O pedido, então

1. **`CONFIG_UHID=y`** — é o único símbolo que falta. Sem `/dev/uhid` o BlueZ não
   consegue criar o dispositivo de entrada de um controle **BLE/HOGP**, e o
   controle pareia e nunca vira `/dev/input/event*`. Medido: `/dev/uhid` não
   existe, e a config diz `# CONFIG_UHID is not set`.
2. **`hidp` carregado no boot** — medido no R35S em 15/11/2026:

   ```console
   $ ls /etc/modules /etc/modules-load.d/ 2>/dev/null
   $ grep -rl hidp /etc/modules /etc/modules-load.d/ /etc/modprobe.d/ 2>/dev/null
   $                                  # <- nenhuma referencia a hidp no autoload
   ```

   Nem `/etc/modules` nem um `modules-load.d/*.conf` citam `hidp`, então o módulo
   `=m` não sobe sozinho: fica fora até alguém rodar `modprobe`. (O estado atual
   do R35S mostra `hidp` carregado porque foi carregado à mão durante este
   diagnóstico — não é o estado de um boot limpo.)

   Baixa gravidade, porque dá para carregar sob demanda — mas o efeito é que o
   primeiro controle pareado depois do boot pode falhar sem motivo aparente, e o
   sintoma ("pareou mas não funciona") não aponta para o módulo.

Para **HID clássico não há nada a mudar no kernel** — `CONFIG_HID=y`,
`CONFIG_HID_GENERIC=y` e `CONFIG_BT_HIDP=m` cobrem o caminho.

### O que continua NÃO medido, e precisa de um aparelho com rádio

O R35S **não tem rádio** (`/sys/class/bluetooth/` vazio, sem `hci0`) e o X55 não
estava na rede em 15/11/2026 (`aparelho.sh procurar` achou só o R35S). Então:

- **não há prova** de que um controle clássico entrega relatório nesta imagem;
- o `disconnected with reason 2` observado no X55 em sessão anterior **não foi
  reproduzido** com o kernel aqui analisado.

O teste decisivo, com um aparelho que tenha rádio e um controle pareado:

```console
$ modprobe hidp
$ bash scripts/aparelho.sh bluetooth
$ bash scripts/aparelho.sh parear <MAC>
$ bash scripts/aparelho.sh input      # lê /dev/input cru, sem mapa
```

Se aparecer um `event*` novo, o transporte funciona e a issue 1 pode ser fechada
só com o item do `UHID`. Se o controle conectar e cair **sem** `event*`, aí sim há
defeito de transporte — e o motivo do BlueZ (em `pareados`) é o próximo dado.

### As três provas falsas, e por que cada uma engana

| Prova usada antes | Por que era falsa |
| --- | --- |
| `modinfo hidp` mostra `depends: bluetooth` sem `hid` | **`modinfo` não existe nesta imagem**: `sh: modinfo: not found`. A linha nunca foi medida — era memória. O equivalente real é `grep hidp /lib/modules/*/modules.dep`, e ele não lista `hid.ko` porque `CONFIG_HID=y` está **embutido no vmlinux**: não há `.ko` para depender |
| `strings hidp.ko \| grep hid_register_driver` devolve vazio | o símbolo no 6.12 é `__hid_register_driver` (gerado pela macro `hid_register_driver`), e `strings` não o mostra. Pior: **não há driver a registrar** — no 6.12 o `hidp` é transport, e o `.ko` referencia `hid_allocate_device`, `hid_parse_report`, `hid_add_device`, `hid_input_report`, `hid_destroy_device`. Esse é exatamente o conjunto de um **transport** |
| `/sys/bus/hid/drivers/hidp` não existe | **é o comportamento correto.** Só um `hid_driver` cria diretório ali; o transport cria `hid_device` e quem aparece no barramento é o `hid-generic`. Esse diretório nunca existiria, em kernel nenhum |

Duas das três mediam a **intenção** ("o módulo parece incompleto") em vez do
efeito ("o módulo carrega e inicializa?"), e a terceira usou uma ferramenta que
não está instalada. A ferramenta do repo do app (`scripts/aparelho.sh bluetooth`)
repetia o mesmo erro e foi corrigida em 15/11/2026: agora ela reporta
`hidp carregado; entrega ao controle NÃO MEDIDA` em vez de "QUEBRADO".

## 2. `GIOCHI-SYSTEM` — Wi-Fi não sobrevive ao reboot

**Título:** Wi-Fi: `update_config` fica fora do caminho de provisionamento — `save_config` responde FAIL e a rede não volta no reboot

### O que foi medido no X55 em 15/11/2026

Conexão real, ponta a ponta, com a rede do dono:

```text
wpa_state=COMPLETED   inet 192.168.1.7/24   ping 192.168.1.1: 0% perda
DNS resolve (github.com)   HTTP ok
```

Mas o `save_config` respondia **`FAIL`**, em silêncio, e a rede sumia no reboot.
A causa foi medida, não deduzida — ela depende da ordem:

| Passo | `save_config` |
| --- | --- |
| como está hoje | **`FAIL`** |
| criando o arquivo com `update_config=1` e reiniciando o `wpa_supplicant` | **`OK`**, e o `ssid=` aparece no arquivo |
| matando e subindo o supplicant de novo | reconecta **sozinho** (`wpa_state=COMPLETED`) |
| **reboot do aparelho** | subiu, associou e pediu DHCP **sem ninguém tocar em nada** |

A `update_config` só é lida na **partida** do supplicant — daí a linha do meio da
tabela falhar e a seguinte funcionar.

### O arquivo existe, e é o template do Buildroot — medido no R35S em 15/11/2026

```console
$ ls -l /etc/wpa_supplicant.conf
-rw-r--r--    1 root     root            78 Nov 15  2026 /etc/wpa_supplicant.conf
$ cat /etc/wpa_supplicant.conf
ctrl_interface=/var/run/wpa_supplicant
ap_scan=1

network={
  key_mgmt=NONE
}
```

Não tem `update_config`, e é por isso que `save_config` responde `FAIL`: essa
opção só é lida na **partida** do supplicant.

**Correção de uma afirmação minha anterior:** o pedido de 15/11/2026 dizia
"pôr `update_config=1` em `/etc/wpa_supplicant.conf`", supondo que o arquivo
nascesse com a imagem. Ele nasce — mas **com o que não deve** e sem o que falta,
e a receita de provisionamento abaixo é o que estraga a diferença.

### Três defeitos, e o segundo é pior que o original

**(a) `S40wifi` diz o contrário do que é.** Linhas 5-7:

```sh
# O arquivo /etc/wpa_supplicant.conf NAO vem na imagem: ele carrega a senha da
# rede do usuario, e senha nao se versiona.
```

O arquivo **vem** — é o template do Buildroot listado acima, e ele não tem senha
nenhuma (é `key_mgmt=NONE`, justamente sem credencial). O comentário está errado
e, pior, a conclusão que ele induz ("não há arquivo, não há `ctrl_interface`") é a
que faz alguém tratar o `>` como inofensivo.

A guarda de verdade deste repositório é outra e está certa:
`scripts/build-device.sh:1236-1249` **falha o build** se o `target/` tiver
`ssid=`/`psk=` em `/etc/wpa_supplicant.conf`. Essa não deve sair.

**(b) A receita de provisionamento destrói o arquivo.**
`docs/x55-bringup.md:618-628` publica:

```sh
wpa_passphrase 'MINHA-REDE' 'MINHA-SENHA' > /etc/wpa_supplicant.conf
chmod 600 /etc/wpa_supplicant.conf
```

O `>` **trunca**. `wpa_passphrase` emite só o bloco `network={ ssid= ... psk= ... }`,
então depois deste comando o arquivo **perde `ctrl_interface` e `ap_scan`**:

- sem `ctrl_interface`, **`wpa_cli` não consegue falar com o supplicant** — a tela
  de Wi-Fi do launcher inteira para de funcionar, e o sintoma ("não lista redes,
  não conecta") não aponta para a receita que foi seguida antes do reboot;
- sem `update_config`, `save_config` continua `FAIL`.

Ou seja: seguir a documentação deixa o aparelho **pior** que não seguir. É o
defeito mais grave das quatro issues deste documento, e é de documentação.

**(c) O placeholder vazio faz a tela procurar para sempre.**
`network={ key_mgmt=NONE }` sem `ssid` faz o supplicant subir e entrar em
`SCANNING` atrás de uma rede aberta sem nome. A tela fica em "procurando…" e
nunca acha nada. Não é "sem Wi-Fi" — é um bloco que parece uma rede.

### O que a correção provavelmente é

Preservar o template e **acrescentar** a linha, em vez de sobrescrever:

```sh
CONF=/etc/wpa_supplicant.conf
{ printf 'ctrl_interface=/var/run/wpa_supplicant\nap_scan=1\nupdate_config=1\ncountry=BR\n';
  wpa_passphrase 'MINHA-REDE' 'MINHA-SENHA'; } > "$CONF"
chmod 600 "$CONF"
/etc/init.d/S40wifi restart
```

E, no `S40wifi`:

1. **corrigir o comentário das linhas 5-7** — o arquivo vem na imagem como
   template sem credencial; o que não vem é a `network={}` do usuário;
2. **garantir `ctrl_interface` e `update_config` no `start`** quando o arquivo
   existir e não os tiver. São configuração, não credencial: não esbarram na
   guarda do `build-device.sh`;
3. **remover o `network={ key_mgmt=NONE }` vazio** antes de subir o supplicant,
   ou não subir quando não houver `network={}` com `ssid=` — um placeholder que
   parece rede custa uma sessão de diagnóstico em campo.

### Como conferir

```console
$ grep -c 'ctrl_interface' /etc/wpa_supplicant.conf # 1   <- wpa_cli volta a funcionar
$ grep -c 'update_config=1' /etc/wpa_supplicant.conf # 1
$ wpa_cli save_config                                # OK
$ grep -c 'ssid=' /etc/wpa_supplicant.conf           # >= 1, e o ssid certo
$ reboot
$ # sem tocar em nada:
$ wpa_cli status | grep wpa_state                    # COMPLETED
$ ip -4 addr show wlan0 | grep inet                  # tem IP
```

A primeira linha é a que pega a regressão que a receita antiga causava: sem
`ctrl_interface` o `wpa_cli` não responde, e aí **nenhum** dos comandos seguintes
tem como passar.

### Consequência do lado do app, enquanto não for corrigido

O launcher detecta a falha e **diz na tela**:

> Conectado a X — a rede não foi salva: vai pedir a senha de novo

Antes isso ia só para o `stderr`, que ninguém lê no aparelho: o usuário
reiniciava e encontrava o console desconectado sem nenhuma pista.

---

## 3. `GIOCHI-SYSTEM` — Raízes de jogos e apps no aparelho

**Título:** Raízes separadas `/opt|/user/games` e `/opt|/user/apps`: o selftest e o layout não acompanharam

### O que mudou no app (decisão do dono, 15/11/2026)

> "os apps deverão ficar em locais diferentes dos jogos para que a detecção não
> se confunda"

O launcher passou a **detectar** apps e jogos do disco, em quatro raízes:

```text
jogos:  /opt/giochi/games    /user/games
apps:   /opt/giochi/apps     /user/apps
```

O `kind` do `metadata.json` (`"game"` | `"app"`) decide a aba; ausente vale
`game`, por compatibilidade. Detalhes em
[`docs/integracao.md`](integracao.md) §2.1 e AGENTS.md §6.9.

### O que precisa acompanhar deste lado

**(a) `giochi-selftest` conta uma raiz só.** Hoje:

```sh
# device/common/rootfs-overlay/usr/bin/giochi-selftest:998-1003
if [ -r /opt/giochi/apps/index.json ]; then
    pass "catálogo de apps: $(grep -c '"id"' /opt/giochi/apps/index.json) app(s)"
else
    warn "sem /opt/giochi/apps/index.json — a grade ficaria vazia"
fi
```

Dois problemas concretos:

- conta linhas com `"id"` no índice e não confere se as pastas existem — o número
  pode ser 4 com zero binários no disco, que é o falso verde que este projeto
  combate;
- a mensagem "a grade ficaria vazia" **deixou de ser verdade**: desde 15/11/2026 o
  launcher varre o disco e um índice vazio não esvazia nada. O aviso manda
  procurar problema no lugar errado.

**(b) As raízes de jogos não são criadas por ninguém.** `grep` em todo o repo não
acha `mkdir` de `/opt/giochi/games` nem `/user/games`, e nenhuma entrada em
`device/common/rootfs-overlay`. Medido no R35S em 15/11/2026:

```console
$ for d in /opt/giochi/games /user/games /opt/giochi/apps /user/apps; do
>   printf '%-22s ' "$d"; [ -d "$d" ] && echo existe || echo "NAO EXISTE"; done
/opt/giochi/games      NAO EXISTE
/user/games            existe
/opt/giochi/apps       existe
/user/apps             existe
```

A raiz de **jogos da imagem** é a única que falta — ela nunca foi criada porque
até 15/11/2026 todo jogo morava em `/opt/giochi/apps`. Pasta que não existe é inofensiva para a
detecção (ela só não varre), mas:

- `device/common/rootfs-overlay/etc/giochi/pontos-de-montagem:19-23` descreve
  `/user` como "dados do usuario, incluindo jogos instalados pela loja" e diz que
  a partição de dados **ainda não foi implementada**. Está correto e serve; só
  falta contar que agora há **duas** subpastas com papéis distintos;
- quem instala um `.giochi` no aparelho (`instalar-do-cartao.sh`, no repo do app)
  já cria a pasta de destino. Então isto é arrumação, não bloqueio — por isso a
  gravidade desta issue é a menor das três do System.

### Como conferir

```console
$ ls -d /opt/giochi/games /user/games /opt/giochi/apps /user/apps
$ giochi-selftest | grep -i catálogo
```

O esperado: as quatro raízes listadas (existentes ou criadas vazias) e uma
contagem que confira o binário de cada entrada, não só o número de linhas do
índice.

---

## 4. `GIOCHI-OS` — `runAppsPhase` grava todo app em `/opt/giochi/apps`

**Título:** Fase apps: o overlay e o índice ignoram o `kind` — app `image` e jogo `image` caem na mesma raiz

### Onde

`scripts/lib/pipeline.mjs:38-80`, na fase `apps`:

```js
const appsDir = path.join(overlayDir, "opt", "giochi", "apps");   // linha 39
...
for (const app of apps.embedded) {
  const target = path.join(appsDir, app.id);                      // linha 51
  ...
  index.push({
    id: app.id,
    version: app.version,
    entrypoint: app.entrypoint,
    dir: `/opt/giochi/apps/${app.id}`,                            // linha 72
  });                                                             // linhas 68-73
}
```

### Consequência

Desde 15/11/2026 o launcher separa jogos e apps em raízes distintas:

```text
jogos:  /opt/giochi/games    /user/games
apps:   /opt/giochi/apps     /user/apps
```

e classifica por `kind` no `metadata.json`. Um **jogo** `image` publicado por esta
fase vai para `/opt/giochi/apps/<id>` e o índice o anuncia lá — o launcher o
desenha na aba correta (a declaração vence o lugar) e escreve uma linha de aviso a
cada abertura:

```text
catálogo: cobrinha (/opt/giochi/apps/cobrinha) está em /opt/giochi/apps, que é raiz
  de outro tipo; o lugar de um jogo é /opt/giochi/games
```

**Hoje isso não acontece porque só existe um app `image`** — `musica`, que é
`kind: "app"`. A fase acerta por acidente. No dia em que um jogo voltar para
`app/apps.json` com `distribution: "image"`, a imagem sai com o jogo na raiz de
app e um aviso por abertura.

### Por que vale corrigir antes de acontecer

A regra do projeto é não aceitar falso verde. Aqui o estado atual é "verde por
falta de caso", que é a mesma família do `--teclas` que aplicava teclas antes de a
tela existir e imprimia um estado plausível e inalterado.

### O que a correção provavelmente é

`scripts/lib/apps.mjs:310` já monta a entrada de `embedded`; basta incluir o
`kind` lá (lido do `metadata.json`, com o default `"game"`) e então:

- `pipeline.mjs`: escolher `opt/giochi/games` ou `opt/giochi/apps` por `app.kind`,
  mantendo o `rmSync`/`mkdirSync` para **as duas** raízes (a recriação é o que
  apaga app removido do catálogo — a lição de 14/11/2026 já está no comentário
  das linhas 44-49);
- `index.push`: `dir: \`/opt/giochi/${app.kind === "app" ? "apps" : "games"}/${app.id}\``;
- `index.push`: acrescentar `kind` à entrada — o launcher já o lê e o default
  ausente é `game`, então isto é compatível com índice antigo.

O contrato do índice está em
[`docs/integracao.md`](https://github.com/xtrempkch-droid/GIOCHI-APP/blob/main/docs/integracao.md)
§2.1 (repo do app).

### Como conferir

O caso de teste que falta em `tests/pipeline.test.mjs`: um app `kind: "app"` e um
`kind: "game"`, ambos `distribution: "image"`, e a asserção de que o overlay tem
`opt/giochi/apps/<app>` **e** `opt/giochi/games/<jogo>`, com o `index.json`
declarando o `dir` e o `kind` de cada um.

---

## 5. `GIOCHI-SYSTEM` — fontes para chinês, hindi e bengali (o launcher fala 10 idiomas)

> **Criada:** [GIOCHI-SYSTEM#5](https://github.com/xtrempkch-droid/GIOCHI-SYSTEM/issues/5).
> A `#4` do mesmo repositório é uma **duplicata byte a byte** desta, aberta 22 s
> antes, e foi fechada como duplicata. Responder na **#5**.
>
> Esta nota fica **antes** do título de propósito: o `abrir-issue.mjs` monta o
> corpo a partir da linha `**Título:**`, então o que está acima dela é
> procedência — não vai para a issue. Um "Criada: #5" dentro do corpo de uma
> issue nova seria informação sobre o passado do arquivo, não sobre o pedido.

**Título:** launcher multi-idioma: faltam fontes de CJK, devanágari e bengali — `zh`/`hi`/`bn` saem em quadradinhos

### O que mudou deste lado

O launcher passou a ter os **dez idiomas do site** em 15/11/2026
([idiomas.md](idiomas.md)). A escolha funciona, a interface traduz e o desenho
escolhe a fonte **por string**: a principal (DejaVu) desenha o latim, e a fonte da
escrita entra só no texto que ela não cobre. Nada disso precisa de mudança no
sistema — **desde que as fontes existam na imagem**. Três não existem.

### Medido no R35S, imagem `built_at_utc=2026-11-15T21:00:56Z`

```console
$ find /usr/share/fonts -type f | wc -l
21
$ find / -name 'Noto*' -maxdepth 6 2>/dev/null | head -5
$ find / -name '*wqy*' -o -name '*Droid*' 2>/dev/null | head -5
```

Vinte e uma fontes, **todas DejaVu**. Nenhuma Noto, nenhuma WenQuanYi.

E o veredito, medido com o `TTF_GlyphIsProvided` do próprio aparelho (comando
novo `giochi-launcher --fontes`, que roda por SSH sem tela):

```console
$ giochi-launcher --fontes --idioma pt
veredito: 15/15 caractere(s) cobertos por /usr/share/fonts/dejavu/DejaVuSans.ttf
$ giochi-launcher --fontes --idioma zh
veredito: 5/8 caractere(s) cobertos por /usr/share/fonts/dejavu/DejaVuSans.ttf
$ giochi-launcher --fontes --idioma hi
veredito: 1/11 caractere(s) cobertos por /usr/share/fonts/dejavu/DejaVuSans.ttf
$ giochi-launcher --fontes --idioma bn
veredito: 1/10 caractere(s) cobertos por /usr/share/fonts/dejavu/DejaVuSans.ttf
```

| Idioma | Cobertura medida | Efeito na tela |
| --- | --- | --- |
| `en`, `es`, `fr`, `pt` | **15/15** | legível |
| `ru` | **10/10** | legível |
| `ar`, `ur` | **10/10** | legível — e com shaping correto (abaixo) |
| `zh` | **5/8** | quadradinho nos caracteres que faltam |
| `hi` | **1/11** | praticamente tudo quadradinho |
| `bn` | **1/10** | idem |

O log avisa uma vez por execução:

```text
aviso: nenhuma das 1 fonte(s) cobre "中文" — o texto sai com glifo faltando
       (instale a fonte do idioma; ver docs/idiomas.md)
```

### A consequência

O usuário escolhe हिन्दी na tela de boas-vindas e recebe uma interface de
quadradinhos. **Não é falha do launcher** — ele traduziu e pede a fonte certa; a
imagem é que não tem a fonte. Hoje o app **degrada e avisa**, o que é melhor que
não abrir e pior que funcionar. A lista de idiomas continua utilizável porque cada
linha mostra o nome nativo **e** o nome em inglês à direita.

### O pedido — e ele é barato, porque metade já está na árvore

**O `BR2_PACKAGE_WQY_ZENHEI` já existe no Buildroot de vocês e está desligado.**
Medido nos três `buildroot.config` gerados:

```console
$ grep -n WQY out/_local-r35s-*/buildroot.config out/powkiddy-x55/buildroot.config out/powkiddy-x35h/buildroot.config
995:# BR2_PACKAGE_WQY_ZENHEI is not set      ← r35s
1000:# BR2_PACKAGE_WQY_ZENHEI is not set     ← x55
956:# BR2_PACKAGE_WQY_ZENHEI is not set      ← x35h
```

Ou seja: para o **chinês**, a correção é **uma linha por defconfig**, sem baixar
nada novo.

| Escrita | Como obter | Custo |
| --- | --- | --- |
| CJK (chinês) | `BR2_PACKAGE_WQY_ZENHEI=y` — **já na árvore** | poucos MB, sem fonte nova no repositório |
| devanágari (hindi) | `BR2_PACKAGE_NOTO` + família devanágari, **ou** copiar `NotoSansDevanagari-Regular.ttf` para `/usr/share/fonts/truetype/noto/` | **224 KB** (medido) |
| bengali | idem, `NotoSansBengali-Regular.ttf` | **200 KB** (medido) |
| árabe (opcional) | `NotoNaskhArabic-Regular.ttf` | 172 KB — **não é necessário**: o DejaVu cobre o alfabeto árabe (10/10 medido) |

Tamanhos conferidos no PC com `du -h`, para não pedir "um pacote de fontes" sem
número: `NotoSansCJK-Regular.ttc` = **19 MB** (por isso o `wqy-zenhei`, que é bem
menor, é a escolha certa para uma imagem de console).

### Um detalhe que precisa acompanhar, senão o pacote não resolve

O Buildroot instala cada fonte na **sua própria** pasta
(`wqy-zenhei` → `/usr/share/fonts/wqy-zenhei/`), e não numa comum. O app passou a
procurar também em `/usr/share/fonts/wqy-zenhei`,
`/usr/share/fonts/wqy-microhei` e `/usr/share/fonts/truetype/wqy` **neste mesmo
commit** — então ligar o pacote já funciona, sem segunda rodada.

Se preferirem um caminho único, `/usr/share/fonts/truetype/noto/` também está na
lista e é o que o Debian usa.

### Como conferir (sem precisar de tela)

```bash
giochi-launcher --fontes --idioma zh     # tem de dar 8/8
giochi-launcher --fontes --idioma hi     # tem de dar 11/11
giochi-launcher --fontes --idioma bn     # tem de dar 10/10
```

O comando sai com o veredito medido, então a conferência é uma linha por idioma.

### O que **não** é pedido para o System (e este arquivo já errou nisso)

A versão anterior desta issue dizia "`SDL2_ttf` sem HarfBuzz, as letras árabes
saem isoladas". **Estava errado, e era suposição.** Medido no R35S:

```console
$ readelf -d /usr/lib/libSDL2_ttf-2.0.so.0 | grep -i needed
 (NEEDED)  Shared library: [libharfbuzz.so.0]
```

O `defconfig` de vocês liga `BR2_PACKAGE_HARFBUZZ=y` — e ele chega ao `SDL2_ttf`
de fato. A contraprova visual: o mesmo texto árabe no aparelho (KMSDRM) e no PC
dá **99,8%** de identidade; se um dos dois não fizesse shaping, as formas isoladas
contra as conectadas derrubariam esse número.

O que **continua** faltando em `ar`/`ur` é **bidi de parágrafo misto** (árabe +
número + latino na mesma linha) e layout espelhado — e isso é do `SDL2_ttf` (que
faz shaping por *run*, não reordena parágrafo) e do layout do app, **não da
imagem**. Fonte não conserta, e por isso não entra neste pedido.
