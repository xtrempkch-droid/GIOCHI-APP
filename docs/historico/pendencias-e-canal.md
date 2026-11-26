# Pendências e canal com o Giochi System — histórico

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**. Resumo em
> [`AGENTS.md`](../../AGENTS.md) §16 e §17. Inclui o estado medido das
> issues abertas nos repositórios vizinhos.

---

## 16. O que falta

| Item | Estado |
| --- | --- |
| app `image` que não seja shell | **existe**: `musica` (§7) — o catálogo deixou de sair vazio (§8) |
| migrar jogos de `/user/apps` para `/user/games` | **manual** — feito à mão no R35S em 15/11/2026; a detecção os mantém corretos e só avisa, mas um `scripts/migrar-raizes.sh` evitaria repetir |
| barra de progresso no app de música | **não existe** — o `SDL_mixer` não informa a posição; a barra mostraria número inventado |
| duração de MP3/OGG/FLAC no app de música | **`—`** — só o WAV declara o tamanho no cabeçalho |
| licença consultada antes de lançar | **não feito** — o verificador existe, o launcher ainda não pergunta ao servidor |
| conexão Wi-Fi real | **feita e persistente** (15/11/2026) — falta só o `update_config=1` na imagem (§17) |
| pareamento BT virar entrada | **bloqueado na imagem** — pedido no canal (§17) |
| analógico | **não lido** — normalizar exige `absinfo` por ioctl. O controle virtual de 18/11/2026 já entrega `-32767..32767` e dispensa o ioctl; falta implementar (§6.3, `docs/controle-virtual.md`) |
| controle virtual `Giochi Gamepad` | **documentado, não implementado** (18/11/2026) — 17 botões e 4 eixos num nó só; plano, armadilhas e decisões abertas em `docs/controle-virtual.md` |
| tema em tempo de execução | **a base existe** (15/11/2026): `Tema` no estado, `Paleta` + `theme::accent()` lendo o tema do quadro, arquivo em `/user/.config/giochi/theme`. Falta o **segundo** tema — hoje há **um**, e a linha da Config o anuncia como único |
| idioma | **feito**: os dez do site, padrão inglês, boas-vindas na primeira abertura e troca em Config › Aparência (§6.10) |
| fonte para `zh`/`hi`/`bn` | **falta na imagem** — medido no R35S: `zh` 5/8, `hi` 1/11, `bn` 1/10 de cobertura. Pedido nº 5 em `docs/issues-para-outros-repos.md` |
| shaping de `ar`/`ur` | **funciona** — HarfBuzz presente no R35S (`readelf` medido) e 99,8% de identidade contra o PC |
| bidi de parágrafo misto / layout espelhado | **não existe** — `SDL2_ttf` não reordena parágrafo e nenhuma tela inverte âncora. Fonte não conserta |
| teclado de senha em outros alfabetos | **não existe** — `KEYBOARD` é QWERTY latino fixo; senha cirílica/árabe só pelo PC |
| `bash -n` no `build-device.sh` | fica no **Giochi System**, outro repositório |

## 17. Canal com o Giochi System

A conversa entre as equipes vive em `/home/charles/Giochi/chat_de_equipe.txt` —
**fora dos dois repositórios**, porque nenhum dos dois é visível para o outro lado.
Aqui há só um ponteiro: `docs/mensagem-giochi-system.md`.

**Janela: 08:00–18:50, horário de Brasília.** Fora dela não se responde nem se
inicia rodada. `bash scripts/canal.sh` responde, e `--exigir` sai com 1 para
encadear em script.

Pedidos abertos em **`docs/issues-para-outros-repos.md`** — os cinco corpos
prontos, com medição, arquivo e linha, consequência e como conferir. Resumo, com
o estado **medido por `list_issues` em 15/11/2026**:

| # (doc) | Repo | Pedido | Gravidade | Issue no GitHub |
| --- | --- | --- | --- | --- |
| 1 | System | `CONFIG_UHID=y` (BLE) + `hidp` no boot | média — clássico já funciona no papel | **#1, #2, #3** — três cópias, e o **corpo é o antigo** (ver abaixo) |
| 2 | System | `update_config` no **caminho de provisionamento** do Wi-Fi | **a mais grave** | não existe |
| 3 | System | raízes `/opt|/user/games` e `giochi-selftest` contando o binário | aviso falso | não existe |
| 4 | OS | `runAppsPhase` escolher a raiz por `kind` e gravar `kind` no índice | latente | não existe |
| 5 | System | fontes de CJK/devanágari/bengali — `BR2_PACKAGE_WQY_ZENHEI=y` (já na árvore) + 2 arquivos de 224/200 KB | **o mais visível**: 3 dos 10 idiomas em quadradinhos | **#5** (canônica) — a **#4** é duplicata byte a byte |

**O número do documento não é o número da issue, e essa confusão já é o
primeiro defeito da tabela.** A #4 do GitHub é a duplicata da #5, enquanto a #4
daqui é o pedido do `GIOCHI-OS`; e as issues #1–#3 do System são todas o pedido
#1 daqui. Quem apontar "a issue 4" sem dizer de qual lado está falando manda a
outra pessoa para o repositório errado. Daqui em diante: **`#N` sozinho é do
GitHub**, e o pedido do documento é sempre "pedido N do documento".

**A #5 está criada e é a única com estado medido do lado de cá.** Ela é a mais
barata das cinco, e a verificação ponta a ponta foi feita **antes** de abrir:
simulando o que o pacote faria (fonte em `/usr/share/fonts/wqy-zenhei/`), o
`--fontes` no R35S foi de **5/8 para 8/8** e o quadro mudou 8.557 px, com 4.760
no texto chinês — e **0** na linha do hindi, que é o controle negativo. Ligar o
pacote resolve sem segunda rodada do app.

**A duplicata foi um erro do agente, e o registro vale mais que a limpeza.** Duas
submissões ao `issue_write` foram dadas como "paradas no clique" porque o
`list_issues` de conferência rodou **antes** de o segundo clique chegar (22:48:11
a #4, 22:48:33 a #5). O dono do projeto recebeu "a issue não existe" com as duas
já criadas. A lição: **o resultado de um formulário assíncrono não se confere
antes do clique** — a leitura devolve o passado, e chamar isso de verificação é o
§13 na sua forma mais cara, porque o efeito é uma issue duplicada no repositório
de outra equipe.

**As três de Bluetooth carregam o pedido antigo, que está errado.** Elas pedem
`CONFIG_HID` e `CONFIG_BT_HIDP`, que **já estão** na imagem (§6.6, §13); o que
falta é `CONFIG_UHID` e o `hidp` no autoload. Não foram reescritas porque escrever
em repo alheio é só por issue (§1.1) e a correção é um comentário que precisa
deste corpo novo — o texto corrigido está no pedido 1 do documento, pronto para
ser colado como comentário nas três.

**A #4 foi fechada como duplicata da #5** em 15/11/2026, pelo mesmo caminho de
formulário; se ela ainda aparecer aberta, o clique não aconteceu — e agora se sabe
que isso não significa que a operação falhou, significa que **não houve operação**.

**O pedido #2 CORRIGE o anterior, e a correção é do meu lado.** Até 15/11/2026
esta seção pedia "`update_config=1` em `/etc/wpa_supplicant.conf`" como se o
arquivo fosse criado pela imagem. Medido no R35S em 15/11/2026, ele **existe** —
é o template que o Buildroot instala:

```console
$ ls -l /etc/wpa_supplicant.conf
-rw-r--r-- 1 root root 78 /etc/wpa_supplicant.conf
$ cat /etc/wpa_supplicant.conf
ctrl_interface=/var/run/wpa_supplicant
ap_scan=1

network={
  key_mgmt=NONE
}
```

Três consequências, e as duas primeiras ninguém tinha visto:

1. **O comentário do `S40wifi:5-7` está errado** — ele afirma "o arquivo NAO vem
   na imagem", e vem. Quem o lesse concluiria que não há `ctrl_interface`, que é
   justamente o que faz `wpa_cli` funcionar.
2. **A receita de provisionamento destrói esse arquivo.** `docs/x55-bringup.md:618`
   manda `wpa_passphrase 'REDE' 'SENHA' > /etc/wpa_supplicant.conf`, e o `>` trunca:
   some `ctrl_interface` e some `ap_scan`, então **`wpa_cli` para de responder** e a
   tela de Wi-Fi do launcher morre inteira — um defeito pior que o original.
3. **O placeholder vazio é uma armadilha própria.** `network={ key_mgmt=NONE }`
   sem `ssid` faz o supplicant entrar em `SCANNING` atrás de uma rede aberta que
   não existe: a tela fica "procurando" e nunca acha. Não é "sem Wi-Fi" — é um
   bloco que parece uma rede.

A correção certa é no **caminho de provisionamento**, preservando o template e
acrescentando a linha — não "pôr a linha na imagem", que já foi tentado e é
apagado por regravar o cartão. É a mesma família do §13: pedir de novo o que já
era impossível é a ferramenta medindo a intenção em vez do efeito.

**Estado do mecanismo de issue, medido em 15/11/2026:** o MCP do GitHub
autentica (`POST` em issue inexistente devolve 404, não 401 — o `add_issue_comment`
provou), mas `issue_write` **renderiza um formulário de confirmação no VS Code** e
não cria nada sozinho: o agente não consegue clicar. Consequência prática: o
entregável é o corpo pronto em `docs/issues-para-outros-repos.md`, e o número da
issue só entra na tabela acima **depois** que existir de fato. Anunciar "issue
criada" sem ter criado é o §10 aplicado à conversa entre equipes.

**E o formulário não é garantia de nada:** duas submissões seguidas terminaram com
o dono do projeto dizendo que *não apareceu* o pedido para clicar. Hoje o caminho
não passa por ele — `node scripts/abrir-issue.mjs 5 --abrir` monta a URL do
`issues/new` com título e corpo preenchidos (§1.1), e o clique que resta é no
GitHub. O mecanismo, o tamanho da URL e o `--corpo` estão descritos no §1.1.

E o item 1 foi **refeito do zero** em 15/11/2026, porque o pedido anterior estava
errado: `CONFIG_HID=y` e `CONFIG_BT_HIDP=m` **já estão** na imagem, e as três
provas que sustentavam o pedido eram falsas (§6.6). O que resta de verdade é
`# CONFIG_UHID is not set` e `hidp` fora do autoload.
