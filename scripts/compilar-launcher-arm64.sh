#!/usr/bin/env bash
# Compila o launcher para o aparelho (aarch64) e, opcionalmente, instala.
#
# Uso:
#   bash scripts/compilar-launcher-arm64.sh                 # só compila
#   bash scripts/compilar-launcher-arm64.sh --instalar root@192.168.1.4
#   bash scripts/compilar-launcher-arm64.sh --host root@192.168.1.4
#   bash scripts/compilar-launcher-arm64.sh --sem-texto   # força build sem glifos
#   bash scripts/compilar-launcher-arm64.sh --app launcher-star-dust
#
# `--app` existe porque o Star Dust (o launcher com a camada de efeitos) é o
# **mesmo crate com outro nome de binário**: mesma árvore de dependências, mesmo
# sysroot, mesmo cache. Duplicar o script faria as duas cópias envelhecerem
# diferente — a primeira a receber uma correção de cross-compile seria a única
# corrigida. O nome do binário e o destino vêm do `Cargo.toml` e do
# `metadata.json` do app, não de uma tabela paralela.
#
# Sem --host/--instalar, o aparelho é descoberto por `aparelho.sh procurar`
# (o IP muda a cada boot).
#
# ## Por que este script existe
#
# Não há toolchain aarch64 pronto para este código. O `cargo` compila; quem
# falha é o **link**, por quatro motivos que este script resolve:
#
#  1. Falta sysroot aarch64 com glibc e crt1.o → baixa `libc6-*-arm64-cross`.
#  2. `rust-lld` não é driver de compilador: não adiciona crt1/crti/crtn nem
#     `--dynamic-linker`. Sem o segundo, o ELF sai sem PT_INTERP e o kernel dá
#     segfault imediato. → wrapper `cc-aarch64.sh`.
#  3. `libSDL2` e `libc` do aparelho são glibc **2.38**; o sysroot do Ubuntu é
#     2.35, e o link recusa símbolos (`strlcat@GLIBC_2.38`). → copia libc/libm/
#     libgcc_s/libSDL2 **do próprio aparelho**, que é o alvo real.
#  4. A feature `texto` depende de `libSDL2_ttf` NO APARELHO. O script pergunta
#     ao aparelho e compila com ou sem ela; presumir custa uma rodada, porque o
#     binário sem texto roda igual no aparelho que tem a biblioteca — só que sem
#     rótulo nenhum, sem erro. `--com-texto` / `--sem-texto` sobrepõem.
#  5. `libasound` **não** é copiada e não é exigida no link: o mixer é resolvido
#     com `dlopen` (ver `app/launcher/crates/ui-sdl/src/volume.rs`), então o
#     mesmo binário serve no aparelho com a biblioteca e no que não tem — lá o
#     volume mostra `—` e o resto funciona. Copiá-la do aparelho só para linkar
#     foi o que quebrou o R35S, que não a tem.
#
# Requisitos: `ssh`/`scp` para o aparelho, `curl`, `dpkg-deb`, `rustup`.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_RELATIVO="app/launcher"
ALVO="aarch64-unknown-linux-gnu"
RAIZ_CACHE="${GIOCHI_ARM64_CACHE:-$HOME/.cache/giochi-arm64}"
SYSROOT="$RAIZ_CACHE/sysroot"
DEVS="$RAIZ_CACHE/debs"
LIBDIR="$RAIZ_CACHE/devlibs"
# O IP do aparelho muda a cada boot (DHCP). Sem `--instalar` explícito o script
# tenta descobrir; se falhar, ele diz como apontar.
HOST_SSH="${GIOCHI_DEVICE:-}"
BASE_URL="https://archive.ubuntu.com/ubuntu/pool/main/c/cross-toolchain-base"
LIB_DEB="libc6-dev-arm64-cross_2.35-0ubuntu1cross3_all.deb"
RUN_DEB="libc6-arm64-cross_2.35-0ubuntu1cross3_all.deb"

log()  { printf '\033[1m%s\033[0m\n' "$*"; }
erro() { printf '\033[31merro:\033[0m %s\n' "$*" >&2; exit 1; }

# Descobre o aparelho reusando `aparelho.sh procurar` em vez de duplicar a
# varredura da /24. O endereço muda a cada boot, então fixar um padrão daria a
# impressão de que o script "sabe" o IP — e a primeira inicialização após
# reiniciar o aparelho falharia com uma mensagem de rede, não de configuração.
resolver_aparelho() {
  # `GIOCHI_DEVICE` pode vir só com o IP (`aparelho.sh instalar` passa assim).
  # Sem o `root@`, o ssh tenta o usuário local, falha, e a detecção de texto
  # conclui "aparelho sem SDL2_ttf" — o build sai sem glifos e nada avisa.
  # Normalizar aqui evita depender de quem chama.
  if [ -n "$HOST_SSH" ] && [ "${HOST_SSH#*@}" = "$HOST_SSH" ]; then
    HOST_SSH="root@$HOST_SSH"
  fi
  [ -n "$HOST_SSH" ] && return 0
  local ip
  # Só a linha de resultado tem a forma `<ip> <hostname>`. A linha de cabeçalho
  # do `procurar` contém `192.168.1.0/24`, e um `grep` por IP sozinho pegava
  # aquele endereço — que não é o aparelho.
  ip=$(bash "$RAIZ/scripts/aparelho.sh" procurar 2>/dev/null |
    grep -oE '^[[:space:]]*([0-9]{1,3}\.){3}[0-9]{1,3}[[:space:]]+[^[:space:]]+' |
    head -1 | awk '{print $1}')
  [ -n "$ip" ] || erro "aparelho não encontrado na rede. Use --host root@<ip> ou GIOCHI_DEVICE=root@<ip>"
  HOST_SSH="root@$ip"
  log "aparelho em $HOST_SSH"
}

INSTALAR=""
PEDIU_INSTALAR=""
FORCAR_TEXTO=""
while [ $# -gt 0 ]; do
  case "$1" in
    --app) [ $# -ge 2 ] || erro "--app precisa do caminho do app"
      APP_RELATIVO="$2"; shift 2 ;;
    # O destino é **opcional**: `--instalar` sozinho descobre o aparelho, que é
    # o caso normal (o IP muda a cada boot). A primeira versão fazia
    # `INSTALAR="${2:-}"; shift 2` e, chamada só com `--instalar`, o `shift 2`
    # falhava com um argumento só — e `set -e` matava o script **sem imprimir
    # nada**, com código 1. Um script de build que sai calado é indistinguível
    # de um build que não rodou.
    --instalar)
      PEDIU_INSTALAR="sim"
      if [ $# -ge 2 ] && [ "${2#-}" = "$2" ]; then
        INSTALAR="$2"; shift 2
      else
        shift
      fi
      ;;
    --com-texto) FORCAR_TEXTO="sim"; shift ;;
    --sem-texto) FORCAR_TEXTO="nao"; shift ;;
    --host)
      [ $# -ge 2 ] || erro "--host precisa de root@ip"
      HOST_SSH="$2"; shift 2 ;;
    *) erro "argumento desconhecido: $1" ;;
  esac
done

# Aceita as três formas: absoluto, `app/<nome>` e o nome solto. O nome solto é
# o que a documentação usa (`--app launcher-star-dust`), e obrigar a escrever
# `app/` seria uma armadilha de digitação sem ganho nenhum.
case "$APP_RELATIVO" in
  /*) LAUNCHER="$APP_RELATIVO" ;;
  *)
    if [ -d "$RAIZ/$APP_RELATIVO" ]; then
      LAUNCHER="$RAIZ/$APP_RELATIVO"
    else
      LAUNCHER="$RAIZ/app/$APP_RELATIVO"
    fi
    ;;
esac
[ -f "$LAUNCHER/crates/ui-sdl/Cargo.toml" ] \
  || erro "$LAUNCHER não parece um launcher (sem crates/ui-sdl/Cargo.toml)"

# Nome do binário e slug saem do próprio app. Fixá-los aqui criaria uma segunda
# verdade: renomear o crate e esquecer o script instala o arquivo com o nome
# antigo, e o `giochi-init` procura o novo.
BIN_NAME=$(sed -n 's/^name = "\(giochi-launcher[^"]*\)"/\1/p' \
  "$LAUNCHER/crates/ui-sdl/Cargo.toml" | tail -1)
[ -n "$BIN_NAME" ] || erro "não achei o nome do binário em $LAUNCHER/crates/ui-sdl/Cargo.toml"
SLUG=$(sed -n 's/.*"slug": *"\([^"]*\)".*/\1/p' "$LAUNCHER/metadata.json" 2>/dev/null | head -1)
[ -n "$SLUG" ] || SLUG=$(basename "$LAUNCHER")
log "app: $LAUNCHER — binário $BIN_NAME, slug $SLUG"

command -v cargo >/dev/null || erro "cargo não encontrado (https://rustup.rs)"
command -v dpkg-deb >/dev/null || erro "dpkg-deb não encontrado"

# ── 1. target Rust ──────────────────────────────────────────
if ! rustup target list --installed 2>/dev/null | grep -qx "$ALVO"; then
  log "instalando target $ALVO"
  rustup target add "$ALVO"
fi

# ── 2. sysroot com glibc ────────────────────────────────────
if [ ! -f "$SYSROOT/usr/aarch64-linux-gnu/lib/crt1.o" ]; then
  log "montando sysroot em $SYSROOT"
  mkdir -p "$DEVS" "$SYSROOT"
  for deb in "$LIB_DEB" "$RUN_DEB"; do
    if [ ! -f "$DEVS/$deb" ]; then
      log "  baixando $deb"
      curl -fsSL --max-time 180 -o "$DEVS/$deb" "$BASE_URL/$deb" \
        || erro "falha ao baixar $BASE_URL/$deb"
    fi
    dpkg-deb -x "$DEVS/$deb" "$SYSROOT" || erro "falha ao extrair $deb"
  done
fi

# ── 2b. o aparelho tem texto? (decide a feature `texto`) ────
#
# A pergunta é feita ao APARELHO, não ao PC: é lá que o binário vai rodar.
# Presumir custa uma rodada, e silenciosa — o binário sem texto roda normal no
# aparelho que tem a biblioteca, só que sem rótulo nenhum e sem erro.
#
# Precisa vir antes da cópia de libs: se o alvo tem texto, o link vai pedir
# `-lSDL2_ttf` e a biblioteca tem de estar no cache.
TEM_TEXTO=""
case "$FORCAR_TEXTO" in
  sim|nao) TEM_TEXTO="$FORCAR_TEXTO" ;;
esac
if [ -z "$TEM_TEXTO" ]; then
  resolver_aparelho
  if [ -n "$HOST_SSH" ] && ssh -o BatchMode=yes -o StrictHostKeyChecking=no \
      -o ConnectTimeout=8 "$HOST_SSH" \
      'ls /usr/lib/libSDL2_ttf.so* >/dev/null 2>&1' 2>/dev/null; then
    TEM_TEXTO="sim"
  else
    TEM_TEXTO="nao"
  fi
fi
TEM_TEXTO_PEDIDO="$TEM_TEXTO"

# ── 3b. ALSA (só informativo) ───────────────────────────────
#
# O mixer **não** é mais dependência de link: `volume.rs` resolve
# `libasound.so.2` com `dlopen` e, se não achar, o volume mostra `—` e o resto
# funciona. Não há feature nem cópia da biblioteca para o cache.
#
# A pergunta continua sendo feita porque "volume vai funcionar neste aparelho?"
# é a primeira coisa que alguém quer saber ao instalar, e a resposta só o
# aparelho tem. Medido no R35S (06/11/2026): tem `/dev/snd`, **não** tem a
# biblioteca — e o binário ligado contra ela nem abria.
TEM_SOM=""
if [ -z "$INSTALAR" ]; then
  resolver_aparelho
fi
# `--instalar` sem destino usa o aparelho descoberto — a forma que o
# `aparelho.sh instalar` chama e a que a documentação manda usar.
if [ -n "$PEDIU_INSTALAR" ] && [ -z "$INSTALAR" ]; then
  [ -n "$HOST_SSH" ] || erro "não achei o aparelho; use --instalar root@ip"
  INSTALAR="$HOST_SSH"
fi
if [ -n "$HOST_SSH" ] && ssh -o BatchMode=yes -o StrictHostKeyChecking=no \
    -o ConnectTimeout=8 "$HOST_SSH" \
    'ls /usr/lib/libasound.so* >/dev/null 2>&1 || ls /lib/libasound.so* >/dev/null 2>&1' 2>/dev/null; then
  TEM_SOM="sim"
else
  TEM_SOM="nao"
fi

# ── 3. bibliotecas do aparelho (glibc 2.38 + SDL2) ──────────
# Não dá para usar as do Ubuntu: o libSDL2 do aparelho pede símbolos
# `@GLIBC_2.38` que o sysroot 2.35 não tem, e o link falha.
precisa_do_aparelho() {
  # `libasound` não entra mais no cache: o link não a pede (ver 3b).
  [ -f "$LIBDIR/libSDL2.so" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/lib/libc.so.6" ] &&
    [ -f "$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so.6" ] &&
    # Sem o SDL2_ttf no cache o link com a feature `texto` falha, e o erro é
    # `unable to find library -lSDL2_ttf` — que não diz que a culpa é do cache.
    { [ -z "$TEM_TEXTO_PEDIDO" ] || [ -f "$LIBDIR/libSDL2_ttf.so" ]; }
}

if ! precisa_do_aparelho; then
  command -v scp >/dev/null || erro "scp necessário para copiar as libs do aparelho"
  resolver_aparelho
  log "copiando libs do aparelho ($HOST_SSH) — o alvo real define o glibc"
  mkdir -p "$LIBDIR"
  # Copiados pelos nomes NÃO versionados (os symlinks `libSDL2.so`,
  # `libasound.so`, `libSDL2_ttf.so`). Antes eram os arquivos reais com versão
  # fixa no script — `libSDL2-2.0.so.0.2800.5` —, e qualquer upgrade do SDL2 no
  # aparelho quebraria a cópia com erro que não falava de versão.
  # Um symlink que não exista no aparelho vira erro explícito abaixo.
  for par in \
    "/usr/lib/libSDL2.so:$LIBDIR/libSDL2.so" \
    "/usr/lib/libSDL2_ttf.so:$LIBDIR/libSDL2_ttf.so" \
    "/lib/libc.so.6:$SYSROOT/usr/aarch64-linux-gnu/lib/libc.so.6" \
    "/lib/libm.so.6:$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so.6" \
    "/lib/libgcc_s.so.1:$LIBDIR/libgcc_s.so.1" \
    "/lib/ld-linux-aarch64.so.1:$SYSROOT/usr/aarch64-linux-gnu/lib/ld-linux-aarch64.so.1"
  do
    origem="${par%%:*}"; destino="${par##*:}"
    if ! scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
        "$HOST_SSH:$origem" "$destino" 2>/dev/null; then
      # Mensagem com o caminho, porque o caso comum é o aparelho não ter o
      # symlink de desenvolvimento — e aí a resposta é instalar o pacote, não
      # mexer no script.
      erro "não copiei $origem do aparelho (o symlink existe lá? \`ls -l $origem\`)"
    fi
  done
  ln -sf libgcc_s.so.1 "$LIBDIR/libgcc_s.so"
  # O linker script do glibc aponta para caminhos absolutos do sysroot; o
  # symlink abaixo deixa `libm.so` resolver sem duplicar arquivo.
  ln -sf libm.so.6 "$SYSROOT/usr/aarch64-linux-gnu/lib/libm.so"
fi

# ── 4. wrapper de link ──────────────────────────────────────
cat > "$RAIZ_CACHE/cc-aarch64.sh" <<'WRAP'
#!/usr/bin/env bash
# Ver docs/compilar-launcher.md § aarch64 para o porquê de cada linha.
set -euo pipefail
SYSROOT="${GIOCHI_ARM64_SYSROOT:-$HOME/.cache/giochi-arm64/sysroot}"
CRT="$SYSROOT/usr/aarch64-linux-gnu/lib"
LOADER="${GIOCHI_ARM64_LOADER:-/lib/ld-linux-aarch64.so.1}"
args=()
pendente=""
for a in "$@"; do
  case "$pendente" in
    L)
      pendente=""
      case "$a" in
        *x86_64-linux-gnu*) continue ;;
      esac
      args+=("-L" "$a"); continue ;;
  esac
  case "$a" in
    -L)
      # `-L` de diretório x86-64 faz o `rust-lld` escolher a emulação do
      # **host**, e o link morre com
      #   Scrt1.o is incompatible with elf64-x86-64
      # mensagem que não fala de arquitetura de alvo em lugar nenhum. O rustc
      # passa esse caminho sozinho, em dois argumentos; num link aarch64 ele
      # não serve para nada.
      pendente="L" ;;
    -L/usr/lib/x86_64-linux-gnu*|-L/lib/x86_64-linux-gnu*) : ;;
    -Wl,*)
      IFS=',' read -r -a parts <<< "${a#-Wl,}"
      for parte in "${parts[@]}"; do
        case "$parte" in
          --fix-cortex-a53-843419) : ;;
          *) args+=("$parte") ;;
        esac
      done ;;
    -nodefaultlibs|-nostdlib|-B*) : ;;
    *) args+=("$a") ;;
  esac
done
exec rust-lld -flavor gnu -m aarch64linux --dynamic-linker "$LOADER" \
  "$CRT/Scrt1.o" "$CRT/crti.o" "${args[@]}" "$CRT/crtn.o"
WRAP
chmod +x "$RAIZ_CACHE/cc-aarch64.sh"

# ── 5. compilar ─────────────────────────────────────────────
export GIOCHI_ARM64_SYSROOT="$SYSROOT"
export GIOCHI_ARM64_LOADER="/lib/ld-linux-aarch64.so.1"
export RUSTFLAGS="-C linker=$RAIZ_CACHE/cc-aarch64.sh \
-C link-arg=--sysroot=$SYSROOT \
-C link-arg=-L$LIBDIR \
-C link-arg=-L$SYSROOT/usr/aarch64-linux-gnu/lib \
-C link-arg=-L$SYSROOT/lib \
-C link-arg=-L$SYSROOT/usr/lib"

# Só o `texto` é feature: `SDL2_ttf` é dependência de **link**, e sem ela o
# link falha. O mixer é `dlopen` e não entra nesta conta.
FEATURES="--no-default-features"
if [ "$TEM_TEXTO" = "sim" ]; then
  FEATURES="$FEATURES --features texto"
  log "features do aparelho: texto"
else
  log "aparelho SEM libSDL2_ttf — sem glifos"
  [ -z "$HOST_SSH" ] && log "  (aparelho não alcançado: build sem texto é o seguro)"
fi
if [ "$TEM_SOM" = "nao" ]; then
  log "  sem libasound: o volume mostra — (o binário roda igual)"
else
  log "  com libasound: controle de volume ativo"
fi

cd "$LAUNCHER"
# shellcheck disable=SC2086 # FEATURES é vazio ou uma flag; sem aspas de propósito.
cargo build -p "$BIN_NAME" --release --target "$ALVO" $FEATURES --offline

BIN="$LAUNCHER/target/$ALVO/release/$BIN_NAME"
[ -f "$BIN" ] || erro "binário não gerado em $BIN"

mkdir -p "$LAUNCHER/bin"
cp "$BIN" "$LAUNCHER/bin/$BIN_NAME"
chmod +x "$LAUNCHER/bin/$BIN_NAME"
log "pronto: $LAUNCHER/bin/$BIN_NAME ($(du -h "$BIN" | cut -f1))"

# ── 6. instalar no aparelho ─────────────────────────────────
if [ -n "$INSTALAR" ]; then
  # O caminho do aparelho segue a convenção do `Giochi System`: a pasta é o
  # **slug**, não o nome do crate. `<grupo>/<slug>` vira `/opt/giochi/apps/<slug>`
  # — e é essa a regra que o catálogo usa.
  DESTINO="/opt/giochi/apps/$SLUG/bin"
  # O dropbear/openssh do aparelho avisa sobre criptografia pós-quântica a cada
  # conexão. O aviso vai para stderr e polui a saída do script inteiro.
  filtrar_ruido() { grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true; }

  log "instalando em $INSTALAR:$DESTINO"
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$INSTALAR" "mkdir -p $DESTINO" 2>&1 | filtrar_ruido
  scp -q -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$BIN" "$INSTALAR:$DESTINO/$BIN_NAME" 2>&1 | filtrar_ruido
  ssh -o BatchMode=yes -o StrictHostKeyChecking=no "$INSTALAR" \
    "chmod +x $DESTINO/$BIN_NAME && ls -la $DESTINO/$BIN_NAME" 2>&1 | filtrar_ruido
  # **Perguntar ao aparelho**, não afirmar. A mensagem dizia sempre "o inittab
  # não inicia o giochi-init", verdade na imagem do X55 de 03/11 e **falsa** nas
  # de 04/11 e na do R35S — onde o `::respawn:` existe e a interface sobe sozinha.
  #
  # Uma mensagem fixa sobre o estado de outra pessoa envelhece calada, e o custo
  # é diagnosticar "não sobe" quando sobe.
  if ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=8 "$INSTALAR" \
      'grep -qE "^[^#]*respawn:.*giochi-init" /etc/inittab' 2>/dev/null; then
    log "instalado. O inittab TEM respawn do giochi-init: a interface sobe no boot."
    log "  para ver agora sem reiniciar: bash scripts/aparelho.sh subir"
  else
    log "instalado. O inittab NÃO tem respawn do giochi-init nesta imagem."
    log "  suba com: bash scripts/aparelho.sh subir   (e relate ao Giochi System)"
  fi
fi
