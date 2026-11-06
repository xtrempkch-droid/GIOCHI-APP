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
roubar os jogos?"*. A resposta é que uma chave **pública** não permite nada além
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

## O que a loja não faz, de propósito

- **Não vê número de cartão.** Cartão é pago em checkout hospedado, no
  navegador. PIX é o caminho principal, e é uma string — não uma credencial.
- **Não se declara paga.** `Estado::Pago` só existe com uma licença assinada, e a
  chave de licença não está nesta máquina. Não é disciplina, é o compilador.
- **Não inventa código PIX.** Sem provedor configurado, a tela de compra diz
  "sem provedor de pagamento configurado". Com `--demo`, o código sai com um
  aviso vermelho dizendo que não é pagável.

## Estrutura

```text
crates/core/     # sem SDL, sem rede: catálogo, chave, licença, pedido, pacote
  chave.rs       # Ed25519 nos 3 formatos que o projeto já usa
  licenca.rs     # assina/verifica licença; amarra ao aparelho
  catalogo.rs    # catálogo assinado + invalida_antes (revogação)
  pagamento.rs   # máquina de estados; só `Pago` libera, e exige licença
  pacote.rs      # confere hash + assinatura + ordem dos membros do tar
  baixador.rs    # trait + curl (com retomada) + simulado para teste
crates/ui-sdl/   # janela, desenho, 4 telas, navegação por teclado
```

`cargo test` roda 74 testes. Os 65 do núcleo **não precisam de tela** — é a mesma
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
| tamanho dos binários | linux 280.621 · macos 241.678 · windows 202.296 bytes |
