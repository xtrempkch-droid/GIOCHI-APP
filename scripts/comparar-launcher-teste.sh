#!/usr/bin/env bash
# Compara o launcher-teste (GIOCHI-ENGINE) com o launcher de produção.
#
#   bash scripts/comparar-launcher-teste.sh
#   bash scripts/comparar-launcher-teste.sh --tamanhos 640x480 1280x720
#
# ## O que este script prova, e o que não prova
#
# Prova **fidelidade de desenho**: os dois binários desenham o mesmo quadro com
# a mesma entrada (catálogo de demonstração, mesmas teclas), e a comparação é
# pixel a pixel. É a única medida que separa "portou" de "parece igual".
#
# Não prova que a engine é mais rápida, nem que ela se comporta bem no aparelho:
# isso exige o KMSDRM e o painel, que só o hardware tem.
#
# ## Por que `--demo` nos dois
#
# O catálogo do aparelho tem caminhos que não existem no PC, e o launcher poda
# app sem binário (`podar_sem_binario`). Comparar com catálogos diferentes já
# produziu 36,5 % de acerto **sem nenhuma regressão de desenho** — a lição está
# em `docs/ferramentas.md`. Aqui a entrada é idêntica de propósito.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIXO="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
LIBS="$PREFIXO/usr/lib/x86_64-linux-gnu"
REAL="$RAIZ/app/launcher/target/debug/giochi-launcher"
TESTE="$RAIZ/app/launcher-teste/target/debug/giochi-launcher-teste"
SAIDA="$RAIZ/dist/launcher-teste"
TAMANHOS=("640x480" "800x600" "1280x720")
TECLAS="${TECLAS:-}"

while [ $# -gt 0 ]; do
  case "$1" in
    --tamanhos) shift; TAMANHOS=(); while [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; do TAMANHOS+=("$1"); shift; done ;;
    --teclas) TECLAS="${2:-}"; shift 2 ;;
    -h|--ajuda) sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

verde=$'\033[32m'; vermelho=$'\033[31m'; zero=$'\033[0m'
ok() { printf '  %sok%s   %s\n' "$verde" "$zero" "$*"; }
falha() { printf '  %sFALHA%s %s\n' "$vermelho" "$zero" "$*"; }
aviso() { printf '  aviso %s\n' "$*"; }

[ -x "$REAL" ] || { falha "launcher de produção não compilado — rode bash scripts/compilar-launcher.sh"; exit 1; }
[ -x "$TESTE" ] || { falha "launcher-teste não compilado — rode bash scripts/compilar-launcher-teste.sh"; exit 1; }
[ -d "$LIBS" ] || { falha "prefixo SDL2 ausente em $LIBS"; exit 1; }

mkdir -p "$SAIDA"
falhas=0

comparar_um() {
  local tamanho="$1" rot="${2:-none}" extra=()
  [ "$rot" = "270" ] && extra=(GIOCHI_ROTATE=270)
  # `--rotacao none` nos dois casos, de propósito: os dois binários foram
  # executados com a MESMA orientação, então as capturas já estão no mesmo
  # espaço de dispositivo. Pedir `--rotacao ccw` faria o verificador
  # des-rotacionar a captura e compará-la com uma referência em paisagem — que é
  # o modo de comparar aparelho × PC, não este.
  local rot_arg="none"

  local nome="real-$tamanho"; [ "$rot" = "270" ] && nome="real-${tamanho}-retrato"
  local nome_t="engine-$tamanho"; [ "$rot" = "270" ] && nome_t="engine-${tamanho}-retrato"

  env "${extra[@]}" SDL_VIDEODRIVER=dummy LD_LIBRARY_PATH="$LIBS" \
    "$REAL" --tamanho "$tamanho" --demo ${TECLAS:+--teclas "$TECLAS"} \
    --dump-frame "$SAIDA/$nome.bmp" >/dev/null 2>&1 \
    || { falha "$tamanho: o launcher de produção não gerou quadro"; return 1; }

  env "${extra[@]}" SDL_VIDEODRIVER=dummy LD_LIBRARY_PATH="$LIBS" \
    "$TESTE" --tamanho "$tamanho" --demo ${TECLAS:+--teclas "$TECLAS"} \
    --dump-frame "$SAIDA/$nome_t.bmp" >/dev/null 2>&1 \
    || { falha "$tamanho: o launcher-teste não gerou quadro"; return 1; }

  local saida
  saida=$(node "$RAIZ/scripts/verificar-frame.mjs" \
    --captura "$SAIDA/$nome_t.bmp" --referencia "$SAIDA/$nome.bmp" \
    --rotacao "$rot_arg" 2>&1)
  local pct
  pct=$(printf '%s\n' "$saida" | sed -n 's/.*comparação: \([0-9.]*\)%.*/\1/p')
  local texto
  texto=$(printf '%s\n' "$saida" | sed -n 's/.*texto: \(.*\)/\1/p')
  if [ "${pct:-0}" = "100.0" ]; then
    local etiqueta="$tamanho"
    [ "$rot" = "270" ] && etiqueta="$tamanho (retrato, Ccw90)"
    ok "$etiqueta: 100,0% — $texto"
  else
    local etiqueta="$tamanho"
    [ "$rot" = "270" ] && etiqueta="$tamanho (retrato, Ccw90)"
    falha "$etiqueta: ${pct:-?}% — $texto"
    return 1
  fi
}

for t in "${TAMANHOS[@]}"; do comparar_um "$t" || falhas=$((falhas + 1)); done
# O painel retrato é o caso em que a rotação e a textura girada erram juntas.
comparar_um "720x1280" 270 || falhas=$((falhas + 1))

echo
if [ "$falhas" -eq 0 ]; then
  ok "todos os quadros idênticos — o desenho não depende de quem desenha"
else
  falha "$falhas caso(s) divergente(s); BMPs em $SAIDA"
fi
exit "$falhas"
