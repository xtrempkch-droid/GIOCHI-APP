# Giochi Painel

Interface web **local** do computador: mostra os aparelhos reais, diz se um app
roda neles, empacota e assina o `.giochi`.

Existe porque as duas ferramentas de PC (`forge` e `store`) são de linha de
comando, e conferir à mão o que elas respondem — num terminal, com `--json`,
lendo código de saída — não é o que um usuário faz. O painel não reimplementa
nada: ele **chama** o forge e o store.

```bash
app/painel/bin/giochi-painel          # abre em http://127.0.0.1:8710
app/painel/bin/giochi-painel --porta 9000 --sem-navegador
```

## O que ele faz

| Ação | Quem executa |
| --- | --- |
| listar aparelhos e o display utilizável | `giochi_forge.profile` |
| classificar um app num aparelho | `giochi_forge.classify` |
| empacotar + assinar `.giochi` | `giochi_forge.package` |
| listar pacotes de uma pasta | `giochi_store.catalogo` |
| achar o console conectado | `giochi_store.destino` |

## Por que só em `127.0.0.1`

O servidor escuta apenas no loopback e **confere o cabeçalho `Host`** contra
`127.0.0.1`, `localhost` e `[::1]`. Sem essa checagem, uma página qualquer na
internet poderia resolver um domínio para `127.0.0.1` e falar com o painel
(*DNS rebinding*) — a origem da requisição não protege nada nesse cenário.

Caminhos estáticos recusam `..` antes de tocar no disco. Foi testado com
`/%2e%2e/%2e%2e/etc/passwd` e `/..%2f..%2fetc%2fpasswd`: 404 nos dois.

## Estrutura

```text
src/giochi_painel/
├── acoes.py      # o que acontece (sem HTTP) — é o que os testes cobrem
├── servidor.py   # http.server, Host, estáticos, rotas
├── __main__.py   # argparse, --porta, --sem-navegador
└── estatico/     # index.html, estilo.css, app.js — sem framework
```

A separação `acoes` × `servidor` é de propósito: o que pode estar errado é
**o que acontece** (classificar, empacotar), não o roteamento. Os testes de
`acoes` rodam sem abrir socket; os do servidor sobem um de verdade em porta
efêmera.

## Os estáticos vêm por `importlib.resources`

Não por `Path(__file__)`. No `.pyz` (pacote de PC, um arquivo só) não existe
diretório ao lado do módulo: `__file__` aponta para dentro do zip e
`Path.exists()` devolve `False`. Com `Path`, o painel empacotado serviria **404
na própria página** — e só depois de distribuído alguém descobriria.

## Empacotamento

```bash
node scripts/empacotar-pc.mjs --app painel --saida dist/pc
python3 dist/pc/giochi-painel.pyz
```

O `.pyz` do painel leva **três** pacotes: `giochi_painel`, `giochi_forge` e
`giochi_store`. Num zip não há `PYTHONPATH`, então os três têm de estar dentro
dele — é o que `extras` declara em `scripts/empacotar-pc.mjs`.

## Testes

```bash
bash scripts/testar-apps-pc.sh --painel
```

32 testes. Eles cobrem, entre outras coisas, duas regressões de 06/11/2026 que
só apareceram quando o painel chamou o forge de verdade:

- `sign_package` exige `version` (keyword-only). Sem ela o pacote saía **sem
  manifesto**, e o erro era um `TypeError` virando 500 no navegador;
- o `.sig` tem de ter **64 bytes crus** e o manifesto tem de **apontar o
  arquivo**, não carregar base64 — senão o `openssl pkeyutl -verify -rawin
  -sigfile` do outro lado recusa o pacote.
