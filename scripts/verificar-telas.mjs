#!/usr/bin/env node
// Verifica TODAS as telas do launcher em vários tamanhos de painel.
//
// O `--dump-frame` do launcher desenha uma tela e grava um BMP. Sozinho ele só
// mostra a Home. Este script usa o `--tela` para percorrer as telas e, para
// cada uma, responde duas perguntas que o olho erra:
//
//   1. o conteúdo cabe? (margem nas quatro bordas — nada encostando na borda)
//   2. a tela aproveita o painel? (fração de pixels de fundo puro)
//
// Uso:
//   node scripts/verificar-telas.mjs
//   node scripts/verificar-telas.mjs --tamanho 640x480
//   node scripts/verificar-telas.mjs --tela home --tela busca
//   node scripts/verificar-telas.mjs --bin app/launcher/target/release/giochi-launcher
//
// Saída: dist/telas/<LxA>/<tela>.bmp e .png, mais uma tabela no terminal.
// Sai com 1 se alguma tela estourar a borda.

import { spawnSync } from "node:child_process";
import { mkdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir } from "node:os";
import { gravarPng, lerBmp } from "./verificar-frame.mjs";

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// Telas que o `--tela` do launcher aceita. Sonhar com uma tela nova sem
// acrescentá-la aqui deixa o teste cego para ela.
const TELAS = [
  "home",
  "biblioteca",
  "apps",
  "ajustes",
  "busca",
  "rapido",
  "detalhe",
  "energia",
  "sono",
  // Telas de rede: existem desde 06/11/2026 e ficaram de fora desta lista na
  // primeira versão — sonhar com uma tela nova sem acrescentá-la aqui deixa o
  // teste cego justamente para ela.
  "wifi",
  "senha",
  "bluetooth",
];

function argumentos(argv) {
  const opcoes = { tamanhos: [], telas: [], bin: null, saida: join(raiz, "dist/telas") };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const proximo = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} precisa de um valor`);
      return v;
    };
    if (a === "--tamanho") opcoes.tamanhos.push(proximo());
    else if (a === "--tela") opcoes.telas.push(proximo().toLowerCase());
    else if (a === "--bin") opcoes.bin = resolve(proximo());
    else if (a === "--saida") opcoes.saida = resolve(proximo());
    else if (a === "-h" || a === "--ajuda") {
      console.log(
        "uso: node scripts/verificar-telas.mjs [--tamanho LxA]... [--tela nome]... [--bin caminho] [--saida dir]",
      );
      process.exit(0);
    } else throw new Error(`argumento desconhecido: ${a}`);
  }
  // Os dois painéis que existem de fato: X35H e X55. 800×600 é uma tela
  // intermediária usada para ver se a transição entre os dois é suave.
  if (opcoes.tamanhos.length === 0) opcoes.tamanhos = ["640x480", "800x600", "1280x720"];
  if (opcoes.telas.length === 0) opcoes.telas = TELAS;
  for (const t of opcoes.telas) {
    if (!TELAS.includes(t)) {
      throw new Error(`tela desconhecida: ${t}\naceitas: ${TELAS.join(", ")}`);
    }
  }
  return opcoes;
}

function acharBinario(pedido) {
  if (pedido) {
    if (!existsSync(pedido)) throw new Error(`binário não encontrado: ${pedido}`);
    return pedido;
  }
  const candidatos = [
    join(raiz, "app/launcher/target/release/giochi-launcher"),
    join(raiz, "app/launcher/target/debug/giochi-launcher"),
  ];
  const achado = candidatos.find((c) => existsSync(c));
  if (!achado) {
    throw new Error(
      `nenhum binário do launcher. compile com:\n  bash scripts/compilar-launcher.sh`,
    );
  }
  return achado;
}

// As bibliotecas do SDL vêm do prefixo local que o `compilar-launcher.sh`
// extrai. Sem isto o carregador não acha `libSDL2_ttf` e o processo nem sobe.
function ambiente(bin) {
  const prefixos = [
    join(homedir(), ".cache/giochi-sdl2/root/usr/lib/x86_64-linux-gnu"),
    join(homedir(), ".cache/giochi-sdl2/root/usr/lib"),
    join(dirname(bin), "../../../.cache/giochi-sdl2/root/usr/lib/x86_64-linux-gnu"),
  ].filter((p) => existsSync(p));
  const atual = process.env.LD_LIBRARY_PATH ?? "";
  return {
    ...process.env,
    SDL_VIDEODRIVER: "dummy",
    SDL_RENDER_DRIVER: "software",
    LD_LIBRARY_PATH: [...prefixos, atual].filter(Boolean).join(":"),
  };
}

function renderizar(bin, tamanho, tela, destino) {
  const r = spawnSync(
    bin,
    ["--tamanho", tamanho, "--tela", tela, "--dump-frame", destino],
    { env: ambiente(bin), encoding: "utf8", timeout: 25_000 },
  );
  if (r.error) return `falhou ao executar: ${r.error.message}`;
  if (r.status !== 0) return `saiu com ${r.status}: ${(r.stderr ?? "").trim().split("\n")[0]}`;
  if (!existsSync(destino)) return "não gravou o quadro";
  return null;
}

/**
 * Mede o quadro.
 *
 * `margem` é a distância do conteúdo até cada borda. Conteúdo encostando na
 * borda quer dizer que algo foi desenhado fora do painel — foi assim que o
 * rodapé em 640×480 apareceu indo até y=484 de um painel de 480.
 */
function medir(img) {
  const limiar = 260; // soma RGB acima disto conta como conteúdo visível
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < img.altura; y++) {
    for (let x = 0; x < img.largura; x++) {
      const [r, g, b] = img.em(x, y);
      if (r + g + b > limiar) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return { vazio: true, margens: [0, 0, 0, 0], fundo: 1 };
  const margens = [x0, y0, img.largura - 1 - x1, img.altura - 1 - y1];
  return { vazio: false, margens, fundo: 0 };
}

function main() {
  const opcoes = argumentos(process.argv.slice(2));
  const bin = acharBinario(opcoes.bin);
  console.log(`binário: ${bin}`);
  console.log(`saída:   ${opcoes.saida}\n`);

  // Conteúdo a menos desta distância da borda é considerado estouro. Não é 0
  // porque um pixel de antialiasing na borda de um painel cheio é normal.
  const LIMITE = 6;
  let falhas = 0;

  for (const tamanho of opcoes.tamanhos) {
    const dir = join(opcoes.saida, tamanho);
    mkdirSync(dir, { recursive: true });
    const [l, a] = tamanho.split(/[xX]/).map(Number);
    if (!l || !a) throw new Error(`tamanho inválido: ${tamanho}`);
    console.log(`── ${tamanho} ──`);
    console.log("  tela          margens (esq/topo/dir/base)   ocupação   estado");

    for (const tela of opcoes.telas) {
      const bmp = join(dir, `${tela}.bmp`);
      const erro = renderizar(bin, tamanho, tela, bmp);
      if (erro) {
        console.log(`  ${tela.padEnd(12)}  ${erro}`);
        falhas++;
        continue;
      }
      const img = lerBmp(bmp);
      gravarPng(join(dir, `${tela}.png`), img.largura, img.altura, img.em);

      if (img.largura !== l || img.altura !== a) {
        console.log(
          `  ${tela.padEnd(12)}  quadro ${img.largura}x${img.altura}, esperado ${l}x${a}`,
        );
        falhas++;
        continue;
      }

      const { vazio, margens } = medir(img);
      if (vazio) {
        console.log(`  ${tela.padEnd(12)}  tela em branco`);
        falhas++;
        continue;
      }
      const estouro = margens.some((m) => m < LIMITE);
      // Quanto da tela o conteúdo cobre, para perceber tela "vazia demais".
      const ocupacao =
        ((img.largura - margens[0] - margens[2]) / img.largura) *
        ((img.altura - margens[1] - margens[3]) / img.altura);
      if (estouro) falhas++;
      console.log(
        `  ${tela.padEnd(12)}  ${margens.map((m) => String(m).padStart(3)).join(" ")}` +
          `                ${(ocupacao * 100).toFixed(0).padStart(3)}%      ${estouro ? "ESTOURO" : "ok"}`,
      );
    }
    console.log("");
  }

  if (falhas > 0) {
    console.error(`${falhas} problema(s). Quadros em ${opcoes.saida}`);
    process.exit(1);
  }
  console.log(`todas as telas cabem. Quadros em ${opcoes.saida}`);
}

main();
