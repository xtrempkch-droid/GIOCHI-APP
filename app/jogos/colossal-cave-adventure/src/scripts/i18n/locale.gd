class_name Locale
extends RefCounted
## Todo o texto do jogo, por idioma. A simulacao nao tem NENHUMA frase.
##
## O modelo (scripts/sim/cave.gd) so devolve chaves e argumentos: `message_key`
## e `message_args`. Quem monta a frase e esta classe, chamada pela
## apresentacao. Tres consequencias, todas boas:
##
##   * traduzir e um arquivo so (este), sem tocar em regra;
##   * o teste de idiomas compara as chaves de TODOS os idiomas e falha se
##     faltar uma traducao -- o erro classico de i18n vira teste, nao bug;
##   * a simulacao roda igual em qualquer idioma (os testes de regra nao
##     dependem de texto).
##
## Adicionar um idioma = acrescentar uma entrada em LANGUAGES e em cada
## dicionario abaixo. Nada mais.

## Ordem de exibicao no menu de pausa.
const LANGUAGES := ["pt-BR", "en", "es"]
const LANGUAGE_NAMES := {
	"pt-BR": "Português (Brasil)",
	"en": "English",
	"es": "Español",
}
const DEFAULT_LANGUAGE := "pt-BR"
const CONFIG_PATH := "user://colossal-cave.cfg"

var language := DEFAULT_LANGUAGE

func _init(lang: String = DEFAULT_LANGUAGE) -> void:
	language = lang if LANGUAGES.has(lang) else DEFAULT_LANGUAGE

## Idioma salvo; "auto" segue o idioma do sistema (e cai no padrao se nao
## houver traducao para ele).
static func load_language() -> String:
	var config := ConfigFile.new()
	if config.load(CONFIG_PATH) != OK:
		return detect_system()
	var saved := String(config.get_value("video", "idioma", "auto"))
	return detect_system() if saved == "auto" else normalize(saved)

static func save_language(lang: String) -> void:
	var config := ConfigFile.new()
	config.load(CONFIG_PATH)
	config.set_value("video", "idioma", lang)
	var err := config.save(CONFIG_PATH)
	if err != OK:
		push_warning("idioma: nao foi possivel salvar %s (erro %d)" % [CONFIG_PATH, err])

## Primeiro idioma que casa com o locale do sistema; senao, o padrao.
static func detect_system() -> String:
	var system_locale := OS.get_locale().replace("_", "-").to_lower()
	for lang in LANGUAGES:
		if system_locale.begins_with(lang.to_lower()):
			return lang
	# Variantes: "pt", "pt-PT", "en-GB", "es-419".
	var base := system_locale.split("-")[0]
	for lang in LANGUAGES:
		if lang.split("-")[0] == base:
			return lang
	return DEFAULT_LANGUAGE

static func normalize(lang: String) -> String:
	return lang if LANGUAGES.has(lang) else DEFAULT_LANGUAGE

## Proximo idioma da lista (para o jogador trocar com um botao).
func next_language() -> String:
	var index := LANGUAGES.find(language)
	return LANGUAGES[(index + 1) % LANGUAGES.size()]

func language_label() -> String:
	return LANGUAGE_NAMES.get(language, language)

# --- consultas de texto -------------------------------------------------

func room_name(room_id: String) -> String:
	return _lookup(["rooms", room_id, "name"], room_id)

func room_text(room_id: String) -> String:
	return _lookup(["rooms", room_id, "text"], "")

func item_name(item_id: String) -> String:
	return _lookup(["items", item_id, "name"], item_id)

func item_desc(item_id: String) -> String:
	return _lookup(["items", item_id, "desc"], item_id)

func direction_name(dir: String) -> String:
	return _lookup(["directions", dir], dir)

func exit_list(dirs: Array) -> String:
	if dirs.is_empty():
		return text("ui.no_exits")
	var names := []
	for dir in dirs:
		names.append(direction_name(dir))
	return text("ui.exits_sep").join(names)

## Monta a frase de um evento do turno. `args` traz valores ja resolvidos
## (nomes de item, direcao, etc) -- quem chama resolve, porque so a
## apresentacao sabe o idioma.
func message(key: String, args: Dictionary = {}) -> String:
	var template := _lookup(["messages", key], "")
	if template == "":
		return ""
	var result := template
	for name in args.keys():
		result = result.replace("{%s}" % name, str(args[name]))
	return result

## Texto solto da interface (rotulos, pausa, HUD).
func text(key: String) -> String:
	return _lookup(["ui", key], key)

## Nao pode se chamar `_get`: colide com o Object._get nativo do engine.
func _lookup(path: Array, fallback: String) -> String:
	var value: Variant = _walk(STRINGS[language], path)
	if value == null:
		# Falta traducao neste idioma: cai no padrao antes de desistir.
		value = _walk(STRINGS[DEFAULT_LANGUAGE], path)
	if value == null:
		return fallback
	return str(value)

## Percorre o dicionario aninhado; null quando o caminho nao existe.
func _walk(root: Variant, path: Array) -> Variant:
	var value: Variant = root
	for step in path:
		if not (value is Dictionary) or not value.has(step):
			return null
		value = value[step]
	return value

# =========================================================================
# DICIONARIOS
# =========================================================================

const STRINGS := {
	"pt-BR": {
		"directions": {
			"n": "norte", "s": "sul", "l": "leste", "o": "oeste",
			"c": "cima", "b": "baixo",
		},
		"rooms": {
			"estrada": {
				"name": "Fim da Estrada",
				"text": "O caminho de terra termina aqui. Adiante o terreno sobe e a mata fecha dos dois lados; a leste ha uma construcao baixa de pedra.",
			},
			"predio": {
				"name": "Casa do Poco",
				"text": "Uma casa de pedra pequena, com um poco fundo no centro do piso. Ha uma placa torta: deixe o que achou aqui dentro. O escuro do poco desce para sempre.",
			},
			"grade": {
				"name": "Fora da Grade",
				"text": "Uma grade de ferro enferrujada cobre um buraco na terra. Entre as barras da para ver uma pedra lisa la embaixo.",
			},
			"pedra": {
				"name": "Pedra Lisa",
				"text": "Uma laje de pedra polida pelo tempo. A passagem segue estreita para o sul, e a grade balanca acima.",
			},
			"fenda": {
				"name": "Fenda",
				"text": "A passagem aperta a ponto de raspar os ombros. O ar aqui e frio e cheira a terra molhada; a saida larga esta ao norte.",
			},
			"rastro": {
				"name": "Trilha de Pedras",
				"text": "Pedras soltas cobrem o chao e rangem sob os pes. A fenda fica ao norte e a galeria continua ao sul.",
			},
			"vale": {
				"name": "Vale",
				"text": "Uma garganta larga desce entre paredes altas. Um riacho raso corre ao sul e a luz cinzenta entra por uma abertura no alto.",
			},
			"riacho": {
				"name": "Riacho Subterraneo",
				"text": "A agua e rasa e transparente, correndo sobre cascalho. Do outro lado a parede sobe reta, sem passagem.",
			},
			"canion": {
				"name": "Canion Desajeitado",
				"text": "O corredor virou um degrau de rocha alto demais para saltar. Quem tentar descer sem corda vai rolar.",
			},
			"caverna": {
				"name": "Caverna do Fundo",
				"text": "Uma sala redonda e baixa, com o chao coberto de poeira fina. Ha pegadas antigas demais para serem suas.",
			},
			"osso": {
				"name": "Sala dos Ossos",
				"text": "Ossos pequenos se acumulam num canto, limpos e organizados. Nada aqui parece ter morrido sozinho.",
			},
			"poco": {
				"name": "Topo do Poco Pequeno",
				"text": "Um poco estreito se abre no chao, fundo demais para medir a vista. Uma pedra jogada la dentro demora a responder.",
			},
			"lago": {
				"name": "Lago Escuro",
				"text": "Uma agua parada e preta ocupa a sala inteira. A superficie nao reflete nada.",
			},
			"galeria": {
				"name": "Galeria Longa",
				"text": "Uma galeria reta e alta, com estalactites no teto. O som dos passos volta com atraso, o que nao devia acontecer.",
			},
			"salao": {
				"name": "Salao do Rei da Montanha",
				"text": "O teto some na escuridao. No centro, um trono de pedra bruta, vazio, com veios de metal que brilham quando a luz passa.",
			},
			"camara": {
				"name": "Camara do Tesouro",
				"text": "Prateleiras de rocha furadas a mao. Alguma coisa ainda fica nelas, coberta de poeira.",
			},
			"abismo": {
				"name": "Beira do Abismo",
				"text": "O chao termina. Um vazio enorme respira devagar la embaixo, e o vento que sobe cheira a ferro.",
			},
			"queda": {
				"name": "Fundo da Queda",
				"text": "Pedra quebrada por todo lado. La em cima, muito acima, um ponto de luz marca de onde voce caiu; a parede tem agarra para subir.",
			},
		},
		"items": {
			"lampiao": {"name": "lampiao", "desc": "um lampiao de lata, com pavio"},
			"chave": {"name": "chave", "desc": "uma chave de ferro comprida"},
			"corda": {"name": "corda", "desc": "um rolo de corda grossa"},
			"passaro": {"name": "passaro", "desc": "um passaro de penas escuras, manso"},
			"diamante": {"name": "diamante", "desc": "um diamante grande, cortado a mao"},
			"ouro": {"name": "ouro", "desc": "moedas de ouro antigas"},
			"prata": {"name": "prata", "desc": "uma barra de prata fosca"},
			"joia": {"name": "joia", "desc": "uma joia encravada em pedra"},
			"coroa": {"name": "coroa", "desc": "uma coroa de metal escurecido"},
		},
		"messages": {
			"nothing": "Nada acontece.",
			"invalid_dir": "Direcao invalida.",
			"no_exit": "Nao ha passagem para o {dir}.",
			"dark": "Escuro demais. Voce nao sabe o que ha ali.",
			"fall": "Voce cai. Muito tempo depois, acorda na Casa do Poco.",
			"not_here": "Nao ha {item} aqui.",
			"hands_full": "Suas maos estao cheias.",
			"take": "Voce pega {item}.",
			"not_carrying": "Voce nao tem {item}.",
			"dropped": "Voce larga {item}.",
			"deposited": "Guardado. {score} pontos.",
			"lamp_on": "O lampiao acende.",
			"lamp_off": "O lampiao apaga.",
			"no_lamp": "Voce nao tem lampiao.",
			"lamp_empty": "O lampiao esta vazio.",
			"lamp_died": "O lampiao morre. A escuridao fecha em volta.",
			"end_turns": "A noite acaba. Voce nao volta.",
		},
		"ui": {
			"points": "pontos",
			"turns": "turnos",
			"treasures": "tesouros",
			"carrying": "carregando: nada",
			"carrying_prefix": "carregando: ",
			"lamp": "lampiao",
			"lamp_none": "sem lampiao",
			"lamp_off": "apagado",
			"lamp_on": "aceso",
			"lamp_empty": "vazio",
			"pause": "PAUSA",
			"graphics": "Gráficos",
			"language": "Idioma",
			"resume": "ESC  continuar",
			"quit": "Q  sair",
			"controls_move": "WASD/setas  andar",
			"controls_take": "E  pegar",
			"controls_drop": "Q  largar",
			"controls_lamp": "L  lampiao",
			"exits": "Saidas",
			"exits_sep": ", ",
			"no_exits": "nenhuma",
			"title": "COLOSSAL CAVE",
			"won": "TESOURO COMPLETO",
			"fell": "QUEDA!",
			"end": "FIM",
			"points_gain": "+{points} PONTOS",
			"new_game": "NOVA PARTIDA",
			"quality_0": "Baixo",
			"quality_1": "Médio",
			"quality_2": "Alto",
			"reason_treasure": "tesouro completo",
			"reason_turns": "tempo esgotado",
			"reason_win": "vitoria",
		},
	},
	"en": {
		"directions": {
			"n": "north", "s": "south", "l": "east", "o": "west",
			"c": "up", "b": "down",
		},
		"rooms": {
			"estrada": {
				"name": "End of the Road",
				"text": "The dirt track stops here. Ahead the ground rises and the woods close in on both sides; to the east stands a low stone building.",
			},
			"predio": {
				"name": "Well House",
				"text": "A small stone building with a deep well at the centre of the floor. A crooked sign reads: leave what you found in here. The dark of the well goes down forever.",
			},
			"grade": {
				"name": "Outside the Grate",
				"text": "A rusty iron grate covers a hole in the earth. Between the bars you can make out a smooth stone far below.",
			},
			"pedra": {
				"name": "Smooth Stone",
				"text": "A stone slab worn smooth by time. The passage narrows to the south, and the grate sways above.",
			},
			"fenda": {
				"name": "Cleft",
				"text": "The passage squeezes until it scrapes your shoulders. The air is cold and smells of wet earth; the wide exit is north.",
			},
			"rastro": {
				"name": "Stone Trail",
				"text": "Loose stones cover the floor and grind underfoot. The cleft lies north and the gallery carries on south.",
			},
			"vale": {
				"name": "Valley",
				"text": "A broad gully drops between high walls. A shallow stream runs south and grey light leaks through an opening above.",
			},
			"riacho": {
				"name": "Underground Stream",
				"text": "The water is shallow and clear, running over gravel. Across it the wall rises sheer, with no passage.",
			},
			"canion": {
				"name": "Awkward Canyon",
				"text": "The corridor has become a rock step too high to jump. Anyone who tries to go down without a rope will tumble.",
			},
			"caverna": {
				"name": "Deep Cavern",
				"text": "A round, low room with fine dust over the floor. There are footprints far too old to be yours.",
			},
			"osso": {
				"name": "Hall of Bones",
				"text": "Small bones gather in one corner, clean and neatly arranged. Nothing here looks like it died on its own.",
			},
			"poco": {
				"name": "Top of the Small Pit",
				"text": "A narrow shaft opens in the floor, too deep to judge by eye. A stone dropped in takes a long time to answer.",
			},
			"lago": {
				"name": "Dark Lake",
				"text": "Still, black water fills the whole room. The surface reflects nothing.",
			},
			"galeria": {
				"name": "Long Gallery",
				"text": "A tall, straight gallery with stalactites overhead. The sound of your steps comes back late, which it should not.",
			},
			"salao": {
				"name": "Hall of the Mountain King",
				"text": "The ceiling vanishes into darkness. At the centre stands a throne of rough stone, empty, its metal veins glinting as the light passes.",
			},
			"camara": {
				"name": "Treasure Chamber",
				"text": "Shelves of rock carved by hand. Something still rests on them, under a coat of dust.",
			},
			"abismo": {
				"name": "Edge of the Chasm",
				"text": "The floor ends. An enormous void breathes slowly below, and the wind rising from it smells of iron.",
			},
			"queda": {
				"name": "Foot of the Fall",
				"text": "Broken rock everywhere. Far above, a point of light marks where you fell from; the wall has holds for climbing.",
			},
		},
		"items": {
			"lampiao": {"name": "lamp", "desc": "a tin lamp with a wick"},
			"chave": {"name": "key", "desc": "a long iron key"},
			"corda": {"name": "rope", "desc": "a coil of thick rope"},
			"passaro": {"name": "bird", "desc": "a tame bird with dark feathers"},
			"diamante": {"name": "diamond", "desc": "a large hand-cut diamond"},
			"ouro": {"name": "gold", "desc": "old gold coins"},
			"prata": {"name": "silver", "desc": "a bar of dull silver"},
			"joia": {"name": "jewel", "desc": "a jewel set in stone"},
			"coroa": {"name": "crown", "desc": "a crown of darkened metal"},
		},
		"messages": {
			"nothing": "Nothing happens.",
			"invalid_dir": "Invalid direction.",
			"no_exit": "There is no passage {dir_word}.",
			"dark": "Too dark. You cannot tell what is there.",
			"fall": "You fall. Much later you wake in the Well House.",
			"not_here": "There is no {item} here.",
			"hands_full": "Your hands are full.",
			"take": "You take {item}.",
			"not_carrying": "You are not carrying {item}.",
			"dropped": "You drop {item}.",
			"deposited": "Stored. {score} points.",
			"lamp_on": "The lamp burns.",
			"lamp_off": "The lamp goes out.",
			"no_lamp": "You have no lamp.",
			"lamp_empty": "The lamp is empty.",
			"lamp_died": "The lamp dies. Darkness closes in.",
			"end_turns": "The night runs out. You do not come back.",
		},
		"ui": {
			"points": "points",
			"turns": "turns",
			"treasures": "treasures",
			"carrying": "carrying: nothing",
			"carrying_prefix": "carrying: ",
			"lamp": "lamp",
			"lamp_none": "no lamp",
			"lamp_off": "out",
			"lamp_on": "lit",
			"lamp_empty": "empty",
			"pause": "PAUSED",
			"graphics": "Graphics",
			"language": "Language",
			"resume": "ESC  resume",
			"quit": "Q  quit",
			"controls_move": "WASD/arrows  walk",
			"controls_take": "E  take",
			"controls_drop": "Q  drop",
			"controls_lamp": "L  lamp",
			"exits": "Exits",
			"exits_sep": ", ",
			"no_exits": "none",
			"title": "COLOSSAL CAVE",
			"won": "TREASURE COMPLETE",
			"fell": "YOU FELL!",
			"end": "THE END",
			"points_gain": "+{points} POINTS",
			"new_game": "NEW GAME",
			"quality_0": "Low",
			"quality_1": "Medium",
			"quality_2": "High",
			"reason_treasure": "treasure complete",
			"reason_turns": "out of time",
			"reason_win": "victory",
		},
	},
	"es": {
		"directions": {
			"n": "norte", "s": "sur", "l": "este", "o": "oeste",
			"c": "arriba", "b": "abajo",
		},
		"rooms": {
			"estrada": {
				"name": "Fin del Camino",
				"text": "El camino de tierra termina aqui. Adelante el terreno sube y el bosque cierra a los dos lados; al este hay una construccion baja de piedra.",
			},
			"predio": {
				"name": "Casa del Pozo",
				"text": "Una casa pequena de piedra, con un pozo profundo en el centro del suelo. Hay un cartel torcido: deje aqui lo que encuentre. La oscuridad del pozo baja para siempre.",
			},
			"grade": {
				"name": "Fuera de la Reja",
				"text": "Una reja de hierro oxidada cubre un agujero en la tierra. Entre las barras se ve una piedra lisa alla abajo.",
			},
			"pedra": {
				"name": "Piedra Lisa",
				"text": "Una losa de piedra pulida por el tiempo. El paso sigue estrecho hacia el sur, y la reja se mece arriba.",
			},
			"fenda": {
				"name": "Grieta",
				"text": "El paso se aprieta hasta rozar los hombros. El aire es frio y huele a tierra mojada; la salida ancha esta al norte.",
			},
			"rastro": {
				"name": "Senda de Piedras",
				"text": "Piedras sueltas cubren el suelo y crujen bajo los pies. La grieta queda al norte y la galeria sigue al sur.",
			},
			"vale": {
				"name": "Valle",
				"text": "Una garganta ancha baja entre paredes altas. Un arroyo poco profundo corre al sur y una luz gris entra por una abertura arriba.",
			},
			"riacho": {
				"name": "Arroyo Subterraneo",
				"text": "El agua es poco profunda y clara, corriendo sobre grava. Al otro lado la pared sube recta, sin paso.",
			},
			"canion": {
				"name": "Canon Torpe",
				"text": "El corredor se volvio un escalon de roca demasiado alto para saltar. Quien baje sin cuerda rodara.",
			},
			"caverna": {
				"name": "Caverna del Fondo",
				"text": "Una sala redonda y baja, con el suelo cubierto de polvo fino. Hay huellas demasiado viejas para ser suyas.",
			},
			"osso": {
				"name": "Sala de los Huesos",
				"text": "Huesos pequenos se acumulan en un rincon, limpios y ordenados. Nada aqui parece haber muerto solo.",
			},
			"poco": {
				"name": "Cima del Pozo Pequeno",
				"text": "Un pozo estrecho se abre en el suelo, demasiado profundo para medirlo a la vista. Una piedra lanzada adentro tarda en responder.",
			},
			"lago": {
				"name": "Lago Oscuro",
				"text": "Un agua quieta y negra ocupa toda la sala. La superficie no refleja nada.",
			},
			"galeria": {
				"name": "Galeria Larga",
				"text": "Una galeria recta y alta, con estalactitas en el techo. El sonido de los pasos vuelve con retraso, lo que no deberia pasar.",
			},
			"salao": {
				"name": "Salon del Rey de la Montana",
				"text": "El techo se pierde en la oscuridad. En el centro, un trono de piedra bruta, vacio, con vetas de metal que brillan al pasar la luz.",
			},
			"camara": {
				"name": "Camara del Tesoro",
				"text": "Estantes de roca cavados a mano. Algo queda todavia en ellos, cubierto de polvo.",
			},
			"abismo": {
				"name": "Borde del Abismo",
				"text": "El suelo termina. Un vacio enorme respira despacio alla abajo, y el viento que sube huele a hierro.",
			},
			"queda": {
				"name": "Fondo de la Caida",
				"text": "Piedra rota por todas partes. Muy arriba, un punto de luz marca de donde cayo; la pared tiene agarres para subir.",
			},
		},
		"items": {
			"lampiao": {"name": "lampara", "desc": "una lampara de hojalata con mecha"},
			"chave": {"name": "llave", "desc": "una llave de hierro larga"},
			"corda": {"name": "cuerda", "desc": "un rollo de cuerda gruesa"},
			"passaro": {"name": "pajaro", "desc": "un pajaro manso de plumas oscuras"},
			"diamante": {"name": "diamante", "desc": "un diamante grande, tallado a mano"},
			"ouro": {"name": "oro", "desc": "monedas de oro antiguas"},
			"prata": {"name": "plata", "desc": "una barra de plata opaca"},
			"joia": {"name": "joya", "desc": "una joya engarzada en piedra"},
			"coroa": {"name": "corona", "desc": "una corona de metal oscurecido"},
		},
		"messages": {
			"nothing": "No pasa nada.",
			"invalid_dir": "Direccion invalida.",
			"no_exit": "No hay paso hacia el {dir_word}.",
			"dark": "Demasiado oscuro. No sabe que hay alli.",
			"fall": "Usted cae. Mucho despues despierta en la Casa del Pozo.",
			"not_here": "No hay {item} aqui.",
			"hands_full": "Tiene las manos llenas.",
			"take": "Usted toma {item}.",
			"not_carrying": "Usted no lleva {item}.",
			"dropped": "Usted suelta {item}.",
			"deposited": "Guardado. {score} puntos.",
			"lamp_on": "La lampara se enciende.",
			"lamp_off": "La lampara se apaga.",
			"no_lamp": "Usted no tiene lampara.",
			"lamp_empty": "La lampara esta vacia.",
			"lamp_died": "La lampara muere. La oscuridad se cierra.",
			"end_turns": "La noche se acaba. Usted no vuelve.",
		},
		"ui": {
			"points": "puntos",
			"turns": "turnos",
			"treasures": "tesoros",
			"carrying": "llevando: nada",
			"carrying_prefix": "llevando: ",
			"lamp": "lampara",
			"lamp_none": "sin lampara",
			"lamp_off": "apagada",
			"lamp_on": "encendida",
			"lamp_empty": "vacia",
			"pause": "PAUSA",
			"graphics": "Graficos",
			"language": "Idioma",
			"resume": "ESC  continuar",
			"quit": "Q  salir",
			"controls_move": "WASD/flechas  andar",
			"controls_take": "E  tomar",
			"controls_drop": "Q  soltar",
			"controls_lamp": "L  lampara",
			"exits": "Salidas",
			"exits_sep": ", ",
			"no_exits": "ninguna",
			"title": "COLOSSAL CAVE",
			"won": "TESORO COMPLETO",
			"fell": "¡CAIDA!",
			"end": "FIN",
			"points_gain": "+{points} PUNTOS",
			"new_game": "NUEVA PARTIDA",
			"quality_0": "Bajo",
			"quality_1": "Medio",
			"quality_2": "Alto",
			"reason_treasure": "tesoro completo",
			"reason_turns": "tiempo agotado",
			"reason_win": "victoria",
		},
	},
}
