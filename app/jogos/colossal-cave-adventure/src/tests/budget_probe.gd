extends SceneTree
## Mede o consumo real do jogo contra o orcamento do aparelho.
##
##   DISPLAY=:0 godot --path godot --script res://tests/budget_probe.gd
##
## Precisa de contexto OpenGL (nao roda com --headless). Mede os tres presets:
## RAM pelo RSS do processo (/proc/self/status) e VRAM pelo monitor do
## renderizador. Sai com 1 se algum preset passar de 700 MB de RAM ou 128 MB de
## VRAM (RK3566, ver device_profile.json).

const RAM_LIMIT_MB := 700.0
const VRAM_LIMIT_MB := 128.0
const WARMUP_FRAMES := 90

var _scene
var _frame := 0
var _tier := 0
var _results: Array[String] = []
var _failed := false

func _initialize() -> void:
	_scene = load("res://scenes/Main.tscn").instantiate()
	root.add_child(_scene)

func _process(_delta: float) -> bool:
	_frame += 1
	if _frame == 1:
		_apply_tier(0)
		# Sem audio: o probe nao mede som e um playback vivo no `quit()` deixa
		# AudioStreamPlaybackWAV para tras ("ObjectDB instances leaked at exit").
		_scene._sfx.set_enabled(false)
		return false
	# Avanca a caverna para medir com o jogo em andamento, nao parado na entrada.
	if _frame % 30 == 0:
		_scene._act(_scene._autoplay.next())
	if _frame % WARMUP_FRAMES != 0:
		return false
	return _measure()

func _apply_tier(tier: int) -> void:
	_tier = tier
	_scene._tier = tier
	_scene._apply_quality()

func _measure() -> bool:
	var ram := _rss_mb()
	var vram: float = Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0
	var draws := int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
	var tier_name: String = Quality.NAMES[_tier]

	var status := "OK"
	if ram > RAM_LIMIT_MB or vram > VRAM_LIMIT_MB:
		status = "ESTOUROU"
		_failed = true
	_results.append("  %-6s RAM %6.1f MB   VRAM %5.2f MB   %3d draws   %s" % [
		tier_name, ram, vram, draws, status])

	if _tier + 1 < Quality.NAMES.size():
		_apply_tier(_tier + 1)
		return false

	print("orcamento (RK3566): RAM <= %.0f MB, VRAM <= %.0f MB" % [RAM_LIMIT_MB, VRAM_LIMIT_MB])
	print("medido no host com %s -- no aparelho, medir de novo (F3 no jogo)"
		% RenderingServer.get_video_adapter_name())
	for line in _results:
		print(line)
	print("ORCAMENTO ESTOURADO" if _failed else "ORCAMENTO OK")
	quit(1 if _failed else 0)
	return true

func _rss_mb() -> float:
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
