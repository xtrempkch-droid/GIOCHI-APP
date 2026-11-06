class_name Quality
extends RefCounted
## Presets graficos. O preset define quanto do orcamento de VRAM/CPU e usado.
##
## Medido no host com GL Compatibility (ver "Orcamento" no README): os tres
## presets ficam abaixo de 1/4 do teto de VRAM, e o "baixo" cabe ate no RK3326
## (64 MB de VRAM). Nada aqui compra qualidade com textura grande.

enum Tier { BAIXO, MEDIO, ALTO }

const NAMES := ["Baixo", "Médio", "Alto"]
const DEFAULT_TIER := Tier.MEDIO
const CONFIG_PATH := "user://spacewar.cfg"

const PRESETS := [
	{
		"name": "baixo",
		"particles": 0,
		"trail_points": 0,
		"glow": false,
		"corona": false,
		"nebula": 0.0,
		"vignette": 0.40,
		"grain": 0.0,
		"shake": 0.5,
		"hitstop_ms": 0,
		"slowmo": false,
		"thrust_flame": false,
	},
	{
		"name": "medio",
		"particles": 18,
		"trail_points": 10,
		"glow": true,
		"corona": true,
		"nebula": 0.55,
		"vignette": 0.65,
		"grain": 0.008,
		"shake": 1.0,
		"hitstop_ms": 55,
		"slowmo": true,
		"thrust_flame": true,
	},
	{
		"name": "alto",
		"particles": 34,
		"trail_points": 20,
		"glow": true,
		"corona": true,
		"nebula": 0.75,
		"vignette": 0.72,
		"grain": 0.012,
		"shake": 1.25,
		"hitstop_ms": 70,
		"slowmo": true,
		"thrust_flame": true,
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
	config.load(CONFIG_PATH)
	config.set_value("video", "qualidade", clampi(tier, 0, NAMES.size() - 1))
	var err := config.save(CONFIG_PATH)
	if err != OK:
		push_warning("qualidade: nao foi possivel salvar %s (erro %d)" % [CONFIG_PATH, err])
