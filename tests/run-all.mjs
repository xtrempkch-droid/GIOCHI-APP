#!/usr/bin/env node
// Roda todos os testes em um único processo.
//
// Por que não `node --test tests/`: o runner do node:test spawna um processo
// por arquivo, e sob o sandbox de arquivos isso falha com `spawn EPERM`.
//
// ## Por que a lista é descoberta, e não escrita à mão
//
// Era uma lista de `import` fixa. Para um arquivo novo rodar, alguém tinha de
// lembrar de acrescentar a linha — e esquecer não dava erro nenhum: a suíte
// continuava verde com **menos** testes. `tests/instalar-do-cartao.test.mjs` foi
// escrito nesta sessão e não rodou por isso, apesar de `npm test` dizer 164/164.
//
// Um teste que existe e não roda é pior que teste nenhum: ele dá a impressão de
// cobertura. Agora quem manda é o disco.
import { readdirSync } from "node:fs";
import path from "node:path";

const aqui = import.meta.dirname;
const arquivos = readdirSync(aqui)
  .filter((n) => n.endsWith(".test.mjs"))
  .sort();

if (arquivos.length === 0) {
  console.error("nenhum arquivo *.test.mjs encontrado — a descoberta falhou");
  process.exit(1);
}

for (const arquivo of arquivos) {
  await import(path.join(aqui, arquivo));
}
