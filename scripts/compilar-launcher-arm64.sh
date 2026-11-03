#!/usr/bin/env bash
# Compila o launcher para o aparelho (aarch64) e, opcionalmente, instala.
#
# Uso:
#   bash scripts/compilar-launcher-arm64.sh                 # só compila
#   bash scripts/compilar-launcher-arm64.sh --instalar root@192.168.1.6
#
# ## Por que este script existe
#
# Não há toolchain aarch64 pronto para este código. O `cargo` compila; quem
# falha é o **link**, por quatro motivos que este script resolve:
#
#  1. Falta sysroot aarch64 com glibc e crt1.o → baixa `libc6-*-arm64-cross`.
#  2. `rust-lld` não é driver de compilador: não adiciona crt1/crti/crtn nem
#     `--dynamic-linker`. Sem o segundo, o ELF sai sem PT_INTERP e o kernel dá
#     segfault imediato. → wrapper `cc-aarch64.sh`.
#  3. `libSDL2` e `libc` do aparelho são glibc **2.38**; o sysroot do Ubuntu é
#     2.35, e o link recusa símbolos (`strlcat@GLIBC_2.38`). → copia libc/libm/
#     libgcc_s/libSDL2 **do próprio aparelho**, que é o alvo real.
#  4. O aparelho não tem `libSDL2_ttf` → compila com `--no-default-features`
#     (sem a feature `texto`). Ver docs/dependencias-aparelho.md.
#  5. `libasound` (controle de volume) também é copiada do aparelho: a versão do
#     sysroot do Ubuntu não é a que o codec dele espera.
#
# Requisitos: `ssh`/`scp` para o aparelho, `curl`, `dpkg-deb`, `rustup`.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LAUNCHER="$RAIZ/app/launcher"
ALVO="aarch64-unknown-linux-gnu"
RAIZ_CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SYSROOT="$RAIZ_CACHE/sysroot"
DEVS="$RAIZ_CACHE/debs"
LIBDIR="$RAIZ_CACHE/devlibs"
HOST_SSH="${GIOCHI_DEVICE:-root@192.168.1.6}"
BASE_URL="https://archive.ubuntu.com/ubuntu/pool/main/c/cross-toolchain-base"
LIB_DEB="libc6-dev-arm64-cross_2.35-0ubuntu1cross3_all.deb"
RUN_DEB="libc6-arm64-cross_2.35-0ubuntu1cross3_all.deb"

log()  { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

INSTALAR=""
while [ $# -gt 0 ]; do
  case "$1" in
    --instalar) INSTALAR="${2:-}"; shift 2 ;;
    --host)     HOST_SSH="${2:-}"; shift 2 ;;
    *) erro "argumento desconhecido: $1" ;;
  esac
done

command -v cargo >/dev/null || erro "cargo não encontrado (https://rustup.rs)"
command -v dpkg-deb >/dev/null || erro "dpkg-deb não encontrado"

# ── 1. target Rust ──────────────────────────────────────────
if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO"; then
  log "instalando target $ALVO"
  rustup target add "$ALVO"
fi

# ── 2. sysroot com glibc ────────────────────────────────────
if [ ! -f "$SYSROOT/usr/aarch64-linux-gnu/lib/crt1.o" ]; then
  log "montando sysroot em $SYSROOT"
  mkdir -p "$DEVS" "$SYSROOT"
  for deb in "$LIB_DEB" "$RUN_DEB"; do
    if [ ! -f "$DEVS/$deb" ]; then
      log "  baixando $deb"
      curl -fsSL --max-time 180 -o "$DEVS/$deb" "$BASE_URL/$deb" \
        || erro "falha ao baixar $BASE_URL/$deb"
    fi
    dpkg-deb -x "$DEVS/$deb" "$SYSROOT" || erro "falha ao extrair $deb"
  done
fi

# ── 3. bibliotecas do aparelho (glibc 2.38 + SDL2) ──────────
# Não dá para usar as do Ubuntu: o libSDL2 do aparelho pede símbolos
# `@GLIBC_2.38` que o sysroot 2.35 não tem, e o link falha.
precisa_do_aparelho() {
  [ -f "$LIBDIR/libSDL2.so" ] && [ -f "$LIBDIR/libasound.so" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/lib/libc.so.6" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so.6" ]
}

if ! precisa_do_aparelho; then
  command -v scp >/dev/null || erro "scp necessário para copiar as libs do aparelho"
  log "copiando libs do aparelho ($HOST_SSH) — o alvo real define o glibc"
  mkdir -p "$LIBDIR"
  for par in \
    "/usr/lib/libSDL2-2.0.so.0.2800.5:$LIBDIR/libSDL2.so" \
    "/usr/lib/libasound.so.2.0.0:$LIBDIR/libasound.so" \
    "/lib/libc.so.6:$SYSROOT/usr/aarch64-linux-gnu/lib/libc.so.6" \
    "/lib/libm.so.6:$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so.6" \
    "/lib/libgcc_s.so.1:$LIBDIR/libgcc_s.so.1" \
    "/lib/ld-linux-aarch64.so.1:$SYSROOT/usr/aarch64-linux-gnu/lib/ld-linux-aarch64.so.1"
  do
    origem="${par%%:*}"; destino="${par##*:}"
    scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
      "$HOST_SSH:$origem" "$destino" 2>/dev/null \
      || erro "falha ao copiar $origem do aparelho"
  done
  ln -sf libgcc_s.so.1 "$LIBDIR/libgcc_s.so"
  # O linker script do glibc aponta para caminhos absolutos do sysroot; o
  # symlink abaixo deixa `libm.so` resolver sem duplicar arquivo.
  ln -sf libm.so.6 "$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so"
fi

# ── 4. wrapper de link ──────────────────────────────────────
cat > "$RAIZ_CACHE/cc-aarch64.sh" <<'WRAP'
#!/usr/bin/env bash
# Ver docs/compilar-launcher.md § aarch64 para o porquê de cada linha.
set -euo pipefail
SYSROOT="${GIOCHI_ARM64_SYSROOT:-$HOME/.cache/giochi-arm64/sysroot}"
CRT="$SYSROOT/usr/aarch64-linux-gnu/lib"
LOADER="${GIOCHI_ARM64_LOADER:-/lib/ld-linux-aarch64.so.1}"
args=()
for a in "$@"; do
  case "$a" in
    -Wl,*) IFS=',' read -r -a parts <<< "${a#-Wl,}"; args+=("${parts[@]}") ;;
    -nodefaultlibs|-nostdlib|-B*) : ;;
    *) args+=("$a") ;;
  esac
done
exec rust-lld -flavor gnu --dynamic-linker "$LOADER" \
  "$CRT/Scrt1.o" "$CRT/crti.o" "${args[@]}" "$CRT/crtn.o"
WRAP
chmod +x "$RAIZ_CACHE/cc-aarch64.sh"

# ── 5. compilar ─────────────────────────────────────────────
export GIOCHI_ARM64_SYSROOT="$SYSROOT"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
export RUSTFLAGS="-C linker=$RAIZ_CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$SYSROOT \
-C link-arg=-L$LIBDIR \
-C link-arg=-L$SYSROOT/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$SYSROOT/lib \
-C link-arg=-L$SYSROOT/usr/lib"

log "cargo build --release --target $ALVO --no-default-features"
cd "$LAUNCHER"
cargo build -p giochi-launcher --release --target "$ALVO" \
  --no-default-features --offline

BIN="$LAUNCHER/target/$ALVO/release/giochi-launcher"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

mkdir -p "$RAIZ/app/launcher/bin"
cp "$BIN" "$RAIZ/app/launcher/bin/giochi-launcher"
chmod +x "$RAIZ/app/launcher/bin/giochi-launcher"
log "pronto: $RAIZ/app/launcher/bin/giochi-launcher ($(du -h "$BIN" | cut -f1))"

# ── 6. instalar no aparelho ─────────────────────────────────
if [ -n "$INSTALAR" ]; then
  DESTINO="/opt/giochi/apps/launcher/bin"
  # O dropbear/openssh do aparelho avisa sobre criptografia pós-quântica a cada
  # conexão. O aviso vai para stderr e polui a saída do script inteiro.
  filtrar_ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

  log "instalando em $INSTALAR:$DESTINO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$INSTALAR" "mkdir -p $DESTINO" 2>&1 | filtrar_ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$BIN" "$INSTALAR:$DESTINO/giochi-launcher" 2>&1 | filtrar_ruido
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$INSTALAR" \
    "chmod +x $DESTINO/giochi-launcher && ls -la $DESTINO/giochi-launcher" 2>&1 | filtrar_ruido
  # O `inittab` desta imagem não tem entrada para o `giochi-init`, então o app
  # NÃO sobe no boot. Dizer o contrário aqui já custou um diagnóstico errado.
  log "instalado. A interface NÃO sobe sozinha: o inittab não inicia o giochi-init."
  log "suba com: bash scripts/aparelho.sh subir"
fi
