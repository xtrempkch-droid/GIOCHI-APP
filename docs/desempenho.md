# Desempenho medido no aparelho

> Responde ao **item 5** da [issue #7](https://github.com/xtrempkch-droid/GIOCHI-APP/issues/7):
> *"medir RSS e draws por quadro **no aparelho**, não no desktop"*.
>
> Medido em 27/11/2026 no **R35S** (`built_at_utc` da imagem `main-37e08b19`,
> 981 MiB de RAM, Mali-G31 MP2, painel 640×480) e no PC como contraprova.
>
> O comando que produz cada número está em cada seção. Onde não medi, está
> escrito (R10).

---

## 1. O comando

```bash
SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher --desempenho 15
```

Ele roda 15 s, imprime uma linha e sai:

```text
desempenho: 2929 chamada(s) de desenho em 3 quadro(s) desenhado(s) de 873
            (15.0 s) — 976 por quadro · RSS 10 MiB (pico 10 MiB)
```

Três detalhes que fazem o número valer:

- **conta chamada de backend**, não chamada de função pública: `fill_round_rect`
  são várias bandas, e cada banda é uma submissão ao SDL. É a submissão que
  limita este aparelho (49,0 kdraws/s medidos pelo Giochi System, contra 90,7 na
  X55, com 4000 retângulos pequenos);
- **divide por quadro desenhado, não por iteração do laço**: o `render()` sai
  cedo quando nada mudou (`dirty`), e o Home parado desenha 3 quadros em 873.
  Dividir pelas iterações dava **10 por quadro** num quadro que custa 976 — a
  régua medindo a intenção (R4 do documento de regras);
- **RSS vem de `/proc/self/status`** (`VmRSS` e `VmHWM`), que é o que o kernel
  contabiliza — não é estimativa nem soma de `malloc`.

## 2. Os números

| | R35S (640×480) | PC (1280×720, `dummy`) |
| --- | --- | --- |
| chamadas por quadro (Home, 4 jogos + 1 app) | **976** | **1160** |
| RSS do launcher | **10 MiB** | **24 MiB** |
| pico de RSS | **10 MiB** | 24 MiB |
| quadros desenhados em 15 s, tela parada | 3 de 873 | 2 de 239 |

O RSS do aparelho é **menor** que o do PC, e isso não é erro de medida: o
binário é `release` (884 KB contra 18 MB de `debug`) e a resolução é um quarto.

**O que isso quer dizer para o orçamento.** 976 chamadas a 49,0 kdraws/s dão
**~50 fps** de teto se o launcher redesenhasse sem parar. Ele não redesenha: o
Home parado custa **zero** submissão por quadro. O teto só importa quando algo
força quadro novo — e é exatamente o que a camada de efeitos faz.

### A camada de efeitos (Star Dust, mesmo aparelho)

```bash
/opt/giochi/apps/launcher-star-dust/bin/giochi-launcher-star-dust \
  --desempenho 12 [--efeitos|--sem-efeitos]
```

| Modo | chamadas/quadro | quadros desenhados em 12 s | fps efetivo |
| --- | --- | --- | --- |
| efeitos **desligados** | 976 | 4 de 696 | desenha só quando muda |
| efeitos **ligados** | 975 | **286** de 344 | **23,8** |

Duas conclusões, e a segunda é a que importa:

1. a **camada** custa ~0 chamada por quadro além da Home — ela não infla o
   quadro, ela **força** quadro novo;
2. com efeitos ligados o aparelho faz **23,8 fps**, e não os 30 que a cadência
   pede. Ou seja: no R35S a camada **não sustenta** os 30 fps. Isso não aparecia
   em nenhuma medição anterior porque todas eram de desenho correto, não de
   ritmo.

## 3. O que **não** foi medido

- **Uso de GPU**: `--desempenho` roda com `SDL_RENDER_DRIVER=software` para ser
  reprodutível, e o número de submissão não depende do backend. O tempo de GPU
  do `opengles2` **não** foi medido — o que se mede aqui é o teto de submissão,
  que é o gargalo documentado pelo System.
- **RSS com um jogo aberto** (launcher + jogo somados): o launcher destrói o
  `Painter` e espera o filho, então a soma nunca acontece ao mesmo tempo. O que
  importa é o RSS do jogo, que não é deste repositório.
- **X55 e X35H**: só o R35S foi medido; os outros dois ficam com o número do
  System (90,7 kdraws/s) como referência.
- **`musica` e `launcher-teste`**: não têm `--desempenho`; o contador existe no
  `Painter`, mas não há flag que o exponha.

## 4. Como repetir

```bash
bash scripts/compilar-launcher-arm64.sh --instalar
ssh root@10.55.0.2 'SDL_VIDEODRIVER=dummy SDL_RENDER_DRIVER=software \
  /opt/giochi/apps/launcher/bin/giochi-launcher --desempenho 15'
```

O número muda com o catálogo: **cada cartão a mais custa ~80 chamadas** (é o
orçamento que o `draw.rs` documenta). Comparar duas medições exige o mesmo
catálogo — a mesma armadilha que o `verificar` já resolve congelando o índice.
