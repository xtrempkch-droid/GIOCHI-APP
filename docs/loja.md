# Loja e multi-lojas — o que existe, o que não existe

Estado em 06/11/2026. Este documento existe para não repetir o erro de chamar de
pronto o que não foi executado.

> **Resumo:** o caminho `pasta de app → .giochi assinado → aparelho → jogo
> rodando` **funciona e foi medido no X55**. O que **não** existe é o download:
> login em loja, OAuth, butler/butlerd. Nada disso foi escrito, e não faz sentido
> escrever sem uma credencial e uma rede para testar.

---

## 0. O desenho, em uma frase

**A loja roda no computador do usuário.** O cliente compra o jogo lá; a loja baixa,
prepara o `.giochi` e entrega o pacote pronto para o aparelho com o Giochi
instalado (X55 e outros) apenas **executar**. Todo o trabalho pesado — download,
análise de dependências, empacotamento — acontece no PC. O console é reprodutor
de pacotes.

```text
[PC]                                    [aparelho]
loja ──▶ baixa ──▶ prepara ──▶ .giochi ──▶ /user/apps/<slug> ──▶ executa
                                       (+ assinatura Ed25519)
```

| Peça | Onde roda | Estado |
| --- | --- | --- |
| `app/store/` — a loja | **PC** (`distribution: host`) | **não escrita** |
| `app/forge/` — empacotador | **PC** (`distribution: host`) | a lógica existe em `scripts/lib/publicar.mjs`; o app não |
| pacote `.giochi` assinado | PC → aparelho | **funciona** (`empacotar.mjs`) |
| instalação no aparelho | PC → aparelho | **funciona** (`aparelho.sh instalar-pacote`) |
| execução | aparelho | **funciona** (launcher + catálogo) |

O aparelho **não baixa nada** e **não compila nada**: ele extrai e executa. É o
que mantém o rootfs pequeno e o sistema substituível.

## 1. O que funciona hoje

```text
app/jogos/pacman ──empacotar──▶ pacman.giochi ──instalar-pacote──▶ X55 ──▶ jogo
                     + .sig
                     + .manifest.json
```

Medido no aparelho:

```text
287  giochi-launcher --lancar pacman     ← pai
300  giochi-pacman                       ← filho, iniciado pelo launcher
```

Ver [`pacote.md`](pacote.md) para o formato, a assinatura e por que o `tar` é
escrito à mão.

---

## 2. O que existe e o que não existe na loja

| Peça | Estado | Por que não foi feita |
| --- | --- | --- |
| Login OAuth no itch.io | **não escrita, e não é o caminho** | a loja é da Giochi, não uma vitrine do itch.io; ver §4.2 |
| butler / butlerd | **decidido não usar** | pacotes de 81 KB; ver §4 decisão 2 |
| Backend de pagamento | **não existe** | exige conta no provedor, CNPJ e servidor; ver §4.2 |
| Interface gráfica da loja | **escrita** — `app/loja`, Rust + SDL2, 4 telas | decisão de stack em §4 |
| Transferência por USB automática | **parcial** | `aparelho.sh instalar-pacote` faz o envio; o "detectar pendrive" não existe |
| Compra/checkout | **modelo pronto, backend não** | `pagamento.rs` tem a máquina de estados; falta o servidor |

`app/store/` (CLI) e `app/forge/` rodam no PC com `distribution: host`. A
**interface gráfica** é `app/loja/`, um binário nativo por sistema, também
`host`. A separação é de propósito: a CLI serve script e CI, a loja serve pessoa.

---

## 3. As limitações que o relatório arquitetural identificou

Valem como restrição de projeto, não como pendência de código:

| Limitação | Consequência |
| --- | --- |
| itch.io não expõe compra por API de terceiros | o botão "Comprar" abre o navegador; a posse se confirma pelo butlerd já logado |
| itch.io só entrega builds, nunca fonte | empacotar é copiar binário, não compilar |
| `jpackage` não faz cross-build | jogo Java ou vem pronto para aarch64, ou o JRE fica no sistema |
| Steam não tem download oficial por terceiros | no máximo "importar biblioteca", sem baixar |
| Editoras podem não publicar build Linux ARM | sem build aarch64 não há o que empacotar — e isso não é resolvível do nosso lado |

Sobre a última: é a limitação mais dura e a menos discutida. Um catálogo de
jogos de PC não é um catálogo de jogos ARM64. O que existe hoje neste repositório
são três jogos **escritos aqui** (`volley`, `pacman`, `cobrinha`) e quatro
projetos Godot sem export.

---

## 4. As quatro decisões — tomadas em 06/11/2026

### 1. Onde a loja roda, e em que stack

**No PC, como executável nativo em Rust + SDL2.** Um binário por sistema, gerado
pelo CI em runner nativo.

A medição veio antes da escolha. `egui`/`eframe` seria a escolha natural para uma
interface de PC — formulário, lista, rolagem —, mas `winit` exige
`libx11-dev`, `libxkbcommon-dev` e `libwayland-dev`, e **nenhum dos três está
nesta máquina de desenvolvimento**. SDL2 já está resolvido por
`scripts/compilar-launcher.sh`, e é a mesma stack do launcher.

Isso não é escolher o conhecido por preguiça. É que a objeção do launcher ao
`egui` ("desenho custom limitado") **não se transfere**: ela era sobre imitar um
console, com layout próprio e navegação por controle. Uma loja precisa de lista,
texto e botão — exatamente o que o renderer de primitivas já faz.

O que se perde: campo de texto nativo e IME. A consequência está na decisão
seguinte, e ela é boa.

### 2. `butlerd` — **não**. `curl` com retomada.

O `butlerd` é um daemon JSON-RPC do itch.io que faz download incremental por
blocos. Ele resolve um problema que **não temos**, e o número é medido:

```text
arcade-volleyball.giochi    81.408 bytes  (81 KB)
```

Baixar 81 KB de novo não é problema. O `butlerd` acrescentaria um daemon para
empacotar, supervisionar, versionar e explicar no suporte, para economizar zero.

| | butlerd | curl com retomada |
| --- | --- | --- |
| peças a empacotar e supervisionar | daemon + protocolo + estado | **nenhuma** |
| retomada de download interrompido | sim | **sim** (`-C -`) |
| ganho com 81 KB | zero | zero |
| ganho com 5 GB | grande | nenhum |

**A regra para reconsiderar, escrita para não ser redescoberta por intuição:
quando o catálogo tiver pacotes acima de ~500 MB.** Antes disso, é peça a mais.

`curl` e não uma crate de HTTP: ele existe nos três sistemas que a loja suporta
(Linux, macOS, e Windows desde a 10 1803). Trocar uma dependência de sistema **já
presente** por uma árvore de crates de TLS aumenta a superfície de atualização de
um binário que o usuário baixa e executa. O preço é traduzir código de saída em
mensagem, e é o que `baixador.rs` faz.

### 3. A chave: **três**, não uma

Esta era a pergunta mais importante, e a resposta está em
[§4.1](#41-a-segurança-três-perguntas-três-chaves).

### 4. A chave pública no rootfs — e por que ela não é o risco

A preocupação do dono do projeto foi: *"usarem essa chave para ter acesso aos
dados e roubarem os jogos via rede"*.

**Uma chave pública não dá acesso a nada.** Ela só confere assinaturas: não
decifra, não abre catálogo, não emite licença, não autentica. Publicá-la na
imagem é o mesmo que publicar um cadeado aberto.

O risco real é outro, e tem nome:

| Risco | Consequência | Mitigação implementada |
| --- | --- | --- |
| **chave privada vaza** do servidor | qualquer um publica pacote "oficial" | `invalida_antes` no catálogo: tudo assinado antes do corte deixa de valer |
| **"autêntico" tratado como "comprado"** | um pacote copiado serve para todos | licença separada, amarrada ao aparelho |
| **catálogo trocado no meio da rede** | app do atacante aparece na loja | catálogo assinado; item sem assinatura válida não entra |
| servidor de pacote comprometido | binário diferente do publicado | SHA-256 do catálogo + assinatura Ed25519 do `.giochi` |

O segundo é o que realmente rouba jogo, e é o mais fácil de errar sem perceber.
É por isso que `pacote.rs` e `licenca.rs` são módulos diferentes, com tipos
diferentes: `PacoteVerificado` prova **origem** e nada mais;
`LicencaVerificada` prova **compra** e exige o aparelho.

---

## 4.1 A segurança: três perguntas, três chaves

```text
                     servidor (nunca no cliente)
                     ┌──────────────────────┐
                     │ chave de CATÁLOGO    │ assina a lista e os pacotes
                     │ chave de LICENÇA     │ assina "este aparelho pagou"
                     └──────────────────────┘
                                │ públicas
                                ▼
   PC (loja)                                    aparelho
   ┌────────────────────┐                      ┌──────────────────────┐
   │ verifica catálogo  │  ── baixa .giochi ─► │ verifica pacote      │
   │ verifica pacote    │  ── envia ─────────► │ verifica licença     │
   │ NÃO assina nada    │                      │ tem a privada DELE   │
   └────────────────────┘                      └──────────────────────┘
```

| Pergunta | Quem responde | Chave | Onde a privada vive |
| --- | --- | --- | --- |
| "veio da Giochi?" | assinatura do pacote | catálogo | servidor |
| "este aparelho pagou?" | licença | licença (**outra**) | servidor |
| "é este aparelho?" | identidade | par gerado **no aparelho** | aparelho, nunca sai |

### Por que a licença é o que impede a cópia

Copiar `pacman.giochi` e o arquivo de licença para o aparelho do vizinho não
funciona: a licença carrega a **chave pública do aparelho de origem**, e a
verificação compara com a do aparelho que está rodando.

```text
licença.aparelho == aparelho.base64()   → válida
senão                                   → OutroAparelho  (recusa)
```

Testado nos dois sentidos, em `licenca.rs`: `copiar_o_pacote_e_a_licenca_para_outro_aparelho_falha`.

### O que isto **não** garante

Não é DRM inquebrável, e não fingimos que seja. Quem tem root no aparelho pode:

- copiar a chave privada do aparelho junto com a licença;
- rodar `bin/pacman` direto, sem passar pelo launcher.

**Em aparelho que o usuário controla, qualquer trava no cliente é contornável.**
O objetivo é outro e mais honesto:

1. a **cópia casual** para, que é o que acontece de verdade;
2. a proteção real é o **pagamento no servidor** — o pacote só é liberado depois
   do webhook do provedor;
3. nenhuma chave privada viaja para o cliente, então vazar binário não vaza
   capacidade de emitir licença.

Gastar esforço em ofuscação aqui seria teatro. O que dá para fazer bem, foi
feito.

### Cinco estados, não dois

`Veredito` tem `Valida`, `AssinaturaInvalida`, `OutroAparelho`, `OutroApp`,
`Expirada` e `NaoSei`. O último existe por um motivo concreto: **o aparelho liga
sem RTC** e o NTP corrige segundos depois. Sem hora confiável, uma licença
vencida parece válida e uma válida parece vencida.

`NaoSei` **não libera**. É a escolha conservadora: um verificador que aceita o que
não entendeu transforma qualquer falha de configuração em porta aberta. E licença
perpétua (`expira_em: None`) não depende do relógio — robustez de graça.

---

## 4.2 O pagamento: a loja não vê cartão

**PIX é o caminho principal.** O pagamento é um *pedido* — uma string que a loja
mostra sem manipular credencial nenhuma.

**Cartão vai para checkout hospedado**, aberto no navegador. O dado do cartão
nunca passa pelo processo da loja. Não é preciosismo: implementar formulário de
cartão significa assumir PCI-DSS SAQ D; redirecionar significa SAQ A. É a
diferença entre uma auditoria e nenhuma.

O desenho é o mesmo nos dois casos:

```text
1. loja   ── cria pedido ──────────────────────────►  backend
2. backend ── cria cobrança ───────────────────────►  provedor (PIX/Stripe)
3. loja   ◄── código PIX / URL de checkout ───────  backend
4. usuário paga
5. provedor ── webhook ────────────────────────────►  backend
6. loja   ── "já caiu?" ───────────────────────────►  backend
7. loja   ◄── licença ASSINADA (amarrada ao aparelho)
8. loja   ── baixa pacote, verifica, envia ao aparelho
```

O passo 7 é o único que produz `Estado::Pago`, e o tipo garante: `Pago` carrega
uma `LicencaAssinada`, que só existe com a chave do servidor. **O cliente não
consegue se declarar pago** — não é disciplina, é o compilador.

Teste que prova: `a_tela_de_pagamento_nao_se_declara_paga` aperta "Já paguei" dez
vezes e exige que o pedido continue em `Aguardando`.

### O que falta para cobrar de verdade

| Item | Estado |
| --- | --- |
| cliente do backend | **não escrito** — a loja hoje tem catálogo e demonstração |
| provedor (PIX/Stripe) | **não integrado** — exige conta, credencial e backend |
| webhook de confirmação | **do backend**, que não existe |
| contrato de API | **não escrito** — é o próximo documento |

Nada disso é deste repositório sozinho: cobrar exige CNPJ, conta no provedor e um
servidor. O que dá para fazer aqui — e está feito — é o **cliente correto** para
esse backend e o modelo de dados que ele vai usar.

---

## 4.3 A licença no aparelho: quem confere

O verificador de pacote já roda no aparelho (`verificador-ed25519`, §18 do
AGENTS). Falta o mesmo para licença, e falta o **launcher consultar**:

```text
launcher ── app tem licença válida? ── sim ──► lança
                                     └─ não ─► explica e oferece comprar
```

O binário de licença é o mesmo formato e a mesma `libcrypto`, então é trabalho
pequeno. Falta decidir **onde a chave pública de licença mora** — e é a mesma
pergunta já enviada ao Giochi System sobre `loja-publica.pem`.

---

## 5. Estado, medido em 06/11/2026

| Peça | Estado |
| --- | --- |
| Núcleo (catálogo, chave, licença, pedido, pacote, baixador) | **65 testes**, sem SDL |
| Interface (SDL2, quatro telas) | **9 testes** + prova de quadro |
| `--dump-frame` / `--teclas` | **executado**: quadro de 3.587.254 bytes, navegação muda o quadro |
| Build Linux | **executado**: 525.456 bytes em release |
| Build Windows e macOS | **CI escrito, não executado** |
| Emissão de licença (ferramenta de teste) | **executado**: emite e **confere** o que emitiu |
| Backend de pagamento | **não existe** |
| Licença validada no aparelho | **não** — só no PC |

## 6. O que este documento não promete

Não promete cobrança funcionando: sem backend e conta no provedor, não há
pagamento. O que existe hoje é o **cliente** — catálogo assinado, licença amarrada
ao aparelho, verificação de pacote, baixador com retomada — e a interface que o
mostra. A parte que não existe está listada como ausente, com o motivo.
