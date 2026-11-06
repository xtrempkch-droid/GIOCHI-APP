shader_type canvas_item;
render_mode unshaded;

// Fundo do espaco: campo de estrelas em tres camadas de paralaxe, nebulosa
// suave, halo do star e vinheta. Tudo procedural -- nenhuma textura, um draw
// call. O original de 1962 tinha um campo de estrelas fixo de 40 pontos; aqui
// sao 3 camadas com brilho e cintilacao, e o mesmo espirito de "ceu de fundo"
// sem gastar VRAM.

uniform vec4 bg_top : source_color = vec4(0.0196, 0.0196, 0.0314, 1.0);
uniform vec4 bg_bottom : source_color = vec4(0.0392, 0.0314, 0.0510, 1.0);
uniform vec4 nebula_color : source_color = vec4(0.22, 0.10, 0.30, 1.0);
uniform vec4 star_glow : source_color = vec4(1.0, 0.60, 0.235, 1.0);

uniform vec2 star_uv = vec2(0.5, 0.5);
uniform float vignette_strength : hint_range(0.0, 1.0) = 0.65;
uniform float grain_strength : hint_range(0.0, 0.02) = 0.008;
uniform float star_pulse : hint_range(0.5, 1.5) = 1.0;
uniform float nebula_strength : hint_range(0.0, 1.0) = 0.55;

const float ASPECT = 1.7777778; // 1280 / 720

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Estrelas: grade celular. Cada celula tem no maximo uma estrela, com
// brilho proprio; a camada define o tamanho.
float star_layer(vec2 uv, float cells, float seed, float threshold) {
	vec2 grid = uv * cells;
	vec2 cell = floor(grid);
	float h = hash(cell + seed);
	if (h < threshold) {
		return 0.0;
	}
	vec2 center = fract(grid) - 0.5 - (vec2(hash(cell + 1.0), hash(cell + 2.0)) - 0.5) * 0.6;
	float d = length(center);
	float size = mix(0.006, 0.022, hash(cell + 3.0));
	return smoothstep(size, 0.0, d) * (0.35 + 0.65 * hash(cell + 4.0));
}

void fragment() {
	vec2 uv = UV;
	vec2 px = vec2(uv.x * ASPECT, uv.y);

	vec3 col = mix(bg_top.rgb, bg_bottom.rgb, smoothstep(0.0, 1.0, uv.y));

	// --- nebulosa: duas gaussianas largas ---
	float n1 = exp(-dot(px - vec2(0.30 * ASPECT, 0.30), px - vec2(0.30 * ASPECT, 0.30)) * 6.0);
	float n2 = exp(-dot(px - vec2(0.78 * ASPECT, 0.72), px - vec2(0.78 * ASPECT, 0.72)) * 9.0);
	col += nebula_color.rgb * (n1 * 0.45 + n2 * 0.30) * nebula_strength;

	// --- halo do star (fica atras do sprite do star) ---
	vec2 star_px = vec2(star_uv.x * ASPECT, star_uv.y);
	float ds = length(px - star_px);
	col += star_glow.rgb * (exp(-ds * 9.0) * 0.30 + exp(-ds * 2.4) * 0.10) * star_pulse;

	// --- tres camadas de estrelas ---
	float twinkle = 0.85 + 0.15 * sin(TIME * 1.7 + hash(floor(px * 900.0)));
	col += vec3(0.75, 0.82, 1.0) * star_layer(uv, 26.0, 7.0, 0.72) * 0.55;
	col += vec3(0.95, 0.95, 1.0) * star_layer(uv, 52.0, 31.0, 0.80) * 0.75 * twinkle;
	col += vec3(1.0, 0.92, 0.85) * star_layer(uv, 104.0, 57.0, 0.88) * 0.9;

	// --- vinheta ---
	vec2 v = (uv - vec2(0.5)) * vec2(ASPECT, 1.0);
	float vig = smoothstep(0.5, 1.3, length(v) * 1.3);
	col *= 1.0 - vig * vignette_strength;

	// --- grao fixo (disfarca o banding do painel) ---
	col += (hash(floor(uv * vec2(1280.0, 720.0))) - 0.5) * grain_strength;

	COLOR = vec4(col, 1.0);
}
