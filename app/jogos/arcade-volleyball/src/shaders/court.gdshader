shader_type canvas_item;
render_mode unshaded;

// Quadra desenhada inteiramente no fragment shader: fundo, paredes com feixes
// de luz, piso em perspectiva, halo atras da rede, luz que segue a bola,
// vinheta e grao. Custa um unico draw call e zero textura -- o que importa num
// aparelho com 128 MB de VRAM compartilhada.

uniform vec4 bg_top : source_color = vec4(0.0588, 0.0588, 0.0588, 1.0);
uniform vec4 bg_bottom : source_color = vec4(0.1059, 0.1059, 0.1059, 1.0);
uniform vec4 floor_color : source_color = vec4(0.1176, 0.1176, 0.1176, 1.0);
uniform vec4 floor_line : source_color = vec4(0.2, 0.2, 0.2, 1.0);
uniform vec4 accent : source_color = vec4(0.898, 0.0353, 0.0784, 1.0);

// 670/720: a linha do piso, a mesma de VolleyMatch.FLOOR_Y.
uniform float floor_start : hint_range(0.0, 1.0) = 0.93056;
uniform float spotlight : hint_range(0.0, 1.0) = 0.6;
uniform float ball_glow : hint_range(0.0, 1.0) = 0.35;
uniform float vignette_strength : hint_range(0.0, 1.0) = 0.55;
uniform float grain_strength : hint_range(0.0, 0.02) = 0.01;
uniform vec2 ball_uv = vec2(0.5, 0.5);

const float ASPECT = 1.7777778; // 1280 / 720

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

void fragment() {
	vec2 uv = UV;
	vec2 px = vec2(uv.x * ASPECT, uv.y);

	// Fundo: gradiente vertical + halo quente atras da rede.
	vec3 col = mix(bg_top.rgb, bg_bottom.rgb, smoothstep(0.0, 1.0, uv.y));
	float halo = exp(-dot(px - vec2(ASPECT * 0.5, 0.4), px - vec2(ASPECT * 0.5, 0.4)) * 7.0);
	col += accent.rgb * halo * 0.12 * spotlight;

	// Paredes: dois feixes de luz descendo, so acima do piso.
	float above_floor = 1.0 - smoothstep(floor_start - 0.10, floor_start, uv.y);
	float beam_l = exp(-pow((px.x - ASPECT * 0.26) * 2.6, 2.0));
	float beam_r = exp(-pow((px.x - ASPECT * 0.74) * 2.6, 2.0));
	col += (accent.rgb * 0.05 + vec3(0.04)) * (beam_l + beam_r) * above_floor * spotlight;

	// Piso: tabuas, linhas em perspectiva e brilho de verniz.
	if (uv.y > floor_start) {
		float depth = (uv.y - floor_start) / max(1.0 - floor_start, 0.001);
		vec3 floor_col = mix(floor_color.rgb * 1.45, floor_color.rgb * 0.45, depth);

		// Tabuas verticais (constantes na tela: piso visto de frente).
		float plank_id = floor(uv.x * 22.0);
		float seam = step(0.94, fract(uv.x * 22.0 + hash(vec2(plank_id, 3.0)) * 0.2));
		floor_col = mix(floor_col, floor_line.rgb, seam * 0.32);

		// Juntas horizontais em perspectiva: mais espacadas perto da camera.
		float rows = fract(pow(depth, 1.7) * 4.0);
		floor_col = mix(floor_col, floor_line.rgb, smoothstep(0.93, 1.0, rows) * 0.30);

		// Linha de base bem marcada (a borda da quadra).
		float edge = smoothstep(0.0, 0.05, depth);
		floor_col = mix(floor_line.rgb * 2.1, floor_col, edge);
		col = floor_col;
	}

	// Luz que acompanha a bola (barata: um exp).
	vec2 ball_px = vec2(ball_uv.x * ASPECT, ball_uv.y);
	float d = length(px - ball_px);
	col += accent.rgb * exp(-d * d * 55.0) * 0.16 * ball_glow;

	// Vinheta.
	vec2 v = (uv - vec2(0.5)) * vec2(ASPECT, 1.0);
	float vig = smoothstep(0.55, 1.35, length(v) * 1.35);
	col *= 1.0 - vig * vignette_strength;

	// Grao fixo: disfarca o banding do painel de 6 bits.
	float n = hash(floor(uv * vec2(1280.0, 720.0))) - 0.5;
	col += n * grain_strength;

	COLOR = vec4(col, 1.0);
}
