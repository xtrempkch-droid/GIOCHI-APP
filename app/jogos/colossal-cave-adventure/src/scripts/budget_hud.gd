class_name BudgetHud
extends Label
## Painel de orcamento (F3): mostra o consumo REAL do processo contra o teto do
## aparelho -- 700 MB de RAM e 128 MB de VRAM (RK3566, ver device_profile.json).
##
## Copia do arquivo do Arcade Volleyball (cada jogo e um projeto Godot isolado).
##
## RAM vem de /proc/self/status (VmRSS), nao do contador interno do engine: e o
## que o kernel cobra do aparelho. VRAM vem do monitor do renderizador.

const RAM_LIMIT_MB := 700.0
const VRAM_LIMIT_MB := 128.0
const REFRESH_SECONDS := 0.25

var _elapsed := 0.0
var _warned_ram := false
var _warned_vram := false

func _ready() -> void:
	position = Vector2(14, 10)
	size = Vector2(680, 96)
	horizontal_alignment = HORIZONTAL_ALIGNMENT_LEFT
	vertical_alignment = VERTICAL_ALIGNMENT_TOP
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_theme_font_size_override("font_size", 14)
	add_theme_color_override("font_color", Art.TEXT_SECONDARY)
	add_theme_constant_override("outline_size", 6)
	add_theme_color_override("font_outline_color", Color(0.0, 0.0, 0.0, 0.9))
	text = "orçamento: coletando..."

func _process(delta: float) -> void:
	if not visible:
		return
	_elapsed += delta
	if _elapsed < REFRESH_SECONDS:
		return
	_elapsed = 0.0
	_refresh()

func _refresh() -> void:
	var ram_mb := _rss_mb()
	var vram_mb := Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0
	var draw_calls := int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
	var objects := int(Performance.get_monitor(Performance.OBJECT_COUNT))

	var ram_text := "n/d" if ram_mb < 0.0 else "%.0f" % ram_mb
	text = "RAM %s / %.0f MB    VRAM %.1f / %.0f MB    %d fps    %d draws    %d obj" % [
		ram_text, RAM_LIMIT_MB, vram_mb, VRAM_LIMIT_MB,
		Engine.get_frames_per_second(), draw_calls, objects
	]

	var over_ram := ram_mb >= 0.0 and ram_mb > RAM_LIMIT_MB
	var over_vram := vram_mb > VRAM_LIMIT_MB
	if over_ram or over_vram:
		add_theme_color_override("font_color", Color("ff6b5e"))
		# Um aviso por limite, nao um por atualizacao do painel.
		if over_ram and not _warned_ram:
			_warned_ram = true
			push_warning("orcamento: RAM %.0f MB acima do teto de %.0f MB" % [ram_mb, RAM_LIMIT_MB])
		if over_vram and not _warned_vram:
			_warned_vram = true
			push_warning("orcamento: VRAM %.1f MB acima do teto de %.0f MB" % [vram_mb, VRAM_LIMIT_MB])
	elif ram_mb >= 0.0 and ram_mb > RAM_LIMIT_MB * 0.8:
		add_theme_color_override("font_color", Color("e5c07b"))
	else:
		add_theme_color_override("font_color", Art.TEXT_SECONDARY)

## RSS do processo em MB. -1 quando /proc nao esta disponivel (nao-Linux).
static func _rss_mb() -> float:
	var file := FileAccess.open("/proc/self/status", FileAccess.READ)
	if file == null:
		return -1.0
	while not file.eof_reached():
		var line := file.get_line().replace("\t", " ")
		if line.begins_with("VmRSS:"):
			var parts := line.split(" ", false)
			if parts.size() >= 2:
				return float(parts[1].to_int()) / 1024.0
	return -1.0
