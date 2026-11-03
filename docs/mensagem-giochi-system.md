# Canal com o Giochi System — ponteiro

**Este arquivo não contém mais a conversa.** Ele aponta para onde ela está.

| | |
| --- | --- |
| Arquivo do canal | `/home/charles/Giochi/chat_de_equipe.txt` |
| Máquina | `charles-X99-V9S` |
| Formato | append-only: cada mensagem é acrescentada ao fim, com hora |

## Por que saiu daqui

O canal estava em `docs/`, dentro deste repositório. Três problemas:

1. **Os dois lados não dividem este repositório.** A equipe do System edita o
   dela; um arquivo de conversa aqui só existe se for copiado. Um canal precisa
   de um lugar que os dois alcançam, e o diretório acima dos dois repositórios é
   esse lugar.
2. **Conversa não é documentação.** O que é contrato é estável e revisado; o que
   é conversa muda a cada rodada e tem hora no cabeçalho. Misturar faz o
   `git log` deste repositório registrar discussão em vez de decisão.
3. **Um arquivo compartilhado não pode ser reescrito.** Aconteceu nesta rodada:
   um `head` + `mv` cortou na posição errada e apagou a mensagem da outra equipe.
   Canal append-only reduz o dano a "faltou uma mensagem", não "perdi o
   histórico".

## Onde está o conteúdo técnico

A conversa saiu; o que ela produziu ficou:

| Assunto | Onde |
| --- | --- |
| O que o app lê e escreve no sistema | [contrato-app-sistema.md](contrato-app-sistema.md) |
| O que falta na imagem (SDL2_ttf, harfbuzz, TTF) | [dependencias-aparelho.md](dependencias-aparelho.md) |
| O que validar quando chega imagem nova | [checklist-imagem.md](checklist-imagem.md) |
| Ferramentas de teste do aparelho | [ferramentas.md](ferramentas.md) |
| Fatos medidos no X55 | [AGENTS.md](../AGENTS.md) §3 e §12 |
| Por que copyleft não entra na imagem assinada | [licenciamento.md](licenciamento.md) |
| Por que `adc-joystick`, e o Y já vem invertido | [controles.md](controles.md) |

## Regras do canal

- **Acrescentar ao fim, nunca reescrever.** A mensagem recebida é histórico; não
  se corrige o texto do outro lado, responde-se.
- **Copiar antes de qualquer operação destrutiva** —
  `cp canal canal.$(date +%H%M).bak`.
- **Hora e autor no cabeçalho** de cada bloco, para datar diagnóstico depois.
- **Decisão fechada sai da conversa** e vira documento no repositório de quem a
  executa. O canal é o caminho, não o destino.

## Pontos abertos

Atualizar a cada rodada — se a lista crescer, o canal virou documento e perdeu a
função.

Estado em 03/11/2026, 19:55:

| # | Pergunta | Quem responde |
| --- | --- | --- |
| 1 | A imagem do X55 sai nesta rodada com o `post-build` corrigido? | System |
| 2 | Qual o tamanho da partição de dados da loja? | dono do projeto |
| 3 | ~~Godot 4 Compatibility ou Forward+?~~ | **respondido**: Compatibility, por contrato (`forge.json`) |

Do nosso lado, o próximo trabalho não é engine: é **o launcher conseguir lançar
um app e voltar**. `Cmd::OpenGame` hoje só navega para a tela de detalhe —
nenhum processo é criado.
