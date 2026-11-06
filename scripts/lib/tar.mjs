// Núcleo do pacote `.giochi`: escrever e ler o tar na ordem obrigatória.
//
// ## Por que tar ustar à mão, e não `tar` do sistema
//
// Três razões, todas de comportamento observável:
//
// 1. **Ordem.** O `tar` do sistema escreve na ordem em que os arquivos aparecem
//    no `readdir`, que não é controlada. A ordem aqui é contrato: `metadata.json`
//    e `capa.png` **antes** de `bin/`, porque o extrator no aparelho lê em
//    streaming e precisa recusar um pacote incompatível antes de baixar
//    megabytes (ver `docs/pacote.md`).
// 2. **Reprodutibilidade.** `mtime`, `uid`, `gid` e nome de usuário entram no
//    cabeçalho. Se vierem do sistema, dois empacotamentos do mesmo conteúdo dão
//    SHA-256 diferentes e a assinatura deixa de valer como identidade do
//    conteúdo. Aqui são fixos.
// 3. **Bit de execução.** `tar` com `--mode` mascara permissões; o extrator
//    precisa de `bin/app` executável e o erro só aparece no aparelho, como
//    "entrypoint não existe" (lição do teste do antecessor).
//
// O resultado é testável sem disco e sem `tar` instalado.

import { createHash } from "node:crypto";

/** Limite do `metadata.json` dentro do pacote. */
export const METADATA_MAX = 16 * 1024;

/** Lado da capa, em pixels. Obrigatório. */
export const CAPA_LADO = 320;

/** Ordem obrigatória dos membros. `bin/` é diretório: seus arquivos vêm depois. */
export const ORDEM = Object.freeze(["metadata.json", "capa.png", "thumb.png", "bin/"]);

const BLOCO = 512;

/**
 * Cabeçalho ustar de 512 bytes.
 *
 * Campos numéricos em octal com terminador NUL, largura fixa — é o que o formato
 * define, e um `toString(8)` sem padding produz cabeçalho ilegível pelo kernel
 * do `tar` na hora de extrair.
 */
function cabecalho({ nome, tamanho, modo, tipo = "0" }) {
  const b = Buffer.alloc(BLOCO, 0);
  const escreve = (texto, pos, max) => {
    const bytes = Buffer.from(texto, "utf8");
    if (bytes.length > max) {
      throw new Error(`nome longo demais para ustar (${bytes.length} > ${max}): ${texto}`);
    }
    bytes.copy(b, pos);
  };
  const octal = (valor, pos, largura) => {
    // largura inclui o NUL final.
    const s = valor.toString(8).padStart(largura - 1, "0");
    b.write(s, pos, largura - 1, "ascii");
  };

  // Prefixo/subcampo: ustar divide o caminho em 155 + 100. Nomes de jogo podem
  // passar de 100, então dividimos no último `/` que caiba.
  const { prefixo, nome: resto } = divideCaminho(nome);
  escreve(resto, 0, 100);
  octal(modo & 0o7777, 100, 8); // modo
  octal(0, 108, 8); // uid: fixo, para o pacote não depender de quem empacotou
  octal(0, 116, 8); // gid
  octal(tamanho, 124, 12); // tamanho
  octal(0, 136, 12); // mtime: fixo — é o que torna o pacote reprodutível
  b.write("        ", 148, 8, "ascii"); // checksum: espaços durante o cálculo
  b.write(tipo, 156, 1, "ascii");
  escreve("ustar\0", 257, 6);
  escreve("00", 263, 2);
  escreve(prefixo, 345, 155);

  // Checksum: soma dos bytes com o campo em branco. Assinado (alguns `tar`
  // antigos usavam soma com sinal); o formato moderno é unsigned.
  let soma = 0;
  for (const byte of b) soma += byte;
  b.write(soma.toString(8).padStart(6, "0") + "\0 ", 148, 8, "ascii");

  return b;
}

function divideCaminho(nome) {
  if (Buffer.byteLength(nome) <= 100) return { prefixo: "", nome };
  const partes = nome.split("/");
  for (let corte = partes.length - 1; corte > 0; corte--) {
    const prefixo = partes.slice(0, corte).join("/");
    const resto = partes.slice(corte).join("/");
    if (Buffer.byteLength(prefixo) <= 155 && Buffer.byteLength(resto) <= 100) {
      return { prefixo, nome: resto };
    }
  }
  throw new Error(`caminho não cabe no ustar: ${nome}`);
}

function padding(tamanho) {
  const resto = tamanho % BLOCO;
  return resto === 0 ? 0 : BLOCO - resto;
}

/**
 * Monta o pacote.
 *
 * `membros` é uma lista de `{ caminho, conteudo, modo }` **já na ordem final**.
 * A validação da ordem fica em `scripts/lib/publicar.mjs`, separada da escrita:
 * aqui só se escreve o que mandaram, e é isso que permite testar as duas coisas
 * de forma independente.
 */
export function escreverTar(membros) {
  const partes = [];
  for (const m of membros) {
    const conteudo = m.conteudo ?? Buffer.alloc(0);
    const ehDiretorio = m.caminho.endsWith("/");
    partes.push(
      cabecalho({
        nome: m.caminho,
        tamanho: ehDiretorio ? 0 : conteudo.length,
        modo: m.modo ?? (ehDiretorio ? 0o755 : 0o644),
        tipo: ehDiretorio ? "5" : "0",
      }),
    );
    if (!ehDiretorio) {
      partes.push(conteudo);
      const pad = padding(conteudo.length);
      if (pad) partes.push(Buffer.alloc(pad, 0));
    }
  }
  // Dois blocos de zero no fim: é o que marca EOF no tar.
  partes.push(Buffer.alloc(BLOCO * 2, 0));
  return Buffer.concat(partes);
}

function lerOctal(buf, pos, largura) {
  const texto = buf
    .subarray(pos, pos + largura)
    .toString("ascii")
    .replace(/\0.*$/, "")
    .trim();
  return texto === "" ? 0 : parseInt(texto, 8);
}

/**
 * Lê o pacote em streaming, devolvendo cada membro na ordem em que aparece.
 *
 * Streaming importa no aparelho: recusar cedo exige parar de ler. O leitor
 * devolve um iterador, então quem chama pode dar `break` no primeiro membro
 * inesperado sem ler o resto.
 */
export function* lerTar(buffer) {
  let pos = 0;
  let fim = false;
  while (pos + BLOCO <= buffer.length) {
    const bloco = buffer.subarray(pos, pos + BLOCO);
    if (bloco.every((b) => b === 0)) {
      fim = true;
      break;
    }

    const prefixo = lerTexto(bloco, 345, 155);
    const nome = lerTexto(bloco, 0, 100);
    const caminho = prefixo ? `${prefixo}/${nome}` : nome;
    const tamanho = lerOctal(bloco, 124, 12);
    const modo = lerOctal(bloco, 100, 8);
    const tipo = String.fromCharCode(bloco[156]);
    const ehDiretorio = tipo === "5";

    pos += BLOCO;
    const conteudo = ehDiretorio ? Buffer.alloc(0) : buffer.subarray(pos, pos + tamanho);
    if (!ehDiretorio) {
      if (pos + tamanho > buffer.length) {
        throw new Error(`pacote truncado: ${caminho} declara ${tamanho} bytes`);
      }
      pos += tamanho + padding(tamanho);
    }

    yield { caminho, conteudo, modo, diretorio: ehDiretorio };
  }

  if (!fim) throw new Error("pacote sem o marcador de fim (dois blocos de zero)");
}

function lerTexto(buf, pos, largura) {
  return buf.subarray(pos, pos + largura).toString("utf8").replace(/\0.*$/, "");
}

/** SHA-256 em hexadecimal. */
export function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}
