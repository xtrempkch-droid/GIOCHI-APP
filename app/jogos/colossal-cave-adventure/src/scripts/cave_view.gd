class_name CaveView
extends Node2D
## Vista em 2,5D da sala: passagens como poligonos em perspectiva e os objetos
## do chao como billboards, escalados pela profundidade.
##
## Nao ha modelo 3D: o "3D" e perspectiva de um ponto de fuga no horizonte,
## que e o que faz o jogador ler a sala como espaco.

const CaveScript := preload("res://scripts/sim/cave.gd")

## Horizonte na tela: onde o piso encontra a parede do fundo (mesmo valor do
## uniform `horizon` do shader da caverna).
const HORIZON_Y := 396.0
const CENTER_X := 640.0

## Posicoes dos itens no piso, por ordem: (x em px do centro, profundidade).
## Profundidade 0 = colado na parede do fundo, 1 = nos pes do jogador.
const ITEM_SLOTS := [
	Vector2(-150.0, 0.28),
	Vector2(150.0, 0.36),
	Vector2(-70.0, 0.55),
	Vector2(80.0, 0.62),
	Vector2(0.0, 0.78),
]

var cave: Cave
var light := 0.0
var reveal := 1.0

func set_room(cave_ref: Cave) -> void:
	cave = cave_ref
	queue_redraw()

## `amount` 0..1: quanto da sala ja foi "revelado" (transicao de passo).
func set_reveal(amount: float) -> void:
	reveal = clampf(amount, 0.0, 1.0)
	queue_redraw()

func set_light(amount: float) -> void:
	light = clampf(amount, 0.0, 1.0)
	queue_redraw()

func _draw() -> void:
	if cave == null:
		return
	_draw_exits()
	_draw_items()

## --- passagens ---------------------------------------------------------

func _draw_exits() -> void:
	var exits: Dictionary = cave.exits()
	# Passagem do fundo (norte): pequena, no centro do horizonte.
	if exits.has("n"):
		_draw_back_opening(0.16 + 0.08 * reveal)
	if exits.has("s"):
		# Ao sul o buraco fica atras de nos: desenha na base da tela.
		_draw_floor_opening(1.0)
	if exits.has("l"):
		_draw_side_opening(1.0, 0.72)
	if exits.has("o"):
		_draw_side_opening(-1.0, 0.72)
	if exits.has("c"):
		_draw_ceiling_opening()
	if exits.has("b"):
		_draw_floor_opening(0.55)

func _opening_color() -> Color:
	# A passagem e mais escura que a parede; com luz, ganha contorno quente.
	return Color(0.03, 0.03, 0.04, 0.95)

func _draw_back_opening(scale_factor: float) -> void:
	var half_w := 96.0 * scale_factor
	var half_h := 118.0 * scale_factor
	var top := HORIZON_Y - half_h
	var bottom := HORIZON_Y + half_h * 0.35
	var poly := PackedVector2Array([
		Vector2(CENTER_X - half_w * 0.8, top),
		Vector2(CENTER_X + half_w * 0.8, top),
		Vector2(CENTER_X + half_w, bottom),
		Vector2(CENTER_X - half_w, bottom),
	])
	draw_colored_polygon(poly, _opening_color())
	# Contorno iluminado pela direita (de onde vem a luz do lampiao).
	var rim := Color(Art.LAMP, 0.5 * light)
	draw_polyline(PackedVector2Array([poly[0], poly[1], poly[2]]), rim, 2.0, true)

func _draw_side_opening(side: float, depth: float) -> void:
	var edge := CENTER_X + side * 720.0
	var near_top := HORIZON_Y - 300.0
	var near_bottom := HORIZON_Y + 300.0
	var far_top := HORIZON_Y - 120.0 * depth
	var far_bottom := HORIZON_Y + 130.0 * depth
	var far_x := CENTER_X + side * 300.0

	var poly := PackedVector2Array([
		Vector2(edge, near_top),
		Vector2(far_x, far_top),
		Vector2(far_x, far_bottom),
		Vector2(edge, near_bottom),
	])
	draw_colored_polygon(poly, _opening_color())
	draw_polyline(PackedVector2Array([poly[0], poly[1], poly[2], poly[3]]),
		Color(Art.LAMP, 0.25 * light), 2.0, true)

func _draw_ceiling_opening() -> void:
	var half_w := 110.0
	var poly := PackedVector2Array([
		Vector2(CENTER_X - half_w, 0.0),
		Vector2(CENTER_X + half_w, 0.0),
		Vector2(CENTER_X + half_w * 0.5, 120.0),
		Vector2(CENTER_X - half_w * 0.5, 120.0),
	])
	draw_colored_polygon(poly, _opening_color())
	# Luz do dia entrando pela abertura.
	if light > 0.05:
		draw_colored_polygon(poly, Color(0.55, 0.62, 0.75, 0.12 * light))

func _draw_floor_opening(intensity: float) -> void:
	var half_w := 130.0 * intensity
	var depth_h := 90.0 * intensity
	var poly := PackedVector2Array([
		Vector2(CENTER_X - half_w, 720.0),
		Vector2(CENTER_X + half_w, 720.0),
		Vector2(CENTER_X + half_w * 0.45, 720.0 - depth_h),
		Vector2(CENTER_X - half_w * 0.45, 720.0 - depth_h),
	])
	draw_colored_polygon(poly, _opening_color())

## --- objetos -----------------------------------------------------------

func _draw_items() -> void:
	var items: Array = cave.items_here()
	if items.is_empty():
		return

	for index in items.size():
		if index >= ITEM_SLOTS.size():
			break
		var slot: Vector2 = ITEM_SLOTS[index]
		_draw_item(items[index], slot.x, slot.y)

func _draw_item(item_id: String, offset_x: float, depth: float) -> void:
	# Escala por profundidade: perto = grande, longe = pequeno.
	var scale := lerpf(0.45, 1.25, depth) * lerpf(0.85, 1.0, reveal)
	var y := lerpf(HORIZON_Y + 10.0, 700.0, depth)
	var x := CENTER_X + offset_x * lerpf(0.55, 1.15, depth)
	var size := 26.0 * scale
	var color := Art.item_color(item_id)

	# Sombra no piso.
	draw_ellipse(Vector2(x, y + size * 0.55), size * 0.55, size * 0.20,
		Color(0.0, 0.0, 0.0, 0.45))

	if bool(Cave.ITEMS[item_id]["treasure"]):
		# Tesouro: losango com brilho (le como "pegue isto").
		var poly := PackedVector2Array([
			Vector2(x, y - size), Vector2(x + size * 0.7, y),
			Vector2(x, y + size * 0.6), Vector2(x - size * 0.7, y),
		])
		draw_colored_polygon(poly, color)
		draw_polyline(PackedVector2Array([poly[0], poly[1], poly[2], poly[3], poly[0]]),
			Color(1.0, 1.0, 1.0, 0.55), 1.5, true)
		# Halo aditivo.
		draw_circle(Vector2(x, y), size * 1.6, Color(color, 0.10 * maxf(light, 0.4)))
	else:
		# Ferramenta: caixa com contorno.
		var rect := Rect2(x - size * 0.5, y - size * 0.7, size, size * 1.1)
		draw_rect(rect, color)
		draw_rect(rect, Color(0.0, 0.0, 0.0, 0.55), false, 1.5)

func draw_ellipse(center: Vector2, radius_x: float, radius_y: float, color: Color) -> void:
	var points := PackedVector2Array()
	for i in 24:
		var angle := TAU * float(i) / 24.0
		points.append(center + Vector2(cos(angle) * radius_x, sin(angle) * radius_y))
	draw_colored_polygon(points, color)
