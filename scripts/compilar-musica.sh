#!/usr/bin/env bash
# Compila e testa o app de música no PC.
#
# Uso:
#   bash scripts/compilar-musica.sh              # cargo build (debug)
#   bash scripts/compilar-musica.sh --release    # repassa a flag ao cargo
#   bash scripts/compilar-musica.sh --testes     # núcleo (sem SDL) + binário
#   bash scripts/compilar-musica.sh --clippy     # `-D warnings`, como o CI
#   bash scripts/compilar-musica.sh --verificar  # determinismo e rotação
#
# ## Por que o núcleo é testado separado
#
# `crates/core` não tem dependência nenhuma: é o que o CI consegue rodar. O
# binário depende da **engine**, que mora num repositório irmão e depende do
# SDL2 — e é por isso que ele não roda no CI (`launcher-teste` está fora pelo
# mesmo motivo). Testar os dois aqui é o que fecha a lacuna localmente.
#
# ## Por que este script confere as bibliotecas antes de compilar
#
# A engine liga `SDL2`, `SDL2_image`, `SDL2_mixer` e `SDL2_ttf`. Faltando uma,
# o `rustc` compila o crate inteiro e quem falha é o **link**, com
# `unable to find library -lSDL2_mixer` — mensagem que não diz qual pacote
# instalar. A pergunta é feita antes, e a resposta é o comando `apt`.
set -euo pipefail

# `set -e` mata o script **sem imprimir nada** quando um comando falha dentro de
# `$(...)`: medido aqui, `resolver_aparelho` saía com código 1 e zero linhas de
# saída — indistinguível de um build que não rodou (§13 do AGENTS.md). O trap
# garante que a última palavra seja uma frase com a linha.
trap 'printf "\033[31merro:\033[0m linha %s falhou: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR

MODO="build"
ARGS_CARGO=()

for arg in "$@"; do
  case "$arg" in
    --testes) MODO="test" ;;
    --clippy) MODO="clippy" ;;
    --verificar) MODO="verificar" ;;
    -h|--ajuda) sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) ARGS_CARGO+=("$arg") ;;
  esac
done

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/musica"
ENGINE="$RAIZ/../GIOCHI-ENGINE-3D/giochi-engine"

log() { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

command -v cargo >/dev/null || erro "cargo não encontrado. Instale o Rust: https://rustup.rs"

[ -f "$ENGINE/Cargo.toml" ] || erro "engine não encontrada em $ENGINE
  o app depende do repositório irmão GIOCHI-ENGINE-3D:
  git clone <url> $(dirname "$ENGINE")
  é a única dependência fora deste repositório (o launcher-teste tem a mesma)."

# O `sdl2-sys` usa pkg-config. `--exists` valida também os `Requires.private`
# (harfbuzz/freetype do SDL2_ttf), que o linker dinâmico não precisa — por isso
# a checagem é pela versão, como no compilar-launcher.sh.
for pacote in sdl2 SDL2_ttf SDL2_mixer SDL2_image; do
  pkg-config --modversion "$pacote" >/dev/null 2>&1 || erro "falta $pacote (pkg-config)
  sudo apt install libsdl2-dev libsdl2-ttf-dev libsdl2-mixer-dev libsdl2-image-dev"
done

case "$MODO" in
  test)
    # O núcleo primeiro: ele é o que o CI roda, e um erro aqui tem de aparecer
    # antes do teste que precisa de SDL.
    log "cargo test — núcleo (sem SDL): $APP/crates/core"
    ( cd "$APP/crates/core" && cargo test "${ARGS_CARGO[@]}" 2>&1 | grep -E "^test result|^error" || true )
    ( cd "$APP/crates/core" && cargo test "${ARGS_CARGO[@]}" >/dev/null 2>&1 ) \
      || erro "testes do núcleo falharam"

    log "cargo test — binário (engine + SDL2)"
    ( cd "$APP" && cargo test "${ARGS_CARGO[@]}" 2>&1 | grep -E "^test result|^error" || true )
    ( cd "$APP" && cargo test "${ARGS_CARGO[@]}" >/dev/null 2>&1 ) \
      || erro "testes do binário falharam"
    ;;

  clippy)
    # `-D warnings` e não só "rodar clippy": aviso que ninguém lê é aviso que não
    # existe (§13 do AGENTS.md).
    log "cargo clippy -D warnings — núcleo"
    ( cd "$APP/crates/core" && cargo clippy --all-targets -- -D warnings )
    log "cargo clippy -D warnings — binário"
    ( cd "$APP" && cargo clippy --all-targets -- -D warnings )
    ;;

  verificar)
    log "verificação de quadro (determinismo, rotação, orçamento)"
    node "$RAIZ/scripts/verificar-musica.mjs" "$@"
    ;;

  *)
    log "cargo build ${ARGS_CARGO[*]-}"
    ( cd "$APP" && cargo build "${ARGS_CARGO[@]}" )
    BIN="$APP/target/debug/giochi-musica"
    [ -f "$BIN" ] || BIN="$APP/target/release/giochi-musica"
    [ -f "$BIN" ] && log "pronto: $BIN"
    ;;
esac
