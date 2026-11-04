#!/usr/bin/env node
// verificar-frame.mjs — confere um quadro renderizado pelo launcher.
//
// ## Por que existe
//
// "O processo está vivo" não prova que a interface aparece. Já aconteceu de o
// app rodar com o DRM aberto e o quadro sair preto — e de o conteúdo sair de
// cabeça para baixo por causa de 180° de diferença na rotação.
//
// A verificação é por pixel, sem dependência externa: nada de ImageMagick, nada
// de PNG decoder. O quadro vem como BMP do próprio SDL.
//
// ## Como usar
//
//   node scripts/verificar-frame.mjs --captura quadro.bmp
//   node scripts/verificar-frame.mjs --captura quadro.bmp --referencia pc.bmp --rotacao ccw
//   node scripts/verificar-frame.mjs --captura quadro.bmp --png saida.png
//
// ## O que é considerado suspeito
//
// | Sintoma | Leitura |
// | --- | --- |
// | menos de 8 cores distintas | provavelmente tela lisa — render parou cedo |
// | mais de 90 % de preto puro | letterbox ou tela apagada |
// | menos de 90 % de acerto na comparação | rotação errada (difere 180°) |
//
// A comparação é a que vale: des-rotaciona o quadro do aparelho e compara com o
// render de 1280×720 do PC. 99,9 % de acerto com tolerância 12 foi o resultado
// que confirmou a orientação correta no X55.

import fs from 'node:fs';
import zlib from 'node:zlib';
import { pathToFileURL } from 'node:url';

// ── leitura de BMP ──────────────────────────────────────────

/** Lê um BMP 24 ou 32 bpp e devolve pixels RGB acessíveis por (x, y). */
function lerBmp(caminho) {
  const b = fs.readFileSync(caminho);
  if (b.length < 54 || b[0] !== 0x42 || b[1] !== 0x4d) {
    throw new Error(`${caminho}: não é BMP (assinatura ausente)`);
  }
  const deslocamento = b.readUInt32LE(10);
  const largura = b.readInt32LE(18);
  const alturaCrua = b.readInt32LE(22);
  const bpp = b.readUInt16LE(28);
  if (bpp !== 24 && bpp !== 32) {
    throw new Error(`${caminho}: esperava 24 ou 32 bpp, veio ${bpp}`);
  }
  const deBaixoParaCima = alturaCrua > 0;
  const altura = Math.abs(alturaCrua);
  const bytesPorPixel = bpp / 8;
  const tamanhoLinha = Math.floor((bpp * largura + 31) / 32) * 4;

  return {
    largura,
    altura,
    bpp,
    em: (x, y) => {
      const linha = deslocamento + (deBaixoParaCima ? altura - 1 - y : y) * tamanhoLinha;
      const p = linha + x * bytesPorPixel;
      // BMP guarda BGR.
      return [b[p + 2], b[p + 1], b[p]];
    },
  };
}

// ── escrita de PNG ──────────────────────────────────────────
//
// Sem dependência: são 40 linhas de CRC e deflate. Necessário porque não há
// ImageMagick no PC nem no aparelho, e olhar o quadro é a única forma de julgar
// o desenho sem estar com o aparelho na mão.

const TABELA_CRC = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = TABELA_CRC[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function bloco(tipo, dados) {
  const tam = Buffer.alloc(4);
  tam.writeUInt32BE(dados.length);
  const t = Buffer.from(tipo, 'ascii');
  const c = Buffer.alloc(4);
  c.writeUInt32BE(crc32(Buffer.concat([t, dados])));
  return Buffer.concat([tam, t, dados, c]);
}

/** Grava uma imagem RGB em PNG. */
function gravarPng(caminho, largura, altura, em) {
  const linhas = [];
  for (let y = 0; y < altura; y++) {
    const linha = Buffer.alloc(largura * 3 + 1);
    linha[0] = 0; // filtro "None"
    for (let x = 0; x < largura; x++) {
      const [r, g, bl] = em(x, y);
      linha[1 + x * 3] = r;
      linha[2 + x * 3] = g;
      linha[3 + x * 3] = bl;
    }
    linhas.push(linha);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(largura, 0);
  ihdr.writeUInt32BE(altura, 4);
  ihdr[8] = 8; // bits por canal
  ihdr[9] = 2; // RGB
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    bloco('IHDR', ihdr),
    bloco('IDAT', zlib.deflateSync(Buffer.concat(linhas), { level: 9 })),
    bloco('IEND', Buffer.alloc(0)),
  ]);
  fs.writeFileSync(caminho, png);
  return png.length;
}

// ── rotação ─────────────────────────────────────────────────

/**
 * Desfaz a rotação aplicada pelo app, devolvendo o quadro em paisagem.
 *
 * O app gira 90° no sentido horário (`cw`) ou anti-horário (`ccw`) quando o
 * painel é retrato. Girar é involução em 90°: desfazer é girar no sentido
 * oposto. A conta é feita direto na leitura para não alocar duas imagens.
 */
function desRotacionar(img, rotacao) {
  const { largura: L, altura: A } = img;
  if (rotacao === 'none') {
    return { largura: L, altura: A, em: (x, y) => img.em(x, y) };
  }
  // Em paisagem, a imagem des-rotacionada tem (A x L).
  const larguraSaida = A;
  const alturaSaida = L;
  if (rotacao === 'ccw') {
    // O app fez (x,y) -> (y, 1280-x). Inverso: (dx,dy) -> (1280-dy, dx),
    // simplificado para o laço abaixo.
    return {
      largura: larguraSaida,
      altura: alturaSaida,
      em: (x, y) => img.em(y, larguraSaida - 1 - x),
    };
  }
  if (rotacao === 'cw') {
    return {
      largura: larguraSaida,
      altura: alturaSaida,
      em: (x, y) => img.em(alturaSaida - 1 - y, x),
    };
  }
  throw new Error(`rotação desconhecida: ${rotacao} (use none, cw ou ccw)`);
}

// ── análise ─────────────────────────────────────────────────

function analisar(img, emitir = console.log) {
  let pretos = 0;
  let total = 0;
  const cores = new Set();
  for (let y = 0; y < img.altura; y += 4) {
    for (let x = 0; x < img.largura; x += 4) {
      const [r, g, b] = img.em(x, y);
      total++;
      cores.add((r << 16) | (g << 8) | b);
      if (r === 0 && g === 0 && b === 0) pretos++;
    }
  }
  const pctPreto = (100 * pretos) / Math.max(1, total);
  emitir(`  tamanho:    ${img.largura}x${img.altura} (${img.bpp ?? '?'} bpp)`);
  emitir(`  cores:      ${cores.size} distintas`);
  emitir(`  preto puro: ${pctPreto.toFixed(2)}%`);
  return { cores: cores.size, pctPreto };
}

/**
 * Mede a faixa preta no topo e na base.
 *
 * Serve para dimensionar o letterbox, **não** para julgar alinhamento: o BMP
 * não carrega o deslocamento do viewport (ver aviso em `principal`).
 */
function medirFaixas(img) {
  const preto = (x, y) => {
    const [r, g, b] = img.em(x, y);
    return r === 0 && g === 0 && b === 0;
  };
  const linhaTodaPreta = (y) => {
    for (let x = 0; x < img.largura; x += 8) if (!preto(x, y)) return false;
    return true;
  };
  let topo = 0;
  while (topo < img.altura && linhaTodaPreta(topo)) topo++;
  let base = 0;
  while (base < img.altura && linhaTodaPreta(img.altura - 1 - base)) base++;
  return { topo, base, pct: (100 * (topo + base)) / Math.max(1, img.altura) };
}

function comparar(captura, referencia, rotacao, tolerancia, emitir = console.log) {
  const paisagem = desRotacionar(captura, rotacao);
  if (paisagem.largura !== referencia.largura || paisagem.altura !== referencia.altura) {
    emitir(
      `  dimensões incompatíveis: captura ${paisagem.largura}x${paisagem.altura} ` +
        `vs referência ${referencia.largura}x${referencia.altura}`
    );
    return { pctAcerto: 0, motivo: 'dimensoes' };
  }
  let iguais = 0;
  let total = 0;
  for (let y = 0; y < referencia.altura; y += 3) {
    for (let x = 0; x < referencia.largura; x += 3) {
      const a = referencia.em(x, y);
      const b = paisagem.em(x, y);
      const d = Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]) + Math.abs(a[2] - b[2]);
      total++;
      if (d <= tolerancia) iguais++;
    }
  }
  const pct = (100 * iguais) / Math.max(1, total);
  emitir(`  comparação: ${pct.toFixed(1)}% de acerto (tolerância ${tolerancia}, ${total} amostras)`);
  return { pctAcerto: pct };
}

// ── linha de comando ────────────────────────────────────────

function lerArgs(argv) {
  const args = { rotacao: 'none', tolerancia: 12 };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--captura') args.captura = argv[++i];
    else if (a === '--referencia') args.referencia = argv[++i];
    else if (a === '--rotacao') args.rotacao = argv[++i];
    else if (a === '--tolerancia') args.tolerancia = Number(argv[++i]);
    else if (a === '--png') args.png = argv[++i];
    else if (a === '--ajuda' || a === '-h') args.ajuda = true;
    else throw new Error(`argumento desconhecido: ${a}`);
  }
  return args;
}

function ajuda() {
  console.log(`
verificar-frame.mjs — confere um quadro renderizado pelo launcher

  --captura <bmp>       quadro capturado no aparelho (obrigatório)
  --referencia <bmp>    render de 1280x720 do PC, para comparação
  --rotacao <modo>      none | cw | ccw  (padrão: none)
  --tolerancia <n>      soma de diferença RGB aceita por pixel (padrão: 12)
  --png <arquivo>       grava o quadro (des-rotacionado) em PNG para inspeção

Sai com código 1 quando o quadro é suspeito: menos de 8 cores, mais de 90% de
preto, ou menos de 90% de acerto na comparação.
`);
}

export { analisar, comparar, desRotacionar, gravarPng, lerBmp, medirFaixas };

function principal() {
  let args;
  try {
    args = lerArgs(process.argv);
  } catch (e) {
    console.error(`erro: ${e.message}`);
    process.exit(2);
  }
  if (args.ajuda || !args.captura) {
    ajuda();
    process.exit(args.ajuda ? 0 : 2);
  }

  let captura;
  try {
    captura = lerBmp(args.captura);
  } catch (e) {
    console.error(`erro: ${e.message}`);
    process.exit(2);
  }

  console.log(`quadro: ${args.captura}`);
  const { cores, pctPreto } = analisar(captura);

  let problemas = [];
  if (cores < 8) problemas.push(`só ${cores} cores distintas — o render provavelmente parou cedo`);
  if (pctPreto > 90) problemas.push(`${pctPreto.toFixed(0)}% de preto — tela apagada ou letterbox`);

  // Análise de POSIÇÃO de faixa é enganosa em BMP, e é melhor avisar do que
  // deixar alguém concluir errado. `read_pixels` — que o `--dump-frame` usa —
  // não respeita o deslocamento do viewport: o SDL centraliza o letterbox na
  // tela, mas a leitura devolve o conteúdo a partir de (0,0). Num painel 4:3 a
  // faixa aparece toda embaixo no arquivo, embora esteja simétrica na tela.
  //
  // O percentual de preto e a comparação continuam válidos: o PC tem o mesmo
  // artefato, então as duas imagens se comparam 1:1.
  const faixas = medirFaixas(captura);
  if (faixas.topo > 4 || faixas.base > 4) {
    console.log(
      `  faixa:      ${faixas.topo}px no topo, ${faixas.base}px na base ` +
        `(${faixas.pct.toFixed(1)}% da altura)`
    );
    if (Math.abs(faixas.topo - faixas.base) > 8) {
      console.log("  aviso:      a faixa parece assimétrica NO ARQUIVO. Isso não prova");
      console.log("              desalinhamento na tela: read_pixels ignora o offset do");
      console.log("              viewport. Confira no aparelho antes de chamar de bug.");
    }
  }

  if (args.referencia) {
    console.log(`referência: ${args.referencia} (rotação ${args.rotacao})`);
    let referencia;
    try {
      referencia = lerBmp(args.referencia);
    } catch (e) {
      console.error(`erro: ${e.message}`);
      process.exit(2);
    }
    const { pctAcerto, motivo } = comparar(
      captura,
      referencia,
      args.rotacao,
      args.tolerancia
    );
    if (motivo === 'dimensoes') {
      problemas.push(
        `o quadro não está em ${referencia.largura}x${referencia.altura} após a rotação ` +
          `"${args.rotacao}" — painel retrato precisa de cw ou ccw`
      );
    } else if (pctAcerto < 90) {
      // Sugere o oposto do que foi tentado: os dois sentidos diferem 180°, então
      // o outro é a única alternativa quando as dimensões batem e o conteúdo não.
      const outra = args.rotacao === 'cw' ? 'ccw' : 'cw';
      problemas.push(
        `${pctAcerto.toFixed(1)}% de acerto com rotação "${args.rotacao}" — ` +
          `tente --rotacao ${outra} (os dois sentidos diferem 180°)`
      );
    }
  }

  if (args.png) {
    const paisagem = desRotacionar(captura, args.rotacao);
    const bytes = gravarPng(args.png, paisagem.largura, paisagem.altura, paisagem.em);
    console.log(`  png:        ${args.png} (${bytes} bytes)`);
  }

  if (problemas.length > 0) {
    console.log('');
    for (const p of problemas) console.log(`  SUSPEITO: ${p}`);
    process.exit(1);
  }
  console.log('  resultado:  ok');
}

// Só executa quando chamado direto: os testes importam este módulo.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  principal();
}
