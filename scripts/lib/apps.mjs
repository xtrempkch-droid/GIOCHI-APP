// Validação de uma pasta de app.
//
// O objetivo é pegar, antes do push, os erros que hoje só aparecem tarde:
// versão fora de semver, entrypoint que não existe, capa fora de 320x320,
// runtime inventado. Todos quebram a loja em silêncio.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { readPngSize } from "./png.mjs";

export const ARCH_VALUES = ["aarch64"];
// "native" foi acrescentado em 02/11/2026. O antecessor registrou a lacuna
// (LOJA.md §6): com só godot/retroarch/proton, app nativo — como o forge e a
// store, escritos em Python — não tinha como se declarar.
export const RUNTIME_VALUES = ["godot", "retroarch", "proton", "native"];
export const GRAPHICS_MIN_VALUES = ["gles2", "gles3", "vulkan"];
export const GRAPHICS_FALLBACK_VALUES = ["gles2", "gles3"];

export const REQUIRED_METADATA = [
  "title",
  "developer",
  "version",
  "arch",
  "runtime",
  "entrypoint",
];

export const CAPA_SIZE = 320;

const SEMVER = /^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$/;
const GODOT_VERSION = /^[0-9]+\.[0-9]+$/;
const SLUG = /^[a-z0-9][a-z0-9-]*$/;
const LICENSE_ID = /^[A-Za-z0-9][A-Za-z0-9.+()-]*$/;

const FORGE_KEYS = new Set([
  "arch",
  "runtime",
  "graphics_api_min",
  "graphics_api_fallback",
  "godot_version",
  "ram_min_mb",
  "vram_min_mb",
  "display_min_width",
  "display_min_height",
  "touchscreen_required",
]);

function readJson(filePath, label, errors) {
  let text;
  try {
    text = readFileSync(filePath, "utf8");
  } catch {
    errors.push({ code: "arquivo_ilegivel", path: label, message: `não foi possível ler ${label}` });
    return null;
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    errors.push({
      code: "json_invalido",
      path: label,
      message: `${label} não é JSON válido: ${error.message}`,
    });
    return null;
  }
}

function validateMetadata(metadata, appDir, errors) {
  for (const key of REQUIRED_METADATA) {
    if (metadata[key] === undefined || metadata[key] === null || metadata[key] === "") {
      errors.push({
        code: "campo_ausente",
        path: `metadata.json:${key}`,
        message: `campo obrigatório ausente: ${key}`,
      });
    }
  }

  for (const key of Object.keys(metadata)) {
    if (
      !REQUIRED_METADATA.includes(key) &&
      ![
        "graphics_api",
        "buttons_map",
        "license",
        "licenseUrl",
        // O ano vem do publicador e é opcional: a loja repassa ao catálogo do
        // aparelho. Sem ele no schema, um app que declare o ano era reprovado
        // por `campo_desconhecido` e a informação não tinha por onde viajar.
        "releaseYear",
        // Mesmo motivo: o gênero aparece no cartão, e sem campo próprio o
        // gerador de catálogo não teria de onde tirá-lo.
        "category",
        // Identidade estável do pacote. Sem ele, a loja derivaria o diretório de
        // instalação do **título** — e renomear o título moveria o app de lugar,
        // quebrando o catálogo e o favorito do usuário. O `id` do `apps.json`
        // precisa ser igual quando os dois existem.
        "slug",
      ].includes(key)
    ) {
      errors.push({
        code: "campo_desconhecido",
        path: `metadata.json:${key}`,
        message: `campo não previsto no schema: ${key}`,
      });
    }
  }

  if (metadata.slug !== undefined) {
    const slug = metadata.slug;
    if (typeof slug !== "string" || !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(slug)) {
      errors.push({
        code: "slug_invalido",
        path: "metadata.json:slug",
        message: "slug precisa ser kebab-case (a-z, 0-9, hífen; sem hífen nas pontas)",
      });
    } else if (slug.length > 40) {
      errors.push({
        code: "slug_invalido",
        path: "metadata.json:slug",
        message: "slug tem no máximo 40 caracteres",
      });
    }
  }

  if (metadata.category !== undefined) {
    if (typeof metadata.category !== "string" || metadata.category.trim() === "") {
      errors.push({
        code: "categoria_invalida",
        path: "metadata.json:category",
        message: "category precisa ser texto não vazio (ex.: Arcade)",
      });
    } else if (metadata.category.length > 40) {
      errors.push({
        code: "categoria_invalida",
        path: "metadata.json:category",
        message: "category tem no máximo 40 caracteres",
      });
    }
  }

  if (metadata.releaseYear !== undefined) {
    const ano = metadata.releaseYear;
    // Faixa larga de propósito: 1900..2199 aceita todo jogo plausível e recusa
    // `2024` como string, ano de dois dígitos e índice de cartão (0, 1, 2) —
    // que foi exatamente o que apareceu na tela quando o campo não existia.
    if (!Number.isInteger(ano) || ano < 1900 || ano > 2199) {
      errors.push({
        code: "ano_invalido",
        path: "metadata.json:releaseYear",
        message: "releaseYear precisa ser inteiro de quatro dígitos (ex.: 1988)",
      });
    }
  }

  if (metadata.license !== undefined) {
    if (typeof metadata.license !== "string" || !LICENSE_ID.test(metadata.license)) {
      errors.push({
        code: "licenca_invalida",
        path: "metadata.json:license",
        message:
          "license precisa ser identificador SPDX (ex.: MIT, GPL-3.0-only, Apache-2.0) ou 'Proprietary'",
      });
    }
  }

  if (metadata.licenseUrl !== undefined) {
    if (typeof metadata.licenseUrl !== "string" || !/^https?:\/\//.test(metadata.licenseUrl)) {
      errors.push({
        code: "licenca_url_invalida",
        path: "metadata.json:licenseUrl",
        message: "licenseUrl precisa começar com http:// ou https://",
      });
    }
  }

  if (typeof metadata.version === "string" && !SEMVER.test(metadata.version)) {
    errors.push({
      code: "versao_invalida",
      path: "metadata.json:version",
      message: `version precisa ser semver (ex.: 1.0.0), recebido "${metadata.version}"`,
    });
  }

  if (metadata.arch !== undefined && !ARCH_VALUES.includes(metadata.arch)) {
    errors.push({
      code: "arquitetura_invalida",
      path: "metadata.json:arch",
      message: `arch só aceita ${ARCH_VALUES.join(", ")}; recebido "${metadata.arch}"`,
    });
  }

  if (metadata.runtime !== undefined && !RUNTIME_VALUES.includes(metadata.runtime)) {
    errors.push({
      code: "runtime_invalido",
      path: "metadata.json:runtime",
      message: `runtime só aceita ${RUNTIME_VALUES.join(", ")}; recebido "${metadata.runtime}"`,
    });
  }

  if (typeof metadata.entrypoint === "string") {
    const entry = metadata.entrypoint;
    if (!entry.startsWith("bin/")) {
      errors.push({
        code: "entrypoint_fora_de_bin",
        path: "metadata.json:entrypoint",
        message: `entrypoint deve começar em "bin/"; recebido "${entry}"`,
      });
    } else if (path.isAbsolute(entry) || entry.split("/").includes("..")) {
      errors.push({
        code: "entrypoint_inseguro",
        path: "metadata.json:entrypoint",
        message: `entrypoint não pode ser absoluto nem conter "..": "${entry}"`,
      });
    } else {
      const resolved = path.resolve(appDir, entry);
      const binDir = path.resolve(appDir, "bin");
      if (!resolved.startsWith(binDir + path.sep) && resolved !== binDir) {
        errors.push({
          code: "entrypoint_fora_de_bin",
          path: "metadata.json:entrypoint",
          message: `entrypoint escapa de bin/: "${entry}"`,
        });
      } else if (!existsSync(resolved)) {
        errors.push({
          code: "entrypoint_ausente",
          path: "metadata.json:entrypoint",
          message: `entrypoint aponta para arquivo que não existe: ${entry}`,
        });
      }
    }
  }
}

function validateForge(forge, errors) {
  for (const key of Object.keys(forge)) {
    if (!FORGE_KEYS.has(key)) {
      errors.push({
        code: "forge_campo_desconhecido",
        path: `forge.json:${key}`,
        message: `campo não previsto no schema: ${key}`,
      });
    }
  }

  if (forge.arch !== undefined && !ARCH_VALUES.includes(forge.arch)) {
    errors.push({
      code: "forge_arquitetura_invalida",
      path: "forge.json:arch",
      message: `arch só aceita ${ARCH_VALUES.join(", ")}`,
    });
  }
  if (forge.runtime !== undefined && !RUNTIME_VALUES.includes(forge.runtime)) {
    errors.push({
      code: "forge_runtime_invalido",
      path: "forge.json:runtime",
      message: `runtime só aceita ${RUNTIME_VALUES.join(", ")}`,
    });
  }
  if (
    forge.graphics_api_min !== undefined &&
    !GRAPHICS_MIN_VALUES.includes(forge.graphics_api_min)
  ) {
    errors.push({
      code: "forge_graphics_min_invalido",
      path: "forge.json:graphics_api_min",
      message: `graphics_api_min só aceita ${GRAPHICS_MIN_VALUES.join(", ")}`,
    });
  }
  if (
    forge.graphics_api_fallback !== undefined &&
    !GRAPHICS_FALLBACK_VALUES.includes(forge.graphics_api_fallback)
  ) {
    errors.push({
      code: "forge_graphics_fallback_invalido",
      path: "forge.json:graphics_api_fallback",
      message: `graphics_api_fallback só aceita ${GRAPHICS_FALLBACK_VALUES.join(", ")}`,
    });
  }
  if (forge.godot_version !== undefined && !GODOT_VERSION.test(String(forge.godot_version))) {
    errors.push({
      code: "forge_godot_version_invalida",
      path: "forge.json:godot_version",
      message: "godot_version precisa ser major.minor (ex.: 4.3)",
    });
  }
  for (const key of [
    "ram_min_mb",
    "vram_min_mb",
    "display_min_width",
    "display_min_height",
  ]) {
    if (forge[key] === undefined) continue;
    if (!Number.isInteger(forge[key]) || forge[key] < 0) {
      errors.push({
        code: "forge_inteiro_invalido",
        path: `forge.json:${key}`,
        message: `${key} precisa ser inteiro >= 0`,
      });
    }
  }
  if (forge.touchscreen_required !== undefined && typeof forge.touchscreen_required !== "boolean") {
    errors.push({
      code: "forge_booleano_invalido",
      path: "forge.json:touchscreen_required",
      message: "touchscreen_required precisa ser booleano",
    });
  }
}

function validateImages(appDir, errors, warnings) {
  const capa = path.join(appDir, "capa.png");
  if (existsSync(capa)) {
    const size = readPngSize(capa);
    if (!size) {
      errors.push({
        code: "capa_invalida",
        path: "capa.png",
        message: "capa.png não é um PNG válido",
      });
    } else if (size.width !== CAPA_SIZE || size.height !== CAPA_SIZE) {
      errors.push({
        code: "capa_dimensao",
        path: "capa.png",
        message: `capa.png precisa ser ${CAPA_SIZE}x${CAPA_SIZE}; recebido ${size.width}x${size.height}`,
      });
    }
  } else {
    warnings.push({
      code: "capa_ausente",
      path: "capa.png",
      message: "sem capa.png o app não pode ser publicado na loja",
    });
  }

  const thumb = path.join(appDir, "thumb.png");
  if (existsSync(thumb) && !readPngSize(thumb)) {
    errors.push({
      code: "thumb_invalida",
      path: "thumb.png",
      message: "thumb.png não é um PNG válido",
    });
  }
}

function validateBin(appDir, errors, warnings) {
  const bin = path.join(appDir, "bin");
  if (!existsSync(bin) || !statSync(bin).isDirectory()) {
    errors.push({
      code: "bin_ausente",
      path: "bin/",
      message: "a pasta bin/ é obrigatória: é o que vai para o aparelho",
    });
    return;
  }
  const entries = readdirSync(bin).filter((item) => item !== ".gitkeep");
  if (entries.length === 0) {
    warnings.push({
      code: "bin_vazia",
      path: "bin/",
      message: "bin/ está vazia; o app ainda não tem o que instalar",
    });
  }
}

export const STATUS_VALUES = ["scaffold", "ready"];
// image — vai dentro da imagem assinada do aparelho
// store — distribuído pela loja, instalado em /user no aparelho
// host  — roda no computador do usuário; não vai para o aparelho
export const DISTRIBUTION_VALUES = ["image", "store", "host"];

/**
 * Lê app/apps.json, a fonte da verdade da lista.
 * Devolve { entries: Map<slug, entry>, errors }.
 */
export function readIndex(appRoot) {
  const indexPath = path.join(appRoot, "apps.json");
  const errors = [];

  if (!existsSync(indexPath)) {
    return { entries: new Map(), errors };
  }

  let parsed;
  try {
    parsed = JSON.parse(readFileSync(indexPath, "utf8"));
  } catch (error) {
    errors.push({
      code: "indice_json_invalido",
      path: "app/apps.json",
      message: `apps.json não é JSON válido: ${error.message}`,
    });
    return { entries: new Map(), errors };
  }

  if (!parsed || !Array.isArray(parsed.apps)) {
    errors.push({
      code: "indice_invalido",
      path: "app/apps.json",
      message: "apps.json precisa ter a chave 'apps' como lista",
    });
    return { entries: new Map(), errors };
  }

  const entries = new Map();
  for (const item of parsed.apps) {
    if (!item || typeof item.id !== "string") {
      errors.push({
        code: "indice_entrada_invalida",
        path: "app/apps.json",
        message: "toda entrada precisa de 'id' string",
      });
      continue;
    }
    if (!STATUS_VALUES.includes(item.status)) {
      errors.push({
        code: "indice_status_invalido",
        path: `app/apps.json:${item.id}`,
        message: `status precisa ser ${STATUS_VALUES.join(" ou ")}; recebido "${item.status}"`,
      });
    }
    if (item.distribution !== undefined && !DISTRIBUTION_VALUES.includes(item.distribution)) {
      errors.push({
        code: "indice_distribuicao_invalida",
        path: `app/apps.json:${item.id}`,
        message: `distribution precisa ser ${DISTRIBUTION_VALUES.join(" ou ")}; recebido "${item.distribution}"`,
      });
    }
    entries.set(item.id, item);
  }

  return { entries, errors };
}

/**
 * Valida uma pasta de app.
 * `status` "scaffold" rebaixa a ausência de entrypoint para aviso: o app existe
 * na árvore, mas a fonte ainda não foi migrada e o binário não foi exportado.
 * Sem isso o CI ficaria vermelho desde o primeiro push e ninguém olharia mais.
 */
export function validateApp(appDir, { status = "ready", distribution = "image" } = {}) {
  const errors = [];
  const warnings = [];
  const name = path.basename(appDir);
  let parsedMetadata = null;

  if (!existsSync(appDir) || !statSync(appDir).isDirectory()) {
    return {
      ok: false,
      app: name,
      errors: [
        { code: "pasta_ausente", path: name, message: `pasta não encontrada: ${appDir}` },
      ],
      warnings,
    };
  }

  // _template é a única pasta que começa com underscore.
  if (!SLUG.test(name) && name !== "_template") {
    errors.push({
      code: "nome_invalido",
      path: name,
      message: "nome da pasta deve ser slug kebab (a-z, 0-9, -) ou _template",
    });
  }

  const metadataPath = path.join(appDir, "metadata.json");
  if (!existsSync(metadataPath)) {
    errors.push({
      code: "metadata_ausente",
      path: "metadata.json",
      message: "metadata.json é obrigatório",
    });
  } else {
    const metadata = readJson(metadataPath, "metadata.json", errors);
    if (metadata && typeof metadata === "object" && !Array.isArray(metadata)) {
      parsedMetadata = metadata;
      validateMetadata(metadata, appDir, errors);
    } else if (metadata !== null) {
      errors.push({
        code: "metadata_invalido",
        path: "metadata.json",
        message: "metadata.json precisa ser um objeto",
      });
    }
  }

  const forgePath = path.join(appDir, "forge.json");
  if (existsSync(forgePath)) {
    const forge = readJson(forgePath, "forge.json", errors);
    if (forge && typeof forge === "object" && !Array.isArray(forge)) {
      validateForge(forge, errors);
    } else if (forge !== null) {
      errors.push({
        code: "forge_invalido",
        path: "forge.json",
        message: "forge.json precisa ser um objeto",
      });
    }
  }

  validateImages(appDir, errors, warnings);
  validateBin(appDir, errors, warnings);

  if (!existsSync(path.join(appDir, "README.md"))) {
    warnings.push({
      code: "readme_ausente",
      path: "README.md",
      message: "sem README.md fica difícil saber o que o app faz",
    });
  }

  // Regra de licença × onde o app é distribuído.
  //
  // A GPL-3.0 §6 (Installation Information) exige, ao entregar código-objeto GPL
  // num "User Product", fornecer os meios — inclusive chaves — de instalar versão
  // modificada. O Giochi tem rootfs assinado com chave do projeto: ninguém além
  // do projeto consegue assinar a raiz. Logo a dispensa do §6 não se aplica, e
  // app GPL não pode morar dentro da imagem assinada.
  //
  // Pela loja, em /user, funciona: a instalação vira um procedimento público sem
  // chave. Por isso a checagem é mecânica aqui, e não só documentada.
  if (distribution === "image") {
    const license = parsedMetadata ? parsedMetadata.license : undefined;
    if (!license) {
      warnings.push({
        code: "licenca_ausente",
        path: "metadata.json:license",
        message:
          "app vai na imagem assinada sem licença declarada; as obrigações de distribuição ficam indefinidas",
      });
    } else if (/^(A?GPL|LGPL)/i.test(String(license))) {
      errors.push({
        code: "licenca_gpl_na_imagem",
        path: "metadata.json:license",
        message:
          `"${license}" não pode entrar na imagem assinada: a GPL §6 exige fornecer meios de instalar versão modificada, ` +
          `e a chave de assinatura é do projeto. Distribua pela loja em /user (distribution: "store").`,
      });
    }
  }

  // App em scaffold ainda não tem binário: a fonte não foi migrada e/ou o
  // export não foi feito. Isso é esperado, não é defeito.
  if (status === "scaffold") {
    for (let index = errors.length - 1; index >= 0; index -= 1) {
      if (errors[index].code === "entrypoint_ausente") {
        const [moved] = errors.splice(index, 1);
        warnings.push({
          code: "entrypoint_pendente",
          path: moved.path,
          message: `${moved.message} (esperado em app em scaffold)`,
        });
      }
    }
  }

  return { ok: errors.length === 0, app: name, status, distribution, errors, warnings };
}

/**
 * Lista as pastas de app dentro de `app/`, ignorando `_template`.
 *
 * ## Grupos
 *
 * Um jogo não é a mesma coisa que uma ferramenta, e a árvore diz isso:
 * `app/launcher`, `app/forge` e `app/store` são apps soltos, enquanto os jogos
 * vivem em `app/jogos/<slug>`. **O que distingue um do outro é o
 * `metadata.json`** — o arquivo que, desde o começo do contrato, é o que faz de
 * uma pasta um app.
 *
 * Por isso a varredura desce um nível apenas em pasta **sem** `metadata.json`.
 * Não há lista de grupos escritos à mão em lugar nenhum: quem cria a pasta e
 * põe `metadata.json` dentro já está dizendo o que ela é. Uma convenção que
 * morasse em dois arquivos envelheceria diferente nos dois.
 *
 * A profundidade máxima é 2 (`app/grupo/app`). Mais que isso não é organização,
 * é hierarquia inventada — e nada no contrato precisa dela.
 */
export function listApps(appRoot) {
  if (!existsSync(appRoot)) return [];
  const encontrados = [];

  const visitar = (dir, nivel) => {
    for (const item of readdirSync(dir)) {
      if (item.startsWith("_") || item.startsWith(".")) continue;
      const completo = path.join(dir, item);
      let ehDir = false;
      try {
        ehDir = statSync(completo).isDirectory();
      } catch {
        continue;
      }
      if (!ehDir) continue;

      if (existsSync(path.join(completo, "metadata.json"))) {
        encontrados.push(completo);
      } else if (nivel + 1 < 2) {
        // Sem `metadata.json` e ainda no nível 1: é grupo. Desce.
        visitar(completo, nivel + 1);
      }
      // Sem `metadata.json` e no fundo: não é app nem grupo. Ignora em silêncio
      // — pode ser um diretório de trabalho de quem está montando um app.
    }
  };

  visitar(appRoot, 0);
  return encontrados.sort();
}
