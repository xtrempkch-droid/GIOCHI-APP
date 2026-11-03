#!/usr/bin/env bash
# ============================================================
# Migra os apps e jogos do antecessor GIOCHI-KONSOL-OS
# ============================================================
# Uso:
#   bash scripts/migrar-do-antecessor.sh <caminho-do-clone>
#
# Exemplo:
#   git clone https://github.com/xtrempkch-droid/GIOCHI-KONSOL-OS.git ~/giochi-antigo
#   bash scripts/migrar-do-antecessor.sh ~/giochi-antigo
#
# Regras:
#   - nunca sobrescreve destino que já tem conteúdo;
#   - copia para src/ (código) e assets/ (arte/documentação);
#   - não copia o launcher (será redesenhado) nem Shattered Pixel Dungeon (GPL-3.0).
# ============================================================

set -euo pipefail

SOURCE="${1:-}"

if [[ -z "$SOURCE" ]]; then
  echo "Uso: bash scripts/migrar-do-antecessor.sh <caminho-do-clone-do-KONSOL-OS>" >&2
  exit 2
fi

if [[ ! -d "$SOURCE" ]]; then
  echo "ERRO: caminho não encontrado: $SOURCE" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPS="$ROOT/app"

if [[ ! -d "$APPS" ]]; then
  echo "ERRO: pasta app/ não encontrada em $ROOT" >&2
  exit 2
fi

COPIED=0
SKIPPED=0
MISSING=0

copy_tree() {
  local src="$1"
  local dest="$2"
  local label="$3"

  if [[ ! -d "$src" ]]; then
    echo "  FALTA  $label"
    echo "         esperado em: $src"
    MISSING=$((MISSING + 1))
    return 0
  fi

  mkdir -p "$dest"

  # Considera ocupado se houver qualquer item além de .gitkeep / README de arte
  local occupied
  occupied="$(find "$dest" -mindepth 1 -maxdepth 1 \
    ! -name '.gitkeep' ! -name 'README.md' 2>/dev/null | head -n 1 || true)"

  if [[ -n "$occupied" ]]; then
    echo "  PULA   $label (destino já tem conteúdo)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  cp -a "$src/." "$dest/"
  echo "  OK     $label"
  COPIED=$((COPIED + 1))
  return 0
}

copy_file() {
  local src="$1"
  local dest="$2"
  local label="$3"

  if [[ ! -f "$src" ]]; then
    echo "  FALTA  $label"
    MISSING=$((MISSING + 1))
    return 0
  fi

  if [[ -e "$dest" ]]; then
    echo "  PULA   $label (destino já existe)"
    SKIPPED=$((SKIPPED + 1))
    return 0
  fi

  mkdir -p "$(dirname "$dest")"
  cp -a "$src" "$dest"
  echo "  OK     $label"
  COPIED=$((COPIED + 1))
  return 0
}

echo "Migrando de: $SOURCE"
echo "Destino:     $APPS"
echo

echo "Apps:"
copy_tree "$SOURCE/apps/forge/giochi_forge" "$APPS/forge/src/giochi_forge" "forge (módulo Python)"
copy_file "$SOURCE/apps/forge/giochi-forge" "$APPS/forge/src/giochi-forge" "forge (atalho executável)"

copy_tree "$SOURCE/apps/store/giochi_store" "$APPS/store/src/giochi_store" "store (módulo Python)"
copy_file "$SOURCE/apps/store/giochi-store" "$APPS/store/src/giochi-store" "store (atalho executável)"
echo

echo "Jogos:"
copy_tree "$SOURCE/Jogos/Arcade Volleyball/godot" "$APPS/arcade-volleyball/src" "arcade-volleyball (projeto Godot)"
copy_tree "$SOURCE/Jogos/Colossal Cave Adventure/godot" "$APPS/colossal-cave-adventure/src" "colossal-cave-adventure (projeto Godot)"
copy_tree "$SOURCE/Jogos/Spacewar/godot" "$APPS/spacewar/src" "spacewar (projeto Godot)"
copy_file "$SOURCE/Jogos/Spacewar/Spacewar-1962-listing.pdp1" \
  "$APPS/spacewar/assets/Spacewar-1962-listing.pdp1" "spacewar (listing histórico do PDP-1)"
echo

echo "Não migrados por decisão:"
echo "  launcher                    — será redesenhado por completo"
echo "  Shattered Pixel Dungeon     — GPL-3.0, fora da regra de licença"
echo

echo "Resumo: $COPIED copiado(s), $SKIPPED pulado(s), $MISSING ausente(s)."

if [[ "$MISSING" -gt 0 ]]; then
  echo
  echo "Atenção: houve caminho ausente. Confira se o clone está na branch main"
  echo "e se o antecessor ainda mantém a estrutura esperada."
fi

echo
echo "Próximo passo: node scripts/validate-app.mjs --all"
