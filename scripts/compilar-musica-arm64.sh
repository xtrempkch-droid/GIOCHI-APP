#!/usr/bin/env bash
# Compila o app de música para o aparelho (aarch64) e, opcionalmente, instala.
#
# Uso:
#   bash scripts/compilar-musica-arm64.sh
#   bash scripts/compilar-musica-arm64.sh --instalar
#   bash scripts/compilar-musica-arm64.sh --instalar root@192.168.1.4
#
# ## Por que este script **consome** o cache do launcher em vez de montá-lo
#
# Montar o sysroot aarch64 custa quatro armadilhas já resolvidas em
# `compilar-launcher-arm64.sh`: o `rust-lld` não é driver de compilador (não põe
# `crt1.o` nem `--dynamic-linker`), o `-L` de x86-64 faz o linker escolher a
# emulação do **host**, o glibc do aparelho é 2.38 e não o do Ubuntu, e as
# bibliotecas do SDL2 têm de vir do **alvo real**. Escrever uma segunda cópia
# disso aqui faria as duas envelhecerem diferente — a primeira a receber uma
# correção de cross-compile seria a única corrigida.
#
# Então: a receita mora no script do launcher, e este aqui usa o resultado
# (`$GIOCHI_ARM64_CACHE`). A única coisa própria é o que muda de app para app —
# o crate, o nome do binário e o destino.
set -euo pipefail

# `set -e` mata o script **sem imprimir nada** quando um comando falha dentro de
# `$(...)`: medido aqui, `resolver_aparelho` saía com código 1 e zero linhas de
# saída — indistinguível de um build que não rodou (§13 do AGENTS.md). O trap
# garante que a última palavra seja uma frase com a linha.
trap 'printf "\033[31merro:\033[0m linha %s falhou: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/musica"
ALVO="aarch64-unknown-linux-gnu"
CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SLUG="musica"
BIN_NAME="giochi-musica"

log() { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

HOST_SSH="${GIOCHI_DEVICE:-}"
INSTALAR=""
PEDIU_INSTALAR=""
while [ $# -gt 0 ]; do
  case "$1" in
    --instalar)
      PEDIU_INSTALAR="sim"
      if [ $# -ge 2 ] && [ "${2#-}" = "$2" ]; then
        INSTALAR="${2#root@}"; shift 2
      else
        shift
      fi
      ;;
    --host) [ $# -ge 2 ] || erro "--host precisa de root@ip"
      HOST_SSH="$2"; shift 2 ;;
    -h|--ajuda) sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) erro "argumento desconhecido: $1" ;;
  esac
done

# Uma função decide o alvo, e todo ssh/scp a consulta: `INSTALAR` é bare em
# `--instalar <ip>` e `HOST_SSH` já vem com `root@`. A vez em que os dois
# significados conviveram no mesmo arquivo, o resultado foi
# `root@root@10.55.0.2: Permission denied` — mensagem que manda procurar chave
# quando o errado era o alvo.
alvo_ssh() {
  case "$INSTALAR" in
    *@*) printf '%s' "$INSTALAR" ;;
    *)   printf 'root@%s' "$INSTALAR" ;;
  esac
}

# Descobre o aparelho reusando `aparelho.sh procurar` — o IP muda a cada boot.
# Fixar um padrão daria a impressão de que o script "sabe" o endereço, e a
# primeira inicialização depois de reiniciar falharia com erro de rede.
resolver_aparelho() {
  if [ -n "$HOST_SSH" ] && [ "${HOST_SSH#*@}" = "$HOST_SSH" ]; then
    HOST_SSH="root@$HOST_SSH"
  fi
  [ -n "$HOST_SSH" ] && return 0
  local saida ip
  # `|| true` é obrigatório: o `procurar` sai com 1 quando acha na rede um host
  # que responde SSH e recusa autenticação (aconteceu com o roteador em
  # 192.168.1.1) — e sob `set -e` isso matava o script **antes** da mensagem.
  # Falhar por causa de um vizinho de rede é aceitável; falhar calado, não.
  saida=$(bash "$RAIZ/scripts/aparelho.sh" procurar 2>/dev/null || true)
  # Só a linha de resultado tem a forma `<ip> <hostname>`; o cabeçalho do
  # `procurar` cita `192.168.1.0/24`, e um `grep` por IP sozinho pegava esse
  # endereço — que não é o aparelho.
  ip=$(printf '%s\n' "$saida" |
    grep -oE '^[[:space:]]*([0-9]{1,3}\.){3}[0-9]{1,3}[[:space:]]+[^[:space:]]+' |
    head -1 | awk '{print $1}' || true)
  [ -n "$ip" ] || erro "aparelho não encontrado na rede. Use --instalar root@<ip> ou GIOCHI_DEVICE=root@<ip>
  (se o aparelho responde por USB: GIOCHI_DEVICE=root@10.55.0.2)"
  HOST_SSH="root@$ip"
  log "aparelho em $HOST_SSH"
}

command -v cargo >/dev/null || erro "cargo não encontrado (https://rustup.rs)"

[ -f "$CACHE/cc-aarch64.sh" ] && [ -f "$CACHE/pkgconfig/sdl2.pc" ] || erro "cache de cross-compile ausente em $CACHE
  monte-o uma vez com o script do launcher (é ele que copia as bibliotecas do
  aparelho e escreve o wrapper do linker):
    bash scripts/compilar-launcher-arm64.sh --host root@<ip>"

if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO"; then
  log "instalando target $ALVO"
  rustup target add "$ALVO"
fi

# `PKG_CONFIG_ALLOW_CROSS` porque o pkg-config recusa responder sobre `.pc` de
# outra arquitetura sem ele; `PKG_CONFIG_PATH` porque os `.pc` do cache apontam
# para as bibliotecas copiadas do aparelho.
export GIOCHI_ARM64_SYSROOT="$CACHE/sysroot"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH="$CACHE/pkgconfig"
export RUSTFLAGS="-C linker=$CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$CACHE/sysroot \
-C link-arg=-L$CACHE/devlibs \
-C link-arg=-L$CACHE/sysroot/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$CACHE/sysroot/lib \
-C link-arg=-L$CACHE/sysroot/usr/lib"

log "compilando $BIN_NAME para $ALVO"
( cd "$APP" && cargo build --release --target "$ALVO" --offline )

BIN="$APP/target/$ALVO/release/$BIN_NAME"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

# `strip` do host não entende ELF aarch64 e falha em silêncio quando o
# resultado é redirecionado: o arquivo sai com os símbolos e 400 KB a mais.
STRIP=""
for candidato in aarch64-linux-gnu-strip llvm-strip; do
  if command -v "$candidato" >/dev/null; then STRIP="$candidato"; break; fi
done
mkdir -p "$APP/bin"
if [ -n "$STRIP" ]; then
  "$STRIP" --strip-all "$BIN" -o "$APP/bin/$BIN_NAME"
else
  log "aviso: sem strip para aarch64; o binário em bin/ vai com símbolos"
  cp "$BIN" "$APP/bin/$BIN_NAME"
fi
chmod 755 "$APP/bin/$BIN_NAME"

log "pronto: $APP/bin/$BIN_NAME ($(du -h "$APP/bin/$BIN_NAME" | cut -f1))"
echo "  NEEDED: $(readelf -d "$APP/bin/$BIN_NAME" | sed -n 's/.*\[\(lib[^]]*\)\].*/\1/p' | tr '\n' ' ')"

# ── instalar no aparelho ────────────────────────────────────
if [ -n "$PEDIU_INSTALAR" ] && [ -z "$INSTALAR" ]; then
  resolver_aparelho
  INSTALAR="$HOST_SSH"
fi

if [ -n "$INSTALAR" ]; then
  DESTINO="/opt/giochi/apps/$SLUG/bin"
  ALVO_SSH="$(alvo_ssh)"
  filtrar_ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

  # A pergunta que decide se o app **abre** — e não se ele instala.
  # `SDL2_mixer` é dependência de link desde que o app toca música: sem ela o
  # processo morre no carregador, com uma mensagem que não diz o que instalar.
  # O launcher nunca precisou dela, então a imagem de um aparelho novo pode não
  # tê-la sem que nada avise.
  #
  # `test -e` em cada caminho, e não `ls A B`: com dois argumentos o `ls` falha
  # se **qualquer um** faltar, então um `/lib` sem o arquivo (Buildroot não tem
  # `/lib -> /usr/lib`) fazia a conferência acusar as quatro bibliotecas como
  # ausentes num aparelho que as tinha — o oposto do erro certo.
  FALTA=$(ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=8 \
    "$ALVO_SSH" 'for l in libSDL2-2.0.so.0 libSDL2_ttf-2.0.so.0 libSDL2_mixer-2.0.so.0 libSDL2_image-2.0.so.0; do
      if [ -e /usr/lib/$l ] || [ -e /lib/$l ]; then :; else echo $l; fi
    done' 2>/dev/null | filtrar_ruido || true)
  if [ -n "$FALTA" ]; then
    log "ATENÇÃO: falta no aparelho: $(echo "$FALTA" | tr '\n' ' ')"
    echo "         sem essas bibliotecas o app não abre (falha de carga, não de"
    echo "         configuração). É pedido para a imagem do Giochi System."
  else
    log "bibliotecas do aparelho: SDL2, SDL2_ttf, SDL2_mixer e SDL2_image presentes"
  fi

  log "instalando em $ALVO_SSH:$DESTINO"

  # O `metadata.json` **vai junto**, e isso não é detalhe de arrumação: é ele que
  # carrega o `kind` (a aba) e o título. Medido no R35S em 15/11/2026: a versão
  # anterior deste script copiava só o binário, a pasta do app ficou sem
  # `metadata.json`, e a detecção do launcher não tinha como saber que ali havia
  # um **app** — ele aparecia como jogo, no carrossel da Home.
  #
  # O launcher é tema e não precisa (a poda o descarta pelo prefixo); um app
  # precisa: sem identidade no disco, ele só existe no índice, e o índice é
  # justamente o que a detecção veio tornar dispensável.
  RAIZ_APP="$(dirname "$DESTINO")"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$ALVO_SSH" "mkdir -p $DESTINO" 2>&1 | filtrar_ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$APP/bin/$BIN_NAME" "$ALVO_SSH:$DESTINO/$BIN_NAME" 2>&1 | filtrar_ruido
  for arquivo in metadata.json capa.png; do
    [ -f "$APP/$arquivo" ] || continue
    scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
      "$APP/$arquivo" "$ALVO_SSH:$RAIZ_APP/$arquivo" 2>&1 | filtrar_ruido
  done
  # `chmod` explícito: o umask do aparelho é 077 e o `scp` criaria o arquivo
  # `700`. O mesmo binário empacotado pelo `empacotar.mjs` chega `755` (o tar
  # escreve modo) — duas origens, duas permissões. Ver §13 do AGENTS.md.
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$ALVO_SSH" \
    "chmod 755 $DESTINO/$BIN_NAME && chmod 644 $RAIZ_APP/metadata.json 2>/dev/null; \
     ls -la $DESTINO/$BIN_NAME $RAIZ_APP/metadata.json" 2>&1 | filtrar_ruido

  echo
  echo "próximos passos:"
  echo "  bash scripts/aparelho.sh catalogo            # publica o cartão"
  echo "  ssh $( [ -n "$ALVO_SSH" ] && echo "$ALVO_SSH" ) $DESTINO/$BIN_NAME --ajuda"
fi
