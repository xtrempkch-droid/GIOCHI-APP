# Arcade Volleyball

Jogo padrão do sistema. Original de 1988; aqui é um projeto Godot 4.3 com port
C++/SDL2 de referência.

- **Runtime:** `godot`
- **Origem:** `Jogos/Arcade Volleyball/` no antecessor `GIOCHI-KONSOL-OS`
- **Licença:** domínio público / arte própria — verifica antes de publicar

## Estado

Projeto Godot ainda **não migrado** para `src/`. Ver [docs/migracao.md](../../docs/migracao.md).

## Pendência para virar `.giochi`

Falta o **export ARM64** (template musl). Enquanto não houver export, `bin/` fica
vazio e o app não é instalável — o projeto em `src/` é fonte, não artefato.