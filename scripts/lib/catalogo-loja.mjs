// O catálogo da loja: montagem e assinatura.
//
// ## Por que a mensagem assinada é repetida aqui
//
// O verificador é o Rust, em `app/loja/crates/core/src/catalogo.rs`, e ele monta
// a mensagem campo a campo. Aqui ela é montada de novo, em JavaScript, e **as
// duas têm de produzir bytes idênticos**.
//
// Isso é uma duplicação consciente, e este projeto já pagou o preço de uma
// parecida: a assinatura do `.giochi` existia em dois formatos (64 bytes crus no
// Python, base64 em texto no JS) e cada lado testava a si mesmo. A diferença é
// que ali a divergência era acidental; aqui ela é **contratual e testada dos
// dois lados contra o mesmo literal**.
//
// A alternativa — assinar JSON serializado — tem um problema pior: reordenar
// chaves ou mudar espaçamento invalida a assinatura sem o conteúdo ter mudado, e
// a mensagem de erro aponta para o lugar errado.
//
// ## Formato
//
// ```text
// giochi-catalogo-v1
// gerado=1700000000
// invalida_antes=-
// item=slug|titulo|ano|centavos|MOEDA|versao|tamanho|sha256|url|assinatura|chave
// ```
//
// Itens em ordem de slug, porque a ordem de exibição é da interface e não do
// formato. Sem ordenar, dois catálogos idênticos em conteúdo assinariam
// diferente.

import { createHash, createPrivateKey, sign } from "node:crypto";

/** Versão do formato. Tem de bater com `catalogo::VERSAO` do Rust. */
export const VERSAO = 1;

/**
 * A mensagem canônica que é assinada.
 *
 * @param {object} catalogo  `{versao, gerado_em, invalida_antes, itens}`
 * @returns {Buffer}
 */
export function mensagemDoCatalogo(catalogo) {
  const versao = catalogo.versao ?? VERSAO;
  const linhas = [`giochi-catalogo-v${versao}`, `gerado=${catalogo.gerado_em}`];
  linhas.push(
    catalogo.invalida_antes === null || catalogo.invalida_antes === undefined
      ? "invalida_antes=-"
      : `invalida_antes=${catalogo.invalida_antes}`,
  );

  // Cópia ordenada por slug. `sort()` de string em JS compara unidades UTF-16;
  // `cmp` de `str` em Rust compara bytes. Para ASCII os dois concordam, e é por
  // isso que o slug é kebab-case ASCII no validador — não por estética.
  for (const i of [...(catalogo.itens ?? [])].sort((a, b) => (a.slug < b.slug ? -1 : a.slug > b.slug ? 1 : 0))) {
    // `item=<slug>|...` — o `=` depois de `item` não é decoração: o Rust escreve
    // `item={}` e a primeira versão daqui escrevia `item|`, o que fazia o
    // catálogo assinado **nunca** verificar. Um caractere, e a mensagem de erro
    // ("assinatura inválida — não foi a Giochi") apontava para o lugar errado.
    linhas.push(
      [
        `item=${i.slug}`,
        i.titulo,
        i.ano ?? 0,
        i.preco?.centavos ?? 0,
        i.preco?.moeda ?? "BRL",
        i.versao,
        i.tamanho_bytes,
        i.sha256,
        i.url,
        i.assinatura,
        i.chave_pacote,
      ].join("|"),
    );
  }
  return Buffer.from(`${linhas.join("\n")}\n`, "utf8");
}

/**
 * Assina o catálogo. Devolve o objeto no formato que o Rust lê:
 * `{catalogo, assinatura}` com a assinatura em base64 dos 64 bytes crus.
 */
export function assinarCatalogo(catalogo, chavePrivadaPem) {
  const privada = createPrivateKey(chavePrivadaPem);
  const assinatura = sign(null, mensagemDoCatalogo(catalogo), privada);
  return {
    catalogo,
    assinatura: assinatura.toString("base64"),
  };
}

/** SHA-256 em hex, do jeito que o Rust compara (`eq_ignore_ascii_case`). */
export function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

/**
 * `invalida_antes` para revogar uma chave vazada.
 *
 * Existe como função nomeada porque o erro aqui é caro nos dois sentidos:
 * esquecer de revogar mantém a chave vazada válida para sempre; revogar cedo
 * demais derruba pacotes legítimos que já estão na mão dos compradores.
 */
export function corteDeRevogacao(agoraSegundos) {
  return {
    invalida_antes: agoraSegundos,
    explicacao:
      "pacotes assinados antes deste instante deixam de ser aceitos; a chave " +
      "comprometida morre e o que já saiu precisa ser reempacotado",
  };
}
