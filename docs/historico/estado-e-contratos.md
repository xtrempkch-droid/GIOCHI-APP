# Estado, fronteira, estrutura, contratos, comandos e decisões

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**: §2 (estrutura),
> §3 (contratos: `apps.json`, `metadata.json`, `forge.json`),
> §4 (comandos) e §5 (decisões que não se reabrem).
>
> O resumo fica em [`AGENTS.md`](../../AGENTS.md) §2–§5; esta página é a
> referência completa, com o motivo de cada campo e de cada comando.

---

## 2. Estrutura

```text
app/
├── apps.json              # fonte da verdade: id, folder, runtime, status, distribution, license, role
├── _template/             # modelo para app novo
├── launcher/              # Rust + SDL2 — a interface do aparelho
│   ├── Cargo.toml         # workspace: crates/core + crates/ui-sdl
│   ├── crates/core/       # 8 módulos, 117 testes, sem SDL
│   └── crates/ui-sdl/     # 15 módulos, 136 testes, KMSDRM + evdev + catálogo
├── launcher-teste/        # o mesmo launcher sobre a GIOCHI-ENGINE (scaffold)
├── launcher-star-dust/    # o launcher + camada de efeitos (ready)
├── forge/                 # empacotador/classificador (host, Python)
├── store/                 # baixa/paga e envia ao aparelho (host, Python)
├── painel/                # interface web local (host, Python)
└── loja/                  # loja gráfica (host, Rust + SDL2)
schemas/                   # metadata.schema.json + forge.schema.json

# no aparelho (convenção do Giochi System, não deste repo)
/opt/giochi/games/<id>     # jogos que vêm na imagem
/opt/giochi/apps/<id>      # apps que vêm na imagem (inclui o launcher e o musica)
/user/games/<id>           # jogos instalados pela loja / cartão
/user/apps/<id>            # apps instalados pela loja / cartão
scripts/                   # validador, empacotadores, ferramentas do aparelho e de issue
tests/                     # 233 testes JS + suites Python de forge e painel
docs/                      # 29 páginas (inclui idiomas.md e issues-para-outros-repos.md, §1.1)
.github/workflows/         # validate.yml, pc-apps.yml, loja.yml
```

### Anatomia de um app

```text
app/<slug>/
├── metadata.json     # identidade do pacote (obrigatório)
├── forge.json        # requisitos de hardware (opcional)
├── capa.png          # 320x320, capa da loja
├── src/              # CÓDIGO-FONTE
├── bin/              # O QUE VAI PARA O APARELHO
├── assets/           # ARTE-FONTE: logos, fontes, arquivos editáveis
└── README.md
```

**A distinção `src/` × `bin/` × `assets/` é a decisão estrutural mais importante.**
`src/` é o que se edita, `bin/` é o que o aparelho executa, `assets/` é material
de trabalho que não viaja. Confundir isso gera pacote de centenas de megabytes com
arquivos editáveis dentro.

**Grupos** (`app/<grupo>/<slug>`) continuam suportados pelo validador e não são
usados hoje. O que distingue grupo de app é o `metadata.json` — pasta com o
arquivo é app, pasta sem ele é grupo, e a varredura desce um nível. Profundidade
máxima: 2.

---

## 3. Contratos

### `app/apps.json`

```json
{ "id": "launcher", "folder": "launcher", "runtime": "native",
  "status": "ready", "distribution": "image",
  "license": "Proprietary", "role": "shell" }
```

| Campo | Valores | Observação |
| --- | --- | --- |
| `id` | slug kebab | identidade estável; é o nome da pasta no aparelho |
| `folder` | caminho em `app/` | pode ter um nível de grupo |
| `runtime` | `godot`, `retroarch`, `proton`, `native` | `native` cobre Rust, C++, Python |
| `status` | `scaffold`, `ready` | `scaffold` rebaixa `entrypoint` ausente a **aviso** |
| `distribution` | `image`, `store`, `host` | decide destino **e** licença |
| `license` | SPDX ou `Proprietary` | precisa bater com o `metadata.json` |
| `role` | `shell` | opcional; fica fora do catálogo |

`role: shell` existe porque o launcher tem `status: ready`, `distribution: image`
e binário no lugar — entrava por todos os critérios, e o cartão "Launcher"
abriria um **segundo** launcher, que perde o DRM (exclusivo) e morre com
`kmsdrm not available`.

### O prefixo `launcher` **é** tema — decisão do dono, 15/11/2026

Até então o papel era declarado e não inferido do nome. O dono pediu o
contrário: *"todo app com o nome começando com `launcher` seja tratado como tema,
assim eu poderia criar um outro `launcher-*` e o launcher vai reconhecer como
tema"*. Vale, e o motivo é bom: **esquecer o `role` era fácil e o sintoma
aparecia longe da causa**. Hoje as duas condições **somam**:

```js
papelDeApp(id, declarado) = declarado === "shell" || ehTema(id)
ehTema(id)                = id === "launcher" || id.startsWith("launcher-")
```

| Situação | Resultado |
| --- | --- |
| `launcher`, `launcher-teste`, `launcher-star-dust`, `launcher-neon` | tema, **sem** declarar nada |
| `my-launcher-tool`, `launchertool`, `relancher` | app comum — o prefixo é no **começo** e com **hífen** |
| `meu-tema` com `role: "shell"` | tema, pela declaração |

O hífen não é decoração: sem ele `launchertool` viraria tema e o app sumiria do
catálogo em silêncio. E a declaração continua valendo — quem não usa o prefixo
não perde a saída.

O custo aceito é que um app chamado `launcher-*` que **não** seja interface
também fica de fora. É o erro certo para o lado certo: um cartão a menos é
recuperável renomeando a pasta; um cartão a mais abre um segundo launcher e
derruba a interface inteira.

`role` só aceita `shell` (`indice_role_invalido`). Antes qualquer texto passava
e era descartado em silêncio — `role: "tema"`, o nome que o dono usa ao falar,
não fazia nada e o app virava cartão como se não tivesse papel nenhum.

**Tema fora da imagem avisa** (`tema_fora_da_imagem`, aviso e não erro): quem
inicia um tema é o `giochi-init`, que procura em `/opt/giochi/apps`. Um tema com
`distribution: store` instala em `/user/apps`, onde nada o procura — instala sem
erro e nunca abre. Rodá-lo à mão para testar é legítimo, daí aviso.

Hoje vale para os três apps de interface.

Pasta em `app/` fora do `apps.json` reprova com `app_nao_registrado`: um app
invisível para a loja é pior que um app quebrado.

### `distribution` decide o destino e a licença

| Valor | Onde roda | Efeito no licenciamento |
| --- | --- | --- |
| `image` | dentro da imagem assinada | exige licença permissiva |
| `store` | instalado em `/user/apps`, enviado pela loja | aceita GPL |
| `host` | no **computador do usuário** | não vai para o aparelho |

O validador **recusa mecanicamente** GPL/AGPL/LGPL com `distribution: "image"`
(código `licenca_gpl_na_imagem`). É o que permite vender app GPL: o pacote
baixado não faz parte da imagem assinada e o comprador pode substituí-lo em
`/user` — exatamente o procedimento sem chave que a GPL-3.0 §6 exige.

`arch` é validado **em par com `distribution`**: `aarch64` para `image`/`store`,
`x86_64`/`arm64`/`any` para `host`. Códigos
`arquitetura_de_console_em_app_de_pc` e `arquitetura_de_pc_em_app_do_aparelho`.
Só ampliar o enum teria deixado o campo voltar a não significar nada.

### `metadata.json`

Obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`, `entrypoint`.
Opcionais: `graphics_api`, `buttons_map`, `license`, `licenseUrl`, `releaseYear`,
`category`, `slug`, **`kind`**.

**`kind` decide a aba e a raiz** (`game` ou `app`; ausente = `game`). É
obrigatório no validador para app de **imagem** que não é tema: é ele que escolhe
entre `/opt/giochi/games/<id>` e `/opt/giochi/apps/<id>` e, portanto, entre a
Home e a aba Apps. Sem o campo, o padrão é `game` — e um app novo apareceria no
carrossel de jogos sem um único aviso. `additionalProperties: false` — campo desconhecido reprova.
Semver obrigatório (`1.0.0`, não `1.0`).

- **`slug`** — identidade do pacote. Derivar do **título** faria "Editor" →
  "Editor Pro" mover o app de lugar, quebrando catálogo e favorito. Ordem:
  `metadata.slug` → `id` do `apps.json` → título (último recurso, e o pacote diz
  que usou). Um `slug` derivado do título já instalou em
  `/opt/giochi/apps/pac-man` enquanto o catálogo apontava para `pacman`.
- **`releaseYear`** e **`category`** viajam numa cadeia de três elos —
  `metadata.json` → índice da loja → catálogo do launcher. `0`/ausente é
  *desconhecido* e a UI mostra `—`. **Nunca** derivar do índice do app.

**Substituto de dado ausente precisa ser declarado ausente, não preenchido com
número interno.** A Home já mostrou `Spacewar • Ação • #002`, que parecia
informação do jogo e era o índice do cartão.

### `forge.json`

Requisitos de capacidade — todas opcionais: `arch`, `runtime`,
`graphics_api_min`, `graphics_api_fallback`, `godot_version`, `ram_min_mb`,
`vram_min_mb`, `display_min_width/height`, `touchscreen_required`.

**Nunca declara `BR2_PACKAGE_*`.** O Buildroot descarta símbolo inexistente em
silêncio; um app que declarasse pacote produziria build verde com biblioteca
ausente. As dependências ficam no `Config.in` do **Giochi System**.

`graphics_api_min: "none"` existe para app que roda em software (SDL2 puro). O
**default continua `gles3`**: omitir num app gráfico tem de ser conservador —
`none` é declaração, não esquecimento. A cadeia de checagem tem `else` que
reprova valor desconhecido (`gles4`, erro de digitação).
