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

## aarch64 — o aparelho

```bash
bash scripts/compilar-launcher-arm64.sh                        # compila
bash scripts/compilar-launcher-arm64.sh --instalar root@<ip>   # e instala
```

O script põe o binário em `app/launcher/bin/giochi-launcher` (que é o
`entrypoint` do `metadata.json`) e, com `--instalar`, em
`/opt/giochi/apps/launcher/bin/` no aparelho — o caminho que o `giochi-init`
espera para reiniciar o launcher quando um jogo termina.

### As quatro armadilhas do cross-compile

Nenhuma delas dá erro de código: todas falham no **link** ou no **runtime**.

| Sintoma | Causa | Solução |
| --- | --- | --- |
| `unable to find library -lgcc_s` | o sysroot do Ubuntu não traz `libgcc_s` | copiar do aparelho |
| `cannot find entry symbol _start` | `rust-lld` não é driver cc: não adiciona `crt1.o` | wrapper injeta `Scrt1.o`/`crti.o`/`crtn.o` |
| binário compila e dá **segfault imediato** | falta `PT_INTERP`: sem `--dynamic-linker` o kernel não sabe quem carregar | wrapper passa `--dynamic-linker /lib/ld-linux-aarch64.so.1` |
| `undefined reference: strlcat@GLIBC_2.38` | `libSDL2` e `libc` do aparelho são glibc **2.38**; o sysroot do Ubuntu é 2.35 | copiar `libc.so.6`/`libm.so.6` **do aparelho** |

O ponto do meio é o mais enganoso: `file` diz "ELF aarch64 válido" e o binário
simplesmente morre sem mensagem. Conferir com:

```bash
readelf -l app/launcher/bin/giochi-launcher | grep INTERP
```

Tem que aparecer `INTERP` com `/lib/ld-linux-aarch64.so.1`.

### Por que copiar as libs do aparelho

Parece errado linkar contra a biblioteca do alvo, mas é o que garante
compatibilidade: o aparelho é o **único** alvo que importa, e o glibc dele é
mais novo que o do sysroot do Ubuntu. Linkar contra 2.35 quando o `libSDL2`
instalado foi construído contra 2.38 produz símbolos que não resolvem.

O sysroot do Ubuntu continua sendo usado para os `crt*.o` e os stubs de libc
genéricos — só `libc.so.6`, `libm.so.6` e `libSDL2` vêm do aparelho.

## Sem SDL2_ttf: a feature `texto`

O aparelho **não tem** `libSDL2_ttf`, então o build para ele é:

```bash
cargo build -p giochi-launcher --no-default-features
```

Sem a feature `texto`, tudo é desenhado menos os glifos. O detalhamento do que
pedir ao Giochi System está em
[dependencias-aparelho.md](dependencias-aparelho.md).

## Rotação para painel retrato

O painel do aparelho é **720×1280 (retrato)** e a UI é projetada em **1280×720**.
Sem girar, o resultado é a interface deitada dentro de tarjas pretas — medido: o
centro da tela saía `[0,0,0]`, ~40 % da área era letterbox.

A rotação é automática: se o output é retrato, o `Painter` gira 90° no sentido
horário. Forçar com `GIOCHI_ROTATE=90` ou desligar com `GIOCHI_ROTATE=0`.

Implementação: as primitivas recebem coordenadas de projeto e são mapeadas em
`map_rect` / `map_point`. Cada função pública mapeia **uma vez** e delega para uma
versão `*_dev` que opera em espaço de dispositivo — sem essa separação,
`stroke_round_rect` chamaria `fill_rect` e as coordenadas seriam giradas duas
vezes. O toque faz o caminho inverso (`unmap_point`).

Resultado no aparelho, verificado por contagem de pixels no frame capturado:

| Métrica | Antes | Depois |
| --- | --- | --- |
| pixels pretos puros (letterbox) | ~40 % | **0 %** |
| faixa do rodapé na borda certa | — | 96,9 % |
