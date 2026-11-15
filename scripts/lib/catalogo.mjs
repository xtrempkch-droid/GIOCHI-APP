//! Gera o catálogo do aparelho a partir do que está versionado.
//!
//! ## O problema que isto resolve
//!
//! O `/opt/giochi/apps/index.json` do aparelho era **escrito à mão**. Ele não
//! derivava de nada: nem do `app/apps.json`, nem dos `metadata.json`. As
//! consequências apareceram na prática, em 04/11/2026:
//!
//! - instalei dois apps e o aparelho continuou mostrando **um** cartão até eu
//!   reescrever o JSON à mão;
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
//! repositório, e o caminho no repo (`<grupo>/<slug>`) não se traduz nele.

import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

import { ehTema, papelDeApp, readIndex, tipoDeMetadata } from "./apps.mjs";

// Onde o `Giochi System` põe os apps no aparelho. Ver `docs/integracao.md` §2.1.
export const DIR_BASE_PADRAO = "/opt/giochi/apps";

/**
 * Raízes de instalação por tipo.
 *
 * ## Por que separar
 *
 * Decisão do dono, 15/11/2026: "apps deverão ficar em locais diferentes dos
 * jogos para que a detecção não se confunda". Sem a separação, a aba Apps teria
 * de distinguir por declaração, e um pacote com o campo esquecido cairia na aba
 * errada sem aviso.
 *
 * | tipo | imagem | usuário / loja |
 * | --- | --- | --- |
 * | jogo | `/opt/giochi/games` | `/user/games` |
 * | app | `/opt/giochi/apps` | `/user/apps` |
 *
 * As quatro constam no `AGENTS.md` §8 e no detector do launcher
 * (`giochi_launcher_core::deteccao`), que é quem realmente varre o aparelho.
 * O gerador precisa delas para publicar o `dir` de um app de imagem.
 */
export const RAIZES = {
  game: { imagem: "/opt/giochi/games", usuario: "/user/games" },
  app: { imagem: "/opt/giochi/apps", usuario: "/user/apps" },
};

/** Todas as raízes de imagem, do jeito que o aparelho as vê. */
export const RAIZES_IMAGEM = [RAIZES.game.imagem, RAIZES.app.imagem];

/** Raiz de imagem do `kind` declarado (padrão: jogo). */
export function dirDoApp(id, kind) {
  return `${RAIZES[kind === "app" ? "app" : "game"].imagem}/${id}`;
}

/** Raiz de usuário do `kind` declarado — onde a loja/cartão instala. */
export function raizDeUsuario(kind) {
  return RAIZES[kind === "app" ? "app" : "game"].usuario;
}

/** `true` quando `dir` está dentro de `raiz`, por componente de caminho. */
export function dentroDe(dir, raiz) {
  const base = raiz.replace(/\/+$/, "");
  return dir === base || dir.startsWith(`${base}/`);
}

// O que entra no catálogo, e o que fica de fora (com motivo).
export const MOTIVOS = {
  /** Um pacote da loja está instalado e substitui a entrada do `apps.json`. */
  INSTALADO_SUBSTITUI: "instalado no aparelho; a entrada do apps.json foi substituída",
  SCAFFOLD: "status scaffold: a fonte não foi migrada ou o binário não foi exportado",
  HOST: "distribution host: roda no PC, não vai para o aparelho",
  SHELL: "role shell: é a própria interface, não algo para iniciar de dentro dela",
  /** Tema reconhecido pelo prefixo do nome, sem `role` declarado.
   *
   *  A mensagem separada existe porque "role shell" para um app que nunca
   *  declarou papel nenhum é a ferramenta dizendo que mediu a intenção em vez
   *  do efeito: quem lê `--lista` procuraria um `role` no `apps.json` que não
   *  está lá. O motivo tem de ser o motivo de verdade. */
  TEMA: 'tema do launcher (prefixo "launcher"): a interface não lista a si mesma',
  /** O mesmo motivo de `TEMA`/`SHELL`, mas a entrada veio de um `metadata.json`
   *  colhido no aparelho — o caso do launcher instalado como app. */
  PAPEL_SHELL: "tema do launcher: a cópia instalada no aparelho não vira cartão",
  STORE: "distribution store: instalado em /user pela loja, não faz parte da imagem",
  SEM_BIN: "entrypoint não existe no disco",
  /** O pacote declara um tipo e mora na raiz do outro.
   *
   *  Não é fatal: o launcher mostra o pacote na aba que ele **declara** e diz no
   *  log que o lugar está errado. Fatal seria o contrário — sumir com um pacote
   *  que existe porque ele está numa pasta que ninguém esperava. */
  LUGAR_ERRADO: "está na raiz do outro tipo (kind declarado não bate com a pasta)",
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
export function gerarCatalogo(appRoot, { dirBase = null, apenasInstalados = true } = {}) {
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
    // Tema do launcher: `launcher` e tudo que começa com `launcher-`.
    //
    // A regra vive em `papelDeApp`, e o prefixo do nome vale tanto quanto a
    // declaração `role: "shell"`. Motivo: criar um `launcher-<tema>` novo e
    // esquecer o `role` publicava um cartão que abria um SEGUNDO launcher —
    // que perde o DRM (exclusivo) e morre com `kmsdrm not available`. O erro é
    // fácil de cometer e o sintoma aparece longe da causa.
    if (papelDeApp(id, entrada.role) === "shell") {
      avisos.push({ id, motivo: ehTema(id) ? MOTIVOS.TEMA : MOTIVOS.SHELL });
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
    // `<grupo>/<slug>` vira `/opt/giochi/{games,apps}/<id>` — o tipo decide
    // qual das duas raízes, e o `id` é que manda no nome da pasta.
    const kind = tipoDeMetadata(metadata);
    apps.push({
      id,
      kind,
      version: metadata.version,
      entrypoint,
      // `dirBase` explícito vence (é o que os testes e o `--dir-base` usam);
      // sem ele, o tipo decide a raiz.
      dir: dirBase ? `${dirBase}/${id}` : dirDoApp(id, kind),
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
  { papeis = new Map() } = {},
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
    // Launcher` e `Giochi Launcher (engine)` como se fossem apps comuns.
    // Apertar `A` no primeiro abriria um segundo launcher, que perde o DRM
    // (exclusivo) e morre com `kmsdrm not available`.
    //
    // O papel **não** pode vir do catálogo já filtrado: o launcher está fora
    // dele justamente por ser shell, então `porId` não tem a informação. É por
    // isso que o chamador passa `papeis` lido do `apps.json`.
    //
    // E a mesma função decide os dois lados: o que está no `apps.json` e o que
    // só existe no aparelho. Um `launcher-<tema>` instalado por engano em
    // `/user/apps` cai aqui pela mesma regra de prefixo, sem depender de alguém
    // ter lembrado de declarar o papel.
    const papel = papelDeApp(id, porId.get(id)?.role ?? papeis.get(id));
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
    const kind = tipoDeMetadata(metadata);

    // O pacote declara um tipo e está na raiz do outro: aparece (a declaração
    // manda) e o aviso diz que o lugar está errado. Sumir com ele seria pior —
    // é o "app que existe e não aparece" que o catálogo projetado veio corrigir.
    const raizDoTipo = RAIZES[kind].usuario;
    const outraRaiz = RAIZES[kind === "app" ? "game" : "app"];
    const emOutraRaiz =
      inst.dir &&
      !dentroDe(inst.dir, raizDoTipo) &&
      (dentroDe(inst.dir, outraRaiz.usuario) || dentroDe(inst.dir, outraRaiz.imagem));
    if (emOutraRaiz) {
      avisos.push({
        id,
        motivo: `${MOTIVOS.LUGAR_ERRADO}: ${inst.dir} — o lugar de um ${kind === "app" ? "app" : "jogo"} é ${raizDoTipo}`,
      });
    }

    const entrada = {
      id,
      kind,
      version: metadata.version,
      entrypoint,
      dir: inst.dir || `${raizDoTipo}/${id}`,
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
