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
| `SDL2_image` | não carrega PNG nem sprite: o desenho é de retângulos |

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

## Rotação — o jogo obedece ao sistema

O jogo desenha em **espaço de projeto** (paisagem) e gira cada retângulo ao
desenhar, lendo a orientação de `/sys/class/graphics/fbcon/rotate` — o mesmo
arquivo e a mesma precedência do launcher:

```text
X35H  (640×480,  rotate=0)  →  projeto 640x480   (Nenhuma)
X55   (720×1280, rotate=3)  →  projeto 1280x720  (Ccw90)
```

Nenhum nome de aparelho no código: o app não escolhe o sentido, ele obedece.
Antes disto o jogo no X55 saía espremido nos primeiros 480 px de 1280.

Para testar no PC, onde não existe esse arquivo:

```bash
GIOCHI_ROTATE=ccw SDL_VIDEODRIVER=dummy ./giochi-volley --quadro /tmp/v.bmp
```

`GIOCHI_ROTATE` é a mesma variável do launcher, de propósito: um comando que
gira um app tem de girar o outro, senão o teste mente sobre um dos dois.

## Limitações conhecidas

- **Analógico não lido.** Só o direcional. Normalizar eixo exige `absinfo` por
  ioctl — a mesma lacuna do launcher.
- **Um jogador.** O segundo lado é CPU.
- **`rotate=2` (180°) não suportado.** Nenhum painel do projeto precisa, e girar
  meia volta exigiria desenhar de cabeça para baixo.
