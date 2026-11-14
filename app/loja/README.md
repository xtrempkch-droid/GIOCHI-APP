# Giochi Loja

A loja do Giochi no PC: executável nativo para **Linux, Windows e macOS**.
Mostra o catálogo, mostra o preço, recebe o pagamento e entrega o `.giochi`.

```bash
bash scripts/compilar-loja.sh          # resolve o SDL2 e compila
app/loja/bin/giochi-loja               # abre a janela

app/loja/bin/giochi-loja --demo        # fluxo de pagamento de teste
app/loja/bin/giochi-loja --dump-frame /tmp/q.bmp   # desenha sem tela
app/loja/bin/giochi-loja --teclas down,down --dump-frame /tmp/q.bmp
```

## A decisão que define tudo: três chaves, três perguntas

A preocupação que originou esta loja foi *"e se usarem a chave pública para
roubar os apps?"*. A resposta é que uma chave **pública** não permite nada além
de conferir assinaturas — o perigo nunca foi ela. O perigo é **confundir
"autêntico" com "comprado"**, porque aí um único pacote assinado serve para todo
mundo.

| Pergunta | Responde | Chave | Privada vive em |
| --- | --- | --- | --- |
| "veio da Giochi?" | assinatura do `.giochi` | catálogo | servidor |
| "este aparelho pagou?" | licença | **licença** (outra) | servidor |
| "é este aparelho?" | identidade | par gerado no aparelho | aparelho |

No código isso são **tipos diferentes**: `PacoteVerificado` prova origem e nada
mais; `LicencaAssinada` prova compra e carrega a chave do aparelho.

## De onde vem o "este aparelho pagou"

Não do nosso backend — **do provedor**. Duas formas, e a diferença decide se o
jogo é vendável:

| | **itch.io** | **Lemon Squeezy / Paddle / Gumroad** |
| --- | --- | --- |
| como se sabe que comprou | `game:view:ownership` | chave de licença |
| credencial | a **conta do usuário** | a chave |
| vínculo com o aparelho | **nenhum** | **sim**: `activation_limit` por instância |
| preço e catálogo | do itch.io | nossos |

O limite de instâncias do provedor é o que substitui o backend: a chave pública do
aparelho vira o `instance_name`, e o servidor *deles* conta quantas instalações
existem. Esgotado, a resposta é
`{"activated": false, "error": "This license key has reached the activation limit."}`.

```bash
app/loja/bin/giochi-loja --ativar CHAVE-DE-TESTE-1        # provedor simulado
app/loja/bin/giochi-loja --ativar <chave> --provedor lemonsqueezy
```

Saída em JSON; código 3 quando recusa. `--aparelho <b64|arquivo>` define a chave
do aparelho (sem ele, uma chave de demonstração derivada de semente).

**O itch.io está implementado mesmo sem vincular ao aparelho**, porque serve para
jogo grátis e conteúdo de terceiro — e porque dizer "não dá" sobre algo que dá
seria mentira.

## O que a loja não faz, de propósito

- **Não vê número de cartão.** Cartão é pago em checkout hospedado, no
  navegador. PIX é o caminho principal, e é uma string — não uma credencial.
- **Não se declara paga.** `Estado::Pago` só existe com uma licença assinada, e a
  chave de licença não está nesta máquina. Não é disciplina, é o compilador.
- **Não inventa código PIX.** Sem provedor configurado, a tela de compra diz
  "sem provedor de pagamento configurado". Com `--demo`, o código sai com um
  aviso vermelho dizendo que não é pagável.

## Como a lista de apps é preenchida

Não há lista escrita à mão. Publicar um app são **dois comandos**:

```bash
node scripts/empacotar.mjs app/meu-app --saida dist/pacotes
node scripts/gerar-catalogo-loja.mjs --pacotes dist/pacotes \
     --chave ~/.giochi/catalogo.pem --url-base https://cdn.giochi.example
```

O primeiro gera `<slug>.giochi` + `.sig` + `.manifest.json`; o segundo lê os
pacotes e produz `catalogo.json` **assinado** com a chave de catálogo. A loja lê
esse arquivo; o aparelho confia na chave pública dele e em mais nada.

O gerador **recusa** pacote sem assinatura, assinatura de tamanho errado (89
bytes = base64 gravado como texto) e hash que não bate com o manifesto. Slug,
título, versão, tamanho e sha256 vêm do pacote — nunca digitados.

## O layout: itch.io como referência visual

Barra lateral estreita, grade de cartões com capa, título e preço. Isso é
**desenho**, não integração: nenhuma chamada ao itch.io, nenhuma credencial,
nenhum `butlerd`. Ver decisão 2 em [`docs/loja.md`](../../docs/loja.md).

As capas são **desenhadas** a partir do slug (matiz + detalhe + deslocamento), não
carregadas: o SDL2 puro não lê PNG, e a `capa.png` do pacote só existe depois do
download.

## Estrutura

```text
crates/core/     # sem SDL, sem rede: catálogo, chave, licença, pedido, pacote
  chave.rs       # Ed25519 nos 3 formatos que o projeto já usa
  licenca.rs     # assina/verifica licença; amarra ao aparelho
  catalogo.rs    # catálogo assinado + invalida_antes (revogação)
  pagamento.rs   # máquina de estados; só `Pago` libera, e exige licença
  pacote.rs      # confere hash + assinatura + ordem dos membros do tar
  baixador.rs    # trait + curl (com retomada) + simulado para teste
  grade.rs       # geometria da grade: desenho e navegação usam a MESMA
  http.rs        # curl mínimo; o que erra é a montagem, e ela tem teste
  provedor.rs    # LICENSE API: ativar/validar, o limite de instâncias do provedor
crates/ui-sdl/   # janela, desenho, 4 telas, navegação por teclado
```

`cargo test` roda 113 testes. Os 97 do núcleo **não precisam de tela** — é a mesma
separação do launcher, pelo mesmo motivo: teste que precisa de GPU falha no lugar
errado.

## Por que SDL2 e não egui

Medição, não gosto: `egui/eframe` exige `libx11-dev`, `libxkbcommon-dev` e
`libwayland-dev`, e nenhum dos três está na máquina de desenvolvimento deste
projeto. SDL2 já é resolvido pelo `compilar-launcher.sh`.

A objeção do launcher ao `egui` ("desenho custom limitado") **não se transfere**:
ela era sobre imitar um console. Uma loja precisa de lista, texto e botão.

## Verificado

| Afirmação | Como |
| --- | --- |
| desenha | `--dump-frame` → BMP de 3.587.254 bytes (= 1180×760×4 + 54) |
| navega | `--teclas down,down` → seleção 2, e o BMP **difere** do primeiro |
| build Linux | 525.456 bytes em release |
| emite e confere licença | `cargo run --example emitir_licenca` |
| Windows e macOS | **executado no CI** (run `42222562164`): os três desenharam quadro e navegaram |
| catálogo assinado | **provado ponta a ponta**: JS assina, o Rust verifica e carrega |
| grade | `down` de 0 vai a **3** (uma linha), não a 1 |
| tamanho dos binários | linux 280.621 · macos 241.678 · windows 202.296 bytes |
