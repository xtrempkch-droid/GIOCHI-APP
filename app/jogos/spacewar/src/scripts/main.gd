extends Node2D
## Spacewar -- camada de apresentacao.
##
## A simulacao vive em scripts/sim/spacewar.gd e nao conhece engine grafico.
## Aqui so ha entrada, passos fixos, sincronizacao das views e o "juice"
## (shake, particulas, slow motion, som). F3 liga o painel de orcamento.
##
## Dois jogadores: com dois controles conectados (ou usando o teclado do P2) o
## segundo navio e humano; senao a IA (scripts/sim/ai.gd) assume.

const SIM := preload("res://scripts/sim/spacewar.gd")
const AiScript := preload("res://scripts/sim/ai.gd")
const FixedStepScript := preload("res://scripts/sim/fixed_step.gd")
const SpaceShader := preload("res://shaders/space.gdshader")

@onready var space: ColorRect = $Background/Space
@onready var star: StarView = $Arena/Star
@onready var ship_views: Array[ShipView] = [$Arena/Ship1, $Arena/Ship2]
@onready var torpedoes: TorpedoLayer = $Arena/Torpedoes
@onready var fx: Node2D = $Arena/FX
@onready var camera: Camera2D = $Camera
@onready var hud: GameHud = $HUD

var model: SpacewarMatch
var _ai
var _clock: FixedStep
var _p1
var _p2
var _tier := Quality.DEFAULT_TIER
var _preset: Dictionary = {}
var _sfx: SfxKit
var _material: ShaderMaterial
var _dust: GradientTexture2D

var _trauma := 0.0
var _punch := 0.0
var _shake_clock := 0.0
var _slowmo_scale := 1.0
var _slowmo_until := 0
var _hitstop_until := 0
var _restart_at := 0
var _paused := false
var _p2_human := false
var _ship_alive := [true, true]
var _bench: Bench

func _ready() -> void:
	_bench = Bench.from_cmdline("spacewar")
	model = SIM.new(_bench.seed_value if _bench.enabled else 0)
	_ai = AiScript.new()
	_clock = FixedStepScript.new()
	_p1 = SIM.Controls.new()
	_p2 = SIM.Controls.new()
	_tier = Quality.load_tier()
	if _bench.enabled and _bench.quality_tier >= 0:
		_tier = clampi(_bench.quality_tier, 0, Quality.NAMES.size() - 1)

	_setup_space()
	_setup_audio()
	_dust = Art.radial_texture(128)
	_apply_quality()

	_p2_human = Input.get_connected_joypads().size() >= 2
	for i in 2:
		hud.set_score(i, 0, false)
		hud.set_tag(i, "P2 (CPU)" if (i == 1 and not _p2_human) else "P%d" % (i + 1))
	hud.set_pause_visible(false)
	hud.set_quality_text(Quality.label(_tier))
	_sync(0.0)

	if _bench.enabled:
		# Benchmark: sem vsync (mede o custo do jogo, nao a espera do monitor),
		# sem IA reagindo ao acaso, e o P1 jogado pela propria IA.
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
		Engine.max_fps = 0
		_p2_human = false
		# O HUD fica visivel: benchmark de tela vazia nao mede o jogo.

func _exit_tree() -> void:
	Engine.time_scale = 1.0

# --- configuracao --------------------------------------------------------

func _setup_space() -> void:
	_material = ShaderMaterial.new()
	_material.shader = SpaceShader
	space.material = _material
	_material.set_shader_parameter("star_uv", SIM.ARENA * 0.5 / SIM.ARENA)

func _setup_audio() -> void:
	_sfx = SfxKit.new()
	add_child(_sfx)

## Troca preset em tempo de execucao (pausa: ← →).
func _apply_quality() -> void:
	_preset = Quality.preset(_tier)
	torpedoes.trail_points = int(_preset["trail_points"])
	torpedoes.clear()
	star.set_corona_visible(bool(_preset["corona"]))
	for view in ship_views:
		view.set_thrust_visible(bool(_preset["thrust_flame"]))
	_material.set_shader_parameter("nebula_strength", _preset["nebula"])
	_material.set_shader_parameter("vignette_strength", _preset["vignette"])
	_material.set_shader_parameter("grain_strength", _preset["grain"])
	hud.set_quality_text(Quality.label(_tier))

func _change_quality(direction: int) -> void:
	_tier = clampi(_tier + direction, 0, Quality.NAMES.size() - 1)
	Quality.save_tier(_tier)
	_apply_quality()
	_sfx.play("ui", 1.0 + 0.1 * direction)

# --- laco principal ------------------------------------------------------

func _physics_process(delta: float) -> void:
	if _paused:
		return

	_read_controls()

	# delta ja vem escalado por Engine.time_scale: o slow motion entrega menos
	# passos por segundo real sem quebrar a integracao.
	var steps := _clock.advance(delta)
	for i in steps:
		model.step(1.0 / 60.0, _p1, _p2)
		_handle_events()
	if steps > 0:
		_sync(delta)

func _process(delta: float) -> void:
	if _bench.enabled and _bench.frame():
		_bench_out()

	_update_time_scale()
	_update_camera(delta)
	_maybe_restart()
	# O star pulsa por tempo real; o shader usa o mesmo relogio.
	var t := Time.get_ticks_msec() * 0.001
	_material.set_shader_parameter("star_pulse", 1.0 + 0.06 * sin(t * 1.6))

# --- entrada -------------------------------------------------------------

func _read_controls() -> void:
	if _bench.enabled:
		# Autoplay deterministico: o P1 e jogado pela IA, o P2 tambem.
		_p1 = _ai.decide(model, 0)
		_p2 = _ai.decide(model, 1)
		return

	var pads := Input.get_connected_joypads()

	# P1: teclado (A/D/W/S/Q) ou o primeiro controle.
	var axis1 := 0.0
	if pads.size() >= 1:
		axis1 = Input.get_joy_axis(pads[0], JOY_AXIS_LEFT_X)
	_p1.rotate_left = (Input.is_physical_key_pressed(KEY_A)
		or (pads.size() >= 1 and Input.is_joy_button_pressed(pads[0], JOY_BUTTON_DPAD_LEFT))
		or axis1 < -0.4)
	_p1.rotate_right = (Input.is_physical_key_pressed(KEY_D)
		or (pads.size() >= 1 and Input.is_joy_button_pressed(pads[0], JOY_BUTTON_DPAD_RIGHT))
		or axis1 > 0.4)
	_p1.thrust = (Input.is_physical_key_pressed(KEY_W)
		or (pads.size() >= 1 and Input.is_joy_button_pressed(pads[0], JOY_BUTTON_A)))
	_p1.fire = (Input.is_physical_key_pressed(KEY_S)
		or (pads.size() >= 1 and Input.is_joy_button_pressed(pads[0], JOY_BUTTON_B)))
	_p1.hyperspace = (Input.is_physical_key_pressed(KEY_Q)
		or (pads.size() >= 1 and Input.is_joy_button_pressed(pads[0], JOY_BUTTON_Y)))

	# P2: teclado (setas + "/") ou o segundo controle.
	var key_used := (Input.is_physical_key_pressed(KEY_LEFT)
		or Input.is_physical_key_pressed(KEY_RIGHT)
		or Input.is_physical_key_pressed(KEY_UP)
		or Input.is_physical_key_pressed(KEY_DOWN)
		or Input.is_physical_key_pressed(KEY_SLASH))
	if key_used and not _p2_human:
		_p2_human = true
		hud.set_tag(1, "P2")

	var axis2 := 0.0
	if pads.size() >= 2:
		axis2 = Input.get_joy_axis(pads[1], JOY_AXIS_LEFT_X)

	_p2.rotate_left = (Input.is_physical_key_pressed(KEY_LEFT)
		or (pads.size() >= 2 and Input.is_joy_button_pressed(pads[1], JOY_BUTTON_DPAD_LEFT))
		or axis2 < -0.4)
	_p2.rotate_right = (Input.is_physical_key_pressed(KEY_RIGHT)
		or (pads.size() >= 2 and Input.is_joy_button_pressed(pads[1], JOY_BUTTON_DPAD_RIGHT))
		or axis2 > 0.4)
	_p2.thrust = (Input.is_physical_key_pressed(KEY_UP)
		or (pads.size() >= 2 and Input.is_joy_button_pressed(pads[1], JOY_BUTTON_A)))
	_p2.fire = (Input.is_physical_key_pressed(KEY_DOWN)
		or (pads.size() >= 2 and Input.is_joy_button_pressed(pads[1], JOY_BUTTON_B)))
	_p2.hyperspace = (Input.is_physical_key_pressed(KEY_SLASH)
		or (pads.size() >= 2 and Input.is_joy_button_pressed(pads[1], JOY_BUTTON_Y)))

	if not _p2_human:
		_p2 = _ai.decide(model, 1)

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		match event.physical_keycode:
			KEY_ESCAPE:
				_toggle_pause()
			KEY_F3:
				hud.toggle_budget()
			KEY_LEFT:
				if _paused:
					_change_quality(-1)
			KEY_RIGHT:
				if _paused:
					_change_quality(1)
			KEY_Q:
				if _paused:
					get_tree().quit()
	elif event is InputEventJoypadButton and event.pressed:
		if event.button_index == JOY_BUTTON_START:
			_toggle_pause()

func _bench_out() -> void:
	var code := _bench.finish({
		"preset": Quality.label(_tier),
		"torpedoes": model.torpedoes.size(),
		"kills": model.ships[0].kills + model.ships[1].kills,
	}, SIM.ARENA, Quality.label(_tier))
	get_tree().quit(code)

func _toggle_pause() -> void:
	_paused = not _paused
	hud.set_pause_visible(_paused)
	if _paused:
		# Pausa sempre em velocidade normal: ao voltar, nada de slow motion
		# "preso" de um abate que aconteceu antes da pausa.
		_slowmo_until = 0
		_hitstop_until = 0
		Engine.time_scale = 1.0
	_sfx.play("ui")

# --- simulacao -> visual -------------------------------------------------

func _sync(_delta: float) -> void:
	for i in 2:
		var ship = model.ships[i]
		ship_views[i].sync(ship)
		hud.sync_ship(i, ship)
		if ship.alive and not _ship_alive[i]:
			_spawn_burst(ship.pos, Art.ship_color(i), 14, 90.0, 0.5)
		_ship_alive[i] = ship.alive

	ship_views[0].set_thrust(_p1.thrust and model.ships[0].alive)
	ship_views[1].set_thrust(_p2.thrust and model.ships[1].alive)
	torpedoes.sync(model.torpedoes)

func _handle_events() -> void:
	var now := Time.get_ticks_msec()
	for event in model.events:
		match String(event["type"]):
			"fire":
				_sfx.play("fire", 0.95 + 0.1 * float(event["who"]))
				_spawn_burst(event["pos"], Art.TORPEDO, 4, 60.0, 0.22)
			"hit":
				_sfx.play("denied", 0.9)
			"denied":
				_sfx.play("denied", 0.8)
			"hyper":
				_on_hyper(event)
			"explode":
				_on_explode(event, now)
			"score":
				hud.set_score(int(event["who"]), int(event["kills"]), true)
			"win":
				_on_win(event, now)
			"respawn":
				_spawn_ring(event["pos"], Art.ship_color(int(event["who"])), 110.0, 0.45)

func _on_hyper(event: Dictionary) -> void:
	_sfx.play("hyper")
	_spawn_ring(event["from"], Art.ship_color(int(event["who"])), 70.0, 0.35)
	_spawn_ring(event["pos"], Art.ship_color(int(event["who"])), 70.0, 0.35)
	_trauma = minf(1.0, _trauma + 0.12)

func _on_explode(event: Dictionary, now: int) -> void:
	var who: int = int(event["who"])
	_sfx.play("explode")
	_camera_shake(0.55)
	_spawn_burst(event["pos"], Art.STAR_GLOW, 40, 320.0, 0.85)
	_spawn_burst(event["pos"], Art.ship_color(who), 18, 190.0, 0.65)
	_spawn_ring(event["pos"], Art.TEXT_PRIMARY, 210.0, 0.55)
	hud.flash(Art.STAR_GLOW, 0.16)

	if int(_preset["hitstop_ms"]) > 0:
		_hitstop_until = now + int(_preset["hitstop_ms"])
	if bool(_preset["slowmo"]):
		_slowmo_scale = 0.35
		_slowmo_until = now + 240

func _on_win(event: Dictionary, now: int) -> void:
	var who: int = int(event["who"])
	_sfx.play("win")
	hud.show_banner("P%d VENCE" % (who + 1), Art.ship_color(who))
	hud.flash(Art.ship_color(who), 0.30)
	_camera_shake(0.9)
	if bool(_preset["slowmo"]):
		_slowmo_scale = 0.4
		_slowmo_until = now + 900
	_restart_at = now + 3200

func _maybe_restart() -> void:
	if _restart_at == 0 or Time.get_ticks_msec() < _restart_at:
		return
	_restart_at = 0
	model.restart()
	_ai.reset()
	torpedoes.clear()
	for i in 2:
		hud.set_score(i, 0, false)
		_ship_alive[i] = true
	_sync(0.0)
	hud.show_banner("NOVA PARTIDA", Art.TEXT_PRIMARY)

# --- juice ---------------------------------------------------------------

func _camera_shake(strength: float) -> void:
	_trauma = minf(1.0, _trauma + strength)

func _update_time_scale() -> void:
	var now := Time.get_ticks_msec()
	var target := 1.0
	if now < _hitstop_until:
		target = 0.05
	elif now < _slowmo_until:
		target = _slowmo_scale

	var current := Engine.time_scale
	if absf(current - target) < 0.01:
		Engine.time_scale = target
	else:
		Engine.time_scale = lerpf(current, target, 0.25)

func _update_camera(delta: float) -> void:
	_trauma = maxf(0.0, _trauma - delta * 1.7)
	_punch = maxf(0.0, _punch - delta * 3.0)
	_shake_clock += delta

	var amount: float = _trauma * _trauma * 18.0 * float(_preset["shake"])
	camera.offset = Vector2(
		sin(_shake_clock * 63.0) * amount,
		cos(_shake_clock * 47.0) * amount * 0.8
	)
	camera.zoom = Vector2.ONE * (1.0 + _punch * 0.02)

func _spawn_ring(at: Vector2, color: Color, radius: float, life: float) -> void:
	if int(_preset["particles"]) <= 0:
		return # o preset baixo nao gasta com efeito decorativo
	var ring := FxRing.new()
	fx.add_child(ring)
	ring.setup(at, color, radius, life)

## Explosao de particulas reaproveitando o mesmo GradientTexture2D do brilho.
func _spawn_burst(at: Vector2, color: Color, amount: int, speed: float, lifetime: float) -> void:
	var scaled := int(amount * (float(_preset["particles"]) / 18.0))
	if scaled <= 0:
		return

	var particles := CPUParticles2D.new()
	particles.texture = _dust
	particles.material = Art.additive_material()
	particles.amount = mini(scaled, 120)
	particles.lifetime = lifetime
	particles.one_shot = true
	particles.explosiveness = 0.92
	particles.local_coords = false
	particles.direction = Vector2.UP
	particles.spread = 180.0
	particles.gravity = Vector2.ZERO
	particles.damping_min = speed * 0.4
	particles.damping_max = speed * 0.9
	particles.initial_velocity_min = speed * 0.25
	particles.initial_velocity_max = speed
	particles.scale_amount_min = 0.08
	particles.scale_amount_max = 0.26
	particles.emission_shape = CPUParticles2D.EMISSION_SHAPE_SPHERE
	particles.emission_sphere_radius = 8.0
	particles.color = color
	particles.position = at
	particles.emitting = true
	particles.finished.connect(particles.queue_free)
	fx.add_child(particles)
