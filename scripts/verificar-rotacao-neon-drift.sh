#!/usr/bin/env bash
# verificar-rotacao-neon-drift.sh — prova que o **mundo** e o **HUD** do
# Neon Drift giram no mesmo sentido que o sistema manda.
#
#   bash scripts/verificar-rotacao-neon-drift.sh            # no PC
#   bash scripts/verificar-rotacao-neon-drift.sh --aparelho # PC x aparelho
#
# ## Por que esta ferramenta existe
#
# O X55 mostrou "o jogo de ponta a cabeça e as informações na vertical". Os dois
# lados estavam **coerentes sozinhos**: o HUD seguia `video::girar_rect` (a regra
# do launcher) e o mundo seguia o `roll` da câmera. Discordavam 180° — e não
# havia uma linha de código, teste ou log que comparasse os dois.
#
# A pergunta que responde: **o quadro retrato, des-rotacionado, é o quadro
# paisagem?** Um quadro colorido é sempre um quadro colorido; só a comparação
# com a referência de projeto diz se o sentido está certo.
#
# ## A inversa tem de ser a inversa
#
# Desfazer `Ccw90` **não** é "girar para o outro lado": `girar_rect(Ccw90)`
# leva o projeto em (y, W−x), e a inversa leva o painel em (y, H−x) no eixo
# trocado. Girar 90° para o lado errado dá uma **transposta**, que num quadro
# quase simétrico passa despercebida. A prova de que a inversa está certa é o
# `--prova-rotacao`: três barras de cor conhecida, com posição prevista em
# número. Se as barras não voltarem ao lugar, a conta da ferramenta está errada,
# não o jogo.
#
# ## Uso de `--prova-rotacao`
#
# As três barras, em coordenadas de projeto 1280×720:
#
#   vermelho  (0,0,1280,24)  faixa horizontal no topo
#   verde     (0,0,24,720)   faixa vertical à esquerda
#   azul      (40,40,64,64)  quadrado pequeno perto do canto superior esquerdo
#
# Sob `Ccw90` elas têm de cair em:
#
#   vermelho  x 0..23,    y 0..1279     (borda esquerda, vertical)
#   verde     x 0..719,   y 1256..1279  (borda inferior, horizontal)
#   azul      x 40..103,  y 1176..1239
#
# O launcher usa a mesma regra e mede 99,9 % no aparelho — as duas contas têm de
# concordar, senão um dos apps mente sobre a orientação do sistema.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$RAIZ/app/jogos/neon-drift"
BIN="$APP/target/release/giochi-neon-drift"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

VERDE=$'\033[32m'; VERMELHO=$'\033[31m'; AMARELO=$'\033[33m'; ZERA=$'\033[0m'
FALHAS=0
ok()    { printf '  %sok%s   %s\n'   "$VERDE" "$ZERA" "$1"; }
falha() { printf '  %sFALHA%s %s\n' "$VERMELHO" "$ZERA" "$1"; FALHAS=$((FALHAS + 1)); }
aviso() { printf '  %saveiso%s %s\n' "$AMARELO" "$ZERA" "$1"; }

MODO=pc
[ "${1:-}" = "--aparelho" ] && MODO=aparelho

printf '== neon-drift: rotação ==\n'

# ── 1. as barras, no PC ───────────────────────────────────────────────────────
# Sem elas a comparação de quadro inteiro não vale: uma inversa transposta passa
# num quadro quase simétrico e reprova num assimétrico, e ninguém saberia qual
# dos dois está errado.
norm_barras() {
  # Extrai as caixas das três barras de um BMP 32/24 bits, em coordenadas do
  # arquivo. Usa o leitor do verificar-frame (mesmo parser, uma casa).
  node - "$1" <<'JS'
const fs = require('node:fs');
const b = fs.readFileSync(process.argv[2]);
if (b.toString('ascii', 0, 2) !== 'BM') { console.error('não é BMP'); process.exit(2); }
const off = b.readUInt32LE(10);
const w = b.readInt32LE(18);
const hRaw = b.readInt32LE(22);
const h = Math.abs(hRaw), topDown = hRaw < 0;
const bpp = b.readUInt16LE(28);
const bytes = bpp / 8;
const passo = Math.ceil((w * bytes) / 4) * 4;
const px = (x, y) => {
  const p = off + ((topDown ? y : h - 1 - y) * passo) + x * bytes;
  return [b[p + 2], b[p + 1], b[p]];
};
const alvos = { vermelho: [255, 0, 0], verde: [0, 255, 0], azul: [0, 0, 255] };
const caixas = {};
for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
  const [r, g, bl] = px(x, y);
  for (const [n, [R, G, B]] of Object.entries(alvos)) {
    if (Math.abs(r - R) < 24 && Math.abs(g - G) < 24 && Math.abs(bl - B) < 24) {
      const c = caixas[n] ??= { x0: Infinity, y0: Infinity, x1: -1, y1: -1 };
      c.x0 = Math.min(c.x0, x); c.x1 = Math.max(c.x1, x);
      c.y0 = Math.min(c.y0, y); c.y1 = Math.max(c.y1, y);
    }
  }
}
console.log(JSON.stringify({ w, h, caixas }));
JS
}

if [ ! -x "$BIN" ]; then
  falha "binário do PC ausente — rode: bash scripts/compilar-jogo.sh --jogo neon-drift"
else
  SDL_VIDEODRIVER=offscreen GIOCHI_ROTATE=ccw "$BIN" \
    --prova-rotacao "$TMP/barras.bmp" --tamanho 720x1280 2>/dev/null

  BARRAS="$(norm_barras "$TMP/barras.bmp")"
  LER_CAIXA='const d=JSON.parse(process.argv[1]);const c=d.caixas[process.argv[2]];if(!c){console.log("ausente");process.exit(0)};console.log(`${c.x0}..${c.x1} x ${c.y0}..${c.y1}`)'

  # A prova é o **centro** de cada barra. As barras saem com metade do tamanho
  # pedido (`--prova-rotacao` reusa o quad do HUD, que é 1x1, e a matriz supõe
  # 2x2), mas o centro é exato — e é o centro que carrega a rotação. Medir a
  # extensão misturaria "a rotação está certa" com "o quad é do tamanho X", que
  # são duas perguntas.
  #
  # Centros previstos por `girar_ponto(Ccw90, (1280,720), x, y) = (y, 1280-x)`:
  #   vermelho (640, 12) -> (12, 640)
  #   verde    (12, 360) -> (360, 1268)
  #   azul     (72, 72)  -> (72, 1208)
  PROVA_OK=1
  CENTRO='const d=JSON.parse(process.argv[1]);const c=d.caixas[process.argv[2]];if(!c){console.log("ausente");process.exit(0)};console.log(((c.x0+c.x1)/2)+" "+((c.y0+c.y1)/2))'
  for caso in "vermelho:12:640" "verde:360:1268" "azul:72:1208"; do
    IFS=: read -r nome ex ey <<<"$caso"
    LIDO="$(node -e "$CENTRO" "$BARRAS" "$nome")"
    if [ "$LIDO" = "ausente" ]; then
      falha "barra $nome sumiu — a rotação do HUD não está sendo aplicada"
      PROVA_OK=0; continue
    fi
    CX="${LIDO%% *}"; CY="${LIDO##* }"
    if ! awk "BEGIN{exit !(($CX-$ex)^2 <= 4 && ($CY-$ey)^2 <= 4)}"; then
      falha "centro da barra $nome em ($CX, $CY), esperado ($ex, $ey)"
      PROVA_OK=0
    fi
  done
  [ "$PROVA_OK" = 1 ] && ok "as três barras giram exatamente como o \`girar_ponto\` manda"
fi

# ── 2. mundo e HUD concordam ──────────────────────────────────────────────────
# O quadro retrato des-rotacionado com a inversa correta tem de reproduzir o
# quadro paisagem. É a pergunta que ninguém tinha feito.
if [ -x "$BIN" ]; then
  export GIOCHI_HORA_FIXA=12:00 GIOCHI_BATERIA_FIXA=77 GIOCHI_TEMPERATURA_FIXA=40

  SDL_VIDEODRIVER=offscreen "$BIN" --tela corrida --piloto --passos 900 \
    --tamanho 1280x720 --quadro "$TMP/paisagem.bmp" >/dev/null 2>&1
  GIOCHI_ROTATE=ccw SDL_VIDEODRIVER=offscreen "$BIN" --tela corrida --piloto --passos 900 \
    --tamanho 720x1280 --quadro "$TMP/retrato.bmp" >/dev/null 2>&1

  # Inversa de `girar_rect(Ccw90)`: painel (x,y) → projeto (W−1−y, x)... na
  # prática: destino(x,y) lê a origem em (y, H_dest−1−x). Ver o cabeçalho.
  node - "$TMP/retrato.bmp" "$TMP/desfeito.bmp" <<'JS'
const fs = require('node:fs');
const [, , entra, sai] = process.argv;
function ler(caminho) {
  const b = fs.readFileSync(caminho);
  const off = b.readUInt32LE(10), w = b.readInt32LE(18), hRaw = b.readInt32LE(22);
  const h = Math.abs(hRaw), topo = hRaw < 0, bpp = b.readUInt16LE(28), bytes = bpp / 8;
  const passo = Math.ceil((w * bytes) / 4) * 4;
  return { w, h, px: (x, y) => { const p = off + ((topo ? y : h - 1 - y) * passo) + x * bytes; return [b[p + 2], b[p + 1], b[p]]; } };
}
const A = ler(entra);
const W = A.h, H = A.w;              // 90° troca as medidas
const passo = W * 4;
const dados = Buffer.alloc(54 + passo * H);
dados.write('BM', 0, 'ascii');
dados.writeUInt32LE(54 + passo * H, 2);
dados.writeUInt32LE(54, 10); dados.writeUInt32LE(40, 14);
dados.writeInt32LE(W, 18); dados.writeInt32LE(H, 22);
dados.writeUInt16LE(1, 26); dados.writeUInt16LE(32, 28);
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
  const [r, g, b] = A.px(y, A.h - 1 - x);
  const p = 54 + (H - 1 - y) * passo + x * 4;
  dados[p] = b; dados[p + 1] = g; dados[p + 2] = r; dados[p + 3] = 0;
}
fs.writeFileSync(sai, dados);
JS

  IGUAIS="$(node - "$TMP/desfeito.bmp" "$TMP/paisagem.bmp" <<'JS'
const fs = require('node:fs');
function ler(c) {
  const b = fs.readFileSync(c);
  const off = b.readUInt32LE(10), w = b.readInt32LE(18), hRaw = b.readInt32LE(22);
  const h = Math.abs(hRaw), topo = hRaw < 0, bpp = b.readUInt16LE(28), bytes = bpp / 8;
  const passo = Math.ceil((w * bytes) / 4) * 4;
  return { w, h, px: (x, y) => { const p = off + ((topo ? y : h - 1 - y) * passo) + x * bytes; return [b[p + 2], b[p + 1], b[p]]; } };
}
const A = ler(process.argv[2]), B = ler(process.argv[3]);
if (A.w !== B.w || A.h !== B.h) { console.log('tamanho'); process.exit(0); }
let ok = 0, perto = 0, total = 0;
for (let y = 0; y < A.h; y++) for (let x = 0; x < A.w; x++) {
  const p = A.px(x, y), q = B.px(x, y);
  const d = Math.max(Math.abs(p[0] - q[0]), Math.abs(p[1] - q[1]), Math.abs(p[2] - q[2]));
  total++; if (d === 0) ok++; if (d <= 4) perto++;
}
console.log((100 * ok / total).toFixed(2) + ' ' + (100 * perto / total).toFixed(2));
JS
)"

  EXATO="${IGUAIS%% *}"; PERTO="${IGUAIS##* }"
  if [ "$EXATO" = "tamanho" ]; then
    falha "a inversa devolveu tamanho diferente — o `girar_rect` e a ferramenta discordam"
  elif awk "BEGIN{exit !($EXATO >= 99.0)}"; then
    ok "mundo + HUD des-rotacionados: $EXATO% idênticos ($PERTO% dentro de 4 níveis)"
  elif awk "BEGIN{exit !($EXATO >= 95.0)}"; then
    aviso "mundo + HUD a $EXATO% (aceitável, mas abaixo do esperado: >99%)"
  else
    falha "mundo + HUD a só $EXATO% — os dois giram em sentidos diferentes, é o bug do 'ponta a cabeça'"
  fi
fi

# ── 3. o aparelho, quando pedido ──────────────────────────────────────────────
if [ "$MODO" = "aparelho" ]; then
  printf '\n== aparelho ==\n'
  if ! bash "$RAIZ/scripts/aparelho.sh" opcoes >/dev/null 2>&1; then
    aviso "nenhum aparelho configurado — rode \`bash scripts/aparelho.sh procurar\`"
  else
    ssh -o BatchMode=yes -o ConnectTimeout=8 "${GIOCHI_ALVO:-root@10.55.0.2}" \
      'cat /sys/class/graphics/fbcon/rotate 2>/dev/null; cat /proc/cmdline' 2>/dev/null \
      | sed 's/^/  /' || falha "não consegui ler a rotação do aparelho"
  fi
fi

printf '\n'
if [ "$FALHAS" -eq 0 ]; then
  printf '%sok%s rotação verificada\n' "$VERDE" "$ZERA"
else
  printf '%sFALHA%s %d verificação(ões) falharam\n' "$VERMELHO" "$ZERA" "$FALHAS"
  exit 1
fi
