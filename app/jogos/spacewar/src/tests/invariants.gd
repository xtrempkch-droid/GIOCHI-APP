extends SceneTree
## Invariantes da simulacao de Spacewar, sem janela (headless).
##
##   godot --headless --path godot --script res://tests/invariants.gd
##
## Cada teste tenta provar UMA regra do original de 1962 (o campo toroidal, a
## gravidade do star, o torpedo com velocidade relativa, o tubo com recarga, o
## hyperspace aleatorio) e o fuzz joga 20 000 passos com a IA nos dois lados.
## Sai com 0 se tudo passar.

const SIM := preload("res://scripts/sim/spacewar.gd")
const FixedStepScript := preload("res://scripts/sim/fixed_step.gd")
const AiScript := preload("res://scripts/sim/ai.gd")

var failures: int = 0

func _initialize() -> void:
	test_nascimento_simetrico()
	test_campo_toroidal()
	test_gravidade_do_star()
	test_torpedo_herda_velocidade()
	test_tubo_recarrega_e_acaba()
	test_torpedo_do_adversario_mata()
	test_star_mata_e_pontua()
	test_hyperspace_sorteia_e_espera()
	test_invulnerabilidade()
	test_passo_fixo()
	test_vencedor()
	test_fuzz_com_ia()

	if failures == 0:
		print("TODOS OS TESTES PASSARAM")
	else:
		print("%d FALHA(S)" % failures)
	quit(0 if failures == 0 else 1)

func check(ok: bool, what: String) -> void:
	if not ok:
		print("FALHA: ", what)
		failures += 1

func make_match(seed_value: int = 1234) -> SpacewarMatch:
	return SIM.new(seed_value)

func idle() -> Variant:
	return SIM.Controls.new()

## Passo com controle explicito nos dois navios.
func step(match_state, p1: Variant = null, p2: Variant = null) -> void:
	match_state.step(1.0 / 60.0, p1 if p1 != null else idle(),
		p2 if p2 != null else idle())

## Roda N passos ignorando eventos.
func run(match_state, steps: int, p1: Variant = null, p2: Variant = null) -> void:
	for i in steps:
		step(match_state, p1, p2)

func inside_arena(pos: Vector2) -> bool:
	return pos.x >= -0.01 and pos.x <= SIM.ARENA.x + 0.01 \
		and pos.y >= -0.01 and pos.y <= SIM.ARENA.y + 0.01

# --- 1. nascimento --------------------------------------------------------

func test_nascimento_simetrico() -> void:
	var m := make_match()
	var center: Vector2 = SIM.ARENA * 0.5
	var d0: float = m.ships[0].pos.distance_to(center)
	var d1: float = m.ships[1].pos.distance_to(center)
	check(absf(d0 - d1) < 0.5, "os dois nascem a mesma distancia do star")
	check(d0 > SIM.SPAWN_CLEARANCE, "ninguem nasce dentro do star")
	check(m.ships[0].alive and m.ships[1].alive, "os dois comecam vivos")
	check(m.ships[0].ammo == SIM.TORPEDO_MAGAZINE, "tubo cheio no nascimento")

# --- 2. campo toroidal ----------------------------------------------------

func test_campo_toroidal() -> void:
	var m := make_match()
	m.ships[0].pos = Vector2(SIM.ARENA.x + 30.0, 300.0)
	step(m)
	check(inside_arena(m.ships[0].pos), "navio que sai pela direita entra pela esquerda")
	check(m.ships[0].pos.x < 100.0, "a volta traz para perto de x=0")

	m.ships[1].pos = Vector2(-40.0, -25.0)
	step(m)
	check(inside_arena(m.ships[1].pos), "navio que sai pela esquerda entra pela direita")

# --- 3. gravidade ---------------------------------------------------------

func test_gravidade_do_star() -> void:
	var m := make_match()
	var center: Vector2 = SIM.ARENA * 0.5
	# Navio parado ao lado do star: em 1 s ja precisa ter caido para dentro.
	m.ships[0].pos = center + Vector2(300.0, 0.0)
	m.ships[0].vel = Vector2.ZERO
	m.ships[0].invuln = 99.0  # para nao explodir durante a medicao
	var before: float = m.ships[0].pos.distance_to(center)
	run(m, 60)
	var after: float = m.ships[0].pos.distance_to(center)
	check(after < before - 1.0, "o star puxa o navio (%.1f -> %.1f)" % [before, after])

	# Um torpedo tambem cai: a rotina de torpedo do original soma a gravidade.
	m.torpedoes.clear()
	var torpedo := SIM.Torpedo.new()
	torpedo.pos = center + Vector2(300.0, 0.0)
	torpedo.vel = Vector2(0.0, 0.0)
	torpedo.owner = 1
	m.torpedoes.append(torpedo)
	run(m, 30)
	check(m.torpedoes.is_empty() or m.torpedoes[0].pos.distance_to(center) < 300.0,
		"o torpedo tambem cai no star")

# --- 4. torpedo com velocidade relativa ------------------------------------

func test_torpedo_herda_velocidade() -> void:
	var m := make_match()
	var ship = m.ships[0]
	ship.pos = SIM.ARENA * 0.5 + Vector2(0.0, 250.0)
	ship.vel = Vector2(120.0, 0.0)
	ship.angle = -PI * 0.5  # apontando para cima
	ship.invuln = 99.0

	var fire = SIM.Controls.new()
	fire.fire = true
	step(m, fire)

	var fired := []
	for torpedo in m.torpedoes:
		if torpedo.owner == 0:
			fired.append(torpedo)
	check(fired.size() == 1, "um torpedo por aperto")
	if fired.size() == 1:
		var expected: Vector2 = ship.vel + ship.forward() * SIM.TORPEDO_SPEED
		var got: Vector2 = fired[0].vel
		check(got.distance_to(expected) < 25.0,
			"torpedo soma a velocidade do navio (esperado %s, obtido %s)" % [expected, got])
		check(got.x > 100.0, "a componente herdada aparece no eixo x")

	# Segurar o botao nao dispara de novo.
	step(m, fire)
	check(m.torpedoes.size() == 1, "segurar o tiro nao dispara em rajada")

# --- 5. tubo --------------------------------------------------------------

func test_tubo_recarrega_e_acaba() -> void:
	var m := make_match()
	var ship = m.ships[0]
	ship.invuln = 99.0
	var fire = SIM.Controls.new()

	# Cada aperto gasta exatamente um torpedo do magazine.
	for shot in SIM.TORPEDO_MAGAZINE:
		var before: int = ship.ammo
		fire.fire = true
		step(m, fire)
		fire.fire = false
		check(ship.ammo == before - 1, "o tiro gasta um torpedo (%d -> %d)"
			% [before, ship.ammo])
		run(m, 40)  # 40/60 s > TORPEDO_RELOAD: o tubo ja recarregou

	# Sem carga e sem recarga pendente: o tiro e recusado com evento.
	ship.ammo = 0
	ship.reload = 0.0
	ship.replenish = SIM.TORPEDO_REPLENISH
	var before_count: int = m.torpedoes.size()
	fire.fire = true
	step(m, fire)
	check(m.torpedoes.size() == before_count, "sem carga nao sai torpedo")
	check(m.events.any(func(e): return e["type"] == "denied"),
		"o jogo responde 'denied' ao tiro sem carga")

	# O paiol devolve um torpedo por TORPEDO_REPLENISH segundos; sem isso a
	# nave ficaria sem torpedo para sempre depois do quarto tiro.
	fire.fire = false
	var steps_needed := int(SIM.TORPEDO_REPLENISH * 60.0) + 2
	run(m, steps_needed)
	check(ship.ammo >= 1, "o paiol devolve torpedo em %.1f s (ammo=%d)"
		% [SIM.TORPEDO_REPLENISH, ship.ammo])
	check(ship.ammo <= SIM.TORPEDO_MAGAZINE, "o paiol nao passa do magazine")

# --- 6. torpedo do adversario mata ---------------------------------------

func test_torpedo_do_adversario_mata() -> void:
	var m := make_match()
	# Alvo parado no meio da arena, longe do star.
	var target = m.ships[1]
	target.pos = SIM.ARENA * 0.5 + Vector2(0.0, 240.0)
	target.vel = Vector2.ZERO
	target.invuln = 0.0
	m.ships[0].invuln = 99.0

	var torpedo := SIM.Torpedo.new()
	torpedo.owner = 0
	torpedo.pos = target.pos + Vector2(0.0, 40.0)
	torpedo.vel = Vector2(0.0, -300.0)
	m.torpedoes.append(torpedo)

	run(m, 20)
	check(not target.alive, "o torpedo do adversario destroi o navio")
	check(m.ships[0].kills == 1, "o dono do torpedo pontua (kills=%d)" % m.ships[0].kills)

# --- 7. star -------------------------------------------------------------

func test_star_mata_e_pontua() -> void:
	var m := make_match()
	m.ships[0].pos = SIM.ARENA * 0.5 + Vector2(SIM.STAR_RADIUS * 0.5, 0.0)
	m.ships[0].vel = Vector2.ZERO
	m.ships[0].invuln = 0.0
	step(m)
	check(not m.ships[0].alive, "navio dentro do star explodiu")
	check(m.ships[1].kills == 1, "quem caiu no star deu o ponto ao adversario")

# --- 8. hyperspace -------------------------------------------------------

func test_hyperspace_sorteia_e_espera() -> void:
	var m := make_match()
	var ship = m.ships[0]
	ship.pos = SIM.ARENA * 0.5 + Vector2(0.0, 260.0)
	ship.vel = Vector2(200.0, 0.0)
	ship.invuln = 99.0
	ship.hyper_cooldown = 0.0
	var origin: Vector2 = ship.pos

	var hyper = SIM.Controls.new()
	hyper.hyperspace = true
	step(m, hyper)
	check(ship.pos.distance_to(origin) > 60.0, "hyperspace muda a posicao")
	check(inside_arena(ship.pos), "hyperspace cai dentro da arena")
	check(ship.pos.distance_to(SIM.ARENA * 0.5) > SIM.STAR_RADIUS * 2.0,
		"hyperspace nao cai no star")
	check(ship.vel.length() < 200.0, "hyperspace escala a velocidade para baixo")
	check(ship.hyper_cooldown > 0.0, "hyperspace entra em recarga")

	# Segundo aperto com o cooldown ativo nao move.
	hyper.hyperspace = false
	step(m, hyper)
	var after: Vector2 = ship.pos
	hyper.hyperspace = true
	step(m, hyper)
	check(ship.pos.distance_to(after) < 40.0, "hyperspace em recarga nao teleporta")

# --- 9. invulnerabilidade -------------------------------------------------

func test_invulnerabilidade() -> void:
	var m := make_match()
	var ship = m.ships[0]
	ship.pos = SIM.ARENA * 0.5 + Vector2(0.0, 240.0)
	ship.vel = Vector2.ZERO
	ship.invuln = 1.0

	var torpedo := SIM.Torpedo.new()
	torpedo.owner = 1
	torpedo.pos = ship.pos + Vector2(0.0, 20.0)
	torpedo.vel = Vector2(0.0, -200.0)
	m.torpedoes.append(torpedo)

	run(m, 4)
	check(ship.alive, "navio recem-nascido nao morre no primeiro torpedo")

# --- 10. passo fixo (base do slow motion) --------------------------------

func test_passo_fixo() -> void:
	var clock = FixedStepScript.new()
	var step_seconds: float = clock.step_seconds

	var total := 0
	for i in 60:
		total += clock.advance(step_seconds)
	check(total == 60, "60 quadros a 60 Hz = 60 passos (obtido %d)" % total)

	clock.reset()
	total = 0
	for i in 60:
		total += clock.advance(step_seconds * 0.25)
	check(total == 15, "slow motion 0,25x roda 15 passos (obtido %d)" % total)

	clock.reset()
	var burst: int = clock.advance(1.0)
	check(burst == clock.max_steps_per_advance, "travamento respeita o teto")
	check(clock.pending() == 0.0, "travamento nao deixa divida")

# --- 11. vencedor --------------------------------------------------------

func test_vencedor() -> void:
	var m := make_match()
	m.ships[1].invuln = 0.0
	m.ships[0].kills = SIM.WIN_SCORE - 1
	m.ships[1].pos = SIM.ARENA * 0.5 + Vector2(0.0, 240.0)
	m.ships[1].vel = Vector2.ZERO

	var torpedo := SIM.Torpedo.new()
	torpedo.owner = 0
	torpedo.pos = m.ships[1].pos + Vector2(0.0, 30.0)
	torpedo.vel = Vector2(0.0, -300.0)
	m.torpedoes.append(torpedo)

	run(m, 20)
	check(m.is_over, "a partida fecha em %d abates" % SIM.WIN_SCORE)
	check(m.winner == 0, "o vencedor e quem chegou em %d" % SIM.WIN_SCORE)

	var snapshot: Vector2 = m.ships[1].pos
	run(m, 30)
	check(m.ships[1].pos == snapshot, "partida encerrada nao continua simulando")

# --- 12. fuzz com a IA dos dois lados -----------------------------------

func test_fuzz_com_ia() -> void:
	var m := make_match(12345)
	var ai1 = AiScript.new()
	var ai2 = AiScript.new()
	var kills := 0
	var last_total := 0
	var steps_without_kill := 0
	var worst := 0
	var matches := 0

	for i in 20000:
		if m.is_over:
			matches += 1
			m.restart(12345 + matches)
			ai1.reset()
			ai2.reset()
			last_total = 0

		var p1 = ai1.decide(m, 0)
		var p2 = ai2.decide(m, 1)
		m.step(1.0 / 60.0, p1, p2)

		var total: int = m.ships[0].kills + m.ships[1].kills
		if total != last_total:
			last_total = total
			kills += 1
			steps_without_kill = 0
		else:
			steps_without_kill += 1
			worst = maxi(worst, steps_without_kill)

		if not invariant(m, "fuzz q%d" % i):
			break

	check(kills > 0, "a IA pontua")
	check(matches > 0, "a partida termina (partidas completas: %d)" % matches)
	# 2400 passos = 40 s sem ninguem morrer: a partida travou. O primeiro
	# abate sai por volta de 23 s (medido em tres sementes); 40 s deixa folga
	# para nao dar falso positivo quando alguem erra a esquiva.
	check(worst < 2400, "fuzz nao trava (maior espera %d passos)" % worst)
	print("  fuzz: %d abates, %d partidas, maior espera %d passos (%.1f s)"
		% [kills, matches, worst, worst / 60.0])

## Regras que precisam valer em TODO passo.
func invariant(m, where: String) -> bool:
	for i in 2:
		var ship = m.ships[i]
		if not (is_finite(ship.pos.x) and is_finite(ship.pos.y)
				and is_finite(ship.vel.x) and is_finite(ship.vel.y)):
			check(false, "navio %d com NaN (%s)" % [i, where])
			return false
		if not inside_arena(ship.pos):
			check(false, "navio %d fora da arena: %s (%s)" % [i, ship.pos, where])
			return false
		if ship.vel.length() > SIM.SHIP_MAX_SPEED * 1.6:
			check(false, "navio %d acima da velocidade: %.1f (%s)"
				% [i, ship.vel.length(), where])
			return false
		if ship.ammo < 0 or ship.ammo > SIM.TORPEDO_MAGAZINE:
			check(false, "navio %d com carga invalida: %d (%s)" % [i, ship.ammo, where])
			return false
	for torpedo in m.torpedoes:
		if not inside_arena(torpedo.pos):
			check(false, "torpedo fora da arena: %s (%s)" % [torpedo.pos, where])
			return false
		if torpedo.life <= 0.0:
			check(false, "torpedo vencido na lista (%s)" % where)
			return false
	if m.torpedoes.size() > SIM.MAX_TORPEDOES:
		check(false, "torpedos demais: %d (%s)" % [m.torpedoes.size(), where])
		return false
	return true
