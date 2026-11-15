#!/usr/bin/env node
// Testes do `scripts/abrir-issue.mjs` — o caminho que leva um corpo pronto de
// `docs/issues-para-outros-repos.md` até uma issue no repositório vizinho.
//
// Por que ele merece teste, e não é zelo: **toda falha dele é silenciosa**.
//
//  - título sem o `**Título:**` cai no cabeçalho do documento, e a issue nasce
//    com um título plausível e diferente do combinado;
//  - link relativo (`idiomas.md`) não dá erro em lugar nenhum — abre 404 na
//    página da issue, semanas depois, para quem lê;
//  - número inexistente aceito em silêncio devolveria a URL de outra seção, e a
//    pessoa abriria o pedido errado.
//
// Nenhuma dessas três coisas quebra o script. É a mesma família do §13 do
// AGENTS.md: ferramenta que mede a intenção ("consumi o arquivo") em vez do
// efeito ("a issue sai com este texto").
//
// Os testes usam `test()` do `node:test`, e não `assert` solto no topo: as
// asserções rodariam do mesmo jeito, mas **fora do contador** do `run-all.mjs` —
// medido, o `run-all` dizia 222 antes e depois de o arquivo existir. É o mesmo
// defeito de um teste que existe e não roda, que é o motivo de o runner
// descobrir a lista por `readdir` em vez de `import` fixo.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";

const RAIZ = path.resolve(import.meta.dirname, "..");
const SCRIPT = path.join(RAIZ, "scripts", "abrir-issue.mjs");
const DOC = path.join(RAIZ, "docs", "issues-para-outros-repos.md");

const roda = (...args) => execFileSync(process.execPath, [SCRIPT, ...args], { encoding: "utf8" });

/** As seções do documento, do jeito que o próprio doc as declara. */
function secoesDoDoc() {
  return readFileSync(DOC, "utf8")
    .split("\n")
    .filter((l) => /^## \d+\. `[^`]+` — /.test(l))
    .map((l) => ({ numero: Number(/^## (\d+)\./.exec(l)[1]), repo: /`([^`]+)`/.exec(l)[1] }));
}

const secoes = secoesDoDoc();
const secoesProcedencia = [];

test("o documento declara os cinco pedidos", () => {
  // Se este número mudar, os testes abaixo continuam verdes — cada um itera
  // sobre `secoes`. Este é o que segura a regressão de "some um pedido".
  assert.ok(secoes.length >= 5, `esperava ao menos 5 pedidos, achei ${secoes.length}`);
});

test("a numeração dos pedidos é contígua, de 1 a N", () => {
  // As referências cruzadas deste repositório dizem "pedido 3 do documento", e
  // não "a terceira seção". Um buraco na numeração (alguém apaga o pedido 2) faz
  // a frase "pedido 3" passar a apontar para outro assunto — sem erro em lugar
  // nenhum, porque o texto continua existindo.
  const numeros = secoes.map((s) => s.numero);
  assert.deepEqual(numeros, numeros.map((_, i) => i + 1), `numeração com buraco: ${numeros.join(", ")}`);
});

test("a nota de procedência aponta para issue do repositório da própria seção", () => {
  // A armadilha que já apareceu: o número do documento não é o número da issue,
  // e as issues do System são #1..#5 enquanto o documento também vai de 1 a 5.
  // Um link para `GIOCHI-OS/issues/5` numa seção do `GIOCHI-SYSTEM` manda a
  // equipe errada ler um pedido que não é dela.
  const texto = readFileSync(DOC, "utf8").split("\n");
  let atual = null;
  for (const linha of texto) {
    const cabecalho = /^## \d+\. `([^`]+)` — /.exec(linha);
    if (cabecalho) {
      atual = { repo: cabecalho[1], notas: [] };
      secoesProcedencia.push(atual);
      continue;
    }
    // O corpo da issue começa no `**Título:**`; daí para baixo é texto do
    // pedido, e um link ali é conteúdo, não procedência.
    if (linha.startsWith("**Título:**")) atual = null;
    if (!atual) continue;
    for (const m of linha.matchAll(/github\.com\/([^/]+)\/([^/\s)]+)\/issues\/\d+/g)) {
      // Comparação pelo **nome** do repo: o link é qualificado com dono
      // (`xtrempkch-droid/GIOCHI-SYSTEM`) e o cabeçalho não, mas o que precisa
      // bater é o repositório — é ele que decide qual equipe lê o pedido.
      assert.equal(m[2], atual.repo, `nota da seção de ${atual.repo} aponta para issue de ${m[1]}/${m[2]}`);
    }
  }
  assert.ok(secoesProcedencia.length >= 5, "não achei as seções pelo cabeçalho");
});

test("--lista mostra todos os pedidos, com número e repo", () => {
  // `--lista` existe para responder "o que está pendente?" sem abrir o
  // documento. Listar menos do que o documento tem deixa um pedido invisível —
  // e invisível aqui significa "nunca foi feito".
  const lista = roda("--lista");
  for (const s of secoes) {
    assert.match(lista, new RegExp(`^${s.numero}\\s`, "m"), `--lista omitiu o pedido ${s.numero}`);
    assert.ok(lista.includes(s.repo), `--lista omitiu o repo do pedido ${s.numero} (${s.repo})`);
  }
  const linhasDePedido = lista.split("\n").filter((l) => /^\d+\s/.test(l));
  assert.equal(linhasDePedido.length, secoes.length, "--lista tem uma linha por pedido, nem mais nem menos");
});

test("cada pedido abre issue no repositório da própria seção", () => {
  // Um `## 5. \`GIOCHI-SYSTEM\`` que abrisse issue no `GIOCHI-OS` mandaria o
  // pedido para a equipe errada, e a equipe certa nunca saberia.
  for (const s of secoes) {
    const url = new URL(roda(String(s.numero)).trim());
    assert.match(
      `${url.host}${url.pathname}`,
      new RegExp(`^github\\.com/${s.repo}/issues/new$`),
      `pedido ${s.numero} deveria abrir issue em ${s.repo}, abriu em ${url.pathname}`,
    );
  }
});

test("o título vem do campo Título, não do cabeçalho do documento", () => {
  for (const s of secoes) {
    const titulo = new URL(roda(String(s.numero)).trim()).searchParams.get("title");
    assert.ok(titulo && titulo.length > 10, `pedido ${s.numero} sem título`);
    assert.doesNotMatch(titulo, /^#/, `pedido ${s.numero}: título é cabeçalho de markdown — o "**Título:**" não foi lido`);
    assert.doesNotMatch(titulo, /^`/, `pedido ${s.numero}: título começa com crase, é o nome do repo`);
  }
});

test("o corpo é longo e não tem o campo Título vazando", () => {
  for (const s of secoes) {
    const corpo = new URL(roda(String(s.numero)).trim()).searchParams.get("body");
    assert.ok(corpo && corpo.length > 200, `pedido ${s.numero} com corpo curto demais (${corpo?.length} bytes)`);
    assert.doesNotMatch(corpo, /^\s*\*\*Título:\*\*/m, `pedido ${s.numero}: o campo "**Título:**" vazou para o corpo`);
  }
});

test("link relativo vira absoluto — relativo numa issue é 404", () => {
  // O corpo sai deste diretório. `idiomas.md` só resolve em `docs/`; na página
  // da issue ele aponta para o vazio. O pedido 3 é o que tem `integracao.md`
  // para provar que a reescrita cobre mais de um caso.
  for (const s of secoes) {
    const corpo = new URL(roda(String(s.numero)).trim()).searchParams.get("body");
    const relativos = [...corpo.matchAll(/\]\((?!https?:|#)([^)]+)\)/g)].map((m) => m[1]);
    assert.deepEqual(relativos, [], `pedido ${s.numero}: link relativo sobreviveu (${relativos.join(", ")}) — abre 404`);
  }
});

test("a reescrita aponta para o arquivo que existe neste repositório", () => {
  // Prova de que a reescrita **funciona**, e não só de que ela não quebra: o
  // pedido 5 cita `idiomas.md`, e o link tem de apontar para o arquivo de
  // verdade — ter virado `https://…` não basta.
  const corpo = new URL(roda("5").trim()).searchParams.get("body");
  const alvo = "https://github.com/xtrempkch-droid/GIOCHI-APP/blob/main/docs/idiomas.md";
  assert.ok(corpo.includes(alvo), `o link para idiomas.md não virou ${alvo}`);
  assert.ok(readFileSync(path.join(RAIZ, "docs", "idiomas.md"), "utf8").length > 0, "docs/idiomas.md não existe");
});

test("número inexistente falha alto, em vez de abrir outro pedido", () => {
  const r = spawnSync(process.execPath, [SCRIPT, "99"], { encoding: "utf8" });
  assert.notEqual(r.status, 0, "número inexistente devia sair com código != 0");
  assert.match(r.stderr, /não existe pedido/, "o erro tem de dizer o que houve");
});

test("--corpo e a URL carregam exatamente o mesmo texto", () => {
  // `--corpo` existe porque a URL do pedido de fontes passa de 7 KB. Se o texto
  // divergisse do que a URL carrega, o caminho "cola à mão" mandaria outra coisa
  // e a divergência só apareceria na issue publicada.
  for (const s of secoes) {
    const colado = roda(String(s.numero), "--corpo");
    const daUrl = new URL(roda(String(s.numero)).trim()).searchParams.get("body");
    const semCabecalho = colado.split("\n").slice(2).join("\n").trim();
    assert.equal(semCabecalho, daUrl, `pedido ${s.numero}: --corpo e a URL carregam textos diferentes`);
  }
});
