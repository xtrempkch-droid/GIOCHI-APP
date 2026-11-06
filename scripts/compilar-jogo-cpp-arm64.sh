#!/usr/bin/env bash
# Compila um jogo em C++/SDL2 (`app/jogos/<nome>/src/nativo`) para o aparelho
# (aarch64, glibc) e, opcionalmente, instala em `/user/apps/<nome>/bin`.
#
# Uso:
#   bash scripts/compilar-jogo-cpp-arm64.sh                          # arcade-volleyball
#   bash scripts/compilar-jogo-cpp-arm64.sh --jogo arcade-volleyball --instalar
#
# ## Por que existe, se já há `compilar-jogo-arm64.sh`
#
# Aquele compila jogos em **Rust** e usa `rust-lld` como linker. Um jogo em C++
# precisa de um compilador cruzado de verdade (`g++` para aarch64) e do
# `libstdc++` do alvo, que o `rustup` não traz. São dois caches com vidas
# diferentes, e misturá-los num script só faria o segundo caso quebrar o
# primeiro.
#
# ## O que este script monta (e por que cada peça é necessária)
#
#   1. `libc6-dev-arm64-cross` e companhia dão headers e crt do alvo, mas os
#      `.deb` do Ubuntu **não podem ser instalados sem root**. Então eles são
#      extraídos num prefixo local — a mesma saída que o `compilar-launcher.sh`
#      já usa para o SDL2.
#   2. O binutils cruzado é um programa x86-64 e carrega `libopcodes`/`libbfd`
#      x86-64 do próprio prefixo: daí o `LD_LIBRARY_PATH`.
#   3. Os scripts de linker do pacote (`libc.so`, `libm.so`) apontam para
#      `/usr/aarch64-linux-gnu/lib/...` — caminho de instalação real, que não
#      existe aqui. São reescritos para resolver sob `--sysroot`.
#   4. A `libSDL2.so` vem do **aparelho** (`~/.cache/giochi-arm64/devlibs`),
#      não do Ubuntu: é o alvo real, e o SDL2 do aparelho pede símbolos que o
#      sysroot do Ubuntu não tem.
#
# Requisitos: `apt-get download` (não precisa de root), `dpkg-deb`, `curl`.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NOME_JOGO="arcade-volleyball"
INSTALAR=""
APARELHO=""

while [ $# -gt 0 ]; do
  case "$1" in
    --jogo) NOME_JOGO="${2:-}"; shift 2 ;;
    --jogo=*) NOME_JOGO="${1#*=}"; shift ;;
    --instalar) INSTALAR=1; shift ;;
    --aparelho) APARELHO="${2:-}"; shift 2 ;;
    --aparelho=*) APARELHO="${1#*=}"; shift ;;
    -h|--ajuda) sed -n '2,34p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "opção desconhecida: $1" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()  { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
log() { printf '  ..   %s\n' "$*"; }
erro(){ printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

JOGO="$RAIZ/app/jogos/$NOME_JOGO"
NATIVO="$JOGO/src/nativo"
BIN="$JOGO/bin/$NOME_JOGO"

[ -d "$NATIVO" ] || erro "sem port nativo em $NATIVO"
[ -f "$NATIVO/Makefile" ] || erro "sem Makefile em $NATIVO"

CACHE="$HOME/.cache/giochi-crosscpp"
DEBS="$CACHE/debs"
SYSROOT="$CACHE/root"
DEVLIBS="$HOME/.cache/giochi-arm64/devlibs"

# ── 1. pacotes do compilador cruzado ────────────────────────
PACOTES=(
  g++-aarch64-linux-gnu gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu
  libstdc++-15-dev-arm64-cross libstdc++6-arm64-cross
  libc6-dev-arm64-cross libc6-arm64-cross
  libgcc-15-dev-arm64-cross libgcc-s1-arm64-cross
  linux-libc-dev-arm64-cross
)

precisa_deb() {
  # `stdio.h` fica em `usr/aarch64-linux-gnu/include` (multiarch do Debian), não
  # em `usr/include` — conferir o caminho errado faz o script baixar tudo de
  # novo a cada execução, que é caro e esconde uma falha de download.
  [ -x "$SYSROOT/usr/bin/aarch64-linux-gnu-g++" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/include/stdio.h" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/lib/libc_nonshared.a" ]
}

if ! precisa_deb; then
  log "baixando o compilador cruzado (sem root, via apt-get download)"
  mkdir -p "$DEBS"
  ( cd "$DEBS" && apt-get download "${PACOTES[@]}" >/dev/null 2>&1 ) ||
    erro "apt-get download falhou (confira os nomes dos pacotes para esta distro)"
  mkdir -p "$SYSROOT"
  for d in "$DEBS"/*.deb; do dpkg-deb -x "$d" "$SYSROOT"; done
fi

# Os scripts de linker do pacote usam caminhos absolutos de instalação real.
# Reescrevê-los é o que faz `--sysroot` funcionar; sem isso o link falha
# dizendo que não achou `/usr/aarch64-linux-gnu/lib/libc.so.6` — que existe,
# só que dentro do prefixo.
for par in \
  "libc.so:GROUP ( libc.so.6 libc_nonshared.a AS_NEEDED ( ld-linux-aarch64.so.1 ) )" \
  "libm.so:GROUP ( libm.so.6 )"; do
  nome="${par%%:*}"; corpo="${par#*:}"
  destino="$SYSROOT/usr/aarch64-linux-gnu/lib/$nome"
  if [ -f "$destino" ] && grep -q "^GROUP ( /usr/" "$destino"; then
    printf 'OUTPUT_FORMAT(elf64-littleaarch64)\n%s\n' "$corpo" > "$destino"
  fi
done
ok "compilador cruzado em $SYSROOT"

# ── 2. headers do SDL2 para aarch64 ─────────────────────────
if [ ! -f "$SYSROOT/usr/include/SDL2/SDL.h" ]; then
  log "baixando headers do SDL2 para arm64"
  V=$(. /etc/os-release 2>/dev/null; echo "${UBUNTU_CODENAME:-resolute}")
  BAIXOU=""
  # A versão sobe com o tempo; tentar algumas é mais honesto que fixar uma e
  # quebrar quando o pool gira.
  for v in 2.32.10+dfsg-6 2.30.9+dfsg-1 2.28.4+dfsg-1; do
    u="http://archive.ubuntu.com/ubuntu/pool/universe/libs/libsdl2/libsdl2-dev_${v}_arm64.deb"
    if curl -fsSL -o "$DEBS/sdl2-dev-arm64.deb" "$u" 2>/dev/null; then
      dpkg-deb -x "$DEBS/sdl2-dev-arm64.deb" "$SYSROOT" && BAIXOU=1 && break
    fi
  done
  [ -n "$BAIXOU" ] || erro "não baixei os headers do SDL2 arm64 (pool do Ubuntu mudou?)"
fi
ok "headers do SDL2 arm64 presentes"

# ── 3. compilar ─────────────────────────────────────────────
log "compilando $NOME_JOGO para aarch64"
# A libSDL2 tem de ser a do aparelho. Se o cache do launcher não existir, ele é
# montado pelo script do launcher — que é quem sabe copiá-lo por SSH.
if [ ! -f "$DEVLIBS/libSDL2.so" ]; then
  erro "libSDL2 do aparelho ausente em $DEVLIBS
  rode primeiro: bash scripts/compilar-launcher-arm64.sh"
fi

make -C "$NATIVO" arm64 \
  CACHE_CUZ="$CACHE" SYSROOT_CUZ="$SYSROOT" LIBS_ALVO="$DEVLIBS" >/dev/null

mkdir -p "$JOGO/bin"
cp -f "$NATIVO/$NOME_JOGO" "$BIN"
chmod +x "$BIN"
ok "$BIN ($(stat -c %s "$BIN") bytes)"

# ── 4. instalar no aparelho ─────────────────────────────────
if [ -n "$INSTALAR" ]; then
  if [ -z "$APARELHO" ]; then
    IP=$(bash "$RAIZ/scripts/aparelho.sh" procurar 2>/dev/null |
         awk '/giochi-system-|giochi-|aparelho/{print $1; exit}')
    [ -n "$IP" ] || erro "aparelho não encontrado; use --aparelho root@<ip>"
    APARELHO="root@$IP"
  fi
  case "$APARELHO" in *@*) ;; *) APARELHO="root@$APARELHO" ;; esac

  log "instalando em $APARELHO:/user/apps/$NOME_JOGO/bin/"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$APARELHO" \
    "mkdir -p /user/apps/$NOME_JOGO/bin"
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$BIN" "$APARELHO:/user/apps/$NOME_JOGO/bin/$NOME_JOGO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$APARELHO" \
    "chmod +x /user/apps/$NOME_JOGO/bin/$NOME_JOGO && sha256sum /user/apps/$NOME_JOGO/bin/$NOME_JOGO" |
    awk -v local="$(sha256sum "$BIN" | cut -d' ' -f1)" '
      { if ($1 == local) printf "  ok   sha256 do aparelho confere (%s)\n", substr($1,1,16);
        else { printf "  ERRO sha256 diverge: aparelho %s, local %s\n", substr($1,1,16), substr(local,1,16); exit 1 } }'
fi

echo
echo "Próximo passo: bash scripts/aparelho.sh verificar-jogo --jogo $NOME_JOGO"
