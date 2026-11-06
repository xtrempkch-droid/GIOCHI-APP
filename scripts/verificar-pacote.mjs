#!/usr/bin/env node
// Verifica um `.giochi`: estrutura, SHA-256 e assinatura Ed25519.
//
//   node scripts/verificar-pacote.mjs dist/pacotes/pacman.giochi
//   node scripts/verificar-pacote.mjs <pacote> --chave-publica <pub.pem>
//
// Sai com 0 só quando **tudo** confere. Um verificador que aceita o que não deve
// é pior que nenhum: dá a sensação de segurança sem a segurança.
//
// ## O teste que importa
//
// O antecessor provava o verificador **adulterando** o pacote e exigindo recusa.
// Aqui isso virou `tests/pacote.test.mjs` (`pacote_adulterado_e_recusado`), e
// roda em cada `node tests/run-all.mjs`. Um verificador que nunca recusou nada
// não provou nada.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { conferirEstrutura, lerTar, verificar } from "./lib/publicar.mjs";

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Chave pública padrão: ao lado da privada usada no empacotamento. */
function chavePublicaPadrao() {
  const candidatos = [
    join(process.env.HOME ?? "", ".giochi", "chave-privada-publica.pem"),
    join(raiz, "chaves", "loja-publica.pem"),
  ];
  return candidatos.find((c) => existsSync(c)) ?? null;
}

function argumentos(argv) {
  const opcoes = { pacote: null, chavePublica: null, json: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const proximo = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} precisa de um valor`);
      return v;
    };
    if (a === "--chave-publica") opcoes.chavePublica = resolve(proximo());
    else if (a === "--json") opcoes.json = true;
    else if (a === "-h" || a === "--ajuda") {
      console.log(
        [
          "verificar-pacote.mjs — estrutura + sha256 + assinatura",
          "",
          "  <pacote.giochi>          pacote a verificar",
          "  --chave-publica <pem>    chave pública Ed25519",
          "  --json                   saída para script",
          "",
          "Sem --chave-publica, usa ~/.giochi/chave-privada-publica.pem.",
          "Sai com 1 quando algo não confere.",
        ].join("\n"),
      );
      process.exit(0);
    } else if (!a.startsWith("--")) {
      if (opcoes.pacote) throw new Error(`pacote especificado duas vezes: ${a}`);
      opcoes.pacote = resolve(a);
    } else {
      throw new Error(`argumento desconhecido: ${a}`);
    }
  }
  if (!opcoes.pacote) throw new Error("informe o pacote .giochi");
  return opcoes;
}

function principal() {
  const opcoes = argumentos(process.argv.slice(2));

  if (!existsSync(opcoes.pacote)) throw new Error(`pacote não existe: ${opcoes.pacote}`);
  const pacote = readFileSync(opcoes.pacote);

  const caminhoManifesto = `${opcoes.pacote}.manifest.json`;
  const manifesto = existsSync(caminhoManifesto)
    ? JSON.parse(readFileSync(caminhoManifesto, "utf8"))
    : null;

  const caminhoChave = opcoes.chavePublica ?? chavePublicaPadrao();
  const chave = caminhoChave && existsSync(caminhoChave) ? readFileSync(caminhoChave, "utf8") : null;

  const estrutura = conferirEstrutura(pacote);
  const erros = [...estrutura.erros];

  // `irrestrito` = sem manifesto ou sem chave. Nesse caso a verificação é só
  // estrutural, e o relatório **precisa dizer isso**: chamar de "ok" um pacote
  // não assinado seria a mentira mais fácil de todas.
  let assinatura = null;
  if (!manifesto) {
    erros.push(`manifesto ausente: ${caminhoManifesto}`);
  } else if (!chave) {
    erros.push(
      "chave pública ausente: não há como verificar a assinatura " +
        "(use --chave-publica, ou gere um par com scripts/empacotar.mjs)",
    );
  } else {
    // Os bytes vem do arquivo que o manifesto aponta. Ler o `.sig` aqui, e nao
    // dentro de `verificar`, mantem a verificacao pura e testavel com buffers.
    const caminhoSig = manifesto?.signature
      ? join(dirname(opcoes.pacote), manifesto.signature)
      : `${opcoes.pacote}.sig`;
    let bytesSig = null;
    if (existsSync(caminhoSig)) bytesSig = readFileSync(caminhoSig);
    assinatura = verificar(pacote, manifesto, chave, bytesSig);
    erros.push(...assinatura.erros);
  }

  let metadata = null;
  try {
    const m = [...lerTar(pacote)].find((x) => x.caminho === "metadata.json");
    if (m) metadata = JSON.parse(m.conteudo.toString("utf8"));
  } catch {
    /* já reportado por conferirEstrutura */
  }

  if (opcoes.json) {
    console.log(
      JSON.stringify(
        {
          pacote: opcoes.pacote,
          ok: erros.length === 0,
          erros,
          membros: estrutura.membros,
          entrypoint: estrutura.entrypoint,
          sha256: assinatura?.sha256 ?? null,
          metadata,
        },
        null,
        2,
      ),
    );
    process.exit(erros.length === 0 ? 0 : 1);
  }

  console.log(`pacote: ${opcoes.pacote}`);
  console.log(`  tamanho: ${pacote.length} bytes`);
  console.log(`  membros: ${estrutura.membros.join(" → ")}`);
  console.log(`  entrypoint: ${estrutura.entrypoint ?? "—"}`);
  if (metadata) {
    console.log(`  app: ${metadata.title ?? "?"} ${metadata.version ?? "?"} (${metadata.runtime ?? "?"})`);
  }
  console.log(`  sha256: ${assinatura?.sha256 ?? "—"}`);
  console.log(`  chave pública: ${caminhoChave ?? "AUSENTE"}`);

  if (erros.length === 0) {
    console.log("  resultado: ok");
    process.exit(0);
  }

  console.log("");
  for (const e of erros) console.log(`  FALHA: ${e}`);
  process.exit(1);
}

try {
  principal();
} catch (e) {
  console.error(`erro: ${e.message}`);
  process.exit(2);
}
