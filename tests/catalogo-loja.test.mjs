// O gerador do catálogo da loja, e o contrato da mensagem assinada.
//
// ## Por que o literal aparece duas vezes no projeto
//
// O verificador é o Rust (`app/loja/crates/core/src/catalogo.rs`); o assinador é
// este lado, em JavaScript. Os dois montam a mesma mensagem, e ela é o
// **contrato**: bytes diferentes = catálogo que nunca verifica.
//
// Já aconteceu na primeira execução: o Rust escrevia `item=<slug>|…` e aqui
// saía `item|<slug>|…`. Um caractere. A mensagem de erro do verificador era
// "assinatura inválida — não foi a Giochi", que manda procurar no lugar errado.
//
// A defesa é a mesma dos dois lados: um literal fixo, idêntico, e teste.
// Divergir passa a ser impossível sem reprovar.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { assinarCatalogo, mensagemDoCatalogo, sha256Hex, VERSAO } from "../scripts/lib/catalogo-loja.mjs";
import { escreverTar } from "../scripts/lib/tar.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const GERADOR = path.join(RAIZ, "scripts", "gerar-catalogo-loja.mjs");
const EMPACOTADOR = path.join(RAIZ, "scripts", "empacotar.mjs");

// App de mentira, comitado em `tests/fixtures/app-teste/`.
//
// Antes estes testes empacotavam um app publicado de verdade. Isso fazia o
// teste do gerador depender do catálogo: apagar ou renomear aquele app
// reprovava testes que não têm nada a ver com ele. A fixture é um app
// completo (metadata + capa 320×320 + binário), então o empacotador real roda
// igual — só a origem do conteúdo deixou de ser acidental.
const APP_FIXTURE = path.join(RAIZ, "tests", "fixtures", "app-teste");

function catalogoDeTeste() {
  return {
    versao: VERSAO,
    gerado_em: 1_700_000_000,
    invalida_antes: null,
    itens: [
      {
        slug: "b",
        titulo: "B",
        ano: 0,
        descricao: "",
        preco: { centavos: 100, moeda: "BRL" },
        versao: "1.0.0",
        tamanho_bytes: 10,
        sha256: "aa",
        url: "https://x/b",
        assinatura: "sig",
        chave_pacote: "chave",
      },
    ],
  };
}

test("a mensagem bate byte a byte com o literal do verificador Rust", () => {
  // Este literal é o mesmo de `a_mensagem_bate_byte_a_byte_com_o_literal_do_gerador_js`
  // em `catalogo.rs`. Mexer num sem o outro reprova um dos dois.
  const esperado =
    "giochi-catalogo-v1\n" +
    "gerado=1700000000\n" +
    "invalida_antes=-\n" +
    "item=b|B|0|100|BRL|1.0.0|10|aa|https://x/b|sig|chave\n";
  assert.equal(mensagemDoCatalogo(catalogoDeTeste()).toString("utf8"), esperado);
});

test("a ordem dos itens não muda a mensagem", () => {
  // Dois catálogos com o **mesmo conteúdo** e ordem de inserção diferente têm de
  // assinar igual. Sem a ordenação por slug, reordenar a lista na interface
  // invalidaria a assinatura sem o conteúdo ter mudado.
  const base = catalogoDeTeste().itens[0];
  const itemA = { ...base, slug: "a", titulo: "A" };
  const itemB = { ...base, slug: "b", titulo: "B" };
  const a = { ...catalogoDeTeste(), itens: [itemA, itemB] };
  const b = { ...catalogoDeTeste(), itens: [itemB, itemA] };
  assert.equal(mensagemDoCatalogo(a).toString(), mensagemDoCatalogo(b).toString());
});

test("invalida_antes ausente vira '-', e presente vira número", () => {
  const c = catalogoDeTeste();
  assert.match(mensagemDoCatalogo(c).toString(), /invalida_antes=-\n/);
  c.invalida_antes = 1800000000;
  assert.match(mensagemDoCatalogo(c).toString(), /invalida_antes=1800000000\n/);
});

test("assinar produz assinatura de 64 bytes em base64", () => {
  const { gerarChaves } = { gerarChaves: () => null };
  const chaves = execFileSync(process.execPath, ["-e", `
    import(${JSON.stringify(path.join(RAIZ, "scripts/lib/publicar.mjs"))}).then(m => {
      const p = m.gerarChaves();
      process.stdout.write(JSON.stringify(p));
    });
  `], { encoding: "utf8" });
  const par = JSON.parse(chaves);
  const { assinatura } = assinarCatalogo(catalogoDeTeste(), par.privada);
  const bytes = Buffer.from(assinatura, "base64");
  assert.equal(bytes.length, 64, "Ed25519 tem 64 bytes crus");
  void gerarChaves;
});

// ── a ferramenta, de ponta a ponta ─────────────────────────

function comPacotes(cb) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-cat-"));
  const pacotes = path.join(dir, "pacotes");
  mkdirSync(pacotes, { recursive: true });
  try {
    return cb({ dir, pacotes });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function pacoteMinimo(slug, versao = "1.0.0") {
  const metadata = Buffer.from(
    JSON.stringify({
      title: slug.toUpperCase(),
      slug,
      developer: "Giochi",
      version: versao,
      arch: "aarch64",
      runtime: "native",
      entrypoint: `bin/${slug}`,
    }),
  );
  return escreverTar([
    { caminho: "metadata.json", conteudo: metadata, modo: 0o644 },
    { caminho: `bin/${slug}`, conteudo: Buffer.from("#!/bin/sh\n"), modo: 0o755 },
  ]);
}

function gerar(args) {
  try {
    return { codigo: 0, saida: execFileSync(process.execPath, [GERADOR, ...args], { encoding: "utf8", stdio: "pipe" }) };
  } catch (e) {
    return { codigo: e.status ?? 1, saida: `${e.stdout ?? ""}${e.stderr ?? ""}` };
  }
}

test("pacote sem .sig fica de fora e o código de saída avisa", () => {
  // Catálogo que anuncia um pacote que ninguém consegue verificar é pior que
  // catálogo vazio: o usuário baixa e a instalação recusa.
  comPacotes(({ dir, pacotes }) => {
    writeFileSync(path.join(pacotes, "solto.giochi"), pacoteMinimo("solto"));
    const r = gerar(["--pacotes", pacotes, "--saida", dir, "--chave", path.join(dir, "c.pem"), "--criar-chave"]);
    assert.equal(r.codigo, 1, r.saida);
    assert.match(r.saida, /sem \.sig/);
  });
});

test("pacote com assinatura em base64-texto (89 bytes) é recusado", () => {
  comPacotes(({ dir, pacotes }) => {
    writeFileSync(path.join(pacotes, "x.giochi"), pacoteMinimo("x"));
    writeFileSync(path.join(pacotes, "x.giochi.sig"), Buffer.alloc(89, 5));
    const r = gerar(["--pacotes", pacotes, "--saida", dir, "--chave", path.join(dir, "c.pem"), "--criar-chave"]);
    assert.equal(r.codigo, 1, r.saida);
    assert.match(r.saida, /89 bytes; Ed25519 tem 64/);
  });
});

test("catálogo gerado tem os campos que o Rust lê", () => {
  comPacotes(({ dir, pacotes }) => {
    // Pacote de verdade, assinado pelo empacotador de verdade.
    execFileSync(process.execPath, [
      EMPACOTADOR, APP_FIXTURE, "--saida", pacotes,
      "--chave", path.join(dir, "pacote.pem"),
    ], { cwd: RAIZ, stdio: "pipe" });

    const r = gerar([
      "--pacotes", pacotes, "--saida", dir,
      "--chave", path.join(dir, "cat.pem"), "--criar-chave",
      "--url-base", "https://cdn.giochi.example",
      "--preco", "app-teste=1990", "--ano", "app-teste=1980",
    ]);
    assert.equal(r.codigo, 0, r.saida);

    const json = JSON.parse(readFileSync(path.join(dir, "catalogo.json"), "utf8"));
    assert.ok(json.assinatura, "sem assinatura");
    assert.equal(Buffer.from(json.assinatura, "base64").length, 64);
    assert.equal(json.catalogo.versao, VERSAO);

    const item = json.catalogo.itens[0];
    assert.equal(item.slug, "app-teste");
    assert.equal(item.preco.centavos, 1990);
    assert.equal(item.ano, 1980);
    assert.match(item.url, /^https:\/\//);
    assert.equal(Buffer.from(item.assinatura, "base64").length, 64);
    // A chave do pacote é a do catálogo: é o que faz a cadeia fechar.
    assert.match(item.chave_pacote, /BEGIN PUBLIC KEY/);

    // O sha256 anunciado tem de ser o do arquivo, senão o comprador recebe
    // "download corrompido" e o defeito está aqui.
    const bytes = readFileSync(path.join(pacotes, "app-teste.giochi"));
    assert.equal(item.sha256, sha256Hex(bytes));
    assert.equal(item.tamanho_bytes, bytes.length);
  });
});

test("URL sem https não é gerada, e o aviso aparece", () => {
  // A loja recusa http://; gerar catálogo com http produziria lista que ela
  // própria não abre.
  comPacotes(({ dir, pacotes }) => {
    execFileSync(process.execPath, [
      EMPACOTADOR, APP_FIXTURE, "--saida", pacotes, "--chave", path.join(dir, "p.pem"),
    ], { cwd: RAIZ, stdio: "pipe" });
    const r = gerar(["--pacotes", pacotes, "--saida", dir, "--chave", path.join(dir, "c.pem"), "--criar-chave"]);
    assert.match(r.saida, /sem --url-base/);
    const json = JSON.parse(readFileSync(path.join(dir, "catalogo.json"), "utf8"));
    assert.ok(json.catalogo.itens[0].url.startsWith("./"), "não deve inventar host");
  });
});

test("--lista não grava nada e diz o que ficaria de fora", () => {
  comPacotes(({ dir, pacotes }) => {
    writeFileSync(path.join(pacotes, "sem-sig.giochi"), pacoteMinimo("sem-sig"));
    const r = gerar(["--pacotes", pacotes, "--chave", path.join(dir, "c.pem"), "--lista"]);
    assert.match(r.saida, /entram no catálogo \(0\)/);
    assert.match(r.saida, /ficam de fora \(1\)/);
    assert.throws(() => readFileSync(path.join(dir, "catalogo.json")), "gravou catálogo em modo --lista");
  });
});
