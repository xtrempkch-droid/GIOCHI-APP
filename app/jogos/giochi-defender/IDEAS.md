# IDEAS (não implementar sem aprovação)

Fora do escopo do plano de nove blocos. Registrado para não se perder — e para
não entrar no jogo sem o dono pedir (regra R2).

- [ ] Sistema de power-ups (tiro triplo, escudo, slow-motion)
- [ ] Leaderboard local (top 10 scores)
- [ ] Mais tipos de inimigos (kamikaze, atirador, boss)
- [ ] Partículas de explosão (depende de batching/instancing de sprite na engine)
- [ ] Animação skeletal
- [ ] Múltiplas armas (laser, metralhadora, railgun)
- [ ] Modo endless (waves infinitas com dificuldade crescente)
- [ ] Save/load de progresso
- [ ] Boss na wave 5

## Observações colhidas no bloco 1a

- **Céu mais denso.** 500 estrelas numa casca esférica põem ~58 no campo de
  visão de 60°. É um céu esparso; se o dono quiser densidade de "500 na tela",
  são ~4300 na casca (mesmo custo de desenho, 3 chamadas). Não foi feito porque
  o plano diz 500.
- **Iluminação por face no asteroide.** Com 80 faces e uma luz direcional, o
  facetado aparece (21 tons medidos), mas de baixo o asteroide fica numa sombra
  uniforme. Um segundo termo de luz (preenchimento) resolveria; fica para o
  polimento do bloco 1c se o dono quiser.
