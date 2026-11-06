# RETOMAR AGORA — leia antes de fazer qualquer outra coisa

Escrito em **06/11/2026, 18:0x**, imediatamente antes de um reboot de emergência.
A máquina perdeu o shell (`/usr/bin/bash` → `Exec format error`). Este arquivo é
o bilhete para depois do reboot.

**Depois de ligar, o primeiro comando:**

```bash
cd ~/Giochi/GIOCHI-APP && bash scripts/consertar-shell.sh
```

Se esse script não puder rodar (não há shell), siga a seção 3 de
[docs/salva-vidas.md](docs/salva-vidas.md) — mas o reboot pode ter resolvido:
`ENOEXEC` em `bash` costuma ser arquivo corrompido, e corrupção em disco às vezes
é só leitura suja em cache. **Teste antes de reinstalar nada:**

```bash
bash -c 'echo vivo'        # se imprimir "vivo", acabou o problema
df -h /                    # se estiver 100%, libere espaço ANTES de tudo
```

## Passo 0 — a sobra que eu deixei

Tentei rodar `git` por outros caminhos enquanto o shell estava morto, e um
arquivo de tentativa ficou na raiz: **`salva-vidas.ipynb`**. Ele **não** é código
deste repositório. Já entrou no `.gitignore` para não atrapalhar um `git add -A`,
mas o certo é apagar:

```bash
rm -f salva-vidas.ipynb .vscode/salva-vidas.ipynb
```

### O que foi tentado, e por que nada funcionou

Registro para não repetir a rodada. **Todos os caminhos de execução estão
fechados**, e o motivo não é falta de ideia:

| Caminho | Resultado |
| --- | --- |
| `bash` (ferramenta padrão) | `Exec format error` |
| tarefa do VS Code tipo `shell` | `execvp(3) failed` no `/usr/bin/bash` |
| tarefa do VS Code tipo `process` | o runner insiste em embrulhar em `/usr/bin/bash` |
| `runTask` com `tasks.json` escrito à mão | `Task not found` |
| depurador Python (debugpy) | bloqueado: *workspace is not trusted* |
| kernel Jupyter / notebook | `Notebook file … not found` |
| `/usr/bin/ripgrep` (busca do editor) | `Exec format error` — o mesmo defeito |

**Descoberta útil:** `/usr/bin/dash` está **intacto** (ELF válido). Só `bash` e
`ripgrep` corromperam — o que indica dois arquivos atingidos, não o sistema
inteiro. Deixei um perfil de terminal "dash" **disponível mas não padrão** em
`~/.config/Code/User/settings.json`: se o `bash` continuar quebrado, escolha esse
perfil no seletor do terminal e o reparo vira

```bash
sudo apt-get install --reinstall bash
```

---

## 1. Nada do trabalho do dia foi perdido

O dia inteiro está no GitHub. `main` local e `origin/main` estavam em
**`f842448`** quando o shell caiu, e a árvore de trabalho estava limpa.

| Commit | O que é |
| --- | --- |
| `4a7eaaf` | R35S com launcher instalado; `dlopen` no lugar da feature `som`; `design` no build sem texto; `RESTART_MAX`; o `pid_do_giochi_init` que casava consigo mesmo |
| `f842448` | diário de 06/11 |
| `5508688` | `docs/salva-vidas.md` (salvo pela API, sem git local) |
| `a9a0c7f` | link dele no índice |
| `dd3c756` | este arquivo |

## 2. O que precisa de commit depois do reboot

### `app/launcher/crates/ui-sdl/src/app.rs` — a correção do `energia:`

A equipe do Giochi System apontou, e estava certa: a linha que aparecia a cada
5 s no console do R35S é **nossa**, `app.rs:~654`. Meu diagnóstico anterior
("não é código deste repositório") estava **errado** — procurei a frase
renderizada e não a estrutura que a monta.

O bug: `hud.temp_c` é `u16`, então não há valor que signifique "sem leitura". Com
sensor ausente, `None != Some(x)` é sempre verdadeiro e o "só registra quando
muda" registra sempre.

```rust
// antes
if nivel != self.state.battery || temperatura.map(|t| t as u16) != Some(self.state.hud.temp_c)

// depois  (já aplicado no disco desta máquina)
let temperatura_c = temperatura.map(|t| t as u16);
if nivel != self.state.battery
    || temperatura_c.map_or(false, |t| t != self.state.hud.temp_c)
```

**Não foi compilado nem commitado** — não havia shell. Rodar:

```bash
cd app/launcher
cargo test --workspace          # esperado: 223 (107 core + 116 ui-sdl)
cargo clippy --workspace        # o pedantic sugere `is_some_and`
cd ../..
git add -A && git commit -m "app: a linha energia saia a cada 5s porque Option comparado com u16 e sempre verdadeiro"
```

### `docs/salva-vidas.md` e `scripts/consertar-shell.sh`

O `salva-vidas.md` em disco ganhou a seção 6 (esta correção) e o número da imagem
do R35S depois do commit `5508688`. O `consertar-shell.sh` é novo. Commitar junto.

## 3. Ainda errado na documentação

`AGENTS.md` §23 diz que a linha `energia:` **"não é código deste repositório"**.
É falso, e foi o defeito que originou o diagnóstico errado. O arquivo tem 3.144
linhas e não deu para corrigir pela API sem risco de truncar. A frase a
substituir começa com "E há outra coisa em aberto, do lado do **Giochi System**".

## 4. Novidades do canal (Giochi System, 06/11 19:55)

1. **A imagem do R35S existe e não está gravada**:
   `giochi-system-r35s-sd.img`, sha256 `40ae6925…`, construída 19:37 UTC, com
   `readelf`, `alsactl`, harfbuzz, splash, `respawn` e o `giochi-init` novo (cota
   por tipo de saída + `/run/giochi/launcher-parado`). O cartão está **dentro do
   aparelho**: ou eles gravam, ou nós gravamos com `dd`.
2. **Eles puseram `libasound` na imagem.** A resposta deles diz "o `#[link]` volta
   a funcionar sem `dlopen`, é só recompilar" — está **desatualizada**: o mixer já
   é `dlopen`, então o volume passa a funcionar **sem recompilar**.
3. Perguntas deles em aberto: `GIOCHI_WITH_GODOT=1` é caminho de produto? e
   `hwclock -s` no boot ajuda ou atrapalha o piso de tempo da licença?

## 5. Onde tudo está escrito

- [docs/salva-vidas.md](docs/salva-vidas.md) — o diagnóstico completo do shell.
- [AGENTS.md](AGENTS.md) §23 — o R35S, o `dlopen`, o `RESTART_MAX`.
- [docs/diario/2026-11-06.md](docs/diario/2026-11-06.md) — o dia.
- `/home/charles/Giochi/chat_de_equipe.txt` — o canal (fora dos dois repos).
