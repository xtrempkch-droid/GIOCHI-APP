class_name SfxKit
extends Node
## Efeitos sonoros gerados por codigo: nenhum .wav entra no pacote.
##
## Cada som e um AudioStreamWAV montado no boot (poucos milhares de amostras,
## ~5 KB cada) e tocado por um pool fixo de AudioStreamPlayer. Sem placa de
## audio (ou com o driver Dummy) nada quebra: play() so nao produz som.
##
## Copia adaptada do SfxKit dos outros jogos: mesmos tiros de sintese, sons
## proprios deste jogo (passo no cascalho, lampiao acendendo, queda).

const RATE := 22050
const VOICES := 8

var _players: Array[AudioStreamPlayer] = []
var _streams: Dictionary = {}
var _next := 0
var _enabled := true

func _ready() -> void:
	for i in VOICES:
		var player := AudioStreamPlayer.new()
		player.bus = "Master"
		add_child(player)
		_players.append(player)

	_streams["step"] = _noise(0.10, 0.35, 0.06)
	_streams["take"] = _tone(520.0, 780.0, 0.10, 0.40, 2)
	_streams["drop"] = _tone(400.0, 260.0, 0.10, 0.35, 1)
	_streams["lamp"] = _tone(320.0, 640.0, 0.16, 0.38, 2)
	_streams["fall"] = _noise(0.55, 0.80, 0.05)
	_streams["deposit"] = _arpeggio([523.25, 659.25, 880.0], 0.09, 0.5)
	_streams["denied"] = _tone(150.0, 90.0, 0.08, 0.35, 1)
	_streams["win"] = _arpeggio([523.25, 659.25, 783.99, 1046.5], 0.14, 0.55)
	_streams["ui"] = _tone(900.0, 1100.0, 0.05, 0.25, 1)

func play(sound: String, pitch: float = 1.0) -> void:
	if not _enabled:
		return
	var stream: AudioStream = _streams.get(sound)
	if stream == null:
		push_warning("sfx: som desconhecido '%s'" % sound)
		return
	var player := _players[_next]
	_next = (_next + 1) % _players.size()

	player.stream = stream
	player.pitch_scale = clampf(pitch, 0.5, 2.0)
	player.play()

## Usado pelos testes headless (nao ha dispositivo de audio).
func set_enabled(value: bool) -> void:
	_enabled = value

func sounds() -> Array:
	return _streams.keys()

func _exit_tree() -> void:
	# Sem isso o AudioServer mantem o AudioStreamPlaybackWAV vivo e o engine
	# avisa "ObjectDB instances leaked at exit" ao fechar no meio de um som.
	for player in _players:
		player.stop()
		player.stream = null
	_streams.clear()

# --- sintese -------------------------------------------------------------

func _tone(f0: float, f1: float, seconds: float, gain: float, harmonics: int) -> AudioStreamWAV:
	var count := int(RATE * seconds)
	var data := PackedByteArray()
	data.resize(count * 2)

	var phase := 0.0
	for i in count:
		var t := float(i) / float(count)
		var freq: float = lerpf(f0, f1, t)
		phase += TAU * freq / float(RATE)
		# Ataque curto + decaimento: sem zero no comeco nao ha "clique".
		var env: float = pow(1.0 - t, 2.2) * minf(1.0, t * 40.0)
		var sample := sin(phase)
		if harmonics >= 2:
			sample += 0.35 * sin(phase * 2.0)
		if harmonics >= 3:
			sample += 0.18 * sin(phase * 3.0)
		data.encode_s16(i * 2, int(clampf(sample * env * gain, -1.0, 1.0) * 32767.0))

	return _wav(data)

## Ruido com passa-baixa e cauda longa: o estrondo da explosao.
func _noise(seconds: float, gain: float, damping: float) -> AudioStreamWAV:
	var count := int(RATE * seconds)
	var data := PackedByteArray()
	data.resize(count * 2)

	var state := 987654321
	var low := 0.0
	for i in count:
		var t := float(i) / float(count)
		state = (state * 1103515245 + 12345) & 0x7FFFFFFF
		var white := (float(state) / float(0x7FFFFFFF)) * 2.0 - 1.0
		low = lerpf(low, white, damping)
		var env: float = pow(1.0 - t, 2.6) * minf(1.0, t * 60.0)
		data.encode_s16(i * 2, int(clampf(low * env * gain, -1.0, 1.0) * 32767.0))

	return _wav(data)

func _arpeggio(notes: Array, note_seconds: float, gain: float) -> AudioStreamWAV:
	var per_note := int(RATE * note_seconds)
	var count := per_note * notes.size()
	var data := PackedByteArray()
	data.resize(count * 2)

	var phase := 0.0
	for i in count:
		var note_index: int = mini(i / per_note, notes.size() - 1)
		var local := float(i % per_note) / float(per_note)
		var freq: float = notes[note_index]
		phase += TAU * freq / float(RATE)
		var env: float = pow(1.0 - local, 1.8) * minf(1.0, local * 30.0)
		var sample := sin(phase) + 0.3 * sin(phase * 2.0)
		data.encode_s16(i * 2, int(clampf(sample * env * gain, -1.0, 1.0) * 32767.0))

	return _wav(data)

func _wav(data: PackedByteArray) -> AudioStreamWAV:
	var wav := AudioStreamWAV.new()
	wav.format = AudioStreamWAV.FORMAT_16_BITS
	wav.mix_rate = RATE
	wav.stereo = false
	wav.data = data
	return wav
