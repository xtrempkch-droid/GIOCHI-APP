#!/usr/bin/env bash
# Compila o launcher (Rust + SDL2) resolvendo as bibliotecas de desenvolvimento.
#
# Uso:
#   bash scripts/compilar-launcher.sh            # cargo build --workspace
#   bash scripts/compilar-launcher.sh --release  # repassa o argumento ao cargo
#   bash scripts/compilar-launcher.sh --testes   # roda os testes do workspace
#
# Por que este script existe: o `sdl2-sys` precisa de `pkg-config` para achar
# `sdl2` e, com a feature `ttf`, de `SDL2_ttf`. Sem elas o `rustc` compila e o
# **link** falha (`unable to find library -lSDL2`). Em máquina sem root, dá para
# extrair os `.deb` num prefixo local e apontar `PKG_CONFIG_PATH`/`LIBRARY_PATH`
# para lá, sem instalar nada no sistema.
#
# `LIBRARY_PATH` não basta para os testes: o crate compila com `rust-lld`
# (`-fuse-ld=lld`), que **ignora** `LIBRARY_PATH` e só olha `-L`. Por isso o
# `RUSTFLAGS` abaixo. Sem ele, `cargo test --workspace` falha com
# `unable to find library -lSDL2` mesmo com o build passando.
#
# Variáveis:
#   GIOCHI_SDL2_PREFIX  onde extrair os pacotes (padrão: ~/.cache/giochi-sdl2/root)
#   GIOCHI_SDL2_DEB_URL base dos pacotes (padrão: archive.ubuntu.com)
set -euo pipefail

MODO="build"
ARGS_CARGO=()

for arg in "$@"; do
  case "$arg" in
    --testes) MODO="test" ;;
    -h|--ajuda) sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) ARGS_CARGO+=("$arg") ;;
  esac
done

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCHER="$RAIZ/app/launcher"
PREFIXO="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
BASE_URL="${GIOCHI_SDL2_DEB_URL:-https://archive.ubuntu.com/ubuntu/pool/universe}"

# Versões verificadas em 03/11/2026 (Ubuntu 25.10 "resolute"). Atualizar aqui.
SDL2_DEV_DEB="libsdl2/libsdl2-dev_2.32.10+dfsg-6_amd64.deb"
SDL2_TTF_DEV_DEB="libsdl2-ttf/libsdl2-ttf-dev_2.24.0+dfsg-3_amd64.deb"
SDL2_TTF_RUN_DEB="libsdl2-ttf/libsdl2-ttf-2.0-0_2.24.0+dfsg-3_amd64.deb"

log() { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

# `--exists` não serve aqui: o `SDL2_ttf.pc` declara `harfbuzz` e `freetype2`
# em `Requires.private`, e o pkg-config valida isso mesmo quando o link é
# dinâmico. O linker não precisa deles, então basta achar as versões.
tem_sdl2() {
  pkg-config --modversion sdl2 >/dev/null 2>&1 &&
    pkg-config --modversion SDL2_ttf >/dev/null 2>&1
}

if ! command -v cargo >/dev/null; then
  erro "cargo não encontrado. Instale o Rust: https://rustup.rs"
fi

if ! tem_sdl2; then
  if command -v apt-get >/dev/null; then
    log "SDL2 de desenvolvimento ausente — extraindo pacotes em $PREFIXO"
    mkdir -p "$PREFIXO/debs"
    for deb in "$SDL2_DEV_DEB" "$SDL2_TTF_DEV_DEB" "$SDL2_TTF_RUN_DEB"; do
      nome="$(basename "$deb")"
      if [ ! -f "$PREFIXO/debs/$nome" ]; then
        log "  baixando $nome"
        curl -fsSL --max-time 120 -o "$PREFIXO/debs/$nome" "$BASE_URL/$deb" \
          || erro "falha ao baixar $BASE_URL/$deb"
      fi
      dpkg-deb -x "$PREFIXO/debs/$nome" "$PREFIXO" \
        || erro "falha ao extrair $nome"
    done

    # `libSDL2.so` aponta para `libSDL2-2.0.so.0`, que vem do pacote de runtime
    # (já instalado na maioria das máquinas). Se não resolver, o linker cai no
    # `libSDL2.a` e falha com dezenas de símbolos do PulseAudio.
    LIBDIR="$PREFIXO/usr/lib/x86_64-linux-gnu"
    if [ ! -e "$LIBDIR/libSDL2-2.0.so.0" ]; then
      SISTEMA="$(ldconfig -p 2>/dev/null | awk '/libSDL2-2\.0\.so\.0 /{print $NF; exit}')"
      [ -n "$SISTEMA" ] || erro "libSDL2-2.0.so.0 não está instalada no sistema"
      ln -sf "$SISTEMA" "$LIBDIR/libSDL2-2.0.so.0"
    fi
    if [ ! -e "$LIBDIR/libSDL2.so" ]; then
      ln -sf libSDL2-2.0.so.0 "$LIBDIR/libSDL2.so"
    fi

    # `libasound` (volume) costuma existir só como `libasound.so.2`, que o
    # linker não usa: ele procura `libasound.so`, que vem do pacote -dev.
    if [ ! -e "$LIBDIR/libasound.so" ]; then
      ASOUND="$(ldconfig -p 2>/dev/null | awk '/libasound\.so\.2 /{print $NF; exit}')"
      if [ -n "$ASOUND" ]; then
        ln -sf "$ASOUND" "$LIBDIR/libasound.so"
      else
        echo "aviso: libasound.so.2 ausente — o volume não vai linkar" >&2
      fi
    fi
  else
    erro "pkg-config não achou sdl2/SDL2_ttf. Instale libsdl2-dev e libsdl2-ttf-dev."
  fi

  export PKG_CONFIG_PATH="$PREFIXO/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  export LIBRARY_PATH="$PREFIXO/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
  tem_sdl2 || erro "pkg-config continua sem achar sdl2/SDL2_ttf após extrair os pacotes"
  log "SDL2 resolvido em $PREFIXO"
else
  log "SDL2 de desenvolvimento encontrado no sistema"
fi

# ## Symlink quebrado no prefixo é pior que prefixo nenhum
#
# O `-L` abaixo aponta para o prefixo ANTES do sistema. Se `libSDL2.so` estiver
# quebrado lá, o linker não erra na hora: ele **cai no `libSDL2.a`** e falha
# depois com dezenas de símbolos do PulseAudio/libsamplerate (`src_new`,
# `src_process`, …) — mensagem que manda procurar defeito no código.
#
# Medido em 09/11/2026: `libSDL2-2.0.so.0.3200.10 -> libSDL2-2.0.so.0.3200.10`
# (laço em si mesmo, de uma extração antiga) deixava `libSDL2.so` sem alvo.
# A verificação abaixo segue o symlink de verdade e conserta só o que não
# resolve, dizendo o que fez. Roda mesmo quando o SDL2 vem do sistema — é
# justamente esse o caso em que o bloco de extração não roda e o laço sobrava.
LIBDIR_PREFIXO="$PREFIXO/usr/lib/x86_64-linux-gnu"
if [ -d "$LIBDIR_PREFIXO" ]; then
  for alvo in libSDL2 libSDL2_ttf; do
    [ -e "$LIBDIR_PREFIXO/$alvo.so" ] && continue
    # `$1` é o nome na saída do `ldconfig`; comparar com espaço à direita no
    # padrão nunca casava, e a reparação virava silenciosa — o mesmo defeito que
    # ela existe para evitar.
    real="$(ldconfig -p 2>/dev/null | awk -v n="$alvo-2.0.so.0" '$1 == n {print $NF; exit}')"
    if [ -n "$real" ]; then
      ln -sfn "$real" "$LIBDIR_PREFIXO/$alvo.so"
      log "  symlink consertado: $alvo.so → $real"
    else
      echo "aviso: $LIBDIR_PREFIXO/$alvo.so não resolve e não achei a .so.0 no sistema" >&2
    fi
  done
fi

cd "$LAUNCHER"

# `rust-lld` ignora `LIBRARY_PATH`; só `-L` conta. Sem isto o build passa e o
# `cargo test` falha no link, que é o pior tipo de erro: parece defeito do teste.
export RUSTFLAGS="-L $PREFIXO/usr/lib/x86_64-linux-gnu${RUSTFLAGS:+ $RUSTFLAGS}"

if [ "$MODO" = "test" ]; then
  log "cargo test --workspace"
  cargo test --workspace "${ARGS_CARGO[@]}" 2>&1 | grep -E "^test result|^error" || true
  exit 0
fi

log "cargo build --workspace ${ARGS_CARGO[*]-}"
cargo build --workspace "${ARGS_CARGO[@]}"

cat <<FIM

Pronto. Para rodar, o carregador precisa achar o \`libSDL2_ttf.so.0\`:

  export LD_LIBRARY_PATH=$PREFIXO/usr/lib/x86_64-linux-gnu\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}
  ./target/debug/giochi-launcher

Sem tela (CI, servidor), dá para conferir o desenho num BMP:

  SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \\
    ./target/debug/giochi-launcher --dump-frame frame.bmp
FIM
