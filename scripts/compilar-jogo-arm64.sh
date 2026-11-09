#!/usr/bin/env bash
# Compila o jogo (`app/jogos/volley`) para o aparelho (aarch64) e, opcionalmente,
# instala em `/opt/giochi/apps/volley/bin`.
#
# Uso:
#   bash scripts/compilar-jogo-arm64.sh                       # compila o volley
#   bash scripts/compilar-jogo-arm64.sh --jogo pacman         # outro jogo
#   bash scripts/compilar-jogo-arm64.sh --jogo cobrinha --instalar 10.55.0.2
#
# ## Por que um script separado, e não uma opção no do launcher
#
# O launcher e o jogo são apps diferentes, em projetos Cargo diferentes. O que
# os dois compartilham são as cinco armadilhas de link do aarch64 — sysroot,
# `--dynamic-linker`, glibc do aparelho, wrapper de `rust-lld` e libSDL2 —, e
# todas elas já estão resolvidas em `~/.cache/giochi-arm64` pelo
# `compilar-launcher-arm64.sh`.
#
# Então este script **não redescobre nada**: ele exige que aquele cache já
# exista e reusa o wrapper e as bibliotecas. Duplicar a lógica de download do
# sysroot significaria dois lugares para envelhecer, e a lição deste repositório
# é que o segundo lugar sempre envelhece diferente.
#
# Diferença importante: o jogo **não** usa `SDL2_ttf` nem `libasound`. Ele
# desenha o placar em sete segmentos de propósito, para depender de uma
# biblioteca só.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Padrão: o primeiro jogo, que é o que existe há mais tempo. `--jogo` escolhe
# outro. Um script por jogo seria três scripts para o mesmo link.
NOME_JOGO="volley"
ALVO="aarch64-unknown-linux-gnu"
CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SYSROOT="$CACHE/sysroot"
LIBDIR="$CACHE/devlibs"
INSTALAR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --jogo) NOME_JOGO="${2:-}"; shift 2 ;;
    --jogo=*) NOME_JOGO="${1#*=}"; shift ;;
    --instalar) INSTALAR="${2:-}"; shift 2 ;;
    --instalar=*) INSTALAR="${1#*=}"; shift ;;
    -h|--ajuda)
      sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "opção desconhecida: $1" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()  { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
erro(){ printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

JOGO="$RAIZ/app/jogos/$NOME_JOGO"
# A convenção é pasta `<slug>` → binário `giochi-<slug>`. Mas `giochi-defender`
# já nasce com o prefixo, e a regra cega produz `giochi-giochi-defender` — um
# nome que não existe em `target/`, com erro de "binário não gerado" que manda
# procurar defeito no compilador. Pasta que já começa com `giochi-` usa o nome
# dela mesma.
case "$NOME_JOGO" in
  giochi-*) BINARIO="$NOME_JOGO" ;;
  *)        BINARIO="giochi-$NOME_JOGO" ;;
esac
if [ ! -f "$JOGO/Cargo.toml" ]; then
  disponiveis="$(find "$RAIZ/app/jogos" -mindepth 1 -maxdepth 1 -type d -printf '%f ' 2>/dev/null)"
  erro "jogo desconhecido: $NOME_JOGO (esperado em app/jogos/<nome>)
  em app/jogos/ existe: ${disponiveis:-nada}
  use: --jogo <nome>"
fi

# ── 1. o cache do cross-compile precisa existir ─────────────
# Sem ele não há como linkar: `rust-lld` sozinho produz um ELF sem `PT_INTERP`
# e o kernel mata o processo com segfault — `file` diz "valid ELF" e não ajuda.
if [ ! -x "$CACHE/cc-aarch64.sh" ] || [ ! -f "$LIBDIR/libSDL2.so" ]; then
  erro "cache aarch64 incompleto em $CACHE
  rode primeiro: bash scripts/compilar-launcher-arm64.sh
  (ele baixa o sysroot, copia a glibc/SDL2 do aparelho e escreve o wrapper)"
fi
ok "cache aarch64 em $CACHE"

if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO"; then
  erro "alvo $ALVO não instalado: rustup target add $ALVO"
fi
ok "alvo $ALVO presente"

# ── 2. codegen para a CPU do console (fase 18 da engine) ────
# `-C target-cpu=native` não serve num cross-compile: o compilador roda no
# **host**, então "native" descreveria a máquina errada. O core tem de ser
# nomeado.
#
# ## Medido: este jogo **não ganha**, e por isso o padrão é desligado
#
# O engine mediu o phase-18 e concluiu "tamanho, não velocidade" — lá o ganho de
# tamanho vinha de `strip` e LTO, que este jogo **já tinha** (`lto = true` é fat,
# `strip = true`, `codegen-units = 1`). O que sobra para medir aqui é só o
# `target-cpu`, e no X35H ele custou sem pagar: 5 corridas intercaladas de 400
# quadros deram mediana **37,0 fps sem** (36,7/36,8/37,0/37,4/37,5) contra
# **36,8 com** (36,4/36,7/36,8/36,8/36,8); e o binário ficou 3,9 % **maior**
# (675 584 → 701 712 B). Mais lento dentro do ruído, maior de verdade e sem
# ganho nenhum: o padrão fica sem `target-cpu`.
#
# O mecanismo continua aqui porque o dia em que a engine ou o rustc mudarem a
# qualidade de codegen para ARM o número muda, e porque build de aparelho único
# pode querer o core exato.
#
# ## Se você ligar, o valor seguro é `cortex-a35`
#
# O mesmo `bin/giochi-neon-drift` é instalado no R35S (A35), no X35H e no X55
# (A55). Medido com `rustc --print cfg`:
#
#   -C target-cpu=cortex-a35  → aes, crc, neon, sha2
#   -C target-cpu=cortex-a55  → aes, crc, neon, sha2, **lse**
#
# `lse` são as atômicas ARMv8.1 (`ldadd`/`swpal`); o A35 é ARMv8.0-A e não
# decodifica nenhuma delas — o binário morre com SIGILL na primeira atômica, que
# num jogo é um `Arc::clone` do primeiro quadro. O engine mediu a mesma coisa
# (docs/PERF_LOG.md): lá o plano pedia `+lse` e foi recusado pelo mesmo motivo.
#
# `GIOCHI_TARGET_CPU` troca o alvo para builds de aparelho único, sabendo que o
# binário deixa de rodar nos aparelhos mais antigos.
# Vazio = sem `-C target-cpu` (o padrão medido). `GIOCHI_TARGET_CPU=cortex-a35`
# liga o alvo conservador para toda a frota.
GIOCHI_TARGET_CPU="${GIOCHI_TARGET_CPU:-}"
case "$GIOCHI_TARGET_CPU" in
  "") GIOCHI_TARGET_FEATURES="${GIOCHI_TARGET_FEATURES:-}" ;;
  cortex-a35|cortex-a53|cortex-a55|cortex-a72|cortex-a76|cortex-a78)
    # `+crc,+aes,+sha2` já vêm implícitos no `-C target-cpu=cortex-a35` deste
    # rustc (conferido com `--print cfg`); declarados mesmo assim porque um
    # `target-cpu` diferente, ou um rustc que mude a tabela, deixaria de os
    # ligar em silêncio. São instruções que os três aparelhos declaram no
    # `/proc/cpuinfo` e que o codegen do jogo não usa — ficam ligadas porque
    # descrevem a peça, não porque pagam.
    GIOCHI_TARGET_FEATURES="${GIOCHI_TARGET_FEATURES:-+crc,+aes,+sha2}" ;;
  *)
    # CPU desconhecida não ganha feature nenhuma: `+feature` errada é SIGILL num
    # aparelho que ninguém está olhando; otimização faltando é só mais lento.
    GIOCHI_TARGET_FEATURES="${GIOCHI_TARGET_FEATURES:-}" ;;
esac
if printf '%s' "$GIOCHI_TARGET_FEATURES" | grep -q 'lse'; then
  erro "+lse não pode ser ligado: o R35S é Cortex-A35 (ARMv8.0-A) e não
  decodifica as atômicas ARMv8.1 — o binário morreria com SIGILL no primeiro
  quadro. Use GIOCHI_TARGET_CPU=cortex-a55 só para build de um aparelho único."
fi

# ── 3. compilar ─────────────────────────────────────────────
# Os mesmos `-L` do launcher. `sdl2-sys` acha os **cabeçalhos** pelo pkg-config
# do host (cabeçalho não tem arquitetura); quem precisa ser aarch64 é a
# biblioteca, e é isso que estes `-L` garantem — `$LIBDIR` vem primeiro.
#
# A CPU entra **aqui** e não num `.cargo/config.toml`: cargo ignora a tabela
# `rustflags` do config por completo quando `RUSTFLAGS` está definido no
# ambiente, e este script o define para pôr o sysroot na linha de link. Foi
# exatamente assim que a fase 18 da engine passou um tempo compilando para
# ARMv8-A genérico com o config "certo", sem nada falhar.
export GIOCHI_ARM64_SYSROOT="$SYSROOT"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
FLAGS_CPU=""
[ -n "$GIOCHI_TARGET_CPU" ] && FLAGS_CPU="-C target-cpu=$GIOCHI_TARGET_CPU"
[ -n "$GIOCHI_TARGET_FEATURES" ] && FLAGS_CPU="$FLAGS_CPU -C target-feature=$GIOCHI_TARGET_FEATURES"
export RUSTFLAGS="$FLAGS_CPU \
-C linker=$CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$SYSROOT \
-C link-arg=-L$LIBDIR \
-C link-arg=-L$SYSROOT/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$SYSROOT/lib \
-C link-arg=-L$SYSROOT/usr/lib"

# Sem isto o crate `pkg-config` recusa trabalhar cross e o erro fala de
# "cross-compilation", não de SDL — exatamente o erro que a `neon-drift`
# produziu na primeira tentativa. Os `include`s vêm do `sdl2.pc` do host
# (cabeçalho não tem arquitetura); quem precisa ser aarch64 é a biblioteca, e
# `-L$LIBDIR` vem primeiro na linha de link.
export PKG_CONFIG_ALLOW_CROSS=1

# O `sdl2-sys` precisa de um `sdl2.pc` só para os `include`s. Se o host não tem,
# o erro é `SDL2 headers not found`, que não diz que o problema é de host.
if [ -n "${GIOCHI_SDL2_PREFIX:-}" ] || [ -d "$HOME/.cache/giochi-sdl2/root" ]; then
  PREFIXO_SDL="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}"
  export PKG_CONFIG_PATH="$PREFIXO_SDL/usr/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
fi

cd "$JOGO"
echo "  cpu: ${GIOCHI_TARGET_CPU:-(sem target-cpu: ARMv8-A genérico — foi o mais rápido medido)}"
echo "  compilando $BINARIO para $ALVO..."
cargo build --release --target "$ALVO" --offline 2>&1 | grep -vE "^\s*(Compiling|Finished|Downloading)" || true

BIN="$JOGO/target/$ALVO/release/$BINARIO"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

# Sem `PT_INTERP` o kernel mata o processo na hora, e `file` ainda diz que o ELF
# é válido. Conferir aqui custa um `readelf` e economiza um cartão regravado.
if command -v readelf >/dev/null 2>&1; then
  if ! readelf -l "$BIN" 2>/dev/null | grep -q "INTERP"; then
    erro "$BIN não tem PT_INTERP — o kernel daria segfault. Wrapper de link errado?"
  fi
  ok "PT_INTERP presente"
fi

mkdir -p "$JOGO/bin"
cp "$BIN" "$JOGO/bin/$BINARIO"
chmod +x "$JOGO/bin/$BINARIO"
ok "pronto: $JOGO/bin/$BINARIO ($(du -h "$BIN" | cut -f1))"

# ── 3. instalar ─────────────────────────────────────────────
if [ -n "$INSTALAR" ]; then
  HOST="${INSTALAR#root@}"
  DESTINO="/opt/giochi/apps/$NOME_JOGO/bin"
  filtrar_ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

  echo "  instalando em root@$HOST:$DESTINO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" "mkdir -p $DESTINO" 2>&1 | filtrar_ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$JOGO/bin/$BINARIO" "root@$HOST:$DESTINO/$BINARIO" 2>&1 | filtrar_ruido
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" \
    "chmod 755 $DESTINO/$BINARIO; sha256sum $DESTINO/$BINARIO" 2>&1 | filtrar_ruido

  LOCAL=$(sha256sum "$JOGO/bin/$BINARIO" | cut -d' ' -f1)
  REMOTO=$(ssh -o BatchMode=yes -o StrictHostKeyChecking=no "root@$HOST" \
    "sha256sum $DESTINO/$BINARIO 2>/dev/null | cut -d' ' -f1" 2>/dev/null | filtrar_ruido)
  if [ "$LOCAL" = "$REMOTO" ]; then
    ok "sha256 idêntico (${LOCAL:0:16}…)"
  else
    erro "sha256 diferente: local ${LOCAL:0:16}… dispositivo ${REMOTO:0:16}…"
  fi
fi
