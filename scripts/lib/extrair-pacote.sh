#!/bin/sh
# Extrai um pacote .giochi no aparelho. Roda **no aparelho**, não no PC.
#
# ## Por que um script separado
#
# A primeira versão disto era um `sh -c` gigante montado com aspas dentro de
# aspas dentro de `ssh`. Ela produziu
#
#     FALHA instalação falhou: sh: "bin/giochi-pacman": unknown operand
#
# que é o shell comendo o `$(...)` aninhado. Aninhar três níveis de aspas para
# falar com outra máquina não é difícil de escrever — é difícil de **ler quando
# falha**, e o erro não aponta para a aspa errada.
#
# Copiar um arquivo e rodá-lo custa um `scp` e elimina a classe inteira.
#
# ## Uso
#
#     extrair-pacote.sh <pacote.giochi> <destino>
#
# Sai com 0 só se o app ficou instalado e o entrypoint é executável.
# Deixa o motivo em uma linha, no stdout, quando falha — quem chama mostra.

set -u

PACOTE="${1:-}"
DESTINO="${2:-}"

falhar() {
  echo "ERRO $1"
  exit 1
}

[ -n "$PACOTE" ] || falhar "uso: extrair-pacote.sh <pacote.giochi> <destino>"
[ -n "$DESTINO" ] || falhar "uso: extrair-pacote.sh <pacote.giochi> <destino>"
[ -f "$PACOTE" ] || falhar "pacote não existe: $PACOTE"

# Extrai num temporário primeiro. Extrair direto no destino deixaria um app pela
# metade se o `tar` falhasse no meio — e app pela metade no catálogo é pior que
# app ausente: o cartão aparece e não abre.
TEMP="${TMPDIR:-/tmp}/giochi-extrai.$$"
rm -rf "$TEMP"
mkdir -p "$TEMP" || falhar "não consegui criar $TEMP"

if ! tar -xf "$PACOTE" -C "$TEMP" 2>/dev/null; then
  rm -rf "$TEMP"
  falhar "tar recusou o pacote (corrompido, ou não é tar)"
fi

[ -f "$TEMP/metadata.json" ] || {
  rm -rf "$TEMP"
  falhar "pacote sem metadata.json"
}

# O entrypoint vem do metadata, não de um nome fixo. Lido com `sed` porque o
# aparelho não tem `node` nem `jq` — e instalar um deles no rootfs para ler um
# campo seria troca ruim.
ENTRY=$(sed -n 's/.*"entrypoint"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TEMP/metadata.json" | head -1)
[ -n "$ENTRY" ] || {
  rm -rf "$TEMP"
  falhar "metadata.json sem entrypoint"
}

case "$ENTRY" in
  bin/*) : ;;
  *) rm -rf "$TEMP"; falhar "entrypoint fora de bin/: $ENTRY" ;;
esac

case "$ENTRY" in
  *..*) rm -rf "$TEMP"; falhar "entrypoint com ..: $ENTRY" ;;
esac

[ -f "$TEMP/$ENTRY" ] || {
  rm -rf "$TEMP"
  falhar "entrypoint não está no pacote: $ENTRY"
}

# O bit de execução vem do pacote, mas o `tar` de algumas imagens BusyBox não o
# aplica em todos os casos. Reforçar aqui é barato; depender dele não é.
chmod +x "$TEMP/$ENTRY" 2>/dev/null
[ -x "$TEMP/$ENTRY" ] || {
  rm -rf "$TEMP"
  falhar "entrypoint não ficou executável: $ENTRY"
}

# Troca só agora: o destino antigo é removido depois de o novo estar pronto.
rm -rf "$DESTINO"
mkdir -p "$DESTINO" || {
  rm -rf "$TEMP"
  falhar "não consegui criar $DESTINO"
}

cp -a "$TEMP"/. "$DESTINO"/ || {
  rm -rf "$TEMP" "$DESTINO"
  falhar "falhou ao copiar para $DESTINO"
}
rm -rf "$TEMP"

echo "OK $ENTRY"
