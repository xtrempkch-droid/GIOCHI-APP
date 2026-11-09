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
import { existsSync, mkdirSync, rmSync, statSync } from "node:fs";
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
    id: "cobrinha",
    // ## Por que não é mais a caixa da moldura
    //
    // Era: caixa envolvente da cor BORDA (45,52,74) menos 4 px, dividida por 24.
    // Isso valia para o tabuleiro **2D de frente**. O jogo virou 3D com câmera
    // em perspectiva, e a mesma conta passou a medir 2,0 px enquanto o tabuleiro
    // media 415..501 px de largura na tela — o teste reprovava um quadro
    // perfeito, que é o falso vermelho que este repositório combate.
    //
    // Em perspectiva a célula **não tem um tamanho**: a da fileira do fundo é
    // menor que a da frente. Legibilidade se decide pelo menor, então a medida
    // é a largura do tabuleiro na sua fileira mais estreita dividida pelas
    // colunas — o caso pessimista, e não a média.
    //
    // As colunas ao longo de x são `COLUNAS` (24): `--tamanho` desliga a
    // rotação do sistema, então o tabuleiro é sempre desenhado em paisagem.
    medir: (img) => {
      const larguras = largurasPorLinha(img, (c) => {
        // Tabuleiro: casas claras/escuras e a borda, todas azuladas. O fundo é
        // (6,9,19) e o placar é âmbar, então nenhum dos dois entra.
        const [r, g, b] = c;
        return b > r + 18 && b > g + 8 && r < 120 && g < 140;
      });
      if (larguras.length === 0) {
        return { nome: "célula", valor: 0, minimo: CELULA_MINIMA };
      }
      // Descarta as linhas de transição (borda superior/inferior), que medem
      // menos que o tabuleiro por causa do arredondamento do pixel.
      const maxima = Math.max(...larguras);
      const uteis = larguras.filter((w) => w >= maxima * 0.6);
      const estreita = Math.min(...uteis);
      const celula = estreita / COLUNAS;
      return { nome: "célula", valor: Math.round(celula * 10) / 10, minimo: CELULA_MINIMA };
    },
  },
  {
    id: "neon-drift",
    // Num jogo de corrida não há grade: o que decide "dá para jogar" é **ver o
    // carro**. Pedir `--tela corrida --piloto --passos` coloca o jogo em
    // andamento, porque na tela de título não há carro nenhum na pista.
    argumentos: ["--tela", "corrida", "--piloto", "--passos", "900", "--sem-som"],
    // A medida é a largura do carro do jogador em pixels de device. Abaixo de
    // ~40 px a silhueta vira um borrão verde e o jogador não sabe onde está o
    // próprio carro num painel de 3,5".
    medir: (img) => {
      const caixa = caixaDeCor(img, (c) => c[1] > 140 && c[1] > c[0] + 40 && c[1] > c[2] + 40);
      return { nome: "largura do carro", valor: caixa.w, minimo: 40 };
    },
  },
];

/** Quantas colunas o `cobrinha` desenha ao longo de x (espelha o núcleo). */
const COLUNAS = 24;

/** Largura ocupada, por linha de pixel, pelos pixels que passam no teste. */
function largurasPorLinha(img, teste) {
  const larguras = [];
  for (let y = 0; y < img.altura; y++) {
    let primeiro = -1;
    let ultimo = -1;
    for (let x = 0; x < img.largura; x++) {
      if (!teste(img.em(x, y))) continue;
      if (primeiro < 0) primeiro = x;
      ultimo = x;
    }
    if (primeiro >= 0) larguras.push(ultimo - primeiro + 1);
  }
  return larguras;
}

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
        `compile com: cd app/jogos/${id} && cargo build --release`,
    );
  }
  return alvo;
}

/**
 * Os jogos que existem de fato, não a lista escrita à mão.
 *
 * A lista `JOGOS` descreve **como medir** cada jogo — isso é conhecimento de
 * cada um e não dá para derivar. Mas *quais* jogos medir é fato do diretório,
 * e manter isso à mão significava que apagar um jogo deixava o script
 * procurando um binário que nunca mais existiria. Mesma regra do catálogo:
 * projeção, não segunda fonte da verdade.
 */
function jogosPresentes() {
  return JOGOS.filter((jogo) =>
    existsSync(path.join(RAIZ, "app", "jogos", jogo.id, "metadata.json")),
  );
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

  // Sem jogo no diretório não há o que medir — e isso não é falha. Antes, a
  // lista fixa fazia o script parar em "binário ausente" mesmo com o grupo
  // vazio, o que parece defeito de build e é ausência de conteúdo.
  const jogos = jogosPresentes();
  if (jogos.length === 0) {
    console.log(
      "nenhum jogo em app/jogos/ — nada a medir.\n" +
        "Um jogo entra aqui quando a pasta tem `metadata.json` e o `id` está " +
        "descrito em JOGOS neste script (a medida varia por jogo).",
    );
    return 0;
  }

  // `dummy` respeita o tamanho pedido; `x11` devolveria o desktop.
  //
  // Mas `dummy` **não tem OpenGL**: o `cobrinha` 3D precisa de um contexto GLES e
  // falhava com `OpenGL support is either not configured in SDL`, que lido de
  // fora parece defeito do jogo. `offscreen` (EGL/OSMesa) respeita o tamanho
  // pedido e **cria** contexto — medido: grava o BMP 640x480 com `instancing
  // ativo`. A ordem é `offscreen` e depois `dummy`, e o primeiro que produzir
  // BMP fica valendo para todos os jogos da rodada.
  const candidatos = [
    { ...process.env, SDL_VIDEODRIVER: "offscreen", SDL_RENDER_DRIVER: "software" },
    { ...process.env, SDL_VIDEODRIVER: "dummy", SDL_RENDER_DRIVER: "software" },
  ];
  let falhou = 0;

  for (const tamanho of o.tamanhos) {
    console.log(`\n── ${tamanho} ──`);
    console.log("  jogo        medida              valor    mínimo   estado");

    for (const jogo of jogos) {
      const bin = binarioDoJogo(jogo.id);
      const bmp = path.join(SAIDA, `${jogo.id}-${tamanho}.bmp`);
      let erro = null;
      let gravou = false;
      for (const ambiente of candidatos) {
        rmSync(bmp, { force: true });
        try {
          execFileSync(bin, ["--tamanho", tamanho, ...(jogo.argumentos ?? []), "--quadro", bmp], {
            env: ambiente,
            stdio: "ignore",
          });
        } catch (e) {
          erro = e;
          continue;
        }
        if (existsSync(bmp) && statSync(bmp).size > 0) {
          gravou = true;
          break;
        }
      }
      if (!gravou) {
        // Sem BMP não há medida — dizer isso é diferente de "o jogo não serve
        // nesse painel", que é o que a tabela abaixo afirmaria.
        console.log(
          `  ${jogo.id.padEnd(11)} não gravou quadro em ${tamanho} (${erro?.message?.split("\n")[0] ?? "sem BMP"})`,
        );
        falhou++;
        continue;
      }

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
