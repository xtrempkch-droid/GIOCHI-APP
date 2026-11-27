# Regras de engenharia — adoção pelo `Giochi app`

> **Adotado em 26/11/2026**, respondendo à
> [issue #7](https://github.com/xtrempkch-droid/GIOCHI-APP/issues/7).
>
> O documento base é
> [`docs/regras-para-outras-equipes.md`](https://github.com/xtrempkch-droid/GIOCHI-SYSTEM/blob/main/docs/regras-para-outras-equipes.md)
> no `GIOCHI-SYSTEM`, no commit
> [`7eef035`](https://github.com/xtrempkch-droid/GIOCHI-SYSTEM/commit/7eef035797aecd7c643defb7ad66b9c8c3688009)
> (*"docs: as regras de engenharia vão para as outras equipes"*, 26/11/2026).
>
> **Este arquivo não reescreve as regras.** Ele diz onde cada uma já está
> aplicada neste repositório, onde não está, e o que foi medido aqui. Duas
> cópias do mesmo texto divergem no dia em que uma muda (§R6) — a verdade é a
> do documento base.

---

## R1 — Medir antes de afirmar

**Aplicada.** Cada número do `AGENTS.md` carrega o comando que o produziu, e a
regra de honestidade do §14 diz que "compila" e "roda" são afirmações
diferentes. O que **não** foi medido fica escrito como tal (§R10, abaixo).

Contraexemplo que este repositório já pagou: `docs/controle-virtual.md` foi
escrito **antes** de o controle virtual existir no aparelho, e diz isso na
primeira linha em vez de sugerir que funciona.

## R2 — Verde não é correto

**Aplicada, e é a regra que mais mudou teste aqui.** Todo teste novo é conferido
**revertendo a correção de propósito**:

| Correção | O que reprova sem ela |
| --- | --- |
| `SIGTERM` durante o filho (`system.rs`) | `sigterm_arma_no_lancamento_e_e_anotado` |
| cartão da loja limpa `.pugwash-reboot` | `o_cartao_da_loja_limpa_o_marcador_de_reboot_do_pugwash` |
| cartão da loja não é podado | `a_loja_do_portmaster_vira_cartao_com_credito` |
| `PYSDL2_DLL_PATH` no script gerado | o mesmo teste, na asserção da variável |

O caso medido do lado do app: a suíte passava "164/164" **com menos cobertura**,
porque o runner descobria os testes por uma lista fixa (§R4).

## R3 — Pergunte pela consequência, não pelo indireto

**Aplicada — e o `AGENTS.md` §13 é uma lista delas.** Três do lado do app:

| O indireto | O que ele não distingue |
| --- | --- |
| "o processo do launcher está vivo" | estar **desenhando** — sem os descritores de `card0` ele está vivo e a tela está parada |
| "o descritor `card0` está aberto" | quem tem o **master** — o `giochi-ports` pergunta por descritor, e foi por isso que o cartão do FORM não abria |
| "o ícone do app aparece na grade" | o app **abrir** — é o cartão que aparece e não abre |

## R4 — Teste que pergunta a coisa errada é pior que nenhum

**Aplicada.** `verificar-frame.mjs` foi feito exatamente por isso: contar cores
distintas e "não é preto puro" reprovavam um quadro girado de cabeça para baixo.
A régua do projeto é comparação pixel a pixel com tolerância 12 e amostra 3×3.

E o runner de testes: `tests/run-all.mjs` descobre os `.test.mjs` por `readdir`,
nunca por lista — a lista fixa já deixou um arquivo novo de fora.

## R5 — "Não existe" e "existe desligado" dão o mesmo sintoma

**Aplicada.** `scripts/verificar-mapa.mjs` lê a lista de símbolos do **binário**
(`--simbolos`) em vez de uma cópia, e o `ButtonsMap` avisa o que não reconheceu:
um símbolo desconhecido é descartado em silêncio e o botão simplesmente não
funciona. Aconteceu duas vezes (`KEY_VOLUME*`, comentário no fim da linha).

## R6 — Uma fonte da verdade, ou duas que divergem

**Aplicada, e é a razão de existir o `#[path]`.** `input_evdev.rs`, `catalogo.rs`,
`system.rs` e mais doze módulos são **os mesmos arquivos** no launcher, no Star
Dust e no `musica`. O núcleo do Star Dust já foi uma cópia e ficou 400 linhas
para trás: o botão "Conectar" aparecia e não funcionava.

E é a regra que a issue #6 do controle virtual estende ao **input**: uma fonte
só, nunca `evdev | sdl`. Ver `docs/controle-virtual.md` §3.3.

## R7 — Repositório alheio é somente leitura

**Aplicada** desde 15/11/2026 (`AGENTS.md` §1.1), e endurecida em 26/11/2026 com
a regra que faltava: **contornar aqui não substitui a issue**. O caso que a
gerou é o `giochi-ports` sem `PYSDL2_DLL_PATH` — o cartão da loja contorna, e o
pedido 6 de `docs/issues-para-outros-repos.md` existe para a correção ir ao
wrapper, onde vale para todos.

## R8 — Licença antes de recurso

**Aplicada mecanicamente.** `validate-app.mjs` recusa GPL/AGPL/LGPL com
`distribution: "image"` (`licenca_gpl_na_imagem`), e o Giochi OS repete a
checagem de propósito — o empacotamento não pode depender de o outro repositório
ter rodado o validador.

O caso concreto: o pacote do PortMaster (`gptokeyb2` GPL-2.0, `7-Zip` LGPL-2.1)
**não** entra na imagem; é instalado em `/user` pela GUI, e o cartão da loja
declara a licença e a URL do projeto.

## R9 — Não confie no relatório: reproduza

**Aplicada.** `AGENTS.md` §11–§12: todo ambiente tem commit e data, e
`aparelho.sh diagnostico` lê `/etc/giochi/build-info.txt` em vez de confiar no
nome do aparelho ou no IP — o `10.55.0.2` já foi X35H e R35S no mesmo dia.

## R10 — Registre o que NÃO foi medido

**Aplicada, e é a seção que mais existe em documento daqui.** Três exemplos:

- `docs/controle-virtual.md` — "**não implementado**" na primeira linha;
- `AGENTS.md` §14 — a coluna diz *como* cada alegação foi medida, e "corrigido no
  config, imagem não medida" é uma resposta legítima (Bluetooth);
- `docs/musica.md` — a barra de progresso **não existe** porque o `SDL_mixer` não
  informa a posição, e uma barra mostraria número inventado.

## R11 — Meça no aparelho, não no desktop

**Aplicada.** É a razão de `aparelho.sh verificar` existir: comparar o quadro do
aparelho com a referência desenhada no PC, des-rotacionada. Quatro coisas do
ambiente são congeladas dos dois lados (relógio, bateria, temperatura, idioma) e
o **catálogo** também — sem ele a comparação reprova por entrada diferente, não
por desenho.

O número de hoje está em [`docs/desempenho.md`](desempenho.md).

## R12 — Estado que sobra mente sobre a próxima execução

**Aplicada — e este repositório achou duas no mesmo dia (26/11/2026):**

| Estado | O que ele fez |
| --- | --- |
| `.pugwash-reboot` de 20/11 | a GUI do PortMaster abria e fechava em 1 s, sem mensagem |
| `.pugwash-reboot` num laço de amostragem | fez o teste medir 60 s de nada e concluir "abriu" |

Mais o marcador de DRM (`/run/giochi/dev-free-console`), que é do sistema: se o
dono morrer por `kill -9`, o `trap` não roda e o aparelho volta **sem
interface**. A autocura do `giochi-init` lê o PID ao lado do marcador.

---

## Checklist aplicado a este repositório

- [x] Rodei o comando que sustenta cada afirmação — §14 do `AGENTS.md`.
- [x] Estraguei de propósito o que o teste protege — e ele ficou vermelho.
      (Três correções de 26/11/2026 foram conferidas assim.)
- [x] Cada verificação responde *"isto prova que ___"* — é a régua do
      `verificar-frame.mjs`.
- [x] Tenho caso negativo testado: `podar_temas` recusa `launcher-*`,
      `validate-app` reprova GPL na imagem, `verificar-frame` reprova rotação
      oposta (10,9 %).
- [x] O dado que duas partes conhecem mora em um lugar só — `#[path]`.
- [x] Defeito em repo de outra equipe vira issue, não commit — §1.1.
- [x] O que uso é compatível com a licença do destino — `apps.json` valida.
- [x] Medi no aparelho o que depende de hardware — `docs/desempenho.md`.
- [x] Escrevi o que **não** foi medido — §14 e cada página de app.
- [x] Nenhum estado que deixo para trás é lido como verdade sem checagem — a
      marca `.portmaster-port` é o único, e ela **é** a checagem.

## O que este repositório **não** faz do jeito do documento base

- `eval` no `aparelho.sh`: o documento não fala disso; aqui existe um
  `eval "$2"` no laço de espera dos testes de aparelho. Ele recebe **string
  escrita no próprio script**, nunca entrada externa — mas é uma construção que
  merece revisão no dia em que alguém passar argumento de fora para dentro.
