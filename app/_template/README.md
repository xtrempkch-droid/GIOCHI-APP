# Template de app

Copie esta pasta para criar um app novo:

```bash
cp -r app/_template app/meu-app
```

Depois edite o `README.md` para descrever o app de verdade.

## Arquivos

| Caminho | Obrigatório | Papel |
| --- | --- | --- |
| `metadata.json` | sim | identidade do pacote: título, versão, arquitetura, runtime, entrypoint |
| `forge.json` | não | requisitos de hardware/software que o aparelho precisa ter |
| `capa.png` | sim (para publicar) | capa 320×320 exibida na loja |
| `thumb.png` | não | miniatura |
| `bin/` | sim | **o que é instalado no aparelho**; o `entrypoint` aponta para dentro daqui |
| `src/` | não | **código-fonte**: Rust, C++, Python |
| `assets/` | não | **arte-fonte**: logos, ícones, fontes, sons, arquivos editáveis |
| `README.md` | sim | o que o app é, como rodar e o que ainda falta |

## `src/`, `bin/` e `assets/` — não confunda

| Pasta | O que é | Vai para o aparelho? |
| --- | --- | --- |
| `src/` | o código que você edita | **não** |
| `bin/` | o resultado do build do `src/` | **sim** |
| `assets/` | o material de arte em alta resolução | **não** |

O **build** do `src/` é que vai para `bin/`. Enquanto não houver build, `bin/`
fica vazio e o app não é instalável — e isso está correto: fonte não é artefato.

## Regras que o validador cobra

- `version` em semver: `1.0.0`, não `1.0`.
- `runtime` só aceita `godot`, `retroarch`, `proton` ou `native`. Na prática é
  `native` para todos os apps deste repositório: `godot` sai **Incompatível**
  porque nenhum build oficial do motor tem KMSDRM (ver `docs/godot.md`).
- `arch` é conferido em par com `distribution`: `aarch64` para `image`/`store`,
  `x86_64`/`arm64`/`any` para `host`.
- `entrypoint` começa em `bin/`, não pode ser absoluto nem conter `..`, e o
  arquivo precisa existir.
- `capa.png`, quando existir, precisa ser **320×320**.

Valide com:

```bash
node scripts/validate-app.mjs app/meu-app
```
