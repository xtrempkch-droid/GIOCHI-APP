#!/usr/bin/env node
//! Gera `app/musica/capa.png` — 320x320, a capa do app na loja.
//!
//! Por que um gerador e não uma imagem solta: a capa mostra o **estilo de capa
//! que o app desenha** (três cartões do carrossel, cores derivadas do nome com
//! FNV-1a). Uma imagem desenhada à mão envelheceria na primeira mudança de
//! paleta; esta é regerada com um comando.
//!
//! As cores saem da mesma conta do lado Rust (`crates/core/src/capa.rs`), mas
//! **não há código compartilhado**: este arquivo só roda ao gerar a capa, e o
//! app nunca importa o que está aqui. Se as duas conversões divergirem, o
//! ícone da loja fica diferente do app — e é por isso que a paleta está
//! duplicada em três constantes visíveis, não espalhada em números mágicos.
//!
//! Uso:
//!   node scripts/gerar-capa-musica.mjs            # regrava app/musica/capa.png
//!   node scripts/gerar-capa-musica.mjs --saida /tmp/x.png
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const AQUI = path.dirname(fileURLToPath(import.meta.url));
const RAIZ = path.resolve(AQUI, "..");

const LADO = 320;

// ── as mesmas três constantes do app ────────────────────────
const SATURACAO = 0.58;
const LUMINOSIDADE = 0.42;
const FUNDO = [0x0b, 0x11, 0x1d];
const FUNDO2 = [0x12, 0x1b, 0x2b];
const ACENTO = [0x66, 0xc0, 0xf4];
const TEXTO = [0xf1, 0xf5, 0xf9];

export function fnv1a64(bytes) {
  let hash = 0xcbf29ce484222325n;
  const primo = 0x100000001b3n;
  for (const b of bytes) {
    hash ^= BigInt(b);
    hash = (hash * primo) & 0xffffffffffffffffn;
  }
  return hash;
}

export function matizDe(nome) {
  return Number(fnv1a64(Buffer.from(nome, "utf8")) % 360n);
}

export function rgbHsl(matiz, s = SATURACAO, l = LUMINOSIDADE) {
  const h = (matiz % 360) / 360;
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const canal = (t) => {
    let x = t < 0 ? t + 1 : t > 1 ? t - 1 : t;
    let v;
    if (x < 1 / 6) v = p + (q - p) * 6 * x;
    else if (x < 0.5) v = q;
    else if (x < 2 / 3) v = p + (q - p) * (2 / 3 - x) * 6;
    else v = p;
    return Math.round(Math.min(255, Math.max(0, v * 255)));
  };
  return [canal(h + 1 / 3), canal(h), canal(h - 1 / 3)];
}

// ── tela em RGBA ────────────────────────────────────────────
function tela() {
  const px = Buffer.alloc(LADO * LADO * 4, 0);
  const por = (x, y, [r, g, b], alfa = 255) => {
    if (x < 0 || y < 0 || x >= LADO || y >= LADO) return;
    const i = (y * LADO + x) * 4;
    const a = alfa / 255;
    px[i] = Math.round(r * a + px[i] * (1 - a));
    px[i + 1] = Math.round(g * a + px[i + 1] * (1 - a));
    px[i + 2] = Math.round(b * a + px[i + 2] * (1 - a));
    px[i + 3] = 255;
  };
  return { px, por };
}

function cartao(por, x0, y0, lado, matiz, destaque = false) {
  const base = rgbHsl(matiz);
  const claro = rgbHsl(matiz, 0.55, 0.62);
  const detalhe = rgbHsl((matiz + 47) % 360);

  for (let y = 0; y < lado; y += 1) {
    for (let x = 0; x < lado; x += 1) {
      // Faixa diagonal e listras: o mesmo desenho de `arte.rs`, em pequeno.
      const faixa = (x + (lado - y) / 3) % lado;
      let cor = base;
      if (faixa < lado * 0.12) cor = claro;
      if (x > lado * 0.72 && x < lado * 0.78) cor = detalhe;
      por(x0 + x, y0 + y, cor);
    }
  }
  // Círculo central do "disco".
  const cx = x0 + lado / 2;
  const cy = y0 + lado / 2;
  const r = lado / 4;
  for (let y = -r; y <= r; y += 1) {
    for (let x = -r; x <= r; x += 1) {
      const d2 = x * x + y * y;
      if (Math.abs(d2 - r * r) < r * 2) por(Math.round(cx + x), Math.round(cy + y), detalhe, 220);
      else if (d2 < (r / 3) * (r / 3)) por(Math.round(cx + x), Math.round(cy + y), claro, 230);
    }
  }
  // Moldura.
  const cor = destaque ? ACENTO : claro;
  const espessura = destaque ? 4 : 2;
  for (let e = 0; e < espessura; e += 1) {
    for (let x = 0; x < lado; x += 1) {
      por(x0 + x, y0 + e, cor);
      por(x0 + x, y0 + lado - 1 - e, cor);
    }
    for (let y = 0; y < lado; y += 1) {
      por(x0 + e, y0 + y, cor);
      por(x0 + lado - 1 - e, y0 + y, cor);
    }
  }
}

function triangulo(por, cx, cy, lado, cor) {
  // Triângulo de "tocar", sólido.
  const meio = lado / 2;
  for (let y = -meio; y <= meio; y += 1) {
    const avanco = (y + meio) / lado;
    const largura = lado * (1 - Math.abs(avanco * 2 - 1)) * 0.86;
    for (let x = 0; x < largura; x += 1) {
      por(Math.round(cx + x - largura / 6), Math.round(cy + y), cor);
    }
  }
}

export function desenharCapa() {
  const { px, por } = tela();

  // Fundo: gradiente vertical em bandas.
  for (let y = 0; y < LADO; y += 1) {
    const t = y / (LADO - 1);
    const cor = [0, 1, 2].map((i) => Math.round(FUNDO[i] * (1 - t) + FUNDO2[i] * t));
    for (let x = 0; x < LADO; x += 1) por(x, y, cor);
  }

  // Três cartões em leque: a esquerda e a direita atrás, o do meio na frente.
  //
  // Os nomes são escolhidos pelas **matizes** que produzem (243°, 45° e 155°,
  // medidas com o mesmo FNV-1a do app): três tons vizinhos, como "Faixa 2" e
  // "Faixa 3" davam, viram um borrão só numa capa de 64 px na lista da loja.
  cartao(por, 18, 96, 120, matizDe("Faixa 1.mp3"));
  cartao(por, 182, 96, 120, matizDe("Vento.mp3"));
  cartao(por, 82, 84, 156, matizDe("Maré.flac"), true);

  // Botão de tocar no centro do cartão da frente.
  triangulo(por, 160 + 8, 162, 54, TEXTO);

  return px;
}

// ── PNG (sem dependência) ───────────────────────────────────
const TABELA = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = TABELA[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(tipo, dados) {
  const corpo = Buffer.concat([Buffer.from(tipo, "ascii"), dados]);
  const tamanho = Buffer.alloc(4);
  tamanho.writeUInt32BE(dados.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(corpo), 0);
  return Buffer.concat([tamanho, corpo, crc]);
}

export function paraPng(px, largura = LADO, altura = LADO) {
  const linhas = [];
  for (let y = 0; y < altura; y += 1) {
    linhas.push(Buffer.from([0]));
    linhas.push(px.subarray(y * largura * 4, (y + 1) * largura * 4));
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(largura, 0);
  ihdr.writeUInt32BE(altura, 4);
  ihdr[8] = 8; // bits por canal
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(Buffer.concat(linhas), { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function principal() {
  const argv = process.argv.slice(2);
  const indice = argv.indexOf("--saida");
  const destino = indice >= 0 && argv[indice + 1]
    ? path.resolve(argv[indice + 1])
    : path.join(RAIZ, "app", "musica", "capa.png");
  mkdirSync(path.dirname(destino), { recursive: true });
  const png = paraPng(desenharCapa());
  writeFileSync(destino, png);
  console.log(`capa ${LADO}x${LADO} gravada em ${destino} (${png.length} bytes)`);
}

if (process.argv[1] && import.meta.url === `file://${path.resolve(process.argv[1])}`) {
  principal();
}
