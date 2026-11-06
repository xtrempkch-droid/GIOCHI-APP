class_name ShipView
extends Node2D
## Navio desenhado como vetor, no estilo do canhao do PDP-1: contorno fino,
## brilho aditivo e chama do propulsor proporcional ao empuxo.
##
## Desenha tambem o "fantasma" do outro lado do campo toroidal: quando o navio
## esta perto de uma borda, ele aparece cortado nos dois lados, como no original.

const SIM := preload("res://scripts/sim/spacewar.gd")

@export var side: int = 0

## Agulha de Spacewar: nariz fino em +x, duas aletas para tras e um entalhe
## na popa. Pontos em px de tela: 27 px de comprimento, o dobro do display do
## PDP-1 -- num painel de 5" o traco original de 10 px desaparece.
const HULL := [
	Vector2(27.0, 0.0),
	Vector2(-1.5, -4.2),
	Vector2(-15.5, -14.0),
	Vector2(-7.0, -4.9),
	Vector2(-11.0, 0.0),
	Vector2(-7.0, 4.9),
	Vector2(-15.5, 14.0),
	Vector2(-1.5, 4.2),
]
const FLAME := [
	Vector2(-9.0, -4.0),
	Vector2(-27.0, 0.0),
	Vector2(-9.0, 4.0),
]

var _pos := Vector2.ZERO
var _angle := 0.0
var _thrust := 0.0
var _target_thrust := 0.0
var _alive := true
var _invuln := 0.0
var _color := Art.SHIP_1
var _show_thrust := true
## O modelo nao guarda "estou empurrando": quem sabe disso e o Main.
var _thrust_on := false

func _ready() -> void:
	_color = Art.ship_color(side)
	z_index = 1

func set_thrust_visible(value: bool) -> void:
	_show_thrust = value

func sync(ship) -> void:
	_pos = ship.pos
	_angle = ship.angle
	_alive = ship.alive
	_invuln = ship.invuln
	# Empuxo visual: sobe rapido, cai devagar (nao "pisca" a cada quadro).
	_target_thrust = 1.0 if _thrust_on else 0.0
	position = _pos
	rotation = _angle
	queue_redraw()

## Chamado pelo Main a cada passo.
func set_thrust(on: bool) -> void:
	_thrust_on = on

func _process(delta: float) -> void:
	_thrust = lerpf(_thrust, _target_thrust, 1.0 - exp(-18.0 * delta))
	queue_redraw()

func _draw() -> void:
	if not _alive:
		return

	# Piscar durante a invulnerabilidade (respawn/hyperspace).
	if _invuln > 0.0 and fmod(_invuln, 0.24) < 0.12:
		return

	# Fantasmas nas bordas: o campo e toroidal, o desenho tambem tem de ser.
	for ghost in _ghost_offsets():
		_draw_ship(ghost)

func _ghost_offsets() -> Array:
	var arena: Vector2 = SIM.ARENA
	var offsets := [Vector2.ZERO]
	var margin := 24.0
	if _pos.x < margin:
		offsets.append(Vector2(arena.x, 0.0))
	elif _pos.x > arena.x - margin:
		offsets.append(Vector2(-arena.x, 0.0))
	if _pos.y < margin:
		offsets.append(Vector2(0.0, arena.y))
	elif _pos.y > arena.y - margin:
		offsets.append(Vector2(0.0, -arena.y))
	return offsets

func _draw_ship(offset: Vector2) -> void:
	# Halo aditivo atras do casco (o "brilho do fosforo" do canhao vetorial).
	var glow := Color(_color, 0.10 + 0.06 * sin(Time.get_ticks_msec() * 0.006))
	_draw_polyline(HULL, offset, glow, 9.0, true)

	if _show_thrust and _thrust > 0.05:
		var flame := Color(Art.STAR_GLOW, 0.55 * _thrust)
		var scaled := []
		for point in FLAME:
			scaled.append(point * (0.55 + 0.85 * _thrust))
		_draw_polyline(scaled, offset, flame, 3.0, true)

	_draw_polyline(HULL, offset, _color, 2.4, true)
	# Cabine: um ponto claro perto do nariz, para ler a direcao.
	draw_circle(Vector2(2.0, 0.0) + offset.rotated(-_angle), 2.2, Art.TEXT_PRIMARY)

func _draw_polyline(points: Array, offset: Vector2, color: Color, width: float,
		closed: bool) -> void:
	var packed := PackedVector2Array()
	# Os pontos estao no espaco local do navio; o node ja esta rotacionado,
	# entao o offset do fantasma precisa ser girado de volta.
	for point in points:
		packed.append(point + offset.rotated(-_angle))
	if closed:
		packed.append(packed[0])
	draw_polyline(packed, color, width, true)
