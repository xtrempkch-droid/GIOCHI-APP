#!/usr/bin/env bash
# Cross-compila o launcher-teste (launcher sobre a giochi-engine do
# GIOCHI-ENGINE-3D) para aarch64.
#
#   bash scripts/compilar-launcher-teste-arm64.sh
#   bash scripts/compilar-launcher-teste-arm64.sh --instalar root@10.55.0.2
#
# ## Por que um script separado do `compilar-launcher-arm64.sh`
#
# O launcher de produção linka contra **SDL2** e, opcionalmente, **SDL2_ttf**.
# Este linka contra **SDL2**, **SDL2_image** e **SDL2_ttf**, porque a engine
# habilita `image`, `mixer` e `ttf` no `sdl2`.
#
# `sdl2-system` é uma feature do crate da engine (acrescentada em 07/11/2026):
# sem ela o `bundled` do workspace seria obrigatório e cross-compilar exigiria
# cmake e uma segunda cópia do SDL2 no rootfs. Só isso já justificaria, mas há
# uma diferença maior: a engine usa `sdl2-system` (pkg-config), e o pkg-config
# precisa de `.pc` para **cada** uma das bibliotecas, apontando para o cache de
# cross-compile. O aparelho não tem os `.pc` (é Buildroot, sem `-dev`), então
# eles são gerados aqui.
#
# ## O que é reusado
#
# Sysroot glibc, `libSDL2.so`, `libSDL2_ttf.so`, `libgcc_s`, o wrapper
# `cc-aarch64.sh` e o LOADER: tudo o que `compilar-launcher-arm64.sh` já monta em
# `~/.cache/giochi-arm64`. Redescobrir isso significaria dois lugares para
# envelhecer — a lição deste repositório.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/launcher-teste"
ENGINE="${GIOCHI_ENGINE_DIR:-$RAIZ/../GIOCHI-ENGINE-3D}"
CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SYSROOT="$CACHE/sysroot"
LIBDIR="$CACHE/devlibs"
PKGDIR="$CACHE/pkgconfig"
ALVO="aarch64-unknown-linux-gnu"
HOST_SSH="${GIOCHI_DEVICE:-}"
INSTALAR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --instalar) INSTALAR="${2#root@}"; shift 2 ;;
    --host) HOST_SSH="${2#root@}"; shift 2 ;;
    -h|--ajuda) sed -n '2,6p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

log() { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }
ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

[ -d "$ENGINE" ] || erro "engine não encontrada em $ENGINE
  clone ao lado: git clone https://github.com/xtrempkch-droid/GIOCHI-ENGINE-3D \"$ENGINE\"
  ou aponte: GIOCHI_ENGINE_DIR=/caminho/da/engine"
command -v cargo >/dev/null || erro "cargo não encontrado"
rustup target list --installed 2>/dev/null | grep -qx "$ALVO" || rustup target add "$ALVO"

# ── 1. o cache do launcher de produção tem de existir ───────
#
# Ele traz sysroot + SDL2 + SDL2_ttf + wrapper. Reaproveitar é o que garante que
# os dois binários usem o **mesmo** glibc do aparelho.
if [ ! -f "$LIBDIR/libSDL2.so" ] || [ ! -f "$SYSROOT/usr/aarch64-linux-gnu/lib/crt1.o" ]; then
  log "cache de cross-compile incompleto — montando pelo launcher de produção"
  GIOCHI_DEVICE="$HOST_SSH" bash "$RAIZ/scripts/compilar-launcher-arm64.sh" --host "${HOST_SSH:+root@$HOST_SSH}" \
    || erro "falha ao montar o cache (rode scripts/compilar-launcher-arm64.sh --host root@<ip>)"
fi

# ── 2. as bibliotecas que só a engine exige ─────────────────
resolver_host() {
  [ -n "$HOST_SSH" ] && return 0
  local ip
  ip=$(bash "$RAIZ/scripts/aparelho.sh" procurar 2>/dev/null |
    grep -oE '^[[:space:]]*([0-9]{1,3}\.){3}[0-9]{1,3}[[:space:]]+[^[:space:]]+' |
    head -1 | awk '{print $1}')
  [ -n "$ip" ] || erro "aparelho não encontrado. Use --host root@<ip> ou GIOCHI_DEVICE"
  HOST_SSH="$ip"
}

precisa_de_imagem() {
  [ -f "$LIBDIR/libSDL2_image.so" ] && [ -f "$LIBDIR/libSDL2_mixer.so" ] && [ -f "$PKGDIR/sdl2.pc" ]
}

if ! precisa_de_imagem; then
  resolver_host
  log "copiando SDL2_image e SDL2_mixer do aparelho ($HOST_SSH)"
  mkdir -p "$LIBDIR" "$PKGDIR"
  for lib in libSDL2_image.so libSDL2_mixer.so; do
    # Os symlinks NÃO versionados: fixar `...so.0.800.2` quebraria na primeira
    # atualização da imagem, com erro que não fala de versão.
    scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
      "root@$HOST_SSH:/usr/lib/$lib" "$LIBDIR/$lib" 2>/dev/null \
      || erro "não copiei /usr/lib/$lib do aparelho (o symlink existe lá?)"
  done

  # Os `.pc` apontam para o cache: `pkg-config` do host os lê e emite `-L`/`-l`
  # para o alvo. Sem `Requires`, de propósito — o aparelho não tem `.pc` de
  # freetype/harfbuzz/alsa, e exigi-los faria o pkg-config falhar por
  # dependência que o link dinâmico nem precisa declarar.
  for par in "sdl2:SDL2:2.28.5:-lSDL2" \
             "SDL2_image:SDL2_image:2.8.2:-lSDL2_image" \
             "SDL2_mixer:SDL2_mixer:2.6.3:-lSDL2_mixer" \
             "SDL2_ttf:SDL2_ttf:2.22.0:-lSDL2_ttf"; do
    IFS=: read -r arq nome ver lib <<< "$par"
    cat > "$PKGDIR/$arq.pc" <<PC
prefix=$LIBDIR
libdir=\${prefix}
includedir=$SYSROOT/usr/aarch64-linux-gnu/include

Name: $nome
Description: $nome (do aparelho, para cross-compile)
Version: $ver
Libs: -L\${libdir} $lib
Cflags: -I\${includedir}
PC
  done
fi

# ── 3. compilar ─────────────────────────────────────────────
export PKG_CONFIG_PATH="$PKGDIR"
# Sem isto o crate `pkg-config` recusa trabalhar cross e o erro fala de
# "cross-compilation", não de SDL.
export PKG_CONFIG_ALLOW_CROSS=1
export GIOCHI_ARM64_SYSROOT="$SYSROOT"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
export RUSTFLAGS="-C linker=$CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$SYSROOT \
-C link-arg=-L$LIBDIR \
-C link-arg=-L$SYSROOT/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$SYSROOT/lib \
-C link-arg=-L$SYSROOT/usr/lib"

log "compilando launcher-teste para $ALVO"
cd "$APP" || erro "app não encontrado: $APP"
cargo build --release --target "$ALVO" || erro "falha na compilação"

BIN="$APP/target/$ALVO/release/giochi-launcher-teste"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

# O interpretador é o que faz o ELF rodar no aparelho; sem `PT_INTERP` o kernel
# devolve um segfault imediato, sem mensagem.
if command -v readelf >/dev/null; then
  # `LC_ALL=C` porque o `readelf` é **localizado**: em pt_BR ele imprime
  # "interpretador do programa", e um grep por "interpreter" reprova um binário
  # correto. Medido aqui em 07/11/2026 — a mensagem mandava culpar o wrapper de
  # link, que estava certo.
  LC_ALL=C readelf -l "$BIN" 2>/dev/null | grep -q "/lib/ld-linux-aarch64.so.1" \
    || erro "o binário saiu sem PT_INTERP correto — o wrapper de link não foi usado"
fi

mkdir -p "$APP/bin"
cp "$BIN" "$APP/bin/giochi-launcher-teste"
chmod +x "$APP/bin/giochi-launcher-teste"
log "pronto: app/launcher-teste/bin/giochi-launcher-teste ($(du -h "$BIN" | cut -f1))"
if command -v readelf >/dev/null; then
  # `LC_ALL=C` de novo: em pt_BR a linha é "Biblioteca Compartilhada", e o
  # script imprimia `NEEDED:` vazio — o silêncio que este repositório combate.
  printf '  NEEDED: %s\n' "$(LC_ALL=C readelf -d "$BIN" | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p' | tr '\n' ' ')"
fi

# ── 4. instalar ─────────────────────────────────────────────
if [ -n "$INSTALAR" ]; then
  DESTINO="/opt/giochi/apps/launcher-teste/bin"
  log "instalando em root@$INSTALAR:$DESTINO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$INSTALAR" "mkdir -p $DESTINO" 2>&1 | ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$BIN" "root@$INSTALAR:$DESTINO/giochi-launcher-teste" 2>&1 | ruido
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$INSTALAR" \
    "chmod +x $DESTINO/giochi-launcher-teste && sha256sum $DESTINO/giochi-launcher-teste" 2>&1 | ruido
  printf '  local: %s\n' "$(sha256sum "$BIN" | cut -d' ' -f1)"

  # O `giochi-teste` é o que alterna a tela do aparelho entre os dois launchers.
  # Vai com o binário porque sem ele a única forma de ver o launcher-teste é a
  # sequência manual de `touch`/`kill`/`setsid`, que já custou a cota do
  # `giochi-init` uma vez. Fica **fora** de `bin/` (que é do entrypoint) para o
  # pacote não confundir os dois.
  auxiliar="$APP/bin/giochi-teste"
  if [ -f "$auxiliar" ]; then
    scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
      "$auxiliar" "root@$INSTALAR:/opt/giochi/apps/launcher-teste/giochi-teste" 2>&1 | ruido
    ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$INSTALAR" \
      "chmod +x /opt/giochi/apps/launcher-teste/giochi-teste" 2>&1 | ruido
    printf '  utilitário: /opt/giochi/apps/launcher-teste/giochi-teste\n'
  fi
  printf '  verificar no aparelho: bash scripts/aparelho.sh verificar-teste\n'
fi
