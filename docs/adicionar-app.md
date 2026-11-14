# Adicionar um app

## 1. Copie o template

```bash
cp -r app/_template app/meu-app
```

O nome da pasta é o `id` do app: minúsculas, números e hífen. `Meu App` e
`meu_app` são rejeitados pelo validador.

**Onde criar:** solto em `app/<slug>`. A varredura também aceita um nível de
grupo (`app/<grupo>/<slug>`) — o que faz da pasta um app é o `metadata.json`, não
uma lista de grupos escrita à mão. Se usar grupo, registre com o mesmo caminho:

```json
{ "id": "meu-app", "folder": "ferramentas/meu-app", "runtime": "native", "status": "scaffold" }
```

Grupo não precisa — e não deve — ter `metadata.json` próprio: pasta com o arquivo
é app, e um grupo que se declara app vira um app quebrado.

## 2. Preencha o `metadata.json`

```json
{
  "title": "Meu App",
  "developer": "Seu Nome",
  "version": "1.0.0",
  "arch": "aarch64",
  "runtime": "native",
  "entrypoint": "bin/meu-app",
  "releaseYear": 2026,
  "category": "Ferramenta"
}
```

Campos obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`,
`entrypoint`. Nenhum campo fora do schema é aceito — ver
[metadata.md](metadata.md).

`releaseYear` e `category` são opcionais, mas **coloque os dois**: são o ano e o
gênero que aparecem no cartão. Sem o ano a UI mostra `—`; sem o gênero, mostra
"Aplicativo". O `developer` daqui é o mesmo fabricante que a ficha exibe.

Depois de instalar, publique o catálogo — senão o app existe e não aparece:

```bash
bash scripts/aparelho.sh catalogo
```

## 3. Coloque o executável em `bin/`

`entrypoint` precisa apontar para um arquivo que **existe** dentro de `bin/`.

```bash
mkdir -p app/meu-app/bin
chmod +x app/meu-app/bin/meu-app
```

O validador confere que o arquivo existe. Ele **não** confere se o binário roda no
aparelho — isso só se descobre no hardware.

## 4. Declare os requisitos em `forge.json`

Opcional, mas recomendado: sem ele o sistema assume os defaults (`Godot`, GLES 3,
0 MB de RAM) e pode liberar o app num aparelho que não aguenta.

```json
{
  "runtime": "native",
  "graphics_api_min": "none",
  "ram_min_mb": 32,
  "display_min_width": 640,
  "display_min_height": 480
}
```

`graphics_api_min: "none"` é para app que desenha em software (SDL2 puro). O
default continua `gles3`: omitir num app gráfico é conservador, `none` é
declaração.

Ver [forge.md](forge.md).

## 5. Adicione a capa

`capa.png` na raiz do app, **exatamente 320×320**. O validador lê o cabeçalho do
PNG e compara; imagem de tamanho errado reprova.

Corrija o tamanho antes de commitar:

```bash
# ImageMagick
magick logo-alta.png -resize 320x320! app/meu-app/capa.png
```

O logo em alta resolução fica em `assets/`, que **não** entra no pacote.

## 6. Escreva o `README.md` do app

Obrigatório na prática: é o que alguém lê daqui a seis meses. Diga o que o app é,
como rodar e o que ainda falta.

## 7. Valide e commite

```bash
node scripts/validate-app.mjs app/meu-app
node tests/run-all.mjs
```

Saída esperada: `OK: 1 app(s) válido(s).` e `23/23` nos testes.

## Erros comuns

| Código | Causa |
| --- | --- |
| `versao_invalida` | `version: "1.0"` — falta o patch, use `1.0.0` |
| `entrypoint_ausente` | `entrypoint` aponta para arquivo que você esqueceu de criar |
| `entrypoint_fora_de_bin` | caminho começando fora de `bin/` |
| `capa_dimensao` | PNG não é 320×320 |
| `campo_desconhecido` | chave inventada no `metadata.json` |
| `nome_invalido` | pasta com maiúscula, espaço ou underscore |
| `bin_ausente` | faltou a pasta `bin/` |
