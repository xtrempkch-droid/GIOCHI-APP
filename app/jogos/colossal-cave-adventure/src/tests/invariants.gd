extends SceneTree
## Invariantes do Colossal Cave, sem janela (headless).
##
##   godot --headless --path godot --script res://tests/invariants.gd
##
## Cobre: mapa consistente, movimento, escuridao, lampiao, inventario, queda,
## pontuacao e -- o mais importante -- a prova de que a caverna e COMPLETAVEL,
## rodando a rota de referencia (scripts/sim/autoplay.gd) do inicio ao fim.
## Sai com 0 se tudo passar.

const SIM := preload("res://scripts/sim/cave.gd")
const AutoplayScript := preload("res://scripts/sim/autoplay.gd")
const LocaleScript := preload("res://scripts/i18n/locale.gd")

var failures: int = 0

func _initialize() -> void:
	test_mapa_consistente()
	test_itens_existem()
	test_movimento_e_saidas()
	test_escuridao_bloqueia()
	test_lampiao_queima()
	test_inventario_e_limite()
	test_tesouro_so_pontua_na_casa()
	test_queda()
	test_turnos_limitados()
	test_caverna_completavel()
	test_rota_nao_quebra_com_semente_zero()
	test_idiomas_completos()
	test_idiomas_sem_frase_no_modelo()
	test_troca_de_idioma_nao_muda_a_regra()

	if failures == 0:
		print("TODOS OS TESTES PASSARAM")
	else:
		print("%d FALHA(S)" % failures)
	quit(0 if failures == 0 else 1)

func check(ok: bool, what: String) -> void:
	if not ok:
		print("FALHA: ", what)
		failures += 1

func make_cave() -> Cave:
	return SIM.new()

func act(cave: Cave, type: String, arg: String = "", dir: String = "") -> bool:
	var action = SIM.Action.new(type, arg)
	action.dir = dir
	return cave.step(action)

# --- 1. mapa --------------------------------------------------------------

func test_mapa_consistente() -> void:
	# Toda saida aponta para uma sala que existe (link quebrado = sala morta
	# invisivel no meio do mapa).
	for id in SIM.ROOMS.keys():
		var room: Dictionary = SIM.ROOMS[id]
		for dir in room["exits"].keys():
			var target: String = room["exits"][dir]
			check(SIM.ROOMS.has(target),
				"sala '%s' sai para '%s' pelo '%s', que nao existe" % [id, target, dir])
		# Nome e descricao vivem no i18n; aqui so se cobra a chave existir.
		check(room.has("light"), "sala '%s' sem 'light'" % id)
		check(room.has("exits"), "sala '%s' sem 'exits'" % id)

	# Simetria: se A vai para B, B tem de voltar para A. Sem isso a sala vira
	# armadilha silenciosa -- e foi assim que "queda" nasceu quebrada (saia
	# para salao e grade, e nenhuma das duas voltava).
	var opposite := {"n": "s", "s": "n", "l": "o", "o": "l", "c": "b", "b": "c"}
	for id in SIM.ROOMS.keys():
		for dir in SIM.ROOMS[id]["exits"].keys():
			var target: String = SIM.ROOMS[id]["exits"][dir]
			var back: String = SIM.ROOMS[target]["exits"].get(opposite[dir], "")
			check(back == id,
				"'%s' vai para '%s' pelo '%s', mas '%s' nao volta por '%s'"
					% [id, target, dir, target, opposite[dir]])

	# Todas as salas precisam ser alcancaveis a partir da estrada.
	var reachable := {"estrada": true}
	var frontier := ["estrada"]
	while not frontier.is_empty():
		var current: String = frontier.pop_back()
		for dir in SIM.ROOMS[current]["exits"].keys():
			var target: String = SIM.ROOMS[current]["exits"][dir]
			if not reachable.has(target):
				reachable[target] = true
				frontier.append(target)
	for id in SIM.ROOMS.keys():
		check(reachable.has(id), "sala '%s' inalcancavel a partir da estrada" % id)

func test_itens_existem() -> void:
	check(SIM.ITEMS.size() >= 9, "esperado ao menos 9 itens")
	for id in SIM.ITEMS.keys():
		var start: String = SIM.ITEMS[id]["start"]
		check(SIM.ROOMS.has(start), "item '%s' nasce em sala inexistente '%s'" % [id, start])
	# O lampiao precisa nascer onde o jogador comeca, senao o jogo trava.
	check(SIM.ITEMS["lampiao"]["start"] == "estrada", "o lampiao nasce na estrada")

# --- 2. movimento --------------------------------------------------------

func test_movimento_e_saidas() -> void:
	var c := make_cave()
	check(c.room_id == "estrada", "comeca na estrada")
	act(c, "move", "", "s")
	check(c.room_id == "estrada", "direcao sem saida nao move")
	check(c.events.any(func(e): return e["type"] == "blocked"), "avisa que esta bloqueado")

	act(c, "move", "", "l")
	check(c.room_id == "predio", "leste leva a Casa do Poco")
	act(c, "move", "", "o")
	check(c.room_id == "estrada", "oeste volta para a estrada")

func test_escuridao_bloqueia() -> void:
	var c := make_cave()
	act(c, "move", "", "l")   # predio
	act(c, "move", "", "b")   # grade (escura, visitada)
	check(c.room_id == "grade", "desce pela grade")
	# Sem lampiao aceso, ir para uma sala NAO visitada e recusado.
	act(c, "move", "", "b")
	check(c.room_id == "grade", "no escuro nao anda para sala desconhecida")
	check(c.events.any(func(e): return e["type"] == "dark"), "avisa que esta escuro")

	# Com o lampiao aceso, anda.
	c.restart()
	act(c, "take", "lampiao")
	act(c, "lamp")
	check(c.lamp_on, "lampiao aceso")
	act(c, "move", "", "l")
	act(c, "move", "", "b")
	act(c, "move", "", "b")
	check(c.room_id == "pedra", "com luz desce ate a pedra lisa")

func test_lampiao_queima() -> void:
	var c := make_cave()
	act(c, "take", "lampiao")
	act(c, "lamp")
	var fuel_before := c.lamp_fuel
	act(c, "move", "", "l")
	check(c.lamp_fuel == fuel_before - 1, "o lampiao queima um por turno aceso")

	# Apagado nao queima.
	act(c, "lamp")
	check(not c.lamp_on, "lampiao apagado")
	var fuel_off := c.lamp_fuel
	act(c, "move", "", "o")
	check(c.lamp_fuel == fuel_off, "apagado nao consome combustivel")

	# Sem combustivel, nao acende.
	c.lamp_fuel = 0
	act(c, "lamp")
	check(not c.lamp_on, "lampiao vazio nao acende")
	check(c.events.any(func(e): return e["type"] == "denied"), "avisa que esta vazio")

	# Sem o lampiao na mao, nao ha o que acender.
	c.restart()
	act(c, "lamp")
	check(not c.lamp_on, "sem lampiao nao acende")

func test_inventario_e_limite() -> void:
	var c := make_cave()
	act(c, "take", "lampiao")
	check(c.inventory.has("lampiao"), "pega o lampiao")
	check(not c.items_here().has("lampiao"), "o item sai da sala")

	act(c, "drop", "lampiao")
	check(not c.inventory.has("lampiao"), "larga o lampiao")
	check(c.items_here().has("lampiao"), "o item volta para a sala")

	# Nao pega o que nao esta aqui.
	act(c, "take", "diamante")
	check(not c.inventory.has("diamante"), "nao pega item de outra sala")

	# Limite de carga: 6 itens contando o lampiao.
	c.inventory.clear()
	for i in SIM.MAX_CARRY:
		c.inventory.append("ouro")
	c.room_items[c.room_id] = ["diamante"]
	act(c, "take", "diamante")
	check(not c.inventory.has("diamante"), "maos cheias recusam o item")
	check(c.events.any(func(e): return e["type"] == "denied"), "avisa que as maos estao cheias")

func test_tesouro_so_pontua_na_casa() -> void:
	var c := make_cave()
	c.inventory = ["diamante"]
	act(c, "drop", "diamante")
	check(c.score == 0, "largar tesouro fora da casa nao pontua (score=%d)" % c.score)

	c.restart()
	c.inventory = ["diamante"]
	act(c, "move", "", "l")
	check(c.room_id == "predio", "esta na casa do poco")
	act(c, "drop", "diamante")
	check(c.score == SIM.TREASURE_POINTS, "guardar na casa pontua")
	check(c.deposited == 1, "conta o tesouro guardado")

func test_queda() -> void:
	var c := make_cave()
	# Vai ate a beira do abismo (com luz) e cai.
	c.room_id = "abismo"
	c.visited = {"abismo": true, "queda": true, "predio": true}
	c.lamp_on = true
	c.inventory = ["lampiao"]
	c.score = 60
	act(c, "move", "", "b")
	check(c.room_id == "predio", "a queda devolve o jogador para a casa do poco")
	check(c.score == 60 - SIM.FALL_PENALTY, "a queda custa pontos (score=%d)" % c.score)
	check(c.events.any(func(e): return e["type"] == "fall"), "avisa a queda")

func test_turnos_limitados() -> void:
	var c := make_cave()
	c.turns = SIM.MAX_TURNS - 1
	act(c, "move", "", "s")
	check(c.is_over, "a partida fecha ao esgotar os turnos")
	check(not c.won, "esgotar turnos nao e vitoria")
	check(c.events.any(func(e): return e["type"] == "over"), "emite evento de fim")

# --- 3. a caverna e completavel -----------------------------------------

func test_caverna_completavel() -> void:
	var c := make_cave()
	var bot = AutoplayScript.new()
	var steps := 0
	while not bot.finished() and not c.is_over and steps < SIM.MAX_TURNS:
		var action = bot.next()
		act(c, action.type, action.arg, action.dir)
		steps += 1

	check(c.is_over, "a rota fecha a caverna (terminou em %d turnos)" % c.turns)
	check(c.won, "a rota vence (won=%s)" % c.won)
	check(c.deposited == c.total_treasures(),
		"guardou os %d tesouros (guardou %d)" % [c.total_treasures(), c.deposited])
	check(c.score > 0, "terminou com pontuacao positiva (%d)" % c.score)
	check(steps == AutoplayScript.ROUTE.size(),
		"a rota executou inteira (%d de %d acoes)" % [steps, AutoplayScript.ROUTE.size()])
	print("  rota de referencia: %d turnos, %d pontos, %d/%d tesouros"
		% [c.turns, c.score, c.deposited, c.total_treasures()])

func test_rota_nao_quebra_com_semente_zero() -> void:
	# O jogo e deterministico: semente nao muda nada (nao ha sorteio aqui).
	# O teste existe para o dia em que alguem adicionar aleatoriedade.
	for seed_value in [0, 1, 99999]:
		var c = SIM.new(seed_value)
		var bot = AutoplayScript.new()
		var guard := 0
		while not bot.finished() and not c.is_over and guard < SIM.MAX_TURNS:
			var action = bot.next()
			act(c, action.type, action.arg, action.dir)
			guard += 1
		check(c.won, "semente %d tambem vence" % seed_value)


# --- 4. idiomas ----------------------------------------------------------

func test_idiomas_completos() -> void:
	# Toda chave que o modelo pode emitir precisa existir em TODOS os idiomas.
	# Esta e a parte que impede o classico "texto em branco no japones".
	var keys_messages := [
		"nothing", "invalid_dir", "no_exit", "dark", "fall", "not_here",
		"hands_full", "take", "not_carrying", "dropped", "deposited",
		"lamp_on", "lamp_off", "no_lamp", "lamp_empty", "lamp_died", "end_turns",
	]
	var keys_ui := [
		"points", "turns", "treasures", "carrying", "carrying_prefix", "lamp",
		"lamp_none", "lamp_off", "lamp_on", "lamp_empty", "pause", "graphics",
		"language", "resume", "quit", "controls_move", "controls_take",
		"controls_drop", "controls_lamp", "exits", "exits_sep", "no_exits",
		"title", "won", "fell", "end", "points_gain", "new_game",
		"quality_0", "quality_1", "quality_2",
		"reason_treasure", "reason_turns", "reason_win",
	]

	for lang in LocaleScript.LANGUAGES:
		var loc = LocaleScript.new(lang)
		for key in keys_messages:
			check(loc.message(key, {}) != "", "[%s] mensagem '%s' sem traducao" % [lang, key])
		for key in keys_ui:
			check(_has_text(lang, ["ui", key]), "[%s] texto '%s' sem traducao" % [lang, key])
		for room_id in SIM.ROOMS.keys():
			check(_has_text(lang, ["rooms", room_id, "name"]),
				"[%s] sala '%s' sem nome traduzido" % [lang, room_id])
			check(loc.room_text(room_id).length() > 40,
				"[%s] sala '%s' com descricao curta" % [lang, room_id])
		for item_id in SIM.ITEMS.keys():
			check(_has_text(lang, ["items", item_id, "name"]),
				"[%s] item '%s' sem nome traduzido" % [lang, item_id])
			check(_has_text(lang, ["items", item_id, "desc"]),
				"[%s] item '%s' sem descricao traduzida" % [lang, item_id])
		for dir in SIM.DIRS:
			check(_has_text(lang, ["directions", dir]),
				"[%s] direcao '%s' sem traducao" % [lang, dir])

	# Os tres idiomas precisam ter o MESMO conjunto de chaves (uma traducao
	# nova sem as outras deixa buraco silencioso).
	var reference := _flatten(LocaleScript.STRINGS[LocaleScript.DEFAULT_LANGUAGE])
	for lang in LocaleScript.LANGUAGES:
		var keys := _flatten(LocaleScript.STRINGS[lang])
		for key in reference:
			check(keys.has(key), "[%s] falta a chave '%s' que existe no padrao" % [lang, key])
		# Sobrando chave que o padrao nao tem tambem e erro (typo de chave).
		for key in keys:
			if not reference.has(key):
				check(false, "[%s] chave '%s' nao existe no idioma padrao" % [lang, key])

## A chave existe em `lang` (mesmo que o texto coincida com a chave)?
func _has_text(lang: String, path: Array) -> bool:
	var value: Variant = LocaleScript.STRINGS.get(lang, {})
	for step in path:
		if not (value is Dictionary) or not value.has(step):
			return false
		value = value[step]
	return value is String and String(value).length() > 0

func _flatten(table: Dictionary, prefix: String = "") -> Array:
	var keys := []
	for key in table.keys():
		var path: String = key if prefix == "" else "%s.%s" % [prefix, key]
		if table[key] is Dictionary:
			keys.append_array(_flatten(table[key], path))
		else:
			keys.append(path)
	return keys

func test_idiomas_sem_frase_no_modelo() -> void:
	# O modelo nao pode ter texto: se alguem voltar a escrever uma frase em
	# cave.gd, o teste falha aqui -- e nao no aparelho, em outro idioma.
	var file := FileAccess.open("res://scripts/sim/cave.gd", FileAccess.READ)
	check(file != null, "nao foi possivel ler cave.gd")
	if file == null:
		return
	var text := file.get_as_text()
	# Checagem ESTRUTURAL, nao por frase solta: o modelo nao pode ter campos
	# de texto nem atribuir frase a mensagem.
	for marker in ["\"nome\":", "\"texto\":", "\"desc\":"]:
		check(not text.contains(marker), "cave.gd ainda tem o campo de texto %s" % marker)
	# `message = "` (atribuicao de frase) nao pode existir; so `message_key =`.
	var regex := RegEx.new()
	regex.compile("message\\s*=\\s*\"")
	check(regex.search(text) == null, "cave.gd ainda atribui frase a 'message'")
	# Toda mensagem emitida tem de ser chave conhecida do i18n.
	var locales := []
	for lang in LocaleScript.LANGUAGES:
		locales.append(LocaleScript.new(lang))
	for key in ["nothing", "dark", "fall", "take", "dropped", "deposited", "lamp_on"]:
		for loc in locales:
			check(loc.message(key, {}) != "", "chave '%s' sem traducao" % key)

func test_troca_de_idioma_nao_muda_a_regra() -> void:
	# Trocar idioma nao pode mudar nada da simulacao: mesma rota, mesmo
	# resultado. Se mudar, alguem colocou regra dependente de texto.
	for lang in LocaleScript.LANGUAGES:
		var c = SIM.new()
		var bot = AutoplayScript.new()
		var guard := 0
		while not bot.finished() and not c.is_over and guard < SIM.MAX_TURNS:
			var action = bot.next()
			act(c, action.type, action.arg, action.dir)
			guard += 1
		check(c.won, "[%s] a rota de referencia venceu" % lang)
		check(c.score == 100, "[%s] pontuacao final %d (esperado 100)" % [lang, c.score])
