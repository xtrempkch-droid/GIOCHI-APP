#!/usr/bin/env node
// Confere um `buttons.map` contra os símbolos que o launcher entende.
//
// ## Por que isto existe
//
// Um símbolo que o app não conhece é descartado **em silêncio**: o botão
// simplesmente não funciona, sem erro, sem aviso na tela. Aconteceu duas vezes
// neste projeto — comentário no fim da linha (`up = BTN_DPAD_UP  # gpio4 PA0`,
// que deixava 1 de 13 botões) e `KEY_VOLUMEUP` fora do tradutor.
//
// E acontece **antes de haver hardware**: o mapa do R35S é gerado do DTB, e se
// ele declarar um símbolo desconhecido, ninguém descobre até alguém apertar o
// botão num aparelho que talvez nem esteja na mão. Este script fecha isso sem o
// aparelho: lê o mapa, pergunta ao launcher o que ele conhece, e compara.
//
// ## Por que a lista vem do binário, e não de uma cópia aqui
//
// Mesma razão do catálogo ser projeção: uma lista de símbolos escrita neste
// arquivo envelheceria separada do `code_for`. O `--simbolos` do launcher imprime
// a lista **do tradutor**, e há um teste Rust (`lista_de_simbolos_bate_com_o_
// tradutor`) que impede a lista publicada de divergir do que ele aceita.
//
// Se o binário não estiver disponível, o script falha em vez de adivinhar.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Extrai `nome = SÍMBOLO` do mapa, cortando comentário.
 *
 * O corte do `#` é o que conserta o bug documentado: `up = BTN_DPAD_UP  # gpio4
 * PA0` precisa virar `BTN_DPAD_UP`, não `BTN_DPAD_UP  # gpio4 PA0`.
 */
export function lerMapa(texto) {
  const botoes = [];
  const calibragem = [];

  for (const [numero, linha] of texto.split("\n").entries()) {
    const semComentario = linha.split("#")[0].trim();
    if (!semComentario) continue;

    const igual = semComentario.indexOf("=");
    if (igual < 0) {
      // Linha sem `=` não é entrada: é cabeçalho ou separador.
      continue;
    }

    const nome = semComentario.slice(0, igual).trim().toLowerCase();
    const valor = semComentario.slice(igual + 1).trim();
    if (!nome || !valor) continue;

    if (nome.startsWith("axis_")) {
      calibragem.push({ nome, valor, linha: numero + 1 });
      continue;
    }

    // Símbolo é o primeiro token: `BTN_EAST` ou `BTN_EAST extra`.
    const simbolo = valor.split(/\s+/)[0];
    botoes.push({ nome, simbolo, linha: numero + 1 });
  }

  return { botoes, calibragem };
}

/** Símbolos que o launcher entende, perguntando ao binário. */
export function simbolosDoLauncher(bin) {
  const saida = spawnSync(bin, ["--simbolos"], {
    encoding: "utf8",
    env: { ...process.env, SDL_VIDEODRIVER: "dummy" },
  });
  if (saida.error) throw new Error(`não consegui executar ${bin}: ${saida.error.message}`);
  if (saida.status !== 0) {
    throw new Error(`${bin} --simbolos saiu com ${saida.status}:\n${saida.stderr}`);
  }
  const lista = saida.stdout.split("\n").map((l) => l.trim()).filter(Boolean);
  if (lista.length === 0) throw new Error(`${bin} --simbolos não imprimiu nada`);
  return new Set(lista);
}

/**
 * Compara o mapa com o que o app conhece.
 *
 * Devolve o que falta e o que sobra — os dois lados importam. Um símbolo no mapa
 * que o app não conhece é **botão morto**; um símbolo que o app conhece e o mapa
 * não usa pode ser só ausência de hardware, então é informativo.
 */
export function comparar(botoes, conhecidos) {
  const mortos = botoes.filter((b) => !conhecidos.has(b.simbolo));
  const usados = new Set(botoes.map((b) => b.simbolo));
  const naoUsados = [...conhecidos].filter((s) => !usados.has(s));
  return { mortos, naoUsados };
}

function argumentos(argv) {
  const opcoes = {
    mapa: null,
    bin: join(raiz, "app/launcher/target/release/giochi-launcher"),
    json: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const proximo = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} precisa de um valor`);
      return v;
    };
    if (a === "--mapa") opcoes.mapa = resolve(proximo());
    else if (a === "--bin") opcoes.bin = resolve(proximo());
    else if (a === "--json") opcoes.json = true;
    else if (a === "-h" || a === "--ajuda") {
      console.log(
        [
          "verificar-mapa.mjs — confere um buttons.map contra o launcher",
          "",
          "  --mapa <arquivo>   buttons.map a verificar (obrigatório)",
          "  --bin <caminho>    launcher (padrão: target/release)",
          "  --json             saída para script",
          "",
          "Sai com 1 quando o mapa declara símbolo que o app não conhece: esse",
          "botão seria descartado em silêncio no aparelho.",
        ].join("\n"),
      );
      process.exit(0);
    } else {
      throw new Error(`argumento desconhecido: ${a}`);
    }
  }
  if (!opcoes.mapa) throw new Error("--mapa é obrigatório");
  return opcoes;
}

function principal() {
  const opcoes = argumentos(process.argv.slice(2));

  if (!existsSync(opcoes.mapa)) throw new Error(`mapa não existe: ${opcoes.mapa}`);
  if (!existsSync(opcoes.bin)) {
    throw new Error(
      `launcher não encontrado: ${opcoes.bin}\n` +
        "compile antes: bash scripts/compilar-launcher.sh",
    );
  }

  const texto = readFileSync(opcoes.mapa, "utf8");
  const { botoes, calibragem } = lerMapa(texto);
  const conhecidos = simbolosDoLauncher(opcoes.bin);
  const { mortos, naoUsados } = comparar(botoes, conhecidos);

  if (opcoes.json) {
    console.log(
      JSON.stringify(
        {
          mapa: opcoes.mapa,
          total: botoes.length,
          mortos: mortos.map((b) => ({ nome: b.nome, simbolo: b.simbolo, linha: b.linha })),
          calibragem: calibragem.length,
          naoUsados,
        },
        null,
        2,
      ),
    );
    process.exit(mortos.length === 0 ? 0 : 1);
  }

  console.log(`mapa: ${opcoes.mapa}`);
  console.log(`  ${botoes.length} botão(ões), ${calibragem.length} entrada(s) de calibragem`);
  console.log(`  símbolos conhecidos pelo launcher: ${conhecidos.size}`);

  if (mortos.length === 0) {
    console.log("  todos os símbolos são reconhecidos");
  } else {
    console.log("");
    console.log(`  ${mortos.length} símbolo(s) NÃO reconhecido(s) — esses botões ficariam mortos:`);
    for (const b of mortos) {
      console.log(`    linha ${b.linha}: ${b.nome} = ${b.simbolo}`);
    }
  }

  if (naoUsados.length > 0) {
    console.log("");
    console.log(`  informativo: ${naoUsados.length} símbolo(s) que o app conhece e este mapa não usa`);
    console.log(`    ${naoUsados.join(", ")}`);
  }

  process.exit(mortos.length === 0 ? 0 : 1);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    principal();
  } catch (e) {
    console.error(`erro: ${e.message}`);
    process.exit(2);
  }
}
