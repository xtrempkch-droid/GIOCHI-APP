// ZIP determinístico — é o formato do `.pyz` (zipapp) dos apps de PC.
//
// ## Por que escrever o ZIP à mão, e não `python3 -m zipapp`
//
// Mesma razão do `tar.mjs`: **reprodutibilidade**. O `zipapp` grava o `mtime` de
// cada arquivo de origem. Dois empacotamentos do mesmo código em máquinas ou
// horários diferentes produzem SHA-256 diferentes, e aí o checksum que o
// workflow publica deixa de identificar o conteúdo — que é justamente o papel
// dele.
//
// Aqui o `mtime` de cada entrada é fixo, a ordem é ordenada por nome e o método
// é sempre `deflate`, com o mesmo nível. O resultado é byte a byte estável.
//
// ## O que o Python exige de um `.pyz`
//
// Um arquivo ZIP comum com `__main__.py` na **raiz**. O interpretador abre com
// `zipimport`, que aceita `deflate` e `stored`. Não há requisito de estrutura de
// diretórios: caminhos com `/` bastam, e diretórios explícitos são opcionais
// (por isso não os escrevemos).

import { deflateRawSync, inflateRawSync } from "node:zlib";

/** Data fixa das entradas: 1980-01-01 00:00, o mínimo do formato DOS. */
const DOS_DATA = 0x0021; // (1980-1980)<<9 | 1<<5 | 1
const DOS_HORA = 0x0000;

const METODO_DEFLATE = 8;

function crc32(buf) {
  let c;
  let crc = 0xffffffff;
  for (let n = 0; n < buf.length; n += 1) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/**
 * Escreve um ZIP com as entradas dadas.
 *
 * `entradas` é uma lista de `{ caminho, conteudo }`. A ordem **é** aplicada
 * aqui (ordenada por caminho) e não pelo chamador: ordem de `readdir` não é
 * controlada, e uma ordem que vem de fora é uma ordem que pode variar.
 */
export function escreverZip(entradas) {
  const ordenadas = [...entradas].sort((a, b) =>
    a.caminho < b.caminho ? -1 : a.caminho > b.caminho ? 1 : 0,
  );

  const locais = [];
  const centrais = [];
  let deslocamento = 0;

  for (const e of ordenadas) {
    const nome = Buffer.from(e.caminho, "utf8");
    const bruto = e.conteudo ?? Buffer.alloc(0);
    const comprimido = deflateRawSync(bruto, { level: 9 });
    const crc = crc32(bruto);

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0); // assinatura
    local.writeUInt16LE(20, 4); // versão necessária: 2.0 (deflate)
    local.writeUInt16LE(0x0800, 6); // bandeira: nomes em UTF-8
    local.writeUInt16LE(METODO_DEFLATE, 8);
    local.writeUInt16LE(DOS_HORA, 10);
    local.writeUInt16LE(DOS_DATA, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(comprimido.length, 18);
    local.writeUInt32LE(bruto.length, 22);
    local.writeUInt16LE(nome.length, 26);
    local.writeUInt16LE(0, 28); // sem campo extra
    locais.push(local, nome, comprimido);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0); // assinatura do diretório central
    central.writeUInt16LE(20, 4); // versão que criou
    central.writeUInt16LE(20, 6); // versão necessária
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(METODO_DEFLATE, 10);
    central.writeUInt16LE(DOS_HORA, 12);
    central.writeUInt16LE(DOS_DATA, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(comprimido.length, 20);
    central.writeUInt32LE(bruto.length, 24);
    central.writeUInt16LE(nome.length, 28);
    central.writeUInt16LE(0, 30); // extra
    central.writeUInt16LE(0, 32); // comentário
    central.writeUInt16LE(0, 34); // disco inicial
    central.writeUInt16LE(0, 36); // atributos internos
    central.writeUInt32LE(0o644 << 16, 38); // atributos externos: modo unix
    central.writeUInt32LE(deslocamento, 42);
    centrais.push(central, nome);

    deslocamento += local.length + nome.length + comprimido.length;
  }

  const diretorio = Buffer.concat(centrais);
  const fim = Buffer.alloc(22);
  fim.writeUInt32LE(0x06054b50, 0);
  fim.writeUInt16LE(0, 4); // este disco
  fim.writeUInt16LE(0, 6); // disco do diretório
  fim.writeUInt16LE(ordenadas.length, 8);
  fim.writeUInt16LE(ordenadas.length, 10);
  fim.writeUInt32LE(diretorio.length, 12);
  fim.writeUInt32LE(deslocamento, 16);
  fim.writeUInt16LE(0, 20); // sem comentário

  return Buffer.concat([...locais, diretorio, fim]);
}

/**
 * Lê o índice de um ZIP, sem extrair.
 *
 * Serve para a verificação: provar que o `.pyz` tem `__main__.py`, que os
 * módulos estão lá e que nenhum caminho saiu do esperado. Não descompacta —
 * quem valida conteúdo é o Python de verdade, rodando o arquivo.
 */
export function lerIndiceZip(buffer) {
  // Localiza o "fim do diretório central" varrendo de trás para frente. O
  // comentário final pode ter até 65535 bytes, então a janela é esse tamanho.
  const minimo = Math.max(0, buffer.length - 65557);
  let fimPos = -1;
  for (let i = buffer.length - 22; i >= minimo; i -= 1) {
    if (buffer.readUInt32LE(i) === 0x06054b50) {
      fimPos = i;
      break;
    }
  }
  if (fimPos < 0) {
    throw new Error("ZIP sem assinatura de fim de diretório central");
  }

  const total = buffer.readUInt16LE(fimPos + 10);
  let pos = buffer.readUInt32LE(fimPos + 16);
  const entradas = [];

  for (let i = 0; i < total; i += 1) {
    if (pos + 46 > buffer.length || buffer.readUInt32LE(pos) !== 0x02014b50) {
      throw new Error(`entrada ${i} do diretório central inválida`);
    }
    const metodo = buffer.readUInt16LE(pos + 10);
    const tamanhoComprimido = buffer.readUInt32LE(pos + 20);
    const tamanho = buffer.readUInt32LE(pos + 24);
    const nomeLen = buffer.readUInt16LE(pos + 28);
    const extraLen = buffer.readUInt16LE(pos + 30);
    const comentarioLen = buffer.readUInt16LE(pos + 32);
    const deslocamento = buffer.readUInt32LE(pos + 42);
    const nome = buffer.subarray(pos + 46, pos + 46 + nomeLen).toString("utf8");

    entradas.push({
      caminho: nome,
      tamanho,
      tamanhoComprimido,
      metodo,
      crc32: buffer.readUInt32LE(pos + 16),
      deslocamento,
    });
    pos += 46 + nomeLen + extraLen + comentarioLen;
  }

  return { entradas };
}

/** Extrai um membro do ZIP. Usado para conferir o que foi escrito. */
export function extrair(buffer, indice, caminho) {
  const entrada = indice.entradas.find((e) => e.caminho === caminho);
  if (!entrada) return null;

  const local = entrada.deslocamento;
  if (buffer.readUInt32LE(local) !== 0x04034b50) {
    throw new Error(`cabeçalho local inválido para ${caminho}`);
  }
  const nomeLen = buffer.readUInt16LE(local + 26);
  const extraLen = buffer.readUInt16LE(local + 28);
  const inicio = local + 30 + nomeLen + extraLen;
  const dados = buffer.subarray(inicio, inicio + entrada.tamanhoComprimido);

  if (entrada.metodo === 0) return Buffer.from(dados);
  if (entrada.metodo === METODO_DEFLATE) {
    return inflateRawSync(dados);
  }
  throw new Error(`método de compressão ${entrada.metodo} não suportado`);
}
