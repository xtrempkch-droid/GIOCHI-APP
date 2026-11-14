#!/usr/bin/env node
// Gerador dos modelos (OBJ) e das texturas (BMP) do GIOCHI Tower Defense.
//
// # Por que gerar em vez de desenhar à mão
//
// Os personagens eram cubos porque a engine, até o snapshot de 13/11/2026, não
// tinha caminho de modelo nem de textura. Agora tem (`load_obj_mesh` e
// `create_texture`), e este script produz os arquivos que o jogo carrega.
//
// Arte desenhada em código tem duas propriedades que importam aqui:
//
// 1. **é reproduzível** — a mesma execução dá os mesmos bytes, então um
//    `git diff` nos assets diz exatamente o que mudou, e `--conferir` pode
//    exigir isso no CI;
// 2. **é auditável** — "o Tank é mais largo que o Grunt" vira número no gerador
//    (`ombro: 0.72` contra `0.38`), não uma impressão sobre um binário.
//
// O que ela não é: arte final. Um modelador faz melhor. O que existe aqui é
// **silhueta legível a 640×480**, que é o que o aparelho precisa.
//
// # Formato
//
// OBJ com `v` e `vt`, faces `f v/vt`, **sem `vn`**. O loader da engine deriva a
// normal por face quando ela falta (`prepare_normals`), e faceta visível é
// exatamente o que se quer num low-poly: normal suave aqui esconderia o relevo
// que a luz do shader `textured` desenha.
//
// BMP 24 bits e não PNG: o `SDL2_image` do aparelho já apareceu compilado sem
// `--enable-png` (AGENTS.md §24), e BMP atravessa todas as imagens do projeto.
//
// Uso:
//   node scripts/gerar-modelos.mjs             # gera em assets/
//   node scripts/gerar-modelos.mjs --conferir  # não escreve; falha se divergir

import { mkdirSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..");

// ─────────────────────────────────────────────────────────────── paletas
//
// Uma paleta por personagem, não cor por peça: a textura é multiplicada pela luz
// no shader, então dois tons próximos viram um só a 640×480. O `acento` é o que
// sobrevive à distância e diz de quem é o quê.
//
// # A direção de arte é a de Tropas Estelares (1997)
//
// Dois lados, duas linguagens de cor:
//
// - **infantaria** é aço claro e cinza-azulado, com viseira e faixa de acento
//   (âmbar no soldado, laranja no guerreiro, verde no sniper, vermelho no
//   médico). É o casaco blindado cinza-prata do filme; o acento é o que
//   distingue a classe a 20 m.
// - **aracnídeos** são quitina: marrom-ocre, oliva e vinho, com acento nos olhos
//   e nas juntas. Nada de roxo ou azul-claro de desenho animado — o filme é
//   marrom, e é o que separa "bug" de "brinquedo".
//
// A torre e o cenário ficam na mesma família do terreno: metal fosco e basalto,
// para o olho achar o alvo (a torre) pela forma, não pela cor berrante.
const PALETAS = {
  soldado: { base: [126, 134, 146], claro: [192, 200, 212], escuro: [48, 54, 64], acento: [236, 188, 72] },
  guerreiro: { base: [96, 102, 112], claro: [154, 162, 174], escuro: [34, 38, 46], acento: [226, 118, 56] },
  sniper: { base: [100, 114, 106], claro: [158, 176, 164], escuro: [38, 46, 42], acento: [124, 226, 150] },
  medico: { base: [204, 208, 214], claro: [244, 246, 250], escuro: [88, 92, 100], acento: [226, 58, 68] },
  grunt: { base: [124, 96, 60], claro: [176, 142, 96], escuro: [44, 30, 14], acento: [246, 206, 84] },
  tank: { base: [102, 94, 74], claro: [152, 140, 112], escuro: [34, 30, 20], acento: [216, 174, 62] },
  ranged: { base: [80, 104, 110], claro: [128, 162, 168], escuro: [26, 38, 40], acento: [98, 238, 232] },
  swarm: { base: [138, 104, 64], claro: [190, 150, 98], escuro: [50, 32, 14], acento: [250, 218, 96] },
  flying: { base: [116, 84, 92], claro: [166, 124, 136], escuro: [42, 24, 30], acento: [250, 130, 176] },
  tanker: { base: [88, 80, 64], claro: [138, 126, 102], escuro: [28, 24, 16], acento: [234, 96, 58] },
  plasma: { base: [62, 96, 158], claro: [110, 152, 222], escuro: [20, 30, 68], acento: [152, 238, 255] },
  brain: { base: [178, 126, 146], claro: [226, 176, 194], escuro: [76, 44, 58], acento: [255, 178, 222] },
  queen: { base: [108, 66, 128], claro: [164, 110, 190], escuro: [42, 20, 60], acento: [255, 218, 118] },
  coronel: { base: [62, 46, 40], claro: [112, 80, 66], escuro: [22, 14, 10], acento: [228, 62, 92] },
  torre: { base: [104, 118, 138], claro: [164, 182, 208], escuro: [30, 38, 52], acento: [122, 210, 255] },
  rocha: { base: [116, 100, 84], claro: [162, 144, 124], escuro: [50, 42, 34], acento: [186, 166, 140] },
  espigao: { base: [92, 82, 72], claro: [134, 120, 104], escuro: [36, 30, 24], acento: [150, 132, 112] },
  cratera: { base: [96, 82, 68], claro: [130, 112, 94], escuro: [44, 36, 28], acento: [150, 130, 108] },
  mesa: { base: [110, 88, 74], claro: [154, 126, 106], escuro: [48, 36, 28], acento: [188, 152, 124] },
  destroco: { base: [112, 118, 126], claro: [158, 166, 176], escuro: [42, 46, 54], acento: [214, 148, 64] },
};

// ───────────────────────────────────────────────────── primitivas de malha

/**
 * Uma malha em construção: posições, normais, UVs e triângulos.
 *
 * # Por que existe `vn`
 *
 * A primeira versão não emitia normal nenhuma e o loader da engine derivava uma
 * **por face**. O resultado na vitrine foi uma figura feita de placas empilhadas:
 * cada quadrilátero do lathe com o seu próprio tom, e o corpo lia como um totem
 * de panquecas. Normal por vértice (radial no lathe, da face na caixa) devolve a
 * curvatura ao cilindro e mantém o painel chapado onde ele deve ser chapado.
 */
function malha() {
  return { v: [], vn: [], vt: [], f: [] };
}

/**
 * Revoluciona um perfil 2D `[[raio, y], ...]` em torno do eixo Y.
 *
 * É o que dá corpo, cabeça, braço e perna com a mesma função: um cilindro é um
 * perfil de dois pontos, uma esfera achatada é um perfil de cinco. O UV é
 * cilíndrico — `u` dá a volta e `v` sobe o perfil —, então a textura acompanha
 * qualquer forma sem costura.
 */
function lathe(m, perfil, { segmentos = 10, centro = [0, 0, 0], escala = 1, fechar = true } = {}) {
  const base = m.v.length;
  const linhas = perfil.length;
  // Normal do perfil em 2D: tangente entre o ponto anterior e o seguinte, girada
  // 90°. É o que faz um perfil côncavo (o pescoço do elmo) sombrear certo em vez
  // de apontar para fora do sólido.
  const normaisPerfil = perfil.map((ponto, i) => {
    const anterior = perfil[Math.max(0, i - 1)];
    const seguinte = perfil[Math.min(linhas - 1, i + 1)];
    const dr = seguinte[0] - anterior[0];
    const dy = seguinte[1] - anterior[1];
    const comprimento = Math.hypot(dr, dy);
    if (comprimento < 1e-6) return [0, 1];
    return [dy / comprimento, -dr / comprimento];
  });
  for (let i = 0; i < linhas; i++) {
    const [raio, y] = perfil[i];
    const [nr, ny] = normaisPerfil[i];
    for (let s = 0; s < segmentos; s++) {
      const a = (s / segmentos) * Math.PI * 2;
      const r = raio * escala;
      m.v.push([centro[0] + Math.cos(a) * r, centro[1] + y * escala, centro[2] + Math.sin(a) * r]);
      m.vn.push([Math.cos(a) * nr, ny, Math.sin(a) * nr]);
      m.vt.push([s / segmentos, i / Math.max(1, linhas - 1)]);
    }
  }
  const idx = (linha, seg) => base + linha * segmentos + (seg % segmentos);
  for (let i = 0; i < linhas - 1; i++) {
    for (let s = 0; s < segmentos; s++) {
      // Sentido horário visto de fora, que é o que o descarte de faces espera.
      // Inverter some com metade do modelo sem erro nenhum.
      m.f.push([idx(i, s), idx(i + 1, s), idx(i + 1, s + 1)]);
      m.f.push([idx(i, s), idx(i + 1, s + 1), idx(i, s + 1)]);
    }
  }
  if (fechar) {
    tapar(m, base, segmentos, 0, true);
    tapar(m, base, segmentos, linhas - 1, false);
  }
  return m;
}

/** Fecha um anel com um leque no centro. */
function tapar(m, base, segmentos, linha, invertido) {
  let cx = 0;
  let cz = 0;
  let cy = 0;
  for (let s = 0; s < segmentos; s++) {
    const v = m.v[base + linha * segmentos + s];
    cx += v[0];
    cy += v[1];
    cz += v[2];
  }
  const iCentro = m.v.length;
  m.v.push([cx / segmentos, cy / segmentos, cz / segmentos]);
  m.vn.push(invertido ? [0, 1, 0] : [0, -1, 0]);
  m.vt.push([0.5, 0.5]);
  for (let s = 0; s < segmentos; s++) {
    const a = base + linha * segmentos + s;
    const b = base + linha * segmentos + ((s + 1) % segmentos);
    if (invertido) m.f.push([iCentro, b, a]);
    else m.f.push([iCentro, a, b]);
  }
}

/** Caixa alinhada aos eixos, com UV planar por face. */
function caixa(m, { centro = [0, 0, 0], tamanho = [1, 1, 1] } = {}) {
  const [cx, cy, cz] = centro;
  const [w, h, d] = tamanho.map((t) => t * 0.5);
  const cantos = [
    [-w, -h, -d], [w, -h, -d], [w, h, -d], [-w, h, -d],
    [-w, -h, d], [w, -h, d], [w, h, d], [-w, h, d],
  ];
  // UV por face, não projetado no mundo: uma caixa de 4 m com UV de mundo
  // mostraria 4 repetições e a mesma textura pareceria de outro material.
  const faces = [
    [4, 5, 6, 7], [1, 0, 3, 2], [5, 1, 2, 6], [0, 4, 7, 3], [3, 7, 6, 2], [0, 1, 5, 4],
  ];
  const uvFace = [[0, 0], [1, 0], [1, 1], [0, 1]];
  const normaisFace = [
    [0, 0, 1], [0, 0, -1], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0],
  ];
  faces.forEach((face, indiceFace) => {
    const base = m.v.length;
    const normal = normaisFace[indiceFace];
    face.forEach((indice, i) => {
      const c = cantos[indice];
      m.v.push([cx + c[0], cy + c[1], cz + c[2]]);
      m.vn.push(normal);
      m.vt.push(uvFace[i]);
    });
    m.f.push([base, base + 1, base + 2]);
    m.f.push([base, base + 2, base + 3]);
  });
}

/** Esfera achatada por lathe — cabeça, olho e boss redondo. */
function esfera(m, { centro = [0, 0, 0], raio = 0.5, achatada = 1, segmentos = 10 } = {}) {
  const perfil = [];
  const passos = 5;
  for (let i = 0; i <= passos; i++) {
    const t = i / passos;
    const y = (t - 0.5) * 2 * raio * achatada;
    const r = Math.sqrt(Math.max(0, raio * raio - (y / achatada) * (y / achatada)));
    perfil.push([r, y]);
  }
  lathe(m, perfil, { segmentos, centro });
}

/**
 * Aplica `fn` a cada vértice criado a partir de `inicio`.
 *
 * É como o elipsoide e a haste são construídos: geram uma forma por `lathe`
 * (cujo sentido de face já está provado) e depois deformam **só os vértices
 * novos**. Reescrever a geração com outra ordem de triângulo seria a forma mais
 * fácil de inverter metade das faces e ver o modelo sumir sem erro nenhum.
 */
function transformar(m, inicio, fn, fnNormal = (n) => n) {
  for (let i = inicio; i < m.v.length; i++) {
    m.v[i] = fn(m.v[i]);
    m.vn[i] = fnNormal(m.vn[i]);
  }
}

/** Elipsoide por raios independentes em X, Y e Z. */
function elipsoide(m, { centro = [0, 0, 0], raios = [0.5, 0.5, 0.5], segmentos = 10 } = {}) {
  const inicio = m.v.length;
  esfera(m, { raio: 1, achatada: 1, segmentos });
  transformar(
    m,
    inicio,
    ([x, y, z]) => [
      centro[0] + x * raios[0],
      centro[1] + y * raios[1],
      centro[2] + z * raios[2],
    ],
    // Normal de elipsoide é a da esfera dividida pelos raios (transposta da
    // inversa da escala). Usar a posição como normal daria uma esfera "gorda".
    ([x, y, z]) => {
      const nx = x / raios[0];
      const ny = y / raios[1];
      const nz = z / raios[2];
      const l = Math.hypot(nx, ny, nz) || 1;
      return [nx / l, ny / l, nz / l];
    },
  );
}

/**
 * Cilindro entre dois pontos — perna, mandíbula, antena e cano.
 *
 * Uma `lathe` só sabe crescer em Y; um inseto é feito de membros inclinados, e
 * caixas alinhadas aos eixos leriam como blocos. A rotação é Rodrigues de
 * `(0,1,0)` para a direção pedida, aplicada depois que o `lathe` fez a geometria.
 */
function haste(m, { de, ate, raio = 0.05, segmentos = 5 }) {
  const dx = ate[0] - de[0];
  const dy = ate[1] - de[1];
  const dz = ate[2] - de[2];
  const comprimento = Math.hypot(dx, dy, dz);
  if (comprimento < 1e-6) return;
  const d = [dx / comprimento, dy / comprimento, dz / comprimento];

  // Eixo de rotação = (0,1,0) × d. Quando d é quase vertical o produto some, e aí
  // a haste já está alinhada: só translada.
  let eixo = [-d[2], 0, d[0]];
  const sen = Math.hypot(eixo[0], eixo[1], eixo[2]);
  const cos = d[1];
  const angulo = Math.atan2(sen, cos);
  if (sen > 1e-6) eixo = eixo.map((c) => c / sen);

  const inicio = m.v.length;
  lathe(m, [[raio, 0], [raio, comprimento]], { segmentos });

  const girar = ([x, y, z]) => {
    if (sen <= 1e-6) return [x, y, z];
    const [kx, ky, kz] = eixo;
    const c = Math.cos(angulo);
    const s = Math.sin(angulo);
    const kp = kx * x + ky * y + kz * z;
    const kxp = [ky * z - kz * y, kz * x - kx * z, kx * y - ky * x];
    return [
      x * c + kxp[0] * s + kx * kp * (1 - c),
      y * c + kxp[1] * s + ky * kp * (1 - c),
      z * c + kxp[2] * s + kz * kp * (1 - c),
    ];
  };
  transformar(
    m,
    inicio,
    (v) => {
      const [x, y, z] = girar(v);
      return [x + de[0], y + de[1], z + de[2]];
    },
    girar,
  );
}

/**
 * Aracnídeo — a silhueta que diz "bug" a 640×480.
 *
 * O que lê à distância não é textura, é contorno: corpo **alongado em Z**, mais
 * largo que alto, seis pernas abertas e cabeça baixa com mandíbulas. Um humanoide
 * com cor de inseto continua lendo como humanoide — foi o defeito da primeira
 * versão, em que os cinco inimigos eram o mesmo `humanoide()` pintado.
 *
 * A frente do modelo é **-Z**, como o fuzil dos soldados: quem gira pelo rumo do
 * movimento (o jogo faz isso) aponta a mandíbula para onde anda.
 */
function aracnideo({
  altura = 1.6,
  comprimento = 1.7,
  largura = 0.46,
  pernas = 3,
  corpo = 0.56,
  chifre = 0,
  asas = 0,
  bolsa = 0,
  placas = 0,
  segmentos = 8,
} = {}) {
  const m = malha();
  const y = altura * corpo;
  const yCorpo = y * 0.96;

  // Abdômen atrás (+Z), tórax no meio, cabeça à frente (-Z).
  elipsoide(m, {
    centro: [0, yCorpo, comprimento * 0.30],
    raios: [largura * 1.06, altura * 0.27, comprimento * 0.40],
    segmentos,
  });
  elipsoide(m, {
    centro: [0, y, -comprimento * 0.02],
    raios: [largura, altura * 0.25, comprimento * 0.24],
    segmentos,
  });
  elipsoide(m, {
    centro: [0, yCorpo * 0.94, -comprimento * 0.36],
    raios: [largura * 0.74, altura * 0.19, comprimento * 0.19],
    segmentos,
  });

  // Olhos: duas contas pequenas, o único ponto brilhante da silhueta.
  for (const lado of [-1, 1]) {
    elipsoide(m, {
      centro: [lado * largura * 0.34, yCorpo * 1.02, -comprimento * 0.46],
      raios: [largura * 0.16, altura * 0.055, largura * 0.16],
      segmentos: 6,
    });
  }

  // Mandíbulas: duas pinças curvas para dentro, à frente da cabeça.
  for (const lado of [-1, 1]) {
    haste(m, {
      de: [lado * largura * 0.42, yCorpo * 0.9, -comprimento * 0.45],
      ate: [lado * largura * 0.62, yCorpo * 0.62, -comprimento * 0.72],
      raio: largura * 0.1,
      segmentos: 4,
    });
  }

  // Antenas: finas, para cima e para a frente. Somem a 640×480, mas quebram a
  // linha horizontal da cabeça quando o inseto está perto da câmera.
  for (const lado of [-1, 1]) {
    haste(m, {
      de: [lado * largura * 0.22, yCorpo * 1.1, -comprimento * 0.4],
      ate: [lado * largura * 0.5, yCorpo + altura * 0.22, -comprimento * 0.58],
      raio: largura * 0.045,
      segmentos: 4,
    });
  }

  if (chifre > 0) {
    haste(m, {
      de: [0, yCorpo * 0.98, -comprimento * 0.4],
      ate: [0, yCorpo + altura * chifre, -comprimento * 0.78],
      raio: largura * 0.18,
      segmentos: 5,
    });
  }

  // Pernas: `pernas` pares, do tórax ao abdômen. Joelho acima do corpo e pé no
  // chão dão o "andar de inseto" que uma perna reta não dá.
  for (let par = 0; par < pernas; par++) {
    const t = pernas <= 1 ? 0.5 : par / (pernas - 1);
    const z = -comprimento * 0.24 + t * comprimento * 0.58;
    const abertura = 0.9 + t * 0.5;
    for (const lado of [-1, 1]) {
      const quadril = [lado * largura * 0.78, y * 0.94, z];
      const joelho = [
        lado * largura * abertura,
        y * (1.28 - t * 0.16),
        z + (t - 0.5) * comprimento * 0.22,
      ];
      const pe = [lado * largura * (abertura + 0.7), 0, z + (t - 0.5) * comprimento * 0.5];
      haste(m, { de: quadril, ate: joelho, raio: largura * 0.14, segmentos: 4 });
      haste(m, { de: joelho, ate: pe, raio: largura * 0.1, segmentos: 4 });
    }
  }

  // Placas de carapaça: o Tank e os bosses precisam ler como blindados.
  for (let i = 0; i < placas; i++) {
    const t = placas <= 1 ? 0.5 : i / (placas - 1);
    caixa(m, {
      centro: [0, yCorpo + altura * 0.22, comprimento * (0.08 + t * 0.3)],
      tamanho: [largura * 1.5, altura * 0.12, comprimento * 0.14],
    });
  }

  // Saco de plasma/veneno no abdômen: é o que distingue o Ranged do Grunt à
  // distância, mais do que a cor.
  if (bolsa > 0) {
    elipsoide(m, {
      centro: [0, yCorpo + altura * 0.2, comprimento * 0.34],
      raios: [largura * 0.8 * bolsa, altura * 0.34 * bolsa, comprimento * 0.26 * bolsa],
      segmentos: 8,
    });
  }

  // Asas: placas finas em V. Planar em Y e não inclinadas — uma placa inclinada
  // exigiria matriz por peça, e a silhueta de cima é o que se vê de qualquer jeito.
  for (let i = 0; i < asas; i++) {
    const lado = i % 2 === 0 ? -1 : 1;
    const par = Math.floor(i / 2);
    elipsoide(m, {
      centro: [lado * largura * 1.3, yCorpo + altura * (0.18 + par * 0.08), comprimento * (0.16 - par * 0.18)],
      raios: [largura * 1.5, altura * 0.02, comprimento * 0.28],
      segmentos: 6,
    });
  }

  return m;
}

// ─────────────────────────────────────────────────────── modelos

/**
 * Corpo humanoide: torso, cabeça, dois braços e duas pernas.
 *
 * # Os raios são de gente, não de barril
 *
 * A primeira versão usava `ombro: 0.42`, que é **raio** — 84 cm de ombro a ombro
 * num soldado de 1,85 m. O resultado na vitrine foi uma figura duas vezes mais
 * larga que alta, lendo como caixa empilhada. As medidas abaixo são de um adulto
 * em forma: ombro 0,24 (48 cm), cintura 0,17, cabeça 0,13, braço 0,06, perna 0,08.
 * O ombro do Guerreiro é maior porque ele usa blindagem de assalto — e é o que o
 * distingue à distância, junto do escudo.
 */
function humanoide({ altura = 1.8, ombro = 0.24, cintura = 0.17, cabeca = 0.13, braco = 0.06, perna = 0.08, segmentos = 10 } = {}) {
  const m = malha();
  const pe = altura * 0.46;
  const topoTorso = altura * 0.86;

  lathe(m, [
    [cintura * 0.9, pe],
    [cintura, pe + altura * 0.06],
    [ombro, topoTorso - altura * 0.1],
    [ombro * 0.86, topoTorso],
  ], { segmentos });

  lathe(m, [[cabeca * 0.55, topoTorso], [cabeca * 0.5, topoTorso + altura * 0.02]], { segmentos });
  esfera(m, { centro: [0, topoTorso + altura * 0.11, 0], raio: cabeca, achatada: 1.05, segmentos });

  for (const lado of [-1, 1]) {
    lathe(m, [
      [braco, topoTorso - altura * 0.02],
      [braco * 0.9, pe + altura * 0.2],
      [braco * 0.7, pe + altura * 0.02],
    ], { centro: [lado * ombro * 1.02, 0, 0], segmentos: 6 });
  }

  for (const lado of [-1, 1]) {
    lathe(m, [
      [perna, pe],
      [perna * 0.9, altura * 0.24],
      [perna * 0.75, 0],
    ], { centro: [lado * cintura * 0.6, 0, 0], segmentos: 6 });
  }
  return m;
}

/**
 * Elmo da infantaria móvel: casco, crista e **viseira**.
 *
 * A viseira é o detalhe que separa "soldado" de "boneco": no filme o rosto não
 * aparece, e a faixa escura na frente é o que o olho reconhece a 20 m.
 */
function capacete(m, { y = 1.7, raio = 0.16 } = {}) {
  esfera(m, { centro: [0, y, 0], raio, achatada: 0.78 });
  // Crista: uma aleta baixa no topo, não uma placa em pé.
  caixa(m, { centro: [0, y + raio * 0.72, 0], tamanho: [0.035, raio * 0.3, raio * 1.45] });
  // Viseira: a faixa escura na frente do elmo é o que faz o rosto "sumir", como
  // no filme — e é o detalhe que distingue a infantaria de um boneco.
  caixa(m, { centro: [0, y + raio * 0.02, -raio * 0.95], tamanho: [raio * 1.25, raio * 0.5, 0.035] });
  // Protetor de nuca.
  caixa(m, { centro: [0, y - raio * 0.35, raio * 0.7], tamanho: [raio * 1.2, raio * 0.5, raio * 0.5] });
}

/**
 * Blindagem de infantaria: ombreiras, mochila e cinto.
 *
 * Sem elas o humanoide é um manequim com arma; com elas a silhueta fica mais
 * larga em cima, que é como o casaco blindado do filme lê de longe.
 */
function blindagem(m, { altura = 1.8, ombro = 0.24, cintura = 0.17 } = {}) {
  const topoTorso = altura * 0.86;
  const pe = altura * 0.46;
  for (const lado of [-1, 1]) {
    // Ombreira redonda e **rente** ao ombro. Uma elipsoide larga e rasa no topo do
    // torso lê como prateleira na vista isométrica, que é a única que o jogo tem.
    elipsoide(m, {
      centro: [lado * ombro * 0.95, topoTorso - altura * 0.045, 0],
      raios: [ombro * 0.44, altura * 0.075, ombro * 0.42],
      segmentos: 8,
    });
  }
  // Mochila: o volume nas costas que o filme usa para rádio e munição.
  caixa(m, {
    centro: [0, pe + altura * 0.42, cintura * 1.05],
    tamanho: [ombro * 1.35, altura * 0.24, cintura * 0.55],
  });
  caixa(m, { centro: [0, pe + altura * 0.03, 0], tamanho: [cintura * 1.9, altura * 0.045, cintura * 1.35] });
}

function fuzil(m, { y = 1.2, comprimento = 1.1, lado = 0.2 } = {}) {
  caixa(m, { centro: [lado, y, -comprimento * 0.35], tamanho: [0.05, 0.07, comprimento] });
  caixa(m, { centro: [lado, y - 0.05, -comprimento * 0.62], tamanho: [0.08, 0.13, 0.16] });
  // Carregador e coronha: a arma do filme é grande, e um cano fino sozinho lê
  // como graveto.
  caixa(m, { centro: [lado, y - 0.09, -comprimento * 0.18], tamanho: [0.07, 0.15, 0.13] });
  caixa(m, { centro: [lado, y - 0.02, comprimento * 0.18], tamanho: [0.06, 0.09, 0.19] });
}

/**
 * Posto avançado — a torre que o jogador defende.
 *
 * Deixou de ser um cristal flutuante: agora é um bunker com sacos de areia,
 * antena e um farol no topo. O GDD chama a torre de "núcleo de comando", e uma
 * posição militar cercada de pedras conta isso sem texto na tela.
 */
function torre() {
  const m = malha();
  // Bunker: octógono baixo e largo.
  lathe(m, [[3.1, 0], [3.2, 0.45], [2.7, 1.0], [2.2, 1.3]], { segmentos: 8 });
  // Sacos de areia no anel externo — seis volumes chanfrados.
  for (let i = 0; i < 8; i++) {
    const a = (i / 8) * Math.PI * 2 + Math.PI / 8;
    caixa(m, {
      centro: [Math.cos(a) * 3.3, 0.28, Math.sin(a) * 3.3],
      tamanho: [0.9, 0.56, 0.55],
    });
  }
  // Torre de comando.
  lathe(m, [[1.35, 1.3], [1.05, 1.7], [0.95, 4.0], [1.25, 4.6]], { segmentos: 10 });
  // Farol: o "coração" que a barra de HP representa.
  lathe(m, [[0, 4.6], [0.8, 5.25], [0, 5.9]], { segmentos: 6 });
  // Antena e prato.
  haste(m, { de: [0.9, 4.3, 0], ate: [1.9, 5.5, 0.3], raio: 0.07, segmentos: 4 });
  haste(m, { de: [-0.9, 4.3, 0.2], ate: [-1.6, 5.0, -0.4], raio: 0.05, segmentos: 4 });
  elipsoide(m, { centro: [1.95, 5.6, 0.32], raios: [0.45, 0.08, 0.45], segmentos: 8 });
  return m;
}

// ────────────────────────────────────────── aracnídeos (os inimigos)
//
// Cinco tipos comuns, quatro bosses e o Coronel. Cada um muda **silhueta**, não
// só cor: largura, comprimento, número de pernas, placas e bolsa. É o que faz o
// jogador distinguir um Tank de um Swarm a 30 m, onde a textura não chega.

function tanque() {
  return aracnideo({
    altura: 1.75,
    comprimento: 2.3,
    largura: 0.9,
    pernas: 3,
    corpo: 0.46,
    placas: 3,
    segmentos: 10,
  });
}

function plasma() {
  return aracnideo({
    altura: 2.7,
    comprimento: 3.0,
    largura: 0.78,
    pernas: 3,
    corpo: 0.58,
    bolsa: 1.25,
    chifre: 0.1,
    placas: 1,
    segmentos: 10,
  });
}

function cerebro() {
  const m = aracnideo({
    altura: 2.9,
    comprimento: 3.1,
    largura: 0.95,
    pernas: 3,
    corpo: 0.42,
    segmentos: 10,
  });
  // O cérebro exposto: a única parte macia de um exército de quitina.
  elipsoide(m, { centro: [0, 1.95, 0.15], raios: [1.15, 0.9, 1.25], segmentos: 14 });
  elipsoide(m, { centro: [0, 2.55, 0.05], raios: [0.7, 0.5, 0.8], segmentos: 10 });
  // Probóscide: o que ele enfia na cabeça dos soldados no filme.
  haste(m, { de: [0, 1.35, -0.9], ate: [0, 0.55, -1.7], raio: 0.16, segmentos: 6 });
  return m;
}

function rainha() {
  const m = aracnideo({
    altura: 3.4,
    comprimento: 4.0,
    largura: 1.05,
    pernas: 4,
    corpo: 0.5,
    chifre: 0.16,
    placas: 2,
    segmentos: 10,
  });
  // Coroa de espinhos: a silhueta que anuncia o boss final antes da barra.
  for (let i = 0; i < 5; i++) {
    const a = (i / 5) * Math.PI * 2;
    haste(m, {
      de: [Math.cos(a) * 0.22, 1.85, -1.3 + Math.sin(a) * 0.22],
      ate: [Math.cos(a) * 0.5, 2.9, -1.55 + Math.sin(a) * 0.5],
      raio: 0.08,
      segmentos: 4,
    });
  }
  return m;
}

function coronel() {
  const m = aracnideo({
    altura: 2.6,
    comprimento: 3.0,
    largura: 0.88,
    pernas: 3,
    corpo: 0.56,
    chifre: 0.24,
    placas: 2,
    segmentos: 10,
  });
  // Crista óssea e "insígnia": o nemesis tem de ser reconhecível no minimapa e
  // no meio da horda, e cor sozinha não basta quando ele está de costas.
  caixa(m, { centro: [0, 2.35, -0.55], tamanho: [0.1, 0.5, 0.9] });
  for (const lado of [-1, 1]) {
    haste(m, {
      de: [lado * 0.55, 2.1, 0.2],
      ate: [lado * 0.95, 2.55, 0.9],
      raio: 0.09,
      segmentos: 4,
    });
  }
  return m;
}

// ─────────────────────────────────────────────── catálogo de assets
//
// A tabela é de **silhueta**, não de gameplay: a altura final é a do modelo e o
// jogo escala por cima para casar com o raio de colisão.
export const MODELOS = {
  soldado: () => {
    const m = humanoide({ altura: 1.85, ombro: 0.25, cintura: 0.17, cabeca: 0.13, braco: 0.062, perna: 0.08 });
    blindagem(m, { altura: 1.85, ombro: 0.25, cintura: 0.17 });
    capacete(m, { y: 1.74, raio: 0.155 });
    fuzil(m, { y: 1.24, comprimento: 1.0, lado: 0.19 });
    return m;
  },
  guerreiro: () => {
    const m = humanoide({ altura: 2.05, ombro: 0.33, cintura: 0.23, cabeca: 0.13, braco: 0.09, perna: 0.105 });
    blindagem(m, { altura: 2.05, ombro: 0.33, cintura: 0.23 });
    capacete(m, { y: 1.93, raio: 0.16 });
    // Escudo no antebraço esquerdo: a habilidade 1 tem de existir no desenho.
    caixa(m, { centro: [-0.4, 1.32, -0.2], tamanho: [0.1, 0.78, 0.5] });
    fuzil(m, { y: 1.3, comprimento: 0.95, lado: 0.24 });
    return m;
  },
  sniper: () => {
    const m = humanoide({ altura: 1.85, ombro: 0.22, cintura: 0.16, cabeca: 0.125, braco: 0.055, perna: 0.072 });
    blindagem(m, { altura: 1.85, ombro: 0.22, cintura: 0.16 });
    capacete(m, { y: 1.74, raio: 0.15 });
    fuzil(m, { y: 1.28, comprimento: 1.55, lado: 0.17 });
    return m;
  },
  medico: () => {
    const m = humanoide({ altura: 1.8, ombro: 0.24, cintura: 0.17, cabeca: 0.13, braco: 0.06, perna: 0.078 });
    blindagem(m, { altura: 1.8, ombro: 0.24, cintura: 0.17 });
    capacete(m, { y: 1.7, raio: 0.15 });
    // Cruz no peito, na frente (-Z): é o símbolo que se lê de longe.
    caixa(m, { centro: [0, 1.33, -0.185], tamanho: [0.24, 0.085, 0.03] });
    caixa(m, { centro: [0, 1.33, -0.185], tamanho: [0.085, 0.24, 0.03] });
    return m;
  },

  // ── inimigos comuns ───────────────────────────────────────────────
  grunt: () => aracnideo({ altura: 1.65, comprimento: 2.0, largura: 0.52, pernas: 3, corpo: 0.52, chifre: 0.1 }),
  tank: tanque,
  ranged: () =>
    aracnideo({ altura: 1.95, comprimento: 2.35, largura: 0.48, pernas: 3, corpo: 0.6, bolsa: 0.95, chifre: 0.08 }),
  swarm: () => aracnideo({ altura: 0.95, comprimento: 1.0, largura: 0.34, pernas: 2, corpo: 0.48 }),
  flying: () => aracnideo({ altura: 1.6, comprimento: 1.6, largura: 0.42, pernas: 2, corpo: 0.7, asas: 4 }),

  // ── bosses ────────────────────────────────────────────────────────
  tanker: () =>
    aracnideo({
      altura: 3.0,
      comprimento: 3.6,
      largura: 1.15,
      pernas: 3,
      corpo: 0.5,
      placas: 4,
      chifre: 0.3,
      segmentos: 10,
    }),
  plasma,
  brain: cerebro,
  queen: rainha,
  coronel,
  torre,
};

// ────────────────────────────────────────────── cenário (planeta morto)
//
// O pedido do dono: "um planeta sem vegetação e com pedras espalhadas", com a
// torre como posto disputado. Nada aqui tem colisão — é silhueta e escala. A
// distribuição fica em `src/game/cenario.rs`, que é quem sabe onde o campo de
// batalha fica; o gerador só entrega as formas.
export const CENARIO = {
  // Bloco de rocha quebrado: seis faces irregulares, base enterrada.
  rocha: () => {
    const m = malha();
    lathe(m, [[0.85, 0], [1.0, 0.35], [0.82, 0.85], [0.45, 1.0]], { segmentos: 6 });
    caixa(m, { centro: [0.55, 0.35, -0.3], tamanho: [0.7, 0.7, 0.6] });
    caixa(m, { centro: [-0.5, 0.25, 0.45], tamanho: [0.6, 0.5, 0.55] });
    return m;
  },
  // Laje baixa: o chão rachado visto de cima, sem virar parede na frente da câmera.
  espigao: () => {
    const m = malha();
    lathe(m, [[0.42, 0], [0.34, 0.9], [0.2, 2.1], [0.02, 3.0]], { segmentos: 6 });
    caixa(m, { centro: [0.42, 0.35, 0.1], tamanho: [0.55, 0.7, 0.45] });
    return m;
  },
  // Cratera de impacto: bacia rasa com borda levantada. O jogador lê como "aqui
  // já caiu alguma coisa", que é a atmosfera de guerra sem um único sprite.
  cratera: () => {
    const m = malha();
    // 0,06 m de borda num raio de 1 m: 6 % do raio, que é o perfil de uma cratera
    // de impacto rasa. Mais que isso vira tigela e o inimigo atravessa a borda.
    lathe(m, [[1.0, 0.06], [0.94, 0.025], [0.7, 0.0], [0.3, 0.012], [0.0, 0.02]], {
      segmentos: 12,
      fechar: false,
    });
    return m;
  },
  // Mesa distante: silhueta de horizonte, colocada fora do campo jogável.
  mesa: () => {
    const m = malha();
    lathe(m, [[1.0, 0], [0.92, 0.35], [0.72, 0.82], [0.66, 0.95], [0.2, 1.0]], { segmentos: 9 });
    return m;
  },
  // Destroço: o que sobrou de uma nave. Caixa atravessada + chapa torta.
  destroco: () => {
    const m = malha();
    caixa(m, { centro: [0, 0.45, 0], tamanho: [1.7, 0.55, 0.9] });
    caixa(m, { centro: [0.85, 0.75, 0.15], tamanho: [0.6, 0.9, 0.5] });
    haste(m, { de: [0.6, 0.9, -0.35], ate: [1.7, 1.5, -0.6], raio: 0.09, segmentos: 4 });
    haste(m, { de: [0.3, 0.25, 0.4], ate: [-0.9, 0.05, 0.7], raio: 0.12, segmentos: 4 });
    return m;
  },
};

// ───────────────────────────────────────────────────── texturas (BMP)
//
// 64×64 com painéis de 8 px, emenda escura, faixa de acento e gradiente
// vertical. Uma cor chapada voltaria a ser um cubo pintado: o shader `textured`
// tem Lambert fixo, e é a variação de tom na textura que desenha o relevo.
const LADO_TEXTURA = 64;

/**
 * Regolito: o chão do planeta morto.
 *
 * É a única textura que **repete**: o chão tem 600 m e o UV é derivado do mundo
 * (ver `engine_bridge::chao`), então ela é desenhada dezenas de vezes por quadro.
 * Por isso o desenho é grão + pedrinhas esparsas, sem estrutura grande: uma
 * feição reconhecível (uma mancha, uma faixa) denunciaria a repetição.
 *
 * As pedrinhas são sorteadas com `% LADO`, que faz o desenho atravessar a borda
 * e a emenda sumir.
 *
 * # O contraste foi reduzido à metade (11/11/2026)
 *
 * Relato do dono: *"a textura do mapa está sobresaindo muito"*. Ela estava: grão
 * de ±11 e 120 pedrinhas de ±22..48 sobre um tile de 12 m. A 640×480 cada
 * pedrinha ocupa 3–8 px e há uma a cada metro — o chão ficava mais barulhento que
 * os personagens, e o olho lia a repetição do tile pelos calhaus brilhantes.
 *
 * O que muda o quanto ela "salta" é o **contraste**, não o tamanho do tile: um
 * tile maior só repete menos vezes, e a 12 m o limite é o interpolador
 * `mediump` do GLES 2 (UV acima de ~50 perde precisão e aparece uma grade).
 * Medido no quadro: ver DECISIONS.md (D-045).
 */
function texturaRegolito(semente) {
  const px = new Uint8Array(LADO_TEXTURA * LADO_TEXTURA * 3);
  let estado = (semente * 2654435761) >>> 0;
  const sorteio = () => {
    estado = (estado * 1664525 + 1013904223) >>> 0;
    return estado / 0xffffffff;
  };
  const base = [122, 105, 88];
  for (let y = 0; y < LADO_TEXTURA; y++) {
    for (let x = 0; x < LADO_TEXTURA; x++) {
      const grao = (sorteio() - 0.5) * 10;
      const i = (y * LADO_TEXTURA + x) * 3;
      px[i] = Math.max(0, Math.min(255, Math.round(base[0] + grao)));
      px[i + 1] = Math.max(0, Math.min(255, Math.round(base[1] + grao * 0.94)));
      px[i + 2] = Math.max(0, Math.min(255, Math.round(base[2] + grao * 0.86)));
    }
  }
  for (let pedra = 0; pedra < 44; pedra++) {
    const cx = Math.floor(sorteio() * LADO_TEXTURA);
    const cy = Math.floor(sorteio() * LADO_TEXTURA);
    const raio = 0.5 + sorteio() * 1.6;
    const claro = sorteio() < 0.42;
    const delta = claro ? 9 + sorteio() * 7 : -(7 + sorteio() * 6);
    for (let dy = -Math.ceil(raio); dy <= Math.ceil(raio); dy++) {
      for (let dx = -Math.ceil(raio); dx <= Math.ceil(raio); dx++) {
        if (dx * dx + dy * dy > raio * raio) continue;
        const x = (cx + dx + LADO_TEXTURA) % LADO_TEXTURA;
        const y = (cy + dy + LADO_TEXTURA) % LADO_TEXTURA;
        const i = (y * LADO_TEXTURA + x) * 3;
        for (let c = 0; c < 3; c++) {
          px[i + c] = Math.max(0, Math.min(255, Math.round(px[i + c] + delta)));
        }
      }
    }
  }
  return px;
}

function textura(paleta, semente) {
  const px = new Uint8Array(LADO_TEXTURA * LADO_TEXTURA * 3);
  // LCG com semente fixa por modelo. `Math.random()` faria dois `--conferir`
  // divergirem e o teste de determinismo reprovaria o próprio gerador.
  let estado = (semente * 2654435761) >>> 0;
  const sorteio = () => {
    estado = (estado * 1664525 + 1013904223) >>> 0;
    return estado / 0xffffffff;
  };

  for (let y = 0; y < LADO_TEXTURA; y++) {
    for (let x = 0; x < LADO_TEXTURA; x++) {
      const emenda = x % 8 === 0 || y % 8 === 0;
      const brilho = 0.82 + sorteio() * 0.18;
      let cor = paleta.base.map((c) => c * brilho);
      if (emenda) cor = paleta.escuro;
      const v = 1 - y / LADO_TEXTURA;
      if (v > 0.45 && v < 0.62 && ((x > 4 && x < 12) || (x > 52 && x < 60))) cor = paleta.acento;
      const tom = 0.78 + 0.4 * v;
      const i = (y * LADO_TEXTURA + x) * 3;
      px[i] = Math.min(255, Math.round(cor[0] * tom));
      px[i + 1] = Math.min(255, Math.round(cor[1] * tom));
      px[i + 2] = Math.min(255, Math.round(cor[2] * tom));
    }
  }
  return px;
}

/** BMP 24 bits, linhas de baixo para cima e alinhadas em 4 bytes. */
function bmp(largura, altura, rgb) {
  const linhaBytes = largura * 3;
  const padding = (4 - (linhaBytes % 4)) % 4;
  const tamanhoPixels = (linhaBytes + padding) * altura;
  const buf = Buffer.alloc(54 + tamanhoPixels);
  buf.write("BM", 0, "ascii");
  buf.writeUInt32LE(54 + tamanhoPixels, 2);
  buf.writeUInt32LE(54, 10);
  buf.writeUInt32LE(40, 14);
  buf.writeInt32LE(largura, 18);
  buf.writeInt32LE(altura, 22);
  buf.writeUInt16LE(1, 26);
  buf.writeUInt16LE(24, 28);
  buf.writeUInt32LE(tamanhoPixels, 34);
  let deslocamento = 54;
  for (let y = altura - 1; y >= 0; y--) {
    for (let x = 0; x < largura; x++) {
      const i = (y * largura + x) * 3;
      buf[deslocamento++] = rgb[i + 2];
      buf[deslocamento++] = rgb[i + 1];
      buf[deslocamento++] = rgb[i];
    }
    for (let p = 0; p < padding; p++) buf[deslocamento++] = 0;
  }
  return buf;
}

// ────────────────────────────────────────────────────── OBJ

function obj(m, nome) {
  const linhas = [`# ${nome} — gerado por scripts/gerar-modelos.mjs`, `o ${nome}`];
  for (const [x, y, z] of m.v) linhas.push(`v ${fmt(x)} ${fmt(y)} ${fmt(z)}`);
  for (const [u, v] of m.vt) linhas.push(`vt ${fmt(u)} ${fmt(v)}`);
  for (const [x, y, z] of m.vn) linhas.push(`vn ${fmt(x)} ${fmt(y)} ${fmt(z)}`);
  for (const [a, b, c] of m.f) {
    const ia = a + 1;
    const ib = b + 1;
    const ic = c + 1;
    linhas.push(`f ${ia}/${ia}/${ia} ${ib}/${ib}/${ib} ${ic}/${ic}/${ic}`);
  }
  return linhas.join("\n") + "\n";
}

/** Três casas: milímetro é o que a malha precisa, e o arquivo encolhe. */
function fmt(valor) {
  const arredondado = Math.round(valor * 1000) / 1000;
  return Object.is(arredondado, -0) ? "0" : String(arredondado);
}

// ─────────────────────────────────────────────────────── execução

export function gerar() {
  const arquivos = new Map();
  let semente = 1;
  for (const [nome, construir] of Object.entries({ ...MODELOS, ...CENARIO })) {
    const m = construir();
    arquivos.set(join("modelos", `${nome}.obj`), Buffer.from(obj(m, nome), "utf8"));
    const paleta = PALETAS[nome] ?? PALETAS.soldado;
    arquivos.set(join("texturas", `${nome}.bmp`), bmp(LADO_TEXTURA, LADO_TEXTURA, textura(paleta, semente++)));
  }
  // O chão é a única textura sem modelo: ele é um plano com UV de mundo,
  // desenhado por `engine_bridge::chao`, e não um `.obj` do catálogo.
  arquivos.set(join("texturas", "terreno.bmp"), bmp(LADO_TEXTURA, LADO_TEXTURA, texturaRegolito(7)));
  return arquivos;
}

/**
 * Altura medida de cada malha, como bloco pronto para `cena::altura_do_arquivo`.
 *
 * A tabela em Rust existe porque a matriz de instância precisa da escala sem
 * GPU; se ela envelhece, o personagem afunda ou flutua. Imprimir as medidas do
 * próprio gerador é o que torna a tabela atualizável em um comando, em vez de
 * copiar números de um teste que falhou.
 */
function medidas() {
  const linhas = [];
  for (const [nome, construir] of Object.entries({ ...MODELOS, ...CENARIO })) {
    const m = construir();
    const ys = m.v.map((v) => v[1]);
    const altura = Math.max(...ys) - Math.min(...ys);
    linhas.push(`        "${nome}" => ${altura.toFixed(3)},`);
  }
  console.log(linhas.join("\n"));
}

function principal() {
  if (process.argv.includes("--medidas")) {
    medidas();
    return;
  }
  const conferir = process.argv.includes("--conferir");
  const arquivos = gerar();
  let divergentes = 0;

  for (const [relativo, conteudo] of arquivos) {
    const destino = join(RAIZ, "assets", relativo);
    if (conferir) {
      if (!existsSync(destino)) {
        console.error(`FALTA ${relativo}`);
        divergentes++;
      } else if (!readFileSync(destino).equals(conteudo)) {
        console.error(`DIVERGE ${relativo}`);
        divergentes++;
      }
      continue;
    }
    mkdirSync(dirname(destino), { recursive: true });
    writeFileSync(destino, conteudo);
  }

  if (conferir) {
    if (divergentes > 0) {
      console.error(`\n${divergentes} arquivo(s) divergente(s) — rode sem --conferir`);
      process.exit(1);
    }
    console.log(`ok   ${arquivos.size} arquivo(s) idêntico(s) ao gerador`);
    return;
  }

  const modelos = [...arquivos.keys()].filter((n) => n.startsWith("modelos")).length;
  console.log(`ok   ${modelos} modelo(s) e ${arquivos.size - modelos} textura(s) em assets/`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) principal();
