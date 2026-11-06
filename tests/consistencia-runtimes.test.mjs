// O enum de `runtime` existe em **quatro** lugares. Este teste exige que os
// quatro concordem.
//
// ## Por que isto é teste e não documentação
//
// O enum já divergiu duas vezes, e das duas vezes o sintoma apareceu longe da
// causa:
//
// 1. `native` foi acrescentado ao schema em 02/11/2026 e **não** ao forge: o
//    forge recusava todo app deste repositório com "runtime 'native'
//    desconhecido".
// 2. Corrigido o forge, o **store** ficou para trás: recusou os dois pacotes do
//    próprio projeto com "runtime 'native' fora de godot|retroarch|proton".
//
// São pacotes Python separados de propósito (loja e forge são apps diferentes,
// e um não deve depender do outro em tempo de execução). A duplicação é a
// decisão; a divergência é o risco — e é o risco que se fecha aqui.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { RUNTIME_VALUES } from "../scripts/lib/apps.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const ler = (rel) => readFileSync(path.join(RAIZ, rel), "utf8");

/** Extrai os literais de string de um trecho `("a", "b")` ou `{"a", "b"}`. */
function literais(trecho) {
  return [...trecho.matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]).sort();
}

test("o schema e o validador concordam sobre os runtimes", () => {
  const schema = JSON.parse(ler("schemas/metadata.schema.json"));
  assert.deepEqual(
    [...schema.properties.runtime.enum].sort(),
    [...RUNTIME_VALUES].sort(),
    "schemas/metadata.schema.json e scripts/lib/apps.mjs divergiram",
  );
});

test("ALLOWED_RUNTIMES do forge é igual ao schema", () => {
  const fonte = ler("app/forge/src/giochi_forge/package.py");
  const m = fonte.match(/ALLOWED_RUNTIMES\s*=\s*\{([^}]*)\}/);
  assert.ok(m, "ALLOWED_RUNTIMES não encontrado em package.py");
  assert.deepEqual(
    literais(m[1]),
    [...RUNTIME_VALUES].sort(),
    "o forge vai recusar pacotes com um runtime que o schema aceita",
  );
});

test("RUNTIMES_OK do store é igual ao schema", () => {
  const fonte = ler("app/store/src/giochi_store/catalogo.py");
  const m = fonte.match(/RUNTIMES_OK\s*=\s*\(([^)]*)\)/);
  assert.ok(m, "RUNTIMES_OK não encontrado em catalogo.py");
  assert.deepEqual(
    literais(m[1]),
    [...RUNTIME_VALUES].sort(),
    "o store vai recusar pacotes com um runtime que o schema aceita",
  );
});

// Um dos dois apps é o que o outro consome: o forge empacota, o store lê. Se
// concordarem com o schema mas não entre si, o defeito é o mesmo.
test("as duas listas de runtime são idênticas entre si", () => {
  const forge = ler("app/forge/src/giochi_forge/package.py").match(
    /ALLOWED_RUNTIMES\s*=\s*\{([^}]*)\}/,
  )[1];
  const store = ler("app/store/src/giochi_store/catalogo.py").match(
    /RUNTIMES_OK\s*=\s*\(([^)]*)\)/,
  )[1];
  assert.deepEqual(literais(forge), literais(store));
});
