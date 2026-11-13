#!/usr/bin/env node
// Gerador dos modelos (OBJ) e das texturas (BMP) do GIOCHI Tower Defense.
//
// # Por que gerar em vez de desenhar à mão
//
// Os personagens eram cubos porque a engine, até o snapshot de 13/11/2026, não
// tinha caminho de modelo nem de textura. Agora tem (`load_obj_mesh` e
// `create_texture`), e este script produz os arquivos que o jogo carrega.
//
// Arte desenhada em código tem duas propriedades que importam aqui:
//
// 1. **é reproduzível** — a mesma execução dá os mesmos bytes, então um
//    `git diff` nos assets diz exatamente o que mudou, e `--conferir` pode
//    exigir isso no CI;
// 2. **é auditável** — "o Tank é mais largo que o Grunt" vira número no gerador
//    (`ombro: 0.72` contra `0.38`), não uma impressão sobre um binário.
//
// O que ela não é: arte final. Um modelador faz melhor. O que existe aqui é
// **silhueta legível a 640×480**, que é o que o aparelho precisa.
//
// # Formato
//
// OBJ com `v` e `vt`, faces `f v/vt`, **sem `vn`**. O loader da engine deriva a
// normal por face quando ela falta (`prepare_normals`), e faceta visível é
// exatamente o que se quer num low-poly: normal suave aqui esconderia o relevo
// que a luz do shader `textured` desenha.
//
// BMP 24 bits e não PNG: o `SDL2_image` do aparelho já apareceu compilado sem
// `--enable-png` (AGENTS.md §24), e BMP atravessa todas as imagens do projeto.
//
// Uso:
//   node scripts/gerar-modelos.mjs             # gera em assets/
//   node scripts/gerar-modelos.mjs --conferir  # não escreve; falha se divergir

import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..");

// ─────────────────────────────────────────────────────────────── paletas
//
// Uma paleta por personagem, não cor por peça: a textura é multiplicada pela luz
// no shader, então dois tons próximos viram um só a 640×480. O `acento` é o que
// sobrevive à distância e diz de quem é o quê.
const PALETAS = {
  soldado: { base: [58, 92, 150], claro: [96, 138, 200], escuro: [26, 42, 74], acento: [236, 196, 72] },
  guerreiro: { base: [140, 62, 46], claro: [196, 104, 74], escuro: [62, 24, 20], acento: [240, 168, 72] },
  sniper: { base: [52, 110, 88], claro: [96, 168, 132], escuro: [22, 48, 40], acento: [180, 236, 132] },
  medico: { base: [168, 168, 178], claro: [214, 216, 224], escuro: [70, 74, 86], acento: [222, 72, 84] },
  grunt: { base: [116, 74, 128], claro: [164, 116, 178], escuro: [50, 28, 60], acento: [236, 96, 196] },
  tank: { base: [92, 92, 72], claro: [140, 140, 112], escuro: [40, 40, 30], acento: [214, 176, 64] },
  ranged: { base: [70, 116, 148], claro: [112, 168, 204], escuro: [28, 50, 68], acento: [96, 232, 240] },
  swarm: { base: [148, 96, 56], claro: [196, 140, 92], escuro: [64, 38, 20], acento: [248, 208, 96] },
  flying: { base: [132, 68, 92], claro: [180, 108, 140], escuro: [56, 24, 38], acento: [248, 120, 168] },
  tanker: { base: [78, 84, 96], claro: [126, 134, 148], escuro: [30, 34, 42], acento: [224, 92, 64] },
  plasma: { base: [64, 92, 168], claro: [108, 148, 232], escuro: [24, 34, 76], acento: [140, 232, 255] },
  brain: { base: [148, 108, 132], claro: [196, 152, 176], escuro: [62, 38, 52], acento: [255, 168, 220] },
  queen: { base: [104, 60, 132], claro: [156, 104, 196], escuro: [42, 20, 62], acento: [255, 220, 120] },
  coronel: { base: [72, 40, 48], claro: [124, 76, 88], escuro: [28, 14, 18], acento: [226, 64, 96] },
  torre: { base: [58, 74, 108], claro: [104, 132, 184], escuro: [22, 30, 48], acento: [120, 208, 255] },
};

// ───────────────────────────────────────────────────── primitivas de malha

/** Uma malha em construção: posições, UVs e triângulos. */
function malha() {
  return { v: [], vt: [], f: [] };
}

/**
 * Revoluciona um perfil 2D `[[raio, y], ...]` em torno do eixo Y.
 *
 * É o que dá corpo, cabeça, braço e perna com a mesma função: um cilindro é um
 * perfil de dois pontos, uma esfera achatada é um perfil de cinco. O UV é
 * cilíndrico — `u` dá a volta e `v` sobe o perfil —, então a textura acompanha
 * qualquer forma sem costura.
 */
function lathe(m, perfil, { segmentos = 10, centro = [0, 0, 0], escala = 1, fechar = true } = {}) {
  const base = m.v.length;
  const linhas = perfil.length;
  for (let i = 0; i < linhas; i++) {
    const [raio, y] = perfil[i];
    for (let s = 0; s < segmentos; s++) {
      const a = (s / segmentos) * Math.PI * 2;
      const r = raio * escala;
      m.v.push([centro[0] + Math.cos(a) * r, centro[1] + y * escala, centro[2] + Math.sin(a) * r]);
      m.vt.push([s / segmentos, i / Math.max(1, linhas - 1)]);
    }
  }
  const idx = (linha, seg) => base + linha * segmentos + (seg % segmentos);
  for (let i = 0; i < linhas - 1; i++) {
    for (let s = 0; s < segmentos; s++) {
      // Sentido horário visto de fora, que é o que o descarte de faces espera.
      // Inverter some com metade do modelo sem erro nenhum.
      m.f.push([idx(i, s), idx(i + 1, s), idx(i + 1, s + 1)]);
      m.f.push([idx(i, s), idx(i + 1, s + 1), idx(i, s + 1)]);
    }
  }
  if (fechar) {
    tapar(m, base, segmentos, 0, true);
    tapar(m, base, segmentos, linhas - 1, false);
  }
  return m;
}

/** Fecha um anel com um leque no centro. */
function tapar(m, base, segmentos, linha, invertido) {
  let cx = 0;
  let cz = 0;
  let cy = 0;
  for (let s = 0; s < segmentos; s++) {
    const v = m.v[base + linha * segmentos + s];
    cx += v[0];
    cy += v[1];
    cz += v[2];
  }
  const iCentro = m.v.length;
  m.v.push([cx / segmentos, cy / segmentos, cz / segmentos]);
  m.vt.push([0.5, 0.5]);
  for (let s = 0; s < segmentos; s++) {
    const a = base + linha * segmentos + s;
    const b = base + linha * segmentos + ((s + 1) % segmentos);
    if (invertido) m.f.push([iCentro, b, a]);
    else m.f.push([iCentro, a, b]);
  }
}

/** Caixa alinhada aos eixos, com UV planar por face. */
function caixa(m, { centro = [0, 0, 0], tamanho = [1, 1, 1] } = {}) {
  const [cx, cy, cz] = centro;
  const [w, h, d] = tamanho.map((t) => t * 0.5);
  const cantos = [
    [-w, -h, -d], [w, -h, -d], [w, h, -d], [-w, h, -d],
    [-w, -h, d], [w, -h, d], [w, h, d], [-w, h, d],
  ];
  // UV por face, não projetado no mundo: uma caixa de 4 m com UV de mundo
  // mostraria 4 repetições e a mesma textura pareceria de outro material.
  const faces = [
    [4, 5, 6, 7], [1, 0, 3, 2], [5, 1, 2, 6], [0, 4, 7, 3], [3, 7, 6, 2], [0, 1, 5, 4],
  ];
  const uvFace = [[0, 0], [1, 0], [1, 1], [0, 1]];
  for (const face of faces) {
    const base = m.v.length;
    face.forEach((indice, i) => {
      const c = cantos[indice];
      m.v.push([cx + c[0], cy + c[1], cz + c[2]]);
      m.vt.push(uvFace[i]);
    });
    m.f.push([base, base + 1, base + 2]);
    m.f.push([base, base + 2, base + 3]);
  }
}

/** Esfera achatada por lathe — cabeça, olho e boss redondo. */
function esfera(m, { centro = [0, 0, 0], raio = 0.5, achatada = 1, segmentos = 10 } = {}) {
  const perfil = [];
  const passos = 5;
  for (let i = 0; i <= passos; i++) {
    const t = i / passos;
    const y = (t - 0.5) * 2 * raio * achatada;
    const r = Math.sqrt(Math.max(0, raio * raio - (y / achatada) * (y / achatada)));
    perfil.push([r, y]);
  }
  lathe(m, perfil, { segmentos, centro });
}

// ─────────────────────────────────────────────────────── modelos

/** Corpo humanoide: torso, cabeça, dois braços e duas pernas. */
function humanoide({ altura = 1.8, ombro = 0.42, cintura = 0.3, cabeca = 0.24, braco = 0.1, perna = 0.13, segmentos = 10 } = {}) {
  const m = malha();
  const pe = altura * 0.46;
  const topoTorso = altura * 0.86;

  lathe(m, [
    [cintura * 0.9, pe],
    [cintura, pe + altura * 0.06],
    [ombro, topoTorso - altura * 0.1],
    [ombro * 0.86, topoTorso],
  ], { segmentos });

  lathe(m, [[cabeca * 0.55, topoTorso], [cabeca * 0.5, topoTorso + altura * 0.02]], { segmentos });
  esfera(m, { centro: [0, topoTorso + altura * 0.11, 0], raio: cabeca, achatada: 1.05, segmentos });

  for (const lado of [-1, 1]) {
    lathe(m, [
      [braco, topoTorso - altura * 0.02],
      [braco * 0.9, pe + altura * 0.2],
      [braco * 0.7, pe + altura * 0.02],
    ], { centro: [lado * ombro * 1.02, 0, 0], segmentos: 6 });
  }

  for (const lado of [-1, 1]) {
    lathe(m, [
      [perna, pe],
      [perna * 0.9, altura * 0.24],
      [perna * 0.75, 0],
    ], { centro: [lado * cintura * 0.6, 0, 0], segmentos: 6 });
  }
  return m;
}

function capacete(m, { y = 1.62, raio = 0.27 } = {}) {
  esfera(m, { centro: [0, y, 0], raio, achatada: 0.7 });
  caixa(m, { centro: [0, y + raio * 0.55, 0], tamanho: [0.06, 0.1, raio * 1.5] });
}

function fuzil(m, { y = 1.2, comprimento = 1.1 } = {}) {
  caixa(m, { centro: [0.12, y, -comprimento * 0.35], tamanho: [0.06, 0.08, comprimento] });
  caixa(m, { centro: [0.12, y - 0.06, -comprimento * 0.62], tamanho: [0.1, 0.16, 0.2] });
}

function asas(m, { y = 1.2, envergadura = 1.6 } = {}) {
  for (const lado of [-1, 1]) {
    caixa(m, { centro: [lado * envergadura * 0.35, y, 0], tamanho: [envergadura * 0.7, 0.04, 0.5] });
    caixa(m, { centro: [lado * envergadura * 0.55, y - 0.12, 0.16], tamanho: [envergadura * 0.3, 0.03, 0.34] });
  }
}

function torre() {
  const m = malha();
  lathe(m, [[2.6, 0], [2.8, 0.5], [2.2, 0.9], [1.4, 1.3]], { segmentos: 12 });
  lathe(m, [[0.9, 1.3], [0.7, 3.6], [1.1, 4.2]], { segmentos: 10 });
  // Cristal: o "coração" que a barra de HP representa.
  lathe(m, [[0, 4.2], [0.85, 5.0], [0, 5.9]], { segmentos: 6 });
  return m;
}

function tanque() {
  const m = humanoide({ altura: 2.6, ombro: 0.9, cintura: 0.7, cabeca: 0.34, braco: 0.24, perna: 0.3, segmentos: 12 });
  for (const lado of [-1, 1]) {
    esfera(m, { centro: [lado * 0.82, 2.05, 0], raio: 0.42, achatada: 0.7, segmentos: 8 });
  }
  return m;
}

function plasma() {
  const m = humanoide({ altura: 2.4, ombro: 0.5, cintura: 0.36, cabeca: 0.28, braco: 0.11, perna: 0.16, segmentos: 12 });
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2;
    esfera(m, {
      centro: [Math.cos(a) * 0.95, 1.9 + Math.sin(a) * 0.35, Math.sin(a) * 0.95],
      raio: 0.28,
      segmentos: 8,
    });
  }
  return m;
}

function cerebro() {
  const m = malha();
  esfera(m, { centro: [0, 1.9, 0], raio: 1.15, achatada: 0.85, segmentos: 14 });
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    lathe(m, [[0.22, 1.3], [0.16, 0.5], [0.1, 0]], {
      centro: [Math.cos(a) * 0.7, 0, Math.sin(a) * 0.7],
      segmentos: 6,
    });
  }
  return m;
}

function rainha() {
  const m = humanoide({ altura: 3.0, ombro: 0.5, cintura: 0.34, cabeca: 0.3, braco: 0.1, perna: 0.12, segmentos: 12 });
  lathe(m, [[0.4, 1.35], [1.0, 0.5], [1.3, 0]], { segmentos: 12 });
  for (let i = 0; i < 5; i++) {
    const a = (i / 5) * Math.PI * 2;
    caixa(m, { centro: [Math.cos(a) * 0.2, 3.42, Math.sin(a) * 0.2], tamanho: [0.07, 0.34, 0.07] });
  }
  return m;
}

function coronel() {
  const m = humanoide({ altura: 2.5, ombro: 0.52, cintura: 0.36, cabeca: 0.27, braco: 0.1, perna: 0.15, segmentos: 12 });
  lathe(m, [[0.18, 2.05], [0.75, 1.2], [1.05, 0.05]], { segmentos: 12, centro: [0, 0, -0.18] });
  capacete(m, { y: 2.28, raio: 0.3 });
  return m;
}

// ─────────────────────────────────────────────── catálogo de assets
//
// A tabela é de **silhueta**, não de gameplay: a altura final é a do modelo e o
// jogo escala por cima para casar com o raio de colisão.
export const MODELOS = {
  soldado: () => {
    const m = humanoide();
    capacete(m, { y: 1.62 });
    fuzil(m, { comprimento: 0.9 });
    return m;
  },
  guerreiro: () => {
    const m = humanoide({ altura: 1.95, ombro: 0.55, cintura: 0.38, cabeca: 0.23, braco: 0.14, perna: 0.17 });
    for (const lado of [-1, 1]) {
      esfera(m, { centro: [lado * 0.6, 1.55, 0], raio: 0.3, achatada: 0.6, segmentos: 8 });
    }
    return m;
  },
  sniper: () => {
    const m = humanoide({ altura: 1.8, ombro: 0.34, cintura: 0.28, cabeca: 0.22, braco: 0.09, perna: 0.12 });
    fuzil(m, { comprimento: 1.4 });
    return m;
  },
  medico: () => {
    const m = humanoide({ altura: 1.75, ombro: 0.38 });
    caixa(m, { centro: [0, 1.15, -0.28], tamanho: [0.34, 0.36, 0.16] });
    // Cruz no peito: o símbolo que se lê de longe.
    caixa(m, { centro: [0, 1.3, 0.27], tamanho: [0.3, 0.1, 0.04] });
    caixa(m, { centro: [0, 1.3, 0.27], tamanho: [0.1, 0.3, 0.04] });
    return m;
  },
  grunt: () => humanoide({ altura: 1.6, ombro: 0.38, cintura: 0.28, cabeca: 0.26, braco: 0.1, perna: 0.12 }),
  tank: () => humanoide({ altura: 1.5, ombro: 0.72, cintura: 0.6, cabeca: 0.22, braco: 0.2, perna: 0.24, segmentos: 12 }),
  ranged: () => {
    const m = humanoide({ altura: 1.7, ombro: 0.36, cintura: 0.26, cabeca: 0.24, braco: 0.09, perna: 0.11 });
    esfera(m, { centro: [0, 2.05, 0], raio: 0.2, segmentos: 8 });
    return m;
  },
  swarm: () => {
    const m = malha();
    esfera(m, { centro: [0, 0.45, 0], raio: 0.42, achatada: 0.9, segmentos: 8 });
    esfera(m, { centro: [0.3, 0.7, 0.2], raio: 0.22, segmentos: 6 });
    return m;
  },
  flying: () => {
    const m = humanoide({ altura: 1.5, ombro: 0.3, cintura: 0.24, cabeca: 0.2, braco: 0.07, perna: 0.08 });
    asas(m, { y: 1.3, envergadura: 2.0 });
    return m;
  },
  tanker: tanque,
  plasma,
  brain: cerebro,
  queen: rainha,
  coronel,
  torre,
};

// ───────────────────────────────────────────────────── texturas (BMP)
//
// 64×64 com painéis de 8 px, emenda escura, faixa de acento e gradiente
// vertical. Uma cor chapada voltaria a ser um cubo pintado: o shader `textured`
// tem Lambert fixo, e é a variação de tom na textura que desenha o relevo.
const LADO_TEXTURA = 64;

function textura(paleta, semente) {
  const px = new Uint8Array(LADO_TEXTURA * LADO_TEXTURA * 3);
  // LCG com semente fixa por modelo. `Math.random()` faria dois `--conferir`
  // divergirem e o teste de determinismo reprovaria o próprio gerador.
  let estado = (semente * 2654435761) >>> 0;
  const sorteio = () => {
    estado = (estado * 1664525 + 1013904223) >>> 0;
    return estado / 0xffffffff;
  };

  for (let y = 0; y < LADO_TEXTURA; y++) {
    for (let x = 0; x < LADO_TEXTURA; x++) {
      const emenda = x % 8 === 0 || y % 8 === 0;
      const brilho = 0.82 + sorteio() * 0.18;
      let cor = paleta.base.map((c) => c * brilho);
      if (emenda) cor = paleta.escuro;
      const v = 1 - y / LADO_TEXTURA;
      if (v > 0.45 && v < 0.62 && ((x > 4 && x < 12) || (x > 52 && x < 60))) cor = paleta.acento;
      const tom = 0.78 + 0.4 * v;
      const i = (y * LADO_TEXTURA + x) * 3;
      px[i] = Math.min(255, Math.round(cor[0] * tom));
      px[i + 1] = Math.min(255, Math.round(cor[1] * tom));
      px[i + 2] = Math.min(255, Math.round(cor[2] * tom));
    }
  }
  return px;
}

/** BMP 24 bits, linhas de baixo para cima e alinhadas em 4 bytes. */
function bmp(largura, altura, rgb) {
  const linhaBytes = largura * 3;
  const padding = (4 - (linhaBytes % 4)) % 4;
  const tamanhoPixels = (linhaBytes + padding) * altura;
  const buf = Buffer.alloc(54 + tamanhoPixels);
  buf.write("BM", 0, "ascii");
  buf.writeUInt32LE(54 + tamanhoPixels, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(largura, 18);
  buf.writeInt32LE(altura, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(tamanhoPixels, 34);
  let deslocamento = 54;
  for (let y = altura - 1; y >= 0; y--) {
    for (let x = 0; x < largura; x++) {
      const i = (y * largura + x) * 3;
      buf[deslocamento++] = rgb[i + 2];
      buf[deslocamento++] = rgb[i + 1];
      buf[deslocamento++] = rgb[i];
    }
    for (let p = 0; p < padding; p++) buf[deslocamento++] = 0;
  }
  return buf;
}

// ────────────────────────────────────────────────────── OBJ

function obj(m, nome) {
  const linhas = [`# ${nome} — gerado por scripts/gerar-modelos.mjs`, `o ${nome}`];
  for (const [x, y, z] of m.v) linhas.push(`v ${fmt(x)} ${fmt(y)} ${fmt(z)}`);
  for (const [u, v] of m.vt) linhas.push(`vt ${fmt(u)} ${fmt(v)}`);
  for (const [a, b, c] of m.f) {
    const ia = a + 1;
    const ib = b + 1;
    const ic = c + 1;
    linhas.push(`f ${ia}/${ia} ${ib}/${ib} ${ic}/${ic}`);
  }
  return linhas.join("\n") + "\n";
}

/** Três casas: milímetro é o que a malha precisa, e o arquivo encolhe. */
function fmt(valor) {
  const arredondado = Math.round(valor * 1000) / 1000;
  return Object.is(arredondado, -0) ? "0" : String(arredondado);
}

// ─────────────────────────────────────────────────────── execução

export function gerar() {
  const arquivos = new Map();
  let semente = 1;
  for (const [nome, construir] of Object.entries(MODELOS)) {
    const m = construir();
    arquivos.set(join("modelos", `${nome}.obj`), Buffer.from(obj(m, nome), "utf8"));
    const paleta = PALETAS[nome] ?? PALETAS.soldado;
    arquivos.set(join("texturas", `${nome}.bmp`), bmp(LADO_TEXTURA, LADO_TEXTURA, textura(paleta, semente++)));
  }
  return arquivos;
}

function principal() {
  const conferir = process.argv.includes("--conferir");
  const arquivos = gerar();
  let divergentes = 0;

  for (const [relativo, conteudo] of arquivos) {
    const destino = join(RAIZ, "assets", relativo);
    if (conferir) {
      if (!existsSync(destino)) {
        console.error(`FALTA ${relativo}`);
        divergentes++;
      } else if (!readFileSync(destino).equals(conteudo)) {
        console.error(`DIVERGE ${relativo}`);
        divergentes++;
      }
      continue;
    }
    mkdirSync(dirname(destino), { recursive: true });
    writeFileSync(destino, conteudo);
  }

  if (conferir) {
    if (divergentes > 0) {
      console.error(`\n${divergentes} arquivo(s) divergente(s) — rode sem --conferir`);
      process.exit(1);
    }
    console.log(`ok   ${arquivos.size} arquivo(s) idêntico(s) ao gerador`);
    return;
  }

  const modelos = [...arquivos.keys()].filter((n) => n.startsWith("modelos")).length;
  console.log(`ok   ${modelos} modelo(s) e ${arquivos.size - modelos} textura(s) em assets/`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) principal();
