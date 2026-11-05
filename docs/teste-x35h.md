# Testar esta versão no X35H

> Escrito em 04/11/2026, depois de a versão nova rodar **só no X55**.
> O X35H esteve fora da rede durante todo o trabalho de hoje.
> **Nada do que está aqui foi verificado no X35H.**

---

## PRIORIDADE PARA A PRÓXIMA SESSÃO: detecção automática de jogos

Pedido do dono do projeto em 04/11/2026: *"temos que deixar a detecção de jogos
de forma automática"*.

### O que é manual hoje

Três passos, e **todos os três são digitados à mão**:

| Passo | Como está | Quem deveria fazer |
| --- | --- | --- |
| 1. compilar e instalar o binário | `bash scripts/compilar-jogo-arm64.sh --jogo pacman --instalar 10.55.0.2` | o passo já existe, mas é por jogo |
| 2. **gerar `/opt/giochi/apps/index.json`** | **copiado à mão** de um arquivo escrito à mão (`/tmp/index3.json` nesta sessão) | ninguém: é o buraco |
| 3. subir o launcher | `bash scripts/aparelho.sh subir` | já é automático |

O passo 2 é o problema. Hoje o catálogo do aparelho é um JSON que eu escrevi
num `heredoc`. Ele **não é derivado de nada**: nem do `apps.json`, nem dos
`metadata.json`, nem do que existe em `/opt/giochi/apps/`. Consequências já
observadas:

- Instalei `cobrinha` e `pacman` e o aparelho continuou mostrando **um** jogo
  até eu reescrever o JSON à mão.
- O `year`, o `title` e o `category` do catálogo podem divergir do
  `metadata.json` do pacote, e nada avisa.
- Um app instalado mas fora do JSON fica **invisível** — e não há erro em lugar
  nenhum.

### O que "automático" deve significar

Duas detecções diferentes, e a distinção importa:

**(a) Descobrir o que existe no aparelho.** Varrer `/opt/giochi/apps/*/`, e para
cada pasta com `metadata.json` + `entrypoint` executável, adicionar ao catálogo.
Não perguntar ao PC o que está lá — perguntar ao aparelho. Isso resolve
"instalei e não apareceu".

**(b) Derivar o catálogo dos metadados.** `title`, `developer` e `year` saem do
`metadata.json` do pacote, não de um JSON paralelo. O `id` sai do nome da pasta
(ou do `metadata.json`). Regra: **o catálogo é uma projeção, não uma segunda
fonte da verdade.** Já temos essa lição escrita em `docs/metadata.md`: campo que
mora em dois lugares envelhece diferente nos dois.

### Desenho sugerido, e por que

```text
app/apps.json + app/**/metadata.json   (fonte, versionada)
            │
            ▼  scripts/gerar-catalogo.mjs
      index.json                        (artefato, gerado)
            │
            ▼  scp
   /opt/giochi/apps/index.json          (no aparelho)
```

Um comando novo, `bash scripts/aparelho.sh catalogo`, que faz as duas coisas:
gera a partir de `apps.json` + `metadata.json` e envia. Nada de escrever JSON à
mão nunca mais.

Detalhes que vão morder, na ordem em que vão morder:

1. **`folder` pode ter grupo.** `jogos/pacman` → `id` é `pacman`, e o `dir` no
   aparelho é `/opt/giochi/apps/pacman`. Não usar o caminho do repo como caminho
   do aparelho.
2. **Filtrar por `status`.** Só `ready` entra. `scaffold` tem `bin/` vazio e
   `distribution: host` (`forge`, `store`) não vai para o aparelho. Hoje o
   catálogo escrito à mão simplesmente omitia — automático precisa de regra.
3. **`distribution: "host"` nunca vai**, e `"store"` também não (é instalado em
   `/user`, não na imagem).
4. **`entrypoint` precisa existir no disco** antes de entrar no catálogo. Um
   catálogo que aponta para binário ausente é pior que catálogo vazio: o usuário
   aperta e nada acontece.
5. **O `dir` do aparelho não é derivável do repo.** É `/opt/giochi/apps/<id>`
   por convenção do `Giochi System`. Isso tem de ficar explícito no gerador —
   é a única informação que não vem nem do `apps.json` nem do `metadata.json`.

### Teste que fecha a tarefa

Instalar um jogo novo, rodar **um** comando, e ele aparecer na tela sem editar
JSON nenhum:

```bash
bash scripts/compilar-jogo-arm64.sh --jogo <novo> --instalar 10.55.0.2
bash scripts/aparelho.sh catalogo          # <- o que falta
# e o launcher mostra N+1 jogos
```

Depois: remover um app do aparelho e o catálogo encolher na chamada seguinte.
Projeção de verdade reflete remoção, não só adição.

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
