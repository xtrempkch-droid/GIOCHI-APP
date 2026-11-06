// A verificação de integridade do `instalar-do-cartao.sh`, exercitada de verdade.
//
// ## Por que este teste existe
//
// O script roda **no aparelho**, e a verificação de SHA-256 dele nunca tinha sido
// executada desde que foi escrita. Um script que só roda no aparelho é um script
// sem teste — e "recusa hash que não confere" era afirmação, não fato.
//
// Os caminhos são parametrizáveis (`GIOCHI_APPS_BASE`, `GIOCHI_DESTINO_BASE`,
// `GIOCHI_LAUNCHER`), então o script roda aqui com um launcher de mentira no
// lugar do binário aarch64. O que se testa é a **decisão** (aceita, recusa, e
// por quê), não o desenho — isso continua sendo trabalho de aparelho.
//
// ## O que cada caso protege
//
// - hash errado: é o erro real de quem copia pelo leitor de cartão e o arquivo
//   fica pela metade;
// - sem manifesto: o script **avisa** em vez de fingir que verificou;
// - pacote bom: o caminho feliziroso tem de continuar funcionando, senão a
//   correção vira "recusa tudo" e o usuário desliga a verificação.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { escreverTar } from "../scripts/lib/tar.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const SCRIPT = path.join(RAIZ, "scripts", "lib", "instalar-do-cartao.sh");

/** Um pacote mínimo válido, montado com o mesmo escritor do empacotador. */
function pacoteDeExemplo({ slug = "teste", binario = "#!/bin/sh\necho oi\n" } = {}) {
  const metadata = Buffer.from(
    JSON.stringify(
      {
        title: "Teste",
        slug,
        developer: "Giochi",
        version: "1.0.0",
        arch: "aarch64",
        runtime: "native",
        entrypoint: `bin/${slug}`,
      },
      null,
      2,
    ),
  );
  // `capa.png` não entra: o script não olha para ela, e um PNG inválido aqui
  // faria o teste falhar por um motivo que não é o dele.
  return escreverTar([
    { caminho: "metadata.json", conteudo: metadata, modo: 0o644 },
    { caminho: `bin/${slug}`, conteudo: Buffer.from(binario), modo: 0o755 },
  ]);
}

function sha256(buf) {
  return execFileSync("sha256sum", [], { input: buf, encoding: "utf8" }).split(" ")[0];
}

/** Monta um "aparelho" falso em disco: cartão, raiz de apps e launcher. */
function aparelhoFalso(cb) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-cartao-"));
  const cartao = path.join(dir, "cartao");
  const apps = path.join(dir, "apps");
  const destino = path.join(dir, "user-apps");
  mkdirSync(cartao);
  mkdirSync(apps, { recursive: true });
  mkdirSync(destino, { recursive: true });

  // Launcher de mentira: registra o que recebeu, para provar que o catálogo foi
  // atualizado pelo binário e não por um `sed` no JSON.
  const launcher = path.join(dir, "launcher-falso");
  const registro = path.join(dir, "catalogo.log");
  writeFileSync(
    launcher,
    `#!/bin/sh\ncase "$1" in\n  --catalogo-adicionar) echo "$2" >> ${JSON.stringify(registro)}; exit 0;;\nesac\nexit 0\n`,
  );
  chmodSync(launcher, 0o755);

  try {
    return cb({ dir, cartao, apps, destino, launcher, registro });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Roda o instalador como o aparelho rodaria: `sh script <pasta>`. */
function instalar({ cartao, apps, destino, launcher, verificador, chave }) {
  try {
    const saida = execFileSync("sh", [SCRIPT, cartao], {
      encoding: "utf8",
      stdio: "pipe",
      env: {
        ...process.env,
        GIOCHI_APPS_BASE: apps,
        GIOCHI_DESTINO_BASE: destino,
        GIOCHI_LAUNCHER: launcher,
        ...(verificador ? { GIOCHI_VERIFICADOR: verificador } : {}),
        ...(chave ? { GIOCHI_CHAVE_PUBLICA: chave } : {}),
      },
    });
    return { codigo: 0, saida };
  } catch (e) {
    return { codigo: e.status ?? 1, saida: `${e.stdout ?? ""}${e.stderr ?? ""}` };
  }
}

function copiarPacote(cartao, nome, pacote, { manifesto = true, assinatura = null } = {}) {
  const arquivo = path.join(cartao, nome);
  writeFileSync(arquivo, pacote);
  if (manifesto) {
    writeFileSync(
      `${arquivo}.manifest.json`,
      JSON.stringify(
        { schema_version: 1, package: nome, version: "1.0.0", sha256: sha256(pacote), size_bytes: pacote.length },
        null,
        2,
      ),
    );
  }
  if (assinatura) writeFileSync(`${arquivo}.sig`, assinatura);
  return arquivo;
}

test("pacote íntegro é instalado e o catálogo é atualizado", () => {
  aparelhoFalso(({ cartao, apps, destino, launcher, registro }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo());
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    assert.equal(codigo, 0, saida);
    assert.match(saida, /teste\.giochi: sha256 confere/);
    assert.match(saida, /teste\.giochi: instalado em/);
    assert.match(saida, /RESUMO 1 instalado\(s\), 0 recusado\(s\)/);
    assert.ok(existsSync(path.join(destino, "teste", "metadata.json")), "app não foi extraído");
    // O binário precisa chegar executável: o extrator reforça o modo, porque
    // nem todo `tar` de BusyBox aplica o bit.
    assert.ok(existsSync(path.join(destino, "teste", "bin", "teste")));
    assert.match(readFileSync(registro, "utf8"), /teste/);
  });
});

test("pacote corrompido é RECUSADO e não aparece em /user/apps", () => {
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    const bom = pacoteDeExemplo();
    const arquivo = copiarPacote(cartao, "teste.giochi", bom);
    // Adultera um byte **depois** de o manifesto ter sido calculado sobre o
    // original: é exatamente o que uma cópia interrompida produz.
    writeFileSync(arquivo, Buffer.concat([bom, Buffer.from([0])]));

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    assert.equal(codigo, 1, "saiu com 0 apesar de haver pacote recusado");
    assert.match(saida, /RECUSADO — sha256 não confere/);
    assert.match(saida, /RESUMO 0 instalado\(s\), 1 recusado\(s\)/);
    assert.ok(!existsSync(path.join(destino, "teste")), "pacote recusado foi para /user/apps");
  });
});

test("sem manifesto o script AVISA em vez de fingir que verificou", () => {
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo(), { manifesto: false });
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    // Instala (o `.giochi` sozinho é o formato documentado), mas **diz** que a
    // integridade não foi verificada. Silêncio aqui seria a mentira fácil.
    assert.equal(codigo, 0, saida);
    assert.match(saida, /aviso — sem manifest\.json ao lado, integridade não verificada/);
  });
});

test("manifesto sem sha256 avisa e não compara com vazio", () => {
  // Um manifesto com `sha256` ausente deixaria `ESPERADO` vazio; comparar com
  // vazio recusaria tudo, ou pior, passaria. O script trata o caso à parte.
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    const pacote = pacoteDeExemplo();
    const arquivo = copiarPacote(cartao, "teste.giochi", pacote, { manifesto: false });
    writeFileSync(`${arquivo}.manifest.json`, JSON.stringify({ schema_version: 1, version: "1.0.0" }));

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });
    assert.match(saida, /aviso — manifesto sem sha256/);
    assert.equal(codigo, 0, saida);
  });
});

test("pacote sem metadata.json é recusado, não instalado pela metade", () => {
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    const semMetadata = escreverTar([
      { caminho: "bin/teste", conteudo: Buffer.from("#!/bin/sh\n"), modo: 0o755 },
    ]);
    copiarPacote(cartao, "quebrado.giochi", semMetadata);
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    assert.equal(codigo, 1, saida);
    assert.match(saida, /RECUSADO — entrypoint ausente ou fora do pacote/);
  });
});

test("o slug vem do metadata, não do nome do arquivo", () => {
  // Renomear o `.giochi` não pode mudar onde o app é instalado, senão o
  // catálogo e o favorito do usuário apontam para pastas diferentes.
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "nome-trocado.giochi", pacoteDeExemplo({ slug: "nome-real" }));
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    assert.equal(codigo, 0, saida);
    assert.ok(existsSync(path.join(destino, "nome-real")), "usou o nome do arquivo");
    assert.ok(!existsSync(path.join(destino, "nome-trocado")), "criou pasta com o nome do arquivo");
  });
});

test("pasta sem nenhum .giochi devolve zero sem erro", () => {
  // O glob sem casamento devolve o próprio padrão; se o script não tratar, ele
  // tentaria instalar `*.giochi` como arquivo.
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });
    assert.equal(codigo, 0, saida);
    assert.match(saida, /RESUMO 0 instalado\(s\), 0 recusado\(s\), 0 encontrado\(s\)/);
    assert.equal(readdirSync(destino).length, 0);
  });
});

test("dois pacotes: um bom e um corrompido, cada um com o seu veredito", () => {
  // A primeira versão de qualquer lote quebra no primeiro item e esquece o
  // resto. Três pacotes provam que o laço continua.
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "a.giochi", pacoteDeExemplo({ slug: "a" }));
    const ruim = pacoteDeExemplo({ slug: "b" });
    const arquivo = copiarPacote(cartao, "b.giochi", ruim);
    writeFileSync(arquivo, Buffer.concat([ruim, Buffer.from([1])]));
    copiarPacote(cartao, "c.giochi", pacoteDeExemplo({ slug: "c" }));

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });

    assert.equal(codigo, 1, saida);
    assert.match(saida, /RESUMO 2 instalado\(s\), 1 recusado\(s\), 3 encontrado\(s\)/);
    assert.ok(existsSync(path.join(destino, "a")));
    assert.ok(!existsSync(path.join(destino, "b")));
    assert.ok(existsSync(path.join(destino, "c")), "o laço parou no pacote recusado");
  });
});

// ── Assinatura ─────────────────────────────────────────────
//
// O verificador de verdade é um binário aarch64 ligado à `libcrypto` do
// aparelho (ver `scripts/lib/verificador-ed25519.c`). Ele foi exercitado no X55
// com seis casos. Aqui o que se testa é a **decisão do instalador** — recusar,
// aceitar, ou avisar que não verificou — usando um verificador de mentira que
// devolve o código escolhido. Assim este teste roda em qualquer PC, sem o
// binário cruzado, e continua provando o que importa.

/** Um verificador falso que devolve sempre o mesmo código. */
function verificadorFalso(dir, codigo) {
  const caminho = path.join(dir, `verificador-${codigo}`);
  writeFileSync(caminho, `#!/bin/sh\nexit ${codigo}\n`);
  chmodSync(caminho, 0o755);
  return caminho;
}

test("assinatura que confere: instala", () => {
  aparelhoFalso(({ dir, cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo(), { assinatura: Buffer.alloc(64, 7) });
    const verificador = verificadorFalso(dir, 0);
    const chave = path.join(dir, "chave.pem");
    writeFileSync(chave, "-----BEGIN PUBLIC KEY-----\n");

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher, verificador, chave });
    assert.equal(codigo, 0, saida);
    // E DIZ que conferiu: silêncio no sucesso tornaria "verificou" e "não
    // verificou" indistinguíveis na saída.
    assert.match(saida, /assinatura confere/);
    assert.match(saida, /RESUMO 1 instalado\(s\), 0 recusado\(s\)/);
  });
});

test("assinatura que NÃO confere: recusa e não extrai", () => {
  // O caso que justifica tudo isto: pacote com hash certo e assinatura errada.
  // Sem esta recusa, um cartão com o app trocado passaria — o SHA-256 do
  // manifesto pode ser reescrito por quem escreve no cartão.
  aparelhoFalso(({ dir, cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo(), { assinatura: Buffer.alloc(64, 7) });
    const verificador = verificadorFalso(dir, 1);
    const chave = path.join(dir, "chave.pem");
    writeFileSync(chave, "x");

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher, verificador, chave });
    assert.equal(codigo, 1, saida);
    assert.match(saida, /RECUSADO — assinatura não confere/);
    assert.ok(!existsSync(path.join(destino, "teste")), "pacote com assinatura inválida foi extraído");
  });
});

test("sem assinatura ao lado, mas com chave: recusa", () => {
  // Havendo chave, a ausência da assinatura é omissão, não "formato simples".
  aparelhoFalso(({ dir, cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo());
    const verificador = verificadorFalso(dir, 0);
    const chave = path.join(dir, "chave.pem");
    writeFileSync(chave, "x");

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher, verificador, chave });
    assert.equal(codigo, 1, saida);
    assert.match(saida, /RECUSADO — sem assinatura ao lado/);
  });
});

test("erro do verificador (chave errada) AVISA e instala, não recusa", () => {
  // Código 2/3 é problema de configuração da imagem, não veredito sobre o jogo.
  // Recusar aqui pararia a instalação inteira por um motivo que não é do pacote.
  aparelhoFalso(({ dir, cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo(), { assinatura: Buffer.alloc(64, 7) });
    const verificador = verificadorFalso(dir, 3);
    const chave = path.join(dir, "chave.pem");
    writeFileSync(chave, "x");

    const { codigo, saida } = instalar({ cartao, apps, destino, launcher, verificador, chave });
    assert.equal(codigo, 0, saida);
    assert.match(saida, /aviso — assinatura não verificada/);
  });
});

test("sem verificador, o script DIZ que não verificou", () => {
  // Silêncio aqui seria a mentira fácil: o usuário acharia que foi conferido.
  aparelhoFalso(({ cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo());
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher });
    assert.equal(codigo, 0, saida);
    assert.match(saida, /aviso — sem verificador, assinatura não verificada/);
  });
});

test("com verificador e sem chave, avisa que faltou a chave", () => {
  aparelhoFalso(({ dir, cartao, apps, destino, launcher }) => {
    copiarPacote(cartao, "teste.giochi", pacoteDeExemplo());
    const verificador = verificadorFalso(dir, 0);
    const { codigo, saida } = instalar({ cartao, apps, destino, launcher, verificador });
    assert.equal(codigo, 0, saida);
    assert.match(saida, /aviso — sem chave em .*, assinatura não verificada/);
  });
});
