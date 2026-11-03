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
- `node tests/run-all.mjs` passou: **66/66** (39 de validação + 14 do verificador
  de quadro + 13 de capacidades de entrada).
- `cargo build --workspace` em `app/launcher`: **compila e linka**, zero erro e
  zero aviso (rustc 1.99, SDL2 2.32.10, SDL2_ttf 2.24.0).
- `cargo test --workspace` em `app/launcher`: **121/121** (64 no `core`, 57 no
  `ui-sdl`).
- O launcher **roda**: Home desenhada de verdade e gravada em BMP com
  `--dump-frame`, sem tela (`SDL_VIDEODRIVER=dummy`).
- `bash -n scripts/migrar-do-antecessor.sh` e `bash -n
  scripts/compilar-launcher.sh`: **sem erro de sintaxe**.
- `bash scripts/aparelho.sh` — todos os subcomandos **executados contra o
  aparelho real**: `procurar`, `diagnostico`, `instalar`, `subir`, `parar`,
  `verificar`, `log`, `input`, `ajuda`. `verificar` fechou o ciclo completo:
  capturou 720×1280, gerou a referência 1280×720 no PC e comparou com **99,9 %**
  de acerto na rotação `ccw` e **10,9 %** em `cw`.
- O workflow ganhou `launcher-core` (testa os 48 do núcleo, sem SDL) e
  `launcher-build` (compila o workspace com SDL2). Os dois comandos passam
  localmente com `--locked`; o YAML foi parseado, mas os jobs **não rodaram no
  GitHub** ainda.

### Verificado NO APARELHO (03/11/2026)

Aparelho `giochi-system-x55`: Buildroot 2024.02, aarch64, kernel 6.12.0,
glibc 2.38, SDL2 2.28.5, PMIC `rk817`, painel DSI 720×1280, 1968 MB de RAM.
Acesso por `ssh root@<ip>` — **o IP muda a cada boot** (DHCP), então use
`bash scripts/aparelho.sh procurar` em vez de decorar um endereço.

- **Cross-compile aarch64 funciona**: `scripts/compilar-launcher-arm64.sh`
  gera o binário do zero (sysroot + libs do aparelho + wrapper de link).
- **O launcher RODA no aparelho**: `SDL_VIDEODRIVER=kmsdrm` abre o DRM real,
  desenha e grava frame; 6 descritores de DRM e 5 de `/dev/input` abertos.
- **Rotação para painel retrato**: antes ~40 % da tela era letterbox preto e a
  UI saía deitada; agora 0 % de preto e o layout encaixa.
- **Instalado** em `/opt/giochi/apps/launcher/bin/giochi-launcher`, o caminho
  que o `giochi-init` espera, com sha256 idêntico ao do repositório —
  conferido por `aparelho.sh instalar` a cada rodada.
- **Corrigido o sentido da rotação**: o primeiro teste mostrou a UI de cabeça
  para baixo. Horário e anti-horário diferem 180°; o X55 precisa de
  anti-horário. Confirmado por comparação de frame: **99,9 %** igual à
  referência, contra **10,9 %** no sentido oposto.
- **Config de rotação com recarga ao vivo**: o arquivo é relido a cada segundo,
  então trocar o sentido não exige reabrir o launcher.
- **Corrigido um bug que impedia os botões de funcionar**: o
  `/etc/giochi/buttons.map` tem comentário no fim de cada linha
  (`up = BTN_DPAD_UP  # gpio4 PA0`), e o parser não cortava o `#`. Resultado:
  **1 de 13** botões parseados, os outros 12 descartados em silêncio. Agora
  corta e tem teste com o conteúdo real do mapa.
- **Botão de energia, volume, brilho, desligar e reiniciar: exercitados no
  hardware.** Não são mais suposição — ver §12.
- **Os 20 códigos de tecla declarados pelo kernel têm entrada no app.** O
  `B: KEY=` de `/proc/bus/input/devices` foi decodificado e cruzado com
  `code_for` e com o `buttons.map`: cobertura completa.
- **O `inittab` NÃO inicia o `giochi-init`** — não há entrada `::respawn:`. O
  launcher nunca sobe no boot; só por SSH. É do **Giochi System**.
- `app/launcher/bin/giochi-launcher` versionado; o app passou a **`ready`**.
- `capa.png` 320×320 gerada da paleta do tema (provisória).
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
- **GLES2 não exercitado.** O que rodou foi o renderer *software* sobre KMSDRM,
  que é o plano B do RK3326. O backend acelerado nunca foi aberto.
- **Analógico não lido.** Os eixos estão declarados em `adc-joystick` e a
  calibração (`axis_*`) já é parseada, mas normalizar exige `absinfo` por ioctl
  e nada disso foi implementado. Os 13 botões, sim, funcionam.
- **Desempenho no RK3326** não medido — não há contagem de FPS em campo.
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
│   ├── compilar-launcher-arm64.sh  # cross-compile + instala no aparelho
│   ├── aparelho.sh            # operação e diagnóstico do aparelho por SSH
│   ├── verificar-frame.mjs    # decide se o quadro renderizado é a UI ou lixo
│   ├── capacidades-input.mjs  # códigos que cada /dev/input declara
│   └── lib/
│       ├── apps.mjs
│       ├── png.mjs
│       └── capacidades.mjs    # bitmask B: KEY= de /proc/bus/input/devices
├── tests/
│   ├── validate-app.test.mjs
│   ├── verificar-frame.test.mjs
│   ├── capacidades-input.test.mjs
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
│   ├── compilar-launcher.md
│   ├── dependencias-aparelho.md
│   ├── contrato-app-sistema.md
│   ├── ferramentas.md
│   └── checklist-imagem.md
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
node tests/run-all.mjs                    # 66 testes JS
bash scripts/migrar-do-antecessor.sh <clone>   # migra do antecessor

bash scripts/compilar-launcher.sh         # build do launcher (x86-64), resolve o SDL2
bash scripts/compilar-launcher-arm64.sh   # cross-compile (acha o aparelho sozinho)
cd app/launcher && cargo test --workspace  # 121 testes do launcher
```

### Ferramentas do aparelho

O aparelho vive mudando de IP e não inicia a UI sozinho, então a operação dele
tem script próprio. Guia completo em `docs/ferramentas.md`.

```bash
bash scripts/aparelho.sh procurar         # varre a /24, acha pelo hostname
bash scripts/aparelho.sh diagnostico      # sistema, tela, áudio, entrada, libs
bash scripts/aparelho.sh instalar         # compila, instala, confere sha256
bash scripts/aparelho.sh subir            # inicia o launcher (o init não faz)
bash scripts/aparelho.sh parar
bash scripts/aparelho.sh verificar        # processo + DRM + frame comparado
bash scripts/aparelho.sh log
bash scripts/aparelho.sh input 15         # eventos crus de /dev/input
bash scripts/aparelho.sh capacidades      # códigos que cada dispositivo declara

node scripts/verificar-frame.mjs --captura f.bmp [--referencia r.bmp] [--rotacao ccw]
node scripts/capacidades-input.mjs [--codigo 116] [--dispositivo gpio]
```

`verificar` é o que importa: para o app (o KMSDRM é exclusivo), captura um
quadro, desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`, des-rotaciona
pela orientação que o sistema declara e compara pixel a pixel. No X55 dá **99,9 %**
de acerto; com a rotação oposta cai para **10,9 %**. É essa diferença que
distingue "desenhou" de "desenhou de ponta a cabeça".

`capacidades` responde a outra pergunta: o que o kernel **promete** emitir.
Cruzando com `input`, que mostra o que ele **entregou**, separa-se driver/DTS
(não declarou, não entrega) de app (`buttons.map` ou `code_for` sem o código).
No X55 o controle declara **17 códigos**, que são os 17 botões do `buttons.map`;
os 4 do direcional estão em `BTN_DPAD_*` (0x220), faixa `BTN_` e não `KEY_`, e
por isso parecem códigos sem nome. Todos os 20 códigos declarados têm entrada no
app — cobertura completa, agora verificável em segundos.

Três armadilhas que os scripts já tratam e que não devem ser reintroduzidas:
`pkill -f giochi-launcher` mata a própria sessão SSH (use `pidof` + `kill`);
`--dump-frame` sob `x11` grava um BMP **preto** porque o swap esvazia o buffer
(use `dummy` no PC, `kmsdrm` + `software` no aparelho); e o `B: KEY=` do `/proc`
lista as palavras de 64 bits em ordem **decrescente**, então a última coluna é a
palavra 0 — ler da esquerda desloca todos os códigos em múltiplos de 64.

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

### Com o aparelho na mão

O IP muda a cada boot e o `inittab` ainda não inicia o `giochi-init`. Os scripts
já sabem disso:

```bash
bash scripts/aparelho.sh procurar     # acha o IP pelo hostname
bash scripts/aparelho.sh instalar     # compila, instala, confere sha256
bash scripts/aparelho.sh subir        # faz o que o init deveria fazer
bash scripts/aparelho.sh verificar    # processo + DRM + frame comparado
```

`verificar` é o que prova que está desenhando, e não só vivo: captura o quadro,
gera a referência no PC e compara pixel a pixel (99,9 % em `ccw`, 10,9 % em
`cw`). Guia completo em `docs/ferramentas.md`.

### Quando chegar uma imagem nova do sistema

Siga `docs/checklist-imagem.md`. Ele responde, em ordem, se a UI sobe no boot,
se o texto aparece, se a orientação continua certa e se os botões continuam
chegando — e diz **de quem é o problema** quando algo falha. É o roteiro da
próxima rodada.

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

Sessão de 03/11/2026: **Linux x86-64 com a toolchain completa**, mais o aparelho
na rede. Isto encerra a "era sem ferramentas" das versões anteriores deste
arquivo, em que `rustc`, `git` e `bash` não existiam e nada podia ser executado.

| Ferramenta | Estado | Consequência |
| --- | --- | --- |
| `node` | 24.22.0 | validador e testes JS rodam |
| `rustc` / `cargo` | 1.99.0 | launcher compila, linka, roda e testa |
| `git` | 2.53.0 | repositório existe, commits possíveis |
| `bash` | 5.3.9 | `bash -n` nos scripts passa |
| SDL2 / SDL2_ttf | 2.32.10 / 2.24.0 | via prefixo local em `~/.cache/giochi-sdl2/root` |
| cross aarch64 | rustup target + sysroot 2.35 + libs do aparelho | `~/.cache/giochi-arm64` |
| **aparelho** | `ssh root@<ip>`, IP **muda a cada boot** | aarch64 real, KMSDRM funciona |

O aparelho entra e sai da rede (desligamento, troca de IP). Antes de qualquer
comando, `bash scripts/aparelho.sh procurar`.

O que **continua sem verificação**:

1. **Backend GLES2** sobre KMSDRM — só o renderer software foi exercitado.
2. **Eixos analógicos** — declarados pelo kernel, não lidos pelo app.
3. **Desempenho no RK3326** — sem medição de FPS.
4. `bash -n` no `build-device.sh` do **Giochi System** — outro repositório.
5. `scripts/migrar-do-antecessor.sh` — sintaxe ok, execução nunca testada (falta
   um clone do antecessor).

### Regra de honestidade

Nada aqui pode ser declarado "funciona" sem ter sido executado. Estado em
03/11/2026, tudo rodado:

- `node scripts/validate-app.mjs --all` → **7 apps OK**
- `node tests/run-all.mjs` → **66/66**
- `cargo test --workspace` → **121/121**
- `cargo build --workspace` → zero erro, zero aviso, nos dois modos de feature
- `--dump-frame` no PC e no aparelho → BMP com conteúdo real (135 cores)
- `aparelho.sh verificar` → frame 720×1280 comparado, **99,9 %** de acerto

"Compila" e "roda" são afirmações diferentes, e agora as duas valem para x86-64
**e** para o aparelho. O que não vale para o aparelho é o **texto** — e isso é
dependência de imagem, não código.

Uma afirmação já caiu por não ter sido testada: o `poweroff` (§12). A lição não
é "cuidado com o kernel", é **testar antes de escrever**.

### Sobre o protótipo HTML

Continua sendo a referência visual e roda sem build. Ele não é o produto: o
produto é o binário Rust.

## 12. Launcher — estado específico

**Stack decidida em 02/11/2026: SDL2 + renderer próprio.** Alternativas
descartadas: egui (desenho custom limitado) e Slint (DSL + runtime grande para
1 GB de RAM). SDL2 já está no Buildroot e o KMSDRM dispensa compositor.

| Item | Estado |
| --- | --- |
| `crates/core` — 6 módulos, 64 testes | **compila, testa e passa** |
| `crates/ui-sdl` — 11 módulos, 57 testes | **compila, linka e roda** |
| `cargo build --workspace` | zero erro, zero aviso, com e sem a feature `texto` |
| Execução real | **roda** — `--dump-frame` grava a Home em BMP |
| Protótipo HTML | **funcional** — `app/launcher/prototipo/launcher.html` |
| Fonte | **DejaVu Sans** — o TTF **não está no repositório**: copiar para `assets/fonts/DejaVuSans.ttf`, ou o binário usa `/usr/share/fonts/...`. |
| **aarch64 no aparelho** | **roda** — KMSDRM real, frame capturado |
| Rotação para painel retrato (720×1280) | **funciona** — anti-horário, 99,9 % igual à referência |
| Execução do caminho instalado | **confirmado** — `/opt/giochi/apps/launcher/bin/`, laço vivo |
| Orientação lida do sistema | **confirmado** — `fbcon/rotate=3` → `Ccw90`, sem override |
| Brilho no painel | **confirmado** — `--brilho 20/70/50` → 333/1166/833, e persiste |
| Despertar por botão | **confirmado no aparelho** — log registra o gesto |
| Botão de energia físico | **confirmado no aparelho** — segurar 1 s abriu o menu |
| Teclas de volume | **confirmado no aparelho** — 70→65→...→100 e `alsactl` em 255/255 |
| Desligar (`poweroff`) | **confirmado no aparelho** — cortou a energia de fato |
| Mixer ALSA | **confirmado** — `Master Playback Volume`, elemento descoberto |
| Cobertura do mapa de botões | **20/20** códigos do kernel têm entrada no app |
| Ferramentas de teste | **9 subcomandos**, todos exercitados no aparelho — ver `docs/ferramentas.md` |
| GLES2 / analógico / desempenho | **não exercitados** |
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

### Desligar e reiniciar

O núcleo troca de tela; `crates/ui-sdl/src/system.rs` executa `poweroff` /
`reboot`. `Screen::Reboot` existe separado de `Screen::Boot` porque `Boot` também
é o estado inicial — sem isso ninguém saberia quando chamar `reboot`.

**Medido em 03/11/2026: `poweroff` desliga o X55 de verdade** (o aparelho
precisou ser religado pelo botão).

Isso corrigiu uma conclusão errada minha. Eu tinha afirmado que, sem
`rockchip,system-power-controller` no device tree, o `poweroff` só pararia o
sistema sem cortar energia. A propriedade **está ausente** e o desligamento
funciona assim mesmo. Escrevi a conclusão a partir do comportamento documentado
do kernel, sem testar, e o teste a desmentiu.

`--checar-energia` agora é informativo e diz isso na saída. A regra que fica:
**ausência de propriedade de device tree não autoriza prever o comportamento de
um comando.**

### Descarte silencioso no `buttons.map`

O mapa é do **sistema**; o tradutor de símbolos é do app. Um símbolo que o app
não conhece era descartado em silêncio, e o botão simplesmente não funcionava —
sem pista nenhuma. Aconteceu **duas vezes**: com comentário no fim da linha
(`up = BTN_DPAD_UP  # gpio4 PA0`, que deixava 1 de 13 botões) e com
`KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`.

Hoje `ButtonsMap::parse` acumula `desconhecidos` e `load` avisa uma vez:

```text
aviso: 9 símbolo(s) do mapa não reconhecido(s): axis_x_min = 15, ...
```

Foi esse aviso que revelou que as chaves `axis_*` (calibração do analógico) eram
declaradas pelo sistema e ignoradas pelo app. Agora são parseadas para
`calibragem` e ficam prontas para quando a leitura do analógico entrar — sem
voltar a aparecer como "desconhecido".

### O que o kernel promete × o que ele entrega

Duas perguntas diferentes, e confundi-las é o que faz depurar no escuro:

| Pergunta | Ferramenta | Responde |
| --- | --- | --- |
| o evento chegou? | `aparelho.sh input` | lê `/dev/input` cru, sem mapa |
| o dispositivo declara esse código? | `aparelho.sh capacidades` | lê `B: KEY=` do `/proc` |

Quando um botão não reage, o cruzamento das duas dá o veredito:

| declara? | entrega? | Conclusão |
| --- | --- | --- |
| sim | sim | kernel ok; o problema é `buttons.map` ou `code_for` |
| sim | não | driver ou DTS não registra o evento |
| não | sim | bitmask incompleto; aviso, não veredito |
| não | não | o botão não existe neste hardware |

**O formato do bitmask engana.** O kernel lista as palavras de 64 bits em ordem
**decrescente**, então a última coluna é a palavra 0. Ler da esquerda desloca
todos os códigos em múltiplos de 64 e produz um mapa plausível e errado:

```text
B: KEY=10000000000000 0    → palavra 1, bit 52 → 64+52 = 116 = KEY_POWER
B: KEY=c000000000000 0     → palavra 1, bits 50-51 → 114 e 115 = VOLUMEDOWN/UP
```

`evtest --query` não ajuda: **devolve 0 para qualquer código** nesta imagem. O
`/proc` é a única fonte.

Mapa real do X55, decodificado:

```text
rk805 pwrkey        [event0]  116
gpio-keys-control   [event3]  304,305,307,308,310-318   (13 botões do gamepad)
                              544-547                   (direcional)
gpio-keys-vol       [event4]  114,115
```

Os 4 do direcional são `BTN_DPAD_UP/DOWN/LEFT/RIGHT` — faixa `BTN_` 0x220, não
faixa `KEY_`, e é por isso que parecem códigos sem nome. Somando: **20 códigos**,
e os 20 têm entrada em `code_for` **e** no `buttons.map`. Cobertura completa, e
agora há como provar isso em segundos em vez de conferir 20 linhas à mão.

### Feature `texto`

O crate `ui-sdl` compila em dois modos:

| Build | Feature | Linka contra |
| --- | --- | --- |
| padrão | `texto` | `libSDL2` + `libSDL2_ttf` |
| aparelho hoje | `--no-default-features` | só `libSDL2` |

Sem `texto`, `text_size` devolve altura aproximada para o layout não colapsar e
`text` não desenha nada. Existe porque o aparelho **não tem** `SDL2_ttf` nem
freetype nem fonte — ver `docs/dependencias-aparelho.md`.

### Sono, despertar e backlight

Três regras, todas nascidas de bug real no aparelho:

1. **Qualquer entrada de pessoa acorda.** `Cmd::is_user_input` separa "um dedo
   apertou" de "o sistema reportou" (bateria, HUD, presença de controle). O
   reducer intercepta em `Sleeping`/`Shutdown` **antes** do `match`, e a entrada
   é consumida pelo despertar — o botão que acorda não age. Antes disto, só
   toque e teclado chamavam `Wake`; nenhum botão do gamepad acordava, embora a
   tela de sono prometesse "pressione qualquer botão".
2. **O botão segurado não pode vazar.** `poll_gamepad` roda `mapper.update`
   mesmo dormindo (senão borda/repetição envelhecem e sai um `Confirm` fantasma
   logo depois de acordar) e **descarta o resto do quadro** quando estava
   travado.
3. **Dormir apaga o backlight.** `crates/ui-sdl/src/backlight.rs` escreve em
   `/sys/class/backlight/*/brightness`. Sem isso a tela mostrava a lua a 50 % de
   brilho — o "bug de energia" que o usuário suspeitou era real. `apagar` é
   idempotente (dormir duas vezes não salva 0 como "anterior"). Na abertura,
   `destravar_tela_apagada` reacende se o valor veio 0 — proteção para o caso de
   o processo morrer dormindo.
4. **O botão de energia físico funciona.** `rk805 pwrkey` emite `KEY_POWER`
   (116) e o app passou a lê-lo. Toque curto dorme (apaga o backlight); segurar
   1 s abre o menu de energia, **ao atingir o limiar** e não ao soltar — senão
   o usuário segura sem nenhum sinal. Antes disto o `PowerHold` estava ligado
   apenas ao botão START, e o power não fazia nada.
   A medição vive em `crates/ui-sdl/src/power.rs` (puro, 7 testes).
5. **As teclas de volume funcionam e controlam o mixer.** `gpio-keys-vol`
   (event4) emite `KEY_VOLUMEDOWN`/`KEY_VOLUMEUP` (114/115), declarados no
   `buttons.map` — então passam pelo mapa, ao contrário do power. O volume vai
   ao ALSA de verdade (`Master Playback Volume` via libasound), com passo de
   5 % e repetição ao segurar (400 ms, depois 120 ms). Antes era só número na
   tela.
6. **O slider de brilho agora chega no painel.** Antes era decorativo: mudava
   `settings.brightness` e nada em `sysfs`. Agora `definir_fracao` converte para
   a escala do `max_brightness`, escreve e **grava
   `/user/.config/giochi/brightness`** — o arquivo que o `giochi-init` lê no
   boot. Sem essa escrita a escolha do usuário sumia a cada reinício.

Diagnóstico de campo, no próprio aparelho:

```bash
giochi-launcher --testar-backlight   # apaga 2 s e restaura
giochi-launcher --brilho 40          # mesmo caminho do slider, e persiste
giochi-launcher --mostrar-volume     # o que o mixer ALSA diz agora
giochi-launcher --volume 30          # aplica pelo mesmo caminho da UI
giochi-launcher --espiar-input 20    # cada tecla crua que chega do kernel
```

`--espiar-input` é o que separa "o evento não chega ao sistema" de "chega e o
app ignora". Lê `/dev/input/event*` cru, sem mapa, e nomeia cada código.

O botão de energia deixa rastro no log, o que permite diagnosticar sem adivinhar:

```text
botão de energia: toque curto, dormindo
botão de energia: segurado 1.0s, abrindo menu de energia
```

Diagnóstico de campo, sem passar pelo menu de energia:

```bash
giochi-launcher --testar-backlight   # apaga 2 s e restaura
```

Verificado no X55: 833 → 0 → 833.

### Rotação — a orientação é fato do sistema

Painel é retrato (720×1280) e a UI é paisagem (1280×720).

**O sentido já esteve errado e o sintoma foi "de ponta a cabeça".** Horário e
anti-horário diferem exatamente 180°, não há meio-termo.

A primeira correção foi uma remenda: `DEFAULT_ROTATION = Ccw90` com o comentário
"o X55 precisa". Isso ficaria errado na primeira revisão de hardware. Hoje o app
**não contém nome de aparelho nem sentido fixo**: lê a orientação que o sistema
declara em `/sys/class/graphics/fbcon/rotate` (de `fbcon=rotate:N` no cmdline).

Precedência (`crates/ui-sdl/src/display.rs`): `GIOCHI_ROTATE` →
`/user/.config/giochi/rotation` → **`fbcon/rotate`** → dedução pelo formato do
output. O log na abertura diz a fonte:

```text
rotação: Ccw90 (fonte: sistema (fbcon/rotate)) [output 720x1280]
```

`fbcon=rotate:2` (180°) **não é suportado**: o app avisa e pede configuração
manual, em vez de fingir que atendeu.

O arquivo do usuário é relido a cada segundo (troca ao vivo, sem recompilar); a
variável de ambiente é lida só na abertura.

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
