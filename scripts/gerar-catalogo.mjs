#!/usr/bin/env node
// Gera o catálogo do aparelho a partir de `app/apps.json` + os `metadata.json`.
//
// Uso:
//   node scripts/gerar-catalogo.mjs                    # imprime na tela
//   node scripts/gerar-catalogo.mjs --saida index.json # grava
//   node scripts/gerar-catalogo.mjs --lista            # só o que entra e o que não
//   node scripts/gerar-catalogo.mjs --tudo             # ignora a checagem de binário
//   node scripts/gerar-catalogo.mjs --instalados f.json  # junta pacotes da loja
//
// Normalmente não se chama isto à mão: `bash scripts/aparelho.sh catalogo` gera
// e envia para o aparelho num passo.
//
// ## Por que a checagem de binário existe
//
// Catálogo que aponta para executável ausente é **pior** que catálogo vazio: o
// usuário vê o jogo, aperta, e nada acontece. Sem mensagem. Já aconteceu neste
// repositório com `entrypoint` de app em `scaffold`.
//
// `--tudo` desliga a checagem, e serve para inspecionar o que *seria* publicado
// sem instalar nada.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { readIndex } from "./lib/apps.mjs";
import { gerarCatalogo, mesclarInstalados, serializar, DIR_BASE_PADRAO } from "./lib/catalogo.mjs";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const APP_ROOT = path.join(RAIZ, "app");

function analisarArgs(argv) {
  const opcoes = {
    saida: null,
    lista: false,
    apenasInstalados: true,
    dirBase: DIR_BASE_PADRAO,
    instalados: null,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--saida") opcoes.saida = argv[++i];
    else if (a === "--lista") opcoes.lista = true;
    else if (a === "--tudo") opcoes.apenasInstalados = false;
    else if (a === "--dir-base") opcoes.dirBase = argv[++i];
    else if (a === "--instalados") opcoes.instalados = argv[++i];
    else if (a === "-h" || a === "--ajuda") opcoes.ajuda = true;
    else if (a.startsWith("--")) throw new Error(`opção desconhecida: ${a}`);
  }
  return opcoes;
}

function main() {
  const opcoes = analisarArgs(process.argv.slice(2));
  if (opcoes.ajuda) {
    console.log(
      "uso: node scripts/gerar-catalogo.mjs [--saida ARQ] [--lista] [--tudo] [--dir-base DIR] " +
        "[--instalados f.json]",
    );
    return 0;
  }

  const doRepo = gerarCatalogo(APP_ROOT, {
    dirBase: opcoes.dirBase,
    apenasInstalados: opcoes.apenasInstalados,
  });

  // Pacotes da loja não estão no `apps.json`: chegam no aparelho e são
  // extraídos em `/opt/giochi/apps/<slug>/`. Quem sabe quais existem é o
  // aparelho, então a varredura vem de lá (`aparelho.sh catalogo` colhe).
  //
  // Sem isto, instalar um jogo pela loja o deixaria **invisível, sem erro** —
  // o mesmo defeito do catálogo escrito à mão.
  let { catalogo, avisos } = doRepo;
  if (opcoes.instalados) {
    if (!existsSync(opcoes.instalados)) {
      throw new Error(`--instalados: arquivo não existe: ${opcoes.instalados}`);
    }
    let lista;
    try {
      lista = JSON.parse(readFileSync(opcoes.instalados, "utf8"));
    } catch (e) {
      throw new Error(`--instalados: JSON inválido: ${e.message}`);
    }
    if (!Array.isArray(lista)) throw new Error("--instalados: esperava uma lista");

    // O `role` vem do `apps.json`, **não** do catálogo já filtrado: o launcher
    // está fora dele justamente por ser shell, e é essa informação que impede a
    // cópia instalada de voltar como cartão. Ver `mesclarInstalados`.
    const papeis = new Map();
    for (const [id, entrada] of readIndex(APP_ROOT).entries) {
      if (entrada.role) papeis.set(id, entrada.role);
    }

    const juntos = mesclarInstalados(catalogo, lista, {
      dirBase: opcoes.dirBase,
      papeis,
    });
    catalogo = juntos.catalogo;
    avisos = [...avisos, ...juntos.avisos];
  }

  if (opcoes.lista) {
    console.log(`entram no catálogo (${catalogo.apps.length}):`);
    for (const app of catalogo.apps) {
      console.log(`  ${app.id.padEnd(24)} ${app.dir}/${app.entrypoint}`);
    }
    if (avisos.length > 0) {
      console.log(`\nficam de fora (${avisos.length}):`);
      for (const { id, motivo } of avisos) {
        console.log(`  ${id.padEnd(24)} ${motivo}`);
      }
    }
    return 0;
  }

  const texto = serializar(catalogo);
  if (opcoes.saida) {
    mkdirSync(path.dirname(path.resolve(opcoes.saida)), { recursive: true });
    writeFileSync(opcoes.saida, texto, "utf8");
    // stderr: a saída padrão fica livre para o JSON, para `> arquivo` funcionar.
    console.error(`catálogo: ${catalogo.apps.length} app(s) em ${opcoes.saida}`);
    for (const { id, motivo } of avisos) {
      console.error(`  fora: ${id} — ${motivo}`);
    }
  } else {
    process.stdout.write(texto);
  }
  return 0;
}

try {
  process.exit(main());
} catch (erro) {
  console.error(`erro: ${erro.message}`);
  process.exit(1);
}
