//! Gera o catálogo do aparelho a partir do que está versionado.
//!
//! ## O problema que isto resolve
//!
//! O `/opt/giochi/apps/index.json` do aparelho era **escrito à mão**. Ele não
//! derivava de nada: nem do `app/apps.json`, nem dos `metadata.json`. As
//! consequências apareceram na prática, em 04/11/2026:
//!
//! - instalei `cobrinha` e `pacman` e o aparelho continuou mostrando **um**
//!   jogo até eu reescrever o JSON à mão;
//! - um app instalado fora do JSON fica **invisível**, e não há erro em lugar
//!   nenhum — nem no app, nem no log, nem na tela;
//! - `title`, `year` e `category` podiam divergir do `metadata.json` do pacote
//!   sem que nada avisasse.
//!
//! ## A regra
//!
//! **O catálogo é projeção, não segunda fonte da verdade.** Tudo o que ele
//! contém sai de `app/apps.json` e dos `metadata.json`. É a mesma lição do
//! `releaseYear`, que precisou entrar em três lugares para não divergir: dado
//! que mora em dois lugares envelhece diferente nos dois.
//!
//! O que **não** é derivável e por isso está explícito aqui: o `dir` do
//! aparelho (`/opt/giochi/apps/<id>`). É convenção do `Giochi System`, não do
//! repositório, e o caminho no repo (`jogos/pacman`) não se traduz nele.

import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

import { readIndex } from "./apps.mjs";

// Onde o `Giochi System` põe os apps no aparelho. Ver `docs/integracao.md` §2.1.
export const DIR_BASE_PADRAO = "/opt/giochi/apps";

// O que entra no catálogo, e o que fica de fora (com motivo).
export const MOTIVOS = {
  /** Um pacote da loja está instalado e substitui a entrada do `apps.json`. */
  INSTALADO_SUBSTITUI: "instalado no aparelho; a entrada do apps.json foi substituída",
  SCAFFOLD: "status scaffold: a fonte não foi migrada ou o binário não foi exportado",
  HOST: "distribution host: roda no PC, não vai para o aparelho",
  SHELL: "role shell: é a própria interface, não algo para iniciar de dentro dela",
  /** O mesmo motivo de `SHELL`, mas a entrada veio de um `metadata.json` colhido
   *  no aparelho — o caso do launcher instalado como app. */
  PAPEL_SHELL: "role shell (declarado no apps.json): a cópia instalada não vira cartão",
  STORE: "distribution store: instalado em /user pela loja, não faz parte da imagem",
  SEM_BIN: "entrypoint não existe no disco",
  SEM_METADATA: "metadata.json ausente",
  METADATA_INVALIDO: "metadata.json não é JSON válido",
};

/**
 * Lê o `metadata.json` de uma pasta de app.
 *
 * Devolve `null` quando o arquivo não existe — que é diferente de inválido, e a
 * diferença aparece na mensagem.
 */
export function lerMetadata(appDir) {
  const caminho = path.join(appDir, "metadata.json");
  if (!existsSync(caminho)) return { metadata: null, motivo: MOTIVOS.SEM_METADATA };
  try {
    return { metadata: JSON.parse(readFileSync(caminho, "utf8")), motivo: null };
  } catch {
    return { metadata: null, motivo: MOTIVOS.METADATA_INVALIDO };
  }
}

/**
 * Monta o catálogo a partir da árvore.
 *
 * @param {string} appRoot  pasta `app/` do repositório
 * @param {{dirBase?: string, apenasInstalados?: boolean}} opcoes
 *   `dirBase` — prefixo do aparelho (padrão `/opt/giochi/apps`)
 *   `apenasInstalados` — quando `true`, também exige que o binário exista.
 *     É o modo usado antes de publicar: catálogo que aponta para binário
 *     ausente é pior que catálogo vazio, porque o usuário aperta e nada
 *     acontece.
 * @returns {{ catalogo: object, avisos: Array<{id: string, motivo: string}> }}
 */
export function gerarCatalogo(appRoot, { dirBase = DIR_BASE_PADRAO, apenasInstalados = true } = {}) {
  const { entries, errors } = readIndex(appRoot);
  if (errors.length > 0) {
    // Índice inválido é erro de build, não aviso: gerar catálogo a partir de um
    // `apps.json` quebrado produziria uma lista plausível e errada.
    throw new Error(`app/apps.json inválido: ${errors.map((e) => e.message).join("; ")}`);
  }

  const apps = [];
  const avisos = [];

  for (const [id, entrada] of entries) {
    const folder = entrada.folder || id;
    const appDir = path.join(appRoot, folder);

    if (!existsSync(appDir) || !statSync(appDir).isDirectory()) {
      avisos.push({ id, motivo: `pasta ausente: app/${folder}` });
      continue;
    }

    // A interface não lista a si mesma.
    //
    // O launcher tem `status: ready`, `distribution: image` e binário no lugar,
    // então entrava no catálogo por todos os critérios — e o cartão "Launcher"
    // abriria um SEGUNDO launcher, que brigaria com o primeiro pelo DRM master
    // e falharia com `kmsdrm not available`.
    //
    // O papel é **declarado** (`role: "shell"` no `apps.json`), não inferido do
    // nome: fixar `id === "launcher"` no gerador funcionaria até alguém renomear
    // a pasta, e o erro voltaria sem aviso.
    if (entrada.role === "shell") {
      avisos.push({ id, motivo: MOTIVOS.SHELL });
      continue;
    }

    // `scaffold` não tem binário: entra na árvore mas não no aparelho.
    if (entrada.status !== "ready") {
      avisos.push({ id, motivo: MOTIVOS.SCAFFOLD });
      continue;
    }

    // `host` roda no PC (forge, store). `store` é instalado em /user pela loja.
    // Nenhum dos dois faz parte da imagem assinada.
    const distribuicao = entrada.distribution || "image";
    if (distribuicao === "host") {
      avisos.push({ id, motivo: MOTIVOS.HOST });
      continue;
    }
    if (distribuicao === "store") {
      avisos.push({ id, motivo: MOTIVOS.STORE });
      continue;
    }

    const { metadata, motivo } = lerMetadata(appDir);
    if (!metadata) {
      avisos.push({ id, motivo });
      continue;
    }

    const entrypoint = metadata.entrypoint;
    if (typeof entrypoint !== "string" || !entrypoint.startsWith("bin/")) {
      avisos.push({ id, motivo: `entrypoint inválido no metadata.json: ${entrypoint}` });
      continue;
    }

    if (apenasInstalados) {
      const bin = path.join(appDir, entrypoint);
      if (!existsSync(bin)) {
        avisos.push({ id, motivo: `${MOTIVOS.SEM_BIN}: ${entrypoint}` });
        continue;
      }
    }

    // O `dir` do aparelho é convenção do sistema e **não** o caminho no repo.
    // `jogos/pacman` vira `/opt/giochi/apps/pacman`, e o `id` é que manda.
    apps.push({
      id,
      version: metadata.version,
      entrypoint,
      dir: `${dirBase}/${id}`,
      title: metadata.title,
      developer: metadata.developer,
      ...(metadata.releaseYear !== undefined ? { year: metadata.releaseYear } : {}),
      ...(metadata.category ? { category: metadata.category } : {}),
      license: entrada.license,
    });
  }

  // Ordem estável e previsível: por título. Sem isto a lista mudaria com a
  // ordem de inserção no `apps.json`, e a Home mudaria de conteúdo sem nada
  // ter mudado nela — o tipo de instabilidade que faz o teste mentir.
  apps.sort((a, b) => a.title.localeCompare(b.title, "pt-BR"));

  return { catalogo: { schemaVersion: 1, apps }, avisos };
}

/**
 * Junta apps **instalados no aparelho** ao catálogo gerado do repositório.
 *
 * ## Por que esta função existe
 *
 * `gerarCatalogo` deriva de `app/apps.json`. Um pacote `.giochi` vindo da loja
 * **não está lá**: ele chega no aparelho e é extraído em
 * `/opt/giochi/apps/<slug>/`. Sem esta junção, instalar um jogo pela loja o
 * deixaria invisível — e invisível sem erro nenhum, que é o pior modo de falha
 * já registrado neste projeto (o catálogo escrito à mão mostrava 1 jogo com 3
 * instalados).
 *
 * ## A regra de precedência
 *
 * Quem manda é o **disco**, não o `apps.json`. Se um id aparece nos dois, a
 * entrada instalada vence: ela é o que existe de fato no aparelho. O
 * `apps.json` descreve o que a imagem *deveria* ter; o diretório descreve o que
 * ela *tem*. Divergir é informação, não erro — vai para `avisos`.
 *
 * `instalados` vem da varredura do aparelho: `[{ id, metadata, dir }]`.
 */
export function mesclarInstalados(
  catalogo,
  instalados,
  { dirBase = DIR_BASE_PADRAO, papeis = new Map() } = {},
) {
  const avisos = [];
  const porId = new Map(catalogo.apps.map((a) => [a.id, a]));

  for (const inst of instalados) {
    const { id, metadata } = inst;
    if (!id || !metadata) continue;

    // ## `role: shell` vale também para o que veio do aparelho
    //
    // O papel é declarado no `apps.json`, e o gerador já o respeitava — mas só
    // para as entradas do repositório. Instalar o launcher **com o seu
    // `metadata.json`** no aparelho (que é o certo a fazer, e é o que o
    // `aparelho.sh catalogo` colhe) trazia ele de volta pela porta dos fundos:
    // medido em 09/11/2026 na imagem nova, o catálogo ficou com `Giochi
    // Launcher` e `Giochi Launcher (engine)` como se fossem jogos. Apertar `A`
    // no primeiro abriria um segundo launcher, que perde o DRM (exclusivo) e
    // morre com `kmsdrm not available`.
    //
    // O papel **não** pode vir do catálogo já filtrado: o launcher está fora
    // dele justamente por ser shell, então `porId` não tem a informação. É por
    // isso que o chamador passa `papeis` lido do `apps.json`.
    //
    // E herdar do repositório — em vez de fixar `id === "launcher"` — é o que
    // mantém a regra declarada: renomear a pasta não muda o papel.
    const papel = porId.get(id)?.role ?? papeis.get(id);
    if (papel === "shell") {
      avisos.push({ id, motivo: MOTIVOS.PAPEL_SHELL });
      continue;
    }

    const entrypoint = metadata.entrypoint;
    if (typeof entrypoint !== "string" || !entrypoint.startsWith("bin/")) {
      avisos.push({ id, motivo: `entrypoint inválido no metadata.json: ${entrypoint}` });
      continue;
    }

    const anterior = porId.get(id);

    const entrada = {
      id,
      version: metadata.version,
      entrypoint,
      dir: inst.dir || `${dirBase}/${id}`,
      title: metadata.title,
      developer: metadata.developer,
      ...(metadata.releaseYear !== undefined ? { year: metadata.releaseYear } : {}),
      ...(metadata.category ? { category: metadata.category } : {}),
      // Campos que **não** existem no `metadata.json` do pacote e só o
      // `apps.json` tem (`license`). Substituir a entrada inteira os apagava:
      // o pacman apareceu no catálogo sem licença depois de instalado pelo
      // pacote. Herdar do anterior preserva a informação em vez de perdê-la.
      // `license` também existe no `metadata.json` (é campo do schema), e o
      // app que **não** está no `apps.json` só tem essa fonte. Antes, um pacote
      // instalado sem entrada no repositório aparecia sem licença na ficha.
      // O `apps.json` continua ganhando quando existe: é ele que o validador
      // confere contra o `metadata.json`.
      ...(anterior?.license
        ? { license: anterior.license }
        : metadata.license
          ? { license: metadata.license }
          : {}),
    };

    if (anterior) {
      avisos.push({ id, motivo: MOTIVOS.INSTALADO_SUBSTITUI });
    }
    porId.set(id, entrada);
  }

  const apps = [...porId.values()].sort((a, b) => a.title.localeCompare(b.title, "pt-BR"));
  return { catalogo: { schemaVersion: catalogo.schemaVersion ?? 1, apps }, avisos };
}

/** Serializa com a mesma indentação que o resto do repositório usa. */
export function serializar(catalogo) {
  return `${JSON.stringify(catalogo, null, 2)}\n`;
}
