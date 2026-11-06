#!/bin/sh
# Descobre e monta os cartões removíveis do aparelho. Roda **no aparelho**.
#
#     descobrir-cartoes.sh <ponto-base>
#
# Saída: uma linha `PONTO=<mountpoint>` por cartão montado. Nada quando não há
# cartão. Sai sempre com 0 — "nenhum cartão" é resposta, não erro.
#
# ## Por que não existe nome de dispositivo aqui
#
# A primeira versão usava `/dev/mmcblk1p1` fixo, porque é o que o X55 tem. O dono
# avisou que **alguns aparelhos têm dois slots de SD e ainda eMMC**, com o sistema
# num cartão e os jogos no outro — e casos de jogos nos dois. Nome de dispositivo
# no código é a mesma classe de bug do `DEFAULT_ROTATION = Ccw90`: acerta num
# aparelho e mente no seguinte, sem avisar.
#
# ## Três armadilhas, todas medidas no X55
#
# 1. **`removable` não serve.** `/sys/class/block/mmcblk1/removable = 0` para um
#    cartão SD físico. Qualquer filtro por `removable=1` acharia **nada**, nem
#    num aparelho com dois slots.
# 2. **O glob pega partição.** `/sys/class/block/mmcblk*` casa com `mmcblk1`,
#    `mmcblk1p1` e `mmcblk1p2` — tratar cada um como "disco" montou a mesma
#    partição duas vezes. O teste correto é a existência de
#    `/sys/class/block/<nome>/partition`: o kernel só cria esse arquivo para
#    partições.
# 3. **O cartão do sistema inteiro tem de ser ignorado**, não só a partição do
#    rootfs. A `GIOCHI_BOOT` dele também é um sistema de arquivos válido — mas
#    não é cartão de jogos.
#
# ## O critério que funciona
#
# Qual disco sustenta a partição montada em `/`? Esse disco está fora. Todo o
# resto com partição legível é candidato. Assim o mesmo script serve para:
#
#   - 1 cartão (o do sistema)          → 0 candidatos, e é o correto;
#   - 2 cartões                        → 1 candidato (o de jogos);
#   - eMMC + 2 cartões                 → 2 candidatos.

set -u

BASE="${1:-/mnt/cartao}"

# ── qual disco sustenta `/` ────────────────────────────────────────────
#
# `/proc/mounts` mostra `/dev/root`, que é nome lógico — comparar por caminho
# falha. `mountinfo` traz major:minor da fonte, que é fato do kernel.
raiz_mm=$(awk '$5 == "/" { print $3; exit }' /proc/self/mountinfo 2>/dev/null)

disco_da_raiz=""
if [ -n "$raiz_mm" ]; then
  for b in /sys/class/block/*; do
    [ -e "$b" ] || continue
    [ "$(cat "$b/dev" 2>/dev/null)" = "$raiz_mm" ] || continue
    # Se for partição, o disco é o diretório pai no sysfs.
    disco_da_raiz=$(basename "$(dirname "$(readlink -f "$b")")")
    break
  done
fi

# ── candidatos ─────────────────────────────────────────────────────────
#
# Padrões sobrescrevíveis **só para teste**: no PC não há root para `losetup`, e
# no aparelho não há como inventar um segundo slot. Com `CARTOES_PADROES`
# aponta-se para dispositivos de loop e o caminho de dois cartões é exercitado
# de verdade, em vez de afirmado.
PADROES="${CARTOES_PADROES:-/sys/class/block/*}"
# Com override explícito, `loop` é aceito: é como o teste simula dois cartões.
case "${CARTOES_PADROES:-}" in
  "") SOMENTE_REAIS=1 ;;
  *)  SOMENTE_REAIS=0 ;;
esac

n=0
# shellcheck disable=SC2086
for disco in $PADROES; do
  [ -e "$disco" ] || continue
  nome=$(basename "$disco")

  # Partição não é disco: ela entra pelo laço do disco dela.
  [ -f "$disco/partition" ] && continue

  if [ "$SOMENTE_REAIS" = "1" ]; then
    case "$nome" in
      loop*|ram*|zram*) continue ;;
    esac
  fi
  case "$nome" in
    *boot*|*rpmb*) continue ;;
  esac

  # O disco do sistema está fora — as duas partições dele.
  [ "$nome" = "$disco_da_raiz" ] && continue

  tam=$(cat "$disco/size" 2>/dev/null || echo 0)
  [ "$tam" -gt 0 ] 2>/dev/null || continue

  # Partições deste disco. Sem nenhuma, o próprio disco pode ser o sistema de
  # arquivos (cartão formatado sem tabela de partições).
  particoes=""
  for p in "$disco"p* "$disco"[0-9]*; do
    [ -e "$p" ] || continue
    # Só partições de verdade.
    [ -f "$p/partition" ] || continue
    particoes="$particoes $(basename "$p")"
  done
  [ -n "$particoes" ] || particoes="$nome"

  for pn in $particoes; do
    dev="/dev/$pn"
    [ -b "$dev" ] || continue

    # Já montado em qualquer lugar? Usa o ponto que existe — montar de novo
    # criaria uma segunda visão do mesmo cartão.
    ponto_existente=$(awk -v d="$dev" '$1 == d { print $2; exit }' /proc/mounts)
    if [ -n "$ponto_existente" ]; then
      echo "PONTO=$ponto_existente"
      continue
    fi

    # Só monta o que tem sistema de arquivos reconhecível: montar às cegas uma
    # partição de firmware produz erro no console a cada boot.
    tipo=$(blkid -s TYPE -o value "$dev" 2>/dev/null)
    [ -n "$tipo" ] || continue

    n=$((n + 1))
    ponto="${BASE}${n}"
    mkdir -p "$ponto" 2>/dev/null || continue
    # `-o ro`: o app não tem o que escrever no cartão, e montar rw um FAT32 com o
    # sistema rodando do mesmo cartão é risco sem retorno.
    if mount -o ro "$dev" "$ponto" 2>/dev/null; then
      echo "PONTO=$ponto"
    else
      rmdir "$ponto" 2>/dev/null
      n=$((n - 1))
    fi
  done
done

exit 0
