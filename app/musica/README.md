# Música

Tocador de música do Giochi, sobre a **GIOCHI-ENGINE** (o crate 2D do
`GIOCHI-ENGINE-3D`). É o primeiro app `image` deste repositório que **não é
interface**: aparece na aba de apps do launcher e abre de lá.

```bash
# no PC (precisa do repositório irmão GIOCHI-ENGINE-3D ao lado)
bash scripts/compilar-musica.sh                       # compila
bash scripts/compilar-musica.sh --testes              # 120 + 38 testes
app/musica/target/debug/giochi-musica --demo          # biblioteca sintética

# no aparelho
bash scripts/compilar-musica-arm64.sh --instalar      # compila aarch64 e instala
bash scripts/aparelho.sh catalogo                     # publica o cartão
```

## O que ele faz

- Lê uma pasta de música — `/user/musica` no aparelho, `~/Música` no PC — e
  lista **todos** os formatos que o `SDL_mixer` da imagem costuma decodificar:
  `wav, ogg, flac, mp3, mod, xm, it, s3m`.
- Ordem **natural**: `Faixa 2` antes de `Faixa 10`. Ordem de bytes poria "10"
  antes de "2" e a playlist de um álbum sairia fora de ordem.
- Carrossel de capas com **volta** nas duas pontas, foco com borda e selo de
  "está no mixer".
- Capa **gerada** do nome do arquivo (FNV-1a → matiz): mesma faixa, mesma cara,
  nenhum PNG no pacote.
- Transporte completo: tocar/pausar, próxima, anterior, parar, volume em passos
  de 5 — pelo mixer da engine (`Mix_Music`, que faz *streaming*: um álbum não é
  carregado na memória).
- Tela de pasta vazia que **diz o motivo** e o caminho procurado.
- Roda com o controle embutido (evdev, o mesmo leitor e o mesmo mapa de botões
  do launcher), teclado e as teclas de volume do aparelho.

## Controles

| Botão | Ação |
| --- | --- |
| ← → | escolhe a faixa |
| A | toca a faixa em foco · de novo pausa · de novo retoma |
| X | próxima |
| Y | anterior |
| B | para (mantém a seleção) |
| LB / RB, ↑ ↓, teclas de volume | volume −/+ (5%) |
| SELECT | relê a pasta |
| START | sai |

No teclado: setas, `A`/`D`, espaço/Enter, `N`, `P`, `S`, `+`/`-`, `R`, `Esc`.

## Como está organizado

```text
app/musica/
├── crates/core/          # giochi-musica-core — decisão, sem SDL (120 testes)
│   └── src/              #   faixa, biblioteca, carrossel, capa, texto,
│                         #   layout, rotação, transporte, reducer
├── src/                  # o binário
│   ├── main.rs           #   flags, laço, execução dos efeitos
│   ├── pintor.rs         #   primitivas + texto + rotação sobre a engine
│   ├── tela.rs           #   as telas
│   ├── arte.rs           #   capa em pixels RGBA + equalizador
│   ├── audio.rs          #   o mixer, encapsulado
│   └── entrada.rs        #   evdev + teclado → ação
├── crates/core/Cargo.toml  # sem NENHUMA dependência (é o que o CI testa)
└── Cargo.toml              # engine + sdl2 + mapa de botões do launcher
```

A divisão não é estilo: `crates/core` não conhece pixel nem placa de som, então
roda `cargo test` em qualquer runner. O binário depende da engine, que depende do
**repositório irmão** — e isso o CI não tem.

## O que foi medido (15/11/2026)

| Medida | Resultado |
| --- | --- |
| `crates/core` | **120 testes**, sem SDL |
| binário (testes) | **38 testes** (arte, entrada, pintor, flags) |
| quadro 640×480 contra o PC | **100,00% idênticos** (mesma pasta, R35S × x86-64) |
| quadro 720×1280 girado contra 1280×720 | **99,96%** (só bordas de retângulo girado) |
| desenhos por quadro | **96** (640×480) a **112** (1080p) |
| áudio no R35S | mixer 2.6.3, `Playing → Stopped` em **2,5 s** num WAV de 2,5 s |
| binário aarch64 | 750 KB, `NEEDED`: SDL2, SDL2_image, SDL2_ttf, SDL2_mixer, libc |

O que **não** foi medido:

- **som audível pelo alto-falante** — o teste acima prova que o mixer consome o
  fluxo (o relógio dele anda exatamente a duração do arquivo), não que o
  amplificador está ligado;
- **giro num aparelho retrato real** — o X55 não estava na bancada; o giro foi
  verificado contra a rotação do próprio quadro;
- **abrir a janela no painel** — na bancada deu `kmsdrm not available` porque o
  DRM master é exclusivo e o `::respawn:` do init reiniciou o launcher antes; no
  fluxo de produção é o **launcher** que fecha a janela e libera o DRM antes do
  `fork`, e isso não se aciona por SSH sem apertar `A` no aparelho.

## O que falta

| Item | Estado |
| --- | --- |
| barra de progresso | **não existe** — o `SDL_mixer` não informa a posição da música. A barra mostraria um número inventado |
| ler ID3/FLAC picture | **não existe** — a capa é derivada do nome; embutir metadados exigiria um decodificador por formato |
| duração de MP3/OGG/FLAC | **`—`** — só o WAV declara o tamanho no cabeçalho, e adivinhar seria pior |
| playlists, busca, favoritos | não existem |
| trocar de pasta pela interface | não existe — `--pasta` ou `GIOCHI_MUSICA` |
| launcher perguntar a licença antes de abrir | não existe aqui também (ver `AGENTS.md` §16) |

## Dependência de imagem: `SDL2_mixer`

O binário **pede `libSDL2_mixer-2.0.so.0`** no carregamento. Medido na imagem do
R35S em 15/11/2026: **está presente** (`2.6.3`), junto de `libasound` e
`/dev/snd`. Sem ela, o app não sobe — biblioteca ausente é falha de carga, não
degradação (§13 do `AGENTS.md`). Antes de empacotar para um aparelho novo:

```bash
ssh root@<ip> 'ls /usr/lib/libSDL2_mixer*'
```

O `README` do launcher dizia que `mixer` não era usado pela imagem; isso valia
para o launcher, que não usa. Para este app é requisito.

## Flags de diagnóstico

```text
--pasta <dir>      pasta de música
--demo             biblioteca sintética (captura de tela sem disco)
--tocar <n>        toca a faixa n na abertura
--volume <0-100>   volume inicial
--sem-som          não abre o mixer
--tamanho LxA      força o tamanho da janela
--dump-frame <f>   grava um BMP e sai
--teclas <lista>   aplica ações e sai (right,right,a,…)
--bench <n>        mede ms/quadro e desenhos
--testar-som <s>   toca a faixa em foco e acompanha o estado do mixer
```

`--testar-som` é o que separa "o mixer aceitou a faixa" de "o áudio está
correndo": se um WAV de 2,5 s vira `Stopped` sozinho em 2,5 s, o mixer está
consumindo o fluxo.
