// Testes do formato `.giochi`: montagem, ordem, assinatura e recusa.
//
// O padrão vem do antecessor: o teste que importa não é o de ida e volta — é o de
// **adulteração**. Um verificador que nunca recusou um pacote não provou que
// verifica nada.

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  assinar,
  conferirEstrutura,
  escreverTar,
  gerarChaves,
  lerTar,
  ordenarMembros,
  slugDoPacote,
  validarPacote,
  verificar,
} from "../scripts/lib/publicar.mjs";

/** Metadata mínima válida. */
const META = {
  title: "Teste",
  developer: "giochi",
  version: "1.0.0",
  arch: "aarch64",
  runtime: "native",
  entrypoint: "bin/app",
};

const CAPA = { largura: 320, altura: 320 };

function pacoteDeExemplo({
  // `modo` explícito: o empacotador real lê a permissão do arquivo em disco.
  // Sem isto o fixture montava `bin/app` como 0644 e `conferirEstrutura`
  // reprovava — o que é o comportamento correto, e foi ele que apontou o
  // fixture errado.
  binarios = [{ caminho: "bin/app", conteudo: Buffer.from("#!/bin/sh\n"), modo: 0o755 }],
} = {}) {
  const membros = ordenarMembros({
    metadataJson: JSON.stringify(META),
    capa: Buffer.alloc(64, 1),
    arquivos: binarios,
  });
  return escreverTar(membros);
}

test("a ordem dos membros é a do contrato", () => {
  const pacote = pacoteDeExemplo({
    binarios: [
      { caminho: "bin/zzz", conteudo: Buffer.from("z"), modo: 0o644 },
      { caminho: "bin/aaa", conteudo: Buffer.from("a"), modo: 0o644 },
    ],
  });
  const ordem = [...lerTar(pacote)].map((m) => m.caminho);
  assert.deepEqual(ordem, ["metadata.json", "capa.png", "bin/", "bin/aaa", "bin/zzz"]);
});

test("dentro de bin a ordem é alfabética e estável", () => {
  // Sem isso, dois empacotamentos do mesmo conteúdo dariam SHA diferente e a
  // assinatura deixaria de identificar o conteúdo.
  const a = pacoteDeExemplo({
    binarios: [
      { caminho: "bin/b", conteudo: Buffer.from("2"), modo: 0o644 },
      { caminho: "bin/a", conteudo: Buffer.from("1"), modo: 0o644 },
    ],
  });
  const b = pacoteDeExemplo({
    binarios: [
      { caminho: "bin/a", conteudo: Buffer.from("1"), modo: 0o644 },
      { caminho: "bin/b", conteudo: Buffer.from("2"), modo: 0o644 },
    ],
  });
  assert.deepEqual(a, b, "a entrada em ordem diferente tem de dar bytes iguais");
});

test("o pacote é reprodutível byte a byte", () => {
  assert.deepEqual(pacoteDeExemplo(), pacoteDeExemplo());
});

test("bin/ entra antes dos arquivos, porque o extrator cria o diretório ao vê-lo", () => {
  const pacote = pacoteDeExemplo();
  const membros = [...lerTar(pacote)];
  const iDir = membros.findIndex((m) => m.caminho === "bin/" && m.diretorio);
  const iApp = membros.findIndex((m) => m.caminho === "bin/app");
  assert.ok(iDir >= 0, "o diretório bin/ precisa ter cabeçalho próprio");
  assert.ok(iDir < iApp, "bin/ antes de bin/app");
});

test("conferirEstrutura aceita o pacote bem formado", () => {
  const { erros, entrypoint } = conferirEstrutura(pacoteDeExemplo());
  assert.deepEqual(erros, []);
  assert.equal(entrypoint, "bin/app");
});

test("conferirEstrutura reprova metadata fora da primeira posição", () => {
  // Monta à mão na ordem errada: é o que um empacotador distraído faria.
  const pacote = escreverTar([
    { caminho: "capa.png", conteudo: Buffer.alloc(64, 1) },
    { caminho: "metadata.json", conteudo: Buffer.from(JSON.stringify(META)) },
    { caminho: "bin/", modo: 0o755 },
    { caminho: "bin/app", conteudo: Buffer.from("x"), modo: 0o755 },
  ]);
  const { erros } = conferirEstrutura(pacote);
  assert.ok(
    erros.some((e) => e.includes("precisa ser metadata.json")),
    `esperava erro de ordem, veio: ${erros.join(" | ")}`,
  );
});

test("conferirEstrutura reprova entrypoint sem bit de execução", () => {
  // O erro real: o app não abre no aparelho e a mensagem é "entrypoint não
  // existe", que não aponta para a permissão.
  const pacote = escreverTar([
    { caminho: "metadata.json", conteudo: Buffer.from(JSON.stringify(META)) },
    { caminho: "capa.png", conteudo: Buffer.alloc(64, 1) },
    { caminho: "bin/", modo: 0o755 },
    { caminho: "bin/app", conteudo: Buffer.from("x"), modo: 0o644 },
  ]);
  const { erros } = conferirEstrutura(pacote);
  assert.ok(erros.some((e) => e.includes("sem bit de execução")), erros.join(" | "));
});

test("conferirEstrutura reprova entrypoint que não está no pacote", () => {
  const meta = { ...META, entrypoint: "bin/nao-existe" };
  const pacote = escreverTar([
    { caminho: "metadata.json", conteudo: Buffer.from(JSON.stringify(meta)) },
    { caminho: "capa.png", conteudo: Buffer.alloc(64, 1) },
    { caminho: "bin/", modo: 0o755 },
    { caminho: "bin/app", conteudo: Buffer.from("x"), modo: 0o755 },
  ]);
  const { erros } = conferirEstrutura(pacote);
  assert.ok(erros.some((e) => e.includes("não está no pacote")), erros.join(" | "));
});

test("o escritor preserva o bit de execução escolhido", () => {
  const pacote = escreverTar([
    { caminho: "bin/", modo: 0o755 },
    { caminho: "bin/exec", conteudo: Buffer.from("x"), modo: 0o755 },
    { caminho: "bin/dado", conteudo: Buffer.from("x"), modo: 0o644 },
  ]);
  const m = Object.fromEntries([...lerTar(pacote)].map((x) => [x.caminho, x.modo]));
  assert.equal(m["bin/exec"] & 0o111, 0o111);
  assert.equal(m["bin/dado"] & 0o111, 0);
});

// ── Validação do que é publicável ───────────────────────────

test("metadata e capa válidos não geram erro", () => {
  const erros = validarPacote({
    metadata: META,
    capa: CAPA,
    tamanhoMetadata: 100,
    binarios: ["bin/app"],
  });
  assert.deepEqual(erros, []);
});

test("capa fora de 320x320 reprova", () => {
  const erros = validarPacote({
    metadata: META,
    capa: { largura: 512, altura: 512 },
    tamanhoMetadata: 100,
    binarios: ["bin/app"],
  });
  assert.ok(erros.some((e) => e.includes("320×320")), erros.join(" | "));
});

test("metadata acima de 16 KiB reprova", () => {
  // O limite existe porque o extrator no aparelho lê em streaming e decide com
  // o metadata na mão: um metadata gigante obrigaria a ler o pacote inteiro.
  const erros = validarPacote({
    metadata: META,
    capa: CAPA,
    tamanhoMetadata: 16 * 1024 + 1,
    binarios: ["bin/app"],
  });
  assert.ok(erros.some((e) => e.includes("excede o limite")), erros.join(" | "));
});

test("entrypoint fora de bin/ reprova", () => {
  const erros = validarPacote({
    metadata: { ...META, entrypoint: "app" },
    capa: CAPA,
    tamanhoMetadata: 100,
    binarios: ["bin/app"],
  });
  assert.ok(erros.some((e) => e.includes("precisa começar em bin/")), erros.join(" | "));
});

test("entrypoint com .. reprova", () => {
  const erros = validarPacote({
    metadata: { ...META, entrypoint: "bin/../../etc/passwd" },
    capa: CAPA,
    tamanhoMetadata: 100,
    binarios: ["bin/app"],
  });
  assert.ok(erros.some((e) => e.includes("..")), erros.join(" | "));
});

test("pacote sem binário reprova", () => {
  const erros = validarPacote({
    metadata: META,
    capa: CAPA,
    tamanhoMetadata: 100,
    binarios: [],
  });
  assert.ok(erros.some((e) => e.includes("bin/: vazia")), erros.join(" | "));
});

test("campo obrigatório ausente reprova", () => {
  const { developer, ...semDeveloper } = META;
  const erros = validarPacote({
    metadata: semDeveloper,
    capa: CAPA,
    tamanhoMetadata: 100,
    binarios: ["bin/app"],
  });
  assert.ok(erros.some((e) => e.includes("developer")), erros.join(" | "));
});

test("todos os erros saem de uma vez", () => {
  // Corrigir um por rodada é o que torna o validador inútil na prática.
  const erros = validarPacote({
    metadata: { title: "x" },
    capa: null,
    tamanhoMetadata: 99 * 1024,
    binarios: [],
  });
  assert.ok(erros.length >= 5, `esperava vários erros, veio ${erros.length}: ${erros.join(" | ")}`);
});

// ── Assinatura ──────────────────────────────────────────────

test("ida e volta: assina, verifica, aceita", () => {
  const chaves = gerarChaves();
  const pacote = pacoteDeExemplo();
  const { manifesto } = assinar(pacote, chaves.privada, { slug: "teste", version: "1.0.0" });

  const r = verificar(pacote, manifesto, chaves.publica);
  assert.equal(r.ok, true, r.erros.join(" | "));
  assert.equal(r.sha256, manifesto.sha256);
});

test("pacote adulterado é recusado", () => {
  // O teste do antecessor, que este projeto adotou como padrão: se o verificador
  // aceitar um byte a mais, ele não verifica nada.
  const chaves = gerarChaves();
  const pacote = pacoteDeExemplo();
  const { manifesto } = assinar(pacote, chaves.privada, { slug: "teste", version: "1.0.0" });

  const adulterado = Buffer.concat([pacote, Buffer.from([0])]);
  const r = verificar(adulterado, manifesto, chaves.publica);
  assert.equal(r.ok, false, "pacote com um byte a mais foi ACEITO");
  assert.ok(r.erros.some((e) => e.includes("sha256")), r.erros.join(" | "));
  assert.ok(r.erros.some((e) => e.includes("tamanho")), r.erros.join(" | "));
});

test("pacote trocado por outro válido é recusado", () => {
  // Este é o ataque que o sha256 sozinho não pega se a assinatura assinasse o
  // manifesto: trocar o pacote e manter o manifesto.
  const chaves = gerarChaves();
  const original = pacoteDeExemplo();
  const { manifesto } = assinar(original, chaves.privada, { slug: "teste", version: "1.0.0" });

  const outro = pacoteDeExemplo({
    binarios: [
      { caminho: "bin/app", conteudo: Buffer.from("#!/bin/sh\necho outro\n"), modo: 0o755 },
    ],
  });
  const r = verificar(outro, manifesto, chaves.publica);
  assert.equal(r.ok, false, "pacote diferente com o manifesto do original foi ACEITO");
});

test("assinatura feita com outra chave é recusada", () => {
  const minha = gerarChaves();
  const outra = gerarChaves();
  const pacote = pacoteDeExemplo();
  const { manifesto } = assinar(pacote, minha.privada, { slug: "teste", version: "1.0.0" });

  const r = verificar(pacote, manifesto, outra.publica);
  assert.equal(r.ok, false);
  assert.ok(r.erros.some((e) => e.includes("assinatura")), r.erros.join(" | "));
});

test("manifesto sem assinatura é recusado, não aceito por omissão", () => {
  const chaves = gerarChaves();
  const pacote = pacoteDeExemplo();
  const { manifesto } = assinar(pacote, chaves.privada, { slug: "teste", version: "1.0.0" });
  const semAssinatura = { ...manifesto, signature: "" };

  const r = verificar(pacote, semAssinatura, chaves.publica);
  assert.equal(r.ok, false);
  assert.ok(r.erros.some((e) => e.includes("ausente")), r.erros.join(" | "));
});

// ── Leitor ──────────────────────────────────────────────────

test("pacote truncado é detectado, não lido como se estivesse inteiro", () => {
  const pacote = pacoteDeExemplo();
  const truncado = pacote.subarray(0, 1200);
  assert.throws(() => [...lerTar(truncado)], /truncado|marcador de fim/);
});

test("leitor devolve os bytes exatos de cada membro", () => {
  const conteudo = Buffer.from([0, 1, 2, 250, 255, 0]);
  const pacote = escreverTar([{ caminho: "bin/x", conteudo, modo: 0o644 }]);
  const [m] = [...lerTar(pacote)];
  assert.deepEqual(m.conteudo, conteudo);
});

// ── Slug: identidade estável ────────────────────────────────

test("slug declarado no metadata vence", () => {
  const r = slugDoPacote({ slug: "pacman", title: "Pac-Man DX" }, "id-do-indice");
  assert.equal(r.slug, "pacman");
  assert.equal(r.origem, "metadata.slug");
});

test("sem slug, usa o id do apps.json", () => {
  const r = slugDoPacote({ title: "Pac-Man" }, "pacman");
  assert.equal(r.slug, "pacman");
  assert.equal(r.origem, "apps.json");
});

test("sem slug e sem id, deriva do título e declara isso", () => {
  // Último recurso: renomear o título moveria o app de lugar, então a origem
  // aparece no relatório para o publicador perceber.
  const r = slugDoPacote({ title: "Pac-Man DX" }, null);
  assert.equal(r.slug, "pac-man-dx");
  assert.match(r.origem, /título/);
});

test("título com acento e símbolo vira kebab-case", () => {
  assert.equal(slugDoPacote({ title: "Ação & Aventura!" }, null).slug, "acao-aventura");
});

test("título sem nenhum caractere útil não vira slug vazio", () => {
  // Um diretório chamado "" instalaria o app em `/opt/giochi/apps/`, em cima de
  // todos os outros.
  assert.equal(slugDoPacote({ title: "!!!" }, null).slug, "app");
  assert.equal(slugDoPacote({}, null).slug, "app");
});

test("renomear o título não muda o slug quando ele é declarado", () => {
  const a = slugDoPacote({ slug: "pacman", title: "Pac-Man" }, "pacman");
  const b = slugDoPacote({ slug: "pacman", title: "Pac-Man DX" }, "pacman");
  assert.equal(a.slug, b.slug, "o app não pode mudar de diretório ao ser renomeado");
});
