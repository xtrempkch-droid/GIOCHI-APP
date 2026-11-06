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

## 2. O que NÃO existe: a loja

| Peça | Estado | Por que não foi feita |
| --- | --- | --- |
| Login OAuth no itch.io | **não escrita** | precisa de `client_id` registrado no itch.io; sem ele o código não executa uma linha |
| butler / butlerd | **não integrado** | precisa de rede e de credencial; e o binário não está nesta máquina |
| Interface gráfica da loja | **não escrita** | depende de decidir a stack (ver §4) |
| Transferência por USB automática | **parcial** | `aparelho.sh instalar-pacote` faz o envio; o "detectar pendrive" não existe |
| Compra/checkout | **não escrita** | não há API pública; ver §3 |

`app/store/` e `app/forge/` estão em `scaffold`, sem `bin/`, e declarados como
`distribution: host` (rodam no PC, não vão para a imagem).

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

## 4. Decisões que precisam ser tomadas antes de escrever a loja

Não são detalhes de implementação — mudam o desenho:

1. **Onde a loja roda.** O relatório diz "no PC do usuário", e é o que o
   `apps.json` já declara (`distribution: host`). Confirmar: uma loja gráfica no
   PC pressupõe uma stack de UI que este repositório não tem (o launcher é
   SDL2/Rust específico para o aparelho).
2. **butlerd ou butler.** O `butlerd` é um daemon JSON-RPC e faz download
   incremental; o `butler` é CLI. Para uma loja gráfica o `butlerd` é a escolha,
   mas é mais uma peça para empacotar e supervisionar.
3. **Assinatura: quem tem a chave.** Hoje a chave privada fica em
   `~/.giochi/` na máquina de quem empacota. Numa loja, a chave do **catálogo**
   (o que a Giochi assina) é diferente da chave de **terceiro** (o que um
   desenvolvedor assina). O formato tem um campo de assinatura só; multi-assinatura
   é mudança de formato.
4. **O aparelho confia no cabo.** Hoje a verificação é no PC, antes de enviar. Se
   o `.giochi` puder chegar por pendrive, o aparelho precisa verificar — e isso
   exige ter a chave pública no rootfs, o que é decisão de imagem (Giochi System).

---

## 5. Ordem sugerida

O que dá para fazer sem depender de credencial nem de decisão alheia:

1. ~~Empacotador + assinador~~ — **feito**.
2. ~~Extrator + catálogo automático~~ — **feito**.
3. **Verificação no aparelho** — o launcher ganha `verificar-pacote` em Rust e
   lê `.giochi` de `/media` ou `/user`. Isso tira a dependência de PC e é
   inteiramente deste repositório. **É o próximo passo natural.**
4. **Um jogo Godot empacotado** — depende de export ARM64, que depende de
   template de exportação. Sem isso, o formato está provado mas só com jogos
   nativos.
5. **butlerd** — só depois de existir `client_id` e uma rede para testar.

Sobre (3): hoje quem verifica é o PC, e o aparelho confia no cabo. Enquanto o
cabo for o único caminho, isso é aceitável e está registrado. Deixa de ser quando
o arquivo puder vir de um pendrive.

---

## 6. O que este documento não promete

Não promete loja funcionando, nem data. A parte que existe foi executada no
aparelho; a que não existe está listada como ausente, com o motivo.
