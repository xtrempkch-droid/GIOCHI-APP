# Formato do pacote `.giochi`

O arquivo que a loja instala no aparelho. Formato herdado do antecessor
`GIOCHI-KONSOL-OS`.

## Ordem dos membros é obrigatória

Um arquivo `tar` **ustar**, com os membros nesta ordem:

```text
metadata.json  →  capa.png  →  thumb.png (opcional)  →  bin/
```

A ordem não é capricho: o extrator no aparelho lê em **streaming** e precisa de
`metadata.json` e `capa.png` **antes** de decidir se aceita o pacote. Se o binário
vier primeiro, o aparelho já baixou megabytes antes de saber que o app é
incompatível.

## Regras

| Item | Regra |
| --- | --- |
| Formato | `tar` ustar puro (sem gzip, sem extensões GNU) |
| `metadata.json` | ≤ **16 KiB**, validado contra [metadata.schema.json](../schemas/metadata.schema.json) |
| `capa.png` | **320×320**, obrigatória |
| `thumb.png` | opcional; miniatura |
| `bin/` | o que é instalado; o `entrypoint` do metadata aponta para dentro |
| `entrypoint` | precisa existir no pacote |

## O que **não** entra no pacote

- `src/` — código-fonte
- `assets/` — arte-fonte (logo em alta, `.kra`, `.psd`, fontes)
- `forge.json` — o manifesto de requisitos é consumido na **publicação**, pelo
  ferramental do Giochi app; não precisa viajar para o aparelho
- `README.md`

O pacote leva só o que o aparelho precisa executar. Um projeto Godot completo
dentro dele seria centenas de megabytes de arquivos editáveis.

## Assinatura

O antecessor usava **Ed25519 + SHA-256**, com dois arquivos ao lado do pacote:

```text
app.giochi
app.giochi.sig            # assinatura Ed25519
app.giochi.manifest.json  # {schema_version, package, version, sha256, size_bytes, signature}
```

O verificador no aparelho recusa o pacote se a assinatura não conferir.

## Estado real: o antecessor já implementa tudo isso

**Correção (02/11/2026).** A versão anterior deste documento dizia "nada disso está
implementado". Está errado: o `GIOCHI-KONSOL-OS` tem a cadeia completa, e ela roda
no CI dele. O que falta aqui é **migrar**, não desenhar.

Ferramentas que existem lá:

| Arquivo | O que faz |
| --- | --- |
| `tools/giochi-pack.sh` | monta o `.giochi` na ordem obrigatória |
| `tools/giochi-keys.sh` | gera par de chaves Ed25519 |
| `tools/giochi-sign.sh` | assina o pacote, gerando `.sig` e `.manifest.json` |
| `build/host/bin/giochi-verify` | verifica assinatura + SHA-256 |
| `build/host/bin/giochi-extract` | extrai no aparelho |

### O teste que prova que funciona

O job `package-roundtrip` do `ci.yml` deles faz o ciclo inteiro:

```text
pack → keys → sign → verify → extract
```

E depois **adultera de propósito**: copia o pacote, acrescenta um byte, e exige que
`giochi-verify` **recuse**. Se aceitar, o job falha com "imagem adulterada foi
aceita".

Esse teste é o modelo a copiar. Ele cobre o que a documentação não cobre: que o
verificador não é decorativo.

### Duas lições do teste deles

1. **Preservar o bit de execução.** O teste faz `test -x "$out/bin/ci"` no arquivo
   extraído. Se o empacotador perder a permissão, o app não abre — e o erro aparece
   só no aparelho, como "entrypoint não existe".
2. **Só o comprimido vai para o artefato.** Publicar `.img` cru (~920 MB) em vez do
   comprimido (~40–90 MB) estoura armazenamento sem ganho nenhum.

### Divergência de schema a resolver

O `metadata.json` dos testes deles inclui **`graphics_api`**:

```json
{"title":"CI","developer":"ci","version":"1.0.0","arch":"aarch64",
 "runtime":"godot","graphics_api":"gles3","entrypoint":"bin/ci"}
```

O nosso `metadata.schema.json` não tem esse campo — e como ele é
`additionalProperties: false`, um pacote herdado **seria reprovado** pelo nosso
validador. É exatamente o tipo de quebra silenciosa que a migração vai encontrar:
decidir se `graphics_api` entra no metadata ou se fica só no `forge.json`.

## O que falta para o pacote existir

1. **Export ARM64** dos projetos Godot (`src/` → `bin/`). Continua pendente: os
   jogos Godot seguem sem `bin/`.
2. ~~Migrar empacotador e assinador~~ — **feito em 06/11/2026**, em Node, sem
   dependência externa. Ver abaixo.
3. ~~Portar o teste de ida e volta~~ — **feito**: `tests/pacote.test.mjs`,
   incluindo a adulteração.

---

## Implementado em 06/11/2026

```bash
node scripts/empacotar.mjs app/jogos/pacman          # gera + assina
node scripts/verificar-pacote.mjs dist/pacotes/pacman.giochi
bash scripts/aparelho.sh instalar-pacote dist/pacotes/pacman.giochi
```

| Arquivo | Papel |
| --- | --- |
| `scripts/lib/tar.mjs` | escreve e lê o ustar à mão |
| `scripts/lib/publicar.mjs` | valida, ordena, assina, verifica, confere estrutura |
| `scripts/empacotar.mjs` | CLI: pasta do app → `.giochi` + `.sig` + `.manifest.json` |
| `scripts/verificar-pacote.mjs` | CLI: estrutura + SHA-256 + Ed25519 |
| `scripts/lib/extrair-pacote.sh` | extrator que roda **no aparelho** |
| `scripts/aparelho.sh instalar-pacote` | verifica, envia, extrai, republica o catálogo |

### Por que o `tar` é escrito à mão

Três razões, todas de comportamento observável:

1. **Ordem.** O `tar` do sistema escreve na ordem do `readdir`. A ordem aqui é
   contrato, e a montagem é uma função só (`ordenarMembros`), testável.
2. **Reprodutibilidade.** `mtime`, `uid` e `gid` fixos. Dois empacotamentos do
   mesmo conteúdo dão o **mesmo SHA-256** — conferido: `0dddbe2b…` duas vezes.
   Se o hash mudasse, a assinatura deixaria de identificar o conteúdo.
3. **Bit de execução.** `tar --mode` mascara permissão; o extrator precisa de
   `bin/app` executável, e o erro só aparece no aparelho como "entrypoint não
   existe".

O formato foi verificado contra o `tar` do sistema: GNU tar lê o ustar escrito à
mão, extrai com as permissões certas e `test -x` passa.

### `slug`: a identidade não vem do título

`metadata.json` ganhou `slug` (opcional, kebab-case). Sem ele a loja derivaria o
diretório de instalação do **título** — e renomear "Pac-Man" para "Pac-Man DX"
moveria o app de lugar, quebrando o catálogo e o favorito do usuário.

Ordem: `metadata.slug` → `id` do `apps.json` → derivação do título (último
recurso, e o pacote **diz** que usou).

### O catálogo passou a incluir o que veio da loja

Um pacote `.giochi` não está no `apps.json`: ele chega pelo cartão e é extraído
em `/opt/giochi/apps/<slug>/`. `aparelho.sh catalogo` agora **pergunta ao
aparelho** o que está instalado (`colher_instalados`) e o gerador junta isso ao
catálogo do repositório, com o disco vencendo.

Sem essa junção, instalar pela loja deixaria o jogo **invisível sem erro** — o
mesmo modo de falha do `index.json` escrito à mão, que mostrava 1 jogo com 3
instalados.

Achado durante a implementação: substituir a entrada inteira apagava a
`license`, que só existe no `apps.json`. O pacman apareceu no catálogo do
aparelho sem licença. Agora a entrada do repositório é herdada nos campos que o
pacote não traz.

### O que foi medido no X55

```text
instalar-pacote dist/pacotes/pacman.giochi
  verificado: sha256 c52a181b…, assinatura ok
  extraído: bin/giochi-pacman
  catálogo republicado: 3 app(s)

--lancar pacman
  287  giochi-launcher --lancar pacman      ← pai
  300  giochi-pacman                        ← filho, iniciado pelo launcher
  quadro depois da volta: 346 cores, 0,00% de preto
```

Ciclo completo: **empacotar → assinar → verificar → enviar → extrair → catalogar
→ lançar → voltar**.

### Onde o pacote é instalado

| Origem | Destino | Por quê |
| --- | --- | --- |
| imagem assinada | `/opt/giochi/apps/<slug>` | faz parte do sistema |
| **loja** (`instalar-pacote`) | **`/user/apps/<slug>`** | fora da imagem, substituível |

Não é organização de pastas. `distribution: store` significa "não faz parte da
imagem assinada", e é isso que permite vender app **GPL**: o comprador pode
substituir o pacote em `/user`, e a GPL-3.0 §6 se resolve sem entregar chave.

A primeira versão instalava tudo em `/opt/giochi/apps` — funcionava, e destruía
esse argumento. Corrigido e medido:
`957 ppid=949 /user/apps/pacman/bin/giochi-pacman`.

**Ressalva:** nesta imagem `/user` ainda é diretório no rootfs, não partição
separada (só `/dev/root` em `/`). O caminho está certo; a partição de dados
depende do Giochi System.

### Ainda não feito

| Item | Estado |
| --- | --- |
| verificação de **assinatura** no aparelho | **sim**, pelo cartão — binário aarch64 ligado a `libcrypto`, exercitado no X55 |
| chave pública em `/etc/giochi/` | **não** — falta na imagem; sem ela o instalador avisa que não verificou |
| verificação de **SHA-256** no aparelho | **sim, pelo cartão** — `instalar-do-cartao.sh` recusa hash que não confere |
| `.giochi` lido de pendrive/SD pelo launcher | **não** — hoje só por `aparelho.sh instalar-pacote` |
| loja (download, OAuth, butlerd) | **não** — ver [`loja.md`](loja.md) |
| app Godot empacotado | **não** — falta export ARM64 |

#### O cartão SD muda o modelo de confiança

"O aparelho confia no cabo" era aceitável enquanto o único caminho era
`aparelho.sh instalar-pacote`: quem envia, controla. Com jogos em **cartão SD**
(que é o plano para dois slots + eMMC) o arquivo chega ao aparelho **sem passar
pelo PC**, e aí confiar sem verificar deixa de ser conveniência e vira risco.

**O caminho do cartão já verifica integridade.** `scripts/lib/instalar-do-cartao.sh`
compara o SHA-256 do pacote com o do manifesto ao lado (`sha256sum`, que vem do
busybox) e **recusa** o arquivo quando não confere. Sem manifesto ao lado, ele
avisa e segue — o `.giochi` sozinho é o formato documentado.

**O caminho do SSH não verifica no aparelho**, e não precisa: o PC roda
`verificar-pacote.mjs` (estrutura + SHA-256 + Ed25519) **antes** de enviar, e
aborta se o pacote for recusado. Quem envia é quem verifica.

O que nenhum dos dois faz é **autenticidade**: a assinatura Ed25519 não é
conferida no aparelho. E isso é uma limitação medida, não uma suposição.

#### O que existe na imagem (medido com `debugfs`, 06/11/2026)

```text
busybox sha256sum   /usr/bin/sha256sum → ../../bin/busybox   presente
busybox awk         necessário pelo instalar-do-cartao         presente
openssl (CLI)       /usr/bin/openssl, /bin/openssl             AUSENTE
python3                                                        AUSENTE
libcrypto           /usr/lib/libcrypto.so.3                    presente
```

As duas imagens (`powkiddy-x55`, `powkiddy-x35h`, 04/11 20:32 e 18:14) dão o
mesmo resultado. Ou seja: **o hash dá para conferir hoje**; a **assinatura não**,
porque verificar Ed25519 sem `openssl` exige um binário — `libcrypto` está lá,
mas é biblioteca, não programa.

#### Integridade não é autenticidade

Quem tem acesso de escrita ao cartão reescreve o `.giochi` **e** o manifesto, e o
SHA-256 confere. O que a verificação pega é **cópia incompleta ou corrompida**,
que é o erro real de quem copia pelo leitor. Isso está dito no cabeçalho do
`instalar-do-cartao.sh` desde que ele foi escrito, e continua valendo.

#### O verificador existe e foi exercitado no X55

`scripts/lib/verificador-ed25519.c` + `scripts/compilar-verificador-arm64.sh`.

```bash
bash scripts/compilar-verificador-arm64.sh              # gera dist/arm64/verificador-ed25519
bash scripts/compilar-verificador-arm64.sh --instalar   # compila e instala no aparelho
```

É um binário de **8.912 bytes** ligado a `libcrypto` e `libc.so.6` — as duas que
o aparelho tem. Usa `EVP_DigestVerify` de uma vez, que é o que Ed25519 pede
(assina a mensagem, não um hash dela), e consome os 64 bytes crus do `.sig` sem
conversão nenhuma.

**Medido no X55, não deduzido:**

| Caso | Saída | Código |
| --- | --- | --- |
| pacote bom, chave certa | `OK assinatura confere (81408 bytes)` | 0 |
| um byte a mais no pacote | `RECUSADO assinatura não confere` | 1 |
| chave de outra pessoa | `RECUSADO assinatura não confere` | 1 |
| chave RSA no lugar de Ed25519 | `ERRO chave não é Ed25519 (tipo 6)` | 3 |
| `.sig` em base64 (89 bytes) | `ERRO assinatura com 89 bytes; Ed25519 tem 64` | 1 |
| arquivo inexistente | `ERRO pacote: não consegui abrir` | 2 |

Custo no aparelho: **20 MB de RSS, 0,01 s**. Quase tudo é o `libcrypto`, que fica
na página e é liberado no fim.

O código 3 existe porque uma chave RSA falharia na verificação e o veredito
sairia "assinatura não confere" — culpando o pacote por um erro de configuração.
O usuário trocaria o jogo em vez da chave.

#### O instalador do cartão usa, e diz quando não pode

Com verificador **e** chave em `/etc/giochi/loja-publica.pem`, o
`instalar-do-cartao.sh` recusa antes de extrair:

```text
arcade-volleyball.giochi: sha256 confere
arcade-volleyball.giochi: assinatura confere
arcade-volleyball.giochi: instalado em /user/apps/arcade-volleyball
RESUMO 1 instalado(s), 0 recusado(s), 1 encontrado(s)
```

As duas verificações **falam** no sucesso. A primeira versão verificava em
silêncio, e `OK` sem mensagem é indistinguível de "não verifiquei" — defeito
espelhado do silêncio no fracasso.

Pacote adulterado, no aparelho:

```text
arcade-volleyball.giochi: RECUSADO — assinatura não confere
RESUMO 0 instalado(s), 1 recusado(s), 1 encontrado(s)
```

Sem verificador ou sem chave, ele **avisa** — não passa calado:

```text
teste.giochi: aviso — sem verificador, assinatura não verificada
teste.giochi: aviso — sem chave em /etc/giochi/loja-publica.pem, assinatura não verificada
```

**Silêncio no sucesso era um defeito, nas duas verificações.** Ambas passaram a
imprimir quando conferem.

#### O que continua faltando

**A chave pública não está na imagem.** Medido com `debugfs`: `/etc/giochi` tem
só `buttons.map`. Sem `loja-publica.pem` na imagem, o instalador avisa e segue
sem verificar — que é o comportamento correto, mas não é segurança. Colocar a
chave lá é decisão do **Giochi System**, e é o que transforma o verificador em
garantia em vez de ferramenta.
