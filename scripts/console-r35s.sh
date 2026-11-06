#!/usr/bin/env bash
# Console serial do R35S — o único canal que ele tem.
#
#   bash scripts/console-r35s.sh "uname -r"
#   bash scripts/console-r35s.sh "cat /sys/class/graphics/fb0/virtual_size" "date -u"
#   bash scripts/console-r35s.sh --escutar 12          # só ouve (boot, panic)
#   bash scripts/console-r35s.sh --porta /dev/ttyUSB0 --baud 1500000 "..."
#
# ## Por que este script existe, e por que não é o `aparelho.sh`
#
# O `aparelho.sh` é todo feito de `ssh`/`scp`, e isso pressupõe rede. O **R35S
# não tem rádio** e o `dwc2` nasce como *host*, então não há SSH nem `usb0`: a
# console serial (`ttyS2`, 1500000 baud) é o único caminho.
#
# ## Por que dentro de um container
#
# Dois motivos, e o segundo é o que dói:
#
# 1. **Permissão.** O nó é `root:dialout`, e quem não está no grupo não abre.
# 2. **O adaptador cai.** O CH341 sai do barramento de vez em quando; com
#    `-v /dev:/dev` o container sobrevive à queda, enquanto um
#    `--device=/dev/ttyUSB0` fixado no `start` aborta quando o nó some.
#    Medido pela equipe do System.
#
# `--privileged` é necessário: montar `/dev` não basta, o cgroup de devices
# bloqueia a abertura com "Operation not permitted".
#
# ## A armadilha que já custou tempo
#
# Um `docker run --rm` cujo script do host foi morto **continua rodando** e
# segurando a porta. O próximo leitor recebe `returned no data` — a mesma
# mensagem de "adaptador desconectado", que manda procurar defeito no cabo.
# Por isso os leitores órfãos são encerrados antes de abrir.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGEM="${GIOCHI_SERIAL_IMAGE:-giochi-serial-scan:3}"
PORTA="${GIOCHI_SERIAL_PORT:-/dev/ttyUSB0}"
BAUD="${GIOCHI_SERIAL_BAUD:-1500000}"
ESCUTAR=""

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; AMARELO=$'\033[33m'; ZERA=$'\033[0m'
ok()    { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
aviso() { printf '  %sAVISO%s %s\n' "$AMARELO" "$ZERA" "$*"; }
erro()  { printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

# ── argumentos ────────────────────────────────────────────
COMANDOS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --porta) PORTA="$2"; shift 2 ;;
    --baud)  BAUD="$2";  shift 2 ;;
    --escutar) ESCUTAR="${2:-12}"; shift 2 ;;
    -h|--ajuda) sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) COMANDOS+=("$1"); shift ;;
  esac
done

command -v docker >/dev/null || erro "docker não encontrado; é o caminho para abrir a serial sem estar no grupo dialout"
docker image inspect "$IMAGEM" >/dev/null 2>&1 || erro "imagem $IMAGEM ausente.
  construa no repositório do sistema:
    bash ../GIOCHI-SYSTEM/scripts/serial-check.sh --build"

# ── órfãos ────────────────────────────────────────────────
ORFAOS=$(docker ps -q --filter "ancestor=$IMAGEM" 2>/dev/null)
if [ -n "$ORFAOS" ]; then
  aviso "encerrando leitor(es) órfão(s) da serial: $(echo $ORFAOS)"
  for id in $ORFAOS; do docker kill "$id" >/dev/null 2>&1 || true; done
  sleep 1
fi

# O nó pode não existir se o adaptador caiu. Dizer isso é melhor que o erro do
# driver, que fala de "no such device" e não de cabo.
if [ ! -e "$PORTA" ]; then
  erro "$PORTA não existe. O adaptador está conectado? (o CH341 cai do barramento)"
fi

# ── escuta passiva ────────────────────────────────────────
if [ -n "$ESCUTAR" ]; then
  echo "  ouvindo $PORTA a $BAUD baud por ${ESCUTAR}s (não escreve nada)"
  echo "  ligue/reinicie o aparelho agora"
  echo
  docker run --rm --privileged -v /dev:/dev --entrypoint python3 "$IMAGEM" \
    -u /work/serial-listen.py "$PORTA" "$BAUD" "$ESCUTAR" /tmp/escuta.bin 3600 2>&1 | tail -20
  exit $?
fi

[ ${#COMANDOS[@]} -gt 0 ] || erro "informe ao menos um comando (ou use --escutar N)"

# ── executa ───────────────────────────────────────────────
docker run --rm --privileged -v /dev:/dev \
  -v "$RAIZ/scripts/lib:/w:ro" \
  --entrypoint python3 "$IMAGEM" \
  -u /w/console-serial.py "$PORTA" "$BAUD" "${COMANDOS[@]}"
CODIGO=$?

if [ "$CODIGO" -eq 2 ]; then
  aviso "a porta não respondeu. Confira, nesta ordem:
    1. o aparelho está ligado? (a serial só fala com o console ativo)
    2. o adaptador está em 3V3 e o GND comum? (R35S é 3V3)
    3. TX do adaptador no RX do aparelho, e não o contrário
    4. o baud: o cmdline do R35S declara console=ttyS2,1500000"
fi

echo
if [ "$CODIGO" -eq 0 ]; then
  ok "todos os comandos responderam"
else
  aviso "algum comando falhou ou não respondeu (código $CODIGO)"
fi
exit "$CODIGO"
