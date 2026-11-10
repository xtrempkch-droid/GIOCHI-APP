# Fonte

O launcher usa **DejaVu Sans**.

## O que colocar aqui

```text
assets/fonts/DejaVuSans.ttf
```

O código procura nesta ordem:

1. `assets/fonts/DejaVuSans.ttf` — junto do app, é o que vai para o aparelho
2. `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` — Debian/Ubuntu
3. `/usr/share/fonts/TTF/DejaVuSans.ttf` — Arch
4. `app/launcher/assets/fonts/DejaVuSans.ttf` — rodando da raiz do repositório

Ou passe explicitamente:

```bash
cargo run -- --font /caminho/DejaVuSans.ttf
```

## Por que DejaVu Sans

- **Licença permissiva.** Bitstream Vera + domínio público para as adições. Permite
  embarcar no aparelho sem obrigação de abrir o resto do sistema — diferente de uma
  fonte GPL.
- **Cobertura ampla.** Latin, grego, cirílico, setas e símbolos. Os rodapés usam
  `↑↓ ←→` e o catálogo tem acento em português; fonte sem esses glifos mostra
  caixa vazia.
- **Já está instalada** na maioria das distros Linux, então no PC não precisa
  copiar nada.

## Aviso: DejaVu é mais larga

DejaVu Sans é mais larga que Inter/Segoe UI. Os tamanhos em `theme.rs`
(`F_TINY=9`, `F_SMALL=11`, `F_BODY=13`) foram escolhidos pensando numa fonte
estreita. Se algum rótulo estourar — o rodapé e as etiquetas de categoria são os
candidatos — reduza o tamanho em `theme.rs` em vez de encurtar o texto:

```rust
pub const F_TINY: u16 = 8;
```

Os títulos longos já são truncados com reticências nos cartões, então o problema
aparece primeiro no rodapé.

## Se a fonte faltar

O desenho continua: só o texto é omitido, com um aviso no console. Não há
fallback embutido — inventar um bitmap de emergência daria texto ilegível pior
que a ausência.
