# Verificação no aparelho e ambientes — histórico medido

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**. Resumo em
> [`AGENTS.md`](../../AGENTS.md) §11 e §12. Inclui o que congelar na
> comparação de quadro e as diferenças medidas entre X55, X35H e R35S.

---

## 11. Verificação no aparelho

```bash
bash scripts/aparelho.sh procurar     # 10.55.0.2 (usb0), depois varre a /24
bash scripts/aparelho.sh diagnostico  # sistema, tela, áudio, entrada, libs
bash scripts/aparelho.sh bluetooth    # a pilha inteira, com veredito
bash scripts/aparelho.sh instalar     # compila, instala, confere sha256
bash scripts/aparelho.sh subir        # inicia o launcher (o init não faz sozinho)
bash scripts/aparelho.sh verificar    # processo + DRM + frame comparado
bash scripts/aparelho.sh capacidades  # códigos que cada /dev/input declara
bash scripts/aparelho.sh catalogo     # gera e publica o índice de apps
bash scripts/aparelho.sh boot-log     # salva o log ANTES de regravar o cartão
```

**`verificar` é o que importa**: para o app (o KMSDRM é exclusivo), captura um
quadro, desenha a mesma Home no PC com `SDL_VIDEODRIVER=dummy`, des-rotaciona pela
orientação que o sistema declara e compara pixel a pixel. No X55 dá **98,9 %** e
no X35H **99,9 %**; com a rotação oposta cai para **10,9 %**. É essa diferença que
distingue "desenhou" de "desenhou de ponta a cabeça".

A comparação só vale com a **mesma entrada dos dois lados**: catálogo igual e
referência no espaço de projeto. As duas coisas já faltaram, e das duas vezes o
launcher pareceu pior sem ter regredido.

**Quatro coisas do ambiente mudam e precisam ser congeladas dos dois lados**,
senão a comparação reprova por hardware: relógio (aparelho sem NTP), bateria,
temperatura e **idioma**.

| Variável | Onde | Efeito |
| --- | --- | --- |
| `GIOCHI_HORA_FIXA` | `views::current_clock` | desenha o texto dado |
| `GIOCHI_BATERIA_FIXA` | `app::tick_energia` | nível fixo |
| `GIOCHI_TEMPERATURA_FIXA` | idem | °C fixo |
| `--idioma en` (argumento novo) | `main.rs` → `App::new` | abre no idioma dado e **não** grava |

Antes das três primeiras cada uma derrubava a comparação para 99,5–99,9 % e
nenhuma era regressão de desenho. Depois: **100,0 %** nos dois aparelhos.

O idioma entrou nesta lista em 15/11/2026 por um motivo diferente dos outros três:
ele não muda sozinho, mas muda **por escolha**, e um PC sem
`/user/.config/giochi/language` desenha a tela de boas-vindas enquanto o aparelho
já configurado desenha a Home. Por isso os scripts de captura passam `--idioma en`
dos dois lados (§6.10).

O `retry` de 99,5 % também já esteve **invertido**:
`awk 'exit !(p < 99.5)' && break` quebrava o laço quando o resultado era **bom**,
então a segunda tentativa nunca acontecia justamente no caso que ela cobre (o
minuto virando entre as duas capturas).

## 12. Ambientes medidos

| | X55 | X35H | R35S |
| --- | --- | --- | --- |
| SoC / GPU | RK3566 / Mali-G52 | RK3566 / Mali-G52 | RK3326 / Mali-G31 |
| painel | 720×1280 | 640×480 | 640×480 |
| `fbcon/rotate` | `0` + cmdline `3` → **Ccw90** | `0` → None | `0` → None |
| RAM | 1968 MB | 964 MB | 964 MB |
| acesso | `usb0` `10.55.0.2` + Wi-Fi | `usb0` + rede | `usb0` ou serial |
| `KEY_POWER` | sim (`rk805`) | sim | **não** |
| Wi-Fi / BT | sim | sim | **sem rádio** |
| `libasound` | sim | sim | **sim** na imagem de 15/11 (§12.1) |
| `SDL2_ttf` | 2.22 | 2.22 | presente |

### O R35S mudou na imagem de 15/11/2026

A tabela acima é de 06/11 e **não vale mais para o R35S**. Medido na imagem
`built_at_utc=2026-11-15T12:58:43Z`, com o launcher instalado por SSH:

| | 06/11/2026 | 15/11/2026 |
| --- | --- | --- |
| `libasound` | ausente → volume `—` | **presente**, mixer `rk817_int`, Master 255 |
| `SDL2` / `SDL2_mixer` / `SDL2_image` | — | **2.28.5 / 2.6.3 / 2.0.8** — é o que faz o app de música abrir (§7) |
| `/dev/input` | — | `rk817_int`, `gpio-keys-vol`, `r36s_Gamepad` (**17** códigos) |
| `KEY_POWER` | não declarado | **continua não declarado** — o launcher avisa e o menu fica no START |
| rotação | `fbcon/rotate=0` → None | igual, e a referência bate **100,0%** |
| catálogo | — | 3 apps em `/user/apps`, quadro idêntico ao PC |

O `buttons.map` tem **19 entradas** e todas são símbolos que o `code_for`
conhece (`verificar-mapa.mjs`: 0 desconhecidos). Isso importa porque o mapa é
gerado do DTB e traz comentário no fim de cada linha — foi um comentário
desses que já deixou 1 de 13 botões funcionando.

**Permissão de binário instalado por `scp` depende do umask do aparelho.** Medido:
umask `077` no R35S, então `chmod +x` deixava o launcher `700` enquanto o mesmo
app empacotado pelo `empacotar.mjs` chega `755` (o tar escreve modo explícito).
Mesma família do `tar --mode` que mascara permissão: o defeito não é o modo
errado, é ele depender de um estado ambiente que ninguém declarou. Hoje os dois
scripts de instalação fazem `chmod 755`.

**Não confie no nome do aparelho nem no IP.** Leia
`/etc/giochi/build-info.txt`; o `10.55.0.2` já foi X35H e R35S no mesmo dia, e a
host key muda ao regravar (`ssh-keygen -R 10.55.0.2` — a mensagem fala de ataque
MITM e não é).

**O IP do Wi-Fi é DHCP e muda entre boots.** O mesmo X55 respondeu
`192.168.1.7` numa sessão e `192.168.1.2` na seguinte, e a host key é outra —
`sem host key` é endereço reciclado, não aparelho diferente. Hardcodar o IP
rende `Host key verification failed` que parece defeito de rede. `aparelho.sh
procurar` varre a `/24` justamente por isso.

O R35S não tem rádio e o `dwc2` nasce como *host*: `aparelho.sh` (feito de
`ssh`/`scp`) não o alcança sem `usb0`. Quando o USB não sobe, a console serial
(`ttyS2` @ 1500000) é o único caminho:

```bash
bash scripts/console-r35s.sh "uname -r"      # um comando
bash scripts/console-r35s.sh --escutar 12    # só ouve (boot, panic)
```

Por dentro é um container `--privileged -v /dev:/dev` com a imagem do System: o
nó é `root:dialout` e o CH341 **cai do barramento**, então `-v /dev:/dev`
sobrevive e `--device=` fixado aborta quando o nó some. Leitores órfãos seguram a
porta e o erro aparece como `returned no data`, que manda procurar defeito no
cabo.
