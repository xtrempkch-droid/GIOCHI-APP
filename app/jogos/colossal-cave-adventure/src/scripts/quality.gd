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
const CONFIG_PATH := "user://colossal-cave.cfg"

const PRESETS := [
	{
		"name": "baixo",
		"particles": 0,
		"glow": false,
		"grain": 0.0,
		"fog": 0.0,
		"step_time": 0.0,
		"torch_flicker": false,
	},
	{
		"name": "medio",
		"particles": 14,
		"glow": true,
		"grain": 0.008,
		"fog": 0.35,
		"step_time": 0.26,
		"torch_flicker": true,
	},
	{
		"name": "alto",
		"particles": 26,
		"glow": true,
		"grain": 0.012,
		"fog": 0.5,
		"step_time": 0.34,
		"torch_flicker": true,
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
