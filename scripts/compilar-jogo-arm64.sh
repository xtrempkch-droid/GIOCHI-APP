#!/usr/bin/env bash
# Compila o jogo (`app/jogos/volley`) para o aparelho (aarch64) e, opcionalmente,
# instala em `/opt/giochi/apps/volley/bin`.
#
# Uso:
#   bash scripts/compilar-jogo-arm64.sh                       # compila o volley
#   bash scripts/compilar-jogo-arm64.sh --jogo pacman         # outro jogo
#   bash scripts/compilar-jogo-arm64.sh --jogo cobrinha --instalar 10.55.0.2
#
# ## Por que um script separado, e não uma opção no do launcher
#
# O launcher e o jogo são apps diferentes, em projetos Cargo diferentes. O que
# os dois compartilham são as cinco armadilhas de link do aarch64 — sysroot,
# `--dynamic-linker`, glibc do aparelho, wrapper de `rust-lld` e libSDL2 —, e
# todas elas já estão resolvidas em `~/.cache/giochi-arm64` pelo
# `compilar-launcher-arm64.sh`.
#
# Então este script **não redescobre nada**: ele exige que aquele cache já
# exista e reusa o wrapper e as bibliotecas. Duplicar a lógica de download do
# sysroot significaria dois lugares para envelhecer, e a lição deste repositório
# é que o segundo lugar sempre envelhece diferente.
#
# Diferença importante: o jogo **não** usa `SDL2_ttf` nem `libasound`. Ele
# desenha o placar em sete segmentos de propósito, para depender de uma
# biblioteca só.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Padrão: o primeiro jogo, que é o que existe há mais tempo. `--jogo` escolhe
# outro. Um script por jogo seria três scripts para o mesmo link.
NOME_JOGO="volley"
ALVO="aarch64-unknown-linux-gnu"
CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SYSROOT="$CACHE/sysroot"
LIBDIR="$CACHE/devlibs"
INSTALAR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --jogo) NOME_JOGO="${2:-}"; shift 2 ;;
    --jogo=*) NOME_JOGO="${1#*=}"; shift ;;
    --instalar) INSTALAR="${2:-}"; shift 2 ;;
    --instalar=*) INSTALAR="${1#*=}"; shift ;;
    -h|--ajuda)
      sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "opção desconhecida: $1" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()  { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
erro(){ printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

JOGO="$RAIZ/app/jogos/$NOME_JOGO"
BINARIO="giochi-$NOME_JOGO"
if [ ! -f "$JOGO/Cargo.toml" ]; then
  disponiveis="$(find "$RAIZ/app/jogos" -mindepth 1 -maxdepth 1 -type d -printf '%f ' 2>/dev/null)"
  erro "jogo desconhecido: $NOME_JOGO (esperado em app/jogos/<nome>)
  em app/jogos/ existe: ${disponiveis:-nada}
  use: --jogo <nome>"
fi

# ── 1. o cache do cross-compile precisa existir ─────────────
# Sem ele não há como linkar: `rust-lld` sozinho produz um ELF sem `PT_INTERP`
# e o kernel mata o processo com segfault — `file` diz "valid ELF" e não ajuda.
if [ ! -x "$CACHE/cc-aarch64.sh" ] || [ ! -f "$LIBDIR/libSDL2.so" ]; then
  erro "cache aarch64 incompleto em $CACHE
  rode primeiro: bash scripts/compilar-launcher-arm64.sh
  (ele baixa o sysroot, copia a glibc/SDL2 do aparelho e escreve o wrapper)"
fi
ok "cache aarch64 em $CACHE"

if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO"; then
  erro "alvo $ALVO não instalado: rustup target add $ALVO"
fi
ok "alvo $ALVO presente"

# ── 2. compilar ─────────────────────────────────────────────
# Os mesmos `-L` do launcher. `sdl2-sys` acha os **cabeçalhos** pelo pkg-config
# do host (cabeçalho não tem arquitetura); quem precisa ser aarch64 é a
# biblioteca, e é isso que estes `-L` garantem — `$LIBDIR` vem primeiro.
export GIOCHI_ARM64_SYSROOT="$SYSROOT"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
export RUSTFLAGS="-C linker=$CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$SYSROOT \
-C link-arg=-L$LIBDIR \
-C link-arg=-L$SYSROOT/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$SYSROOT/lib \
-C link-arg=-L$SYSROOT/usr/lib"

# Sem isto o crate `pkg-config` recusa trabalhar cross e o erro fala de
# "cross-compilation", não de SDL — exatamente o erro que a `neon-drift`
# produziu na primeira tentativa. Os `include`s vêm do `sdl2.pc` do host
# (cabeçalho não tem arquitetura); quem precisa ser aarch64 é a biblioteca, e
# `-L$LIBDIR` vem primeiro na linha de link.
export PKG_CONFIG_ALLOW_CROSS=1

# O `sdl2-sys` precisa de um `sdl2.pc` só para os `include`s. Se o host não tem,
# o erro é `SDL2 headers not found`, que não diz que o problema é de host.
if [ -n "${GIOCHI_SDL2_PREFIX:-}" ] || [ -d "$HOME/.cache/giochi-sdl2/root" ]; then
  PREFIXO_SDL="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
  export PKG_CONFIG_PATH="$PREFIXO_SDL/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
fi

cd "$JOGO"
echo "  compilando $BINARIO para $ALVO..."
cargo build --release --target "$ALVO" --offline 2>&1 | grep -vE "^\s*(Compiling|Finished|Downloading)" || true

BIN="$JOGO/target/$ALVO/release/$BINARIO"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

# Sem `PT_INTERP` o kernel mata o processo na hora, e `file` ainda diz que o ELF
# é válido. Conferir aqui custa um `readelf` e economiza um cartão regravado.
if command -v readelf >/dev/null 2>&1; then
  if ! readelf -l "$BIN" 2>/dev/null | grep -q "INTERP"; then
    erro "$BIN não tem PT_INTERP — o kernel daria segfault. Wrapper de link errado?"
  fi
  ok "PT_INTERP presente"
fi

mkdir -p "$JOGO/bin"
cp "$BIN" "$JOGO/bin/$BINARIO"
chmod +x "$JOGO/bin/$BINARIO"
ok "pronto: $JOGO/bin/$BINARIO ($(du -h "$BIN" | cut -f1))"

# ── 3. instalar ─────────────────────────────────────────────
if [ -n "$INSTALAR" ]; then
  HOST="${INSTALAR#root@}"
  DESTINO="/opt/giochi/apps/$NOME_JOGO/bin"
  filtrar_ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

  echo "  instalando em root@$HOST:$DESTINO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" "mkdir -p $DESTINO" 2>&1 | filtrar_ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$JOGO/bin/$BINARIO" "root@$HOST:$DESTINO/$BINARIO" 2>&1 | filtrar_ruido
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" \
    "chmod 755 $DESTINO/$BINARIO; sha256sum $DESTINO/$BINARIO" 2>&1 | filtrar_ruido

  LOCAL=$(sha256sum "$JOGO/bin/$BINARIO" | cut -d' ' -f1)
  REMOTO=$(ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" \
    "sha256sum $DESTINO/$BINARIO 2>/dev/null | cut -d' ' -f1" 2>/dev/null | filtrar_ruido)
  if [ "$LOCAL" = "$REMOTO" ]; then
    ok "sha256 idêntico (${LOCAL:0:16}…)"
  else
    erro "sha256 diferente: local ${LOCAL:0:16}… dispositivo ${REMOTO:0:16}…"
  fi
fi
