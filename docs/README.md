# Documentação — Giochi app

Índice da documentação deste repositório.

- [Salva-vidas: quando o shell morre](salva-vidas.md) — **leia primeiro se a máquina estiver sem terminal**: o que conferir sem `git`, o que `Exec format error` significa e como trabalhar só com a API do GitHub.
- [Adicionar um app](adicionar-app.md) — passo a passo a partir do `_template`.
- [Migração do antecessor](migracao.md) — o que vem do `GIOCHI-KONSOL-OS`, o que não vem e por quê.
- [Contrato de metadata](metadata.md) — os campos do `metadata.json` e por que são fechados.
- [Requisitos do app](forge.md) — o `forge.json` e como a compatibilidade é calculada.
- [Formato do pacote](pacote.md) — o `.giochi`: ordem, assinatura Ed25519 e o empacotador que existe.
- [Godot no aparelho](godot.md) — por que nenhum build oficial do motor tem KMSDRM, e o que isso faz com a classificação de um app `runtime: godot`.
- [Apps no cartão SD](cartao-sd.md) — o X55 roda o sistema do próprio cartão, 116 GB sem partição, e o que funciona hoje.
- [Apps de PC](pc.md) — a loja e o forge nos três sistemas: um `.pyz` determinístico, matriz Windows/Linux/macOS e o que ainda não foi executado.
- [Loja e multi-lojas](loja.md) — o que funciona, o que não existe e as decisões que faltam antes de escrever a loja.
- [Licenciamento](licenciamento.md) — o que pode ir na imagem, o que vai pela loja e o que pode ser vendido.
- [Idiomas do launcher](idiomas.md) — os dez idiomas do site, a tela de boas-vindas da primeira abertura, a troca em Config › Aparência e a cadeia de fontes por escrita; o que **não** está pronto (shaping árabe) e por quê.
- [Compilar o launcher](compilar-launcher.md) — cargo + SDL2, o prefixo local sem root e como conferir o desenho sem tela.
- [Launcher sobre a GIOCHI-ENGINE](launcher-teste.md) — o mesmo launcher desenhado na engine própria: 100% idêntico pixel a pixel em 4 painéis, 4 abas e 4 overlays, e as cinco primitivas que a engine ganhou por causa do porte.
- [Música](musica.md) — o primeiro app de imagem que não é tema: carrossel de capas geradas do nome, transporte completo pelo mixer da engine, 120 testes sem SDL e **100,00%** de identidade de quadro entre PC e R35S.
- [Star Dust](star-dust.md) — o launcher com a camada de efeitos: oito efeitos, 29 chamadas de desenho para a camada inteira, e desligado ele é **idêntico byte a byte** ao launcher de produção.
- [Dependências que faltam no aparelho](dependencias-aparelho.md) — o que pedir ao Giochi System: SDL2_ttf, freetype e uma fonte.
- [Contrato app × sistema](contrato-app-sistema.md) — quem corrige o quê: características de aparelho no sistema, comportamento no app.
- [Ferramentas](ferramentas.md) — `aparelho.sh`, `verificar-frame.mjs` e os scripts de compilação: o que cada um resolve e por que existe.
- [Canal com o Giochi System](mensagem-giochi-system.md) — ponteiro para o arquivo de conversa (fora dos repos), a janela de contato (08:00–18:50) e os pontos abertos.
- [Issues para os outros repositórios](issues-para-outros-repos.md) — os quatro corpos prontos (3 para o System, 1 para o OS). Repo alheio é somente leitura: aqui só se lê, o pedido de correção viaja como issue.
- [Checklist de imagem nova](checklist-imagem.md) — o que validar quando o Giochi System entrega uma imagem: em que ordem, e de quem é o problema quando falha.
- [Diagnóstico do protótipo do launcher](diagnostico-prototipo.md) — defeitos do HTML original e o plano em Rust.
- [Integração](integracao.md) — o que o launcher lê do sistema, splash, NPU e por que foi descartado.
- [Controles](controles.md) — entrada por evdev, múltiplos controles e o programa de gerenciamento.
- [Controle virtual "Giochi Gamepad"](controle-virtual.md) — a mudança de 18/11/2026 no Giochi System: 17 botões e 4 eixos num nó só, e o que falta implementar aqui (documentado, **não** implementado).
- [PortMaster: a loja e os ports](portmaster.md) — créditos (é obra da comunidade PortsMaster; aqui só o invólucro `giochi-ports`), a GUI com cartão próprio na aba Apps, `/user/ports` virando cartão, o layout que o detector exige e o que foi medido no R35S.
- [Histórico medido](historico/) — o texto completo que saiu do `AGENTS.md` em 26/11/2026, **verbatim**: launcher, apps e catálogo, PC e pacote, verificação e ambientes, armadilhas, pendências.
- [R35S](r35s.md) — o que o app já garante nesse aparelho, o que **ainda** falta do sistema, e o estado real do build (a imagem existe desde 07/11).
- [Testar no X35H](teste-x35h.md) — roteiro para uma imagem nova: o que conferir, em que ordem, e de quem é o problema quando falha.
- [Rede: Wi-Fi e Bluetooth](rede.md) — a aba de configurações navegável, a lista de redes, o teclado de senha e o que foi medido no aparelho.

## Princípio

Um app é **uma pasta**. Se precisa de mais que isso, o desenho está errado.

Cada pasta `app/<slug>/` é autossuficiente: contrato
(`metadata.json`), requisitos (`forge.json`), o que instala (`bin/`) e a
arte-fonte (`assets/`). Nada de configuração espalhada pela raiz do repositório.

## O que este repositório NÃO faz

- Não compila o sistema operacional — isso é do **Giochi System**.
- Não declara pacotes Buildroot — quem monta a imagem sabe o que tem nela.
- Não gera a imagem final — isso é do **Giochi os**.
