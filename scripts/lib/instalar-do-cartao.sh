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
# | assinatura Ed25519 | **se houver verificador e chave** | ver abaixo |
# | entrypoint existe e é executável | **sim** | o erro aparece como "não abre" e não aponta a causa |
#
# ## Integridade não é autenticidade
#
# O SHA-256 sozinho não prova origem: quem escreve no cartão reescreve o `.giochi`
# **e** o manifesto, e os dois concordam. Ele pega corrupção na cópia, que é o
# erro comum.
#
# A assinatura fecha essa lacuna, e é verificada por um binário nativo
# (`verificador-ed25519`, ligado à `libcrypto` do aparelho — medido: 20 MB de RSS,
# 0,01 s). Sem `openssl` na imagem, foi a forma de fazer isso sem instalar 4 MB de
# CLI.
#
# **Quando não dá para verificar, este script DIZ.** Faltando o binário ou a
# chave, ele avisa em vez de deixar o pacote passar como se tivesse sido
# conferido — silêncio aqui seria a mentira fácil.

set -u

PASTA="${1:-/mnt/cartao}"

# Os caminhos são configuráveis por ambiente **para poderem ser testados no PC**.
# No aparelho nada é definido e os padrões valem. Sem isto, a única forma de
# exercitar a verificação de SHA-256 seria com o aparelho na mão — e ela nunca
# tinha sido exercitada desde que foi escrita. É a mesma razão do
# `GIOCHI_CATALOGO` do launcher.
APPS_BASE="${GIOCHI_APPS_BASE:-/opt/giochi/apps}"
INDICE="$APPS_BASE/index.json"
LAUNCHER="${GIOCHI_LAUNCHER:-$APPS_BASE/launcher/bin/giochi-launcher}"
DESTINO_BASE="${GIOCHI_DESTINO_BASE:-/user/apps}"

# O verificador e a chave pública. Ausentes = aviso explícito, não passe livre.
VERIFICADOR="${GIOCHI_VERIFICADOR:-$APPS_BASE/bin/verificador-ed25519}"
CHAVE_PUBLICA="${GIOCHI_CHAVE_PUBLICA:-/etc/giochi/loja-publica.pem}"

[ -d "$PASTA" ] || { echo "ERRO pasta não existe: $PASTA"; exit 1; }
[ -x "$LAUNCHER" ] || { echo "ERRO launcher não está em $LAUNCHER"; exit 1; }

mkdir -p "$DESTINO_BASE" "$APPS_BASE" || { echo "ERRO não consegui criar destino"; exit 1; }

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

  # ── autenticidade ──────────────────────────────────────
  #
  # Depois do hash e antes de extrair: um pacote com assinatura inválida não
  # deve sequer tocar o disco.
  ASSINATURA="$PACOTE.sig"
  if [ -x "$VERIFICADOR" ] && [ -f "$CHAVE_PUBLICA" ]; then
    if [ ! -f "$ASSINATURA" ]; then
      echo "  $NOME: RECUSADO — sem assinatura ao lado, e há chave para conferir"
      FALHOU=$((FALHOU + 1))
      continue
    fi
    SAIDA_ASSIN=$("$VERIFICADOR" "$PACOTE" "$ASSINATURA" "$CHAVE_PUBLICA" 2>&1)
    CODIGO_ASSIN=$?
    case "$CODIGO_ASSIN" in
      0) # Confere, e **diz** que conferiu.
         #
         # Silêncio no sucesso é o defeito espelhado do silêncio no fracasso:
         # sem esta linha, "verificou e passou" e "não verificou" produzem a
         # mesma saída, e o usuário não tem como saber em qual está.
         echo "  $NOME: assinatura confere" ;;
      1) echo "  $NOME: RECUSADO — assinatura não confere"
         FALHOU=$((FALHOU + 1))
         continue ;;
      *) # 2 = não consegui verificar, 3 = chave não é Ed25519.
         # Não é veredito sobre o pacote: avisa e segue, porque recusar por
         # defeito de configuração da imagem pararia a instalação inteira por um
         # motivo que não é do jogo.
         echo "  $NOME: aviso — assinatura não verificada: $SAIDA_ASSIN" ;;
    esac
  elif [ -x "$VERIFICADOR" ]; then
    echo "  $NOME: aviso — sem chave em $CHAVE_PUBLICA, assinatura não verificada"
  else
    echo "  $NOME: aviso — sem verificador, assinatura não verificada"
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
