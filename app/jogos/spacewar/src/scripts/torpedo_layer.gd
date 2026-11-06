class_name TorpedoLayer
extends Node2D
## Desenha todos os torpedos num unico node.
##
## Um node por torpedo custaria alocacao e destruto a cada tiro; aqui so ha uma
## lista de rastros e um `_draw` por quadro. O rastro e arrastado manualmente
## porque o torpedo do modelo e recriado a cada disparo.

const SIM := preload("res://scripts/sim/spacewar.gd")

## Quantos pontos de rastro por torpedo (o preset baixo zera).
var trail_points := 10

## Uma entrada por torpedo vivo: {"torpedo": ref, "trail": PackedVector2Array}
var _entries: Array[Dictionary] = []

func clear() -> void:
	_entries.clear()
	queue_redraw()

## Sincroniza com a lista do modelo e redesenha.
func sync(torpedoes: Array) -> void:
	var kept: Array[Dictionary] = []
	for torpedo in torpedoes:
		var entry := {}
		for candidate in _entries:
			if candidate["torpedo"] == torpedo:
				entry = candidate
				break
		if entry.is_empty():
			entry = {"torpedo": torpedo, "trail": PackedVector2Array()}

		var trail: PackedVector2Array = entry["trail"]
		trail.append(torpedo.pos)
		if trail_points <= 0:
			trail = PackedVector2Array()
		else:
			while trail.size() > trail_points:
				trail.remove_at(0)
		entry["trail"] = trail
		kept.append(entry)

	_entries = kept
	queue_redraw()

func _draw() -> void:
	var arena: Vector2 = SIM.ARENA
	for entry in _entries:
		var torpedo = entry["torpedo"]
		var trail: PackedVector2Array = entry["trail"]
		var color: Color = Art.TORPEDO

		# Rastro: desenhado em segmentos de alpha decrescente (sem gradiente de
		# Line2D, que exigiria um node por torpedo).
		var count := trail.size()
		for i in range(1, count):
			var t := float(i) / float(count)
			draw_line(trail[i - 1], trail[i], Color(color, 0.55 * t), 1.0 + 2.0 * t, true)

		# Cabeca: nucleo claro + halo.
		draw_circle(torpedo.pos, SIM.TORPEDO_RADIUS, Art.TEXT_PRIMARY)

		# Fantasmas nas bordas do campo toroidal.
		if torpedo.pos.x < 12.0:
			draw_circle(torpedo.pos + Vector2(arena.x, 0.0), SIM.TORPEDO_RADIUS, Art.TEXT_PRIMARY)
		elif torpedo.pos.x > arena.x - 12.0:
			draw_circle(torpedo.pos - Vector2(arena.x, 0.0), SIM.TORPEDO_RADIUS, Art.TEXT_PRIMARY)
		if torpedo.pos.y < 12.0:
			draw_circle(torpedo.pos + Vector2(0.0, arena.y), SIM.TORPEDO_RADIUS, Art.TEXT_PRIMARY)
		elif torpedo.pos.y > arena.y - 12.0:
			draw_circle(torpedo.pos - Vector2(0.0, arena.y), SIM.TORPEDO_RADIUS, Art.TEXT_PRIMARY)
