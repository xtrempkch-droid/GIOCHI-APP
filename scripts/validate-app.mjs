#!/usr/bin/env node
// Valida uma pasta de app, ou todas as registradas em app/apps.json.
//
// Uso:
//   node scripts/validate-app.mjs app/meu-app
//   node scripts/validate-app.mjs --all
//   node scripts/validate-app.mjs --all --json
//
// O status vem de app/apps.json:
//   scaffold — fonte ainda não migrada; ausência de entrypoint é aviso, não erro
//   ready    — deve estar completo; qualquer erro reprova
//
// Códigos de saída: 0 = ok, 1 = erro de validação, 2 = uso/pasta inválida.
import { existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { listApps, readIndex, validateApp } from "./lib/apps.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const APP_ROOT = path.join(ROOT, "app");

const argv = process.argv.slice(2);
const asJson = argv.includes("--json");
const all = argv.includes("--all");
const targets = argv.filter((item) => !item.startsWith("--"));

if (!all && targets.length === 0) {
  console.error("Uso: node scripts/validate-app.mjs <pasta-do-app> | --all [--json]");
  process.exit(2);
}

const index = readIndex(APP_ROOT);
const results = [];

for (const error of index.errors) {
  results.push({ ok: false, app: "app/apps.json", status: "ready", errors: [error], warnings: [] });
}

if (all) {
  const registered = new Set();

  for (const [id, entry] of index.entries) {
    registered.add(id);
    const folder = entry.folder || id;
    const dir = path.join(APP_ROOT, folder);

    if (!existsSync(dir) || !statSync(dir).isDirectory()) {
      results.push({
        ok: false,
        app: id,
        status: entry.status,
        errors: [
          {
            code: "pasta_registrada_ausente",
            path: `app/${folder}`,
            message: "registrado em apps.json, mas a pasta não existe",
          },
        ],
        warnings: [],
      });
      continue;
    }

    results.push(
      validateApp(dir, {
        status: entry.status,
        distribution: entry.distribution || "image",
      }),
    );
  }

  // Pasta na árvore sem registro no índice: fica invisível para a loja.
  for (const dir of listApps(APP_ROOT)) {
    const name = path.basename(dir);
    if (!registered.has(name)) {
      results.push({
        ok: false,
        app: name,
        status: "ready",
        errors: [
          {
            code: "app_nao_registrado",
            path: `app/${name}`,
            message: "pasta existe mas não está em app/apps.json",
          },
        ],
        warnings: [],
      });
    }
  }
} else {
  for (const target of targets) {
    const dir = path.resolve(target);
    const name = path.basename(dir);
    const entry = index.entries.get(name);
    results.push(
      validateApp(dir, {
        status: entry ? entry.status : "ready",
        distribution: entry ? entry.distribution || "image" : "image",
      }),
    );
  }
}

const failed = results.filter((item) => !item.ok);

if (asJson) {
  console.log(JSON.stringify({ ok: failed.length === 0, results }, null, 2));
} else {
  console.log("Giochi app — validação\n");
  for (const result of results) {
    const mark = result.ok ? "OK   " : "ERRO ";
    const suffix = result.status === "scaffold" ? "  [scaffold]" : "";
    console.log(`${mark} ${result.app}${suffix}`);
    for (const error of result.errors) {
      console.log(`         [${error.code}] ${error.path}: ${error.message}`);
    }
    for (const warning of result.warnings) {
      console.log(`         aviso [${warning.code}] ${warning.path}: ${warning.message}`);
    }
  }
  console.log("");
  if (failed.length === 0) {
    const scaffolds = results.filter((item) => item.status === "scaffold").length;
    const note = scaffolds > 0 ? ` (${scaffolds} em scaffold)` : "";
    console.log(`OK: ${results.length} app(s) válido(s)${note}.`);
  } else {
    console.error(`FALHOU: ${failed.length} de ${results.length} app(s) com erro.`);
  }
}

process.exit(failed.length === 0 ? 0 : 1);
