class_name PlayerView
extends Node2D
## Desenho do jogador: cabeca, tronco arredondado com contorno luminoso, sombra
## no piso e "squash and stretch" no impacto. Sem textura -- StyleBoxFlat e
## vetor; a cabeca e um circulo com viseira na cor do time.

const Match := preload("res://scripts/sim/match.gd")

@export var side: int = 0
## Anel de luz ao redor do corpo (o "rim light" que separa do fundo).
@export var accent_ring: bool = true

var _pos := Vector2.ZERO
var _squash := Vector2.ONE
var _flash := 0.0
var _grounded := true
var _box: StyleBoxFlat
var _rim: StyleBoxFlat

func _ready() -> void:
	var fill := Art.ACCENT if side == 0 else Art.TEXT_SECONDARY
	var rim := Art.ACCENT_WARM if side == 0 else Art.TEXT_PRIMARY
	_box = Art.player_style(fill, rim, true)
	_rim = Art.line_box(Color(1.0, 1.0, 1.0, 0.0))

func set_softness(enabled: bool) -> void:
	var fill := Art.ACCENT if side == 0 else Art.TEXT_SECONDARY
	var rim := Art.ACCENT_WARM if side == 0 else Art.TEXT_PRIMARY
	_box = Art.player_style(fill, rim, enabled)
	queue_redraw()

func sync(player) -> void:
	_pos = player.pos
	position = _pos
	_grounded = player.grounded
	queue_redraw()

## Compressao no eixo do impacto (squash vertical + alargamento).
func punch(strength: float) -> void:
	_squash = Vector2(1.0 + strength, 1.0 - strength)
	_flash = 1.0

func _process(delta: float) -> void:
	var target := Vector2.ONE
	if not _grounded:
		# No ar o corpo estica um pouco na vertical.
		target = Vector2(0.96, 1.05)
	_squash = _squash.lerp(target, 1.0 - exp(-18.0 * delta))
	_flash = maxf(0.0, _flash - delta * 5.0)
	queue_redraw()

func _draw() -> void:
	var size := Match.PLAYER_SIZE

	# Sombra no piso: cresce e fica mais fraca quando o jogador sobe.
	var height := clampf((Match.FLOOR_Y - (self.position.y + size.y)) / 260.0, 0.0, 1.0)
	var shadow_width: float = size.x * (1.0 - height * 0.45)
	var shadow := Color(0.0, 0.0, 0.0, 0.5 * (1.0 - height * 0.7))
	draw_set_transform(Vector2(size.x * 0.5, 0.0), 0.0, Vector2.ONE)
	draw_circle(Vector2(0.0, Match.FLOOR_Y - self.position.y), shadow_width * 0.5, shadow)
	draw_set_transform(Vector2.ZERO, 0.0, Vector2.ONE)

	# Tudo com pivot no pe: o squash nao descola do chao.
	var pivot := Vector2(size.x * 0.5, size.y)
	draw_set_transform(pivot, 0.0, _squash)

	var head_radius: float = size.x * 0.21
	var head_center := Vector2(0.0, -size.y + head_radius + 2.0)

	# Tronco: do ombro ao chao.
	var torso := Rect2(-size.x * 0.42, -size.y + head_radius * 2.0,
		size.x * 0.84, size.y - head_radius * 2.0)
	draw_style_box(_box, torso)

	# Cabeca (mesma cor clara nos dois jogadores: le como capacete).
	draw_circle(head_center, head_radius, Art.TEXT_PRIMARY)
	draw_arc(head_center, head_radius - 1.0, 0.0, TAU, 28, Art.DIVIDER, 1.5, true)

	# Viseira na cor do time, virada para o lado da rede.
	var team := Art.ACCENT if side == 0 else Art.TEXT_SECONDARY
	var face_x: float = head_radius * 0.35 if side == 0 else -head_radius * 0.35
	draw_rect(Rect2(head_center.x + face_x - head_radius * 0.3,
		head_center.y - head_radius * 0.12, head_radius * 0.9, head_radius * 0.42), team)

	# Numero/faixa do time no peito.
	if accent_ring:
		draw_rect(Rect2(-size.x * 0.26, -size.y * 0.52, size.x * 0.52, 4.0),
			Color(1.0, 1.0, 1.0, 0.30))

	draw_set_transform(Vector2.ZERO, 0.0, Vector2.ONE)

	# Flash branco no impacto.
	if _flash > 0.01:
		draw_rect(Rect2(Vector2.ZERO, size), Color(1.0, 1.0, 1.0, _flash * 0.30))
