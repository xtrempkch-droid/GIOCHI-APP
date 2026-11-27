# Apps no cartão SD — o que funciona, o que não

Estado em 06/11/2026, medido no X55. Responde "dá para usar cartão SD para levar
apps?" com o que foi executado, e separa o que é do app do que é do sistema.

> Os transcriptos citam `pacman` e `cobrinha` porque foram os pacotes usados na
> medição. O mecanismo (`instalar-do-cartao`, descoberta de cartões, poda do
> catálogo) vale para qualquer `.giochi`.

---

## 1. O X55 não tem eMMC: o sistema roda do próprio cartão

```text
/sys/class/mmc_host/mmc1/mmc1:59b4/type = SD
                                         name = SD128     (128 GB)
mmc3 = SDIO  (o rádio de Wi-Fi)
```

Tabela de partições (`fdisk -l /dev/mmcblk1`, MBR `0x47494f43`):

| Partição | Setores | Tamanho | FS | Rótulo |
| --- | --- | --- | --- | --- |
| `mmcblk1p1` | 131072–655359 | 256 MiB | FAT32 | `GIOCHI_BOOT` |
| `mmcblk1p2` | 655360–4194303 | 1,7 GiB | ext4 | `rootfs` (em execução) |

O último setor usado é **4.194.303 = 2 GiB**. Num cartão de 128 GB sobram
**116,5 GiB sem partição nenhuma**.

```text
/dev/root on / type ext4 (rw)       ← é o maior volume do aparelho
/dev/mmcblk1p1                      ← GIOCHI_BOOT: NÃO MONTADA (nem no fstab)
df /  →  974 MB total, 757 MB livres
```

Ou seja: "espaço para apps" não falta no cartão — falta **partição**.

---

## 2. O que funciona hoje

Copiar `.giochi` pelo leitor do PC, devolver o cartão e instalar no aparelho:

```bash
bash scripts/aparelho.sh instalar-do-cartao
```

```text
ok   /dev/mmcblk1p1 montado em /mnt/cartao (somente leitura)
ok   2 pacote(s) para instalar
  corrompido.giochi: RECUSADO — sha256 não confere (esperado c52a181b…, obtido ca214b23…)
  pacman.giochi: instalado em /user/apps/pacman
RESUMO 1 instalado(s), 1 recusado(s), 2 encontrado(s)
aviso instalação terminou com pacote(s) recusado(s)
```

E o jogo abre:

```text
1297 ppid=1295  giochi-launcher --lancar pacman   ← pai
1305 ppid=1297  /user/apps/pacman/bin/giochi-pacman
```

### Como funciona sem PC no meio

| Passo | Quem faz |
| --- | --- |
| montar `mmcblk1p1` **read-only** | **app** — ninguém monta |
| achar `*.giochi` | `scripts/lib/instalar-do-cartao.sh` (roda no aparelho) |
| conferir SHA-256 contra o manifesto | `sha256sum` da imagem |
| extrair, checar entrypoint, `chmod +x` | `extrair-pacote.sh` |
| pôr no catálogo | `giochi-launcher --catalogo-adicionar <dir>` |

O último passo é o que evita depender de Node: o launcher já tem `serde_json`, e
um `sed` no `index.json` produziria JSON inválido no primeiro campo com vírgula.

**O app montar a partição é medida de app, não invasão** — mesmo motivo do
`ip link set up` no Wi-Fi, que o `S40wifi` do sistema também não faz. Se o
sistema passar a montar, o script detecta (`mountpoint`) e não monta de novo.

---

## 3. O que a verificação cobre — e o que não cobre

| Verificação | No aparelho | Por quê |
| --- | --- | --- |
| SHA-256 contra o manifesto | **sim** | pega cópia corrompida, que é o erro real de quem copia pelo leitor |
| entrypoint existe e é executável | **sim** | o erro apareceria como "não abre", sem apontar causa |
| **assinatura Ed25519** | **NÃO** | não há crate de Ed25519 disponível offline |

**Isso é limitação declarada, não recurso.** A assinatura importa quando o pacote
vem de um canal em que se confia pouco — a loja pela rede. Aqui o arquivo vem de
um cartão que o dono pôs na mão dele: quem escreve no cartão também copia um
binário para `/user/apps` sem passar por este script. O que a verificação
**resolve** é a corrupção silenciosa na cópia.

Enquanto o aparelho não verifica assinatura, o PC continua verificando antes de
enviar (`verificar-pacote.mjs`).

Para fechar isso é preciso um crate de Ed25519 vendorizado no cross-compile — ou
seja, uma decisão de dependência, não um `if`.

---

## 3b. Vários cartões e eMMC — o desenho genérico

Alguns aparelhos têm **dois slots de SD e ainda eMMC**: o sistema pode ficar num
cartão e os apps no outro, e haverá casos de apps **nos dois**. A primeira
versão deste código tinha `/dev/mmcblk1p1` fixo — o mesmo erro do
`DEFAULT_ROTATION = Ccw90` que já foi corrigido na rotação: acerta num aparelho e
mente no seguinte, sem avisar.

### Três armadilhas, todas medidas no X55

| Armadilha | O que se vê |
| --- | --- |
| **`removable` não serve** | `/sys/class/block/mmcblk1/removable = 0` para cartão SD **físico**. Filtrar por `removable=1` não acha nada, nem com dois slots |
| **o glob pega partição** | `/sys/class/block/mmcblk*` casa com `mmcblk1`, `mmcblk1p1` e `mmcblk1p2`; tratar cada um como disco montou a mesma partição duas vezes |
| **`/dev/root` não é caminho** | `/proc/mounts` mostra `/dev/root`, nome lógico; comparar por caminho nunca casa |

### O critério que funciona

1. O disco do sistema é **o que sustenta a partição montada em `/`**, descoberto
   por major:minor em `/proc/self/mountinfo` (fato do kernel, não heurística).
   Esse disco inteiro está fora — inclusive a `GIOCHI_BOOT` dele.
2. Partição é o que tem `/sys/class/block/<nome>/partition`; o kernel só cria
   esse arquivo para partições.
3. Todo o resto com sistema de arquivos reconhecível é candidato.

Assim o mesmo script serve para 1 cartão (0 candidatos, correto), 2 cartões
(1 candidato) e eMMC + 2 cartões (2 candidatos).

### Medido no X55, com dois cartões simulados

No PC não há root para `losetup`; no aparelho não há como inventar um segundo
slot. Então `scripts/lib/descobrir-cartoes.sh` aceita `CARTOES_PADROES` (só para
teste) e o caminho de dois cartões foi **exercitado**, não afirmado:

```text
$ CARTOES_PADROES="…/loop0 …/loop1" aparelho.sh instalar-do-cartao
  ok   2 cartão(ões): /mnt/cartao1 /mnt/cartao2
  /mnt/cartao1: 1 pacote(s)
      pacman.giochi: instalado em /user/apps/pacman
  /mnt/cartao2: 1 pacote(s)
      cobrinha.giochi: instalado em /user/apps/cobrinha
  ok   tudo instalado
```

E o teste que mais importa: com o padrão real, **zero candidatos** — o cartão do
sistema não é montado como se fosse mídia.

### Jogo em cartão pode sumir — e não pode virar cartão morto

Se um jogo for catalogado com `dir` num cartão e o cartão for removido, a Home
mostraria um cartão que não abre. O launcher agora **poda em tempo de execução**
(`Catalogo::podar_sem_binario`), e diz o motivo:

```text
catálogo: fantasma fora — /mnt/cartao-que-sumiu/fantasma/bin/x não existe (cartão removido?)
catálogo: 1 app(s) de /tmp/cat-teste.json
```

A poda é em tempo de execução, não na geração: o PC que gera o índice não sabe se
o cartão estará lá no próximo boot. Quem sabe é o aparelho.

## 4. O que é do Giochi System

Levado ao canal em 06/11/2026 — 05:55 (partição e montagem) e 06:08 (multi-cartão),
com os números acima. As três perguntas objetivas ao System estão na segunda
mensagem: convenção de montagem, onde ficam os apps com eMMC, e se "partição de
dados" e "slot 2" são a mesma coisa ou duas.

| # | Item | Estado em 27/11/2026 |
| --- | --- | --- |
| 1 | **Partição de dados** nos 116,5 GB | **resolvido no Giochi System** ([issue #8](https://github.com/xtrempkch-droid/GIOCHI-APP/issues/8)): a p3 (`GIOCHI_USER`) é montada em `/user` pelo `S20userdata`, achada por **rótulo** — `commit` do System `43e52a5` e ajuste `3fa5b07`. Ver a ressalva da imagem abaixo |
| 1b | **Layout nos aparelhos com 2 slots + eMMC** | **respondido**: nenhum dos três aparelhos tem eMMC (`no-mmc` no `sdmmc1` da X55/X35H; o R35S não tem nó). Apps moram em `/user`, que é a p3 |
| 1c | **Convenção de montagem** | **resolvido**: `/user` fixo; o app **para de montar** — quem monta é o sistema. O que o app ainda monta é a `GIOCHI_BOOT` do cartão **dele**, e só quando ninguém montou (`descobrir-cartoes.sh` checa `/proc/mounts` antes) |
| 1d | **eMMC: apps onde?** | resolvido junto com 1b: `/user`, e quando existir eMMC o caminho não muda — muda o device por baixo, que o app não precisa saber |
| 2 | **Montar `GIOCHI_BOOT`** no boot | **não feito no sistema**. Continua valendo: se o app lê o `giochi-boot.log`, ele monta a p1 por conta própria |
| 3 | **Automount** de mídia removível | **não feito**. Cartão sem o rótulo não aparece montado |

### A ressalva da imagem, medida

O código do sistema **tem** a partição (`S20userdata` em
`device/common/rootfs-overlay/etc/init.d/`, commit `43e52a5`), mas a **imagem
medida no R35S em 27/11/2026** (`main-37e08b19`) **não a tem**:

```console
$ ls /etc/init.d/S20userdata
ls: /etc/init.d/S20userdata: No such file or directory
$ blkid | grep mmcblk1        # só p1 (GIOCHI_BOOT) e p2 (rootfs): não há p3
$ grep -F " /user " /proc/self/mountinfo     # vazio: /user está no rootfs
```

Ou seja: **a promessa é do próximo build**, não desta imagem. É a regra R9 do
[documento de regras](regras.md) no sentido inverso: o relatório estava certo, a
imagem é que é anterior. `aparelho.sh diagnostico` passou a dizer isso na cara,
em vez de deixar deduzir:

```text
user/dados: mesmo device do rootfs (0:32) — a particao de dados NAO esta montada
pastas:    /user/games AUSENTE; /user/apps AUSENTE; /user/musica AUSENTE; /user/ports AUSENTE;
```

### O que mudou do lado do app (issue #8)

| Pedido | O que foi feito |
| --- | --- |
| remover a montagem de `/user` | **já não existia**: nenhum `mount` de `/user` no app, e o `descobrir-cartoes.sh` só monta a `GIOCHI_BOOT` de um cartão que **não é** o do sistema |
| `mkdir -p` defensivo e parar de culpar o usuário | `musica` cria `/user/musica` na abertura (só a pasta **padrão** — `--pasta`/`GIOCHI_MUSICA` são escolha do usuário e criar o que ele digitou errado esconderia o erro), e a frase de pasta ausente passou a dizer que é **falha da partição de dados**. O launcher chama `deteccao::garantir_pastas("/user")` antes da detecção — as quatro pastas do contrato, `mkdir -p` idempotente, e o que ele cria aparece no log (`pastas: /user/musica criada (imagem anterior à partição de dados)`) |
| ler o espaço por `/user` | `aparelho.sh diagnostico` ganhou `dados:` e `user/dados:`, e o **store do PC** passou a ler: `giochi-store espaco` imprime livre/usado do destino, e `instalar` recusa antes de copiar quando não cabe (o número vem do caminho — `/user/games` —, nunca de `/`, que é outro device). O painel mostra o livre em cada destino |
| não ler `/run/giochi/user-antes-da-montagem` | **não é lido** (`grep` no repo inteiro: zero ocorrências) |
| init script depois do S20 | o app **não instala** init script; se um dia precisar, é `S21` para cima |

O item 1 é o que tem consequência jurídica. Sem ele, app GPL vendido pela loja
volta a ser inseparável da imagem assinada, e o argumento da GPL-3.0 §6 que
justifica a loja neste desenho não se sustenta.

---

## 5. Limite prático hoje

- `GIOCHI_BOOT` tem **210 MB livres** (45 MB em uso de 256 MB). Cabem dois ou
  três apps pequenos, não uma biblioteca.
- É a **única** partição que um PC lê (FAT32). A `rootfs` é ext4 e não é
  montável pelo Windows.
- Instalar exige o cartão no leitor e depois `instalar-do-cartao` no aparelho —
  não é "plug and play". Automount + um cartão de dados resolveriam; ambos são
  do sistema.

---

## 6. O que este documento não afirma

Não afirma que a partição vai existir, nem que a imagem nova muda algo: layout
de imagem é decisão do dono com a equipe do System. Se responderem "p3 não vai
existir", a limitação fica registrada e o caminho por SSH
(`instalar-pacote`) continua sendo o único.
