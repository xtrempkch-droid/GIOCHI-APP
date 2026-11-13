//! Estado do jogo: relógio fixo, jogadores, ondas, inimigos, torre e fases.
//!
//! O M2 fecha o laço single-player: soldado anda e atira, inimigos nascem nas
//! bordas e atacam a torre, a torre cai e a partida termina em derrota. A
//! arquitetura continua host-autoritativa — este módulo **é** o host.
//!
//! # Ordem do tick (importa)
//!
//! ```text
//! pausa → jogador (mover, mirar, atacar) → spawn → IA dos inimigos
//!       → torre (regen) → mortes → respawn → fases → derrota
//! ```
//!
//! A IA roda a `ai_hz` do perfil (GDD §16), acumulando o tempo que passou: em
//! 60 Hz com `ai_hz = 20`, dois de cada três ticks não movem inimigo nenhum. É o
//! que mantém o RK3326 capaz de hospedar (maldade nenhuma: o inimigo anda
//! `1/20 s` de uma vez, não `1/60 s` três vezes).
//!
//! # Determinismo
//!
//! A posição de spawn sai de um hash do tick e do id, não de `rand`: o mesmo
//! número de ticks produz a mesma partida. Isso vale mais que variedade agora —
//! é o que permite um teste afirmar "a onda 1 gera 60 inimigos, e o 3º nasce na
//! borda leste" (ver `DECISIONS.md`, D-024). `rand` entra no M5, atrás do perfil.

use glam::Vec3;
use giochi_net::wire::SnapshotInimigo;

use crate::config::{ClassId, EnemyKind, GameConfig, ItemId, ProfileId, WaveMode};
use crate::engine_bridge::input::InputFrame;
use crate::game::boss::{Boss, BossEvent};
use crate::game::camera::{IsoCamera, MapBounds};
use crate::game::enemy::Enemy;
use crate::game::nemesis::{Nemesis, NemesisEvent};
use crate::game::player::PlayerState;
use crate::game::score::Score;
use crate::game::shop::Shop;
use crate::game::tower::Tower;
use crate::game::waves::Spawner;

/// Frequência da simulação, em Hz (a do `GameLoop` da engine).
pub const TICK_HZ: u32 = 60;

/// Passo fixo da simulação, em segundos.
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;

/// Modo da partida (GDD §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Seis ondas, bosses nas 3–6, vitória na 6 (GDD §5).
    Campaign,
    /// Ondas infinitas depois da campanha (GDD §4).
    Endless,
}

impl RunMode {
    /// Nome curto para log e HUD.
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Campaign => "Campanha",
            RunMode::Endless => "Endless",
        }
    }

    /// Converte o `mode` declarado no RON de ondas.
    pub fn from_wave_mode(mode: WaveMode) -> Self {
        match mode {
            WaveMode::Campaign => RunMode::Campaign,
            WaveMode::Endless => RunMode::Endless,
        }
    }
}

/// Fase do ciclo de uma onda (GDD §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Preparação curta antes da onda.
    Warmup,
    /// Combate de 6 min.
    Combat,
    /// Loja de 1 min e fortificação.
    Shop,
    /// Campanha vencida.
    Victory,
    /// Torre caiu.
    Defeat,
}

impl Phase {
    /// Nome curto para log e HUD.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Warmup => "preparação",
            Phase::Combat => "combate",
            Phase::Shop => "loja",
            Phase::Victory => "vitória",
            Phase::Defeat => "derrota",
        }
    }

    /// A partida está em andamento.
    pub fn is_running(self) -> bool {
        matches!(self, Phase::Warmup | Phase::Combat | Phase::Shop)
    }

    /// A partida terminou (vitória ou derrota).
    pub fn is_over(self) -> bool {
        matches!(self, Phase::Victory | Phase::Defeat)
    }
}

/// Estado completo de uma partida local (o host).
#[derive(Debug, Clone)]
pub struct GameState {
    /// Passos de simulação já executados (relógio autoritativo).
    pub tick: u64,
    /// Modo da partida.
    pub mode: RunMode,
    /// Fase atual.
    pub phase: Phase,
    /// Número da onda atual, 1-based.
    pub wave: u32,
    /// Tempo acumulado na fase atual, em segundos.
    pub phase_timer_s: f32,
    /// Jogadores da partida.
    pub players: Vec<PlayerState>,
    /// Id do jogador local.
    pub local_id: u8,
    /// Câmera isométrica do jogador local (GDD §6: uma por jogador).
    pub camera: IsoCamera,
    /// Limites do mapa.
    pub map: MapBounds,
    /// Perfil de host em uso (GDD §16).
    pub profile: ProfileId,
    /// Pausa local.
    pub paused: bool,
    /// Inimigos vivos no mundo do host.
    pub enemies: Vec<Enemy>,
    /// Próximo id de inimigo.
    pub next_enemy_id: u16,
    /// A torre.
    pub tower: Tower,
    /// O spawner da onda.
    pub spawner: Spawner,
    /// O placar.
    pub score: Score,
    /// Acumulador da IA (para rodar a `ai_hz`, não a 60 Hz).
    pub ai_accumulator_s: f32,
    /// Boss da onda, se houver (GDD §9).
    pub boss: Option<Boss>,
    /// O Coronel, enquanto estiver vivo (GDD §10).
    pub nemesis: Option<Nemesis>,
    /// A loja do intervalo (GDD §12).
    pub shop: Shop,
    /// Item selecionado na loja.
    pub shop_cursor: usize,
    /// Repetição do cursor (para não pular de item a 60 Hz).
    pub shop_cursor_timer_s: f32,
}

impl GameState {
    /// Cria a partida no estado inicial: onda 1, preparação, jogador na torre.
    pub fn new(config: &GameConfig, class: ClassId, profile: ProfileId) -> Self {
        let mut players = Vec::with_capacity(4);
        players.push(PlayerState::new(0, class, config));

        let mode = RunMode::from_wave_mode(config.campaign.mode);
        let spawner = match mode {
            RunMode::Campaign => Spawner::for_campaign_wave(1, config),
            RunMode::Endless => Spawner::for_endless_wave(config.campaign.wave_count + 1, config),
        };

        Self {
            tick: 0,
            mode,
            phase: Phase::Warmup,
            wave: 1,
            phase_timer_s: 0.0,
            players,
            local_id: 0,
            camera: IsoCamera::new(Vec3::ZERO),
            map: MapBounds::new(),
            profile,
            paused: false,
            enemies: Vec::new(),
            next_enemy_id: 1,
            tower: Tower::new(&config.tower),
            spawner,
            score: Score::new(),
            ai_accumulator_s: 0.0,
            boss: None,
            nemesis: None,
            shop: Shop::new(),
            shop_cursor: 0,
            shop_cursor_timer_s: 0.0,
        }
    }

    /// Jogador local.
    pub fn local(&self) -> &PlayerState {
        &self.players[self.local_index()]
    }

    /// Jogador local, mutável.
    pub fn local_mut(&mut self) -> &mut PlayerState {
        let index = self.local_index();
        &mut self.players[index]
    }

    fn local_index(&self) -> usize {
        self.index_of(self.local_id).unwrap_or(0)
    }

    /// Índice do jogador com esse id.
    pub fn index_of(&self, id: u8) -> Option<usize> {
        self.players.iter().position(|p| p.id == id)
    }

    /// Jogador com esse id, se estiver na partida.
    pub fn player_of(&self, id: u8) -> Option<&PlayerState> {
        self.index_of(id).map(|i| &self.players[i])
    }

    /// Jogador com esse id, mutável.
    pub fn player_mut_of(&mut self, id: u8) -> Option<&mut PlayerState> {
        self.index_of(id).map(move |i| &mut self.players[i])
    }

    /// Põe um jogador em campo (host, ao receber o primeiro input de um cliente).
    ///
    /// Nasce **ao lado da torre** e em ângulo diferente por id, para quatro
    /// jogadores não aparecerem empilhados no mesmo ponto — o que faria o
    /// primeiro quadro da partida parecer que só um entrou.
    pub fn adicionar_jogador(&mut self, id: u8, class: ClassId, config: &GameConfig) -> bool {
        if self.index_of(id).is_some() {
            return false;
        }
        let mut player = PlayerState::new(id, class, config);
        let angulo = (id as f32) * std::f32::consts::FRAC_PI_2;
        player.position = glam::Vec3::new(angulo.cos() * 6.0, 0.0, angulo.sin() * 6.0);
        player.aim = glam::Vec3::new(-player.position.x, 0.0, -player.position.z).normalize_or_zero();
        self.players.push(player);
        log::info!("partida: jogador {id} entrou como {}", class.as_str());
        true
    }

    /// Tira um jogador da partida (cliente desconectado).
    pub fn remover_jogador(&mut self, id: u8) -> bool {
        if id == self.local_id {
            return false;
        }
        let Some(indice) = self.index_of(id) else {
            return false;
        };
        self.players.remove(indice);
        log::info!("partida: jogador {id} saiu");
        true
    }

    /// Aplica o input de **outro** jogador, no host.
    ///
    /// É o mesmo caminho do jogador local ([`Self::step_player`]) parametrizado
    /// pelo id: um segundo laço de movimento para os remotos envelheceria
    /// diferente do primeiro, e a divergência apareceria como "o cliente anda
    /// mais rápido que o host".
    pub fn aplicar_input_de(
        &mut self,
        id: u8,
        dt: f32,
        input: &InputFrame,
        config: &GameConfig,
    ) {
        if self.index_of(id).is_none() || id == self.local_id {
            return;
        }
        self.step_player_of(id, dt, input, config);
    }

    /// Atualiza o resumo de um jogador vindo do host (lado cliente).
    ///
    /// O cliente prevê o **próprio** movimento; a posição dos outros vem daqui.
    /// Aplicar o resumo no local também faria a predição brigar com o servidor,
    /// e o sintoma seria o personagem tremendo parado.
    pub fn aplicar_resumo_de_jogador(
        &mut self,
        id: u8,
        pos: [f32; 2],
        hp: u16,
        max_hp: u16,
        score: u32,
        alive: bool,
    ) {
        if id == self.local_id {
            // Do local, só a vida e o placar: a posição é a predita.
            if let Some(player) = self.player_mut_of(id) {
                player.hp = f32::from(hp);
                player.max_hp = f32::from(max_hp);
                player.score = score;
                player.alive = alive;
            }
            return;
        }
        if let Some(player) = self.player_mut_of(id) {
            player.position.x = pos[0];
            player.position.z = pos[1];
            player.hp = f32::from(hp);
            player.max_hp = f32::from(max_hp);
            player.score = score;
            player.alive = alive;
        }
    }

    /// Executa um passo fixo de simulação.
    pub fn step(&mut self, dt: f32, input: &InputFrame, config: &GameConfig) {
        if input.edges.pause {
            self.paused = !self.paused;
            log::info!("pausa: {}", if self.paused { "ligada" } else { "desligada" });
        }
        if self.paused || !self.phase.is_running() {
            return;
        }

        self.tick = self.tick.saturating_add(1);
        self.phase_timer_s += dt;

        self.step_player(dt, input, config);
        // A câmera é **por jogador** (GDD §6) e segue o local. Sem esta linha
        // ela ficava parada na torre para sempre: o jogador andava 79 m e o
        // mundo continuava enquadrado no centro do mapa, e a AOI do cliente
        // (`area_from_camera`) nunca acompanhava ninguém.
        //
        // Direto, sem interpolação: num TD o enquadramento é previsível, e um
        // atraso de câmera só faz a mira "escorregar" sobre o alvo.
        if let Some(player) = self.players.iter().find(|p| p.id == self.local_id) {
            self.camera.focus = player.position;
        }
        self.camera.clamp_focus(self.map);

        self.step_spawns(dt, config);
        self.step_enemies(dt, config);
        self.step_boss(dt, config);
        self.step_nemesis(dt, config);
        self.step_shop(dt, input, config);
        self.tower.step(dt);
        self.step_deaths(config);
        self.step_respawns(dt);
        self.advance_phase(config);

        if !self.tower.is_alive() && self.phase != Phase::Defeat {
            self.defeat(config);
        }
    }

    // ----------------------------------------------------------------- jogador
    fn step_player(&mut self, dt: f32, input: &InputFrame, config: &GameConfig) {
        self.step_player_of(self.local_id, dt, input, config);
    }

    /// O mesmo passo, para o jogador `id`. O local e os remotos passam por aqui.
    fn step_player_of(&mut self, id: u8, dt: f32, input: &InputFrame, config: &GameConfig) {
        let map = self.map;
        let Some(speed) = self.player_of(id).map(|p| p.move_speed(config)) else {
            return;
        };
        let camera = self.camera;
        {
            let Some(player) = self.player_mut_of(id) else {
                return;
            };
            player.move_local(input, &camera, map, speed, dt);
            player.attack_timer_s = (player.attack_timer_s - dt).max(0.0);
        }

        // Atacar: hitscan no inimigo mais próximo dentro do alcance e do cone.
        let pode_atirar = self
            .player_of(id)
            .map(|p| input.buttons.attack && p.attack_timer_s <= 0.0)
            .unwrap_or(false);
        if !pode_atirar {
            return;
        }
        let Some((index, distance)) = self.target_in_aim_de(id, config) else {
            return;
        };
        let damage = self.player_of(id).map(|p| p.damage(config)).unwrap_or(0.0);
        let (killed, kind, gold) = {
            let enemy = &mut self.enemies[index];
            let killed = enemy.take_damage(damage);
            let kind = enemy.kind;
            let gold = config.enemy(kind).map(|d| d.gold).unwrap_or(0);
            (killed, kind, gold)
        };
        log::trace!(
            "hit: {:?} a {distance:.1} m, -{damage:.1} hp{}",
            kind,
            if killed { " (morto)" } else { "" }
        );

        let cooldown = self.player_of(id).map(|p| p.attack_cooldown(config)).unwrap_or(0.0);
        let award = if killed {
            self.enemies.swap_remove(index);
            Some(self.score.add_common_kill(&config.score))
        } else {
            None
        };
        if let Some(player) = self.player_mut_of(id) {
            player.attack_timer_s = cooldown;
            if let Some(award) = award {
                player.gold = player.gold.saturating_add(gold);
                player.score = player.score.saturating_add(award);
            }
        }
    }

    /// Inimigo mais próximo dentro do alcance e à frente da mira do jogador `id`.
    /// Inimigo mais próximo dentro do alcance e à frente da mira do local.
    #[cfg(test)]
    fn target_in_aim(&self, config: &GameConfig) -> Option<(usize, f32)> {
        self.target_in_aim_de(self.local_id, config)
    }

    fn target_in_aim_de(&self, id: u8, config: &GameConfig) -> Option<(usize, f32)> {
        let player = self.player_of(id)?;
        if !player.can_act() {
            return None;
        }
        let range = player.attack_range(config);
        let aim = player.aim;
        let mut best: Option<(usize, f32)> = None;
        for (index, enemy) in self.enemies.iter().enumerate() {
            if !enemy.is_alive() {
                continue;
            }
            let dx = enemy.position.x - player.position.x;
            let dz = enemy.position.z - player.position.z;
            let distance = (dx * dx + dz * dz).sqrt();
            if distance > range || distance <= 1.0e-4 {
                continue;
            }
            // Cone de ~70°: sem isso, o Soldado atira pelas costas e o jogo
            // perde o "twin-stick" (GDD §14) na prática.
            let dot = (dx * aim.x + dz * aim.z) / distance;
            if dot < 0.35 {
                continue;
            }
            if best.map(|(_, d)| distance < d).unwrap_or(true) {
                best = Some((index, distance));
            }
        }
        best
    }

    // ------------------------------------------------------------------- spawn
    fn step_spawns(&mut self, dt: f32, config: &GameConfig) {
        let cap = self.enemy_cap(config);
        let kinds = self.spawner.update(dt, self.enemies.len(), cap);
        for kind in kinds {
            let id = self.next_enemy_id;
            self.next_enemy_id = self.next_enemy_id.wrapping_add(1).max(1);
            let position = self.spawn_position(id);
            let Some(def) = config.enemy(kind) else {
                continue;
            };
            let scaling = match self.mode {
                RunMode::Campaign => config.scaling_for_wave(self.wave),
                RunMode::Endless => config.scaling_for_endless_wave(self.wave),
            };
            let enemy = Enemy::spawn(id, def, position, scaling);
            log::trace!("spawn: {:?} id {id} em {position:?}", kind);
            self.enemies.push(enemy);
        }
    }

    /// Teto de vivos: o menor entre o pico da onda e o limite do perfil.
    pub fn enemy_cap(&self, config: &GameConfig) -> usize {
        let profile_cap = config
            .profile(self.profile)
            .map(|p| p.max_enemies)
            .unwrap_or(100) as usize;
        (self.spawner.peak_alive as usize).min(profile_cap).max(1)
    }

    /// Posição de nascimento numa das quatro bordas (GDD §6).
    ///
    /// Hash determinístico do id: mesma partida, mesmas posições.
    fn spawn_position(&self, id: u16) -> Vec3 {
        let limit = (self.map.half_extent_m - 3.0).max(1.0);
        let mut hash = (id as u64)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.wave as u64 * 1_442_695_040_888_963_407)
            .wrapping_add(self.tick.wrapping_mul(31));
        hash ^= hash >> 33;
        let side = (hash % 4) as u8;
        let t = ((hash >> 16) % 1000) as f32 / 1000.0;
        let coord = -limit + 2.0 * limit * t;
        match side {
            0 => Vec3::new(coord, 0.0, limit),
            1 => Vec3::new(coord, 0.0, -limit),
            2 => Vec3::new(limit, 0.0, coord),
            _ => Vec3::new(-limit, 0.0, coord),
        }
    }

    // -------------------------------------------------------------------- IA
    fn step_enemies(&mut self, dt: f32, config: &GameConfig) {
        let hz = config
            .profile(self.profile)
            .map(|p| p.ai_hz)
            .unwrap_or(15)
            .max(1) as f32;
        let interval = 1.0 / hz;
        self.ai_accumulator_s += dt;
        if self.ai_accumulator_s < interval {
            return;
        }
        let ai_dt = self.ai_accumulator_s.min(0.25);
        self.ai_accumulator_s = 0.0;

        let tower = crate::game::tower::TOWER_POSITION;
        let players: Vec<(u8, Vec3)> = self
            .players
            .iter()
            .filter(|p| p.can_act())
            .map(|p| (p.id, p.position))
            .collect();

        let mut tower_damage = 0.0;
        let mut player_hits: Vec<(u8, f32)> = Vec::new();

        for enemy in self.enemies.iter_mut() {
            if !enemy.is_alive() {
                continue;
            }
            let nearest = players
                .iter()
                .min_by(|a, b| {
                    let da = enemy.distance_to(a.1);
                    let db = enemy.distance_to(b.1);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
            let target = enemy.pick_target(nearest.map(|(_, pos)| pos));
            enemy.move_towards(target, ai_dt);

            // Ataca a torre se chegou nela; senão, o jogador que estiver no alvo.
            if enemy.distance_to(tower) <= crate::game::tower::TOWER_RADIUS_M {
                if enemy.tick_attack(ai_dt, true) {
                    tower_damage += enemy.damage;
                }
            } else if let Some((id, pos)) = nearest {
                if enemy.in_range_of(pos) && enemy.tick_attack(ai_dt, true) {
                    player_hits.push((id, enemy.damage));
                }
            }
        }

        if tower_damage > 0.0 {
            let effective = self.tower.take_damage(tower_damage);
            log::debug!(
                "torre: -{effective:.0} hp (bruto {tower_damage:.0}), {:.0}/{:.0}",
                self.tower.hp,
                self.tower.max_hp
            );
        }
        for (id, damage) in player_hits {
            if let Some(player) = self.players.iter_mut().find(|p| p.id == id) {
                player.take_damage(damage);
            }
        }
    }

    fn step_deaths(&mut self, config: &GameConfig) {
        for player in self.players.iter_mut() {
            if player.alive && player.hp <= 0.0 {
                player.die(config);
                log::info!("jogador {} caiu; volta em {:.0} s na torre", player.id, player.respawn_timer_s);
            }
        }
        self.enemies.retain(|enemy| enemy.is_alive());
    }

    fn step_respawns(&mut self, dt: f32) {
        for player in self.players.iter_mut() {
            if player.tick_respawn(dt) {
                log::info!("jogador {} voltou na torre", player.id);
            }
        }
    }

    // ----------------------------------------------------------------- fases
    fn advance_phase(&mut self, config: &GameConfig) {
        match self.phase {
            Phase::Warmup => {
                if self.phase_timer_s >= config.campaign.warmup_s {
                    self.enter_combat(config);
                }
            }
            Phase::Combat => {
                let time_up = self.phase_timer_s >= config.campaign.combat_s;
                if !time_up {
                    return;
                }
                let last_wave = self.mode == RunMode::Campaign
                    && self.wave >= config.campaign.wave_count;
                if last_wave {
                    // A onda final não termina com inimigo vivo em campo: senão a
                    // "vitória" seria anunciada com a torre ainda cercada.
                    if self.enemies.is_empty() {
                        let bonus = self.score.finish(self.tower.hp, &config.score);
                        if bonus > 0 {
                            log::info!("bônus de torre preservada: +{bonus} pontos");
                        }
                        self.enter(Phase::Victory);
                    }
                    return;
                }
                // Fim de onda: o campo é limpo (a próxima onda traz a sua leva).
                if !self.enemies.is_empty() {
                    log::info!("onda {} terminou com {} inimigo(s) em campo", self.wave, self.enemies.len());
                    self.enemies.clear();
                }
                let award = self.score.add_wave_clear(self.wave, &config.score);
                let player_score = self.local().score.saturating_add(award);
                self.local_mut().score = player_score;
                // O Coronel que sobreviveu à onda sobe de tier (GDD §10).
                if let Some(nemesis) = self.nemesis.as_mut() {
                    nemesis.on_wave_survived(config);
                }
                self.enter(Phase::Shop);
            }
            Phase::Shop => {
                if self.phase_timer_s >= config.campaign.shop_s {
                    self.players_ready_for_next_wave(config);
                    self.wave = self.wave.saturating_add(1);
                    self.spawner = match self.mode {
                        RunMode::Campaign => Spawner::for_campaign_wave(self.wave, config),
                        RunMode::Endless => Spawner::for_endless_wave(self.wave, config),
                    };
                    self.ai_accumulator_s = 0.0;
                    self.shop_cursor = 0;
                    self.enter_combat(config);
                }
            }
            Phase::Victory | Phase::Defeat => {}
        }
    }

    /// No intervalo, todos revivem e a torre não muda (GDD §5: "jogadores mortos
    /// ressuscitam").
    fn players_ready_for_next_wave(&mut self, config: &GameConfig) {
        for player in self.players.iter_mut() {
            if !player.alive {
                player.alive = true;
                player.hp = player.max_hp;
                player.position = crate::game::tower::TOWER_POSITION;
                player.respawn_timer_s = 0.0;
                let _ = config;
            }
        }
    }

    fn enter(&mut self, phase: Phase) {
        if self.phase != phase {
            log::info!("onda {}: {} -> {}", self.wave, self.phase.as_str(), phase.as_str());
        }
        self.phase = phase;
        self.phase_timer_s = 0.0;
    }

    // ------------------------------------------------------------------ boss
    fn step_boss(&mut self, dt: f32, config: &GameConfig) {
        let Some(boss) = self.boss.as_mut() else {
            return;
        };
        let Some(def) = config.boss(boss.kind).cloned() else {
            self.boss = None;
            return;
        };
        let nearest = self
            .players
            .iter()
            .filter(|p| p.can_act())
            .min_by(|a, b| {
                let da = boss.position.distance(a.position);
                let db = boss.position.distance(b.position);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|p| p.position);
        let tower = crate::game::tower::TOWER_POSITION;
        let events = boss.step(dt, &def, nearest, tower, config);
        let boss_pos = boss.position;

        for event in events {
            match event {
                BossEvent::Stomp {
                    center,
                    radius_m,
                    damage,
                } => {
                    let effective = self.tower.take_damage(damage);
                    log::debug!("boss stomp: torre -{effective:.0} hp em {center:?} r={radius_m}");
                }
                BossEvent::Summon { kind, count } => {
                    self.spawn_adds(kind, count, boss_pos, config);
                }
                BossEvent::Projectile { damage, .. } => {
                    // Projétil do boss acerta a torre: é o alvo que ele guarda.
                    let effective = self.tower.take_damage(damage);
                    log::debug!("boss projétil: torre -{effective:.0} hp");
                }
                BossEvent::Teleported { to } => {
                    log::debug!("boss teleportou para {to:?}");
                }
            }
        }

        if self.boss.as_ref().map(|b| !b.is_alive()).unwrap_or(false) {
            let kind = self.boss.as_ref().map(|b| b.kind);
            self.boss = None;
            if let Some(kind) = kind {
                if let Some(def) = config.boss(kind) {
                    let player = self.local_mut();
                    player.gold = player.gold.saturating_add(def.gold_reward);
                    let award = self.score.add_boss_kill(&config.score);
                    self.local_mut().score = self.local().score.saturating_add(award);
                    log::info!("{} morto: +{} gold, +{award} pontos", kind.as_str(), def.gold_reward);
                }
            }
        }
    }

    /// Adds de um boss, ao redor dele.
    fn spawn_adds(&mut self, kind: EnemyKind, count: u32, around: Vec3, config: &GameConfig) {
        let Some(def) = config.enemy(kind) else {
            return;
        };
        let scaling = match self.mode {
            RunMode::Campaign => config.scaling_for_wave(self.wave),
            RunMode::Endless => config.scaling_for_endless_wave(self.wave),
        };
        for index in 0..count {
            let id = self.next_enemy_id;
            self.next_enemy_id = self.next_enemy_id.wrapping_add(1).max(1);
            let angle = (index as f32) * std::f32::consts::TAU / count.max(1) as f32;
            let offset = Vec3::new(angle.cos() * 4.0, 0.0, angle.sin() * 4.0);
            let enemy = Enemy::spawn(id, def, around + offset, scaling);
            self.enemies.push(enemy);
        }
    }

    // --------------------------------------------------------------- nemesis
    fn step_nemesis(&mut self, dt: f32, config: &GameConfig) {
        let Some(nemesis) = self.nemesis.as_mut() else {
            return;
        };
        let nearest = self
            .players
            .iter()
            .filter(|p| p.can_act())
            .min_by(|a, b| {
                let da = nemesis.position.distance(a.position);
                let db = nemesis.position.distance(b.position);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|p| (p.id, p.position));
        let tower = crate::game::tower::TOWER_POSITION;
        let events = nemesis.step(dt, nearest, tower, config);
        let nemesis_pos = nemesis.position;

        for event in events {
            match event {
                NemesisEvent::HitTower { damage } => {
                    let effective = self.tower.take_damage(damage);
                    log::warn!("o Coronel alcançou a torre: -{effective:.0} hp");
                }
                NemesisEvent::HitPlayer { player_id, damage } => {
                    if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                        player.take_damage(damage);
                    }
                }
                NemesisEvent::Summon { count } => {
                    self.spawn_adds(EnemyKind::Swarm, count, nemesis_pos, config);
                }
            }
        }

        if self
            .nemesis
            .as_ref()
            .map(|n| !n.is_alive())
            .unwrap_or(false)
        {
            let reward = self.nemesis.as_ref().map(|n| n.reward(config));
            self.nemesis = None;
            if let Some((points, item)) = reward {
                let player = self.local_mut();
                player.score = player.score.saturating_add(points);
                // O item exclusivo é concedido direto (não há inventário no M3):
                // o efeito dele é aplicado e o fato fica no log.
                if let Some(def) = config.item(item) {
                    match def.effect {
                        crate::config::ItemEffect::PlayerDamagePct => {
                            player.damage_bonus_pct += def.magnitude
                        }
                        crate::config::ItemEffect::PlayerRangePct => {
                            player.range_bonus_pct += def.magnitude
                        }
                        _ => {}
                    }
                    log::info!(
                        "o Coronel caiu: +{points} pontos e item exclusivo '{}'",
                        def.label
                    );
                }
                let award = self.score.add_nemesis_kill(0, &config.score);
                let _ = award;
            }
        }
    }

    // ------------------------------------------------------------------ loja
    fn step_shop(&mut self, dt: f32, input: &InputFrame, config: &GameConfig) {
        if self.phase == Phase::Shop {
            self.shop.open();
        } else {
            self.shop.close();
            return;
        }

        // Cursor: movimento vertical do analógico, com repetição lenta.
        self.shop_cursor_timer_s -= dt;
        if self.shop_cursor_timer_s <= 0.0 && !input.move_dir.is_neutral() {
            let entries = ItemId::SHOP.len();
            if entries > 0 {
                if input.move_dir.y > 0.4 {
                    self.shop_cursor = (self.shop_cursor + entries - 1) % entries;
                    self.shop_cursor_timer_s = 0.25;
                } else if input.move_dir.y < -0.4 {
                    self.shop_cursor = (self.shop_cursor + 1) % entries;
                    self.shop_cursor_timer_s = 0.25;
                }
            }
        }

        if input.edges.attack {
            let item = ItemId::SHOP[self.shop_cursor % ItemId::SHOP.len()];
            let index = self.local_index();
            let tower = &mut self.tower;
            let shop = &mut self.shop;
            let player = &mut self.players[index];
            match shop.buy(item, player, tower, config) {
                Ok(price) => {
                    log::info!("comprou {} por {price}", item.as_str());
                }
                Err(error) => log::warn!("compra recusada: {error}"),
            }
        }
    }

    /// Entra em combate e põe em campo o que a onda pede.
    fn enter_combat(&mut self, config: &GameConfig) {
        self.enter(Phase::Combat);
        // Boss da onda (GDD §5): nasce longe da torre, no anel externo.
        self.boss = None;
        if let Some(def) = config
            .bosses
            .bosses
            .iter()
            .find(|b| b.wave == self.wave)
            .cloned()
        {
            let position = Vec3::new(self.map.half_extent_m * 0.7, 0.0, self.map.half_extent_m * 0.7);
            log::info!("onda {}: {} entrou em campo", self.wave, def.id.as_str());
            self.boss = Some(Boss::spawn(&def, position));
        }
        // O Coronel nasce na onda 1 e não volta se morrer (GDD §10).
        if self.wave == config.nemesis.spawn_wave && self.nemesis.is_none() {
            let position = Vec3::new(-self.map.half_extent_m * 0.8, 0.0, self.map.half_extent_m * 0.5);
            self.nemesis = Some(Nemesis::spawn(config, position));
            log::info!("o Coronel entrou em campo (tier 0)");
        }
    }

    fn defeat(&mut self, config: &GameConfig) {
        log::warn!("a torre caiu na onda {}", self.wave);
        self.score.finish(0.0, &config.score);
        self.enter(Phase::Defeat);
    }

    // ------------------------------------------------------------- consulta
    /// Inimigos vivos.
    pub fn enemies_alive(&self) -> usize {
        self.enemies.iter().filter(|e| e.is_alive()).count()
    }

    /// Snapshot para a rede (o host o filtra por AOI antes de enviar).
    ///
    /// Boss e nemesis entram com `tipo` 9 e 10 e com `fase_chefe` preenchida: o
    /// cliente não conhece o vocabulário de boss, só desenha. É o mesmo contrato
    /// do `giochi-net` — a engine transporta, o jogo interpreta.
    pub fn snapshot_enemies(&self) -> Vec<SnapshotInimigo> {
        let mut snapshot = crate::net::aoi::from_enemies(&self.enemies);
        if let Some(boss) = self.boss.as_ref() {
            snapshot.push(SnapshotInimigo {
                id: u16::MAX - 1,
                x: boss.position.x,
                z: boss.position.z,
                hp: (boss.hp_pct() * f32::from(u16::MAX)) as u16,
                tipo: 9,
                fase_chefe: boss.phase + 1,
            });
        }
        if let Some(nemesis) = self.nemesis.as_ref() {
            snapshot.push(SnapshotInimigo {
                id: u16::MAX,
                x: nemesis.position.x,
                z: nemesis.position.z,
                hp: (nemesis.hp_pct() * f32::from(u16::MAX)) as u16,
                tipo: 10,
                fase_chefe: nemesis.tier + 1,
            });
        }
        snapshot
    }

    /// Duração total da fase atual, em segundos.
    pub fn phase_duration_s(&self, config: &GameConfig) -> f32 {
        match self.phase {
            Phase::Warmup => config.campaign.warmup_s,
            Phase::Combat => config.campaign.combat_s,
            Phase::Shop => config.campaign.shop_s,
            Phase::Victory | Phase::Defeat => 0.0,
        }
    }

    /// Segundos restantes da fase atual.
    pub fn phase_remaining_s(&self, config: &GameConfig) -> f32 {
        (self.phase_duration_s(config) - self.phase_timer_s).max(0.0)
    }

    /// Rótulo do relógio da fase, `mm:ss`.
    pub fn clock_label(&self, config: &GameConfig) -> String {
        let remaining = self.phase_remaining_s(config) as u32;
        format!("{:02}:{:02}", remaining / 60, remaining % 60)
    }

    /// Rótulo da onda: `3/6` na campanha, `7/∞` no Endless.
    pub fn wave_label(&self, config: &GameConfig) -> String {
        match self.mode {
            RunMode::Campaign => format!("{}/{}", self.wave, config.campaign.wave_count),
            RunMode::Endless => format!("{}/∞", self.wave),
        }
    }

    /// Linha do HUD superior (GDD §17).
    pub fn hud_line(&self, config: &GameConfig) -> String {
        format!(
            "Onda {} — {} · torre {:.0}/{:.0} · {} inimigo(s) · {:.0} gold",
            self.wave_label(config),
            self.clock_label(config),
            self.tower.hp,
            self.tower.max_hp,
            self.enemies_alive(),
            self.local().gold
        )
    }

    /// Resumo para o log de encerramento.
    pub fn summary(&self) -> String {
        format!(
            "{} — onda {} — {} pontos ({} comuns, {} ondas) — torre {:.0}/{:.0}",
            self.phase.as_str(),
            self.wave,
            self.score.points,
            self.score.common_kills,
            self.score.waves_cleared,
            self.tower.hp,
            self.tower.max_hp
        )
    }

    /// Tipos de inimigo vivos, para o desenho (o cliente só conhece bytes).
    pub fn enemy_kind_of_byte(byte: u8) -> EnemyKind {
        match byte {
            1 => EnemyKind::Tank,
            2 => EnemyKind::Ranged,
            3 => EnemyKind::Swarm,
            4 => EnemyKind::Flying,
            _ => EnemyKind::Grunt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_bridge::input::{Axis, Buttons};
    use std::path::Path;

    fn config() -> GameConfig {
        GameConfig::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config")).expect("config")
    }

    fn state(config: &GameConfig) -> GameState {
        GameState::new(config, ClassId::Soldier, ProfileId::Balanced)
    }

    fn combat(config: &GameConfig) -> GameState {
        let mut s = state(config);
        s.phase = Phase::Combat;
        s
    }

    /// Combate **sem inimigos**, para testes que medem a máquina de fases.
    ///
    /// Jogador parado numa onda real perde — é o desenho (`prompt.md`, M2:
    /// "derrota quando torre cai") e não um defeito. Mas dois testes daqui
    /// rodam 6 min de onda sem ninguém defender e esperavam chegar à loja: eles
    /// mediam a transição `Combat → Shop` e reprovavam por balanceamento. Com a
    /// onda esvaziada, a transição é medida sozinha, e a regra de "parado
    /// perde" fica coberta por `the_tower_falling_ends_the_match_in_defeat`.
    /// Avança até a fase virar `alvo`, com teto de passos.
    ///
    /// Contar passos a partir da duração (`combat_s / TICK_DT`) parece exato e
    /// não é: somar `1/60` 21 601 vezes em `f32` dá **359,97**, não 360, então o
    /// teste ficava 2 passos curto e reprovava uma transição que funciona. É a
    /// mesma deriva que atrasava a cadência de ataque (`game::timers`). Avançar
    /// até a condição, com limite, mede o que o teste quer dizer.
    fn avancar_ate(s: &mut GameState, config: &GameConfig, alvo: Phase, limite_s: f32) {
        let limite = (limite_s / TICK_DT) as u32;
        for _ in 0..limite {
            s.step(TICK_DT, &InputFrame::default(), config);
            if s.phase == alvo {
                return;
            }
        }
    }

    fn combat_sem_inimigos(config: &GameConfig) -> GameState {
        let mut s = combat(config);
        s.spawner.groups.clear();
        s.spawner.total = 0;
        s.spawner.exhausted = true;
        s.enemies.clear();
        // O Coronel não vem do spawner: ele nasce na onda 1 e, chegando à
        // torre, aplica `tower_damage_massive` (1500). Deixá-lo aqui faria o
        // teste de fase reprovar pelo mesmo motivo dos inimigos comuns.
        s.nemesis = None;
        s
    }

    #[test]
    fn a_new_match_starts_on_wave_one_with_the_tower_at_full_hp() {
        let c = config();
        let s = state(&c);
        assert_eq!(s.wave, 1);
        assert_eq!(s.phase, Phase::Warmup);
        assert_eq!(s.tower.hp, 5000.0);
        assert_eq!(s.enemies_alive(), 0);
        assert_eq!(s.spawner.total, 60, "onda 1 vem do RON");
    }

    #[test]
    fn enemies_spawn_during_combat_and_are_drawn_from_the_borders() {
        let c = config();
        let mut s = combat(&c);
        for _ in 0..120 {
            s.step(TICK_DT, &InputFrame::default(), &c);
        }
        assert!(!s.enemies.is_empty(), "2 s de combate já geram inimigos");

        // A borda é propriedade do **nascimento**, não do estado 2 s depois: em
        // 2 s um Grunt a 2,2 m/s já andou 4,4 m para dentro, e a versão anterior
        // deste teste reprovava justamente por medir a posição depois do
        // movimento. `spawn_position` é o lugar certo para afirmar a regra.
        let limit = (s.map.half_extent_m - 3.0).max(1.0);
        for id in 0..200u16 {
            let p = s.spawn_position(id);
            let na_borda = (p.x.abs() - limit).abs() < 1.0e-3 || (p.z.abs() - limit).abs() < 1.0e-3;
            assert!(na_borda, "id {id}: nasceu fora da borda em {p:?} (limite {limit})");
        }
    }

    #[test]
    fn the_alive_cap_is_the_smaller_of_wave_peak_and_profile() {
        let c = config();
        let mut s = combat(&c);
        s.profile = ProfileId::Rk3326;
        let cap = s.enemy_cap(&c);
        assert_eq!(cap, 30, "onda 1 pede 30; o perfil aguenta 45");
        assert!(cap <= 45);
    }

    #[test]
    fn shooting_kills_and_pays_gold_and_points() {
        let c = config();
        let mut s = combat(&c);
        let def = c.enemy(EnemyKind::Grunt).expect("grunt");
        // Um Grunt bem na frente da mira, a 2 m.
        let mut enemy = Enemy::spawn(99, def, Vec3::new(2.0, 0.0, 0.0), c.scaling_for_wave(1));
        enemy.position = Vec3::new(2.0, 0.0, 0.0);
        s.enemies.push(enemy);
        s.local_mut().aim = Vec3::new(1.0, 0.0, 0.0);

        let input = InputFrame {
            buttons: Buttons {
                attack: true,
                ..Buttons::default()
            },
            ..InputFrame::default()
        };
        // Dano 12 contra 34,5 de HP: três tiros com recarga de 0,28 s.
        for _ in 0..120 {
            s.step(TICK_DT, &input, &c);
            // O alvo do teste é o Grunt de id 99. Esperar o campo vazio é
            // errado: a onda 1 continua spawnando enquanto o teste roda, e
            // "campo vazio" nunca acontece num combate real.
            if !s.enemies.iter().any(|e| e.id == 99) {
                break;
            }
        }
        assert!(
            !s.enemies.iter().any(|e| e.id == 99),
            "o Grunt do teste deveria ter morrido"
        );
        assert_eq!(s.local().gold, 1, "gold do Grunt no GDD §12");
        assert_eq!(s.score.common_kills, 1);
        assert_eq!(s.local().score, 10, "kill comum vale 10 (GDD §13)");
    }

    #[test]
    fn an_enemy_that_reaches_the_tower_damages_it() {
        let c = config();
        let mut s = combat(&c);
        s.spawner = Spawner::for_campaign_wave(1, &c);
        let def = c.enemy(EnemyKind::Tank).expect("tank");
        // Tank já encostado na torre (raio 8 m).
        let mut enemy = Enemy::spawn(1, def, Vec3::new(7.0, 0.0, 0.0), c.scaling_for_wave(1));
        enemy.position = Vec3::new(7.0, 0.0, 0.0);
        s.enemies.push(enemy);
        let before = s.tower.hp;
        for _ in 0..120 {
            s.step(TICK_DT, &InputFrame::default(), &c);
        }
        assert!(s.tower.hp < before, "a torre levou dano");
    }

    #[test]
    fn the_tower_falling_ends_the_match_in_defeat() {
        let c = config();
        let mut s = combat(&c);
        s.tower.hp = 1.0;
        let def = c.enemy(EnemyKind::Tank).expect("tank");
        let mut enemy = Enemy::spawn(1, def, Vec3::new(7.0, 0.0, 0.0), c.scaling_for_wave(6));
        enemy.position = Vec3::new(7.0, 0.0, 0.0);
        s.enemies.push(enemy);
        for _ in 0..300 {
            s.step(TICK_DT, &InputFrame::default(), &c);
            if s.phase == Phase::Defeat {
                break;
            }
        }
        assert_eq!(s.phase, Phase::Defeat);
        assert_eq!(s.tower.hp, 0.0);
        assert!(s.score.is_finished());
    }

    #[test]
    fn um_jogador_remoto_entra_ao_lado_da_torre_e_nao_sobre_o_outro() {
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        assert!(s.adicionar_jogador(1, ClassId::Sniper, &c));
        assert!(s.adicionar_jogador(2, ClassId::Medic, &c));
        // Entrar duas vezes com o mesmo id não duplica: o host chama isto a cada
        // input até o cliente ser conhecido, e um `push` cego criaria um jogador
        // novo por pacote.
        assert!(!s.adicionar_jogador(1, ClassId::Sniper, &c));
        assert_eq!(s.players.len(), 3);

        let p1 = s.player_of(1).expect("jogador 1").position;
        let p2 = s.player_of(2).expect("jogador 2").position;
        let d1 = (p1.x * p1.x + p1.z * p1.z).sqrt();
        let d2 = (p2.x * p2.x + p2.z * p2.z).sqrt();
        assert!(d1 < 15.0 && d2 < 15.0, "nasceram longe: {p1:?} {p2:?}");
        assert!((p1 - p2).length() > 1.0, "nasceram empilhados: {p1:?} {p2:?}");
    }

    #[test]
    fn o_input_remoto_move_o_jogador_dele_e_nao_o_local() {
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        s.phase = Phase::Combat;
        s.adicionar_jogador(1, ClassId::Warrior, &c);
        let local_antes = s.local().position;
        let remoto_antes = s.player_of(1).expect("jogador 1").position;

        let frame = InputFrame {
            move_dir: Axis::new(0.0, 1.0),
            ..InputFrame::default()
        };
        s.aplicar_input_de(1, 1.0 / 60.0, &frame, &c);

        let local_depois = s.local().position;
        let remoto_depois = s.player_of(1).expect("jogador 1").position;
        assert_eq!(local_antes, local_depois, "o input do cliente mexeu no host");
        assert_ne!(remoto_antes, remoto_depois, "o input do cliente não mexeu nele");
    }

    #[test]
    fn o_host_ignora_input_do_proprio_jogador_local() {
        // O host também manda input (é jogador), mas a simulação dele vem do laço
        // local. Aplicar duas vezes dobraria a velocidade — o defeito clássico de
        // host autoritativo mal costurado.
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        s.phase = Phase::Combat;
        let antes = s.local().position;
        let frame = InputFrame {
            move_dir: Axis::new(0.0, 1.0),
            ..InputFrame::default()
        };
        s.aplicar_input_de(s.local_id, 1.0 / 60.0, &frame, &c);
        assert_eq!(s.local().position, antes);
    }

    #[test]
    fn o_resumo_do_host_nao_sobrescreve_a_posicao_predita_do_local() {
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        s.local_mut().position = glam::Vec3::new(3.0, 0.0, -4.0);
        // O host manda a posição "atrasada" do nosso jogador; aplicá-la faria o
        // personagem voltar e tremer.
        s.aplicar_resumo_de_jogador(s.local_id, [0.0, 0.0], 80, 100, 42, true);
        assert_eq!(s.local().position, glam::Vec3::new(3.0, 0.0, -4.0));
        assert_eq!(s.local().hp, 80.0, "a vida vem do host");
        assert_eq!(s.local().score, 42, "o placar vem do host");
    }

    #[test]
    fn o_resumo_do_host_move_os_outros_jogadores() {
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        s.adicionar_jogador(2, ClassId::Sniper, &c);
        s.aplicar_resumo_de_jogador(2, [12.0, -7.0], 55, 90, 9, true);
        let outro = s.player_of(2).expect("jogador 2");
        assert_eq!(outro.position.x, 12.0);
        assert_eq!(outro.position.z, -7.0);
        assert_eq!(outro.hp, 55.0);
        assert_eq!(outro.score, 9);
    }

    #[test]
    fn um_jogador_desconectado_sai_da_partida_mas_o_local_nao() {
        let c = config();
        let mut s = GameState::new(&c, ClassId::Soldier, ProfileId::Balanced);
        s.adicionar_jogador(1, ClassId::Medic, &c);
        assert!(s.remover_jogador(1));
        assert!(s.player_of(1).is_none());
        assert_eq!(s.players.len(), 1);
        // O local nunca sai por rede: sem ele não há quem desenhe a câmera.
        assert!(!s.remover_jogador(s.local_id));
    }

    #[test]
    fn the_first_wave_ends_in_the_shop_and_advances_afterwards() {
        let c = config();
        let mut s = combat_sem_inimigos(&c);
        avancar_ate(&mut s, &c, Phase::Shop, c.campaign.combat_s + 5.0);
        assert_eq!(s.phase, Phase::Shop, "onda 1 acaba em loja");
        assert_eq!(s.wave, 1);
        assert!(s.enemies.is_empty(), "o campo é limpo no intervalo");

        avancar_ate(&mut s, &c, Phase::Combat, c.campaign.shop_s + 5.0);
        assert_eq!(s.wave, 2);
        assert_eq!(s.phase, Phase::Combat);
    }

    #[test]
    fn the_dead_come_back_when_the_wave_interval_opens() {
        let c = config();
        let mut s = combat_sem_inimigos(&c);
        s.local_mut().die(&c);
        assert!(!s.local().can_act());
        avancar_ate(&mut s, &c, Phase::Shop, c.campaign.combat_s + 5.0);
        assert_eq!(s.phase, Phase::Shop);
        assert!(s.local().can_act(), "GDD §5: mortos ressuscitam no intervalo");
        assert_eq!(s.local().hp, s.local().max_hp);
    }

    #[test]
    fn the_last_campaign_wave_only_ends_with_a_clear_field() {
        let c = config();
        let mut s = combat(&c);
        s.wave = c.campaign.wave_count;
        s.phase_timer_s = c.campaign.combat_s + 1.0;
        s.enemies.push({
            let def = c.enemy(EnemyKind::Grunt).expect("grunt");
            Enemy::spawn(1, def, Vec3::new(20.0, 0.0, 20.0), c.scaling_for_wave(6))
        });
        s.step(TICK_DT, &InputFrame::default(), &c);
        assert_eq!(s.phase, Phase::Combat, "ainda há inimigo em campo");

        s.enemies.clear();
        s.step(TICK_DT, &InputFrame::default(), &c);
        assert_eq!(s.phase, Phase::Victory);
    }

    #[test]
    fn the_hud_line_counts_the_clients_enemies_and_gold() {
        let c = config();
        let mut s = combat(&c);
        s.phase_timer_s = 122.0;
        s.wave = 3;
        s.local_mut().gold = 45;
        assert_eq!(s.hud_line(&c), "Onda 3/6 — 03:58 · torre 5000/5000 · 0 inimigo(s) · 45 gold");
    }

    #[test]
    fn the_snapshot_carries_every_live_enemy() {
        let c = config();
        let mut s = combat(&c);
        let def = c.enemy(EnemyKind::Swarm).expect("swarm");
        s.enemies.push(Enemy::spawn(1, def, Vec3::new(10.0, 0.0, 10.0), c.scaling_for_wave(1)));
        s.enemies.push(Enemy::spawn(2, def, Vec3::new(-10.0, 0.0, -10.0), c.scaling_for_wave(1)));
        assert_eq!(s.snapshot_enemies().len(), 2);
    }

    #[test]
    fn the_ai_runs_at_the_profile_rate_not_every_frame() {
        let c = config();
        let mut s = combat(&c);
        s.profile = ProfileId::Rk3326; // 12 Hz
        let def = c.enemy(EnemyKind::Grunt).expect("grunt");
        let mut enemy = Enemy::spawn(1, def, Vec3::new(0.0, 0.0, 60.0), c.scaling_for_wave(1));
        enemy.position = Vec3::new(0.0, 0.0, 60.0);
        s.enemies.push(enemy);

        // Um tick a 60 Hz: a IA (12 Hz) não deve ter rodado.
        s.step(TICK_DT, &InputFrame::default(), &c);
        let moved = s.enemies[0].distance_to_tower();
        assert!((moved - 60.0).abs() < 1.0e-3, "andou fora do ciclo de IA: {moved}");

        // 1/12 s depois, sim.
        for _ in 0..5 {
            s.step(TICK_DT, &InputFrame::default(), &c);
        }
        assert!(s.enemies[0].distance_to_tower() < 60.0, "a IA rodou no ciclo");
    }

    #[test]
    fn a_paused_match_does_not_simulate() {
        let c = config();
        let mut s = combat(&c);
        let mut pause = InputFrame::default();
        pause.edges.pause = true;
        s.step(TICK_DT, &pause, &c);
        assert!(s.paused);
        let tick = s.tick;
        for _ in 0..60 {
            s.step(TICK_DT, &InputFrame::default(), &c);
        }
        assert_eq!(s.tick, tick);
        assert_eq!(s.phase_timer_s, 0.0);
    }

    #[test]
    fn the_aim_cone_keeps_the_shooter_from_firing_backwards() {
        let c = config();
        let mut s = combat(&c);
        let def = c.enemy(EnemyKind::Grunt).expect("grunt");
        s.enemies.push(Enemy::spawn(1, def, Vec3::new(-3.0, 0.0, 0.0), c.scaling_for_wave(1)));
        s.local_mut().aim = Vec3::new(1.0, 0.0, 0.0); // mirando para +X, inimigo em -X
        assert!(s.target_in_aim(&c).is_none());
        s.local_mut().aim = Vec3::new(-1.0, 0.0, 0.0);
        assert!(s.target_in_aim(&c).is_some());
    }

    #[test]
    fn the_endless_mode_never_wins_by_wave_number() {
        let c = config();
        let mut s = state(&c);
        s.mode = RunMode::Endless;
        s.wave = 6;
        s.phase = Phase::Combat;
        s.phase_timer_s = c.campaign.combat_s + 1.0;
        s.step(TICK_DT, &InputFrame::default(), &c);
        assert_eq!(s.phase, Phase::Shop);
        assert_eq!(s.wave_label(&c), "6/∞");
    }

    #[test]
    fn movement_is_relative_to_the_camera_and_stays_inside_the_map() {
        let c = config();
        let mut s = state(&c);
        s.local_mut().position = Vec3::new(79.0, 0.0, 0.0);
        let input = InputFrame {
            move_dir: Axis::new(1.0, 0.0),
            ..InputFrame::default()
        };
        for _ in 0..900 {
            s.step(TICK_DT, &input, &c);
        }
        assert!(s.map.contains(s.local().position));
        assert!((s.camera.focus - s.local().position).length() < 1.0e-4);
    }
}
