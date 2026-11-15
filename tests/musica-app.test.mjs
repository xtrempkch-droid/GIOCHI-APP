#!/usr/bin/env node
// Testes do app de música, do lado de fora do Rust.
//
// O que só dá para verificar aqui (e não no `cargo test`):
//
//  - **o app entra no catálogo do aparelho.** Era o objetivo da entrega: até
//    este app, os sete do repositório eram host ou tema e o catálogo saía vazio
//    por construção. Uma regressão aqui (status `scaffold`, `bin/` apagado,
//    `role` declarado por engano) não aparece em nenhum lugar — o app
//    simplesmente deixa de existir na tela;
//  - **o binário de `bin/` é do aparelho**, e não o build x86-64 do PC. O
//    validador confere que o arquivo existe e não confere a arquitetura; um
//    binário errado instala, abre no PC e morre no aparelho com
//    `Exec format error`, que não fala de arquitetura em lugar nenhum;
//  - **a capa da loja** é 320x320, determinística e gerada pelo script.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { DIR_BASE_PADRAO, gerarCatalogo } from "../scripts/lib/catalogo.mjs";
import { readPngSize } from "../scripts/lib/png.mjs";
import { desenharCapa, matizDe, paraPng } from "../scripts/gerar-capa-musica.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const APP = path.join(RAIZ, "app", "musica");

test("o app de música entra no catálogo do aparelho", () => {
  const { catalogo } = gerarCatalogo(path.join(RAIZ, "app"));
  const app = catalogo.apps.find((a) => a.id === "musica");

  assert.ok(app, "musica não está no catálogo: a aba de apps fica sem o app");
  assert.equal(app.dir, `${DIR_BASE_PADRAO}/musica`);
  assert.equal(app.entrypoint, "bin/giochi-musica");
  assert.equal(app.title, "Música");
  // Sem categoria a UI mostra "Aplicativo", que é o substituto de dado ausente;
  // um tocador anunciado como aplicativo genérico não ajuda ninguém.
  assert.equal(app.category, "Música");
});

test("metadata.json declara o pacote do aparelho", () => {
  const meta = JSON.parse(readFileSync(path.join(APP, "metadata.json"), "utf8"));

  assert.equal(meta.slug, "musica", "sem slug a loja deriva do título e move o app");
  assert.equal(meta.arch, "aarch64", "app de imagem roda no aparelho");
  assert.equal(meta.runtime, "native");
  assert.match(meta.version, /^\d+\.\d+\.\d+$/);
  assert.match(meta.entrypoint, /^bin\//);
  assert.equal(meta.license, "Proprietary");
});

test("o binário de bin/ é aarch64, não o build do PC", () => {
  const bin = path.join(APP, "bin", "giochi-musica");
  assert.ok(existsSync(bin), "bin/giochi-musica não existe");

  const cabecalho = readFileSync(bin).subarray(0, 20);
  assert.equal(cabecalho.subarray(0, 4).toString("binary"), "\x7fELF", "não é ELF");

  // `e_machine` fica no offset 18; o ELF do aparelho é little-endian.
  const maquina = cabecalho.readUInt16LE(18);
  assert.equal(maquina, 0xb7, `e_machine 0x${maquina.toString(16)}: isto não é aarch64`);

  // E tem de ser estático o bastante: `PT_INTERP` do aparelho, não do PC.
  const interpretador = readFileSync(bin).toString("binary").match(/\/lib\/ld-linux-[a-z0-9-]+\.so\.\d/);
  assert.equal(interpretador && interpretador[0], "/lib/ld-linux-aarch64.so.1");
});

test("a capa da loja é 320x320", () => {
  const tamanho = readPngSize(path.join(APP, "capa.png"));
  assert.deepEqual(tamanho, { width: 320, height: 320 });
});

test("a capa gerada é determinística", () => {
  // Mesma conta, mesmos bytes: se o gerador passar a sortear, o ícone da loja
  // muda a cada release sem que ninguém tenha pedido.
  const a = paraPng(desenharCapa());
  const b = paraPng(desenharCapa());
  assert.ok(a.equals(b), "duas gerações da mesma capa deram bytes diferentes");
});

test("a capa usa as três matizes do carrossel, e não três vizinhas", () => {
  const matizes = ["Faixa 1.mp3", "Vento.mp3", "Maré.flac"].map(matizDe);
  const distintos = new Set(matizes);
  assert.equal(distintos.size, 3);
  // Duas matizes a menos de 30° de distância viram um borrão só numa capa de
  // 64 px na lista da loja.
  const ordenadas = [...matizes].sort((x, y) => x - y);
  for (let i = 1; i < ordenadas.length; i += 1) {
    const distancia = Math.min(
      ordenadas[i] - ordenadas[i - 1],
      360 - (ordenadas[i] - ordenadas[i - 1]),
    );
    assert.ok(distancia > 30, `matizes ${ordenadas.join(", ")} ficaram vizinhas demais`);
  }
});

test("o gerador regrava a capa no caminho do app", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-capa-"));
  try {
    const destino = path.join(dir, "capa.png");
    execFileSync(process.execPath, [path.join(RAIZ, "scripts", "gerar-capa-musica.mjs"), "--saida", destino], {
      stdio: "pipe",
    });
    assert.deepEqual(readPngSize(destino), { width: 320, height: 320 });
    // O arquivo versionado tem de ser o que o script produz, senão o próximo
    // `node scripts/gerar-capa-musica.mjs` aparece como mudança sem motivo.
    assert.ok(
      readFileSync(destino).equals(readFileSync(path.join(APP, "capa.png"))),
      "app/musica/capa.png está desatualizado — rode node scripts/gerar-capa-musica.mjs",
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
