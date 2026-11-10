#!/usr/bin/env bash
# verificar-star-dust.sh — prova que o Star Dust desenha os efeitos, é
# determinístico, cabe no orçamento de chamadas e que **desligado ele é
# idêntico ao launcher de produção**.
#
#   bash scripts/verificar-star-dust.sh
#   bash scripts/verificar-star-dust.sh --tamanho 1280x720
#   bash scripts/verificar-star-dust.sh --tamanho 720x1280 --rotacao ccw
#
# ## Por que esta ferramenta existe
#
# "Efeito visual" é o tipo de código que parece funcionar: um quadro colorido é
# sempre um quadro colorido. As cinco perguntas diferentes que precisam de
# resposta, e nenhuma se responde olhando a tela:
#
#   1. **Desenhou?**             compara com o quadro sem efeito e exige diferença.
#   2. **Determinístico?**       duas execuções dão o mesmo BMP byte a byte — sem
#                                isso `--dump-frame` não serve de referência.
#   3. **Desligado é produção?** o quadro sem efeitos tem de ser **idêntico** ao
#                                do launcher de produção. É a prova de que o
#                                clone não regrediu nada ao receber a camada.
#   4. **Cabe?**                 conta as chamadas ao backend; se crescer com o
#                                número de partículas, a batelada foi perdida.
#   5. **Cada efeito desenha?**  um de cada vez, com `GIOCHI_EFEITOS_SO`.
#
# A 3 é a mais forte, e só existe porque a camada é desligável. Sem ela, "o clone
# está igual" seria afirmação.
#
# ## Relógio, bateria e temperatura congelados
#
# As três vêm do **sistema** e mudam entre duas execuções: o relógio vira o
# minuto, a temperatura oscila 1 °C. Sem congelar, a comparação byte a byte
# falharia por motivo que não é desenho — foi o que derrubou a comparação do
# X35H para 99,5 % antes de existirem estas variáveis.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/launcher-star-dust"
BIN="$APP/target/release/giochi-launcher-star-dust"
PRODUCAO="$RAIZ/app/launcher/target/release/giochi-launcher"
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
FALHAS=0
ok()     { printf '%s  ok%s   %s\n'   "$VERDE" "$ZERA" "$*"; }
falha()  { FALHAS=$((FALHAS + 1)); printf '%sFALHA%s %s\n' "$VERMELHO" "$ZERA" "$*"; }
aviso()  { printf '%saviso%s %s\n'    "$AMARELO" "$ZERA" "$*"; }
titulo() { printf '\n%s%s%s\n'         "$NEGRITO" "$*" "$ZERA"; }

[ -x "$BIN" ] || { falha "sem binário em $BIN — rode (cd app/launcher-star-dust && cargo build --release)"; exit 1; }
if [ ! -x "$PRODUCAO" ]; then
  falha "sem o launcher de produção em $PRODUCAO — rode (cd app/launcher && cargo build --release)"
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Sistema congelado dos dois lados: só o desenho pode diferir.
export GIOCHI_HORA_FIXA="12:00"
export GIOCHI_BATERIA_FIXA="77"
export GIOCHI_TEMPERATURA_FIXA="40"

desenhar() { # <destino> [flags extras...]
  local destino="$1"; shift
  SDL_VIDEODRIVER=dummy "$BIN" --tamanho "$TAMANHO" ${ROTACAO:+--rotacao "$ROTACAO"} \
    --demo "$@" --dump-frame "$destino" >"$TMP/log" 2>&1 || {
      printf '%sFALHA%s o binário saiu com erro:\n' "$VERMELHO" "$ZERA"
      sed 's/^/  /' "$TMP/log"
      exit 1
    }
}

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

chamadas() { sed -n 's/^efeitos: \([0-9]*\) chamada.*/\1/p' "$TMP/log" | tail -1; }

titulo "star dust — camada de efeitos ($TAMANHO${ROTACAO:+, rotação $ROTACAO})"

# ── 1. determinismo ─────────────────────────────────────────
desenhar "$TMP/a.bmp"
desenhar "$TMP/b.bmp"
if cmp -s "$TMP/a.bmp" "$TMP/b.bmp"; then
  ok "duas execuções dão o mesmo BMP (a animação é por contagem de quadro)"
else
  falha "não é determinístico: o mesmo comando deu BMPs diferentes"
fi

# ── 2. desenhou ─────────────────────────────────────────────
desenhar "$TMP/sem.bmp" --sem-efeitos
n=$(diferentes "$TMP/sem.bmp" "$TMP/a.bmp")
if [ "$n" -lt 0 ]; then
  falha "tamanhos diferentes entre com e sem efeitos"
elif [ "$n" -lt 2000 ]; then
  falha "só $n pixels mudaram — a camada não está desenhando"
else
  ok "a camada muda $n pixels do quadro (desenha de verdade)"
fi

# ── 3. desligado == launcher de produção ────────────────────
# A prova de que o clone não regrediu. Não é "parecido": é byte a byte, com
# relógio, bateria e temperatura congelados nos dois lados.
SDL_VIDEODRIVER=dummy "$PRODUCAO" --tamanho "$TAMANHO" ${ROTACAO:+--rotacao "$ROTACAO"} \
  --demo --dump-frame "$TMP/producao.bmp" >"$TMP/log-prod" 2>&1 \
  || { falha "o launcher de produção não gerou quadro"; sed 's/^/  /' "$TMP/log-prod"; }
if cmp -s "$TMP/producao.bmp" "$TMP/sem.bmp"; then
  ok "com --sem-efeitos o quadro é IDÊNTICO ao do launcher de produção"
else
  n=$(diferentes "$TMP/producao.bmp" "$TMP/sem.bmp")
  falha "o quadro sem efeitos difere do launcher de produção em $n pixels"
fi

# ── 4. batelada ─────────────────────────────────────────────
desenhar "$TMP/x.bmp"
d=$(chamadas)
if [ -z "${d:-}" ]; then
  falha "não consegui ler a contagem de chamadas do log"
elif [ "$d" -gt 40 ]; then
  falha "$d chamadas de desenho — acima de 40, alguma batelada foi perdida"
else
  ok "$d chamada(s) de desenho para a camada inteira (batelada preservada)"
fi

# ── 5. cada efeito isolado desenha ──────────────────────────
titulo "efeito por efeito"
for efeito in vinheta scanlines varredura brasas brilho halo rastro borda; do
  GIOCHI_EFEITOS_SO="$efeito" SDL_VIDEODRIVER=dummy "$BIN" --tamanho "$TAMANHO" \
    ${ROTACAO:+--rotacao "$ROTACAO"} --demo --dump-frame "$TMP/e.bmp" >/dev/null 2>&1
  n=$(diferentes "$TMP/sem.bmp" "$TMP/e.bmp")
  # `rastro` só aparece depois de um comando: isolado e sem `--teclas` ele
  # legitimamente não muda nada, e exigir pixels dele num quadro parado seria
  # exigir o contrário do desenho.
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
GIOCHI_EFEITOS_SO=rastro SDL_VIDEODRIVER=dummy "$BIN" --tamanho "$TAMANHO" \
  ${ROTACAO:+--rotacao "$ROTACAO"} --demo --teclas right,right \
  --dump-frame "$TMP/rastro.bmp" >/dev/null 2>&1
n=$(diferentes "$TMP/sem.bmp" "$TMP/rastro.bmp")
if [ "$n" -lt 50 ]; then
  falha "rastro: $n pixels mesmo com dois comandos — a rajada não desenhou"
else
  ok "rastro: $n pixels depois de dois comandos"
fi

# ── 7. telemetria não dispara rajada ────────────────────────
# Bateria, temperatura e "controle conectado" também mudam o estado. Se a
# rajada reagisse a eles, o rastro apareceria sozinho e o efeito perderia o
# significado — medido antes desta guarda: 20 chamadas num quadro sem tecla.
GIOCHI_EFEITOS_SO=rastro SDL_VIDEODRIVER=dummy "$BIN" --tamanho "$TAMANHO" \
  ${ROTACAO:+--rotacao "$ROTACAO"} --demo --dump-frame "$TMP/parado.bmp" >/dev/null 2>&1
n=$(diferentes "$TMP/sem.bmp" "$TMP/parado.bmp")
if [ "$n" -gt 50 ]; then
  falha "rastro desenhou $n pixels sem nenhum comando — a rajada está reagindo a telemetria"
else
  ok "sem comando o rastro não aparece ($n px) — telemetria não dispara rajada"
fi

titulo "resumo"
mkdir -p "$RAIZ/dist"
cp "$TMP/a.bmp" "$RAIZ/dist/star-dust-$TAMANHO.bmp" 2>/dev/null \
  && printf '  cópia em           : dist/star-dust-%s.bmp\n' "$TAMANHO"
echo
if [ "$FALHAS" -gt 0 ]; then
  falha "$FALHAS verificação(ões) falharam"
  exit 1
fi
ok "camada de efeitos verificada"
