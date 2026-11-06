extends Control
## Barra do cooldown do hyperspace: cheia = pronto, vazia = recarregando.

var side := 0
var ratio := 1.0

func _draw() -> void:
	var color: Color = Art.ship_color(side)
	draw_rect(Rect2(0.0, 0.0, size.x, size.y), Color(color, 0.18))
	var width := size.x * clampf(ratio, 0.0, 1.0)
	if width > 0.5:
		# Pronto (ratio 1) fica mais claro.
		var alpha := 0.95 if ratio >= 0.999 else 0.55
		draw_rect(Rect2(0.0, 0.0, width, size.y), Color(color, alpha))
