# Compilar o launcher

O launcher é a única parte deste repositório que precisa de toolchain: `node` para
o resto, `cargo` + SDL2 para ele.

```bash
bash scripts/compilar-launcher.sh          # cargo build --workspace
bash scripts/compilar-launcher.sh --release
```

O script resolve o SDL2 e, no fim, imprime as duas linhas de ambiente que faltam:
`LD_LIBRARY_PATH` para rodar e `--dump-frame` para conferir o desenho sem tela.

## O que o `cargo` encontra sozinho e o que não encontra

| Camada | Precisa de | Erro se faltar |
| --- | --- | --- |
| `crates/core` | nada | — |
| `crates/ui-sdl` — compilação | nada além do crate | — |
| `crates/ui-sdl` — **link** | `libSDL2.so`, `libSDL2_ttf.so` | `rust-lld: unable to find library -lSDL2` |

O `rustc` compila os dois crates sem SDL instalado; quem falha é o **link**. É uma
distinção que engana: a mensagem parece erro de código, mas é falta de biblioteca.

## Sem root

Em máquina onde não se pode instalar pacote, o script extrai os `.deb` num prefixo
local e aponta as variáveis para lá:

```text
~/.cache/giochi-sdl2/root/            # GIOCHI_SDL2_PREFIX
└── usr/lib/x86_64-linux-gnu/
    ├── libSDL2.so -> libSDL2-2.0.so.0
    ├── libSDL2-2.0.so.0 -> /usr/lib/x86_64-linux-gnu/libSDL2-2.0.so.0
    ├── libSDL2_ttf.so -> libSDL2_ttf-2.0.so.0.2400.0
    └── pkgconfig/{sdl2,SDL2_ttf}.pc
```

Três armadilhas que custaram tempo:

1. **`libSDL2.so` fica pendurado.** Ele aponta para `libSDL2-2.0.so.0`, que vem do
   pacote de *runtime*, não do `-dev`. Pendurado, o linker ignora o symlink e cai
   no `libSDL2.a` — aí falha com dezenas de símbolos do PulseAudio. O script
   aponta o link para a biblioteca do sistema.
2. **`pkg-config --exists sdl2 SDL2_ttf` mente.** O `SDL2_ttf.pc` declara
   `harfbuzz` e `freetype2` em `Requires.private`, e o pkg-config valida isso
   mesmo com link dinâmico. Use `--modversion`, que é o que o `sdl2-sys` faz.
3. **`LD_LIBRARY_PATH` na hora de rodar.** Linkar certo não basta: o carregador
   precisa achar `libSDL2_ttf.so.0` se ela não estiver no sistema.

## Fonte

O TTF não está no repositório (é arte de terceiro, e o `.gitignore` não o traz).
O launcher procura, nesta ordem:

```text
assets/fonts/DejaVuSans.ttf
/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
/usr/share/fonts/TTF/DejaVuSans.ttf
app/launcher/assets/fonts/DejaVuSans.ttf
```

`--font <arquivo>` força outro caminho. Sem fonte, o launcher **roda e desenha**,
só não escreve texto — aviso único no console.

## Conferir o desenho sem tela

```bash
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  ./target/debug/giochi-launcher --dump-frame /tmp/frame.bmp
```

`--dump-frame` desenha a Home, grava um BMP de 1280×720 e sai com código 0. Foi
assim que a UI foi verificada pela primeira vez: sem aparelho, sem X, sem
screenshot. Um BMP de verdade tem centenas de cores distintas; um quadro em branco
ou preto é sinal de que algo parou antes do primeiro `render`.
