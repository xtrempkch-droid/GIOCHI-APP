#!/usr/bin/env node
// verificar-musica.mjs — confere os quadros do app de música.
//
// ## As cinco perguntas
//
// 1. **Desenhou?** Um quadro com poucas cores ou quase todo preto é render que
//    parou cedo. Já aconteceu, e o sintoma era "o app abre e fica preto".
// 2. **É determinístico?** Dois quadros com a mesma entrada têm de sair
//    idênticos byte a byte. Sem isso a comparação com o aparelho compararia
//    duas coisas diferentes — e o número pareceria regressão de desenho.
// 3. **A rotação está certa?** O quadro de um painel retrato des-rotacionado
//    tem de bater com o quadro de paisagem. Errar o sentido dá diferença de
//    180°, e "de ponta a cabeça" é invisível para quem só lê a fórmula.
// 4. **Cabe no orçamento?** Chamada de desenho é custo de backend: o número
//    sai do próprio pintor (`desenhos`), contado onde o backend é chamado.
// 5. **A ausência de música aparece?** A pasta vazia tem de desenhar uma tela
//    com texto — não uma tela em branco.
//
// ## E o aparelho
//
// `--aparelho root@ip` roda o binário **de aarch64** lá, com driver dummy,
// puxa o BMP e compara com o do PC: mesma entrada, mesmo desenho. Medido em
// 15/11/2026 no R35S: **100,00%**.
import { spawnSync, execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { analisar, avaliarQuadro, comparar, desRotacionar, lerBmp } from "./verificar-frame.mjs";

const RAIZ = path.resolve(import.meta.dirname, "..");
const APP = path.join(RAIZ, "app", "musica");
const CANDIDATOS = [
  path.join(APP, "target", "debug", "giochi-musica"),
  path.join(APP, "target", "release", "giochi-musica"),
];

const TAMANHOS = ["640x480", "1280x720", "1920x1080"];
const ORCAMENTO_DESENHOS = 130;

let falhas = 0;
const ok = (msg) => console.log(`  ok    ${msg}`);
const falha = (msg) => {
  falhas += 1;
  console.log(`  FALHA ${msg}`);
};

function binario() {
  const encontrado = CANDIDATOS.find((c) => existsSync(c));
  if (!encontrado) {
    console.error("erro: binário não encontrado — rode bash scripts/compilar-musica.sh");
    process.exit(2);
  }
  return encontrado;
}

/**
 * Roda o app com driver dummy e devolve { arquivo, desenhos, saida }.
 *
 * `--sem-som` em tudo: o verificador mede **desenho**, e abrir o mixer numa
 * máquina sem placa faz o teste falhar por motivo que não é o testado.
 */
function renderizar(bin, destino, tamanho, extra = []) {
  const r = spawnSync(
    bin,
    [
      "--demo",
      "--sem-som",
      `--tamanho=${tamanho}`,
      `--dump-frame=${destino}`,
      ...extra,
    ],
    { env: { ...process.env, SDL_VIDEODRIVER: "dummy" }, encoding: "utf8" },
  );
  if (r.status !== 0) {
    throw new Error(`o app saiu com ${r.status}:\n${r.stderr || r.stdout}`);
  }
  const desenhos = Number((r.stderr.match(/(\d+) desenhos/) || [])[1] || 0);
  return { arquivo: destino, desenhos, saida: r.stderr };
}

function bytes(a, b) {
  return readFileSync(a).equals(readFileSync(b));
}

function principal() {
  const argv = process.argv.slice(2);
  const indice = argv.indexOf("--aparelho");
  const aparelho = indice >= 0 ? argv[indice + 1] : null;

  const bin = binario();
  const dir = mkdtempSync(path.join(os.tmpdir(), "giochi-musica-"));
  const quadro = (nome) => path.join(dir, `${nome}.bmp`);

  try {
    console.log("desenho e determinismo");

    for (const tamanho of TAMANHOS) {
      const a = renderizar(bin, quadro(`a-${tamanho}`), tamanho);
      const img = lerBmp(a.arquivo);
      const medida = analisar(img, () => {});
      // O app desenha texto antialiasing e capas com faixas: um quadro com
      // dezenas de cores é o normal. O piso de 40 é folgado contra "parou cedo".
      const avaliacao = avaliarQuadro({ ...medida, minCores: 40 });
      if (avaliacao.problemas.length > 0) {
        falha(`${tamanho}: ${avaliacao.problemas.join("; ")}`);
      } else {
        ok(`${tamanho}: ${medida.cores} cores, ${medida.pctPreto.toFixed(1)}% de preto`);
      }

      const b = renderizar(bin, quadro(`b-${tamanho}`), tamanho);
      if (!bytes(a.arquivo, b.arquivo)) {
        falha(`${tamanho}: dois quadros iguais deram bytes diferentes`);
      } else {
        ok(`${tamanho}: determinístico (${a.desenhos} desenhos)`);
      }

      if (a.desenhos > ORCAMENTO_DESENHOS) {
        falha(`${tamanho}: ${a.desenhos} desenhos, orçamento ${ORCAMENTO_DESENHOS}`);
      }
    }

    console.log("rotação");
    const paisagem = renderizar(bin, quadro("paisagem"), "1280x720");
    const retrato = renderizar(bin, quadro("retrato"), "720x1280");
    const comparacao = comparar(
      lerBmp(retrato.arquivo),
      lerBmp(paisagem.arquivo),
      "ccw",
      12,
      () => {},
    );
    if (comparacao.pctAcerto >= 99.5) {
      ok(`720x1280 des-rotacionado bate ${comparacao.pctAcerto.toFixed(2)}% com 1280x720`);
    } else {
      falha(`rotação: ${comparacao.pctAcerto.toFixed(2)}% (esperado ≥ 99,5%)`);
    }

    console.log("ausência de música");
    const vazio = path.join(dir, "vazio.bmp");
    const r = spawnSync(
      bin,
      ["--pasta=/tmp/giochi-musica-pasta-que-nao-existe", "--sem-som", "--tamanho=640x480", `--dump-frame=${vazio}`],
      { env: { ...process.env, SDL_VIDEODRIVER: "dummy" }, encoding: "utf8" },
    );
    if (r.status !== 0) {
      falha(`tela de pasta vazia saiu com ${r.status}`);
    } else {
      const medida = analisar(lerBmp(vazio), () => {});
      if (medida.cores < 20) {
        falha(`tela de pasta vazia com ${medida.cores} cores — parece em branco`);
      } else {
        ok(`pasta inexistente desenha ${medida.cores} cores (a tela diz o motivo)`);
      }
      // E não pode ser a mesma tela da biblioteca cheia.
      if (bytes(vazio, quadro("a-640x480"))) {
        falha("a tela vazia é idêntica à cheia");
      }
    }

    if (aparelho) {
      console.log(`aparelho (${aparelho})`);
      const remoto = "/tmp/giochi-musica-verificar.bmp";
      const ssh = ["-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=8"];
      const r2 = spawnSync(
        "ssh",
        [
          ...ssh,
          aparelho,
          `SDL_VIDEODRIVER=dummy /opt/giochi/apps/musica/bin/giochi-musica --demo --sem-som --tamanho=640x480 --dump-frame=${remoto}`,
        ],
        { encoding: "utf8" },
      );
      if (r2.status !== 0) {
        falha(`não consegui rodar no aparelho: ${(r2.stderr || "").trim().split("\n").pop()}`);
      } else {
        const local = quadro("aparelho");
        execFileSync("scp", [...ssh, `${aparelho}:${remoto}`, local], { stdio: "pipe" });
        const c = comparar(lerBmp(local), lerBmp(quadro("a-640x480")), "none", 0, () => {});
        if (c.pctAcerto >= 99.99) {
          ok(`aparelho × PC: ${c.pctAcerto.toFixed(2)}% idênticos`);
        } else {
          falha(`aparelho × PC: ${c.pctAcerto.toFixed(2)}% (esperado 100%)`);
        }
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  console.log();
  if (falhas > 0) {
    // O resumo que ignora falha é pior que nenhum: a primeira versão do
    // `verificar-efeitos.sh` imprimia "ok camada verificada" depois de um FALHA.
    console.log(`FALHOU: ${falhas} verificação(ões)`);
    process.exit(1);
  }
  console.log("ok: todas as verificações passaram");
}

principal();
