# Contrato de `metadata.json`

Identidade de um app. Schema em [`schemas/metadata.schema.json`](../schemas/metadata.schema.json).

## Campos

| Campo | Tipo | Obrigatório | Regra |
| --- | --- | --- | --- |
| `title` | string | sim | nome exibido na loja, 1–120 caracteres |
| `developer` | string | sim | 1–120 caracteres |
| `version` | string | sim | **semver**: `1.0.0`, nunca `1.0` |
| `arch` | enum | sim | `aarch64` no aparelho; `x86_64`, `arm64` ou `any` em app de PC |
| `runtime` | enum | sim | `godot`, `retroarch`, `proton` ou `native` |
| `entrypoint` | string | sim | começa em `bin/`, não absoluto, sem `..` |
| `graphics_api` | enum | não | `gles2` ou `gles3` |
| `buttons_map` | objeto | não | mapeamento de botões específico |
| `releaseYear` | inteiro | não | ano de lançamento, quatro dígitos (`1988`) |
| `category` | string | não | gênero exibido no cartão (`Arcade`, `Esporte`), até 40 caracteres |

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

## Por que `category` existe

O cartão da Home mostra `Cobrinha · Arcade · 2026`. O gênero é dado do programa,
como o ano — e, como o ano, precisa de campo próprio: sem ele o gerador de
catálogo não tem de onde tirar e todos os cartões diriam "Aplicativo".

Ele entra na mesma cadeia de três elos do `releaseYear`:

```text
metadata.json:category  →  índice da loja: category  →  cartão
       Arcade                    "Arcade"                 Arcade
```

Ausente é aceitável: a UI cai em "Aplicativo". Não é o caso de `—`, porque um
gênero genérico é honesto — diferente de um número inventado.

## Por que `releaseYear` existe

O cartão da Home mostra `Spacewar • Ação • 1962`. O ano é **dado do programa**,
publicado junto com ele — não algo que o launcher possa deduzir.

Houve um período em que não havia campo nenhum, e o launcher preencheu o buraco
com o id do jogo: o cartão desenhava `Spacewar • Ação • #002`. Lido na tela,
parece informação; `002` é o índice do cartão. Foi o usuário que percebeu, não
um teste.

A cadeia é esta, e cada elo depende do anterior:

```text
metadata.json:releaseYear   →  índice da loja: year  →  catálogo: Game.year
       1988                          "1988"                1988 (u16)
```

Sem o primeiro elo, a loja não tem o que repassar e o launcher não tem o que
desenhar. Por isso o campo precisa ser aceito aqui **e** repassado adiante — ver
[`docs/integracao.md`](integracao.md) §2.1.

`0`/ausente é **desconhecido**, nunca "ano zero": a UI mostra `—`, o mesmo
marcador que já usa para bateria e temperatura não lidas. Substituto de dado
ausente precisa ser declarado ausente, não preenchido com número interno.

O fabricante segue a mesma lógica e já existia: `developer` é obrigatório e é
quem a ficha exibe. Não há campo separado de "publicadora" — se um dia houver
diferença entre desenvolvedor e publicador, ela entra como campo novo, com a
mesma regra dos três lugares.

## Por que `arch` depende de `distribution`

O console é `aarch64`. Emular outra arquitetura está fora de escopo (decisão
herdada do antecessor, ADR-13).

Mas `distribution: "host"` descreve o oposto: o app roda no **computador do
usuário**. Até 06/11/2026 o enum tinha um valor só, e a loja e o forge eram
obrigados a declarar `aarch64` sendo programas de PC — o campo existia e era
sempre falso.

Hoje o valor é validado **em par** com `distribution`:

| `distribution` | `arch` aceito | por quê |
| --- | --- | --- |
| `image`, `store` | `aarch64` | vai para o console |
| `host` | `x86_64`, `arm64`, `any` | roda no PC; `any` = sem binário nativo (Python puro) |

Declarar `aarch64` num app de PC reprova com
`arquitetura_de_console_em_app_de_pc`, e o inverso com
`arquitetura_de_pc_em_app_do_aparelho`. Só ampliar o enum teria deixado o campo
voltar a não significar nada — a graça de um valor só era justamente não poder
estar errado.

## Por que `entrypoint` precisa existir no disco

Um `entrypoint` apontando para arquivo inexistente é o erro mais comum e o mais
tarde descoberto: o app é publicado, o usuário instala, e nada abre.

O validador resolve o caminho contra `bin/` e confere que o arquivo está lá. Não
valida se é executável nem se roda no aparelho — isso exige o hardware.
