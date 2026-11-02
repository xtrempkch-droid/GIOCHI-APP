# Giochi Forge

Ferramenta de empacotamento e classificação de apps. Lê o `forge.json` de um
projeto e diz se ele cabe no perfil do aparelho.

- **Runtime:** `native` (Python)
- **Origem:** `apps/forge/` no antecessor `GIOCHI-KONSOL-OS`
- **Entrypoint:** `bin/giochi-forge`

## Estado

Código-fonte ainda **não migrado**. Ver [docs/migracao.md](../../docs/migracao.md).

## Pendência de modelagem

O forge é ferramenta de linha de comando, sem interface gráfica. O classificador
assume `graphics_api_min: gles3` quando a chave está ausente — o que é errado para
uma ferramenta headless. Precisa de uma forma de declarar "não exige gráficos".