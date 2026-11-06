class_name NetView
extends Node2D
## Rede: postes, malha e fita superior. Desenhada com vetores e sem textura.

const Match := preload("res://scripts/sim/match.gd")

func _ready() -> void:
	position = Vector2(Match.NET_X, Match.FLOOR_Y - Match.NET_HEIGHT)
	queue_redraw()

func _draw() -> void:
	var width: float = Match.NET_WIDTH
	var height: float = Match.NET_HEIGHT

	# Sombra no piso.
	draw_rect(Rect2(width * 0.5 - 40.0, height, 80.0, 6.0), Color(0.0, 0.0, 0.0, 0.45))

	# Malha: linhas horizontais e verticais bem apagadas.
	var mesh := Color(Art.TEXT_SECONDARY, 0.28)
	var step := 22.0
	var y := 10.0
	while y < height - 6.0:
		draw_line(Vector2(1.0, y), Vector2(width - 1.0, y), mesh, 1.0)
		y += step
	var x := 2.0
	while x < width:
		draw_line(Vector2(x, 8.0), Vector2(x, height - 4.0), mesh, 1.0)
		x += step * 0.5

	# Postes.
	draw_rect(Rect2(0.0, 0.0, 3.0, height), Art.DIVIDER)
	draw_rect(Rect2(width - 3.0, 0.0, 3.0, height), Art.DIVIDER)

	# Fita superior em accent: a "boca" da rede.
	draw_rect(Rect2(-2.0, 0.0, width + 4.0, 7.0), Art.ACCENT)
	draw_rect(Rect2(-2.0, 0.0, width + 4.0, 2.0), Art.ACCENT_WARM)
