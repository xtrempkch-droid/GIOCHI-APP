# Migração do antecessor `GIOCHI-KONSOL-OS`

Os apps deste repositório vêm do antecessor monolítico
[GIOCHI-KONSOL-OS](https://github.com/xtrempkch-droid/GIOCHI-KONSOL-OS).

O código-fonte **não foi copiado automaticamente** para cá: são projetos Godot com
assets binários e pacotes Python, e copiar por API produziria uma árvore
incompleta e sem histórico. Em vez disso há um script que faz a cópia de forma
verificável na sua máquina, onde o `git` existe.

## O que vem e o que não vem

| Antecessor | Novo caminho | Situação |
| --- | --- | --- |
| `apps/forge/` | `app/forge/src/` | migrar |
| `apps/store/` | `app/store/src/` | migrar |
| `Jogos/Arcade Volleyball/godot/` | `app/jogos/arcade-volleyball/src/` | migrar |
| `Jogos/Colossal Cave Adventure/godot/` | `app/jogos/colossal-cave-adventure/src/` | migrar |
| `Jogos/Spacewar/godot/` | `app/jogos/spacewar/src/` | migrar |
| `Jogos/Spacewar/Spacewar-1962-listing.pdp1` | `app/jogos/spacewar/assets/` | migrar (documentação) |
| `apps/launcher/` | — | **não vem** |
| `Jogos/Shattered Pixel Dungeon/` | — | **não vem** |

### Por que o `launcher` não vem

Decisão do usuário (02/11/2026): o launcher será **redesenhado por completo**. Trazer
o código atual só criaria trabalho de remoção depois.

### Por que Shattered Pixel Dungeon não vem

A própria documentação do antecessor exclui: é **GPL-3.0**, não é domínio público,
e a regra do catálogo é aceitar apenas domínio público, permissão dos autores ou
arte própria. GPL e software proprietário ficam de fora.

Os outros títulos da `Lista de jogos.md` (Spatial Hyperdrive, Giana Sisters, Eye of
the Beholder, Gloom, Crazy Sue, Super Obliteration) são **candidatos sem código**
no repositório — não há o que migrar.

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

## Depois de migrar: o que ainda falta

Migrar o fonte **não torna o app instalável**. Os três jogos são projetos Godot e
precisam de **export ARM64** para virar pacote `.giochi`:

```text
src/  (projeto Godot)  ──export ARM64──►  bin/  (artefato)  ──►  .giochi
```

O export exige o template de exportação do Godot para `linux arm64`, que não está
neste repositório. Até lá, `bin/` fica vazio e o validador apenas avisa
(`bin_vazia`), sem reprovar.

## O que ainda não existe

- **Empacotador `.giochi`** — o formato está descrito em [pacote.md](pacote.md), mas
  não há script que gere o arquivo a partir de `app/<slug>/`.
- **Capa 320×320** — `capa.png` precisa ser gerada a partir do logo em `assets/`;
  nenhum app tem capa ainda, então o validador emite `capa_ausente`.
- **`device_profile.json`** — do lado do Giochi System. Sem ele, a classificação de
  compatibilidade não tem com o que comparar.
