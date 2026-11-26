# Apps de PC e o pacote `.giochi` — histórico medido

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**. Resumo em
> [`AGENTS.md`](../../AGENTS.md) §9 e §10. Inclui forge, store, painel, loja
> e o formato do `.giochi` com a assinatura Ed25519.

---

## 9. Os apps de PC

### forge e store — Python, biblioteca padrão apenas

Decisão de projeto: a loja tem de funcionar numa máquina limpa. Isso torna o
`.pyz` possível — **um** arquivo, idêntico nos três sistemas — e o PyInstaller
desnecessário para quem já tem Python 3.9+.

| | `.pyz` | PyInstaller |
| --- | --- | --- |
| artefatos | um | um por sistema e por arch |
| determinístico | **sim** | não |
| tamanho | ~14 KB | ~10 MB |
| precisa de Python | **sim** (3.9+) | não |

`scripts/lib/zip.mjs` escreve o ZIP à mão pela mesma razão do `tar.mjs`: o
`zipapp` oficial grava o `mtime` de origem e dois empacotamentos do mesmo código
dariam SHA-256 diferentes — o checksum deixaria de identificar conteúdo. Medido:
mesmo SHA-256 em duas rodadas.

```bash
bash scripts/testar-apps-pc.sh            # 90 testes
node scripts/empacotar-pc.mjs             # .pyz + .sh + .cmd + SHA256SUMS
app/painel/bin/giochi-painel              # http://127.0.0.1:8710
```

**`bin/` tem lançador POSIX** que roda o módulo de `../src`: o repositório clonado
funciona sem instalar nada. **Windows não executa `sh`** — ali o caminho é o
`.cmd` da release, e isso está na documentação em vez de ser escondido.

**O erro clássico de batch:** `py ... %* && exit /b %ERRORLEVEL%` não funciona —
em `.cmd` o `%VAR%` é expandido quando a linha é **lida**, então `%ERRORLEVEL%`
vale o valor de antes. O atalho usa `where py` numa linha e `if %ERRORLEVEL%==0`
na seguinte, e diz o que instalar se nenhum Python existir. Duplo-clique que abre
e fecha sem texto é o pior resultado possível.

O `sdl2` 0.37 tem `default = []`, sem `use-vcpkg`: o `sdl2-sys` emite só `-l SDL2`
e deixa o linker procurar, então exportar `SDL2_DIR` não faz nada. O que funciona
no MSVC é `LIB`, com os zips oficiais "VC" (`include/` + `lib/x64/`).

### forge — classificação

Perfis medidos em `device.conf` do Giochi System: `x55`, `x35h`, `x35s`, `r35s`
e três genéricos por chip (`rk3326-1gb`, `rk3326-2gb`, `rk3566-2gb`).

Três regras que só existem porque a medição derrubou a crença anterior:

1. **O painel é do aparelho, não do SoC.** X55 é 720×1280 e X35H é 640×480, os
   dois com RK3566. Um perfil por chip reprovava no X55 um app que roda lá.
2. **A tela é comparada com o espaço de projeto, não com o painel cru.**
   `usable_display()` = maior lado × menor lado. O X55 é retrato, mas nenhum app
   desenha em retrato: 720×1280 → 1280×720, e um app 1280×720 **cabe**.
3. **`godot_runtime` tem três estados.** `False` = medido que não tem (X55, X35H,
   X35S), `True` = medido que tem, `None` = não medido (perfis genéricos). Só
   `False` reprova: **não medir não é reprovar**. Nenhum build oficial do Godot
   tem KMSDRM, então `runtime: godot` é **Incompatível** — antes era classificado
   como compatível, o falso verde que este repositório combate.

A RAM dos perfis é a **medida** (`/proc/meminfo`: 1968 MB no X55, 964 MB no
X35H), não o valor nominal do SoC — o firmware reserva memória para a GPU, e um
app que declara "preciso de 2048 MB" passaria no número redondo e morreria no
real.

E o rootfs é **glibc 2.38**, não musl. "Recompilar contra musl" não resolveria
nada: o bloqueio de Godot é KMSDRM, não a libc.

### painel — a interface web dos apps de PC

Serve `127.0.0.1:8710`, mostra os aparelhos reais, classifica um app e empacota o
`.giochi`. **Chama o forge e o store; não os copia** — uma segunda implementação
da regra de compatibilidade envelheceria diferente da primeira, e a primeira é a
que decide o que o usuário pode comprar. A separação `acoes` × `servidor` é o que
permite testar sem abrir socket.

**`importlib.resources`, não `Path(__file__)`.** Dentro de um `.pyz` não existe
diretório ao lado do módulo: `__file__` aponta para dentro do zip e
`Path.exists()` devolve `False`. Com `Path`, o painel **empacotado** serve 404 na
própria página. E `__package__` é `None` no 3.9 dentro de um zip (o `zipimport`
legado de lá não preenche), então `resources.files(None)` estoura com
`AttributeError` — só no `.pyz`, só no piso. A guarda tem de **executar o zip**, e
ela roda sob `python3.9` quando ele existe.

**Loopback não basta.** O servidor escuta só `127.0.0.1` **e confere o cabeçalho
`Host`** contra `127.0.0.1`, `localhost` e `[::1]`. Sem isso, uma página na
internet resolve um domínio para o loopback e fala com o painel (*DNS
rebinding*): a origem da requisição não protege nada nesse cenário. Caminhos
estáticos recusam `..` antes de tocar no disco.

Ele não instala no aparelho (isso é `aparelho.sh instalar-pacote`, por SSH) e não
guarda chave nenhuma: recebe o caminho de uma chave existente e repassa ao
`sign_package`.

### loja — Rust + SDL2, três sistemas

Resposta à pergunta que originou tudo — *"não quero que usem a chave para roubar
os apps"*: **uma chave pública não permite nada além de conferir assinaturas.**
O risco real nunca foi ela.

| Risco | Mitigação |
| --- | --- |
| chave **privada** vaza | `invalida_antes` no catálogo: o que foi assinado antes do corte não vale |
| **"autêntico" tratado como "comprado"** | licença separada, amarrada ao aparelho |
| catálogo trocado na rede | catálogo assinado; item sem assinatura não entra |
| servidor de pacote comprometido | sha256 + assinatura do `.giochi` |

Daí **três chaves e três perguntas**:

| Pergunta | Responde | Privada vive em |
| --- | --- | --- |
| "veio da Giochi?" | assinatura do pacote | servidor |
| "este aparelho pagou?" | **licença** (chave separada) | servidor |
| "é este aparelho?" | par gerado **no aparelho** | aparelho |

No código são **tipos diferentes**: `PacoteVerificado` prova origem e nada mais;
`LicencaAssinada` prova compra e exige a chave do aparelho. Confundir os dois é o
que faria um único pacote copiado servir para todos.

O que ela **não** faz, de propósito:

- **Não vê cartão.** Checkout hospedado no navegador (PCI-DSS SAQ A).
- **Não se declara paga.** `Estado::Pago` carrega `LicencaAssinada`, e a chave de
  licença não está no cliente. Não é disciplina, é o sistema de tipos — o teste
  aperta "Já paguei" dez vezes e exige que continue `Aguardando`.
- **Não inventa código PIX.** Sem provedor, a tela diz isso. Um "copia e cola"
  falso é a pior falha possível: o usuário tenta pagar.

**`butlerd` foi decidido não usar, com número:** é um daemon JSON-RPC de download
incremental para builds de dezenas de GB; um pacote de 81 KB custaria um processo
a supervisionar para economizar zero. **Regra para reconsiderar: pacotes acima de
~500 MB.** Antes disso, `curl -C -` resolve e já existe nos três sistemas.

**A mensagem assinada é montada em Rust (verifica) e em JS (assina).** Ela já
divergiu por um caractere (`item=<slug>|` contra `item|<slug>|`) e o catálogo
assinado **nunca** verificava, com a mensagem "assinatura inválida, não foi a
Giochi" mandando procurar no lugar errado. Hoje os dois lados têm o mesmo literal
com teste, e `giochi-loja --mensagem-catalogo` imprime os bytes.

**O layout é emprestado, não integrado.** Inspiração na forma de grade de cartões;
**zero** chamadas a serviços externos. A lista vem do catálogo da Giochi. E a
arquitetura de provedor é explícita: o provedor faz o checkout e **conta as
instalações** (`activation_limit` fica no servidor dele), o que resolve sem
backend nosso. Credencial de conta de usuário (API key) prova posse, não vínculo
com o aparelho — não serve para vender.

**Uma função decide a grade; desenho e navegação leem dela.**
`giochi_loja_core::grade::grade_para` + `mover`, com 11 testes incluindo um de
**propriedade** (nenhuma tecla produz índice fora da lista, em nenhuma grade, com
qualquer total). No launcher este projeto já teve o bug oposto — navegação
supondo 4 colunas com a tela calculando 2 — e o sintoma foi "a seta para baixo não
faz nada".

**Capas são desenhadas, porque o PNG só existe depois do download.** FNV-1a
(**não** `DefaultHasher`, que não é estável entre versões do compilador) → matiz,
matiz do detalhe e deslocamento da faixa. A cor **colidiu de verdade** (dois apps
na mesma matiz de 8 bits em 12 itens), então ela não é o único diferenciador e o
teste exige que o **conjunto** seja único. Um teste separado *documenta* a
colisão da matiz sozinha, para ninguém "consertar" achando que é bug.

## 10. O pacote `.giochi`

```text
app/<slug> ──empacotar──▶ <slug>.giochi (+.sig +.manifest.json)
            ──instalar-pacote──▶ /user/apps/<slug>
            ──catálogo──▶ cartão na tela ──▶ o app
```

**O `tar` é escrito à mão** (`scripts/lib/tar.mjs`) por três motivos medidos:

1. **Ordem** — o `tar` do sistema usa a ordem do `readdir`; aqui a ordem é
   contrato e a montagem é uma função só, testável.
2. **Reprodutibilidade** — `mtime`/`uid`/`gid` fixos. Dois empacotamentos do mesmo
   conteúdo dão **o mesmo SHA-256** (conferido). Se mudasse, a assinatura deixaria
   de identificar o conteúdo.
3. **Bit de execução** — `tar --mode` mascara permissão, e o erro só apareceria no
   aparelho como "entrypoint não existe".

Verificado contra o GNU tar: ele lê o ustar escrito à mão, extrai com as
permissões certas e `test -x` passa. Sem essa conferência seria o meu parser
concordando com o meu escritor.

**O extrator é script copiado, não `sh -c` aninhado.** A primeira versão montava o
comando com aspas dentro de aspas dentro de `ssh` e produziu
`sh: "bin/giochi-pacman": unknown operand`. Aninhar três níveis de aspas para
falar com outra máquina não é difícil de escrever — é difícil de **ler quando
falha**. Hoje é `scripts/lib/extrair-pacote.sh`, copiado e executado.

**App da loja vai para `/user/apps`, não para `/opt`.** `/opt/giochi/apps` é o
mesmo lugar dos apps **da imagem**, e gravar ali destrói o argumento que justifica
a loja existir: `distribution: store` significa "não faz parte da imagem
assinada", e é isso que permite vender app GPL.

### A assinatura tem um formato só, e é o do antecessor

| Guarda | Onde |
| --- | --- |
| `manifesto.signature` aponta o arquivo `.sig` | `assinar()` + teste JS |
| assinatura com tamanho diferente de 64 é recusada | `verificar()` + teste JS |
| o `.sig` tem 64 bytes e nenhum `\n` | `tests/painel/test_acoes.py` |

Os dois lados já divergiram: o Python gravava 64 bytes crus e o JS gravava base64
**em texto** (89 bytes com `\n`). Os testes JS assinavam e verificavam com a mesma
biblioteca, então passavam com qualquer formato. A prova que faltava é
**ferramenta externa**:

```text
openssl pkeyutl -verify -pubin -inkey chave-publica.pem \
  -rawin -in app.giochi -sigfile app.giochi.sig
Signature Verified Successfully     ← tanto o .sig do Python quanto o do JS
```

### A verificação no aparelho existe

`verificador-ed25519` (8.912 bytes, `EVP_DigestVerify`, ligado a `libcrypto.so.3`)
roda no aparelho: **20 MB de RSS, 0,01 s**. Ele confere o **tamanho** antes de
verificar (um `.sig` de 89 bytes é base64 gravado como texto, e o
`EVP_DigestVerify` recusaria com uma mensagem que não aponta a causa) e confere o
**tipo de chave** (uma chave RSA falharia na verificação e o veredito sairia
"assinatura não confere", culpando o pacote por erro de configuração).

`instalar-do-cartao.sh` recusa antes de extrair e **avisa** quando não tem
verificador ou chave. A primeira versão verificava em silêncio — o que torna
"verificou e passou" indistinguível de "não verificou". **Silêncio no sucesso é o
defeito espelhado do silêncio no fracasso.**

**Integridade não é autenticidade.** SHA-256 pega cópia incompleta — o erro real
de quem copia pelo leitor de cartão. Quem escreve no cartão reescreve o `.giochi`
**e** o manifesto, e o hash confere.
