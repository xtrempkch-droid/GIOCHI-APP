📘 GIOCHI TOWER DEFENSE — Game Design Document (GDD)

Versão: 0.1 (pré-produção)
Plataforma: GIOCHI-OS (Linux ARM64) sobre GIOCHI-ENGINE-3D
Hardware mínimo: RK3326 + 1 GB RAM
Status: Design travado; rede P2P em issue na engine; gamepad pronto na ABI C

---

1. Sumário executivo

Tower Defense 3D co-op PvE para 2–4 jogadores. Uma torre central é o objetivo a defender. Soldados/guerreiros circulam um campo de batalha de 4×4 telas, segurando ondas de inimigos que escalam em dificuldade. Dois modos: Campanha (~42 min, 6 ondas, bosses nas ondas 3–6) e Endless (escala infinita pós-onda 6). Um inimigo especial — "O Coronel" — se fortalece a cada onda em que sobrevive e precisa ser caçado antes de virar ameaça maior que o boss final. Arquitetura de rede host-autoritativa com Interest Management (AOI) por cliente. Gamepad obrigatório; touch como 3ª opção (handhelds com touchscreen).

---

2. Pilares de design

Pilar O que significa na prática
Defesa em movimento Jogadores não ficam parados atrás da torre — precisam circular, caçar o nemesis, cobrir ângulos. Mapa grande força isso.
Visão fragmentada Cada jogador vê apenas sua AOI. Coordenação exige comunicação ("inimigos na sua ala", "o Coronel está no canto nordeste").
Escalada legível Cada onda é visivelmente mais difícil, e o jogador entende por quê (mais inimigos, novos tipos, bosses com padrões distintos).
Aposta do nemesis Ignorar "O Coronel" é uma escolha tática com custo crescente. Matá-lo recompensa.
Roda em handheld Toda decisão de design respeita o RK3326 como cliente mínimo. Nada de efeitos ou contagens que quebrem em 1 GB.

---

3. Tema e narrativa

Tema primário: Starship Troopers (bugs aracnídeos) — combate à distância, enxames, bosses insetoides.
Alternativa: Medieval (orcs, trolls, dragão) — reskin puro, mecânica idêntica.

Premissa: Uma colônia/fortaleza resiste a uma invasão aracnídea. A torre central é o núcleo de comando (ou o último refúgio civil). Os soldados são infantaria móvel equipada com rifles e habilidades. A cada ciclo de ataque, os bugs vêm em levas maiores. Um líder aracnídeo — O Coronel — observa e evolui a cada leva que sobrevive.

---

4. Loop principal

```
Preparação (1 min) → Combate (6 min) → Fim de onda → Loja (1 min) → Próxima onda
                                    └→ Derrota se Torre HP = 0
```

Duração Campanha: 6 ciclos × 7 min = 42 min (+ boss final estendido se necessário).
Endless: após onda 6, ciclos continuam até derrota; toda onda ≥ 7 tem boss escalado.

---

5. Estrutura de ondas — Campanha

Onda Duração Inimigos (pico vivo) Boss Notas
1 6 min ~30 — Tutorial implícito; nemesis spawna
2 6 min ~50 — Introduz Ranged e Flying
3 6 min ~60 Tanker Bug Nemesis ganha tier 1 se sobreviveu
4 6 min ~80 Plasma Bug Introduz Swarm coordenado
5 6 min ~100 Brain Bug Nemesis em tier alto se ignorado
6 6 min ~120 Queen (final, multi-fase) Se nemesis sobreviveu, ele é mais forte que a Queen

Intervalo entre ondas (1 min): loja aberta, torre fortificável, jogadores ressuscitam se mortos.

---

6. Mapa e câmera

Mapa: 4 telas por eixo = 16 telas totais. Torre no centro (4 telas centrais). Anel externo de spawn por todos os lados.

Câmera: top-down isométrica por jogador, independente. Zoom fixo. Cada jogador tem sua própria AOI (~1,5 × meia-diagonal da tela).

Minimapa: canto da tela, mostra torre, posição dos aliados (tempo real) e último avistamento do nemesis (defasado ~5 s). Inimigos comuns não aparecem no minimapa — só via visão direta.

---

7. Personagens jogáveis (defensores)

Classe Papel HP Dano Habilidade 1 Habilidade 2
Soldado DPS equilibrado 100 Médio Dash Granada
Guerreiro Tanque / frontline 180 Alto (melee) Escudo (bloqueia 50% por 3 s) Provocar (aggro em raio)
Sniper Dano à distância 80 Alto Mira precisa (zoom) Tiro perfurante
Médico Suporte 90 Baixo Cura em área Reanimar aliado caído

Respawn: ao morrer, o jogador volta na torre em 10 s (cooldown). Se a torre cair, game over.

---

8. Inimigos comuns

Tipo Comportamento HP Dano Gold
Grunt Caminha reto para torre 30 5 1
Tank Lento, alto HP 150 15 3
Ranged Para a distância e atira 60 8 (projétil) 2
Swarm Rápido, em grupo, frágil 15 3 1 (grupo)
Flying Ignora obstáculos, ataca de cima 40 6 2

Escala por onda: HP ×(1 + 0.15 × onda), dano ×(1 + 0.10 × onda), velocidade ×(1 + 0.05 × onda).

---

9. Bosses

9.1 Tanker Bug — Onda 3

· HP: 2500. Dano: 40 (stomp AoE).
· Padrão: caminha lento, para a cada 15 s para stomp em raio 3 m, imune a knockback.
· Recompensa: 50 gold, 500 score.

9.2 Plasma Bug — Onda 4

· HP: 2000. Dano: 30 (projétil em arco).
· Padrão: atira de longe, teleporta 5 m quando jogador se aproxima, mantém distância.
· Recompensa: 50 gold, 500 score.

9.3 Brain Bug — Onda 5

· HP: 3000. Dano: 20 (melee fraco).
· Padrão: invoca 4 adds a cada 20 s, buff +30% dano em inimigos próximos, foge do jogador.
· Recompensa: 50 gold, 500 score.

9.4 Queen — Onda 6 (BOSS FINAL)

· HP: 6000. Dano: 60 (slam) / 25 (projétil).
· Fases:
  · Fase 1 (100–60%): spawna filhotes a cada 10 s, slam AoE.
  · Fase 2 (60–30%): ganha projéteis em leque, filhotes mais rápidos.
  · Fase 3 (30–0%): enrage — velocidade +50%, slam duplo.
· Recompensa: 200 gold, 2000 score, vitória da Campanha.

---

10. Nemesis — "O Coronel"

Identidade: inimigo único, marcado no minimapa com posição defasada (~5 s). Patrulha as bordas do mapa, ataca jogadores isolados, causa dano massivo à torre se chegar.

Spawn: início da onda 1, em ponto aleatório da borda. Se morrer, não respawna — caçada encerrada na run.

Evolução (por onda sobrevivida):

Tier Sobreviveu até Ganho
0 Spawn (onda 1) Grunt reforçado: HP 300, dano 20
1 Fim onda 1 + Dash (avanço rápido)
2 Fim onda 2 + Invoca 2 adds a cada 30 s
3 Fim onda 3 + 30% redução de dano
4 Fim onda 4 + Ataque à distância
5 Fim onda 5 Fase final: HP 5000, dano 80, mais forte que a Queen — mas matável

Recompensa por matar: score = tier × 200 + item exclusivo (arma/upgrade de torre que não existe na loja normal).
Penalidade por ignorar: dano massivo na torre se alcançá-la; pressão constante sobre jogadores isolados.

Regras de rede: posição defasada em broadcast (minimapa de todos); posição real só para quem está na AOI dele.

---

11. A Torre

Atributo Base Máx. via loja
HP 5000 8000
Armadura 5 15
Regen 0 / s 10 / s

Fortificações na loja (intervalo de 1 min):

· Reparo: +500 HP por 50 gold
· Upgrade HP máx.: +500 por 150 gold
· Upgrade armadura: +2 por 100 gold
· Regen: +2/s por 200 gold (cumulativo)

---

12. Loja e economia

Fonte de gold: kills (Grunt 1, Tank 3, Ranged 2, Swarm 1, Flying 2, Boss 50, Nemesis tier × 200).

Itens:

Categoria Item Efeito Preço
Arma Rifle melhorado +20% dano 80
Arma Scope +30% alcance 60
Armadura Colete +30 HP máx. 70
Consumível Poção de cura +50 HP 20
Habilidade Dash extra +1 carga 100
Torre Reparo +500 HP 50
Torre Upgrade HP +500 máx. 150
Torre Upgrade armadura +2 100
Torre Regen +2/s 200

Validação: compras sempre validadas no host (ShopBuy só aplica com gold suficiente e item existente).

---

13. Pontuação e progressão

Evento Pontos
Kill comum 10
Kill boss 500
Kill nemesis tier × 200
Onda limpa 100 × número da onda
HP da torre preservado (fim da run) 1 ponto por 10 HP restante

Ranking final da run: soma de tudo. Modo Endless acumula até a derrota.

---

14. Controles

Ordem de prioridade: 1) Gamepad (obrigatório) → 2) Teclado/mouse (desenvolvimento) → 3) Touch (plus, handhelds com tela).

Gamepad (twin-stick):

Input Ação
Analógico esquerdo Movimento
Analógico direito Mira / direção de tiro
Botão A / ✕ Atacar / atirar
Botão B / ○ Habilidade 1 (dash / escudo)
Botão X / □ Habilidade 2 (granada / provocar)
Botão Y / △ Item rápido (poção)
D-pad ↑ Abrir loja (só no intervalo)
Start Pausar

Touch (3ª opção): joystick virtual esquerdo + botões na tela + tap-to-aim. Emulado via eventos de mouse da SDL até extensão SDL_Finger.

Hot-plug: jogador entra no meio da partida; se desconectar, pausa local e reconecta.

---

15. Multiplayer e rede

Arquitetura: Host-autoritativo. Host simula inimigos, ondas, torre, loja, nemesis. Clientes enviam input e recebem snapshots filtrados.

Descoberta: mDNS na LAN. Host cria sala, clientes listam e conectam.

Transporte: UDP síncrono (std::net::UdpSocket), serialização postcard.

Interest Management: host envia apenas inimigos dentro da AOI de cada cliente (unicast). Estado global (torre, onda, boss, nemesis defasado) em broadcast.

Jogadores: 2 a 4. Todos aliados (PvE).

Protocolo: Handshake, Input, PlayerState, EnemySnapshot (AOI), WaveState, TowerState, NemesisState, ShopOpen, ShopBuy, Ping/Pong.

Stub de rede: até a issue do giochi-net ser mergeada, o jogo roda com stub local (host+client no mesmo processo) com a mesma assinatura da ABI C. Troca futura sem retrabalho no jogo.

---

16. Perfis de host (performance)

Perfil Alvo Configuração
Full RK3566/RK3588/PC ARM64 ~150 inimigos, IA 20 Hz, A* em grid, AOI padrão
Balanced Hardware intermediário ~100 inimigos, IA 15 Hz, A* com cache, AOI padrão
Rk3326 (host de emergência) RK3326 1 GB ~40–50 inimigos, steering behavior, IA 10–15 Hz, AOI agressiva, VFX mínimos

Cliente prioritário: RK3326 roda sempre como cliente no perfil padrão (não usa perfil de host).

Benchmark obrigatório: medir no RK3326 quantos inimigos com IA + pathfinding antes de cair abaixo de 30 FPS estáveis. O número real define se o perfil Rk3326 é viável como host.

---

17. UI / HUD

· HP do jogador (barra inferior esquerda)
· Gold (canto superior esquerdo)
· HP da torre (barra superior central, sempre visível)
· Onda + timer (superior direito: "Onda 3/6 — 04:12")
· Minimapa (canto inferior direito): torre, aliados, último avistamento do nemesis
· Indicador de boss (aparece quando boss ativo): nome + HP%
· Loja (overlay no intervalo de 1 min): grid de itens, gold, botão "pronto"

---

18. Direção de arte e áudio

Arte: estilizada low-poly, paleta dessaturada com destaques em vermelho (inimigos) e azul (aliados). Silhuetas distintas por tipo. Sem sombras dinâmicas (perfil RK3326). Partículas mínimas.

Áudio: trilha ambiente tensa em loop por onda, stingers para spawn de boss e nemesis, feedback curto para kill/level-up. Voz do "comando" no intervalo entre ondas.

---

19. Requisitos técnicos

Item Requisito
Engine GIOCHI-ENGINE-3D (Rust + SDL2 + OpenGL ES 2.0)
Alvo aarch64-unknown-linux-gnu
RAM mínima 1 GB (RK3326)
Gamepad SDL_GameController via ABI C ✅
Touch 3ª opção, emulável via mouse; SDL_Finger como extensão futura
Rede crate giochi-net (issue aberta na engine)
Empacotamento /opt/giochi/apps/giochi-tower/ com manifesto de permissão de rede

---

20. Métricas de sucesso

· Cliente RK3326: 30 FPS estáveis em partida com 4 jogadores.
· Host RK3326 (benchmark): ≥ 40 inimigos com 30 FPS — se sim, perfil viável.
· Banda por cliente: ≤ 5 KB/s com AOI.
· Latência aceitável: RTT < 100 ms LAN; jogo pausa ou atrasa ondas se RTT > 200 ms.
· Campanha completa: 42 min ± 3 min de duração real.

---

21. Riscos e mitigações

Risco Mitigação
RK3326 não aguenta ser host Perfil Rk3326 + benchmark define viabilidade; padrão é cliente
Rede P2P atrasa na engine Stub local permite desenvolvimento paralelo
Nemesis ignorado trivializa ou quebra run Escala calibrada; recompensa alta o suficiente para justificar a caçada
Touch sem SDL_Finger Emulação via mouse cobre joystick virtual e botões
16 telas esvaziam o mapa Spawner distribui inimigos em todas as bordas; nemesis força exploração

---

22. Marcos de produção

Marco Entregável
M1 — Fundação Estrutura + configs de onda + tabelas de boss/nemesis + loop single-player com stub
M2 — Combate Inimigos comuns, torre, loja, classes jogáveis, pontuação
M3 — Bosses e nemesis 4 bosses com padrões + nemesis com evolução
M4 — Multiplayer giochi-net integrado, AOI, 4 jogadores LAN
M5 — Handheld Perfis de host, otimizações RK3326, touch emulado
M6 — Polimento Balanceamento, arte final, áudio, campanha Endless

---

23. Anexos

· Relatório de gamepad — ABI C já expõe giochi_gamepad_*; ação só no OS (SDL2 + /dev/input).
· Relatório de rede — issue feat(net): giochi-net com AOI.
· Nota de touch/handhelds — prioridade 3, emulação via mouse hoje.
· Configs de onda — waves_campaign.ron, waves_endless.ron.
· Schema de itens — items.ron.