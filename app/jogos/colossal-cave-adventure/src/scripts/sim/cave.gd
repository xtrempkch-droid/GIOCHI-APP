class_name Cave
extends RefCounted
## Regras do Colossal Cave, sem dependencia do engine grafico.
##
## Mesma separacao dos outros jogos: aqui so ha estado e regras; a cena
## (scripts/main.gd) desenha a caverna em 2,5D. Assim a partida inteira roda
## headless, com semente, e pode ser medida pelo modo benchmark.
##
## O jogo e por TURNOS, como o original de 1976 era por comandos digitados:
## cada acao consome um turno, e o lampiao queima por turno aceso. Isso serve
## de benchmark porque o custo e deterministico -- da para comparar aparelhos
## sem depender da habilidade de quem joga.
##
## IDIOMAS: este arquivo nao tem NENHUMA frase. Salas, itens e mensagens sao
## identificados por chave (room_id, item_id, message_key) e o texto vive em
## scripts/i18n/locale.gd, com portugues, ingles e espanhol. Assim a regra e a
## traducao evoluem separadas -- e o teste de idiomas cobra a traducao de cada
## chave que o modelo pode emitir.

const MAX_TURNS := 900
const LAMP_FUEL := 330
## Premio de cada tesouro guardado e custo de cada queda.
const TREASURE_POINTS := 20
const FALL_PENALTY := 25
## Quantos itens o jogador carrega ao mesmo tempo. Precisa caber o lampiao
## (que ocupa mao) mais os 5 tesouros, senao a partida fica impossivel de
## completar -- o autoplay do benchmark e o teste de vitoria pegam isso.
const MAX_CARRY := 6

const DIRS := ["n", "s", "l", "o", "c", "b"]

## Mapa. `light` = sala iluminada por conta propria (perto da entrada da
## caverna); nas escuras, sem lampiao aceso, o jogador anda no escuro.
const ROOMS := {
	"estrada": {
		"light": true,
		"exits": {"l": "predio"},
	},
	"predio": {
		"light": true,
		"exits": {"o": "estrada", "b": "grade"},
	},
	"grade": {
		"light": false,
		"exits": {"c": "predio", "b": "pedra"},
	},
	"pedra": {
		"light": false,
		"exits": {"c": "grade", "s": "fenda"},
	},
	"fenda": {
		"light": false,
		"exits": {"n": "pedra", "s": "rastro"},
	},
	"rastro": {
		"light": false,
		"exits": {"n": "fenda", "s": "vale"},
	},
	"vale": {
		"light": true,
		"exits": {"n": "rastro", "s": "riacho", "l": "canion"},
	},
	"riacho": {
		"light": true,
		"exits": {"n": "vale"},
	},
	"canion": {
		"light": false,
		"exits": {"o": "vale", "b": "caverna"},
	},
	"caverna": {
		"light": false,
		"exits": {"c": "canion", "l": "osso"},
	},
	"osso": {
		"light": false,
		"exits": {"o": "caverna", "n": "poco"},
	},
	"poco": {
		"light": false,
		"exits": {"s": "osso", "b": "lago"},
	},
	"lago": {
		"light": false,
		"exits": {"c": "poco", "l": "galeria"},
	},
	"galeria": {
		"light": false,
		"exits": {"o": "lago", "l": "salao"},
	},
	"salao": {
		"light": false,
		"exits": {"o": "galeria", "n": "camara", "s": "abismo"},
	},
	"camara": {
		"light": false,
		"exits": {"s": "salao"},
	},
	"abismo": {
		"light": false,
		"exits": {"n": "salao", "b": "queda"},
	},
	"queda": {
		"light": false,
		"exits": {"c": "abismo"},
	},
}

## Itens. `treasure` = pontua quando guardado na Casa do Poco.
const ITEMS := {
	"lampiao": {"treasure": false, "start": "estrada"},
	"chave": {"treasure": false, "start": "pedra"},
	"corda": {"treasure": false, "start": "rastro"},
	"passaro": {"treasure": false, "start": "canion"},
	"diamante": {"treasure": true, "start": "camara"},
	"ouro": {"treasure": true, "start": "osso"},
	"prata": {"treasure": true, "start": "lago"},
	"joia": {"treasure": true, "start": "caverna"},
	"coroa": {"treasure": true, "start": "salao"},
}

## Estado de um passo. `type` e o verbo.
class Action:
	var type := "wait"
	var arg := ""
	## Direcao do movimento (n/s/l/o/c/b).
	var dir := ""

	func _init(action_type: String = "wait", argument: String = "") -> void:
		type = action_type
		arg = argument

## Eventos do turno, para a apresentacao e o audio.
## type: "move" | "dark" | "blocked" | "take" | "drop" | "lamp" | "fall"
##       | "deposit" | "denied" | "win" | "over"
var events: Array[Dictionary] = []

var room_id := "estrada"
var inventory: Array[String] = []
var room_items: Dictionary = {}
var lamp_on := false
var lamp_fuel := LAMP_FUEL
var score := 0
var deposited := 0
var turns := 0
var visited: Dictionary = {}
var is_over := false
var won := false
## Mensagem do turno como CHAVE de traducao + argumentos. O modelo nao tem
## nenhuma frase: quem monta o texto e scripts/i18n/locale.gd.
var message_key := ""
var message_args: Dictionary = {}

func _init(_seed_value: int = 0) -> void:
	restart()

func restart() -> void:
	room_id = "estrada"
	inventory.clear()
	room_items.clear()
	for id in ITEMS.keys():
		var start: String = ITEMS[id]["start"]
		if not room_items.has(start):
			room_items[start] = []
		room_items[start].append(id)

	lamp_on = false
	lamp_fuel = LAMP_FUEL
	score = 0
	deposited = 0
	turns = 0
	visited = {room_id: true}
	is_over = false
	won = false
	events.clear()
	_clear_message()

# --- consultas ---------------------------------------------------------

func room() -> Dictionary:
	return ROOMS[room_id]

func is_lit() -> bool:
	return bool(ROOMS[room_id]["light"]) or (lamp_on and lamp_fuel > 0)

func exits() -> Dictionary:
	return ROOMS[room_id]["exits"]

func items_here() -> Array:
	return room_items.get(room_id, [])

func total_treasures() -> int:
	var count := 0
	for id in ITEMS.keys():
		if ITEMS[id]["treasure"]:
			count += 1
	return count

## Progresso de 0 a 1: quanto do tesouro ja foi guardado.
func progress() -> float:
	return float(deposited) / float(total_treasures())

# --- acoes -------------------------------------------------------------

## Executa um turno. Devolve true quando o estado mudou.
func step(action: Action) -> bool:
	events.clear()
	_clear_message()
	if is_over:
		return false

	turns += 1
	match action.type:
		"move":
			_move(action.dir)
		"take":
			_take(action.arg)
		"drop":
			_drop(action.arg)
		"lamp":
			_toggle_lamp()
		_:
			_set_message("nothing")

	# O lampiao queima por turno aceso (nao por segundo: o jogo e por turnos).
	if lamp_on:
		lamp_fuel = maxi(0, lamp_fuel - 1)
		if lamp_fuel == 0:
			lamp_on = false
			events.append({"type": "denied", "reason": "lamp_dead"})
			_set_message("lamp_died")

	if turns >= MAX_TURNS and not is_over:
		_set_message("end_turns")
		_end(false, "turns")

	return true

func _move(dir: String) -> void:
	if not DIRS.has(dir):
		_set_message("invalid_dir")
		return

	var target: String = exits().get(dir, "")
	if target == "":
		events.append({"type": "blocked", "dir": dir})
		_set_message("no_exit", {"dir": dir})
		return

	# Sem luz nao da para ver a passagem: o jogador so anda pelo que ja visitou.
	if not is_lit() and not visited.has(target):
		events.append({"type": "dark", "dir": dir})
		_set_message("dark")
		return

	room_id = target
	visited[room_id] = true
	events.append({"type": "move", "dir": dir, "to": room_id})

	# A queda machuca e devolve o jogador para a casa do poco.
	if room_id == "queda":
		score = maxi(0, score - FALL_PENALTY)
		events.append({"type": "fall", "room": room_id})
		room_id = "predio"
		visited[room_id] = true
		_set_message("fall")

func _take(item: String) -> void:
	var here := items_here()
	if not here.has(item):
		_set_message("not_here", {"item": item})
		return
	if inventory.size() >= MAX_CARRY:
		events.append({"type": "denied", "reason": "carry"})
		_set_message("hands_full")
		return

	here.erase(item)
	room_items[room_id] = here
	inventory.append(item)
	events.append({"type": "take", "item": item})
	_set_message("take", {"item": item})

func _drop(item: String) -> void:
	if not inventory.has(item):
		_set_message("not_carrying", {"item": item})
		return

	inventory.erase(item)
	if not room_items.has(room_id):
		room_items[room_id] = []
	room_items[room_id].append(item)
	events.append({"type": "drop", "item": item})

	# Guardar tesouro na Casa do Poco pontua (o contrato do jogo original).
	if room_id == "predio" and bool(ITEMS[item]["treasure"]):
		score += TREASURE_POINTS
		deposited += 1
		events.append({"type": "deposit", "item": item, "points": TREASURE_POINTS})
		_set_message("deposited", {"score": score})
		if deposited >= total_treasures():
			_end(true, "treasure")
		return

	_set_message("dropped", {"item": item})

func _toggle_lamp() -> void:
	if not inventory.has("lampiao"):
		_set_message("no_lamp")
		events.append({"type": "denied", "reason": "no_lamp"})
		return
	if not lamp_on and lamp_fuel <= 0:
		_set_message("lamp_empty")
		events.append({"type": "denied", "reason": "lamp_dead"})
		return
	lamp_on = not lamp_on
	events.append({"type": "lamp", "on": lamp_on, "fuel": lamp_fuel})
	_set_message("lamp_on" if lamp_on else "lamp_off")

func _end(victory: bool, reason: String) -> void:
	is_over = true
	won = victory
	events.append({"type": "win" if victory else "over",
		"reason": reason, "score": score})

# --- mensagens (chaves, nunca frases) ----------------------------------

func _set_message(key: String, args: Dictionary = {}) -> void:
	message_key = key
	message_args = args

func _clear_message() -> void:
	message_key = ""
	message_args = {}

## Estado do lampiao como chave + valor: o HUD monta o texto no idioma ativo.
## Devolve {"key": "ui.lamp_on", "fuel": 120} -- `key` ja e caminho de `ui`.
func lamp_state() -> Dictionary:
	if not inventory.has("lampiao"):
		return {"key": "lamp_none", "fuel": 0}
	if lamp_fuel <= 0:
		return {"key": "lamp_empty", "fuel": 0}
	return {"key": "lamp_on" if lamp_on else "lamp_off", "fuel": lamp_fuel}

## Direcoes com saida, cruas (n/s/l/o/c/b).
func exit_dirs() -> Array:
	var dirs := []
	for dir in DIRS:
		if exits().has(dir):
			dirs.append(dir)
	return dirs
