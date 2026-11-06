class_name StarView
extends Node2D
## O star no centro: nucleo, corona pulsante e algumas "erupcoes" (raios que
## giram devagar). Tudo vetor + dois Sprite2D aditivos com a textura radial.

const SIM := preload("res://scripts/sim/spacewar.gd")

var _texture: GradientTexture2D
var _elapsed := 0.0
var _corona: Sprite2D
var _halo: Sprite2D
var _show_corona := true

func _ready() -> void:
	position = SIM.ARENA * 0.5
	z_index = 0

	_texture = Art.radial_texture(128)

	_halo = Sprite2D.new()
	_halo.texture = _texture
	_halo.material = Art.additive_material()
	_halo.modulate = Color(Art.STAR_GLOW, 0.30)
	_halo.scale = Vector2.ONE * (SIM.STAR_RADIUS * 9.0 / 128.0)
	add_child(_halo)

	_corona = Sprite2D.new()
	_corona.texture = _texture
	_corona.material = Art.additive_material()
	_corona.modulate = Color(Art.STAR_CORE, 0.55)
	_corona.scale = Vector2.ONE * (SIM.STAR_RADIUS * 3.6 / 128.0)
	add_child(_corona)

func set_corona_visible(value: bool) -> void:
	_show_corona = value
	if _corona:
		_corona.visible = value

func _process(delta: float) -> void:
	_elapsed += delta
	# Pulso lento da corona: o star "respira", sem custo de CPU.
	var pulse := 1.0 + 0.05 * sin(_elapsed * 1.6)
	if _corona:
		_corona.scale = Vector2.ONE * (SIM.STAR_RADIUS * 3.6 / 128.0) * pulse
	if _halo:
		_halo.scale = Vector2.ONE * (SIM.STAR_RADIUS * 9.0 / 128.0) * (2.0 - pulse)
	queue_redraw()

func _draw() -> void:
	# Nucleo.
	draw_circle(Vector2.ZERO, SIM.STAR_RADIUS, Art.STAR_CORE)
	draw_arc(Vector2.ZERO, SIM.STAR_RADIUS + 2.0, 0.0, TAU, 42,
		Color(Art.STAR_GLOW, 0.75), 2.0, true)

	if not _show_corona:
		return

	# Raios: 12 pontas com comprimento variavel, girando devagar.
	for i in 12:
		var angle := TAU * float(i) / 12.0 + _elapsed * 0.25
		var length := SIM.STAR_RADIUS * (1.35 + 0.35 * sin(_elapsed * 2.0 + i))
		var from := Vector2(cos(angle), sin(angle)) * (SIM.STAR_RADIUS + 1.0)
		var to := Vector2(cos(angle), sin(angle)) * length
		draw_line(from, to, Color(Art.STAR_GLOW, 0.35), 1.5, true)
