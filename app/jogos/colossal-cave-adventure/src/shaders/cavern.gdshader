shader_type canvas_item;
render_mode unshaded;

// Interior da caverna em 2,5D: um corredor visto de frente, com parede de
// fundo, duas paredes laterais, teto e piso. As passagens (poligonos) e os
// objetos (billboards) sao desenhados em cima pelo GDScript.
//
// Tudo procedural, com uma semente por sala: cada sala tem veios de rocha
// proprios sem carregar nenhuma imagem. Um draw call, zero textura.
//
// A leitura do espaco vem de quatro faixas de luminosidade distintas (piso
// claro, parede media, fundo escuro, teto mais escuro) -- sem elas o jogador
// ve "uma mancha escura", que foi exatamente o primeiro resultado medido
// (luminancia media 12/255 na sala do tesouro).

uniform vec4 rock_dark : source_color = vec4(0.176, 0.157, 0.141, 1.0);
uniform vec4 rock_light : source_color = vec4(0.463, 0.412, 0.353, 1.0);
uniform vec4 lamp_color : source_color = vec4(1.0, 0.78, 0.42, 1.0);

// 0 = escuridao total, 1 = sala iluminada pela luz do dia.
uniform float light : hint_range(0.0, 1.0) = 0.0;
// Semente da sala: muda veios, manchas e rachaduras.
uniform float room_seed : hint_range(0.0, 100.0) = 0.0;
// Altura do horizonte (onde a parede do fundo encontra o piso).
uniform float horizon : hint_range(0.35, 0.75) = 0.55;
uniform float vram_fog : hint_range(0.0, 1.0) = 0.35;
uniform float grain_strength : hint_range(0.0, 0.03) = 0.012;

const float ASPECT = 1.7777778; // 1280 / 720

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
		mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Rochas: ruido em oitavas + veios horizontais.
float rock(vec2 p) {
	float value = noise(p * 3.0) * 0.52 + noise(p * 7.0) * 0.30 + noise(p * 19.0) * 0.14;
	value += 0.09 * sin(p.y * 9.0 + noise(p * 2.0) * 6.0);
	return clamp(value, 0.0, 1.0);
}

void fragment() {
	vec2 uv = UV;
	vec2 px = vec2(uv.x * ASPECT, uv.y);

	// Coordenada centrada com correcao de aspecto: 0 no meio da tela.
	vec2 centered = (uv - vec2(0.5)) * vec2(ASPECT, 1.0);
	float depth = length(centered);

	// --- geometria: quatro faixas ---
	// Parede do fundo: retangulo central logo acima do horizonte.
	float back_w = 0.30;
	float back_top = horizon - 0.20;
	float in_back = step(abs(centered.x), back_w * ASPECT * 0.5)
		* step(back_top, uv.y) * step(uv.y, horizon + 0.06);

	float is_floor = step(horizon, uv.y);
	float is_ceiling = 1.0 - is_floor;

	// Perspectiva: dividir pela distancia ao horizonte faz as linhas
	// convergirem para o ponto de fuga (e o que da a sensacao de 3D).
	float dz = max(abs(uv.y - horizon), 0.02);
	vec2 persp = vec2(centered.x / dz, 1.0);

	// Textura de rocha com veios horizontais (nas laterais o veio acompanha a
	// perspectiva; no fundo e vertical, para nao desenhar um "chao" no meio).
	vec2 tex_uv = is_floor > 0.5
		? vec2(px.x * 2.2, persp.x * 1.1 + room_seed)
		: vec2(px.x * 3.4 + room_seed * 0.7, uv.y * 5.0 + room_seed);
	float tex = rock(tex_uv);

	vec3 col = mix(rock_dark.rgb, rock_light.rgb, tex);

	// --- separacao das faixas por luminosidade (a leitura do espaco) ---
	// Piso: o mais claro (recebe a luz do lampiao). Teto: o mais escuro.
	col *= mix(0.72, 1.30, is_floor);          // 0.72 no teto, 1.30 no piso
	col *= mix(0.88, 1.0, 1.0 - in_back);      // parede do fundo mais escura
	// Escurece em direcao as bordas da tela (paredes laterais ao fundo).
	col *= mix(1.0, 0.62, smoothstep(0.25, 0.95, abs(centered.x)));

	// Rachaduras finas.
	float crack = smoothstep(0.63, 0.69, noise(vec2(px.x * 24.0, uv.y * 17.0 + room_seed)));
	col *= 1.0 - crack * 0.40;

	// Linhas de perspectiva no piso (tabuas/juntas rumo ao fundo).
	if (is_floor > 0.5) {
		float plank = step(0.93, fract(centered.x / dz * 0.55));
		col *= 1.0 - plank * 0.22;
	}

	// --- iluminacao ---
	// Luz ambiente da sala + halo do lampiao no centro-baixo da tela.
	float halo = exp(-dot(centered - vec2(0.0, 0.20), centered - vec2(0.0, 0.20)) * 7.0);
	float ambient = 0.22 + light * 0.95;
	float lamp = clamp(ambient + halo * 0.85, 0.0, 2.0);

	col *= lamp;
	// O halo do lampiao puxa para ambar; a luz do dia nao.
	col = mix(col, col * lamp_color.rgb, clamp(halo * 0.55, 0.0, 0.5) * (1.0 - light * 0.55));

	// --- neblina de profundidade: o fundo perde contraste ---
	float far = 1.0 - smoothstep(0.0, 0.55, depth);
	col = mix(col, rock_dark.rgb * (0.55 + 0.55 * lamp), far * vram_fog);

	// --- vinheta + grao ---
	col *= 1.0 - smoothstep(0.62, 1.15, depth) * 0.45;
	col += (hash(floor(uv * vec2(1280.0, 720.0))) - 0.5) * grain_strength;

	COLOR = vec4(clamp(col, 0.0, 1.0), 1.0);
}
