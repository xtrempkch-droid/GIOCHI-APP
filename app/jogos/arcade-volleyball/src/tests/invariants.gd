extends SceneTree
## Invariantes da simulacao, rodando sem janela (headless).
##
##   godot --headless --path godot --script res://tests/invariants.gd
##
## Mesmo espirito do harness descartavel usado no port C++: o modelo roda
## milhares de passos e cada passo precisa respeitar as regras (bola dentro da
## area, ninguem atravessa a rede, partida nao trava). Sai com 0 se tudo passar.

const Match := preload("res://scripts/sim/match.gd")
const FixedStepScript := preload("res://scripts/sim/fixed_step.gd")

var failures: int = 0

func _initialize() -> void:
	test_movimento_e_limites()
	test_pulo_de_borda()
	test_saque()
	test_partida_longa()
	test_vencedor()
	test_passo_fixo()
	test_fuzz()

	if failures == 0:
		print("TODOS OS TESTES PASSARAM")
	else:
		print("%d FALHA(S)" % failures)
	quit(0 if failures == 0 else 1)

func check(ok: bool, what: String) -> void:
	if not ok:
		print("FALHA: ", what)
		failures += 1

func make_match() -> VolleyMatch:
	return Match.new()

func held(left := false, right := false, jump := false) -> Variant:
	var input = Match.Controls.new()
	input.left = left
	input.right = right
	input.jump = jump
	return input

func idle() -> Variant:
	return held()

# --- 1. movimento ---------------------------------------------------------

func test_movimento_e_limites() -> void:
	var m := make_match()
	var x0: float = m.players[0].pos.x
	for i in 30:
		m.step(held(false, true))
	check(m.players[0].pos.x > x0, "segurar direita move o P1")
	check(m.players[0].pos.x + Match.PLAYER_SIZE.x <= Match.NET_X + 0.001,
		"P1 nao passa da rede")

	for i in 400:
		m.step(held(true, false))
	check(m.players[0].pos.x == 0.0, "P1 para na parede esquerda")
	check(m.players[1].pos.x >= Match.NET_X + Match.NET_WIDTH - 0.001,
		"P2 fica do lado direito")

# --- 2. pulo --------------------------------------------------------------

func test_pulo_de_borda() -> void:
	var m := make_match()
	m.step(held(false, false, true))
	check(not m.players[0].grounded, "pulo tira do chao")
	check(m.players[0].vel.y < 0.0, "pulo sobe")

	# Continua com o botao apertado: nao pode repicar ao aterrissar.
	var airborne := 0
	for i in 400:
		m.step(held(false, false, true))
		if not m.players[0].grounded:
			airborne += 1
	check(airborne < 400, "pulo nao repica com o botao segurado")
	check(m.players[0].grounded, "jogador volta ao chao")

# --- 3. saque -------------------------------------------------------------

func test_saque() -> void:
	var m := make_match()
	m.ball.serve(true)
	check(m.ball.pos.x < Match.ARENA.x * 0.5 and m.ball.vel.x > 0.0,
		"saque da esquerda vai para a direita")
	m.ball.serve(false)
	check(m.ball.pos.x > Match.ARENA.x * 0.5 and m.ball.vel.x < 0.0,
		"saque da direita vai para a esquerda")

# --- 4. partida longa com o P1 jogando ------------------------------------

func invariants(m: VolleyMatch, where: String) -> bool:
	var r := Match.BALL_RADIUS
	var p1 = m.players[0]
	var p2 = m.players[1]

	if not (is_finite(m.ball.pos.x) and is_finite(m.ball.pos.y)):
		check(false, "bola com NaN/inf (%s)" % where)
		return false
	if m.ball.pos.x - r < -0.01 or m.ball.pos.x + r > Match.ARENA.x + 0.01:
		check(false, "bola fora do eixo x: %.1f (%s)" % [m.ball.pos.x, where])
		return false
	if m.ball.pos.y - r < -0.01:
		check(false, "bola acima do teto: %.1f (%s)" % [m.ball.pos.y, where])
		return false
	var net_top: float = Match.FLOOR_Y - Match.NET_HEIGHT
	var net_right: float = Match.NET_X + Match.NET_WIDTH
	if (m.ball.pos.y + r > net_top and m.ball.pos.x + r > Match.NET_X
			and m.ball.pos.x - r < net_right):
		check(false, "bola dentro da rede: %.1f,%.1f (%s)"
			% [m.ball.pos.x, m.ball.pos.y, where])
		return false
	if p1.pos.x < -0.01 or p1.pos.x + Match.PLAYER_SIZE.x > Match.NET_X + 0.01:
		check(false, "P1 atravessou a rede: %.1f (%s)" % [p1.pos.x, where])
		return false
	if p2.pos.x < net_right - 0.01 or p2.pos.x + Match.PLAYER_SIZE.x > Match.ARENA.x + 0.01:
		check(false, "P2 atravessou a rede: %.1f (%s)" % [p2.pos.x, where])
		return false
	if p1.pos.y + Match.PLAYER_SIZE.y > Match.FLOOR_Y + 0.01:
		check(false, "P1 afundou no chao (%s)" % where)
		return false
	if p2.pos.y + Match.PLAYER_SIZE.y > Match.FLOOR_Y + 0.01:
		check(false, "P2 afundou no chao (%s)" % where)
		return false
	return true

func test_partida_longa() -> void:
	var m := make_match()
	var points := 0
	for i in 6000:
		if m.is_over:
			m.restart()
		# P1 segue a bola: mesma politica do harness do port C++.
		var delta: float = m.ball.pos.x - m.players[0].center_x()
		var jump: bool = (m.ball.pos.y < m.players[0].pos.y
			and absf(delta) < 60.0 and m.players[0].grounded)
		var input = held(delta < -8.0, delta > 8.0, jump)
		var before: int = m.players[0].score + m.players[1].score
		m.step(input)
		if m.players[0].score + m.players[1].score != before:
			points += 1
		if not invariants(m, "partida longa q%d" % i):
			break
	check(points > 0, "partida longa marca pontos")
	print("  partida longa: %d pontos, placar %d x %d"
		% [points, m.players[0].score, m.players[1].score])

# --- 5. fim de set --------------------------------------------------------

func test_vencedor() -> void:
	var m := make_match()
	m.players[0].score = Match.WIN_SCORE - 1
	# Bola caindo do lado do P2 (x > rede): ponto do P1 fecha o set.
	m.ball.pos = Vector2(Match.NET_X + 100.0, Match.FLOOR_Y - Match.BALL_RADIUS - 1.0)
	m.ball.vel = Vector2(0.0, 6.0)
	m.step(idle())
	check(m.is_over, "set fecha ao chegar em %d" % Match.WIN_SCORE)
	check(m.winner == 0, "vencedor e quem chegou em %d" % Match.WIN_SCORE)
	check(m.players[0].score == Match.WIN_SCORE, "placar do vencedor")

	var pos := m.ball.pos
	m.step(idle())
	check(m.ball.pos == pos, "partida encerrada nao continua simulando")

# --- 6. acumulador de passo fixo (base do slow motion) --------------------

func test_passo_fixo() -> void:
	var clock = FixedStepScript.new()
	var step: float = clock.step_seconds

	# 60 quadros de 1/60 s: exatamente 60 passos.
	var total := 0
	for i in 60:
		total += clock.advance(step)
	check(total == 60, "60 quadros a 60 Hz = 60 passos (obtido %d)" % total)

	# Slow motion 0,25x: um quarto dos passos no mesmo numero de quadros.
	clock.reset()
	total = 0
	for i in 60:
		total += clock.advance(step * 0.25)
	check(total == 15, "slow motion 0,25x roda 15 passos em 60 quadros (obtido %d)" % total)

	# Framerate alto: 240 quadros de 1/240 s tambem dao 60 passos.
	clock.reset()
	total = 0
	for i in 240:
		total += clock.advance(step * 0.25)
	check(total == 60, "240 quadros a 240 Hz = 60 passos (obtido %d)" % total)

	# Engasgo de 1 s: nao roda mais que o teto nem acumula divida.
	clock.reset()
	var burst: int = clock.advance(1.0)
	check(burst == clock.max_steps_per_advance,
		"travamento respeita o teto de passos (obtido %d)" % burst)
	check(clock.pending() == 0.0, "travamento nao deixa divida acumulada")

# --- 7. fuzz com entrada aleatoria ---------------------------------------

func test_fuzz() -> void:
	var m := make_match()
	var rng := RandomNumberGenerator.new()
	rng.seed = 12345
	var points := 0
	var steps_without_point := 0
	var worst := 0

	for i in 20000:
		if m.is_over:
			m.restart()
		var roll := rng.randi_range(0, 10)
		var input = held(roll < 4, roll > 6, roll == 5)
		var before: int = m.players[0].score + m.players[1].score
		m.step(input)
		if m.players[0].score + m.players[1].score != before:
			points += 1
			steps_without_point = 0
		else:
			steps_without_point += 1
			worst = maxi(worst, steps_without_point)
		if not invariants(m, "fuzz q%d" % i):
			break

	check(points > 0, "fuzz marca pontos")
	# 600 passos = 10 s sem ninguem pontuar: partida travada.
	check(worst < 600, "fuzz nao trava (maior espera %d passos)" % worst)
	print("  fuzz: %d pontos, maior espera %d passos (%.1f s)"
		% [points, worst, worst / 60.0])
