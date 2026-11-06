# Testar esta versão no X35H

> Escrito em 04/11/2026, atualizado em 06/11/2026.
> O X35H esteve fora da rede em todo o trabalho até agora.
> **Nada do que está aqui foi verificado no X35H — só no X55.**

---

## FEITO: detecção automática de jogos (06/11/2026)

Era a prioridade pedida em 04/11. Está implementado e medido no X55.

```bash
bash scripts/aparelho.sh catalogo        # gera, confere no aparelho e publica
node scripts/gerar-catalogo.mjs --lista  # quem entra e quem fica de fora
```

O catálogo deixou de ser JSON escrito à mão e virou **projeção** de
`app/apps.json` + os `metadata.json`.

**Prova de que é projeção e não cópia:** removi o binário do `cobrinha` do
aparelho e o comando reduziu o catálogo de 3 para 2, dizendo o que tirou e como
instalar; restaurei e voltou a 3. Reflete remoção, não só adição.

Uma descoberta do próprio gerador: **o launcher entrava no seu próprio
catálogo** — tem `status: ready`, `distribution: image` e binário no lugar. O
cartão "Launcher" abriria um segundo launcher e os dois brigariam pelo DRM. Agora
o papel é declarado (`role: "shell"`), não inferido do nome.

Detalhes e as cinco armadilhas em [`AGENTS.md`](../AGENTS.md) §12.

### O que sobrou de manual

Ainda é preciso **instalar** o jogo antes de publicar o catálogo:

```bash
bash scripts/compilar-jogo-arm64.sh --jogo <nome> --instalar 10.55.0.2
bash scripts/aparelho.sh catalogo
```

Dois comandos em vez de um. Dá para juntar, mas são operações diferentes
(compilar é do PC, instalar é do aparelho) e falham por motivos diferentes —
separar ajuda o diagnóstico. Se virar atrito, o próximo passo é um
`aparelho.sh publicar` que encadeia os dois.

---

## O BOOT AUTOMÁTICO JÁ FOI VERIFICADO (06/11/2026)

O item 1 do roteiro abaixo **já passou**, no X55:

```text
giochi-init     pid=216  ppid=1      <- ::respawn: do inittab
giochi-launcher pid=224  ppid=216    <- filho do init
```

O launcher subiu sozinho 29 s depois do boot, sem SSH. Quadro: 351 cores,
0,00% de preto, 99,5% de acerto, texto 0,99.

**Falta repetir no X35H** — mesmo `inittab`, mas a tela é outra.

---

## Por que este documento existe

O X55 e o X35H divergem em três coisas que mexem justamente no que mudou hoje.
Testar num aparelho e concluir pelo outro é o erro que já custou sessões neste
repositório.

| | X55 | X35H |
| --- | --- | --- |
| Painel | 720×1280 **retrato** | 640×480 **paisagem** |
| `fbcon/rotate` | `3` → `Ccw90` | `0` → `Nenhuma` |
| RAM | 1967 MB | 964 MB |
| Imagem | 04/11 21:04 | **a conferir** |

## O risco principal: o texto toma um caminho diferente

A correção de hoje fez o texto girar com a interface. E ela tem **dois ramos**:

```rust
Rotation::None      => canvas.copy(...)                  // X35H cai aqui
Cw90 | Ccw90        => canvas.copy_ex(..., 270.0, ...)   // X55 cai aqui
```

**O ramo que o X55 exercita é o que tem `copy_ex`. O X35H exercita o outro.** O
bug de hoje nasceu exatamente de uma diferença assim — e o `copy_ex` que eu
tinha posto primeiro encolhia a fonte de um jeito que o percentual de acerto não
via.

Então: **o texto do X35H pode estar certo ou pode ter regredido sem ninguém
notar.** Testar é obrigatório.

### Como saber, sem achismo

Rode no PC a referência paisagem e compare com a captura do aparelho:

```bash
bash scripts/aparelho.sh verificar
```

Duas linhas importam:

```text
comparação: 99.x% de acerto
texto: orientação ok (linhas 0.9x)      <- novo, de hoje
```

Se aparecer `ATENÇÃO: o texto parece girado 90°`, a correção não vale para o
ramo sem rotação. Se `linhas` vier abaixo de ~0,85, investigar antes de seguir.

**Limite conhecido da medida:** ela compara a captura com a referência do PC. No
X35H a referência é gerada em 640×480 e a captura também — mesmo espaço, sem
des-rotacionar. Se a fonte estiver com tamanho errado nos dois lados, a
correlação passa e o problema não aparece. Por isso o item abaixo.

### A conferência que a ferramenta não faz

Contar pixels claros nos dois lados e comparar a **razão**. Foi assim que o bug
do `copy_ex` apareceu (611 contra 1275):

```bash
# referência no PC, 640x480
SDL_VIDEODRIVER=dummy ./app/launcher/target/release/giochi-launcher \
  --tamanho 640x480 --dump-frame /tmp/ref-640.bmp
# e no aparelho
bash scripts/aparelho.sh verificar     # salva dist/testes/<data>/frame.bmp
```

Razão muito abaixo de 0,8 = fonte encolhida. Muito acima = fonte grande demais.

## Roteiro

### 1. Boot sozinho — **nunca verificado em aparelho nenhum**

O `::respawn:/usr/bin/giochi-init` existe no artefato (conferido com `debugfs`)
e o `giochi-init` está vivo no X55 desde o boot. Mas **a UI nunca foi vista
subindo sozinha**: em todas as rodadas ela foi iniciada por SSH.

```bash
# desligar, ligar, e NÃO fazer ssh
# esperar ~40 s
```

Depois, por SSH, conferir que os dois processos existem e que o launcher desenha:

```bash
bash scripts/aparelho.sh verificar
```

Se o launcher estiver vivo mas o quadro for preto, o `kmsdrm` abriu antes do
conector estabilizar — diagnóstico diferente de "não subiu".

### 2. Orientação e texto

Ver acima. É o item com maior chance de ter regredido.

### 3. Os três jogos

Todos os três rodam no X55 com `projeto 1280x720 (Ccw90)`. No X35H o projeto é
640×480 — **outro espaço**, com a mesma lógica proporcional mas números
diferentes.

| Jogo | O que olhar | Risco específico |
| --- | --- | --- |
| `volley` | faixa preta, raquete, placar | já rodou no X35H em versão anterior; não deve mudar |
| `cobrinha` | **legibilidade das células** | 24 colunas em 640 px = célula de ~25 px; em 480 de altura a grade pode ficar apertada |
| `pacman` | **legibilidade do labirinto** | 21×21 células em 480 de altura = ~22 px por célula; é o menor dos três |

O `pacman` foi projetado para 640×480 ser o caso pequeno — se ele não for legível
lá, o mapa (21×21) precisa encolher ou a pontuação sair da área do labirinto.

```bash
# conferir os três do catálogo, no aparelho
for app in pacman cobrinha volley; do
  echo "== $app"
  /opt/giochi/apps/launcher/bin/giochi-launcher --lancar $app --dump-frame /tmp/$app.bmp
done
```

Cada um imprime `painel LxA, projeto LxA (Nenhuma)` no X35H.

### 4. Lista com três itens

O catálogo agora tem **três** entradas. O X55 desenha a Home com um cartão por
vez (carrossel); em 640×480 o layout é `compacto` e os cartões vizinhos somem.

```bash
bash scripts/aparelho.sh subir
```

Olhar: o cartão precisa mostrar `Pac-Man · Arcade · 2026`, e o direcional
precisa **andar entre os três** sem dessincronizar do desenho. Essa classe de bug
(carrossel dessincronizado do foco) já aconteceu uma vez.

### 5. Ciclo jogar → sair → voltar

Já verificado no X55 e no X35H antes de hoje. O que muda: agora a volta
**recria o `Painter`** e o texto vem por `copy` no X35H — o caminho do TTF na
segunda janela é o que já foi corrigido (`ttf::init` não pode rodar duas vezes).
Confirmar que o texto **continua aparecendo** depois de voltar de um jogo. Se
sumir, é aquele bug de volta por outro caminho.

## O que **não** precisa repetir

Já verificado em hardware e sem mudança desde então:

- `poweroff` corta a energia (X55)
- volume pelo ALSA (os dois)
- brilho em sysfs e persistência (os dois)
- rotação lida de `fbcon/rotate` (os dois)
- mapa de botões completo (`buttons.map` + `code_for`)
- `--lancar` abrindo pelo catálogo (X55, nos três jogos)

## Registro do que aconteceu hoje

Para quem pegar isto depois sem contexto.

### O bug do texto

Relatado por quem estava olhando a tela do X55: *"o texto está com orientação
vertical e o restante horizontal"*. Exato: `Painter::text` fazia `canvas.copy`
num retângulo de **espaço de projeto**, enquanto todo o resto passava por
`rotate_rect`.

A primeira correção — `copy_ex` com 270° — deixou o texto **espremido**:
`SDL_RenderCopyEx` escala a textura para caber no destino *antes* de girar, e o
destino ia com as medidas trocadas. O que denunciou não foi o percentual de
acerto (98,9%, "ok"), foi **contar pixels claros: 611 no aparelho contra 1275 no
PC**.

Uma segunda tentativa, girando os pixels numa `Surface` antes de virar textura,
deu **segfault**: o buffer do `without_lock` é menor que `pitch × altura` nesta
combinação fontconfig/freetype (`range start index 4224 out of range for slice
of length 1716`).

O que ficou: destino no tamanho **original**, ancorado no **centro** do
retângulo girado. Sem mudança de tamanho não há escala.

### As três lições

1. **Percentual de acerto não aprova tipografia.** Texto é pouca área; um erro
   de orientação nos glifos move menos pixels que o relógio da barra. Antes de
   hoje eu tinha olhado o PNG do X55 e dito "está correto" — o texto estava a
   90° e eu não ampliei o suficiente para ler. Fonte se confere **lendo**.

2. **Instrumento novo só vale depois de medir o que ele mede.** A
   `correlacaoDeTexto()` entrou no `verificar-frame.mjs` com teste próprio, e
   ela separa deitado de girado com folga (0,98 contra -0,01 no X55 real).

3. **Duas correções podem ser pior que uma.** O `copy_ex` "consertou" a rotação
   e quebrou o tamanho, e o sintoma novo era mais difícil de ver que o antigo.

### Sobre a NPU

Perguntaram se a IA dos fantasmas podia usar a NPU. **Não dá, nesta imagem:**

```text
/proc/device-tree/compatible  →  rockchip,rk3566     (tem NPU no silício)
/dev/rknpu*                   →  não existe
/usr/lib/librknn*             →  não existe
buildroot.config              →  nenhum símbolo RKNPU
```

E mesmo que estivesse habilitada, não valeria: a escolha de direção de um
fantasma é uma distância de Manhattan sobre quatro candidatas — ~16 operações
inteiras, 6,7 vezes por segundo. Uma inferência seria milhares de vezes mais
cara, exigiria modelo e runtime (~dezenas de MB), e o comportamento ficaria
**menos previsível** — ou seja, mais difícil de depurar, não melhor.

NPU serve para rede neural. Escolher entre quatro direções não é rede neural.

Se um dia houver app que precise mesmo (visão, áudio, tradução), aí a conversa
muda — e ela começa no `Giochi System`, habilitando `RKNPU` no kernel e o
`librknn` no rootfs. Não é trabalho deste repositório.
