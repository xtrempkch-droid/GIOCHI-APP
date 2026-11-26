# Apps sobre a engine e o catálogo — histórico medido

> Extraído de `AGENTS.md` em 26/11/2026, **verbatim**. Resumo em
> [`AGENTS.md`](../../AGENTS.md) §7 e §8. Inclui o `launcher-teste`, o Star
> Dust, o `musica`, a detecção das duas abas e as regras do catálogo.

---

## 7. Apps sobre a engine: launcher-teste, Star Dust e Música

### `launcher-teste` (scaffold)

O **mesmo desenho sobre a GIOCHI-ENGINE**, em app separado para não arriscar o de
produção. Reusa o crate `core` e as telas reais por `#[path]`; foi ele que provou
o porte: **100,0 % pixel a pixel** com o launcher em 640×480, 800×600, 1280×720 e
720×1280 girado, e nas seis telas.

Depende de `../../../GIOCHI-ENGINE-3D/giochi-engine` — **repositório irmão**. Sem
ele o build falha; é a única dependência fora deste repo.

Segue `scaffold` porque não tem sono, brilho, volume, energia nem Wi-Fi/BT: é um
banco de prova de desenho, e chamá-lo de `ready` prometeria o que ele não faz.

### `launcher-star-dust` (ready)

O launcher de produção **mais** a camada de efeitos. Ligada por padrão
(`--sem-efeitos` ou `GIOCHI_EFEITOS=0` desliga); desligada, o quadro é
**idêntico byte a byte** ao do launcher — propriedade medida em quatro tamanhos.

Oito efeitos (`efeitos.rs`): `brasas`, `brilho`, `varredura`, `vinheta`,
`scanlines`, `halo` do foco, `rastro` e `borda` pulsante. A camada inteira custa
**29 chamadas de desenho**.

O que a medição ensinou, e que vale para qualquer efeito novo:

| Versão | Custo | Causa |
| --- | --- | --- |
| uma chamada por brasa + `stroke_round_rect` no halo | +7,3 ms | 162 chamadas; o halo sozinho +6,9 ms |
| batelada por faixa de alfa | **+3,1 ms** | 27 chamadas |

**`draw_rect` é uma chamada de backend por retângulo.** O halo com
`stroke_round_rect` desenhava 4 arestas e 4 arcos de ~30 `draw_line` cada para
uma borda que ninguém vê; quatro bandas preenchidas num `fill_rects` leem igual e
custam 4× menos. A camada acrescenta **zero** divergência entre PC e aparelho
(98,10 % sem efeitos × 98,15 % com) — se introduzisse desenho dependente de GPU,
esse número teria caído.

O `halo` lê a geometria que a **tela publicou** (`layout.hits`), não recalcula:
recalcular seria a segunda verdade que já desalinhou o carrossel do foco.

A cadência de animação é própria (`GIOCHI_EFEITOS_FPS`, padrão **30**): a UI só
redesenha quando algo muda, e um efeito animado muda sempre. Não é 60 porque a
diferença visual em brasas de 1–3 px é nula e animação é contínua — interação é
que precisa de resposta imediata. Valor inválido cai no padrão; número que não
parseia não pode zerar a animação em silêncio.

O `rastro` é disparado por `cmd.is_user_input()`, **não** por "o estado mudou":
com o gatilho errado ele custava 20 chamadas num quadro em que ninguém apertou
nada, e o efeito perdia o significado ("algo aconteceu") virando ruído
permanente. A camada é opcional no `launcher-teste` justamente porque lá a medida
é a fidelidade de desenho; aqui ela **é** o motivo do app.

```bash
bash scripts/verificar-star-dust.sh              # 640x480
bash scripts/verificar-star-dust.sh --tamanho 1280x720
bash scripts/verificar-efeitos.sh                # 4 tamanhos + 1 rotação
```

Cinco perguntas em cada: desenhou? é determinística? some quando desligado? cabe
no orçamento de chamadas? desligada é idêntica à produção? O determinismo vem de
animar por **contagem de quadros**, nunca pelo relógio. E `verificar-efeitos.sh`
tem contador e `exit 1`: a primeira versão imprimiu "ok camada verificada" depois
de um FALHA, porque o resumo não olhava o que tinha acontecido. **Um resumo que
ignora falha é pior que nenhum.**

### `musica` (ready) — o primeiro app de imagem que não é tema

O app que faz a aba de apps ter o que mostrar. Tocador sobre a mesma engine do
`launcher-teste`, em `app/musica/`:

| Peça | Onde | O que decide |
| --- | --- | --- |
| núcleo | `crates/core` | ordem natural, foco com volta, transporte, layout, rotação, capa, título — **120 testes, sem SDL** |
| desenho | `src/pintor.rs`, `src/tela.rs` | pixels, texto, texturas |
| mundo | `src/audio.rs`, `src/entrada.rs` | mixer e `/dev/input` |

`crates/core` não tem **nenhuma** dependência de propósito: o binário depende da
engine, que depende de SDL2 e do repositório irmão — e é por isso que só o núcleo
roda no CI, como o `giochi-launcher-core`.

O que o app de exemplo que originou o pedido fazia de errado, e onde cada coisa
foi resolvida:

| Defeito do exemplo | Correção |
| --- | --- |
| lia o botão por **nível** dentro do passo fixo (segurar o direcional corria a lista) | borda vem do `Mapper` do launcher, com a curva medida |
| só havia `play`: sem pausa, sem parar, sem volume | `Efeito::{Pausar, Retomar, Parar, Volume}` no reducer |
| o carrossel parava nas pontas | `Carrossel::mover` dá a volta |
| pasta vazia derrubava o app no primeiro asset ausente | `Biblioteca::Vazia { motivo }`, que vira frase na tela |
| ordem vinha do `readdir` | `faixa::ordem_natural` (`Faixa 2` antes de `Faixa 10`) |
| `aspect` fixo 16:9, sem painel nem rotação | `Layout::de(largura, altura)` + `rotacao::decidir` |
| `play_sound` (efeito, decodifica na RAM) | API de **música** do mixer, que faz *streaming* |
| nenhum teste | 158, e 120 deles sem SDL |

**Medido em 15/11/2026:** quadro de 640×480 do R35S **100,00% idêntico** ao do
PC (mesma pasta, comparação pixel a pixel); 720×1280 des-rotacionado bate
**99,96%** com 1280×720; 96 a 112 desenhos por quadro; áudio no R35S com mixer
**2.6.3**, `Playing → Stopped` em **2,5 s** num WAV de 2,5 s (`--testar-som`).

**A imagem passou a trazer o que o app precisa.** O binário pede
`libSDL2_mixer-2.0.so.0` (e `SDL2_image`), que o launcher nunca pediu — o
`docs/dependencias-aparelho.md` chamava isso de ausente, e para esta imagem é
falso. `compilar-musica-arm64.sh --instalar` **pergunta ao aparelho** antes de
copiar, porque biblioteca ausente aqui é falha de carga, não degradação.

## 8. O catálogo do aparelho

**O catálogo não é escrito à mão nem lido cegamente: ele é projetado do que
existe no disco, e `index.json` é o complemento.** Duas camadas:

| Camada | Quem usa | Alcance |
| --- | --- | --- |
| **varredura** de `/opt|/user/{games,apps}` | o aparelho, sempre | tudo que está numa raiz, com `kind` lido do pacote |
| **`index.json`** | complementar | conteúdo fora das raízes e o contrato de `docs/integracao.md` |

O disco **vence** na junção (§6.9): o que foi varrido substitui a entrada do
índice para o mesmo slug, com o `dir` real.

No PC, é o gerador que projeta `app/apps.json` + `metadata.json`:

```bash
bash scripts/aparelho.sh catalogo        # gera, confere no aparelho e publica
node scripts/gerar-catalogo.mjs --lista  # quem entra e quem fica de fora
```

Antes ele era escrito à mão e o aparelho mostrou **um** cartão com dois apps
instalados — e um app fora do JSON ficava invisível sem erro em lugar nenhum: nem
no app, nem no log, nem na tela.

| Regra | Por quê |
| --- | --- |
| `status: scaffold` fica fora | `bin/` vazio; cartão que não abre é pior que cartão ausente |
| `distribution: host` fica fora | roda no PC |
| `distribution: store` fica fora | é instalado em `/user/apps` ou `/user/games` pela loja |
| **tema** (`role: shell` **ou** prefixo `launcher`) fica fora | senão o launcher **lista a si mesmo** |
| `entrypoint` precisa existir | idem scaffold, mas medido no arquivo |

A linha do tema mudou de nome de propósito: o papel agora vem de duas fontes
(§3), e `--lista` diz qual delas valeu — `tema do launcher (prefixo "launcher")`
contra `role shell: é a própria interface`. Dizer "role shell" para um app que
nunca declarou `role` manda quem lê procurar no `apps.json` um campo que não
existe; é a ferramenta medindo a intenção em vez do efeito (§13).

**Hoje o catálogo do R35S tem 4 entradas: 3 jogos e `musica`.** O app de música
é o primeiro `image` sem `role: shell` — o que fez a aba Apps deixar de mostrar
"Nenhum jogo instalado." (e de mostrar oito apps que não existiam, §6.9). Um
teste (`tests/musica-app.test.mjs`) reprova se ele sair do catálogo: `status`
`scaffold`, `bin/` apagado, `kind` perdido ou `role` declarado por engano não
apareceriam em nenhum outro lugar.

**O `kind` vai em cada entrada do índice**, e não é decoração: é a única coisa
que diz ao launcher qual aba desenha o cartão. Sem ele o default é `game` (§3),
e um app cairia na Home.

**A conferência que faltava é no aparelho, não no repo.** O gerador checa o
binário na árvore; quem importa é o aparelho. `aparelho.sh catalogo` percorre cada
`dir/entrypoint` por SSH e **reduz** o catálogo antes de publicar, dizendo o que
tirou e como instalar. Medido: remover o binário do aparelho fez `gerado: 3`
virar `catálogo reduzido para 2`. **Projeção de verdade reflete remoção, não só
adição.**

**O `dir` do aparelho não é derivável do repo — e desde 15/11/2026 depende do
`kind`.** `<grupo>/<slug>` no repositório é `/opt/giochi/games/<id>` para um jogo
e `/opt/giochi/apps/<id>` para um app. É convenção do `Giochi System`, e a única
informação do catálogo que não vem do `apps.json` nem do `metadata.json` — por
isso está explícita em `scripts/lib/catalogo.mjs` (`dirDoApp`) e tem teste. Um
app `image` com `kind: "app"` publicado em `/opt/giochi/games` apareceria na
Home; o gerador avisa com `LUGAR_ERRADO`.

**O catálogo inclui o que veio da loja.** Um `.giochi` não está no `apps.json`; a
colheita lê as **quatro** raízes (`RAIZES`) e o disco vence. E a mesclagem
**herda** `license` do `apps.json` — substituir a entrada inteira apagava a
licença.

### A guarda de tema existe dos dois lados, e precisa

O gerador filtra no PC. Mas ele **não é o único caminho até o índice**:

| Caminho | Quem escreve | Passa pelo gerador? |
| --- | --- | --- |
| `aparelho.sh catalogo` | PC, via SSH | sim |
| `--catalogo-adicionar` (o launcher instala um `.giochi`) | **o próprio aparelho** | **não** |
| índice de uma imagem antiga, ou escrito à mão no diagnóstico | ninguém sabe | **não** |

`adicionar_ao_indice` é o caminho do aparelho, e ele existe porque amarrar
instalação a Node+SSH não serve num portátil. Instalar um `launcher-neon.giochi`
por ali criava o cartão que abre um segundo launcher e perde o DRM — então agora
ele **recusa com o motivo**, em vez de gravar e deixar o usuário descobrir
apertando `A`.

E `Catalogo::podar_temas` fecha o que sobra, pela mesma razão que
`podar_sem_binario` é em tempo de execução: **o PC que gera o índice não sabe o
que estará no aparelho**. A poda de tema roda **antes** da de binário, senão o
tema apareceria no log como "cartão removido" e mandaria procurar o problema no
lugar errado.

As duas podas compartilham um `reter` privado — uma segunda cópia da renumeração
de `Game.id` envelheceria diferente da primeira — e as duas cobrem **as duas
listas**: `podar_sem_binario` e `podar_temas` percorrem `jogos` **e** `apps`. Um
tema só na lista de apps passaria pela poda e abriria um segundo launcher.

**Vigiar: foco e desenho descrevem a mesma lista.** `settings_rows()` no núcleo
decide até onde o foco anda; `views::linhas_da_categoria` monta o que é desenhado.
Dois lugares descrevendo a mesma lista foi exatamente como o carrossel ficou
dessincronizado do foco; há teste reprovando se divergirem.
