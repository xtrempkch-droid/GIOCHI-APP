# Diagnóstico do protótipo do launcher

Análise do `launcher.html` (protótipo em 1280×720) antes da reescrita em Rust.

**Conclusão curta:** o protótipo não é um launcher quebrado — é um **mockup de
interface com teclado**. A arquitetura conceitual é boa e foi aproveitada; o que
não existia é justamente o que caracteriza um launcher de console.

## O que o protótipo acertou

1. **Estado + reducer.** `reduce(state, cmd)` com estado único é a escolha certa e
   foi mantida.
2. **Foco como (zona, índice).** Modelo correto para navegação espacial com
   controle. Mantido, com zona virando `enum`.
3. **Pilha de navegação** com restauração de foco ao voltar. Mantido, com as abas
   separadas dos overlays.
4. **Identidade visual.** A paleta, o carrossel com cartão centralizado e o painel
   de ajustes rápidos estão definidos e servem de referência para a UI em Rust.

## Defeitos encontrados

### 1. Escala corta a interface em 4:3 — o alvo RK3326

```js
const scale = Math.min(vw/960, vh/720);        // 960 = área segura
c.style.left = ((vw - 1280*scale)/2) + 'px';   // 1280 = canvas
```

Duas larguras diferentes no mesmo cálculo. Em 640×480: `scale = 0.667`,
`left = -106px` → **106 px cortados de cada lado**, comendo a área segura.

**Correção:** `Viewport::fit` usa `DESIGN_WIDTH` para escala **e** centralização,
com letterbox. Testado em 1280×720, 640×480, 1920×1080 e 640×720.

### 2. Nenhum código de gamepad

Não há uma linha de `navigator.getGamepads()`. A entrada é só teclado
(`KEYMAP`) e ponteiro. O requisito "100% controlado por gamepad" **não estava
implementado** — é a maior lacuna, não um detalhe.

**Correção:** `crates/core/src/gamepad.rs`, com borda de pressão nos botões e
repetição com atraso nas direções.

### 3. Estado inconsistente: três booleanos independentes

`booting`, `sleeping` e `shutdown` são campos separados e nada impede dois de
serem verdadeiros ao mesmo tempo. `renderOverlays` desenha os três overlays em
ordem fixa, então o resultado é uma tela por cima da outra.

Caso concreto: `power-actions` com índice 1 dispara

```js
showToastEffect({ ...state, stack:state.stack.slice(0,-1), shutdown:true }, 'Reiniciando...')
```

Mostra o toast **"Reiniciando..."** e o overlay **"Sistema desligado"** juntos.

**Correção:** um único `Screen { Boot, Running, Sleeping, Shutdown }`. Estados
mutuamente exclusivos por construção.

### 4. `stack[0]` assumido como aba

`renderTopBar` e `renderMain` leem `State.stack[0].id` e tratam como aba. Funciona
porque overlays são empurrados no fim, mas qualquer `push` fora de ordem quebra a
tela inteira sem aviso.

**Correção:** `tab: Tab` e `overlays: Vec<Overlay>` são campos distintos. O estado
inválido não é representável.

### 5. Grade da biblioteca com altura fixa

`.grid` tem `grid-template-rows: repeat(2,1fr)`. Com 8 jogos, funciona. Com 5, os
cartões esticam; com 12, a segunda linha estoura o container.

**Correção:** paginação calculada a partir da contagem real; a UI desenha o que
couber e o núcleo sabe quantas páginas existem.

### 6. Filtro "Recentes" não é recente

```js
const filteredGames = s => {
  if (s.libraryFilter === 'recent') return [...s.games].sort((a,b) =>
    GAMES_INIT.findIndex(g=>g.id===a.id) - GAMES_INIT.findIndex(g=>g.id===b.id));
```

Ordena pela posição no vetor original — ou seja, pela ordem de cadastro. É
estável, mas não tem relação com "recente".

**Correção:** ordenar por `last_played_at`; o vetor já é mantido nessa ordem.

### 7. Dados falsos apresentados como reais

| O que aparece | De onde vem |
| --- | --- |
| Bateria 89% | `const bat = 89` |
| FPS 56–60, temp 48–56 °C, TDP 5–8 W | `Math.random()` |
| Wi-Fi "Gamer_5G • Excelente" | string fixa |
| 8 jogos (Hollow Knight, Celeste, Hades…) | lista inventada, nenhum no catálogo |

Isso não é só cosmético: um número aleatório de bateria é **pior que ausente**,
porque o usuário toma decisão com informação falsa.

**Correção:** `TickHud` é `no-op` até existir leitura real. A UI deve mostrar
"—" quando não há dado.

### 8. "Iniciando…" não inicia nada

`confirmAction` no carrossel e nos apps devolve apenas um toast. Não há leitura
de `/apps`, nem `spawn` de processo, nem mapeamento de `APP:<id>`.

**Correção:** pendente — exige a camada de integração com o sistema.

### 9. Ícones em emoji

`{ id:'retro', icon:'🎮' }`, `'📁'`, `'🌐'`, `'🛒'`. Em console com GLES e sem
fonte de emoji, isso desenha retângulo vazio. O campo virou **nome de ícone do
tema**.

### 10. Alvos de toque minúsculos

`.hint kbd` tem 15 px de altura; `.chip` tem 4 px de padding; `.tab` 7 px. Para
dedo, o mínimo praticável é ~44 px. O suporte a toque era "um plus", mas do jeito
que estava, não era utilizável.

**Correção:** `Viewport::MIN_TOUCH_TARGET = 44.0`, com teste que reprova alvo
pequeno.

### 11. Zona desconhecida trava a navegação em silêncio

```js
const zoneCount = (s, zone) => { ...; return 0; };
```

Qualquer zona não listada devolve 0, e `navigate` retorna o estado inalterado. O
usuário aperta o direcional e nada acontece, sem erro.

**Correção:** `Zone` é `enum`, e `zone_len` casa exaustivamente — esquecer uma
zona é erro de compilação.

## Contagem

| Categoria | Itens |
| --- | --- |
| Defeitos corrigidos no núcleo Rust | 7 (itens 1–7, 9–11) |
| Pendentes de camada de UI ou integração | 2 (itens 8 e UI) |
| Testes no núcleo | 40, todos sem hardware |

## Próximo passo: escolher a stack de UI

O núcleo está pronto e testável. Falta a renderização, e ela depende de uma
decisão que é caro reverter:

| Stack | A favor | Contra |
| --- | --- | --- |
| **SDL2 + renderer próprio** | mesma stack do antecessor (SDL2 + KMSDRM já no Buildroot); controle total do desenho; GLES2 via SDL | escrever a UI do zero |
| **Slint** | feito para embedded, tem renderer GLES e software; menos código de UI | DSL própria; mais uma dependência grande |
| **egui** | iteração rápida em Rust puro | desenho custom (gradiente, glow) fica limitado; integração KMS/DRM não é o foco |

**Recomendação: SDL2 + renderer próprio.** Motivo: o alvo inclui Mali-G31 com
GLES 2.0 e 1 GB de RAM; SDL2 já está no Buildroot do antecessor e o backend
KMSDRM dispensa compositor. O custo é escrever o desenho, mas o layout é simples
(cartões, gradientes, texto) e não justifica carregar um toolkit inteiro.
