//! Carregamento e validação da configuração em RON.
//!
//! Todo número de balanceamento do jogo vive em `config/*.ron`; nenhum valor de
//! GDD é escrito no código (prompt §10). Este módulo é a única porta de entrada
//! desses arquivos, e portanto o único lugar onde eles podem estar errados.
//!
//! A validação é mecânica e roda antes da primeira linha de jogo: quatro classes,
//! cinco inimigos, seis ondas campanha, quatro bosses, seis tiers de nemesis e os
//! limites do GDD (torre, preços, perfis). Sem isso, um `classes.ron` com três
//! classes abriria o jogo e falharia depois, longe da causa — exatamente o
//! "descarte silencioso" que o AGENTS.md §12 registra como armadilha recorrente.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Erros
// ---------------------------------------------------------------------------

/// Falha ao carregar ou validar a configuração.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// O arquivo não pôde ser lido do disco.
    #[error("não consegui ler {path}: {source}")]
    Read {
        /// Caminho tentado.
        path: PathBuf,
        /// Causa do sistema de arquivos.
        #[source]
        source: std::io::Error,
    },

    /// O arquivo existe mas não é RON válido, ou não bate com a struct.
    #[error("{path} não é RON válido para este jogo: {source}")]
    Parse {
        /// Caminho tentado.
        path: PathBuf,
        /// Erro do parser, com posição.
        #[source]
        source: ron::error::SpannedError,
    },

    /// O arquivo é RON válido mas viola uma regra do GDD.
    #[error("{0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------
// Vocabulário fechado
// ---------------------------------------------------------------------------

/// As quatro classes jogáveis (GDD §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ClassId {
    /// DPS equilibrado, dash e granada.
    Soldier,
    /// Tanque corpo a corpo, escudo e provocar.
    Warrior,
    /// Dano alto à distância, mira e tiro perfurante.
    Sniper,
    /// Suporte, cura em área e reanimar.
    Medic,
}

impl ClassId {
    /// Todas as classes, na ordem do GDD.
    pub const ALL: [ClassId; 4] = [
        ClassId::Soldier,
        ClassId::Warrior,
        ClassId::Sniper,
        ClassId::Medic,
    ];

    /// Nome curto para log e HUD.
    pub fn as_str(self) -> &'static str {
        match self {
            ClassId::Soldier => "Soldado",
            ClassId::Warrior => "Guerreiro",
            ClassId::Sniper => "Sniper",
            ClassId::Medic => "Médico",
        }
    }

    /// Converte um `--classe` de linha de comando.
    pub fn from_arg(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "soldado" | "soldier" => Some(ClassId::Soldier),
            "guerreiro" | "warrior" => Some(ClassId::Warrior),
            "sniper" => Some(ClassId::Sniper),
            "medico" | "médico" | "medic" => Some(ClassId::Medic),
            _ => None,
        }
    }
}

/// Identificador de habilidade (GDD §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum AbilityId {
    /// Soldado: avanço rápido.
    Dash,
    /// Soldado: dano em área.
    Grenade,
    /// Guerreiro: reduz 50% do dano por 3 s.
    Shield,
    /// Guerreiro: puxa o aggro num raio.
    Taunt,
    /// Sniper: zoom e precisão.
    Aim,
    /// Sniper: atravessa alvos enfileirados.
    PiercingShot,
    /// Médico: cura em área.
    HealArea,
    /// Médico: reanima aliado caído.
    Revive,
}

/// Tipo de inimigo comum (GDD §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum EnemyKind {
    /// Caminha reto para a torre.
    Grunt,
    /// Lento, muito HP.
    Tank,
    /// Para a distância e atira.
    Ranged,
    /// Rápido, frágil, em grupo.
    Swarm,
    /// Ignora obstáculos.
    Flying,
}

impl EnemyKind {
    /// Os cinco tipos comuns, na ordem do GDD.
    pub const ALL: [EnemyKind; 5] = [
        EnemyKind::Grunt,
        EnemyKind::Tank,
        EnemyKind::Ranged,
        EnemyKind::Swarm,
        EnemyKind::Flying,
    ];

    /// Nome curto para log.
    pub fn as_str(self) -> &'static str {
        match self {
            EnemyKind::Grunt => "Grunt",
            EnemyKind::Tank => "Tank",
            EnemyKind::Ranged => "Ranged",
            EnemyKind::Swarm => "Swarm",
            EnemyKind::Flying => "Flying",
        }
    }
}

/// Como o inimigo se move (GDD §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum EnemyBehavior {
    /// Reta até a torre.
    Straight,
    /// Reta, mas devagar e com muito HP.
    SlowHeavy,
    /// Para na distância de ataque.
    StopAndShoot,
    /// Em grupo, com coesão.
    SwarmGroup,
    /// Voa em linha reta ignorando o chão.
    Flying,
}

/// Os quatro bosses (GDD §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum BossKind {
    /// Onda 3 — stomp AoE, imune a knockback.
    Tanker,
    /// Onda 4 — projétil em arco, teleporta.
    Plasma,
    /// Onda 5 — invoca adds e bufa aliados.
    Brain,
    /// Onda 6 — final, três fases.
    Queen,
}

impl BossKind {
    /// Os quatro bosses, na ordem do GDD.
    pub const ALL: [BossKind; 4] = [
        BossKind::Tanker,
        BossKind::Plasma,
        BossKind::Brain,
        BossKind::Queen,
    ];

    /// Nome exibido no indicador de boss.
    pub fn as_str(self) -> &'static str {
        match self {
            BossKind::Tanker => "Tanker Bug",
            BossKind::Plasma => "Plasma Bug",
            BossKind::Brain => "Brain Bug",
            BossKind::Queen => "Queen",
        }
    }
}

/// Item da loja (GDD §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ItemId {
    /// +20% de dano.
    RifleUpgrade,
    /// +30% de alcance.
    Scope,
    /// +30 HP máximo.
    Vest,
    /// +50 HP imediatos.
    HealingPotion,
    /// +1 carga de dash.
    ExtraDash,
    /// +500 HP na torre.
    TowerRepair,
    /// +500 HP máximo da torre.
    TowerHpUpgrade,
    /// +2 de armadura da torre.
    TowerArmorUpgrade,
    /// +2 HP/s de regeneração da torre.
    TowerRegen,
    /// Item exclusivo do nemesis, não vendido na loja.
    ColonelTrophy,
}

impl ItemId {
    /// Os nove itens da loja, na ordem do GDD §12 (sem o exclusivo).
    pub const SHOP: [ItemId; 9] = [
        ItemId::RifleUpgrade,
        ItemId::Scope,
        ItemId::Vest,
        ItemId::HealingPotion,
        ItemId::ExtraDash,
        ItemId::TowerRepair,
        ItemId::TowerHpUpgrade,
        ItemId::TowerArmorUpgrade,
        ItemId::TowerRegen,
    ];

    /// Nome curto para log.
    pub fn as_str(self) -> &'static str {
        match self {
            ItemId::RifleUpgrade => "rifle",
            ItemId::Scope => "scope",
            ItemId::Vest => "colete",
            ItemId::HealingPotion => "poção",
            ItemId::ExtraDash => "dash extra",
            ItemId::TowerRepair => "reparo",
            ItemId::TowerHpUpgrade => "hp da torre",
            ItemId::TowerArmorUpgrade => "armadura da torre",
            ItemId::TowerRegen => "regen da torre",
            ItemId::ColonelTrophy => "troféu do Coronel",
        }
    }
}

/// O que a compra faz quando o host a aceita (GDD §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ItemEffect {
    /// Percentual somado ao dano do jogador.
    PlayerDamagePct,
    /// Percentual somado ao alcance do jogador.
    PlayerRangePct,
    /// HP máximo somado ao jogador.
    PlayerMaxHpFlat,
    /// Cura imediata do jogador.
    PlayerHealFlat,
    /// Carga extra de dash.
    PlayerDashCharge,
    /// Cura imediata da torre.
    TowerHealFlat,
    /// HP máximo somado à torre.
    TowerMaxHpFlat,
    /// Armadura somada à torre.
    TowerArmorFlat,
    /// Regeneração somada à torre, por segundo.
    TowerRegenPerSecond,
}

/// Perfil de host (GDD §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ProfileId {
    /// RK3566/RK3588/PC ARM64.
    Full,
    /// Hardware intermediário.
    Balanced,
    /// RK3326 com 1 GB — host de emergência.
    Rk3326,
}

impl ProfileId {
    /// Os três perfis, do mais forte ao mais fraco.
    pub const ALL: [ProfileId; 3] = [ProfileId::Full, ProfileId::Balanced, ProfileId::Rk3326];

    /// Nome curto para log e menu.
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileId::Full => "Full",
            ProfileId::Balanced => "Balanced",
            ProfileId::Rk3326 => "Rk3326",
        }
    }

    /// Converte um `--perfil` de linha de comando.
    pub fn from_arg(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "full" => Some(ProfileId::Full),
            "balanced" => Some(ProfileId::Balanced),
            "rk3326" | "rk-3326" => Some(ProfileId::Rk3326),
            _ => None,
        }
    }
}

/// Papel padrão da instância (GDD §16: RK3326 entra como cliente).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum HostRole {
    /// Simula o mundo.
    Host,
    /// Envia input e desenha snapshots.
    Client,
}

/// Estratégia de pathfinding do host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum PathfindingKind {
    /// A* em grid, recalculado.
    AStarGrid,
    /// A* em grid com cache de caminho.
    AStarCached,
    /// Steering behavior, sem grid.
    Steering,
}

/// Quanto de efeito visual o perfil permite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum VfxLevel {
    /// Tudo.
    Full,
    /// Menos partículas.
    Reduced,
    /// Mínimo (RK3326).
    Minimal,
}

/// Modo de jogo (GDD §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum WaveMode {
    /// Seis ondas e vitória.
    Campaign,
    /// Ondas infinitas após a campanha.
    Endless,
}

// ---------------------------------------------------------------------------
// classes.ron — GDD §7
// ---------------------------------------------------------------------------

/// Uma classe jogável.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassDef {
    /// Identidade.
    pub id: ClassId,
    /// Nome exibido.
    pub label: String,
    /// HP máximo base.
    pub hp: f32,
    /// Dano por acerto.
    pub damage: f32,
    /// Alcance do ataque, em metros.
    pub attack_range_m: f32,
    /// Intervalo entre ataques, em segundos.
    pub attack_cooldown_s: f32,
    /// Velocidade de caminhada, em m/s.
    pub move_speed_mps: f32,
    /// Tempo de respawn na torre, em segundos (GDD: 10 s).
    pub respawn_s: f32,
    /// Habilidade 1.
    pub ability_1: AbilityDef,
    /// Habilidade 2.
    pub ability_2: AbilityDef,
}

/// Uma habilidade de classe.
#[derive(Debug, Clone, Deserialize)]
pub struct AbilityDef {
    /// Identidade.
    pub id: AbilityId,
    /// Nome exibido.
    pub label: String,
    /// Recarga, em segundos.
    pub cooldown_s: f32,
    /// Raio de efeito, em metros (0 = alvo único).
    pub radius_m: f32,
    /// Magnitude: dano, cura, metros de dash ou percentual, conforme a habilidade.
    pub magnitude: f32,
    /// Duração do efeito, em segundos (0 = instantâneo).
    pub duration_s: f32,
}

/// `config/classes.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassesConfig {
    /// As quatro classes.
    pub classes: Vec<ClassDef>,
}

// ---------------------------------------------------------------------------
// enemies.ron — GDD §8
// ---------------------------------------------------------------------------

/// Um tipo de inimigo comum.
#[derive(Debug, Clone, Deserialize)]
pub struct EnemyDef {
    /// Identidade.
    pub id: EnemyKind,
    /// Nome exibido.
    pub label: String,
    /// HP base (antes da escala por onda).
    pub hp: f32,
    /// Dano base (antes da escala por onda).
    pub damage: f32,
    /// Gold entregue ao morrer.
    pub gold: u32,
    /// Velocidade base, em m/s.
    pub speed_mps: f32,
    /// Como ele se move.
    pub behavior: EnemyBehavior,
    /// Ignora obstáculos.
    pub flying: bool,
    /// Distância a que ataca, em metros.
    pub attack_range_m: f32,
}

/// Coeficientes de escala por onda (GDD §8).
#[derive(Debug, Clone, Deserialize)]
pub struct ScalingDef {
    /// HP ×(1 + hp_per_wave·N).
    pub hp_per_wave: f32,
    /// Dano ×(1 + damage_per_wave·N).
    pub damage_per_wave: f32,
    /// Velocidade ×(1 + speed_per_wave·N).
    pub speed_per_wave: f32,
}

/// Teto de inimigos vivos por perfil (GDD §16).
#[derive(Debug, Clone, Deserialize)]
pub struct AliveCapDef {
    /// Perfil `Full`.
    pub full: u32,
    /// Perfil `Balanced`.
    pub balanced: u32,
    /// Perfil `Rk3326`.
    pub rk3326: u32,
}

/// `config/enemies.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct EnemiesConfig {
    /// Os cinco tipos comuns.
    pub enemies: Vec<EnemyDef>,
    /// Escala por onda.
    pub scaling: ScalingDef,
    /// Teto de vivos por perfil.
    pub alive_cap: AliveCapDef,
}

// ---------------------------------------------------------------------------
// waves_campaign.ron / waves_endless.ron — GDD §5
// ---------------------------------------------------------------------------

/// Um grupo de spawn dentro de uma onda.
#[derive(Debug, Clone, Deserialize)]
pub struct SpawnGroupDef {
    /// Tipo de inimigo.
    pub kind: EnemyKind,
    /// Quantos spawnam na onda.
    pub count: u32,
    /// Intervalo entre spawns do grupo, em segundos.
    pub interval_s: f32,
}

/// Uma onda da campanha.
#[derive(Debug, Clone, Deserialize)]
pub struct WaveDef {
    /// Número da onda, 1-based.
    pub number: u32,
    /// Pico de inimigos vivos (GDD §5).
    pub peak_alive: u32,
    /// Boss da onda, se houver.
    #[serde(default)]
    pub boss: Option<BossKind>,
    /// Composição.
    pub spawns: Vec<SpawnGroupDef>,
}

/// `config/waves_campaign.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct WavesConfig {
    /// Modo declarado no arquivo.
    pub mode: WaveMode,
    /// Quantas ondas a campanha tem (GDD: 6).
    pub wave_count: u32,
    /// Duração do combate de uma onda, em segundos (GDD: 6 min).
    pub combat_s: f32,
    /// Duração da loja entre ondas, em segundos (GDD: 1 min).
    pub shop_s: f32,
    /// Preparação antes da onda 1, em segundos.
    pub warmup_s: f32,
    /// As ondas.
    pub waves: Vec<WaveDef>,
}

/// `config/waves_endless.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct EndlessConfig {
    /// Primeira onda do Endless (GDD: 7).
    pub first_wave: u32,
    /// A partir de qual onda todo ciclo tem boss (GDD: 7).
    pub boss_from_wave: u32,
    /// Sequência de bosses, repetida.
    pub boss_cycle: Vec<BossKind>,
    /// Pico vivo da primeira onda Endless.
    pub peak_alive_base: u32,
    /// Quanto o pico cresce por onda.
    pub peak_alive_growth: u32,
    /// Quanto a contagem de spawns cresce por onda (fração).
    pub spawn_count_growth: f32,
    /// Escala extra por onda, somada à do GDD §8.
    pub extra_scaling_per_wave: f32,
}

// ---------------------------------------------------------------------------
// bosses.ron — GDD §9
// ---------------------------------------------------------------------------

/// Uma fase de boss.
#[derive(Debug, Clone, Deserialize)]
pub struct BossPhaseDef {
    /// Limite superior de HP, em porcentagem (100 no topo da fase 1).
    pub from_hp_pct: f32,
    /// Limite inferior de HP, em porcentagem.
    pub to_hp_pct: f32,
    /// Multiplicador de velocidade nesta fase.
    pub speed_multiplier: f32,
    /// Dois slams seguidos (Queen fase 3).
    pub slam_double: bool,
    /// Quantos projéteis no leque (0 = nenhum).
    pub projectile_fan: u32,
    /// Período de invocação nesta fase, em segundos (0 = não invoca).
    pub summon_period_s: f32,
    /// Quantos filhotes por invocação.
    pub summon_count: u32,
}

/// Um boss.
#[derive(Debug, Clone, Deserialize)]
pub struct BossDef {
    /// Identidade.
    pub id: BossKind,
    /// Onda em que aparece.
    pub wave: u32,
    /// HP (GDD §9).
    pub hp: f32,
    /// Dano de corpo a corpo / slam.
    pub damage_melee: f32,
    /// Dano de projétil (0 = não tem).
    pub damage_projectile: f32,
    /// Velocidade, em m/s.
    pub speed_mps: f32,
    /// Raio do stomp/slam, em metros.
    #[serde(default)]
    pub stomp_radius_m: Option<f32>,
    /// Período do stomp/slam, em segundos.
    #[serde(default)]
    pub stomp_period_s: Option<f32>,
    /// Distância do teleporte (Plasma).
    #[serde(default)]
    pub teleport_distance_m: Option<f32>,
    /// Distância de jogador que dispara o teleporte.
    #[serde(default)]
    pub teleport_trigger_m: Option<f32>,
    /// Adds por invocação (Brain).
    #[serde(default)]
    pub summon_count: Option<u32>,
    /// Período de invocação (Brain).
    #[serde(default)]
    pub summon_period_s: Option<f32>,
    /// Bônus de dano dado a aliados próximos (Brain), em porcentagem.
    #[serde(default)]
    pub buff_damage_pct: Option<f32>,
    /// Raio do buff (Brain), em metros.
    #[serde(default)]
    pub buff_radius_m: Option<f32>,
    /// Distância que tenta manter do jogador (Plasma), em metros.
    #[serde(default)]
    pub keeps_distance_m: Option<f32>,
    /// Imune a knockback (Tanker).
    pub knockback_immune: bool,
    /// Gold entregue.
    pub gold_reward: u32,
    /// Score entregue.
    pub score_reward: u32,
    /// Fases, se o boss tiver mais de uma.
    #[serde(default)]
    pub phases: Vec<BossPhaseDef>,
}

/// `config/bosses.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct BossesConfig {
    /// Os quatro bosses.
    pub bosses: Vec<BossDef>,
}

// ---------------------------------------------------------------------------
// nemesis.ron — GDD §10
// ---------------------------------------------------------------------------

/// Um tier do nemesis.
#[derive(Debug, Clone, Deserialize)]
pub struct NemesisTierDef {
    /// Tier, 0 a 5.
    pub tier: u32,
    /// HP neste tier.
    pub hp: f32,
    /// Dano neste tier.
    pub damage: f32,
    /// Ganhos acumulados, legíveis para log.
    pub gains: Vec<String>,
    /// Redução de dano recebido, em porcentagem.
    pub damage_reduction_pct: f32,
    /// Tem dash.
    pub dash: bool,
    /// Quantos adds invoca.
    pub summons: u32,
    /// Período de invocação, em segundos.
    pub summons_period_s: f32,
    /// Ataca à distância.
    pub ranged_attack: bool,
}

/// `config/nemesis.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct NemesisConfig {
    /// Onda em que spawna (GDD: 1).
    pub spawn_wave: u32,
    /// HP no tier 0 (GDD: 300).
    pub base_hp: f32,
    /// Dano no tier 0 (GDD: 20).
    pub base_damage: f32,
    /// Atraso do último avistamento no minimapa, em segundos (GDD: ~5).
    pub minimap_delay_s: f32,
    /// Score por tier ao matar (GDD: tier × 200).
    pub reward_score_per_tier: u32,
    /// Dano na torre se alcançá-la (penalidade por ignorar).
    pub tower_damage_massive: f32,
    /// Item exclusivo entregue ao matar.
    pub exclusive_item: ItemId,
    /// Os seis tiers.
    pub tiers: Vec<NemesisTierDef>,
}

// ---------------------------------------------------------------------------
// items.ron — GDD §12
// ---------------------------------------------------------------------------

/// Um item da loja.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    /// Identidade.
    pub id: ItemId,
    /// Nome exibido.
    pub label: String,
    /// Preço em gold (0 = não vendido).
    pub price: u32,
    /// O que a compra aplica.
    pub effect: ItemEffect,
    /// Magnitude do efeito.
    pub magnitude: f32,
    /// Quantas vezes pode ser comprado (0 = sem limite).
    pub stack_limit: u32,
    /// Exclusivo do nemesis, fora da loja.
    pub exclusive: bool,
}

/// `config/items.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemsConfig {
    /// Os itens, incluindo o exclusivo.
    pub items: Vec<ItemDef>,
}

// ---------------------------------------------------------------------------
// host_profiles.ron — GDD §16
// ---------------------------------------------------------------------------

/// Um perfil de host.
#[derive(Debug, Clone, Deserialize)]
pub struct HostProfileDef {
    /// Identidade.
    pub id: ProfileId,
    /// Nome exibido no menu.
    pub label: String,
    /// Teto de inimigos vivos.
    pub max_enemies: u32,
    /// Frequência da IA, em Hz (desacoplada do render).
    pub ai_hz: u32,
    /// Estratégia de pathfinding.
    pub pathfinding: PathfindingKind,
    /// Multiplicador do raio de AOI (menor = mais agressivo/filtrado).
    pub aoi_scale: f32,
    /// Nível de efeitos.
    pub vfx_level: VfxLevel,
}

/// `config/host_profiles.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct HostProfilesConfig {
    /// Perfil escolhido quando ninguém passa `--perfil`.
    pub default: ProfileId,
    /// Papel padrão desta instância.
    pub default_role: HostRole,
    /// Os três perfis.
    pub profiles: Vec<HostProfileDef>,
}

// ---------------------------------------------------------------------------
// tower.ron / score.ron — GDD §11 e §13
// ---------------------------------------------------------------------------

/// `config/tower.ron` (GDD §11).
#[derive(Debug, Clone, Deserialize)]
pub struct TowerConfig {
    /// HP inicial.
    pub hp_base: f32,
    /// HP máximo alcançável pela loja.
    pub hp_max_shop: f32,
    /// Armadura inicial.
    pub armor_base: f32,
    /// Armadura máxima alcançável pela loja.
    pub armor_max_shop: f32,
    /// Regeneração inicial, por segundo.
    pub regen_base: f32,
    /// Regeneração máxima alcançável pela loja, por segundo.
    pub regen_max_shop: f32,
}

/// `config/score.ron` (GDD §13).
#[derive(Debug, Clone, Deserialize)]
pub struct ScoreConfig {
    /// Pontos por kill comum.
    pub kill_common: u32,
    /// Pontos por kill de boss.
    pub kill_boss: u32,
    /// Pontos por tier ao matar o nemesis.
    pub kill_nemesis_per_tier: u32,
    /// Pontos por onda limpa, multiplicado pelo número da onda.
    pub wave_clear_per_wave: u32,
    /// Quantos HP da torre valem 1 ponto no fim da run.
    pub tower_hp_per_point: u32,
}

// ---------------------------------------------------------------------------
// Configuração completa
// ---------------------------------------------------------------------------

/// Escala aplicada a um inimigo numa onda (GDD §8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    /// Multiplicador de HP.
    pub hp: f32,
    /// Multiplicador de dano.
    pub damage: f32,
    /// Multiplicador de velocidade.
    pub speed: f32,
}

/// Toda a configuração do jogo, já validada.
#[derive(Debug, Clone)]
pub struct GameConfig {
    /// Classes jogáveis.
    pub classes: ClassesConfig,
    /// Inimigos comuns.
    pub enemies: EnemiesConfig,
    /// Ondas da campanha.
    pub campaign: WavesConfig,
    /// Configuração do Endless.
    pub endless: EndlessConfig,
    /// Itens da loja.
    pub items: ItemsConfig,
    /// Bosses.
    pub bosses: BossesConfig,
    /// Nemesis.
    pub nemesis: NemesisConfig,
    /// Perfis de host.
    pub profiles: HostProfilesConfig,
    /// Torre.
    pub tower: TowerConfig,
    /// Pontuação.
    pub score: ScoreConfig,
}

impl GameConfig {
    /// Carrega todos os arquivos de `dir` e valida o conjunto.
    ///
    /// A ordem de leitura é fixa para que a primeira mensagem de erro seja sempre
    /// a mesma diante do mesmo diretório quebrado.
    pub fn load(dir: &Path) -> Result<Self, ConfigError> {
        let config = Self {
            classes: load_ron(&dir.join("classes.ron"))?,
            enemies: load_ron(&dir.join("enemies.ron"))?,
            campaign: load_ron(&dir.join("waves_campaign.ron"))?,
            endless: load_ron(&dir.join("waves_endless.ron"))?,
            items: load_ron(&dir.join("items.ron"))?,
            bosses: load_ron(&dir.join("bosses.ron"))?,
            nemesis: load_ron(&dir.join("nemesis.ron"))?,
            profiles: load_ron(&dir.join("host_profiles.ron"))?,
            tower: load_ron(&dir.join("tower.ron"))?,
            score: load_ron(&dir.join("score.ron"))?,
        };
        config.validate()?;
        Ok(config)
    }

    /// Confere as regras quantitativas do GDD. Falha com a primeira violação.
    /// HP **efetivo** do tier 5 do Coronel: o HP com a redução de dano aplicada.
    ///
    /// É esta a grandeza de "mais forte que a Queen" (GDD §10). O documento dá
    /// tier 5 = 5000 HP e Queen = 6000 HP, o que parece contradizer a frase; mas
    /// o tier 5 carrega 30% de redução de dano, então são 7143 de dano bruto
    /// para derrubá-lo. Comparar HP cru reprovava o próprio conteúdo do GDD, e
    /// subir o HP para 6500 seria inventar um número que o documento não tem.
    ///
    /// Fica em `GameConfig` porque a validação e o teste do módulo precisam do
    /// mesmo número — duas contas iguais em lugares diferentes envelhecem
    /// diferente (D-0xx, `AGENTS.md` §16).
    pub fn nemesis_tier5_effective_hp(&self) -> f32 {
        match self.nemesis.tiers.get(5) {
            Some(t) => {
                let dr = (t.damage_reduction_pct / 100.0).clamp(0.0, 0.99);
                t.hp / (1.0 - dr)
            }
            None => f32::MAX,
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.validate_classes()?;
        self.validate_enemies()?;
        self.validate_waves()?;
        self.validate_bosses()?;
        self.validate_nemesis()?;
        self.validate_items()?;
        self.validate_profiles()?;
        self.validate_tower()?;

        if self.campaign.wave_count == 0 {
            return Err(invalid("waves_campaign.ron: wave_count precisa ser ≥ 1"));
        }
        if self.endless.first_wave != self.campaign.wave_count + 1 {
            return Err(invalid(format!(
                "waves_endless.ron: first_wave ({}) precisa ser wave_count + 1 ({})",
                self.endless.first_wave,
                self.campaign.wave_count + 1
            )));
        }
        if self.endless.boss_cycle.is_empty() {
            return Err(invalid(
                "waves_endless.ron: boss_cycle não pode ser vazio (GDD §4 manda boss em toda onda ≥ 7)",
            ));
        }
        if self.score.tower_hp_per_point == 0 {
            return Err(invalid(
                "score.ron: tower_hp_per_point não pode ser 0 (divisão por zero em §13)",
            ));
        }

        Ok(())
    }

    fn validate_classes(&self) -> Result<(), ConfigError> {
        if self.classes.classes.len() != ClassId::ALL.len() {
            return Err(invalid(format!(
                "classes.ron: esperava {} classes (GDD §7), achei {}",
                ClassId::ALL.len(),
                self.classes.classes.len()
            )));
        }
        for id in ClassId::ALL {
            let def = self.class(id).ok_or_else(|| {
                invalid(format!("classes.ron: falta a classe {}", id.as_str()))
            })?;
            if def.hp <= 0.0 {
                return Err(invalid(format!("classes.ron: {} com hp ≤ 0", id.as_str())));
            }
            if def.damage < 0.0 || def.attack_cooldown_s <= 0.0 || def.move_speed_mps <= 0.0 {
                return Err(invalid(format!(
                    "classes.ron: {} com dano/recarga/velocidade inválidos",
                    id.as_str()
                )));
            }
            if def.respawn_s < 0.0 {
                return Err(invalid(format!(
                    "classes.ron: {} com respawn_s negativo",
                    id.as_str()
                )));
            }
        }
        Ok(())
    }

    fn validate_enemies(&self) -> Result<(), ConfigError> {
        if self.enemies.enemies.len() != EnemyKind::ALL.len() {
            return Err(invalid(format!(
                "enemies.ron: esperava {} tipos (GDD §8), achei {}",
                EnemyKind::ALL.len(),
                self.enemies.enemies.len()
            )));
        }
        for kind in EnemyKind::ALL {
            let def = self.enemy(kind).ok_or_else(|| {
                invalid(format!("enemies.ron: falta o tipo {}", kind.as_str()))
            })?;
            if def.hp <= 0.0 || def.gold == 0 {
                return Err(invalid(format!(
                    "enemies.ron: {} precisa de hp > 0 e gold > 0",
                    kind.as_str()
                )));
            }
            if def.speed_mps <= 0.0 {
                return Err(invalid(format!(
                    "enemies.ron: {} com speed_mps ≤ 0",
                    kind.as_str()
                )));
            }
        }
        let s = &self.enemies.scaling;
        if s.hp_per_wave < 0.0 || s.damage_per_wave < 0.0 || s.speed_per_wave < 0.0 {
            return Err(invalid(
                "enemies.ron: os coeficientes de escala não podem ser negativos",
            ));
        }
        Ok(())
    }

    fn validate_waves(&self) -> Result<(), ConfigError> {
        let c = &self.campaign;
        if c.mode != WaveMode::Campaign {
            return Err(invalid(
                "waves_campaign.ron: mode precisa ser Campaign; Endless mora em waves_endless.ron",
            ));
        }
        if c.waves.len() as u32 != c.wave_count {
            return Err(invalid(format!(
                "waves_campaign.ron: wave_count = {} mas há {} ondas declaradas",
                c.wave_count,
                c.waves.len()
            )));
        }
        if c.combat_s <= 0.0 || c.shop_s <= 0.0 || c.warmup_s < 0.0 {
            return Err(invalid(
                "waves_campaign.ron: combat_s e shop_s precisam ser > 0 e warmup_s ≥ 0",
            ));
        }
        for (index, wave) in c.waves.iter().enumerate() {
            let expected = index as u32 + 1;
            if wave.number != expected {
                return Err(invalid(format!(
                    "waves_campaign.ron: a {}ª onda declara number = {} (precisa ser {expected}, e em ordem)",
                    expected, wave.number
                )));
            }
            if wave.peak_alive == 0 {
                return Err(invalid(format!(
                    "waves_campaign.ron: onda {} com peak_alive = 0",
                    wave.number
                )));
            }
            if wave.spawns.is_empty() {
                return Err(invalid(format!(
                    "waves_campaign.ron: onda {} sem spawns",
                    wave.number
                )));
            }
            for group in &wave.spawns {
                if group.count == 0 || group.interval_s <= 0.0 {
                    return Err(invalid(format!(
                        "waves_campaign.ron: onda {} tem grupo {} com count 0 ou interval_s ≤ 0",
                        wave.number,
                        group.kind.as_str()
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_bosses(&self) -> Result<(), ConfigError> {
        if self.bosses.bosses.len() != BossKind::ALL.len() {
            return Err(invalid(format!(
                "bosses.ron: esperava {} bosses (GDD §9), achei {}",
                BossKind::ALL.len(),
                self.bosses.bosses.len()
            )));
        }
        for kind in BossKind::ALL {
            let def = self.boss(kind).ok_or_else(|| {
                invalid(format!("bosses.ron: falta o boss {}", kind.as_str()))
            })?;
            if def.hp <= 0.0 || def.speed_mps <= 0.0 {
                return Err(invalid(format!(
                    "bosses.ron: {} precisa de hp > 0 e speed_mps > 0",
                    kind.as_str()
                )));
            }
            if def.wave == 0 || def.wave > self.campaign.wave_count {
                return Err(invalid(format!(
                    "bosses.ron: {} declara onda {}, fora de 1..={}",
                    kind.as_str(),
                    def.wave,
                    self.campaign.wave_count
                )));
            }
            if def.gold_reward == 0 || def.score_reward == 0 {
                return Err(invalid(format!(
                    "bosses.ron: {} sem recompensa (GDD §9 exige gold e score)",
                    kind.as_str()
                )));
            }
        }

        // O GDD §5 amarra cada boss a uma onda; conferir aqui evita a surpresa de
        // uma Queen aparecendo na onda 3 por um erro de digitação no RON.
        let expected = [
            (BossKind::Tanker, 3u32),
            (BossKind::Plasma, 4),
            (BossKind::Brain, 5),
            (BossKind::Queen, 6),
        ];
        for (kind, wave) in expected {
            let def = self.boss(kind).expect("bosses validados acima");
            if def.wave != wave && self.campaign.wave_count >= wave {
                return Err(invalid(format!(
                    "bosses.ron: {} é da onda {wave} (GDD §9), arquivo diz {}",
                    kind.as_str(),
                    def.wave
                )));
            }
        }
        Ok(())
    }

    fn validate_nemesis(&self) -> Result<(), ConfigError> {
        let n = &self.nemesis;
        if n.tiers.len() != 6 {
            return Err(invalid(format!(
                "nemesis.ron: esperava 6 tiers (0..=5, GDD §10), achei {}",
                n.tiers.len()
            )));
        }
        for (index, tier) in n.tiers.iter().enumerate() {
            if tier.tier != index as u32 {
                return Err(invalid(format!(
                    "nemesis.ron: tier fora de ordem em {:?} (esperava {})",
                    tier.tier, index
                )));
            }
            if tier.hp <= 0.0 || tier.damage <= 0.0 {
                return Err(invalid(format!(
                    "nemesis.ron: tier {} precisa de hp > 0 e dano > 0",
                    tier.tier
                )));
            }
            if !(0.0..100.0).contains(&tier.damage_reduction_pct) {
                return Err(invalid(format!(
                    "nemesis.ron: tier {} com damage_reduction_pct fora de [0,100)",
                    tier.tier
                )));
            }
        }
        // ## "Mais forte que a Queen" é HP **efetivo**, não HP cru
        //
        // O GDD dá dois números explícitos e uma frase que parece contradizê-los:
        // Queen HP 6000 (§9.4) e Coronel tier 5 HP 5000 (§10), "mais forte que a
        // Queen — mas matável". Comparando HP cru, 5000 < 6000 e o próprio
        // conteúdo do GDD reprova na validação.
        //
        // Não é contradição: o tier 5 carrega 30% de redução de dano, que a Queen
        // não tem. Contra 5000 HP com 30% de DR é preciso causar 7143 de dano
        // bruto — mais que os 6000 da Queen. A regra estava medindo a grandeza
        // errada; corrigi-la preserva **todos** os números do GDD, enquanto subir
        // o HP do tier para 6500 seria inventar um número que o documento não
        // tem.
        let tier5 = &n.tiers[5];
        let efetivo_tier5 = self.nemesis_tier5_effective_hp();
        let queen_hp = self.boss(BossKind::Queen).map(|b| b.hp).unwrap_or(f32::MAX);
        if efetivo_tier5 <= queen_hp {
            return Err(invalid(format!(
                "nemesis.ron: o tier 5 precisa ser mais forte que a Queen (GDD §10): \
                 HP efetivo {:.0} ({} de HP com {:.0}% de redução) contra {} da Queen",
                efetivo_tier5, tier5.hp, tier5.damage_reduction_pct, queen_hp
            )));
        }
        if n.minimap_delay_s <= 0.0 {
            return Err(invalid(
                "nemesis.ron: minimap_delay_s precisa ser > 0 (posição defasada, GDD §10)",
            ));
        }
        if n.reward_score_per_tier == 0 {
            return Err(invalid("nemesis.ron: reward_score_per_tier não pode ser 0"));
        }
        let exclusive = self.item(n.exclusive_item).ok_or_else(|| {
            invalid("nemesis.ron: exclusive_item aponta para um item que não existe")
        })?;
        if !exclusive.exclusive {
            return Err(invalid(format!(
                "nemesis.ron: o item exclusivo '{}' está marcado exclusive: false e apareceria na loja",
                exclusive.id.as_str()
            )));
        }
        Ok(())
    }

    fn validate_items(&self) -> Result<(), ConfigError> {
        for id in ItemId::SHOP {
            let def = self.item(id).ok_or_else(|| {
                invalid(format!("items.ron: falta o item {}", id.as_str()))
            })?;
            if def.price == 0 {
                return Err(invalid(format!(
                    "items.ron: {} é de loja e precisa de preço > 0",
                    id.as_str()
                )));
            }
            if def.exclusive {
                return Err(invalid(format!(
                    "items.ron: {} está marcado exclusive, então não pode ser item de loja",
                    id.as_str()
                )));
            }
            if def.magnitude == 0.0 {
                return Err(invalid(format!(
                    "items.ron: {} com magnitude 0 não muda nada",
                    id.as_str()
                )));
            }
        }
        Ok(())
    }

    fn validate_profiles(&self) -> Result<(), ConfigError> {
        if self.profiles.profiles.len() != ProfileId::ALL.len() {
            return Err(invalid(format!(
                "host_profiles.ron: esperava {} perfis (GDD §16), achei {}",
                ProfileId::ALL.len(),
                self.profiles.profiles.len()
            )));
        }
        for id in ProfileId::ALL {
            let def = self.profile(id).ok_or_else(|| {
                invalid(format!("host_profiles.ron: falta o perfil {}", id.as_str()))
            })?;
            if def.max_enemies == 0 {
                return Err(invalid(format!(
                    "host_profiles.ron: {} com max_enemies = 0",
                    id.as_str()
                )));
            }
            if def.ai_hz == 0 || def.ai_hz > 60 {
                return Err(invalid(format!(
                    "host_profiles.ron: {} com ai_hz fora de 1..=60",
                    id.as_str()
                )));
            }
            if def.aoi_scale <= 0.0 {
                return Err(invalid(format!(
                    "host_profiles.ron: {} com aoi_scale ≤ 0",
                    id.as_str()
                )));
            }
        }
        if self.profile(self.profiles.default).is_none() {
            return Err(invalid(
                "host_profiles.ron: default aponta para um perfil que não existe",
            ));
        }
        Ok(())
    }

    fn validate_tower(&self) -> Result<(), ConfigError> {
        let t = &self.tower;
        if t.hp_base <= 0.0 {
            return Err(invalid("tower.ron: hp_base precisa ser > 0 (GDD §11)"));
        }
        if t.hp_max_shop < t.hp_base {
            return Err(invalid(
                "tower.ron: hp_max_shop menor que hp_base deixaria a loja sem efeito",
            ));
        }
        if t.armor_max_shop < t.armor_base || t.regen_max_shop < t.regen_base {
            return Err(invalid(
                "tower.ron: um teto de loja (armadura/regen) é menor que o valor base",
            ));
        }
        Ok(())
    }

    /// Definição de uma classe.
    pub fn class(&self, id: ClassId) -> Option<&ClassDef> {
        self.classes.classes.iter().find(|c| c.id == id)
    }

    /// Definição de um inimigo.
    pub fn enemy(&self, kind: EnemyKind) -> Option<&EnemyDef> {
        self.enemies.enemies.iter().find(|e| e.id == kind)
    }

    /// Definição de um boss.
    pub fn boss(&self, kind: BossKind) -> Option<&BossDef> {
        self.bosses.bosses.iter().find(|b| b.id == kind)
    }

    /// Definição de um item.
    pub fn item(&self, id: ItemId) -> Option<&ItemDef> {
        self.items.items.iter().find(|i| i.id == id)
    }

    /// Definição de um perfil de host.
    pub fn profile(&self, id: ProfileId) -> Option<&HostProfileDef> {
        self.profiles.profiles.iter().find(|p| p.id == id)
    }

    /// Escala do GDD §8 para a onda `wave`:
    ///
    /// ```text
    /// HP ×(1 + 0.15·N) · dano ×(1 + 0.10·N) · velocidade ×(1 + 0.05·N)
    /// ```
    ///
    /// Os coeficientes vêm de `enemies.ron`; a fórmula é literal para que o teste
    /// `wave_scaling` possa conferir a conta do documento, não a minha.
    pub fn scaling_for_wave(&self, wave: u32) -> Scaling {
        let s = &self.enemies.scaling;
        let n = wave as f32;
        Scaling {
            hp: 1.0 + s.hp_per_wave * n,
            damage: 1.0 + s.damage_per_wave * n,
            speed: 1.0 + s.speed_per_wave * n,
        }
    }

    /// Escala da onda somando o excedente do Endless (GDD §4).
    pub fn scaling_for_endless_wave(&self, wave: u32) -> Scaling {
        let base = self.scaling_for_wave(wave);
        let extra_wave = wave.saturating_sub(self.campaign.wave_count);
        let extra = 1.0 + self.endless.extra_scaling_per_wave * extra_wave as f32;
        Scaling {
            hp: base.hp * extra,
            damage: base.damage * extra,
            speed: base.speed * extra,
        }
    }

    /// Onda da campanha, se `number` estiver na faixa.
    pub fn campaign_wave(&self, number: u32) -> Option<&WaveDef> {
        self.campaign.waves.iter().find(|w| w.number == number)
    }
}

fn invalid(text: impl Into<String>) -> ConfigError {
    ConfigError::Invalid(text.into())
}

fn load_ron<T>(path: &Path) -> Result<T, ConfigError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    ron::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> GameConfig {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
        GameConfig::load(&dir).expect("config/ do repositório precisa carregar e validar")
    }

    #[test]
    fn the_shipped_config_loads_and_validates() {
        let c = config();
        assert_eq!(c.classes.classes.len(), 4);
        assert_eq!(c.enemies.enemies.len(), 5);
        assert_eq!(c.campaign.waves.len(), 6);
        assert_eq!(c.bosses.bosses.len(), 4);
        assert_eq!(c.nemesis.tiers.len(), 6);
        assert_eq!(c.profiles.profiles.len(), 3);
    }

    #[test]
    fn gdd_numbers_are_the_ones_in_config() {
        let c = config();
        assert_eq!(c.tower.hp_base, 5000.0);
        assert_eq!(c.tower.hp_max_shop, 8000.0);
        assert_eq!(c.tower.armor_base, 5.0);
        assert_eq!(c.tower.armor_max_shop, 15.0);
        assert_eq!(c.tower.regen_max_shop, 10.0);
        assert_eq!(class(ClassId::Warrior, &c).hp, 180.0);
        assert_eq!(c.nemesis.tiers[0].hp, 300.0);
        assert_eq!(c.nemesis.tiers[5].hp, 5000.0);
        assert_eq!(c.item(ItemId::TowerRegen).map(|i| i.price), Some(200));
        assert_eq!(c.item(ItemId::HealingPotion).map(|i| i.price), Some(20));
    }

    fn class(id: ClassId, c: &GameConfig) -> &ClassDef {
        c.class(id).expect("classe presente")
    }

    #[test]
    fn wave_scaling_follows_the_gdd_formula() {
        let c = config();
        let s1 = c.scaling_for_wave(1);
        assert!((s1.hp - 1.15).abs() < 1e-6);
        assert!((s1.damage - 1.10).abs() < 1e-6);
        assert!((s1.speed - 1.05).abs() < 1e-6);

        let s6 = c.scaling_for_wave(6);
        assert!((s6.hp - 1.90).abs() < 1e-6);
        assert!((s6.damage - 1.60).abs() < 1e-6);
        assert!((s6.speed - 1.30).abs() < 1e-6);
    }

    #[test]
    fn endless_scaling_grows_past_wave_six() {
        let c = config();
        let s6 = c.scaling_for_wave(6);
        let s7 = c.scaling_for_endless_wave(7);
        assert!(s7.hp > s6.hp, "onda 7 do Endless tem de ser mais dura que a 6");

        // O excedente começa a contar depois da campanha: a onda 7 é a primeira
        // com extra, e o extra vale 1 + extra_scaling_per_wave × 1.
        let base7 = c.scaling_for_wave(7);
        let extra = 1.0 + c.endless.extra_scaling_per_wave;
        assert!((s7.hp - base7.hp * extra).abs() < 1e-5);
        assert!((s6.hp - c.scaling_for_wave(6).hp).abs() < 1e-6);
    }

    #[test]
    fn a_class_missing_from_the_file_is_rejected() {
        let mut c = config();
        c.classes.classes.pop();
        let err = c.validate().expect_err("três classes precisa reprovar");
        assert!(err.to_string().contains("classes.ron"), "{err}");
    }

    #[test]
    fn a_nemesis_tier5_weaker_than_the_queen_is_rejected() {
        let mut c = config();
        c.nemesis.tiers[5].hp = 100.0;
        let err = c.validate().expect_err("tier 5 fraco precisa reprovar");
        assert!(err.to_string().contains("mais forte que a Queen"), "{err}");
    }
}
