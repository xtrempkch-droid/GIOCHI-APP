#!/usr/bin/env node
// capacidades-input.mjs — quais códigos de tecla cada dispositivo de entrada
// afirma poder emitir, lido de /proc/bus/input/devices.
//
//   node scripts/capacidades-input.mjs                       # lê o /proc local
//   node scripts/capacidades-input.mjs --arquivo dados.txt   # lê um dump
//   node scripts/capacidades-input.mjs --codigo 116          # quem emite KEY_POWER
//   node scripts/capacidades-input.mjs --dispositivo gpio    # só um dispositivo
//
// Complementa `aparelho.sh input`, que mostra o que o kernel **entregou**. Este
// mostra o que ele **promete**. Quando um botão não reage, a pergunta é qual dos
// dois falta: o evento ou a capacidade.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  nomeDaTecla,
  parseDispositivos,
  porCodigo,
  quemEmite,
  rotulo,
} from "./lib/capacidades.mjs";

const PROC = "/proc/bus/input/devices";

function lerArgs(argv) {
  const args = { arquivo: PROC };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--arquivo") args.arquivo = argv[++i];
    else if (a === "--codigo") args.codigo = Number(argv[++i]);
    else if (a === "--dispositivo") args.dispositivo = argv[++i];
    else if (a === "--ajuda" || a === "-h") args.ajuda = true;
    else throw new Error(`argumento desconhecido: ${a}`);
  }
  return args;
}

function ajuda() {
  console.log(`
capacidades-input.mjs — códigos de tecla que cada dispositivo declara

  --arquivo <caminho>    origem (padrão: ${PROC})
  --codigo <n>           mostra só quem afirma emitir o código <n>
  --dispositivo <trecho> filtra por trecho do nome

Sem argumentos, lista todos os dispositivos e os códigos de cada um.
`);
}

function principal() {
  let args;
  try {
    args = lerArgs(process.argv);
  } catch (e) {
    console.error(`erro: ${e.message}`);
    process.exit(2);
  }
  if (args.ajuda) {
    ajuda();
    return;
  }
  if (!fs.existsSync(args.arquivo)) {
    console.error(`erro: ${args.arquivo} não existe`);
    process.exit(2);
  }

  const dispositivos = parseDispositivos(fs.readFileSync(args.arquivo, "utf8"));

  if (args.codigo !== undefined) {
    if (!Number.isInteger(args.codigo) || args.codigo < 0) {
      console.error("erro: --codigo precisa ser inteiro não negativo");
      process.exit(2);
    }
    const quem = quemEmite(dispositivos, args.codigo);
    console.log(`código ${rotulo(args.codigo)}`);
    if (quem.length === 0) {
      console.log("  nenhum dispositivo declara este código");
      console.log(
        "  se o evento aparece em `aparelho.sh input`, o kernel entrega mas o\n" +
          "  bitmask não anuncia — vale como aviso, não como veredito."
      );
      process.exit(1);
    }
    for (const d of quem) {
      const onde = d.eventos.length > 0 ? d.eventos.join(", ") : "(sem eventN)";
      console.log(`  ${onde.padEnd(12)} ${d.nome}`);
    }
    return;
  }

  const filtro = args.dispositivo?.toLowerCase();
  let total = 0;
  for (const d of dispositivos) {
    if (filtro && !d.nome.toLowerCase().includes(filtro)) continue;
    if (d.key.size === 0) continue;
    total++;
    const eventos = d.handlers.filter((h) => h.startsWith("event")).join(", ") || "(sem eventN)";
    console.log(`\n${d.nome}   [${eventos}]   ${d.key.size} código(s)`);

    const codigos = [...d.key].sort((a, b) => a - b);
    // Faixas contíguas ficam numa linha: 13 códigos soltos viram 13 linhas de
    // ruído, e o que interessa é o conjunto.
    const faixas = [];
    let inicio = codigos[0];
    let anterior = codigos[0];
    for (const c of codigos.slice(1)) {
      if (c === anterior + 1) {
        anterior = c;
        continue;
      }
      faixas.push([inicio, anterior]);
      inicio = c;
      anterior = c;
    }
    faixas.push([inicio, anterior]);

    for (const [de, ate] of faixas) {
      if (de === ate) {
        console.log(`  ${rotulo(de).padEnd(22)}`);
      } else {
        const nomes = codigos
          .filter((c) => c >= de && c <= ate)
          .map((c) => nomeDaTecla(c) ?? String(c))
          .join(", ");
        console.log(`  ${de}-${ate}`.padEnd(22) + ` ${nomes}`);
      }
    }
  }
  if (total === 0) {
    console.log("nenhum dispositivo declara códigos de tecla" + (filtro ? ` (filtro "${args.dispositivo}")` : ""));
    process.exit(1);
  }
}

// Só executa quando chamado direto: os testes importam o módulo de lib.
const chamadoDireto =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);
if (chamadoDireto) principal();
