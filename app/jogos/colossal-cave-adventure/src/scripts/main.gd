extends Node2D
## Colossal Cave -- camada de apresentacao 2,5D.
##
## A simulacao vive em scripts/sim/cave.gd (por turnos, deterministica). Aqui
## so ha entrada, transicao de passo, luz do lampiao e o modo benchmark.
##
## O "2,5D": a sala e um corredor em perspectiva de um ponto de fuga, com
## passagens em poligono nas paredes e objetos em billboard escalados pela
## profundidade. Nao ha malha 3D -- o custo e um shader e poucos poligonos,
## que e o que cabe no orcamento de VRAM do aparelho.

const SIM := preload("res://scripts/sim/cave.gd")
const AutoplayScript := preload("res://scripts/sim/autoplay.gd")
const LocaleScript := preload("res://scripts/i18n/locale.gd")
const CavernShader := preload("res://shaders/cavern.gdshader")

## Um turno a cada N quadros no autoplay do benchmark (~0,5 s a 60 Hz).
const BENCH_FRAMES_PER_TURN := 30

@onready var background: ColorRect = $Background/Cavern
@onready var view: CaveView = $Arena/CaveView
@onready var dust: CPUParticles2D = $Arena/Dust
@onready var hud: GameHud = $HUD
@onready var camera: Camera2D = $Camera

var cave: Cave
var locale: Locale
var _autoplay
var _bench: Bench
var _tier := Quality.DEFAULT_TIER
var _preset: Dictionary = {}
var _sfx: SfxKit
var _material: ShaderMaterial

var _reveal := 1.0
var _reveal_speed := 0.0
var _light := 0.0
var _target_light := 0.0
var _flicker := 0.0
var _paused := false
var _restart_at := 0
var _bench_frame := 0
var _last_room := ""
## Linha ativa do menu de pausa: 0 = graficos, 1 = idioma.
var _menu_row := 0

func _ready() -> void:
	_bench = Bench.from_cmdline("colossal-cave")
	cave = SIM.new(_bench.seed_value if _bench.enabled else 0)
	locale = LocaleScript.new(LocaleScript.load_language())
	_autoplay = AutoplayScript.new()
	_tier = Quality.load_tier()
	if _bench.enabled and _bench.quality_tier >= 0:
		_tier = clampi(_bench.quality_tier, 0, Quality.NAMES.size() - 1)

	_setup_cavern()
	_setup_audio()
	_setup_dust()
	_apply_quality()

	_last_room = cave.room_id
	_refresh_scene()
	view.set_room(cave)
	hud.sync(cave)
	hud.set_locale(locale)
	hud.set_pause_visible(false)
	hud.set_quality_text(_quality_name())
	hud.set_language_label(locale.language_label())
	hud.set_menu_row(_menu_row)
	hud.show_banner(locale.text("title"), Art.LAMP)

	if _bench.enabled:
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
		Engine.max_fps = 0

func _exit_tree() -> void:
	Engine.time_scale = 1.0

# --- configuracao --------------------------------------------------------

func _setup_cavern() -> void:
	_material = ShaderMaterial.new()
	_material.shader = CavernShader
	background.material = _material

func _setup_audio() -> void:
	_sfx = SfxKit.new()
	add_child(_sfx)

func _setup_dust() -> void:
	dust.texture = Art.radial_texture(64)
	dust.material = Art.additive_material()
	dust.amount = 24
	dust.lifetime = 6.0
	dust.local_coords = false
	dust.gravity = Vector2(0.0, -6.0)
	dust.direction = Vector2.UP
	dust.spread = 60.0
	dust.initial_velocity_min = 4.0
	dust.initial_velocity_max = 16.0
	dust.scale_amount_min = 0.05
	dust.scale_amount_max = 0.18
	dust.color = Color(Art.LAMP, 0.35)
	dust.emission_shape = CPUParticles2D.EMISSION_SHAPE_RECTANGLE
	dust.emission_rect_extents = Vector2(520.0, 260.0)
	dust.position = Vector2(640.0, 380.0)

func _apply_quality() -> void:
	_preset = Quality.preset(_tier)
	dust.amount = int(_preset["particles"])
	dust.emitting = int(_preset["particles"]) > 0 and _light > 0.05
	_material.set_shader_parameter("grain_strength", _preset["grain"])
	_material.set_shader_parameter("vram_fog", _preset["fog"])
	hud.set_quality_text(_quality_name())

## Nome do preset no idioma ativo (os presets nao sao nomes proprios).
func _quality_name() -> String:
	return locale.text("quality_%d" % _tier)

func _change_quality(direction: int) -> void:
	_tier = clampi(_tier + direction, 0, Quality.NAMES.size() - 1)
	Quality.save_tier(_tier)
	_apply_quality()
	_sfx.play("ui", 1.0 + 0.1 * direction)

## Troca de idioma a quente: a simulacao nao muda (ela nao tem texto), so o
## texto exibido -- e o estado do jogo continua onde estava.
func _change_language() -> void:
	locale = LocaleScript.new(locale.next_language())
	LocaleScript.save_language(locale.language)
	hud.set_locale(locale)
	hud.set_language_label(locale.language_label())
	hud.set_quality_text(_quality_name())
	hud.sync(cave)
	_sfx.play("ui", 1.1)

# --- estado visual -------------------------------------------------------

## Recalcula luz alvo, semente da sala e poeira.
func _refresh_scene() -> void:
	_target_light = 0.85 if bool(cave.room()["light"]) else (0.55 if cave.lamp_on else 0.03)
	# Semente por sala: cada sala tem veios de rocha proprios.
	var seed_value := float(cave.room_id.hash() % 100)
	_material.set_shader_parameter("room_seed", seed_value)
	_material.set_shader_parameter("light", _light)
	view.set_room(cave)
	view.set_light(_light)

func _start_step() -> void:
	_reveal = 0.0
	_reveal_speed = 1.0 / maxf(float(_preset["step_time"]), 0.05)

# --- laco ----------------------------------------------------------------

func _process(delta: float) -> void:
	if _bench.enabled and _bench.frame():
		_bench_out()

	# Transicao de passo: a sala "revela" da parede do fundo para os pes.
	if _reveal < 1.0 and _reveal_speed > 0.0:
		_reveal = minf(1.0, _reveal + _reveal_speed * delta * maxf(Engine.time_scale, 0.2))
		view.set_reveal(_reveal)

	_update_light(delta)
	_maybe_restart()

	if _bench.enabled:
		_bench_frame += 1
		if _bench_frame % BENCH_FRAMES_PER_TURN == 0:
			_act(_autoplay.next())

func _update_light(delta: float) -> void:
	# A luz segue o alvo com atraso (entrar numa sala escura nao e um estalo).
	_light = lerpf(_light, _target_light, 1.0 - exp(-6.0 * delta))

	var value := _light
	if bool(_preset["torch_flicker"]) and cave.lamp_on:
		# Tremulacao do lampiao: ruido lento, sem sortear a cada quadro.
		_flicker += delta
		value *= 1.0 + 0.05 * sin(_flicker * 9.0) + 0.03 * sin(_flicker * 23.0)
		value *= 0.96 + 0.04 * clampf(float(cave.lamp_fuel) / 60.0, 0.0, 1.0)

	_material.set_shader_parameter("light", clampf(value, 0.0, 1.0))
	view.set_light(clampf(value, 0.0, 1.0))
	dust.emitting = int(_preset["particles"]) > 0 and value > 0.08

# --- entrada -------------------------------------------------------------

func _unhandled_input(event: InputEvent) -> void:
	if _bench.enabled:
		return
	if event is InputEventKey and event.pressed and not event.echo:
		match event.physical_keycode:
			KEY_ESCAPE:
				_toggle_pause()
			KEY_F3:
				hud.toggle_budget()
			KEY_LEFT:
				if _paused:
					_menu_adjust(-1)
				else:
					_act_move("o")
			KEY_RIGHT:
				if _paused:
					_menu_adjust(1)
				else:
					_act_move("l")
			KEY_W, KEY_UP:
				if _paused:
					_menu_move(-1)
				else:
					_act_move("n")
			KEY_S, KEY_DOWN:
				if _paused:
					_menu_move(1)
				else:
					_act_move("s")
			KEY_A:
				if not _paused:
					_act_move("o")
			KEY_D:
				if not _paused:
					_act_move("l")
			KEY_R, KEY_PAGEUP:
				if not _paused:
					_act_move("c")
			KEY_F, KEY_PAGEDOWN:
				if not _paused:
					_act_move("b")
			KEY_TAB:
				if _paused:
					_menu_move(1)
			KEY_E, KEY_ENTER:
				if not _paused:
					_take_first()
			KEY_Q:
				if _paused:
					get_tree().quit()
				else:
					_drop_first()
			KEY_L:
				if not _paused:
					_act(SIM.Action.new("lamp"))
	elif event is InputEventJoypadButton and event.pressed:
		match event.button_index:
			JOY_BUTTON_START:
				_toggle_pause()
			JOY_BUTTON_A:
				if not _paused:
					_take_first()
			JOY_BUTTON_B:
				if not _paused:
					_drop_first()
			JOY_BUTTON_X:
				if not _paused:
					_act(SIM.Action.new("lamp"))
			JOY_BUTTON_Y:
				if not _paused:
					_act_move("b")
	elif event is InputEventJoypadMotion:
		if absf(event.axis_value) > 0.7 and not _paused:
			if event.axis == JOY_AXIS_LEFT_X:
				_act_move("l" if event.axis_value > 0 else "o")
			elif event.axis == JOY_AXIS_LEFT_Y:
				_act_move("s" if event.axis_value > 0 else "n")

## Troca a linha ativa do menu (0 = graficos, 1 = idioma).
func _menu_move(direction: int) -> void:
	_menu_row = clampi(_menu_row + direction, 0, 1)
	hud.set_menu_row(_menu_row)
	_sfx.play("ui", 1.0)

## Ajusta a opcao da linha ativa (-1 / +1).
func _menu_adjust(direction: int) -> void:
	if _menu_row == 0:
		_change_quality(direction)
	else:
		_change_language()

func _act_move(dir: String) -> void:
	var action = SIM.Action.new("move")
	action.dir = dir
	_act(action)

func _take_first() -> void:
	var here: Array = cave.items_here()
	if here.is_empty():
		return
	_act(SIM.Action.new("take", here[0]))

func _drop_first() -> void:
	# Larga o primeiro item que nao seja o lampiao (perder a luz e fatal).
	for item in cave.inventory:
		if item != "lampiao":
			_act(SIM.Action.new("drop", item))
			return

func _toggle_pause() -> void:
	_paused = not _paused
	hud.set_pause_visible(_paused)
	_sfx.play("ui")

# --- turno ---------------------------------------------------------------

func _act(action) -> void:
	if _paused or cave.is_over:
		return
	var before: String = cave.room_id
	if not cave.step(action):
		return

	_handle_events(before)
	hud.sync(cave)
	_refresh_scene()
	if cave.room_id != before:
		_start_step()

func _handle_events(before: String) -> void:
	for event in cave.events:
		match String(event["type"]):
			"move":
				_sfx.play("step", randf_range(0.9, 1.1))
			"dark":
				_sfx.play("denied", 0.8)
				hud.flash(Color(0.0, 0.0, 0.0), 0.35)
			"blocked":
				_sfx.play("denied", 0.9)
			"take":
				_sfx.play("take")
				_spawn_ring(640.0, 470.0, Art.item_color(event["item"]), 120.0)
			"drop":
				_sfx.play("drop")
			"lamp":
				_sfx.play("lamp")
			"fall":
				_sfx.play("fall")
				hud.flash(Art.ACCENT, 0.5)
				hud.show_banner(locale.text("fell"), Art.ACCENT)
			"deposit":
				_sfx.play("deposit")
				hud.show_banner(locale.message("ui.points_gain", {"points": int(event["points"])}), Art.GOLD)
				hud.flash(Art.GOLD, 0.18)
				_spawn_ring(640.0, 470.0, Art.GOLD, 220.0)
			"win":
				_sfx.play("win")
				hud.show_banner(locale.text("won"), Art.GOLD)
				hud.flash(Art.GOLD, 0.4)
				_restart_at = Time.get_ticks_msec() + 3200
			"over":
				_sfx.play("denied", 0.7)
				hud.show_banner("%s: %s" % [locale.text("end"), locale.text("reason_%s" % String(event["reason"]))], Art.ACCENT)
				_restart_at = Time.get_ticks_msec() + 2600

func _maybe_restart() -> void:
	if _restart_at == 0 or Time.get_ticks_msec() < _restart_at:
		return
	_restart_at = 0
	cave.restart()
	_autoplay.reset()
	_last_room = cave.room_id
	hud.sync(cave)
	_refresh_scene()
	_start_step()

func _spawn_ring(x: float, y: float, color: Color, radius: float) -> void:
	if int(_preset["particles"]) <= 0:
		return
	var ring := FxRing.new()
	$Arena/FX.add_child(ring)
	ring.setup(Vector2(x, y), color, radius, 0.5)

# --- benchmark -----------------------------------------------------------

func _bench_out() -> void:
	var code := _bench.finish({
		"room": cave.room_id,
		"turns": cave.turns,
		"score": cave.score,
		"deposited": cave.deposited,
		"treasures": cave.total_treasures(),
		"won": cave.won,
	}, Vector2(1280, 720), Quality.label(_tier))
	get_tree().quit(code)
