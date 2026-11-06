// O `.pyz` é um ZIP com `__main__.py` na raiz. Estes testes fixam as duas
// propriedades que o resto do projeto depende.
//
// 1. **Reprodutibilidade.** O workflow publica um SHA-256; se dois
//    empacotamentos do mesmo código derem hashes diferentes, o checksum deixa
//    de identificar o conteúdo — que é o único papel dele. O `zipapp` do Python
//    grava o `mtime` de cada arquivo de origem, e é por isso que o ZIP daqui é
//    escrito à mão.
// 2. **Legibilidade pelo Python.** ZIP "quase certo" produz
//    `zipimport: bad magic number` no usuário, não no teste.

import assert from "node:assert/strict";
import { test } from "node:test";
import { escreverZip, extrair, lerIndiceZip } from "../scripts/lib/zip.mjs";

const membros = [
  { caminho: "__main__.py", conteudo: Buffer.from("print('ola')\n") },
  { caminho: "pkg/__init__.py", conteudo: Buffer.from("") },
  { caminho: "pkg/cli.py", conteudo: Buffer.from("def main():\n    return 0\n") },
];

test("o índice do ZIP lista os membros", () => {
  const indice = lerIndiceZip(escreverZip(membros));
  assert.deepEqual(
    indice.entradas.map((e) => e.caminho).sort(),
    ["__main__.py", "pkg/__init__.py", "pkg/cli.py"],
  );
});

test("o conteúdo volta idêntico", () => {
  const zip = escreverZip(membros);
  const indice = lerIndiceZip(zip);
  for (const m of membros) {
    assert.deepEqual(extrair(zip, indice, m.caminho), m.conteudo, m.caminho);
  }
});

test("a ordem é por caminho, não a de entrada", () => {
  // Ordem de `readdir` não é controlada; a do pacote tem de ser.
  const invertido = [...membros].reverse();
  const a = escreverZip(membros);
  const b = escreverZip(invertido);
  assert.deepEqual(a, b, "a mesma lista em ordem diferente tem de dar o mesmo ZIP");
  assert.deepEqual(
    lerIndiceZip(a).entradas.map((e) => e.caminho),
    ["__main__.py", "pkg/__init__.py", "pkg/cli.py"],
  );
});

test("empacotar duas vezes dá bytes idênticos", () => {
  assert.deepEqual(escreverZip(membros), escreverZip(membros));
});

test("arquivo vazio é aceito (o .py vazio é comum)", () => {
  const zip = escreverZip([{ caminho: "vazio.py", conteudo: Buffer.alloc(0) }]);
  const indice = lerIndiceZip(zip);
  const e = indice.entradas[0];
  assert.equal(e.tamanho, 0);
  assert.deepEqual(extrair(zip, indice, "vazio.py"), Buffer.alloc(0));
});

test("recusa lixo no lugar de ZIP", () => {
  assert.throws(() => lerIndiceZip(Buffer.from("nao e um zip")), /diretório central/);
});

test("nome com acento sobrevive (UTF-8, não latin-1)", () => {
  const zip = escreverZip([{ caminho: "não.py", conteudo: Buffer.from("x=1\n") }]);
  const indice = lerIndiceZip(zip);
  assert.equal(indice.entradas[0].caminho, "não.py");
  assert.deepEqual(extrair(zip, indice, "não.py"), Buffer.from("x=1\n"));
});
