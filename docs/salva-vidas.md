# Salva-vidas: quando o shell morre

Escrito em **06/11/2026, 17:57**, alguns minutos depois de a máquina de
desenvolvimento perder o shell. Existe para responder três perguntas na ordem em
que elas importam:

1. **perdi trabalho?**
2. **o que está quebrado, exatamente?**
3. **como eu trabalho enquanto não tem shell?**

Ele é curto de propósito: num momento de aperto ninguém lê cinco páginas.

---

## 1. Não se perdeu nada — e a evidência está abaixo

O `bash` quebrou por volta das **13:50**, logo depois do último commit do dia. A
partir dali não houve produção: o `dist/testes/` (que ganha uma pasta com data e
hora a cada verificação no aparelho) para em **`20261106-134423`**. Ou seja, entre
a quebra e este documento não nasceu trabalho não salvo.

Estado conferido sem rodar `git` — lendo os arquivos de referência do próprio
repositório, que são texto puro:

```text
.git/refs/heads/main                  f84244867c5b...
.git/refs/remotes/origin/main         f84244867c5b...
topo de main no GitHub                f84244867c5b...
```

Os três iguais significam: **local e remoto estão em sincronia, nada por
empurrar.** Os dois commits de hoje estão no GitHub:

| Commit | O que trouxe |
| --- | --- |
| `4a7eaaf` | R35S com launcher instalado; `dlopen` no lugar da feature `som`; `design` no build sem texto; `RESTART_MAX` e o `pid_do_giochi_init` que casava consigo mesmo |
| `f842448` | seção do diário de 06/11 |

Árvore de trabalho limpa: o último `git status --short` da sessão (13:50, depois
do `git add -A && git commit`) não imprimiu nada.

> **A lição que este episódio deixa.** Os dois commits só sobreviveram porque
> foram **empurrados**. Enquanto estavam só no disco, "commitado" e "salvo" eram
> coisas diferentes — exatamente a distinção de "existe, declara e é usado" que
> este repositório aplica a bibliotecas e a botões. Vale para git também:
> **commit não é backup; push é.**

### O que NÃO está em git, e não pode estar

| Coisa | Onde | Por que fica fora |
| --- | --- | --- |
| `chat_de_equipe.txt` | `/home/charles/Giochi/` | canal entre as duas equipes; nenhum dos dois repos o vê |
| `dist/testes/*` | ignorado | quadro é evidência para olhar agora, não artefato |
| imagens e rootfs | `GIOCHI-SYSTEM/out/` | outro repositório, e são GiB |
| o cartão dos aparelhos | hardware | — |

---

## 2. O que quebrou

O binário `/usr/bin/bash` **não é mais um executável válido**:

```text
Terminal process "/usr/bin/bash '-c', 'git status'" failed to launch
execvp(3) failed.: Erro no formato exec
```

`ENOEXEC` (*exec format error*) não é "não achei", nem "faltou biblioteca":

| Erro | Significado |
| --- | --- |
| `No such file or directory` | o arquivo não existe (ou o *loader* que ele pede) |
| `cannot open shared object file` | existe, mas falta uma `.so` — o caso do `libasound` no R35S |
| **`Exec format error`** | **o arquivo existe e não é um executável válido** |

Ou seja: o `bash` foi **truncado ou substituído**. O mesmo aconteceu com o
`ripgrep` (a busca do editor parou com o mesmo erro), o que indica causa sistêmica
e não um arquivo isolado.

**A hipótese mais provável é disco cheio durante uma escrita.** É o modo clássico
de chegar aqui: enche o disco, o arquivo é escrito pela metade, o cabeçalho ELF
vai embora, e o resultado é `ENOEXEC` em vez de uma mensagem que fale de espaço.

O que **continua funcionando** (testado nesta sessão): leitura e escrita de
arquivos, e a API do GitHub. Foi com essas duas que este documento foi salvo.

---

## 3. Quando houver um shell (ou antes dele), nesta ordem

```bash
df -h /                                              # 1. tem espaço?
ls -l /usr/bin/bash /usr/bin/git                     # 2. tamanho plausível?
file /usr/bin/bash                                   # 3. ainda é ELF?
sha256sum /usr/bin/bash /usr/bin/git                 # 4. e o git?
dmesg | tail -40                                     # 5. I/O error? ENOSPC?
```

`bash` saudável tem ~1,2 MB e o `file` diz *ELF 64-bit LSB pie executable*. Se
`file` disser *data* ou o tamanho estiver perto de zero, o arquivo está corrompido
— reinstalar é o caminho, não "consertar".

Se o `df` mostrar 100 %, **libere espaço antes de qualquer coisa**: reinstalar por
cima de um disco cheio corrompe o próximo binário.

### Reparo, por cenário

| Cenário | O que fazer |
| --- | --- |
| Linux normal, `apt` disponível | `sudo apt-get install --reinstall bash git ripgrep` |
| Nada executa (nem `apt`) | mídia live → montar a raiz → `chroot` → reinstalar os pacotes |
| **WSL** | no PowerShell: `wsl --shutdown`; se voltar quebrado, `wsl --update` e reinstalar a distro |
| Disco com erro de I/O no `dmesg` | trocar o disco antes de reinstalar; senão volta a acontecer |

### Depois do reparo, confirme que o trabalho está são

```bash
git status                     # deve dizer "nothing to commit, working tree clean"
git fsck --full                # integridade dos objetos
git log --oneline -3           # f842448 no topo
git pull                       # traz este documento, que foi salvo pela API
```

Se `git fsck` reclamar, o GitHub tem tudo: `git fetch origin && git reset --hard
origin/main` reconstrói a árvore a partir do remoto.

---

## 4. Como trabalhar enquanto não há shell

Foi o que permitiu salvar o dia. Sem `bash`, ainda existem:

| Ferramenta | Para quê |
| --- | --- |
| ler/escrever arquivo | inspecionar e editar o repositório |
| **API do GitHub** | ler o remoto, e **criar commit sem git local** |
| `view` em `.git/refs/...` | saber em que commit o local está — é texto puro |
| nome das pastas em `dist/testes/` | relógio: `AAAAMMDD-HHMMSS` da última atividade no aparelho |

Três truques que valem a pena lembrar:

1. **`git log` sem `git`.** `.git/refs/heads/main` e
   `.git/refs/remotes/origin/main` contêm o SHA em texto. Iguais = em sincronia.
2. **Commit sem shell.** A API do GitHub escreve arquivo e cria commit no remoto.
   Serve para documento e código-fonte; **não serve para binário** (o conteúdo
   passa por texto e corromperia `app/launcher/bin/giochi-launcher` — esse precisa
   de `git push` de verdade).
3. **A pasta de teste é o relógio.** Sem `date`, a última pasta em `dist/testes/`
   diz quando o aparelho foi tocado pela última vez.

O que **não** dá para fazer sem shell: compilar, rodar teste, falar com o aparelho
(`ssh`/`scp`), empacotar e assinar. Nesses casos, a atitude honesta é registrar e
esperar — não afirmar que passou.

---

## 5. O que estava em aberto quando o shell caiu

Nada disso é trabalho perdido: tudo está em `AGENTS.md` §23 e no
[diário de 06/11](diario/2026-11-06.md). Fica aqui só como fila, para a retomada
não começar do zero:

1. **Um reboot não explicado no R35S** — matar o launcher por SSH derrubou a
   conexão e o aparelho voltou com uptime de 245 s. O `respawn` funcionou depois,
   então não foi ele. Pergunta aberta no canal.
2. **Uma linha a cada ~5 s no console do R35S**
   (`energia: bateria sem medidor · temperatura sem sensor`). Texto do
   `giochi-selftest` do **Giochi System**, não é syslog, nenhum processo com
   `selftest` visível no momento da amostra. Reportado a eles em 06/11 13:50.
3. **`libasound` no R35S** — se eles adicionarem a biblioteca na imagem, o volume
   passa a funcionar **sem recompilar** (o mixer é `dlopen`).
4. **`KEY_POWER` (116) no R35S** — não existe entre os 19 códigos; é device tree,
   não pacote.
