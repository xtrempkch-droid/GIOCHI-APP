#!/usr/bin/env node
// Traça a linha do tempo de uma corrida: quando cada volta é contada, e com o
// carro onde.
//
// ## Por que existe
//
// O contador de voltas errou duas vezes na mesma semana, e nas duas o sintoma
// apareceu longe da causa:
//
//   1. os adversários nasciam **antes** da linha, e 8 m de pista creditavam
//      962 (o `progresso` soma `(volta-1)·comprimento + s`);
//   2. o **jogador** nascia antes da linha pela mesma razão, e a primeira
//      passagem — que é a largada — virava "volta 2/2". Medido: `--passos 250`
//      já respondia `volta=2` com o carro a 0,5 m/s.
//
// Nos dois casos o estado final parecia plausível: `fase=Resultado`, `pos=1`.
// O que denuncia é a **ordem** dos acontecimentos, e ordem não se lê num
// retrato — só numa linha do tempo.
//
// ## Como lê a corrida
//
// O binário não tem laço de quadros exposto: cada execução avança N passos de
// simulação (`--passos`) e imprime o estado. Este script roda o binário várias
// vezes com passos crescentes e reporta **só as transições**: mudança de volta,
// de fase ou de `largou`.
//
// `--passos` é determinístico (passo fixo de 1/75 s, sem relógio), então a
// varredura dá a mesma linha do tempo em qualquer máquina — foi o que permitiu
// comparar duas versões do código com o mesmo número de passos.
//
//   node scripts/traco-voltas.mjs
//   node scripts/traco-voltas.mjs --mapa boavista --ate 6000 --passo 20
//   node scripts/traco-voltas.mjs --binario app/jogos/neon-drift/bin/giochi-neon-drift
//
// Sai com 1 quando a linha do tempo tem um erro conhecido — volta creditada
// antes da largada, ou largada que não acontece.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function argumentos(argv) {
  const o = {
    mapa: "avenida",
    ate: 4200,
    passo: 25,
    binario: resolve(RAIZ, "app/jogos/neon-drift/target/release/giochi-neon-drift"),
    tamanho: "640x480",
    // `--teclas up` reproduz o que um jogador faz: apertar o acelerador. Com
    // `--piloto` o carro acelera no mesmo passo em que a contagem acaba, e o
    // defeito da largada **não aparece** — o piloto esconde o primeiro metro,
    // que é exatamente onde o contador errava.
    teclas: null,
    json: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const proximo = () => argv[++i];
    if (a === "--mapa") o.mapa = proximo();
    else if (a === "--ate") o.ate = Number(proximo());
    else if (a === "--passo") o.passo = Number(proximo());
    else if (a === "--binario") o.binario = resolve(RAIZ, proximo());
    else if (a === "--tamanho") o.tamanho = proximo();
    else if (a === "--teclas") o.teclas = proximo();
    else if (a === "--json") o.json = true;
    else if (a === "--ajuda" || a === "-h") {
      console.log(
        "uso: traco-voltas.mjs [--mapa avenida|boavista] [--ate N] [--passo N] [--teclas up] [--binario caminho] [--json]",
      );
      process.exit(0);
    } else {
      console.error(`argumento desconhecido: ${a}`);
      process.exit(2);
    }
  }
  return o;
}

/** Estado da corrida depois de `passos` passos de simulação. */
function estadoEm(binario, o, passos) {
  const r = spawnSync(
    binario,
    [
      "--mapa", o.mapa,
      "--tela", "corrida",
      "--tamanho", o.tamanho,
      "--passos", String(passos),
      "--quadros", "1",
      ...(o.teclas ? ["--teclas", o.teclas] : ["--piloto"]),
      "--sem-som",
      "--json",
    ],
    { encoding: "utf8", maxBuffer: 1 << 20 },
  );
  if (r.error) throw new Error(`não consegui executar ${binario}: ${r.error.message}`);
  const linhas = (r.stdout || "").trim().split("\n");
  const ultima = linhas[linhas.length - 1];
  try {
    return JSON.parse(ultima);
  } catch {
    throw new Error(
      `saída não é JSON (o binário está velho?):\n${(r.stdout || "").slice(0, 400)}\n${(r.stderr || "").slice(0, 400)}`,
    );
  }
}

function principal() {
  const o = argumentos(process.argv.slice(2));
  if (!existsSync(o.binario)) {
    console.error(`binário não encontrado: ${o.binario}`);
    console.error("compile com: cd app/jogos/neon-drift && cargo build --release");
    process.exit(2);
  }

  const amostras = [];
  for (let p = 0; p <= o.ate; p += o.passo) {
    amostras.push({ amostra: p, ...estadoEm(o.binario, o, p) });
  }

  // Transições: o que mudou entre uma amostra e a anterior.
  const eventos = [];
  for (let i = 1; i < amostras.length; i++) {
    const antes = amostras[i - 1];
    const agora = amostras[i];
    const marcos = [];
    if (agora.volta !== antes.volta) marcos.push(`volta ${antes.volta} → ${agora.volta}`);
    if (agora.fase !== antes.fase) marcos.push(`fase ${antes.fase} → ${agora.fase}`);
    // `largou` só existe no binário com a correção; ausente = binário velho.
    if ("largou" in agora && agora.largou !== antes.largou) {
      marcos.push(`largou ${antes.largou} → ${agora.largou}`);
    }
    if (marcos.length) {
      eventos.push({
        // O rótulo é o `p` desta varredura, não o `passos` do JSON: aquele
        // conta os passos que o **laço de quadros** deu, e com `--quadros 1`
        // ele é sempre 0. Um rótulo errado numa tabela de tempo é pior que
        // tabela nenhuma.
        passos: agora.amostra,
        tempo: agora.tempo,
        s: agora.s,
        volta: agora.volta,
        fase: agora.fase,
        marcos,
      });
    }
  }

  if (o.json) {
    console.log(JSON.stringify({ mapa: o.mapa, eventos }, null, 2));
  } else {
    const primeira = amostras[0];
    const ultima = amostras[amostras.length - 1];
    console.log(`mapa ${o.mapa} · ${primeira.fase} → ${ultima.fase}`);
    console.log(`passos de 0 a ${o.ate}, amostra a cada ${o.passo} (1 passo = 1/75 s)`);
    console.log(`entrada: ${o.teclas ? `teclas ${o.teclas}` : "piloto artificial"}\n`);
    console.log("  passos   tempo       s   volta  fase          o que mudou");
    console.log("  ───────  ──────  ──────  ─────  ────────────  ────────────────────────");
    for (const e of eventos) {
      console.log(
        `  ${String(e.passos).padStart(7)}  ${e.tempo.toFixed(1).padStart(6)}  ${e.s
          .toFixed(1)
          .padStart(6)}  ${String(e.volta).padStart(5)}  ${e.fase.padEnd(12)}  ${e.marcos.join(", ")}`,
      );
    }
    if (!eventos.length) {
      console.log("  (nenhuma transição no intervalo)");
    }
    console.log(
      `\n  estado final: fase=${ultima.fase} volta=${ultima.volta} tempo=${ultima.tempo.toFixed(1)}s s=${ultima.s.toFixed(1)}`,
    );
  }

  // ── veredito ────────────────────────────────────────────────────────
  const problemas = [];
  const primeiraVolta = eventos.find((e) => e.marcos.some((m) => m.startsWith("volta")));
  if (primeiraVolta && primeiraVolta.tempo < 5) {
    problemas.push(
      `volta creditada a ${primeiraVolta.tempo.toFixed(1)} s — antes disso o carro nem cruzou a linha em velocidade`,
    );
  }
  const primeiraLargada = eventos.find((e) => e.marcos.some((m) => m.startsWith("largou")));
  if ("largou" in amostras[amostras.length - 1] && !primeiraLargada) {
    problemas.push(
      "o carro andou e nunca cruzou a linha: `largou` ficou falso (a linha não é detectada)",
    );
  }
  if (problemas.length) {
    console.error("\nFALHA");
    for (const p of problemas) console.error(`  - ${p}`);
    process.exit(1);
  }
}

principal();
