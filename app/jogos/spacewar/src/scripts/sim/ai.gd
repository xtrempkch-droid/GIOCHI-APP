class_name SpacewarAi
extends RefCounted
## IA do segundo navio. Pura: recebe o estado e devolve Controls.
##
## Ela existe porque o console tem um gamepad so -- com dois controles
## conectados, o segundo jogador assume e a IA sai de cena. As tres decisoes,
## em ordem de urgencia: fugir do star, desviar de torpedo, atacar.

const SIM := preload("res://scripts/sim/spacewar.gd")

## Distancia em que a IA considera o star perigoso.
const STAR_DANGER := 185.0
## Distancia em que um torpedo deixa de ser ignoravel.
const THREAT_RADIUS := 210.0
## Distancia de combate: mais perto que isso, nao vale gastar torpedo.
const MIN_FIRE_DISTANCE := 120.0
const MAX_FIRE_DISTANCE := 760.0
## Erro angular tolerado para atirar (rad).
const FIRE_ANGLE := 0.13
## Janela de simulacao usada para saber se o empuxo joga o navio no star.
const THRUST_HORIZON := 0.9
## Distancia abaixo da qual a IA usa hyperspace para escapar de torpedo.
const PANIC_DISTANCE := 95.0
## Tempo de reacao da IA (s). Sem atraso ela desviava de 100% dos torpedos e a
## partida nao terminava: medido em 2 abates por 333 s de jogo.
const REACTION_TIME := 0.30

## Angulo desejado persistido entre passos: sem isso a IA oscila em torno do
## alvo e nunca aponta.
var _aim_angle := 0.0
var _has_aim := false
var _reaction := 0.0
var _threat_dir := Vector2.ZERO
var _threat_known := false
var _threat_panic := false

func reset() -> void:
	_has_aim = false
	_reaction = 0.0
	_threat_known = false
	_threat_panic = false

func decide(match_state, index: int) -> Variant:
	var controls = SIM.Controls.new()
	var ship = match_state.ships[index]
	if not ship.alive:
		return controls

	# A decisao de desviar e tomada em intervalos de REACTION_TIME, nao a cada
	# quadro: e isso que torna a IA boa sem ser invencivel.
	_reaction -= 1.0 / 60.0
	if _reaction <= 0.0:
		_reaction = REACTION_TIME
		_scan_threats(match_state, index)

	var center: Vector2 = SIM.ARENA * 0.5
	var to_star: Vector2 = center - ship.pos
	var star_distance := to_star.length()

	# 1. O star vem primeiro: sem isso a IA se mata sozinha.
	if star_distance < STAR_DANGER:
		return _get_away(ship, to_star.normalized(), controls)

	# 2. Torpedo a caminho: desvia na direcao decidida no ultimo scan.
	if _threat_known:
		if _threat_panic and ship.hyper_cooldown <= 0.0:
			controls.hyperspace = true
			return controls
		var evade: Vector2 = _threat_dir
		if evade.dot(ship.forward()) < 0.0:
			evade = -evade
		return _get_away(ship, evade, controls)

	# 3. Atacar: mira com antecedencia, aproxima se estiver longe.
	var opponent = match_state.ships[1 - index]
	if not opponent.alive:
		return controls

	var distance: float = ship.pos.distance_to(opponent.pos)
	var target: Vector2 = _intercept_point(match_state, ship, opponent)
	var desired: float = (target - ship.pos).angle()

	_aim_angle = desired
	_has_aim = true

	_point_at(ship, desired, controls)

	var error: float = absf(angle_difference(ship.angle, desired))
	# Thrust so quando alinhado E quando o empuxo nao joga o navio no star.
	if error < 0.7 and distance > 260.0 and _thrust_is_safe(match_state, ship):
		controls.thrust = true

	if distance > MIN_FIRE_DISTANCE and distance < MAX_FIRE_DISTANCE:
		if error < FIRE_ANGLE and ship.reload <= 0.0 and ship.ammo > 0:
			controls.fire = true

	return controls

## Le os torpedos que podem acertar e decide, de uma vez, se desvia e para onde.
## O desvio e tangente ao torpedo mais proximo -- a manobra classica de esquiva.
func _scan_threats(match_state, index: int) -> void:
	var ship = match_state.ships[index]
	var threats = match_state.incoming_torpedoes(index)
	_threat_known = false
	_threat_panic = false
	if threats.is_empty():
		return

	var closest = threats[0]
	var distance: float = ship.pos.distance_to(closest.pos)
	_threat_known = true
	_threat_panic = distance < PANIC_DISTANCE

	var direction: Vector2 = (closest.pos - ship.pos).normalized()
	_threat_dir = Vector2(-direction.y, direction.x)

## Vira o navio para `direction`, com histerese para nao tremer no alvo.
func _get_away(ship, direction: Vector2, controls) -> Variant:
	_point_at(ship, direction.angle(), controls)
	# Fugir do star sempre pode empurrar: e o que tira o navio de la.
	controls.thrust = true
	return controls

## Ponto onde atirar: o alvo nao anda em linha reta (esta orbitando), entao a
## mira ingenua erra por dezenas de px. Itera tres vezes somando a aceleracao
## centripeta do alvo: e um intercepto de segunda ordem.
func _intercept_point(match_state, ship, opponent) -> Vector2:
	var center: Vector2 = SIM.ARENA * 0.5
	var travel := 0.0
	var target: Vector2 = opponent.pos
	for iteration in 3:
		target = _advance_with_gravity(opponent.pos, opponent.vel, travel)
		travel = clampf(ship.pos.distance_to(target) / SIM.TORPEDO_SPEED, 0.0, 1.8)
	return target

func _advance_with_gravity(pos: Vector2, vel: Vector2, seconds: float) -> Vector2:
	if seconds <= 0.0:
		return pos
	var to_star: Vector2 = SIM.ARENA * 0.5 - pos
	var distance: float = maxf(to_star.length(), SIM.GRAVITY_FLOOR)
	var accel: Vector2 = to_star.normalized() * (SIM.STAR_GRAVITY / (distance * distance))
	return pos + vel * seconds + accel * 0.5 * seconds * seconds

## Simula o empuxo por THRUST_HORIZON segundos: se em algum momento o navio
## entrar na zona do star, nao vale empurrar agora. Sem isso a IA mergulhava
## no star para "perseguir" o adversario (medido: 2 abates em 60 s, os dois
## por suicidio).
func _thrust_is_safe(match_state, ship) -> bool:
	var center: Vector2 = SIM.ARENA * 0.5
	var pos: Vector2 = ship.pos
	var vel: Vector2 = ship.vel + ship.forward() * SIM.SHIP_THRUST * (1.0 / 60.0)
	var limit: float = STAR_DANGER * 0.85
	var steps := int(THRUST_HORIZON * 60.0)
	for i in steps:
		var to_star: Vector2 = center - pos
		var distance: float = maxf(to_star.length(), SIM.GRAVITY_FLOOR)
		vel += to_star.normalized() * (SIM.STAR_GRAVITY / (distance * distance)) * (1.0 / 60.0)
		pos += vel * (1.0 / 60.0)
		if pos.distance_to(center) < limit:
			return false
	return true

func _point_at(ship, desired: float, controls) -> void:
	var error: float = angle_difference(ship.angle, desired)
	controls.rotate_left = error < -0.05
	controls.rotate_right = error > 0.05
