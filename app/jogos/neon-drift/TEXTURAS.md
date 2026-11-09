# Texturas e sprites do Neon Drift — guia para desenhar as suas

> Este documento é para quem vai **pintar** textura e sprite do jogo. Ele diz o
> tamanho exato, o que cada pixel significa e onde o arquivo entra. Os números
> aqui não são recomendação: são os que o código lê hoje, e mudá-los sem mudar o
> código faz a textura aparecer esticada, tremida ou invisível.

## Regra que vale para tudo: PNG não carrega

O aparelho **não decodifica PNG**. Medido no R35S: o `libSDL2_image` foi
compilado sem `--enable-png` e o `IMG_Load` responde *"PNG images are not
supported"* (ver `AGENTS.md` §24). Por isso:

| Onde | Formato aceito |
| --- | --- |
| texturas de superfície | **só as sete geradas em código** — não há carregamento externo |
| sprites de partícula | **BMP** 24 ou 32 bits, sem compressão (`GIOCHI_SPRITES`) |
| captura e conferência | BMP (`--texturas`, `--atlas`, `--quadro`) |

Um PNG no asset não abre no aparelho: o jogo sobe sem a textura e sem erro.
Quando quiser conferir uma arte, exporte como BMP.

---

## Parte 1 — Texturas de superfície

### A ficha técnica

| Item | Valor | Por quê |
| --- | --- | --- |
| tamanho | **64×64 px** | `texturas::LADO`; potência de dois é exigência do GLES 2.0 para `REPEAT` — 96×96 funciona no Mesa e falha em driver embarcado |
| canais | **RGBA8**, 4 bytes por pixel | 16 384 bytes por textura |
| escala no mundo | **1 textura = 3,0 m** | `malha::METROS_POR_TILE` |
| endereçamento | `REPEAT` | o chão tem 2 081 m: a textura repete ~694 vezes |
| filtro | `LINEAR` | sem mipmap; a 640×480 é o que dá conta |
| cor | **cinza-claro e neutro** | a cor final é `amostra × tint` (ver abaixo) |
| UV | **por posição no mundo** | não 0..1 por quadrilátero |

### A regra do cinza-claro, e por que ela existe

A textura **não** traz a cor do material: ela traz o **relevo**, e o `tint` traz a
cor. `final = amostra × tint`.

Isso foi aprendido errado na primeira tentativa. As texturas nasceram escuras, no
tom do material (asfalto a 0,06), e o `tint` também era escuro: escuro × escuro =
preto. A serra ficou com **28 % do quadro quase preto**. A correção foi clarear a
**textura**, não o `tint` — porque o `tint` é o que muda de mapa para mapa e a
textura é compartilhada.

Consequência prática para quem pinta:

- **faixa útil: 0,45 a 0,95 de luminância.** Abaixo de 0,35 o material afunda no
  preto noturno; acima de 0,98 ele estoura e perde o desenho.
- **contraste dentro da textura** é o que vira relevo. Um asfalto liso de 0,70
  uniforme desenha um plano chapado; granulado entre 0,55 e 0,80 desenha
  asfalto.
- **nunca pinte a cor do material.** Verde de mata na textura × tint verde =
  verde duas vezes, e o resultado é uma massa preta esverdeada.

Os `tint` em uso hoje (`cena3d.rs`):

| Material | `tint` | Textura |
| --- | --- | --- |
| asfalto | `0.155 0.160 0.215` | `asfalto` |
| concreto | `0.44 0.46 0.56` | `concreto` |
| pedra | `0.52 0.50 0.48` | `rocha` |
| mata | `0.34 0.60 0.34` | `mata` |
| guarda-corpo | `0.62 0.64 0.70` | `guarda_corpo` |
| painel de neon | `1.0 0.55 0.92` | `painel` |
| xadrez da largada | `0.95 0.95 1.0` | `xadrez` |

### As sete texturas e onde cada uma aparece

| Textura | O que desenha | Regra de desenho |
| --- | --- | --- |
| `asfalto` | o piso inteiro | granulado fino + emendas ocasionais; **sem faixa pintada** (a faixa central é geometria separada) |
| `concreto` | mureta, laje de passarela, parede de túnel, torre | manchas largas e juntas a cada 16 px; é o material mais visto de perto |
| `rocha` | paredão da serra | estratos horizontais irregulares, claros e escuros alternados |
| `mata` | copa de árvore vista de longe | massas verdes com pontos claros (reflexo de folha); **não é folha**, é copa |
| `guarda_corpo` | chapa do guarda-corpo da serra | nervura vertical a cada 8 px, sombra de um lado e luz do outro — passando depressa é metade da sensação de velocidade |
| `painel` | outdoor de neon | gradiente diagonal + grade fina; o `tint` é forte, então aqui a textura deve ser **clara** |
| `xadrez` | faixa de largada | 8×8 quadrados alternados, puro preto e branco |

### O UV vem do mundo, não do quadrilátero

`chao3` calcula o UV da **posição no mundo** dividida pelo `tile`. Com UV 0..1 por
segmento, a textura reiniciaria a cada 4 m de asfalto e a emenda apareceria como
uma faixa correndo ao lado do carro. É por isso que o piso é **uma** malha de
2 560 vértices em vez de 480 quadriláteros independentes.

`caixa_texturizada` faz o mesmo por face (`largura / tile`), e `parede3` estica ao
longo da tira. Quem pinta não precisa saber disso — só precisa saber que a
textura é **contínua por 3 m** e que a escala é sempre a mesma em qualquer malha.

### Como conferir o que você fez

```bash
giochi-neon-drift --texturas /tmp/texturas.bmp
```

Sai uma tira horizontal 480×72 com as sete texturas de 64×64 na ordem de
`texturas::todas()`, com 4 px de separação e fundo cinza-escuro. É o molde: pinte
dentro da célula de 64×64 e a densidade já está certa.

Para olhar uma superfície no jogo, isole a malha:

```bash
giochi-neon-drift --mapa boavista --tela corrida --piloto --passos 600 \
                  --so asfalto_t --quadro /tmp/asfalto.bmp
```

O `--so` aceita `asfalto_t`, `concreto_t`, `pedra_t`, `mata_t`, `guarda_t`,
`painel_t`, `xadrez_t` (nome da malha) e `asfalto`, `concreto`… (nome da textura,
via `GIOCHI_SEM`).

### Onde mexer no código

| Quero… | Arquivo | O que fazer |
| --- | --- | --- |
| mudar o desenho de uma textura | `crates/ui-sdl/src/texturas.rs` | reescrever a função (`asfalto()`, `mata()`…) |
| mudar a cor de um material | `crates/ui-sdl/src/cena3d.rs` | a constante `ASFALTO`, `CONCRETO`… |
| mudar a escala no mundo | `crates/ui-sdl/src/malha.rs` | `METROS_POR_TILE` — afeta **todas** as texturas |
| acrescentar uma textura | `texturas.rs` + o `struct TexturasGpu` em `cena3d.rs` | função nova, campo novo, `criar()` e um braço no laço de desenho |
| trocar a geometria que a usa | `crates/ui-sdl/src/malha.rs` | `chao3`, `caixa_texturizada`, `parede3` |

**Orçamento:** textura não custa chamada de desenho — custa área de tela. As sete
somadas ocupam 114 688 bytes na GPU e nenhum draw call extra. O que pesa é
**quantos pixels do quadro** cada superfície cobre: o asfalto é o quadro inteiro
embaixo do carro, e por isso ele é o material com maior orçamento de detalhe.

---

## Parte 2 — Sprites de partícula

### A ficha técnica

| Item | Valor |
| --- | --- |
| atlas | **128×128 px** |
| grade | **4×4 células** de 32×32 (só as 8 primeiras são usadas) |
| canais | **RGBA8** |
| RGB | **branco em toda parte** (`255,255,255`) |
| alfa | é o que desenha |
| margem interna | **1e-4** de encolhimento por célula |

### Branco, não cinza: a cor vem do `tint`

Uma nuvem de fumaça nasce branca, fica cinza e some — sem uma textura por cor. O
que muda é o **tint** da instância e a **faixa de vida** (o alfa de desenho), não
o pixel. Se você pintar RGB colorido, o tint multiplica e a cor sai dobrada.

O RGB **não** é lixo: ele existe para que a folha externa de 24 bits possa ser
lida (ver Parte 3). Internamente só o `A` importa.

### A margem de 1e-4, e o risco que ela evita

O retângulo de UV de cada célula é encolhido 1e-4 para dentro. Sem isso, a
amostragem `LINEAR` na borda puxa o texel da **célula vizinha** e cada sprite
ganha um risco reto atravessando a forma — visível como uma linha de 1 px que
aparece e some conforme a partícula gira.

Ao pintar uma célula, mantenha ~1 px de margem transparente em volta. É a mesma
proteção pelo lado da arte.

### As oito formas

| Índice | Forma | Onde aparece |
| --- | --- | --- |
| 0 | `puff` | fumaça de pneu no drift — a única que **desbota** |
| 1 | `gota` | respingo de chuva no asfalto |
| 2 | `faisca` | raspão na mureta e batida |
| 3 | `risco` | rastro de nitro (esticado ao longo da velocidade) |
| 4 | `chuva` | gota caindo (esticada na velocidade relativa) |
| 5 | `halo` | mote de neon flutuando na cidade |
| 6 | `borrao` | marca de pneu deitada no chão |
| 7 | `anel` | reservada (fragmento de vidro) |

A ordem é **contrato**: o índice é o que a partícula guarda no estado e o que o
desenho usa para escolher a célula. Trocar duas formas de lugar troca a arte de
todos os efeitos que as usam.

### A forma certa para o espaço certo

- **redonda e cheia** (`puff`, `borrao`, `halo`): desenhada a 1,0× — já encosta na
  borda da célula.
- **redonda e pequena** (`gota`): o raio útil é **0,72 do meio-quadrado**, e o
  desenho a amplia **1,30×** para compensar. Se você pintar uma gota que já ocupa
  a célula inteira, ela sai 30 % maior que as outras.
- **alongada** (`risco`, `chuva`, `faisca`): o jogo estica o quadrilátero ao longo
  da velocidade, então pinte-a **fina e centrada**; o alongamento é do shader.

### Como conferir

```bash
giochi-neon-drift --atlas /tmp/atlas.bmp
```

O fundo sai **magenta**, de propósito: alfa 0 sobre preto é indistinguível de
"não desenhou". Se a célula aparecer magenta inteira, a forma está vazia; se
aparecer com borda dura, você esqueceu de suavizar o alfa.

Prova de que o sprite está sendo **amostrado** (e não desenhado como quadrado):

```bash
GIOCHI_SEM=mundo,carros,hud giochi-neon-drift --tela corrida --piloto \
    --passos 400 --quadro /tmp/part.bmp
```

Um quadrilátero sólido dá 2 ou 3 níveis de luminância e ~0 % de rampa. O atlas
procedural, medido nesse comando, dá **114 níveis e 86,2 % de pixels em rampa**
(o número exato varia com o quadro; o que importa é a ordem de grandeza — dezenas
de níveis, não dois). Se o seu dá 3, o alfa está binário.

### Onde mexer no código

| Quero… | Arquivo |
| --- | --- |
| mudar o desenho de uma forma | `crates/ui-sdl/src/atlas.rs`, a função `alfa_da_forma` |
| mudar o ganho de tamanho | `atlas.rs`, `Forma::ganho` |
| mudar o UV da célula | `atlas.rs`, `Forma::retangulo` |
| trocar qual forma cada partícula usa | `crates/ui-sdl/src/particulas.rs`, `Marca` |
| mudar cor, gravidade, arrasto | `particulas.rs`, a tabela por tipo |

---

## Parte 3 — Usar a sua própria folha de sprites

```bash
GIOCHI_SPRITES=minha-folha.bmp GIOCHI_SPRITES_COLUNAS=8 giochi-neon-drift
```

O jogo lê o BMP, **normaliza** para o mesmo atlas 128×128 de 4×4 e segue como se
fosse o procedural. Depois da normalização, `retangulo()`, `quad_celula()` e o
desenho instanciado não sabem de onde os pixels vieram — não existe um segundo
caminho de desenho para envelhecer diferente.

### Formato do arquivo

| Item | Aceito | Recusado |
| --- | --- | --- |
| formato | BMP `BITMAPINFOHEADER` (`40` bytes) | qualquer outro cabeçalho |
| compressão | **nenhuma** (`BI_RGB`) | `BI_RLE8`, `BI_RLE4`, `BI_BITFIELDS` |
| bits | **24 ou 32** | 8, 4, 1 |
| altura | positiva (de baixo para cima) ou **negativa** (de cima para baixo) | zero |
| alinhamento | o leitor trata as linhas alinhadas em 4 bytes | — |
| grade | `colunas` × `ceil(8 / colunas)` | mais formas que células |

`GIOCHI_SPRITES_COLUNAS` padrão é **4**. Uma tira 8×1 (`256×32`) entra igual a
uma grade 4×2 (`128×64`): a célula *i* da folha vai para a posição da forma *i*.

### 24 bits não tem alfa — a luminância é a máscara

Num BMP de 32 bits o alfa vem junto. Num de 24 bits não existe, e a
**luminância** vira opacidade: **branco opaco, preto transparente** — a convenção
das folhas de efeito. Sem isso, todo sprite de uma folha 24 bits sairia como
retângulo cheio.

Pratique com fundo preto e desenhe a forma em branco. Escala de cinza no meio vira
semi-transparência, que é o que faz a borda da fumaça desbotar em vez de recortar.

### Quando a folha é ignorada, o motivo aparece

```text
neon-drift: folha de sprites ignorada — não consegui abrir /tmp/x.bmp: No such file or directory
neon-drift: atlas de sprites 128x128 [...] (origem: procedural)
```

Cair em silêncio faria você concluir que a sua folha não serve, quando o que
falhou foi o caminho, o formato ou o número de colunas.

E `--atlas` respeita as duas variáveis: ferramenta que mostra o procedural
enquanto o jogo desenha a folha do usuário mede outra coisa.

### Três armadilhas de BMP, todas com teste

| Armadilha | Sintoma |
| --- | --- |
| altura **negativa** = de cima para baixo | folha de cabeça para baixo (invisível num puff, óbvio num risco) |
| linhas alinhadas em 4 bytes | imagem **cisalhada**, cada linha deslocada um pouco mais |
| BMP comprimido | aceitar e ler como cru desenha lixo |

---

## Parte 4 — O que **não** existe hoje, para não perder tempo

Não é falta de aviso: está declarado aqui para ninguém procurar o recurso.

- **Não há carregamento de textura de superfície externa.** As sete são geradas
  em código; um PNG/BMP em `assets/` não é lido. Quem quiser trocar o asfalto
  edita `texturas::asfalto()`.
- **Não há mipmap.** Textura a 64×64 num chão que recua ao horizonte produz
  *aliasing* longe da câmera. Resolver exige `GL_LINEAR_MIPMAP_LINEAR` e geração
  de mip, que a engine não expõe hoje.
- **Não há folha de sprite com mais de 8 formas.** `Forma::TODAS` tem 8 e o atlas
  é 4×4; uma folha com 16 formas entraria nas células certas, mas o jogo só sabe
  pedir as 8.
- **Não há textura animada** (sprite sheet de 2 quadros, por exemplo). A célula é
  geometria (UV embutido no quadrilátero), não uniforme — animar exigiria trocar
  o UV por quadro.

---

## Resumo de uma linha por arquivo

```text
texturas.rs   64x64 RGBA, cinza-claro, cor vem do tint — 7 materiais
atlas.rs      128x128, grade 4x4 de 32px, RGB branco, alfa desenha — 8 formas
malha.rs      METROS_POR_TILE = 3.0; chao3/caixa_texturizada/parede3 fazem o UV
cena3d.rs     os tints (ASFALTO, CONCRETO, ...) e qual malha usa qual textura
particulas.rs qual forma cada tipo de partícula usa, e cor/gravidade/arrasto
```
