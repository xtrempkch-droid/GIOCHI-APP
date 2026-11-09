#!/usr/bin/env node
// Empacota um app da pasta `app/` num `.giochi` assinado.
//
// Uso:
//   node scripts/empacotar.mjs app/jogos/pacman
//   node scripts/empacotar.mjs app/jogos/pacman --saida dist/pacotes
//
// Gera três arquivos, como o antecessor:
//   <slug>.giochi               o pacote
//   <slug>.giochi.sig           assinatura Ed25519 (base64)
//   <slug>.giochi.manifest.json sha256 + tamanho + assinatura
//
// ## A chave
//
// `--chave <arquivo.pem>` manda; sem ele, a resolução está em
// `scripts/lib/chave.mjs` (local histórico → chave do `Giochi System`, que é a
// que a imagem confia). Se não houver nenhuma, **cria** e avisa — e diz
// explicitamente que a imagem vai recusar até a pública entrar nela.
//
// A chave privada nunca vai para o repositório nem para o pacote: quem assina é a
// máquina de quem publica.

import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assinar,
  conferirEstrutura,
  escreverTar,
  gerarChaves,
  lerTar,
  ordenarMembros,
  slugDoPacote,
  validarPacote,
} from "./lib/publicar.mjs";
import { chavePrivadaDoDisco, combinaComAImagem, diretorioDasChavesDoSystem } from "./lib/chave.mjs";
import { readPngSize } from "./lib/png.mjs";

const raiz = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function argumentos(argv) {
  const opcoes = { app: null, saida: null, chave: null, json: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const proximo = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} precisa de um valor`);
      return v;
    };
    if (a === "--saida") opcoes.saida = resolve(proximo());
    else if (a === "--chave") opcoes.chave = resolve(proximo());
    else if (a === "--json") opcoes.json = true;
    else if (a === "-h" || a === "--ajuda") {
      console.log(
        [
          "empacotar.mjs — monta e assina um pacote .giochi",
          "",
          "  <pasta do app>       app/<slug> ou app/jogos/<slug>",
          "  --saida <dir>        onde gravar (padrão: dist/pacotes)",
          "  --chave <arquivo>    chave privada PEM Ed25519",
          "",
          "Gera <slug>.giochi, <slug>.giochi.sig e <slug>.giochi.manifest.json.",
        ].join("\n"),
      );
      process.exit(0);
    } else if (!a.startsWith("--")) {
      if (opcoes.app) throw new Error(`app especificado duas vezes: ${opcoes.app} e ${a}`);
      opcoes.app = resolve(a);
    } else {
      throw new Error(`argumento desconhecido: ${a}`);
    }
  }
  if (!opcoes.app) throw new Error("informe a pasta do app (ex.: app/jogos/pacman)");
  if (!opcoes.saida) opcoes.saida = join(raiz, "dist/pacotes");
  return opcoes;
}

/** `id` do `apps.json` para esta pasta, se ela estiver registrada. */
function idDoIndice(dirApp) {
  const caminho = join(raiz, "app/apps.json");
  if (!existsSync(caminho)) return null;
  try {
    const indice = JSON.parse(readFileSync(caminho, "utf8"));
    const rel = relative(join(raiz, "app"), dirApp).split(/[\\/]/).join("/");
    const entrada = (indice.apps ?? []).find((a) => (a.folder ?? a.id) === rel);
    return entrada?.id ?? null;
  } catch {
    return null;
  }
}

/** Chave privada: a pedida, uma das documentadas, ou uma nova — avisando. */
function obterChave(caminhoPedido) {
  const doDisco = caminhoPedido ?? chavePrivadaDoDisco();
  if (doDisco && existsSync(doDisco)) {
    return { privada: readFileSync(doDisco, "utf8"), publica: null, caminho: doDisco, criada: false };
  }

  const caminho = join(homedir(), ".giochi", "chave-privada.pem");
  const { privada, publica } = gerarChaves();
  mkdirSync(dirname(caminho), { recursive: true });
  // 0600: a privada assina pacotes; legível por outros usuários seria o mesmo
  // que não ter chave.
  writeFileSync(caminho, privada, { mode: 0o600 });
  const caminhoPublica = caminho.replace(/\.pem$/, "-publica.pem");
  writeFileSync(caminhoPublica, publica, { mode: 0o644 });
  return { privada, publica, caminho, caminhoPublica, criada: true };
}

/** Lista os arquivos de `bin/` com caminho relativo à pasta do app. */
function listarBin(dirApp) {
  const arquivos = [];
  const raizBin = join(dirApp, "bin");
  if (!existsSync(raizBin)) return arquivos;

  const percorre = (dir) => {
    for (const nome of readdirSync(dir).sort()) {
      const completo = join(dir, nome);
      const st = statSync(completo);
      if (st.isDirectory()) {
        percorre(completo);
      } else {
        // `.gitkeep` existe para o Git guardar a pasta vazia; empacotá-lo
        // produziria um pacote com lixo e sem funcionalidade.
        if (nome === ".gitkeep") continue;
        arquivos.push({
          caminho: relative(dirApp, completo).split(/[\\/]/).join("/"),
          conteudo: readFileSync(completo),
          // Preservar o bit de execução **do arquivo**, não inventar por nome.
          modo: st.mode & 0o111 ? 0o755 : 0o644,
        });
      }
    }
  };
  percorre(raizBin);
  return arquivos;
}

function principal() {
  const opcoes = argumentos(process.argv.slice(2));
  const dirApp = opcoes.app;

  if (!existsSync(dirApp)) throw new Error(`pasta não existe: ${dirApp}`);

  const caminhoMetadata = join(dirApp, "metadata.json");
  if (!existsSync(caminhoMetadata)) {
    throw new Error(
      `${dirApp} não tem metadata.json\n` +
        "— se é um grupo (app/jogos/), informe a pasta do app dentro dele",
    );
  }

  const metadataJson = readFileSync(caminhoMetadata, "utf8");
  let metadata;
  try {
    metadata = JSON.parse(metadataJson);
  } catch (e) {
    throw new Error(`metadata.json inválido: ${e.message}`);
  }

  const caminhoCapa = join(dirApp, "capa.png");
  const capa = existsSync(caminhoCapa) ? readFileSync(caminhoCapa) : null;
  // `readPngSize` devolve `{width, height}`; o validador trabalha com
  // `largura`/`altura`. A conversão fica aqui, num lugar só, para o nome
  // divergente não virar "undefined×undefined" na mensagem de erro.
  const medida = capa ? readPngSize(caminhoCapa) : null;
  const infoCapa = medida ? { largura: medida.width, altura: medida.height } : null;

  const caminhoThumb = join(dirApp, "thumb.png");
  const thumb = existsSync(caminhoThumb) ? readFileSync(caminhoThumb) : null;

  const arquivos = listarBin(dirApp);
  const binarios = arquivos.map((a) => a.caminho);

  const erros = validarPacote({
    metadata,
    capa: infoCapa,
    tamanhoMetadata: Buffer.byteLength(metadataJson, "utf8"),
    binarios,
  });

  if (erros.length > 0) {
    console.error(`${basename(dirApp)}: NÃO publicável`);
    for (const e of erros) console.error(`  - ${e}`);
    process.exit(1);
  }

  const membros = ordenarMembros({ metadataJson, capa, thumb, arquivos });
  const pacote = escreverTar(membros);

  // Confere os **bytes finais**, não a lista que acabamos de montar: só assim um
  // erro do empacotador (ordem trocada, exec perdido) aparece aqui e não no
  // aparelho.
  const estrutura = conferirEstrutura(pacote);
  if (estrutura.erros.length > 0) {
    console.error("pacote montado mas inconsistente:");
    for (const e of estrutura.erros) console.error(`  - ${e}`);
    process.exit(1);
  }

  const chave = obterChave(opcoes.chave);
  // A chave que assina tem de ser a mesma cuja pública está na imagem. Quando
  // não é, o pacote sai íntegro e o aparelho recusa — o erro aparece longe da
  // causa. Avisar aqui é a diferença entre uma rodada e uma sessão perdida.
  if (combinaComAImagem(chave.privada) === false) {
    console.warn("aviso: esta chave NÃO é a da imagem do aparelho — ela vai recusar o pacote");
    console.warn("       (a pública em /etc/giochi/loja-publica.pem é outra)");
  }
  // O slug **não** é o nome da pasta: `jogos/pacman` → `pacman`, e um app
  // renomeado na árvore não pode mudar o diretório no aparelho.
  const { slug, origem: origemSlug } = slugDoPacote(metadata, idDoIndice(dirApp));
  const nomePacote = `${slug}.giochi`;
  const { manifesto, assinatura } = assinar(pacote, chave.privada, {
    slug,
    version: metadata.version,
    nomePacote,
  });

  mkdirSync(opcoes.saida, { recursive: true });
  const caminhoPacote = join(opcoes.saida, nomePacote);
  writeFileSync(caminhoPacote, pacote);
  // Os 64 bytes crus: e o que `openssl pkeyutl -verify -rawin -sigfile` le.
  writeFileSync(`${caminhoPacote}.sig`, assinatura);
  writeFileSync(`${caminhoPacote}.manifest.json`, JSON.stringify(manifesto, null, 2) + "\n");

  if (opcoes.json) {
    console.log(
      JSON.stringify(
        { app: dirApp, pacote: caminhoPacote, membros: estrutura.membros, manifesto },
        null,
        2,
      ),
    );
    return;
  }

  console.log(`${slug}: pacote gerado`);
  console.log(`  arquivo: ${caminhoPacote}`);
  console.log(`  tamanho: ${pacote.length} bytes`);
  console.log(`  sha256:  ${manifesto.sha256}`);
  console.log(`  membros: ${estrutura.membros.join(" → ")}`);
  console.log(`  entrypoint: ${estrutura.entrypoint}`);
  console.log(`  slug:    ${slug} (de ${origemSlug})`);
  console.log(`  chave:   ${chave.caminho}${chave.criada ? " (CRIADA agora)" : ""}`);
  if (chave.criada) {
    console.log("");
    console.log(`  chave pública gravada em ${chave.caminhoPublica}`);
    console.log("  é ela que vai para o aparelho, em /etc/giochi/loja-publica.pem");
    console.log("  a privada NÃO vai para o repositório nem para o pacote.");
    console.log("");
    console.log("  ATENÇÃO: a imagem em uso confia na chave do Giochi System.");
    console.log("  Este pacote só instala depois que esta pública entrar em");
    console.log("  /etc/giochi/loja-publica.pem — senão o aparelho recusa a assinatura.");
    console.log("  Em máquina de desenvolvimento, o esperado é achar a chave em");
    console.log(`  ${join(diretorioDasChavesDoSystem(), "loja-privada.pem")}`);
  }
}

try {
  principal();
} catch (e) {
  console.error(`erro: ${e.message}`);
  process.exit(2);
}
