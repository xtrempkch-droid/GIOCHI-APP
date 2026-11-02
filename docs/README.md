# Documentação — Giochi app

Índice da documentação deste repositório.

- [Adicionar um app](adicionar-app.md) — passo a passo a partir do `_template`.
- [Migração do antecessor](migracao.md) — o que vem do `GIOCHI-KONSOL-OS`, o que não vem e por quê.
- [Contrato de metadata](metadata.md) — os campos do `metadata.json` e por que são fechados.
- [Requisitos do app](forge.md) — o `forge.json` e como a compatibilidade é calculada.
- [Formato do pacote](pacote.md) — o arquivo `.giochi` que vai para a loja.
- [Licenciamento](licenciamento.md) — o que pode ir na imagem, o que vai pela loja e o que pode ser vendido.
- [Diagnóstico do protótipo do launcher](diagnostico-prototipo.md) — defeitos do HTML original e o plano em Rust.
- [Integração](integracao.md) — o que o launcher lê do sistema, splash, NPU e por que foi descartado.
- [Controles](controles.md) — entrada por evdev, múltiplos controles e o programa de gerenciamento.

## Princípio

Um app é **uma pasta**. Se precisa de mais que isso, o desenho está errado.

Cada pasta `app/<slug>/` é autossuficiente: contrato (`metadata.json`), requisitos
(`forge.json`), o que instala (`bin/`) e a arte-fonte (`assets/`). Nada de
configuração espalhada pela raiz do repositório.

## O que este repositório NÃO faz

- Não compila o sistema operacional — isso é do **Giochi System**.
- Não declara pacotes Buildroot — quem monta a imagem sabe o que tem nela.
- Não gera a imagem final — isso é do **Giochi os**.
