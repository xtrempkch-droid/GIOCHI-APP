# `forge.json` — requisitos do app

Declara **o que o app precisa do aparelho**. Schema em
[`schemas/forge.schema.json`](../schemas/forge.schema.json).

Todas as chaves são opcionais. Ausente o arquivo, o sistema assume os defaults.

## Campos

| Campo | Tipo | Default | Valores |
| --- | --- | --- | --- |
| `arch` | enum | `aarch64` | `aarch64` |
| `runtime` | enum | `godot` | `godot`, `retroarch`, `proton`, `native` |
| `graphics_api_min` | enum | `gles3` | `gles2`, `gles3`, `vulkan` |
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

### `vulkan` é sempre incompatível

O aparelho tem GLES 3.0/2.0. Pedir `graphics_api_min: "vulkan"` resulta em
incompatível — não é bug, é o hardware.

## Exemplo

```json
{
  "runtime": "godot",
  "graphics_api_min": "gles3",
  "graphics_api_fallback": "gles2",
  "godot_version": "4.3",
  "ram_min_mb": 512,
  "vram_min_mb": 64,
  "display_min_width": 640,
  "display_min_height": 480,
  "touchscreen_required": false
}
```
