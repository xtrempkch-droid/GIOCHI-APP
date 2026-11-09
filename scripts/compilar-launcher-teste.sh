#!/usr/bin/env bash
# Compila o launcher-teste (o launcher desenhado sobre a giochi-engine do
# GIOCHI-ENGINE-3D).
#
#   bash scripts/compilar-launcher-teste.sh
#   bash scripts/compilar-launcher-teste.sh --release
#
# ## Por que a engine é um repositório irmão
#
# A engine é o crate 2D do workspace GIOCHI-ENGINE-3D — o que tem 2D, 3D, ABI em
# C e Lua. `app/launcher-teste/Cargo.toml` aponta para
# `../../../GIOCHI-ENGINE-3D/giochi-engine`. O repositório `GIOCHI-ENGINE`
# separado é a versão 0.1.0 da mesma engine 2D e não é usado por este app.
# Fixar isso é proposital — a engine é "de uso nosso" e evolui junto; uma
# dependência de git traria a versão publicada, que não tem as primitivas que o
# porte exigiu (`draw_line`, `stroke_rect`, `fill_rects`, `set_blend_mode`).
#
# ## Por que `sdl2-system` e não `sdl2-bundled`
#
# No aparelho o SDL2 vem da imagem (Buildroot) com KMSDRM; compilar do
# código-fonte exigiria cmake e uma segunda cópia do SDL2 no rootfs. No PC o
# mesmo caminho funciona com o prefixo que `compilar-launcher.sh` já extrai,
# acrescido de `SDL2_image` e `SDL2_mixer` — que a engine exige e o launcher de
# produção não.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/launcher-teste"
ENGINE="${GIOCHI_ENGINE_DIR:-$RAIZ/../GIOCHI-ENGINE-3D}"
PREFIXO="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
BASE_URL="${GIOCHI_SDL2_DEB_URL:-https://archive.ubuntu.com/ubuntu/pool/universe}"

# Os `-dev` trazem os .pc e os symlinks; os de runtime trazem o `.so.N` de
# verdade. Sem os dois, o link acha o nome e não acha o arquivo.
DEBS=(
  "lib/libsdl2-image/libsdl2-image-dev_2.8.8+dfsg-2_amd64.deb"
  "libsdl2-image/libsdl2-image-2.0-0_2.8.8+dfsg-2_amd64.deb"
  "lib/libsdl2-mixer/libsdl2-mixer-dev_2.8.2+dfsg-1_amd64.deb"
  "libsdl2-mixer/libsdl2-mixer-2.0-0_2.8.2+dfsg-1_amd64.deb"
)

PERFIL="debug"
[ "${1:-}" = "--release" ] && PERFIL="release"

log() { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

[ -d "$ENGINE" ] || erro "engine não encontrada em $ENGINE
  clone ao lado: git clone https://github.com/xtrempkch-droid/GIOCHI-ENGINE-3D \"$ENGINE\"
  ou aponte: GIOCHI_ENGINE_DIR=/caminho/da/engine"

# O prefixo do launcher já tem SDL2 e SDL2_ttf; a engine exige também image e
# mixer. Baixar sob demanda mantém o mesmo prefixo para os dois apps.
for deb in "${DEBS[@]}"; do
  nome="$(basename "$deb")"
  if [ ! -f "$PREFIXO/debs/$nome" ]; then
    command -v apt-get >/dev/null || erro "apt-get ausente e $nome não está em $PREFIXO/debs"
    mkdir -p "$PREFIXO/debs"
    log "baixando $nome"
    curl -fsSL --max-time 120 -o "$PREFIXO/debs/$nome" "$BASE_URL/$deb" \
      || erro "falha ao baixar $BASE_URL/$deb"
  fi
  # `dpkg-deb -x` não precisa de root e sobrescreve só os arquivos do pacote.
  dpkg-deb -x "$PREFIXO/debs/$nome" "$PREFIXO" 2>/dev/null || true
done

export PKG_CONFIG_PATH="$PREFIXO/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LIBRARY_PATH="$PREFIXO/usr/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$PREFIXO/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

log "compilando launcher-teste ($PERFIL)"
cd "$APP" || erro "app não encontrado: $APP"
if [ "$PERFIL" = "release" ]; then
  cargo build --release || erro "falha na compilação"
else
  cargo build || erro "falha na compilação"
fi

BIN="$APP/target/$PERFIL/giochi-launcher-teste"
[ -x "$BIN" ] || erro "binário não gerado em $BIN"
log "pronto: $BIN"
printf '  comparar com o launcher de produção: bash scripts/comparar-launcher-teste.sh\n'
