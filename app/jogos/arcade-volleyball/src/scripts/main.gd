extends Node2D
## Arcade Volleyball -- camada de apresentacao.
##
## A simulacao vive em scripts/sim/match.gd e nao conhece engine grafico. Aqui
## so ha entrada, passos fixos, sincronizacao das views e o "juice" (shake,
## particulas, slow motion, som). F3 liga o painel de orcamento (RAM/VRAM).

const Match := preload("res://scripts/sim/match.gd")
const FixedStepScript := preload("res://scripts/sim/fixed_step.gd")
const CourtShader := preload("res://shaders/court.gdshader")

@onready var court: ColorRect = $Background/Court
@onready var net: NetView = $Arena/Net
@onready var player_views: Array[PlayerView] = [$Arena/Player1, $Arena/Player2]
@onready var ball_view: BallView = $Arena/Ball
@onready var trail: Line2D = $Arena/Trail
@onready var glow: Sprite2D = $Arena/BallGlow
@onready var fx: Node2D = $Arena/FX
@onready var camera: Camera2D = $Camera
@onready var hud: GameHud = $HUD

var model: VolleyMatch
var _bench: Bench
var _controls
var _clock: FixedStep
var _tier := Quality.DEFAULT_TIER
var _preset: Dictionary = {}

var _sfx: SfxKit
var _court_material: ShaderMaterial
var _dust_texture: GradientTexture2D

var _trauma := 0.0
var _punch := 0.0
var _shake_clock := 0.0
var _slowmo_scale := 1.0
var _slowmo_until := 0
var _hitstop_until := 0
var _restart_at := 0
var _paused := false
var _prev_grounded := [true, true]

func _ready() -> void:
	_bench = Bench.from_cmdline("arcade-volleyball")
	model = Match.new()
	if _bench.enabled:
		for i in 2:
			model.players[i].score = 0
	_controls = Match.Controls.new()
	_clock = FixedStepScript.new()
	_tier = Quality.load_tier()
	if _bench.enabled and _bench.quality_tier >= 0:
		_tier = clampi(_bench.quality_tier, 0, Quality.NAMES.size() - 1)

	_setup_court()
	_setup_trail()
	_setup_glow()
	_setup_audio()
	_apply_quality()

	for i in 2:
		_prev_grounded[i] = model.players[i].grounded
		hud.set_score(i, 0, false)
	_sync()
	hud.set_pause_visible(false)
	hud.set_quality_text(Quality.label(_tier))

	if _bench.enabled:
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
		Engine.max_fps = 0

func _exit_tree() -> void:
	Engine.time_scale = 1.0

# --- configuracao --------------------------------------------------------

func _setup_court() -> void:
	_court_material = ShaderMaterial.new()
	_court_material.shader = CourtShader
	court.material = _court_material
	_court_material.set_shader_parameter("floor_start", Match.FLOOR_Y / Match.ARENA.y)
	_court_material.set_shader_parameter("accent", Art.ACCENT)

func _setup_trail() -> void:
	trail.width = 14.0
	trail.joint_mode = Line2D.LINE_JOINT_ROUND
	trail.begin_cap_mode = Line2D.LINE_CAP_ROUND
	trail.end_cap_mode = Line2D.LINE_CAP_ROUND
	trail.z_index = -1

	var gradient := Gradient.new()
	gradient.set_color(0, Color(Art.ACCENT, 0.0))
	gradient.set_color(1, Color(Art.ACCENT_WARM, 0.75))
	gradient.add_point(0.7, Color(1.0, 1.0, 1.0, 0.5))
	trail.gradient = gradient

func _setup_glow() -> void:
	_dust_texture = Art.radial_texture(128)
	glow.texture = _dust_texture
	glow.material = Art.additive_material()
	glow.modulate = Color(1.0, 0.45, 0.4, 0.5)
	glow.scale = Vector2.ONE * (Match.BALL_RADIUS * 5.0 / 128.0)
	glow.z_index = -2

func _setup_audio() -> void:
	_sfx = SfxKit.new()
	add_child(_sfx)

func _apply_quality() -> void:
	_preset = Quality.preset(_tier)

	for view in player_views:
		view.set_softness(bool(_preset["soft_shadow"]))

	glow.visible = bool(_preset["glow"])
	trail.visible = int(_preset["trail_points"]) > 0
	trail.clear_points()

	_court_material.set_shader_parameter("vignette_strength", _preset["vignette"])
	_court_material.set_shader_parameter("grain_strength", _preset["grain"])
	_court_material.set_shader_parameter("ball_glow", _preset["ball_glow"])
	hud.set_quality_text(Quality.label(_tier))

	if _bench.enabled:
		DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)
		Engine.max_fps = 0

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

	# delta ja vem escalado por Engine.time_scale: o slow motion apenas entrega
	# menos passos por segundo real, sem quebrar a integracao.
	var steps := _clock.advance(delta)
	for i in steps:
		model.step(_controls)
		_handle_events()

	if steps > 0:
		_sync()

func _process(delta: float) -> void:
	if _bench.enabled and _bench.frame():
		_bench_out()

	_update_time_scale()
	_update_camera(delta)
	_update_trail_and_glow()
	_maybe_restart()

# --- entrada -------------------------------------------------------------

func _read_controls() -> void:
	if _bench.enabled:
		# Autoplay deterministico: P1 segue a bola (mesma politica dos testes).
		_read_autoplay()
		return

	var axis := _joy_axis(JOY_AXIS_LEFT_X)
	_controls.left = (Input.is_physical_key_pressed(KEY_A)
		or Input.is_physical_key_pressed(KEY_LEFT)
		or _joy_button(JOY_BUTTON_DPAD_LEFT)
		or axis < -0.5)
	_controls.right = (Input.is_physical_key_pressed(KEY_D)
		or Input.is_physical_key_pressed(KEY_RIGHT)
		or _joy_button(JOY_BUTTON_DPAD_RIGHT)
		or axis > 0.5)
	_controls.jump = (Input.is_physical_key_pressed(KEY_W)
		or Input.is_physical_key_pressed(KEY_UP)
		or Input.is_physical_key_pressed(KEY_SPACE)
		or _joy_button(JOY_BUTTON_A))

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

func _joy_axis(axis: int) -> float:
	var pads := Input.get_connected_joypads()
	if pads.is_empty():
		return 0.0
	return Input.get_joy_axis(pads[0], axis)

func _joy_button(button: int) -> bool:
	var pads := Input.get_connected_joypads()
	if pads.is_empty():
		return false
	return Input.is_joy_button_pressed(pads[0], button)

## Politica de autoplay do benchmark: persegue a bola e pula quando ela esta
## acima e perto. E a mesma do teste de invariantes, entao o bench mede uma
## partida de verdade, com pontos acontecendo.
func _read_autoplay() -> void:
	var delta: float = model.ball.pos.x - model.players[0].center_x()
	_controls.left = delta < -8.0
	_controls.right = delta > 8.0
	_controls.jump = (model.ball.pos.y < model.players[0].pos.y
		and absf(delta) < 60.0 and model.players[0].grounded)

func _bench_out() -> void:
	var code := _bench.finish({
		"score_p1": model.players[0].score,
		"score_p2": model.players[1].score,
	}, Match.ARENA, Quality.label(_tier))
	get_tree().quit(code)

func _toggle_pause() -> void:
	_paused = not _paused
	hud.set_pause_visible(_paused)
	if _paused:
		# Pausa sempre em velocidade normal: ao voltar, nada de slow motion
		# "preso" de um ponto que caiu antes da pausa.
		_clear_time_effects()
	_sfx.play("ui")

func _clear_time_effects() -> void:
	_slowmo_until = 0
	_hitstop_until = 0
	Engine.time_scale = 1.0

# --- simulacao -> visual -------------------------------------------------

func _sync() -> void:
	for i in 2:
		var player = model.players[i]
		player_views[i].sync(player)
		if player.grounded and not _prev_grounded[i]:
			_spawn_dust(player.pos + Vector2(Match.PLAYER_SIZE.x * 0.5, Match.PLAYER_SIZE.y))
		_prev_grounded[i] = player.grounded
	ball_view.sync(model.ball)

func _handle_events() -> void:
	var now := Time.get_ticks_msec()
	for event in model.events:
		match String(event["type"]):
			"hit":
				_on_hit(event, now)
			"net":
				_sfx.play("net", 1.0 + float(event["power"]) * 0.03)
				_spawn_burst(event["pos"], Art.TEXT_SECONDARY, 6, 90.0, 0.25)
				_trauma = minf(1.0, _trauma + 0.06)
			"point":
				_on_point(event, now)
			"win":
				_on_win(event, now)

func _on_hit(event: Dictionary, now: int) -> void:
	var who: int = int(event["who"])
	var power: float = float(event["power"])
	var strength: float = clampf(power / 70.0, 0.05, 0.24)

	player_views[who].punch(strength)
	ball_view.punch(strength * 0.8)
	_trauma = minf(1.0, _trauma + 0.10 + power * 0.004)

	var strong := power > 11.0
	_sfx.play("spike" if strong else "bounce", 0.9 + power * 0.02)
	_spawn_burst(event["pos"], Art.TEXT_PRIMARY, 10 if strong else 6, 150.0, 0.3)

	_spawn_ring(event["pos"], Art.TEXT_PRIMARY, 46.0 + power * 2.0, 0.26)

	if strong and int(_preset["hitstop_ms"]) > 0:
		_hitstop_until = now + int(_preset["hitstop_ms"])

func _on_point(event: Dictionary, now: int) -> void:
	var side: int = int(event["side"])
	hud.set_score(side, model.players[side].score, true)
	hud.show_banner("PONTO!", Art.ACCENT if side == 0 else Art.TEXT_PRIMARY)
	_sfx.play("point")
	_trauma = 1.0
	_punch = 1.0
	_spawn_burst(event["pos"], Art.ACCENT, 26, 260.0, 0.5)
	_spawn_ring(event["pos"], Art.ACCENT, 190.0, 0.5)
	hud.flash(Art.ACCENT if side == 0 else Art.TEXT_PRIMARY, 0.20)
	if bool(_preset["slowmo"]):
		_slowmo_scale = 0.3
		_slowmo_until = now + 260

func _on_win(event: Dictionary, now: int) -> void:
	var side: int = int(event["side"])
	hud.show_banner("VITÓRIA P%d" % (side + 1), Art.ACCENT if side == 0 else Art.TEXT_PRIMARY)
	_sfx.play("win")
	_trauma = 1.0
	_punch = 1.0
	hud.flash(Art.ACCENT if side == 0 else Art.TEXT_PRIMARY, 0.32)
	_spawn_ring(ball_view.position, Art.TEXT_PRIMARY, 260.0, 0.7)
	if bool(_preset["slowmo"]):
		_slowmo_scale = 0.35
		_slowmo_until = now + 900
	_restart_at = now + 3200

func _maybe_restart() -> void:
	if _restart_at == 0 or Time.get_ticks_msec() < _restart_at:
		return
	_restart_at = 0
	model.restart()
	for i in 2:
		hud.set_score(i, 0, false)
		_prev_grounded[i] = model.players[i].grounded
	_sync()

# --- juice ---------------------------------------------------------------

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
		# Suaviza a volta; o "congelamento" do hitstop entra seco de proposito.
		Engine.time_scale = lerpf(current, target, 0.25)

func _update_camera(delta: float) -> void:
	_trauma = maxf(0.0, _trauma - delta * 1.8)
	_punch = maxf(0.0, _punch - delta * 3.0)
	_shake_clock += delta

	var amount: float = _trauma * _trauma * 16.0 * float(_preset["shake"])
	camera.offset = Vector2(
		sin(_shake_clock * 61.0) * amount,
		cos(_shake_clock * 47.0) * amount * 0.8
	)
	camera.zoom = Vector2.ONE * (1.0 + _punch * 0.025)

func _update_trail_and_glow() -> void:
	var ball_pos: Vector2 = model.ball.pos
	glow.position = ball_pos
	_court_material.set_shader_parameter("ball_uv", ball_pos / Match.ARENA)

	var max_points := int(_preset["trail_points"])
	if max_points <= 0:
		return
	trail.add_point(ball_pos)
	while trail.get_point_count() > max_points:
		trail.remove_point(0)

func _spawn_dust(at: Vector2) -> void:
	_spawn_burst(at, Art.TEXT_SECONDARY, 8, 70.0, 0.35)

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
	particles.texture = _dust_texture
	particles.material = Art.additive_material()
	particles.amount = mini(scaled, 120)
	particles.lifetime = lifetime
	particles.one_shot = true
	particles.explosiveness = 0.92
	particles.local_coords = false
	particles.direction = Vector2.UP
	particles.spread = 180.0
	particles.gravity = Vector2(0.0, 420.0)
	particles.initial_velocity_min = speed * 0.35
	particles.initial_velocity_max = speed
	particles.scale_amount_min = 0.10
	particles.scale_amount_max = 0.28
	particles.emission_shape = CPUParticles2D.EMISSION_SHAPE_SPHERE
	particles.emission_sphere_radius = 10.0
	particles.color = color
	particles.position = at
	particles.emitting = true
	particles.finished.connect(particles.queue_free)
	fx.add_child(particles)
