# Documentação — Giochi app

Índice da documentação deste repositório.

- [Salva-vidas: quando o shell morre](salva-vidas.md) — **leia primeiro se a máquina estiver sem terminal**: o que conferir sem `git`, o que `Exec format error` significa e como trabalhar só com a API do GitHub.
- [Adicionar um app](adicionar-app.md) — passo a passo a partir do `_template`.
- [Migração do antecessor](migracao.md) — o que vem do `GIOCHI-KONSOL-OS`, o que não vem e por quê.
- [Contrato de metadata](metadata.md) — os campos do `metadata.json` e por que são fechados.
- [Requisitos do app](forge.md) — o `forge.json` e como a compatibilidade é calculada.
- [Formato do pacote](pacote.md) — o `.giochi`: ordem, assinatura Ed25519 e o empacotador que existe.
- [Godot no aparelho](godot.md) — por que o Arcade Volleyball roda como C++/SDL2 e não como Godot: nenhum build oficial do motor tem KMSDRM.
- [Jogos no cartão SD](cartao-sd.md) — o X55 roda o sistema do próprio cartão, 116 GB sem partição, e o que funciona hoje.
- [Apps de PC](pc.md) — a loja e o forge nos três sistemas: um `.pyz` determinístico, matriz Windows/Linux/macOS e o que ainda não foi executado.
- [Diário de 06/11/2026](diario/2026-11-06.md) — o jogo que não existia, a listagem que não era confiável, a navegação que não seguia a tela e o app de PC que não tinha como ser distribuído.
- [Diário de 07/11/2026](diario/2026-11-07.md) — a rodada da engine no R35S: o que compilou, o que rodou e o PNG que a imagem não decodificava.
- [Loja e multi-lojas](loja.md) — o que funciona, o que não existe e as decisões que faltam antes de escrever a loja.
- [Licenciamento](licenciamento.md) — o que pode ir na imagem, o que vai pela loja e o que pode ser vendido.
- [Compilar o launcher](compilar-launcher.md) — cargo + SDL2, o prefixo local sem root e como conferir o desenho sem tela.
- [Launcher sobre a GIOCHI-ENGINE](launcher-teste.md) — o mesmo launcher desenhado na engine própria: 100% idêntico pixel a pixel em 4 painéis, 4 abas e 4 overlays, e as cinco primitivas que a engine ganhou por causa do porte.
- [Dependências que faltam no aparelho](dependencias-aparelho.md) — o que pedir ao Giochi System: SDL2_ttf, freetype e uma fonte.
- [Contrato app × sistema](contrato-app-sistema.md) — quem corrige o quê: características de aparelho no sistema, comportamento no app.
- [Ferramentas](ferramentas.md) — `aparelho.sh`, `verificar-frame.mjs` e os scripts de compilação: o que cada um resolve e por que existe.
- [Canal com o Giochi System](mensagem-giochi-system.md) — ponteiro para o arquivo de conversa (fora dos repos), a janela de contato (08:00–18:50) e os pontos abertos.
- [Checklist de imagem nova](checklist-imagem.md) — o que validar quando o Giochi System entrega uma imagem: em que ordem, e de quem é o problema quando falha.
- [Diagnóstico do protótipo do launcher](diagnostico-prototipo.md) — defeitos do HTML original e o plano em Rust.
- [Integração](integracao.md) — o que o launcher lê do sistema, splash, NPU e por que foi descartado.
- [Controles](controles.md) — entrada por evdev, múltiplos controles e o programa de gerenciamento.
- [R35S](r35s.md) — o que o app já garante nesse aparelho, o que **ainda** falta do sistema, e o estado real do build (a imagem existe desde 07/11).
- [Testar no X35H](teste-x35h.md) — roteiro escrito em 04/11 quando o aparelho ainda estava fora da rede; hoje ele já foi medido (ver §27 do `AGENTS.md`), e o roteiro continua servindo para uma imagem nova.
- [Rede: Wi-Fi e Bluetooth](rede.md) — a aba de configurações navegável, a lista de redes, o teclado de senha e o que foi medido no aparelho.

## Princípio

Um app é **uma pasta**. Se precisa de mais que isso, o desenho está errado.

Cada pasta `app/<slug>/` — ou `app/jogos/<slug>/` — é autossuficiente: contrato
(`metadata.json`), requisitos (`forge.json`), o que instala (`bin/`) e a
arte-fonte (`assets/`). Nada de configuração espalhada pela raiz do repositório.

## O que este repositório NÃO faz

- Não compila o sistema operacional — isso é do **Giochi System**.
- Não declara pacotes Buildroot — quem monta a imagem sabe o que tem nela.
- Não gera a imagem final — isso é do **Giochi os**.
