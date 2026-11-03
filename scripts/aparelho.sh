#!/usr/bin/env bash
# aparelho.sh — operação e diagnóstico do aparelho X55 por SSH.
#
# Substitui a sequência manual de comandos que era digitada a cada teste. O
# aparelho tem três peculiaridades que justificam cada subcomando existir:
#
#   1. **O IP muda a cada boot** (DHCP). `procurar` varre a /24 e identifica o
#      aparelho pelo hostname, em vez de assumir o endereço antigo.
#   2. **O `inittab` não inicia o `giochi-init`**, então a interface não sobe
#      sozinha. `subir` faz o que o init deveria fazer.
#   3. **"Processo vivo" não prova nada.** `verificar` exige os descritores do
#      DRM e dos dispositivos de entrada abertos — sem isso o app pode estar
#      rodando sem desenhar.
#
# Uso:
#   bash scripts/aparelho.sh procurar
#   bash scripts/aparelho.sh diagnostico
#   bash scripts/aparelho.sh instalar
#   bash scripts/aparelho.sh subir
#   bash scripts/aparelho.sh parar
#   bash scripts/aparelho.sh verificar
#   bash scripts/aparelho.sh log
#   bash scripts/aparelho.sh input [segundos]
#   bash scripts/aparelho.sh capacidades [filtro]
#
# Variáveis:
#   GIOCHI_DEVICE   host do aparelho (padrão: descobre sozinho)
#   GIOCHI_SUBREDE  prefixo da rede (padrão: 192.168.1)
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBREDE="${GIOCHI_SUBREDE:-192.168.1}"
DESTINO_BIN="/opt/giochi/apps/launcher/bin/giochi-launcher"
LOG="/tmp/giochi.log"

# Opções do ssh: sem host key interativa (o IP muda, a chave seria outra) e com
# timeout curto, porque o aparelho desliga e a conexão fica pendurada.
SSH_OPTS=(-o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=8)
SCP_OPTS=(-o BatchMode=yes -o StrictHostKeyChecking=no)

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; AMARELO=$'\033[33m'; NEGRITO=$'\033[1m'; ZERA=$'\033[0m'
# Sem cor quando a saída não é um terminal. Sem isto, qualquer script que leia
# esta saída (como a descoberta de IP do cross-compile) recebe `\x1b[32m` no meio
# do endereço e o `grep` por IP deixa de casar.
if [ ! -t 1 ]; then
  VERDE=''; VERMELHO=''; AMARELO=''; NEGRITO=''; ZERA=''
fi

ok()     { printf '%s  ok%s   %s\n' "$VERDE" "$ZERA" "$*"; }
falha()  { printf '%sFALHA%s %s\n' "$VERMELHO" "$ZERA" "$*"; }
aviso()  { printf '%saviso%s %s\n' "$AMARELO" "$ZERA" "$*"; }
titulo() { printf '\n%s%s%s\n' "$NEGRITO" "$*" "$ZERA"; }

morrer() { falha "$*"; exit 1; }

# Remove o ruído de aviso de criptografia do dropbear/openssh antigo.
sem_ruido() {
  grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" || true
}

# ── descoberta ──────────────────────────────────────────────

# O aparelho se identifica pelo hostname. Não usamos MAC nem fabricante porque
# nenhum dos dois é estável entre builds da imagem.
eh_o_aparelho() {
  local ip="$1"
  local nome
  nome=$(timeout 6 ssh "${SSH_OPTS[@]}" "root@$ip" 'uname -n' 2>/dev/null | sem_ruido)
  case "$nome" in
    *giochi*) printf '%s' "$ip"; return 0 ;;
  esac
  return 1
}

# Varre a /24 em paralelo. Serial levaria minutos com 254 endereços e timeout.
procurar() {
  local encontrados
  encontrados=$(mktemp)
  local pids=()

  for i in $(seq 1 254); do
    (
      ip="$SUBREDE.$i"
      if timeout 1 bash -c "echo > /dev/tcp/$ip/22" 2>/dev/null; then
        if eh_o_aparelho "$ip" >/dev/null 2>&1; then
          printf '%s\n' "$ip" >> "$encontrados"
        fi
      fi
    ) &
    pids+=($!)
    # Limita concorrência: 254 processos simultâneos estouram o limite de fds.
    if [ "${#pids[@]}" -ge 40 ]; then
      wait "${pids[0]}" 2>/dev/null
      pids=("${pids[@]:1}")
    fi
  done
  wait 2>/dev/null

  local lista
  lista=$(sort -u "$encontrados")
  rm -f "$encontrados"

  if [ -z "$lista" ]; then
    return 1
  fi
  printf '%s\n' "$lista"
}

# Resolve o host: variável de ambiente, senão descoberta.
resolver_host() {
  if [ -n "${GIOCHI_DEVICE:-}" ]; then
    printf '%s' "$GIOCHI_DEVICE"
    return 0
  fi
  local achado
  achado=$(procurar)
  [ -n "$achado" ] || return 1
  printf '%s' "$(printf '%s' "$achado" | head -1)"
}

# Executa no aparelho. Aborta com mensagem clara se a rede não responder.
no_aparelho() {
  ssh "${SSH_OPTS[@]}" "root@$HOST" "$@" 2>&1 | sem_ruido
}

exigir_aparelho() {
  HOST=$(resolver_host) || verdadeiro
  if [ -z "${HOST:-}" ]; then
    morrer "aparelho não encontrado em $SUBREDE.0/24 — está ligado? (GIOCHI_DEVICE=ip para fixar)"
  fi
  if ! timeout 10 ssh "${SSH_OPTS[@]}" "root@$HOST" true 2>/dev/null; then
    morrer "sem SSH em $HOST"
  fi
  ok "aparelho em $HOST"
}

verdadeiro() { return 0; }

# ── auxiliares de captura ───────────────────────────────────

# Orientação que o sistema declara, no vocabulário do verificador.
#
# A fonte é a mesma que o app lê: `/sys/class/graphics/fbcon/rotate`, que o
# kernel preenche a partir de `fbcon=rotate:N` na cmdline. Ler daqui garante que
# o teste e o app concordem — se um dia discordarem, a comparação contra a
# referência do PC acusa 180° de diferença e o problema aparece.
rotacao_do_aparelho() {
  local n
  n=$(no_aparelho 'cat /sys/class/graphics/fbcon/rotate 2>/dev/null' | tr -dc '0-9')
  case "${n:-0}" in
    0) echo none ;;
    1) echo cw ;;    # 90° horário
    2) echo none ;;  # 180° — o app não implementa; a comparação vai reprovar
    3) echo ccw ;;   # 270° = 90° anti-horário
    *) echo none ;;
  esac
}

# Desenha a mesma Home no PC, sem tela, para servir de referência.
#
# `SDL_VIDEODRIVER=dummy` é obrigatório: no driver x11 o SDL troca o buffer e a
# leitura de volta vem preta, mesmo com a UI desenhada. Já aconteceu — o quadro
# saiu 100% preto e parecia bug do app.
gerar_referencia() {
  local destino="$1"
  local bin="$RAIZ/app/launcher/target/debug/giochi-launcher"
  local libs="$RAIZ/app/launcher/vendor/sdl2/lib"
  [ -x "$bin" ] || return 1
  rm -f "$destino"
  SDL_VIDEODRIVER=dummy \
    LD_LIBRARY_PATH="${libs}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    timeout 60 "$bin" --dump-frame "$destino" >/dev/null 2>&1
  [ -s "$destino" ]
}

# Sobe o launcher e imprime pid + log. Separado de `cmd_subir` porque `verificar`
# precisa retomar o app depois de capturar o frame.
iniciar_launcher() {
  no_aparelho "
    rm -f $LOG
    setsid nohup env SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
      $DESTINO_BIN > $LOG 2>&1 < /dev/null &
    sleep 5
    pid=\$(pidof giochi-launcher 2>/dev/null)
    if [ -z \"\$pid\" ]; then
      echo \"MORREU\"
      tail -20 $LOG
    else
      echo \"pid=\$pid\"
      cat $LOG
    fi
  "
}

# ── subcomandos ─────────────────────────────────────────────

cmd_procurar() {
  titulo "procurando o aparelho na $SUBREDE.0/24"
  local lista
  if ! lista=$(procurar); then
    falha "nenhum aparelho Giochi encontrado"
    printf '  dicas: está ligado? mesma rede Wi-Fi? GIOCHI_DEVICE=ip para fixar\n'
    exit 1
  fi
  printf '%s\n' "$lista" | while read -r ip; do
    printf '  %s %s\n' "$(printf '%s' "$VERDE")$ip$(printf '%s' "$ZERA")" "$(ssh "${SSH_OPTS[@]}" "root@$ip" 'uname -n' 2>/dev/null | sem_ruido)"
  done
}

cmd_diagnostico() {
  exigir_aparelho

  titulo "sistema"
  no_aparelho '
    echo "host:      $(uname -n)"
    echo "kernel:    $(uname -r)  $(uname -m)"
    echo "uptime:    $(cut -d. -f1 /proc/uptime)s"
    echo "distro:    $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")"
    echo "memória:   $(awk "/MemTotal/{printf \"%d MB\", \$2/1024}" /proc/meminfo)"
    echo "rootfs:    $(df -h / | awk "NR==2{print \$4\" livres de \"\$2}")"
  '

  titulo "tela"
  no_aparelho '
    echo "painel:    $(cat /sys/class/drm/card0-DSI-1/status 2>/dev/null) $(head -1 /sys/class/drm/card0-DSI-1/modes 2>/dev/null)"
    echo "dpms:      $(cat /sys/class/drm/card0-DSI-1/dpms 2>/dev/null)"
    echo "rotação:   fbcon/rotate=$(cat /sys/class/graphics/fbcon/rotate 2>/dev/null)  ($(grep -o "fbcon=rotate:[0-9]" /proc/cmdline 2>/dev/null || echo "sem fbcon=rotate"))"
    for b in /sys/class/backlight/*; do
      [ -d "$b" ] && echo "backlight: $(cat $b/brightness 2>/dev/null)/$(cat $b/max_brightness 2>/dev/null) em $(basename $b)"
    done
  '

  titulo "áudio"
  no_aparelho '
    echo "cards:     $(grep -c "^ *[0-9]" /proc/asound/cards 2>/dev/null || echo 0)"
    grep -E "^ *[0-9]" /proc/asound/cards 2>/dev/null | sed "s/^/           /"
    if [ -x /usr/sbin/alsactl ]; then
      /usr/sbin/alsactl -f /tmp/diag.state store 2>/dev/null &&
        grep -A2 "Master Playback Volume" /tmp/diag.state | grep value.0 | sed "s/^/           Master: /"
      rm -f /tmp/diag.state
    fi
  '

  titulo "entrada"
  no_aparelho '
    for e in /dev/input/event*; do
      [ -e "$e" ] || continue
      n=$(basename $e)
      nome=$(cat /sys/class/input/$n/device/name 2>/dev/null)
      printf "  %-14s %s\n" "$n" "$nome"
    done
  '

  titulo "bibliotecas"
  no_aparelho '
    for l in libSDL2-2.0.so.0 libSDL2_ttf-2.0.so.0 libasound.so.2 libfreetype.so.6; do
      if [ -e "/usr/lib/$l" ]; then printf "  %-24s presente\n" "$l"; else printf "  %-24s AUSENTE\n" "$l"; fi
    done
    echo "  fonte TTF: $(find /usr/share/fonts -name "*.ttf" 2>/dev/null | head -1 || true)"
  '

  titulo "configuração do Giochi"
  no_aparelho '
    for f in /user/.config/giochi/brightness /user/.config/giochi/volume /user/.config/giochi/rotation; do
      if [ -f "$f" ]; then printf "  %-38s = %s\n" "$(basename $f)" "$(cat $f)"; else printf "  %-38s ausente\n" "$(basename $f)"; fi
    done
    if [ -f /etc/giochi/buttons.map ]; then
      echo "  buttons.map: $(grep -cE "^[a-z]" /etc/giochi/buttons.map) entradas"
    fi
  '

  titulo "boot e init"
  no_aparelho '
    if grep -q "giochi-init" /etc/inittab 2>/dev/null; then
      echo "  inittab:    giochi-init declarado"
    else
      echo "  inittab:    giochi-init AUSENTE (a UI não sobe no boot)"
    fi
    echo "  giochi-init: $([ -x /usr/bin/giochi-init ] && echo presente || echo ausente)"
    if [ -f /proc/device-tree/i2c@fdd40000/pmic@20/compatible ]; then
      echo "  PMIC:       $(tr -d "\0" < /proc/device-tree/i2c@fdd40000/pmic@20/compatible 2>/dev/null)"
    fi
  '

  titulo "processo do launcher"
  no_aparelho '
    p=$(pidof giochi-launcher 2>/dev/null)
    if [ -z "$p" ]; then
      echo "  rodando:   NÃO"
    else
      echo "  rodando:   sim (pid $p)"
      echo "  DRM:       $(ls -l /proc/$p/fd 2>/dev/null | grep -c dri) descritores"
      echo "  entrada:   $(ls -l /proc/$p/fd 2>/dev/null | grep -c "input/event") descritores"
      echo "  binário:   $(ls -l '"$DESTINO_BIN"' 2>/dev/null | awk "{print \$5\" bytes\"}")"
    fi
  '
}

cmd_instalar() {
  exigir_aparelho
  titulo "compilando e instalando"
  # O script de cross-compile já sabe compilar e instalar; aqui só delegamos para
  # não duplicar a lógica de sysroot e wrapper de link.
  GIOCHI_DEVICE="$HOST" bash "$RAIZ/scripts/compilar-launcher-arm64.sh" --instalar "root@$HOST" \
    || morrer "falha na compilação/instalação"

  titulo "conferindo integridade"
  local remoto local_h
  remoto=$(no_aparelho "sha256sum $DESTINO_BIN | cut -d' ' -f1")
  local_h=$(sha256sum "$RAIZ/app/launcher/bin/giochi-launcher" | cut -d' ' -f1)
  if [ "$remoto" = "$local_h" ]; then
    ok "sha256 idêntico ($(printf '%s' "$local_h" | cut -c1-16)…)"
  else
    morrer "sha256 diferente: remoto=$remoto local=$local_h"
  fi
}

cmd_subir() {
  exigir_aparelho

  # `pidof` em vez de `pkill -f`: `pkill -f giochi-launcher` casa com o próprio
  # shell que executa o comando (a linha de comando contém o nome) e mata a
  # sessão SSH antes de fazer o resto. Já aconteceu.
  no_aparelho 'p=$(pidof giochi-launcher 2>/dev/null); [ -n "$p" ] && kill $p 2>/dev/null; sleep 1; true'

  # `[ -x ]` não imprime nada nem quando dá certo, então é preciso um eco
  # explícito: sem ele `... | grep -q .` falha sempre e a mensagem diz que o
  # binário não existe mesmo existindo.
  if ! no_aparelho "[ -x $DESTINO_BIN ] && echo PRESENTE" | grep -q PRESENTE; then
    morrer "$DESTINO_BIN não existe ou não é executável — rode 'instalar'"
  fi

  titulo "iniciando"
  iniciar_launcher | while IFS= read -r linha; do
    case "$linha" in
      MORREU) falha "o launcher não subiu" ;;
      pid=*)  ok "${linha}" ;;
      *)      printf '  %s\n' "$linha" ;;
    esac
  done
}

cmd_parar() {
  exigir_aparelho
  local saida
  saida=$(no_aparelho 'p=$(pidof giochi-launcher 2>/dev/null); if [ -z "$p" ]; then echo NADA; else kill $p 2>/dev/null; sleep 1; echo "PARADO $p"; fi')
  case "$saida" in
    NADA)    aviso "já não estava rodando" ;;
    PARADO*) ok "parado (${saida#PARADO })" ;;
    *)       falha "$saida" ;;
  esac
}

cmd_log() {
  exigir_aparelho
  titulo "log do launcher ($LOG)"
  no_aparelho "if [ -f $LOG ]; then tail -40 $LOG; else echo '(sem log — o app nunca subiu nesta sessão)'; fi"
}

# Mostra o que o kernel entrega, sem mapa e sem tradução.
#
# É o ponto de partida de qualquer bug de botão: se o evento não aparece aqui, o
# problema é do kernel ou do `buttons.map`; se aparece aqui e o app não reage, é
# do app. Sem essa separação se depura no escuro — foi assim que o `KEY_POWER` e
# as teclas de volume ficaram meses sem função.
cmd_input() {
  exigir_aparelho
  local segundos="${1:-15}"
  titulo "eventos crus de /dev/input por ${segundos}s"
  printf '  aperte os botões agora. Cada linha é um EV_KEY do kernel.\n\n'
  # Sem `timeout`: o BusyBox do aparelho não tem esse comando. O próprio
  # `--espiar-input <segundos>` termina sozinho.
  no_aparelho "env SDL_VIDEODRIVER=dummy $DESTINO_BIN --espiar-input $segundos"
  printf '\n  para saber o código de um botão sem apertá-lo, use:\n'
  printf '  bash scripts/aparelho.sh capacidades\n'
}

# O que o kernel **promete** emitir, por dispositivo.
#
# Complementa `input`, que mostra o que ele **entregou**. Quando um botão não
# reage, são duas perguntas diferentes: o evento chegou? o dispositivo declara
# esse código? Aqui responde-se a segunda, lendo `/proc/bus/input/devices` — o
# único caminho, porque `evtest --query` devolve 0 para qualquer código nesta
# imagem e não serve para descobrir nada.
cmd_capacidades() {
  exigir_aparelho
  local filtro="${1:-}"
  titulo "códigos que cada dispositivo declara"
  local entrada
  entrada=$(no_aparelho 'cat /proc/bus/input/devices')
  local tmp
  tmp="$(mktemp)"
  printf '%s\n' "$entrada" > "$tmp"
  if [ -n "$filtro" ]; then
    node "$RAIZ/scripts/capacidades-input.mjs" --arquivo "$tmp" --dispositivo "$filtro"
  else
    node "$RAIZ/scripts/capacidades-input.mjs" --arquivo "$tmp"
  fi
  rm -f "$tmp"
  printf '\n  cruzar com o app: se o código não aparece em `input` ao apertar o\n'
  printf '  botão, o problema é driver/DTS. Se aparece e o app ignora, o problema\n'
  printf '  é o buttons.map ou o codigo nao tratado no app.\n'
}

# Verificação que importa: processo vivo E hardware aberto E frame correto.
cmd_verificar() {
  exigir_aparelho

  titulo "processo"
  local pid
  pid=$(no_aparelho 'pidof giochi-launcher 2>/dev/null')
  if [ -z "$pid" ]; then
    falha "não está rodando"
    printf '  suba com: bash scripts/aparelho.sh subir\n'
    exit 1
  fi
  ok "pid $pid"

  titulo "hardware aberto"
  local drm entrada
  drm=$(no_aparelho "ls -l /proc/$pid/fd 2>/dev/null | grep -c dri")
  entrada=$(no_aparelho "ls -l /proc/$pid/fd 2>/dev/null | grep -c 'input/event'")
  [ "${drm:-0}" -ge 2 ] && ok "DRM: ${drm} descritores" || falha "DRM: nenhum descritor (não está desenhando)"
  [ "${entrada:-0}" -ge 1 ] && ok "entrada: ${entrada} dispositivos" || aviso "entrada: nenhum dispositivo aberto"

  titulo "tela"
  no_aparelho '
    echo "  dpms:      $(cat /sys/class/drm/card0-DSI-1/dpms 2>/dev/null)"
    for b in /sys/class/backlight/*; do [ -d "$b" ] && echo "  backlight: $(cat $b/brightness)/$(cat $b/max_brightness)"; done
  '

  titulo "frame"
  # Captura um quadro e confere que ele tem conteúdo. Sem isso, "o processo
  # está vivo" pode significar uma tela preta.
  #
  # O KMSDRM é exclusivo: enquanto a instância em execução tem o DRM aberto,
  # um segundo processo recebe `kmsdrm not available` e não gera BMP nenhum.
  # Por isso a captura para o app, e sobe de novo no fim — e a decisão de subir
  # de novo depende de ele estar rodando ANTES da captura, não depois.
  local tmp_captura estava_rodando
  # Capturas ficam em `dist/testes/`, que é ignorado pelo git: o quadro é
  # evidência para olhar agora, não artefato para versionar.
  tmp_captura="$RAIZ/dist/testes/$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$tmp_captura"
  estava_rodando=$(no_aparelho 'pidof giochi-launcher 2>/dev/null')

  if no_aparelho "
      p=\$(pidof giochi-launcher 2>/dev/null); [ -n \"\$p\" ] && kill \$p 2>/dev/null
      sleep 1
      rm -f /tmp/verif.bmp
      env SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software $DESTINO_BIN --dump-frame /tmp/verif.bmp >/dev/null 2>&1
      [ -s /tmp/verif.bmp ] && echo GERADO
    " | grep -q GERADO; then
    if scp "${SCP_OPTS[@]}" -q "root@$HOST:/tmp/verif.bmp" "$tmp_captura/frame.bmp" 2>/dev/null; then
      # A referência é o mesmo código rodando no PC, sem tela. Se o binário de
      # PC existir, comparar vale mais que as heurísticas de cor: é o que separa
      # "desenhou" de "desenhou de cabeça para baixo".
      local referencia rotacao
      referencia="$RAIZ/app/launcher/target/debug/giochi-launcher"
      rotacao=$(rotacao_do_aparelho)
      if [ -x "$referencia" ]; then
        if gerar_referencia "$tmp_captura/ref.bmp"; then
          node "$RAIZ/scripts/verificar-frame.mjs" \
            --captura "$tmp_captura/frame.bmp" \
            --referencia "$tmp_captura/ref.bmp" \
            --rotacao "$rotacao" \
            --png "$tmp_captura/frame.png" || falha "frame suspeito"
          ok "quadro para inspeção: $tmp_captura/frame.png"
        else
          aviso "não consegui gerar a referência no PC — só as heurísticas"
          node "$RAIZ/scripts/verificar-frame.mjs" --captura "$tmp_captura/frame.bmp" || falha "frame suspeito"
        fi
      else
        aviso "sem binário de PC para comparar — rode scripts/compilar-launcher.sh"
        node "$RAIZ/scripts/verificar-frame.mjs" --captura "$tmp_captura/frame.bmp" || falha "frame suspeito"
      fi
      no_aparelho 'rm -f /tmp/verif.bmp'
    else
      aviso "não consegui baixar o frame"
    fi
  else
    falha "o app não gerou frame — provavelmente não consegue abrir o DRM"
  fi

  if [ -n "$estava_rodando" ]; then
    titulo "retomando"
    iniciar_launcher | while IFS= read -r linha; do printf '  %s\n' "$linha"; done
  fi
}

# ── entrada ─────────────────────────────────────────────────

case "${1:-ajuda}" in
  procurar)    cmd_procurar ;;
  diagnostico) cmd_diagnostico ;;
  instalar)    cmd_instalar ;;
  subir)       cmd_subir ;;
  parar)       cmd_parar ;;
  verificar)   cmd_verificar ;;
  log)         cmd_log ;;
  input)       shift; cmd_input "${1:-15}" ;;
  capacidades) shift; cmd_capacidades "${1:-}" ;;
  ajuda|--help|-h|"")
    # Só o bloco de comentário do topo: para no primeiro `set -`, não numa
    # faixa fixa de linhas que sai do lugar a cada edição.
    awk 'NR>1 && /^#/ {sub(/^# ?/, ""); print; next} NR>1 {exit}' "${BASH_SOURCE[0]}"
    ;;
  *)
    falha "subcomando desconhecido: $1"
    printf 'use: procurar | diagnostico | instalar | subir | parar | verificar | log\n'
    exit 1
    ;;
esac
