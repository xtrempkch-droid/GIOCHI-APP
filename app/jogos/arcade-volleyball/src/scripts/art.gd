class_name Art
extends RefCounted
## Paleta e recursos visuais gerados em tempo de execucao.
##
## Nenhum PNG entra no pacote: a bola, o brilho e a poeira usam um unico
## GradientTexture2D radial (128x128, ~64 KB de VRAM) e o resto e desenho
## vetorial. A paleta vem de assets/theme.json (tema dark_default) para o jogo
## nao destoar do Launcher.

const BG := Color("0f0f0f")
const SURFACE := Color("1e1e1e")
const SURFACE_ELEVATED := Color("2a2a2a")
const DIVIDER := Color("333333")
const TEXT_PRIMARY := Color("e5e5e5")
const TEXT_SECONDARY := Color("a0a0a0")
const ACCENT := Color("e50914")
## Derivada do accent, so para a luz de contorno (nao existe no tema).
const ACCENT_WARM := Color("ff6b5e")

## Textura radial reaproveitada por brilho da bola e particulas.
static func radial_texture(size: int = 128) -> GradientTexture2D:
	var gradient := Gradient.new()
	gradient.set_color(0, Color(1.0, 1.0, 1.0, 1.0))
	gradient.set_color(1, Color(1.0, 1.0, 1.0, 0.0))
	gradient.add_point(0.45, Color(1.0, 1.0, 1.0, 0.35))

	var texture := GradientTexture2D.new()
	texture.gradient = gradient
	texture.fill = GradientTexture2D.FILL_RADIAL
	texture.fill_from = Vector2(0.5, 0.5)
	texture.fill_to = Vector2(1.0, 0.5)
	texture.width = size
	texture.height = size
	return texture

## Corpo do jogador: cantos arredondados, contorno luminoso e sombra projetada.
## `softness` desliga a sombra no preset baixo (custa um passe extra).
static func player_style(fill: Color, rim: Color, softness: bool) -> StyleBoxFlat:
	var box := StyleBoxFlat.new()
	box.bg_color = fill
	box.corner_radius_top_left = 16
	box.corner_radius_top_right = 16
	box.corner_radius_bottom_left = 10
	box.corner_radius_bottom_right = 10
	box.border_width_left = 2
	box.border_width_top = 2
	box.border_width_right = 2
	box.border_width_bottom = 3
	box.border_color = rim
	box.anti_aliasing = true
	if softness:
		box.shadow_size = 12
		box.shadow_color = Color(0.0, 0.0, 0.0, 0.45)
		box.shadow_offset = Vector2(0.0, 6.0)
	return box

## Barra fina usada no chao e nas linhas da quadra.
static func line_box(color: Color) -> StyleBoxFlat:
	var box := StyleBoxFlat.new()
	box.bg_color = color
	box.anti_aliasing = true
	return box

static func additive_material() -> CanvasItemMaterial:
	var material := CanvasItemMaterial.new()
	material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
	return material
