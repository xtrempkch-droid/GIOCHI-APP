#!/usr/bin/env node
// Monta a URL de `issues/new` com título e corpo já preenchidos, a partir dos
// corpos prontos de `docs/issues-para-outros-repos.md`.
//
// Uso:
//   node scripts/abrir-issue.mjs 5            # imprime a URL
//   node scripts/abrir-issue.mjs 5 --abrir    # e abre no navegador
//   node scripts/abrir-issue.mjs 5 --corpo    # só o corpo, para colar à mão
//   node scripts/abrir-issue.mjs --lista      # quais pedidos existem
//
// `--corpo` existe porque a URL de um pedido grande passa de 7 KB: cabe, mas
// nem toda ferramenta que cola link gosta. O texto é o mesmo — o que muda é o
// veículo.
//
// ## Por que esta ferramenta existe
//
// O caminho "normal" daqui é o MCP do GitHub, e ele **autentica** (um POST em
// issue inexistente devolve 404, não 401). O que ele não faz é criar sozinho: o
// `issue_write` renderiza um formulário de confirmação no VS Code, e o agente
// não clica em formulário. A promessa de "abrir uma issue" ficou duas vezes
// parada nesse clique — e o §10 do AGENTS.md é explícito que anunciar issue não
// criada é pior que não ter regra.
//
// A saída não é desistir da issue: é tirar o clique do VS Code do caminho. O
// GitHub aceita `?title=` e `?body=` no `issues/new`, então o corpo que já está
// escrito e revisado vira um link. Quem clica é a pessoa, mas clica no
// **GitHub**, no formulário que ela já conhece — não numa extensão que pode nem
// aparecer na tela.
//
// O corpo vai **verbatim**: o texto abaixo de cada `## N.` foi medido, e
// reescrevê-lo aqui criaria uma segunda versão que envelhece diferente da
// primeira. A justificativa é a mesma do `catalogo.mjs`.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const DOC = path.join(RAIZ, "docs", "issues-para-outros-repos.md");

// Um link relativo (`idiomas.md`) resolve para 404 na página da issue: o corpo
// sai do repositório e perde o diretório. Vira link absoluto do GitHub.
const REPO = "xtrempkch-droid/GIOCHI-APP";
const RAW = `https://github.com/${REPO}/blob/main/docs`;

const linhas = readFileSync(DOC, "utf8").split("\n");

/** Cada seção `## N. <repo> — <assunto>` vira um pedido. */
function secoes() {
  const achadas = [];
  for (let i = 0; i < linhas.length; i++) {
    const m = /^## (\d+)\. `([^`]+)` — (.+)$/.exec(linhas[i]);
    if (!m) continue;
    achadas.push({ numero: Number(m[1]), repo: m[2], assunto: m[3].trim(), inicio: i });
  }
  for (let k = 0; k < achadas.length; k++) {
    const fim = k + 1 < achadas.length ? achadas[k + 1].inicio : linhas.length;
    achadas[k].corpo = linhas.slice(achadas[k].inicio + 1, fim).join("\n");
  }
  return achadas;
}

/**
 * Separa o título do corpo.
 *
 * O `**Título:** ...` é o contrato do arquivo — sem ele a issue abriria com o
 * título "## 5. `GIOCHI-SYSTEM` — fontes...", que é cabeçalho de documento, não
 * de issue. Ausente é erro e não silêncio: cair no assunto da seção produziria
 * uma issue com título plausível e diferente do combinado.
 */
function titulo_e_corpo(secao) {
  const linhasCorpo = secao.corpo.split("\n");
  const i = linhasCorpo.findIndex((l) => l.startsWith("**Título:**"));
  if (i < 0) throw new Error(`seção ${secao.numero} não tem "**Título:**"`);
  const titulo = linhasCorpo[i].replace("**Título:**", "").trim();
  const corpo = linhasCorpo.slice(i + 1).join("\n").trim();
  return { titulo, corpo };
}

function linkAbsoluto(corpo) {
  return corpo.replace(/\]\((?!https?:)([^)]+)\)/g, `](${RAW}/$1)`);
}

const url = (secao) => {
  const { titulo, corpo } = titulo_e_corpo(secao);
  const q = new URLSearchParams({ title: titulo, body: linkAbsoluto(corpo) });
  return `https://github.com/${secao.repo}/issues/new?${q}`;
};

const args = process.argv.slice(2);
const todas = secoes();

if (args.includes("--lista") || args.length === 0) {
  for (const s of todas) {
    const { titulo } = titulo_e_corpo(s);
    console.log(`${s.numero}  ${s.repo.padEnd(14)}  ${titulo}`);
  }
  console.log("\nnode scripts/abrir-issue.mjs <número> --abrir");
  process.exit(0);
}

const numero = Number(args[0]);
const secao = todas.find((s) => s.numero === numero);
if (!secao) {
  console.error(`não existe pedido ${args[0]}; use --lista`);
  process.exit(1);
}

const destino = url(secao);

if (args.includes("--corpo")) {
  const { titulo, corpo } = titulo_e_corpo(secao);
  console.log(`# ${titulo}\n`);
  console.log(linkAbsoluto(corpo));
  process.exit(0);
}

console.log(destino);

if (args.includes("--abrir")) {
  // `xdg-open` só no Linux deste repo; em outro sistema a URL impressa já basta
  // e um erro de "não achei o comando" mandaria procurar defeito onde não tem.
  const { spawnSync } = await import("node:child_process");
  const r = spawnSync("xdg-open", [destino], { stdio: "ignore" });
  if (r.error) console.error("(não consegui abrir o navegador; use a URL acima)");
}
