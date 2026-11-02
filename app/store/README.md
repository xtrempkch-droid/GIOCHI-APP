# Giochi Store

Loja de apps: instala, atualiza e remove pacotes `.giochi`, checando
compatibilidade com o aparelho antes de instalar.

- **Runtime:** `native` (Python)
- **Origem:** `apps/store/` no antecessor `GIOCHI-KONSOL-OS`
- **Entrypoint:** `bin/giochi-store`

## Estado

Código-fonte ainda **não migrado**. Ver [docs/migracao.md](../../docs/migracao.md).

## Dependência do perfil do aparelho

A store compara o `forge.json` de cada app com o `device_profile.json` publicado
pelo Giochi System. Enquanto esse perfil não existir, a verificação de
compatibilidade não tem com o que comparar.