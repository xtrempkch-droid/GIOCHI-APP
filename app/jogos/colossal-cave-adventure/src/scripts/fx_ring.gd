class_name FxRing
extends Node2D
## Anel de choque: circulo que expande e some. Uma chamada de desenho por
## quadro e nenhuma textura -- o "juice" mais barato que existe.
##
## Copia do arquivo do Arcade Volleyball: cada jogo e um projeto Godot isolado.

var color := Color.WHITE
var radius_start := 4.0
var radius_end := 90.0
var duration := 0.32
var thickness := 3.0

var _elapsed := 0.0

func setup(at: Vector2, tint: Color, end_radius: float, life: float) -> void:
	position = at
	color = tint
	radius_end = end_radius
	duration = life
	z_index = 2

func _process(delta: float) -> void:
	_elapsed += delta
	if _elapsed >= duration:
		queue_free()
		return
	queue_redraw()

func _draw() -> void:
	var t: float = clampf(_elapsed / duration, 0.0, 1.0)
	# Expande rapido no comeco e desacelera.
	var eased: float = 1.0 - pow(1.0 - t, 2.4)
	var radius: float = lerpf(radius_start, radius_end, eased)
	var alpha: float = (1.0 - t) * 0.85
	var width: float = thickness * (1.0 - t * 0.6)
	draw_arc(Vector2.ZERO, radius, 0.0, TAU, 40, Color(color, alpha), maxf(width, 1.0), true)
