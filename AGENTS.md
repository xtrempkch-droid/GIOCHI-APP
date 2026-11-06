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

- `node scripts/validate-app.mjs --all` passou: **7 apps** (6 em `scaffold`, o
  launcher em `ready`).
- `node tests/run-all.mjs` passou: **86/86** (43 de validação + 17 do verificador
  de quadro + 13 de capacidades de entrada + 13 do gerador de catálogo).
- `cargo build --workspace` em `app/launcher`: **compila e linka**, zero erro e
  zero aviso (rustc 1.99, SDL2 2.32.10, SDL2_ttf 2.24.0).
- `cargo test --workspace` em `app/launcher`: **153/153** (59 no `core`, 94 no
  `ui-sdl`). As contagens estiveram invertidas por várias rodadas: quem tem mais
  testes é o `ui-sdl`, não o `core`.
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

> **04/11/2026: o X55 apareceu em `192.168.1.6`, pela rede, sem `usb0`.** O
> gadget USB dele está `not attached` (`giochi-selftest` → `UDC:
> fcc00000.usb · estado not attached`), então a rota fixa `10.55.0.2` só
> funciona quando o cabo de dados está no modo certo. `procurar` com
> `GIOCHI_SUBREDE=192.168.1` acha os dois casos.

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
- **O `inittab` da imagem de 03/11 NÃO inicia o `giochi-init`** — não há entrada
  `::respawn:`, então o launcher não sobe no boot; só por SSH. **Corrigido nas
  imagens de 04/11** (conferido dentro do `rootfs.ext4`, ver §3).
- `app/launcher/bin/giochi-launcher` versionado; o app passou a **`ready`**.
- `capa.png` 320×320 gerada da paleta do tema (provisória).
- O validador foi exercitado contra: versão fora de semver, runtime inválido,
  arquitetura inválida, campo desconhecido, entrypoint fora de `bin/`, entrypoint
  com `..`, entrypoint inexistente, capa fora de 320×320, capa que não é PNG,
  `bin/` ausente, nome de pasta inválido, `forge.json` inválido, licença em formato
  inválido e **GPL declarada na imagem assinada**.

### X35H — PRIMEIRO TESTE EM HARDWARE (04/11/2026)

**A arquitetura passou.** O mesmo binário, sem nenhuma linha específica de
aparelho, roda no X35H: a orientação veio de `fbcon/rotate=0`, o letterbox de
25% é o esperado para 4:3, e o mapa de 16 botões sem `home` foi aceito sem aviso.

| Medido | Valor |
| --- | --- |
| painel | **640×480** (4:3), `fbcon/rotate=0` → `Rotation::None` |
| frame | 640×480, **25,00%** de preto (60 px por lado) |
| comparação com a referência do PC | **99,9%** (34240 amostras) |
| RAM | **964 MB** (a pergunta que faltava responder) |
| códigos de tecla | **19**: 16 do controle + vol(2) + power(1) — **sem `BTN_MODE`** |
| cores com texto / sem texto | **180 / 97** — prova que os glifos desenham |

Acesso por **`usb0`**: `10.55.0.2`, senha `giochi-dev`. Nossa chave foi
instalada em `/root/.ssh/authorized_keys`, então o toolkit funciona sem senha. O
host key muda quando a imagem é regravada — `ssh-keygen -R` resolve, e o erro que
aparece antes disso fala de ataque MITM, não de chave nova.

### Layout adaptativo: escala 1,0 em qualquer painel (04/11/2026)

**A faixa preta sumiu e o texto dobrou de tamanho**, resolvendo as duas queixas
do usuário ("faixa em cima e embaixo", "fonte muito pequena em telas menores").

A causa era uma só: o espaço de projeto era **fixo em 1280×720** e o SDL
escalava. Num painel 640×480 isso dá escala **0,5** — e aí `F_TINY` (9 px) vira
**4,5 px de device**, ilegível, além da faixa.

| | antes | agora |
| --- | --- | --- |
| espaço de projeto | 1280×720 fixo | **o painel** (640×480) |
| escala | 0,5 | **1,0** |
| `F_TINY` no device | 4,5 px | **9 px** |
| fração da largura | 0,70 % | **1,41 %** |
| faixa preta | 25 % | **0 %** |

`draw::projecao(rot, output)` devolve o espaço de projeto e o lógico:
- **sem giro** → projeto = painel (4:3 usa 4:3 inteiro);
- **com giro** → painel é retrato, projeto é paisagem com medidas trocadas.

O espaço lógico continua sendo o do dispositivo, então a escala é 1,0 **sempre** —
a fonte nunca encolhe. Em troca, o layout passou a ser proporcional:
`views::Moldura` calcula margens, barras e espaçamentos a partir do tamanho do
painel, usando o desenho de 1280×720 só como **proporção de referência**.

Num painel 1280×720 o resultado é **idêntico** ao anterior (`k = 1,0`), o que
permite conferir a mudança comparando frames. Medido:

```text
640x480    640x480, 0,00% de preto, 351 cores, 99,8% de acerto contra o PC
1280x720   1280x720, 0,00% de preto, 392 cores  (sem regressão)
720x1280   720x1280 girado, 0,00% de preto, 348 cores  (X55, sem regressão)
```

3 testes novos fixam a regra: `projecao_acompanha_o_painel_e_nao_encolhe`,
`projecao_com_giro_troca_as_medidas`, `projecao_de_16_9_nao_muda_nada`.

### O que aconteceu com o diagnóstico da faixa

A faixa era **esperada** (16:9 num painel 4:3) e estava **simétrica** — o BMP
mentia sobre a posição, porque `read_pixels` ignora o deslocamento do viewport.
Registrado em `docs/ferramentas.md`. O layout adaptativo resolve a faixa por
outro caminho: não há mais letterbox porque o projeto acompanha o painel.

### O que a imagem do X35H ainda NÃO tem

- **`::respawn:` no `inittab`** — **só na imagem em uso.** Ela é a de 03/11
  19:17, anterior à correção do `post-build` (19:32); a UI não sobe sozinha.
  As imagens de 04/11 **têm a linha** nos dois aparelhos — verificado com
  `debugfs` no `rootfs.ext4`, não no fonte. Falta gravar o cartão.
- **`readelf`** — sem ele o `giochi-selftest` não consegue provar que o
  `SDL2_ttf` usa harfbuzz (fica `[SKIP]`).

### Um `FAIL` do selftest que era falso

O `giochi-selftest` reporta as cinco bibliotecas como **ausentes**, e as cinco
estão em `/usr/lib`. Causa: `ls /usr/lib/${lib}.so* /lib/${lib}.so*` — o segundo
glob não casa (`/lib` é diretório real, não symlink) e o `ls` retorna 1 mesmo
tendo listado o arquivo do primeiro. A correção é `||` entre dois `ls`, e foi
validada no aparelho: as cinco passam a `PASS`.

### Histórico da validação

O rootfs da imagem do X35H em `out/` foi extraído e inspecionado antes de haver
hardware; depois o teste foi executado.

| Item | Estado |
| --- | --- |
| Texto na imagem (`SDL2_ttf` + harfbuzz + DejaVu) | **funciona em hardware** nos dois aparelhos |
| `::respawn:` no `inittab` | **falta só na imagem em uso**; as de 04/11 têm (verificado com `debugfs`) |
| Imagem do **X55** com as correções | **existe em `out/` (04/11 21:22 UTC), NÃO está gravada** |
| Segundo aparelho (**X35H**) | **testado**, 99,9% de acerto |
| `giochi-selftest` bloco "bibliotecas do launcher" | **existe só na imagem nova**; a do X55 em uso não tem |

A premissa deste repositório era que o mesmo binário serve em hardware diferente
porque orientação, brilho, volume e mapa de botões vêm do sistema. Com um
aparelho só (X55) isso era afirmação; **com dois, ficou verificado**: o X35H
rodou sem uma linha de código específica.

Duas coisas mudaram no ambiente e já estão no toolkit:

- **Acesso por `usb0`**: endereço fixo `10.55.0.2`, autenticação por **senha**
  (`ssh root@10.55.0.2`). Não é preciso tirar o cartão para nada.
  `GIOCHI_SENHA=... bash scripts/aparelho.sh ...` usa essa rota, e
  `aparelho.sh opcoes` mostra qual seria usada.
- **`/root/giochi-boot.log`** é criado pelo init e **é apagado ao regravar o
  cartão**. Ler antes de regravar é o que preserva o diagnóstico.
  `aparelho.sh boot-log` salva em `dist/`.

### Ainda pendente

- **Código-fonte dos apps não foi migrado.** Nenhum `src/` tem conteúdo. O script
  existe e passa por `bash -n`, mas **nunca rodou de verdade** — falta um clone do
  antecessor para apontar.
- **Nenhum `bin/` tem conteúdo, exceto o launcher e o volley.** Os jogos Godot
  precisam de export ARM64; as ferramentas são Python e precisam ser empacotadas.
  `app/jogos/volley` é o único jogo que roda hoje — ver §12.
- **`capa.png` só existe no launcher e no volley.** Os outros cinco apps não têm
  320×320, então não podem ser publicados na loja (o validador avisa).
- **Não há empacotador `.giochi`**, nem assinatura Ed25519.
- **Texto: o X55 em uso é a imagem ANTIGA (03/11) e não tem `SDL2_ttf`.** O
  X35H roda a imagem nova, com as libs, e desenha texto sozinho. Não confie no
  nome do aparelho para decidir: o `compilar-launcher-arm64.sh` **pergunta ao
  aparelho** se `libSDL2_ttf` existe e compila com ou sem a feature conforme a
  resposta.

  **A imagem nova do X55 existe em `out/powkiddy-x55/` (04/11 21:22 UTC, mesmo
  commit `70d86a1` da do X35H) e tem as 5 bibliotecas** — conferido com `debugfs`
  no `rootfs.ext4`. Ela só não está gravada no cartão.

  Enquanto isso, o X55 em uso foi desbloqueado copiando para `/usr/lib` as três
   bibliotecas que faltavam (`libSDL2_ttf`, `libfreetype`, `libharfbuzz`) e a
  fonte `DejaVuSans.ttf`, **extraídas do `rootfs.ext4` oficial** da imagem nova.
   É medida temporária e reversível: gravar a imagem nova torna os arquivos
   redundantes. Não é o caminho recomendado — é o que permite testar hoje.
- **GLES2: medido no X35H, e é acelerado.** `GIOCHI_GLES=1` sobre KMSDRM abre
  `opengles2` com `textura máx 8192x8192`; sem a variável, `software`. O launcher
  anuncia qual backend abriu de fato na abertura (ver `Painter::new`), porque
  `SDL_RENDER_DRIVER` inválido **cai em silêncio** para outro driver e pedir não
  prova nada. O que continua sem medição é o **desempenho**: não há FPS em campo
  no RK3326 — o contador do HUD existe, mas só aparece com `hud` ligado.
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
│   ├── jogos/                 # GRUPO: não é app, não tem metadata.json
│   │   ├── pacman/            # nativo, labirinto 21x21, 4 fantasmas
│   │   ├── cobrinha/          # nativo, grade discreta
│   │   ├── volley/            # nativo (Rust+SDL2) — ver §12
│   │   ├── arcade-volleyball/     # jogo padrão (image, godot)
│   │   ├── colossal-cave-adventure/   # jogo padrão (image, godot)
│   │   ├── spacewar/          # jogo padrão (image, godot)
│   │   └── shattered-pixel-dungeon/   # GPL-3.0 (store, native)
│   └── launcher/              # Rust + SDL2 (image, native)
│       ├── Cargo.toml         # workspace: crates/core + crates/ui-sdl
│       ├── crates/core/       # núcleo puro, sem SDL
│       └── crates/ui-sdl/     # janela, desenho, evdev, catálogo
├── schemas/
│   ├── metadata.schema.json
│   └── forge.schema.json
├── scripts/
│   ├── validate-app.mjs
│   ├── migrar-do-antecessor.sh
│   ├── compilar-launcher.sh   # resolve SDL2 e compila o launcher
│   ├── compilar-launcher-arm64.sh  # cross-compile + instala no aparelho
│   ├── compilar-jogo-arm64.sh      # idem para o jogo volley (reusa o cache)
│   ├── aparelho.sh            # operação e diagnóstico do aparelho por SSH
│   ├── verificar-frame.mjs    # decide se o quadro renderizado é a UI ou lixo
│   ├── verificar-telas.mjs    # as 12 telas em 3 tamanhos: cabe? ocupa quanto?
│   ├── verificar-mapa.mjs     # buttons.map contra os símbolos do launcher
│   ├── empacotar.mjs          # app → .giochi assinado (Ed25519)
│   ├── verificar-pacote.mjs   # estrutura + sha256 + assinatura
│   ├── verificar-jogos.mjs    # geometria dos jogos: a célula é legível?
│   ├── gerar-catalogo.mjs     # projeta o índice do aparelho
│   ├── capacidades-input.mjs  # códigos que cada /dev/input declara
│   └── lib/
│       ├── tar.mjs            # ustar à mão: ordem, reprodutibilidade, exec bit
│       ├── publicar.mjs       # valida, ordena, assina, verifica o pacote
│       ├── extrair-pacote.sh  # extrator que roda NO aparelho
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
│   ├── checklist-imagem.md
│   └── mensagem-giochi-system.md   # ponteiro para o canal (fora do repo)
└── .github/workflows/validate.yml
```

### Anatomia de um app

```text
app/<slug>/            # ou app/jogos/<slug>/ para jogos
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
    { "id": "forge", "folder": "forge", "runtime": "native", "status": "scaffold" },
    { "id": "volley", "folder": "jogos/volley", "runtime": "native", "status": "ready" }
  ]
}
```

- `id` — slug kebab; é a identidade estável.
- `folder` — pasta real em `app/`. **Pode ter um nível de grupo** (`jogos/volley`).
- `runtime` — `godot`, `retroarch`, `proton` ou `native`.
- `status` — `scaffold` ou `ready`.
- `distribution` — `image`, `store` ou `host` (ver abaixo).
- `license` — identificador SPDX ou `Proprietary`; precisa bater com o `metadata.json`.

#### Grupos: `app/jogos/`

Os jogos vivem em `app/jogos/<slug>`; ferramentas e launcher ficam soltos em
`app/`. A separação é organizacional — muda onde o arquivo mora, não o que ele é.

**O que distingue grupo de app é o `metadata.json`.** Pasta com o arquivo é app;
pasta sem ele é grupo, e o validador desce um nível. Não há lista de grupos
escrita à mão: quem cria a pasta e põe `metadata.json` dentro já disse o que ela
é. Uma convenção que morasse em dois arquivos envelheceria diferente nos dois —
que é a mesma razão pela qual `releaseYear` precisou entrar no schema nos três
lugares de uma vez.

Profundidade máxima: 2 (`app/grupo/app`). Mais que isso não é organização.

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
`entrypoint`. Opcionais: `graphics_api`, `buttons_map`, `license`, `licenseUrl`,
`releaseYear`. `additionalProperties: false` — campo desconhecido reprova.

Semver obrigatório (`1.0.0`, não `1.0`). `arch` só aceita `aarch64`.

**`releaseYear` é opcional e é o ano que o cartão e a ficha mostram.** Ele viaja
numa cadeia de três elos — `metadata.json` → índice da loja (`year`) → catálogo
do launcher — e sem o primeiro elo os outros dois não têm o que repassar. `0` ou
ausente é *desconhecido*, e a UI mostra `—`. Nunca derivar do índice do app: ver
"índice interno não é dado do jogo" em §12.

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
node tests/run-all.mjs                    # 133 testes JS
bash scripts/migrar-do-antecessor.sh <clone>   # migra do antecessor

bash scripts/compilar-launcher.sh         # build do launcher (x86-64), resolve o SDL2
bash scripts/compilar-launcher-arm64.sh   # cross-compile (acha o aparelho sozinho)
cd app/launcher && cargo test --workspace  # 194 testes do launcher
```

### Ferramentas do aparelho

O aparelho vive mudando de IP e não inicia a UI sozinho, então a operação dele
tem script próprio. Guia completo em `docs/ferramentas.md`.

```bash
bash scripts/aparelho.sh procurar         # tenta 10.55.0.2 (usb0), depois varre a /24
bash scripts/aparelho.sh diagnostico      # sistema, tela, áudio, entrada, libs
bash scripts/aparelho.sh instalar         # compila, instala, confere sha256
bash scripts/aparelho.sh subir            # inicia o launcher (o init não faz)
bash scripts/aparelho.sh parar
bash scripts/aparelho.sh verificar        # processo + DRM + frame comparado
bash scripts/aparelho.sh log
bash scripts/aparelho.sh input 15         # eventos crus de /dev/input
bash scripts/aparelho.sh capacidades      # códigos que cada dispositivo declara
bash scripts/aparelho.sh opcoes           # rota e autenticação (rede x usb0)
bash scripts/aparelho.sh boot-log         # salva o log ANTES de regravar o cartão
bash scripts/aparelho.sh catalogo         # gera e publica o índice de apps

node scripts/gerar-catalogo.mjs --lista   # quem entra no catálogo e quem fica fora
node scripts/verificar-frame.mjs --captura f.bmp [--referencia r.bmp] [--rotacao ccw]
node scripts/verificar-telas.mjs         # as 12 telas em 640x480, 800x600, 1280x720
node scripts/verificar-jogos.mjs         # a geometria dos jogos nos mesmos 3 tamanhos
node scripts/capacidades-input.mjs [--codigo 116] [--dispositivo gpio]
node scripts/verificar-mapa.mjs --mapa <buttons.map>   # símbolos que o app entende
bash scripts/canal.sh                    # janela de contato com o Giochi System
```

### Canal com o Giochi System

A conversa entre as equipes vive em `/home/charles/Giochi/chat_de_equipe.txt` —
**fora dos dois repositórios**, porque nenhum dos dois é visível para o outro
lado. Neste repo há só um ponteiro: `docs/mensagem-giochi-system.md`.

**Janela: 08:00–18:50, horário de Brasília.** Fora dela não se responde nem se
inicia rodada. `bash scripts/canal.sh` responde, e `--exigir` sai com 1 para
encadear em script.

`verificar` é o que importa: para o app (o KMSDRM é exclusivo), captura um
quadro, desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`, des-rotaciona
pela orientação que o sistema declara e compara pixel a pixel. No X55 dá
**98,9 %** de acerto e no X35H **99,9 %**; com a rotação oposta cai para
**10,9 %**. É essa diferença que distingue "desenhou" de "desenhou de ponta a
cabeça".

A comparação só vale com a **mesma entrada dos dois lados**: catálogo igual e
referência no espaço de projeto (paisagem). As duas coisas já faltaram, e das
duas vezes o launcher pareceu pior sem ter regredido — ver
`docs/ferramentas.md`.

`capacidades` responde a outra pergunta: o que o kernel **promete** emitir.
Cruzando com `input`, que mostra o que ele **entregou**, separa-se driver/DTS
(não declarou, não entrega) de app (`buttons.map` ou `code_for` sem o código).
No X55 o controle declara **17 códigos**, que são os 17 botões do `buttons.map`;
os 4 do direcional estão em `BTN_DPAD_*` (0x220), faixa `BTN_` e não `KEY_`, e
por isso parecem códigos sem nome. Todos os 20 códigos declarados têm entrada no
app — cobertura completa, agora verificável em segundos.

**A mesma distinção vale do lado da imagem**, e a equipe do System chegou nela
pelo mesmo caminho: **biblioteca presente não prova uso.** Um `libharfbuzz.so` no
rootfs não garante que o `SDL2_ttf` o use — só `readelf -d` mostra as
`DT_NEEDED`. O `giochi-selftest` da imagem nova passou a fazer essa verificação.

Regra geral que vale nos dois lados: **existir, declarar e ser usado são fatos
diferentes**, e confundir os três custa uma sessão de diagnóstico.

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
gera a referência no PC e compara pixel a pixel (98,9 % no X55 e 99,9 % no
X35H em `ccw`, 10,9 % em `cw`). Guia completo em `docs/ferramentas.md`.

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

1. **Desempenho no RK3326** — os dois backends abrem (software e `opengles2`), mas
   não há medição de FPS em campo.
2. **Eixos analógicos** — declarados pelo kernel, não lidos pelo app.
3. `bash -n` no `build-device.sh` do **Giochi System** — outro repositório.
4. `scripts/migrar-do-antecessor.sh` — sintaxe ok, execução nunca testada (falta
   um clone do antecessor).

### Regra de honestidade

Nada aqui pode ser declarado "funciona" sem ter sido executado. Estado em
03/11/2026, tudo rodado:

- `node scripts/validate-app.mjs --all` → **7 apps OK**
- `node tests/run-all.mjs` → **86/86**
- `cargo test --workspace` → **153/153**
- `cargo build --workspace` → zero erro, zero aviso, nos dois modos de feature
- `--dump-frame` no PC e no aparelho → BMP com conteúdo real (135 cores)
- `aparelho.sh verificar` → frame 720×1280 comparado, **98,9 %** de acerto
  (era 99,9 % antes de a referência passar a usar o catálogo real; a diferença é
  o relógio da barra superior)

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
| `crates/core` — 7 módulos, 87 testes | **compila, testa e passa** |
| `crates/ui-sdl` — 14 módulos, 107 testes | **compila, linka e roda** |
| `cargo build --workspace` | zero erro, zero aviso, com e sem a feature `texto` |
| Execução real | **roda** — `--dump-frame` grava a Home em BMP |
| Protótipo HTML | **funcional** — `app/launcher/prototipo/launcher.html` |
| Fonte | **DejaVu Sans** — o TTF **não está no repositório**: copiar para `assets/fonts/DejaVuSans.ttf`, ou o binário usa `/usr/share/fonts/...`. |
| **aarch64 no aparelho** | **roda** — KMSDRM real, frame capturado |
| **X35H (640×480, rotação 0)** | **roda sem código novo** — 99,9% de acerto, 25% de letterbox esperado |
| **Texto no aparelho** | **funciona** — 180 cores com glifos contra 97 sem |
| Rotação para painel retrato (720×1280) | **funciona** — anti-horário, 98,9 % igual à referência (relógio e bateria mudam entre as capturas) |
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
| Layout em 640×480 (X35H) | **medido no aparelho** — 0,00% de preto, 99,4–99,9% igual à referência |
| Renderer `opengles2` | **abre no X35H** — acelerado, textura máx 8192x8192 |
| Leitura do catálogo (`index.json`) | **no aparelho** — `catálogo: 1 app(s)` |
| Lançar jogo e voltar | **no aparelho** — volley rodou e o launcher voltou com texto |
| Jogos instaláveis que rodam | **3**: `pacman`, `cobrinha`, `volley` — os Godot seguem sem `bin/` |
| Catálogo com 3 itens | **medido no X55** — `catálogo: 3 app(s)`, os 3 abrem por `--lancar` |
| Jogo em painel retrato (X55) | **gira** — `projeto 1280x720 (Ccw90)`, preenche o painel |
| Ciclo jogar → sair → voltar | **nos dois aparelhos** — X35H e X55, com rotação |
| `A` no cartão da Home lança | **no X55** — `giochi-cobrinha` com ppid do launcher |
| L2/R2 trocam sub-aba | **no X55** — `r2,r2,r2` dá a volta nos filtros; `r2` cicla categoria |
| L2/R2 físicos emitem evento | **não testado** — o kernel declara 312/313 e o app os lê, mas ninguém apertou os gatilhos |
| R35S: mapa de botões | **verificado sem aparelho** — 19/19 símbolos reconhecidos |
| R35S: tela sem rádio | **preparada e fotografada** com `--sem-radio` |
| R35S: imagem / boot / botões | **não existe** — bloqueado no Giochi System (sem defconfig, sem driver) |
| Analógico / desempenho (FPS) | **não exercitados** |
| Leitura de bateria/temperatura | **temperatura: medida** (`40C` no X55, de `cpu-thermal`). **Bateria: não julgada** — o X55 não tem célula (ver abaixo) |
| Aba de configurações | **navegável** — `Zone::SettingsContent` existe; anel de foco medido no X55 (x 446..1107) |
| Wi-Fi: varredura real | **no aparelho** — `wi-fi: 11 rede(s) em wlan0`, lista desenhada |
| Wi-Fi: senha | **corrente de comandos provada** com senha errada; conexão real **não testada** (falta a senha) |
| Bluetooth: varredura real | **no aparelho** — `bluetooth: 1 aparelho(s) conhecido(s)` |
| Bluetooth: pareamento real | **não testado** — sem aparelho disponível; o caminho de erro foi exercitado |
| Telas de rede no X35H | **não testadas** — risco declarado, ver `docs/rede.md` §5 |

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

### O jogo gira com o sistema, não com o aparelho

O `volley` desenhava em paisagem num buffer que podia ser retrato. No X35H
(640×480, `rotate=0`) funcionava por coincidência; no X55 (720×1280,
`fbcon=rotate:3`) o conteúdo saía espremido nos primeiros 480 px de 1280 —
medido no frame, 6 cores e uma faixa morta enorme.

Agora ele lê `/sys/class/graphics/fbcon/rotate`, como o launcher, e desenha em
espaço de projeto com os retângulos girados:

```text
X35H:  volley: painel 640x480,  projeto 640x480  (Nenhuma)
X55:   volley: painel 720x1280, projeto 1280x720 (Ccw90)
```

`GIOCHI_ROTATE` sobrepõe, para testar no PC — mesma precedência do launcher
(`display.rs`), de propósito: um comando que gira um app tem de girar o outro,
senão o teste mente sobre um dos dois.

A matemática de `rotate_rect` foi **copiada** do launcher, não transformada em
dependência compartilhada. São apps diferentes; um não deve poder reclamar do
outro quando quebrar. São 12 linhas com teste de ida e volta do lado do launcher.

### O que a imagem do X55 revelou

O X55 em uso roda a imagem de **03/11 17:22** (kernel `#2 SMP ... Tue Nov 3`), e
não tem `SDL2_ttf`, `freetype`, `harfbuzz` nem fonte. A imagem nova **existe** em
`GIOCHI-SYSTEM/out/powkiddy-x55/` (`rootfs.ext4`, 04/11 21:22 UTC, commit
`70d86a1` — o mesmo da imagem do X35H) e tem as cinco bibliotecas: conferido com
`debugfs` direto no `rootfs.ext4`.

Ou seja: **o aparelho está atrasado, não o repositório.** O `boot-log` do X55
também não existe nesta imagem, e o `giochi-selftest` dela não tem o bloco
"bibliotecas do launcher" — o bloco é da imagem nova.

O selftest do X55 mostra duas coisas que o launcher **não lê** e poderia:

```text
[PASS] bateria: 0% (Not charging)
[PASS] temperatura: 42C
```

Os arquivos existem nos dois aparelhos; a UI mostra `—` porque ler
`/sys/class/power_supply` e `/sys/class/thermal` nunca foi implementado. É
trabalho de UI, não de sistema.

### Catálogo é projeção, não segunda fonte da verdade

**Feito em 06/11/2026** (era a prioridade pedida em 04/11).

Até aqui o `/opt/giochi/apps/index.json` do aparelho era **escrito à mão**. Instalei
`cobrinha` e `pacman` e o aparelho continuou mostrando **um** jogo até eu
reescrever o JSON — e um app instalado fora dele ficava invisível, sem erro em
lugar nenhum: nem no app, nem no log, nem na tela.

Hoje o catálogo sai de `app/apps.json` + os `metadata.json`:

```bash
bash scripts/aparelho.sh catalogo      # gera, confere no aparelho e publica
node scripts/gerar-catalogo.mjs --lista  # só mostra quem entra e quem fica de fora
```

#### O que ficou de fora, e por quê

Cada regra existe por um motivo que já apareceu, não por simetria:

| Regra | Por quê |
| --- | --- |
| `status: scaffold` fica fora | tem `bin/` vazio; cartão que não abre é pior que cartão ausente |
| `distribution: host` fica fora | `forge` e `store` rodam no PC |
| `distribution: store` fica fora | é instalado em `/user` pela loja, não faz parte da imagem |
| `role: shell` fica fora | senão o launcher **lista a si mesmo** |
| `entrypoint` precisa existir | idem scaffold, mas medido no arquivo |

**O `role: shell` foi descoberto pelo próprio gerador.** O launcher tem
`status: ready`, `distribution: image` e binário no lugar — entrava por todos os
critérios, e o cartão "Launcher" abriria um **segundo** launcher, que brigaria
com o primeiro pelo DRM master e falharia com `kmsdrm not available`. O papel é
**declarado** no `apps.json`, não inferido do nome: fixar `id === "launcher"`
funcionaria até alguém renomear a pasta.

#### A conferência que faltava: no aparelho, não no repo

O gerador checa o binário na **árvore do repositório**. Quem importa é o
**aparelho**: um jogo presente no repo e não instalado entraria no catálogo e
viraria cartão morto. `aparelho.sh catalogo` percorre cada `dir/entrypoint` por
SSH e **reduz** o catálogo antes de publicar, dizendo o que tirou e como instalar.

Medido: removi o binário do `cobrinha` do aparelho → `gerado: 3` virou
`catálogo reduzido para 2`; restaurei → 3 de novo. **Projeção de verdade reflete
remoção, não só adição.**

#### O `dir` do aparelho não é derivável do repo

`jogos/pacman` no repositório é `/opt/giochi/apps/pacman` no aparelho. É
convenção do `Giochi System`, e a única informação do catálogo que não vem nem do
`apps.json` nem do `metadata.json` — por isso está explícita em
`scripts/lib/catalogo.mjs` e tem teste.

#### `category` entrou no `metadata.json`

O cartão mostra `Cobrinha · Arcade · 2026`. Sem campo de gênero no metadata, o
gerador não tinha de onde tirar e todos diriam "Aplicativo". Entrou nos três
lugares de sempre: schema, validador e docs — mesma regra do `releaseYear`.

### Três resoluções: 480p, 600p e 720p

**600p (800×600) medido em 06/11/2026.** Não há aparelho com esse painel, então
tudo aqui é medida no PC com `SDL_VIDEODRIVER=dummy` — que **respeita o tamanho
pedido**. É o que permite afirmar antes de o hardware existir.

#### O launcher cabe nos três

`node scripts/verificar-telas.mjs` — as 12 telas, margens e ocupação:

| Tamanho | home | busca | detalhe | sono |
| --- | --- | --- | --- | --- |
| 640×480 | 70% | 38% | 15% | 5% |
| **800×600** | **71%** | **38%** | **13%** | **4%** |
| 1280×720 | 72% | 51% | 17% | 3% |

Ocupação praticamente igual nos três: o layout é proporcional desde o
`Moldura`, então 600p não é um caso especial — é o mesmo desenho num terceiro
tamanho.

#### Os jogos: não é "cabe", é "serve"

`node scripts/verificar-jogos.mjs` mede a **geometria**, não pixels iguais: a
célula de uma grade precisa de tamanho mínimo para ser lida num painel de 3,5".
Piso adotado: **14 px**, que é 25% de folga sobre o menor aparelho real.

| Jogo | 640×480 | 800×600 | 1280×720 |
| --- | --- | --- | --- |
| `cobrinha` — célula | 18,4 px | **23,0 px** | 27,7 px |
| `pacman` — célula | 22,2 px | **28,0 px** | 33,6 px |
| `volley` — raquete | 57 px | **72 px** | 86 px |

Tudo proporcional, tudo acima do piso. O `pacman` em 600p dá 28 px de célula —
entre os dois aparelhos que existem, como esperado.

#### Os jogos ganharam `--tamanho`

Antes o `dummy` devolvia o desktop (1024×768) e não havia como pedir outro
painel: testar 480p ou 600p exigia o hardware na mão. Agora
`giochi-<nome> --tamanho 800x600 --quadro /tmp/x.bmp` funciona nos três, com a
mesma flag do launcher.

#### O que continua sendo afirmação

**Nenhum aparelho de 600p foi tocado.** Tudo acima é medição de renderizador com
painel simulado. O que muda num aparelho real não é o layout — é o driver, o
`fbcon/rotate` e o KMSDRM, que são justamente o que o `dummy` não exercita.

### Bateria e temperatura: o dado existia e ninguém lia

**Feito em 06/11/2026.** A HUD mostrava `—` nos dois campos enquanto o
`giochi-selftest` do sistema imprimia `bateria: 0%` e `temperatura: 42C`. Não era
difícil — nunca ninguém tinha ligado as duas pontas.

`crates/ui-sdl/src/energia.rs` lê `/sys/class/power_supply` e
`/sys/class/thermal`, e `App::tick_energia` despacha `Cmd::SetBattery` e
`Cmd::SetTemperature` a cada **5 s** (ler 4 arquivos de sysfs a 60 Hz seria
desperdício, e nenhum dos dois valores muda entre dois quadros).

Medido no X55:

```text
energia: bateria 0% (Not charging) · temperatura 40C
```

#### Eu errei sobre a bateria, e o dono do projeto corrigiu

Registrei que `0% (Not charging)` era "leitura válida com valor ruim". **Não é:**
o X55 **não tem bateria**. Ela foi removida fisicamente para dar acesso à serial
(TTL). O `rk817-battery` continua no sysfs publicando `0%` porque o nó do PMIC
existe — não porque haja célula atrás dele.

Consequência que fica: **este módulo não pode ser julgado no X55.** Quem valida
bateria é o outro aparelho, que tem célula. O que dá para conferir aqui é
temperatura, que é sensor do SoC e independe da bateria — e essa fecha:
`cpu-thermal = 39444` no sysfs contra `40C` na tela.

A lição é a de sempre, e é a terceira vez nesta semana: **um número no sysfs não
prova que o hardware existe.** Existir, declarar e ser usado são fatos
diferentes, e aqui o primeiro enganava o segundo.

#### Por que a temperatura entra na mesma linha de log

A primeira versão logava só quando a **bateria** mudava. Uma leitura de
temperatura que falhasse (sensor ausente, caminho diferente) ficava invisível: a
HUD mostrava `—` e nada dizia se era ausência real ou bug. Agora os dois valores
saem juntos quando qualquer um muda.

### O pacote `.giochi` existe — e o download da loja não

**Feito em 06/11/2026**, substituindo a afirmação "nada disso está implementado"
que o `docs/pacote.md` carregava.

```text
app/jogos/pacman ──empacotar──▶ pacman.giochi (+.sig +.manifest.json)
                              ──instalar-pacote──▶ /opt/giochi/apps/pacman
                              ──catálogo──▶ cartão na tela ──▶ jogo
```

**Medido no X55** — o ciclo inteiro:

```text
287  giochi-launcher --lancar pacman     ← pai
300  giochi-pacman                       ← filho, iniciado pelo launcher
quadro depois da volta: 346 cores, 0,00% de preto
```

#### Por que o `tar` é escrito à mão (`scripts/lib/tar.mjs`)

1. **Ordem** — o `tar` do sistema usa a ordem do `readdir`; aqui a ordem é
   contrato e a montagem é uma função só, testável.
2. **Reprodutibilidade** — `mtime`/`uid`/`gid` fixos. Dois empacotamentos do
   mesmo conteúdo dão **o mesmo SHA-256** (conferido). Se mudasse, a assinatura
   deixaria de identificar o conteúdo.
3. **Bit de execução** — `tar --mode` mascara permissão, e o erro só apareceria
   no aparelho como "entrypoint não existe".

Verificado contra o `tar` do sistema: GNU tar lê o ustar escrito à mão, extrai
com as permissões certas e `test -x` passa. Sem essa conferência seria o meu
parser concordando com o meu escritor.

#### `slug`: a identidade não pode vir do título

`metadata.json` ganhou `slug` opcional. Derivar o diretório do **título** faria
"Pac-Man" → "Pac-Man DX" mover o app de lugar, quebrando catálogo e favorito.
Ordem: `metadata.slug` → `id` do `apps.json` → título (último recurso, e o
pacote diz que usou).

Achado real: a primeira instalação foi para `/opt/giochi/apps/pac-man` (do
título) enquanto o catálogo apontava para `pacman`.

#### O catálogo passou a incluir o que veio da loja

Um `.giochi` **não está** no `apps.json`. `aparelho.sh catalogo` agora colhe do
aparelho o que está em `/opt/giochi/apps/*/metadata.json` e o gerador junta,
com o disco vencendo. Sem isso, instalar pela loja deixaria o jogo **invisível
sem erro**.

Segundo achado: substituir a entrada inteira apagava `license` (que só existe no
`apps.json`) — o pacman apareceu no catálogo sem licença. Agora herda.

#### O extrator é script copiado, não `sh -c` aninhado

A primeira versão montava o comando com aspas dentro de aspas dentro de `ssh` e
produziu `sh: "bin/giochi-pacman": unknown operand`. Aninhar três níveis de aspas
para falar com outra máquina não é difícil de escrever — é difícil de **ler
quando falha**. Hoje é `scripts/lib/extrair-pacote.sh`, copiado e executado.

#### O que NÃO existe: a loja

Nenhuma linha de OAuth, butler/butlerd ou UI de loja foi escrita. Falta
`client_id` registrado no itch.io e rede para testar. Está em
[`docs/loja.md`](docs/loja.md) §2, com as decisões que precisam vir antes (§4) —
inclusive que a chave do formato hoje é única, e multi-assinatura é mudança de
formato.

E a limitação mais dura, que não é de código: **catálogo de jogos de PC não é
catálogo de jogos ARM64**. Hoje existem três jogos escritos aqui e quatro
projetos Godot sem export.

### A aba de configurações não era navegável — e Wi-Fi/BT não existiam

**Duas queixas do usuário em 06/11/2026:**

> "na haba config nao e navegavel"
> "as configuraçoes do wifi e bluetooh nao tem lugar para parear e por senha"

Nenhuma das duas era falta de código: eram **ausências de contrato** entre o
desenho e o núcleo.

| Queixa | Causa real |
| --- | --- |
| aba não navegável | existia `Zone::SettingsSidebar` e **não** existia `Zone::SettingsContent` |
| sem lugar para parear/senha | painel da direita era texto puro: nenhuma linha tinha `hits`, nenhum overlay existia |

O painel desenhava `("Wi-Fi", "Ligado")` de um booleano que **ninguém alternava**.
Informação estável e falsa: sempre "Ligado", houvesse conexão ou não. É a mesma
classe do `year_label()` que formatava o id do cartão — **texto desenhado não é
funcionalidade**, e o usuário descobre isso apertando A e nada acontecendo.

Hoje: `→` entra no painel, `←` volta, andar na coluna troca a categoria na hora,
e as linhas de rede abrem lista de Wi-Fi, lista de Bluetooth e teclado de senha.
O valor das duas linhas passou a ser o que o sistema **respondeu** (SSID
conectado, nome do aparelho) em vez do booleano decorativo.

**Medido no X55:** anel de foco no painel da direita em x 446..1107; `wi-fi: 11
rede(s) em wlan0`; `bluetooth: 1 aparelho(s) conhecido(s)`. Desenho completo,
armadilhas e o que **não** foi provado em [`docs/rede.md`](docs/rede.md).

#### A senha errada provou a corrente inteira

Não tenho a senha de nenhuma rede do usuário, então a conexão real não foi
testada. Mas `--conectar "ORLEANS:senha-errada-de-proposito"` provou duas coisas
que nenhum teste de unidade provaria:

1. `add_network` criou a rede com o SSID certo e `set_network`, `enable_network`
   e `select_network` responderam OK — a corrente de comandos executou. Uma falha
   de sintaxe apareceria como `set_network: FAIL`, não como estado do supplicant.
2. **`save_config` não rodou**: `/etc/wpa_supplicant.conf` ficou intacto. É a
   guarda funcionando — senha errada não é persistida, então o aparelho não tenta
   sozinho no próximo boot e falha sem ninguém entender por quê.

#### Esperar a associação é o que separa "aceitou" de "conectou"

`select_network` responde OK antes de qualquer associação. Sem esperar
`wpa_state=COMPLETED`, a tela diria "Conectado" para uma rede que nunca conectou.
A espera tem 12 s e, ao falhar, devolve **o último estado do supplicant** — não
"deu errado". `4-WAY_HANDSHAKE` é senha recusada; `SCANNING` é a rede fora de
alcance; são ações diferentes do usuário.

#### Três armadilhas de ambiente, todas medidas

- **Não existe `timeout` na imagem.** As janelas usam `--timeout` do próprio
  `bluetoothctl` e espera fixa no `wpa_cli`. Um comando que não existe falha em
  silêncio se ninguém olhar.
- **`wlan0` sobe DOWN.** O `S40wifi` do sistema não levanta a interface quando o
  `wpa_supplicant.conf` está vazio, e o `wpa_cli scan` responde `FAIL-BUSY` com o
  rádio parado. O sintoma é "não acha nenhuma rede", não "erro" — então o app
  chama `ip link set up` antes de varrer.
- **`wpa_cli` está em `/usr/sbin` e `ip` em `/sbin`.** O PATH do `giochi-init`
  cobre os dois, mas o launcher também é iniciado à mão por SSH no diagnóstico.
  `semfio::programa()` procura nos diretórios padrão antes de desistir.

#### O bug da seta esquerda — 169 testes verdes e a tela sem saída

Relatado pelo usuário **depois** de tudo medido e commitado:

> "se eu entrar em uma das configs não consigo voltar… aperto seta para direita
> para trocar o tema depois se eu apertar seta para esquerda ele não faz nada"

A causa era uma linha minha: o `if` de "linha ajustável" rodava **antes** de
`navigate()` e terminava em `return state`, engolindo a seta esquerda de toda
linha que não fosse o volume. O ramo `SettingsContent + ← → Sidebar` existia em
`focus.rs`, estava testado, e era **inalcançável** pelo reducer.

A regra correta separa dado de controle de fluxo:
`state::settings_ajustavel(category, index)`. Linha com valor → setas ajustam;
linha sem valor → setas navegam; `B` sobe um nível sempre, inclusive sem overlay.
Quem desenha consulta a mesma função para escolher as dicas do rodapé — antes o
rodapé prometia `← Categorias` numa linha onde `←` ajustava.

**Como passou por 169 testes:** o teste olhava o estado, não o caminho.
`focus.rs` provava `SettingsContent + ← → Sidebar` isolado; ninguém verificava que
o **reducer** deixava a seta chegar lá. Agora
`sair_do_painel_com_esquerda_funciona_em_toda_linha` percorre as seis categorias ×
todas as linhas.

**E a primeira versão do teste novo também se desculpava:** em linha "ajustável"
ela só verificava que o foco ficava, então `settings_ajustavel` devolvendo `true`
para tudo continuava verde. Corrigido para exigir que o valor **mude**. Verificação
de que o teste vale: o bug foi reintroduzido de propósito e o teste reprovou com
`"declarada ajustável, mas ← não mudou nada — é o 'aperto e não faz nada' de novo"`.
**Um teste que nunca falhou não provou nada.**

**Medido no X55**, anel em espaço de projeto: `--foco conteudo:0` → x 446..1107
(linha "Tema", preso); `--teclas right,left` → x 172..419 (coluna esquerda);
`--teclas right,b` → x 172..419.

#### As linhas de aparência mentiam — e o tema não existe

Junto veio o motivo de o usuário ter apertado `→` esperando algo: "Tema Giochi
Dark" sugeria uma lista de temas e há **um**, constante em `theme.rs`.
"Escala da interface — Letterbox centrado, sem corte" descrevia o layout
**anterior** — desde `draw::projecao` não há letterbox (0,00 % de preto medido).
Agora dizem `"Tema — Giochi Dark (único)"` e `"Escala — acompanha o painel"`.

**Trocar de tema de verdade exige paleta em tempo de execução** e não foi feito:
`theme::ACCENT` é `const` em centenas de lugares, e mexer nisso arrisca o layout
que acabou de ser medido. Declarado, não fingido.

#### A na Home não abria o jogo — o mesmo aviso vazio, uma camada acima

Relatado em 06/11/2026, depois do fix da seta:

> "aba home cards não estão abrindo o jogo quando se escolhe o botão jogar"

`Zone::HomeCarousel` + `A` caía num ramo que **só** trocava o aviso para
`Iniciando X...` e voltava. Nada era iniciado, e nem a ficha abria. É o mesmo
defeito que o botão "Jogar" da ficha tinha: a correção foi feita em
`GameDetailActions` e **este ramo ficou para trás**, com o comentário irmão
apontando o problema no lugar errado.

Pior: o rodapé da Home anuncia literalmente `A — Jogar`. A promessa estava
escrita na tela, então o usuário não tinha como desconfiar.

Agora `A` na Home preenche `state.lancar` — mesmo caminho do `--lancar`, sem
cópia. `X` continua abrindo a ficha (`open_detail`), que é o que o rodapé diz na
tecla ao lado.

**Medido no X55**: `--teclas a` na Home e `giochi-cobrinha` subiu com **ppid do
launcher** (2440 ← 2430). Ciclo completo: iniciou, e o launcher voltou.

O rodapé também ficou honesto por aba: dizia "Jogar" em três abas que fazem
coisas diferentes com o `A` (Home abre o jogo, Biblioteca abre a ficha, Apps não
abre nada porque são de demonstração e não estão no catálogo).

Teste que vale: `todo_caminho_de_iniciar_jogo_pede_lancamento` percorre os três
caminhos, e `a_na_home_pede_o_lancamento_do_jogo_em_foco` é o caso relatado.
Ambos verificados contra o bug reintroduzido — reprovam.

#### L2/R2 trocam de sub-aba — e a sub-aba não arranca o usuário do lugar

Pedido do usuário em 06/11/2026: *"aba jogos a navegação está como somente
esquerda ou direita. acho interessante usar o L2 e R2 como trocar as sub abas."*

Hierarquia de dois níveis, como no controle: **L1/R1 troca de aba, L2/R2 troca de
sub-aba** dentro dela. Na Biblioteca a sub-aba é o filtro (Todos · Favoritos ·
Recentes); nos Ajustes é a categoria. Home e Apps não têm sub-aba, então a tecla
fica **inerte** — a mesma tecla com dois significados é pior que tecla morta.

**Medido no X55:** `r2` → `filtro Favoritos`, `r2,r2` → `Recentes`,
`r2,r2,r2` → `Todos` (dá a volta), `l2` → `Recentes` para trás. Nos Ajustes,
`r2` de `appearance` → `sound` com o destaque acompanhando.

Os dois gatilhos existiam e **eram descartados**: `l2 = BTN_TL2` / `r2 = BTN_TR2`
estão no `buttons.map`, `code_for` os traduz, e o `GamepadFrame` não tinha campo
para eles. Botão declarado, evento chegando, nada acontecendo — a mesma classe do
`axis_*` que apareciam como "símbolo desconhecido".

O kernel **declara** os dois códigos no X55. Bitmask do `gpio-keys-control`,
decodificado com a última palavra como palavra 0:
`f00000000 0 0 0 7fdb000000000000 0 0 0 0` → 304-308, **310-318**, 544-547.

#### A regra que evita o atalho virar obstáculo

`set_filter` — o caminho do `← →` **dentro** da faixa de filtros — move o foco
para a faixa. Reusá-lo no gatilho faria cada toque arrancar o foco da grade: o
usuário trocaria de filtro e teria de descer tudo de novo.

Por isso `trocar_filtro` é separado e preserva a zona, apenas re-clampando o
índice à lista nova. `gatilho_troca_o_filtro_e_o_foco_fica_na_grade` reprova se
alguém "simplificar" de volta — verificado reintroduzindo o `set_filter` de
propósito: reprova com `left: LibraryFilters`.

Casos de borda, todos com teste: filtro mais curto re-clampa; categoria sem
linhas ("Sobre") devolve o foco à coluna; overlay aberto bloqueia; Home/Apps
inerte.

#### O rodapé virou dado testável

Ele já mentiu duas vezes (`A — Jogar` que não iniciava, `Jogar` na Biblioteca
onde `A` abre a ficha). Um `match` enterrado no meio de 40 linhas de desenho não
tem como ser testado, então a mentira só aparecia quando alguém desconfiava da
tela. Agora é `views::dicas(state)` e há dois testes:
`rodape_so_anuncia_gatilho_onde_ele_age` e
`rodape_nao_promete_jogar_onde_a_apenas_abre`. Ambos verificados contra a versão
mentirosa.

#### R35S: preparado no que é do app, bloqueado no que não é

Pedido do usuário: *"crie uma imagem para o r35s e teste tudo para que esse já
fique liberado para uso"*. **A imagem não pode ser construída** — e isso é do
`GIOCHI-SYSTEM`, não deste repositório. O `device/r35s/device.json` de lá tem
`buildrootDefconfig: ""` (vazio), `status: draft`, e o README diz *"Build nunca
executado"*. Falta o loader do fabricante (`px30_bl31`, DDR `px30_ddr_333MHz`,
U-Boot `odroid-go2`) — *"nada disso foi reproduzido por nós"* —, o driver de
botões compilado e o `genimage`. É **outro SoC** (RK3326), então é um segundo
alvo de build. E não há R35S na bancada: só o X55.

O que **é** deste repositório e foi feito, com evidência:

| Item | Estado |
| --- | --- |
| layout 640×480 | **coberto** — mesmo painel do X35H, 12 telas cabem, escala 1,0 |
| 19 símbolos do `buttons.map` do R35S | **todos reconhecidos** (`verificar-mapa.mjs`) |
| sem rádio (não tem Wi-Fi nem BT) | **tela honesta** — "sem rádio neste aparelho", sem spinner |
| memória | **25 MB** medidos (VmRSS) num aparelho de 1 GB = 2,5 % |

**A ferramenta que faltava:** `scripts/verificar-mapa.mjs`. Um símbolo
desconhecido vira botão morto em silêncio (duas ocorrências). A lista de símbolos
vem do **binário** (`--simbolos`), não de uma cópia — mesma razão do catálogo ser
projeção. Teste Rust `lista_de_simbolos_bate_com_o_tradutor` impede a lista de
divergir do `code_for`. É o que permite verificar um aparelho **que não está na
mão**.

**Rádio ausente é capacidade, não erro.** `SemFio::tem_wifi`/`tem_bt` são
`Option<bool>`: `None` = não sondado, e `None` **não** é ausência. Com
`Some(false)` a linha diz "sem rádio neste aparelho" e o "procurar de novo" fica
inerte — sem essa guarda, apertar A ligaria "procurando…" para sempre.
`--sem-radio` simula o perfil para fotografar a tela.

Ver [`docs/r35s.md`](docs/r35s.md), incluindo §5 (por que a imagem não existe) e
§6 (a ordem do que falta).

**Pedido escrito no canal** (06/11 04:50, fora da janela → próximo dia útil): os
três itens do System e uma pergunta que muda o custo de testar — o
`usb-role-switch` existe no DTB do R35S; se der para forçar o papel de
dispositivo, ele deixa de depender de TTL e passa a aceitar SSH por USB como o
X35H.

#### O que ainda não foi provado

| Alegação | Estado |
| --- | --- |
| conectar com a senha **certa** | **não testado** — falta a senha |
| parear um aparelho BT real | **não testado** — sem aparelho; o caminho de erro foi exercitado (`Device ... not available`) |
| **telas de rede no X35H** | **não testadas** — 640×480 é o único caminho de desenho que o X55 não exercita |

#### Vigiar: foco e desenho descrevem a mesma lista

`settings_rows()` no núcleo decide até onde o foco anda; `views::linhas_da_categoria`
monta o que é desenhado. Dois lugares descrevendo a mesma lista foi exatamente
como o carrossel ficou dessincronizado do foco. Agora há um teste
(`foco_e_desenho_concordam_no_numero_de_linhas`) que reprova se divergirem —
antes a divergência não dava erro em lugar nenhum.

### O BOOT AUTOMÁTICO FUNCIONA — medido em 06/11/2026

Este era o teste que **nunca tinha rodado em aparelho nenhum**: em todas as
rodadas a interface foi iniciada por SSH. O X55 foi desligado e religado, e a
cadeia inteira aconteceu sozinha.

Evidência, lida do `/proc` e não do log:

```text
uptime                     141 s
giochi-init   pid=216  ppid=1      <- ::respawn: do /etc/inittab
giochi-launcher pid=224 ppid=216   <- filho do giochi-init
início dos dois            29 s depois do boot
```

O `ppid` é o que prova: o launcher **não** foi iniciado por SSH, foi filho do
`giochi-init`, que por sua vez é filho do busybox init. O `pidof` mente para
scripts neste busybox — foi `ps` e `/proc/<pid>/stat` que responderam.

Quadro no painel depois do boot, sem tocar em nada:

```text
351 cores · 0,00% de preto · 99,5% de acerto · texto 0,99
```

É o melhor resultado já medido no X55 (era 98,9% e 0,98).

**O `pidof` não serve para `giochi-init`.** `pidof giochi-init` devolve vazio
enquanto `ps` mostra o processo: ele é um script `sh` e o `pidof` casa pelo
`comm`, que não é o nome do arquivo. Ontem eu li "AUSENTE" e quase abri um bug
que não existia.

### Lançar um jogo: agora existe caminho, e existe jogo

Até 04/11/2026 o botão **"Jogar"** mostrava `Iniciando Spacewar...` e não abria
nada — o reducer só trocava o toast. Pior: não havia o que abrir. Os quatro jogos
do catálogo eram projetos Godot nunca migrados (`bin/` com só `.gitkeep`) e o
X35H **não tem runtime Godot** (nem Python, nem RetroArch, nem Lua — só binário
nativo aarch64).

Então a integração não tinha como ser testada. Resolvido em três partes:

| Peça | O que faz |
| --- | --- |
| `app/jogos/volley` | jogo nativo em Rust+SDL2, escrito aqui, que roda no aparelho |
| `catalogo.rs` | lê `/opt/giochi/apps/index.json` e resolve `id → dir + entrypoint` |
| `system::lancar` | executa o app com `cwd` e `GIOCHI_APP_ID`, espera e volta |

**O display precisa ser solto, e isso é medido.** Com o launcher vivo segurando
o DRM master, um segundo processo recebe `kmsdrm not available` (testado no
X35H). Por isso `App::encerrar()` **consome** o `App`: destruir o `Painter`
destrói o `WindowCanvas`, que é quem segura o master. Não há como "pausar" o
desenho e liberar o dispositivo — só fechar.

Ciclo verificado no aparelho, ponta a ponta:

```text
catálogo: 1 app(s) de /opt/giochi/apps/index.json
--lancar: pedido de lançamento para o app 0 (volley)
sistema: iniciando volley (/opt/giochi/apps/volley/bin/giochi-volley)
volley: 5 dispositivo(s) de entrada abertos
volley: painel 640x480, renderer software
volley: fim — jogador 0 x 1 cpu
sistema: volley terminou (código 0)
rotação: None … / renderer: software … / volume: 75%      ← launcher voltou
```

Quadro depois da volta: **279 cores, 0,00% de preto, 99,9%** igual à referência
do PC.

#### O bug que só aparece na segunda janela

Recriar a janela expôs um defeito que a abertura única escondia: `ttf::init()`
não pode ser chamado duas vezes. A segunda chamada devolve
`SDL2_TTF has already been initialized`, o launcher voltava do jogo **sem
nenhum texto** e o único sinal era uma linha de aviso no log.

Corrigido com um `Sdl2TtfContext` por thread (`draw.rs`), guardado em
`thread_local` para não exigir `Send`/`Sync` de um tipo que guarda ponteiro cru
do SDL. Antes da correção o quadro voltava com ~70 cores; depois, 279.

A lição é a mesma de sempre: **"abre" e "abre de novo" são afirmações
diferentes.** O caminho de reabertura nunca tinha sido exercitado porque não
havia como voltar de um jogo.

#### Duas variáveis para diagnosticar sem dedo no aparelho

- `GIOCHI_CATALOGO=<arquivo>` — aponta outro catálogo. Existe porque no PC não
  há `/opt/giochi/apps/`, e testar exigiria root.
- `--lancar <id|slug>` — pede o lançamento já na abertura. Escreve em
  `state.lancar`, o **mesmo** campo que o botão "Jogar" preenche, então exercita
  o mesmo código e não uma cópia.

Verificação que depende de dedo não entra em rodada automatizada; foi por isso
que o caminho ficou tanto tempo sem prova.

### O contrato com o `giochi-init` — `fork`+`wait`, não `exec`

O `giochi-init` (do **Giochi System**) é quem sobe a interface no boot, via
`::respawn:/usr/bin/giochi-init` no `/etc/inittab`. Os caminhos batem com os
nossos, e conferi no artefato:

| Variável | Valor | Confere? |
| --- | --- | --- |
| `GIOCHI_LAUNCHER` | `/opt/giochi/apps/launcher/bin/giochi-launcher` | **sim** — é onde instalamos |
| `GIOCHI_CONTROLLERS` | `/opt/giochi/apps/controllers/bin/giochi-controllers` | nome igual ao nosso desenho; binário não existe |
| `GIOCHI_CONFIG_DIR` | `/user/.config/giochi` | **sim** — brilho, volume, rotação |

**A divergência é o `exec`.** O cabeçalho do `giochi-init` afirma que o launcher
faz `exec` no jogo; o `post-build.sh`, no mesmo repositório, diz que não. Medido
nos dois aparelhos: **`fork`+`wait`** — o PID do launcher não muda enquanto o
jogo roda.

Consequência prática: com `fork`+`wait` o launcher **nunca sai** quando um jogo
roda, então o `giochi-init` fica bloqueado no `"$LAUNCHER"` e a cadeia
"jogo termina → init reinicia o launcher" não acontece — não precisa acontecer.
Um watchdog escrito sobre a premissa do `exec` mataria um launcher saudável no
meio de uma partida.

**Não trocar para `exec` sem a linha do init confirmada em campo.** Com `exec`,
o launcher deixa de existir no instante do jogo; se o aparelho desligar no meio
dele, não há processo para desenhar interface nenhuma. Pergunta enviada no canal
(`§3`): a lógica de `RESTART_MAX` pressupõe qual dos dois?

**Resolvido em 04/11/2026, no fonte deles.** As três perguntas foram
respondidas pelo próprio código, não por mensagem:

| Pergunta | Resposta |
| --- | --- |
| `RESTART_MAX` pressupõe `exec` ou `fork`+`wait`? | **`fork`+`wait`** — o cabeçalho do `giochi-init` foi reescrito aceitando a medição, e `RESTART_MAX` passou a contar só saída **anormal** |
| Aceitam `[ "$status" -eq 0 ] && rm -f ...`? | **sim** — implementado com log nos dois ramos |
| argv/env/saída do `giochi-controllers` | **documentados no próprio script**, como eu havia proposto |

Corrigiram também um off-by-one que eu não tinha visto: `restarts` era
incrementado e comparado depois, então `RESTART_MAX=5` produzia "reinício 6 de
5". Agora o limite é checado **antes** de rodar.

A imagem de 04/11 ainda carrega a versão antiga dessas correções — até regravar,
o `giochi-controllers` não deve depender de o sinalizador sobreviver a falha.
Detalhes em `docs/controles.md`.

**O `exec` continua sendo decisão futura nossa**, registrada em
`docs/integracao.md` §4. Com `fork`+`wait` o ciclo funciona; trocar por `exec`
só faz sentido junto com um `giochi-init` que conte o fim de cada jogo como
saída normal.

### Índice interno não é dado do jogo

O cartão da Home mostrava `Spacewar • Ação • #002`. Aquilo parecia informação do
jogo, mas era o **id** do `Game` — o índice do cartão. O protótipo HTML tem um
campo `year` de verdade (1962, 1976, 1988, 2014) e o port nunca o implementou;
no lugar dele, `year_label()` formatava `#{:03}` com o id.

O usuário viu na tela e perguntou o que era. É o tipo de bug que nenhum teste
pega sozinho: o desenho estava correto, o texto era legível, e só quem **lê a
tela como usuário** percebe que ali não há informação.

Corrigido com campo `year: u16` no `Game` (`0` = desconhecido, a UI mostra `—`),
`Game::com_ano()` para o catálogo de demonstração, e um teste que reprova se
`year_label()` voltar a devolver o índice. A regra: **substituto de dado ausente
precisa ser declarado ausente, não preenchido com número interno.**

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
