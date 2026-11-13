# STATUS — GIOCHI Tower Defense

Atualizado em 13/11/2026. Este arquivo é o resumo de uma página; o raciocínio de
cada decisão está em [`DECISIONS.md`](DECISIONS.md), o design em
[`docs/GDD.md`](docs/GDD.md) e o brief original em
[`docs/prompt.md`](docs/prompt.md).

## Em uma tela

| | |
| --- | --- |
| testes | **213 lib + 26 de integração** (`cargo test --workspace`), `clippy -D warnings` limpo |
| binário aarch64 | 2,0 MB, 5 bibliotecas `NEEDED` (SDL2, SDL2_mixer, libgcc_s, libm, libc) |
| aparelho | R35S / RK3326, 640×480, **GLES 3 no Mali-G31** (Panfrost) |
| modelos no aparelho | **15/15 carregados**, **16 texturas** na GPU |
| FPS no aparelho | **59,8** (vblank) sem UI · **59,9** com UI, HUD e 12 inimigos |
| trabalho por quadro | 1,58 ms sem UI · **6,13 ms** com UI + horda (0,47 chamada/quadro de horda) |
| pixel a pixel | 1 506 cores no R35S contra 2 795 no PC; **0,00 %** de quase-preto nos dois |
| multiplicador | host e cliente em dois processos, `jogador 1 entrou` dos dois lados |

## O que foi feito nesta rodada

Três pedidos do dono, e o terceiro achou dois defeitos que a suíte não pegava.

| Pedido | Como ficou |
| --- | --- |
| "os personagens estão como apenas cubos" | 15 modelos OBJ + 15 texturas BMP **gerados por script**, carregados pela engine atualizada |
| "ponha o multiplayer" | `net/descoberta.rs` (UDP broadcast) + `net/sessao.rs`, e a `real-net` deixou de ser feature |
| "um menu para reiniciar, sair e configurar" | `src/menu.rs`, 5 telas, 21 testes, `settings.json` persistido |

### 1. Os personagens têm modelo, e a prova é a contagem de cores

`scripts/gerar-modelos.mjs` escreve **15 OBJ + 15 BMP** de forma determinística
(`--conferir` reprova qualquer divergência): 4 classes, 5 inimigos, 4 bosses, o
Coronel e a torre. Nada de PNG — o `SDL2_image` do aparelho já apareceu compilado
sem `--enable-png` (AGENTS.md §24).

A prova de que o modelo substituiu o cubo **não** é "o arquivo abriu":

```text
com modelos     2 795 cores distintas · 13,63 % dos pixels diferentes
sem modelos       113 cores distintas  (cubos de face plana)
```

113 cores é o que um cubo dá; 2 795 é o que uma superfície texturizada dá. E o
teste `tests/modelos.rs` carrega os 15 arquivos **pelo loader da engine** e mede
a geometria (altura, largura, número de vértices), então um OBJ vazio ou com a
face invertida reprova sem abrir janela.

Custo medido: a horda sai em **um lote instanciado por modelo** (`agrupar`), não
um por inimigo. Com 12 inimigos na tela: **0,32 ms** e **0,47 chamada/quadro**.

### 2. Multiplayer de verdade, e o `local_id` que o handshake manda

`SessaoJogo::{Host, Cliente}` é a única porta de rede no laço. Verificado com
**dois processos**:

```text
host    rede: hospedando em 0.0.0.0:7793
host    net: cliente 1 entrou (AOI 55 m)
cliente rede: conectando; o id de jogador chega no handshake
cliente rede: assumindo o jogador 1
cliente net: handshake aceito, jogador 1
```

O crate `giochi-net` **consome** o `HandshakeOk` e guarda o id em `jogador()` —
nunca o repassa ao jogo. O jogo tinha um braço `Mensagem::HandshakeOk` que por
isso nunca podia rodar, e o `local_id` ficava no 0 de nascimento, que é **o id do
host**: o cliente dirigia o avatar do outro. Agora o id vem de
`cliente.jogador()`, e `state.local_id` acompanha.

A feature `real-net` foi **removida**: rede atrás de `--features` é rede que o
jogador não tem.

### 3. O menu, e o `--teclas` que mentia

Cinco telas (Principal, Multijogador, Configurações, Pausa, Fim), 21 testes,
incluindo um de propriedade com 5 telas × 200 entradas aleatórias. O menu é
**estado puro** — não toca em GL, arquivo nem socket.

O `--teclas` chamava `Menu::entrada` antes do laço e **descartava a ação**:

```text
antes   --teclas "a" -> tela partida · 60 quadros · 0 passo(s) de simulação
depois  --teclas "a" -> partida: nova  · 60 quadros · 60 passo(s) de simulação
```

A tela mudava, o log dizia que sim, e nada começava — a mesma classe do
`--teclas` do launcher (AGENTS.md §24). Agora a tecla entra pela **fila de
eventos**, uma por quadro, com um quadro de soltura: a repetição do menu é de
0,22 s e sem a soltura a tecla seguinte era engolida (`baixo,baixo` parava em
"Multijogador").

Os quatro caminhos verificados no CI:

| Sequência | O que prova |
| --- | --- |
| `a` | inicia a partida (`partida: nova` **e** passos de simulação) |
| `start,baixo,a` | Start pausa, Reiniciar reinicia |
| `cima,a` | Sair encerra o processo |
| `baixo,baixo,a,esq,esq` | grava `settings.json` com volume 60 |

## Capturas no R35S

Menu (97 cores, 0,00 % de preto) e a partida com os 15 modelos + 16 texturas
(1 506 cores, 0,00 % de preto). As duas foram lidas **antes** da troca de
buffers, então o BMP não é preto e não foi preciso forçar o backend por software:

![Menu do GIOCHI Tower Defense no R35S](../../../docs/imagens/giochi-tower-menu-r35s.png)

![Partida com modelos no R35S](../../../docs/imagens/giochi-tower-modelos-r35s.png)

## Onde o projeto está

| Bloco | Estado | O que existe |
| --- | --- | --- |
| M1 — fundação | **entregue** | engine vendorizada, janela GLES/EGL, câmera isométrica, loop fixo 60 Hz, 10 configs RON validadas, entrada (gamepad SDL, teclado/mouse, touch emulado) |
| M2 — rede e combate | **entregue** | protocolo `postcard`, AOI, stub loopback, 5 inimigos com IA, spawner por onda com teto de vivos, torre, placar, fases |
| M3 — bosses, nemesis, loja | **entregue** | 4 bosses com fases, Coronel por tier com item exclusivo, loja no host, HUD, minimapa e tela da loja desenhando |
| M4 — multiplayer | **3 de 6** | host/cliente reais, descoberta por UDP, `local_id` pelo handshake; falta mDNS, predição e o teste de 4 clientes |
| M5 — handheld | parcial | roda no R35S por KMSDRM com modelos; falta o caminho evdev de entrada |
| M6 — polimento | **começado** | modelos, texturas, menu e volume de áudio; falta animação e som por evento |

## O que NÃO foi feito, e por quê

1. **O jogo continua `scaffold` em `apps.json`.** Já não é "não desenha" nem
   "não tem modelo": é **a entrada não ter sido verificada por dedo**. O aparelho
   não tem `/dev/uinput`, então `--teclas` cobre o menu e não o evdev.
2. **M4 incompleto:** mDNS (o crate não tem), predição/smoothing, autoridade real
   do host para dano, e o teste com 4 clientes.
3. **Os modelos não têm animação.** São estáticos com giro derivado do rumo. O
   inimigo aponta para a torre porque é para lá que ele anda — custa uma conta em
   vez de um campo novo no protocolo.
4. **A loja é desenhada em escala reduzida.** Nove linhas de 38 caracteres não
   cabem a 640 px na escala do HUD (2,0).

## O que falta, em ordem

1. **Verificar a entrada com o dedo** (ou trocar para evdev, M5). É o que separa
   o app de `ready`.
2. **M4:** predição/smoothing, autoridade de dano no host, teste de 4 clientes.
3. **Animar os modelos** (balanço de passo) e som por evento.

## Como reproduzir

```bash
cd app/jogos/giochi-tower
cargo test --workspace
cargo clippy --all-targets -- -D warnings
node scripts/gerar-modelos.mjs --conferir            # assets reprodutíveis
SDL_VIDEODRIVER=offscreen cargo run -- --benchmark 300          # FPS no PC
SDL_VIDEODRIVER=offscreen cargo run -- --benchmark 300 --sem-ui # sem o HUD
SDL_VIDEODRIVER=offscreen cargo run -- --menu --teclas "baixo,baixo,a,esq,esq"

bash scripts/compilar-jogo-arm64.sh --jogo giochi-tower --instalar root@10.55.0.2
bash scripts/aparelho.sh verificar-jogo giochi-tower
```

Três armadilhas que custaram rodada, as três já conhecidas do repositório:

1. `SDL_VIDEODRIVER=dummy` **não tem OpenGL**; o erro é
   `OpenGL support is either not configured in SDL`, que parece defeito do jogo.
   Use `offscreen` (tem EGL).
2. Com backend acelerado, `--quadro` gravava BMP **preto** — o swap esvazia o
   buffer. Aqui a captura passou a ler **antes** de `render.end()`, o que resolve
   na origem em vez de obrigar o backend por software.
3. `timeout` não mata o jogo no aparelho: o `SIGTERM` só posta `SDL_QUIT`.

## O que NÃO fazer sem medir

- Não portar para a ABI C (`libgiochi.so`) "porque o prompt pede": o jogo precisa
  de `giochi-net`, que **não tem superfície C**. Ver D-002.
- Não reativar `rand` antes do M5: o spawn determinístico é o que torna
  `--frames` e `--benchmark` comparáveis (D-024).
- Não trocar `font::retangulos` por "um retângulo por ponto" de volta: custa
  3,2× mais instâncias e a loja cai para 30 fps no RK3326.
- Não editar `vendor/`: é cópia do upstream (regra R1). Atualizar é copiar de
  novo, e o `glam` do jogo segue o que o código da engine chama (0.34).
- Não tratar "compila" como "roda": a regra do repositório vale aqui.
