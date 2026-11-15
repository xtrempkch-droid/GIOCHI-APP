# Música — o primeiro app de imagem que não é interface

O `app/musica/` é o app que faltava para a aba de apps do launcher ter o que
mostrar. Até ele, os sete apps do `apps.json` eram `host` (rodam no PC) ou
**tema** do launcher (`role: shell` / prefixo `launcher-*`), e o catálogo do
aparelho saía **vazio por construção** — correto pelas regras, e visível como
"Nenhum jogo instalado." na tela.

Medido em 15/11/2026:

```bash
$ node scripts/gerar-catalogo.mjs --lista
entram no catálogo (1):
  musica                   /opt/giochi/apps/musica/bin/giochi-musica
```

## 1. O que ele é

Um tocador de música sobre a **GIOCHI-ENGINE** (o crate 2D do
`GIOCHI-ENGINE-3D`, versão 0.5.1), com:

| Peça | Como |
| --- | --- |
| biblioteca | lê uma pasta (`/user/musica` no aparelho, `~/Música` no PC), aceita `wav, ogg, flac, mp3, mod, xm, it, s3m` |
| ordem | **natural**: `Faixa 2` antes de `Faixa 10` |
| capas | **geradas** do nome do arquivo (FNV-1a → matiz), sem PNG no pacote |
| navegação | carrossel com volta nas duas pontas, foco com borda, selo da faixa que está no mixer |
| transporte | tocar/pausar, próxima, anterior, parar, volume em passos de 5 |
| entrada | evdev (o mesmo leitor e o mesmo mapa de botões do launcher) + teclado |
| tela vazia | diz **o motivo** e o caminho procurado |

A referência do app de exemplo que originou o pedido está no histórico da
conversa: ele misturava desenho, leitura de gamepad e áudio num `update` só, não
tinha pausa, não tinha volume, não dava a volta no carrossel e morria no primeiro
asset ausente. O que mudou, peça por peça, está no README do app.

## 2. As três camadas, e por que separadas

| Camada | Onde | Sabe o quê | Testes |
| --- | --- | --- | --- |
| decisão | `app/musica/crates/core` | ordem, foco, o que tocar, layout, rotação | **120**, sem SDL |
| desenho | `src/pintor.rs`, `src/tela.rs` | pixels, glifos, texturas | — |
| mundo | `src/audio.rs`, `src/entrada.rs` | mixer e `/dev/input` | 40 (junto do resto do binário) |

`crates/core` **não tem nenhuma dependência**. É o que o CI consegue rodar: o
binário depende da engine, que depende de SDL2 e de um repositório irmão. A
mesma divisão do `giochi-launcher-core`, pelo mesmo motivo.

O reducer devolve um `Efeito` (`Tocar(i)`, `Pausar`, `Retomar`, `Parar`,
`Volume(n)`, `Recarregar`, `Sair`) em vez de agir: assim "apertar A na faixa em
foco troca de música" é testável sem placa de som.

## 3. O que reusa, e o que não reusa

| Peça | Origem | Por quê |
| --- | --- | --- |
| `theme.rs` | `app/launcher/crates/ui-sdl` — **o mesmo arquivo**, `#[path]` | uma paleta só no sistema; uma segunda divergiria na primeira correção |
| `icons.rs` | idem | o traço de 2 px é o que dá a espessura; copiar coordenadas sem o traço produz dois pesos de linha |
| `input_evdev.rs` | idem | o mapa é **do aparelho**; um segundo arquivo apontaria para outro botão |
| `Mapper` | `giochi-launcher-core` | borda e repetição com a curva medida (230 ms / 100 ms) |
| pintor | **novo** | ver abaixo |

O pintor **não** foi reusado do `launcher-teste`, que tem um pronto: aquele app é
banco de prova da engine — existe para ser alterado por experimento — e um app
de produção que dependesse dele herdaria cada experimento. O pintor daqui é a
única peça que não podia ser compartilhada sem inverter a direção da dependência.

Três ícones novos entraram em `icons.rs` (`nota`, `pause`, `anterior`,
`proximo`) porque o glifo pertence à tabela compartilhada, não ao app.

## 4. O que foi medido

| Medida | Resultado | Como |
| --- | --- | --- |
| núcleo | **120 testes** | `cargo test` em `crates/core` |
| binário | **38 testes** | `cd app/musica && cargo test` |
| quadro 640×480 PC × R35S | **100,00% idênticos** | `--demo --sem-som --dump-frame`, comparação pixel a pixel |
| retrato × paisagem | **99,96%** (o `verificar-musica.mjs` mede **99,97%**) | 720×1280 des-rotacionado contra 1280×720 |
| determinismo | 2 quadros, bytes iguais | idem |
| desenhos por quadro | **96** (640×480) a **112** (1080p) | contador do pintor |
| áudio no R35S | mixer **2.6.3**, `Playing → Stopped` em **2,5 s** num WAV de 2,5 s | `--testar-som` |
| binário aarch64 | **736 KB** `strip`ado | `compilar-musica-arm64.sh` |

### O que **não** foi medido, e por quê

- **Abrir no painel real.** Na bancada a tentativa direta recebeu
  `kmsdrm not available`: o DRM master é exclusivo e o `::respawn:` do inittab
  reiniciou o launcher (pid novo) antes de o app abrir a janela. No fluxo de
  produção quem libera o DRM é o **próprio launcher**, que fecha a janela antes
  do `fork` (§6.8 do `AGENTS.md`) — e esse caminho não é acionável por SSH sem
  apertar `A` no aparelho. O que foi medido no aparelho é o **desenho** (driver
  `dummy`, 100% idêntico ao PC) e o **áudio**; a abertura de janela KMSDRM é a
  mesma chamada do `launcher-teste`, verificada no aparelho em 07/11
  ([launcher-teste.md](launcher-teste.md)).
- **Som audível pelo alto-falante.** `--testar-som` prova que o mixer consome o
  fluxo (o relógio dele anda exatamente a duração do arquivo), não que o
  amplificador está ligado.
- **Giro num aparelho retrato real.** O X55 não estava na bancada; o giro foi
  verificado contra a rotação do próprio quadro (99,97%), não contra o painel.

O `--testar-som` é o que separa "o mixer aceitou a faixa" de "o áudio está
correndo": o relógio do mixer anda exatamente a duração do arquivo. **Não** foi
medido som audível pelo alto-falante.

```bash
bash scripts/compilar-musica.sh --testes
bash scripts/compilar-musica.sh --clippy
bash scripts/compilar-musica.sh --verificar
GIOCHI_DEVICE=root@10.55.0.2 node scripts/verificar-musica.mjs --aparelho root@10.55.0.2
```

## 5. Dependência de imagem

O binário pede quatro bibliotecas no carregamento:

```text
libSDL2-2.0.so.0  libSDL2_ttf-2.0.so.0  libSDL2_mixer-2.0.so.0  libSDL2_image-2.0.so.0
```

Medido na imagem do R35S de 15/11/2026: **todas presentes** (`SDL2` 2.28.5,
`SDL2_mixer` 2.6.3, `SDL2_image` 2.0.8, `SDL2_ttf` 2.24.0), com `libasound` e
`/dev/snd`.

Isso corrige uma leitura antiga: `docs/dependencias-aparelho.md` diz que
`libSDL2_mixer` não está na imagem — verdade para o launcher, que não usa mixer,
e **falso** para esta imagem. É requisito deste app, e biblioteca ausente aqui é
falha de carga (o processo não sobe), não degradação. O
`compilar-musica-arm64.sh --instalar` **pergunta ao aparelho** antes de copiar e
avisa quando falta.

## 6. O que falta

| Item | Por quê |
| --- | --- |
| barra de progresso | o `SDL_mixer` não informa a posição da música; qualquer barra seria um número inventado |
| duração de MP3/OGG/FLAC | só o WAV declara o tamanho no cabeçalho; a ficha mostra `—` em vez de adivinhar |
| ler ID3/FLAC picture | exigiria um decodificador de metadados por formato; a capa derivada do nome cobre o caso |
| playlists, busca, favoritos | não existem |
| trocar de pasta pela interface | não existe — `--pasta` ou `GIOCHI_MUSICA` |
