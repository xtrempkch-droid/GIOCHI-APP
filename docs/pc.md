# Apps de PC: a loja, o forge e o painel nos três sistemas

> Medido em 06/11/2026. **Linux: executado.** Windows e macOS: o workflow está
> escrito para executar os dois, mas **ainda não rodou no GitHub** — ver
> [o que está provado](#o-que-está-provado-e-o-que-não-está).

Os três apps com `distribution: "host"` são a **loja** (`app/store`), o
**forge** (`app/forge`) e o **painel** (`app/painel`, interface web local). Eles
rodam no computador do usuário, não no console:

```text
PC do usuário                                  aparelho
─────────────                                  ────────
giochi-store  ── baixa/paga ──►  .giochi  ──►  /user/apps/<slug>/
giochi-forge  ── classifica e empacota ──► .giochi
```

Nenhum dos dois vai para a imagem do aparelho, e o catálogo do console os exclui
por isso mesmo:

```text
$ node scripts/gerar-catalogo.mjs --lista
  forge   distribution host: roda no PC, não vai para o aparelho
  store   distribution host: roda no PC, não vai para o aparelho
```

## O que existe hoje

| Peça | Onde | Estado |
| --- | --- | --- |
| `giochi-store` / `giochi-forge` | `app/*/bin/` | lançador POSIX: roda o módulo de `../src` |
| `giochi-painel` | `app/painel/bin/` | idem; abre a interface web em `127.0.0.1:8710` |
| `.pyz` (um arquivo, três sistemas) | `dist/pc/` | gerado por `scripts/empacotar-pc.mjs` |
| `.sh` e `.cmd` | `dist/pc/` | atalhos por sistema |
| `SHA256SUMS` | `dist/pc/` | o que o usuário confere antes de rodar |
| Workflow matriz | `.github/workflows/pc-apps.yml` | empacota uma vez, **executa** nos três |

## Por que `.pyz` e não PyInstaller

Os dois apps usam **só a biblioteca padrão** — nenhuma dependência de terceiro,
por decisão de projeto: a loja tem de funcionar numa máquina limpa, e um
`pip install` que falha atrás de proxy é o primeiro suporte que ninguém quer dar.

Isso muda a conta:

| | `.pyz` (zipapp) | PyInstaller |
| --- | --- | --- |
| artefatos | **um**, idêntico nos três sistemas | um por sistema **e** por arquitetura |
| precisa compilar no sistema | não | sim (matriz obrigatória) |
| determinístico | **sim**, com ZIP próprio | não |
| tamanho | ~14 KB | ~10 MB |
| precisa de Python instalado | **sim** (3.9+) | não |

A troca é explícita e está no produto: **quem tem Python roda um arquivo só;
quem não tem precisa do binário nativo.** Se a loja for distribuída para um
público sem Python, o passo seguinte é PyInstaller — o workflow já tem a matriz
pronta, e é uma etapa a mais no job `empacotar`.

## Como gerar

```bash
node scripts/empacotar-pc.mjs              # os dois apps em dist/pc/
node scripts/empacotar-pc.mjs --app store  # só um
```

```text
giochi-store.pyz
  tamanho:  14032 bytes
  sha256:   90315c6bcc7517a45d91c1b021c55f29be71d6c9552383639656db0deb4968fd
giochi-forge.pyz
  tamanho:  13704 bytes
  sha256:   12a86742dc3cbdc82746b24d707a28e5e37fe2866b67547409c72600a50f435b
```

Rodar duas vezes dá o **mesmo SHA-256**. Isso não é detalhe: o checksum publicado
identifica o conteúdo, e o `zipapp` oficial do Python não serve aqui porque grava
o `mtime` de cada arquivo de origem. O ZIP é escrito à mão em
`scripts/lib/zip.mjs`, com data fixa e ordem por caminho — é a mesma razão pela
qual o `.giochi` usa um `tar` escrito à mão.

## Como usar em cada sistema

### Linux e macOS

```bash
python3 giochi-forge.pyz --version
./giochi-forge.sh --version          # o atalho faz o mesmo, com o Python do PATH
```

Do repositório clonado, sem empacotar nada:

```bash
app/forge/bin/giochi-forge --version
```

### Windows

```bat
py -3 giochi-store.pyz --help
giochi-store.cmd --help
```

O `.cmd` tenta `py` (o lançador oficial) e depois `python`, e diz o que instalar
se nenhum dos dois existir — um duplo-clique que abre e fecha sem texto é o pior
resultado possível. **Ele não executa `sh`**, e por isso o `bin/` do repositório
não serve no Windows: ali o caminho é o artefato da release.

### Conferir antes de rodar

```bash
sha256sum -c SHA256SUMS           # Linux
shasum -a 256 -c SHA256SUMS       # macOS
certutil -hashfile giochi-store.pyz SHA256   # Windows
```

## O workflow

`.github/workflows/pc-apps.yml`, dois jobs em série e um de publicação:

```text
empacotar (ubuntu)            executar (matriz)                  publicar
──────────────────            ──────────────────                  ────────
gera dist/pc/*.pyz            linux       Python 3.12             em tag de
confere determinismo          linux-3.9   Python 3.9 (o piso)     release,
publica o artefato            windows     Python 3.12             anexa tudo
                              macos       Python 3.12 (arm64)
```

**Por que um pacote e três execuções**, e não três pacotes: o `.pyz` é idêntico
byte a byte em qualquer sistema, então empacotar três vezes só produziria três
hashes para o mesmo código. O que muda de sistema para sistema é o que o Python
**faz** com ele — `pathlib` com `\`, `os.replace` entre volumes, CRLF no `print`,
permissão de execução no macOS. Isso não aparece num job que só roda
`ubuntu-latest`, então a matriz **executa**:

1. confere o `SHA256SUMS` publicado (o mesmo comando que o usuário faria);
2. abre os dois apps com `--help` — prova que o `zipimport` resolveu tudo;
3. o forge empacota o `arcade-volleyball` de verdade;
4. o store lê o pacote que o forge acabou de fazer (um app consome o outro);
5. o store gera `store.json`;
6. roda o atalho do sistema (`.sh` no Unix, `.cmd` no Windows).

O passo 4 é o que pegou o defeito de 06/11/2026: o forge e o store tinham **duas
cópias** da lista de `runtime`, e as duas divergiam do schema. Ver
[`docs/diario/2026-11-06.md`](diario/2026-11-06.md).

## O painel: interface web local

A loja e o forge são CLI, e conferir à mão o que eles respondem — num terminal,
com `--json`, lendo código de saída — não é o que um usuário faz. O painel é uma
página servida por `http.server`, sem framework e sem dependência de terceiro.

```bash
app/painel/bin/giochi-painel            # http://127.0.0.1:8710
app/painel/bin/giochi-painel --porta 9000 --sem-navegador
python3 dist/pc/giochi-painel.pyz       # a versão empacotada, igual
```

**O painel não reimplementa o forge nem o store**: ele chama. Classificar é
`giochi_forge.classify`, empacotar é `giochi_forge.package`, listar pacotes é
`giochi_store.catalogo`. Uma segunda cópia da regra de compatibilidade
envelheceria diferente da primeira — e a primeira é a que decide o que o usuário
pode comprar.

### Três decisões que vieram de erro real

1. **`importlib.resources`, não `Path(__file__)`.** Dentro de um `.pyz` não
   existe diretório ao lado do módulo: `__file__` aponta para dentro do zip e
   `Path.exists()` devolve `False`. Com `Path`, o painel empacotado serve **404
   na própria página** — e só depois de distribuído alguém descobriria.
2. **O `.pyz` do painel leva três pacotes.** Num zip não há `PYTHONPATH`, então
   `giochi_painel`, `giochi_forge` e `giochi_store` têm de estar dentro dele. É o
   campo `extras` de `ALVOS` em `scripts/empacotar-pc.mjs`, e dois testes
   reprovam se um dos três faltar — um lê o índice do zip, o outro **executa**.
3. **Só `127.0.0.1`, com o `Host` conferido.** Sem essa checagem, uma página
   qualquer resolve um domínio para o loopback e fala com o painel (*DNS
   rebinding*). Caminhos estáticos recusam `..` antes de tocar no disco.

### O que o painel não faz

- **Não instala no aparelho.** Enviar o pacote é `aparelho.sh instalar-pacote`,
  por SSH. O painel para no `.giochi` assinado.
- **Não guarda chave.** Recebe o caminho de uma chave existente e repassa ao
  `sign_package`. A privada continua sendo um arquivo do usuário.

### O filtro de `paths` faz parte do teste

O workflow só roda quando um caminho listado muda. `app/painel/**` ficou de fora
na primeira versão e a correção do 3.9 **não rodou** — o workflow que existe para
provar os apps de PC não era acionado por um deles. Um app fora do filtro é um
app sem CI, e o sintoma é silêncio, não falha.

## O que está provado, e o que não está

| Afirmação | Como foi verificado |
| --- | --- |
| os três `.pyz` rodam | **executado**, Linux, `python3` 3.14 |
| o painel serve página, estáticos e API | **executado**: `/`, `/app.js`, `/estilo.css`, `/api/aparelhos` |
| o painel recusa `Host` estrangeiro e `..` | **executado**: 403 e 404 |
| um `.giochi` assinado pelo painel verifica | **executado**: `openssl pkeyutl -verify` aceita |
| o forge empacota um jogo | **executado**: `av.giochi`, 81920 bytes, sha256 conferido |
| o store lê e cataloga | **executado**: `1 de 1 pacote(s) no catalogo` |
| o `.sh` funciona | **executado** |
| o empacotamento é reprodutível | **executado**: mesmo SHA-256 em duas rodadas |
| sintaxe compatível com 3.9 | **verificado**: `ast.parse(feature_version=(3, 9))` nos 12 arquivos |
| **roda no Windows** | **não verificado** — o workflow executa, mas ainda não rodou no GitHub |
| **roda no macOS** | **não verificado** — idem |
| **roda em Python 3.9 de verdade** | **executado** em 3.9.25: três `.pyz`, passo HTTP e 32 testes |

As três últimas são a matriz fazendo o seu trabalho na primeira execução. Até
lá, o honesto é: **Linux está provado, Windows e macOS estão preparados.**

## Pendente

- **Assinatura de código no macOS** (notarização) e no Windows (Authenticode) só
  fazem falta quando existir binário nativo — um `.pyz` é dado, não executável.
- **PyInstaller**, se a loja for distribuída a quem não tem Python.
- **Instalar no aparelho pelo painel** (hoje termina no `.giochi` assinado).
- **Assinatura de código** só faz falta com binário nativo; um `.pyz` é dado.
