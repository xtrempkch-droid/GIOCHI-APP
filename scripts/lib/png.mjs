// Leitura de dimensões de PNG sem dependência externa.
//
// O pacote .giochi exige capa.png de exatamente 320x320. Sem checar isso no
// build, a imagem só é descoberta quando aparece esticada na loja — e a essa
// altura o app já foi publicado.
//
// PNG: assinatura de 8 bytes, depois o chunk IHDR, cujos primeiros campos são
// largura e altura em big-endian de 4 bytes.
import { readFileSync } from "node:fs";

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/**
 * Devolve { width, height } de um PNG, ou null se o arquivo não for PNG válido.
 */
export function readPngSize(filePath) {
  let buffer;
  try {
    buffer = readFileSync(filePath);
  } catch {
    return null;
  }

  if (buffer.length < 24) return null;
  if (!buffer.subarray(0, 8).equals(SIGNATURE)) return null;
  if (buffer.toString("ascii", 12, 16) !== "IHDR") return null;

  const width = buffer.readUInt32BE(16);
  const height = buffer.readUInt32BE(20);

  if (width === 0 || height === 0) return null;
  return { width, height };
}

export function isPng(filePath) {
  return readPngSize(filePath) !== null;
}
