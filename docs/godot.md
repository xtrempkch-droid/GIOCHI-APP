# Godot no aparelho: por que `runtime: godot` sai Incompatível

> Medido em 06/11/2026 no X55 (RK3566, glibc 2.38, kernel 6.12), com Godot
> **4.7.2-stable** oficial. Esta página existe porque o `forge` precisa
> justificar o veredito que dá — e porque a conclusão não é óbvia.

## A conclusão, em uma linha

**Nenhum build oficial do Godot tem driver de vídeo KMSDRM.** O aparelho não tem
X11 nem Wayland, então um app Godot exportado para aarch64 abre, tenta `X11` →
falha, tenta `Wayland` → falha, cai em `Headless` → **não desenha nada**.

Por isso `forge.json` com `runtime: "godot"` é classificado **Incompatível** em
qualquer perfil com `godot_runtime: false` (X55, X35H, X35S), e o caminho
recomendado é `runtime: "native"`.

## A medição

### 1. O aparelho desenha por DRM/GBM, não por servidor gráfico

```text
$ ssh root@<ip> 'ls /run/wayland* /tmp/.X11-unix; command -v Xorg weston sway'
(nenhum servidor grafico rodando)
Xorg: nao    weston: nao    sway: nao

libX11:            AUSENTE
libwayland-client: AUSENTE
libwayland-server: AUSENTE
libxkbcommon:      AUSENTE

libGLESv2: libGLESv2.so     ← presentes: é o que o KMSDRM usa
libEGL:    libEGL.so
libgbm:    libgbm.so
libdrm:    libdrm.so
```

É o mesmo caminho que o launcher usa via `SDL_VIDEODRIVER=kmsdrm`.

### 2. O motor não sabe desenhar assim

```text
$ strings Godot_v4.7.2-stable_linux.x86_64 | grep -oE 'DisplayServer[A-Za-z]+' | sort -u
DisplayServerEnums
DisplayServerHeadless
DisplayServerWayland
DisplayServerX

$ strings ... | grep -ic kmsdrm
0
```

E o template de exportação aarch64 é o mesmo binário com outro alvo:

```text
$ strings templates/linux_release.arm64 | grep -oE 'DisplayServer[A-Za-z]+'
DisplayServerHeadless  DisplayServerWayland  DisplayServerX
$ strings templates/linux_release.arm64 | grep -ic kmsdrm
0
```

**Não é falta de configuração** — é uma plataforma compilada fora do build.

## As três saídas possíveis

| Saída | O que custa | De quem é |
| --- | --- | --- |
| **`runtime: "native"`** (o que este repositório faz) | escrever/portar em Rust, C++ ou Python sobre SDL2 | do app |
| Recompilar o Godot com driver KMSDRM | horas de cross-compile de um motor inteiro; não é build oficial, então cada atualização repete o trabalho | caro, e recomeça a cada versão |
| X11 ou Wayland no rootfs | ~40 MB de imagem, mais um compositor para manter | **Giochi System** |

A escolha é `native` porque o launcher já provou que SDL2 + KMSDRM funciona nos
três aparelhos, e porque um binário nativo não pede nada à imagem além do que já
está lá. Trocar por Godot compilado sob medida só faria sentido se o número de
apps Godot crescer ao ponto de manter o motor valer a pena.

## O que isso significa para o `forge`

| `godot_runtime` no perfil | veredito para `runtime: "godot"` |
| --- | --- |
| `false` (medido que não tem) | **Incompatível**, com o motivo escrito |
| `true` (medido que tem) | segue a checagem de versão |
| `None` (não medido) | **não reprova** — não medir não é reprovar |

Os perfis genéricos (`rk3326-1gb`, `rk3326-2gb`, `rk3566-2gb`) declaram `None`
de propósito: o número do chip não prova nada sobre o motor. Ver
[forge.md](./forge.md) e [AGENTS.md](../AGENTS.md) §9.
