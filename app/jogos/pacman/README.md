# Pac-Man

Terceiro jogo nativo do Giochi. Rust + SDL2, labirinto e arte originais — nada
do Pac-Man original foi copiado (nem mapa, nem sprite, nem som). O que se
reconhece é o gênero.

## Por que existe

Cada jogo novo testa uma camada a mais:

| Jogo | O que exercita |
| --- | --- |
| `volley` | o caminho launcher → app → launcher |
| `cobrinha` | a **lista** com dois itens |
| `pacman` | **ciclo de vida com estado**: pastilhas que somem, pausa ao perder, recomeço com pontuação preservada, quatro agentes |

Se algo no launcher mexe com tempo, foco ou reentrada, quebra aqui antes de
quebrar num jogo de verdade.

## O labirinto

21×21, ímpar para haver simetria em torno da coluna central, pequeno o bastante
para caber legível em 640×480. Um mapa maior ficaria bonito no X55 e ilegível no
X35H — e o alvo é o aparelho pequeno.

## Os quatro fantasmas

Não usam IA de verdade, e isso é decisão registrada: são quatro regras de
distância de Manhattan sobre quatro direções candidatas.

| Humor | Como persegue |
| --- | --- |
| `Cacador` | vai direto na posição atual do herói |
| `Emboscada` | mira quatro casas à frente, tentando cortar o caminho |
| `Lento` | persegue a posição que o herói tinha 12 quadros atrás |
| `Aleatorio` | sorteia um alvo a cada esquina |

**Sobre usar a NPU:** o RK3566 tem NPU no silício, mas ela **não está
habilitada** nesta imagem — não há `/dev/rknpu`, nem `librknn`, nem símbolo
`RKNPU` no config do kernel (verificado no `buildroot.config` e no aparelho).
E mesmo que estivesse: a escolha de direção custa ~16 operações inteiras por
passo, 6,7 vezes por segundo. Uma inferência seria milhares de vezes mais caras,
exigiria um modelo e o runtime RKNN (~dezenas de MB), e o comportamento ficaria
menos previsível — ou seja, mais difícil de depurar, não melhor. NPU serve para
rede neural, não para escolher entre quatro direções.

## Controles

| Entrada | Ação |
| --- | --- |
| direcional | vira na próxima esquina possível (o pedido é guardado, não aplicado) |
| A / START | começa / recomeça |
| B / SELECT | sai |
| setas, Espaço, Esc (PC) | o mesmo |

Pastilha grande deixa os fantasmas assustados por 6 s; assustado, ele foge e
pode ser comido (200 pontos). Ele **pisca** nos últimos 2 s, avisando que a
proteção está acabando.

## Compilar

```bash
bash scripts/compilar-jogo-arm64.sh --instalar 10.55.0.2 --jogo pacman
```

## Limitações

- **O fantasma atravessa a "porta" da casa** — a casa é decorativa, não há
  contenção.
- **Sem túnel lateral** (atravessar a borda). Num tabuleiro pequeno, envolver
  torna a partida longa demais.
- **Sem som.** Três `NEEDED`, como os outros.
