# Adicionar um app

## 1. Copie o template

```bash
cp -r app/_template app/meu-app
```

O nome da pasta é o `slug` do app: minúsculas, números e hífen. `Meu App` e
`meu_app` são rejeitados pelo validador.

## 2. Preencha o `metadata.json`

```json
{
  "title": "Meu App",
  "developer": "Seu Nome",
  "version": "1.0.0",
  "arch": "aarch64",
  "runtime": "godot",
  "entrypoint": "bin/meu-app"
}
```

Campos obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`,
`entrypoint`. Nenhum campo fora do schema é aceito — ver
[metadata.md](metadata.md).

## 3. Coloque o executável em `bin/`

`entrypoint` precisa apontar para um arquivo que **existe** dentro de `bin/`.

```bash
mkdir -p app/meu-app/bin
chmod +x app/meu-app/bin/meu-app
```

O validador confere que o arquivo existe. Ele **não** confere se o binário roda no
aparelho — isso só se descobre no hardware.

## 4. Declare os requisitos em `forge.json`

Opcional, mas recomendado: sem ele o sistema assume os defaults (`godot`, GLES 3,
0 MB de RAM) e pode liberar o app num aparelho que não aguenta.

```json
{
  "runtime": "godot",
  "graphics_api_min": "gles3",
  "ram_min_mb": 512,
  "display_min_width": 640,
  "display_min_height": 480
}
```

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
