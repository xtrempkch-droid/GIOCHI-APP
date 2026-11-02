# Licenciamento

Como o projeto distribui software e como cobra por ele.

> **Não é aconselhamento jurídico.** A estrutura abaixo está tecnicamente
> justificada, mas antes de vender é necessário revisão de um advogado de
> propriedade intelectual. As licenças envolvidas têm consequências reais.

## Como o software sai do projeto

Três destinos diferentes, com obrigações diferentes:

```text
                    ┌──────────────────────────────┐
                    │  Loja (roda no PC do usuário)│
                    └──────────────┬───────────────┘
                                   │ envia o pacote pronto
                                   ▼
  imagem assinada  ──►  aparelho  ──►  /user (gravável)
   (rootfs imutável)                   apps instalados
```

| Destino | O que é | Quem manda na licença |
| --- | --- | --- |
| **`image`** | vai dentro da imagem assinada do aparelho | exige licença permissiva |
| **`store`** | baixado pela loja e instalado em `/user` | a licença do próprio app |
| **`host`** | roda no PC do usuário (loja, forge) | proprietária, sem restrição de copyleft |

O campo `distribution` em `app/apps.json` define o destino, e o validador
**recusa** combinações ilegais — ver a regra mecânica abaixo.

## A loja roda no PC, não no aparelho

Decisão do usuário (02/11/2026): a loja fica no **computador do usuário** e envia
para o aparelho o pacote pronto para uso.

Isso simplifica o licenciamento de forma decisiva:

- o pacote baixado **não faz parte da imagem assinada**;
- ele é instalado em `/user`, a partição gravável;
- logo, app GPL **pode** ser vendido e distribuído pela loja.

O comprador recebe o binário e pode substituí-lo pela própria build em
`/user/games/<id>`. É exatamente o procedimento público e sem chave que a GPL-3.0
§6 exige. O problema de *anti-tivoization* só existiria se o app GPL morasse
**dentro** da imagem assinada.

## A regra mecânica

O validador não confia na disciplina de quem edita. Em `scripts/lib/apps.mjs`:

- `distribution: "image"` + licença `GPL`/`AGPL`/`LGPL` → **erro**
  (`licenca_gpl_na_imagem`);
- `distribution: "image"` sem licença declarada → **aviso** (`licenca_ausente`);
- `distribution: "store"` + GPL → **permitido**.

O motivo está escrito no código, não só aqui: quem for adicionar um app daqui a
seis meses vai ler a mensagem de erro.

## O que continua GPL dentro da imagem, queira ou não

A imagem conterá componentes GPL-2.0 inevitáveis:

| Componente | Licença | Obrigação |
| --- | --- | --- |
| Linux kernel | GPL-2.0 | oferecer a fonte correspondente |
| BusyBox | GPL-2.0 | oferecer a fonte correspondente |
| U-Boot | GPL-2.0 | oferecer a fonte correspondente |

Isso **não** torna a imagem inteira GPL. O kernel e o BusyBox continuam GPL; o
launcher, a loja e os jogos próprios continuam sob a licença escolhida. É
agregação, não derivação.

**Obrigação prática:** publicar a fonte desses componentes (ou apontar para o
upstream na versão exata usada) junto com a imagem. O `build-info.txt` do build já
registra versão e commit — falta o aviso de licença.

## Buildroot não contamina a imagem

Erro comum: assumir que usar Buildroot obriga a abrir a imagem. Não obriga. O
Buildroot é GPL-2.0 como **ferramenta de build**; os arquivos de configuração e as
imagens geradas não herdam essa licença. O que define a licença da imagem são os
pacotes que entram nela.

## Estrutura de licenças deste repositório

| Camada | Licença |
| --- | --- |
| Ferramental deste repositório (validador, schemas, scripts) | ver `LICENSE` |
| Apps próprios (forge, store, jogos do catálogo) | proprietária — ver `metadata.json` de cada um |
| Apps de terceiros (Shattered Pixel Dungeon) | a licença do upstream |

A licença de cada app fica em `metadata.json` → campo `license`, em identificador
SPDX (`MIT`, `GPL-3.0-only`, `Apache-2.0`) ou `Proprietary`. Assim a informação
viaja **com o pacote**, e a loja pode exibi-la antes da compra.

## Cobrar por software GPL

**É permitido.** A GPL não proíbe vender. O que ela proíbe é impedir o comprador de
redistribuir ou de instalar a versão modificada dele.

Na prática, vender um app GPL só funciona se o valor estiver no serviço — suporte,
curadoria, empacotamento, atualização — e não no monopólio do binário. Para
monopólio real, o app precisa ser **próprio** ou de licença permissiva.

## Pendências

1. ~~Licença deste repositório~~ — **decidido em 02/11/2026: permanece MIT.**
   Critério: o objetivo era poder distribuir o sistema e vender apps, e MIT já
   permite os dois. Exclusividade sobre o sistema não é objetivo — aceita-se que
   terceiros possam reaproveitar o código.
2. ~~Licença dos apps próprios~~ — **decidido em 02/11/2026: `Proprietary`.**
   O comprador não pode redistribuir os apps. É o que dá base para cobrar pelo
   binário.
3. **Aviso de licença na imagem** — `NOTICE`/`LICENSES/` com as obrigações dos
   componentes GPL-2.0 (kernel, BusyBox, U-Boot) ainda não existe no Giochi System.
   Enquanto não existir, distribuir a imagem deixa essas obrigações em aberto.
4. **EULA** para os apps vendidos — sem texto de licença de uso, a venda não tem
   base contratual.
5. **Revisão jurídica** antes de qualquer cobrança.

### O que MIT não protege

MIT permite que qualquer pessoa pegue este código, renomeie e venda. Isso foi
aceito conscientemente: o diferencial competitivo está nos apps proprietários e no
serviço, não no monopólio do código do sistema.

Se a intenção mudar, o caminho é trocar `LICENSE` para proprietária — e aí o
histórico do Git continua público para quem já clonou, o que é inevitável.
