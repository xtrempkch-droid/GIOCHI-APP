// capacidades.mjs — lê `/proc/bus/input/devices` e diz quais códigos de tecla
// cada dispositivo afirma poder emitir.
//
// ## Por que existe
//
// Sem isto, descobrir se um botão chega ao app é adivinhação. `evtest --query`
// devolve 0 para **qualquer** código nesta imagem — inútil. A única fonte é o
// bitmask `B: KEY=` do `/proc`.
//
// E o formato engana: o kernel imprime as palavras de 64 bits em ordem
// **decrescente**, então a última coluna é a palavra 0. Ler da esquerda para a
// direita desloca todos os códigos em múltiplos de 64 e produz um mapa que
// parece plausível e está errado.
//
// ## Conferência contra o aparelho
//
// `B: KEY=10000000000000 0` → palavra 1, bit 52 → `64 + 52 = 116 = KEY_POWER`.
// `B: KEY=c000000000000 0`  → palavra 1, bits 50 e 51 → 114 e 115, que são
// `KEY_VOLUMEDOWN` e `KEY_VOLUMEUP`. Os dois batem com o que o hardware faz.

/** Nomes só para os códigos cuja correspondência está confirmada no kernel. */
const NOMES = new Map([
  [1, "KEY_ESC"],
  [2, "KEY_1"],
  [3, "KEY_2"],
  [4, "KEY_3"],
  [5, "KEY_4"],
  [6, "KEY_5"],
  [7, "KEY_6"],
  [8, "KEY_7"],
  [9, "KEY_8"],
  [10, "KEY_9"],
  [11, "KEY_0"],
  [14, "KEY_BACKSPACE"],
  [15, "KEY_TAB"],
  [16, "KEY_Q"],
  [17, "KEY_W"],
  [18, "KEY_E"],
  [19, "KEY_R"],
  [20, "KEY_T"],
  [21, "KEY_Y"],
  [22, "KEY_U"],
  [23, "KEY_I"],
  [24, "KEY_O"],
  [25, "KEY_P"],
  [28, "KEY_ENTER"],
  [29, "KEY_LEFTCTRL"],
  [30, "KEY_A"],
  [31, "KEY_S"],
  [32, "KEY_D"],
  [33, "KEY_F"],
  [34, "KEY_G"],
  [35, "KEY_H"],
  [36, "KEY_J"],
  [37, "KEY_K"],
  [38, "KEY_L"],
  [42, "KEY_LEFTSHIFT"],
  [44, "KEY_Z"],
  [45, "KEY_X"],
  [46, "KEY_C"],
  [47, "KEY_V"],
  [48, "KEY_B"],
  [49, "KEY_N"],
  [50, "KEY_M"],
  [54, "KEY_RIGHTSHIFT"],
  [56, "KEY_LEFTALT"],
  [57, "KEY_SPACE"],
  [58, "KEY_CAPSLOCK"],
  [97, "KEY_RIGHTCTRL"],
  [100, "KEY_RIGHTALT"],
  [103, "KEY_UP"],
  [105, "KEY_LEFT"],
  [106, "KEY_RIGHT"],
  [108, "KEY_DOWN"],
  [113, "KEY_MUTE"],
  [114, "KEY_VOLUMEDOWN"],
  [115, "KEY_VOLUMEUP"],
  [116, "KEY_POWER"],
  [119, "KEY_PAUSE"],
  [121, "KEY_KPCOMMA"],
  [125, "KEY_LEFTMETA"],
  [126, "KEY_RIGHTMETA"],
  [127, "KEY_COMPOSE"],
  [139, "KEY_MENU"],
  [142, "KEY_SLEEP"],
  [143, "KEY_WAKEUP"],
  [152, "KEY_COFFEE"],
  [158, "KEY_BACK"],
  [159, "KEY_FORWARD"],
  [161, "KEY_EJECTCD"],
  [163, "KEY_NEXTSONG"],
  [164, "KEY_PLAYPAUSE"],
  [165, "KEY_PREVIOUSSONG"],
  [166, "KEY_STOPCD"],
  [172, "KEY_HOMEPAGE"],
  [224, "KEY_BRIGHTNESSDOWN"],
  [225, "KEY_BRIGHTNESSUP"],
  [240, "KEY_UNKNOWN"],
  // Direcional. Fica na faixa BTN_ (0x220), não na faixa KEY_ — por isso não
  // aparece nos nomes de teclado e é fácil confundir com código sem nome.
  [544, "BTN_DPAD_UP"],
  [545, "BTN_DPAD_DOWN"],
  [546, "BTN_DPAD_LEFT"],
  [547, "BTN_DPAD_RIGHT"],
  // Botões de gamepad.
  [304, "BTN_SOUTH (A)"],
  [305, "BTN_EAST (B)"],
  [306, "BTN_C"],
  [307, "BTN_NORTH (X)"],
  [308, "BTN_WEST (Y)"],
  [309, "BTN_Z"],
  [310, "BTN_TL"],
  [311, "BTN_TR"],
  [312, "BTN_TL2"],
  [313, "BTN_TR2"],
  [314, "BTN_SELECT"],
  [315, "BTN_START"],
  [316, "BTN_MODE"],
  [317, "BTN_THUMBL"],
  [318, "BTN_THUMBR"],
]);

/** Nome do código, ou `null` quando não há correspondência confirmada. */
export function nomeDaTecla(codigo) {
  return NOMES.get(codigo) ?? null;
}

/** Rótulo pronto para impressão: nome quando existe, número sempre. */
export function rotulo(codigo) {
  const nome = nomeDaTecla(codigo);
  return nome ? `${codigo} ${nome}` : String(codigo);
}

/**
 * Converte a lista de palavras hex do `B: KEY=` em códigos de tecla.
 *
 * A última palavra é a de menor peso. É o ponto onde é fácil errar: ler a
 * esquerda como palavra 0 desloca tudo em 64 por coluna.
 */
export function decodificarBitmask(palavras) {
  const codigos = new Set();
  const n = palavras.length;
  for (let i = 0; i < n; i++) {
    const palavra = i; // índice na string
    const peso = n - 1 - palavra;
    const valor = BigInt(`0x${palavras[i] || "0"}`);
    if (valor === 0n) continue;
    for (let bit = 0; bit < 64; bit++) {
      if ((valor >> BigInt(bit)) & 1n) codigos.add(peso * 64 + bit);
    }
  }
  return codigos;
}

/**
 * Lê o texto de `/proc/bus/input/devices`.
 *
 * Um bloco por dispositivo, separado por linha em branco. Só `KEY` interessa:
 * `EV` diz que o dispositivo é capable de teclas, `KEY` diz quais.
 */
export function parseDispositivos(texto) {
  const dispositivos = [];
  let atual = null;

  for (const linha of texto.split("\n")) {
    const campo = linha.trim();
    if (campo.startsWith("N: Name=")) {
      if (atual) dispositivos.push(atual);
      atual = { nome: campo.slice(9).replace(/^"|"$/g, ""), handlers: [], key: new Set() };
      continue;
    }
    if (!atual) continue;
    if (campo.startsWith("H: Handlers=")) {
      atual.handlers = campo.slice(12).trim().split(/\s+/).filter(Boolean);
    } else if (campo.startsWith("B: KEY=")) {
      const palavras = campo.slice(7).trim().split(/\s+/).filter(Boolean);
      atual.key = decodificarBitmask(palavras);
    }
  }
  if (atual) dispositivos.push(atual);
  return dispositivos;
}

/** Dispositivos que afirmam poder emitir `codigo`, com o `eventN` de cada um. */
export function quemEmite(dispositivos, codigo) {
  return dispositivos
    .filter((d) => d.key.has(codigo))
    .map((d) => ({ nome: d.nome, eventos: d.handlers.filter((h) => h.startsWith("event")) }));
}

/** Índice código → dispositivos, para responder muitos códigos de uma vez. */
export function porCodigo(dispositivos) {
  const mapa = new Map();
  for (const d of dispositivos) {
    for (const codigo of d.key) {
      if (!mapa.has(codigo)) mapa.set(codigo, []);
      mapa.get(codigo).push(d.nome);
    }
  }
  return new Map([...mapa.entries()].sort((a, b) => a[0] - b[0]));
}
