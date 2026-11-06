# Giochi Forge

Ferramenta de PC que classifica e empacota apps: lê o `forge.json` de um projeto,
diz se ele cabe no perfil do aparelho e monta o `.giochi` assinado.

- **Runtime:** `native` (Python)
- **`arch`:** `any` — roda no computador do usuário, não no console
- **Origem:** `apps/forge/` no antecessor `GIOCHI-KONSOL-OS`
- **Entrypoint:** `bin/giochi-forge`

## Estado

**Pronto.** Fonte migrada do antecessor, 57 testes, e distribuível nos três
sistemas: `bin/giochi-forge` (Linux/macOS), `.pyz` e `.cmd` na release
(ver [docs/pc.md](../../docs/pc.md)).

```bash
app/forge/bin/giochi-forge --version
node scripts/empacotar-pc.mjs            # gera dist/pc/giochi-forge.pyz
bash scripts/testar-apps-pc.sh           # os 57 testes
```

## Subcomandos

| Comando | O que faz |
| --- | --- |
| `profile --device x55` | emite o `device_profile.json` de um aparelho conhecido |
| `check --device x55 --project .` | classifica Compatível / Parcial / Incompatível |
| `build --project . --out jogo.giochi` | empacota na ordem da ADR-11 (opcionalmente assina) |
| `store --src baixados/ --out pacotes/` | varre uma pasta de projetos e empacota o que já dá |
| `keys --out keys/` | gera o par Ed25519 de assinatura |

## Por que `graphics_api_min: "none"` existe

Esta era a pendência anotada aqui: *"o classificador assume `gles3` quando a
chave está ausente — o que é errado para uma ferramenta headless"*.

Medido, e era pior que uma imprecisão: com a chave ausente, **a própria forge era
reprovada** num RK3326, que só tem GLES 2.0.

```text
$ giochi-forge check --device rk3326-1gb --project app/forge
Incompatível
  - [Incompatível] graphics_api: exige GLES3; dispositivo nao declara gles3
```

O enum ganhou `none`, e o `forge.json` deste app declara `none`. Um app que não
desenha nada agora passa:

```text
$ giochi-forge check --device rk3326-1gb --project app/forge
Compatível
```

**O default continua `gles3`** de propósito: um app gráfico que esquece de
declarar tem de ser tratado como exigente, não como headless. `none` é uma
declaração, não um esquecimento.

## Os aparelhos conhecidos

`--device` aceita os perfis medidos (`x55`, `x35h`, `x35s`) e os genéricos de SoC
(`rk3326-1gb`, `rk3326-2gb`, `rk3566-2gb`). Tabela em
[docs/forge.md](../../docs/forge.md#perfis-de-aparelho).

Se o aparelho não está na tabela, o `Giochi System` publica o
`device_profile.json` e o forge lê:

```bash
giochi-forge check --profile device_profile.json --project .
```
