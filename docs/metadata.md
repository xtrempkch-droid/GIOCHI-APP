# Contrato de `metadata.json`

Identidade de um app. Schema em [`schemas/metadata.schema.json`](../schemas/metadata.schema.json).

## Campos

| Campo | Tipo | Obrigatório | Regra |
| --- | --- | --- | --- |
| `title` | string | sim | nome exibido na loja, 1–120 caracteres |
| `developer` | string | sim | 1–120 caracteres |
| `version` | string | sim | **semver**: `1.0.0`, nunca `1.0` |
| `arch` | enum | sim | `aarch64` (valor único) |
| `runtime` | enum | sim | `godot`, `retroarch`, `proton` ou `native` |
| `entrypoint` | string | sim | começa em `bin/`, não absoluto, sem `..` |
| `graphics_api` | enum | não | `gles2` ou `gles3` |
| `buttons_map` | objeto | não | mapeamento de botões específico |

## Por que `additionalProperties: false`

O schema **recusa** qualquer chave fora da lista. Isso é deliberado e herdado do
antecessor.

O motivo é o custo do erro silencioso. Se um campo inventado fosse aceito sem
reclamação, alguém escreveria `"dependencias": [...]` esperando que funcionasse,
o arquivo passaria, e o app seria publicado sem a dependência. O erro só
apareceria no aparelho, longe de quem digitou.

Recusar campo desconhecido transforma um erro silencioso em mensagem de
validação — que é exatamente a troca que este projeto escolheu fazer.

**Consequência:** adicionar um campo novo exige mudar **três** lugares — este
schema, o validador em `scripts/lib/apps.mjs` e a documentação. É de propósito:
campo novo em contrato público é decisão, não detalhe.

## Por que `arch` tem um valor só

O console é `aarch64`. Emular outra arquitetura está fora de escopo (decisão
herdada do antecessor, ADR-13). Um enum com um valor só deixa isso explícito em
vez de aceitar `x86_64` e falhar depois.

## Por que `entrypoint` precisa existir no disco

Um `entrypoint` apontando para arquivo inexistente é o erro mais comum e o mais
tarde descoberto: o app é publicado, o usuário instala, e nada abre.

O validador resolve o caminho contra `bin/` e confere que o arquivo está lá. Não
valida se é executável nem se roda no aparelho — isso exige o hardware.
