# Jogos no cartão SD — o que funciona, o que não

Estado em 06/11/2026, medido no X55. Responde "dá para usar cartão SD para levar
jogos?" com o que foi executado, e separa o que é do app do que é do sistema.

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

Ou seja: "espaço para jogos" não falta no cartão — falta **partição**.

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

## 4. O que é do Giochi System

Levado ao canal em 06/11/2026 05:55, com os números acima.

| # | Item | Por que trava |
| --- | --- | --- |
| 1 | **Partição de dados** nos 116,5 GB | hoje `/user` é diretório **dentro do rootfs**. Regravar a imagem apaga os jogos comprados — e `distribution: store` ("fora da imagem assinada") deixa de ser verdade |
| 2 | **Montar `GIOCHI_BOOT`** no boot | não está no `fstab` nem montada. Se eles montarem num caminho fixo, o app para de montar |
| 3 | **Automount** de mídia removível | não há `mdev.conf` nem udev ativo. Irrelevante para o cartão de boot; necessário para um segundo slot ou pendrive |

O item 1 é o que tem consequência jurídica. Sem ele, app GPL vendido pela loja
volta a ser inseparável da imagem assinada, e o argumento da GPL-3.0 §6 que
justifica a loja neste desenho não se sustenta.

---

## 5. Limite prático hoje

- `GIOCHI_BOOT` tem **210 MB livres** (45 MB em uso de 256 MB). Cabem dois ou
  três jogos pequenos, não uma biblioteca.
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
