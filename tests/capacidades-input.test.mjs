#!/usr/bin/env node
// Testes do decodificador de capacidades (scripts/lib/capacidades.mjs).
//
// As fixtures são a saída **real** de `/proc/bus/input/devices` do X55. Um
// decoder testado contra dados inventados só prova que ele concorda consigo
// mesmo; estes números foram conferidos contra o comportamento do hardware.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decodificarBitmask,
  nomeDaTecla,
  parseDispositivos,
  porCodigo,
  quemEmite,
  rotulo,
} from "../scripts/lib/capacidades.mjs";

/** Trecho literal de /proc/bus/input/devices no X55. */
const DEVICES_X55 = `I: Bus=0019 Vendor=0000 Product=0000 Version=0000
N: Name="rk805 pwrkey"
P: Phys=rk805_pwrkey/input0
S: Sysfs=/devices/platform/fdd40000.i2c/i2c-0/0-0020/rk805-pwrkey.2.auto/input/input0
U: Uniq=
H: Handlers=kbd event0 
B: PROP=0
B: EV=3
B: KEY=10000000000000 0

I: Bus=0019 Vendor=0000 Product=0000 Version=0000
N: Name="gpio-keys-control"
H: Handlers=event3 
B: PROP=0
B: EV=3
B: KEY=f00000000 0 0 0 7fdb000000000000 0 0 0 0

I: Bus=0019 Vendor=0000 Product=0000 Version=0000
N: Name="gpio-keys-vol"
H: Handlers=event4 
B: PROP=0
B: EV=3
B: KEY=c000000000000 0
`;

test("decodificarBitmask trata a última palavra como a de menor peso", () => {
  // O erro clássico: ler `10000000000000 0` da esquerda para a direita faria
  // da primeira palavra a de peso 0 → código 52, não 116.
  const codigos = decodificarBitmask(["10000000000000", "0"]);
  assert.deepEqual([...codigos], [116]);
  assert.equal(nomeDaTecla(116), "KEY_POWER");
});

test("decodificarBitmask devolve vazio para bitmask toda zerada", () => {
  assert.equal(decodificarBitmask(["0", "0", "0"]).size, 0);
});

test("decodificarBitmask ignora palavras vazias sem deslocar o peso", () => {
  const codigos = decodificarBitmask(["10000000000000", ""]);
  assert.deepEqual([...codigos], [116]);
});

test("teclas de volume do X55 saem como 114 e 115", () => {
  const codigos = [...decodificarBitmask(["c000000000000", "0"])].sort((a, b) => a - b);
  assert.deepEqual(codigos, [114, 115]);
  assert.equal(nomeDaTecla(114), "KEY_VOLUMEDOWN");
  assert.equal(nomeDaTecla(115), "KEY_VOLUMEUP");
});

test("parseDispositivos lê nome, handlers e teclas do aparelho", () => {
  const d = parseDispositivos(DEVICES_X55);
  assert.equal(d.length, 3);

  const pwr = d.find((x) => x.nome === "rk805 pwrkey");
  assert.deepEqual(pwr.handlers, ["kbd", "event0"]);
  assert.deepEqual([...pwr.key], [116]);

  const controle = d.find((x) => x.nome === "gpio-keys-control");
  assert.deepEqual(controle.handlers, ["event3"]);
  // 17 códigos, que são os 17 botões citados no próprio `buttons.map`: 13 do
  // gamepad (A, B, X, Y, TL, TR, TL2, TR2, SELECT, START, MODE, THUMBL, THUMBR)
  // mais os 4 do direcional, que ficam na faixa BTN_ 0x220 e não na faixa KEY_.
  for (const codigo of [304, 305, 307, 308, 310, 311, 312, 313, 314, 315, 316, 317, 318]) {
    assert.ok(controle.key.has(codigo), `faltou ${codigo}`);
  }
  assert.ok(!controle.key.has(309), "309 não está no bitmask");
  assert.equal(controle.key.size, 17);
  for (const codigo of [544, 545, 546, 547]) {
    assert.ok(controle.key.has(codigo), `faltou ${codigo}`);
  }
  assert.equal(nomeDaTecla(544), "BTN_DPAD_UP");
  assert.equal(nomeDaTecla(547), "BTN_DPAD_RIGHT");

  const vol = d.find((x) => x.nome === "gpio-keys-vol");
  assert.deepEqual([...vol.key].sort((a, b) => a - b), [114, 115]);
});

test("parseDispositivos não inventa teclas para quem não declara KEY", () => {
  const d = parseDispositivos(`N: Name="adc-joystick"
H: Handlers=event1
B: EV=1b
B: ABS=3067b0000 0
`);
  assert.equal(d.length, 1);
  assert.equal(d[0].key.size, 0);
});

test("parseDispositivos remove as aspas do nome", () => {
  const d = parseDispositivos('N: Name="rk817_ext Headphones"\n');
  assert.equal(d[0].nome, "rk817_ext Headphones");
});

test("quemEmite responde com o eventN correto", () => {
  const d = parseDispositivos(DEVICES_X55);
  assert.deepEqual(quemEmite(d, 116), [{ nome: "rk805 pwrkey", eventos: ["event0"] }]);
  assert.deepEqual(quemEmite(d, 114), [{ nome: "gpio-keys-vol", eventos: ["event4"] }]);
  assert.deepEqual(quemEmite(d, 9999), []);
});

test("quemEmite não quebra com dispositivo sem eventN", () => {
  const d = parseDispositivos('N: Name="teclado"\nH: Handlers=kbd\nB: KEY=1 0\n');
  assert.deepEqual(quemEmite(d, 64), [{ nome: "teclado", eventos: [] }]);
});

test("porCodigo ordena os códigos e agrupa por dispositivo", () => {
  const d = parseDispositivos(DEVICES_X55);
  const mapa = porCodigo(d);
  const codigos = [...mapa.keys()];
  assert.deepEqual(codigos, [...codigos].sort((a, b) => a - b));
  assert.deepEqual(mapa.get(116), ["rk805 pwrkey"]);
  assert.deepEqual(mapa.get(304), ["gpio-keys-control"]);
});

test("rotulo mostra nome quando conhecido e só o número quando não", () => {
  assert.equal(rotulo(116), "116 KEY_POWER");
  assert.equal(rotulo(600), "600");
  assert.equal(nomeDaTecla(600), null);
});

test("BTN_SOUTH sai com o rótulo do botão A", () => {
  // Os botões do X55 são BTN_* padrão, não KEY_* — o mapa do app depende disso.
  assert.equal(nomeDaTecla(304), "BTN_SOUTH (A)");
  assert.equal(nomeDaTecla(315), "BTN_START");
});

test("parseDispositivos com entrada vazia devolve lista vazia", () => {
  assert.deepEqual(parseDispositivos(""), []);
});
