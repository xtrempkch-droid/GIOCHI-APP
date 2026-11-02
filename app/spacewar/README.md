# Spacewar

Jogo padrão do sistema. Original de 1962 para o PDP-1; projeto Godot 4.3 com
regras tiradas do listing original.

- **Runtime:** `godot`
- **Origem:** `Jogos/Spacewar/` no antecessor `GIOCHI-KONSOL-OS`
- **Licença:** domínio público — verifica antes de publicar

## Estado

Projeto Godot e o listing original (`Spacewar-1962-listing.pdp1`, 233 KB) ainda
**não migrados**. Ver [docs/migracao.md](../../docs/migracao.md).

O listing é documentação histórica: vai em `assets/`, não em `bin/`.

## Pendência para virar `.giochi`

Falta o **export ARM64** (template musl). Enquanto não houver export, `bin/` fica
vazio e o app não é instalável.