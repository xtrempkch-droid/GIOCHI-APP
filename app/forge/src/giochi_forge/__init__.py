"""giochi-forge — ferramenta de PC do Giochi Konsol OS v1.2 (S13).

Detecta o perfil do console, classifica compatibilidade (secao 4.5.3) e
empacota .giochi (ADR-11) com assinatura Ed25519 + SHA-256 (ADR-12).

O subcomando `store` fecha o ciclo da Loja: le uma pasta de projetos baixados
(do GitHub, por exemplo), descobre o que cada um e e empacota o que ja da.
Roda no PC -- o rootfs do console nao tem toolchain (ver docs/LOJA.md).
"""

__version__ = "1.2.0"
