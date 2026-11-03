# Ferramentas

Os scripts deste repositório existem porque o aparelho é a única fonte de
verdade e testar nele à mão custa tempo. Cada um nasceu de um erro concreto —
nenhum foi escrito "por completude".

## Resumo

| Ferramenta | Roda onde | Para quê |
| --- | --- | --- |
| `scripts/aparelho.sh` | PC | operação e diagnóstico do aparelho por SSH |
| `scripts/verificar-frame.mjs` | PC | decide se o quadro renderizado é a UI ou lixo |
| `scripts/capacidades-input.mjs` | PC | quais códigos de tecla cada `/dev/input` declara |
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
bash scripts/aparelho.sh capacidades     # códigos que cada dispositivo declara
bash scripts/aparelho.sh opcoes          # qual rota e autenticação seriam usadas
bash scripts/aparelho.sh boot-log       # salva /root/giochi-boot.log ANTES de regravar
```

Variáveis:

| Variável | Para quê |
| --- | --- |
| `GIOCHI_DEVICE` | fixa o host, pulando a descoberta |
| `GIOCHI_SUBREDE` | prefixo da varredura (padrão `192.168.1`) |
| `GIOCHI_CANDIDATOS` | endereços tentados antes da varredura (padrão `10.55.0.2`) |
| `GIOCHI_SENHA` | senha; sem ela, autenticação por chave |

### Por que cada subcomando existe

**O IP muda a cada boot.** É DHCP. `procurar` varre a `/24` em paralelo e
identifica o aparelho pelo hostname (`*giochi*`), não pelo endereço antigo nem
pelo MAC. Fixar o IP na memória sempre custa uma rodada de tentativas erradas.

**São dois caminhos físicos.** Pela rede o endereço é imprevisível; pelo cabo
`usb0` é fixo em `10.55.0.2` e autentica por **senha**. `procurar` tenta os
candidatos fixos **antes** de varrer 254 endereços — um segundo em vez de
minutos, e é a única forma de achar o aparelho quando ele está no USB, já que
esse endereço não pertence à faixa varrida.

`GIOCHI_SENHA` desliga o `BatchMode` do ssh. Sem isso a rota `usb0` nunca
funcionaria: `BatchMode=yes` manda o ssh **falhar** em vez de pedir senha. Com
`sshpass` instalado a senha é passada direto; sem ele, o ssh pede no terminal —
uma vez por comando, o que funciona mas incomoda.

`opcoes` mostra a decisão antes da tentativa. Existe porque "não conecta" tem
causas muito diferentes — chave, senha, IP mudou, `BatchMode` — e todas dão a
mesma mensagem do ssh:

```text
$ bash scripts/aparelho.sh opcoes

rota de acesso
  host:        10.55.0.2
  candidatos:  10.55.0.2

autenticação
aviso senha sem sshpass — o ssh vai PEDIR a senha em cada comando

comando que seria executado
  ssh -o StrictHostKeyChecking=no -o ConnectTimeout=8 root@10.55.0.2 uname -n
```

**O `inittab` não inicia o `giochi-init`.** A interface não sobe sozinha hoje;
`subir` faz o que o init deveria fazer. Isto é dívida do **Giochi System**, não
do app.

**"Processo vivo" não prova nada.** Um launcher pode estar rodando e não desenhar
nada. Por isso `verificar` exige descritores do DRM abertos e, mais importante,
compara o quadro do aparelho com o mesmo código rodando no PC.

**`pkill -f` mata a própria sessão SSH.** A linha de comando do shell contém o
padrão procurado, então o shell casa consigo mesmo. O script usa `pidof` e depois
`kill`.

### `boot-log`: a evidência que a regravação apaga

O mesmo cartão serve os dois lados da bancada. **Regravar apaga o
`/root/giochi-boot.log`** — e é exatamente no momento de regravar que a memória
falha, porque a vontade é "resolver logo". Um comando reduz isso a um passo:

```bash
bash scripts/aparelho.sh boot-log
# salvo em dist/boot-log-20261103-191530.txt (48 linhas)
```

Guarda com a data no nome em `dist/` (ignorado pelo git), então dois boots ruins
não se sobrescrevem. Se o arquivo não existir, isso **já é informação**: ou o init
não rodou, ou o caminho mudou — e a mensagem diz isso em vez de falhar em
silêncio.

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

### Testar um painel que não temos em mãos

`--tamanho LxA` cria o output no tamanho pedido. Com `SDL_VIDEODRIVER=dummy`, o
resultado é o `set_logical_size` letterboxando **exatamente como faria no
aparelho** — dá para conferir uma resolução de hardware que não está aqui.

```bash
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  ./target/debug/giochi-launcher --tamanho 640x480 --dump-frame /tmp/x.bmp
node ../../scripts/verificar-frame.mjs --captura /tmp/x.bmp
```

Medido em 03/11/2026 no código real (a UI é projetada em 1280×720):

| `--tamanho` | Proporção | Preto puro | Leitura |
| --- | --- | --- | --- |
| 1280×720 | 16:9 | **0,00 %** | nativo, sem faixa |
| 640×480 | 4:3 | **25,00 %** | letterbox de 60 px por lado |
| 1024×768 | 4:3 | **25,00 %** | idem, outra escala |
| 800×480 | 5:3 | **5,83 %** | quase cheio |
| 720×1280 | 9:16 | **68,13 %** | retrato **sem** giro — não é defeito |

A coluna de preto é o que o `verificar-frame` mede. **`25 %` num painel 4:3 é o
comportamento correto, não regressão** — é a previsão que o X35H vai produzir.

A linha de 720×1280 parece errada e não é: no PC não existe
`/sys/class/graphics/fbcon/rotate`, então o app não gira. No aparelho ele gira e
o preto cai a 0 %. Isso serve de contraprova: o letterbox é consequência da
**proporção**, e o giro é consequência da **declaração do sistema** — dois
mecanismos independentes.

### A armadilha do driver de vídeo

`--dump-frame` só produz conteúdo sob `SDL_VIDEODRIVER=dummy` no PC. No driver
`x11` o SDL troca o buffer ao apresentar, e a leitura de volta vem **preta** com
a UI desenhada — já aconteceu, e o quadro 100 % preto parecia bug do app. No
aparelho, `kmsdrm` com `SDL_RENDER_DRIVER=software` funciona.

## `capacidades-input.mjs`

```bash
node scripts/capacidades-input.mjs                      # lê o /proc local
node scripts/capacidades-input.mjs --arquivo dump.txt   # lê um dump
node scripts/capacidades-input.mjs --codigo 116         # quem emite KEY_POWER
node scripts/capacidades-input.mjs --dispositivo gpio   # um dispositivo só
```

Lê `B: KEY=` de `/proc/bus/input/devices`. É o único caminho: `evtest --query`
devolve **0 para qualquer código** nesta imagem, então não serve para descobrir
o que um botão emite.

### O formato engana

O kernel imprime as palavras de 64 bits em ordem **decrescente** — a **última**
coluna é a palavra 0. Ler da esquerda para a direita desloca todos os códigos em
múltiplos de 64 e produz um mapa plausível e errado.

```text
B: KEY=10000000000000 0     → palavra 1, bit 52 → 64+52 = 116 = KEY_POWER
B: KEY=c000000000000 0      → palavra 1, bits 50-51 → 114 e 115 = VOLUMEDOWN/UP
```

### O que ele revelou no X55

```text
rk805 pwrkey        [event0]  116 KEY_POWER
gpio-keys-control   [event3]  304-305   BTN_SOUTH (A), BTN_EAST (B)
                              307-308   BTN_NORTH (X), BTN_WEST (Y)
                              310-318   BTN_TL … BTN_THUMBR
                              544-547   BTN_DPAD_UP/DOWN/LEFT/RIGHT
gpio-keys-vol       [event4]  114-115   KEY_VOLUMEDOWN, KEY_VOLUMEUP
```

**17 códigos** no controle — que são exatamente os 17 botões citados no cabeçalho
do `buttons.map` do sistema. Os 4 do direcional ficam na faixa `BTN_` (0x220), não
na faixa `KEY_`, e é por isso que parecem códigos sem nome.

Cruzando com o app: todos os 20 códigos declarados pelo kernel têm entrada em
`code_for` e no `buttons.map`. A cobertura está completa — e agora há como provar
isso em segundos, em vez de conferir 20 linhas à mão.

### Como usar quando um botão não reage

| `capacidades` declara? | `input` mostra ao apertar? | Conclusão |
| --- | --- | --- |
| sim | sim | kernel entrega; o problema é `buttons.map` ou `code_for` |
| sim | não | driver ou DTS não registra o evento |
| não | sim | bitmask incompleto; vale como aviso, não como veredito |
| não | não | o botão não existe neste hardware |

É essa separação que evita depurar no escuro. `KEY_POWER` e as teclas de volume
ficaram meses sem função porque a pergunta certa nunca era feita.


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
- **A rota `usb0` não foi exercitada.** O código existe e a montagem das opções
  foi verificada com `opcoes`, mas nenhuma conexão real por `usb0` foi feita —
  o aparelho estava em uso pela equipe do System. `sshpass` também não está
  instalado nesta máquina, então o caminho com senha só foi validado até a
  construção do comando.
- **Nenhuma ferramenta cobre áudio além de listar o mixer.** Testar volume fim a
  fim ainda é manual (`alsactl` + arquivo de persistência).
- **Não há teste de toque.** Não existe `/dev/uinput` nem `evemu` no aparelho,
  então não dá para injetar evento: toque só se testa com o dedo.
- **O bitmask `ABS=` não é decodificado.** `capacidades-input.mjs` lê só `KEY=`.
  Os eixos analógicos declaram faixa em `ABS`, e os `axis_*_min/max` do
  `buttons.map` só podem ser conferidos contra isso — hoje é olho.
- **`boot-log` não foi exercitado.** O arquivo não existe na imagem anterior, e o
  aparelho estava em uso pela equipe do System durante esta rodada — então o
  caminho "arquivo presente" nunca rodou. O caminho "arquivo ausente" é o
  esperado hoje e está implementado com mensagem própria.
