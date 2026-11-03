# Giochi Launcher

Interface do console. **Reescrito em Rust**, a partir de um protótipo HTML.

- **Runtime:** `native` (Rust + SDL2)
- **Distribuição:** `image` — vai dentro da imagem assinada
- **Licença:** `Proprietary`
- **Entrypoint:** `bin/giochi-launcher`

## Stack

**SDL2 + renderer próprio** (decidido em 02/11/2026).

| Alternativa | Por que não |
| --- | --- |
| egui | desenho custom (gradiente, glow) fica limitado |
| Slint | DSL própria e runtime grande para 1 GB de RAM |
| **SDL2** | já está no Buildroot; backend KMSDRM dispensa compositor; controle total do desenho |

O custo é desenhar a UI, mas o layout é simples — cartões, gradientes, texto — e
não justifica carregar um toolkit.

## Estrutura

```text
launcher/
├── Cargo.toml                  # workspace
├── prototipo/
│   └── launcher.html           # protótipo corrigido — abre no navegador
└── crates/
    ├── core/                   # núcleo puro: sem pixel, janela ou GPU
    │   └── src/
    │       ├── cmd.rs          # comandos como enum
    │       ├── state.rs        # estado completo
    │       ├── focus.rs        # zonas e navegação espacial
    │       ├── reduce.rs       # estado + comando → estado
    │       ├── gamepad.rs      # controle → comandos
    │       └── viewport.rs     # escala e área segura
    └── ui-sdl/                 # SDL2 + renderer próprio
        └── src/
            ├── main.rs         # janela, laço, controle
            ├── app.rs          # eventos → comandos → desenho
            ├── theme.rs        # paleta e métricas
            ├── draw.rs         # primitivas: retângulo, gradiente, círculo, texto
            ├── icons.rs        # ícones vetoriais (dados, não PNG)
            └── views.rs        # telas + áreas tocáveis
```

## Decisões de desenho

**Sem PNG para ícone.** Cada ícone é uma lista de linhas, polilinhas e círculos
no espaço 24×24, igual ao SVG do protótipo. Nitidez em qualquer DPI e nenhum
asset binário para versionar.

**Sem shader para gradiente.** `SDL_Renderer` não tem gradiente; ele é
aproximado por faixas de 4 px. Com 4 px a diferença é invisível e o custo é
~80 chamadas por cartão.

**Escala pelo SDL, não à mão.** `set_logical_size(1280, 720)` faz o letterbox.
O protótipo calculava à mão e cortava 106 px de cada lado em 640×480.

**Toque escolhe alvo direto.** Cada elemento interativo registra seu retângulo
durante o desenho; o toque consulta essa lista. Quem desenha sabe onde o
elemento está, então é quem registra — separar os dois garante que fiquem
dessincronizados.

## Rodar

Da raiz do repositório, `bash scripts/compilar-launcher.sh` resolve o SDL2 (inclui
o caso sem root) e compila. Detalhes em
[docs/compilar-launcher.md](../../docs/compilar-launcher.md). Se o SDL2 já está
instalado no sistema:

```bash
cd app/launcher
cargo run                     # janela 1280x720
cargo run -- --fullscreen     # tela cheia
cargo run -- --font caminho/para/fonte.ttf
cargo test --workspace        # 54 testes, sem tela e sem placa
```

Fonte: **DejaVu Sans**. O código procura nesta ordem:

1. `assets/fonts/DejaVuSans.ttf` — junto do app, vai para o aparelho
2. `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` (Debian/Ubuntu)
3. `/usr/share/fonts/TTF/DejaVuSans.ttf` (Arch)
4. `app/launcher/assets/fonts/DejaVuSans.ttf` (rodando da raiz)

Ou `--font /caminho/DejaVuSans.ttf`.

**O arquivo não foi incluído no repositório** — copie `DejaVuSans.ttf` para
`assets/fonts/`. Detalhes e licença em
[assets/fonts/README.md](assets/fonts/README.md).

Sem a fonte, o desenho continua e o texto é omitido, com aviso no console.

> DejaVu é mais larga que Inter/Segoe. Se algum rótulo estourar — o rodapé é o
> candidato — reduza `F_TINY`/`F_SMALL` em `theme.rs`.

No aparelho: `GIOCHI_GLES=1` seleciona o backend `opengles2`. No desktop, deixe
vazio.

## O que está verificado e o que não está

Compilado e exercitado em 03/11/2026 (Linux x86-64, rustc 1.99, SDL2 2.32.10 +
SDL2_ttf 2.24.0):

| Item | Estado |
| --- | --- |
| `prototipo/launcher.html` | funciona — HTML puro, abre no navegador |
| `crates/core` — 48 testes | **passam** |
| `crates/ui-sdl` — 6 testes | **passam** |
| `cargo build --workspace` | **sem erro e sem aviso** |
| Execução real | **roda** — Home desenhada e gravada em BMP com `--dump-frame` |
| Alvo aarch64 / KMSDRM | **não testado** — não há placa aqui |

As três suposições de API do `sdl2` 0.37 anotadas como risco foram resolvidas:

1. `load_font` devolver `Font<'static, 'static>` — **procede**; o caminho vai para
   `Box::leak` para satisfazer o tempo de vida.
2. `GameControllerSubsystem::open` — **não é usado**: a entrada é por evdev.
3. `Canvas::window_to_logical` — **não existe** na 0.37. A conversão foi refeita
   em `Painter::window_to_logical`, com a mesma matemática do
   `SDL_RenderWindowToLogical`.

Falta validar: cross-build aarch64, backend GLES2 sobre KMSDRM e a leitura real
dos dispositivos em `/dev/input`.

## Relação com o protótipo

O HTML e o Rust usam a **mesma** arquitetura: mesmos nomes de comando, reducer
puro, foco `(zona, índice)`, overlays separados da aba. O port é mecânico, não
reinterpretação. O diagnóstico dos defeitos do mockup original está em
[docs/diagnostico-prototipo.md](../../docs/diagnostico-prototipo.md).

## O que falta

- **Cross-build aarch64** e teste na placa (o único alvo que importa).
- **Fonte TTF** dentro do pacote, com licença compatível.
- **Leitura real** de bateria, temperatura e apps instalados; hoje a UI mostra
  `—` onde não há sensor.
- **Lançar apps de verdade** — o código registra a intenção e mostra aviso.
- **Empacotar** para `bin/` no alvo aarch64.
