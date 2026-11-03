# Ferramentas

Os scripts deste repositório existem porque o aparelho é a única fonte de
verdade e testar nele à mão custa tempo. Cada um nasceu de um erro concreto —
nenhum foi escrito "por completude".

## Resumo

| Ferramenta | Roda onde | Para quê |
| --- | --- | --- |
| `scripts/aparelho.sh` | PC | operação e diagnóstico do aparelho por SSH |
| `scripts/verificar-frame.mjs` | PC | decide se o quadro renderizado é a UI ou lixo |
| `scripts/compilar-launcher.sh` | PC | compila o launcher para x86-64, sem root |
| `scripts/compilar-launcher-arm64.sh` | PC | compila para aarch64 e instala no aparelho |
| `scripts/validate-app.mjs` | PC / CI | valida `apps.json`, `metadata.json`, `forge.json` |
| `scripts/migrar-do-antecessor.sh` | PC | traz as fontes do repositório antecessor |

## `aparelho.sh`

```bash
bash scripts/aparelho.sh procurar        # varre a /24 e acha o aparelho
bash scripts/aparelho.sh diagnostico     # sistema, tela, áudio, entrada, libs, processo
bash scripts/aparelho.sh instalar        # cross-compila e instala, conferindo sha256
bash scripts/aparelho.sh subir           # inicia o launcher e mostra pid + log
bash scripts/aparelho.sh parar           # encerra pelo pid
bash scripts/aparelho.sh verificar       # processo + DRM + entrada + frame comparado
bash scripts/aparelho.sh log             # últimas 40 linhas de /tmp/giochi.log
bash scripts/aparelho.sh input 15        # eventos crus de /dev/input por 15 s
```

### Por que cada subcomando existe

**O IP muda a cada boot.** É DHCP. `procurar` varre a `/24` em paralelo e
identifica o aparelho pelo hostname (`*giochi*`), não pelo endereço antigo nem
pelo MAC. Fixar o IP na memória sempre custa uma rodada de tentativas erradas.

**O `inittab` não inicia o `giochi-init`.** A interface não sobe sozinha hoje;
`subir` faz o que o init deveria fazer. Isto é dívida do **Giochi System**, não
do app.

**"Processo vivo" não prova nada.** Um launcher pode estar rodando e não desenhar
nada. Por isso `verificar` exige descritores do DRM abertos e, mais importante,
compara o quadro do aparelho com o mesmo código rodando no PC.

**`pkill -f` mata a própria sessão SSH.** A linha de comando do shell contém o
padrão procurado, então o shell casa consigo mesmo. O script usa `pidof` e depois
`kill`.

### `verificar`, em detalhe

```bash
bash scripts/aparelho.sh verificar
```

1. confere processo, descritores de DRM e de `/dev/input`;
2. **para o launcher** — o KMSDRM é exclusivo, e sem parar o segundo processo
   recebe `kmsdrm not available` e não gera quadro nenhum;
3. captura `--dump-frame` no aparelho e traz por `scp`;
4. desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`;
5. des-rotaciona a captura conforme `/sys/class/graphics/fbcon/rotate` e compara
   pixel a pixel com a referência;
6. sobe o launcher de novo, se ele estava rodando antes.

No X55 o resultado é **99,9 %** de acerto. Rodar com a rotação oposta cai para
**10,9 %** — é essa diferença que separa "desenhou certo" de "desenhou de ponta
a cabeça", e foi o que resolveu o bug de orientação.

Os quadros ficam em `dist/testes/<data>/frame.png` (ignorado pelo git), para
inspeção visual depois.

## `verificar-frame.mjs`

```bash
node scripts/verificar-frame.mjs --captura frame.bmp
node scripts/verificar-frame.mjs --captura frame.bmp --referencia ref.bmp --rotacao ccw
node scripts/verificar-frame.mjs --captura frame.bmp --png saida.png
```

Sem dependência externa: lê BMP (24 ou 32 bpp, de cima ou de baixo para cima)
com o `Buffer` do node e escreve PNG com `zlib`. Não há ImageMagick no PC nem no
aparelho — e não precisa haver.

Sai com código 1 quando desconfia do quadro:

| Sintoma | Leitura |
| --- | --- |
| menos de 8 cores distintas | o render parou cedo |
| mais de 90 % de preto puro | tela apagada ou letterbox |
| menos de 90 % de acerto na comparação | rotação errada |

Quando a comparação falha com as dimensões certas, o script sugere o **outro**
sentido, porque `cw` e `ccw` diferem exatamente 180° e essa é a única
alternativa possível.

### A armadilha do driver de vídeo

`--dump-frame` só produz conteúdo sob `SDL_VIDEODRIVER=dummy` no PC. No driver
`x11` o SDL troca o buffer ao apresentar, e a leitura de volta vem **preta** com
a UI desenhada — já aconteceu, e o quadro 100 % preto parecia bug do app. No
aparelho, `kmsdrm` com `SDL_RENDER_DRIVER=software` funciona.

## `compilar-launcher.sh`

```bash
bash scripts/compilar-launcher.sh
```

Compila para x86-64 sem root. Extrai os `.deb` do SDL2 em `vendor/sdl2` e cria o
symlink de `libasound.so`, que o linker exige mas o pacote de runtime não traz.

## `compilar-launcher-arm64.sh`

```bash
bash scripts/compilar-launcher-arm64.sh
bash scripts/compilar-launcher-arm64.sh --instalar root@192.168.1.4
```

Cross-compila para `aarch64-unknown-linux-gnu` e copia para
`app/launcher/bin/`. Detalhes das quatro armadilhas de link em
[compilar-launcher.md](./compilar-launcher.md).

## `validate-app.mjs`

```bash
node scripts/validate-app.mjs --all
node scripts/validate-app.mjs app/store
node scripts/validate-app.mjs --all --json
node tests/run-all.mjs
```

Regras de `apps.json`, `metadata.json` e `forge.json`, incluindo a recusa
mecânica de GPL declarada em `distribution: "image"`. Ver
[metadata.md](./metadata.md) e [licenciamento.md](./licenciamento.md).

## O que ainda falta

- **`aparelho.sh` depende de `ssh` e `scp` no PC.** Não há plano B.
- **`input` não decodifica o bitmask de capacidades.** `evtest --query` devolve
  0 para qualquer código nesta imagem, então descobrir o código de um botão
  exige ler `/proc/bus/input/devices` à mão. Automatizar isso é o próximo passo
  natural.
- **Nenhuma ferramenta cobre áudio além de listar o mixer.** Testar volume fim a
  fim ainda é manual (`alsactl` + arquivo de persistência).
- **Não há teste de toque.** Não existe `/dev/uinput` nem `evemu` no aparelho,
  então não dá para injetar evento: toque só se testa com o dedo.
