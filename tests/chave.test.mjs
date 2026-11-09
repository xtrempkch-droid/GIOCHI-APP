// A resolução da chave de assinatura — e o defeito que ela existe para impedir.
//
// Medido em 07/11/2026 no R35S: o `empacotar.mjs` não achou `~/.giochi`, gerou
// um par novo **em silêncio**, e a imagem recusou o pacote com "assinatura
// Ed25519 não confere". O pacote estava íntegro; a chave é que era outra. Um
// teste que só assinasse e verificasse com a mesma chave passaria — por isso
// aqui o que se testa é **qual caminho é escolhido**.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";

const raiz = new URL("..", import.meta.url).pathname;

/**
 * Aplica o ambiente e o mantém durante o teste.
 *
 * As funções leem `HOME`/`GIOCHI_CHAVES_DIR` **na chamada**, não no import —
 * restaurar logo depois de importar testaria o ambiente real da máquina, que é
 * exatamente o que estes testes precisam isolar.
 */
function comAmbiente(home, extras = {}) {
  const salvos = {};
  const aplicar = { HOME: home, ...extras };
  for (const [k, v] of Object.entries(aplicar)) {
    salvos[k] = process.env[k];
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
  return () => {
    for (const [k, v] of Object.entries(salvos)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  };
}

async function modulo() {
  return import(`${raiz}scripts/lib/chave.mjs?v=${Math.random()}`);
}

function casa() {
  return mkdtempSync(join(tmpdir(), "giochi-chave-"));
}

test("a chave do System vence a histórica de ~/.giochi", async () => {
  const home = casa();
  const system = join(home, ".cache/giochi-system/chaves-loja");
  const giochi = join(home, ".giochi");
  mkdirSync(system, { recursive: true });
  mkdirSync(giochi, { recursive: true });
  writeFileSync(join(system, "loja-privada.pem"), "SYSTEM");
  writeFileSync(join(giochi, "chave-privada.pem"), "ANTIGA");

  const restaurar = comAmbiente(home, { GIOCHI_CHAVES_DIR: system });
  try {
    const m = await modulo();
    assert.equal(m.chavePrivadaDoDisco(), join(system, "loja-privada.pem"));
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});

test("sem a chave do System, a histórica continua servindo", async () => {
  const home = casa();
  mkdirSync(join(home, ".giochi"), { recursive: true });
  writeFileSync(join(home, ".giochi/chave-privada.pem"), "ANTIGA");

  const restaurar = comAmbiente(home, { GIOCHI_CHAVES_DIR: join(home, "nao-existe") });
  try {
    const m = await modulo();
    assert.match(m.chavePrivadaDoDisco(), /\.giochi\/chave-privada\.pem$/);
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});

test("GIOCHI_CHAVE_PRIVADA manda em tudo", async () => {
  const home = casa();
  const outra = join(home, "outra.pem");
  writeFileSync(outra, "OUTRA");
  const system = join(home, ".cache/giochi-system/chaves-loja");
  mkdirSync(system, { recursive: true });
  writeFileSync(join(system, "loja-privada.pem"), "SYSTEM");

  const restaurar = comAmbiente(home, {
    GIOCHI_CHAVES_DIR: system,
    GIOCHI_CHAVE_PRIVADA: outra,
  });
  try {
    const m = await modulo();
    assert.equal(m.chavePrivadaDoDisco(), outra);
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});

test("sem nenhuma chave, a resolução devolve null — quem chama decide", async () => {
  const home = casa();
  const restaurar = comAmbiente(home, { GIOCHI_CHAVES_DIR: join(home, "vazio") });
  try {
    const m = await modulo();
    assert.equal(m.chavePrivadaDoDisco(), null);
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});

test("a pública derivada verifica o que a privada assinou", async () => {
  // Com openssl real: assinar com a privada e verificar com a pública derivada
  // é o único jeito de provar que `publicaDe` não devolve lixo formatado.
  const home = casa();
  const priv = join(home, "loja-privada.pem");
  execFileSync("openssl", ["genpkey", "-algorithm", "ED25519", "-out", priv]);
  const restaurar = comAmbiente(home, { GIOCHI_CHAVES_DIR: join(home, "vazio") });
  try {
    const m = await modulo();
    const publica = m.publicaDe(priv);
    assert.match(publica, /BEGIN PUBLIC KEY/);
    const pacote = join(home, "x.giochi");
    writeFileSync(pacote, "conteudo");
    const sig = join(home, "x.sig");
    execFileSync("openssl", [
      "pkeyutl", "-sign", "-inkey", priv, "-rawin", "-in", pacote, "-out", sig,
    ]);
    const pub = join(home, "pub.pem");
    writeFileSync(pub, publica);
    execFileSync("openssl", [
      "pkeyutl", "-verify", "-pubin", "-inkey", pub, "-rawin", "-in", pacote, "-sigfile", sig,
    ]);
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});

test("combinaComAImagem reprova chave de outra geração e aprova a certa", async () => {
  const home = casa();
  const system = join(home, ".cache/giochi-system/chaves-loja");
  mkdirSync(system, { recursive: true });
  const daImagem = join(system, "loja-privada.pem");
  execFileSync("openssl", ["genpkey", "-algorithm", "ED25519", "-out", daImagem]);
  execFileSync("openssl", ["pkey", "-in", daImagem, "-pubout", "-out", join(system, "loja-publica.pem")]);

  const outra = join(home, "outra.pem");
  execFileSync("openssl", ["genpkey", "-algorithm", "ED25519", "-out", outra]);

  const restaurar = comAmbiente(home, { GIOCHI_CHAVES_DIR: system });
  try {
    const m = await modulo();
    assert.equal(m.combinaComAImagem(daImagem), true, "a própria chave da imagem tem de combinar");
    assert.equal(m.combinaComAImagem(outra), false, "chave de outra geração não pode passar");
  } finally {
    restaurar();
    rmSync(home, { recursive: true, force: true });
  }
});
