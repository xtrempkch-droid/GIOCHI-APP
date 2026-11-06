#!/usr/bin/env node
// Verifica os jogos nas resoluções que existem (e nas que vão existir).
//
// Uso:
//   node scripts/verificar-jogos.mjs                 # 640x480, 800x600, 1280x720
//   node scripts/verificar-jogos.mjs --tamanhos 800x600
//   node scripts/verificar-jogos.mjs --png           # grava uma imagem por caso
//
// ## Por que existe
//
// `verificar-telas.mjs` mede as telas **do launcher**. Os jogos são outros
// binários, com matemática própria de grade, e não tinham verificação nenhuma
// fora do aparelho — então "funciona em 480p e 720p" era afirmação, não medida,
// e 600p nunca foi olhado.
//
// O que se mede aqui não é pixel igual (não há referência): é **geometria**. A
// célula de uma grade precisa de um tamanho mínimo para ser lida, e essa conta é
// proporcional ao painel. É o que separa "cabe" de "serve".
//
// ## O limite de legibilidade
//
// 14 px de célula é o piso prático: abaixo disso o vão entre células, o corpo da
// cobra e a pastilha viram o mesmo borrão num painel de 3,5". É um número
// escolhido a partir do menor aparelho que temos (640×480 dá 18 px no
// `cobrinha` e 22 px no `pacman`), com folga de 25%.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { gravarPng, lerBmp } from "./verificar-frame.mjs";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SAIDA = path.join(RAIZ, "dist", "jogos");

/** Célula mínima, em pixels de device, para a grade continuar legível. */
const CELULA_MINIMA = 14;

/**
 * Os jogos e como medir cada um.
 *
 * `celula` recebe a caixa do desenho e o número de divisões; devolve o passo em
 * pixels. Cada jogo tem a sua conta porque cada um escolhe a grade de um jeito.
 */
const JOGOS = [
  {
    id: "volley",
    // Sem grade: a medida é a raquete, que precisa ser alcançável com o dedo e
    // visível. 28 px é o piso (ALTURA * 0.12 num painel de 240 px daria menos).
    medir: (img) => {
      const caixa = caixaDeCor(img, (c) => Math.abs(c[0] - 96) < 14 && Math.abs(c[1] - 165) < 14 && Math.abs(c[2] - 250) < 14);
      return { nome: "raquete (altura)", valor: caixa.h, minimo: 28 };
    },
  },
  {
    id: "cobrinha",
    medir: (img) => {
      // A grade é desenhada em 30,34,50 e ocupa o quadrado central.
      const caixa = caixaDeCor(img, (c) => Math.abs(c[0] - 30) < 10 && Math.abs(c[1] - 34) < 10 && Math.abs(c[2] - 50) < 12);
      const celula = caixa.w / 24; // 24 colunas, fixo em `Jogo::novo`
      return { nome: "célula", valor: Math.round(celula * 10) / 10, minimo: CELULA_MINIMA };
    },
  },
  {
    id: "pacman",
    medir: (img) => {
      // Parede em 37,62,160; o labirinto tem 21 colunas e 21 linhas.
      const caixa = caixaDeCor(img, (c) => Math.abs(c[0] - 37) < 14 && Math.abs(c[1] - 62) < 14 && Math.abs(c[2] - 160) < 22);
      const celula = caixa.w / 21;
      return { nome: "célula", valor: Math.round(celula * 10) / 10, minimo: CELULA_MINIMA };
    },
  },
];

/** Caixa envolvente dos pixels que satisfazem o teste. */
function caixaDeCor(img, teste) {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < img.altura; y++) {
    for (let x = 0; x < img.largura; x++) {
      if (!teste(img.em(x, y))) continue;
      if (x < x0) x0 = x;
      if (x > x1) x1 = x;
      if (y < y0) y0 = y;
      if (y > y1) y1 = y;
    }
  }
  if (x1 < 0) return { x0: 0, y0: 0, w: 0, h: 0 };
  return { x0, y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
}

/** O binário do jogo, já compilado para x86-64. */
function binarioDoJogo(id) {
  const alvo = path.join(RAIZ, "app", "jogos", id, "target", "release", `giochi-${id}`);
  if (!existsSync(alvo)) {
    throw new Error(
      `binário ausente: app/jogos/${id}/target/release/giochi-${id}\n` +
        `compile com: cd app/jogos/${id} && bash ../../../scripts/compilar-launcher.sh --release`,
    );
  }
  return alvo;
}

function opcoes(argv) {
  const o = { tamanhos: ["640x480", "800x600", "1280x720"], png: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--tamanhos") o.tamanhos = argv[++i].split(",");
    else if (argv[i] === "--png") o.png = true;
  }
  return o;
}

function main() {
  const o = opcoes(process.argv.slice(2));
  mkdirSync(SAIDA, { recursive: true });

  // `dummy` respeita o tamanho pedido; `x11` devolveria o desktop.
  const ambiente = { ...process.env, SDL_VIDEODRIVER: "dummy", SDL_RENDER_DRIVER: "software" };
  let falhou = 0;

  for (const tamanho of o.tamanhos) {
    console.log(`\n── ${tamanho} ──`);
    console.log("  jogo        medida              valor    mínimo   estado");

    for (const jogo of JOGOS) {
      const bin = binarioDoJogo(jogo.id);
      const bmp = path.join(SAIDA, `${jogo.id}-${tamanho}.bmp`);
      execFileSync(bin, ["--tamanho", tamanho, "--quadro", bmp], { env: ambiente, stdio: "ignore" });

      const img = lerBmp(bmp);
      const { nome, valor, minimo } = jogo.medir(img);
      const estado = valor >= minimo ? "ok" : "APERTADO";
      if (estado !== "ok") falhou++;

      console.log(
        `  ${jogo.id.padEnd(11)} ${nome.padEnd(20)} ${String(valor).padStart(6)} ${String(minimo).padStart(8)}   ${estado}`,
      );

      if (o.png) {
        gravarPng(path.join(SAIDA, `${jogo.id}-${tamanho}.png`), img.largura, img.altura, (x, y) => img.em(x, y));
      }
    }
  }

  if (falhou > 0) {
    console.log(
      `\n${falhou} caso(s) com medida abaixo do mínimo: a grade fica ilegível nesse painel.\n` +
        `Ou a grade encolhe (menos colunas), ou o painel não serve para esse jogo.`,
    );
    process.exit(1);
  }
  console.log(`\ntodos os jogos legíveis nos ${o.tamanhos.length} tamanho(s). Quadros em ${SAIDA}`);
  return 0;
}

try {
  process.exit(main());
} catch (erro) {
  console.error(`erro: ${erro.message}`);
  process.exit(1);
}
