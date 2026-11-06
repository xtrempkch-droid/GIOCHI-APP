#!/usr/bin/env bash
# Compila `verificador-ed25519` para aarch64 e (opcionalmente) instala no aparelho.
#
# Uso:
#   bash scripts/compilar-verificador-arm64.sh              # só compila
#   bash scripts/compilar-verificador-arm64.sh --instalar   # compila e instala
#
# ## Por que este script existe
#
# O verificador é a única peça em C do repositório, e compilá-lo à mão exige
# quatro coisas que ninguém lembra: um compilador cruzado, os headers do
# OpenSSL, o `configuration.h` do alvo (o do host é x86-64 e define
# `SIXTY_FOUR_BIT` errado), e o link com o `libcrypto` **do aparelho** em vez do
# do host — senão o binário pede uma versão de símbolo que não existe lá.
#
# ## Por que gcc musl, se o aparelho é glibc
#
# O compilador só é usado com `-c`, para produzir um objeto aarch64. O link é
# feito pelo `rust-lld` contra o sysroot glibc do aparelho — o mesmo wrapper que
# o launcher usa (`~/.cache/giochi-arm64/cc-aarch64.sh`). O objeto ELF aarch64 é
# o mesmo nos dois mundos; quem escolhe a libc é o link.
#
# ## O que precisa existir
#
#   ~/.cache/giochi-arm64/sysroot         sysroot glibc do aparelho
#   ~/.cache/giochi-arm64/sysroot/usr/lib/libcrypto.so.3
#                                        extraído do rootfs (o script confere)
#   toolchain aarch64 (GCC_ARM64 ou PATH, ou baixa em /tmp)
#
# O `libcrypto` sai do `rootfs.ext4` do Giochi System com `debugfs`, sem root:
#
#   debugfs -R "dump /usr/lib/libcrypto.so.3 $SR/usr/lib/libcrypto.so.3" rootfs.ext4
#
# Foi medido: o aparelho tem `libcrypto.so.3` (OpenSSL 3.2.1) e **não** tem
# `openssl` (a CLI). Por isso este binário existe.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FONTE="$RAIZ/scripts/lib/verificador-ed25519.c"
SR="${GIOCHI_ARM64_SYSROOT:-$HOME/.cache/giochi-arm64/sysroot}"
CC_WRAPPER="$HOME/.cache/giochi-arm64/cc-aarch64.sh"
SAIDA="${GIOCHI_SAIDA:-$RAIZ/dist/arm64}"
INSTALAR=0

for arg in "$@"; do
  case "$arg" in
    --instalar) INSTALAR=1 ;;
    -h|--ajuda) sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "argumento desconhecido: $arg" >&2; exit 2 ;;
  esac
done

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; ZERA=$'\033[0m'
ok()   { printf '  %sok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
erro() { printf '  %sERRO%s %s\n' "$VERMELHO" "$ZERA" "$*" >&2; exit 1; }

# ── pré-requisitos ────────────────────────────────────────
[ -f "$FONTE" ] || erro "fonte ausente: $FONTE"

if [ ! -f "$SR/usr/lib/libcrypto.so.3" ]; then
  erro "libcrypto ausente em $SR/usr/lib/
  extraia do rootfs do sistema:
    debugfs -R \"dump /usr/lib/libcrypto.so.3 $SR/usr/lib/libcrypto.so.3\" <rootfs.ext4>
    ln -sf libcrypto.so.3 $SR/usr/lib/libcrypto.so"
fi
[ -f "$CC_WRAPPER" ] || erro "wrapper de link ausente: $CC_WRAPPER (veja docs/compilar-launcher.md)"

# ── compilador cruzado ────────────────────────────────────
CC="${GCC_ARM64:-}"
if [ -z "$CC" ]; then
  CC=$(command -v aarch64-linux-gnu-gcc || command -v aarch64-linux-musl-gcc || true)
fi
if [ -z "$CC" ]; then
  CACHE=/tmp/aarch64-linux-musl-cross
  if [ ! -x "$CACHE/bin/aarch64-linux-musl-gcc" ]; then
    echo "  baixando toolchain aarch64 (~108 MB, uma vez)..."
    ( cd /tmp && curl -fsSL https://musl.cc/aarch64-linux-musl-cross.tgz -o x.tgz \
      && tar xzf x.tgz && rm -f x.tgz ) || erro "não consegui baixar a toolchain; defina GCC_ARM64"
  fi
  CC="$CACHE/bin/aarch64-linux-musl-gcc"
fi
ok "compilador: $CC"

# ── headers do OpenSSL, configurados para o alvo ──────────
#
# O `configuration.h` do host é x86-64 (`SIXTY_FOUR_BIT` e uma API 3.5). O do
# aparelho é aarch64 e API 3.2.1. Sem trocar os dois, o compilador erra em
# `BN_ULONG` — e o erro não sugere que o problema é o alvo.
INC="${TMPDIR:-/tmp}/giochi-verif-inc"
rm -rf "$INC"; mkdir -p "$INC"
[ -d /usr/include/openssl ] || erro "headers do OpenSSL ausentes no host (apt install libssl-dev)"
cp -r /usr/include/openssl "$INC/openssl"
# Em Debian/Ubuntu o `opensslconf.h` mora em `/usr/include/<tripla>/openssl/`.
# Ele é só um shim (`#include <openssl/configuration.h>`), mas sem ele o
# `macros.h` para a compilação inteira.
if [ ! -f "$INC/openssl/opensslconf.h" ]; then
  SHIM=$(find /usr/include -path '*/openssl/opensslconf.h' 2>/dev/null | head -1)
  [ -n "$SHIM" ] && cp "$SHIM" "$INC/openssl/opensslconf.h"
fi
[ -f "$INC/openssl/opensslconf.h" ] || erro "opensslconf.h não encontrado no host"
cat > "$INC/openssl/configuration.h" <<'CONF'
#ifndef OPENSSL_CONFIGURATION_H
#define OPENSSL_CONFIGURATION_H
#pragma once
/* Alvo: libcrypto 3.2.1 do aparelho, aarch64, little-endian. */
# define OPENSSL_CONFIGURED_API 30200
# define SIXTY_FOUR_BIT_LONG
# define OPENSSL_SYS_LINUX
# define OPENSSL_SYS_UNIX
# define OPENSSL_THREADS
# define OPENSSL_NO_ASAN
# define OPENSSL_NO_MSAN
# define OPENSSL_NO_UBSAN
# define OPENSSL_NO_GOST
# define OPENSSL_NO_SCTP
# define OPENSSL_NO_TS
# define OPENSSL_NO_HEARTBEATS
# define OPENSSL_NO_DYNAMIC_ENGINE
# define OPENSSL_NO_EC_NISTP_64_GCC_128
#endif
CONF
ok "headers: $INC (API 3.2.1, aarch64)"

# ── compilar e linkar ─────────────────────────────────────
mkdir -p "$SAIDA"
OBJ="${TMPDIR:-/tmp}/verificador-arm64.o"

echo "  compilando..."
"$CC" -O2 -Wall -Wextra -I"$INC" -c "$FONTE" -o "$OBJ" 2>&1 | sed 's/^/    /'
[ -f "$OBJ" ] || erro "compilação falhou"

echo "  linkando contra o libcrypto do aparelho..."
# `rust-lld` não está no PATH por padrão; o wrapper depende dele.
RUST_LLD_DIR=$(dirname "$(find "$HOME/.rustup" -name rust-lld -type f 2>/dev/null | head -1)")
if [ -n "${RUST_LLD_DIR:-}" ] && [ "$RUST_LLD_DIR" != "." ]; then
  PATH="$RUST_LLD_DIR:$PATH"
fi
L="$SR/usr/aarch64-linux-gnu/lib"
# Os `.so` vão direto, não com `-l`: o `libc.so` do sysroot é um linker script
# com caminho absoluto (`/usr/aarch64-linux-gnu/lib/libc.so.6`), que não existe
# no host e faz o link falhar com "cannot open".
"$CC_WRAPPER" -o "$SAIDA/verificador-ed25519" "$OBJ" \
  -L"$SR/usr/lib" "$L/libc.so.6" "$SR/usr/lib/libcrypto.so.3" 2>&1 | sed 's/^/    /'
[ -f "$SAIDA/verificador-ed25519" ] || erro "link falhou"

file "$SAIDA/verificador-ed25519" | grep -q "ARM aarch64" || erro "não saiu aarch64"

DEPENDS=$(strings "$SAIDA/verificador-ed25519" | grep -E '^lib.*\.so' | sort -u | tr '\n' ' ')
ok "binário: $SAIDA/verificador-ed25519 ($(stat -c%s "$SAIDA/verificador-ed25519") bytes)"
ok "depende de: $DEPENDS"

# Uma dependência que o aparelho não tem é o erro que só apareceria lá.
case "$DEPENDS" in
  *libcrypto.so.3*|*libcrypto*) : ;;
  *) erro "o binário não linkou contra libcrypto — assinatura não seria verificada" ;;
esac

# ── instalar ──────────────────────────────────────────────
if [ "$INSTALAR" = "1" ]; then
  echo
  echo "  instalando no aparelho..."
  # Reusa a descoberta do toolkit: o IP muda a cada boot.
  if ! GIOCHI_ALVO=$(bash "$RAIZ/scripts/aparelho.sh" procurar 2>/dev/null | tail -1); then
    erro "aparelho não encontrado; rode 'bash scripts/aparelho.sh procurar'"
  fi
  HOST=$(printf '%s\n' "$GIOCHI_ALVO" | grep -oE '([0-9]{1,3}\.){3}[0-9]{1,3}' | head -1)
  [ -n "$HOST" ] || erro "não consegui extrair o IP de: $GIOCHI_ALVO"

  scp -o StrictHostKeyChecking=no -q "$SAIDA/verificador-ed25519" "root@$HOST:/tmp/verificador" \
    || erro "envio falhou"
  ssh -o StrictHostKeyChecking=no "root@$HOST" '
    mkdir -p /opt/giochi/apps/bin &&
    cp /tmp/verificador /opt/giochi/apps/bin/verificador-ed25519 &&
    chmod 755 /opt/giochi/apps/bin/verificador-ed25519 &&
    rm -f /tmp/verificador &&
    /opt/giochi/apps/bin/verificador-ed25519 >/dev/null 2>&1 || [ $? = 2 ]
  ' || erro "instalação falhou"
  ok "instalado em /opt/giochi/apps/bin/verificador-ed25519"

  # A chave pública tem de estar na imagem; se não estiver, o instalador avisa
  # e segue sem verificar — e isso precisa ser visível agora, não na loja.
  if ssh -o StrictHostKeyChecking=no "root@$HOST" 'test -f /etc/giochi/loja-publica.pem'; then
    ok "chave pública presente em /etc/giochi/loja-publica.pem"
  else
    printf '  %sAVISO%s /etc/giochi/loja-publica.pem ausente: o instalador do cartão\n' \
      $'\033[33m' $'\033[0m'
    printf '         vai AVISAR que não verificou assinatura (não passa calado).\n'
  fi
fi

echo
echo "pronto. O instalador do cartão procura em /opt/giochi/apps/bin/verificador-ed25519"
