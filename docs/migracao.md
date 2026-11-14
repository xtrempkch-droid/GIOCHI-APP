# Migração do antecessor `GIOCHI-KONSOL-OS`

Os apps de PC deste repositório vêm do antecessor monolítico
[GIOCHI-KONSOL-OS](https://github.com/xtrempkch-droid/GIOCHI-KONSOL-OS).

O código-fonte **não foi copiado automaticamente** para cá: são pacotes Python com
assets binários, e copiar por API produziria uma árvore incompleta e sem
histórico. Em vez disso há um script que faz a cópia de forma verificável na sua
máquina, onde o `git` existe.

## O que vem e o que não vem

| Antecessor | Novo caminho | Situação |
| --- | --- | --- |
| `apps/forge/` | `app/forge/src/` | **migrado** |
| `apps/store/` | `app/store/src/` | **migrado** |
| `apps/launcher/` | — | **não vem**: será redesenhado por completo |
| `Jogos/**` | — | **não vem**: os jogos saíram deste repositório em 14/11/2026 |

### Por que o `launcher` não vem

Decisão do usuário (02/11/2026): o launcher será **redesenhado por completo**.
Trazer o código atual só criaria trabalho de remoção depois. O que existe hoje é
Rust + SDL2 escrito aqui, e está em `app/launcher/`.

### Por que os jogos não vêm

Eles moraram em `app/jogos/` até 14/11/2026 e foram para um repositório próprio.
Este repositório trata de **apps do sistema**; o script de migração não copia mais
nada de `Jogos/`, porque recriar `app/jogos/` faria o validador reprovar com
`app_nao_registrado` — pasta que existe e não está no `apps.json` é pior que pasta
ausente.

A documentação do antecessor também excluía o **Shattered Pixel Dungeon** por
licença: é GPL-3.0, e a regra da imagem assinada aceita apenas domínio público,
permissão dos autores ou arte própria. GPL pode ir pela **loja**
(`distribution: "store"`), não pela imagem — ver
[licenciamento.md](licenciamento.md).

## Como migrar

Pré-requisito: `git` instalado.

```bash
# 1. Clone o antecessor em algum lugar
git clone https://github.com/xtrempkch-droid/GIOCHI-KONSOL-OS.git ~/giochi-antigo

# 2. Rode o script apontando para o clone
bash scripts/migrar-do-antecessor.sh ~/giochi-antigo

# 3. Confira o resultado
node scripts/validate-app.mjs --all
```

O script **não sobrescreve** nada que já exista: ele copia para `src/` e `assets/`
e avisa o que pulou. Também imprime um resumo do que foi copiado e do que faltou.

## O estado depois da migração

| Item | Estado |
| --- | --- |
| empacotador `.giochi` | **existe** — `scripts/empacotar.mjs`, ver [pacote.md](pacote.md) |
| verificação de pacote | **existe** — `scripts/verificar-pacote.mjs` + verificador no aparelho |
| capa 320×320 | existe em `forge`, `loja`, `painel`, `store`, `launcher`, `star-dust`; falta em `launcher-teste` |
| perfis de aparelho | **existem** no `forge` (`device.conf` do Giochi System + manuais medidos) |
| apps de PC empacotados | **existe** — `.pyz` determinístico + `.sh`/`.cmd`, ver [pc.md](pc.md) |
