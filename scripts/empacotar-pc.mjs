#!/usr/bin/env node
// Empacota os apps de PC (`distribution: "host"`) num `.pyz` que roda sem
// instalar nada e sem Python no PATH do usuário saber do projeto.
//
// Uso:
//   node scripts/empacotar-pc.mjs                 # store e forge
//   node scripts/empacotar-pc.mjs --app store
//   node scripts/empacotar-pc.mjs --saida dist/pc
//
// ## Por que `.pyz` e não PyInstaller
//
// Os dois apps usam **só a biblioteca padrão** — nenhuma dependência de
// terceiro, por decisão de projeto (a loja precisa funcionar numa máquina
// limpa). Isso muda a conta:
//
// | | `.pyz` (zipapp) | PyInstaller |
// | --- | --- | --- |
// | artefato por sistema | **um**, igual nos três | um por SO e por arch |
// | precisa compilar no SO | não | sim (matriz obrigatória) |
// | determinístico | sim, se o ZIP for nosso | não |
// | tamanho | ~80 KB | ~10 MB |
// | precisa de Python instalado | **sim** (3.9+) | não |
//
// A troca é explícita: quem tem Python roda um arquivo só; quem não tem precisa
// do binário nativo, que é o passo seguinte e está no workflow (`--nativo`).
//
// ## O que sai
//
//   dist/pc/giochi-store.pyz       o app inteiro, um arquivo
//   dist/pc/giochi-forge.pyz
//   dist/pc/giochi-store.cmd       atalho para Windows
//   dist/pc/giochi-store.sh        atalho para Linux/macOS
//   dist/pc/SHA256SUMS             o que o usuário confere antes de rodar

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { escreverZip, lerIndiceZip } from "./lib/zip.mjs";

// A raiz sai do próprio arquivo: o script é chamado por caminho absoluto no
// workflow, e um `process.cwd()` diferente quebraria sem avisar.
const RAIZ = path.resolve(import.meta.dirname, "..");

/**
 * App de PC que o empacotador conhece: pasta, pacote e **módulo que tem `main`**.
 *
 * `entrada` não é detalhe: o `store` expõe `main` em `cli.py` e o `forge` no
 * `__main__.py`. Gerar os dois com o mesmo nome produzia um `.pyz` que abre e
 * morre com `ModuleNotFoundError: No module named 'giochi_forge.cli'` — e só
 * apareceu porque os **dois** foram executados. Um teste que roda um caso só
 * teria aprovado o pacote quebrado.
 */
const ALVOS = {
  store: { pasta: "app/store", pacote: "giochi_store", entrada: "giochi_store.cli", binario: "giochi-store" },
  forge: { pasta: "app/forge", pacote: "giochi_forge", entrada: "giochi_forge.__main__", binario: "giochi-forge" },
  // O painel **chama** o forge e o store em vez de reimplementar a
  // classificação e o empacotamento. No PC do usuário isso resolve por
  // `PYTHONPATH`; num `.pyz` os três módulos têm de estar no mesmo zip, porque
  // não há diretório ao lado do executável. `extras` é o que diz isso.
  painel: {
    pasta: "app/painel",
    pacote: "giochi_painel",
    entrada: "giochi_painel.__main__",
    binario: "giochi-painel",
    extras: ["app/forge/src/giochi_forge", "app/store/src/giochi_store"],
  },
};

function lerArgs(argv) {
  const args = { saida: "dist/pc", app: null };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--saida") args.saida = argv[++i];
    else if (a === "--app") args.app = argv[++i];
    else if (a === "--ajuda" || a === "-h") args.ajuda = true;
    else throw new Error(`argumento desconhecido: ${a}`);
  }
  return args;
}

function ajuda() {
  console.log(`
empacotar-pc.mjs — monta os apps de PC como .pyz (um arquivo, três sistemas)

  --app <store|forge>   empacota só um (padrão: os dois)
  --saida <dir>         onde gravar (padrão: dist/pc)

Gera <binario>.pyz, os atalhos .sh e .cmd e SHA256SUMS. O mesmo .pyz roda em
Windows, Linux e macOS: os apps não têm dependência fora da biblioteca padrão.
`);
}

/**
 * Lista os `.py` do pacote Python, em caminho relativo ao `src`.
 *
 * Arquivos começando com `_` e terminando em `.pyc` ficam de fora, e
 * `__pycache__` também: cache de bytecode dentro do `.pyz` não acelera nada (o
 * `zipimport` compila na primeira execução e guarda em memória) e só faria o
 * checksum depender de quem empacotou.
 */
function arquivosDoPacote(base) {
  const saida = [];

  const visitar = (atual) => {
    for (const nome of readdirSync(atual).sort()) {
      if (nome === "__pycache__" || nome.endsWith(".pyc")) continue;
      const completo = path.join(atual, nome);
      const rel = path.relative(path.dirname(base), completo).split(path.sep).join("/");
      if (statSync(completo).isDirectory()) visitar(completo);
      // Não só `.py`: o painel serve `estatico/*.html|css|js` de dentro do
      // pacote, e um `.pyz` sem eles abre a página em branco. O filtro é por
      // arquivo, não por extensão.
      else saida.push({ caminho: rel, conteudo: readFileSync(completo) });
    }
  };

  if (!existsSync(base)) throw new Error(`pacote ausente: ${base}`);
  visitar(base);
  if (saida.length === 0) throw new Error(`pacote vazio: ${base}`);
  return saida;
}

/** Atalho de Unix. Executável, sem extensão de linha mágica que dependa do SO. */
function atalhoSh(binario) {
  return [
    "#!/bin/sh",
    `# Atalho do ${binario}: roda o .pyz ao lado deste arquivo.`,
    "#",
    "# Existe porque dar permissão de execução num .pyz e digitar",
    "#   python3 giochi-store.pyz",
    "# são coisas diferentes, e a segunda é a que ninguém lembra.",
    "set -eu",
    'AQUI=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)',
    'exec python3 "$AQUI/' + binario + '.pyz" "$@"',
    "",
  ].join("\n");
}

/**
 * Atalho de Windows.
 *
 * ## Por que não `py ... %* && exit /b %ERRORLEVEL%`
 *
 * Porque em `.cmd` o `%VAR%` é expandido quando a **linha é lida**, não quando
 * executa. Numa linha composta com `&&`, o `%ERRORLEVEL%` vale o valor de
 * *antes* do comando — o atalho sairia com código errado sempre. O padrão que
 * funciona é `where` numa linha, `if %ERRORLEVEL%==0` na seguinte.
 *
 * ## Por que tenta `py` e depois `python`
 *
 * `py` é o lançador oficial do Python para Windows e escolhe a versão instalada;
 * `python` é o que existe em instalações antigas e na Microsoft Store. Tentar os
 * dois em ordem certa custa duas linhas e evita o suporte mais comum.
 *
 * A mensagem final existe porque um duplo-clique que abre e fecha na mesma hora
 * é o pior resultado possível: o usuário não descobre se falta o Python, se o
 * arquivo veio corrompido, ou se o app travou. `pause` mantém a janela aberta.
 */
function atalhoCmd(binario) {
  return [
    "@echo off",
    `rem Atalho do ${binario}: roda o .pyz ao lado deste arquivo.`,
    "setlocal",
    'set "AQUI=%~dp0"',
    'set "APP=%AQUI%' + binario + '.pyz"',
    "",
    'if not exist "%APP%" goto :sem_arquivo',
    "",
    "where py >nul 2>nul",
    "if %ERRORLEVEL%==0 goto :com_py",
    "",
    "where python >nul 2>nul",
    "if %ERRORLEVEL%==0 goto :com_python",
    "",
    "goto :sem_python",
    "",
    ":com_py",
    'py -3 "%APP%" %*',
    "exit /b %ERRORLEVEL%",
    "",
    ":com_python",
    'python "%APP%" %*',
    "exit /b %ERRORLEVEL%",
    "",
    ":sem_arquivo",
    "echo.",
    `echo Nao encontrei ${binario}.pyz ao lado deste atalho.`,
    "echo Mantenha o .cmd e o .pyz na mesma pasta.",
    "echo.",
    "pause",
    "exit /b 2",
    "",
    ":sem_python",
    "echo.",
    "echo Este app precisa do Python 3.9 ou mais novo, e ele nao esta no PATH.",
    "echo Instale em https://python.org/downloads",
    'echo e marque "Add python.exe to PATH" na primeira tela do instalador.',
    "echo.",
    "pause",
    "exit /b 9009",
    "",
  ].join("\r\n");
}

function sha256(buf) {
  return createHash("sha256").update(buf).digest("hex");
}

function principal() {
  const args = lerArgs(process.argv);
  if (args.ajuda) {
    ajuda();
    return 0;
  }

  const nomes = args.app ? [args.app] : Object.keys(ALVOS);
  for (const nome of nomes) {
    if (!ALVOS[nome]) throw new Error(`app de PC desconhecido: ${nome}`);
  }

  const destino = path.isAbsolute(args.saida) ? args.saida : path.join(RAIZ, args.saida);
  mkdirSync(destino, { recursive: true });

  const linhas = [];
  for (const nome of nomes) {
    const alvo = ALVOS[nome];
    const dirSrc = path.join(RAIZ, alvo.pasta, "src");

    const entradas = arquivosDoPacote(path.join(dirSrc, alvo.pacote));
    for (const extra of alvo.extras ?? []) {
      for (const e of arquivosDoPacote(path.join(RAIZ, extra))) {
        if (entradas.some((x) => x.caminho === e.caminho)) {
          throw new Error(`${alvo.binario}: módulo duplicado no zip: ${e.caminho}`);
        }
        entradas.push(e);
      }
    }
    // `__main__.py` na raiz é o que faz `python arquivo.pyz` executar: é a
    // convenção do `zipimport`, não uma escolha nossa.
    entradas.push({
      caminho: "__main__.py",
      conteudo: Buffer.from(
        [
          `"""Ponto de entrada do ${alvo.binario} empacotado como zipapp."""`,
          "import sys",
          "",
          `from ${alvo.entrada} import main`,
          "",
          'if __name__ == "__main__":',
          "    sys.exit(main())",
          "",
        ].join("\n"),
      ),
    });

    const zip = escreverZip(entradas);
    const indice = lerIndiceZip(zip);
    if (!indice.entradas.some((e) => e.caminho === "__main__.py")) {
      throw new Error(`${alvo.binario}: __main__.py não entrou no pacote`);
    }

    const pyz = path.join(destino, `${alvo.binario}.pyz`);
    writeFileSync(pyz, zip);
    writeFileSync(path.join(destino, `${alvo.binario}.sh`), atalhoSh(alvo.binario), { mode: 0o755 });
    writeFileSync(path.join(destino, `${alvo.binario}.cmd`), atalhoCmd(alvo.binario));

    linhas.push(`${sha256(zip)}  ${alvo.binario}.pyz`);
    console.log(`${alvo.binario}.pyz`);
    console.log(`  arquivo:  ${pyz}`);
    console.log(`  tamanho:  ${zip.length} bytes`);
    console.log(`  módulos:  ${entradas.length - 1} + __main__.py`);
    console.log(`  sha256:   ${sha256(zip)}`);
  }

  // `SHA256SUMS` com quebras de linha de Unix mesmo no Windows: é o que
  // `sha256sum -c` e `shasum -a 256 -c` esperam, e os dois são o que existe no
  // Linux e no macOS. No Windows o padrão é `certutil`, que não lê este
  // arquivo — a documentação diz o comando.
  writeFileSync(path.join(destino, "SHA256SUMS"), `${linhas.join("\n")}\n`);

  console.log(`\n${linhas.length} app(s) em ${destino}`);
  return 0;
}

try {
  process.exitCode = principal();
} catch (e) {
  console.error(`erro: ${e.message}`);
  process.exitCode = 1;
}
