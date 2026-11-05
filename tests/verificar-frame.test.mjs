#!/usr/bin/env node
// Testes do verificador de quadro (scripts/verificar-frame.mjs).
//
// O que estes testes protegem: a verificação existe para não aceitar um quadro
// preto ou girado como se fosse a UI. Se a leitura de BMP ou a matemática da
// rotação estiverem erradas, ela aprova lixo — e aí não serve para nada.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  analisar,
  comparar,
  correlacaoDeTexto,
  desRotacionar,
  gravarPng,
  lerBmp,
  medirFaixas,
} from "../scripts/verificar-frame.mjs";

const silencio = () => {};

function withTempDir(run) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-frame-"));
  try {
    return run(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * BMP 32 bpp sintético, montado linha a linha por `cor(x, y)`.
 *
 * `deBaixoParaCima` reproduz o caso real do SDL: BMP com altura positiva é
 * gravado de baixo para cima. Ler ao contrário inverte a imagem e a comparação
 * passaria a comparar o quadro com ele mesmo de cabeça para baixo.
 */
function bmpSintetico(largura, altura, cor, { deBaixoParaCima = true, bpp = 32 } = {}) {
  const bytesPorPixel = bpp / 8;
  const tamanhoLinha = Math.floor((bpp * largura + 31) / 32) * 4;
  const deslocamento = 54;
  const buffer = Buffer.alloc(deslocamento + tamanhoLinha * altura);

  buffer.write("BM", 0, "ascii");
  buffer.writeUInt32LE(buffer.length, 2);
  buffer.writeUInt32LE(deslocamento, 10);
  buffer.writeUInt32LE(40, 14);
  buffer.writeInt32LE(largura, 18);
  buffer.writeInt32LE(deBaixoParaCima ? altura : -altura, 22);
  buffer.writeUInt16LE(1, 26);
  buffer.writeUInt16LE(bpp, 28);

  for (let y = 0; y < altura; y++) {
    const linha = deslocamento + (deBaixoParaCima ? altura - 1 - y : y) * tamanhoLinha;
    for (let x = 0; x < largura; x++) {
      const [r, g, b] = cor(x, y);
      const p = linha + x * bytesPorPixel;
      buffer[p] = b;
      buffer[p + 1] = g;
      buffer[p + 2] = r;
      if (bytesPorPixel === 4) buffer[p + 3] = 255;
    }
  }
  return buffer;
}

test("lerBmp respeita BMP de baixo para cima", () => {
  withTempDir((dir) => {
    // Faixa vermelha só no topo. Se a leitura invertesse, o vermelho apareceria
    // embaixo e este teste passaria por engano.
    const caminho = path.join(dir, "baixo-para-cima.bmp");
    writeFileSync(
      caminho,
      bmpSintetico(4, 4, (_x, y) => (y === 0 ? [255, 0, 0] : [0, 0, 255])),
    );
    const img = lerBmp(caminho);
    assert.equal(img.largura, 4);
    assert.equal(img.altura, 4);
    assert.deepEqual(img.em(0, 0), [255, 0, 0]);
    assert.deepEqual(img.em(0, 3), [0, 0, 255]);
  });
});

test("lerBmp aceita BMP de cima para baixo (altura negativa)", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "cima-para-baixo.bmp");
    writeFileSync(
      caminho,
      bmpSintetico(4, 4, (_x, y) => (y === 0 ? [0, 255, 0] : [10, 10, 10]), {
        deBaixoParaCima: false,
      }),
    );
    assert.deepEqual(lerBmp(caminho).em(0, 0), [0, 255, 0]);
  });
});

test("lerBmp recusa arquivo que não é BMP", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "falso.bmp");
    writeFileSync(caminho, "isto não é um bitmap");
    assert.throws(() => lerBmp(caminho), /não é BMP/);
  });
});

test("lerBmp recusa 8 bpp em vez de adivinhar a paleta", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "paleta.bmp");
    const buffer = bmpSintetico(2, 2, () => [0, 0, 0]);
    buffer.writeUInt16LE(8, 28);
    writeFileSync(caminho, buffer);
    assert.throws(() => lerBmp(caminho), /esperava 24 ou 32 bpp/);
  });
});

test("analisar acusa quadro de uma só cor", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "preto.bmp");
    writeFileSync(caminho, bmpSintetico(64, 64, () => [0, 0, 0]));
    const r = analisar(lerBmp(caminho), silencio);
    assert.equal(r.cores, 1);
    assert.equal(r.pctPreto, 100);
  });
});

test("analisar conta cores distintas e ignora preto quase-puro", () => {
  withTempDir((dir) => {
    // A análise amostra de 4 em 4 pixels: em 64 de largura são 16 colunas
    // amostradas, cada uma com um vermelho diferente.
    const caminho = path.join(dir, "cores.bmp");
    writeFileSync(
      caminho,
      bmpSintetico(64, 64, (x) => [x * 4, 20, 30]),
    );
    const r = analisar(lerBmp(caminho), silencio);
    assert.equal(r.cores, 16);
    assert.equal(r.pctPreto, 0);
  });
});

test("desRotacionar em 'none' não muda a leitura", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "sem-giro.bmp");
    writeFileSync(caminho, bmpSintetico(8, 4, (x) => [x * 30, 0, 0]));
    const img = lerBmp(caminho);
    const saida = desRotacionar(img, "none");
    assert.equal(saida.largura, 8);
    assert.equal(saida.altura, 4);
    assert.deepEqual(saida.em(3, 1), img.em(3, 1));
  });
});

test("desRotacionar ccw devolve a paisagem a partir de retrato", () => {
  withTempDir((dir) => {
    // Espelha `rotate_point` do Rust: paisagem (x,y) -> retrato (W-1-y, x)
    // para Ccw90, com W = largura da paisagem (8). Retrato é 4x8.
    const caminho = path.join(dir, "retrato.bmp");
    const paisagem = (x, y) => [x * 10, y * 10, 0];
    writeFileSync(
      caminho,
      bmpSintetico(4, 8, (dx, dy) => paisagem(8 - 1 - dy, dx)),
    );
    const saida = desRotacionar(lerBmp(caminho), "ccw");
    assert.equal(saida.largura, 8);
    assert.equal(saida.altura, 4);
    for (const [x, y] of [
      [0, 0],
      [7, 0],
      [0, 3],
      [7, 3],
      [3, 2],
    ]) {
      assert.deepEqual(saida.em(x, y), paisagem(x, y), `ponto ${x},${y}`);
    }
  });
});

test("desRotacionar cw devolve a paisagem a partir de retrato", () => {
  withTempDir((dir) => {
    // Rotação oposta: paisagem (x,y) -> retrato (H-1-y, x), H = 4.
    const caminho = path.join(dir, "retrato-cw.bmp");
    const paisagem = (x, y) => [x * 10, y * 10, 0];
    writeFileSync(
      caminho,
      bmpSintetico(4, 8, (dx, dy) => paisagem(dy, 4 - 1 - dx)),
    );
    const saida = desRotacionar(lerBmp(caminho), "cw");
    assert.equal(saida.largura, 8);
    assert.equal(saida.altura, 4);
    for (const [x, y] of [
      [0, 0],
      [7, 0],
      [0, 3],
      [7, 3],
      [3, 2],
    ]) {
      assert.deepEqual(saida.em(x, y), paisagem(x, y), `ponto ${x},${y}`);
    }
  });
});

test("desRotacionar recusa modo desconhecido", () => {
  const img = { largura: 2, altura: 2, em: () => [0, 0, 0] };
  assert.throws(() => desRotacionar(img, "de-cabeca"), /rotação desconhecida/);
});

test("comparar acusa 100% quando as imagens são iguais", () => {
  withTempDir((dir) => {
    const cor = (x, y) => [x * 3, y * 5, 40];
    const a = path.join(dir, "a.bmp");
    const b = path.join(dir, "b.bmp");
    writeFileSync(a, bmpSintetico(32, 32, cor));
    writeFileSync(b, bmpSintetico(32, 32, cor));
    const r = comparar(lerBmp(a), lerBmp(b), "none", 12, silencio);
    assert.equal(r.pctAcerto, 100);
  });
});

test("comparar acusa rotação errada: cw e ccw diferem 180°", () => {
  withTempDir((dir) => {
    // Este é o bug real: os dois sentidos giram 90°, então o conteúdo aparece,
    // mas de cabeça para baixo. Sem esta verificação ele passa como "ok".
    // Paisagem 16x8; Ccw90 leva (x,y) a (16-1-y, x), então o retrato é 8x16.
    const paisagem = (x, y) => [x * 8, y * 8, (x + y) * 4];
    const retratoCcw = path.join(dir, "retrato.bmp");
    writeFileSync(retratoCcw, bmpSintetico(8, 16, (dx, dy) => paisagem(15 - dy, dx)));
    const referencia = path.join(dir, "ref.bmp");
    writeFileSync(referencia, bmpSintetico(16, 8, paisagem));

    const certo = comparar(lerBmp(retratoCcw), lerBmp(referencia), "ccw", 12, silencio);
    const errado = comparar(lerBmp(retratoCcw), lerBmp(referencia), "cw", 12, silencio);

    assert.ok(certo.pctAcerto > 99, `esperava acerto alto, veio ${certo.pctAcerto}`);
    assert.ok(errado.pctAcerto < 50, `esperava acerto baixo, veio ${errado.pctAcerto}`);
  });
});

test("comparar devolve 0 quando as dimensões não batem", () => {
  withTempDir((dir) => {
    const a = path.join(dir, "a.bmp");
    const b = path.join(dir, "b.bmp");
    writeFileSync(a, bmpSintetico(8, 4, () => [1, 2, 3]));
    writeFileSync(b, bmpSintetico(16, 4, () => [1, 2, 3]));
    assert.equal(comparar(lerBmp(a), lerBmp(b), "none", 12, silencio).pctAcerto, 0);
  });
});

test("gravarPng produz PNG válido com as cores de entrada", () => {
  withTempDir((dir) => {
    const saida = path.join(dir, "saida.png");
    const bytes = gravarPng(saida, 3, 2, (x) => [x * 100, 10, 20]);
    const lido = readFileSync(saida);
    assert.equal(lido.length, bytes);
    assert.deepEqual(
      [...lido.subarray(0, 8)],
      [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    );
    // Confere que o leitor de PNG do validador entende o arquivo gerado.
    assert.equal(lido.readUInt32BE(16), 3);
    assert.equal(lido.readUInt32BE(20), 2);
  });
});

test("medirFaixas mede letterbox de 4:3 em projeto 16:9", () => {
  withTempDir((dir) => {
    // 640x480 com conteúdo de 360 linhas: o resto é preto. É o caso do X35H.
    const caminho = path.join(dir, "letterbox.bmp");
    writeFileSync(
      caminho,
      bmpSintetico(64, 48, (_x, y) => (y < 36 ? [15, 31, 58] : [0, 0, 0])),
    );
    const r = medirFaixas(lerBmp(caminho));
    // Conteúdo no topo, preto embaixo — o artefato que o `read_pixels` produz.
    assert.equal(r.topo, 0);
    assert.equal(r.base, 12);
    assert.equal(Math.round(r.pct), 25);
  });
});

test("medirFaixas dá zero quando não há faixa", () => {
  withTempDir((dir) => {
    const caminho = path.join(dir, "cheio.bmp");
    writeFileSync(caminho, bmpSintetico(32, 32, (x) => [x * 8, 20, 30]));
    const r = medirFaixas(lerBmp(caminho));
    assert.equal(r.topo, 0);
    assert.equal(r.base, 0);
    assert.equal(r.pct, 0);
  });
});

// Orientação de texto: a medida que pega um bug que percentual não pega.
//
// O launcher desenhou a interface deitada certa e os glifos a 90° dela. A
// comparação pixel a pixel fechava em 98,9% — texto é pouca área. O que
// denuncia é o perfil: texto deitado tem picos nas LINHAS, girado, nas COLUNAS.
test("correlação de texto separa deitado de girado", () => {
  const W = 120;
  const H = 120;
  // Três faixas claras. `deitado` = horizontais; `girado` = as mesmas, a 90°.
  const faixas = (vertical) => {
    const tela = [];
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const i = vertical ? x : y;
        const claro = i % 20 < 8;
        tela.push(claro ? [255, 255, 255] : [20, 20, 20]);
      }
    }
    return { largura: W, altura: H, em: (x, y) => tela[y * W + x] };
  };

  const deitado = faixas(false);
  const girado = faixas(true);

  const igual = correlacaoDeTexto(deitado, deitado);
  assert.ok(igual.porLinha > 0.99, `idêntico deveria dar ~1, deu ${igual.porLinha}`);

  const virado = correlacaoDeTexto(girado, deitado);
  assert.ok(
    virado.porColuna > virado.porLinha,
    `girado 90° deveria preferir colunas (linhas ${virado.porLinha.toFixed(2)}, ` +
      `colunas ${virado.porColuna.toFixed(2)})`
  );
});
