# Contrato app × sistema: quem corrige o quê

Regra que este documento existe para sustentar: **característica de aparelho se
resolve no sistema; comportamento errado se resolve no app.** Um app que carrega
exceção de modelo fica errado na primeira revisão de hardware, e o sintoma
("de ponta a cabeça") não aponta para a causa.

Levantado em 03/11/2026 com o aparelho `giochi-system-x55` em mãos.

## Auditoria do que foi corrigido

| Achado | Camada | Por quê |
| --- | --- | --- |
| Nenhum botão acordava da tela de sono | **app** | só toque e teclado chamavam `Wake`; o caminho do gamepad não. Nada de aparelho envolvido. |
| Dormir não apagava o painel | **app** | o app nunca escreveu no backlight. `/sys/class/backlight` é interface genérica do Linux. |
| Slider de brilho não fazia efeito | **app** | mudava `settings.brightness` e nada mais. |
| Teclas de volume não faziam efeito | **app** | `volup`/`voldown` não estavam em `code_for`, então o mapa era descartado; e o volume nunca ia ao mixer. |
| `buttons.map` parseava 1 de 13 botões | **app** | o formato do arquivo (`#` no fim da linha) está correto; quem não seguia era o parser. |
| Rotação saía de cabeça para baixo | **app** (era remenda) | existia `DEFAULT_ROTATION = Ccw90` com o comentário "o X55 precisa". O sistema **já declara** a orientação; o app não lia. |
| `inittab` sem `giochi-init` | **sistema** | o script existe e está correto, mas nada o chama no boot. |
| Falta `SDL2_ttf`/freetype/fonte | **sistema** | dependência de imagem. Ver [dependencias-aparelho.md](dependencias-aparelho.md). |

Os três primeiros são app e ficaram no app. O quinto era app **usando**
característica de aparelho — foi movido para leitura de sistema. Os dois últimos
estão documentados como pedido ao Giochi System.

## O que o sistema já fornece (e o app passou a ler)

| Necessidade | Interface | Estado no X55 |
| --- | --- | --- |
| Orientação do painel | `/sys/class/graphics/fbcon/rotate` (de `fbcon=rotate:N`) | **`3`** = 90° anti-horário |
| Brilho do painel | `/sys/class/backlight/*/brightness` + `max_brightness` | **833 / 1666** |
| Volume | mixer ALSA via `/dev/snd/controlC0` (libasound) | **`Master Playback Volume`, 0..255** |
| Estado ALSA entre boots | — | **o sistema não guarda**; o app persiste |
| Brilho salvo entre boots | `/user/.config/giochi/brightness` (lido pelo `giochi-init`) | existe o contrato, o arquivo ainda não |
| Mapa de botões | `/etc/giochi/buttons.map` | presente, 19 teclas + calibração de eixos |
| Dispositivos de entrada | `/dev/input/event*` | 5 |
| Tela | `/dev/dri/card0` | KMSDRM funciona |
| Reinício da UI após um jogo | `giochi-init` | **não é chamado no boot** |

### Semântica de `fbcon=rotate:N`

Giros de 90° no sentido horário, conforme `Documentation/fb/fbcon.rst`:

| `N` | Kernel diz | App faz |
| --- | --- | --- |
| 0 | normal | sem rotação |
| 1 | 90° horário | gira horário |
| 2 | 180° | **avisa que não suporta** e pede configuração manual |
| 3 | 270° (= 90° anti-horário) | gira anti-horário |

`2` não é atendido de propósito. Um app que finge cumprir um pedido que não
cumpre é pior que um que avisa no console.

## Precedência da rotação (do mais específico ao mais genérico)

| Ordem | Fonte | Recarrega ao vivo | Uso |
| --- | --- | --- | --- |
| 1 | `GIOCHI_ROTATE` | não | depuração em campo |
| 2 | `/user/.config/giochi/rotation` | **sim** | escolha do usuário |
| 3 | `/sys/class/graphics/fbcon/rotate` | sim | **declaração do sistema** |
| 4 | formato do output | — | último recurso, registrado como "deduzido" |

O log na abertura diz qual fonte venceu:

```text
rotação: Ccw90 (fonte: sistema (fbcon/rotate)) [output 720x1280]
```

Isso é o que transforma "a tela está torta" em uma frase diagnosticável: ou o
sistema declarou errado, ou o usuário sobrepôs, ou ninguém declarou.

## Desligar e reiniciar

O núcleo só troca de tela (`Screen::Shutdown` / `Screen::Reboot`); quem executa
`poweroff` / `reboot` é a camada de UI (`crates/ui-sdl/src/system.rs`). O reducer
continua puro e testável sem hardware.

`Screen::Reboot` é distinto de `Screen::Boot` de propósito: `Boot` também é o
estado inicial, e sem a variante separada "reiniciar" e "primeiro boot" seriam
indistinguíveis — ninguém saberia quando chamar `reboot`.

### Medição: `poweroff` desliga de verdade neste aparelho

Testado em 03/11/2026 com o aparelho em mãos: `poweroff` cortou a energia e o
aparelho precisou ser religado pelo botão físico.

**Correção de uma conclusão errada.** A primeira versão do diagnóstico afirmava
que, sem `rockchip,system-power-controller` no device tree, o `poweroff` apenas
pararia o sistema sem cortar energia. A propriedade de fato **está ausente** no
X55, mas o desligamento funciona — o driver do PMIC registra o caminho de outra
forma. A conclusão foi escrita a partir do comportamento documentado do kernel,
sem teste, e o teste a desmentiu.

O diagnóstico hoje é informativo, e diz isso na própria saída:

```text
$ giochi-launcher --checar-energia
system-power-controller no device tree: ausente

isto é informação, não veredito: no X55 a propriedade é ausente
e `poweroff` desliga o aparelho de verdade (medido em 03/11/2026).
para saber se desliga, o caminho é desligar.
```

Regra que fica: **ausência de uma propriedade de device tree não autoriza
prever o comportamento de um comando.** Só o comando responde.

### Saídas para falha

Se `poweroff`/`reboot` falhar:

- a mensagem vai para a tela (não dá para dizer "desligado" e continuar ligado);
- a tentativa não se repete a cada quadro, para não inundar o console;
- em `Reboot`, qualquer botão devolve a interface — sem isso o usuário ficaria
  preso em "Reiniciando..." para sempre, já que o processo continuaria vivo.

## O que o app NÃO deve conter

- Nome de aparelho (`x55`, `rk3566`) em lógica.
- Sentido de rotação fixo.
- Caminho de backlight fixo em `card0` ou similar.
- Suposição de resolução de painel na lógica de navegação.

O app pode (e deve) ter **fallback** para quando o sistema não declara nada — mas
o fallback é registrado como palpite, não vendido como fato.

## O que o app fornece ao sistema

Dois arquivos, no diretório que o `giochi-init` já usa:

| Arquivo | Quem escreve | Quem lê | Conteúdo |
| --- | --- | --- | --- |
| `/user/.config/giochi/brightness` | **app** (ao mudar o brilho) | `giochi-init` no boot | inteiro na escala do hardware (`0..max_brightness`) |
| `/user/.config/giochi/rotation` | usuário/sistema | app, a cada segundo | `0`, `90`, `270` ou `ccw` |
| `/user/.config/giochi/volume` | **app** | app na abertura | 0..100 |

**O brilho precisa ser escrito pelo app.** O `giochi-init` restaura o arquivo no
boot; se o app não o escreve, a escolha do usuário some a cada reinício e o init
restaura um valor que ninguém pediu — ou, pior, reacende com o padrão de código.

**E o app também restaura.** Como o `inittab` não inicia o `giochi-init`, o
brilho salvo nunca era aplicado no boot: medido no aparelho, arquivo `1332` e
hardware em `833` (o padrão do painel). O app passou a reaplicar o arquivo na
abertura, igual ao que já fazia com o volume. É idempotente: quando o `inittab`
for corrigido, os dois aplicam o mesmo valor.

## Pendências de sistema, em ordem de impacto

1. **`inittab` sem `::respawn:/usr/bin/giochi-init`** — a UI não sobe no boot e
   não reinicia após um jogo terminar. Um aparelho que só mostra interface por
   SSH não é um console.
2. **`SDL2_ttf` + freetype + fonte** — sem texto não há interface usável.
   Detalhe em [dependencias-aparelho.md](dependencias-aparelho.md).
3. **Declarar orientação de forma explícita** — hoje só existe `fbcon=rotate:`,
   que é do console framebuffer. Funciona como sinal, mas o correto a longo
   prazo é a propriedade `panel orientation` do DRM ou uma linha em
   `/etc/giochi/display.conf`. Enquanto não existir, o app documenta que lê
   `fbcon`.
4. **Escrever `brightness` no primeiro boot** — se o arquivo não existir, o app
   assume a metade do `max_brightness`. É aceitável, mas um valor inicial
   definido pela imagem é melhor.

## Verificação de que o app não regrediu para remenda

```bash
grep -rn "x55\|X55\|rk3566" app/launcher/crates/ | grep -v "^.*//"
```

Fora de comentário que **explica por que não hardcodar**, não deve haver
resultado. O teste também existe em `display.rs`: a leitura de orientação é
testada com arquivo temporário, sem depender de hardware.
