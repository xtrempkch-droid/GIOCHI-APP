class_name Art
extends RefCounted
## Paleta e recursos visuais gerados em tempo de execucao.
##
## Nenhum PNG entra no pacote: o brilho do star, o halo dos motores e as
## particulas usam um unico GradientTexture2D radial (128x128, ~64 KB de VRAM)
## e todo o resto e desenho vetorial -- que e, literalmente, como o original de
## 1962 desenhava (o PDP-1 tinha canhao de vetores, nao raster).
##
## As cores de base vem de assets/theme.json (tema dark_default) para o jogo
## nao destoar do Launcher; os unicos tons fora do tema sao o do star e o azul
## do P2, documentados aqui.

const BG := Color("050508")
const SURFACE := Color("0d0d14")
const DIVIDER := Color("333333")
const TEXT_PRIMARY := Color("e5e5e5")
const TEXT_SECONDARY := Color("a0a0a0")
const ACCENT := Color("e50914")
## P1 usa o accent do tema; P2 usa um azul claro (o tema nao tem ciano).
const SHIP_1 := Color("e50914")
const SHIP_2 := Color("8ad7ff")
## Star: derivado do accent, deslocado para o ambar. Nao existe no tema.
const STAR_CORE := Color("fff2d0")
const STAR_GLOW := Color("ff9a3c")
const TORPEDO := Color("ffe08a")

## Textura radial reaproveitada por brilho, halo e particulas.
static func radial_texture(size: int = 128) -> GradientTexture2D:
	var gradient := Gradient.new()
	gradient.set_color(0, Color(1.0, 1.0, 1.0, 1.0))
	gradient.set_color(1, Color(1.0, 1.0, 1.0, 0.0))
	gradient.add_point(0.40, Color(1.0, 1.0, 1.0, 0.30))

	var texture := GradientTexture2D.new()
	texture.gradient = gradient
	texture.fill = GradientTexture2D.FILL_RADIAL
	texture.fill_from = Vector2(0.5, 0.5)
	texture.fill_to = Vector2(1.0, 0.5)
	texture.width = size
	texture.height = size
	return texture

static func line_box(color: Color) -> StyleBoxFlat:
	var box := StyleBoxFlat.new()
	box.bg_color = color
	box.anti_aliasing = true
	return box

static func additive_material() -> CanvasItemMaterial:
	var material := CanvasItemMaterial.new()
	material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
	return material

## Gradiente do rastro do torpedo: transparente na cauda, quente na ponta.
static func trail_gradient() -> Gradient:
	var gradient := Gradient.new()
	gradient.set_color(0, Color(TORPEDO, 0.0))
	gradient.set_color(1, Color(TORPEDO, 0.85))
	gradient.add_point(0.65, Color(1.0, 0.55, 0.25, 0.55))
	return gradient

static func ship_color(index: int) -> Color:
	return SHIP_1 if index == 0 else SHIP_2
