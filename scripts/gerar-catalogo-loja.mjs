#!/usr/bin/env node
// Enche a loja: lê os pacotes publicados e gera o catálogo **assinado**.
//
// Uso:
//   node scripts/gerar-catalogo-loja.mjs --pacotes dist/pacotes --saida dist/loja
//   node scripts/gerar-catalogo-loja.mjs --pacotes dist/pacotes --chave ~/.giochi/catalogo.pem
//   node scripts/gerar-catalogo-loja.mjs --pacotes dist/pacotes --url-base https://cdn.giochi.example
//   node scripts/gerar-catalogo-loja.mjs --lista        # o que entraria e o que não
//
// ## É esta ferramenta que responde "como a lista da loja é preenchida"
//
// A lista **não** é escrita à mão em lugar nenhum. Ela é o resultado de um
// pipeline, e cada etapa deixa um artefato verificável:
//
//   app/<slug>/            src/ do jogo, metadata.json, capa.png
//        │  forge build
//        ▼
//   <slug>.giochi  + .sig  + .manifest.json        (pacote assinado)
//        │  este script
//        ▼
//   catalogo.json  (+ .sig embutida)                (lista assinada)
//        │  upload
//        ▼
//   CDN  →  loja no PC  →  aparelho
//
// O aparelho confia em **uma** chave pública (a do catálogo). Todo o resto —
// quais apps existem, por quanto, e de onde baixar — vem assinado por ela. É
// por isso que cadastrar um jogo é rodar o forge e rodar este script: não há
// tabela em banco para editar à mão e esquecer.
//
// ## O que ele se recusa a fazer
//
// - **Incluir pacote sem assinatura.** Sem `.sig` ao lado, o item entraria na
//   lista apontando para um arquivo que ninguém pode verificar.
// - **Incluir pacote cujo hash não bate com o manifesto.** Catálogo que anuncia
//   um sha256 diferente do arquivo produz "download corrompido" na cara do
//   comprador, e o defeito está aqui.
// - **Inventar URL.** Sem `--url-base`, a URL é montada a partir do nome do
//   arquivo e o script **avisa** que o caminho é local.
// - **Usar http.** A loja recusa `http://`; gerar catálogo com http produziria
//   uma lista que a própria loja não abre.

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { lerTar } from "./lib/tar.mjs";
import { gerarChaves } from "./lib/publicar.mjs";
import { assinarCatalogo, sha256Hex, VERSAO } from "./lib/catalogo-loja.mjs";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function ajuda() {
  console.log(`
gerar-catalogo-loja.mjs — monta o catálogo assinado da loja a partir dos pacotes

  --pacotes <dir>      pasta com <slug>.giochi + .sig (padrão: dist/pacotes)
  --saida <dir>        onde gravar catalogo.json (padrão: dist/loja)
  --chave <arquivo>    chave privada do catálogo, PEM (padrão: ~/.giochi/catalogo.pem)
  --criar-chave        cria a chave de catálogo se não existir (e diz onde)
  --url-base <url>     prefixo das URLs de download (padrão: ./ , com aviso)
  --ano <slug=ano>     ano de lançamento, repetível (vai para o cartão da loja)
  --descricao <slug="texto">   descrição, repetível
  --preco <slug=centavos>      preço em centavos, repetível
  --revogar-antes <epoch>      invalida pacotes assinados antes deste instante
  --lista              só mostra o que entraria e o que ficaria de fora
  --json               imprime o catálogo em vez de gravar

O catálogo é a **única** coisa assinada que o aparelho conhece. Adicionar um jogo
é publicar o pacote e rodar isto de novo — não há lista escrita à mão.`);
}

function analisarArgs(argv) {
  const o = {
    pacotes: "dist/pacotes",
    saida: "dist/loja",
    chave: path.join(process.env.HOME ?? "", ".giochi", "catalogo.pem"),
    criarChave: false,
    urlBase: null,
    revogarAntes: null,
    lista: false,
    json: false,
    anos: {},
    descricoes: {},
    precos: {},
  };
  const mapa = (valor, destino) => {
    const i = valor.indexOf("=");
    if (i <= 0) throw new Error(`esperava slug=valor, veio: ${valor}`);
    destino[valor.slice(0, i)] = valor.slice(i + 1);
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const prox = () => {
      const v = argv[i + 1];
      if (v === undefined) throw new Error(`${a} precisa de valor`);
      i += 1;
      return v;
    };
    if (a === "--ajuda" || a === "-h") o.ajuda = true;
    else if (a === "--pacotes") o.pacotes = prox();
    else if (a === "--saida") o.saida = prox();
    else if (a === "--chave") o.chave = prox();
    else if (a === "--criar-chave") o.criarChave = true;
    else if (a === "--url-base") o.urlBase = prox().replace(/\/$/, "");
    else if (a === "--revogar-antes") o.revogarAntes = Number(prox());
    else if (a === "--ano") mapa(prox(), o.anos);
    else if (a === "--descricao") mapa(prox(), o.descricoes);
    else if (a === "--preco") mapa(prox(), o.precos);
    else if (a === "--lista") o.lista = true;
    else if (a === "--json") o.json = true;
    else throw new Error(`argumento desconhecido: ${a}`);
  }
  return o;
}

/** Lê o `metadata.json` de dentro do pacote — a fonte da verdade do item. */
function metadataDoPacote(bytes) {
  const membros = [...lerTar(bytes)];
  const m = membros.find((x) => x.caminho === "metadata.json");
  if (!m) throw new Error("pacote sem metadata.json");
  return JSON.parse(m.conteudo.toString("utf8"));
}

async function principal() {
  const o = analisarArgs(process.argv.slice(2));
  if (o.ajuda) {
    ajuda();
    return 0;
  }

  const dirPacotes = path.isAbsolute(o.pacotes) ? o.pacotes : path.join(RAIZ, o.pacotes);
  if (!existsSync(dirPacotes)) {
    console.error(`ERRO pasta de pacotes não existe: ${dirPacotes}`);
    console.error("  gere os pacotes antes:  node scripts/empacotar.mjs app/launcher");
    return 1;
  }

  const arquivos = readdirSync(dirPacotes)
    .filter((n) => n.endsWith(".giochi"))
    .sort();

  if (arquivos.length === 0) {
    console.error(`ERRO nenhum .giochi em ${dirPacotes}`);
    return 1;
  }

  // Caminho da chave, resolvido **antes** de tudo que a usa: a pública deriva
  // dela, e a criação (com --criar-chave) também. A primeira versão lia a
  // pública antes de criar a chave e o item saía com `chave_pacote` vazio — o
  // aparelho não teria como conferir o pacote.
  const caminhoChaveCat = path.isAbsolute(o.chave) ? o.chave : path.join(RAIZ, o.chave);

  const caminhoChave = caminhoChaveCat;
  // `--lista` não assina nada, então não exige chave: listar é diagnóstico, e
  // exigir credencial para diagnosticar é o atrito que faz ninguém usar a
  // ferramenta de diagnóstico.
  if (!o.lista && !existsSync(caminhoChave)) {
    if (!o.criarChave) {
      console.error(`ERRO chave de catálogo não existe: ${caminhoChave}`);
      console.error("  rode com --criar-chave na primeira vez, ou aponte --chave");
      return 1;
    }
    const par = gerarChaves();
    mkdirSync(path.dirname(caminhoChave), { recursive: true });
    writeFileSync(caminhoChave, par.privada);
    writeFileSync(`${caminhoChave}.pub.pem`, par.publica);
    console.log(`chave de catálogo CRIADA: ${caminhoChave}`);
    console.log(`  pública (vai para o aparelho): ${caminhoChave}.pub.pem`);
  }

  // A pública do catálogo, para o campo `chave_pacote` dos itens. Lida da
  // `.pub.pem` ao lado da privada, ou derivada dela quando o par acabou de ser
  // criado. Sem isso o item sairia sem chave e o aparelho não teria como
  // conferir o pacote.
  let chavePublicaDoCatalogo = "";
  if (!o.lista && existsSync(`${caminhoChaveCat}.pub.pem`)) {
    chavePublicaDoCatalogo = readFileSync(`${caminhoChaveCat}.pub.pem`, "utf8").trim();
  } else if (!o.lista && existsSync(caminhoChaveCat)) {
    // Deriva a pública da privada — evita exigir dois arquivos sincronizados.
    const { createPublicKey } = await import("node:crypto");
    chavePublicaDoCatalogo = createPublicKey(readFileSync(caminhoChaveCat, "utf8"))
      .export({ type: "spki", format: "pem" })
      .toString()
      .trim();
  }

  const itens = [];
  const deFora = [];
  const avisos = [];

  for (const nome of arquivos) {
    const caminho = path.join(dirPacotes, nome);
    const bytes = readFileSync(caminho);
    const slugDoArquivo = nome.replace(/\.giochi$/, "");

    let meta;
    try {
      meta = metadataDoPacote(bytes);
    } catch (e) {
      deFora.push({ nome, motivo: e.message });
      continue;
    }

    // O slug vem do metadata, não do nome do arquivo: renomear o arquivo não
    // pode mudar a identidade do app (mesma regra do extrator no aparelho).
    const slug = meta.slug || slugDoArquivo;

    const sig = `${caminho}.sig`;
    if (!existsSync(sig)) {
      deFora.push({ nome, motivo: "sem .sig — ninguém conseguiria verificar este pacote" });
      continue;
    }
    const assinaturaBytes = readFileSync(sig);
    if (assinaturaBytes.length !== 64) {
      // 89 bytes = base64 gravado como texto. Já aconteceu neste projeto.
      deFora.push({
        nome,
        motivo: `.sig com ${assinaturaBytes.length} bytes; Ed25519 tem 64 (base64 gravado como texto?)`,
      });
      continue;
    }

    const sha256 = sha256Hex(bytes);

    // O hash do manifesto tem de bater com o arquivo. Sem isto, o catálogo
    // anuncia um hash e o CDN entrega outro — e o erro aparece no comprador.
    const caminhoManifesto = `${caminho}.manifest.json`;
    if (existsSync(caminhoManifesto)) {
      const manifesto = JSON.parse(readFileSync(caminhoManifesto, "utf8"));
      if (manifesto.sha256 && manifesto.sha256 !== sha256) {
        deFora.push({
          nome,
          motivo: `manifesto diz ${manifesto.sha256.slice(0, 8)}… e o arquivo é ${sha256.slice(0, 8)}…`,
        });
        continue;
      }
    } else {
      avisos.push(`${slug}: sem .manifest.json — o hash foi calculado do arquivo, sem confronto`);
    }

    // A chave que assinou o pacote. Para jogo publicado pela própria Giochi é a
    // **própria chave do catálogo**, e é isso que faz a cadeia fechar: o aparelho
    // conhece uma chave, o catálogo vem assinado por ela, e cada item traz a
    // chave que assinou o pacote. Terceiro aprovado declara a dele no metadata.
    const chavePacote = meta.chave_pacote || chavePublicaDoCatalogo || "";

    const url = o.urlBase
      ? `${o.urlBase}/${nome}`
      : `./${nome}`;
    if (!o.urlBase) {
      avisos.push(`${slug}: sem --url-base, a URL ficou relativa (./${nome})`);
    }

    itens.push({
      slug,
      titulo: meta.title ?? slug,
      ano: Number(o.anos[slug] ?? meta.releaseYear ?? 0),
      descricao: o.descricoes[slug] ?? meta.description ?? "",
      preco: {
        centavos: Number(o.precos[slug] ?? meta.price_cents ?? 0),
        moeda: meta.currency ?? "BRL",
      },
      versao: meta.version,
      tamanho_bytes: bytes.length,
      sha256,
      url,
      assinatura: assinaturaBytes.toString("base64"),
      chave_pacote: chavePacote,
    });
  }

  if (o.lista) {
    console.log(`entram no catálogo (${itens.length}):`);
    for (const i of itens) {
      const preco = i.preco.centavos === 0 ? "grátis" : `${i.preco.centavos} centavos`;
      console.log(`  ${i.slug.padEnd(26)} ${String(i.tamanho_bytes).padStart(8)} B  ${preco}`);
    }
    if (deFora.length) {
      console.log(`\nficam de fora (${deFora.length}):`);
      for (const d of deFora) console.log(`  ${d.nome.padEnd(26)} ${d.motivo}`);
    }
    for (const a of avisos) console.log(`\naviso: ${a}`);
    return deFora.length > 0 ? 1 : 0;
  }

  const catalogo = {
    versao: VERSAO,
    gerado_em: Math.floor(Date.now() / 1000),
    invalida_antes: o.revogarAntes,
    itens,
  };

  // A chave de catálogo: a privada que assina a lista. Ela **não** é a chave de
  // licença e não é a do aparelho — três chaves, três perguntas.
  const assinado = assinarCatalogo(catalogo, readFileSync(caminhoChave, "utf8"));

  if (o.json) {
    console.log(JSON.stringify(assinado, null, 2));
    return 0;
  }

  const dirSaida = path.isAbsolute(o.saida) ? o.saida : path.join(RAIZ, o.saida);
  mkdirSync(dirSaida, { recursive: true });
  const destino = path.join(dirSaida, "catalogo.json");
  writeFileSync(destino, `${JSON.stringify(assinado, null, 2)}\n`);

  console.log(`${itens.length} app(s) no catálogo → ${destino}`);
  for (const a of avisos) console.log(`  aviso: ${a}`);
  if (deFora.length) {
    console.log(`  ${deFora.length} pacote(s) ficaram de fora:`);
    for (const d of deFora) console.log(`    ${d.nome}: ${d.motivo}`);
  }
  console.log(`
  próximo passo: publicar ${path.basename(destino)} e os .giochi no CDN,
  e usar a pública em app/loja (--chave-catalogo) e na imagem do aparelho.`);
  return deFora.length > 0 ? 1 : 0;
}

try {
  // `principal` é `async` por causa do `await import` da geração de chave.
  process.exitCode = await principal();
} catch (e) {
  console.error(`erro: ${e.message}`);
  process.exitCode = 1;
}
