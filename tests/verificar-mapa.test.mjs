// Testes de `scripts/verificar-mapa.mjs`.
//
// O que se está protegendo aqui: um símbolo desconhecido no `buttons.map` vira
// botão morto **em silêncio** no aparelho. Já aconteceu duas vezes (comentário no
// fim da linha e `KEY_VOLUMEUP` fora do tradutor). Este script é a única forma de
// descobrir isso **antes de haver hardware** — foi assim que o mapa do R35S foi
// verificado sem o aparelho na mão.

import { test } from "node:test";
import assert from "node:assert/strict";

import { lerMapa, comparar } from "../scripts/verificar-mapa.mjs";

const CONHECIDOS = new Set([
  "BTN_DPAD_UP",
  "BTN_DPAD_DOWN",
  "BTN_EAST",
  "BTN_SOUTH",
  "BTN_TL",
  "BTN_TL2",
  "BTN_MODE",
  "KEY_VOLUMEUP",
  "KEY_POWER",
]);

test("corta o comentário do fim da linha", () => {
  // O bug real: `up = BTN_DPAD_UP  # gpio4 PA0` deixava 1 de 13 botões.
  const { botoes } = lerMapa("up = BTN_DPAD_UP  # gpio4 PA0\n");
  assert.equal(botoes.length, 1);
  assert.equal(botoes[0].simbolo, "BTN_DPAD_UP");
  assert.equal(botoes[0].nome, "up");
});

test("linha comentada inteira não vira botão", () => {
  const { botoes } = lerMapa("# up = BTN_DPAD_UP\n");
  assert.equal(botoes.length, 0);
});

test("cabeçalho e separador sem `=` são ignorados", () => {
  const texto = [
    "# Mapa de botões",
    "",
    "# ── botões ──────────────────────",
    "a = BTN_EAST",
  ].join("\n");
  const { botoes } = lerMapa(texto);
  assert.equal(botoes.length, 1);
  assert.equal(botoes[0].simbolo, "BTN_EAST");
});

test("ignora espaçamento irregular e maiúsculas no nome", () => {
  const { botoes } = lerMapa("  UP   =   BTN_DPAD_UP   \n");
  assert.equal(botoes[0].nome, "up");
  assert.equal(botoes[0].simbolo, "BTN_DPAD_UP");
});

test("separa calibragem `axis_*` dos botões", () => {
  // As chaves `axis_*` já apareceram como "símbolo desconhecido" no log do app.
  const { botoes, calibragem } = lerMapa(
    "up = BTN_DPAD_UP\naxis_x_min = 15\naxis_x_max = 1023\n",
  );
  assert.equal(botoes.length, 1);
  assert.equal(calibragem.length, 2);
  assert.equal(calibragem[0].nome, "axis_x_min");
  assert.equal(calibragem[0].valor, "15");
});

test("guarda o número da linha para o relatório apontar o lugar", () => {
  const { botoes } = lerMapa("# cabeçalho\na = BTN_EAST\n");
  assert.equal(botoes[0].linha, 2);
});

test("símbolo conhecido não é reportado como morto", () => {
  const { botoes } = lerMapa("up = BTN_DPAD_UP\na = BTN_EAST\n");
  const { mortos } = comparar(botoes, CONHECIDOS);
  assert.equal(mortos.length, 0);
});

test("símbolo inventado é reportado como morto", () => {
  const { botoes } = lerMapa("turbo = BTN_TURBO_FICTICIO\n");
  const { mortos } = comparar(botoes, CONHECIDOS);
  assert.equal(mortos.length, 1);
  assert.equal(mortos[0].simbolo, "BTN_TURBO_FICTICIO");
});

test("`KEY_VOLUMEUP` só passa porque está no tradutor", () => {
  // O caso histórico: o mapa declarava e o app descartava.
  const semVolume = new Set([...CONHECIDOS].filter((s) => !s.startsWith("KEY_VOLUME")));
  const { botoes } = lerMapa("volup = KEY_VOLUMEUP\n");
  assert.equal(comparar(botoes, CONHECIDOS).mortos.length, 0);
  assert.equal(comparar(botoes, semVolume).mortos.length, 1);
});

test("lista os símbolos que o mapa não usa, sem reprovar", () => {
  // Ausência pode ser só hardware que não existe (o R35S não tem BT).
  const { botoes } = lerMapa("a = BTN_EAST\n");
  const { mortos, naoUsados } = comparar(botoes, CONHECIDOS);
  assert.equal(mortos.length, 0);
  assert.ok(naoUsados.includes("BTN_MODE"));
  assert.ok(!naoUsados.includes("BTN_EAST"));
});

test("mapa vazio não quebra", () => {
  const { botoes, calibragem } = lerMapa("");
  assert.equal(botoes.length, 0);
  assert.equal(calibragem.length, 0);
  assert.equal(comparar(botoes, CONHECIDOS).mortos.length, 0);
});
