# PortMaster — ports na aba Apps

> Implementado em 26/11/2026. Responde ao **pedido de PortMaster** registrado
> nas issues #2–#5 deste repositório (as #3, #4 e #5 são duplicatas da #2).
>
> Guia do sistema: `docs/evaluacao-portmaster.md` e `docs/entrada-para-equipes.md`
> no **GIOCHI-SYSTEM** (somente leitura — §1.1 do `AGENTS.md`).

---

## 1. O que o PortMaster faz e onde ele instala

O PortMaster é uma loja de ports que roda **no aparelho**, em `/user/ports`:

```text
/user/ports/<nome>.sh   + /user/ports/<nome>/     ← o instalado pela GUI
/user/ports/<dir>/<nome>.sh                       ← o feito à mão
```

O nome do diretório e o do arquivo **não coincidem sempre** (`FORM.sh` mora em
`form/`, `stardewvalley/StardewValley.sh`). A comparação é sem caixa, e é o que
o `giochi-ports --rodar` já faz.

O launcher varre `/opt|/user/{games,apps}` (§6.9 do `AGENTS.md`). Um port
instalado pela GUI ficaria **invisível**: existe no disco e em nenhum cartão.

---

## 2. O que foi implementado

`app/launcher/crates/ui-sdl/src/portmaster.rs`, compartilhado por `#[path]` com
o Star Dust. Na abertura — e antes da detecção — o launcher:

1. lista `/user/ports` (os dois layouts, ignora `PortMaster/` e nomes ocultos);
2. para cada port, escreve um pacote de app em `/user/apps/<slug>/`:

```text
/user/apps/portmaster-form/
├── .portmaster-port          # marca: "FORM" — só o que tem esta marca é podado
├── metadata.json             # kind: app, entrypoint: bin/portmaster-form
└── bin/portmaster-form       # 755, exec giochi-ports --rodar "FORM"
```

3. apaga os cartões marcados cujo port sumiu.

O slug é `portmaster-<nome normalizado>` e **nunca** começa com `launcher`, então
não cai na poda de tema (§8 do `AGENTS.md`). Nome sem nenhum ASCII recebe um
sufixo FNV-1a determinístico.

### Por que no launcher, e não num script do PC

Quem instala o port é a GUI **no aparelho**, sem PC por perto. A GUI roda com o
KMSDRM livre — o launcher não pode estar no ar — e o `giochi-init` reinicia o
launcher quando ela sai. Portanto o launcher é a única peça que está no
aparelho, sobe logo depois da instalação e já sabe lançar um app sem argumentos.

### Por que `entrypoint` em `bin/` e não na raiz

O pedido original sugeria `entrypoint: "<slug>"` na raiz do app. **Não
funciona**: o detector recusa `entrypoint` que não comece em `bin/` — medido no
R35S em 26/11/2026:

```text
catálogo: /user/apps/portmaster-form: entrypoint inválido no metadata.json
          (`portmaster-form` (precisa começar em bin/)) — fora do catálogo
```

O layout que funciona é `bin/<slug>`, e é o que o módulo escreve.

---

## 3. O caminho do lançamento, sem mudar o `Alvo`

`Alvo` continua sem `args`: o launcher executa o `entrypoint` com `cwd` na pasta
do app e **sem argumentos** (`docs/integracao.md` §4). A escolha do port vai
dentro do script gerado:

```sh
#!/bin/sh
exec giochi-ports --rodar "FORM"
```

`giochi-ports` resolve o resto: acha o `.sh` nos dois layouts, confere o DRM,
roda dentro do `bwrap` com o rootfs somente-leitura e devolve ao sair.

**O marcador de DRM não é criado nesse caminho**, e isso é de propósito: o
launcher destrói o `Painter` antes do `spawn` (§6.8), então quando o filho chega
não há processo com `card0` aberto. Criar `/run/giochi/dev-free-console` ali
seria mentir para o `giochi-init` sobre um console que está livre — se o port
morresse com o marcador, o launcher não voltaria.

---

## 4. Medido no R35S em 26/11/2026

Imagem `e0d74e6` (com `GIOCHI_WITH_PORTMASTER=1`), port `FORM` instalado:

| Passo | Resultado |
| --- | --- |
| `--sincronizar-ports` com um port | `1 novo(s)` |
| segunda passada | `1 igual(es)` — não reescreve (é cartão SD) |
| port removido do disco | `1 removido(s)` e o cartão some |
| port devolvido | `1 novo(s)` de novo |
| detecção com o cartão | `4 jogo(s) e 2 app(s)` |
| `--lancar portmaster-form` (o mesmo campo do botão A) | `sistema: iniciando portmaster-form (...)`, `giochi-ports: rodando FORM.sh (de /user/ports)` |
| o port dentro do sandbox | `FORM 5 s: chip 52.9 C, frames 13.5 ms, held buttons 00000` |
| `aparelho.sh verificar` com o cartão | **99.5%** — e **99.5%** sem ele (a diferença de 0,5% é anterior e não é do desenho) |

O port rodou com `renderer: software` porque o teste subiu o launcher com
`SDL_VIDEODRIVER=dummy`, e a variável foi herdada. Na produção o launcher roda
em KMSDRM e o port pega `opengles2` — foi o que o Giochi System mediu (60 fps).

---

## 5. A referência do `verificar` passou a incluir o disco

O `gerar_referencia` montava o catálogo do PC **só** a partir de
`/opt/giochi/apps/index.json`. Desde 15/11/2026 a detecção é disco-primeiro
(§6.9), então todo pacote instalado fora do índice — um app da loja, o cartão de
um port — existia no aparelho e não na referência.

Medido: **98,6%** com o cartão de port, e **99,5%** depois de somar a colheita
das quatro raízes (`colher_instalados`, a mesma de
`aparelho.sh catalogo`). O disco vence o índice, como no aparelho.

---

## 6. Como verificar

```bash
# no PC: exercita o módulo sem tocar no aparelho
GIOCHI_PORTS=/tmp/p/ports GIOCHI_APPS=/tmp/p/apps \
  ./app/launcher/target/debug/giochi-launcher --sincronizar-ports

# no aparelho, sem abrir a interface (abrir custa uma volta do giochi-init)
ssh root@<ip> '/opt/giochi/apps/launcher/bin/giochi-launcher --sincronizar-ports'

# o cartão e o script gerados
ssh root@<ip> 'cat /user/apps/portmaster-*/metadata.json'
ssh root@<ip> 'cat /user/apps/portmaster-*/bin/*'

# o caminho de lançamento de produção (o mesmo do botão A)
ssh root@<ip> 'SDL_VIDEODRIVER=dummy /opt/giochi/apps/launcher/bin/giochi-launcher --lancar portmaster-form'
```

---

## 7. O que **não** foi feito, e por quê

| Pedido da issue #2 | Estado |
| --- | --- |
| ports listados na aba Apps | **feito** — cartão por port, detectado do disco |
| formato do alvo sem mudar o launcher | **feito** — wrapper em `bin/`, sem `args` no `Alvo` |
| aba "Ports" própria e `args` no `Alvo` | **não feito** — é UX; a aba Apps já mostra os ports, e `args` mexe no contrato do `Alvo` sem necessidade hoje |
| PortMaster como item da loja | **não feito** — a loja é código do PC (`app/store`), e o pacote precisa aparecer como **sideload** (não é assinado). Registrado como pendência em `AGENTS.md` §16 |
| assinatura dos ports | **não existe, de propósito** — port é código de terceiro; o que o sistema garante é o sandbox, não o conteúdo |

Dois detalhes do sistema que quem for instalar por outro caminho precisa
repetir, medidos pela equipe do System:

1. O `gamecontrollerdb.txt` do PortMaster tem 472 KB e o `control.txt` o exporta
   inteiro em `SDL_GAMECONTROLLERCONFIG`; `MAX_ARG_STRLEN` é 128 KB e **todo
   `exec`** falha com `Argument list too long`. O `giochi-ports` monta o nosso
   DB (2 KB) por cima.
2. O zip do PortMaster sai com modo `600` sob `umask 077`; `entrypoint` `.sh`
   precisa de `+x` e shebang.
