# Giochi app

Aplicativos e jogos do projeto Giochi.

Este repositório é a **camada de experiência**: os apps que rodam sobre o sistema
gerado pelo **Giochi System** e que são reunidos com ele no **Giochi os**.

| Repositório | Função |
| --- | --- |
| **Giochi app** (este) | Apps, jogos, arte e o manifesto de requisitos de cada um |
| **Giochi System** | Cria o sistema Linux limpo: kernel, rootfs, boot, drivers, painel |
| **Giochi os** | Junta o sistema com os apps e gera a imagem final por aparelho |

## Estrutura

```text
Giochi app/
├── app/                      # cada app em sua própria pasta
│   ├── _template/            # modelo para app novo
│   ├── forge/                # empacotador/classificador (native, Python)
│   ├── store/                # loja (native, Python)
│   ├── arcade-volleyball/    # jogo (godot)
│   ├── colossal-cave-adventure/
│   └── spacewar/
├── schemas/                  # JSON Schema do metadata.json e do forge.json
├── scripts/                  # validador e migração
├── tests/
└── docs/
```

Cada app tem a mesma anatomia:

```text
app/<slug>/
├── metadata.json     # identidade do pacote (obrigatório)
├── forge.json        # requisitos de hardware (opcional)
├── capa.png          # 320x320, capa da loja
├── thumb.png         # miniatura (opcional)
├── src/              # código-fonte: projeto Godot, Python, C++
├── bin/              # o que é instalado no aparelho
├── assets/           # logos, ícones, fontes, arte-fonte
└── README.md         # o que o app é
```

**`src/` não é `bin/`.** `src/` é o projeto que você edita; `bin/` é o resultado
do build. Um projeto Godot mora em `src/`; o export ARM64 dele vai para `bin/`.
Enquanto não houver export, `bin/` fica vazio — e isso está correto.

## Criar um app

```bash
cp -r app/_template app/meu-app
# edite app/meu-app/metadata.json, forge.json e o README.md
mkdir -p app/meu-app/bin
# coloque o executável em app/meu-app/bin/
```

Depois valide:

```bash
node scripts/validate-app.mjs app/meu-app
```

## O que o validador cobra

- `version` em semver — `1.0.0`, nunca `1.0`.
- `runtime` só aceita `godot`, `retroarch` ou `proton`.
- `arch` só aceita `aarch64`.
- `entrypoint` começa em `bin/`, não é absoluto e não contém `..`.
- `entrypoint` aponta para arquivo que **existe**.
- `capa.png`, quando presente, tem exatamente **320×320**.
- `metadata.json` não aceita campo fora do schema.

## Comandos

```bash
node scripts/validate-app.mjs app/meu-app    # um app
node scripts/validate-app.mjs --all          # todos
node tests/run-all.mjs                       # 23 testes do validador
```

## `assets/` não vai para o aparelho

`assets/` guarda o material de trabalho — logo em alta resolução, `.kra`, `.psd`,
fontes. **Não entra no pacote.** O que o app precisa em tempo de execução vai em
`bin/`.

## Contrato com o Giochi System

Cada app declara o que precisa em `forge.json`: arquitetura, runtime, API gráfica,
memória e resolução mínimas. **Não** declara pacotes Buildroot — quem sabe o que a
imagem tem é o `Giochi System`.

O sistema compara esse manifesto com o perfil do aparelho e responde
`Compatível`, `Parcial` ou `Incompatível`. Detalhes em
[docs/forge.md](docs/forge.md).

## Onde cada app vai parar

O campo `distribution` em `app/apps.json` decide o destino, e cada um tem
obrigações diferentes de licença:

| `distribution` | Onde roda | Exemplo |
| --- | --- | --- |
| `image` | dentro da imagem assinada do aparelho | jogos padrão |
| `store` | instalado em `/user` no aparelho, enviado pela loja | Shattered Pixel Dungeon |
| `host` | no **computador do usuário** — não vai para o aparelho | loja, forge |

A loja roda no PC e envia o pacote pronto para o aparelho. Por isso um app GPL
**pode** ser vendido: ele não entra na imagem assinada, e o comprador pode
substituí-lo em `/user`. Ver [docs/licenciamento.md](docs/licenciamento.md).

## Licença

MIT — ver [LICENSE](LICENSE).
