class_name FixedStep
extends RefCounted
## Acumulador de passo fixo.
##
## `_physics_process` recebe um delta que o Engine.time_scale ja multiplicou.
## Acumular esse delta e gastar em passos inteiros de 1/60 s da tres coisas:
## fisica identica em 30, 60 ou 144 Hz; slow motion correto (menos passos por
## segundo real, sem quebrar a integracao); e um teto de passos por quadro, que
## evita a "espiral da morte" quando o aparelho engasga.

var step_seconds := 1.0 / 60.0
var max_steps_per_advance := 3

var _accum := 0.0

## Devolve quantos passos devem rodar neste quadro (0..max_steps_per_advance).
func advance(delta: float) -> int:
	_accum += maxf(delta, 0.0)
	var steps := 0
	while _accum >= step_seconds and steps < max_steps_per_advance:
		_accum -= step_seconds
		steps += 1
	if _accum >= step_seconds:
		# Ficou devendo mais que o teto: descarta a divida em vez de correr
		# atras dela para sempre.
		_accum = 0.0
	return steps

func reset() -> void:
	_accum = 0.0

func pending() -> float:
	return _accum
