class_name SpacewarMatch
extends RefCounted
## Regras de Spacewar, sem dependencia do engine grafico.
##
## Mesma separacao do Arcade Volleyball: aqui so ha estado e regras. Nada le
## Input, desenha ou toca som, entao a simulacao inteira roda headless
## (tests/invariants.gd) e o resultado e deterministico.
##
## O que foi tirado do listing original de 1962
## (Spacewar-1962-listing.pdp1), e nao de "memoria":
##
##   * o star no centro puxa os dois navios E os torpedos (o listing tem uma
##     rotina de calculo de torpedo que soma o delta da gravidade: "torpedo
##     calc routine");
##   * o torpedo herda o movimento do navio -- e velocidade relativa, nao
##     absoluta;
##   * o torpedo tem tubo com recarga e carga limitada ("check if torp tube
##     reloaded", "check if torpedos exhausted");
##   * o hyperspace sorteia a posicao ("hyperspatial uncertancy") e escala a
##     velocidade ("scale on hyperspatially induced velocity");
##   * navio dentro do star vira explosao ("here to handle spaceships into
##     star" / "now go bang");
##   * o campo e toroidal: o objeto que sai de um lado entra no outro.

## Area logica (mesma escala do port SDL2 do Arcade Volleyball: 1280x720).
const ARENA := Vector2(1280.0, 720.0)
const STAR_RADIUS := 22.0
## G em px/s^2 * px^2. Calibrado para o navio a 300 px do star curvar visivel-
## mente, sem cair irremediavelmente.
const STAR_GRAVITY := 4.2e6
## Distancia minima para a gravidade nao explodir perto do centro.
const GRAVITY_FLOOR := 70.0

const SHIP_RADIUS := 9.0
const SHIP_THRUST := 260.0
const SHIP_TURN := 3.4
const SHIP_MAX_SPEED := 400.0

const TORPEDO_SPEED := 430.0
const TORPEDO_RADIUS := 3.0
const TORPEDO_LIFE := 2.2
## Torpedos simultaneos por navio e tempo de recarga do tubo.
const TORPEDO_MAGAZINE := 4
const TORPEDO_RELOAD := 0.55
## Segundos para o paiol devolver UM torpedo. Sem isso o navio fica sem
## torpedo para sempre depois do quarto tiro (a carga so voltava ao renascer),
## e a partida virava duas naves orbitando sem poder atirar.
const TORPEDO_REPLENISH := 2.4

const HYPERSPACE_COOLDOWN := 4.0
const HYPERSPACE_INVULN := 0.35
const HYPERSPACE_VELOCITY_SCALE := 0.25
## Nao nascer (nem hiperspacear) colado no star.
const SPAWN_CLEARANCE := 210.0

const RESPAWN_DELAY := 1.3
const WIN_SCORE := 7
const MAX_TORPEDOES := 24

## Entrada de um navio por passo. Pulo de motor nao existe aqui: o que precisa
## de borda e o tiro e o hyperspace.
class Controls:
	var rotate_left := false
	var rotate_right := false
	var thrust := false
	var fire := false
	var hyperspace := false

class Ship:
	var pos := Vector2.ZERO
	var vel := Vector2.ZERO
	## Angulo em radianos; 0 aponta para +x.
	var angle := 0.0
	var alive := true
	var invuln := 0.0
	var respawn := 0.0
	var ammo := TORPEDO_MAGAZINE
	var reload := 0.0
	var replenish := TORPEDO_REPLENISH
	var hyper_cooldown := 0.0
	var kills := 0
	## Bordas: sem isso segurar o botao dispara a cada quadro.
	var fire_held := false
	var hyper_held := false

	func forward() -> Vector2:
		return Vector2(cos(angle), sin(angle))

class Torpedo:
	var pos := Vector2.ZERO
	var vel := Vector2.ZERO
	var owner := 0
	var life := TORPEDO_LIFE

## Eventos de um passo, consumidos pela camada visual e pelo audio.
## type: "fire" | "hit" | "explode" | "hyper" | "wrap" | "denied"
var events: Array[Dictionary] = []

var ships: Array[Ship] = [Ship.new(), Ship.new()]
var torpedoes: Array[Torpedo] = []
var is_over := false
var winner := -1

var _rng := RandomNumberGenerator.new()

func _init(seed_value: int = 0) -> void:
	if seed_value == 0:
		_rng.randomize()
	else:
		_rng.seed = seed_value
	_reset_ships()

func restart(seed_value: int = 0) -> void:
	if seed_value != 0:
		_rng.seed = seed_value
	for ship in ships:
		ship.kills = 0
	torpedoes.clear()
	is_over = false
	winner = -1
	events.clear()
	_place_ships_for_new_match()

func _reset_ships() -> void:
	for i in 2:
		ships[i].angle = 0.0 if i == 0 else PI
		ships[i].ammo = TORPEDO_MAGAZINE
		ships[i].reload = 0.0
		ships[i].hyper_cooldown = 0.0
		ships[i].invuln = 1.2
		ships[i].alive = true
		ships[i].respawn = 0.0
	_place_ships_for_new_match()

## Os dois nascem simetricos, fora do alcance do star, com uma componente
## tangencial: eles comecam orbitando, como no original.
func _place_ships_for_new_match() -> void:
	var center := ARENA * 0.5
	for i in 2:
		var side := -1.0 if i == 0 else 1.0
		var ship := ships[i]
		ship.pos = center + Vector2(side * 330.0, 0.0)
		# Tangente ao star: velocidade de orbita aproximada.
		var r := ship.pos.distance_to(center)
		var orbital := sqrt(STAR_GRAVITY / r)
		ship.vel = Vector2(0.0, orbital * (-side))
		# Aponta para fora, para o jogador ver o propulsor.
		ship.angle = 0.0 if i == 0 else PI

## Avanca um passo fixo de `delta` segundos.
func step(delta: float, p1: Controls, p2: Controls) -> void:
	events.clear()
	if is_over:
		return

	_update_ship(0, p1, delta)
	_update_ship(1, p2, delta)
	_update_torpedoes(delta)
	_resolve_collisions()

## --- navios ------------------------------------------------------------

func _update_ship(index: int, input: Controls, delta: float) -> void:
	var ship := ships[index]

	if not ship.alive:
		ship.respawn -= delta
		if ship.respawn <= 0.0:
			_respawn(index)
		return

	ship.invuln = maxf(0.0, ship.invuln - delta)
	ship.reload = maxf(0.0, ship.reload - delta)
	ship.hyper_cooldown = maxf(0.0, ship.hyper_cooldown - delta)
	_replenish(ship, delta)

	if input.rotate_left and not input.rotate_right:
		ship.angle -= SHIP_TURN * delta
	elif input.rotate_right and not input.rotate_left:
		ship.angle += SHIP_TURN * delta

	if input.thrust:
		ship.vel += ship.forward() * SHIP_THRUST * delta

	# Limite de velocidade: sem teto o navio "foge" da tela.
	if ship.vel.length() > SHIP_MAX_SPEED:
		ship.vel = ship.vel.normalized() * SHIP_MAX_SPEED

	_apply_gravity(ship, delta)
	ship.pos += ship.vel * delta
	_wrap_ship(ship)

	# Tiro e hyperspace sao de borda.
	if input.fire and not ship.fire_held:
		_try_fire(index)
	if input.hyperspace and not ship.hyper_held:
		_try_hyperspace(index)
	ship.fire_held = input.fire
	ship.hyper_held = input.hyperspace

## Paiol: devolve um torpedo por vez, num ritmo mais lento que o tubo.
func _replenish(ship: Ship, delta: float) -> void:
	if ship.ammo >= TORPEDO_MAGAZINE:
		ship.replenish = TORPEDO_REPLENISH
		return
	ship.replenish -= delta
	if ship.replenish <= 0.0:
		ship.ammo += 1
		ship.replenish = TORPEDO_REPLENISH

func _apply_gravity(ship: Ship, delta: float) -> void:
	var to_star: Vector2 = ARENA * 0.5 - ship.pos
	var distance: float = maxf(to_star.length(), GRAVITY_FLOOR)
	ship.vel += to_star.normalized() * (STAR_GRAVITY / (distance * distance)) * delta

## Campo toroidal: o objeto que sai entra do outro lado.
func _wrap_ship(ship: Ship) -> void:
	var wrapped := Vector2(fposmod(ship.pos.x, ARENA.x), fposmod(ship.pos.y, ARENA.y))
	if not wrapped.is_equal_approx(ship.pos):
		events.append({"type": "wrap", "who": ships.find(ship), "pos": wrapped})
	ship.pos = wrapped

func _try_fire(index: int) -> void:
	var ship := ships[index]
	if ship.ammo <= 0 or ship.reload > 0.0:
		events.append({"type": "denied", "who": index})
		return

	ship.ammo -= 1
	ship.reload = TORPEDO_RELOAD

	if torpedoes.size() >= MAX_TORPEDOES:
		return

	var torpedo := Torpedo.new()
	torpedo.owner = index
	torpedo.pos = ship.pos + ship.forward() * (SHIP_RADIUS + 6.0)
	# Velocidade RELATIVA: o torpedo herda o movimento do navio.
	torpedo.vel = ship.vel + ship.forward() * TORPEDO_SPEED
	torpedoes.append(torpedo)
	events.append({"type": "fire", "who": index, "pos": torpedo.pos, "vel": torpedo.vel})

func _try_hyperspace(index: int) -> void:
	var ship := ships[index]
	if ship.hyper_cooldown > 0.0 or not ship.alive:
		events.append({"type": "denied", "who": index})
		return

	var origin := ship.pos
	ship.vel *= HYPERSPACE_VELOCITY_SCALE
	ship.pos = _random_far_point(ship)
	ship.hyper_cooldown = HYPERSPACE_COOLDOWN
	ship.invuln = maxf(ship.invuln, HYPERSPACE_INVULN)
	events.append({"type": "hyper", "who": index, "pos": ship.pos, "from": origin})

## Ponto aleatorio longe do star e do outro navio.
func _random_far_point(seek: Ship) -> Vector2:
	var center := ARENA * 0.5
	var other := ships[1 - ships.find(seek)]
	var best := center + Vector2(SPAWN_CLEARANCE, 0.0)
	for attempt in 24:
		var candidate := Vector2(
			_rng.randf_range(40.0, ARENA.x - 40.0),
			_rng.randf_range(40.0, ARENA.y - 40.0)
		)
		best = candidate
		if candidate.distance_to(center) < SPAWN_CLEARANCE:
			continue
		if other.alive and candidate.distance_to(other.pos) < SPAWN_CLEARANCE * 0.5:
			continue
		break
	return best

func _respawn(index: int) -> void:
	var ship := ships[index]
	var other := ships[1 - index]
	var center := ARENA * 0.5

	# Nase do lado oposto ao adversario, a uma distancia segura do star.
	var away := Vector2(1.0, 0.0) if index == 0 else Vector2(-1.0, 0.0)
	if other.alive:
		var delta := ship.pos - other.pos
		if delta.length() > 1.0:
			away = delta.normalized()

	# O respawn e radial: num canto da arena ele cairia fora dela (o
	# invariante pegou y = -39). Prende dentro, com margem para nao nascer
	# colado na borda e voltar imediatamente pelo campo toroidal.
	ship.pos = center + away * SPAWN_CLEARANCE * 1.9
	ship.pos = Vector2(
		clampf(ship.pos.x, SPAWN_CLEARANCE * 0.6, ARENA.x - SPAWN_CLEARANCE * 0.6),
		clampf(ship.pos.y, SPAWN_CLEARANCE * 0.6, ARENA.y - SPAWN_CLEARANCE * 0.6)
	)
	ship.ammo = TORPEDO_MAGAZINE
	ship.reload = 0.0
	ship.replenish = TORPEDO_REPLENISH
	ship.alive = true
	ship.invuln = 1.2
	ship.angle = (center - ship.pos).angle()
	var r: float = maxf(ship.pos.distance_to(center), GRAVITY_FLOOR)
	var orbital: float = sqrt(STAR_GRAVITY / r)
	ship.vel = Vector2(-away.y, away.x) * orbital
	events.append({"type": "respawn", "who": index, "pos": ship.pos})

## --- torpedos ----------------------------------------------------------

func _update_torpedoes(delta: float) -> void:
	var survivors: Array[Torpedo] = []
	for torpedo in torpedoes:
		torpedo.life -= delta
		if torpedo.life <= 0.0:
			continue

		# O torpedo tambem cai no star.
		var to_star: Vector2 = ARENA * 0.5 - torpedo.pos
		var distance: float = maxf(to_star.length(), GRAVITY_FLOOR)
		torpedo.vel += to_star.normalized() * (STAR_GRAVITY / (distance * distance)) * delta

		torpedo.pos += torpedo.vel * delta
		torpedo.pos = Vector2(fposmod(torpedo.pos.x, ARENA.x), fposmod(torpedo.pos.y, ARENA.y))

		if torpedo.pos.distance_to(ARENA * 0.5) < STAR_RADIUS:
			events.append({"type": "hit", "kind": "star", "pos": torpedo.pos})
			continue

		survivors.append(torpedo)
	torpedoes = survivors

## --- colisoes ----------------------------------------------------------

func _resolve_collisions() -> void:
	var center := ARENA * 0.5

	# Navio no star.
	for i in 2:
		var ship := ships[i]
		if not ship.alive:
			continue
		if ship.pos.distance_to(center) < STAR_RADIUS + SHIP_RADIUS:
			_destroy(i, "star")

	# Torpedo x navio e torpedo x torpedo.
	var consumed := {}
	for a in torpedoes.size():
		for b in range(a + 1, torpedoes.size()):
			if consumed.has(a) or consumed.has(b):
				continue
			if torpedoes[a].pos.distance_to(torpedoes[b].pos) < TORPEDO_RADIUS * 2.0:
				consumed[a] = true
				consumed[b] = true
				events.append({"type": "hit", "kind": "torpedo", "pos": torpedoes[a].pos})

	for index in torpedoes.size():
		if consumed.has(index):
			continue
		var torpedo := torpedoes[index]
		for i in 2:
			var ship := ships[i]
			if not ship.alive or torpedo.owner == i:
				continue # torpedo nao acerta o dono
			if torpedo.pos.distance_to(ship.pos) < SHIP_RADIUS + TORPEDO_RADIUS:
				consumed[index] = true
				events.append({"type": "hit", "kind": "ship", "who": i, "pos": torpedo.pos})
				_destroy(i, "torpedo", torpedo.owner)
				break

	# Colisao entre os dois navios: os dois explodem, ninguem pontua.
	if ships[0].alive and ships[1].alive:
		if ships[0].pos.distance_to(ships[1].pos) < SHIP_RADIUS * 2.0:
			events.append({"type": "hit", "kind": "ship", "who": 0, "pos": ships[0].pos})
			_destroy(0, "ram")
			_destroy(1, "ram")

	if not consumed.is_empty():
		var survivors: Array[Torpedo] = []
		for index in torpedoes.size():
			if not consumed.has(index):
				survivors.append(torpedoes[index])
		torpedoes = survivors

func _destroy(index: int, cause: String, killer: int = -1) -> void:
	var ship := ships[index]
	# Navio invulneravel (respawn/hyperspace) nao morre: o torpedo passa.
	if not ship.alive or ship.invuln > 0.0:
		return

	ship.alive = false
	ship.respawn = RESPAWN_DELAY
	events.append({
		"type": "explode",
		"who": index,
		"cause": cause,
		"pos": ship.pos,
	})

	# Pontuar: quem acertou o torpedo; se o navio se matou (star/ram), o ponto
	# vai para o adversario -- e o que da peso ao star e a colisao.
	var scorer := killer
	if scorer < 0 and cause != "ram":
		scorer = 1 - index
	if scorer >= 0:
		ships[scorer].kills += 1
		events.append({"type": "score", "who": scorer, "kills": ships[scorer].kills})
		if ships[scorer].kills >= WIN_SCORE:
			is_over = true
			winner = scorer
			events.append({"type": "win", "who": scorer})

## --- consultas usadas pela IA e pelo HUD -------------------------------

func ship_distance_to_star(index: int) -> float:
	return ships[index].pos.distance_to(ARENA * 0.5)

func torpedoes_of(index: int) -> Array:
	var result := []
	for torpedo in torpedoes:
		if torpedo.owner == index:
			result.append(torpedo)
	return result

## Torpedo que pode acertar `index` nos proximos quadros (usado pela IA).
func incoming_torpedoes(index: int) -> Array:
	var ship := ships[index]
	var result := []
	for torpedo in torpedoes:
		if torpedo.owner == index:
			continue
		var delta := ship.pos - torpedo.pos
		var distance := delta.length()
		if distance > 260.0:
			continue
		# Aproximando? Projecao da velocidade relativa sobre a linha.
		if torpedo.vel.dot(delta) > 0.0:
			result.append(torpedo)
	return result
