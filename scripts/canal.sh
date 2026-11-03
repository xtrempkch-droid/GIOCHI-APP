#!/usr/bin/env bash
# canal.sh — a janela de contato com a outra equipe esta aberta?
#
#   bash scripts/canal.sh            # diz se esta aberto e quanto falta
#   bash scripts/canal.sh --exigir   # sai com 1 se estiver fechado
#
# ## Por que existe
#
# A janela (08:00–18:50, horario de Brasilia) esta escrita em tres lugares, e
# texto em tres lugares nao impede ninguem de responder as 20h. Uma checagem de
# um segundo e o que transforma a regra em algo que se cumpre.
#
# ## Fuso
#
# Fixo em America/Sao_Paulo, nao no fuso da maquina. `charles-X99-V9S` esta em
# -03 e hoje coincide, mas maquina trocada ou container em UTC fariam a conta
# sair errada sem aviso — e o erro seria sempre para o lado de "parece aberto".
set -uo pipefail

ABRE=800     # 08:00
FECHA=1850   # 18:50
FUSO="America/Sao_Paulo"

agora=$(TZ="$FUSO" date +%H%M)
# `10#` forca base 10: sem isso, "0800" seria lido como octal e 08:00 viraria
# erro de sintaxe em vez de horario.
agora_n=$((10#$agora))

if [ "$agora_n" -ge "$ABRE" ] && [ "$agora_n" -lt "$FECHA" ]; then
  restam=$((FECHA - agora_n))
  printf 'canal ABERTO   %s (Brasilia)   fecha em %d:%02d\n' \
    "$(TZ="$FUSO" date '+%H:%M')" "$((restam / 100))" "$((restam % 100))"
  exit 0
fi

if [ "$agora_n" -lt "$ABRE" ]; then
  faltam=$((ABRE - agora_n))
  printf 'canal FECHADO  %s (Brasilia)   abre em %d:%02d\n' \
    "$(TZ="$FUSO" date '+%H:%M')" "$((faltam / 100))" "$((faltam % 100))"
else
  printf 'canal FECHADO  %s (Brasilia)   janela era 08:00-18:50\n' \
    "$(TZ="$FUSO" date '+%H:%M')"
fi
printf '  mensagem que chegar agora fica para o proximo dia util.\n'

if [ "${1:-}" = "--exigir" ]; then
  exit 1
fi
