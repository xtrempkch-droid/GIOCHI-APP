# Controles

Como o launcher recebe input e como o usuário gerencia os controles.

---

## 1. Entrada: evdev direto

**Decisão (02/11/2026): o launcher lê `/dev/input/event*` diretamente. O SDL fica só
para desenhar.**

O mapa por aparelho é `device/<pasta>/buttons.map` no **Giochi System**, derivado do
DTS. Como extrair, o que já foi descoberto e por que os bancos públicos não servem
estão em
[Giochi system/docs/button-maps.md](../../Giochi%20system/docs/button-maps.md).

### Por que não a API de gamepad do SDL

Os botões desses handhelds vêm de driver próprio (`rocknix-joypad`). O SDL só
reconhece um controle se houver entrada no `SDL_GameControllerDB` — que não existe
para aparelho custom. Resultado: **no PC o SDL funciona, no aparelho não**.

Segundo motivo, mais sutil: `buttons.map` mapeia por **código evdev**
(`BTN_EAST`), mas o SDL entrega **índice de botão** (0, 1, 2…). Os dois não
conversam. Ou traduzimos índice→nome em código, ou lemos evdev e usamos o mapa do
sistema como ele já é.

### Múltiplos controles

Evdev é naturalmente melhor aqui que o SDL:

| | evdev | SDL gamepad |
| --- | --- | --- |
| Vários controles | um `fd` por dispositivo | índice global |
| Controle interno + USB | ambos abertos, sem conflito | depende de mapping |
| Remover/plugar | só fechar/abrir o `fd` | precisa tratar evento |
| Identificar o aparelho | nome em `/sys/class/input/eventN/device/name` | nem sempre |

**Como vários controles são combinados:** o estado é a **soma** de todos. Se qualquer
dispositivo manda "A pressionado", a ação dispara. Botão repetido entre dois
controles não dispara duas vezes, porque a borda é calculada sobre o estado mesclado.

Efeito colateral aceito: se dois controles mandam direções opostas ao mesmo tempo, a
última lida vence. Ninguém joga assim.

### O que o leitor faz

```text
/dev/input/event0 ─┐
/dev/input/event3 ─┼─▶ threads de leitura ─▶ canal ─▶ estado mesclado ─▶ Mapper ─▶ Cmd
/dev/input/event5 ─┘
```

- Uma thread por dispositivo, leitura bloqueante. Sem `libc`, sem `fcntl`: thread
  custa menos que reescrever o leitor.
- Cada thread lê `input_event` de 24 bytes (aarch64) e manda `(type, code, value)`.
- O laço principal drena o canal a cada quadro e monta o `GamepadFrame` do núcleo.
- `buttons.map` traduz código evdev → nome lógico.

### Limite desta versão

Suporta **botões e hat digital** (`BTN_DPAD_*`, `ABS_HAT0X/Y`).

**Analógico não está suportado ainda:** normalizar `ABS_X/ABS_Y` exige `absinfo`
(ioctl `EVIOCGABS`), que precisa de `libc`. Fica como próximo passo, com o range lido
do dispositivo em vez de assumido.

---

## 2. O programa de gerenciamento

Pedido: "vamos precisar de alguma aplicação para gerenciar isso, mesmo que seja algo
adicional".

**Sim, e por um motivo específico: o problema do ovo e da galinha.**

### O problema que obriga a existir um programa separado

Se o mapeamento estiver errado — A e B trocados, por exemplo — o usuário **não
consegue navegar** até a tela de configuração para corrigir. O próprio ato de
consertar exige o controle que está quebrado.

Isso elimina a opção de colocar o gerenciador dentro de Configurações do launcher:
ele estaria atrás de uma porta que o usuário não consegue abrir.

### Desenho proposto

Um binário separado, `giochi-controllers`, iniciado pelo `giochi-init` **antes** do
launcher quando existir um sinalizador:

```text
giochi-init
   │
   ├── existe /user/.config/giochi/controller-setup ?
   │        │
   │        ├── sim ──▶ giochi-controllers ──▶ sai ──▶ launcher
   │        └── não ──▶ launcher
```

O sinalizador é criado de três formas, todas independentes do controle:

| Como ativar | Quando serve |
| --- | --- |
| Botão `home` segurado no boot | controle funciona, mas mapeamento está errado |
| Arquivo criado pelo PC (cartão no leitor) | controle não funciona de jeito nenhum |
| Primeiro boot, se nenhum evento chegou em 10 s | aparelho novo, mapa errado |

A terceira é a que resolve o caso geral: se o launcher não recebeu **nenhum** evento
em 10 s, alguma coisa está errada e ele mesmo pede socorro.

### O que o programa faz

1. **Lista dispositivos** — nome de `/sys/class/input/eventN/device/name`
2. **Mostra os códigos que chegam** — aperta um botão, vê `BTN_SOUTH` na tela
3. **Atribui nome lógico** — "esse é o A", "esse é o B"
4. **Testa os 13 nomes obrigatórios** e aponta os que faltam
5. **Salva** em `/user/.config/giochi/buttons.map`
6. **Restaura padrão** — sempre disponível

Operação: **só botões**. Não depende de analógico, não depende de mapa correto — só
de o dispositivo emitir eventos. É o mínimo que funciona quando tudo o mais falhou.

### Onde ele mora

Esta é a parte que precisa de decisão, porque afeta o modelo de três repositórios.

| Opção | A favor | Contra |
| --- | --- | --- |
| App em `Giochi app`, `distribution: image` | fica junto do launcher, que é o consumidor do mapa | mais um binário na imagem |
| Ferramenta do `Giochi System` | mexe com hardware, que é o domínio dele | duplica lógica de leitura de input |
| App pequeno, separado, no `Giochi app` | isolado; falha sem derrubar o launcher | um app a mais no catálogo |

**Recomendação: app pequeno no `Giochi app`, `distribution: image`,** porque ele
lê input exatamente como o launcher — o código de evdev é compartilhado, não
duplicado. E, se quebrar, o launcher continua funcionando.

### O que o sistema precisa entregar

| Item | Caminho | Estado |
| --- | --- | --- |
| Mapa padrão do aparelho | `/etc/giochi/buttons.map` | **não existe** |
| O launcher e o gerenciador leem esse arquivo | — | — |
| Sinalizador de setup | `/user/.config/giochi/controller-setup` | **não existe** |
| `giochi-init` que decide o que iniciar | — | **não existe** |

O mapa padrão deve ser **resolvido pelo sistema** para o aparelho (sem seções
`[default]`/`[x35s]`), porque quem sabe qual aparelho é esse é o `Giochi System`. O
launcher lê um mapa plano e não precisa conhecer modelo de hardware.

---

## 3. Em aberto

| # | Assunto |
| --- | --- |
| 1 | Onde o gerenciador mora — recomendação acima, precisa confirmação |
| 2 | Como o `home` segurado é detectado antes do mapeamento estar válido |
| 3 | Analógico: precisa de `absinfo` (ioctl) para normalizar |
| 4 | Mapa plano resolvido pelo sistema, ou seções lidas pelo launcher |
| 5 | Limite de 10 s sem evento — valor certo? |
