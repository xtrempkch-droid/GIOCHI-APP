class_name Autoplay
extends RefCounted
## Rota deterministica que fecha a caverna: pega o lampiao, acende, desce ate
## o fundo, recolhe os 5 tesouros e volta para guardar tudo na Casa do Poco.
##
## Serve ao modo benchmark (medir o aparelho sem depender da habilidade de
## quem joga) e ao teste de vitoria (provar que a caverna e completavel).
##
## Se a caverna mudar (salas, itens ou saidas), a rota quebra na hora -- e isso
## e proposital: e o teste que avisa que o mapa ficou sem solucao.

const SIM := preload("res://scripts/sim/cave.gd")

## "m:<dir>" move, "t:<item>" pega, "d:<item>" larga, "l" lampiao.
const ROUTE := [
	"t:lampiao",
	"l",
	"m:l", "m:b", "m:b", "m:s", "m:s", "m:s", "m:l", "m:b",
	"t:joia",
	"m:l", "t:ouro",
	"m:n", "m:b", "t:prata",
	"m:l", "m:l", "t:coroa",
	"m:n", "t:diamante",
	"m:s", "m:o", "m:o", "m:c", "m:s", "m:o", "m:c", "m:o",
	"m:n", "m:n", "m:n", "m:c", "m:c",
	"d:diamante", "d:coroa", "d:prata", "d:ouro", "d:joia",
]

var _index := 0

func reset() -> void:
	_index = 0

func finished() -> bool:
	return _index >= ROUTE.size()

## Proxima acao da rota (ou `wait` quando acabou).
func next() -> Variant:
	if finished():
		return SIM.Action.new("wait")
	var token: String = ROUTE[_index]
	_index += 1
	if token == "l":
		return SIM.Action.new("lamp")
	if token.begins_with("m:"):
		var action = SIM.Action.new("move")
		action.dir = token.substr(2)
		return action
	if token.begins_with("t:"):
		return SIM.Action.new("take", token.substr(2))
	if token.begins_with("d:"):
		return SIM.Action.new("drop", token.substr(2))
	return SIM.Action.new("wait")
