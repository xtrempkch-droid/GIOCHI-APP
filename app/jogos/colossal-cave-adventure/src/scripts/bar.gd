extends Control
## Barra generica (usada para a carga do lampiao).

var ratio := 1.0
var lit := false

func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), Color(Art.DIVIDER, 0.7))
	var width := size.x * clampf(ratio, 0.0, 1.0)
	if width <= 0.5:
		return
	var color := Art.LAMP if lit else Art.TEXT_SECONDARY
	# Abaixo de 20% a barra pisca: e o aviso de que a luz vai acabar.
	if ratio < 0.2 and lit:
		var pulse := 0.55 + 0.45 * sin(Time.get_ticks_msec() * 0.012)
		color = color * Color(1.0, 1.0, 1.0, pulse)
	draw_rect(Rect2(0.0, 0.0, width, size.y), color)
