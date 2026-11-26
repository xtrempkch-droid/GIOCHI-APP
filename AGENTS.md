# AGENTS.md — Giochi app

> Handoff técnico deste repositório. Leia antes de continuar o trabalho.
> Atualizado em: 26/11/2026.
>
> **Este arquivo é o resumo.** Em 26/11/2026 ele passou de 2.169 linhas para o
> que está aqui, e o texto completo — com as medições, o porquê de cada decisão
> e o histórico de cada defeito — foi para [`docs/historico/`](docs/historico/),
> **verbatim**. Nada foi reescrito nem descartado na mudança; os números de
> seção são os mesmos dos dois lados, então `§6.3` continua apontando para o
> mesmo assunto em qualquer um deles.
>
> | Preciso de… | Onde |
> | --- | --- |
> | o estado de hoje, em uma tela | §0, abaixo |
> | uma regra ou um comando | este arquivo |
> | a medição que sustenta uma regra | `docs/historico/` (§ da tabela em §0.1) |
> | o que já quebrou antes | §13, e `docs/historico/armadilhas.md` |

## 0. Estado em uma tela

| | |
| --- | --- |
| apps em `app/apps.json` | **8**: `launcher`, `launcher-teste`, `launcher-star-dust` (imagem, tema) + **`musica`** (imagem, app) + `forge`, `store`, `painel`, `loja` (host) |
| `validate-app.mjs --all` | **8 válidos** (1 em scaffold: `launcher-teste`) |
| `tests/run-all.mjs` | **233/233** JS |
| `compilar-launcher.sh --testes` | **330** Rust (171 ui-sdl + 159 core) |
| Star Dust / launcher-teste | **178** / **95** Rust |
| `compilar-musica.sh --testes` | **158** Rust (120 núcleo sem SDL + 38 binário) |
| `compilar-loja.sh --testes` | **116** Rust (100 núcleo + 16 interface) |
| `testar-apps-pc.sh` | **90** Python (58 forge + 32 painel) |
| apps no catálogo do aparelho | **4** — 3 jogos (`/user/games`) + `musica` (`/opt/giochi/apps`), detectados do disco (§8) — **mais um cartão por port do PortMaster** (§6.11) |
| idiomas da interface | **10** (os do site), padrão **inglês**, escolhidos na primeira abertura e em Config › Aparência (§6.10) |
| ports do PortMaster | **listados na aba Apps** desde 26/11/2026 (§6.11): `/user/ports` → cartão em `/user/apps`, lançado por `giochi-ports --rodar`; a **loja** tem cartão próprio (`portmaster`, `--liberar-drm`) |

O que roda no aparelho é **o launcher** e os apps que ele detecta. Até
15/11/2026 o catálogo saía vazio por construção e a aba Apps desenhava **oito
apps escritos no código**, nenhum instalado: apertar `A` dizia "Abrindo X..." e
nada abria. Hoje as duas abas **detectam** — jogos em `/opt|/user/games`, apps
em `/opt|/user/apps` — e um port instalado pela GUI do PortMaster aparece sem
nenhum passo manual.

### 0.1 Onde está o detalhe

| Assunto | Página |
| --- | --- |
| §2–§5 (estrutura, contratos, comandos, decisões) | [`docs/historico/estado-e-contratos.md`](docs/historico/estado-e-contratos.md) |
| §6 (o launcher inteiro) | [`docs/historico/launcher.md`](docs/historico/launcher.md) |
| §7–§8 (apps e catálogo) | [`docs/historico/apps-e-catalogo.md`](docs/historico/apps-e-catalogo.md) |
| §9–§10 (PC e pacote) | [`docs/historico/pc-e-pacote.md`](docs/historico/pc-e-pacote.md) |
| §11–§12 (verificação e ambientes) | [`docs/historico/verificacao-e-ambientes.md`](docs/historico/verificacao-e-ambientes.md) |
| §13–§14 (armadilhas e o que está provado) | [`docs/historico/armadilhas.md`](docs/historico/armadilhas.md) |
| §16–§17 (pendências e canal) | [`docs/historico/pendencias-e-canal.md`](docs/historico/pendencias-e-canal.md) |
| controle virtual `Giochi Gamepad` (18/11/2026, **não implementado**) | [`docs/controle-virtual.md`](docs/controle-virtual.md) |
| ports do PortMaster | [`docs/portmaster.md`](docs/portmaster.md) |
| idiomas | [`docs/idiomas.md`](docs/idiomas.md) |

## 1. Objetivo e fronteira

| Repositório | Função |
| --- | --- |
| **Giochi app** (este) | Apps do sistema, ferramentas de PC, arte e manifesto de requisitos |
| **Giochi System** | Sistema Linux limpo: kernel, rootfs, boot, drivers, painel |
| **Giochi os** | Junta sistema + apps e gera a imagem final por aparelho |
| **repositório de jogos** | Os jogos 3D que moravam em `app/jogos/` até 14/11/2026 |

Cada pasta é **repositório Git individual**. A comunicação é sempre por
**artefato declarado explicitamente**, nunca por caminho relativo presumido.

GitHub: `https://github.com/xtrempkch-droid/GIOCHI-APP`.

### 1.1 Repositório alheio é **SOMENTE LEITURA** — decisão do dono, 15/11/2026

```text
/home/charles/Giochi/
├── GIOCHI-APP/          ← este repo. Alterar é o trabalho.
├── GIOCHI-SYSTEM/       ┐
├── GIOCHI-OS/           │
├── GIOCHI-ENGINE/       ├─ de outra equipe. LER, nunca escrever.
├── GIOCHI-ENGINE-3D/    │
├── GIOCHI-GAMES/        │
└── giochi-engine/       ┘
```

Quando a correção mora em repo alheio: **não editar** (nem um comentário); abrir
uma **issue** no repositório responsável com o que foi medido, arquivo e linha,
a consequência e o que a correção provavelmente é; registrar o número em §17.

**Contornar aqui não substitui a issue** — decisão do dono, 26/11/2026. Um
workaround do lado do app faz a coisa funcionar hoje e esconde o defeito de quem
deveria corrigi-lo: todo frontend novo repete a mesma dança, e o primeiro que
esquecer volta ao sintoma. O caso que gerou a regra é o `giochi-ports` rodando o
`pugwash` sem `PYSDL2_DLL_PATH` e sem limpar o `.pugwash-reboot` (§6.11): o
cartão da loja contorna, e o pedido 6 de `docs/issues-para-outros-repos.md`
existe para que a correção vá para o wrapper, onde vale para todos.
Os vizinhos já têm trabalho não commitado de outra pessoa, e escrever ali
mistura o meu diff com o de alguém.

**O corpo da issue é entregável deste repo**, não a criação dela:

```bash
node scripts/abrir-issue.mjs --lista    # os pedidos, com número e repo
node scripts/abrir-issue.mjs 5 --abrir  # abre a issue já preenchida no GitHub
node scripts/abrir-issue.mjs 5 --corpo  # só o texto, para colar
```

O formulário do VS Code (`issue_write`) **não é garantia**: duas submissões
terminaram com "não apareceu o pedido para clicar" e as issues **foram** criadas
— a conferência antes do clique devolveu o passado e gerou duplicatas (#3, #4 e
#5 do PortMaster). O caminho de hoje é a URL do `issues/new`, e o clique que
resta é no GitHub. O corpo sai verbatim de
[`docs/issues-para-outros-repos.md`](docs/issues-para-outros-repos.md).

## 2. Estrutura

```text
app/
├── apps.json              # fonte da verdade: id, folder, runtime, status, distribution, license, role
├── _template/             # modelo para app novo
├── launcher/              # Rust + SDL2 — a interface do aparelho
│   ├── Cargo.toml         # workspace: crates/core + crates/ui-sdl
│   ├── crates/core/       # 10 módulos, 159 testes, sem SDL
│   └── crates/ui-sdl/     # 16 módulos, 171 testes, KMSDRM + evdev + catálogo + ports
├── launcher-teste/        # o mesmo launcher sobre a GIOCHI-ENGINE (scaffold)
├── launcher-star-dust/    # o launcher + camada de efeitos (ready)
├── forge/                 # empacotador/classificador (host, Python)
├── store/                 # baixa/paga e envia ao aparelho (host, Python)
├── painel/                # interface web local (host, Python)
├── musica/                # tocador (imagem, app) — o primeiro `image` que não é tema
└── loja/                  # loja gráfica (host, Rust + SDL2)
schemas/                   # metadata.schema.json + forge.schema.json

# no aparelho (convenção do Giochi System, não deste repo)
/opt/giochi/games/<id>     # jogos que vêm na imagem
/opt/giochi/apps/<id>      # apps que vêm na imagem (inclui o launcher e o musica)
/user/games/<id>           # jogos instalados pela loja / cartão
/user/apps/<id>            # apps instalados pela loja / cartão (+ cartões de port)
/user/ports/               # PortMaster: `<nome>.sh` + `<nome>/` (§6.11)

scripts/                   # validador, empacotadores, ferramentas do aparelho e de issue
tests/                     # 233 testes JS + suites Python de forge e painel
docs/                      # 29+ páginas (inclui idiomas.md, portmaster.md, historico/)
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

**`src/` × `bin/` × `assets/` é a decisão estrutural mais importante.** `src/` é
o que se edita, `bin/` é o que o aparelho executa, `assets/` é material de
trabalho que não viaja.

**Grupos** (`app/<grupo>/<slug>`) continuam suportados pelo validador e não são
usados hoje. O que distingue grupo de app é o `metadata.json`; profundidade
máxima: 2.

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

**O prefixo `launcher` é tema** (decisão do dono, 15/11/2026): as duas condições
**somam**.

```js
papelDeApp(id, declarado) = declarado === "shell" || ehTema(id)
ehTema(id)                = id === "launcher" || id.startsWith("launcher-")
```

O hífen não é decoração: sem ele `launchertool` viraria tema e o app sumiria do
catálogo em silêncio. Custo aceito: um app `launcher-*` que não seja interface
também fica de fora — um cartão a menos é recuperável renomeando a pasta; um
cartão a mais abre um segundo launcher e derruba a interface.

`role` só aceita `shell`. **Tema fora da imagem avisa** (`tema_fora_da_imagem`):
o `giochi-init` procura em `/opt/giochi/apps`, e `distribution: store` instala
em `/user/apps`, onde nada o procura. Pasta em `app/` fora do `apps.json`
reprova (`app_nao_registrado`).

### `distribution` decide o destino e a licença

| Valor | Onde roda | Efeito no licenciamento |
| --- | --- | --- |
| `image` | dentro da imagem assinada | exige licença permissiva |
| `store` | instalado em `/user/apps`, enviado pela loja | aceita GPL |
| `host` | no **computador do usuário** | não vai para o aparelho |

GPL/AGPL/LGPL com `distribution: "image"` é recusado mecanicamente
(`licenca_gpl_na_imagem`) — é o que permite vender app GPL. `arch` é validado
**em par** com `distribution`: `aarch64` para `image`/`store`,
`x86_64`/`arm64`/`any` para `host`.

### `metadata.json`

Obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`, `entrypoint`.
Opcionais: `graphics_api`, `buttons_map`, `license`, `licenseUrl`, `releaseYear`,
`category`, `slug`, **`kind`**. `additionalProperties: false`; semver obrigatório.

- **`kind`** (`game` ou `app`; ausente = `game`) decide a aba **e** a raiz. É
  obrigatório no validador para app de imagem que não é tema: sem ele, um app
  novo aparece no carrossel de jogos sem um único aviso.
- **`slug`** — identidade do pacote. Ordem: `metadata.slug` → `id` do
  `apps.json` → título (último recurso, e o pacote diz que usou).
- **`releaseYear`** e **`category`** viajam em três elos — `metadata.json` →
  índice → catálogo. `0`/ausente é *desconhecido*, e a UI mostra `—`. **Nunca**
  derivar do índice do app: a Home já mostrou `Spacewar • Ação • #002`.

### `forge.json`

Requisitos de capacidade — todas opcionais: `arch`, `runtime`,
`graphics_api_min`, `graphics_api_fallback`, `godot_version`, `ram_min_mb`,
`vram_min_mb`, `display_min_width/height`, `touchscreen_required`.

**Nunca declara `BR2_PACKAGE_*`** — o Buildroot descarta símbolo inexistente em
silêncio. `graphics_api_min: "none"` existe para app de software; o default é
`gles3`, porque omitir num app gráfico tem de ser conservador.

Detalhe de cada campo e o motivo:
[`docs/historico/estado-e-contratos.md`](docs/historico/estado-e-contratos.md).

## 4. Comandos

```bash
node scripts/validate-app.mjs --all         # valida todos
node scripts/validate-app.mjs app/loja      # valida um
node tests/run-all.mjs                      # 233 testes JS (lista vem de readdir)
bash scripts/testar-apps-pc.sh              # 90 testes Python (forge + painel)

bash scripts/compilar-launcher.sh           # build x86-64, resolve o SDL2
bash scripts/compilar-launcher.sh --testes  # 330 testes
bash scripts/compilar-loja.sh --testes      # 116 testes
bash scripts/compilar-musica.sh --testes    # 158 testes (120 núcleo + 38 binário)
bash scripts/compilar-musica.sh --clippy    # -D warnings nos dois crates
bash scripts/compilar-musica.sh --verificar # determinismo, rotação e orçamento
bash scripts/compilar-musica-arm64.sh --instalar   # aarch64 + instala no aparelho

# Clippy faz parte da barra: `-D warnings` (é o que o CI roda)
( cd app/launcher && cargo clippy --all-targets -- -D warnings )
( cd app/launcher-star-dust && cargo clippy --all-targets -- -D warnings )
( cd app/loja && cargo clippy --all-targets -- -D warnings )

bash scripts/compilar-launcher-arm64.sh                       # cross-compile aarch64
bash scripts/compilar-launcher-arm64.sh --instalar            # e instala no aparelho
bash scripts/compilar-launcher-teste-arm64.sh --instalar
node scripts/empacotar.mjs app/launcher --saida dist/pacotes  # .giochi assinado
node scripts/verificar-pacote.mjs dist/pacotes/launcher.giochi

# ports do PortMaster (§6.11) — no PC, sem aparelho
GIOCHI_PORTS=/tmp/p/ports GIOCHI_APPS=/tmp/p/apps \
  ./app/launcher/target/debug/giochi-launcher --sincronizar-ports

# pedido para outro repo: corpo pronto → URL do `issues/new` (§1.1)
node scripts/abrir-issue.mjs --lista
node scripts/abrir-issue.mjs 5 --abrir
```

**Não rode `cargo test --workspace` direto em `app/launcher`.** O crate linka com
`rust-lld`, que ignora `LIBRARY_PATH`; sem
`RUSTFLAGS=-L .../giochi-sdl2/root/usr/lib/x86_64-linux-gnu` o link falha com
`unable to find library -lSDL2` **mesmo com o build passando**. Use `--testes`.

**Não use `node --test tests/`**: o runner spawna um processo por arquivo e sob
sandbox falha com `spawn EPERM`. O runner próprio descobre os `.test.mjs` por
`readdir` — a lista fixa já deixou um arquivo novo de fora e o resultado saiu
"164/164" com menos cobertura.

**`--testes` compila *harnesses*, não o binário.** Quem for usar
`target/debug/giochi-launcher` (o `verificar`, uma captura, um teste de ponta a
ponta) tem de rodar `compilar-launcher.sh` antes — já custou três rodadas com
número errado (§13).

## 5. Decisões que não se reabrem sem o dono pedir

- **Pasta `app/` (singular)** — escolha do usuário.
- **`runtime: "native"`** no enum — sem ele, ferramentas em Python não tinham
  como se declarar.
- **`status: scaffold`** — espelha o padrão `draft` do Giochi System e evita CI
  vermelho permanente durante migração. Todo o resto continua sendo erro.
- **Licença do repositório: MIT.** Exclusividade sobre o sistema foi descartada
  conscientemente.
- **Licença dos apps próprios: `Proprietary`.** O comprador não pode
  redistribuir; é o que dá base para cobrar pelo binário.
- **A loja roda no PC do usuário**, não no aparelho.
- **Migração por script, não por cópia manual** — projetos com assets binários
  copiados por API produzem árvore incompleta e sem histórico.

## 6. O launcher — o app central

Rust + SDL2, **renderer próprio**. `egui` foi descartado (desenho custom
limitado) e Slint também (DSL + runtime grande para 1 GB de RAM). SDL2 já está
no Buildroot e o KMSDRM dispensa compositor.

| Peça | Onde |
| --- | --- |
| estado, foco, reducer, grade, detecção | `crates/core` (159 testes, sem SDL) |
| janela, desenho, rotação, evdev, catálogo, ports | `crates/ui-sdl` (171 testes) |

| Seção | Assunto | Detalhe completo |
| --- | --- | --- |
| 6.1 | rotação vinda do sistema | `docs/historico/launcher.md` |
| 6.2 | layout adaptativo, escala 1,0 | idem |
| 6.3 | entrada por evdev, não por SDL | idem + [`docs/controle-virtual.md`](docs/controle-virtual.md) |
| 6.4 | o que o kernel promete × entrega | idem |
| 6.5 | energia, brilho, volume, sono | idem |
| 6.6 | Wi-Fi e Bluetooth | idem |
| 6.7 | feature `texto` | idem |
| 6.8 | catálogo e lançamento (`fork`+`wait`) | idem |
| 6.9 | as duas abas detectam | idem |
| 6.10 | idioma e tema | [`docs/idiomas.md`](docs/idiomas.md) |
| 6.11 | ports do PortMaster | [`docs/portmaster.md`](docs/portmaster.md) |

### 6.1 Rotação — a orientação é fato do sistema

O app **não contém nome de aparelho nem sentido fixo**. Precedência, e o log diz
a fonte: `GIOCHI_ROTATE` → `/user/.config/giochi/rotation` (relê a cada 1 s) →
`/sys/class/graphics/fbcon/rotate` → `/proc/cmdline` (`fbcon=rotate:N`, usado só
quando o sysfs diz `0`) → formato do output (marca `Deduzido`).

O cruzamento com o `cmdline` existe por medição: no X55 o sysfs dizia `0`
enquanto o `cmdline` pedia `fbcon=rotate:3`. Cada primitiva pública mapeia a
coordenada **uma vez** e delega para uma versão `*_dev`; sem isso
`stroke_round_rect` giraria duas vezes. Errar o sentido dá "de ponta a cabeça"
(horário e anti-horário diferem 180°).

### 6.2 Layout adaptativo — escala 1,0 em qualquer painel

`draw::projecao(rot, output)` devolve o espaço de projeto: sem giro, projeto =
painel; com giro, projeto é paisagem com medidas trocadas. O espaço lógico
continua sendo o do dispositivo, então a fonte **nunca encolhe**, e o layout é
proporcional (`views::Moldura`). O projeto fixo em 1280×720 dava escala 0,5 num
painel 640×480 e 25 % de letterbox.

### 6.3 Entrada por evdev, não por SDL gamepad

`crates/ui-sdl/src/input_evdev.rs` lê `/dev/input/event*` direto. Motivo medido:
os botões vêm de `gpio-keys`/`adc-joystick`, o `SDL_GameControllerDB` não tem
mapeamento para aparelho custom, e o `buttons.map` mapeia **por código evdev**
enquanto o SDL entrega índice. Uma thread por `event*`, estado é a soma de todos.

**A releitura é obrigatória.** O nó de um controle Bluetooth só existe depois que
ele conecta: até 13/11/2026 os apps liam `/dev/input` uma vez e um controle
pareado em execução nunca era visto. Hoje há varredura a cada **2 s**
(`REVISAO_MS`), idempotente por `HashSet`, com log quando acha algo novo.

`ButtonsMap::parse` acumula `desconhecidos` e `load` avisa **uma vez** — um
símbolo desconhecido era descartado em silêncio e o botão não funcionava
(aconteceu duas vezes: comentário no fim da linha e `KEY_VOLUME*`). **O que foi
ignorado tem de ser dito.** `scripts/verificar-mapa.mjs` lê a lista de símbolos
do **binário**, não de uma cópia.

**O analógico não é lido** (normalizar exige `absinfo` por ioctl). O controle
virtual de 18/11/2026 já entrega `-32767..32767` e dispensa o ioctl — plano em
[`docs/controle-virtual.md`](docs/controle-virtual.md), **não implementado**.

### 6.4 O que o kernel promete × o que ele entrega

| Pergunta | Ferramenta | Responde |
| --- | --- | --- |
| o evento chegou? | `aparelho.sh input` | lê `/dev/input` cru, sem mapa |
| o dispositivo declara esse código? | `aparelho.sh capacidades` | lê `B: KEY=` do `/proc` |

| declara? | entrega? | Conclusão |
| --- | --- | --- |
| sim | sim | kernel ok; o problema é `buttons.map` ou `code_for` |
| sim | não | driver ou DTS não registra o evento |
| não | sim | bitmask incompleto; aviso, não veredito |
| não | não | o botão não existe neste hardware |

**O formato do bitmask engana**: o kernel lista as palavras de 64 bits em ordem
**decrescente**, então a última coluna é a palavra 0. `evtest --query` devolve 0
para qualquer código nesta imagem; o `/proc` é a única fonte. `None` (não medido)
≠ lista vazia (medido, nada declarado): no PC o launcher **não avisa**.

### 6.5 Energia, brilho, volume e sono

1. **Qualquer entrada de pessoa acorda** (`Cmd::is_user_input`), e o botão que
   acorda é consumido pelo despertar.
2. **O botão segurado não pode vazar**: `poll_gamepad` roda mesmo dormindo e
   descarta o resto do quadro quando estava travado.
3. **Dormir apaga o backlight**; `apagar` é idempotente e `destravar_tela_apagada`
   reacende se o valor veio 0.
4. **`KEY_POWER`**: toque curto dorme; segurar 1 s abre o menu **ao atingir o
   limiar**, não ao soltar. Sem `KEY_POWER` no mapa, o launcher avisa e o menu
   fica no START.
5. **Volume controla o mixer ALSA de verdade**, e o brilho grava
   `/user/.config/giochi/brightness` (é o que o `giochi-init` lê no boot).

**`libasound` é resolvida por `dlopen`, não por link**: o R35S não tem a
biblioteca, e biblioteca ausente é falha de carga — o processo não sobe. Um
binário só, sem feature, e `libasound` não aparece no `NEEDED`.

Bateria e temperatura vêm de sysfs a cada **5 s**, e a decisão de logar é a
função pura `energia_deve_registrar` — comparar `Option` com o sentinela `0`
fazia a linha sair para sempre num aparelho sem sensor.

### 6.6 Wi-Fi e Bluetooth

`semfio.rs` chama `wpa_cli` e `bluetoothctl` e publica `Option<bool>`: `None` =
não sondado, `Some(false)` = medido que não tem. **`None` não é ausência**: no
R35S a tela diz "sem rádio neste aparelho" e o "procurar de novo" fica inerte.

Três armadilhas medidas: **não existe `timeout` na imagem**; **`wlan0` sobe
DOWN** (o app chama `ip link set up` antes de varrer); **`wpa_cli` e `ip` moram
em `/usr/sbin` e `/sbin`**.

**`update_config=1` é o que separa "conectou" de "vai lembrar"** — e é da
imagem, não do app (§17, pedido 2 do documento). Quando `save_config` falha, a
tela diz *"a rede não foi salva: vai pedir a senha de novo"* em vez de silêncio.
**Esperar a associação é o que separa "aceitou" de "conectou"**: 12 s, e ao
falhar devolve o **último estado do supplicant** (`4-WAY_HANDSHAKE` = senha
recusada; `SCANNING` = fora de alcance).

**Bluetooth como entrada**: o kernel está completo para clássico (`CONFIG_HID`,
`HID_GENERIC`, `BT_HIDP`) e o `CONFIG_UHID=y` já está no fragmento da x55
(conferido no remoto em 26/11/2026). O ponta a ponta **nunca foi medido** — falta
uma imagem construída com esse config. As três provas que diziam faltar
`CONFIG_HID` eram falsas (§13).

### 6.7 Feature `texto`

| Build | Feature | Linka contra |
| --- | --- | --- |
| padrão | `texto` | `libSDL2` + `libSDL2_ttf` |
| enxuto | `--no-default-features` | só `libSDL2` |

O CI constrói **as duas**. `ttf::init()` **não pode ser chamado duas vezes** — a
segunda devolve erro e o launcher volta do app **sem nenhum texto**; o contexto
é por thread (`thread_local`).

### 6.8 Catálogo e lançamento

`catalogo.rs` monta as duas listas e resolve `id → dir + entrypoint`: varre as
quatro raízes e junta com `/opt/giochi/apps/index.json`, com o disco vencendo
(§8). `GIOCHI_RAIZES` sobrepõe as raízes varridas e `GIOCHI_CATALOGO` aponta
outro índice; isolar de verdade exige apontar `GIOCHI_CATALOGO` para um índice
vazio.

**O display precisa ser solto**: `App::encerrar()` **consome** o `App`, e o
`Painter` destruído libera o DRM master. O ciclo é `fork`+`wait` — medido nos
dois aparelhos, o PID do launcher não muda enquanto o app roda, então **não
trocar para `exec`** sem confirmar a linha do init em campo.

**Sair de um app que não responde**: START+B por 1,5 s → `SIGKILL` no filho (o
SDL só posta `SDL_QUIT` na fila). Sem `start`/`b` no mapa, o launcher **avisa** e
segue sem o gesto.

### 6.9 As duas abas **detectam** — decisão do dono, 15/11/2026

```
jogos:  /opt/giochi/games   /user/games
apps:   /opt/giochi/apps    /user/apps
```

`GIOCHI_RAIZES` (separado por `:`) sobrepõe as raízes **varridas**. A varredura
é **conjunta** e a classificação é pelo `kind` do pacote, não pela pasta: os três
jogos do R35S estavam em `/user/apps` e não declaram `kind` — varrer só a raiz do
próprio tipo os faria sumir da Home. Quando o lugar está errado, o log diz a
frase que existe **em duas linguagens** (gerador do PC e detector do aparelho):

```text
catálogo: <slug> (<dir>) está em /user/apps, que é raiz de outro tipo;
  o lugar de um jogo é /user/games (ou declare "kind": "app" no metadata.json, se for um app)
```

**Tema não recebe esse aviso**: o launcher mora em `/opt/giochi/apps`, não declara
`kind` e cairia no padrão jogo — seguir o aviso quebraria o boot.

**O pedido de lançamento carrega o tipo** (`state::Pedido { tipo, indice }`): os
dois índices valem 0 ao mesmo tempo, e um pedido só com o número abriria o item
errado. **Detectar exige pacote com identidade no disco** — a primeira
instalação do `musica` copiou só o binário e ele apareceu como jogo.

**Vigiar: foco e desenho descrevem a mesma lista.** `settings_rows()` (núcleo) e
`views::linhas_da_categoria` (desenho) já divergiram e dessincronizaram o
carrossel; há teste reprovando.

### 6.10 Idioma e tema

Dez idiomas, **os do site**, na ordem do site: `en`, `zh`, `hi`, `es`, `fr`,
`ar`, `bn`, `pt`, `ru`, `ur`. Padrão **inglês**, escolhido na primeira abertura
(boas-vindas) e trocável em Config › Aparência. `Idioma::PADRAO` é `En`, mas
quem decide a boas-vindas é `State::idioma_definido` — "não escolhido" ≠
"escolhido como inglês".

A tabela fica em `core/src/idioma.rs` (uma linha por chave, dez colunas) e o
desenho pede a chave (`t(state, Chave::X)`); `Chave` é `#[repr(usize)]` e a linha
`i` **tem** de ser a chave `i`. Fonte é escolhida **por texto**
(`Painter::fonte_para`), e a cadeia inclui as fontes dos dez — a lista de
idiomas desenha os dez nomes nativos o tempo todo.

`ar`/`ur`: **shaping funciona** (`SDL2_ttf` puxa HarfBuzz; 99,8 % de identidade
entre aparelho e PC). **Bidi de parágrafo misto e layout espelhado não existem** —
fonte não conserta, e `Idioma::dir()` devolve `Rtl` sem nenhuma tela inverter.

**Tema**: `Tema` no estado, `Paleta` + `theme::accent()` lendo o tema do quadro,
arquivo em `/user/.config/giochi/theme`. Acrescentar um tema é variante +
entrada em `TODOS` + `Paleta` novo, e o `match` exaustivo não deixa compilar sem
a paleta. Hoje há **um** tema, e a linha da Config anuncia isso. Completo:
[`docs/idiomas.md`](docs/idiomas.md).

### 6.11 Ports do PortMaster (26/11/2026)

**O PortMaster é de outra equipe** — a comunidade
[`PortsMaster`](https://github.com/PortsMaster/PortMaster-New): a loja, o
catálogo e os ports são deles, e os ports são gratuitos. O que é deste
repositório é o `giochi-ports`, um invólucro de `sh` que roda o `pugwash` deles
dentro do `bwrap` (rootfs somente-leitura) porque port chega **sem assinatura**.
O crédito está no título do cartão, no `developer` lido do `gameinfo.xml` do
pacote, no `licenseUrl` e no bloco `CRÉDITOS` do script gerado; cada port credita
quem o portou, do `port.json`.

O PortMaster instala em `/user/ports` (`<nome>.sh` + `<nome>/`, ou
`<nome>/<nome>.sh`), e o launcher varre `/user/apps`. `portmaster.rs` fecha a
ponte, na abertura e **antes** da detecção:

```text
/user/apps/portmaster-<nome>/
├── .portmaster-port      # marca: só o que tem esta marca é podado
├── metadata.json         # kind: app, entrypoint: bin/portmaster-<nome>
└── bin/portmaster-<nome> # 755 — exec giochi-ports --rodar "<nome>"

/user/apps/portmaster/            # a LOJA (a GUI do PortMaster)
├── .portmaster-port              # marca "#loja-portmaster" (sentinela, não é port)
├── metadata.json                 # category: Loja, developer do gameinfo.xml
└── bin/portmaster                # 755 — exec giochi-ports --liberar-drm
```

A loja precisa de `--liberar-drm` e não de `--rodar`: ela **abre a GUI**, que tem
de pedir a tela. É o modo que o `giochi-ports` documenta ("para o launcher, abre
a GUI, e o devolve"), e ele só funciona porque o launcher morre de `SIGTERM`
(abaixo). Sem o cartão da loja, instalar um port exigiria SSH — o usuário do
aparelho não tinha como.

O `giochi-ports` roda `python3 ./pugwash` direto, sem passar pelo
`PortMaster.sh` deles. Duas coisas do caminho oficial, então, ficam por conta do
cartão — e sem elas a loja não abre (medido no R35S em 26/11/2026):

- **`PYSDL2_DLL_PATH=/usr/lib`**: o GUI é `pysdl2`; sem a variável ele carrega o
  SDL2 de `exlibs/` do pacote, que não tem KMSDRM, e o erro é
  `No available video device` — que aponta para o pysdl2, não para o driver.
  É o `pm_hook_gui_env` deles;
- **apagar `.pugwash-reboot` antes de abrir**: o `pugwash` lê esse arquivo
  *depois* de inicializar o SDL e, se existe, **pula o `pm.run()`** e retorna 0.
  A loja "abre" e fecha na mesma hora, sem mensagem. O `pm_launch.txt` apaga
  antes de cada execução; o arquivo estava lá desde 20/11 e nada abria. Mesma
  família do marcador de DRM: estado que sobra e mente sobre a próxima.

- O `entrypoint` **precisa** começar em `bin/`: o detector recusa o layout da
  raiz que o pedido original sugeria (`entrypoint inválido ... (precisa começar
  em bin/)`, medido no R35S).
- O slug nunca começa com `launcher`, então a poda de tema não o alcança.
- Conteúdo igual **não reescreve** (é cartão SD); port removido apaga o cartão;
  diretório sem a marca nunca é tocado.
- O marcador de DRM **é** criado nesse caminho, e isso é esperado: o
  `giochi-ports` pergunta por **descritor** (`/proc/*/fd` → `card0`), e o
  launcher mantém `card0` aberto até morrer. Criar o marcador sem matar o dono
  seria mentir ao `giochi-init`; matar e sair por sinal é o contrato (abaixo).
- `--sincronizar-ports` exercita o módulo sem abrir a interface (abrir custa uma
  volta do `giochi-init`, cuja cota é pequena — §13).

**O launcher tem de morrer de `SIGTERM` durante o filho — é o contrato dele com
o `giochi-ports`.** O port não pede só "o DRM livre": ele lê `/proc/*/fd` atrás
de `card0` e manda `SIGTERM` no dono, contando que o supervisor devolva a
interface quando o marcador sumir. O launcher mantém `card0` aberto (é a libmali
do `opengles2`, não o SDL: soltar o `VideoSubsystem` **não** fecha, medido) e,
sem a correção, o SDL engolia o sinal no laço de espera: o `giochi-ports`
desistia em 6 s e **o cartão não abria**, sem mensagem na tela. Hoje
`lancar_com_saida` arma um handler que repõe o default e se reergue (morrer por
sinal, não `exit(143)`: `giochi-init` zera a cota em saída por sinal).

Medido no R35S em 26/11/2026, no ciclo completo: launcher sai por `SIGTERM` →
`FORM video: opengles2 renderer, vsync on` dentro do `bwrap` → marcador presente
durante o port → usuário sai → marcador removido → **supervisor devolve o
launcher com o DRM**. E um app comum (`musica`) continua voltando: a correção não
mexeu nesse caminho.
Detalhe, medições e o que **não** foi feito (aba própria, PortMaster na loja):
[`docs/portmaster.md`](docs/portmaster.md).

## 7. Apps sobre a engine: launcher-teste, Star Dust e Música

| App | Estado | O que é |
| --- | --- | --- |
| `launcher-teste` | scaffold | o **mesmo desenho** sobre a GIOCHI-ENGINE, por `#[path]`: 100,0 % pixel a pixel em 4 painéis e 6 telas. Depende do repo irmão `GIOCHI-ENGINE-3D`. Sem sono, brilho, volume, energia ou Wi-Fi — por isso `scaffold` |
| `launcher-star-dust` | ready | o launcher de produção **mais** 8 efeitos (29 chamadas de desenho). Desligado, é idêntico **byte a byte** ao launcher |
| `musica` | ready | o primeiro app `image` que não é tema: carrossel, transporte completo, 120 testes sem SDL, quadro **100,00 %** idêntico ao do PC |

**O núcleo do Star Dust era uma cópia e ficou para trás** (4244 linhas contra
4631, sem `pede_conexao` nem `PasswordActions`): o botão "Conectar" aparecia e
não funcionava. Hoje o `Cargo.toml` aponta por caminho, como o `launcher-teste` —
**divergência impossível, não improvável**.

Efeitos: `draw_rect` é **uma chamada de backend por retângulo**, e é por isso que
o halo custa 4 bandas em vez de `stroke_round_rect` (162 chamadas/+7,3 ms contra
27 chamadas/+3,1 ms). A cadência é própria (30 fps) e o `rastro` é disparado por
`cmd.is_user_input()`, não por "o estado mudou".

```bash
bash scripts/verificar-star-dust.sh              # 640x480
bash scripts/verificar-star-dust.sh --tamanho 1280x720
bash scripts/verificar-efeitos.sh                # 4 tamanhos + 1 rotação
```

Completo: [`docs/historico/apps-e-catalogo.md`](docs/historico/apps-e-catalogo.md),
[`docs/star-dust.md`](docs/star-dust.md), [`docs/musica.md`](docs/musica.md),
[`docs/launcher-teste.md`](docs/launcher-teste.md).

## 8. O catálogo do aparelho

Duas camadas: **varredura** das quatro raízes (o aparelho, sempre) e
**`index.json`** (complemento para conteúdo fora delas, e contrato de
`docs/integracao.md`). **O disco vence** na junção — um pacote varrido substitui
a entrada do índice para o mesmo slug, com o `dir` real.

| Regra do gerador (`gerar-catalogo.mjs`) | Por quê |
| --- | --- |
| `status: scaffold` fica fora | `bin/` vazio; cartão que não abre é pior que cartão ausente |
| `distribution: host`/`store` fica fora | roda no PC / é instalado em `/user` |
| **tema** (`role: shell` **ou** prefixo `launcher`) fica fora | senão o launcher lista a si mesmo |
| `entrypoint` precisa existir | idem scaffold, mas medido no arquivo |

```bash
bash scripts/aparelho.sh catalogo        # gera, confere no aparelho e publica
node scripts/gerar-catalogo.mjs --lista  # quem entra e quem fica de fora
```

**O `dir` do aparelho não é derivável do repo e depende do `kind`**:
`<grupo>/<slug>` é `/opt/giochi/games/<id>` para jogo e `/opt/giochi/apps/<id>`
para app (`dirDoApp` em `scripts/lib/catalogo.mjs`, com teste).

**A guarda de tema existe dos dois lados**: `--catalogo-adicionar` (o próprio
aparelho) **recusa** um `launcher-*.giochi` com o motivo, e `podar_temas` fecha
o que sobra em tempo de execução — o PC que gera o índice não sabe o que estará
no aparelho. As duas podas usam um `reter` privado e cobrem **as duas listas**.

**A conferência que importa é no aparelho**: `aparelho.sh catalogo` percorre cada
`dir/entrypoint` por SSH e **reduz** o catálogo antes de publicar. Projeção de
verdade reflete remoção, não só adição.

Completo: [`docs/historico/apps-e-catalogo.md`](docs/historico/apps-e-catalogo.md).

## 9. Os apps de PC

**forge e store — Python, biblioteca padrão apenas.** A loja tem de funcionar
numa máquina limpa, o que torna o `.pyz` possível: **um** arquivo, idêntico nos
três sistemas, ~14 KB, determinístico (`zip.mjs` escreve o ZIP à mão para não
gravar `mtime` de origem).

```bash
bash scripts/testar-apps-pc.sh            # 90 testes
node scripts/empacotar-pc.mjs             # .pyz + .sh + .cmd + SHA256SUMS
app/painel/bin/giochi-painel              # http://127.0.0.1:8710
```

Três regras de classificação que só existem porque a medição derrubou a crença
anterior: **o painel é do aparelho, não do SoC** (X55 720×1280 e X35H 640×480,
os dois RK3566); **a tela é comparada com o espaço de projeto** (`720×1280` vira
`1280×720`); e **`godot_runtime` tem três estados** — só `False` reprova. A RAM
dos perfis é a **medida** (`/proc/meminfo`), não o valor nominal do SoC.

**painel** serve em `127.0.0.1:8710`, **chama** o forge e o store (não os copia),
usa `importlib.resources` (dentro de um `.pyz` não existe diretório ao lado do
módulo) e **confere o cabeçalho `Host`** — loopback sozinho não impede DNS
rebinding.

**loja** (Rust + SDL2): `PacoteVerificado` prova origem; `LicencaAssinada` prova
compra e exige a chave do aparelho — **tipos diferentes**, e é o sistema de tipos
que impede "autêntico" virar "comprado". A loja **não vê cartão** (checkout
hospedado), **não se declara paga** e **não inventa código PIX**. Uma função
decide a grade (`grade_para` + `mover`, com teste de propriedade) e o desenho e a
navegação leem dela.

Completo: [`docs/historico/pc-e-pacote.md`](docs/historico/pc-e-pacote.md),
[`docs/pc.md`](docs/pc.md), [`docs/loja.md`](docs/loja.md).

## 10. O pacote `.giochi`

```text
app/<slug> ──empacotar──▶ <slug>.giochi (+.sig +.manifest.json)
            ──instalar-pacote──▶ /user/apps/<slug>
            ──catálogo──▶ cartão na tela ──▶ o app
```

**O `tar` é escrito à mão** (`scripts/lib/tar.mjs`) por três motivos medidos:
ordem de contrato, reprodutibilidade (`mtime`/`uid`/`gid` fixos — mesmo SHA-256
em duas rodadas) e bit de execução (`tar --mode` mascara permissão, e o erro só
apareceria no aparelho como "entrypoint não existe"). Verificado contra o GNU
tar; sem isso seria o meu parser concordando com o meu escritor.

**App da loja vai para `/user/apps`, não para `/opt`** — gravar em `/opt`
destrói o argumento que permite vender app GPL.

**A assinatura tem um formato só**: o `.sig` tem 64 bytes crus, sem base64 nem
`\n`. Os dois lados já divergiram (Python 64 bytes, JS base64 de 89) e os testes
passavam porque assinavam e verificavam com a mesma biblioteca — a prova que
faltava é externa (`openssl pkeyutl -verify`). No aparelho,
`verificador-ed25519` confere **tamanho** e **tipo de chave** antes de verificar.

**Integridade não é autenticidade**: quem escreve no cartão reescreve o
`.giochi` **e** o manifesto, e o hash confere.

Completo: [`docs/historico/pc-e-pacote.md`](docs/historico/pc-e-pacote.md),
[`docs/pacote.md`](docs/pacote.md).

## 11. Verificação no aparelho

```bash
bash scripts/aparelho.sh procurar     # 10.55.0.2 (usb0), depois varre a /24
bash scripts/aparelho.sh diagnostico  # sistema, tela, áudio, entrada, libs
bash scripts/aparelho.sh bluetooth    # a pilha inteira, com veredito
bash scripts/aparelho.sh instalar     # compila, instala, confere sha256
bash scripts/aparelho.sh subir        # inicia o launcher (o init não faz sozinho)
bash scripts/aparelho.sh verificar    # processo + DRM + frame comparado
bash scripts/aparelho.sh capacidades  # códigos que cada /dev/input declara
bash scripts/aparelho.sh catalogo     # gera e publica o índice de apps
bash scripts/aparelho.sh boot-log     # salva o log ANTES de regravar o cartão
```

**`verificar` é o que importa**: para o app (o KMSDRM é exclusivo), captura um
quadro, desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`, des-rotaciona e
compara pixel a pixel. Rotação oposta cai para **10,9 %** — é essa diferença que
distingue "desenhou" de "desenhou de ponta a cabeça".

**Cinco coisas do ambiente precisam ser congeladas dos dois lados**, senão a
comparação reprova por hardware: relógio (`GIOCHI_HORA_FIXA`), bateria
(`GIOCHI_BATERIA_FIXA`), temperatura (`GIOCHI_TEMPERATURA_FIXA`), **idioma**
(`--idioma en`, que não grava) e o **catálogo** — a referência agora inclui a
colheita das quatro raízes, porque a detecção é disco-primeiro (§8) e um pacote
fora do `index.json` (app da loja, cartão de port) não existiria no PC.

Completo: [`docs/historico/verificacao-e-ambientes.md`](docs/historico/verificacao-e-ambientes.md).

## 12. Ambientes medidos

| | X55 | X35H | R35S |
| --- | --- | --- | --- |
| SoC / GPU | RK3566 / Mali-G52 | RK3566 / Mali-G52 | RK3326 / Mali-G31 |
| painel | 720×1280 | 640×480 | 640×480 |
| `fbcon/rotate` | `0` + cmdline `3` → **Ccw90** | `0` → None | `0` → None |
| RAM | 1968 MB | 964 MB | 964 MB |
| acesso | `usb0` `10.55.0.2` + Wi-Fi | `usb0` + rede | `usb0` ou serial |
| `KEY_POWER` | sim (`rk805`) | sim | **não** |
| Wi-Fi / BT | sim | sim | **sem rádio** |
| `libasound` | sim | sim | **sim** na imagem de 15/11 |

**O R35S mudou na imagem de 15/11/2026**: `libasound` presente, SDL2 2.28.5 +
`SDL2_mixer` 2.6.3 + `SDL2_image` 2.0.8, 17 códigos de entrada — e `KEY_POWER`
**continua** não declarado.

**Não confie no nome do aparelho nem no IP.** Leia
`/etc/giochi/build-info.txt`; o `10.55.0.2` já foi X35H e R35S no mesmo dia, e a
host key muda ao regravar (`ssh-keygen -R 10.55.0.2` — a mensagem fala de ataque
MITM e não é). O IP do Wi-Fi é DHCP e muda entre boots.

Sem `usb0`, a console serial é o único caminho:
`bash scripts/console-r35s.sh "uname -r"` / `--escutar 12`.

Completo: [`docs/historico/verificacao-e-ambientes.md`](docs/historico/verificacao-e-ambientes.md),
[`docs/r35s.md`](docs/r35s.md), [`docs/teste-x35h.md`](docs/teste-x35h.md).

## 13. Armadilhas medidas

Cada uma custou pelo menos uma rodada. Nenhuma é hipótese. **Uma linha por
armadilha aqui; o caso completo em
[`docs/historico/armadilhas.md`](docs/historico/armadilhas.md).**

| Armadilha | Em uma linha |
| --- | --- |
| ferramenta que mede a intenção | contador contava chamadas dentro do módulo; `--teclas` aplicava antes de `Screen::Running`; `--tamanho` não conferia o drawable; `verificar-efeitos.sh` imprimia "ok" depois de FALHA |
| teste que nunca falhou | não provou nada: **todo teste novo é conferido revertendo a correção** |
| processo que procura a si mesmo | `case "$c" in *giochi-init*)` casa com o próprio `sh -c`; comparar por caminho exato ou `readlink -f /proc/PID/exe`. E `pkill -f giochi-launcher` **mata a sessão SSH** — `pidof` + `kill` |
| `ps` mente nesta imagem | listou 2 de 94 processos, entre eles `init`, `sshd` e o launcher. Leia `/proc/*/cmdline`; `pidof` não acha script `sh` |
| "vivo" ≠ "desenhando" | um launcher pode estar vivo **sem** os descritores de `card0`/`renderD`. A pergunta é "quem segura o DRM" |
| o dono do DRM pode ser o app | com `fork`+`wait` sem DRM, matar o launcher deixa o app **órfão com o dispositivo** |
| `giochi-init` desiste | `RESTART_MAX=5` quedas em 60 s e ele dorme em `sleep 3600`. **Seis `verificar` seguidos deixam o aparelho sem interface** |
| `SEM_DRM` era aviso, não falha | o caminho de recuperação nunca era alcançado; e `/run/giochi/launcher-parado` nunca era apagado |
| `SIGTERM` não mata o laço | o SDL só posta `SDL_QUIT` na fila: **`kill -9`**. Corrigido em 26/11/2026 **na janela do filho** (§6.11) — antes dela segue valendo, e `timeout 14 launcher --lancar X` agora termina |
| descritor aberto ≠ DRM ocupado | o `giochi-ports` pergunta por `/proc/*/fd`→`card0`, e o `card0` do launcher é da libmali: **soltar o `VideoSubsystem` do SDL não fecha** (medido: `3 -> /dev/dri/card0` continuava). O caminho é morrer de `SIGTERM` (§6.11) |
| teste que arma `SIGTERM` em paralelo | `SIGTERM` é global ao processo: dois testes que armam/restauram o handler se atropelam e o `raise` de um pega o default do outro — **a suíte morre com sinal 15**. Um teste sequencial só |
| estado que sobra e mente | `.pugwash-reboot` de uma execução anterior fez o `pugwash` pular a GUI e sair; o cartão "abria" e nada aparecia. A limpeza é do cartão (§6.11), não do usuário |
| amostrar com `sleep` mentindo o tempo | o laço de amostragem gastava segundos por volta e os rótulos `+42s` eram ficção; a loja rodava 60 s. Medir com `date +%s`, não com contador de laço |
| `timeout` na ponta do host | não mata o processo remoto: matar o `ssh` deixa o comando rodando no aparelho |
| BMP de 32 bpp | calcular o *stride* como `ceil(largura*bpp/4)*4`; ler como 24 bpp invalida a rodada. Girar 90° e voltar tem de dar 100,00 % |
| `--dump-frame` grava preto | com backend acelerado o swap esvazia o buffer: use `GIOCHI_GLES=0` |
| inversa de `girar_rect` | não é "girar para o outro lado" — as dimensões trocam. Errar dá uma **transposição**, invisível em quadro quase simétrico |
| `ssh` dentro de `while read` | come o stdin: só a primeira linha é processada (foi assim que a conferência examinou **só o primeiro app**). `</dev/null` + `read <&3` |
| `sh -c` remoto com aspas duplas | expande no PC e vira `e: variável não associada`: aspas simples com `__MAC__` |
| build `--no-default-features` | grava **no mesmo caminho** o binário sem texto, e a comparação passa a medir um launcher mudo (95,6 %). Rebuild antes de comparar |
| `--instalar <ip>` sem usuário | vira `charles@<ip>` e a mensagem manda procurar chave SSH |
| dois significados para a mesma variável | `INSTALAR` bare + `HOST_SSH` com `root@` produziu `root@root@10.55.0.2`. Hoje **uma função** decide e todo `ssh`/`scp` a consulta |
| a referência vinha de binário velho | `--testes` compila harness, o clippy só faz type-check: 92,4 % sem regressão nenhuma. Hoje o script reconstrói quando um `.rs` é mais novo |
| `u16` de sentinela × `Option` | `None != Some(0)` é **sempre verdadeiro**: a linha de energia saía a cada 5 s num aparelho sem sensor |
| flag de diagnóstico sobrescrita | `--idioma pt` era gravado e o `App::new` lia o arquivo por cima. A correção não foi reordenar: virou **parâmetro** (`idioma_forcado`) |
| ferramenta que não alcança a tela | `--tela senha` empilhava o overlay sem `focus_on` e o `↓` caía no carrossel |
| `is_ok()` mede a variável, não o estado | `GIOCHI_GLES=0` **ligava** o GLES. Hoje são três estados e o `Painter::new` **anuncia** o backend real |
| ferramenta que não está instalada | `modinfo`, `timeout`, `evtest --query` devolvem vazio, e vazio lido como "não tem" virou pedido errado para outra equipe |
| nome de símbolo é detalhe de versão | `hid_register_driver` é macro; o exportado é `__hid_register_driver`. Procurar o nome plausível devolve vazio |
| estado observado ≠ defeito | `/sys/bus/hid/drivers/hidp` **nunca** existiria (é transport driver). Perguntar se o estado é o esperado antes de chamá-lo de defeito |
| comparação exata de quadro | acusa antialiasing de glifo: 98,30 % exato contra **100,0 %** na régua do projeto (tolerância 12, amostra 3×3) |
| aviso de compilador que ninguém lê | 21 avisos acumulados e zero no CI; um deles era `i as u8 * 32`, que **estoura na nona faixa**. CI com `-D warnings` |
| `cfg` que nunca é compilado | `--no-default-features` ficou quebrado por semanas; o CI constrói as duas combinações |
| CI: `cancelled` sem passos | é infraestrutura: `macos-14` foi aposentado. Verificação de CI não lê texto acentuado, e `$array -notmatch "x"` **filtra** em vez de testar |
| `exit 0` no fim de um passo `pwsh` | não é redundância: o runner **anexa** `exit $LASTEXITCODE` e o passo fica vermelho por um sucesso que ele mesmo provocou |
| filtro de `paths` faz parte do teste | `app/painel/**` fora do filtro = app sem CI, e o sintoma é silêncio |
| formulário de issue não é garantia | conferir antes do clique devolve o passado: nasceram três issues duplicadas de PortMaster (#3–#5). O caminho é a URL do `issues/new` |

## 14. O que está provado e o que é afirmação

| Alegação | Como |
| --- | --- |
| launcher roda no aparelho | **medido**: KMSDRM, 100,0 % contra a referência do PC |
| rotação segue o sistema | **medido** nos três aparelhos, com o `cmdline` desempatando o `0` |
| botões do controle embutido | **medido**: cobertura completa dos 20 códigos do X55 |
| brilho, volume, dormir, desligar, reiniciar | **medido** no X55 |
| senha Wi-Fi: digitar, ver e confirmar | **medido** — o caminho de produção `--conectar` conectou no X55 |
| Wi-Fi sobrevive ao reboot | **medido**: voltou sozinho para a rede, com IP, sem digitar senha |
| boot automático (`::respawn:` → init → launcher) | **medido** no X55: `ppid` do launcher é o init, 29 s depois do boot |
| Bluetooth: parear | **medido** (`Bonded: yes`) |
| Bluetooth: virar entrada | **corrigido no config, imagem não medida** — o fragmento da x55 já traz `CONFIG_UHID=y` e `CONFIG_BT_HIDP=m` (conferido no remoto em 26/11/2026); falta uma imagem construída com ele e o teste ponta a ponta |
| Star Dust desligado = launcher idêntico | **medido**, 4 tamanhos, byte a byte |
| launcher-teste 100 % pixel a pixel | **medido**, 4 tamanhos + 6 telas |
| `.pyz` e loja nos três sistemas | **medido** no CI (7 e 4 jobs verdes) |
| assinatura verificada no aparelho | **medido**: 20 MB de RSS, 0,01 s |
| launcher multi-idioma (10 idiomas) | **medido** no R35S: boas-vindas na 1ª abertura, `en -> pt` pelo caminho real, Home `pt` **99,6 %**, `verificar` **100,0 %** |
| escolha do idioma por **botão físico** | **medido**: um toque no controle gravou `en\n`, e a execução seguinte abriu sem perguntar |
| cobertura de fonte por idioma | **medido** com `--fontes` (`TTF_GlyphIsProvided`): `pt/ru/ar` completos, `zh` 5/8, `hi` 1/11, `bn` 1/10 — falta fonte na imagem |
| ports do PortMaster no launcher | **medido** no R35S em 26/11/2026: cartão criado, detectado e **lançado**, com o FORM rodando dentro do `bwrap` — inclusive o **ciclo inteiro**: launcher sai por `SIGTERM`, o port abre com `opengles2`, e o supervisor devolve a interface quando ele termina (§6.11) |

**Regra de honestidade:** nada aqui pode ser declarado "funciona" sem ter sido
executado. "Compila" e "roda" são afirmações diferentes — e o `poweroff` já caiu
por ter sido escrito antes de testado.

**Existir, declarar e ser usado são fatos diferentes.** Uma `.so` no rootfs não
prova que o app a usa (só `readelf -d` mostra as `DT_NEEDED`); um `/dev/input`
declarado no `/proc` não prova que o evento chega; uma variável de ambiente
documentada não prova que o código a lê.

## 15. Arquivos que não devem ser commitados

`dist/`, `out/`, `*.giochi`, `node_modules/`, `.godot/`, `.DS_Store`, `.vscode/`,
`.idea/`, `target/` e `__pycache__/`.

**`bin/` deve ser commitado** — é exatamente o que vai para o aparelho.
**`app/launcher/Cargo.lock` também** — o launcher é binário e o lock fixa as
versões que foram de fato compiladas; o CI usa `--locked`.

## 16. O que falta

| Item | Estado |
| --- | --- |
| app `image` que não seja shell | **existe**: `musica` (§7) |
| ports do PortMaster na aba Apps | **feito** (26/11/2026) — cartão por port, lançado por `giochi-ports` (§6.11) |
| a GUI do PortMaster na aba Apps | **feito** (26/11/2026) — cartão `portmaster`, com crédito no título/metadata/script, aberto por `giochi-ports --liberar-drm` (§6.11) |
| PortMaster como item da **loja do PC** (`app/store`), declarado **sideload** | **não feito** — o pacote não é assinado (GPL-2.0/LGPL-2.1 dentro) e precisa aparecer como tal na interface |
| aba "Ports" própria e `args` no `Alvo` | **não feito** — é UX; a aba Apps já mostra os ports |
| migrar jogos de `/user/apps` para `/user/games` | **manual** — feito no R35S em 15/11/2026; um `scripts/migrar-raizes.sh` evitaria repetir |
| barra de progresso no app de música | **não existe** — o `SDL_mixer` não informa a posição; a barra mostraria número inventado |
| duração de MP3/OGG/FLAC no app de música | **`—`** — só o WAV declara o tamanho no cabeçalho |
| licença consultada antes de lançar | **não feito** — o verificador existe, o launcher ainda não pergunta ao servidor |
| conexão Wi-Fi real | **feita e persistente** (15/11/2026) — falta o `update_config=1` na imagem (§17) |
| pareamento BT virar entrada | **bloqueado na imagem** — pedido no canal (§17) |
| analógico | **não lido** — o controle virtual de 18/11/2026 entrega `-32767..32767` e dispensa o ioctl; falta implementar (§6.3) |
| controle virtual `Giochi Gamepad` | **documentado, não implementado** — [`docs/controle-virtual.md`](docs/controle-virtual.md) |
| tema em tempo de execução | **a base existe**; falta o **segundo** tema (§6.10) |
| idioma | **feito**: os dez do site, padrão inglês (§6.10) |
| fonte para `zh`/`hi`/`bn` | **falta na imagem** — pedido nº 5 de `docs/issues-para-outros-repos.md` |
| shaping de `ar`/`ur` | **funciona**; bidi de parágrafo e layout espelhado **não existem** |
| teclado de senha em outros alfabetos | **não existe** — `KEYBOARD` é QWERTY latino fixo |
| `bash -n` no `build-device.sh` | fica no **Giochi System**, outro repositório |

## 17. Canal com o Giochi System

A conversa entre as equipes vive em `/home/charles/Giochi/chat_de_equipe.txt` —
**fora dos dois repositórios**. Aqui há só um ponteiro:
`docs/mensagem-giochi-system.md`.

**Janela: 08:00–18:50, horário de Brasília.** `bash scripts/canal.sh` responde, e
`--exigir` sai com 1 para encadear em script.

Pedidos abertos em [`docs/issues-para-outros-repos.md`](docs/issues-para-outros-repos.md),
com medição, arquivo e linha, consequência e como conferir:

| # (doc) | Repo | Pedido | Estado medido em 26/11/2026 |
| --- | --- | --- | --- |
| 1 | System | `CONFIG_UHID=y` (BLE) + `hidp` no boot | **atendido**: `CONFIG_UHID=y` e `CONFIG_BT_HIDP=m` no fragmento da x55, e as três issues fechadas. **Não reenviar** |
| 2 | System | `update_config` no **caminho de provisionamento** do Wi-Fi | **aberto** — `update_config` tem 0 ocorrências no repo remoto; é o mais grave |
| 3 | System | raízes `/opt|/user/games` e `giochi-selftest` contando o binário | **aberto** — `giochi-selftest:1128-1131` inalterado |
| 4 | OS | `runAppsPhase` escolher a raiz por `kind` e gravar `kind` no índice | **aberto** — `pipeline.mjs` põe todo `embedded` em `opt/giochi/apps`, sem `kind` |
| 5 | System | fontes de CJK/devanágari/bengali | **já é a issue `GIOCHI-SYSTEM#5`**, aberta. Não reenviar |
| 6 | System | `giochi-ports` sem `PYSDL2_DLL_PATH` e sem limpar o `.pugwash-reboot`: a GUI do PortMaster não abre | **aberto** (26/11/2026) — o app contorna no cartão da loja, mas a correção é do wrapper |

Os pedidos **2**, **3** e **4** foram entregues em 26/11/2026 pelo formulário do
`issues/new` já preenchido, aberto no navegador da máquina
(`node scripts/abrir-issue.mjs {2,3,4} --abrir`). O número entra nesta tabela
**só quando a issue existir**: anunciar "issue criada" sem ter criado é o §10
aplicado à conversa entre equipes.

**O número do documento não é o número da issue**, e essa confusão já é o
primeiro defeito da tabela: as issues **deste** repositório (#2–#5) são o pedido
de PortMaster, hoje respondido em [`docs/portmaster.md`](docs/portmaster.md), e
as dos repos vizinhos têm numeração própria. Use "pedido N do documento" para o
documento e `#N` só para o GitHub deste repositório.

**O pedido 2 corrige um pedido anterior meu**: `/etc/wpa_supplicant.conf`
**existe** na imagem (é o template do Buildroot) e a receita de provisionamento
de `docs/x55-bringup.md` o **trunca** com `>`, apagando `ctrl_interface` e
matando o `wpa_cli`. A correção é no caminho de provisionamento, preservando o
template.

Estado do mecanismo de issue, medido: o MCP do GitHub autentica, mas
`issue_write` renderiza um formulário e o agente não clica — e o formulário
**pode não aparecer**. Por isso o entregável é o corpo pronto + a URL do
`issues/new` (§1.1).

Completo: [`docs/historico/pendencias-e-canal.md`](docs/historico/pendencias-e-canal.md).
