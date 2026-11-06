class_name BallView
extends Node2D
## Bola: circulo com brilho especular, anel de accent e squash no impacto.
## O rastro (Line2D) e o halo aditivo ficam fora deste node, em Main.

const Match := preload("res://scripts/sim/match.gd")

var _squash := Vector2.ONE
var _spin := 0.0

func sync(ball) -> void:
	position = ball.pos
	# Gira conforme a velocidade horizontal, so para dar vida ao anel.
	_spin += ball.vel.x * 0.01
	queue_redraw()

func punch(strength: float) -> void:
	_squash = Vector2(1.0 + strength * 1.4, 1.0 - strength * 1.4)

func _process(delta: float) -> void:
	_squash = _squash.lerp(Vector2.ONE, 1.0 - exp(-16.0 * delta))
	queue_redraw()

func _draw() -> void:
	var radius: float = Match.BALL_RADIUS
	draw_set_transform(Vector2.ZERO, 0.0, _squash)

	# Corpo.
	draw_circle(Vector2.ZERO, radius, Art.TEXT_PRIMARY)
	# Sombra interna (embaixo/direita) para dar volume.
	draw_arc(Vector2.ZERO, radius - 3.0, 0.6, 3.6, 24, Color(0.0, 0.0, 0.0, 0.18), 3.0, true)
	# Brilho especular.
	draw_circle(Vector2(-radius * 0.32, -radius * 0.34), radius * 0.34, Color(1.0, 1.0, 1.0, 0.9))
	# Anel de accent: identidade visual do tema.
	draw_arc(Vector2.ZERO, radius - 1.5, _spin, _spin + TAU, 32, Art.ACCENT, 2.5, true)

	draw_set_transform(Vector2.ZERO, 0.0, Vector2.ONE)
