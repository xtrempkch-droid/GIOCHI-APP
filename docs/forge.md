# `forge.json` — requisitos do app

Declara **o que o app precisa do aparelho**. Schema em
[`schemas/forge.schema.json`](../schemas/forge.schema.json).

Todas as chaves são opcionais. Ausente o arquivo, o sistema assume os defaults.

## Campos

| Campo | Tipo | Default | Valores |
| --- | --- | --- | --- |
| `arch` | enum | `aarch64` | `aarch64` |
| `runtime` | enum | `godot` | `godot`, `retroarch`, `proton`, `native` |
| `graphics_api_min` | enum | `gles3` | `none`, `gles2`, `gles3`, `vulkan` |
| `graphics_api_fallback` | enum | `gles2` | `gles2`, `gles3` |
| `godot_version` | string | — | major.minor, ex.: `4.3` |
| `ram_min_mb` | inteiro ≥ 0 | `0` | |
| `vram_min_mb` | inteiro ≥ 0 | `0` | |
| `display_min_width` | inteiro ≥ 0 | `0` | |
| `display_min_height` | inteiro ≥ 0 | `0` | |
| `touchscreen_required` | booleano | `false` | |

## Por que NÃO declara pacotes Buildroot

Esta é a decisão mais importante do contrato.

Um app que declarasse `BR2_PACKAGE_SDL2` estaria assumindo como o sistema é
montado. Três problemas:

1. **Não conhece o empacotamento.** `BR2_PACKAGE_LIBMPG123` não existe — o certo é
   `BR2_PACKAGE_MPG123`. O Buildroot **descarta símbolo inexistente em silêncio**:
   build verde, biblioteca ausente, app que não abre.
2. **Quebra a abstração de arquitetura.** O app declara o que precisa; como
   compilar é problema do sistema.
3. **Quem sabe o que a imagem tem é o Giochi System.** As dependências de pacote
   ficam no `Config.in` do pacote de sistema, via `select`.

## Como a compatibilidade é calculada

O `Giochi System` publica o perfil do aparelho (`device_profile.json`) e o
`Giochi os` compara com o `forge.json`:

| Veredito | Quando |
| --- | --- |
| **Compatível** | atende todas as exigências |
| **Parcial** | alguma exigência consome entre 80% e 100% do recurso — risco de swap |
| **Incompatível** | alguma exigência não é atendida |

O veredito é o **pior caso** e lista **todas** as exigências não atendidas, nunca
só a primeira: quem vai corrigir precisa saber tudo que falta de uma vez.

### `none`: o app não desenha nada

Ferramenta headless (a `forge`, a `store`) declara `"graphics_api_min": "none"`.
Sem esse valor o classificador assumia `gles3` e reprovava a própria `forge` num
RK3326, que só tem GLES 2.0 — medido.

O **default continua `gles3`**: omitir a chave num app gráfico tem de ser
conservador. `none` é uma declaração, não um esquecimento.

### `vulkan` é sempre incompatível

O aparelho tem GLES 3.0/2.0. Pedir `graphics_api_min: "vulkan"` resulta em
incompatível — não é bug, é o hardware.

### A tela é comparada com a área **utilizável**, não com o painel cru

O painel do X55 é 720×1280 — **retrato**. Mas nenhum app deste projeto desenha em
retrato: o espaço de projeto é paisagem (1280×720) e o sistema gira o desenho
(`fbcon/rotate=3`), o que o launcher e os jogos aplicam.

Comparar `forge.json` com o painel cru reprovava o caso mais comum: um jogo que
declara 1280×720 "não caber" num aparelho de 720×1280 **onde ele roda** — foi
exatamente o que o arcade-volleyball fez, medido.

A regra é maior lado × menor lado, que não precisa saber a rotação:

| Painel | Utilizável | Um jogo 1280×720 |
| --- | --- | --- |
| 720×1280 (X55, retrato) | 1280×720 | **cabe** |
| 640×480 (X35H, 4:3) | 640×480 | não cabe |

### `runtime: "godot"` é incompatível nos aparelhos medidos

`godot_runtime` no perfil é um fato **da imagem**, não do hardware — e é
três-estados de propósito:

| Valor | Significado |
| --- | --- |
| `false` | verificado que a imagem **não** traz o runtime — X55 e X35H, medido |
| `true` | verificado que traz |
| `null` | **não medido** (perfis genéricos de SoC) — e não medir não é reprovar |

Só `false` gera problema. Nos dois aparelhos medidos, declarar
`runtime: "godot"` sai como **Incompatível** com o motivo certo: nenhum build
oficial do Godot tem driver KMSDRM e o rootfs não tem X11 nem Wayland
([godot.md](godot.md)). O caminho é `runtime: "native"`.

Isso é ortogonal a `godot_min`/`godot_max`: aqueles dizem o que o **hardware**
suportaria.

## Perfis de aparelho

`giochi-forge check --device <nome>` sem perfil medido usa a tabela embutida:

| Nome | SoC | RAM | Painel | Godot na imagem |
| --- | --- | --- | --- | --- |
| `x55` | rk3566 | **1968 MB** medido | 720×1280 | não |
| `x35h` | rk3566 | **964 MB** medido | 640×480 | não |
| `x35s` | rk3566 | 964 MB (não medido) | 640×480 | não |
| `rk3326-1gb` | rk3326 | 1024 MB | 640×480 | não medido |
| `rk3326-2gb` | rk3326 | 2048 MB | 640×480 | não medido |
| `rk3566-2gb` | rk3566 | 2048 MB | 640×480 | não medido |

A RAM é a **medida**, não a nominal do SoC: o firmware reserva memória para a
GPU, e um jogo que declara "preciso de 2000 MB" passaria no número redondo e
morreria no real.

**O painel é do aparelho, não do chip.** Até 06/11/2026 havia um perfil só para o
RK3566, com 640×480 — o que fazia o classificador reprovar no X55 um jogo que roda
lá e aprovar no X35H um que sai com faixa preta.

Para o perfil medido de outro aparelho, aponte um arquivo:

```bash
giochi-forge check --profile device_profile.json --project .
```

## Exemplo

Um jogo nativo — o que roda de fato nos aparelhos medidos. É o `forge.json` do
`arcade-volleyball`, com os números **medidos** no X55 (19 MB de pico):

```json
{
  "arch": "aarch64",
  "runtime": "native",
  "graphics_api_min": "gles2",
  "graphics_api_fallback": "gles2",
  "ram_min_mb": 32,
  "vram_min_mb": 8,
  "display_min_width": 320,
  "display_min_height": 240,
  "touchscreen_required": false
}
```

E um projeto Godot, que hoje é **incompatível** nos dois aparelhos medidos — o
arquivo é válido, o que falta é o runtime na imagem:

```json
{
  "runtime": "godot",
  "graphics_api_min": "gles3",
  "graphics_api_fallback": "gles2",
  "godot_version": "4.7",
  "ram_min_mb": 320,
  "vram_min_mb": 24,
  "display_min_width": 1280,
  "display_min_height": 720,
  "touchscreen_required": false
}
```

O `ram_min_mb: 320` e o `vram_min_mb: 24` vêm da medição do próprio projeto no
host (255 MB / 23,8 MB), não de chute — e o `display_min_width: 1280` é o que
antes era reprovado errado no X55.
