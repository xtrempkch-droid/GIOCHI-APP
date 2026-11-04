# Volley

Jogo de exemplo do Giochi. Escrito **neste** repositório, em Rust + SDL2, para
não depender de runtime nenhum além do que a imagem já tem.

- **Runtime:** `native`
- **Licença:** `Proprietary` (código próprio, não derivado de nenhum jogo)
- **Ano:** 2026

## Por que ele existe

O catálogo declarava quatro jogos, todos projetos Godot que nunca foram
migrados — `bin/` com só um `.gitkeep` — e que o aparelho não teria como rodar:
o X35H **não tem runtime Godot** (nem Python, nem RetroArch, nem Lua; só
binários nativos aarch64).

Isso deixava sem resposta prática a pergunta que importa para a integração:
*o launcher consegue entregar o aparelho a um jogo e recebê-lo de volta?* Não
havia jogo nenhum para testar. Este é esse jogo.

Não é o *Arcade Volleyball* de 1988 nem uma releitura dele: é um jogo de raquete
original, pequeno, escrito para exercitar o caminho — abrir o KMSDRM, ler
`/dev/input`, desenhar, sair devolvendo a tela.

## Controles

| Entrada | Ação |
| --- | --- |
| direcional ↑ ↓ | move a raquete |
| A / START | saque |
| B / SELECT | sai do jogo |
| setas / espaço / Esc (PC) | o mesmo, por teclado |

Partida até 7 pontos; ao chegar lá o placar zera.

## O que ele deliberadamente não usa

| Ausente | Motivo |
| --- | --- |
| `SDL2_ttf` / freetype | o placar é desenhado em sete segmentos, então o jogo roda em imagem sem biblioteca de fonte |
| `libasound` | não toca som — uma dependência a menos para o link cross-compilado falhar |
| Rotação | desenha em paisagem no tamanho real do painel. No X35H (`rotate:0`, 640×480) é o caso natural; num painel retrato girado a imagem sairia deitada |

`readelf -d` do binário mostra exatamente três `NEEDED`: `libSDL2-2.0.so.0`,
`libgcc_s.so.1`, `libc.so.6`.

## Compilar

```bash
# PC
bash scripts/compilar-launcher.sh        # só para preparar o SDL2 do host
cd app/jogos/volley && cargo build --release

# aparelho (aarch64)
bash scripts/compilar-jogo-arm64.sh
bash scripts/compilar-jogo-arm64.sh --instalar 10.55.0.2
```

O cross-compile **reusa** o cache de `~/.cache/giochi-arm64` que o
`compilar-launcher-arm64.sh` prepara — sysroot, wrapper de `rust-lld` e a
`libSDL2` copiada do aparelho. Sem esse cache o script para e diz o que rodar
antes, em vez de tentar adivinhar.

## Conferir o desenho sem tela

```bash
SDL_VIDEODRIVER=dummy ./target/release/giochi-volley --quadro /tmp/v.bmp
```

Grava um quadro em BMP. No PC com `dummy` o painel é recriado pelo tamanho
pedido, então dá para ver a interface antes de instalar.

## Limitações conhecidas

- **Sem rotação.** Painel retrato girado (X55) mostraria o jogo deitado.
- **Analógico não lido.** Só o direcional. Normalizar eixo exige `absinfo` por
  ioctl — a mesma lacuna do launcher.
- **Um jogador.** O segundo lado é CPU.
