#!/usr/bin/env bash
# Roda os testes dos apps de PC (Python) — os do antecessor e os nossos.
#
# Uso:
#   bash scripts/testar-apps-pc.sh
#   bash scripts/testar-apps-pc.sh --forge     # só o forge
#
# ## Por que um script, se `python3 -m unittest` já resolve
#
# Porque o PYTHONPATH e o diretório de trabalho **importam** e errar os dois não
# dá erro: os testes simplesmente não são descobertos e o comando sai com 0. Um
# "0 testes" verde é exatamente o falso positivo que este projeto combate. Aqui
# a contagem é conferida: se nenhum teste rodar, sai com erro.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ALVO="${1:-todos}"

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()  { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
erro(){ printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

command -v python3 >/dev/null || erro "python3 não encontrado"

rodar() {
  local nome="$1" pacote="$2" src="$3"
  printf '\n%s\n' "$nome"
  local saida
  saida=$(cd "$src" && PYTHONPATH="." python3 -m unittest discover \
    -s "$RAIZ/tests/$pacote" -t "$RAIZ/tests/$pacote" 2>&1) || {
      printf '%s\n' "$saida" >&2
      erro "$nome: testes falharam"
    }
  printf '%s\n' "$saida" | tail -3
  # `Ran N tests` com N > 0. Sem isto, descobrir zero arquivos daria verde.
  local n
  n=$(printf '%s\n' "$saida" | sed -n 's/^Ran \([0-9]\+\) test.*/\1/p' | head -1)
  [ -n "${n:-}" ] && [ "$n" -gt 0 ] || erro "$nome: nenhum teste rodou (descoberta falhou)"
  ok "$n teste(s)"
}

case "$ALVO" in
  --forge) rodar "giochi-forge" forge "$RAIZ/app/forge/src" ;;
  --store) rodar "giochi-store" store "$RAIZ/app/store/src" 2>/dev/null || \
             printf '  (o store ainda não tem testes próprios)\n' ;;
  *)
    rodar "giochi-forge" forge "$RAIZ/app/forge/src"
    if [ -d "$RAIZ/tests/store" ]; then rodar "giochi-store" store "$RAIZ/app/store/src"; fi
    ;;
esac
