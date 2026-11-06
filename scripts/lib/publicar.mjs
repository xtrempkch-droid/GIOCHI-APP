// Publicação: pasta de app → pacote `.giochi` assinado.
//
// A regra que este módulo impõe é a de `docs/pacote.md`, e ela tem uma
// consequência prática que vale entender antes de mexer: **a ordem dos membros é
// contrato, não estética**. O extrator no aparelho lê em streaming e recusa o
// pacote no primeiro membro que não esperava — se `bin/` viesse antes de
// `metadata.json`, o aparelho teria lido o binário inteiro antes de saber que o
// app é incompatível.
//
// Por isso a montagem é feita aqui, numa função só, e não espalhada por um
// script de linha de comando: a ordem precisa ser verificável por teste.

import {
  sign,
  verify as verificaAssinatura,
  createHash,
  generateKeyPairSync,
  createPrivateKey,
  createPublicKey,
} from "node:crypto";

import { escreverTar, lerTar, METADATA_MAX, CAPA_LADO } from "./tar.mjs";
import { readPngSize, isPng } from "./png.mjs";
import { REQUIRED_METADATA, ARCH_VALUES, RUNTIME_VALUES } from "./apps.mjs";

/** Arquivos que **nunca** entram no pacote, mesmo estando na pasta do app. */
export const FORA_DO_PACOTE = Object.freeze([
  "src",
  "assets",
  "forge.json",
  "README.md",
  "capa.png.sig",
]);

/**
 * Valida o que o aparelho vai receber.
 *
 * Devolve a lista de erros; vazia significa publicável. Não lança na primeira
 * falha porque quem publica precisa ver **todas** de uma vez — corrigir um erro
 * por rodada é como o validador de app era antes de acumular.
 */
export function validarPacote({ metadata, capa, tamanhoMetadata, binarios }) {
  const erros = [];

  for (const campo of REQUIRED_METADATA) {
    if (metadata?.[campo] === undefined || metadata[campo] === "") {
      erros.push(`metadata.json: campo obrigatório ausente: ${campo}`);
    }
  }

  if (metadata?.arch !== undefined && !ARCH_VALUES.includes(metadata.arch)) {
    erros.push(`metadata.json:arch: ${metadata.arch} não é suportado (${ARCH_VALUES.join(", ")})`);
  }
  if (metadata?.runtime !== undefined && !RUNTIME_VALUES.includes(metadata.runtime)) {
    erros.push(`metadata.json:runtime: ${metadata.runtime} inválido (${RUNTIME_VALUES.join(", ")})`);
  }

  if (tamanhoMetadata > METADATA_MAX) {
    erros.push(
      `metadata.json: ${tamanhoMetadata} bytes excede o limite de ${METADATA_MAX} ` +
        "— o extrator no aparelho precisa dele antes de qualquer decisão",
    );
  }

  if (!capa) {
    erros.push("capa.png: ausente (obrigatória, 320×320)");
  } else {
    const { largura, altura } = capa;
    if (largura !== CAPA_LADO || altura !== CAPA_LADO) {
      erros.push(`capa.png: ${largura}×${altura}, precisa ser ${CAPA_LADO}×${CAPA_LADO}`);
    }
  }

  const entrypoint = metadata?.entrypoint;
  if (typeof entrypoint === "string" && entrypoint.length > 0) {
    if (entrypoint.includes("..")) {
      erros.push(`metadata.json:entrypoint: ${entrypoint} contém ".."`);
    } else if (!entrypoint.startsWith("bin/")) {
      erros.push(`metadata.json:entrypoint: ${entrypoint} precisa começar em bin/`);
    } else if (!binarios.includes(entrypoint)) {
      erros.push(
        `metadata.json:entrypoint: ${entrypoint} não existe no pacote ` +
          `(bin/ tem: ${binarios.join(", ") || "nada"})`,
      );
    }
  }

  if (binarios.length === 0) {
    erros.push("bin/: vazia — um pacote sem binário não executa nada");
  }

  return erros;
}

/**
 * Monta a lista de membros **na ordem obrigatória**.
 *
 * `arquivos` já vem com caminho relativo e conteúdo; a função reordena para a
 * ordem do contrato e garante o diretório `bin/` antes dos seus arquivos (um
 * extrator que só cria diretório ao ver o cabeçalho `5` recusaria o membro).
 */
export function ordenarMembros({ metadataJson, capa, thumb = null, arquivos }) {
  const membros = [];

  membros.push({
    caminho: "metadata.json",
    conteudo: Buffer.from(metadataJson, "utf8"),
    modo: 0o644,
  });

  membros.push({ caminho: "capa.png", conteudo: capa, modo: 0o644 });

  if (thumb) membros.push({ caminho: "thumb.png", conteudo: thumb, modo: 0o644 });

  membros.push({ caminho: "bin/", modo: 0o755 });

  // Dentro de `bin/` a ordem é determinística (alfabética): o SHA-256 do pacote
  // precisa ser igual a cada empacotamento do mesmo conteúdo, senão a assinatura
  // deixa de identificar o conteúdo.
  const dentro = arquivos
    .filter((a) => a.caminho === "bin" || a.caminho.startsWith("bin/"))
    .map((a) => ({
      caminho: a.caminho === "bin" ? "bin/" : a.caminho,
      conteudo: a.conteudo,
      modo: a.modo ?? 0o644,
    }))
    .filter((a) => a.caminho !== "bin/") // o diretório já entrou
    .sort((a, b) => (a.caminho < b.caminho ? -1 : a.caminho > b.caminho ? 1 : 0));

  membros.push(...dentro);

  return membros;
}

/**
 * Slug do pacote: identidade estável, nunca derivada do título.
 *
 * Renomear o título é permitido e comum ("Pac-Man" → "Pac-Man DX"). Se o
 * diretório de instalação viesse do título, o app **mudaria de lugar** e o
 * catálogo antigo apontaria para um caminho que não existe mais — cartão que
 * aparece e não abre, o modo de falha já registrado neste projeto.
 *
 * Ordem: `slug` do metadata → id do `apps.json` → derivação do título (último
 * recurso, e o pacote avisa).
 */
export function slugDoPacote(metadata, idDoIndice) {
  if (typeof metadata?.slug === "string" && metadata.slug.trim() !== "") {
    return { slug: metadata.slug.trim(), origem: "metadata.slug" };
  }
  if (typeof idDoIndice === "string" && idDoIndice.trim() !== "") {
    return { slug: idDoIndice.trim(), origem: "apps.json" };
  }
  const base = String(metadata?.title ?? "")
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return { slug: base || "app", origem: "título (sem slug declarado!)" };
}

/** Gera um par de chaves Ed25519, em PEM. */
export function gerarChaves() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  return {
    publica: publicKey.export({ type: "spki", format: "pem" }).toString(),
    privada: privateKey.export({ type: "pkcs8", format: "pem" }).toString(),
  };
}

/**
 * Assina o pacote e devolve o manifesto.
 *
 * O manifesto é o que a loja consulta para saber se já tem o pacote e se ele é o
 * mesmo — por isso leva `sha256`, `size_bytes` e a assinatura sobre o **próprio
 * pacote**, não sobre o manifesto. Assinar o manifesto permitiria trocar o
 * pacote mantendo o manifesto.
 */
export function assinar(pacote, chavePrivadaPem, { slug, version }) {
  const privada = createPrivateKey(chavePrivadaPem);
  const hash = createHash("sha256").update(pacote).digest("hex");
  const assinatura = sign(null, pacote, privada);

  return {
    manifesto: {
      schema_version: 1,
      package: slug,
      version,
      sha256: hash,
      size_bytes: pacote.length,
      signature: assinatura.toString("base64"),
    },
    assinatura: assinatura.toString("base64"),
  };
}

/**
 * Verifica assinatura + SHA-256.
 *
 * Separada de `assinar` de propósito: um verificador que só sabe dizer "sim"
 * para o que ele mesmo produziu não verifica nada. O teste de ida e volta do
 * antecessor **adultera** o pacote e exige recusa — é esse o padrão aqui.
 */
export function verificar(pacote, manifesto, chavePublicaPem) {
  const erros = [];
  const hash = createHash("sha256").update(pacote).digest("hex");

  if (manifesto?.sha256 !== hash) {
    erros.push(`sha256 não confere: manifesto ${manifesto?.sha256}, arquivo ${hash}`);
  }
  if (manifesto?.size_bytes !== undefined && manifesto.size_bytes !== pacote.length) {
    erros.push(`tamanho não confere: manifesto ${manifesto.size_bytes}, arquivo ${pacote.length}`);
  }

  let assinaturaOk = false;
  try {
    const publica = createPublicKey(chavePublicaPem);
    const bruta = Buffer.from(manifesto?.signature ?? "", "base64");
    if (bruta.length === 0) {
      erros.push("assinatura ausente no manifesto");
    } else {
      assinaturaOk = verificaAssinatura(null, pacote, publica, bruta);
      if (!assinaturaOk) erros.push("assinatura Ed25519 não confere — pacote adulterado ou chave errada");
    }
  } catch (e) {
    erros.push(`não consegui verificar a assinatura: ${e.message}`);
  }

  return { ok: erros.length === 0, erros, sha256: hash, assinaturaOk };
}

/**
 * Confere a estrutura do pacote já montado: ordem, presença e bit de execução.
 *
 * Roda sobre os **bytes finais**, não sobre a lista de membros que geramos —
 * só assim pega um erro do próprio empacotador.
 */
export function conferirEstrutura(pacote) {
  const erros = [];
  const vistos = [];

  let primeiroBin = -1;
  for (const [i, m] of [...lerTar(pacote)].entries()) {
    vistos.push(m.caminho);
    if (m.caminho.startsWith("bin") && primeiroBin < 0) primeiroBin = i;
  }

  const indice = (nome) => vistos.indexOf(nome);
  const iMeta = indice("metadata.json");
  const iCapa = indice("capa.png");

  if (iMeta !== 0) erros.push(`primeiro membro é ${vistos[0] ?? "nada"}, precisa ser metadata.json`);
  if (iCapa !== 1) erros.push(`segundo membro é ${vistos[1] ?? "nada"}, precisa ser capa.png`);
  if (primeiroBin >= 0 && primeiroBin < iCapa) {
    erros.push("bin/ aparece antes de capa.png — o extrator recusaria tarde demais");
  }

  // Bit de execução do entrypoint: o erro só apareceria no aparelho, como
  // "entrypoint não existe".
  let entrypoint = null;
  try {
    const meta = [...lerTar(pacote)].find((m) => m.caminho === "metadata.json");
    if (meta) entrypoint = JSON.parse(meta.conteudo.toString("utf8")).entrypoint;
  } catch (e) {
    erros.push(`metadata.json ilegível: ${e.message}`);
  }

  if (entrypoint) {
    const alvo = [...lerTar(pacote)].find((m) => m.caminho === entrypoint);
    if (!alvo) {
      erros.push(`entrypoint ${entrypoint} não está no pacote`);
    } else if ((alvo.modo & 0o111) === 0) {
      erros.push(`entrypoint ${entrypoint} sem bit de execução (modo 0${alvo.modo.toString(8)})`);
    }
  }

  return { erros, membros: vistos, entrypoint };
}

// Reexportado para o CLI não precisar importar dois módulos.
export { escreverTar, lerTar, readPngSize, isPng };
