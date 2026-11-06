#!/usr/bin/env node
// Testes do validador de apps (scripts/lib/apps.mjs e scripts/lib/png.mjs).
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { listApps, readIndex, validateApp } from "../scripts/lib/apps.mjs";
import { readPngSize } from "../scripts/lib/png.mjs";

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** PNG sintético com assinatura + IHDR mínimo, suficiente para ler a dimensão. */
function fakePng(width, height) {
  const buffer = Buffer.alloc(24);
  SIGNATURE.copy(buffer, 0);
  buffer.writeUInt32BE(13, 8);
  buffer.write("IHDR", 12, "ascii");
  buffer.writeUInt32BE(width, 16);
  buffer.writeUInt32BE(height, 20);
  return buffer;
}

const VALID_METADATA = {
  title: "Meu App",
  developer: "Alguém",
  version: "1.0.0",
  arch: "aarch64",
  runtime: "godot",
  entrypoint: "bin/app",
};

function withTempDir(run) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-app-"));
  try {
    return run(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Cria uma pasta de app válida e devolve o caminho. */
function makeApp(parent, name = "meu-app", overrides = {}) {
  const dir = path.join(parent, name);
  mkdirSync(path.join(dir, "bin"), { recursive: true });
  writeFileSync(path.join(dir, "bin", "app"), "#!/bin/sh\n", "utf8");
  writeFileSync(
    path.join(dir, "metadata.json"),
    JSON.stringify({ ...VALID_METADATA, ...overrides.metadata }, null, 2),
    "utf8",
  );
  writeFileSync(
    path.join(dir, "capa.png"),
    overrides.capa ?? fakePng(320, 320),
  );
  if (overrides.forge) {
    writeFileSync(path.join(dir, "forge.json"), JSON.stringify(overrides.forge, null, 2), "utf8");
  }
  if (overrides.readme !== false) {
    writeFileSync(path.join(dir, "README.md"), "# App\n", "utf8");
  }
  return dir;
}

function codes(result) {
  return result.errors.map((item) => item.code);
}

test("aceita um app completo e válido", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir));
    assert.equal(result.ok, true, JSON.stringify(result.errors));
    assert.equal(result.errors.length, 0);
  });
});

test("recusa versão fora de semver", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { version: "1.0" } }));
    assert.equal(result.ok, false);
    assert.ok(codes(result).includes("versao_invalida"));
  });
});

test("recusa runtime inventado", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { runtime: "unity" } }));
    assert.ok(codes(result).includes("runtime_invalido"));
  });
});

test("aceita runtime native para app não-Godot", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { runtime: "native" } }));
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

// `x86_64` passou a ser um valor **válido** de `arch` (app de PC), então a
// recusa aqui não é mais "valor desconhecido" e sim o pareamento: app que vai
// para o aparelho não declara arquitetura de PC.
test("recusa arquitetura de PC num app do aparelho", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { arch: "x86_64" } }));
    assert.ok(
      codes(result).includes("arquitetura_de_pc_em_app_do_aparelho"),
      `códigos: ${codes(result).join(", ")}`,
    );
  });
});

test("recusa valor de arch inventado", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { arch: "mips" } }));
    assert.ok(codes(result).includes("arquitetura_invalida"));
  });
});

// O bug real: `store` e `forge` declararam `aarch64` durante meses sendo apps
// que rodam no PC do usuário. O pareamento existe para o campo significar algo.
test("recusa aarch64 num app de PC (distribution host)", () => {
  withTempDir((dir) => {
    const appDir = makeApp(dir, "a", { metadata: { arch: "aarch64" } });
    const result = validateApp(appDir, { distribution: "host" });
    assert.ok(
      codes(result).includes("arquitetura_de_console_em_app_de_pc"),
      `códigos: ${codes(result).join(", ")}`,
    );
  });
});

test("aceita as três arquiteturas de PC num app de PC", () => {
  for (const arch of ["x86_64", "arm64", "any"]) {
    withTempDir((dir) => {
      const appDir = makeApp(dir, "a", { metadata: { arch } });
      const result = validateApp(appDir, { distribution: "host" });
      assert.ok(
        !codes(result).some((c) => c.startsWith("arquitetura")),
        `${arch}: ${JSON.stringify(result.errors)}`,
      );
    });
  }
});

test("exige todos os campos obrigatórios", () => {
  withTempDir((dir) => {
    const appDir = makeApp(dir);
    writeFileSync(
      path.join(appDir, "metadata.json"),
      JSON.stringify({ title: "X", version: "1.0.0" }),
      "utf8",
    );
    const result = validateApp(appDir);
    const missing = result.errors.filter((item) => item.code === "campo_ausente");
    assert.equal(missing.length, 4, JSON.stringify(codes(result)));
  });
});

test("recusa campo não previsto no schema de metadata", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { autor: "x" } }));
    assert.ok(codes(result).includes("campo_desconhecido"));
  });
});

test("recusa entrypoint fora de bin/", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { entrypoint: "tools/app" } }),
    );
    assert.ok(codes(result).includes("entrypoint_fora_de_bin"));
  });
});

test("recusa entrypoint com ..", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { entrypoint: "bin/../../etc/passwd" } }),
    );
    assert.ok(codes(result).includes("entrypoint_inseguro"));
  });
});

test("recusa entrypoint que não existe no disco", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { entrypoint: "bin/nao-existe" } }),
    );
    assert.ok(codes(result).includes("entrypoint_ausente"));
  });
});

test("recusa capa fora de 320x320", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { capa: fakePng(100, 100) }));
    const error = result.errors.find((item) => item.code === "capa_dimensao");
    assert.ok(error, JSON.stringify(codes(result)));
    assert.match(error.message, /320x320/);
  });
});

test("avisa quando falta capa, sem reprovar", () => {
  withTempDir((dir) => {
    const appDir = makeApp(dir);
    rmSync(path.join(appDir, "capa.png"));
    const result = validateApp(appDir);
    assert.equal(result.ok, true);
    assert.ok(result.warnings.some((item) => item.code === "capa_ausente"));
  });
});

test("recusa capa que não é PNG", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { capa: Buffer.from("nao sou png") }));
    assert.ok(codes(result).includes("capa_invalida"));
  });
});

test("recusa pasta sem bin/", () => {
  withTempDir((dir) => {
    const appDir = makeApp(dir);
    rmSync(path.join(appDir, "bin"), { recursive: true, force: true });
    const result = validateApp(appDir);
    assert.ok(codes(result).includes("bin_ausente"));
  });
});

test("recusa nome de pasta que não é slug", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "Meu App"));
    assert.ok(codes(result).includes("nome_invalido"));
  });
});

test("aceita _template mesmo começando com underscore", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "_template"));
    assert.ok(!codes(result).includes("nome_invalido"));
  });
});

test("valida forge.json e recusa chave desconhecida", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { forge: { pacotes: ["BR2_PACKAGE_SDL2"] } }),
    );
    assert.ok(codes(result).includes("forge_campo_desconhecido"));
  });
});

test("valida forge.json e recusa inteiro negativo", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { forge: { ram_min_mb: -1 } }));
    assert.ok(codes(result).includes("forge_inteiro_invalido"));
  });
});

test("valida forge.json e recusa godot_version fora de major.minor", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { forge: { godot_version: "4" } }));
    assert.ok(codes(result).includes("forge_godot_version_invalida"));
  });
});

test("aceita forge.json completo e válido", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", {
        forge: {
          arch: "aarch64",
          runtime: "godot",
          graphics_api_min: "gles3",
          graphics_api_fallback: "gles2",
          godot_version: "4.3",
          ram_min_mb: 512,
          vram_min_mb: 64,
          display_min_width: 640,
          display_min_height: 480,
          touchscreen_required: false,
        },
      }),
    );
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

test("graphics_api_min 'none' sem fallback é aceito", () => {
  // É o caso dos jogos nativos em SDL2: rodam em software, então não exigem API
  // gráfica. Antes desta rodada eles não tinham forge.json nenhum, e o forge se
  // recusava a classificá-los — inclusive os que rodam de verdade no aparelho.
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { forge: { arch: "aarch64", runtime: "native", graphics_api_min: "none" } }),
    );
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

test("graphics_api_min 'none' COM fallback é contradição e reprova", () => {
  // Se não exige API nenhuma, não há de onde cair. Aceitar os dois juntos faria
  // o relatório dizer duas coisas sobre a mesma coisa.
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", {
        forge: {
          arch: "aarch64",
          runtime: "native",
          graphics_api_min: "none",
          graphics_api_fallback: "gles2",
        },
      }),
    );
    assert.ok(codes(result).includes("forge_fallback_com_min_none"), JSON.stringify(result.errors));
  });
});

test("readPngSize lê dimensões de PNG válido", () => {
  withTempDir((dir) => {
    const file = path.join(dir, "capa.png");
    writeFileSync(file, fakePng(320, 240));
    assert.deepEqual(readPngSize(file), { width: 320, height: 240 });
  });
});

test("readPngSize devolve null para arquivo que não é PNG", () => {
  withTempDir((dir) => {
    const file = path.join(dir, "x.png");
    writeFileSync(file, "conteudo qualquer");
    assert.equal(readPngSize(file), null);
  });
});

test("readPngSize devolve null para arquivo inexistente", () => {
  withTempDir((dir) => {
    assert.equal(readPngSize(path.join(dir, "nada.png")), null);
  });
});

test("listApps ignora _template e pastas ocultas", () => {
  withTempDir((dir) => {
    makeApp(dir, "alfa");
    makeApp(dir, "beta");
    makeApp(dir, "_template");
    mkdirSync(path.join(dir, ".git"), { recursive: true });
    const names = listApps(dir).map((item) => path.basename(item));
    assert.deepEqual(names, ["alfa", "beta"]);
  });
});

test("app em scaffold rebaixa entrypoint ausente para aviso", () => {
  withTempDir((dir) => {
    const appDir = makeApp(dir, "a", { metadata: { entrypoint: "bin/nao-existe" } });
    const ready = validateApp(appDir, { status: "ready" });
    const scaffold = validateApp(appDir, { status: "scaffold" });

    assert.equal(ready.ok, false);
    assert.ok(codes(ready).includes("entrypoint_ausente"));

    assert.equal(scaffold.ok, true, JSON.stringify(scaffold.errors));
    assert.ok(scaffold.warnings.some((item) => item.code === "entrypoint_pendente"));
  });
});

test("scaffold não perdoa outros erros", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { version: "1.0" } }), {
      status: "scaffold",
    });
    assert.equal(result.ok, false);
    assert.ok(codes(result).includes("versao_invalida"));
  });
});

test("readIndex lê app/apps.json", () => {
  withTempDir((dir) => {
    writeFileSync(
      path.join(dir, "apps.json"),
      JSON.stringify({
        schemaVersion: 1,
        apps: [
          { id: "alfa", folder: "alfa", runtime: "godot", status: "ready" },
          { id: "beta", folder: "beta", runtime: "native", status: "scaffold" },
        ],
      }),
      "utf8",
    );
    const { entries, errors } = readIndex(dir);
    assert.equal(errors.length, 0);
    assert.equal(entries.size, 2);
    assert.equal(entries.get("beta").status, "scaffold");
  });
});

test("readIndex recusa status inválido", () => {
  withTempDir((dir) => {
    writeFileSync(
      path.join(dir, "apps.json"),
      JSON.stringify({ apps: [{ id: "a", status: "quase" }] }),
      "utf8",
    );
    const { errors } = readIndex(dir);
    assert.ok(errors.some((item) => item.code === "indice_status_invalido"));
  });
});

test("readIndex lida com índice ausente sem quebrar", () => {
  withTempDir((dir) => {
    const { entries, errors } = readIndex(dir);
    assert.equal(entries.size, 0);
    assert.equal(errors.length, 0);
  });
});

test("readIndex recusa JSON inválido", () => {
  withTempDir((dir) => {
    writeFileSync(path.join(dir, "apps.json"), "{ nao json", "utf8");
    const { errors } = readIndex(dir);
    assert.ok(errors.some((item) => item.code === "indice_json_invalido"));
  });
});

test("aceita identificador SPDX em license", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { license: "MIT" } }));
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

test("recusa license com formato inválido", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { license: "licença livre!" } }));
    assert.ok(codes(result).includes("licenca_invalida"));
  });
});

test("recusa licenseUrl que não é URL", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir, "a", { metadata: { licenseUrl: "ftp://x" } }));
    assert.ok(codes(result).includes("licenca_url_invalida"));
  });
});

test("recusa GPL na imagem assinada", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { license: "GPL-3.0-only" } }),
      { status: "ready", distribution: "image" },
    );
    assert.equal(result.ok, false);
    const error = result.errors.find((item) => item.code === "licenca_gpl_na_imagem");
    assert.ok(error, JSON.stringify(codes(result)));
    assert.match(error.message, /Installation Information|§6/);
  });
});

test("aceita GPL quando distribuído pela loja", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { license: "GPL-3.0-only" } }),
      { status: "ready", distribution: "store" },
    );
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

test("recusa AGPL e LGPL na imagem assinada", () => {
  withTempDir((dir) => {
    for (const license of ["AGPL-3.0-only", "LGPL-2.1-only"]) {
      const result = validateApp(makeApp(dir, license, { metadata: { license } }), {
        status: "ready",
        distribution: "image",
      });
      assert.ok(codes(result).includes("licenca_gpl_na_imagem"), license);
    }
  });
});

test("avisa quando app de imagem não declara licença", () => {
  withTempDir((dir) => {
    const result = validateApp(makeApp(dir), { status: "ready", distribution: "image" });
    assert.equal(result.ok, true);
    assert.ok(result.warnings.some((item) => item.code === "licenca_ausente"));
  });
});

test("aceita licença proprietária na imagem", () => {
  withTempDir((dir) => {
    const result = validateApp(
      makeApp(dir, "a", { metadata: { license: "Proprietary" } }),
      { status: "ready", distribution: "image" },
    );
    assert.equal(result.ok, true, JSON.stringify(result.errors));
  });
});

test("readIndex recusa distribution inválida", () => {
  withTempDir((dir) => {
    writeFileSync(
      path.join(dir, "apps.json"),
      JSON.stringify({ apps: [{ id: "a", status: "ready", distribution: "disquete" }] }),
      "utf8",
    );
    const { errors } = readIndex(dir);
    assert.ok(errors.some((item) => item.code === "indice_distribuicao_invalida"));
  });
});

// O ano do jogo é dado do publicador, não do launcher.
//
// A cadeia é metadata.json -> índice da loja -> catálogo do aparelho. O launcher
// desenhava `#002` (o índice do cartão) porque o campo não existia em lugar
// nenhum; sem aceitar `releaseYear` aqui, a loja não tem o que repassar e o
// cartão volta a inventar.
test("aceita releaseYear e recusa ano fora de faixa", () => {
  withTempDir((dir) => {
    const ok = validateApp(makeApp(dir, "a", { metadata: { releaseYear: 1988 } }));
    assert.equal(ok.errors.length, 0, JSON.stringify(codes(ok)));

    for (const ano of [0, 2, 1899, 2200, "1988", 1988.5]) {
      withTempDir((sub) => {
        const { errors } = validateApp(makeApp(sub, "a", { metadata: { releaseYear: ano } }));
        assert.ok(
          errors.some((e) => e.code === "ano_invalido"),
          `releaseYear=${JSON.stringify(ano)} deveria reprovar`,
        );
      });
    }
  });
});

// ── grupos: app/jogos/<slug> ────────────────────────────────
//
// Os jogos passaram a viver em `app/jogos/`, e `app/jogos` é pasta mas não é
// app. O que distingue um do outro é o `metadata.json` — não uma lista de
// grupos escrita à mão, que envelheceria em outro lugar.

test("listApps desce em grupo e não trata o grupo como app", () => {
  withTempDir((dir) => {
    makeApp(dir, "launcher");
    makeApp(path.join(dir, "jogos"), "volley");
    makeApp(path.join(dir, "jogos"), "spacewar");

    const achados = listApps(dir).map((p) => path.relative(dir, p).split(path.sep).join("/"));
    assert.deepEqual(achados, ["jogos/spacewar", "jogos/volley", "launcher"]);
  });
});

test("listApps ignora pasta sem metadata.json no fundo", () => {
  withTempDir((dir) => {
    makeApp(dir, "launcher");
    // Pasta de trabalho: existe, não é app, não tem metadata.json.
    mkdirSync(path.join(dir, "rascunho", "assets"), { recursive: true });

    const achados = listApps(dir).map((p) => path.basename(p));
    assert.deepEqual(achados, ["launcher"]);
  });
});

test("listApps ignora _template e pastas ocultas", () => {
  withTempDir((dir) => {
    makeApp(dir, "launcher");
    makeApp(dir, "_template");
    makeApp(dir, ".cache");

    const achados = listApps(dir).map((p) => path.basename(p));
    assert.deepEqual(achados, ["launcher"]);
  });
});
