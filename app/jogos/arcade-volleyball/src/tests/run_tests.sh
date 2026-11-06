#!/bin/sh
# Testes do jogo Godot: invariantes, fumaça da cena e orcamento de RAM/VRAM.
#
#   tests/run_tests.sh                 # usa `godot` do PATH
#   GODOT=/caminho/godot tests/run_tests.sh
#
# O motor Godot nao e dependencia do repositorio: sem ele, o script sai com 0
# e avisa (mesmo criterio de tests/test_screenshots.py, que pula sem Pillow).
# O teste de orcamento precisa de contexto OpenGL e por isso e pulado quando
# nao ha DISPLAY.
set -u

cd "$(dirname "$0")/.." || exit 1

GODOT=${GODOT:-godot}
if ! command -v "$GODOT" >/dev/null 2>&1; then
    echo "godot: motor nao encontrado (defina GODOT=/caminho/do/binario); pulando"
    exit 0
fi

fail() {
    echo "FALHOU: $1" >&2
    exit 1
}

# O cache de classes globais (class_name) so existe depois de uma passada de
# importacao: sem isso o carregamento por --script falha com "Could not find
# type". Roda sempre, e barato.
echo "== importacao (cache de classes) =="
"$GODOT" --headless --path . --editor --quit >/tmp/giochi-godot-import.log 2>&1 ||
    { cat /tmp/giochi-godot-import.log >&2; fail "importacao"; }
echo "ok"

echo "== invariantes da simulacao =="
out=$("$GODOT" --headless --path . --script res://tests/invariants.gd 2>&1)
status=$?
echo "$out" | grep -E "partida longa|fuzz|FALHA|PASSARAM"
[ "$status" -eq 0 ] || fail "invariantes (saida $status)"

echo "== cena principal (fumaca, 240 quadros) =="
out=$("$GODOT" --headless --path . res://scenes/Main.tscn --quit-after 240 2>&1)
case "$out" in
    *"SCRIPT ERROR"*) echo "$out" >&2; fail "erro de script ao rodar a cena" ;;
    *"ERROR:"*) echo "$out" >&2; fail "erro do engine ao rodar a cena" ;;
esac
echo "ok"

if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    echo "== orcamento (RAM/VRAM) =="
    echo "sem DISPLAY: teste de orcamento pulado (precisa de OpenGL)"
    exit 0
fi

echo "== orcamento (RAM/VRAM, tres presets) =="
# Roda uma vez so: o exit code e a fonte da verdade, o texto vai para a tela.
out=$("$GODOT" --path . --script res://tests/budget_probe.gd 2>&1)
status=$?
echo "$out" | grep -E "orcamento|RAM |ORCAMENTO|medido"
[ "$status" -eq 0 ] || fail "orcamento estourado (saida $status)"
echo "ok"
