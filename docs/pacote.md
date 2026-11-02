# Formato do pacote `.giochi`

O arquivo que a loja instala no aparelho. Formato herdado do antecessor
`GIOCHI-KONSOL-OS`.

## Ordem dos membros é obrigatória

Um arquivo `tar` **ustar**, com os membros nesta ordem:

```text
metadata.json  →  capa.png  →  thumb.png (opcional)  →  bin/
```

A ordem não é capricho: o extrator no aparelho lê em **streaming** e precisa de
`metadata.json` e `capa.png` **antes** de decidir se aceita o pacote. Se o binário
vier primeiro, o aparelho já baixou megabytes antes de saber que o app é
incompatível.

## Regras

| Item | Regra |
| --- | --- |
| Formato | `tar` ustar puro (sem gzip, sem extensões GNU) |
| `metadata.json` | ≤ **16 KiB**, validado contra [metadata.schema.json](../schemas/metadata.schema.json) |
| `capa.png` | **320×320**, obrigatória |
| `thumb.png` | opcional; miniatura |
| `bin/` | o que é instalado; o `entrypoint` do metadata aponta para dentro |
| `entrypoint` | precisa existir no pacote |

## O que **não** entra no pacote

- `src/` — código-fonte
- `assets/` — arte-fonte (logo em alta, `.kra`, `.psd`, fontes)
- `forge.json` — o manifesto de requisitos é consumido na **publicação**, pelo
  ferramental do Giochi app; não precisa viajar para o aparelho
- `README.md`

O pacote leva só o que o aparelho precisa executar. Um projeto Godot completo
dentro dele seria centenas de megabytes de arquivos editáveis.

## Assinatura

O antecessor usava **Ed25519 + SHA-256**, com dois arquivos ao lado do pacote:

```text
app.giochi
app.giochi.sig            # assinatura Ed25519
app.giochi.manifest.json  # {schema_version, package, version, sha256, size_bytes, signature}
```

O verificador no aparelho recusa o pacote se a assinatura não conferir.

## Estado real: o antecessor já implementa tudo isso

**Correção (02/11/2026).** A versão anterior deste documento dizia "nada disso está
implementado". Está errado: o `GIOCHI-KONSOL-OS` tem a cadeia completa, e ela roda
no CI dele. O que falta aqui é **migrar**, não desenhar.

Ferramentas que existem lá:

| Arquivo | O que faz |
| --- | --- |
| `tools/giochi-pack.sh` | monta o `.giochi` na ordem obrigatória |
| `tools/giochi-keys.sh` | gera par de chaves Ed25519 |
| `tools/giochi-sign.sh` | assina o pacote, gerando `.sig` e `.manifest.json` |
| `build/host/bin/giochi-verify` | verifica assinatura + SHA-256 |
| `build/host/bin/giochi-extract` | extrai no aparelho |

### O teste que prova que funciona

O job `package-roundtrip` do `ci.yml` deles faz o ciclo inteiro:

```text
pack → keys → sign → verify → extract
```

E depois **adultera de propósito**: copia o pacote, acrescenta um byte, e exige que
`giochi-verify` **recuse**. Se aceitar, o job falha com "imagem adulterada foi
aceita".

Esse teste é o modelo a copiar. Ele cobre o que a documentação não cobre: que o
verificador não é decorativo.

### Duas lições do teste deles

1. **Preservar o bit de execução.** O teste faz `test -x "$out/bin/ci"` no arquivo
   extraído. Se o empacotador perder a permissão, o app não abre — e o erro aparece
   só no aparelho, como "entrypoint não existe".
2. **Só o comprimido vai para o artefato.** Publicar `.img` cru (~920 MB) em vez do
   comprimido (~40–90 MB) estoura armazenamento sem ganho nenhum.

### Divergência de schema a resolver

O `metadata.json` dos testes deles inclui **`graphics_api`**:

```json
{"title":"CI","developer":"ci","version":"1.0.0","arch":"aarch64",
 "runtime":"godot","graphics_api":"gles3","entrypoint":"bin/ci"}
```

O nosso `metadata.schema.json` não tem esse campo — e como ele é
`additionalProperties: false`, um pacote herdado **seria reprovado** pelo nosso
validador. É exatamente o tipo de quebra silenciosa que a migração vai encontrar:
decidir se `graphics_api` entra no metadata ou se fica só no `forge.json`.

## O que falta para o pacote existir

1. **Export ARM64** dos projetos Godot (`src/` → `bin/`).
2. **Migrar** empacotador e assinador do antecessor (não escrever do zero).
3. **Portar o teste de ida e volta**, incluindo a checagem de adulteração.

Nenhum dos três foi feito aqui ainda.
