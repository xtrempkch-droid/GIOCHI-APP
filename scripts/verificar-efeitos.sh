#!/usr/bin/env bash
# verificar-efeitos.sh — prova que a camada de efeitos do launcher-teste
# desenha, é determinística, não quebra o desenho da UI e cabe no orçamento.
#
#   bash scripts/verificar-efeitos.sh
#   bash scripts/verificar-efeitos.sh --tamanho 1280x720
#
# ## Por que esta ferramenta existe
#
# Efeito visual é o tipo de código que "parece funcionar": um quadro colorido é
# sempre um quadro colorido. As quatro perguntas que precisam de resposta são
# diferentes entre si, e nenhuma é respondida olhando a tela:
#
#   1. **Desenhou?**       compara com o quadro sem efeito e exige diferença.
#   2. **Determinístico?** duas execuções dão o mesmo BMP byte a byte — sem isso
#                          `--dump-frame` não serve de referência.
#   3. **Some quando desligado?** o quadro sem `--efeitos` tem de ser idêntico ao
#                          de antes da camada existir, senão a comparação de
#                          100 % com o launcher de produção deixa de valer.
#   4. **Cabe?**           conta as chamadas de desenho; crescer com o número de
#                          partículas significa que a batelada foi perdida.
#
# A 3 é a mais importante: a camada é opcional justamente para não invalidar a
# medida que dá sentido ao app.
#
# ## Por que `SDL_VIDEODRIVER=offscreen`
#
# `dummy` não tem OpenGL e o launcher-teste pede um contexto; `offscreen` tem
# EGL por software e desenha de verdade. A armadilha é a mesma do neon-drift.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/launcher-teste"
BIN="$APP/target/release/giochi-launcher-teste"
TAMANHO="640x480"
ROTACAO=""

while [ $# -gt 0 ]; do
  case "$1" in
    --tamanho) TAMANHO="$2"; shift 2 ;;
    --rotacao) ROTACAO="$2"; shift 2 ;;
    -h|--ajuda) sed -n '2,3p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumento desconhecido: $1" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; AMARELO=$'\033[33m'; NEGRITO=$'\033[1m'; ZERA=$'\033[0m'
# Contador de falhas. Sem ele o script imprimia "ok camada verificada" **depois**
# de um FALHA: um resumo que não olha para o que aconteceu é pior que nenhum,
# porque ensina a confiar no verde.
FALHAS=0
ok()     { printf '%s  ok%s   %s\n'   "$VERDE" "$ZERA" "$*"; }
falha()  { FALHAS=$((FALHAS + 1)); printf '%sFALHA%s %s\n' "$VERMELHO" "$ZERA" "$*"; }
aviso()  { printf '%saviso%s %s\n'    "$AMARELO" "$ZERA" "$*"; }
titulo() { printf '\n%s%s%s\n'         "$NEGRITO" "$*" "$ZERA"; }

[ -x "$BIN" ] || { falha "sem binário em $BIN — rode bash scripts/compilar-launcher-teste.sh --release"; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

desenhar() { # <destino> [flags extras...]
  local destino="$1"; shift
  SDL_VIDEODRIVER=offscreen "$BIN" --tamanho "$TAMANHO" ${ROTACAO:+--rotacao "$ROTACAO"} \
    --demo "$@" --dump-frame "$destino" >"$TMP/log" 2>&1 || {
      printf '%sFALHA%s o binário saiu com erro:\n' "$VERMELHO" "$ZERA"
      sed 's/^/  /' "$TMP/log"
      exit 1
    }
}

# Conta pixels diferentes entre dois BMP de mesmo tamanho.
diferentes() {
  node -e '
    const fs = require("fs");
    const [a, b] = process.argv.slice(1).map(f => fs.readFileSync(f));
    if (a.length !== b.length) { console.log(-1); process.exit(0); }
    let n = 0;
    for (let i = 54; i + 2 < a.length; i += 3) {
      if (Math.abs(a[i]-b[i]) + Math.abs(a[i+1]-b[i+1]) + Math.abs(a[i+2]-b[i+2]) > 12) n++;
    }
    console.log(n);
  ' "$1" "$2"
}

titulo "launcher-teste — camada de efeitos ($TAMANHO${ROTACAO:+, rotação $ROTACAO})"

# ── 1. determinismo ─────────────────────────────────────────
# Primeiro, porque as outras medidas comparam quadros: se dois desenhos iguais
# não derem o mesmo arquivo, toda diferença medida depois é ruído.
desenhar "$TMP/a.bmp" --efeitos
desenhar "$TMP/b.bmp" --efeitos
if cmp -s "$TMP/a.bmp" "$TMP/b.bmp"; then
  ok "duas execuções com --efeitos dão o mesmo BMP (determinístico)"
else
  falha "--efeitos não é determinístico: o mesmo comando deu BMPs diferentes"
fi

# ── 2. desenhou ─────────────────────────────────────────────
desenhar "$TMP/sem.bmp"
n=$(diferentes "$TMP/sem.bmp" "$TMP/a.bmp")
if [ "$n" -lt 0 ]; then
  falha "tamanhos diferentes entre com e sem efeitos"
elif [ "$n" -lt 2000 ]; then
  falha "só $n pixels mudaram — a camada não está desenhando"
else
  ok "a camada muda $n pixels do quadro (desenha de verdade)"
fi

# ── 3. não quebra o desenho quando desligada ────────────────
# A comparação com o launcher de produção é feita por
# `comparar-launcher-teste.sh` e `aparelho.sh verificar-teste`. Aqui se prova o
# pré-requisito dela: desligar a camada devolve o quadro de antes.
desenhar "$TMP/sem2.bmp"
if cmp -s "$TMP/sem.bmp" "$TMP/sem2.bmp"; then
  ok "sem --efeitos o quadro é estável (a comparação de 100 % continua válida)"
else
  falha "sem --efeitos o quadro variou entre duas execuções"
fi

# ── 4. batelada ─────────────────────────────────────────────
# O número de chamadas de desenho tem de ser **bounded**: se acompanhar a
# contagem de partículas, a batelada foi perdida e o custo cresce com a cena.
conta_desenhos() {
  SDL_VIDEODRIVER=offscreen "$BIN" --tamanho "$TAMANHO" ${ROTACAO:+--rotacao "$ROTACAO"} \
    --demo --efeitos --bench 60 "$@" 2>&1 | sed -n 's/.*· \([0-9]*\) desenho.*/\1/p'
}
d=$(conta_desenhos)
if [ -z "${d:-}" ]; then
  aviso "não consegui ler a contagem de desenho do --bench"
elif [ "$d" -gt 40 ]; then
  falha "$d chamadas de desenho — acima de 40, alguma batelada foi perdida"
else
  ok "$d chamada(s) de desenho para a camada inteira (batelada preservada)"
fi

# ── 5. cada efeito isolado desenha ──────────────────────────
titulo "efeito por efeito"
for efeito in vinheta scanlines varredura brasas brilho halo rastro borda; do
  GIOCHI_EFEITOS_SO="$efeito" SDL_VIDEODRIVER=offscreen "$BIN" --tamanho "$TAMANHO" \
    ${ROTACAO:+--rotacao "$ROTACAO"} --demo --efeitos --dump-frame "$TMP/e.bmp" >/dev/null 2>&1
  n=$(diferentes "$TMP/sem.bmp" "$TMP/e.bmp")
  # `rastro` só aparece depois de um comando: isolado e sem `--teclas` ele
  # legitimamente não muda nada, e exigir pixels dele seria exigir o contrário
  # do desenho.
  if [ "$efeito" = "rastro" ]; then
    printf '  %-11s (só reage a comando; medido no laço, não no quadro parado)\n' "$efeito"
    continue
  fi
  if [ "$n" -lt 100 ]; then
    falha "$efeito: $n pixels — não desenhou"
  else
    printf '  %-11s %s px\n' "$efeito" "$n"
  fi
done

# ── 6. o rastro aparece quando há comando ───────────────────
GIOCHI_EFEITOS_SO=rastro SDL_VIDEODRIVER=offscreen "$BIN" --tamanho "$TAMANHO" \
  ${ROTACAO:+--rotacao "$ROTACAO"} --demo --efeitos --teclas right,right \
  --dump-frame "$TMP/rastro.bmp" >/dev/null 2>&1
n=$(diferentes "$TMP/sem.bmp" "$TMP/rastro.bmp")
if [ "$n" -lt 50 ]; then
  falha "rastro: $n pixels mesmo com dois comandos — a rajada não desenhou"
else
  ok "rastro: $n pixels depois de dois comandos"
fi

titulo "resumo"
mkdir -p "$RAIZ/dist"
cp "$TMP/a.bmp" "$RAIZ/dist/efeitos-launcher-teste-$TAMANHO.bmp" 2>/dev/null \
  && printf '  cópia em           : dist/efeitos-launcher-teste-%s.bmp\n' "$TAMANHO"
echo
if [ "$FALHAS" -gt 0 ]; then
  falha "$FALHAS verificação(ões) falharam"
  exit 1
fi
ok "camada de efeitos verificada"
