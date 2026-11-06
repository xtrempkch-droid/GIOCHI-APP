class_name GameHud
extends CanvasLayer
## HUD construido em codigo (placar, banner, dica, pausa e o painel de
## orcamento). Um so arquivo define posicao e paleta: o .tscn fica com a
## estrutura e nada de estilo espalhado.
##
## As animacoes correm em TEMPO REAL (medido por Time.get_ticks_msec), nao com
## Tween: tween em Godot 4 roda com delta escalado por Engine.time_scale, e o
## slow motion de um ponto esticava o banner para 2 s de relogio -- ele
## aparecia atrasado, fora de sincronia com o som e com o flash.

var _score: Array[Label] = []
var _separator: Label
var _rule: Label
var _banner: Label
var _hint: Label
var _pause: Control
var _pause_quality: Label
var _budget: BudgetHud
var _flash: ColorRect

## Tempo real (s) desde o inicio de cada animacao; negativo = parada.
var _last_ms := 0
var _banner_t := -1.0
var _pop_t := [-1.0, -1.0]
var _flash_alpha := 0.0

const BANNER_IN := 0.10
const BANNER_HOLD := 0.70
const BANNER_OUT := 0.35
const POP_TIME := 0.22

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

func _build_score() -> void:
	# Placar com a cor de cada lado: vermelho do P1, claro do P2.
	var left := _label(Rect2(360, 8, 240, 100), 78, Art.ACCENT, HORIZONTAL_ALIGNMENT_RIGHT)
	left.text = "0"
	add_child(left)
	_score.append(left)

	_separator = _label(Rect2(602, 24, 76, 72), 44, Art.DIVIDER, HORIZONTAL_ALIGNMENT_CENTER)
	_separator.text = "×"
	add_child(_separator)

	var right := _label(Rect2(680, 8, 240, 100), 78, Art.TEXT_PRIMARY, HORIZONTAL_ALIGNMENT_LEFT)
	right.text = "0"
	add_child(right)
	_score.append(right)

	var tag_left := _label(Rect2(360, 102, 240, 22), 15, Art.ACCENT, HORIZONTAL_ALIGNMENT_RIGHT)
	tag_left.text = "VOCÊ"
	tag_left.modulate.a = 0.85
	add_child(tag_left)

	var tag_right := _label(Rect2(680, 102, 240, 22), 15, Art.TEXT_SECONDARY, HORIZONTAL_ALIGNMENT_LEFT)
	tag_right.text = "CPU"
	tag_right.modulate.a = 0.85
	add_child(tag_right)

	_rule = _label(Rect2(440, 126, 400, 24), 15, Art.TEXT_SECONDARY, HORIZONTAL_ALIGNMENT_CENTER)
	_rule.text = "PRIMEIRO A 15"
	_rule.modulate.a = 0.7
	add_child(_rule)

## Flash de tela (ponto/impacto forte). Barato: um ColorRect com alpha animado.
func _build_flash() -> void:
	_flash = ColorRect.new()
	_flash.color = Color(1.0, 1.0, 1.0, 0.0)
	_flash.position = Vector2.ZERO
	_flash.size = Vector2(1280, 720)
	_flash.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_flash)

func _build_banner() -> void:
	_banner = _label(Rect2(0, 232, 1280, 150), 104, Art.ACCENT, HORIZONTAL_ALIGNMENT_CENTER)
	_banner.text = ""
	_banner.modulate.a = 0.0
	_banner.pivot_offset = _banner.size * 0.5
	add_child(_banner)

func _build_hint() -> void:
	_hint = _label(Rect2(0, 676, 1280, 26), 16, Art.TEXT_SECONDARY, HORIZONTAL_ALIGNMENT_CENTER)
	_hint.text = "A/D mover   ·   W pular   ·   ESC pausa"
	_hint.modulate.a = 0.85
	add_child(_hint)

func _build_pause() -> void:
	_pause = Control.new()
	_pause.visible = false
	_pause.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_pause)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.74)
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
	_hint.modulate.a = 0.0 if value else 0.85

func _process(_delta: float) -> void:
	# Delta em tempo real: o slow motion do jogo nao pode esticar a UI.
	var now := Time.get_ticks_msec()
	var dt: float = clampf(float(now - _last_ms) / 1000.0, 0.0, 0.1)
	_last_ms = now

	_update_banner(dt)
	_update_pop(dt)

	# Decaimento do flash.
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

func set_quality_text(name: String) -> void:
	_pause_quality.text = "Gráficos: %s" % name

## Some com a dica de controles (o launcher chama depois dos primeiros segundos).
func hide_hint() -> void:
	_hint.modulate.a = 0.0

func toggle_budget() -> void:
	_budget.visible = not _budget.visible
