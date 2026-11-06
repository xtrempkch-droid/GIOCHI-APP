class_name Quality
extends RefCounted
## Presets graficos. O preset define quanto do orcamento de VRAM/CPU e usado.
##
## Medido no host com GL Compatibility (ver README do jogo, secao "Orcamento"):
## o preset "alto" fica na casa de dezenas de MB de VRAM, e o "baixo" cabe ate
## no RK3326 (64 MB de VRAM). Nada aqui compra qualidade com textura grande.

enum Tier { BAIXO, MEDIO, ALTO }

const NAMES := ["Baixo", "Médio", "Alto"]
const DEFAULT_TIER := Tier.MEDIO
const CONFIG_PATH := "user://arcade-volleyball.cfg"

const PRESETS := [
	{
		"name": "baixo",
		"particles": 0,
		"trail_points": 0,
		"glow": false,
		"soft_shadow": false,
		"vignette": 0.30,
		"grain": 0.0,
		"ball_glow": 0.0,
		"shake": 0.5,
		"hitstop_ms": 0,
		"slowmo": false,
	},
	{
		"name": "medio",
		"particles": 18,
		"trail_points": 12,
		"glow": true,
		"soft_shadow": true,
		"vignette": 0.55,
		"grain": 0.008,
		"ball_glow": 0.35,
		"shake": 1.0,
		"hitstop_ms": 45,
		"slowmo": true,
	},
	{
		"name": "alto",
		"particles": 36,
		"trail_points": 24,
		"glow": true,
		"soft_shadow": true,
		"vignette": 0.62,
		"grain": 0.012,
		"ball_glow": 0.5,
		"shake": 1.25,
		"hitstop_ms": 60,
		"slowmo": true,
	},
]

static func preset(tier: int) -> Dictionary:
	return PRESETS[clampi(tier, 0, PRESETS.size() - 1)]

static func label(tier: int) -> String:
	return NAMES[clampi(tier, 0, NAMES.size() - 1)]

## Le a preferencia salva; sem arquivo, usa o preset padrao.
static func load_tier() -> int:
	var config := ConfigFile.new()
	if config.load(CONFIG_PATH) != OK:
		return DEFAULT_TIER
	var tier: int = int(config.get_value("video", "qualidade", DEFAULT_TIER))
	return clampi(tier, 0, NAMES.size() - 1)

static func save_tier(tier: int) -> void:
	var config := ConfigFile.new()
	# Preserva outras chaves se o arquivo ja existir.
	config.load(CONFIG_PATH)
	config.set_value("video", "qualidade", clampi(tier, 0, NAMES.size() - 1))
	var err := config.save(CONFIG_PATH)
	if err != OK:
		push_warning("qualidade: nao foi possivel salvar %s (erro %d)" % [CONFIG_PATH, err])
