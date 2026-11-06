# Arcade Volleyball

Jogo padrão do sistema. Original de 1988; aqui é um projeto Godot 4.x **e** um
port nativo em C++/SDL2.

- **Runtime:** `native`
- **Origem:** `Jogos/Arcade Volleyball/` no antecessor `GIOCHI-KONSOL-OS`
- **Licença:** domínio público / arte própria — verificar antes de publicar

## O que roda, e por que não é o Godot

**O binário que vai para o aparelho é o port C++/SDL2** (`src/nativo/`). O projeto
Godot está em `src/` e está preservado, mas **não roda** no aparelho: nenhum
build oficial do Godot tem driver de vídeo KMSDRM, e o rootfs não tem X11 nem
Wayland. Medido — ver [docs/godot.md](../../docs/godot.md).

| Pasta | O que é |
| --- | --- |
| `src/` | projeto Godot 4.x — implementação de referência (simulação, presets, testes) |
| `src/nativo/` | port C++17 + SDL2 — **é o que roda** |
| `bin/arcade-volleyball` | o binário aarch64 instalado no aparelho |
| `assets/` | documento do porte; não entra no pacote |

## Controles

| Ação | Aparelho (evdev) | Teclado |
| --- | --- | --- |
| mover | direcional ← → | A / D ou setas |
| pular | A (304) ou B (305) | W, ↑ ou espaço |
| sair | Start (315) ou Select (314) | Esc ou Q |

A entrada é lida por `/dev/input/event*`, não pela API de gamepad do SDL: ela
devolve `nullptr` para os `gpio-keys` do aparelho, e o jogo abriria sem nenhum
botão funcionando. Mesma decisão do launcher.

## Reconstruir e instalar

```bash
bash scripts/compilar-jogo-cpp-arm64.sh                    # compila para aarch64
bash scripts/aparelho.sh verificar-jogo arcade-volleyball  # roda e analisa o quadro
node scripts/empacotar.mjs app/jogos/arcade-volleyball     # gera o .giochi assinado
bash scripts/aparelho.sh instalar-pacote dist/pacotes/arcade-volleyball.giochi
```

Para testar no PC, sem aparelho:

```bash
make -C app/jogos/arcade-volleyball/src/nativo host
SDL_VIDEODRIVER=dummy app/jogos/arcade-volleyball/src/nativo/arcade-volleyball-host \
  --tamanho 720x1280 --quadros 90 --dump-frame /tmp/av.bmp
GIOCHI_ROTATE=270 ...   # simula a orientação de um painel retrato
```

## Medido no X55

```text
painel 720x1280, projeto 1280x720 (Ccw90), escala 1.00, renderer opengles2
5 dispositivo(s) de entrada abertos
RAM de pico (VmHWM): 19 MB
97,9 % de acerto contra o render do PC; geometria idêntica
```
