// O empacotador dos apps de PC, exercitado de ponta a ponta.
//
// O que estes testes protegem, e por que cada um existe:
//
// - **`__main__.py` importa o módulo certo.** O `store` expõe `main` em `cli.py`
//   e o `forge` no `__main__.py`. Gerar os dois com o mesmo nome produziu um
//   `.pyz` que abria e morria com `ModuleNotFoundError` — e passou porque só um
//   dos dois foi executado. Aqui os dois são lidos.
// - **Determinismo.** O workflow publica um SHA-256; se dois empacotamentos do
//   mesmo código derem hashes diferentes, o checksum não identifica conteúdo.
// - **Os atalhos.** `.cmd` com CRLF (cmd.exe trata LF como fim de linha em
//   alguns casos e ignora o resto da linha em outros) e sem `&& ... %ERRORLEVEL%`,
//   que expande antes de executar.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { lerIndiceZip, extrair } from "../scripts/lib/zip.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const SCRIPT = path.join(RAIZ, "scripts", "empacotar-pc.mjs");

function comSaida(cb) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-pc-"));
  try {
    return cb(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function empacotar(dir) {
  execFileSync(process.execPath, [SCRIPT, "--saida", dir], { cwd: RAIZ, stdio: "pipe" });
  return dir;
}

test("gera os dois .pyz com __main__.py na raiz", () => {
  comSaida((dir) => {
    empacotar(dir);
    for (const nome of ["giochi-store", "giochi-forge"]) {
      const zip = readFileSync(path.join(dir, `${nome}.pyz`));
      const indice = lerIndiceZip(zip);
      const caminhos = indice.entradas.map((e) => e.caminho);
      assert.ok(caminhos.includes("__main__.py"), `${nome}: sem __main__.py`);
      assert.ok(
        indice.entradas.every((e) => e.metodo === 8),
        `${nome}: entrada não comprimida — o zipimport aceita, mas o ZIP tem de ser deflate`,
      );
    }
  });
});

test("o .pyz do painel leva os estáticos e os três pacotes", () => {
  // O painel **chama** o forge e o store, e serve `estatico/*.html|css|js`.
  // Num zip não existe diretório ao lado do módulo nem `PYTHONPATH`: se um dos
  // três pacotes faltar, o app empacotado morre com ModuleNotFoundError; se os
  // estáticos faltarem, ele abre e serve **404 na própria página**. Os dois
  // casos só apareceriam depois de distribuir, por isso são testados aqui.
  comSaida((dir) => {
    empacotar(dir);
    const caminhos = lerIndiceZip(readFileSync(path.join(dir, "giochi-painel.pyz")))
      .entradas.map((e) => e.caminho);

    for (const esperado of [
      "giochi_painel/servidor.py",
      "giochi_forge/classify.py",
      "giochi_store/catalogo.py",
      "giochi_painel/estatico/index.html",
      "giochi_painel/estatico/app.js",
      "giochi_painel/estatico/estilo.css",
    ]) {
      assert.ok(caminhos.includes(esperado), `giochi-painel.pyz sem ${esperado}`);
    }
  });
});

/** Interpretadores a exercitar: sempre o atual e, se existir, o piso 3.9. */
function interpretadores() {
  const lista = ["python3"];
  for (const candidato of ["python3.9"]) {
    try {
      execFileSync(candidato, ["--version"], { stdio: "pipe" });
      lista.push(candidato);
    } catch {
      /* não instalado: o CI tem a perna dele */
    }
  }
  return lista;
}

test("o .pyz do painel roda de fato, inclusive no piso 3.9", () => {
  // "Está no zip" e "executa" são afirmações diferentes. Um `import` que só
  // resolve no diretório do código passa no teste acima e falha aqui.
  //
  // O 3.9 é testado **de propósito**: em 3.9 o `zipimport` carrega por
  // `load_module()` legado e não preenche `__package__`. Com
  // `resources.files(__package__)` o painel empacotado morria com
  // `AttributeError: 'NoneType' object has no attribute 'startswith'` — só no
  // `.pyz`, só no 3.9. A importação normal (que os testes de unidade usam) não
  // reproduz, então a guarda tem de executar o zip.
  comSaida((dir) => {
    empacotar(dir);
    for (const py of interpretadores()) {
      const saida = execFileSync(
        py,
        [path.join(dir, "giochi-painel.pyz"), "--versao"],
        { encoding: "utf8", stdio: "pipe" },
      );
      assert.match(saida, /giochi-painel \d+\.\d+\.\d+/, `${py}: saída inesperada`);
    }
  });
});

test("__main__.py de cada app importa o módulo que realmente tem main", () => {
  comSaida((dir) => {
    empacotar(dir);
    const casos = {
      "giochi-store": "giochi_store.cli",
      "giochi-forge": "giochi_forge.__main__",
    };
    for (const [nome, modulo] of Object.entries(casos)) {
      const zip = readFileSync(path.join(dir, `${nome}.pyz`));
      const indice = lerIndiceZip(zip);
      const main = extrair(zip, indice, "__main__.py").toString();
      assert.match(
        main,
        new RegExp(`from ${modulo.replace(".", "\\.")} import main`),
        `${nome}: __main__ não importa de ${modulo}`,
      );
    }
  });
});

test("os módulos do pacote estão no ZIP", () => {
  comSaida((dir) => {
    empacotar(dir);
    const zip = readFileSync(path.join(dir, "giochi-forge.pyz"));
    const caminhos = lerIndiceZip(zip).entradas.map((e) => e.caminho);
    for (const m of ["giochi_forge/__init__.py", "giochi_forge/package.py"]) {
      assert.ok(caminhos.includes(m), `faltou ${m}`);
    }
    assert.ok(
      !caminhos.some((c) => c.includes("__pycache__") || c.endsWith(".pyc")),
      "cache de bytecode não deve entrar no pacote",
    );
  });
});

test("empacotar duas vezes dá o mesmo SHA-256", () => {
  comSaida((dir) => {
    empacotar(dir);
    const primeira = readFileSync(path.join(dir, "SHA256SUMS"), "utf8");
    empacotar(dir);
    const segunda = readFileSync(path.join(dir, "SHA256SUMS"), "utf8");
    assert.equal(primeira, segunda);
  });
});

test("SHA256SUMS tem o formato que sha256sum -c entende", () => {
  comSaida((dir) => {
    empacotar(dir);
    const texto = readFileSync(path.join(dir, "SHA256SUMS"), "utf8");
    const linhas = texto.trim().split("\n");
    // O número vem dos atalhos .sh gerados, não de um literal: fixar "2" fez
    // este teste falhar quando o painel entrou, e o defeito era o teste, não o
    // empacotador. Assim, um app novo só precisa empacotar para passar.
    const esperados = readdirSync(dir).filter((n) => n.endsWith(".sh")).length;
    assert.equal(linhas.length, esperados, `esperava ${esperados} linhas`);
    for (const linha of linhas) {
      assert.match(linha, /^[0-9a-f]{64} {2}giochi-(store|forge|painel)\.pyz$/);
    }
    // LF, não CRLF: `sha256sum -c` e `shasum -c` esperam LF, e um arquivo
    // gerado no Windows com CRLF falharia nos dois.
    assert.ok(!texto.includes("\r"), "SHA256SUMS não pode ter CRLF");
  });
});

test("o atalho .cmd usa CRLF e não confia em && %ERRORLEVEL%", () => {
  comSaida((dir) => {
    empacotar(dir);
    const cmd = readFileSync(path.join(dir, "giochi-forge.cmd"));
    const texto = cmd.toString("latin1");
    assert.ok(texto.includes("\r\n"), ".cmd precisa de CRLF");
    assert.ok(
      !/&&\s*exit\s+\/b\s+%ERRORLEVEL%/i.test(texto),
      "%ERRORLEVEL% numa linha composta vale o valor de antes do comando",
    );
    assert.match(texto, /where py/);
    assert.match(texto, /where python/);
    assert.match(texto, /giochi-forge\.pyz/);
  });
});

test("o atalho .sh é executável e aponta para o .pyz ao lado", () => {
  comSaida((dir) => {
    empacotar(dir);
    const sh = path.join(dir, "giochi-store.sh");
    assert.ok(statSync(sh).mode & 0o111, ".sh precisa ser executável");
    const texto = readFileSync(sh, "utf8");
    assert.match(texto, /^#!\/bin\/sh/);
    assert.match(texto, /giochi-store\.pyz/);
    assert.match(texto, /exec python3/);
  });
});

test("--app empacota só um", () => {
  comSaida((dir) => {
    execFileSync(process.execPath, [SCRIPT, "--saida", dir, "--app", "store"], {
      cwd: RAIZ,
      stdio: "pipe",
    });
    const sums = readFileSync(path.join(dir, "SHA256SUMS"), "utf8");
    assert.match(sums, /giochi-store\.pyz/);
    assert.ok(!sums.includes("giochi-forge"), "não deveria ter empacotado o forge");
  });
});

test("recusa app desconhecido em vez de gerar nada", () => {
  comSaida((dir) => {
    assert.throws(() =>
      execFileSync(process.execPath, [SCRIPT, "--saida", dir, "--app", "loja"], {
        cwd: RAIZ,
        stdio: "pipe",
      }),
    );
  });
});
