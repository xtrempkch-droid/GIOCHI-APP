# AGENTS.md — Giochi app

> Handoff técnico deste repositório. Leia antes de continuar o trabalho.
> Atualizado em: 03/11/2026 (com acesso ao aparelho real).

## 1. Objetivo

Este repositório é a **camada de experiência** do projeto Giochi: os apps e jogos
que rodam sobre o sistema gerado pelo **Giochi System**, e que são reunidos com ele
no **Giochi os**.

| Repositório | Função |
| --- | --- |
| **Giochi app** (este) | Apps, jogos, arte e manifesto de requisitos |
| **Giochi System** | Sistema Linux limpo: kernel, rootfs, boot, drivers, painel |
| **Giochi os** | Junta sistema + apps e gera a imagem final por aparelho |

Cada pasta é **repositório Git individual**. A comunicação entre elas é sempre por
**artefato declarado explicitamente**, nunca por caminho relativo presumido.

## 2. Localização

```text
C:\Users\Jussara dias\Desktop\plataforma sistema\Giochi app
```

Repositório no GitHub: `https://github.com/xtrempkch-droid/GIOCHI-APP` (a criar).

## 3. Estado atual

### Concluído

- Estrutura de repositório pronta para GitHub.
- Pasta `app/` com um diretório por app, contendo `src/`, `bin/` e `assets/`.
- `app/apps.json` como fonte da verdade da lista e do status.
- `app/_template/` para criar app novo.
- JSON Schema do `metadata.json` e do `forge.json` em `schemas/`.
- Validador: `scripts/validate-app.mjs` + núcleo em `scripts/lib/apps.mjs`.
- Leitor de dimensão de PNG sem dependência: `scripts/lib/png.mjs`.
- Script de migração do antecessor: `scripts/migrar-do-antecessor.sh`.
- Testes: `tests/validate-app.test.mjs`, runner `tests/run-all.mjs`.
- Workflow de validação em PR e push.
- Documentação em `docs/` (5 páginas).

### Validado nesta sessão (03/11/2026, Linux com toolchain completa)

- `node scripts/validate-app.mjs --all` passou: **7 apps**, todos em `scaffold`.
- `node tests/run-all.mjs` passou: **39/39**.
- `cargo build --workspace` em `app/launcher`: **compila e linka**, zero erro e
  zero aviso (rustc 1.99, SDL2 2.32.10, SDL2_ttf 2.24.0).
- `cargo test --workspace` em `app/launcher`: **63/63** (48 no `core`, 15 no
  `ui-sdl`).
- O launcher **roda**: Home desenhada de verdade e gravada em BMP com
  `--dump-frame`, sem tela (`SDL_VIDEODRIVER=dummy`).
- `bash -n scripts/migrar-do-antecessor.sh` e `bash -n
  scripts/compilar-launcher.sh`: **sem erro de sintaxe**.
- O workflow ganhou `launcher-core` (testa os 48 do núcleo, sem SDL) e
  `launcher-build` (compila o workspace com SDL2). Os dois comandos passam
  localmente com `--locked`; o YAML foi parseado, mas os jobs **não rodaram no
  GitHub** ainda.

### Verificado NO APARELHO (03/11/2026)

Aparelho `giochi-system-x55`: Buildroot 2024.02, aarch64, kernel 6.12.0,
glibc 2.38, SDL2 2.28.5, painel DSI 720×1280, 1968 MB de RAM.
Acesso por `ssh root@192.168.1.6`.

- **Cross-compile aarch64 funciona**: `scripts/compilar-launcher-arm64.sh`
  gera o binário do zero (sysroot + libs do aparelho + wrapper de link).
- **O launcher RODA no aparelho**: `SDL_VIDEODRIVER=kmsdrm` abre o DRM real,
  desenha e grava frame; o laço ficou 6 s vivo sem crash.
- **Rotação para painel retrato**: antes ~40 % da tela era letterbox preto e a
  UI saía deitada; agora 0 % de preto e o layout encaixa.
- **Instalado** em `/opt/giochi/apps/launcher/bin/giochi-launcher`, o caminho
  que o `giochi-init` espera, com sha256 idêntico ao do repositório.
- **O `inittab` do aparelho NÃO inicia o `giochi-init`** — não há entrada
  `::respawn:`. O launcher nunca sobe no boot; só por SSH. É do **Giochi
  System**, registrado em `docs/dependencias-aparelho.md`.
- `app/launcher/bin/giochi-launcher` versionado; o app passou a **`ready`**.
- `capa.png` 320×320 gerada da paleta do tema (provisória).
- **Corrigido o sentido da rotação**: o primeiro teste no aparelho mostrou a UI
  de cabeça para baixo. Horário e anti-horário diferem 180°; o X55 precisa de
  anti-horário. Confirmado por comparação de frame: 99,9 % igual à referência.
- **Config de rotação com recarga ao vivo**: o arquivo é relido a cada segundo,
  então trocar o sentido não exige reabrir o launcher.
- **Corrigido um bug que impedia os botões de funcionar**: o
  `/etc/giochi/buttons.map` do aparelho tem comentário no fim de cada linha
  (`up = BTN_DPAD_UP  # gpio4 PA0`), e o parser não cortava o `#`. Resultado:
  **1 de 13** botões parseados, os outros 12 descartados em silêncio. O aviso
  "mapa de botões incompleto" no console era o único indício. Agora corta e
  tem teste com o conteúdo real do mapa.

**O que ficou por confirmar nesta sessão:** o aparelho saiu da rede logo depois
da instalação (`No route to host`, e um scan da /24 não o encontrou). O `scp`
para `/opt/giochi/apps/launcher/bin/` concluiu e o `ls` remoto confirmou os
432 KB, mas **não deu para executar o binário a partir desse caminho**. O
executável testado foi o mesmo arquivo em `/tmp`, byte a byte. Ao retomar:

```bash
ssh root@<ip> 'SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher --dump-frame /tmp/x.bmp'
```
- O validador foi exercitado contra: versão fora de semver, runtime inválido,
  arquitetura inválida, campo desconhecido, entrypoint fora de `bin/`, entrypoint
  com `..`, entrypoint inexistente, capa fora de 320×320, capa que não é PNG,
  `bin/` ausente, nome de pasta inválido, `forge.json` inválido, licença em formato
  inválido e **GPL declarada na imagem assinada**.

### Ainda pendente

- **Código-fonte dos apps não foi migrado.** Nenhum `src/` tem conteúdo. O script
  existe e passa por `bash -n`, mas **nunca rodou de verdade** — falta um clone do
  antecessor para apontar.
- **Nenhum `bin/` tem conteúdo.** Os jogos são projetos Godot e precisam de export
  ARM64; as ferramentas são Python e precisam ser empacotadas.
- **Nenhuma `capa.png`** existe: 320×320 não foi gerado para nenhum app.
- **Não há empacotador `.giochi`**, nem assinatura Ed25519.
- **Texto no aparelho.** Falta `libSDL2_ttf` (e freetype e fonte). O launcher
  roda com `--no-default-features` e desenha tudo menos os glifos. Pedido
  detalhado em `docs/dependencias-aparelho.md`. **É o bloqueio número um.**
- **GLES2 e `/dev/input` no aparelho.** O que rodou foi o renderer *software*
  sobre KMSDRM, e nenhum botão foi exercitado. Há 5 dispositivos
  (`rk805 pwrkey`, `adc-joystick`, `rk817_ext Headphones`, `gpio-keys-control`,
  `gpio-keys-vol`).
- **Desempenho no RK3326** não medido.
- **`bash -n` no `build-device.sh`** — fica no **Giochi System**, fora deste
  repositório, e continua sem verificação.

## 4. Estrutura

```text
Giochi app/
├── app/
│   ├── apps.json              # fonte da verdade: id, pasta, runtime, status, distribution, license
│   ├── _template/             # modelo para app novo
│   ├── forge/                 # empacotador/classificador (host, roda no PC)
│   ├── store/                 # loja (host, roda no PC)
│   ├── arcade-volleyball/     # jogo padrão (image, godot)
│   ├── colossal-cave-adventure/   # jogo padrão (image, godot)
│   ├── spacewar/              # jogo padrão (image, godot)
│   ├── shattered-pixel-dungeon/   # GPL-3.0 (store, native)
│   └── launcher/              # Rust + SDL2 (image, native)
│       ├── Cargo.toml         # workspace: crates/core + crates/ui-sdl
│       ├── crates/core/       # núcleo puro, sem SDL
│       └── crates/ui-sdl/     # janela, desenho, evdev
├── schemas/
│   ├── metadata.schema.json
│   └── forge.schema.json
├── scripts/
│   ├── validate-app.mjs
│   ├── migrar-do-antecessor.sh
│   ├── compilar-launcher.sh   # resolve SDL2 e compila o launcher
│   └── lib/
│       ├── apps.mjs
│       └── png.mjs
├── tests/
│   ├── validate-app.test.mjs
│   └── run-all.mjs
├── docs/
│   ├── README.md
│   ├── adicionar-app.md
│   ├── migracao.md
│   ├── metadata.md
│   ├── forge.md
│   ├── pacote.md
│   ├── licenciamento.md
│   ├── controles.md
│   ├── integracao.md
│   ├── diagnostico-prototipo.md
│   └── compilar-launcher.md
└── .github/workflows/validate.yml
```

### Anatomia de um app

```text
app/<slug>/
├── metadata.json     # identidade do pacote (obrigatório)
├── forge.json        # requisitos de hardware (opcional)
├── capa.png          # 320x320, capa da loja
├── thumb.png         # opcional
├── src/              # CÓDIGO-FONTE: projeto Godot, Python, C++
├── bin/              # O QUE VAI PARA O APARELHO
├── assets/           # ARTE-FONTE: logos, fontes, arquivos editáveis
└── README.md
```

**A distinção `src/` × `bin/` × `assets/` é a decisão estrutural mais importante.**
`src/` é o que se edita, `bin/` é o que o aparelho executa, `assets/` é material de
trabalho que não viaja. Confundir isso gera pacote de centenas de megabytes com
arquivos editáveis dentro.

## 5. Contratos

### `app/apps.json`

```json
{
  "schemaVersion": 1,
  "apps": [
    { "id": "forge", "folder": "forge", "runtime": "native", "status": "scaffold" }
  ]
}
```

- `id` — slug kebab; é a identidade estável.
- `folder` — pasta real em `app/`.
- `runtime` — `godot`, `retroarch`, `proton` ou `native`.
- `status` — `scaffold` ou `ready`.
- `distribution` — `image`, `store` ou `host` (ver abaixo).
- `license` — identificador SPDX ou `Proprietary`; precisa bater com o `metadata.json`.

### `distribution` decide o destino e a licença

| Valor | Onde roda | Efeito no licenciamento |
| --- | --- | --- |
| `image` | dentro da imagem assinada do aparelho | exige licença permissiva |
| `store` | instalado em `/user`, enviado pela loja | aceita GPL |
| `host` | no **computador do usuário** (loja, forge) | não vai para o aparelho |

**A loja roda no PC do usuário**, não no aparelho: ela baixa/paga o pacote e envia
pronto para o dispositivo. Decisão do usuário em 02/11/2026.

Isso é o que permite vender app GPL: o pacote baixado não faz parte da imagem
assinada, e o comprador pode substituí-lo em `/user` — exatamente o procedimento
sem chave que a GPL-3.0 §6 exige. O validador **recusa mecanicamente** GPL/AGPL/LGPL
com `distribution: "image"` (código `licenca_gpl_na_imagem`).

**`scaffold`** significa: existe na árvore, mas a fonte não foi migrada e/ou o
binário não foi exportado. Nesse estado a ausência de `entrypoint` vira **aviso**,
não erro — senão o CI nasce vermelho e ninguém olha mais. Todo o resto continua
sendo erro.

**`ready`** significa: deve estar completo. Qualquer erro reprova.

Pasta em `app/` que não esteja em `apps.json` reprova com `app_nao_registrado`:
um app invisível para a loja é pior que um app quebrado.

### `metadata.json`

Campos obrigatórios: `title`, `developer`, `version`, `arch`, `runtime`,
`entrypoint`. `additionalProperties: false` — campo desconhecido reprova.

Semver obrigatório (`1.0.0`, não `1.0`). `arch` só aceita `aarch64`.

### `forge.json`

Requisitos de capacidade: `arch`, `runtime`, `graphics_api_min`,
`graphics_api_fallback`, `godot_version`, `ram_min_mb`, `vram_min_mb`,
`display_min_width/height`, `touchscreen_required`. Todas opcionais.

**Nunca declara `BR2_PACKAGE_*`.** O Buildroot descarta símbolo inexistente em
silêncio; um app que declarasse pacote produziria build verde com biblioteca
ausente. As dependências de pacote ficam no `Config.in` do **Giochi System**.

## 6. Comandos

```bash
node scripts/validate-app.mjs --all       # valida todos
node scripts/validate-app.mjs app/store   # valida um
node scripts/validate-app.mjs --all --json
node tests/run-all.mjs                    # 39 testes
bash scripts/migrar-do-antecessor.sh <clone>   # migra do antecessor

bash scripts/compilar-launcher.sh         # build do launcher (x86-64), resolve o SDL2
bash scripts/compilar-launcher-arm64.sh   # cross-compile para o aparelho
bash scripts/compilar-launcher-arm64.sh --instalar root@192.168.1.6
cd app/launcher && cargo test --workspace  # 63 testes do launcher
```

`compilar-launcher.sh` existe porque o `sdl2-sys` precisa de `pkg-config` para
achar `sdl2`/`SDL2_ttf`: sem elas o **link** falha, mesmo com o `rustc`
compilando sem erro. Em máquina sem root o script extrai os `.deb` num prefixo
local. Passo a passo e armadilhas em `docs/compilar-launcher.md`.

Não use `node --test tests/`: o runner spawna um processo por arquivo e sob
sandbox falha com `spawn EPERM`. O runner próprio existe por isso.

## 7. Decisões (02/11/2026)

- **Pasta `app/` (singular)** — escolha do usuário.
- **`src/` para código-fonte** — acrescentada ao template ao descobrir que os jogos
  são projetos Godot, não binários. Sem ela, o projeto iria para `assets/` (que não
  viaja) ou `bin/` (que é artefato).
- **`runtime: "native"` acrescentado ao enum** — o antecessor registrou a lacuna
  (`LOJA.md` §6): com só `godot`/`retroarch`/`proton`, as ferramentas em Python
  (forge, store) não tinham como se declarar.
- **`status: scaffold`** — espelha o padrão `draft` já usado no Giochi System
  (`device/devices.json`). Evita CI vermelho permanente durante a migração.
- **Launcher não vem** — será redesenhado por completo. Trazer o código atual só
  criaria trabalho de remoção.
- **Shattered Pixel Dungeon vem, mas só pela loja** — GPL-3.0 com `distribution:
  "store"`. A análise do antecessor está preservada em `docs/licenciamento.md`: o
  problema não é contaminação (§5), é *anti-tivoization* (§6) contra rootfs
  assinado. Em `/user` o app é substituível e a obrigação se resolve.
- **Licença do repositório: MIT — decidido, sem troca.** O objetivo era distribuir o
  sistema e vender apps; MIT já permite os dois. Exclusividade sobre o sistema foi
  descartada conscientemente: aceita-se que terceiros possam reaproveitar o código.
  Não reabra essa discussão sem o usuário pedir.
- **Licença dos apps próprios: `Proprietary` — decidido.** O comprador não pode
  redistribuir. É o que dá base para cobrar pelo binário.
- **Migração por script, não por cópia manual** — os projetos Godot têm assets
  binários; copiar por API produziria árvore incompleta e sem histórico.

## 8. O que falta para um app ser instalável

```text
src/  (projeto Godot)  ──export ARM64──►  bin/  ──empacotar──►  <slug>.giochi
```

Nenhuma das três etapas funciona hoje:

1. **Export ARM64** — exige template de exportação do Godot para `linux arm64`.
2. **Empacotador `.giochi`** — formato documentado em `docs/pacote.md`, script não
  existe. Precisa respeitar a ordem `metadata.json → capa.png → thumb.png → bin/`,
  porque o extrator no aparelho lê em streaming.
3. **Assinatura** — Ed25519 + SHA-256, herdada do antecessor. Não implementada.

## 9. Como retomar

```bash
node scripts/validate-app.mjs --all
node tests/run-all.mjs
bash scripts/compilar-launcher.sh
cd app/launcher && cargo test --workspace
```

Para migrar as fontes do antecessor:

```bash
git clone https://github.com/xtrempkch-droid/GIOCHI-KONSOL-OS.git ~/giochi-antigo
bash scripts/migrar-do-antecessor.sh ~/giochi-antigo
node scripts/validate-app.mjs --all
```

## 10. Arquivos que não devem ser commitados

`dist/`, `out/`, `*.giochi`, `node_modules/`, `.godot/`, `.DS_Store`,
`.vscode/`, `.idea/` e `target/` (build do Rust).

**`bin/` deve ser commitado** — é exatamente o que vai para o aparelho.
**`app/launcher/Cargo.lock` também** — o launcher é binário, e o lock fixa as
versões que foram de fato compiladas.

O `.gitignore` reflete isso. Ele já esteve errado: `target/` estava listado aqui
mas ausente do arquivo, e o build do Rust entrou como não rastreado.

## 11. Ambiente desta sessão — o que dá e o que não dá para verificar

Sessão de 03/11/2026: **Linux x86-64 com a toolchain completa**. Isto encerra a
"era sem ferramentas" descrita nas versões anteriores deste arquivo.

| Ferramenta | Estado | Consequência |
| --- | --- | --- |
| `node` | 24.22.0 | validador e testes JS rodam |
| `rustc` / `cargo` | 1.99.0 | launcher compila, linka, roda e testa |
| `git` | 2.53.0 | repositório existe, commits possíveis |
| `bash` | 5.3.9 | `bash -n` nos scripts passa |
| SDL2 / SDL2_ttf | 2.32.10 / 2.24.0 | via prefixo local em `~/.cache/giochi-sdl2/root` |
| cross aarch64 | rustup target + sysroot 2.35 + libs do aparelho | `~/.cache/giochi-arm64` |
| **aparelho** | `ssh root@192.168.1.6` | aarch64 real, KMSDRM funciona |

O que **continua sem verificação**:

1. **Alvo aarch64.** Tudo rodou em x86-64. `cargo build --target
   aarch64-unknown-linux-gnu`, GLES2 sobre KMSDRM e `/dev/input` real não foram
   exercitados. É o próximo item de verdade.
2. `bash -n` no `build-device.sh` do **Giochi System** — outro repositório.
3. `scripts/migrar-do-antecessor.sh` — sintaxe ok, execução nunca testada (falta
   um clone do antecessor).

### Regra de honestidade

Nada aqui pode ser declarado "funciona" sem ter sido executado. Estado em
03/11/2026:

- `node scripts/validate-app.mjs --all` → 7 apps OK
- `node tests/run-all.mjs` → 39/39
- `cargo test --workspace` → 63/63
- `--dump-frame` gera BMP de 1280×720 com conteúdo real — a UI desenha

"Compila" e "roda" são afirmações diferentes, e agora as duas valem para x86-64.
Para o aparelho, nenhuma das duas vale ainda.

### Sobre o protótipo HTML

Continua sendo a referência visual e roda sem build. Ele não é o produto: o
produto é o binário Rust.

## 12. Launcher — estado específico

**Stack decidida em 02/11/2026: SDL2 + renderer próprio.** Alternativas
descartadas: egui (desenho custom limitado) e Slint (DSL + runtime grande para
1 GB de RAM). SDL2 já está no Buildroot e o KMSDRM dispensa compositor.

| Item | Estado |
| --- | --- |
| `crates/core` — 6 módulos, 48 testes | **compila, testa e passa** |
| `crates/ui-sdl` — 6 módulos, 6 testes | **compila, linka e roda** |
| `cargo build --workspace` | zero erro, zero aviso |
| Execução real | **roda** — `--dump-frame` grava a Home em BMP |
| Protótipo HTML | **funcional** — `app/launcher/prototipo/launcher.html` |
| Fonte | **DejaVu Sans** — o TTF **não está no repositório**: copiar para `assets/fonts/DejaVuSans.ttf`, ou o binário usa `/usr/share/fonts/...`. |
| **aarch64 no aparelho** | **roda** — KMSDRM real, frame capturado |
| Rotação para painel retrato (720×1280) | **funciona** — anti-horário, 99,9 % igual à referência |
| Execução do caminho instalado | **confirmado** — `/opt/giochi/apps/launcher/bin/`, laço 10 s vivo |
| GLES2 / `/dev/input` / desempenho | **não exercitados** |
| Leitura real de bateria/temperatura/TDP | ausente; a UI mostra `—` |

### Suposições de API do `sdl2` 0.37 — resolvidas

As três anotadas como risco na sessão anterior foram verificadas na compilação:

1. `load_font` devolve `Font<'static, 'static>` — **procede**. O caminho é vazado
   com `Box::leak` para satisfazer o tempo de vida.
2. `GameControllerSubsystem::open` — **não é usado**: a entrada é por evdev.
3. `Canvas::window_to_logical` — **não existe** na 0.37. A conversão foi refeita
   em `Painter::window_to_logical`, com a mesma matemática do
   `SDL_RenderWindowToLogical` (escala uniforme + letterbox centrado), sem
   `unsafe`.

Outros pontos que o compilador cobrou e que mudaram a forma do código:

- `TextureCreator` agora vive dentro do `Painter`. Antes era criado por chamada de
  texto, e a `Texture` empresta dele — manter os dois vivos exigia guardá-lo.
- `Event::TouchFingerDown` não existe; o evento é `Event::FingerDown`.
- O macro de ícone devolvia referência a temporário; virou `static` local.

### Bugs de comportamento que a compilação expôs

Compilar não prova nada sobre o comportamento — estes só apareceram porque os
testes passaram a rodar:

1. **Quatro `use of moved value`** em `reduce.rs` (`AdjustVolume`,
   `AdjustBrightness`, e dois em `nav`). Foi o único motivo de o crate não
   compilar.
2. **`carousel` dessincronizado do foco.** `views` desenha o cartão central por
   `state.carousel`, mas só `nav` mantinha os dois juntos. Trocar de aba, voltar
   de um overlay e tocar num cartão deixavam destaque e desenho divergentes.
   Corrigido com o helper `focus_on`, por onde passam `FocusAt`, `GoTab`,
   `tab_step` e `go_back`.
3. **Toque em cartão lateral abria o jogo.** O protótipo centrava primeiro e só
   abria se o cartão já era o central. O port abria direto — regressão silenciosa
   que nenhum teste pegaria sem compilar.
4. **`rows = 6` no painel rápido** era um número solto; o núcleo declara
   `QUICK_SETTINGS_ROWS = 8` (6 controles + 2 botões). Agora vem da constante.

### Feature `texto`

O crate `ui-sdl` compila em dois modos:

| Build | Feature | Linka contra |
| --- | --- | --- |
| padrão | `texto` | `libSDL2` + `libSDL2_ttf` |
| aparelho hoje | `--no-default-features` | só `libSDL2` |

Sem `texto`, `text_size` devolve altura aproximada para o layout não colapsar e
`text` não desenha nada. Existe porque o aparelho **não tem** `SDL2_ttf` nem
freetype nem fonte — ver `docs/dependencias-aparelho.md`.

### Rotação

Painel é retrato (720×1280) e a UI é paisagem (1280×720).

**O sentido já esteve errado e o sintoma foi "de ponta a cabeça".** Horário e
anti-horário diferem em exatamente 180°, então não há meio-termo: um dos dois
entrega a imagem invertida. O X55 precisa de **anti-horário** (`DEFAULT_ROTATION`).

Precedência: `GIOCHI_ROTATE` → `/user/.config/giochi/rotation` → automático.
O arquivo existe porque o `giochi-init` inicia o launcher sem variáveis de
ambiente; sem ele trocar o sentido exigiria recompilar.

O arquivo é **relido a cada segundo com a tela aberta** (`App::tick_rotation`):
`echo 270 > /user/.config/giochi/rotation` gira na hora e o console registra
`rotação alterada em tempo real: Ccw90`. Foi assim que o sentido foi conferido
sem recompilar entre tentativas. A variável de ambiente, ao contrário, é lida só
na abertura.

Cada primitiva pública mapeia a coordenada **uma vez** e delega para uma versão
`*_dev` em espaço de dispositivo. Sem essa separação, `stroke_round_rect`
chamaria `fill_rect` e giraria duas vezes. O toque desfaz em `unmap_point`.

A matemática (`rotate_point`/`rotate_rect`/`unrotate_point`) é pura e testada sem
SDL — 7 testes, incluindo ida-e-volta, cobertura do painel e a relação de 180°
entre os sentidos. A verificação que vale é no aparelho: frame capturado,
des-rotacionado e comparado com o render x86 → **99,9 % igual**.

### Decisões de desenho

- **Entrada por evdev, não por SDL gamepad.** Os botões dos handhelds vêm de driver
  próprio (`rocknix-joypad`) e o SDL só reconhece controle com entrada no
  `SDL_GameControllerDB`, que não existe para aparelho custom. Além disso,
  `buttons.map` mapeia por código evdev, e o SDL entrega índice. O SDL ficou só com
  janela e desenho. Ver `docs/controles.md`.
- **Múltiplos controles:** uma thread de leitura por `/dev/input/event*`, estado é a
  soma de todos. Analógico ainda não lido — normalizar exige `absinfo` (ioctl).
- **Ícones como dados**, não PNG: lista de linhas/polilinhas/círculos no espaço
  24×24. Sem asset binário, nítido em qualquer DPI.
- **Gradiente por faixas de 4 px**, não shader: mantém o backend software do
  SDL funcionando, que é o plano B do RK3326.
- **Escala via `set_logical_size`**: o SDL letterboxa. O protótipo calculava à
  mão e cortava 106 px de cada lado em 640×480.
- **Toque por registro de retângulos durante o desenho**: quem desenha registra
  a área tocável, então geometria e hit-test não podem divergir.
- **Comandos de ponteiro no núcleo** (`FocusAt`, `GoTab`, `OpenGame`,
  `OpenApp`, `GoSettingsCat`): sem eles a UI teria de simular dezenas de `Nav`
  para chegar ao item tocado.
