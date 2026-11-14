# Giochi app

Aplicativos do sistema Giochi. **Os jogos moram em outro repositório** desde
14/11/2026.

Este repositório é a **camada de experiência**: os apps que rodam sobre o sistema
gerado pelo **Giochi System** e que são reunidos com ele no **Giochi os**.

| Repositório | Função |
| --- | --- |
| **Giochi app** (este) | Apps do sistema, ferramentas de PC, arte e o manifesto de requisitos |
| **Giochi System** | Cria o sistema Linux limpo: kernel, rootfs, boot, drivers, painel |
| **Giochi os** | Junta o sistema com os apps e gera a imagem final por aparelho |

## Estrutura

```text
Giochi app/
├── app/                      # cada app em sua própria pasta
│   ├── _template/            # modelo para app novo
│   ├── launcher/             # interface do aparelho (Rust + SDL2)
│   ├── launcher-teste/       # o mesmo launcher desenhado sobre a GIOCHI-ENGINE
│   ├── launcher-star-dust/   # o launcher + camada de efeitos
│   ├── loja/                 # loja do PC (Rust + SDL2)
│   ├── painel/               # interface web local do PC
│   ├── forge/                # empacotador/classificador (native, Python)
│   └── store/                # CLI da loja (native, Python)
├── schemas/                  # JSON Schema do metadata.json e do forge.json
├── scripts/                  # validador, empacotador e toolkit do aparelho
├── tests/
└── docs/
```

O que existe hoje, medido em 14/11/2026:

| Item | Estado |
| --- | --- |
| apps em `apps.json` | **7**: `launcher`, `launcher-teste`, `launcher-star-dust` (image, shell) + `forge`, `store`, `painel`, `loja` (host) |
| `validate-app.mjs --all` | **7 válidos** (1 em `scaffold`) |
| `tests/run-all.mjs` | **199/199** JS |
| `testar-apps-pc.sh` | **90** Python (58 forge + 32 painel) |
| `compilar-launcher.sh --testes` | **241/241** Rust (134 ui-sdl + 107 core) |
| `compilar-loja.sh --testes` | **116/116** Rust |
| aparelhos medidos | X55 (720×1280), X35H e R35S (640×480) |

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
- `runtime` aceita `native`, `godot`, `retroarch` ou `proton`. `native` é o
  caso dos apps em Rust, C++ e Python — e é o que **todos os 7** apps deste
  repositório usam hoje. Os outros três valores existem porque o campo descreve
  como o app roda no aparelho: um app de terceiro classificado como `godot` sai
  **Incompatível**, porque nenhum build oficial do Godot tem KMSDRM (ver
  [docs/godot.md](docs/godot.md)).
- `arch` depende de `distribution`, e é conferido em par com ela:

  | `distribution` | `arch` aceito |
  | --- | --- |
  | `image`, `store` | `aarch64` |
  | `host` | `x86_64`, `arm64`, `any` |

  Um app de console que declare `x86_64` reprova (`arquitetura_de_pc_em_app_do_aparelho`),
  e um app de PC que declare `aarch64` também (`arquitetura_de_console_em_app_de_pc`).
  Só ampliar o enum deixaria o campo voltar a não significar nada.
- `entrypoint` começa em `bin/`, não é absoluto e não contém `..`.
- `entrypoint` aponta para arquivo que **existe** (medido no arquivo, não no JSON).
- `capa.png`, quando presente, tem exatamente **320×320** (não é exigida em
  `distribution: "host"` — avisar sobre a capa de um app que é a própria loja é
  ruído que ensina a ignorar avisos).
- `metadata.json` não aceita campo fora do schema.
- GPL/AGPL/LGPL com `distribution: "image"` reprova: a imagem é assinada e a
  GPL-3.0 §6 exige que o comprador possa substituir o binário.

## Comandos

```bash
node scripts/validate-app.mjs app/meu-app    # um app
node scripts/validate-app.mjs --all          # todos
node tests/run-all.mjs                       # 199 testes JS
bash scripts/testar-apps-pc.sh               # 90 testes Python (forge + painel)
```

## `assets/` não vai para o aparelho

`assets/` guarda o material de trabalho — logo em alta resolução, `.kra`, `.psd`,
fontes. **Não entra no pacote.** O que o app precisa em tempo de execução vai em
`bin/`.

## Contrato com o Giochi System

Cada app declara o que precisa em `forge.json`: arquitetura, runtime, API gráfica,
memória e resolução mínimas. **Não** declara pacotes Buildroot — quem sabe o que a
imagem tem é o `Giochi System`.

A ferramenta compara esse manifesto com o perfil **medido** do aparelho e
responde `Compatível`, `Parcial` ou `Incompatível`. Detalhes em
[docs/forge.md](docs/forge.md).

## Onde cada app vai parar

O campo `distribution` em `app/apps.json` decide o destino, e cada um tem
obrigações diferentes de licença:

| `distribution` | Onde roda | Exemplo |
| --- | --- | --- |
| `image` | dentro da imagem assinada do aparelho | launcher (que é `role: shell`) |
| `store` | instalado em `/user` no aparelho, enviado pela loja | um app comprado |
| `host` | no **computador do usuário** — não vai para o aparelho | loja, forge, store, painel |

Hoje **nenhum app entra no catálogo do aparelho**: os três de interface são
`role: shell` (a interface não lista a si mesma) e os quatro de PC são `host`.
O catálogo sai vazio, e isso é o resultado correto das regras.

A loja roda no PC e envia o pacote pronto para o aparelho. Por isso um app GPL
**pode** ser vendido: ele não entra na imagem assinada, e o comprador pode
substituí-lo em `/user`. Ver [docs/licenciamento.md](docs/licenciamento.md).

## Licença

MIT — ver [LICENSE](LICENSE).
