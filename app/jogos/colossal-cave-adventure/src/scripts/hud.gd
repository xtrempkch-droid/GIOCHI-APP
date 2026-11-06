class_name GameHud
extends CanvasLayer
## HUD do Colossal Cave: nome e descricao da sala, mensagem do turno,
## inventario, lampiao, pontuacao e uma bussola com as saidas.
##
## Animacoes em tempo real (Time.get_ticks_msec), nao com Tween: tween roda em
## tempo escalado e o efeito ficaria fora de sincronia (mesma licao dos outros
## dois jogos).

const CaveScript := preload("res://scripts/sim/cave.gd")
const LocaleScript := preload("res://scripts/i18n/locale.gd")

var _title: Label
var _text: Label
var _message: Label
var _inventory: Label
var _score: Label
var _lamp_bar: Control
var _lamp_label: Label
var _compass: Control
var _banner: Label
var _pause: Control
var _pause_quality: Label
var _pause_language: Label
var _pause_title: Label
var _pause_footer: Label
var _pause_selector: Label
var _budget: BudgetHud
var _flash: ColorRect

var locale: Locale

var _last_ms := 0
var _banner_t := -1.0
var _flash_alpha := 0.0
const BANNER_TIME := 1.1

func _ready() -> void:
	layer = 10
	_build_flash()
	_build_room_panel()
	_build_status()
	_build_compass()
	_build_banner()
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
	label.vertical_alignment = VERTICAL_ALIGNMENT_TOP
	label.mouse_filter = Control.MOUSE_FILTER_IGNORE
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)
	label.add_theme_constant_override("outline_size", maxi(3, size / 6))
	label.add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.9))
	return label

func _build_flash() -> void:
	_flash = ColorRect.new()
	_flash.color = Color(1.0, 1.0, 1.0, 0.0)
	_flash.size = Vector2(1280, 720)
	_flash.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_flash)

func _build_room_panel() -> void:
	# Faixa inferior translucida: o texto precisa de fundo para ler no escuro.
	var panel := ColorRect.new()
	panel.color = Color(0.02, 0.02, 0.02, 0.72)
	panel.position = Vector2(0, 486)
	panel.size = Vector2(1280, 234)
	panel.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(panel)

	_title = _label(Rect2(40, 496, 760, 40), 30, Art.LAMP, HORIZONTAL_ALIGNMENT_LEFT)
	_title.text = ""
	add_child(_title)

	_text = _label(Rect2(40, 538, 780, 96), 17, Art.TEXT_PRIMARY, HORIZONTAL_ALIGNMENT_LEFT)
	_text.text = ""
	_text.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_text)

	_message = _label(Rect2(40, 646, 780, 52), 17, Art.GOLD, HORIZONTAL_ALIGNMENT_LEFT)
	_message.text = ""
	_message.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_message)

func _build_status() -> void:
	_score = _label(Rect2(840, 496, 400, 28), 20, Art.TEXT_PRIMARY, HORIZONTAL_ALIGNMENT_RIGHT)
	_score.text = "0 pontos"
	add_child(_score)

	_lamp_label = _label(Rect2(840, 522, 400, 24), 15, Art.TEXT_SECONDARY,
		HORIZONTAL_ALIGNMENT_RIGHT)
	_lamp_label.text = "lampiao: apagado"
	add_child(_lamp_label)

	_lamp_bar = Control.new()
	_lamp_bar.position = Vector2(1000, 550)
	_lamp_bar.size = Vector2(240, 6)
	_lamp_bar.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_lamp_bar.set_script(preload("res://scripts/bar.gd"))
	add_child(_lamp_bar)

	_inventory = _label(Rect2(840, 566, 400, 120), 15, Art.TEXT_SECONDARY,
		HORIZONTAL_ALIGNMENT_RIGHT)
	_inventory.text = "carregando: nada"
	add_child(_inventory)

func _build_compass() -> void:
	_compass = Control.new()
	_compass.position = Vector2(1090, 372)
	_compass.size = Vector2(150, 130)
	_compass.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_compass.set_script(preload("res://scripts/compass.gd"))
	add_child(_compass)

func _build_banner() -> void:
	_banner = _label(Rect2(0, 240, 1280, 120), 82, Art.LAMP, HORIZONTAL_ALIGNMENT_CENTER)
	_banner.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_banner.text = ""
	_banner.modulate.a = 0.0
	_banner.pivot_offset = _banner.size * 0.5
	add_child(_banner)

## Monta o texto do HUD em outro idioma (a simulacao nao muda).
func set_locale(value: Locale) -> void:
	locale = value
	_refresh_static_text()

func _refresh_static_text() -> void:
	if locale == null:
		return
	if _pause_title:
		_pause_title.text = locale.text("pause")
	if _pause_footer:
		_pause_footer.text = "%s      ·      %s      ·      %s\n%s      ·      %s %s      ·      %s" % [
			locale.text("controls_move"), locale.text("controls_take"),
			locale.text("controls_drop"), locale.text("controls_lamp"),
			"← →", locale.text("graphics"), locale.text("resume"),
		]
	_set_quality_label()
	_set_language_label()

func _build_pause() -> void:
	_pause = Control.new()
	_pause.visible = false
	_pause.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_pause)

	var dim := ColorRect.new()
	dim.color = Color(0.0, 0.0, 0.0, 0.80)
	dim.size = Vector2(1280, 720)
	dim.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_pause.add_child(dim)

	_pause_title = _label(Rect2(0, 212, 1280, 80), 56, Art.TEXT_PRIMARY,
		HORIZONTAL_ALIGNMENT_CENTER)
	_pause_title.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_title.text = ""
	_pause.add_child(_pause_title)

	_pause_quality = _label(Rect2(0, 316, 1280, 36), 24, Art.LAMP,
		HORIZONTAL_ALIGNMENT_CENTER)
	_pause_quality.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_quality.text = ""
	_pause.add_child(_pause_quality)

	_pause_language = _label(Rect2(0, 362, 1280, 36), 24, Art.LAMP,
		HORIZONTAL_ALIGNMENT_CENTER)
	_pause_language.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_language.text = ""
	_pause.add_child(_pause_language)

	# Marcador de qual linha esta selecionada (graficos ou idioma).
	_pause_selector = _label(Rect2(430, 316, 30, 36), 24, Art.TEXT_PRIMARY,
		HORIZONTAL_ALIGNMENT_LEFT)
	_pause_selector.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_selector.text = "›"
	_pause.add_child(_pause_selector)

	_pause_footer = _label(Rect2(0, 430, 1280, 130), 17, Art.TEXT_SECONDARY,
		HORIZONTAL_ALIGNMENT_CENTER)
	_pause_footer.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_footer.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_pause_footer.text = ""
	_pause.add_child(_pause_footer)

# --- API usada pelo Main ------------------------------------------------

func sync(cave: Cave) -> void:
	if locale == null:
		# _ready() do HUD pode rodar antes do Main entregar o idioma.
		locale = LocaleScript.new()
	_title.text = locale.room_name(cave.room_id)
	_text.text = locale.room_text(cave.room_id)
	_message.text = _message_text(cave)

	_score.text = "%d %s   ·   %d %s   ·   %d/%d %s" % [
		cave.score, locale.text("points"), cave.turns, locale.text("turns"),
		cave.deposited, cave.total_treasures(), locale.text("treasures"),
	]

	var lamp: Dictionary = cave.lamp_state()
	_lamp_label.text = "%s: %s (%d)" % [
		locale.text("lamp"), locale.text(lamp["key"]), int(lamp["fuel"]),
	]
	_lamp_bar.ratio = float(cave.lamp_fuel) / float(CaveScript.LAMP_FUEL)
	_lamp_bar.lit = cave.lamp_on and cave.lamp_fuel > 0
	_lamp_bar.queue_redraw()

	if cave.inventory.is_empty():
		_inventory.text = locale.text("carrying")
	else:
		var names := []
		for item in cave.inventory:
			names.append(locale.item_name(item))
		_inventory.text = locale.text("carrying_prefix") + ", ".join(names)

	_compass.exits = cave.exit_dirs()
	_compass.queue_redraw()

## Resolve a chave de mensagem + argumentos (item e direcao sao traduzidos).
func _message_text(cave: Cave) -> String:
	if cave.message_key == "":
		return ""
	var args := {}
	for name in cave.message_args.keys():
		var value: String = str(cave.message_args[name])
		if name == "item":
			args[name] = locale.item_name(value)
		elif name == "dir":
			args[name] = locale.direction_name(value)
		else:
			args[name] = value
	return locale.message(cave.message_key, args)

func show_banner(text: String, color: Color = Art.LAMP) -> void:
	_banner.text = text
	_banner.add_theme_color_override("font_color", color)
	_banner.pivot_offset = _banner.size * 0.5
	_banner_t = 0.0

func flash(color: Color, strength: float = 0.20) -> void:
	_flash.color = color
	_flash_alpha = strength

func set_pause_visible(value: bool) -> void:
	_pause.visible = value

func set_quality_text(text: String) -> void:
	_quality_label_text = text
	_set_quality_label()

## Marca qual das duas linhas do menu (0 = graficos, 1 = idioma) esta ativa.
func set_menu_row(row: int) -> void:
	if _pause_selector:
		_pause_selector.position.y = 316.0 if row == 0 else 362.0

## Nome do idioma exibido no menu de pausa (o Main chama ao trocar).
func set_language_label(text: String) -> void:
	_language_label_text = text
	_set_language_label()

func _set_quality_label() -> void:
	if _pause_quality and locale:
		_pause_quality.text = "%s: %s" % [locale.text("graphics"), _quality_label_text]

func _set_language_label() -> void:
	if _pause_language and locale:
		_pause_language.text = "%s: %s" % [locale.text("language"), _language_label_text]

var _quality_label_text := "Médio"
var _language_label_text := "Português (Brasil)"

func toggle_budget() -> void:
	_budget.visible = not _budget.visible

# --- animacao em tempo real ---------------------------------------------

func _process(_delta: float) -> void:
	var now := Time.get_ticks_msec()
	var dt: float = clampf(float(now - _last_ms) / 1000.0, 0.0, 0.1)
	_last_ms = now

	if _banner_t >= 0.0:
		_banner_t += dt
		if _banner_t >= BANNER_TIME:
			_banner_t = -1.0
			_banner.modulate.a = 0.0
		elif _banner_t < 0.12:
			_banner.modulate.a = _banner_t / 0.12
			_banner.scale = Vector2.ONE * lerpf(0.9, 1.0, _banner_t / 0.12)
		elif _banner_t > BANNER_TIME - 0.35:
			_banner.modulate.a = (BANNER_TIME - _banner_t) / 0.35
		else:
			_banner.modulate.a = 1.0
			_banner.scale = Vector2.ONE

	_flash_alpha = maxf(0.0, _flash_alpha - dt * 2.2)
	_flash.color.a = _flash_alpha
