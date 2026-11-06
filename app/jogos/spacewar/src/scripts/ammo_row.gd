extends Control
## Pips da carga do tubo de torpedos. Um quadrado por torpedo no magazine;
## durante a recarga o pip apagado pulsa, para o jogador ver que o tubo esta
## voltando.

var side := 0
var ammo := 4
var max_ammo := 4
var reload := 0.0

func _draw() -> void:
	var color: Color = Art.ship_color(side)
	var pip_width := 18.0
	var gap := 6.0
	var filled := clampi(ammo, 0, max_ammo)
	var pulse := 0.35 + 0.35 * sin(Time.get_ticks_msec() * 0.02)

	for i in max_ammo:
		var x := float(i) * (pip_width + gap)
		if i < filled:
			draw_rect(Rect2(x, 0.0, pip_width, 7.0), color)
		else:
			# Vazio: contorno. Se o tubo esta recarregando, pulsa.
			var alpha := 0.28 if reload <= 0.0 else pulse
			draw_rect(Rect2(x, 0.0, pip_width, 7.0), Color(color, alpha), false, 1.0)
