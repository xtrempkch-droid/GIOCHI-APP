// O veredito de Bluetooth do `aparelho.sh` — que ele não volte a mentir.
//
// ## Por que este teste existe
//
// Em 15/11/2026 o `aparelho.sh bluetooth` dizia "transporte: QUEBRADO" sempre que
// `hidp` estava carregado, porque exigia `/sys/bus/hid/drivers/hidp`. No kernel
// 6.12 o `hidp` é **transport driver**: ele aloca `hid_device`, chama
// `hid_parse_report`/`hid_add_device`, e quem aparece no barramento é o
// `hid-generic`. Aquele diretório nunca existiria, em kernel nenhum — a
// ferramenta relatava defeito permanente.
//
// Pior: a linha de diagnóstico usava `modinfo`, que **não existe na imagem**
// (`sh: modinfo: not found`). A saída era sempre vazia, e o remédio impresso era
// `CONFIG_HID=y` — símbolo que a imagem já tinha. O pedido foi para o canal do
// Giochi System e estava errado.
//
// Este teste é textual de propósito: o script roda no aparelho e depende de
// sysfs. O que se protege aqui é **o que ele afirma**, não o que ele mede — e a
// afirmação errada foi o defeito.
//
// ## O que cada caso protege
//
// - veredito por `hid-generic`: prova o núcleo HID pelo efeito (driver presente),
//   não pela estrutura que se esperava;
// - nada de `modinfo`: o comando não existe na imagem; usá-lo devolve vazio, e
//   vazio lido como "não tem" foi o que gerou o pedido errado;
// - "NÃO MEDIDA" no lugar de veredito: o que só um controle pareado prova não
//   pode ser afirmado sem ele;
// - `CONFIG_UHID` nomeado: é o único símbolo que faltava de verdade, e um
//   próximo leitor precisa saber disso sem refazer a análise.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fonte = readFileSync(path.join(raiz, "scripts", "aparelho.sh"), "utf8");

// O bloco do perfil HID, isolado: as asserções abaixo são sobre ele, e um
// `grep` no arquivo inteiro passaria por acidente se o texto aparecesse noutro
// comando.
function perfilHid() {
  const inicio = fonte.indexOf('titulo "perfil HID"');
  assert.notEqual(inicio, -1, 'o `titulo "perfil HID"` sumiu do aparelho.sh');
  const fim = fonte.indexOf('titulo "pareados"', inicio);
  assert.notEqual(fim, -1, 'o bloco do perfil HID não termina antes de "pareados"');
  return fonte.slice(inicio, fim);
}

test("o perfil HID prova o núcleo pelo hid-generic, não por /sys/bus/hid/drivers/hidp", () => {
  const bloco = perfilHid();

  assert.match(
    bloco,
    /\/sys\/bus\/hid\/drivers\/hid-generic/,
    "o núcleo HID tem de ser provado pelo `hid-generic`, que é o efeito (driver no barramento)",
  );
  assert.doesNotMatch(
    bloco,
    /-d \/sys\/bus\/hid\/drivers\/hidp\b/,
    "`/sys/bus/hid/drivers/hidp` nunca existe no 6.12 (hidp é transport): usá-lo como prova de defeito é o bug de 15/11/2026",
  );
});

// Sem os comentários: o bloco explica o defeito citando `modinfo`, e a citação é
// documentação. O que se proíbe é **executar** o comando.
function semComentarios(texto) {
  return texto
    .split("\n")
    .filter((linha) => !linha.trimStart().startsWith("#"))
    .join("\n");
}

test("o perfil HID não executa modinfo, que não existe na imagem", () => {
  const codigo = semComentarios(perfilHid());
  assert.doesNotMatch(
    codigo,
    /\bmodinfo\b/,
    "`modinfo` não está instalado no aparelho (`sh: modinfo: not found`): a saída sai vazia e o veredito mente",
  );
});

test("o perfil HID lê a config do kernel, que é a fonte do que foi embutido", () => {
  const bloco = perfilHid();
  assert.match(bloco, /\/proc\/config\.gz/, "a config do kernel é o único lugar que diz o que foi compilado");
  assert.match(bloco, /CONFIG_UHID|cfg UHID/, "`CONFIG_UHID` é o símbolo que faltava de verdade e tem de ser nomeado");
});

test("o perfil HID não declara transporte quebrado sem controle pareado", () => {
  const bloco = perfilHid();

  // A frase é a promessa ao usuário. "QUEBRADO" foi o veredito errado; o certo é
  // dizer que a entrega não foi medida.
  assert.doesNotMatch(
    bloco,
    /transporte: QUEBRADO/,
    "sem controle pareado não há como afirmar que o transporte está quebrado — o módulo carrega e inicializa",
  );
  assert.match(
    bloco,
    /NÃO MEDIDA/,
    'o certo é "entrega ao controle NÃO MEDIDA": o que só o controle pareado prova não se afirma sem ele',
  );
});

test("o veredito final cobra o hidp carregado, não o diretório de driver", () => {
  const inicio = fonte.indexOf('titulo "veredito"');
  assert.notEqual(inicio, -1);
  const bloco = fonte.slice(inicio, inicio + 2000);

  assert.doesNotMatch(
    bloco,
    /-d \/sys\/bus\/hid\/drivers\/hidp\b/,
    "o veredito repetia a mesma prova falsa do perfil HID",
  );
  assert.match(bloco, /-d \/sys\/module\/hidp/, "o sinal real é o módulo carregado");
});
