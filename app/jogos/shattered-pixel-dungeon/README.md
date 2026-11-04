# Shattered Pixel Dungeon

Jogo de 2014, GPL-3.0. **Não é jogo padrão do sistema** — é app instalável.

- **Runtime:** `native` (Java/libGDX, build via Gradle)
- **Licença:** `GPL-3.0-only`
- **Fonte:** https://github.com/00-evan/shattered-pixel-dungeon
- **Origem da nota:** `Jogos/Shattered Pixel Dungeon/` no antecessor `GIOCHI-KONSOL-OS`

## Estado

Projeto **não migrado** e **não portado**. A pasta do antecessor continha apenas
uma nota de intenção, sem código nem assets. O jogo upstream é Java/libGDX, não
Godot — um port seria reescrita, não adaptação.

## A restrição que decide onde este app pode morar

Não é "GPL contamina o sistema". A GPL-3.0 é explícita no §5, último parágrafo:
incluir obra coberta num **agregado** não estende a licença às outras partes. Um
jogo GPL ao lado do launcher na mesma imagem é agregado; o launcher, o kernel e o
resto continuam sob a licença deles.

O impedimento real é outro, e é **§6 — Installation Information**. Entregar
código-objeto GPL *em*, *com* ou *especificamente para* um "User Product" — um
portátil de consumo é exatamente isso — exige fornecer "métodos, procedimentos,
**chaves de autorização** ou outras informações necessárias para instalar e
executar versões modificadas".

O Giochi tem rootfs **assinado** (Ed25519), com a chave na mão do projeto. Se o
jogo vive assinado dentro da imagem, o usuário não consegue instalar a versão
modificada dele, mas o projeto consegue — então a dispensa do §6 **não vale**.
Entregar a chave de assinatura destruiria a raiz imutável, que é premissa do
sistema.

### O caminho que funciona

O app sai da imagem assinada e passa a ser instalável em `/user`, a partição
gravável. Ali a *Installation Information* é um procedimento público e sem chave
("coloque sua build em `/user/games/<id>`"), e a raiz assinada continua intacta.

**Consequência para o projeto:** este app **não pode** entrar na imagem assinada.
Só pode ser distribuído pela loja, instalado em `/user`.

## Obrigações ao distribuir

| Obrigação | Situação |
| --- | --- |
| Aviso de copyright e cópia da licença | obrigatório |
| Corresponding Source disponível | obrigatório — apontar `licenseUrl` para o upstream e/ou espelhar |
| Trabalho inteiro sob GPL-3.0 para quem recebe | obrigatório (§5c) |
| Installation Information | obrigatório (§6) — resolvido pelo caminho em `/user` |
| Cobrar pelo app | **permitido** — a GPL não proíbe vender |

Vender é permitido. O que não se pode é impedir o comprador de redistribuir ou de
instalar a versão modificada dele.

## O que falta

1. Port ou build do upstream para `aarch64`.
2. Decidir se o app é distribuído como está (GPL) ou reescrito com arte própria —
   o mantenedor do antecessor anotou a intenção de "reestruturar com acabamento
   próximo de Zenonia, sem ferir direitos autorais", o que seria outro jogo.
3. Multi-idiomas obrigatório, se seguir o padrão dos outros jogos do catálogo.
