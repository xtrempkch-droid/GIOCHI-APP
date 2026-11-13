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
#   bash scripts/aparelho.sh opcoes
#   bash scripts/aparelho.sh boot-log   (salva ANTES de regravar o cartão)
#   bash scripts/aparelho.sh catalogo   (gera e envia o índice de apps)
#   bash scripts/aparelho.sh instalar-pacote app.giochi
#                                       (verifica, extrai e republica o índice)
#   bash scripts/aparelho.sh instalar-do-cartao
#                                       (instala os .giochi da partição do cartão)
#   bash scripts/aparelho.sh verificar-teste [tela...]
#                                       (launcher da engine × launcher de produção)
#   bash scripts/aparelho.sh verificar-efeitos
#                                       (a camada de efeitos desenha no aparelho?)
#
# Variáveis:
#   GIOCHI_DEVICE      host do aparelho (padrão: descobre sozinho)
#   GIOCHI_SUBREDE     prefixo da rede (padrão: 192.168.1)
#   GIOCHI_CANDIDATOS  endereços tentados antes da varredura (padrão: 10.55.0.2, o usb0)
#   GIOCHI_SENHA       senha do aparelho; sem ela, autenticação por chave
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBREDE="${GIOCHI_SUBREDE:-192.168.1}"
DESTINO_BIN="/opt/giochi/apps/launcher/bin/giochi-launcher"
LOG="/tmp/giochi.log"

# Endereços conhecidos do aparelho, tentados ANTES de varrer a /24.
#
# São dois caminhos físicos diferentes: pela rede local (DHCP, IP imprevisível)
# e pelo `usb0` (endereço fixo `10.55.0.2`, autenticação por senha). Sondar dois
# endereços leva um segundo; varrer 254 leva minutos e às vezes nem acha, porque
# o cliente DHCP pode ter entregue um IP fora do prefixo configurado.
CANDIDATOS=(${GIOCHI_CANDIDATOS:-10.55.0.2})

# Endereços que **aceitaram conexão e recusaram autenticação** na última
# varredura. Vive no escopo global de propósito: `procurar` roda dentro de
# `$(...)`, que é um subshell, e tudo que ele atribui morre com ele. Antes esta
# variável só existia lá dentro e `cmd_procurar` a lia depois — com `set -u`,
# `${#RECUSADOS[@]}` num array inexistente derrubava o próprio `procurar` com
# `RECUSADOS: variável não associada`, escondendo o diagnóstico que ela existe
# para dar. A comunicação entre os dois é por arquivo; este é o destino.
RECUSADOS=()
RECUSADOS_ARQUIVO=""

# Opções do ssh: sem host key interativa (o IP muda, a chave seria outra) e com
# timeout curto, porque o aparelho desliga e a conexão fica pendurada.
#
# `BatchMode=yes` é o padrão porque falhar rápido é melhor que ficar esperando
# digitação em script. Mas ele também impede o ssh de **pedir** senha, e o
# `usb0` do aparelho autentica por senha — então quando há `GIOCHI_SENHA` o
# BatchMode sai, senão a rota usb0 nunca funcionaria.
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=8)
SCP_OPTS=(-o StrictHostKeyChecking=no)
SSH_BIN=(ssh)
SCP_BIN=(scp)

if [ -n "${GIOCHI_SENHA:-}" ]; then
  # `sshpass` evita redigitar a senha a cada comando encadeado (procurar faz
  # uma conexão por candidato). Sem ele, o ssh pede a senha no terminal — uma
  # vez por comando. Funciona, mas é desconfortável; por isso a preferência.
  if command -v sshpass >/dev/null 2>&1; then
    SSH_BIN=(sshpass -p "$GIOCHI_SENHA" ssh)
    SCP_BIN=(sshpass -p "$GIOCHI_SENHA" scp)
  fi
else
  SSH_OPTS+=(-o BatchMode=yes)
  SCP_OPTS+=(-o BatchMode=yes)
fi

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
  # ## Por que o aviso de host key precisa sair daqui
  #
  # `no_aparelho` junta stderr com stdout (`2>&1`), então o aviso de chave
  # diferente entra no MESMO fluxo que os dados do aparelho. Medido em
  # 09/11/2026: a imagem nova do R35S trocou a host key, e `resolucao_do_aparelho`
  # passou a devolver 12 linhas de aviso + `640x480`. O `case` do começo da
  # string deixava de casar e o sintoma era "não descobri a resolução do painel"
  # — com o painel respondendo certo. Qualquer função que faça parse sofre igual.
  #
  # O aviso em si continua útil no terminal: ele é do ssh, não deste script.
  # Quem quiser vê-lo fala com o aparelho direto (`ssh root@<ip>`), e o comando
  # certo para resolvê-lo é `ssh-keygen -R <ip>`.
  grep -vE "WARNING:|post-quantum|store now|openssh\.com/pq" \
    | grep -vE "^@{10,}$|IT IS POSSIBLE THAT SOMEONE|Someone could be eavesdropping|^The fingerprint for the|^Please contact your system administrator|^Add correct host key|^ *remove with: ssh-keygen|^Offending .* key in|^Password authentication is disabled|^Keyboard-interactive authentication is disabled|^UpdateHostkeys is disabled|^It is also possible that a host key" || true
}

# ── descoberta ──────────────────────────────────────────────

# O aparelho se identifica pelo hostname. Não usamos MAC nem fabricante porque
# nenhum dos dois é estável entre builds da imagem.
eh_o_aparelho() {
  local ip="$1"
  local nome
  nome=$(timeout 6 "${SSH_BIN[@]}" "${SSH_OPTS[@]}" "root@$ip" 'uname -n' 2>/dev/null | sem_ruido)
  case "$nome" in
    *giochi*) printf '%s' "$ip"; return 0 ;;
  esac
  return 1
}

# Varre a /24 em paralelo. Serial levaria minutos com 254 endereços e timeout.
procurar() {
  # ## "Não achei" e "achei mas não entrei" são diagnósticos diferentes
  #
  # Medido em 09/11/2026: a imagem nova do R35S trocou a host key **e** não
  # trouxe a nossa chave. O aparelho respondia na porta 22 em 0,5 ms, o `ssh`
  # devolvia `Permission denied (publickey,password)` e `procurar` dizia
  # "nenhum aparelho Giochi encontrado" — que manda conferir cabo e rede, onde
  # não havia nada de errado. Um IP que aceita conexão mas recusa autenticação
  # é a assinatura de chave faltando (ou host key obsoleta), e o remédio é
  # outro: instalar a chave por senha, e `ssh-keygen -R <ip>` se o aviso for
  # de chave trocada.
  #
  # Os recusados vão para `$RECUSADOS_ARQUIVO` (um arquivo), e não para um
  # array: `procurar` é chamado em `$(...)` e o que ele atribui não volta para
  # o pai. O caminho do arquivo vem do pai e é herdado pelo subshell.
  local recusados=${RECUSADOS_ARQUIVO:-}
  if [ -z "$recusados" ]; then
    recusados=$(mktemp)
  fi
  : > "$recusados"

  # Endereços conhecidos primeiro: o `usb0` tem IP fixo (10.55.0.2) e responde
  # em um segundo. Varrer a /24 inteira quando o aparelho está no cabo USB é
  # desperdício — e pior, não acha nada, porque o usb0 não está nessa faixa.
  local ip
  for ip in "${CANDIDATOS[@]}"; do
    if timeout 3 bash -c "echo > /dev/tcp/$ip/22" 2>/dev/null; then
      if eh_o_aparelho "$ip" >/dev/null 2>&1; then
        printf '%s\n' "$ip"
        return 0
      fi
      printf '%s\n' "$ip" >> "$recusados"
    fi
  done

  local encontrados
  encontrados=$(mktemp)
  local pids=()

  for i in $(seq 1 254); do
    (
      ip="$SUBREDE.$i"
      if timeout 1 bash -c "echo > /dev/tcp/$ip/22" 2>/dev/null; then
        if eh_o_aparelho "$ip" >/dev/null 2>&1; then
          printf '%s\n' "$ip" >> "$encontrados"
        else
          # Arquivo próprio e não a variável: isto roda em subshell.
          printf '%s\n' "$ip" >> "$encontrados.recusados"
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
    if [ -s "$encontrados.recusados" ]; then
      sort -u "$encontrados.recusados" >> "$recusados"
    fi
    rm -f "$encontrados.recusados"
    return 1
  fi
  rm -f "$encontrados.recusados"
  printf '%s\n' "$lista"
}

# Resolve o host: variável de ambiente, senão descoberta.
resolver_host() {
  if [ -n "${GIOCHI_DEVICE:-}" ]; then
    # `GIOCHI_DEVICE` é digitado das duas formas ("10.55.0.2" e
    # "root@10.55.0.2"); todo o resto do script já prefixa `root@`, então o
    # prefixo é removido aqui. Sem isto vira "root@root@10.55.0.2" e o ssh
    # falha com uma mensagem que não diz nada sobre a causa.
    printf '%s' "${GIOCHI_DEVICE#root@}"
    return 0
  fi
  local achado
  achado=$(procurar)
  [ -n "$achado" ] || return 1
  printf '%s' "$(printf '%s' "$achado" | head -1)"
}

# Executa no aparelho. Aborta com mensagem clara se a rede não responder.
#
# Os argumentos são unidos com **quebra de linha**, não com espaço. O `ssh` já
# junta argumentos com espaço, e isso colava o fim de um trecho no começo do
# seguinte: `"$TRECHO"'pids_launcher'` virava `}pids_launcher` e o shell remoto
# respondia `syntax error: unexpected end of file (expecting "}")`. Um script
# remoto é uma sequência de linhas; quem chama não deveria precisar lembrar de
# pôr o `\n` na concatenação.
no_aparelho() {
  local remoto="" parte
  for parte in "$@"; do
    remoto="$remoto$parte"$'\n'
  done
  "${SSH_BIN[@]}" "${SSH_OPTS[@]}" "root@$HOST" "$remoto" 2>&1 | sem_ruido
}

# Trecho remoto que define `matar_launcher`: derruba a interface e **espera o
# descritor de DRM** liberar.
#
# `kill` sozinho não serve para o launcher-teste, e isso já custou uma rodada:
# o SDL transforma SIGTERM em `SDL_QUIT` na fila de eventos, e o launcher-teste
# não bombeia eventos (a entrada por evdev não foi portada). O processo ficava
# vivo segurando o KMSDRM, que é exclusivo, e o app seguinte morria com
# `kmsdrm not available` — mensagem que manda procurar defeito no app.
#
# A varredura é por `/proc/<pid>/exe` e não por `pidof`: o `comm` do
# launcher-teste é truncado em 15 caracteres (`giochi-launcher`), então os dois
# nomes colidem — e o que interessa é qual binário está rodando.
#
# Quem decide se liberou é `card0`/`renderD` em `/proc/<pid>/fd`, não um
# `sleep` fixo: foi assim que o R35S revelou um launcher vivo **sem** os
# descritores, que nenhum `sleep` detectaria.
TRECHO_MATAR_LAUNCHER=$(cat <<'REMOTO'
pids_launcher() {
  for d in /proc/[0-9]*; do
    e=$(readlink -f "$d/exe" 2>/dev/null) || continue
    case "$e" in
      */giochi-launcher|*/giochi-launcher-teste) printf '%s ' "${d#/proc/}" ;;
    esac
  done
}

matar_launcher() {
  for p in $(pids_launcher); do kill -9 "$p" 2>/dev/null; done
  i=0
  while [ "$i" -lt 20 ]; do
    n=$(for d in /proc/[0-9]*; do ls -l "$d/fd" 2>/dev/null | grep -c 'card0\|renderD'; done | awk '{s+=$1} END {print s+0}')
    [ "$n" -eq 0 ] && return 0
    i=$((i + 1)); sleep 0.5
  done
  return 1
}
REMOTO
)
# `$(...)` come as quebras de linha finais, então o trecho terminaria em `}` e
# quem concatenasse `"$TRECHO"'comando'` produziria `}comando` — que o shell
# remoto recusa com "unexpected end of file (expecting \"}")". Devolver a
# quebra de linha aqui é o que permite concatenar sem pensar nisso.
TRECHO_MATAR_LAUNCHER+=$'\n'

exigir_aparelho() {
  HOST=$(resolver_host) || verdadeiro
  if [ -z "${HOST:-}" ]; then
    morrer "aparelho não encontrado em $SUBREDE.0/24 — está ligado? (GIOCHI_DEVICE=ip para fixar)"
  fi
  if ! timeout 10 "${SSH_BIN[@]}" "${SSH_OPTS[@]}" "root@$HOST" true 2>/dev/null; then
    morrer "sem SSH em $HOST"
  fi
  ok "aparelho em $HOST"
}

verdadeiro() { return 0; }

# Mostra por qual caminho o script tentaria falar com o aparelho.
#
# Existe porque "não conecta" tem causas muito diferentes — chave errada, senha
# errada, IP mudou, `BatchMode` impedindo o prompt — e cada uma dá a mesma
# mensagem de erro do ssh. Aqui a decisão fica visível antes da tentativa.
cmd_opcoes() {
  titulo "rota de acesso"
  local host
  host=$(resolver_host 2>/dev/null) || host=""
  printf '  host:        %s\n' "${host:-(não descoberto)}"
  printf '  GIOCHI_DEVICE: %s\n' "${GIOCHI_DEVICE:-(vazio)}"
  printf '  candidatos:  %s\n' "${CANDIDATOS[*]}"

  titulo "autenticação"
  if [ -n "${GIOCHI_SENHA:-}" ]; then
    if command -v sshpass >/dev/null 2>&1; then
      ok "senha via sshpass (BatchMode desligado)"
    else
      aviso "senha sem sshpass — o ssh vai PEDIR a senha em cada comando"
      printf '  instale sshpass, ou use chave: ssh-copy-id root@%s\n' "${host:-<ip>}"
    fi
  else
    ok "chave, sem prompt (BatchMode=yes)"
  fi

  titulo "comando que seria executado"
  printf '  %s %s %s uname -n\n' "${SSH_BIN[*]}" "${SSH_OPTS[*]}" "${host:+root@$host}"
}

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

  # Quando o sysfs diz 0, o `cmdline` é consultado — a **mesma** regra que o
  # launcher aplica (`display::rotacao_do_sistema_com_fonte`).
  #
  # Medido no X55 em 07/11/2026 (imagem `60e326f`): o sysfs respondia `0` e o
  # `cmdline` pedia `fbcon=rotate:3`. A ferramenta lia o sysfs, o app passou a
  # ler o cmdline, e os dois lados da comparação discordariam da orientação —
  # reprovando um desenho correto com "dimensões incompatíveis".
  if [ -z "$n" ] || [ "$n" = "0" ]; then
    local do_cmdline
    do_cmdline=$(no_aparelho "grep -o 'fbcon=rotate:[0-9]' /proc/cmdline 2>/dev/null | head -1 | cut -d: -f2" | tr -dc '0-9')
    if [ -n "$do_cmdline" ] && [ "$do_cmdline" != "0" ]; then
      printf '  aviso: fbcon/rotate diz 0 mas o cmdline pede fbcon=rotate:%s\n' "$do_cmdline" >&2
      n="$do_cmdline"
    fi
  fi

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
# Desenha a mesma Home no PC, sem tela.
#
# O tamanho é o do **painel do aparelho**, não 1280x720 fixo. O `set_logical_size`
# letterboxa igual nos dois lados, então gerar a referência no tamanho do painel
# faz a comparação ser 1:1 — antes, um painel 640x480 dava "dimensões
# incompatíveis" e o teste não dizia nada sobre o desenho.
#
#   gerar_referencia <destino.bmp> [LARGURAxALTURA]
gerar_referencia() {
  local destino="$1"
  local tamanho="${2:-}"
  local bin="$RAIZ/app/launcher/target/debug/giochi-launcher"
  # Mesmo prefixo que `compilar-launcher.sh` usa (`GIOCHI_SDL2_PREFIX`). Antes
  # apontava para `app/launcher/vendor/sdl2/lib`, que não existe — o binário
  # então não achava `libSDL2` em runtime e a referência saía vazia, sem
  # mensagem que dissesse por quê.
  local libs="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}/usr/lib/x86_64-linux-gnu"
  [ -x "$bin" ] || return 1
  [ -d "$libs" ] || return 1

  # A referência tem de estar no espaço de PROJETO, que é paisagem.
  #
  # `verificar-frame.mjs` des-rotaciona a captura e compara no espaço de projeto
  # (ver `comparar`). Num painel retrato (X55: 720×1280, `rotate=3`) a captura é
  # retrato, e depois de des-rotacionada fica 1280×720 — então a referência
  # precisa ser gerada em 1280×720, não no modo do painel.
  #
  # Gerar no modo do painel dava "dimensões incompatíveis: captura 1280x720 vs
  # referência 720x1280" e o launcher levava a culpa por uma conta da ferramenta.
  # O launcher desenha em paisagem nos dois casos: `projecao()` troca as medidas
  # quando há giro.
  if [ -n "$tamanho" ]; then
    local l a
    l="${tamanho%x*}"
    a="${tamanho#*x}"
    if [ "$a" -gt "$l" ] 2>/dev/null; then
      tamanho="${a}x${l}"
    fi
  fi

  rm -f "$destino"
  local args=(--dump-frame "$destino")
  [ -n "$tamanho" ] && args+=(--tamanho "$tamanho")
  # O catálogo entra na referência.
  #
  # Sem isto os dois lados desenham catálogos diferentes: o aparelho usa
  # `/opt/giochi/apps/index.json` e o PC usava o catálogo de demonstração do
  # binário. Medido no X35H em 04/11/2026: a comparação caiu de 99,4% para
  # 97,8% **sem nenhuma regressão de desenho** — só porque a Home do aparelho
  # tinha um jogo e a do PC tinha quatro. Comparação só vale com a mesma
  # entrada dos dois lados.
  local catalogo_pc=""
  if [ -n "${HOST:-}" ]; then
    catalogo_pc="$destino.catalogo.json"
    "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q \
      "root@$HOST:/opt/giochi/apps/index.json" "$catalogo_pc" 2>/dev/null || catalogo_pc=""
  fi
  # O `dir` do catálogo é caminho do APARELHO, e no PC ele não existe.
  #
  # O launcher poda em execução o app cujo `entrypoint` não está no disco
  # (`podar_sem_binario`, feita para cartão removido) — então a referência
  # desenhava uma grade VAZIA enquanto o aparelho desenhava o cartão. Medido no
  # R35S em 07/11/2026: 99,6% sem catálogo e **36,5%** logo depois de publicar
  # um único app. A queda era da entrada, não do desenho — exatamente o que
  # esta função existe para evitar.
  #
  # A correção é dar ao PC o **mesmo conteúdo** com caminhos que ele resolve:
  # cada `dir` vira um diretório temporário com o `entrypoint` presente. O
  # cartão só usa os campos do catálogo, então o conteúdo do arquivo é
  # irrelevante — o que importa é ele existir.
  if [ -n "$catalogo_pc" ] && [ -s "$catalogo_pc" ]; then
    local catalogo_local="$destino.catalogo-pc.json"
    if node -e '
      const fs = require("fs"), path = require("path");
      const [src, dst, base] = process.argv.slice(1);
      const j = JSON.parse(fs.readFileSync(src, "utf8"));
      for (const a of j.apps) {
        const dir = path.join(base, a.id);
        const ep = path.join(dir, a.entrypoint || "bin/app");
        fs.mkdirSync(path.dirname(ep), { recursive: true });
        fs.writeFileSync(ep, "#!/bin/sh\nexit 0\n", { mode: 0o755 });
        a.dir = dir;
      }
      fs.writeFileSync(dst, JSON.stringify(j, null, 2));
    ' "$catalogo_pc" "$catalogo_local" "$destino.jogos" 2>/dev/null; then
      catalogo_pc="$catalogo_local"
    else
      falha "não consegui preparar o catálogo para o PC — a comparação usaria caminhos do aparelho"
      return 1
    fi
  fi
  # `env` explícito em vez de `VAR=... VAR=... comando`.
  #
  # A forma anterior usava `${env_catalogo[@]+"${env_catalogo[@]}"}` no meio da
  # lista de atribuições, e o bash tratou `SDL_VIDEODRIVER=dummy` como **comando
  # separado**: a referência era gerada sem o driver dummy, o SDL não abria
  # janela e o script anunciava "não consegui gerar a referência" sem que nada
  # estivesse errado com o launcher. Com `env`, cada variável é argumento e não
  # há ambiguidade.
  local ambiente=(
    env
    SDL_VIDEODRIVER=dummy
    # Relógio congelado nos DOIS lados. Sem isto, o aparelho (sem NTP) e o PC
    # divergem nos quatro dígitos da barra superior e a comparação acusa 99,9 %
    # em todas as telas — medido no X35H em 09/11/2026, com 205 px diferentes
    # exatamente em x 482..517, y 26..34. Ver `current_clock` em views.rs.
    GIOCHI_HORA_FIXA=12:00
    GIOCHI_BATERIA_FIXA=50
    GIOCHI_TEMPERATURA_FIXA=40
    "LD_LIBRARY_PATH=${libs}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  )
  if [ -n "$catalogo_pc" ] && [ -s "$catalogo_pc" ]; then
    ambiente+=("GIOCHI_CATALOGO=$catalogo_pc")
  fi
  # A referência tem de ter o **mesmo texto** que o aparelho desenha.
  #
  # O `--dump-frame` aceita qualquer build, inclusive um compilado com
  # `--no-default-features` (sem a feature `texto`). Esse build desenha a tela
  # inteira e nenhum glifo: a referência sai sem texto, e a comparação cai de
  # ~99% para ~96% sem nenhuma regressão de desenho. Medido em 07/11/2026 — e o
  # único sinal era um aviso em stderr que este `>/dev/null` engolia.
  #
  # Deixar o aviso virar erro aqui é o que impede a próxima sessão de caçar um
  # bug de layout que não existe.
  local saida_erro
  saida_erro=$(timeout 60 "${ambiente[@]}" "$bin" "${args[@]}" 2>&1 >/dev/null)
  case "$saida_erro" in
    *"sem a feature"*)
      falha "o binário do PC foi compilado SEM texto — a referência ficaria sem glifos"
      printf '  rebuilde com: bash scripts/compilar-launcher.sh\n' >&2
      return 1
      ;;
  esac
  # O catálogo baixado é temporário: fica ao lado do BMP, para quem for
  # investigar, e não é reutilizado na próxima rodada (o aparelho pode ter
  # mudado).
  [ -s "$destino" ]
}

# Resolução que o painel declara, como `LARGURAxALTURA`. Vazio se não der.
resolucao_do_aparelho() {
  local modo
  modo=$(no_aparelho 'cat /sys/class/drm/card0-DSI-1/modes 2>/dev/null | head -1' | tr -d '\r')
  case "$modo" in
    [0-9]*x[0-9]*) printf '%s' "$modo" ;;
    *) return 1 ;;
  esac
}

# PID do `giochi-init` neste aparelho, ou vazio se ele não está rodando.
#
# Lido de `/proc/[0-9]*/cmdline`, e não de `pidof` nem de `ps`:
#   - `pidof` não acha script `sh`, e o `giochi-init` é um;
#   - o `ps` desta imagem lista **só a própria TTY**. Medido no R35S em
#     06/11/2026: 94 diretórios em `/proc` e 2 linhas no `ps`, com o `init`, o
#     `sshd` e o launcher invisíveis. "Não aparece no ps" não é "não existe".
pid_do_giochi_init() {
  # Comparação **exata** do cmdline inteiro, e não `*" /usr/bin/giochi-init"*`:
  # o `sh -c` que executa este próprio comando carrega a string no argv e casava
  # consigo mesmo. Medido: o pid devolvido era o do `ssh`, não o do supervisor —
  # e a ferramenta então "esperava" o processo errado e concluía que o launcher
  # não voltou.
  no_aparelho 'for p in /proc/[0-9]*; do c=$(tr "\0" " " < $p/cmdline 2>/dev/null); case "$c" in "/bin/sh /usr/bin/giochi-init "|"/bin/sh /usr/bin/giochi-init"|"sh /usr/bin/giochi-init "|"sh /usr/bin/giochi-init") echo ${p#/proc/}; break;; esac; done' | tr -d '\r'
}

# Sobe o launcher e imprime pid + log. Separado de `cmd_subir` porque `verificar`
# precisa retomar o app depois de capturar o frame.
#
# ## Por que este script não sobe o launcher quando há `giochi-init`
#
# Porque os dois disputariam o **mesmo** DRM, que é exclusivo. Medido no R35S em
# 06/11/2026: `verificar` parava o launcher, capturava o frame e tentava retomar
# com `setsid nohup`; o `giochi-init` reiniciava o dele mais rápido, o nosso
# perdia a corrida e imprimia `Error: "kmsdrm not available"` — que parece falha
# do retomar, quando o retomar tinha acontecido, só que por outra mão (o
# sobrevivente tinha ppid 159 = `giochi-init`, não 1).
#
# Quando há supervisor, esperar é a única atitude honesta. Quando não há (imagens
# antigas, ou init parado), o `nohup` continua sendo o único caminho.
# Espera o launcher supervisionado aparecer e devolve `pid=… ppid=… drm=…`,
# ou `MORREU` se ele não vier.
esperar_launcher_supervisionado() {
  local tentativas="${1:-60}"
  # Devolve tres coisas diferentes, e a diferenca importa:
  #
  #   `pid=... drm=N`  o launcher existe e abriu o hardware  → interface
  #   `MORREU`         nao ha launcher nenhum                → supervisor desistiu
  #   `... + SEM_DRM`  existe e NAO abriu o hardware         → tela preta
  #
  # Os dois ultimos deixam o aparelho sem desenho. Ate esta correcao o
  # `SEM_DRM` era apenas um aviso e a funcao devolvia sucesso, entao o caminho
  # de recuperacao do RESTART_MAX nunca rodava e o script anunciava
  # "supervisor: giochi-init — nao iniciado por este script" com a tela preta.
  #
  # `processo vivo` nao e `abriu o hardware`: o descritor de DRM e a prova.
  no_aparelho "$TRECHO_MATAR_LAUNCHER""
    i=0
    while [ \$i -lt $tentativas ]; do
      pid=\$(pids_launcher)
      [ -n \"\$pid\" ] && break
      i=\$((i+1)); sleep 0.5
    done
    if [ -z \"\$pid\" ]; then
      echo MORREU
      exit 0
    fi
    # Pode haver **mais de um** launcher vivo (um travado de uma rodada
    # anterior, reparentado para o pid 1, e o novo do supervisor). Medir o
    # descritor do primeiro da lista dava `drm=0` com outro desenhando a tela —
    # e o script anunciava tela preta com a tela funcionando. Aqui se procura o
    # primeiro que **abriu** o hardware.
    escolhido=
    d=0; i=0
    while [ \$i -lt 20 ]; do
      for p in \$pid; do
        n=\$(ls -l /proc/\$p/fd 2>/dev/null | grep -c 'card0\|renderD')
        if [ \"\$n\" -gt 0 ]; then escolhido=\$p; d=\$n; break; fi
      done
      [ -n \"\$escolhido\" ] && break
      i=\$((i+1)); sleep 0.5
    done
    if [ -z \"\$escolhido\" ]; then
      escolhido=\$(echo \$pid | cut -d' ' -f1)
      echo \"pid=\$escolhido drm=0 (de \$(echo \$pid | wc -w) processo(s))\"
      echo SEM_DRM
      exit 0
    fi
    echo \"pid=\$escolhido ppid=\$(cut -d' ' -f4 /proc/\$escolhido/stat 2>/dev/null) drm=\$d\"
    exit 0
  "
}

iniciar_launcher() {
  local init_pid saida preciso_reiniciar=0
  # As **duas** flags de uma captura anterior impedem o supervisor de subir o
  # launcher para sempre: `dev-free-console` diz "não suba", e `launcher-parado`
  # é o marcador de que ele já desistiu. Limpar só a primeira deixava o aparelho
  # sem interface depois de cinco capturas.
  no_aparelho 'rm -f /run/giochi/dev-free-console /run/giochi/launcher-parado' || true
  init_pid=$(pid_do_giochi_init)
  if [ -n "$init_pid" ]; then
    saida=$(esperar_launcher_supervisionado 60)
    # `MORREU` é supervisor sem launcher; `SEM_DRM` é launcher vivo que não abriu
    # o hardware. Os dois deixam a tela preta e os dois se recuperam do mesmo
    # jeito: reiniciar o supervisor, que o inittab respawna com a cota zerada.
    case "$saida" in
      MORREU | *SEM_DRM*) preciso_reiniciar=1 ;;
    esac
    if [ "$preciso_reiniciar" = "1" ]; then
      # ## Por que o launcher às vezes não volta
      #
      # O `giochi-init` desiste: `RESTART_MAX=5` quedas dentro de
      # `RESTART_WINDOW` e ele para de reiniciar — fica vivo dormindo, para o
      # kernel não ficar sem init. Cada `verificar` derruba o launcher uma vez,
      # então **seis `verificar` seguidos deixam o aparelho sem interface** até
      # alguém reiniciar o supervisor ou o aparelho.
      #
      # Matar o `giochi-init` devolve a cota: o `::respawn:` do inittab o
      # reinicia com `restarts=0`. É por isso que essa segunda chance existe em
      # vez de uma mensagem de erro.
      case "$saida" in
        MORREU) aviso "o giochi-init (pid $init_pid) está sem launcher vivo — provável RESTART_MAX" ;;
        *)      aviso "o launcher está vivo mas sem DRM: não desenha (tela preta)" ;;
      esac
      printf '  reiniciando o supervisor (o inittab o respawna com a cota zerada)\n'
      # `matar_launcher` junto: o processo travado pode ter sido reparentado para
      # o pid 1 e continuar vivo (medido: um launcher em estado `D`, dormindo em
      # I/O, com `ppid=1`). Só matar o supervisor deixaria o zumbi no caminho.
      no_aparelho "$TRECHO_MATAR_LAUNCHER""
        kill $init_pid 2>/dev/null
        sleep 2
        matar_launcher >/dev/null 2>&1
        rm -f /run/giochi/launcher-parado
        exit 0
      "
      init_pid=$(pid_do_giochi_init)
      if [ -z "$init_pid" ]; then
        echo MORREU
        echo 'o giochi-init não voltou depois de reiniciado'
        return 0
      fi
      saida=$(esperar_launcher_supervisionado 120)
      case "$saida" in
        MORREU)
          echo MORREU
          echo "nem com o giochi-init novo (pid $init_pid) o launcher subiu em 60s"
          return 0
          ;;
        *SEM_DRM*)
          printf '%s\n' "$saida"
          printf '  FALHA o launcher voltou mas não abriu o DRM — a tela continua preta\n'
          return 0
          ;;
      esac
      printf '%s\n' "$saida"
      printf 'supervisor: giochi-init (pid %s) — reiniciado por este script\n' "$init_pid"
      return 0
    fi
    printf '%s\n' "$saida"
    printf 'supervisor: giochi-init (pid %s) — não iniciado por este script\n' "$init_pid"
    return 0
  fi

  no_aparelho "$TRECHO_MATAR_LAUNCHER""
    rm -f $LOG
    setsid nohup env SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software \
      $DESTINO_BIN > $LOG 2>&1 < /dev/null &
    sleep 5
    pid=\$(pids_launcher)
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
  printf '  candidatos fixos tentados antes da varredura: %s\n' "${CANDIDATOS[*]}"
  # O arquivo é criado **aqui**, no processo pai, e o caminho é herdado pelo
  # subshell de `procurar`. Sem isso a lista de recusados não volta.
  RECUSADOS_ARQUIVO=$(mktemp)
  : > "$RECUSADOS_ARQUIVO"
  local lista
  if ! lista=$(procurar); then
    RECUSADOS=()
    while IFS= read -r ip_recusado; do
      [ -n "$ip_recusado" ] && RECUSADOS+=("$ip_recusado")
    done < "$RECUSADOS_ARQUIVO"
    rm -f "$RECUSADOS_ARQUIVO"
    # A distinção importa: mandar conferir o cabo quando o aparelho está no
    # cabo custa uma sessão inteira de diagnóstico no lugar errado.
    if [ "${#RECUSADOS[@]}" -gt 0 ]; then
      falha "aparelho responde em ${RECUSADOS[*]}, mas o SSH recusou a autenticação"
      printf '  isso é chave faltando ou host key obsoleta, NÃO cabo nem rede\n'
      printf '  dicas:\n'
      printf '    - chave nova na imagem? entre por senha e instale a sua:\n'
      printf '        GIOCHI_DEVICE=%s GIOCHI_SENHA=<senha> bash scripts/aparelho.sh procurar\n' "${RECUSADOS[0]}"
      printf '    - aviso de "REMOTE HOST IDENTIFICATION HAS CHANGED"? é a imagem regravada:\n'
      printf '        ssh-keygen -R %s\n' "${RECUSADOS[0]}"
      exit 1
    fi
    falha "nenhum aparelho Giochi encontrado"
    printf '  dicas:\n'
    printf '    - está ligado? mesma rede Wi-Fi? GIOCHI_DEVICE=ip para fixar\n'
    printf '    - por USB (usb0), o aparelho fica em 10.55.0.2 e autentica por senha:\n'
    printf '        GIOCHI_DEVICE=10.55.0.2 GIOCHI_SENHA=<senha> bash scripts/aparelho.sh procurar\n'
    exit 1
  fi
  rm -f "$RECUSADOS_ARQUIVO"
  printf '%s\n' "$lista" | while read -r ip; do
    printf '  %s %s\n' "$(printf '%s' "$VERDE")$ip$(printf '%s' "$ZERA")" "$("${SSH_BIN[@]}" "${SSH_OPTS[@]}" "root@$ip" 'uname -n' 2>/dev/null | sem_ruido)"
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
  no_aparelho "$TRECHO_MATAR_LAUNCHER"'
    p=$(pids_launcher)
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
  #
  # Com `giochi-init` rodando, matar o launcher é só pedir um reinício a ele —
  # e é o `giochi-init` quem traz o novo, não este script. Sem supervisor, o
  # kill limpa uma instância manual anterior.
  local init_pid
  init_pid=$(pid_do_giochi_init)
  if [ -z "$init_pid" ]; then
    no_aparelho "$TRECHO_MATAR_LAUNCHER"'p=$(pids_launcher); [ -n "$p" ] && kill $p 2>/dev/null; sleep 1; true'
  fi

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
  local saida init_pid
  init_pid=$(pid_do_giochi_init)
  saida=$(no_aparelho "$TRECHO_MATAR_LAUNCHER"'p=$(pids_launcher); if [ -z "$p" ]; then echo NADA; else kill $p 2>/dev/null; sleep 2; echo "PARADO $p"; fi')
  case "$saida" in
    NADA)    aviso "já não estava rodando" ;;
    PARADO*) ok "parado (${saida#PARADO })" ;;
    *)       falha "$saida" ;;
  esac

  # Com `giochi-init` rodando, "parado" é uma meia-verdade: ele reinicia o
  # launcher sozinho. Dizer isso é o que evita a próxima sessão caçar um
  # processo zumbi que na verdade é um respawn funcionando.
  if [ -n "$init_pid" ]; then
    local agora
    agora=$(no_aparelho "$TRECHO_MATAR_LAUNCHER"'pids_launcher' | tr -d '\r')
    if [ -n "$agora" ]; then
      aviso "o launcher voltou como $agora — giochi-init (pid $init_pid) o reinicia"
      printf '  para parar de verdade: mate o giochi-init, ou desligue o aparelho\n'
    fi
  fi
}

cmd_log() {
  exigir_aparelho
  titulo "log do launcher ($LOG)"
  no_aparelho "if [ -f $LOG ]; then tail -40 $LOG; else echo '(sem log — o app nunca subiu nesta sessão)'; fi"
}

# Gera o catálogo do aparelho a partir dos metadados e envia.
#
# ## Por que este subcomando existe
#
# Até 04/11/2026 o `/opt/giochi/apps/index.json` era escrito **à mão**. Instalei
# `cobrinha` e `pacman` e o aparelho continuou mostrando **um** jogo até eu
# reescrever o JSON — e um app instalado fora dele fica invisível, sem erro em
# lugar nenhum: nem no app, nem no log, nem na tela.
#
# Agora o catálogo é **projeção** de `app/apps.json` + os `metadata.json`. Quem
# gera é `scripts/gerar-catalogo.mjs`, que também diz o que ficou de fora e por
# quê — a parte que faltava para "instalei e não apareceu" deixar de ser mistério.
#
# Ordem: gera primeiro, envia depois. Se a geração falhar, o aparelho continua
# com o catálogo anterior em vez de ficar sem nenhum.
cmd_catalogo() {
  exigir_aparelho
  titulo "catálogo"

  local temp
  temp=$(mktemp "${TMPDIR:-/tmp}/giochi-catalogo.XXXXXX.json")
  local instalados="$temp.instalados.json"

  # O aparelho é a única fonte que não pode mentir sobre o que está instalado:
  # um pacote da loja não existe no repositório. Sem esta colheita, instalar um
  # jogo pela loja o deixaria fora do catálogo e invisível **sem erro**.
  if colher_instalados "$instalados" 2>&1 | sed 's/^/  /'; then
    :
  else
    aviso "não consegui ler os apps instalados; o catálogo terá só os do repositório"
    printf '[]\n' > "$instalados"
  fi

  # `--lista` vai para stderr do gerador; aqui interessa o que ele decide.
  if ! node "$RAIZ/scripts/gerar-catalogo.mjs" --saida "$temp" --instalados "$instalados" 2>"$temp.avisos"; then
    falha "não consegui gerar o catálogo"
    sed 's/^/  /' "$temp.avisos" >&2 2>/dev/null
    rm -f "$temp" "$temp.avisos" "$instalados"
    return 1
  fi

  local quantos
  quantos=$(node -e "const d=require('$temp');console.log(d.apps.length)" 2>/dev/null || echo '?')
  ok "gerado: $quantos app(s)"
  # Os avisos são o que responde "por que meu app não aparece".
  sed 's/^/  /' "$temp.avisos" 2>/dev/null
  rm -f "$temp.avisos" "$instalados"

  # ── conferência NO APARELHO ────────────────────────────────────────────
  #
  # O gerador checa o binário na árvore do repositório. Quem importa é o
  # aparelho: um jogo presente no repo e **não instalado** entraria no catálogo
  # e viraria cartão morto — o usuário aperta e nada acontece, sem mensagem.
  #
  # É a mesma regra que já vale do lado do repo ("catálogo apontando para
  # binário ausente é pior que catálogo vazio"), aplicada onde ela decide.
  local pares
  pares=$(node -e "
    const d = require('$temp');
    for (const a of d.apps) console.log([a.id, a.dir, a.entrypoint].join('|'));
  " 2>/dev/null)

  local faltando="" linha
  while IFS='|' read -r _id dir entry; do
    [ -n "${entry:-}" ] || continue
    if ! no_aparelho "[ -x '$dir/$entry' ] && echo SIM" | grep -q SIM; then
      faltando="$faltando $_id"
    fi
  done <<< "$pares"

  if [ -n "$faltando" ]; then
    aviso "não instalado(s) no aparelho:$faltando"
    printf '  esses saem do catálogo — cartão que abre nada é pior que cartão ausente\n'
    printf '  instale com: bash scripts/compilar-jogo-arm64.sh --jogo <nome> --instalar %s\n' "$HOST"
    node -e "
      const fs = require('fs');
      const p = '$temp';
      const d = JSON.parse(fs.readFileSync(p, 'utf8'));
      const fora = new Set('$faltando'.trim().split(/\\s+/));
      d.apps = d.apps.filter((a) => !fora.has(a.id));
      fs.writeFileSync(p, JSON.stringify(d, null, 2) + '\n');
      console.error('  catálogo reduzido para ' + d.apps.length + ' app(s)');
    " || { falha "não consegui filtrar o catálogo"; rm -f "$temp"; return 1; }
  fi

  local destino="/opt/giochi/apps/index.json"
  no_aparelho "mkdir -p /opt/giochi/apps" >/dev/null 2>&1

  # Envia para um temporário e move: um `scp` interrompido no meio deixaria
  # JSON truncado no lugar do catálogo bom, e o launcher leria lixo.
  if "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "$temp" "root@$HOST:$destino.novo" 2>/dev/null &&
     no_aparelho "chmod 644 $destino.novo && mv $destino.novo $destino && echo MOVIDO" |
       grep -q MOVIDO; then
    ok "publicado em $destino"
  else
    falha "não consegui publicar em $destino"
    rm -f "$temp"
    return 1
  fi
  rm -f "$temp"

  # Conferência pelo lado do aparelho: quantos apps o launcher vai ver.
  local visto
  visto=$(no_aparelho "grep -c '\"id\"' $destino 2>/dev/null || echo 0")
  case "$visto" in
    0) aviso "catálogo vazio no aparelho — nenhum app vai aparecer na tela" ;;
    *) ok "$visto app(s) no catálogo do aparelho" ;;
  esac

  printf '\n  o launcher lê o catálogo na abertura. Se ele já está rodando e você\n'
  printf '  quer ver a lista nova: bash scripts/aparelho.sh parar && bash scripts/aparelho.sh subir\n'
}

# Traz o log de boot do aparelho.
#
# **Regravar o cartão apaga `/root/giochi-boot.log`** — o mesmo cartão serve os
# dois lados. Um boot que deu errado só é diagnosticável se o log for lido antes
# da regravação, e confiar na memória para isso é frágil: o momento de regravar é
# justamente quando se quer "resolver logo". Ter o comando reduz a chance de
# perder a evidência por esquecimento.
#
# Guarda em `dist/`, que é ignorado pelo git, com a data no nome para que dois
# boots ruins não se sobrescrevam.
# ── pacotes da loja ────────────────────────────────────────────────────

#
# Instala um `.giochi` no aparelho.
#
# ## O que é verificado, e onde
#
# No **PC**, antes de enviar: estrutura (ordem dos membros, entrypoint
# executável), SHA-256 e assinatura Ed25519. O PC é a máquina de quem publica, e
# um pacote corrompido não deve nem atravessar o cabo.
#
# No **aparelho**, depois de extrair: o `entrypoint` existe e é executável. É a
# mesma checagem que `catalogo` faz, e é a que pega o erro que só aparece lá —
# permissão perdida na cópia, arquitetura errada, dependência ausente.
#
# ## Por que o catálogo é republicado no fim
#
# Um pacote instalado e não catalogado é **invisível sem erro**: o arquivo está
# no disco, o usuário não vê o cartão. Foi exatamente o que aconteceu quando o
# `index.json` era escrito à mão (1 jogo na tela, 3 no disco).
cmd_instalar_pacote() {
  exigir_aparelho
  local arquivo="$1"
  if [ -z "$arquivo" ]; then
    falha "informe o pacote: aparelho.sh instalar-pacote <arquivo.giochi>"
    return 1
  fi
  if [ ! -f "$arquivo" ]; then
    falha "pacote não existe: $arquivo"
    return 1
  fi

  titulo "verificando $(basename "$arquivo")"

  # A chave de verificação é a **do aparelho**, não uma do PC.
  #
  # Quem decide se um pacote é aceito é o aparelho, e ele verifica com
  # `/etc/giochi/loja-publica.pem` (o caminho que o `Giochi System` versiona). O
  # PC tem a sua própria chave, de outra geração, e usá-la aqui recusa todo
  # pacote assinado com a chave da loja: medido em 07/11/2026 no R35S —
  # `FALHA: assinatura Ed25519 não confere` num pacote que o próprio aparelho
  # aprovava (`OK assinatura confere`). Divergência silenciosa entre os dois
  # lados, e o sintoma aparecia como "pacote corrompido".
  #
  # Sem chave no aparelho, o script diz que não conseguiu verificar em vez de
  # usar uma chave qualquer — verificar contra a chave errada é pior que não
  # verificar, porque produz um veredito.
  local chave_aparelho
  chave_aparelho="$(mktemp -d)/loja-publica.pem"
  local chave_para_verificar=()
  if no_aparelho 'test -f /etc/giochi/loja-publica.pem && echo SIM' | grep -q SIM; then
    if "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "root@$HOST:/etc/giochi/loja-publica.pem" "$chave_aparelho" 2>/dev/null; then
      ok "chave de verificação: a do aparelho (/etc/giochi/loja-publica.pem)"
      chave_para_verificar=(--chave-publica "$chave_aparelho")
    fi
  fi
  if [ "${#chave_para_verificar[@]}" -eq 0 ]; then
    aviso "o aparelho não tem /etc/giochi/loja-publica.pem"
    aviso "verificando com a chave do PC — o aparelho NÃO vai conferir a assinatura"
  fi

  if ! node "$RAIZ/scripts/verificar-pacote.mjs" "$arquivo" "${chave_para_verificar[@]}" | sed 's/^/  /'; then
    falha "pacote recusado — não foi enviado"
    return 1
  fi

  # O slug vem do próprio pacote, não do nome do arquivo: renomear o arquivo não
  # pode mudar onde ele é instalado.
  local meta
  meta=$(node -e "
    const { readFileSync } = require('fs');
    const m = require('$RAIZ/scripts/lib/publicar.mjs');
    const p = readFileSync('$arquivo');
    const meta = [...m.lerTar(p)].find((x) => x.caminho === 'metadata.json');
    const j = JSON.parse(meta.conteudo.toString('utf8'));
    console.log(JSON.stringify(j));
  " 2>/dev/null)
  if [ -z "$meta" ]; then
    falha "não consegui ler o metadata.json do pacote"
    return 1
  fi

  local slug destino
  slug=$(printf '%s' "$meta" | node -e "
    let s=''; process.stdin.on('data',d=>s+=d).on('end',()=>{
      const j = JSON.parse(s);
      // Sem slug no metadata, o título vira slug: é o que o usuário veria.
      const base = (j.slug || j.title || '').toString();
      console.log(base.toLowerCase().normalize('NFD').replace(/[\\u0300-\\u036f]/g,'')
        .replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'') || 'app');
    })")
  # App da loja vai para `/user`, **não** para `/opt`.
  #
  # O contrato em `app/apps.json`/`AGENTS.md` diz: `distribution: store` é
  # instalado em `/user` pela loja e **não faz parte da imagem assinada**. Isso
  # não é organização de pastas — é o que permite vender app GPL: o pacote
  # baixado fica fora da imagem, o comprador pode substituí-lo, e a GPL-3.0 §6
  # se resolve sem entregar chave nenhuma.
  #
  # A primeira versão instalava em `/opt/giochi/apps`, que é o mesmo lugar dos
  # apps da imagem. Funcionava, e destruía exatamente o argumento que justifica
  # a loja existir neste desenho.
  destino="/user/apps/$slug"

  titulo "instalando em $destino"
  local temp
  temp=$(mktemp "${TMPDIR:-/tmp}/giochi-pkg.XXXXXX.giochi")
  cp "$arquivo" "$temp"

  if ! "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "$temp" "root@$HOST:/tmp/pacote.giochi" 2>/dev/null; then
    falha "não consegui enviar o pacote"
    rm -f "$temp"
    return 1
  fi
  rm -f "$temp"

  # O extrator é um script **do aparelho**, copiado junto com o pacote.
  #
  # A primeira versão montava um `sh -c` com aspas dentro de aspas dentro de
  # `ssh`, e o shell comeu o `$(...)`: `sh: "bin/giochi-pacman": unknown
  # operand`. Copiar um arquivo custa um `scp` e elimina a classe.
  if ! "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "$RAIZ/scripts/lib/extrair-pacote.sh" \
       "root@$HOST:/tmp/extrair-pacote.sh" 2>/dev/null; then
    falha "não consegui enviar o extrator"
    return 1
  fi

  local saida
  saida=$(no_aparelho "sh /tmp/extrair-pacote.sh /tmp/pacote.giochi '$destino'; rm -f /tmp/extrair-pacote.sh" 2>&1)

  case "$saida" in
    OK*)
      ok "extraído: ${saida#OK }"
      ;;
    ERRO*)
      falha "${saida#ERRO }"
      no_aparelho "rm -f /tmp/pacote.giochi" >/dev/null 2>&1
      return 1
      ;;
    *)
      falha "instalação falhou: $saida"
      return 1
      ;;
  esac

  # Republica o catálogo incluindo o que está instalado no aparelho.
  cmd_catalogo >/dev/null 2>&1 || { aviso "instalado, mas o catálogo não foi republicado"; return 1; }
  ok "catálogo republicado"

  printf '\n  o launcher lê o catálogo na abertura:\n'
  printf '  bash scripts/aparelho.sh parar && bash scripts/aparelho.sh subir\n'
}

#
# Instala os `.giochi` que estiverem na partição do cartão.
#
# ## O que descobriu-se medindo o X55 (06/11/2026)
#
# O aparelho **não tem eMMC**: o sistema roda do próprio cartão SD
# (`mmcblk1`, "SD128", 128 GB). A tabela de partições usa ~1,9 GiB:
#
#     mmcblk1p1  256 MiB  FAT32  GIOCHI_BOOT
#     mmcblk1p2  1,7 GiB  ext4   rootfs   ← o sistema em execução
#     (resto)   116,5 GiB NÃO ALOCADO
#
# E a `GIOCHI_BOOT` **não é montada** em lugar nenhum (nem no `fstab`). Como ela
# é FAT32, qualquer PC a lê — é o caminho natural para levar um jogo sem SSH.
# Montar aqui é medida de app porque o sistema não a monta: mesmo motivo do
# `ip link set up` no Wi-Fi, que o `S40wifi` também não faz.
#
# Montagem **somente leitura**: o app não tem o que escrever no cartão, e um
# `rw` num FAT32 com o sistema rodando do mesmo cartão é risco sem retorno.
cmd_instalar_do_cartao() {
  exigir_aparelho
  titulo "instalando do cartão"

  # Descobre os cartões **sem nome de dispositivo**.
  #
  # A primeira versão usava `/dev/mmcblk1p1` fixo, que é o que o X55 tem. O dono
  # avisou que alguns aparelhos têm dois slots de SD e ainda eMMC, com o sistema
  # num cartão e os jogos no outro — e casos de jogos nos dois. Nome fixo acerta
  # num aparelho e mente no seguinte, sem avisar.
  if ! "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q \
       "$RAIZ/scripts/lib/descobrir-cartoes.sh" "root@$HOST:/tmp/descobrir-cartoes.sh" 2>/dev/null; then
    falha "não consegui enviar o descobridor de cartões"
    return 1
  fi

  # `CARTOES_PADROES` atravessa para o aparelho quando definida aqui: é a única
  # forma de exercitar o caso de **dois cartões** sem hardware com dois slots.
  # No uso normal não é definida e a descoberta usa os dispositivos reais.
  local pontos
  pontos=$(no_aparelho "${CARTOES_PADROES:+CARTOES_PADROES=\"$CARTOES_PADROES\" }sh /tmp/descobrir-cartoes.sh /mnt/cartao 2>/dev/null" | sed -n 's/^PONTO=//p')

  if [ -z "$pontos" ]; then
    falha "nenhum cartão de jogos encontrado"
    printf '  o cartão do sistema é ignorado de propósito (senão o app montaria\n'
    printf '  o próprio rootfs como se fosse mídia removível).\n'
    printf '  se você espera um cartão aqui, confira se ele está inserido e\n'
    printf '  formatado (FAT32 ou ext4).\n'
    return 1
  fi

  ok "$(printf '%s\n' "$pontos" | wc -l | tr -d ' ') cartão(ões): $(echo $pontos)"

  # Envia o instalador: roda no aparelho, sem Node e sem PC no meio.
  if ! "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q \
       "$RAIZ/scripts/lib/instalar-do-cartao.sh" "root@$HOST:/tmp/instalar-do-cartao.sh" 2>/dev/null; then
    falha "não consegui enviar o instalador"
    return 1
  fi

  # Instala de **todos** os cartões. Jogos podem estar nos dois.
  local ponto total_ok=0 total_falhou=0 houve_pacote=0
  for ponto in $pontos; do
    local conteudo
    conteudo=$(no_aparelho "ls -1 $ponto/*.giochi 2>/dev/null | wc -l" 2>/dev/null | tr -d ' \r')
    if [ "${conteudo:-0}" = "0" ]; then
      printf '  %s: nenhum .giochi\n' "$ponto"
      continue
    fi
    houve_pacote=1
    printf '  %s: %s pacote(s)\n' "$ponto" "$conteudo"

    # O código de saída vem **dentro** da saída, não de `$?`.
    #
    # A primeira versão terminava o comando remoto com `; rm -f ...`, e o `rm`
    # (que sempre dá 0) engolia o status do instalador: uma execução com pacote
    # recusado reportava "ok pronto". O texto dizia "1 recusado" e o resultado
    # dizia sucesso — a contradição que faz o usuário confiar no lugar errado.
    local saida codigo
    saida=$(no_aparelho "sh /tmp/instalar-do-cartao.sh $ponto; echo \"STATUS=\$?\"" 2>&1)
    codigo=$(printf '%s\n' "$saida" | sed -n 's/^STATUS=//p' | tail -1)
    : "${codigo:=1}"
    printf '%s\n' "$saida" | grep -v '^STATUS=' | sed 's/^/    /'

    if [ "$codigo" = "0" ]; then
      total_ok=$((total_ok + 1))
    else
      total_falhou=$((total_falhou + 1))
    fi
  done

  no_aparelho "rm -f /tmp/instalar-do-cartao.sh /tmp/descobrir-cartoes.sh" >/dev/null 2>&1
  for ponto in $pontos; do
    no_aparelho "umount $ponto 2>/dev/null; rmdir $ponto 2>/dev/null" >/dev/null 2>&1
  done

  if [ "$houve_pacote" = "0" ]; then
    aviso "nenhum .giochi em cartão nenhum"
    printf '  copie os pacotes pelo leitor de cartão, devolva o cartão e rode de novo.\n'
    return 1
  fi
  if [ "$total_falhou" -gt 0 ]; then
    aviso "$total_falhou cartão(ões) com pacote recusado ($total_ok ok)"
    return 1
  fi

  ok "tudo instalado"
  printf '\n  o launcher lê o catálogo na abertura:\n'
  printf '  bash scripts/aparelho.sh parar && bash scripts/aparelho.sh subir\n'
}

#
# Colhe do aparelho o que está instalado, para o gerador juntar ao catálogo.
#
# A varredura é no **aparelho** e não no repositório porque um pacote da loja não
# existe no repositório: ele chega pelo cartão e é extraído em
# `/opt/giochi/apps/<slug>/`. Perguntar ao disco do aparelho é a única resposta
# que não pode mentir.
colher_instalados() {
  local destino="$1"
  no_aparelho '
    # Duas raízes, de propósito: apps da imagem moram em `/opt/giochi/apps`,
    # apps da loja em `/user/apps`. A colheita tem de ver as duas — um app da
    # loja que ficasse fora do catálogo seria invisível sem erro.
    for d in /opt/giochi/apps/*/ /user/apps/*/; do
      m="$d/metadata.json"
      [ -f "$m" ] || continue
      id=$(basename "$d")
      # `tr -d` de quebra de linha: metadata.json é JSON válido com quebras.
      meta=$(cat "$m" | tr -d "\n\r")
      printf "{\"id\":\"%s\",\"dir\":\"%s\",\"metadata\":%s}\n" "$id" "${d%/}" "$meta"
    done
  ' 2>/dev/null | node -e "
    let s=''; process.stdin.on('data',d=>s+=d).on('end',()=>{
      const lista=[];
      for (const linha of s.split('\n')) {
        if (!linha.trim()) continue;
        try { lista.push(JSON.parse(linha)); } catch { /* metadata inválido: ignora */ }
      }
      require('fs').writeFileSync('$destino', JSON.stringify(lista, null, 2) + '\n');
      console.error('  aparelho informa ' + lista.length + ' app(s) instalado(s)');
    })"
}

cmd_boot_log() {
  exigir_aparelho
  local origem="/root/giochi-boot.log"
  if ! no_aparelho "[ -f $origem ] && echo PRESENTE" | grep -q PRESENTE; then
    aviso "$origem não existe neste boot"
    printf '  o log é criado pelo init do Giochi System; se ele não aparece,\n'
    printf '  isso já é informação: ou o init não rodou, ou o caminho mudou.\n'
    return 1
  fi

  local destino
  destino="$RAIZ/dist/boot-log-$(date +%Y%m%d-%H%M%S).txt"
  mkdir -p "$RAIZ/dist"
  if "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "root@$HOST:$origem" "$destino" 2>/dev/null; then
    ok "salvo em $destino ($(wc -l < "$destino" | tr -d ' ') linhas)"
    titulo "últimas 40 linhas"
    tail -40 "$destino"
  else
    # Plano B: sem scp (ou sem sshpass), lê pelo próprio ssh.
    titulo "não consegui copiar; lendo pelo ssh"
    no_aparelho "tail -40 $origem"
    aviso "o log NÃO foi salvo — não regrave sem antes copiá-lo"
  fi
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
  pid=$(no_aparelho "$TRECHO_MATAR_LAUNCHER"'pids_launcher' | tr -d ' ')
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
  estava_rodando=$(no_aparelho "$TRECHO_MATAR_LAUNCHER"'pids_launcher')

  if no_aparelho "$TRECHO_MATAR_LAUNCHER""
      # matar_launcher e nao kill: o launcher-teste nao bombeia eventos, o SDL
      # transforma SIGTERM em SDL_QUIT na fila e o processo continua segurando o
      # KMSDRM. Medido: com kill a captura falhava dizendo que nao gerou frame,
      # com o launcher vivo e 6 descritores abertos.
      #
      # E a flag e o que impede o supervisor de ressuscitar o launcher durante a
      # captura: sem ela o DRM nunca fica livre.
      mkdir -p /run/giochi; touch /run/giochi/dev-free-console
      matar_launcher || true
      rm -f /tmp/verif.bmp
      env SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software GIOCHI_HORA_FIXA=12:00 GIOCHI_BATERIA_FIXA=50 GIOCHI_TEMPERATURA_FIXA=40 $DESTINO_BIN --dump-frame /tmp/verif.bmp >/dev/null 2>&1
      [ -s /tmp/verif.bmp ] && echo GERADO
    " | grep -q GERADO; then
    if "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "root@$HOST:/tmp/verif.bmp" "$tmp_captura/frame.bmp" 2>/dev/null; then
      # A referência é o mesmo código rodando no PC, sem tela. Se o binário de
      # PC existir, comparar vale mais que as heurísticas de cor: é o que separa
      # "desenhou" de "desenhou de cabeça para baixo".
      local referencia rotacao
      referencia="$RAIZ/app/launcher/target/debug/giochi-launcher"
      rotacao=$(rotacao_do_aparelho)
      if [ -x "$referencia" ]; then
        local resolucao
        resolucao=$(resolucao_do_aparelho) || resolucao=""
        if [ -z "$resolucao" ]; then
          aviso "não li a resolução do painel — referência em 1280x720"
        fi
        if gerar_referencia "$tmp_captura/ref.bmp" "$resolucao"; then
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

# ── verificar um jogo no aparelho ───────────────────────────
#
# Responde "o jogo desenha no aparelho?" sem depender de olhar a tela. Roda o
# binário com `--dump-frame`, traz o BMP e analisa.
#
# ## Por que não compara pixel a pixel com o PC como `verificar` faz
#
# Aqui **não dá**: o launcher desenha uma Home parada, então a mesma entrada
# produz o mesmo quadro; um jogo avança a simulação por quadro, e o
# `opengles2` do aparelho não entrega os quadros no mesmo instante que o
# render por software do PC. Medido no arcade-volleyball: 97,9 % de acerto, e
# os 2,1 % restantes são exatamente a bola e os jogadores — que se movem.
#
# O que a comparação ainda prova é a **geometria**: chão, rede e o jogador que
# não se moveu caem nos mesmos pixels. Por isso o piso de cores e o percentual
# aparecem, mas o veredito de "desenhou" é: processo vivo, quadro do tamanho do
# painel, sem lixo.
cmd_verificar_jogo() {
  local jogo="${1:-}" quadros="${2:-240}"
  [ -n "$jogo" ] || morrer "uso: aparelho.sh verificar-jogo <jogo> [quadros]"

  # O binário não se chama igual à pasta em todo jogo: o `cobrinha` instala
  # `bin/giochi-cobrinha`. Ler `metadata.json` é o que permite esta função
  # servir a qualquer app instalado, em vez de só à convenção `<pasta>/bin/<pasta>`
  # que o `volley` e o `arcade-volleyball` usavam.
  local meta="$RAIZ/app/jogos/$jogo/metadata.json"
  [ -f "$meta" ] || morrer "app/jogos/$jogo/metadata.json não existe"

  local entry slug
  entry=$(node -e "process.stdout.write(require('$meta').entrypoint||'')" 2>/dev/null)
  slug=$(node -e "process.stdout.write(require('$meta').slug||'$jogo')" 2>/dev/null)
  [ -n "$entry" ] || morrer "metadata.json de $jogo sem entrypoint"

  local bin_local="$RAIZ/app/jogos/$jogo/$entry"
  [ -f "$bin_local" ] || morrer "sem binário em app/jogos/$jogo/$entry (compile primeiro)"

  exigir_aparelho
  titulo "jogo $jogo"

  local painel rot
  painel=$(resolucao_do_aparelho)
  [ -n "$painel" ] || morrer "não descobri a resolução do painel"
  rot=$(rotacao_do_aparelho)
  ok "painel $painel, rotação $rot"

  # Tenta as duas raízes, porque `distribution` decide onde o app mora: `image`
  # vai para `/opt/giochi/apps` e `store` para `/user/apps`. Fixar uma delas
  # fazia o app certo parecer ausente.
  local raiz_app remoto=""
  for raiz_app in /opt/giochi/apps /user/apps; do
    if [ "$(no_aparelho "[ -x $raiz_app/$slug/$entry ] && echo SIM || echo NAO")" = "SIM" ]; then
      remoto="$raiz_app/$slug/$entry"
      break
    fi
  done
  # `test -x` não imprime nada: canalizar para `grep` nunca casaria e o script
  # diria "não existe" com o arquivo lá. A resposta tem de ser explícita.
  [ -n "$remoto" ] || morrer "$slug/$entry não está instalado (nem em /opt/giochi/apps nem em /user/apps)"
  ok "instalado em $remoto"

  # `--dump-frame` (launcher, volley, arcade-volleyball) e `--quadro`
  # (cobrinha) fazem a mesma coisa com nomes diferentes. Perguntar ao binário
  # é melhor que manter uma tabela de qual usa qual.
  local flag="--dump-frame"
  if [ "$(no_aparelho "$remoto --ajuda 2>&1 | grep -c -- '--quadro'")" != "0" ]; then
    flag="--quadro"
  fi
  ok "flag de captura: $flag"

  titulo "rodando no aparelho"
  # O `inittab` ressuscita o launcher em ~1 s e o KMSDRM é exclusivo: matar uma
  # vez não basta. As duas mortes e a execução vão na MESMA sessão ssh, senão o
  # respawn ganha a corrida e o jogo recebe "kmsdrm not available".
  local saida
  saida=$(no_aparelho "$TRECHO_MATAR_LAUNCHER""
    # A flag diz ao giochi-init para NAO subir o launcher. Sem ela o supervisor
    # o ressuscita em ~1 s: o DRM nunca libera e a captura morre com
    # 'kmsdrm not available' apontando para o app errado.
    mkdir -p /run/giochi; touch /run/giochi/dev-free-console
    matar_launcher || echo \"AVISO o DRM continuou ocupado\"
    cd \$(dirname $remoto)/.. 2>/dev/null || cd /;
    # Software porque com GLES o swap esvazia o buffer e o BMP sai preto —
    # medido. Sem isto a função reprovaria um jogo que desenha.
    SDL_VIDEODRIVER=kmsdrm SDL_RENDER_DRIVER=software $remoto --quadros $quadros $flag /tmp/jogo.bmp 2>&1
    echo \"STATUS=\$?\"
    ls -l /tmp/jogo.bmp 2>/dev/null | awk '{print \"TAMANHO=\"\$5}'
  ")
  printf '%s\n' "$saida" | sed 's/^/  /'

  local status
  status=$(printf '%s' "$saida" | sed -n 's/^STATUS=//p')
  [ "${status:-1}" = "0" ] || falha "o jogo saiu com código ${status:-?}"

  local tamanho
  tamanho=$(printf '%s' "$saida" | sed -n 's/^TAMANHO=//p')
  [ -n "$tamanho" ] || morrer "o jogo não gravou /tmp/jogo.bmp — não desenhou"

  local tmp
  tmp=$(mktemp -d)
  if "${SCP_BIN[@]}" "${SSH_OPTS[@]}" "root@$HOST:/tmp/jogo.bmp" "$tmp/jogo.bmp" 2>/dev/null; then
    titulo "quadro"
    node "$RAIZ/scripts/verificar-frame.mjs" --captura "$tmp/jogo.bmp" --min-cores "${3:-3}" |
      sed 's/^/  /'
  else
    falha "não consegui baixar o quadro"
  fi
  no_aparelho 'rm -f /tmp/jogo.bmp'

  titulo "retomando o launcher"
  iniciar_launcher | while IFS= read -r linha; do printf '  %s\n' "$linha"; done
  rm -rf "$tmp"
}


# ── verificar a camada de efeitos NO APARELHO ───────────────
#
# O `verificar-efeitos.sh` roda no PC e prova que a camada desenha, é
# determinística e cabe no orçamento de chamadas. Ele **não** prova que a mesma
# camada desenha no aparelho: infraestrutura de desenho diferente (KMSDRM,
# `opengles2`) e a fonte é a do rootfs, não a do PC.
#
# Aqui a pergunta é só essa, e a resposta vem de contagem de pixels: o quadro com
# efeitos tem de diferir do sem efeitos por uma fração mínima e tem de sair com o
# tamanho exato do painel. Um BMP de tamanho errado é `read_pixels` devolvendo
# outra coisa; um BMP igual é a camada não desenhando.
#
# `GIOCHI_GLES=0` porque com backend acelerado `--dump-frame` grava **preto** (o
# swap esvazia o buffer). Medido nos três aparelhos, e é o mesmo motivo do
# `verificar-teste`.
# Isolar um efeito é trabalho do `verificar-efeitos.sh`, que roda no PC com
# `GIOCHI_EFEITOS_SO`. Este comando prova a pergunta que só o aparelho responde:
# a camada inteira desenha aqui. Aceitar um nome de efeito e ignorá-lo seria
# pior que não aceitar — seria uma flag que mente.
cmd_verificar_efeitos() {
  exigir_aparelho
  titulo "efeitos do launcher-teste"

  local remoto="/opt/giochi/apps/launcher-teste/bin/giochi-launcher-teste"
  [ "$(no_aparelho "[ -x $remoto ] && echo SIM || echo NAO")" = "SIM" ] \
    || morrer "$remoto não está instalado (bash scripts/compilar-launcher-teste-arm64.sh --instalar)"

  local painel
  painel=$(resolucao_do_aparelho)
  [ -n "$painel" ] || morrer "não descobri a resolução do painel"
  local pl pa esperado
  pl=${painel%x*}
  pa=${painel#*x}
  esperado=$(( pl * pa * 3 + 54 ))
  ok "painel $painel"

  # Duas chamadas e não uma: era uma, com aspas dentro de aspas dentro de
  # `ssh`, e o `$5` do `awk` virou string vazia — o script disse "não gravou"
  # com o arquivo lá. É a mesma armadilha que já custou o extrator do `.giochi`.
  titulo "capturando no aparelho"
  no_aparelho "$TRECHO_MATAR_LAUNCHER"'
    matar_launcher || echo "AVISO o DRM continuou ocupado"
    cd /opt/giochi/apps/launcher-teste || exit 1
    SDL_VIDEODRIVER=kmsdrm GIOCHI_GLES=0 ./bin/giochi-launcher-teste --demo       --dump-frame /tmp/efeitos_sem.bmp >/dev/null 2>&1
    echo "sem: $?"
    SDL_VIDEODRIVER=kmsdrm GIOCHI_GLES=0 GIOCHI_EFEITOS=1 ./bin/giochi-launcher-teste --demo       --efeitos --dump-frame /tmp/efeitos_com.bmp >/dev/null 2>&1
    echo "com: $?"
  ' | sed 's/^/  /'

  local tamanho
  # `wc -c` e não `stat -c`: esta imagem não tem `stat` ("stat: not found").
  # E `ls | awk` foi o que quebrou na primeira versão, por causa das aspas
  # aninhadas dentro do comando remoto.
  tamanho=$(no_aparelho 'wc -c < /tmp/efeitos_com.bmp 2>/dev/null' | tr -d ' ')
  if [ -z "$tamanho" ]; then
    morrer "não gravou /tmp/efeitos_com.bmp — a camada não desenhou"
  elif [ "$tamanho" != "$esperado" ]; then
    falha "BMP de $tamanho bytes; $painel em 24 bpp seria $esperado"
  else
    ok "BMP do tamanho do painel ($tamanho bytes)"
  fi

  local tmp
  tmp=$(mktemp -d)
  if ! "${SCP_BIN[@]}" "${SSH_OPTS[@]}" "root@$HOST:/tmp/efeitos_sem.bmp" "$tmp/sem.bmp" 2>/dev/null ||
     ! "${SCP_BIN[@]}" "${SSH_OPTS[@]}" "root@$HOST:/tmp/efeitos_com.bmp" "$tmp/com.bmp" 2>/dev/null; then
    falha "não consegui baixar os dois quadros"
  else
    local n
    n=$(node -e '
      const fs = require("fs");
      const [a, b] = process.argv.slice(1).map(f => fs.readFileSync(f));
      if (a.length !== b.length) { console.log(-1); process.exit(0); }
      let n = 0;
      for (let i = 54; i + 2 < a.length; i += 3) {
        if (Math.abs(a[i]-b[i]) + Math.abs(a[i+1]-b[i+1]) + Math.abs(a[i+2]-b[i+2]) > 12) n++;
      }
      console.log(n);
    ' "$tmp/sem.bmp" "$tmp/com.bmp")
    if [ "$n" -lt 0 ]; then
      falha "os dois BMP têm tamanhos diferentes"
    elif [ "$n" -lt 2000 ]; then
      falha "só $n pixels mudaram no aparelho — a camada não desenhou"
    else
      ok "a camada muda $n pixels no aparelho"
    fi
    mkdir -p "$RAIZ/dist/testes"
    cp "$tmp/com.bmp" "$RAIZ/dist/testes/efeitos-aparelho.bmp" 2>/dev/null
    ok "quadro em dist/testes/efeitos-aparelho.bmp"
  fi

  no_aparelho 'rm -f /tmp/efeitos_sem.bmp /tmp/efeitos_com.bmp'
  titulo "retomando o launcher"
  iniciar_launcher | while IFS= read -r linha; do printf '  %s\n' "$linha"; done
  rm -rf "$tmp"
}

# ── verificar o launcher sobre a GIOCHI-ENGINE ───────────────
#
# O `launcher-teste` desenha as MESMAS telas que o launcher de produção, então a
# pergunta "funciona no aparelho?" tem uma resposta forte e barata: gerar a
# referência no PC com o launcher de produção e comparar pixel a pixel.
#
# ## Por que não reusa `cmd_verificar`
#
# Porque aquele compara a Home do catálogo do aparelho, e este precisa de várias
# telas — inclusive a de busca, que é a única que usa **recorte** (`push_clip`).
# Recorte é justamente o que o porte reimplementou em software (a engine não
# expõe `set_clip_rect`), então é o caso que mais precisa de prova.
#
# ## Por que `GIOCHI_GLES=0`
#
# Com backend acelerado o `--dump-frame` sai **preto**: o swap esvazia o buffer
# antes da leitura. Medido nos três aparelhos. O teste é do desenho, não do
# backend; quem mede o backend é `renderer_info` no log.
cmd_verificar_teste() {
  local telas=("$@")
  [ "${#telas[@]}" -gt 0 ] || telas=(home biblioteca apps ajustes busca detalhe)

  exigir_aparelho
  local remoto="/opt/giochi/apps/launcher-teste/bin/giochi-launcher-teste"
  local referencia="$RAIZ/app/launcher/target/debug/giochi-launcher"

  [ -x "$referencia" ] || morrer "sem launcher de produção compilado no PC — rode scripts/compilar-launcher.sh"
  if ! no_aparelho "[ -x $remoto ] && echo PRESENTE" | grep -q PRESENTE; then
    morrer "$remoto não existe — rode scripts/compilar-launcher-teste-arm64.sh --instalar root@$HOST"
  fi

  local painel larg alt
  painel=$(resolucao_do_aparelho) || morrer "não descobri a resolução do painel"
  larg="${painel%x*}"; alt="${painel#*x}"
  # O launcher de produção desenha em paisagem; num painel retrato o projeto é o
  # painel com as medidas trocadas. Sem isto a referência sai no tamanho errado
  # e a comparação reprova por dimensão, não por desenho.
  if [ "$alt" -gt "$larg" ] 2>/dev/null; then
    painel="${alt}x${larg}"
  fi
  ok "painel $painel, ${#telas[@]} tela(s)"

  local tmp_captura
  tmp_captura="$RAIZ/dist/testes/$(date +%Y%m%d-%H%M%S)-teste"
  mkdir -p "$tmp_captura"
  local libs="${GIOCHI_SDL2_PREFIX:-$HOME/.cache/giochi-sdl2/root}/usr/lib/x86_64-linux-gnu"
  local estava_rodando
  estava_rodando=$(no_aparelho "$TRECHO_MATAR_LAUNCHER"'pids_launcher')

  # ## Um app aberto é o dono do DRM, não o launcher
  #
  # Medido em 09/11/2026: o launcher abriu o `Equilibrio` pelo cartão e ficou em
  # `fork`+`wait` — vivo e **sem** DRM. Matar o launcher ali deixou o jogo órfão
  # segurando o dispositivo; o launcher-teste então saía com
  # `kmsdrm not available`, e o `giochi-init` gastava a cota tentando subir o
  # launcher de produção contra um DRM ocupado, até desistir. Recusar cedo é
  # melhor que produzir cinco telas pretas e um aparelho sem interface.
  local apps_drm
  apps_drm=$(no_aparelho 'for p in /proc/[0-9]*; do d=$(ls -l $p/fd 2>/dev/null | grep -c "card0\|renderD"); [ "$d" -gt 0 ] || continue; c=$(tr "\0" " " < $p/cmdline 2>/dev/null); case "$c" in */giochi-launcher*) ;; *) echo "${p#/proc/} $c" ;; esac; done')
  if [ -n "$apps_drm" ]; then
    morrer "há app aberto com o DRM — feche no aparelho antes: $(printf '%s' "$apps_drm" | tr '\n' ' ')"
  fi

  # UMA sessão ssh para tudo, e **um** kill.
  #
  # A primeira versão matava o launcher por tela, em sessões separadas: 12 mortes
  # em sequência. O `giochi-init` desistiu de reiniciar (RESTART_MAX) e o
  # aparelho ficou sem interface até o `subir` reiniciar o supervisor — medido em
  # 07/11/2026. Com a flag de console livre e um único kill, a cota nem é tocada.
  titulo "desenhando no aparelho"
  local lista="" tela saida_telas
  for tela in "${telas[@]}"; do lista="$lista $tela"; done
  saida_telas=$(no_aparelho "$TRECHO_MATAR_LAUNCHER""
    mkdir -p /run/giochi; touch /run/giochi/dev-free-console
    # SIGTERM e depois SIGKILL. O SDL transforma SIGTERM em SDL_QUIT na fila de
    # eventos, e o teardown do EGL/Panfrost já foi visto travando um launcher
    # **sem** os descritores de DRM (pid 30803, 09/11/2026) — vivo para sempre,
    # nunca respawnado. Matar de vez é o que libera o dispositivo.
    matar_launcher
    # Esperar o DRM ficar livre, em vez de confiar num sleep fixo: é o descritor
    # que decide, não o relógio.
    i=0; n=1
    while [ \$i -lt 20 ]; do
      n=\$(for p in /proc/[0-9]*; do ls -l \$p/fd 2>/dev/null | grep -c 'card0\|renderD'; done | awk '{s+=\$1} END {print s+0}')
      [ \"\$n\" -eq 0 ] && break
      i=\$((i+1)); sleep 0.5
    done
    if [ \"\$n\" -ne 0 ]; then
      echo \"DRM_OCUPADO \$n descritor(es)\"
      rm -f /run/giochi/dev-free-console
      exit 0
    fi
    for t in$lista; do
      GIOCHI_GLES=0 GIOCHI_HORA_FIXA=12:00 GIOCHI_BATERIA_FIXA=50 GIOCHI_TEMPERATURA_FIXA=40 SDL_VIDEODRIVER=kmsdrm $remoto --demo --tela \$t \
        --dump-frame /tmp/teste-\$t.bmp >/tmp/teste-\$t.log 2>&1
      echo \"\$t: status=\$? \$(grep -c . /tmp/teste-\$t.bmp 2>/dev/null; ls -l /tmp/teste-\$t.bmp 2>/dev/null | awk '{print \$5\" bytes\"}')\"
    done
    rm -f /run/giochi/dev-free-console
    grep -hE 'renderer' /tmp/teste-home.log 2>/dev/null | head -1
  ")
  printf '%s\n' "$saida_telas" | sed 's/^/  /'

  # Se o aparelho recusou porque outra coisa segura o DRM, não há quadro para
  # comparar — dizer isso é diferente de "o launcher-teste não desenhou".
  if printf '%s\n' "$saida_telas" | grep -q DRM_OCUPADO; then
    falha "o DRM continuou ocupado no aparelho — feche o app aberto e repita"
    printf '%s\n' "$saida_telas" | sed 's/^/  /'
    return 1
  fi

  # Uma transferência para todos os BMPs: `scp` por arquivo abriria N sessões.
  local caminhos=()
  for tela in "${telas[@]}"; do caminhos+=("root@$HOST:/tmp/teste-$tela.bmp"); done
  if ! "${SCP_BIN[@]}" "${SCP_OPTS[@]}" -q "${caminhos[@]}" "$tmp_captura/" 2>/dev/null; then
    aviso "nem todos os quadros vieram do aparelho"
  fi
  no_aparelho 'rm -f /tmp/teste-*.bmp /tmp/teste-*.log' >/dev/null 2>&1

  titulo "comparação com o launcher de produção"
  local falhas=0
  for tela in "${telas[@]}"; do
    local captura="$tmp_captura/teste-$tela.bmp"
    local ref="$tmp_captura/ref-$tela.bmp"
    if [ ! -s "$captura" ]; then
      falha "$tela: o aparelho não gravou o quadro"
      falhas=$((falhas + 1))
      continue
    fi
    # A comparação é refeita **uma vez** se o resultado ficar entre 99,5 % e
    # 100 %.
    #
    # Motivo medido em 07/11/2026: a barra superior desenha o relógio. Se o
    # minuto vira entre a captura do aparelho e o render do PC, 125 pixels
    # diferem (x 490..517, y 26..34) e o acerto cai para 99,9 % — sem nenhuma
    # regressão de desenho. Reprovar isso ensina a ignorar o portão.
    #
    # O piso de 99,5 % é o que impede o retry de mascarar defeito real: uma
    # regressão de layout ou de fonte derruba o número muito abaixo disso. A
    # segunda tentativa só repete quando a primeira passou raspando.
    local tentativa pct="" saida=""
    for tentativa in 1 2; do
      # `env` explícito: `A=1 "B=2" cmd` não vira ambiente, e o binário roda sem
      # `LD_LIBRARY_PATH`, não acha `libSDL2` e não grava BMP. O sintoma era
      # "comparação ?%", que manda procurar defeito no aparelho.
      env SDL_VIDEODRIVER=dummy GIOCHI_HORA_FIXA=12:00 GIOCHI_BATERIA_FIXA=50 GIOCHI_TEMPERATURA_FIXA=40 LD_LIBRARY_PATH="$libs" "$referencia" \
        --tamanho "$painel" --demo --tela "$tela" \
        --dump-frame "$ref" >/dev/null 2>&1
      if [ ! -s "$ref" ]; then
        falha "$tela: não gerei a referência no PC (binário sem texto?)"
        falhas=$((falhas + 1))
        pct=""
        break
      fi
      saida=$(node "$RAIZ/scripts/verificar-frame.mjs" \
        --captura "$captura" --referencia "$ref" --rotacao none 2>&1)
      pct=$(printf '%s\n' "$saida" | sed -n 's/.*comparação: \([0-9.]*\)%.*/\1/p')
      [ "$pct" = "100.0" ] && break
      # `break` quando está ABAIXO do piso: aí é regressão de verdade e repetir
      # só gasta tempo. Entre 99,5 % e 100 % (exclusive) vale repetir — é o caso
      # do relógio virando o minuto entre a captura e a referência.
      #
      # A condição já esteve invertida (`exit !(p < 99.5)`), e o efeito era
      # exatamente o oposto do documentado: 99,9 % quebrava o laço **sem** a
      # segunda tentativa, reprovando as seis telas. Medido em 09/11/2026 no
      # X35H, com a diferença em x 482..517, y 26..34 — a caixa do relógio.
      awk -v p="${pct:-0}" 'BEGIN { exit !(p >= 99.5) }' && break
    done

    if [ "${pct:-0}" = "100.0" ]; then
      ok "$tela: 100,0% igual ao launcher de produção"
    elif [ -z "$pct" ]; then
      falhas=$((falhas + 1))
    else
      falha "$tela: ${pct}% — BMP em $tmp_captura"
      falhas=$((falhas + 1))
    fi
  done

  if [ -n "$estava_rodando" ]; then
    titulo "retomando o launcher"
    iniciar_launcher | while IFS= read -r linha; do printf '  %s\n' "$linha"; done
  fi

  echo
  if [ "$falhas" -eq 0 ]; then
    ok "todas as telas idênticas no aparelho"
  else
    falha "$falhas tela(s) divergente(s)"
  fi
  printf '  capturas em %s\n' "$tmp_captura"
  return "$falhas"
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
  opcoes)      cmd_opcoes ;;
  boot-log)    cmd_boot_log ;;
  catalogo)    cmd_catalogo ;;
  instalar-pacote|pacote)
    shift
    cmd_instalar_pacote "${1:-}" ;;
  instalar-do-cartao|cartao) cmd_instalar_do_cartao ;;
  verificar-jogo)
    shift
    cmd_verificar_jogo "${1:-}" "${2:-}" "${3:-}" ;;
  verificar-teste|teste)
    shift
    cmd_verificar_teste "$@" ;;
  verificar-efeitos) cmd_verificar_efeitos ;;
  ajuda|--help|-h|"")
    # Só o bloco de comentário do topo: para no primeiro `set -`, não numa
    # faixa fixa de linhas que sai do lugar a cada edição.
    awk 'NR>1 && /^#/ {sub(/^# ?/, ""); print; next} NR>1 {exit}' "${BASH_SOURCE[0]}"
    ;;
  *)
    falha "subcomando desconhecido: $1"
    printf 'use: procurar | diagnostico | instalar | subir | parar | verificar | log | boot-log | catalogo | instalar-pacote\n'
    exit 1
    ;;
esac
