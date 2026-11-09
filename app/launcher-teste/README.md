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
bash scripts/compilar-launcher-teste-arm64.sh --instalar root@10.55.0.2
bash scripts/aparelho.sh verificar-teste           # compara NO APARELHO
```

## O que é reusado

| Peça | Origem |
| --- | --- |
| estado, foco, reducer, grade | `giochi-launcher-core` — **o mesmo crate**, sem cópia |
| telas, tema, ícones, dicas do rodapé | `launcher/crates/ui-sdl/src/*` — **os mesmos arquivos**, por `#[path]` |
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

## Limites conhecidos

- **Entrada por evdev não foi portada.** Só `--teclas` (comandos diretos no
  núcleo). O launcher de produção lê `/dev/input` com thread por dispositivo;
  mover isso para a engine é o próximo passo.
- **Sem bomba de eventos**: o `Sdl` da engine não expõe a do SDL. Quem cuida de
  evento no uso "completo" é `Engine`; aqui o laço desenha e dorme.
- **Depende do repositório irmão** `../GIOCHI-ENGINE` (clone ao lado deste).
- Sem `capa.png` e sem binário versionado: o app está em `scaffold`.
