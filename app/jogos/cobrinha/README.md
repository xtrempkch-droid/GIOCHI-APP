# Cobrinha

Segundo jogo nativo do Giochi. Rust + SDL2, original — não é o Snake de nenhum
aparelho.

## Por que existe

O `volley` provou o caminho launcher → app → launcher. **Um app só não testa o
que interessa num console: a lista.** Com dois no catálogo, o launcher precisa
desenhar dois cartões, navegar entre eles, iniciar o escolhido e voltar para o
cartão certo.

Este é o mais simples dos três, de propósito: grade discreta, sem física, sem
estado escondido. Se ele falhar, o problema é do launcher.

## Controles

| Entrada | Ação |
| --- | --- |
| direcional | muda a direção (virar 180° é recusado — regra de todo Snake) |
| A / START | começa / recomeça depois de perder |
| B / SELECT | sai do jogo |
| setas, Espaço, Esc (PC) | o mesmo |

## O que não usa

`SDL2_ttf`, `libasound`, `SDL2_image`. `readelf -d` mostra três `NEEDED`:
`libSDL2-2.0.so.0`, `libgcc_s.so.1`, `libc.so.6`.

## Compilar

```bash
bash scripts/compilar-jogo-arm64.sh --jogo cobrinha --instalar 10.55.0.2
```

## Conferir outro tamanho de painel

```bash
giochi-cobrinha --tamanho 800x600 --quadro /tmp/c.bmp
node scripts/verificar-jogos.mjs --tamanhos 800x600
```

Medido: célula de **18,4 px** em 640×480, **23,0 px** em 800×600 e **27,7 px** em
1280×720 (piso adotado: 14 px).

## Rotação

Lê `/sys/class/graphics/fbcon/rotate`, como o launcher e como o `volley`.
`GIOCHI_ROTATE` sobrepõe para testar no PC:

```bash
GIOCHI_ROTATE=ccw SDL_VIDEODRIVER=dummy ./giochi-cobrinha --quadro /tmp/c.bmp
```

## Limitações

- **Um jogador.** Sem modo de dois.
- **Sem som.** Não linka `libasound`.
- **`rotate=2` (180°) não suportado**, igual aos outros.
