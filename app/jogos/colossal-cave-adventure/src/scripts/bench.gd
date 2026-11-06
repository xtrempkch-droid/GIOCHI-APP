class_name Bench
extends RefCounted
## Modo benchmark: roda o jogo com entrada determinística e emite um laudo.
##
## Uso (o que vem depois de `--` chega em OS.get_cmdline_user_args()):
##
##   godot --path <projeto> -- --bench --bench-frames 1800 --bench-out laudo.json
##
## Os quatro jogos padrão do sistema usam este mesmo módulo (cópia em cada
## projeto, como fixed_step.gd: cada jogo é um projeto Godot isolado) para que
## a medição no aparelho seja comparável entre eles.
##
## O laudo tem dois blocos:
##   * frames — distribuição do intervalo entre quadros (p50/p95/p99), fps
##     médio e o tempo de processo/física por quadro (o que o jogo gasta);
##   * budget — RAM (RSS real do processo), VRAM, draw calls e objetos contra
##     o teto do aparelho (700 MB de RAM / 128 MB de VRAM no RK3566).
##
## A entrada em bench é sempre sintética: nenhum humano aperta botão, senão a
## medição não é reprodutível. Cada jogo implementa o seu autoplay.

const SCHEMA_VERSION := "1"
const RAM_LIMIT_MB := 700.0
const VRAM_LIMIT_MB := 128.0
const DEFAULT_FRAMES := 1800
const DEFAULT_WARMUP := 120
const DEFAULT_SEED := 12345

var enabled := false
var game_id := ""
var frames := DEFAULT_FRAMES
var warmup := DEFAULT_WARMUP
var seed_value := DEFAULT_SEED
## -1 = usa o preset salvo pelo jogador; 0..2 = forca o preset (benchmark
## comparavel entre execucoes, independente do que esta salvo no aparelho).
var quality_tier := -1
var out_path := ""

var _samples: Array[float] = []
var _process_ms: Array[float] = []
var _physics_ms: Array[float] = []
var _last_usec := 0
var _counted := 0
var _finished := false

## Le a linha de comando. `default_game` identifica o laudo; `default_out`
## evita briga de nomes quando os quatro rodam em sequência.
static func from_cmdline(default_game: String, default_out: String = "") -> Bench:
	var bench := Bench.new()
	bench.game_id = default_game
	bench.out_path = default_out if default_out != "" else "user://bench-%s.json" % default_game

	var args := OS.get_cmdline_user_args()
	for i in args.size():
		var arg := args[i]
		match arg:
			"--bench":
				bench.enabled = true
			"--bench-frames":
				if i + 1 < args.size():
					bench.frames = maxi(60, args[i + 1].to_int())
			"--bench-warmup":
				if i + 1 < args.size():
					bench.warmup = maxi(0, args[i + 1].to_int())
			"--bench-seed":
				if i + 1 < args.size():
					bench.seed_value = args[i + 1].to_int()
			"--bench-quality":
				if i + 1 < args.size():
					bench.quality_tier = args[i + 1].to_int()
			"--bench-out":
				if i + 1 < args.size():
					bench.out_path = args[i + 1]
			_:
				if arg.begins_with("--bench-out="):
					bench.out_path = arg.substr("--bench-out=".length())
				elif arg.begins_with("--bench-frames="):
					bench.frames = maxi(60, arg.substr("--bench-frames=".length()).to_int())
				elif arg.begins_with("--bench-seed="):
					bench.seed_value = arg.substr("--bench-seed=".length()).to_int()
				elif arg.begins_with("--bench-quality="):
					bench.quality_tier = arg.substr("--bench-quality=".length()).to_int()
	return bench

## Chamado no começo de cada quadro. Devolve true quando a medição acabou.
func frame() -> bool:
	if not enabled or _finished:
		return false

	var now := Time.get_ticks_usec()
	if _last_usec != 0:
		var delta_ms := float(now - _last_usec) / 1000.0
		if _counted >= warmup:
			_samples.append(delta_ms)
			_process_ms.append(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0)
			_physics_ms.append(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0)
		_counted += 1
		_last_usec = now
		if _samples.size() >= frames:
			_finished = true
			return true
	else:
		_last_usec = now
	return false

## Escreve o laudo, imprime o resumo e devolve o código de saída (0 = dentro
## do orçamento).
func finish(config: Dictionary, viewport: Vector2, quality_label: String) -> int:
	var budget := _budget()
	var report := {
		"schema_version": SCHEMA_VERSION,
		"game": game_id,
		"engine": "godot-%s" % Engine.get_version_info()["string"],
		"timestamp_utc": Time.get_datetime_string_from_system(true),
		"device": _device(),
		"config": {
			"quality": quality_label,
			"viewport": [int(viewport.x), int(viewport.y)],
			"frames": frames,
			"warmup_frames": warmup,
			"seed": seed_value,
			"window_size": [
				int(DisplayServer.window_get_size().x),
				int(DisplayServer.window_get_size().y),
			],
		},
		"frames": {
			"p50_ms": _percentile(_samples, 0.50),
			"p95_ms": _percentile(_samples, 0.95),
			"p99_ms": _percentile(_samples, 0.99),
			"fps_mean": _fps_mean(),
			"process_ms_mean": _mean(_process_ms),
			"physics_ms_mean": _mean(_physics_ms),
			"over_33ms_ratio": _ratio_over(_samples, 33.0),
		},
		"budget": budget,
	}
	for key in config.keys():
		report["config"][key] = config[key]

	var json := JSON.stringify(report, "  ")
	# FileAccess so aceita res://, user:// e caminho ABSOLUTO: um caminho
	# relativo ("build/bench/x.json") falha aqui, e antes falhava calado --
	# o resumo era impresso e o laudo nunca existia.
	if not (out_path.begins_with("res://") or out_path.begins_with("user://")
			or out_path.begins_with("/")):
		push_error("bench: caminho de laudo precisa ser absoluto (recebido '%s')"
			% out_path)
		out_path = ProjectSettings.globalize_path(out_path)

	var file := FileAccess.open(out_path, FileAccess.WRITE)
	if file == null:
		push_error("bench: nao foi possivel escrever %s (erro %s)"
			% [out_path, error_string(FileAccess.get_open_error())])
		return 1
	file.store_string(json + "\n")
	file.close()

	print("bench %s: p50 %.1f ms  p95 %.1f ms  p99 %.1f ms  %.1f fps  %s" % [
		game_id, report["frames"]["p50_ms"], report["frames"]["p95_ms"],
		report["frames"]["p99_ms"], report["frames"]["fps_mean"],
		"OK" if budget["passed"] else "ESTOUROU",
	])
	print("bench %s: RAM %.1f MB  VRAM %.2f MB  %d draws  laudo em %s" % [
		game_id, budget["ram_mb"], budget["vram_mb"], budget["draw_calls"], out_path,
	])
	return 0 if budget["passed"] else 1

func _budget() -> Dictionary:
	var ram := _rss_mb()
	var vram := Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0
	var draws := int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
	var passed := ram <= RAM_LIMIT_MB and vram <= VRAM_LIMIT_MB
	return {
		"ram_mb": ram,
		"vram_mb": vram,
		"ram_limit_mb": RAM_LIMIT_MB,
		"vram_limit_mb": VRAM_LIMIT_MB,
		"draw_calls": draws,
		"objects": int(Performance.get_monitor(Performance.OBJECT_COUNT)),
		"nodes": int(Performance.get_monitor(Performance.OBJECT_NODE_COUNT)),
		"passed": passed,
	}

func _device() -> Dictionary:
	return {
		"os": OS.get_name(),
		"cpu": OS.get_processor_name(),
		"gpu": RenderingServer.get_video_adapter_name(),
		"api": RenderingServer.get_video_adapter_api_version(),
		"cores": OS.get_processor_count(),
		"ram_total_mb": _mem_total_mb(),
		"screen": [DisplayServer.screen_get_size().x, DisplayServer.screen_get_size().y],
	}

## RSS do processo em MB -- o número que o kernel cobra do aparelho.
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

static func _mem_total_mb() -> float:
	var file := FileAccess.open("/proc/meminfo", FileAccess.READ)
	if file == null:
		return -1.0
	while not file.eof_reached():
		var line := file.get_line().replace("\t", " ")
		if line.begins_with("MemTotal:"):
			var parts := line.split(" ", false)
			if parts.size() >= 2:
				return float(parts[1].to_int()) / 1024.0
	return -1.0

static func _mean(values: Array[float]) -> float:
	if values.is_empty():
		return 0.0
	var total := 0.0
	for value in values:
		total += value
	return total / float(values.size())

static func _percentile(values: Array[float], p: float) -> float:
	if values.is_empty():
		return 0.0
	var sorted := values.duplicate()
	sorted.sort()
	var index: int = clampi(int(round(p * (sorted.size() - 1))), 0, sorted.size() - 1)
	return sorted[index]

## Média do intervalo, invertida: é o fps médio real. A média das taxas
## instantâneas daria um número otimista (uma queda para 1 fps é diluída).
func _fps_mean() -> float:
	var mean := _mean(_samples)
	return 0.0 if mean <= 0.0 else 1000.0 / mean

static func _ratio_over(values: Array[float], threshold: float) -> float:
	if values.is_empty():
		return 0.0
	var over := 0
	for value in values:
		if value > threshold:
			over += 1
	return float(over) / float(values.size())
