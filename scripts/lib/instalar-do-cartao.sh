#!/bin/sh
# Instala todo `.giochi` que estiver numa pasta. Roda **no aparelho**.
#
#     instalar-do-cartao.sh [pasta]
#
# Pasta padrão: a partição `GIOCHI_BOOT` do cartão, montada em `/mnt/cartao`.
#
# ## Por que isto existe
#
# Até aqui instalar um jogo exigia SSH e um PC com Node. Num portátil o caminho
# natural é outro: tirar o cartão, copiar o `.giochi` pelo leitor do PC, devolver
# o cartão e ligar. Sem este script o arquivo fica no cartão e **nada acontece** —
# o pior modo de falha, porque não há erro nenhum.
#
# ## O que é verificado, e o que NÃO é
#
# | Verificação | Aqui | Por quê |
# | --- | --- | --- |
# | SHA-256 contra o manifesto | **sim** | pega cópia corrompida, que é o erro real de quem copia pelo leitor |
# | assinatura Ed25519 | **NÃO** | não há crate de Ed25519 na imagem; ver abaixo |
# | entrypoint existe e é executável | **sim** | o erro aparece como "não abre" e não aponta a causa |
#
# **Ausência de verificação de assinatura é limitação declarada, não recurso.**
# Ela importa quando o pacote vem de um canal em que se confia pouco — a loja
# pela rede. Aqui o pacote vem de um cartão que o dono pôs na mão dele: quem
# consegue escrever no cartão também consegue copiar um binário para
# `/user/apps` sem passar por aqui. O que a verificação **não** cobre é uma
# corrupção silenciosa na cópia — e é isso que o SHA-256 resolve.
#
# Enquanto a assinatura não é verificada no aparelho, o PC continua verificando
# antes de enviar (`verificar-pacote.mjs`).

set -u

PASTA="${1:-/mnt/cartao}"
INDICE="/opt/giochi/apps/index.json"
LAUNCHER="/opt/giochi/apps/launcher/bin/giochi-launcher"
DESTINO_BASE="/user/apps"

[ -d "$PASTA" ] || { echo "ERRO pasta não existe: $PASTA"; exit 1; }
[ -x "$LAUNCHER" ] || { echo "ERRO launcher não está em $LAUNCHER"; exit 1; }

mkdir -p "$DESTINO_BASE" /opt/giochi/apps || { echo "ERRO não consegui criar destino"; exit 1; }

TOTAL=0
OK=0
FALHOU=0

for PACOTE in "$PASTA"/*.giochi; do
  # Sem casamento, o glob devolve o próprio padrão.
  [ -f "$PACOTE" ] || continue
  TOTAL=$((TOTAL + 1))
  NOME=$(basename "$PACOTE")

  # ── integridade ────────────────────────────────────────
  MANIFESTO="$PACOTE.manifest.json"
  if [ -f "$MANIFESTO" ]; then
    ESPERADO=$(sed -n 's/.*"sha256"[[:space:]]*:[[:space:]]*"\([a-f0-9]*\)".*/\1/p' "$MANIFESTO" | head -1)
    if [ -n "$ESPERADO" ]; then
      REAL=$(sha256sum "$PACOTE" | awk '{print $1}')
      if [ "$ESPERADO" != "$REAL" ]; then
        echo "  $NOME: RECUSADO — sha256 não confere (esperado ${ESPERADO%"${ESPERADO#????????}"}..., obtido ${REAL%"${REAL#????????}"}...)"
        FALHOU=$((FALHOU + 1))
        continue
      fi
    else
      echo "  $NOME: aviso — manifesto sem sha256; seguindo só com a extração"
    fi
  else
    # Sem manifesto não há como detectar corrupção. Recusar seria mais seguro,
    # mas o `.giochi` sozinho é o formato documentado — então avisa e segue.
    echo "  $NOME: aviso — sem manifest.json ao lado, integridade não verificada"
  fi

  # ── extração ───────────────────────────────────────────
  # O slug vem do metadata dentro do pacote (como no lado do PC): renomear o
  # arquivo não pode mudar onde o app é instalado.
  TMP="${TMPDIR:-/tmp}/giochi-cartao.$$"
  rm -rf "$TMP"; mkdir -p "$TMP"
  if ! tar -xf "$PACOTE" -C "$TMP" 2>/dev/null; then
    rm -rf "$TMP"
    echo "  $NOME: RECUSADO — tar não conseguiu extrair"
    FALHOU=$((FALHOU + 1))
    continue
  fi

  SLUG=$(sed -n 's/.*"slug"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP/metadata.json" | head -1)
  [ -n "$SLUG" ] || SLUG=$(basename "$NOME" .giochi)

  ENTRY=$(sed -n 's/.*"entrypoint"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP/metadata.json" | head -1)
  if [ -z "$ENTRY" ] || [ ! -f "$TMP/$ENTRY" ]; then
    rm -rf "$TMP"
    echo "  $NOME: RECUSADO — entrypoint ausente ou fora do pacote"
    FALHOU=$((FALHOU + 1))
    continue
  fi
  chmod +x "$TMP/$ENTRY" 2>/dev/null

  DESTINO="$DESTINO_BASE/$SLUG"
  rm -rf "$DESTINO"
  mkdir -p "$DESTINO" && cp -a "$TMP"/. "$DESTINO"/ || {
    rm -rf "$TMP" "$DESTINO"
    echo "  $NOME: RECUSADO — falhou ao copiar para $DESTINO"
    FALHOU=$((FALHOU + 1))
    continue
  }
  rm -rf "$TMP"

  # ── catálogo ───────────────────────────────────────────
  # O launcher atualiza o índice: é ele que tem o parser de JSON, e um `sed`
  # aqui produziria JSON inválido no primeiro campo com vírgula.
  if ! "$LAUNCHER" --catalogo-adicionar "$DESTINO" >/dev/null 2>&1; then
    echo "  $NOME: instalado em $DESTINO, mas o catálogo NÃO foi atualizado"
    FALHOU=$((FALHOU + 1))
    continue
  fi

  echo "  $NOME: instalado em $DESTINO"
  OK=$((OK + 1))
done

echo "RESUMO $OK instalado(s), $FALHOU recusado(s), $TOTAL encontrado(s)"
[ "$FALHOU" -eq 0 ] || exit 1
exit 0
