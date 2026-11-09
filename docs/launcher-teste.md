# launcher-teste — o launcher sobre a GIOCHI-ENGINE

Porte do launcher para a **giochi-engine do GIOCHI-ENGINE-3D** — o workspace que
tem 2D, 3D, ABI em C e Lua — em app separado para não arriscar o launcher de
produção. Medido em 07/11/2026.

> **Qual repositório de engine.** O `GIOCHI-ENGINE` separado é a versão **0.1.0**
> da mesma engine 2D, com API baseada em tempos de vida (`RenderContext<'!>`,
> `Texture<'a>`). O `GIOCHI-ENGINE-3D` é o **0.2.0**: mesmo 2D, em superfície
> *C-shaped* (textura é `u64`, `#[repr(C)]`, sem lifetime), **mais** o 3D
> (`giochi-engine-3d`), a **ABI em C** (`libgiochi.so` + `giochi.h`) e o
> **Lua** (`giochi-engine-lua`). Este app usa o 0.2.0.

```bash
bash scripts/compilar-launcher-teste.sh                  # PC (x86-64)
bash scripts/comparar-launcher-teste.sh                  # compara no PC
bash scripts/compilar-launcher-teste-arm64.sh --instalar root@10.55.0.2
bash scripts/aparelho.sh verificar-teste                 # compara NO APARELHO
```

## Por que um app separado

O launcher de produção roda no aparelho e tem 223 testes e um histórico de
defeitos medidos em campo — rotação invertida, fonte com metade do tamanho,
texto a 90° da interface, `ttf::init` chamado duas vezes. Trocar a camada de
desenho dele para "experimentar" uma engine colocaria tudo isso em risco.

Aqui a engine é exercitada num binário próprio: **se ela falhar, o launcher real
continua intacto**.

## O que é reusado, e o que é novo

| Peça | Origem |
| --- | --- |
| estado, foco, reducer, grade, dicas | `giochi-launcher-core` — **o mesmo crate** |
| `views.rs`, `theme.rs`, `icons.rs` | `launcher/crates/ui-sdl/src/*` — **os mesmos arquivos**, via `#[path]` |
| janela, textura, apresentação, blend | **GIOCHI-ENGINE-3D**, crate `giochi-engine` |
| `Painter` | novo: `app/launcher-teste/src/draw.rs` |

`#[path]` é a decisão central:

```rust
#[path = "../../launcher/crates/ui-sdl/src/views.rs"]
mod views;
```

Isso faz o módulo apontar para o arquivo real, sem cópia. Consequência: **se o
desenho diferir, a diferença está na engine** — nunca em uma tela reescrita.
Copiar as 2.000 linhas de `views.rs` produziria duas UIs que começam iguais e
divergem na primeira correção; foi assim que o carrossel ficou fora de sincronia
com o foco e que o rodapé passou a prometer `A — Jogar` onde `A` abria a ficha.

A matemática de rotação (`rotate_point`, `rotate_rect`, `projecao`) foi
**copiada** com os mesmos testes. Compartilhá-la exigiria extrair um módulo do
launcher de produção, e o pedido desta app é justamente não mexer nele.

## Resultado medido

Mesma entrada dos dois lados (catálogo `--demo`), comparação pixel a pixel com
`scripts/verificar-frame.mjs`:

| Caso | Acerto |
| --- | --- |
| 640×480 | **100,0 %** |
| 800×600 | **100,0 %** |
| 1280×720 | **100,0 %** |
| 720×1280 retrato (`Ccw90`) | **100,0 %** |
| abas: home · biblioteca · apps · ajustes | **100,0 %** cada |
| overlays: busca · rápidos · detalhe · energia | **100,0 %** cada |
| navegação `r,right,down` | **100,0 %** |
| **no R35S** (`verificar-teste`): home · biblioteca · apps · ajustes · busca · detalhe | **100,0 %** cada |
| **no R35S, girado** (`GIOCHI_ROTATE=270`, projeto 480×640) | **100,0 %** |

100,0 % aqui significa **byte a byte**: cada um dos 34.240 / 53.400 / 102.480
pontos amostrados é igual. Não é "parecido".

## O que a engine ganhou por causa deste porte

Cinco lacunas, todas com teste de pixel na engine (de 45 para 48 testes):

| Adição | Por que era necessária |
| --- | --- |
| `RenderContext::draw_line` | ícones vetoriais e cantos arredondados são polilinhas |
| `RenderContext::stroke_rect` | contorno de 1 px |
| `RenderContext::fill_rects` | cantos e círculos são varreduras de 1 px; um lote por elemento |
| `RenderContext::set_blend_mode` + `Window::set_blend_mode` | alfa do canvas |
| textura nasce em `BlendMode::Blend` | **sem isso o glifo sai como bloco sólido** |

A última foi encontrada pelo próprio porte, e o sintoma engana: o quadro saía com
**97 cores em vez de 321**, e nada no log dizia por quê. A leitura de um pixel
logo depois do desenho mostrou a cor do texto **opaca** onde deveria haver
transparência — `[241,245,249,255]` num canto que era fundo.

A causa é uma pegadinha do SDL2: em `SDL_RenderCopy`, quem governa o alfa é o
modo da **textura**, não o do canvas. O `set_blend_mode` do destino não resolve.
Medido: com o `set_blend_mode` da textura removido de propósito, o teste novo
reprova com `veio (255,0,0)` — vermelho cru em vez da média.

## Recorte sem `set_clip_rect`

O `Painter` de produção usa `set_clip_rect` do canvas, que a engine não expõe. O
porte recorta **em espaço de projeto**, que é melhor para o giro:

- retângulos e varreduras: interseção antes de desenhar;
- texto: interseção aplicada à **origem** da textura (o `source` do
  `draw_texture_ex`), o que recorta o glifo sem recortar o canvas;
- linhas: rasterização por Bresenham ponto a ponto quando há recorte ativo.

A tela de busca é a que usa recorte, e é 100 % idêntica. Ou seja: o caminho de
software reproduz o do SDL.

## No aparelho (R35S, imagem de 07/11)

```bash
bash scripts/compilar-launcher-teste-arm64.sh --instalar root@10.55.0.2
bash scripts/aparelho.sh verificar-teste
```

```text
painel 640x480, 6 tela(s)
  home: 100,0% igual ao launcher de produção
  biblioteca: 100,0%   apps: 100,0%   ajustes: 100,0%
  busca: 100,0%        detalhe: 100,0%
```

O `verificar-teste` mata o launcher **uma vez**, roda as seis telas numa sessão
só e devolve o aparelho ao normal. A primeira versão matava por tela (12 mortes
em sequência) e o `giochi-init` desistiu de reiniciar por `RESTART_MAX` — medido,
e é o motivo de o script ser de um kill só.

### Ver o launcher-teste na tela do aparelho (`giochi-teste`)

`verificar-teste` responde "desenha igual?"; para **olhar** a interface na tela
do handheld existe um utilitário instalado junto com o binário:

```bash
/opt/giochi/apps/launcher-teste/giochi-teste abrir     # troca para o launcher-teste
/opt/giochi/apps/launcher-teste/giochi-teste fechar    # devolve o de produção
/opt/giochi/apps/launcher-teste/giochi-teste tela home # grava um BMP e imprime o caminho
/opt/giochi/apps/launcher-teste/giochi-teste estado    # quem está no DRM, e a cota
```

Ele escreve a ordem que não é óbvia (`touch /run/giochi/dev-free-console` →
matar o de produção → `setsid nohup` no de teste) e **desfaz tudo** no `fechar`,
inclusive a flag — esquecê-la deixava o aparelho sem interface depois do reboot.

Duas armadilhas medidas em 09/11/2026, ambas tratadas lá dentro:

1. **SIGTERM não mata o launcher-teste.** O SDL instala handler que só posta
   `SDL_QUIT` na fila de eventos; este binário não bombeia eventos (a entrada por
   evdev não foi portada), então o `kill` era engolido e o processo continuava
   segurando o DRM. Com o DRM ocupado, o `giochi-init` subia o launcher de
   produção, ele morria com `kmsdrm not available` e a cota era consumida até
   `RESTART_MAX` — **aparelho sem interface**. Agora o utilitário escala para
   `kill -9` e, se o supervisor já tiver desistido, mata o `giochi-init` (o
   `::respawn:` do inittab devolve a cota) e espera o launcher voltar.
2. **A flag só sai depois de o DRM estar livre.** Na ordem inversa, o
   `giochi-init` sobe o launcher contra um DRM ainda ocupado — que é exatamente
   a sequência que gasta a cota.

E a terceira, que só apareceu porque o dono do projeto estava usando o aparelho:

3. **O dono do DRM pode ser um jogo, não um launcher.** O launcher abriu o
   `Equilibrio` pelo cartão e ficou em `fork`+`wait`: vivo, **sem** descritor de
   DRM. Matar o launcher nesse estado deixa o jogo **órfão e dono do
   dispositivo** — o launcher-teste então morre com `kmsdrm not available` e o
   `giochi-init` gasta as cinco tentativas. Por isso as perguntas certas são
   "quem segura `card0`/`renderD`?" e "existe app aberto?"; `PID do launcher`
   não responde nenhuma das duas. O utilitário **recusa** com o nome do processo
   e só encerra com `abrir forcar`; `fechar` também não devolve o console
   enquanto um app estiver com o DRM.

O mesmo defeito existia no `verificar-teste` do `aparelho.sh`: ele matava o
launcher, esperava `sleep 2` e rodava. Agora ele checa o DRM **antes** de tocar
em nada (e recusa com o nome do app), escala para `kill -9` e espera o
descritor liberar em vez de confiar no relógio.

Estado final medido em 09/11/2026 (R35S, imagem de 07/11):

```text
abrir   → pid=3282 drm=6    (launcher-teste na tela, opengles2 acelerado)
fechar  → pid=4940 drm=6    (launcher de produção, supervisor ativo)
verificar-teste (6 telas) → 100,0% em home, biblioteca, apps, ajustes, busca e detalhe
```

### Backend: medido, não presumido

```text
ausente        renderer: opengles2 (acelerado, vsync; textura máx 8192x8192)
GIOCHI_GLES=1  renderer: opengles2 (idêntico)
GIOCHI_GLES=0  renderer: software  (software, vsync; textura máx 0x0)
```

Isso só foi possível porque o **`Window::renderer_info` entrou na engine** por
causa deste porte. Antes, o único sinal disponível era `libEGL`/`libGLESv2` em
`/proc/<pid>/maps` — que **não distingue nada**: o driver de vídeo `kmsdrm` usa
EGL/GBM para apresentar seja qual for o renderer. O teste
`renderer_info_reports_the_backend_that_opened` exige nome real e que o backend
declarado aceite criar textura do tamanho usado.

### Desempenho e memória

| Medida | Valor |
| --- | --- |
| RAM (VmRSS = VmHWM) | **23 MB** |
| descritores de DRM | **6** |
| desenho | 640×480 a 100 % igual ao launcher de produção |

O launcher de produção mede 25 MB no mesmo aparelho: a engine não inflou o
processo.

## Limites conhecidos

| Alegação | Estado |
| --- | --- |
| desenho idêntico no PC | **provado**, 4 painéis + 4 abas + 4 overlays |
| **roda no aparelho** | **provado**: 6 telas a 100 % no R35S, e gira |
| backend acelerado no aparelho | **medido**: `opengles2`, textura 8192×8192 |
| entrada por evdev | **não portada**: só `--teclas` |
| bomba de eventos | **não usada**: o `Sdl` da engine não a expõe |
| telas de rede (`wifi`, `senha`, `bluetooth`) | **não implementadas** no `--tela` (precisam de dados de exemplo) |
| desempenho | **não medido** |

### O que foi preciso acrescentar no crate da engine

O crate 0.2.0 tinha menos superfície de desenho que o 0.1.0: `clear`, `draw_rect`,
`draw_texture` (só `copy`, sem ângulo), `draw_sprite` e `read_pixels`. Faltava
tudo o que uma UI precisa. Entraram lá, com teste de pixel:

| Adição | Por que |
| --- | --- |
| `stroke_rect` | contorno de 1 px; quatro `draw_rect` não fecham o canto |
| `fill_rects` | cantos e círculos são varreduras de 1 px — um lote por elemento |
| `draw_line` | ícones e arcos são polilinhas |
| `draw_texture_ex` | **rotação** de textura; sem ela o texto não gira com o painel |
| `Blend` no canvas | textura já nascia em `Blend`; o **alvo** não, e o alfa saía opaco |
| `renderer_name` / `renderer_is_accelerated` | o log precisa dizer qual backend abriu |

O texto continua fora da engine: o `Painter` rasteriza com `sdl2::ttf`, sobe o
RGBA com `create_texture_from_rgba`, desenha com `draw_texture_ex` e libera com
`destroy_texture` — cada chamada, sem cache. A engine não tem noção de fonte, e
não é este porte que vai dar uma.

### Como o cross-compile resolve o que a engine exige

`compilar-launcher-teste-arm64.sh` reusa o sysroot, o `libSDL2.so`, o
`libSDL2_ttf.so` e o wrapper de link que `compilar-launcher-arm64.sh` já monta, e
acrescenta as duas bibliotecas que só a engine pede — `SDL2_image` e
`SDL2_mixer` — copiadas do aparelho. Como o aparelho é Buildroot e **não tem os
`.pc`** (não há pacote `-dev`), o script **gera** os quatro `.pc` apontando para o
cache; sem eles o `pkg-config` do crate falha com erro que fala de
cross-compilation, não de SDL.

`PKG_CONFIG_ALLOW_CROSS=1` é obrigatório: sem ele o crate `pkg-config` recusa
trabalhar cross.

O `libSDL2_mixer` **não** aparece nas `NEEDED` do binário: o linker descarta com
`--as-needed`, porque o app não usa áudio. Ficam cinco: `libSDL2`, `libSDL2_image`,
`libSDL2_ttf`, `libgcc_s`, `libc` — todas na imagem.
