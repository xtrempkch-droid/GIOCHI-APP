#!/usr/bin/env node
// Testes do gerador de catálogo (scripts/lib/catalogo.mjs).
//
// O ponto destes testes: cada regra de exclusão existe por um motivo concreto
// que já apareceu na prática, ou que apareceria na primeira vez que alguém
// instalasse um app e ele não surgisse na tela.
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  DIR_BASE_PADRAO,
  gerarCatalogo,
  lerMetadata,
  mesclarInstalados,
  MOTIVOS,
  serializar,
} from "../scripts/lib/catalogo.mjs";

function withTempDir(run) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-cat-"));
  try {
    return run(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const METADATA_BASE = {
  title: "Jogo",
  developer: "Giochi",
  version: "1.0.0",
  arch: "aarch64",
  runtime: "native",
  entrypoint: "bin/jogo",
};

/**
 * Cria uma app na árvore e devolve o registro correspondente do `apps.json`.
 *
 * `comBinario` separa "o registro está certo" de "o arquivo está no disco" —
 * a diferença é justamente o que a checagem de binário mede.
 */
function criarApp(appRoot, id, { folder, metadata = {}, registro = {}, comBinario = true } = {}) {
  const pasta = folder || id;
  const dir = path.join(appRoot, pasta);
  mkdirSync(path.join(dir, "bin"), { recursive: true });
  writeFileSync(
    path.join(dir, "metadata.json"),
    JSON.stringify({ ...METADATA_BASE, ...metadata }, null, 2),
    "utf8",
  );
  if (comBinario) {
    const entry = metadata.entrypoint || METADATA_BASE.entrypoint;
    writeFileSync(path.join(dir, entry), "#!/bin/sh\n", { mode: 0o755 });
  }
  return {
    id,
    folder: pasta,
    runtime: "native",
    status: "ready",
    distribution: "image",
    license: "Proprietary",
    ...registro,
  };
}

function escreverIndice(appRoot, apps) {
  writeFileSync(
    path.join(appRoot, "apps.json"),
    JSON.stringify({ schemaVersion: 2, apps }, null, 2),
    "utf8",
  );
}

test("deriva título, ano e categoria do metadata.json", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "pacman", {
        metadata: { title: "Pac-Man", category: "Arcade", releaseYear: 2026 },
      }),
    ]);
    const { catalogo } = gerarCatalogo(dir);
    assert.equal(catalogo.apps.length, 1);
    const app = catalogo.apps[0];
    assert.equal(app.title, "Pac-Man");
    assert.equal(app.year, 2026);
    assert.equal(app.category, "Arcade");
    assert.equal(app.entrypoint, "bin/jogo");
  });
});

// O `dir` do aparelho é convenção do Giochi System e NÃO se deriva do caminho
// no repositório. `jogos/pacman` vira `/opt/giochi/apps/pacman` — o `id` manda.
test("dir do aparelho usa o id, não o caminho do repositório", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [criarApp(dir, "pacman", { folder: "jogos/pacman" })]);
    const { catalogo } = gerarCatalogo(dir);
    assert.equal(catalogo.apps[0].dir, `${DIR_BASE_PADRAO}/pacman`);
    assert.ok(!catalogo.apps[0].dir.includes("jogos"));
  });
});

// O launcher tem status ready, distribution image e binário no lugar — entrava
// por todos os critérios. O cartão "Launcher" abriria um SEGUNDO launcher e os
// dois brigariam pelo DRM master.
test("a interface não lista a si mesma", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "shell", { registro: { role: "shell" } }),
      criarApp(dir, "jogo"),
    ]);
    const { catalogo, avisos } = gerarCatalogo(dir);
    assert.deepEqual(
      catalogo.apps.map((a) => a.id),
      ["jogo"],
    );
    assert.ok(avisos.some((a) => a.id === "shell" && /shell/.test(a.motivo)));
  });
});

test("scaffold fica de fora, mesmo com binário no disco", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "pronto"),
      criarApp(dir, "rascunho", { registro: { status: "scaffold" } }),
    ]);
    const { catalogo, avisos } = gerarCatalogo(dir);
    assert.deepEqual(
      catalogo.apps.map((a) => a.id),
      ["pronto"],
    );
    assert.ok(avisos.some((a) => a.id === "rascunho"));
  });
});

test("host e store não vão para a imagem", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "daImagem"),
      criarApp(dir, "noPc", { registro: { distribution: "host" } }),
      criarApp(dir, "daLoja", { registro: { distribution: "store" } }),
    ]);
    const { catalogo } = gerarCatalogo(dir);
    assert.deepEqual(
      catalogo.apps.map((a) => a.id),
      ["daImagem"],
    );
  });
});

// Catálogo apontando para executável ausente é PIOR que catálogo vazio: o
// usuário vê o jogo, aperta, e nada acontece — sem mensagem em lugar nenhum.
test("entrypoint ausente no disco exclui a entrada", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "instalado"),
      criarApp(dir, "semBinario", { comBinario: false }),
    ]);
    const { catalogo, avisos } = gerarCatalogo(dir);
    assert.deepEqual(
      catalogo.apps.map((a) => a.id),
      ["instalado"],
    );
    assert.ok(avisos.some((a) => a.id === "semBinario" && /não existe/.test(a.motivo)));
  });
});

test("--tudo ignora a checagem de binário", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [criarApp(dir, "semBinario", { comBinario: false })]);
    const { catalogo } = gerarCatalogo(dir, { apenasInstalados: false });
    assert.equal(catalogo.apps.length, 1);
  });
});

test("pasta registrada mas ausente vira aviso, não exceção", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [{ id: "fantasma", folder: "fantasma", status: "ready", distribution: "image" }]);
    const { catalogo, avisos } = gerarCatalogo(dir);
    assert.equal(catalogo.apps.length, 0);
    assert.ok(avisos.some((a) => a.id === "fantasma"));
  });
});

// Ordem estável: sem isto a Home mudaria de conteúdo sem nada ter mudado nela,
// e o teste de frame mentiria sobre uma regressão que não houve.
test("a lista sai em ordem de título", () => {
  withTempDir((dir) => {
    escreverIndice(dir, [
      criarApp(dir, "z", { metadata: { title: "Zebra" } }),
      criarApp(dir, "a", { metadata: { title: "Abacaxi" } }),
      criarApp(dir, "m", { metadata: { title: "Melancia" } }),
    ]);
    const { catalogo } = gerarCatalogo(dir);
    assert.deepEqual(
      catalogo.apps.map((a) => a.title),
      ["Abacaxi", "Melancia", "Zebra"],
    );
  });
});

test("apps.json inválido é erro, não catálogo vazio", () => {
  withTempDir((dir) => {
    writeFileSync(path.join(dir, "apps.json"), "{ não é json", "utf8");
    assert.throws(() => gerarCatalogo(dir), /apps\.json inválido/);
  });
});

test("lerMetadata distingue ausente de inválido", () => {
  withTempDir((dir) => {
    mkdirSync(path.join(dir, "vazio"), { recursive: true });
    assert.match(lerMetadata(path.join(dir, "vazio")).motivo, /ausente/);

    mkdirSync(path.join(dir, "quebrado"), { recursive: true });
    writeFileSync(path.join(dir, "quebrado", "metadata.json"), "{", "utf8");
    assert.match(lerMetadata(path.join(dir, "quebrado")).motivo, /não é JSON/);
  });
});

test("serialização termina com newline e é JSON válido", () => {
  const texto = serializar({ schemaVersion: 1, apps: [] });
  assert.ok(texto.endsWith("\n"));
  assert.deepEqual(JSON.parse(texto), { schemaVersion: 1, apps: [] });
});

// A árvore real do repositório.
//
// A lista era um retrato dos jogos prontos — e retrato de catálogo reprova por
// motivo errado: apagar um jogo é decisão de produto, não defeito de código.
// O que o teste realmente garante continua valendo e não depende de quais apps
// existem: `distribution: image` + `bin/` entram, `distribution: host` e
// `role: shell` ficam fora, e todo `dir` sai da base padrão.
test("na árvore real: o launcher não entra e todo dir sai da base", () => {
  const appRoot = path.resolve(import.meta.dirname, "..", "app");
  const { catalogo } = gerarCatalogo(appRoot);
  const ids = catalogo.apps.map((a) => a.id);

  assert.ok(!ids.includes("launcher"), "o launcher não pode estar no catálogo");
  for (const id of ["forge", "store", "painel", "loja"]) {
    assert.ok(!ids.includes(id), `${id} roda no PC e não pode estar no catálogo`);
  }

  for (const app of catalogo.apps) {
    assert.ok(app.dir.startsWith(DIR_BASE_PADRAO), `${app.id}: dir fora da base`);
    assert.ok(app.title, `${app.id}: sem título`);
  }
});

// ── Pacotes da loja instalados no aparelho ─────────────────
//
// Estes testes cobrem o buraco que existia: um `.giochi` vem da loja, é extraído
// em `/opt/giochi/apps/<slug>/` e **não está** no `apps.json`. Sem a junção ele
// ficaria fora do catálogo — invisível, e sem nenhum erro.

const META_INSTALADO = {
  slug: "pacman",
  title: "Pac-Man",
  developer: "Giochi",
  version: "0.1.0",
  arch: "aarch64",
  runtime: "native",
  entrypoint: "bin/giochi-pacman",
  category: "Arcade",
  releaseYear: 1980,
};

test("app instalado que não está no apps.json entra no catálogo", () => {
  const base = { schemaVersion: 1, apps: [] };
  const { catalogo } = mesclarInstalados(base, [
    { id: "pacman", dir: "/opt/giochi/apps/pacman", metadata: META_INSTALADO },
  ]);

  assert.equal(catalogo.apps.length, 1);
  const app = catalogo.apps[0];
  assert.equal(app.id, "pacman");
  assert.equal(app.dir, "/opt/giochi/apps/pacman");
  assert.equal(app.entrypoint, "bin/giochi-pacman");
  assert.equal(app.year, 1980);
  assert.equal(app.category, "Arcade");
});

test("instalado substitui a entrada do repositório e o aviso diz isso", () => {
  const base = {
    schemaVersion: 1,
    apps: [
      {
        id: "pacman",
        version: "0.0.1",
        entrypoint: "bin/giochi-pacman",
        dir: "/opt/giochi/apps/pacman",
        title: "Pacman",
        developer: "Giochi",
      },
    ],
  };
  const { catalogo, avisos } = mesclarInstalados(base, [
    {
      id: "pacman",
      metadata: { ...META_INSTALADO, version: "0.2.0", title: "Pac-Man" },
    },
  ]);

  assert.equal(catalogo.apps.length, 1, "não pode duplicar");
  assert.equal(catalogo.apps[0].version, "0.2.0", "o disco vence");
  assert.ok(avisos.some((a) => a.motivo === MOTIVOS.INSTALADO_SUBSTITUI));
});

test("substituir preserva a licença, que só existe no apps.json", () => {
  // Regressão real: depois de instalar o pacote, o pacman apareceu no catálogo
  // do aparelho **sem `license`**, porque a entrada inteira era trocada.
  const base = {
    schemaVersion: 1,
    apps: [
      {
        id: "pacman",
        version: "0.1.0",
        entrypoint: "bin/giochi-pacman",
        dir: "/opt/giochi/apps/pacman",
        title: "Pac-Man",
        developer: "Giochi",
        license: "Proprietary",
      },
    ],
  };
  const { catalogo } = mesclarInstalados(base, [
    { id: "pacman", dir: "/opt/giochi/apps/pacman", metadata: META_INSTALADO },
  ]);
  assert.equal(catalogo.apps[0].license, "Proprietary", "a licença não pode sumir");
});

test("pacote com entrypoint inválido não entra no catálogo", () => {
  const base = { schemaVersion: 1, apps: [] };
  const { catalogo, avisos } = mesclarInstalados(base, [
    { id: "ruim", metadata: { ...META_INSTALADO, entrypoint: "/etc/passwd" } },
  ]);
  assert.equal(catalogo.apps.length, 0);
  assert.ok(avisos.some((a) => a.motivo.includes("entrypoint inválido")));
});

test("id ausente ou metadata ausente são ignorados sem quebrar", () => {
  const base = { schemaVersion: 1, apps: [] };
  const { catalogo } = mesclarInstalados(base, [
    { id: "", metadata: META_INSTALADO },
    { id: "x", metadata: null },
  ]);
  assert.equal(catalogo.apps.length, 0);
});

test("a ordem por título é preservada depois da junção", () => {
  const base = { schemaVersion: 1, apps: [] };
  const { catalogo } = mesclarInstalados(base, [
    { id: "zzz", metadata: { ...META_INSTALADO, slug: "zzz", title: "Zzz" } },
    { id: "aaa", metadata: { ...META_INSTALADO, slug: "aaa", title: "Aaa" } },
  ]);
  assert.deepEqual(
    catalogo.apps.map((a) => a.title),
    ["Aaa", "Zzz"],
  );
});
