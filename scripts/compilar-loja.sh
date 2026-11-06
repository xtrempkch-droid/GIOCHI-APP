#!/usr/bin/env bash
# Compila a loja (Rust + SDL2) resolvendo as bibliotecas de desenvolvimento.
#
#   bash scripts/compilar-loja.sh              # debug, com texto
#   bash scripts/compilar-loja.sh --release    # release
#   bash scripts/compilar-loja.sh --testes     # roda os testes
#
# ## Por que não é só `cargo build`
#
# Pelo mesmo motivo do launcher: `sdl2-sys` precisa de `pkg-config` para achar
# `sdl2` e `SDL2_ttf`. Sem elas o `rustc` compila e o **link** falha com
# `unable to find library -lSDL2` — que parece problema de código e é de
# ambiente.
#
# O `compilar-launcher.sh` já sabe resolver isso (extrai os `.deb` num prefixo
# local quando não há root). Este script **reusa** aquela resolução em vez de
# repetir a lógica: duas cópias da mesma regra envelhecem diferente, e este
# projeto já pagou esse preço com a lista de `runtime` e com o formato da
# assinatura.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOJA="$RAIZ/app/loja"
PREFIXO="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
MODO="build"
PERFIL=""

for arg in "$@"; do
  case "$arg" in
    --release) PERFIL="--release" ;;
    --testes) MODO="test" ;;
    -h|--ajuda) sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumento desconhecido: $arg" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()   { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
erro() { printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

command -v cargo >/dev/null || erro "cargo não encontrado (instale Rust: https://rustup.rs)"

LIBDIR="$PREFIXO/usr/lib/x86_64-linux-gnu"
if [ ! -f "$LIBDIR/pkgconfig/sdl2.pc" ]; then
  echo "  SDL2 de desenvolvimento ausente em $PREFIXO; resolvendo..."
  # `--preparar` não existe no outro script; ele faz isso por conta própria ao
  # compilar. Chamar o caminho de preparação sem compilar o launcher precisa de
  # um alvo explícito, então usamos o próprio script com `--somente-preparar`
  # quando existir, e caímos para a mensagem clara quando não.
  if grep -q "somente-preparar" "$RAIZ/scripts/compilar-launcher.sh" 2>/dev/null; then
    bash "$RAIZ/scripts/compilar-launcher.sh" --somente-preparar >/dev/null 2>&1 || true
  fi
fi
[ -f "$LIBDIR/pkgconfig/sdl2.pc" ] || erro "SDL2 não resolvido.
  rode: bash scripts/compilar-launcher.sh
  (é ele que extrai os .deb em $PREFIXO)"

export PKG_CONFIG_PATH="$LIBDIR/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export LIBRARY_PATH="$LIBDIR${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$LIBDIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

cd "$LOJA"

if [ "$MODO" = "test" ]; then
  echo "  testes do núcleo (sem SDL):"
  # Os testes do núcleo são os que não precisam de janela. Rodam primeiro porque
  # são a parte que pode falhar por lógica, não por ambiente.
  cargo test -p giochi-loja-core 2>&1 | grep -E "^test result|^error" | sed 's/^/    /'
  echo "  testes da interface:"
  cargo test -p giochi-loja 2>&1 | grep -E "^test result|^error" | sed 's/^/    /'
  ok "testes"
  exit 0
fi

echo "  compilando ($MODO $PERFIL)..."
cargo build $PERFIL 2>&1 | grep -E "^(error|warning: unused)" -A 2 | sed 's/^/    /' || true

ALVO="$LOJA/target/release/giochi-loja"
[ -n "$PERFIL" ] || ALVO="$LOJA/target/debug/giochi-loja"
[ -x "$ALVO" ] || erro "build falhou (nenhum binário em $ALVO)"

ok "binário: $ALVO ($(stat -c%s "$ALVO") bytes)"

# Prova de que desenha, sem tela — a mesma que o launcher usa. Se o quadro não
# sai, o problema é de desenho e não de "o usuário não achou o ícone".
QUADRO="${TMPDIR:-/tmp}/giochi-loja-quadro.bmp"
if SDL_VIDEODRIVER=dummy "$ALVO" --dump-frame "$QUADRO" >/dev/null 2>&1 && [ -s "$QUADRO" ]; then
  ok "desenhou um quadro sem tela ($(stat -c%s "$QUADRO") bytes)"
  rm -f "$QUADRO"
else
  erro "não consegui desenhar um quadro; o binário abriu mas não renderizou"
fi

echo
echo "  rode com: app/loja/bin/giochi-loja"
