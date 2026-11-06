extends Control
## Bussola das saidas: um ponto central e uma marca por direcao disponivel.
## E o unico jeito de o jogador saber para onde pode ir sem decorar o mapa.

var exits: Array = []

const MARKS := {
	"n": Vector2(0.0, -1.0),
	"s": Vector2(0.0, 1.0),
	"l": Vector2(1.0, 0.0),
	"o": Vector2(-1.0, 0.0),
}

func _draw() -> void:
	var center := size * 0.5
	var radius: float = minf(size.x, size.y) * 0.42

	draw_circle(center, radius, Color(0.0, 0.0, 0.0, 0.45))
	draw_arc(center, radius, 0.0, TAU, 36, Color(Art.DIVIDER, 0.9), 1.5, true)
	draw_circle(center, 3.0, Art.TEXT_SECONDARY)

	for dir in MARKS.keys():
		var direction: Vector2 = MARKS[dir]
		var point := center + direction * radius * 0.72
		if exits.has(dir):
			draw_circle(point, 7.0, Art.LAMP)
			draw_circle(point, 3.0, Color(1.0, 0.95, 0.8))
		else:
			draw_circle(point, 3.0, Color(Art.DIVIDER, 0.9))

	# Cima/baixo: setas no eixo vertical, deslocadas para nao brigar com n/s.
	for dir in ["c", "b"]:
		if not exits.has(dir):
			continue
		var y := center.y - radius * 1.02 if dir == "c" else center.y + radius * 1.02
		var arrow := PackedVector2Array([
			Vector2(center.x - 8.0, y), Vector2(center.x + 8.0, y),
			Vector2(center.x, y - 10.0 if dir == "c" else y + 10.0),
		])
		draw_colored_polygon(arrow, Art.LAMP)
