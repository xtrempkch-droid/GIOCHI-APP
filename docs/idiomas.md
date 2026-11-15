# Idiomas do launcher

Os mesmos dez idiomas do site (`GIOCHI-OS`, `scripts/lib/i18n.mjs`), na mesma
ordem, com **inglês** como padrão comercial. O idioma é escolhido na primeira
abertura, trocado em **Config › Aparência** — a mesma linha onde se troca o tema
— e gravado em `/user/.config/giochi/language`.

| Código | Idioma | Escrita | Fonte que o desenho procura |
| --- | --- | --- | --- |
| `en` | English | latina | DejaVu (o do aparelho) |
| `zh` | 中文 | han | Noto Sans CJK / WenQuanYi |
| `hi` | हिन्दी | devanágari | Noto Sans Devanagari / Lohit |
| `es` | Español | latina | DejaVu |
| `fr` | Français | latina | DejaVu |
| `ar` | العربية | árabe (**RTL**) | Noto Naskh/Sans Arabic |
| `bn` | বাংলা | bengali | Noto Sans Bengali / Lohit |
| `pt` | Português | latina | DejaVu |
| `ru` | Русский | cirílica | Noto Sans / DejaVu |
| `ur` | اردو | árabe (**RTL**) | Noto Nastaliq/Naskh |

## Por que os dez do site, e não os "principais"

O site já é publicado nesses dez — os mais falados por número total de falantes
(Ethnologue). Um launcher com outra lista obrigaria a mesma pessoa a aprender
duas listas, e o idioma que ela escolheu na página não estaria no aparelho.

## Onde cada texto mora

| Camada | Arquivo | O que decide |
| --- | --- | --- |
| tabela de textos | `app/launcher/crates/core/src/idioma.rs` | uma linha por chave, dez colunas na ordem de `Idioma::TODOS` |
| desenho | `app/launcher/crates/ui-sdl/src/views.rs` | `t(state, Chave::X)` / `tv(state, Chave::X, valor)` |
| escolha e persistência | `ui-sdl/src/preferencias.rs` | `/user/.config/giochi/language`, com `GIOCHI_IDIOMA` de sobreposição |
| fonte por escrita | `ui-sdl/src/draw.rs` | `cadeia_de_fontes`: a principal primeiro, as do idioma e das outras nove depois |

A tradução **não** é um `match` por idioma espalhado nas telas: a tabela fica no
núcleo (testável sem SDL) e o desenho só pede a chave. Isso também é o que
permite o teste de completude — ver "O que os testes garantem".

## A primeira abertura

Aparelho novo (sem `/user/.config/giochi/language`, sem `--idioma`) abre a tela
de boas-vindas: a lista dos dez idiomas, com o **inglês já em foco**, porque é o
padrão comercial do produto.

- `↑↓` percorre a lista; o título e a instrução são desenhados **no idioma em
  foco** — é o único jeito de quem não lê inglês confirmar a escolha antes de
  apertar `A`.
- `A` escolhe e fecha; `B` escolhe o inglês e fecha. `B` não é "voltar": não há
  para onde voltar, e o rodapé diz `Starts in English`.
- A tela aparece **uma vez**. Quem decide é `State::idioma_definido`, ligado pela
  escolha ou pela leitura do arquivo.

Sem essa tela, quem não lê inglês abriria o produto e encontraria uma interface
que não entende, sem caminho de volta antes de descobrir onde ficam os ajustes.

## Trocar depois

**Config › Aparência** passou a ter três linhas: `Tema`, `Idioma`, `Escala`.

| Tecla | Efeito na linha do idioma |
| --- | --- |
| `←` `→` | percorre os dez, com volta, e a interface muda **na hora** |
| `A` | avisa que o ajuste é com as setas (o rodapé só anuncia `← → Ajustar` onde a seta age) |

A escolha é gravada por diferença de estado (`App::sync_idioma`), e não a cada
quadro: `dispatch` roda em todo comando, e escrever no cartão SD a cada tecla
desgastaria o cartão sem motivo. Falha na gravação **avisa** — a escolha vale até
o próximo boot, e o usuário não descobre isso reiniciando.

## A cadeia de fontes (e por que uma fonte não bastava)

O aparelho tem `DejaVuSans.ttf`. Ele cobre latim, cirílico e (parcialmente)
árabe, e **não** cobre chinês, devanágari nem bengali. Duas saídas possíveis:

1. trocar a fonte do app conforme o idioma — mudaria também título de jogo,
   relógio e número, que são latinos;
2. escolher a fonte **por string**.

Foi a segunda. `Painter::fonte_para` percorre a cadeia e usa a primeira fonte que
tem glifo para **todos** os caracteres daquele texto, com a resposta memorizada
por `(fonte, caractere)`:

```text
cadeia: DejaVuSans.ttf, NotoSansCJK-Regular.ttc, NotoSansDevanagari-Regular.ttf,
        NotoNaskhArabic-Regular.ttf, NotoSansBengali-Regular.ttf, …
```

A cadeia inclui as fontes **dos dez idiomas**, e não só a do escolhido: a lista
de idiomas desenha os dez nomes nativos o tempo todo — nas boas-vindas e em
Config › Aparência. Com só as fontes do idioma atual, quem está em inglês vê
`中文` como quadradinhos, e é justamente a linha que ele precisa escolher.

Se nenhuma fonte cobrir o texto, o desenho usa a principal (o resto da interface
fica consistente) e o log avisa **uma vez**:

```text
aviso: nenhuma das 1 fonte(s) cobre "中文" — o texto sai com glifo faltando
       (instale a fonte do idioma; ver docs/idiomas.md)
```

Onde procurar está em `draw::DIRETORIOS_FONTE` (assets do app, Noto do Debian,
do Arch, DejaVu do Buildroot). Fonte ausente não impede o app de abrir: degrada,
e diz que degradou.

## Árabe e urdu: shaping **funciona**, bidi de parágrafo não

> **Correção de 15/11/2026, e ela desmente o que este arquivo dizia.** A primeira
> versão afirmava "`SDL2_ttf` sem HarfBuzz, letras isoladas". Era suposição, e a
> medição no R35S a derrubou.

Medido no R35S (imagem `built_at_utc=2026-11-15T21:00:56Z`):

```console
$ readelf -d /usr/lib/libSDL2_ttf-2.0.so.0 | grep -i needed
 (NEEDED)  Shared library: [libharfbuzz.so.0]      ← está lá
$ ls -l /usr/lib/libharfbuzz.so.0*
-rwxr-xr-x 1 root root 1252984 /usr/lib/libharfbuzz.so.0.61030.0
```

E a consequência visual, medida: o mesmo texto árabe renderizado no aparelho
(KMSDRM) e no PC (Debian, também com HarfBuzz) dá **99,8%** de identidade. Se um
dos dois não fizesse shaping, as formas isoladas contra as conectadas derrubariam
esse número muito abaixo — é a mesma lógica do "rotação oposta dá 10,9 %".

O `defconfig` do R35S liga o pacote (`BR2_PACKAGE_HARFBUZZ=y`, com o comentário de
que sem ele "o texto de script complexo sai com shaping errado — sem erro"), e o
`SDL2_ttf` o puxa de fato.

| O que | Estado |
| --- | --- |
| shaping de `ar`/`ur` (ligaduras, formas inicial/medial/final) | **funciona** — HarfBuzz presente e medido |
| bidi de **parágrafo misto** (árabe + número + latino numa linha) | **não existe** — o `SDL2_ttf` faz shaping por *run*, não reordena parágrafo |
| layout espelhado (margens e âncoras) | **não existe** — `Idioma::dir()` devolve `Rtl`, mas nenhuma tela inverte |

Na prática: rótulo curto puramente árabe sai **correto**. Linha que mistura árabe
com número ou palavra latina (ex.: `launcher 0.1.0`, `Wi-Fi`) pode aparecer com
os pedaços fora de ordem — é bidi, não shaping, e a diferença importa: fonte e
Harfbuzz não consertam isso; sinalizar `dir` no layout consertaria.

O `App` avisa no log ao abrir num idioma RTL, para ninguém tratar como regressão
de desenho o que é limitação declarada.

## O que os testes garantem

| Teste | Onde | Reprova quando |
| --- | --- | --- |
| `toda_chave_da_tabela_bate_com_o_enum` | `core/idioma.rs` | uma linha da tabela sai de ordem — a consulta é por índice, e traduziria a frase errada em silêncio |
| `nenhum_texto_vazio` | `core/idioma.rs` | falta tradução em qualquer das dez colunas de qualquer chave |
| `os_dez_idiomas_sao_os_do_site` | `core/idioma.rs` | a lista ou a ordem diverge do site; o padrão deixa de ser inglês |
| `dicas_do_rodape_sao_curtas` | `core/idioma.rs` | uma dica passa de 18 caracteres e empurra as outras para fora do rodapé |
| `todo_idioma_indica_fonte` | `core/idioma.rs` | um idioma não declara fonte, ou o DejaVu deixa de fechar a lista |
| `ciclo_dos_idiomas_da_a_volta_nos_dois_sentidos` | `core/idioma.rs` | `←`/`→` deixam de percorrer os dez |
| `confirmar_na_boas_vindas_escolhe_o_idioma_em_foco` | `core/reduce.rs` | a escolha não grava o idioma ou não fecha a tela |
| `voltar_na_boas_vindas_escolhe_ingles` | `core/reduce.rs` | `B` fica morto numa tela sem saída |
| `seta_em_aparencia_percorre_os_dez_idiomas` | `core/reduce.rs` | a linha da Config deixa de trocar o idioma |
| `linha_de_idioma_mostra_o_idioma_escolhido` | `ui-sdl/views.rs` | a linha mostra o rótulo traduzido e o valor errado |
| `cadeia_de_fontes_comeca_pela_principal` / `…_so_aponta_arquivo_que_existe` | `ui-sdl/draw.rs` | a cadeia repete fonte ou aponta caminho inexistente |
| `grava_e_le_de_volta` | `ui-sdl/preferencias.rs` | a gravação deixa `.tmp` para trás ou não relê |

## Congelar o idioma na comparação de quadros

O idioma passou a ser a **quarta** coisa do ambiente que muda o desenho — junto
de relógio, bateria e temperatura (§11 do `AGENTS.md`). Um PC sem
`/user/.config/giochi/language` desenha a tela de boas-vindas; um aparelho que já
escolheu desenha a Home. Comparar os dois daria uns 60 % e mandaria procurar
defeito de desenho.

Por isso os scripts que capturam quadro passam `--idioma en` **dos dois lados**:

```bash
bash scripts/aparelho.sh verificar        # captura e referência, ambas em en
bash scripts/comparar-launcher-teste.sh   # produção em en; o launcher-teste já é en
bash scripts/verificar-star-dust.sh       # produção e Star Dust, ambos em en
```

`verificar-efeitos.sh` fica **de fora** e isso é deliberado: ele exercita o
`launcher-teste`, que não tem i18n (usa o `views.rs` por `#[path]` e o estado
nasce em `Idioma::PADRAO`, então desenha em inglês sempre). Passar `--idioma`
para um binário que não conhece a chave faz ele imprimir a ajuda e sair com erro
— o script reprovaria por um argumento que ele mesmo inventou.

`--idioma` **não grava** nada: se gravasse, cada foto mudaria o estado do
aparelho.

## Medido no R35S em 15/11/2026

Imagem `built_at_utc=2026-11-15T21:00:56Z`, launcher instalado por
`compilar-launcher-arm64.sh --instalar`.

| O que | Resultado |
| --- | --- |
| primeira abertura (sem `/user/.config/giochi/language`) | tela de boas-vindas, **inglês em foco**, `idioma: English (en) — primeira abertura, o padrão é inglês` |
| escolher português pelo caminho real (`↓`×7 + `A`) | `idioma: en -> pt (Portuguese)` e `/user/.config/giochi/language` = `pt` |
| escolher com **botão físico** do aparelho | a boas-vindas ficou na tela real e um toque no controle escolheu **inglês** (linha 0, a que está em foco) e gravou `en
` — 3 bytes, 22:32. É o caminho de `input_evdev`, não o de `--teclas` |
| segunda abertura | `idioma: Portuguese (pt)` — **sem** a boas-vindas |
| Home em português, KMSDRM × PC | **99,6%** (a diferença é antialiasing de glifo, §13) |
| Config em português, `--dump-frame` × PC | **99,7%** |
| mesmo quadro em inglês × português | **98,4%** — a tradução chega à tela |
| as dez abrem sem travar | 10/10, BMP de 1.228.922 bytes cada |
| `--fontes` por SSH, sem tela | funciona (foi corrigido: antes pedia dispositivo de vídeo) |
| verificação full (`aparelho.sh verificar`) | **100,0%**, pid com `ppid` = `giochi-init` |
| a boas-vindas aparece **uma vez** | depois que o arquivo existe, o launcher abre na Home e nunca mais pergunta |
| troca em Config › Aparência com **botão físico** | o arquivo foi de `pt` → `fr` → `pt` enquanto o aparelho estava na mão; cada `←`/`→` na linha do idioma reescreve o arquivo |
| **ninguém escreve sozinho** | duas medidas: (a) o arquivo ficou em `pt` por 15 s de amostragem sem entrada; (b) com `GIOCHI_DEBUG_CMDS=1` por 10 s o log mostrou só `SetPad`/`SetBattery`/`SetTemperature`/`BootDone` — **nenhum** `TrocarIdioma` partindo do app |
| input real move a lista | com um controle apertando `←`, o log deu 8× `Cmd::Nav(Left)` e o arquivo andou `zh`→`en`→`ur`→`ru`→`pt`→`bn`→`ar`→`fr`→`es` — exatamente 8 passos. Cada mudança de idioma tem um `Nav(Left)` de `evdev` como causa; não há caminho que troque idioma sem entrada |

A escolha por botão físico é a prova mais forte que este aparelho deu: o
`--teclas` já mostrava o reducer funcionando, mas ele **pula** o `input_evdev`.
Um toque no controle atravessou `/dev/input` → mapa → `confirm` → `IdiomaEscolhido`
→ gravação, e o log da execução seguinte abriu em inglês sem boas-vindas. É o
único caminho que um usuário de verdade percorre.

### O que o aparelho **não** tem, e o número

A cadeia resolveu para **uma** fonte: `find /usr/share/fonts -type f | wc -l` dá
**21**, todas DejaVu, nenhuma Noto. Consequência medida com `--fontes` (que
pergunta ao `TTF_GlyphIsProvided` por caractere, em vez de deduzir):

| Idioma | Cobertura no R35S | Na tela |
| --- | --- | --- |
| `en` `es` `fr` `pt` | 15/15 | legível |
| `ru` | 10/10 | legível |
| `ar` | 10/10 | legível, letras isoladas (shaping, abaixo) |
| `zh` | 5/8 | quadradinho no que falta |
| `hi` | 1/11 | quase tudo quadradinho |
| `bn` | 1/10 | idem |

A lista de idiomas continua **utilizável** nesse estado porque cada linha mostra
o nome nativo **e** o nome em inglês à direita: onde a escrita não tem glifo, a
coluna da direita identifica a linha. Foi por isso que ela existe.

O pedido de fonte para a imagem está em
[issues-para-outros-repos.md](issues-para-outros-repos.md) nº 5.

### A correção foi verificada ponta a ponta antes de pedir

Não basta pedir "fonte para uma pasta" e supor que o app acha. Medido no R35S em
15/11/2026, **simulando** o que o `BR2_PACKAGE_WQY_ZENHEI` faria: criado
`/usr/share/fonts/wqy-zenhei/` com uma fonte CJK dentro.

```console
$ giochi-launcher --fontes --idioma zh
  0: /usr/share/fonts/dejavu/DejaVuSans.ttf
  1: /usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc
amostra da escrita: "吉欧奇 0123"
veredito: 8/8 caractere(s) cobertos por /usr/share/fonts/wqy-zenhei/wqy-zenhei.ttc
```

**5/8 → 8/8**, e o mesmo quadro da tela de boas-vindas mudou **8.557 pixels**, com
**4.760** deles no título e na instrução (o texto chinês que antes era
quadradinho). O controle negativo fecha o argumento: a linha do hindi mudou
**0** pixel, porque nenhuma fonte de devanágari foi adicionada — a diferença é da
escrita, não do layout.

Isso importa por um motivo prático: **o pedido à outra equipe é de uma linha por
defconfig**, e este teste prova que ligar o pacote resolve, sem segunda rodada do
app. Sem ele, o pedido seria uma promessa; com ele, é uma verificação.

> O Buildroot instala cada fonte na **sua própria** pasta
> (`/usr/share/fonts/wqy-zenhei/`), e não numa comum. Essas pastas entraram em
> `draw::DIRETORIOS_FONTE`, e há teste (`a_lista_cobre_os_diretorios_do_buildroot`)
> prendendo a lista — sem elas, ligar o pacote não mudaria nada e **nenhum erro
> apareceria em lugar nenhum**.

## Ferramentas de campo

```bash
giochi-launcher --idioma pt              # abre em português, sem gravar
giochi-launcher --idioma pt --tela idiomas --dump-frame /tmp/pt.bmp
giochi-launcher --boas-vindas            # força a tela de escolha
giochi-launcher --fontes                 # a cadeia de fontes resolvida, e o que falta
```

`--fontes` é o que responde "por que o chinês aparece como quadradinhos?" sem
adivinhação — e ele **mede**, não deduz: pergunta ao `TTF_GlyphIsProvided` quantos
caracteres da amostra do idioma cada fonte desenha, e imprime o veredito.

```console
$ giochi-launcher --fontes --idioma hi
idioma: Hindi (hi)
escritas deste idioma: NotoSansDevanagari-Regular.ttf, …, DejaVuSans.ttf
cadeia resolvida neste aparelho:
  0: /usr/share/fonts/dejavu/DejaVuSans.ttf
veredito: 1/11 caractere(s) cobertos por /usr/share/fonts/dejavu/DejaVuSans.ttf
          o texto deste idioma sai com glifo faltando — instale a fonte da escrita (docs/idiomas.md)
```

Duas armadilhas que a primeira versão tinha, e ambas custaram a confiança na
ferramenta:

1. **Deduzia pelo tamanho da cadeia.** Cadeia de um elemento virava "pode sair com
   glifo faltando" — e o veredito saía errado justamente no caso mais comum, o
   português, cuja escrita o DejaVu cobre. Mesma família do §13: medir a
   configuração em vez do efeito.
2. **Pedia dispositivo de vídeo antes de imprimir.** Rodando por SSH num aparelho
   sem tela, respondia `Error: "No available video device"` — a ferramenta que
   explica o quadradinho se recusava a rodar onde o quadradinho aparece. Hoje ela
   sai antes de `video.window(...)`; `sdl2::ttf::init()` não exige vídeo.

## Limites conhecidos

| Item | Estado |
| --- | --- |
| shaping de `ar`/`ur` | **funciona** (HarfBuzz medido no aparelho; 99,8% contra o PC) |
| bidi de parágrafo misto e layout espelhado | **não existe** — `SDL2_ttf` não reordena parágrafo e nenhuma tela inverte âncora |
| tradução dos diagnósticos de Wi-Fi/Bluetooth | **não traduzidos** — são mensagens do sistema (`wpa_state`, códigos do BlueZ) e citam texto que não se traduz; o prefixo `erro:`/`error:` é traduzido |
| teclado de senha | **QWERTY fixo** — sem teclado cirílico/árabe; senha de rede com outros alfabetos tem de ser digitada no PC (cardápio: mapear `KEYBOARD` por idioma) |
| `launcher-teste` | desenha em inglês (não tem tela de boas-vindas nem persistência; é banco de prova de desenho) |
| nomes de jogo e de categoria | **não traduzidos** — vêm do `metadata.json` do próprio app |
