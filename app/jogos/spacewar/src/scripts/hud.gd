class_name GameHud
extends CanvasLayer
## HUD em codigo: mortes de cada lado, carga do tubo, cooldown do hyperspace,
## banner, pausa e o painel de orcamento.
##
## As animacoes correm em TEMPO REAL (Time.get_ticks_msec), nao com Tween:
## tween em Godot 4 roda com delta escalado por Engine.time_scale, e o slow
## motion de um abate esticava o banner para fora de sincronia com o som.

var _score: Array[Label] = []
var _tag: Array[Label] = []
var _ammo: Array = []
var _hyper: Array = []
var _banner: Label
var _hint: Label
var _pause: Control
var _pause_quality: Label
var _budget: BudgetHud
var _flash: ColorRect

var _last_ms := 0
var _banner_t := -1.0
var _pop_t := [-1.0, -1.0]
var _flash_alpha := 0.0

const BANNER_IN := 0.10
const BANNER_HOLD := 0.70
const BANNER_OUT := 0.35
const POP_TIME := 0.22
## Pips da carga do tubo (um por torpedo do magazine).
const PIP_COUNT := 4

func _ready() -> void:
	layer = 10
	_build_flash()
	_build_score()
	_build_banner()
	_build_hint()
	_build_pause()
	_budget = BudgetHud.new()
	_budget.visible = false
	add_child(_budget)
	_last_ms = Time.get_ticks_msec()

func _label(rect: Rect2, size: int, color: Color, align: int) -> Label:
	var label := Label.new()
	label.position = rect.position
	label.size = rect.size
	label.horizontal_alignment = align
	label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	label.add_theme_constant_override("outline_size", maxi(4, size / 6))
	label.add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.85))
	return label

func _build_flash() -> void:
	_flash = ColorRect.new()
	_flash.color = Color(1.0, 1.0, 1.0, 0.0)
	_flash.position = Vector2.ZERO
	_flash.size = Vector2(1280, 720)
	_flash.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_flash)

func _build_score() -> void:
	# Esquerda = P1 (accent), direita = P2 (azul). Numero grande = abates.
	_score.append(_label(Rect2(300, 8, 240, 96), 76, Art.SHIP_1, HORIZONTAL_ALIGNMENT_RIGHT))
	_score.append(_label(Rect2(740, 8, 240, 96), 76, Art.SHIP_2, HORIZONTAL_ALIGNMENT_LEFT))
	for label in _score:
		label.text = "0"
		add_child(label)

	_tag.append(_label(Rect2(300, 96, 240, 22), 15, Art.SHIP_1, HORIZONTAL_ALIGNMENT_RIGHT))
	_tag[0].text = "P1"
	_tag.append(_label(Rect2(740, 96, 240, 22), 15, Art.SHIP_2, HORIZONTAL_ALIGNMENT_LEFT))
	_tag[1].text = "P2"
	for label in _tag:
		label.modulate.a = 0.8
		add_child(label)

	# Carga do tubo: pips quadrados, um por torpedo disponivel.
	for side in 2:
		var rect := Rect2(392 if side == 0 else 828, 122, 100, 8)
		var node := Control.new()
		node.position = rect.position
		node.size = rect.size
		node.mouse_filter = Control.MOUSE_FILTER_IGNORE
		node.set_script(preload("res://scripts/ammo_row.gd"))
		node.side = side
		add_child(node)
		_ammo.append(node)

		# Cooldown do hyperspace: barra fina embaixo dos pips.
		var hyper := Control.new()
		hyper.position = Vector2(rect.position.x, rect.position.y + 12.0)
		hyper.size = Vector2(rect.size.x, 5.0)
		hyper.mouse_filter = Control.MOUSE_FILTER_IGNORE
		hyper.set_script(preload("res://scripts/hyper_row.gd"))
		hyper.side = side
		add_child(hyper)
		_hyper.append(hyper)

func _build_banner() -> void:
	_banner = _label(Rect2(0, 246, 1280, 140), 96, Art.ACCENT, HORIZONTAL_ALIGNMENT_CENTER)
	_banner.text = ""
	_banner.modulate.a = 0.0
	_banner.pivot_offset = _banner.size * 0.5
	add_child(_banner)

func _build_hint() -> void:
	_hint = _label(Rect2(0, 678, 1280, 26), 15, Art.TEXT_SECONDARY, HORIZONTAL_ALIGNMENT_CENTER)
	_hint.text = "P1: A/D girar · W impulso · S tiro · Q hyperspace   |   ESC pausa"
	_hint.modulate.a = 0.8
	add_child(_hint)

func _build_pause() -> void:
	_pause = Control.new()
	_pause.visible = false
	_pause.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_pause)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.78)
	dim.position = Vector2.ZERO
	dim.size = Vector2(1280, 720)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_pause.add_child(dim)

	var title := _label(Rect2(0, 232, 1280, 90), 64, Art.TEXT_PRIMARY, HORIZONTAL_ALIGNMENT_CENTER)
	title.text = "PAUSA"
	title.modulate.a = 1.0
	_pause.add_child(title)

	_pause_quality = _label(Rect2(0, 330, 1280, 40), 26, Art.ACCENT, HORIZONTAL_ALIGNMENT_CENTER)
	_pause_quality.text = "Gráficos: Médio"
	_pause.add_child(_pause_quality)

	var footer := _label(Rect2(0, 384, 1280, 30), 18, Art.TEXT_SECONDARY, HORIZONTAL_ALIGNMENT_CENTER)
	footer.text = "← →  gráficos      ·      ESC  continuar      ·      Q  sair"
	_pause.add_child(footer)

# --- API usada pelo Main ------------------------------------------------

func set_score(side: int, value: int, pop: bool = true) -> void:
	var index := clampi(side, 0, 1)
	var label: Label = _score[index]
	label.text = str(value)
	if not pop:
		return
	label.pivot_offset = label.size * 0.5
	_pop_t[index] = 0.0

func set_tag(side: int, text: String) -> void:
	_tag[clampi(side, 0, 1)].text = text

func sync_ship(side: int, ship) -> void:
	var index := clampi(side, 0, 1)
	_ammo[index].ammo = ship.ammo
	_ammo[index].max_ammo = 4
	_ammo[index].reload = ship.reload
	_ammo[index].queue_redraw()
	_hyper[index].ratio = 1.0 - clampf(ship.hyper_cooldown / 4.0, 0.0, 1.0)
	_hyper[index].queue_redraw()

func show_banner(text: String, color: Color) -> void:
	_banner.text = text
	_banner.add_theme_color_override("font_color", color)
	_banner.pivot_offset = _banner.size * 0.5
	_banner_t = 0.0

func flash(color: Color, strength: float = 0.22) -> void:
	_flash.color = color
	_flash_alpha = strength

func set_pause_visible(value: bool) -> void:
	_pause.visible = value
	_hint.modulate.a = 0.0 if value else 0.8

func set_quality_text(text: String) -> void:
	_pause_quality.text = "Gráficos: %s" % text

func toggle_budget() -> void:
	_budget.visible = not _budget.visible

# --- animacao em tempo real ---------------------------------------------

func _process(_delta: float) -> void:
	var now := Time.get_ticks_msec()
	var dt: float = clampf(float(now - _last_ms) / 1000.0, 0.0, 0.1)
	_last_ms = now

	_update_banner(dt)
	_update_pop(dt)

	_flash_alpha = maxf(0.0, _flash_alpha - dt * 2.6)
	_flash.color.a = _flash_alpha

func _update_banner(dt: float) -> void:
	if _banner_t < 0.0:
		return
	_banner_t += dt
	var t := _banner_t
	var total := BANNER_IN + BANNER_HOLD + BANNER_OUT

	if t >= total:
		_banner_t = -1.0
		_banner.modulate.a = 0.0
		return

	if t < BANNER_IN:
		var k := t / BANNER_IN
		_banner.modulate.a = k
		_banner.scale = Vector2.ONE * lerpf(0.88, 1.0, _ease_out_back(k))
	elif t < BANNER_IN + BANNER_HOLD:
		_banner.modulate.a = 1.0
		_banner.scale = Vector2.ONE
	else:
		var k := (t - BANNER_IN - BANNER_HOLD) / BANNER_OUT
		_banner.modulate.a = 1.0 - k
		_banner.scale = Vector2.ONE

func _update_pop(dt: float) -> void:
	for i in 2:
		if _pop_t[i] < 0.0:
			continue
		_pop_t[i] += dt
		if _pop_t[i] >= POP_TIME:
			_pop_t[i] = -1.0
			_score[i].scale = Vector2.ONE
			continue
		var k: float = _pop_t[i] / POP_TIME
		_score[i].scale = Vector2.ONE * (1.0 + 0.24 * _ease_out_back(1.0 - k))

## Efeito de "volta" do ultimo trecho (o TRANS_BACK do Tween, a mao).
func _ease_out_back(k: float) -> float:
	var c1 := 1.70158
	var c3 := c1 + 1.0
	return 1.0 + c3 * pow(k - 1.0, 3.0) + c1 * pow(k - 1.0, 2.0)
