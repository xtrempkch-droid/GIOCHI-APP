// Qual chave privada assina os pacotes — e por que a ordem importa.
//
// A imagem do aparelho confia em **uma** chave pública:
// `/etc/giochi/loja-publica.pem`, que o `Giochi System` versiona em
// `device/common/rootfs-overlay/etc/giochi/loja-publica.pem`. Pacificar
// `.giochi` com qualquer outra gera o pior defeito possível deste formato:
// o arquivo é íntegro, o `sha256` confere, e o aparelho **recusa** com
// "assinatura não confere" — que manda procurar pacote corrompido, não chave
// trocada.
//
// Aconteceu em 07/11/2026: o `empacotar.mjs` não achou `~/.giochi` e gerou um
// par novo em silêncio; a imagem tinha a chave do System. O pacote foi recusado
// no R35S e o diagnóstico correto só apareceu comparando os dois PEM.
//
// Por isso a chave do System é **candidata de primeira classe**, e não um
// caminho presumido: o local canônico está documentado em
// `GIOCHI-SYSTEM/scripts/chave-da-loja.sh` e pode ser trocado por
// `GIOCHI_CHAVES_DIR`.
import { existsSync, readFileSync } from "node:fs";
import { createPublicKey } from "node:crypto";
import { join } from "node:path";
import { homedir } from "node:os";

/** Onde o `Giochi System` guarda as privadas da loja (fora dos dois repos). */
export function diretorioDasChavesDoSystem() {
  return process.env.GIOCHI_CHAVES_DIR ?? join(homedir(), ".cache/giochi-system/chaves-loja");
}

/**
 * Chaves privadas candidatas, em ordem de precedência.
 *
 * A do **Giochi System** vem primeiro porque é a única cuja pública está na
 * imagem: assinar com outra produz pacote íntegro que o aparelho recusa, e a
 * mensagem ("assinatura não confere") manda procurar corrupção, não chave.
 * `~/.giochi/chave-privada.pem` continua aceita como padrão histórico — quem
 * usa a própria chave também constrói a própria imagem.
 */
export function candidatosDeChavePrivada() {
  const doSystem = diretorioDasChavesDoSystem();
  return [
    process.env.GIOCHI_CHAVE_PRIVADA,
    join(doSystem, "loja-privada.pem"),
    join(homedir(), ".giochi", "chave-privada.pem"),
  ].filter(Boolean);
}

/** Primeira privada que existe, ou `null`. */
export function chavePrivadaDoDisco() {
  return candidatosDeChavePrivada().find((c) => existsSync(c)) ?? null;
}

/**
 * Públicas candidatas para verificar um pacote.
 *
 * A pública que **importa** é a do aparelho, e `aparelho.sh instalar-pacote`
 * busca a de lá. Estas são o palpite local: a primeira que existir.
 */
export function candidatosDeChavePublica() {
  const doSystem = diretorioDasChavesDoSystem();
  return [
    process.env.GIOCHI_CHAVE_PUBLICA,
    join(homedir(), ".giochi", "chave-privada-publica.pem"),
    join(doSystem, "loja-publica.pem"),
  ].filter(Boolean);
}

/** Primeira pública que existe, ou `null`. */
export function chavePublicaDoDisco() {
  return candidatosDeChavePublica().find((c) => existsSync(c)) ?? null;
}

/** A pública que a imagem carrega no aparelho, se estiver nesta máquina. */
export function publicaDaImagem() {
  const candidatos = [
    join(diretorioDasChavesDoSystem(), "loja-publica.pem"),
    join(homedir(), ".giochi", "chave-privada-publica.pem"),
  ];
  return candidatos.find((c) => existsSync(c)) ?? null;
}

/**
 * A privada assina algo que a imagem aceita?
 *
 * Compara a pública derivada com a pública da imagem. `null` = não deu para
 * concluir (falta uma das duas), e nesse caso não se avisa: não medir não é
 * reprovar.
 */
export function combinaComAImagem(privadaPem) {
  const daImagem = publicaDaImagem();
  if (!daImagem) return null;
  const normalizar = (t) => t.replace(/\s+/g, "");
  try {
    return normalizar(publicaDe(privadaPem)) === normalizar(readFileSync(daImagem, "utf8"));
  } catch {
    return null;
  }
}

/**
 * A pública derivada de uma privada, no mesmo formato PEM que a imagem guarda.
 *
 * Usada quando a privada veio de `--chave` e não há `.pem` público ao lado:
 * verificar contra uma pública de outra geração produziria um veredito, e um
 * veredito errado é pior que "não consegui verificar".
 */
export function publicaDe(privadaPem) {
  return createPublicKey(comoPem(privadaPem)).export({ type: "spki", format: "pem" });
}

/** Aceita o PEM em texto (do `readFileSync`) ou o caminho do arquivo. */
function comoPem(valor) {
  return /-----BEGIN/.test(valor) ? valor : readFileSync(valor, "utf8");
}
