class_name Art
extends RefCounted
## Paleta e recursos gerados em tempo de execucao. Nenhum PNG no pacote.
##
## Base vem de assets/theme.json (dark_default) para o jogo nao destoar do
## Launcher; os tons de caverna e de chama sao proprios e estao documentados.

const SURFACE := Color("1e1e1e")
const DIVIDER := Color("333333")
const TEXT_PRIMARY := Color("e5e5e5")
const TEXT_SECONDARY := Color("a0a0a0")
const ACCENT := Color("e50914")
## Luz do lampiao: ambar quente (o accent deslocado, como no Spacewar).
const LAMP := Color("ffc46b")
const GOLD := Color("e8c15a")
const ROCK := Color("2a2622")

static func radial_texture(size: int = 128) -> GradientTexture2D:
	var gradient := Gradient.new()
	gradient.set_color(0, Color(1.0, 1.0, 1.0, 1.0))
	gradient.set_color(1, Color(1.0, 1.0, 1.0, 0.0))
	gradient.add_point(0.42, Color(1.0, 1.0, 1.0, 0.32))

	var texture := GradientTexture2D.new()
	texture.gradient = gradient
	texture.fill = GradientTexture2D.FILL_RADIAL
	texture.fill_from = Vector2(0.5, 0.5)
	texture.fill_to = Vector2(1.0, 0.5)
	texture.width = size
	texture.height = size
	return texture

static func additive_material() -> CanvasItemMaterial:
	var material := CanvasItemMaterial.new()
	material.blend_mode = CanvasItemMaterial.BLEND_MODE_ADD
	return material

## Cor de um item: tesouro brilha em ouro, ferramenta em cinza claro.
static func item_color(item_id: String) -> Color:
	return GOLD if bool(Cave.ITEMS[item_id]["treasure"]) else TEXT_PRIMARY
