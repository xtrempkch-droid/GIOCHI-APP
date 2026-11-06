class_name VolleyMatch
extends RefCounted
## Regras da partida, sem nenhuma dependencia do engine grafico.
##
## O modelo avanca em passos fixos de 1/60 s e recebe a entrada por um objeto
## Controls; nada aqui le o singleton Input, desenha ou toca som. Isso permite rodar a
## simulacao inteira em headless (tests/invariants.gd).
##
## As constantes sao as mesmas de arcade_volleyball.cpp, que ja passaram pelo
## teste de invariantes e pelo fuzz de 20 000 quadros: mudar uma aqui exige
## mudar la, ou aceitar duas fisicas diferentes.

## Area logica. O `window/stretch` do project.godot escala para o painel, como
## o SDL_RenderSetLogicalSize faz no port C++.
const ARENA := Vector2(1280.0, 720.0)
## Faixa de chao desenhada no rodape.
const FLOOR_Y := 670.0
const NET_WIDTH := 16.0
const NET_HEIGHT := 250.0
const NET_X := 632.0

## Gravidade por passo (o modelo roda a 60 passos por segundo), igual ao port
## C++. A bola cai mais devagar que o jogador de proposito.
const GRAVITY := 0.6
const BALL_GRAVITY := 0.42

const BALL_RADIUS := 20.0
const BALL_MAX_SPEED := 15.0
const PLAYER_SIZE := Vector2(80.0, 100.0)
const PLAYER_SPEED := 8.0
const PLAYER_JUMP := -14.0

## Volei de rally: quem chega a 15 primeiro fecha o set.
const WIN_SCORE := 15

## Entrada de um passo. O pulo e de borda: segurar o botao nao repica.
## (Nao pode se chamar `Input`: colide com a classe nativa do engine.)
class Controls:
	var left := false
	var right := false
	var jump := false

class Player:
	var pos := Vector2.ZERO
	var vel := Vector2.ZERO
	var grounded := true
	var score := 0
	## Ultimo estado do pulo, para detectar a borda de descida.
	var jump_held := false

	func center_x() -> float:
		return pos.x + PLAYER_SIZE.x * 0.5

	func place(start_x: float) -> void:
		pos = Vector2(start_x, FLOOR_Y - PLAYER_SIZE.y)
		vel = Vector2.ZERO
		grounded = true
		jump_held = false

class Ball:
	var pos := Vector2.ZERO
	var vel := Vector2.ZERO

	## Saque do lado de quem fez o ponto, indo em direcao ao outro lado.
	func serve(from_left: bool) -> void:
		pos = Vector2(ARENA.x * 0.25 if from_left else ARENA.x * 0.75, ARENA.y * 0.3)
		vel = Vector2(4.0 if from_left else -4.0, 0.0)

## Eventos de um passo, consumidos pela camada visual (juice) e pelo audio.
## type: "hit" | "net" | "point" | "win"
var events: Array[Dictionary] = []

var players: Array[Player] = [Player.new(), Player.new()]
var ball := Ball.new()
var is_over := false
var winner := -1

func _init() -> void:
	players[0].place(ARENA.x * 0.2 - 40.0)
	players[1].place(ARENA.x * 0.8 - 40.0)
	ball.serve(true)

## Recomeca a partida do zero, placar incluido.
func restart() -> void:
	for p in players:
		p.score = 0
		p.jump_held = false
	players[0].place(ARENA.x * 0.2 - 40.0)
	players[1].place(ARENA.x * 0.8 - 40.0)
	ball.serve(true)
	is_over = false
	winner = -1
	events.clear()

## Avanca um passo fixo. `human` controla o jogador 0; o 1 e IA.
func step(human: Controls) -> void:
	events.clear()
	if is_over:
		return

	_step_player(players[0], human.left, human.right, human.jump)
	_step_ai(players[1])
	_step_ball()

# --- jogadores ---------------------------------------------------------

func _step_player(p: Player, left: bool, right: bool, jump: bool) -> void:
	if left and not right:
		p.vel.x = -PLAYER_SPEED
	elif right and not left:
		p.vel.x = PLAYER_SPEED
	else:
		p.vel.x = 0.0

	# Borda de descida: segurar o botao nao repica no chao.
	if jump and not p.jump_held and p.grounded:
		p.vel.y = PLAYER_JUMP
		p.grounded = false
	p.jump_held = jump

	p.vel.y += GRAVITY
	p.pos += p.vel
	_apply_bounds(p, 0)

func _apply_bounds(p: Player, index: int) -> void:
	var net_right := NET_X + NET_WIDTH
	if index == 0:
		p.pos.x = clampf(p.pos.x, 0.0, NET_X - PLAYER_SIZE.x)
	else:
		p.pos.x = clampf(p.pos.x, net_right, ARENA.x - PLAYER_SIZE.x)

	var floor_line := FLOOR_Y - PLAYER_SIZE.y
	if p.pos.y >= floor_line:
		p.pos.y = floor_line
		p.vel.y = 0.0
		p.grounded = true

## IA simples: segue a bola quando ela esta do lado direito, senao volta para a
## posicao de espera. Pula quando a bola chega perto e esta acima.
func _step_ai(p: Player) -> void:
	var net_right := NET_X + NET_WIDTH
	var jump := false

	if ball.pos.x > net_right:
		var delta := ball.pos.x - p.center_x()
		if delta < -10.0:
			p.vel.x = -PLAYER_SPEED * 0.8
		elif delta > 10.0:
			p.vel.x = PLAYER_SPEED * 0.8
		else:
			p.vel.x = 0.0

		if ball.pos.y < p.pos.y and p.grounded and absf(ball.pos.x - p.center_x()) < 50.0:
			jump = true
	else:
		var home_x := ARENA.x * 0.75 - PLAYER_SIZE.x * 0.5
		if p.pos.x < home_x - 10.0:
			p.vel.x = PLAYER_SPEED * 0.5
		elif p.pos.x > home_x + 10.0:
			p.vel.x = -PLAYER_SPEED * 0.5
		else:
			p.vel.x = 0.0

	if jump and not p.jump_held and p.grounded:
		p.vel.y = PLAYER_JUMP
		p.grounded = false
	p.jump_held = jump

	p.vel.y += GRAVITY
	p.pos += p.vel
	_apply_bounds(p, 1)

# --- bola --------------------------------------------------------------

func _step_ball() -> void:
	ball.vel.y += BALL_GRAVITY
	ball.pos += ball.vel

	if ball.pos.x - BALL_RADIUS < 0.0:
		ball.pos.x = BALL_RADIUS
		ball.vel.x = -ball.vel.x
		_nudge_vertical(ball.pos.x)
	if ball.pos.x + BALL_RADIUS > ARENA.x:
		ball.pos.x = ARENA.x - BALL_RADIUS
		ball.vel.x = -ball.vel.x
		_nudge_vertical(ball.pos.x)
	if ball.pos.y - BALL_RADIUS < 0.0:
		ball.pos.y = BALL_RADIUS
		ball.vel.y = -ball.vel.y

	_collide_player(players[0], 0)
	_collide_player(players[1], 1)
	_collide_net()

	# Uma colisao pode empurrar a bola para fora da tela por um passo.
	ball.pos.x = clampf(ball.pos.x, BALL_RADIUS, ARENA.x - BALL_RADIUS)
	ball.pos.y = maxf(ball.pos.y, BALL_RADIUS)

	if ball.pos.y + BALL_RADIUS > FLOOR_Y:
		_score_point()

func _score_point() -> void:
	# Caiu do lado do jogador 0 (x < rede): ponto do jogador 1.
	var scorer := 1 if ball.pos.x < NET_X else 0
	players[scorer].score += 1
	events.append({
		"type": "point",
		"side": scorer,
		"pos": Vector2(ball.pos.x, FLOOR_Y),
	})
	if players[scorer].score >= WIN_SCORE:
		is_over = true
		winner = scorer
		events.append({"type": "win", "side": scorer})
		return
	ball.serve(scorer == 0)

## A rede e solida: a bola so passa por cima dela.
func _collide_net() -> void:
	var net_right := NET_X + NET_WIDTH
	var net_top := FLOOR_Y - NET_HEIGHT

	if ball.pos.y + BALL_RADIUS <= net_top:
		return
	if ball.pos.x + BALL_RADIUS <= NET_X or ball.pos.x - BALL_RADIUS >= net_right:
		return

	var impact := absf(ball.vel.x)
	if ball.vel.x > 0.0:
		ball.pos.x = NET_X - BALL_RADIUS
		ball.vel.x = -ball.vel.x
	elif ball.vel.x < 0.0:
		ball.pos.x = net_right + BALL_RADIUS
		ball.vel.x = -ball.vel.x
	else:
		# Bola parada em cima da fita: empurra para o lado mais proximo, senao
		# a partida trava sem ninguem pontuar.
		if ball.pos.x < NET_X + NET_WIDTH * 0.5:
			ball.pos.x = NET_X - BALL_RADIUS
		else:
			ball.pos.x = net_right + BALL_RADIUS
	events.append({"type": "net", "pos": ball.pos, "power": impact})

func _collide_player(p: Player, index: int) -> void:
	var closest := Vector2(
		clampf(ball.pos.x, p.pos.x, p.pos.x + PLAYER_SIZE.x),
		clampf(ball.pos.y, p.pos.y, p.pos.y + PLAYER_SIZE.y)
	)
	var delta := ball.pos - closest
	var distance_squared := delta.length_squared()

	# Centro da bola dentro do jogador: nao ha normal para refletir.
	if distance_squared <= 0.0 or distance_squared >= BALL_RADIUS * BALL_RADIUS:
		return

	var normal := delta / sqrt(distance_squared)
	ball.pos = closest + normal * BALL_RADIUS

	var kick := normal * BALL_MAX_SPEED
	# Bola na cabeca (normal quase vertical) quicaria na mesma vertical para
	# sempre: o jogador se reposiciona e a bola volta para ele. O desvio sai
	# para o lado, afastando-se do centro do jogador.
	if absf(normal.x) < 0.2:
		var side := -1.0 if ball.pos.x < p.center_x() else 1.0
		kick.x = side * BALL_MAX_SPEED
		if kick.y < 0.0:
			kick.y *= 0.6

	ball.vel = Vector2(
		clampf(kick.x + p.vel.x * 0.5, -BALL_MAX_SPEED, BALL_MAX_SPEED),
		clampf(kick.y + p.vel.y * 0.5, -BALL_MAX_SPEED, BALL_MAX_SPEED)
	)
	events.append({
		"type": "hit",
		"who": index,
		"pos": ball.pos,
		"power": ball.vel.length(),
	})
	_nudge_vertical(ball.pos.x)

## Devolve um pouco de horizontal quando o quique fica praticamente vertical
## junto de uma parede -- o unico caso em que ninguem alcanca a bola.
func _nudge_vertical(x: float) -> void:
	if absf(ball.vel.x) >= 0.5:
		return
	ball.vel.x = 3.0 if x <= ARENA.x * 0.5 else -3.0
