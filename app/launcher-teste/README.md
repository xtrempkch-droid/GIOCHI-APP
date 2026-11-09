# launcher-teste — o launcher do Giochi sobre a GIOCHI-ENGINE

Porte do launcher para a **giochi-engine do GIOCHI-ENGINE-3D** (2D + 3D + ABI em
C + Lua, versão 0.2.0), em app separado de propósito: se a engine falhar, o
launcher de produção continua intacto.

> O repositório `GIOCHI-ENGINE` separado é a 0.1.0 da mesma engine 2D, com API
> baseada em tempos de vida. Este app usa a 0.2.0, de superfície *C-shaped*
> (textura é `u64`).

```bash
bash scripts/compilar-launcher-teste.sh            # PC (x86-64)
bash scripts/comparar-launcher-teste.sh            # compara no PC
bash scripts/verificar-efeitos.sh                  # prova a camada de efeitos
bash scripts/compilar-launcher-teste-arm64.sh --instalar root@10.55.0.2
bash scripts/aparelho.sh verificar-teste           # compara NO APARELHO
```

## O que é reusado

| Peça | Origem |
| --- | --- |
| estado, foco, reducer, grade, mapper de botões | `giochi-launcher-core` — **o mesmo crate**, sem cópia |
| telas, tema, ícones, dicas do rodapé | `launcher/crates/ui-sdl/src/*` — **os mesmos arquivos**, por `#[path]` |
| catálogo (`index.json`), leitura de `/dev/input`, lançamento de jogo | `launcher/crates/ui-sdl/src/{catalogo,input_evdev,system}.rs` — **os mesmos arquivos**, por `#[path]` |
| janela, texturas, apresentação, blend | **GIOCHI-ENGINE-3D** (`giochi-engine`) |
| `Painter` | `src/draw.rs`, novo |

Reusar as telas por `#[path]` é o que dá sentido ao teste: se o desenho diferir,
a diferença está na engine — nunca numa tela reescrita. Copiar `views.rs`
produziria duas UIs que começam iguais e divergem na primeira correção.

## Resultado medido

Comparação pixel a pixel contra o launcher de produção, mesmo binário de
referência, mesmo catálogo de demonstração:

| Painel | Acerto |
| --- | --- |
| 640×480 | **100,0 %** |
| 800×600 | **100,0 %** |
| 1280×720 | **100,0 %** |
| 720×1280 girado (`Ccw90`) | **100,0 %** |
| **R35S**: 6 telas | **100,0 %** cada |
| **X35H**: 6 telas | **100,0 %** cada |
| **R35S** girado (`GIOCHI_ROTATE=270`) | **100,0 %** |

No aparelho: `renderer: opengles2 (acelerado, textura máx 8192x8192)` e **23 MB**
de RSS.

## O que a engine ganhou por causa deste porte

Quatro primitivas que não existiam, todas com teste de pixel:

| Adição | Por que era necessária |
| --- | --- |
| `draw_line` | ícones vetoriais e cantos arredondados são polilinhas |
| `stroke_rect` | contorno de 1 px sem desenhar quatro retângulos |
| `fill_rects` | cantos e círculos são varreduras de 1 px — um lote por elemento |
| `set_blend_mode` | alfa do canvas |
| textura nasce em `Blend` | sem isso **o glifo sai como bloco sólido**: quem governa o alfa em `RenderCopy` é a textura, não o canvas |
| `Window::renderer_info` | sem ele, "GLES funciona" é suposição: `libEGL` em `/proc/maps` não distingue renderer |

A última foi encontrada pelo próprio porte: o quadro saía com 97 cores em vez de
321, e a leitura de um pixel depois do desenho mostrou a cor do texto opaca onde
deveria haver transparência.

## Ele é um launcher, não só um banco de prova de desenho

Até 09/11/2026 este app desenhava as telas e mais nada: o laço era
`update(TickHud)` + desenhar + dormir, sem ler `/dev/input` e sem catálogo. Um
binário assim **não pode ser a interface do aparelho** — a Home sai vazia e
nenhum botão responde.

Hoje ele lê o catálogo do sistema, lê o controle e abre jogos:

| Peça | Como |
| --- | --- |
| catálogo | `catalogo::Catalogo::carregar` de `/opt/giochi/apps/index.json` (`GIOCHI_CATALOGO` sobrepõe) |
| entrada | `input_evdev::EvdevInput` + `gamepad::Mapper` do núcleo — borda e repetição iguais às da produção |
| lançamento | `system::lancar` — **destrói o `Painter` antes**, porque o KMSDRM é exclusivo |

O `drop(painter)` não é detalhe: com o launcher vivo segurando o DRM master, o
filho responde `kmsdrm not available`. Medido no X35H em 09/11/2026, com o
laço novo:

```text
pid=2103 giochi-launcher-teste  drm=0    ← soltou o dispositivo
pid=2112 giochi-neon-drift      drm=3    ppid=2103
```

E o `estado_inicial` passou a usar **o mesmo** catálogo que resolve o `alvo` no
lançamento: uma leitura só. Ler duas vezes criaria dois lugares que podem
discordar — o cartão na tela e o executável aberto sairiam de fontes
diferentes.

`--demo` continua existindo e **não** lê o catálogo: é o caminho das capturas
de tela, e uma captura no PC tem de medir o mesmo que no aparelho.

### No aparelho, ele é o launcher

`/opt/giochi/apps/launcher/bin/giochi-launcher` é um **wrapper** de 30 linhas
que faz `exec` no launcher-teste com `--efeitos`. O binário de produção fica ao
lado (`giochi-launcher.producao`) e volta com um comando:

```sh
touch /run/giochi/launcher-producao && kill <pid do launcher>
rm /run/giochi/launcher-producao        # volta ao launcher-teste no respawn
```

Wrapper e não symlink porque a escolha tem de ser **legível no aparelho** — um
symlink exige `readlink` para dizer o que está rodando, e `--efeitos` é
argumento, que symlink não carrega.

## Camada de efeitos (`--efeitos`)

Oito efeitos desenhados por cima das telas, **desligados por padrão**.

```bash
app/launcher-teste/target/release/giochi-launcher-teste --demo --efeitos
GIOCHI_EFEITOS=1 giochi-launcher-teste     # o caminho do aparelho
GIOCHI_EFEITOS_SO=brasas                    # só um efeito, para medir
```

| Efeito | O que é | Primitiva exercitada |
| --- | --- | --- |
| `brasas` | partículas de luz subindo, na cor do jogo em foco | `fill_circle` → `fill_rects` |
| `brilho` | estrelinhas de 4 pontas acendendo e apagando | `fill_rects` |
| `varredura` | banda de luz descendo a tela a cada 4 s | `draw_rect` em faixas |
| `vinheta` | topo e base escurecidos em 5 faixas | `fill_rects` |
| `scanlines` | textura de CRT | **um** `fill_rects` com ~120 retângulos |
| `halo` | brilho pulsante em volta do item em foco | `fill_rects` em 4 bandas |
| `rastro` | riscos horizontais a cada comando | `draw_line` |
| `borda` | tira de luz respirando na borda do painel | `fill_rects` |

### Por que são opcionais, e isso não é timidez

Este app existe para **uma** pergunta: *o launcher desenhado sobre a
GIOCHI-ENGINE sai igual ao de produção?* A resposta é medida por comparação
pixel a pixel e hoje dá **100,0 %** nos quatro tamanhos e nas seis telas do
aparelho. Um efeito desenhado por padrão destruiria a medida — e a medida é a
razão de o app existir.

Então o desenho das telas não mudou **uma linha**: os efeitos são uma camada
separada, desenhada depois do `hud()` e antes do `present()`. Com a flag
desligada, `comparar-launcher-teste.sh` e `aparelho.sh verificar-teste`
continuam dando 100,0 %.

### O que a camada custou de verdade (X35H, 640×480)

| | ms/quadro | fps | chamadas |
| --- | --- | --- | --- |
| sem efeitos | 24,18 | **41,4** | 0 |
| com efeitos | 27,26 | **36,7** | 27 |

**+3,1 ms por −4,7 fps.** O caminho até esse número é a parte útil:

| Versão | Custo | Por quê |
| --- | --- | --- |
| uma chamada por brasa e `stroke_round_rect` no halo | **+7,3 ms** | 162 chamadas, e o halo sozinho custava +6,9 ms |
| batelada por faixa de alfa | **+3,1 ms** | 27 chamadas |

O halo era o campeão: `stroke_round_rect` desenha 4 arestas e 4 arcos, e cada
arco é uma polilinha de ~30 `draw_line` — 3 contornos custavam **+6,9 ms**, o
efeito mais caro de todos por uma margem de 4×, para uma borda que ninguém vê.
Quatro bandas preenchidas num `fill_rects` por camada leem igual e custam +0,4 ms.

A lição que vale além daqui: **`draw_rect` é uma chamada de backend por
retângulo**. 130 brasas desenhadas uma a uma custavam 129 chamadas; agrupadas em
8 faixas de alfa custam 8. O custo que sobra é a área em tela, não a chamada.

### O contador de desenho media a si mesmo

A primeira versão contava as chamadas **dentro do módulo de efeitos** e
imprimia "25 desenhos". Uma mutação que desfazia a batelada (uma chamada por
partícula) continuava imprimindo 25, porque o contador somava *quantas vezes eu
pedi um lote* — media a intenção, não o custo.

Hoje o contador vive no `Painter`, no ponto em que se chama o `RenderContext`, e
a camada lê a diferença. Com a batelada desfeita ele acusa **182 chamadas** e
`verificar-efeitos.sh` reprova. Reintroduzir o contador local faz o teste parar
de morder, e é isso que ele existe para impedir.

## Limites conhecidos

- **Os efeitos não são o produto.** São uma camada de demonstração da engine:
  ficam desligados por padrão e não entram em nenhuma medida de fidelidade.
- **Entrada é por evdev, e o que falta é o resto.** Botões, d-pad e analógico
  digital chegam; o que este app **não** tem, comparado ao de produção, é:
  dormir/acordar, brilho, volume/mixer, menu de energia, leitura de bateria e
  temperatura, e as telas de Wi-Fi/Bluetooth. Por isso ele continua em
  `scaffold` — "roda na tela" e "substitui o launcher" não são a mesma
  afirmação.
- **SIGTERM não o encerra.** O laço não bombeia eventos do SDL, então o sinal
  só posta `SDL_QUIT` na fila e o processo segue vivo. Para encerrar é `kill -9`
  (ou o `giochi-teste`, que já escala). Custou uma sessão de diagnóstico no
  §25 do `AGENTS.md` e voltou a morder em 09/11, agora com o supervisor
  desistindo por `RESTART_MAX`.
- **Depende do repositório irmão** `../GIOCHI-ENGINE-3D` (clone ao lado deste).
- Sem `capa.png`: o app está em `scaffold`.
