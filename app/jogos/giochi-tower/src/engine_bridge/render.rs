//! Fachada de desenho sobre a GIOCHI-ENGINE-3D.
//!
//! Tudo o que o jogo precisa do renderer 3D passa por aqui: abrir a janela
//! OpenGL-ES, subir as malhas do protótipo, desenhar a cena de um quadro e
//! capturar BMP. O resto do jogo não conhece `RenderContext3D`.
//!
//! # Ordem de inicialização (não é negociável)
//!
//! `configure_attributes` → `Window::new_opengl` → `context::create`. Os
//! atributos `SDL_GL_*` são globais do processo e lidos na **criação da janela**;
//! inverter a ordem abre contexto 2.0 pedindo 3.0, ou falha
//! (`gl_context.rs:90-95`, `cube_demo.rs:129-133`).
//!
//! # KMSDRM
//!
//! No aparelho o driver de vídeo é `KMSDRM` e a janela tem de ser fullscreen: o
//! painel manda no tamanho, não o `WindowConfig` (ADR-035). A comparação do nome
//! do driver é feita sem caixa porque ele vem em maiúsculas (`sdl.rs:246-247`).

use std::path::Path;

use glam::Vec3 as GlamVec3;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine::platform::Platform;
use giochi_engine_3d::context::{self, GlState, GlesVersion};
use giochi_engine_3d::math3d::{Camera3D, Color3D, Quat, Transform3D, Vec3 as EngineVec3};
use giochi_engine_3d::render3d::DirectionalLight;
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::{MeshHandle, RenderContext3D, ShaderHandle, TextureHandle};

use crate::engine_bridge::modelos::{self, Catalogo};
use crate::errors::{GameError, Result};
use crate::ui::{self, layout};

/// Cor do céu: a poeira suspensa de um planeta morto ao entardecer.
///
/// Não é preto de espaço: a cena é diurna e o terreno é claro, então um céu
/// escuro faria o horizonte parecer um buraco. É também o que dá contraste para
/// a horda marrom — bug escuro sobre chão claro, não bug escuro sobre breu.
pub const CLEAR_COLOR: Color3D = Color3D::new(0.380, 0.300, 0.260, 1.0);

/// Cor do chão quando a textura de regolito **não** está disponível.
///
/// É a média da textura (`assets/texturas/terreno.bmp` → 122, 105, 88), para o
/// caminho degradado ter a mesma luminância do caminho bom. Um fallback mais
/// escuro faria "a textura faltou" parecer "a noite caiu".
pub const GROUND_COLOR: Color3D = Color3D::new(0.478, 0.412, 0.345, 1.0);

/// Cor da torre provisória (o marcador do centro do mapa).
pub const TOWER_COLOR: Color3D = Color3D::new(0.290, 0.520, 0.860, 1.0);

/// Cor do marcador do jogador.
pub const PLAYER_COLOR: Color3D = Color3D::new(0.950, 0.820, 0.300, 1.0);

/// Luz que faz a cor chegar à tela sem atenuação.
///
/// `ambient = 1.0` e cor de luz preta: no `phong.frag` o resultado é
/// `cor * 1.0 + cor * 0 * difusa + 0 * especular`, ou seja, a própria cor. O HUD
/// não é um objeto do mundo e não pode escurecer conforme a câmera orbita — com
/// a luz do sol o texto do canto mudaria de brilho junto com o chão.
///
/// É o mesmo truque que o `neon-drift` usa para o neon e o `giochi-defender` para
/// a mira, pelo mesmo motivo.
pub fn luz_do_hud() -> DirectionalLight {
    let mut luz = DirectionalLight::sun();
    luz.color = Color3D::new(0.0, 0.0, 0.0, 1.0);
    luz.ambient = 1.0;
    luz.specular = 0.0;
    luz
}

/// O sol do planeta morto: quente, com pouca especular — e **atrás da câmera**.
///
/// # A direção não é gosto: é a diferença entre uma cena e um borrão
///
/// `DirectionalLight::direction` é a direção para onde a luz **viaja**, então o
/// sol fica do lado oposto. A primeira versão apontava para `(+0.48, -0.78,
/// +0.40)`, ou seja, o sol vinha de `-X, -Z`; a câmera isométrica olha de
/// `+X, +Y, +Z`. Resultado medido no quadro: **toda face virada para o jogador
/// ficava com `lambert = 0`** e sobrava só o ambiente — pedra preta sobre chão
/// claro, e o relevo do modelo sumia.
///
/// Agora a luz viaja para `(-X, -Y, -Z)`: vem do mesmo lado da câmera, e as
/// faces visíveis recebem luz. `ambient` 0,32 mantém as faces de sombra legíveis
/// (o lado oposto de uma rocha não pode virar silhueta chapada).
pub fn luz_do_planeta() -> DirectionalLight {
    DirectionalLight::sun()
        .with_direction(EngineVec3::new(-0.52, -0.72, -0.46))
        .with_color(Color3D::new(1.0, 0.95, 0.88, 1.0))
        .with_ambient(0.32)
        .with_specular(0.06, 16.0)
}

/// Malha do chão com UV repetido pelo mundo.
///
/// `create_plane_mesh` mapeia `0..1` sobre o plano inteiro: a textura esticaria
/// 600 m e cada texel cobriria ~9 m, virando mancha. Aqui o UV é `posição / tile`,
/// então a textura repete a cada [`crate::game::cenario::TILE_DO_CHAO_M`] metros —
/// é o mesmo truque do UV de mundo do `neon-drift` (AGENTS.md §31).
///
/// Quatro vértices bastam: a interpolação de UV é perspectiva-correta, e
/// subdividir só gastaria vértice.
fn malha_do_chao(
    render: &mut RenderContext3D,
    lado: f32,
    tile: f32,
) -> std::result::Result<MeshHandle, giochi_engine_3d::Error3D> {
    use giochi_engine_3d::mesh::Vertex;
    let metade = lado * 0.5;
    let repeticoes = lado / tile.max(0.001);
    let normal = [0.0, 1.0, 0.0];
    let vertices = [
        Vertex::new([-metade, 0.0, -metade], normal, [0.0, 0.0]),
        Vertex::new([-metade, 0.0, metade], normal, [0.0, repeticoes]),
        Vertex::new([metade, 0.0, metade], normal, [repeticoes, repeticoes]),
        Vertex::new([metade, 0.0, -metade], normal, [repeticoes, 0.0]),
    ];
    // Sentido anti-horário visto de cima (+Y), como o plano da engine.
    let indices = [0, 1, 2, 0, 2, 3];
    render.create_mesh(&vertices, &indices)
}

/// Funde as peças do cenário em **uma malha por modelo**, no espaço do mundo.
///
/// # Por que fundir, em vez de instanciar
///
/// `draw_instanced` faz uma chamada por modelo **quando o driver tem
/// instanciação** (GLES 3). No R35S o contexto é GLES 2 (Mali-G31/Panfrost), e o
/// caminho de reserva desenha uma vez por instância. Medido no aparelho com as
/// 473 peças: `--sem-cenario` gastava 1,85 ms por quadro e com o cenário ia a
/// 4,62 ms — **2,77 ms** só para submeter pedra que não se move.
///
/// Fundir resolve na abertura o que a instanciação resolveria por quadro: os
/// vértices já saem girados, escalados e posicionados, e o laço faz uma chamada
/// por modelo em qualquer driver. O custo é não poder esconder uma peça isolada,
/// e cenário não precisa disso.
///
/// # O normal é girado sem escala
///
/// A matriz do cenário tem escala uniforme; transformar a normal com a parte
/// linear e renormalizar é o suficiente. Usar a matriz inteira multiplicaria a
/// normal pela escala e o modelo sairia com brilho errado — o tipo de defeito
/// que ninguém liga à matriz.
fn malhas_do_cenario(
    render: &mut RenderContext3D,
    dir: &Path,
    copias: &[crate::engine_bridge::cena::Copia],
) -> Vec<(MeshHandle, Color3D)> {
    use giochi_engine_3d::mesh::{load_obj, Vertex};

    let mut saida = Vec::new();
    for nome in modelos::CENARIO {
        let caminho = dir.join("modelos").join(format!("{nome}.obj"));
        let (vertices, indices) = match load_obj(&caminho) {
            Ok(par) => par,
            Err(erro) => {
                log::warn!("cenário '{nome}' indisponível ({}): {erro}", caminho.display());
                continue;
            }
        };

        let mut todos: Vec<Vertex> = Vec::new();
        let mut todos_indices: Vec<u32> = Vec::new();
        let mut cor = None;
        let mut usadas = 0usize;
        for copia in copias.iter().filter(|c| c.modelo == nome) {
            cor = Some(copia.cor);
            usadas += 1;
            let escala = copia.altura / crate::engine_bridge::cena::altura_do_arquivo(nome);
            let matriz = RenderBridge::matriz_de_modelo(copia.pos, copia.giro, escala).to_glam();
            let linear = glam::Mat3::from_mat4(matriz);
            let base = todos.len() as u32;
            todos.extend(vertices.iter().map(|v| {
                let posicao = matriz.transform_point3(glam::Vec3::from_array(v.position));
                let normal = (linear * glam::Vec3::from_array(v.normal)).normalize_or_zero();
                Vertex::new(posicao.to_array(), normal.to_array(), v.uv)
            }));
            todos_indices.extend(indices.iter().map(|i| i + base));
        }

        let Some(cor) = cor else { continue };
        match render.create_mesh(&todos, &todos_indices) {
            Ok(malha) => {
                log::info!(
                    "cenário: '{nome}' — {usadas} cópia(s) fundidas em 1 malha ({} vértices)",
                    todos.len()
                );
                saida.push((malha, Color3D::new(cor[0], cor[1], cor[2], cor[3])));
            }
            Err(erro) => log::warn!("cenário '{nome}': malha fundida recusada — {erro}"),
        }
    }
    saida
}

/// Janela + contexto GLES + as malhas compartilhadas do jogo.
pub struct RenderBridge {
    window: Window,
    render: RenderContext3D,
    shader: ShaderHandle,
    plane: MeshHandle,
    /// Chão maior que o campo, com UV repetido (ver [`malha_do_chao`]).
    chao: MeshHandle,
    /// Textura de regolito. `None` quando `assets/` não tem o arquivo.
    textura_chao: Option<TextureHandle>,
    /// Shader do chão texturizado (o mesmo `Textured` dos modelos).
    shader_texturado: ShaderHandle,
    /// O cenário já fundido em uma malha por modelo, no espaço do mundo.
    ///
    /// Ver [`malhas_do_cenario`]: a fusão é o que faz a paisagem custar 5
    /// chamadas de desenho **em qualquer driver**, inclusive no GLES 2 do RK3326,
    /// onde `draw_instanced` cai no caminho de uma chamada por instância.
    cenario: Vec<(MeshHandle, Color3D)>,
    cube: MeshHandle,
    /// Quadrilátero unitário do HUD: a geometria não muda por quadro, só as
    /// matrizes de instância.
    hud_quad: MeshHandle,
    hud_shader: ShaderHandle,
    /// Modelos e texturas do jogo (personagens, bosses, torre).
    modelos: Option<Catalogo>,
    /// Shader instanciado, para a horda sair em uma chamada por tipo.
    instanced_shader: ShaderHandle,
    /// Textura 1×1 branca: o que um modelo sem textura recebe para o shader
    /// `textured` multiplicar por branco em vez de amostrar lixo.
    textura_reserva: Option<giochi_engine_3d::TextureHandle>,
    /// Lotes do último quadro, para o diagnóstico contar chamadas.
    hud_lotes: Vec<ui::Lote>,
    /// Chamadas que o **driver** emitiu no último quadro. Diferente de
    /// `hud_lotes.len()` quando não há instancing: aí `draw_instanced` cai no
    /// caminho comum e emite uma por instância. Sem este número, "9 chamadas"
    /// seria o que eu pedi, não o que saiu.
    hud_chamadas_gl: u32,
    driver: &'static str,
    requested_gles: GlesVersion,
    frames: u64,
}

impl RenderBridge {
    /// Abre a janela e sobe o contexto 3D.
    ///
    /// `title` vai como argumento de `Window::new_opengl`: `WindowConfig` não tem
    /// campo de título nesta versão da engine.
    pub fn new(platform: &Platform, width: u32, height: u32, title: &str) -> Result<Self> {
        let requested_gles = GlesVersion::from_env();
        context::configure_attributes(platform, requested_gles)?;

        let mut config = WindowConfig::new(width.max(1), height.max(1));
        let driver = platform.video_driver_name();
        if driver.eq_ignore_ascii_case("kmsdrm") {
            // Sem compositor no aparelho: a janela É o painel.
            config = config.with_fullscreen();
        }

        let window = Window::new_opengl(platform, config, title)?;
        let mut render = RenderContext3D::new(context::create(&window)?);

        render.set_clear_color(CLEAR_COLOR);
        render.set_light(luz_do_planeta());

        // Blending ligado para o **quadro inteiro**, e não só para o HUD.
        //
        // A engine aplica o `GlState` em `begin_frame` e não oferece troca no
        // meio do quadro (`set_state` só grava; ver `renderer.rs`). O mundo é
        // opaco, então o blend não muda um pixel dele; o HUD usa alfa no véu da
        // loja, na trilha das barras e nos painéis. Ligar só para o HUD exigiria
        // um segundo `begin_frame`, que limparia o quadro.
        render.set_state(GlState {
            blend: true,
            ..GlState::default()
        });

        let shader = render.create_builtin_shader(BuiltinShader::Phong)?;
        let plane = render.create_plane_mesh(crate::game::camera::MapBounds::new().size_m(), 1)?;
        let shader_texturado = render.create_builtin_shader(BuiltinShader::Textured)?;
        let cube = render.create_cube_mesh(1.0)?;

        // O chão do planeta: plano de 600 m com UV de mundo. Se a malha falhar
        // (driver sem VAO, handle esgotado), cai no plano antigo de 320 m — o
        // jogo continua desenhando, só sem horizonte.
        let assets = modelos::achar_diretorio();
        let chao = malha_do_chao(
            &mut render,
            crate::game::cenario::LADO_DO_CHAO_M,
            crate::game::cenario::TILE_DO_CHAO_M,
        )
        .unwrap_or_else(|erro| {
            log::warn!("chão com UV de mundo indisponível ({erro}); usando o plano do campo");
            plane
        });
        let textura_chao = assets
            .as_ref()
            .map(|dir| dir.join("texturas").join("terreno.bmp"))
            .and_then(|caminho| match render.create_texture(&caminho) {
                Ok(textura) => Some(textura),
                Err(erro) => {
                    log::warn!(
                        "textura do chão indisponível ({}): chão fica de cor plana — {erro}",
                        caminho.display()
                    );
                    None
                }
            });
        log::info!(
            "chão: {:.0}x{:.0} m, textura a cada {:.0} m ({})",
            crate::game::cenario::LADO_DO_CHAO_M,
            crate::game::cenario::LADO_DO_CHAO_M,
            crate::game::cenario::TILE_DO_CHAO_M,
            if textura_chao.is_some() { "regolito" } else { "cor plana" }
        );
        let (hud_vertices, hud_indices) = ui::quad_da_hud();
        let hud_quad = render.create_mesh(&hud_vertices, &hud_indices)?;
        let hud_shader = render.create_builtin_shader(BuiltinShader::Instanced)?;
        let instanced_shader = render.create_builtin_shader(BuiltinShader::Instanced)?;

        // Textura 1×1 branca. Não é decoração: `draw_mesh_textured` amostra a
        // unidade com `u_tint`, e um modelo sem textura precisa de algo ligado —
        // sem textura o shader amostra o que estiver na unidade 0, que num driver
        // pode ser lixo do quadro anterior.
        let textura_reserva = render.create_texture_from_rgba(1, 1, &[255, 255, 255, 255]).ok();

        // Os modelos são opcionais: um pacote sem `assets/` continua jogável com
        // os cubos, e o log diz por quê. Ficar sem desenhar nada seria pior.
        let modelos = match (assets.clone(), textura_reserva) {
            (Some(dir), Some(reserva)) => {
                match Catalogo::carregar(&mut render, &dir, cube, reserva) {
                    Ok(catalogo) => Some(catalogo),
                    Err(erro) => {
                        log::warn!("catálogo de modelos indisponível: {erro}");
                        None
                    }
                }
            }
            _ => {
                log::warn!(
                    "assets/ não encontrado (procurei {:?}); personagens ficam como cubos",
                    modelos::diretorios_candidatos()
                );
                None
            }
        };

        // O cenário é fundido **antes** de a janela entrar no laço: uma malha por
        // modelo, com todas as cópias já transformadas para o espaço do mundo.
        let cenario = match assets.as_ref() {
            Some(dir) => malhas_do_cenario(&mut render, dir, &crate::game::cenario::montar()),
            None => Vec::new(),
        };

        Ok(Self {
            window,
            render,
            shader,
            plane,
            chao,
            textura_chao,
            shader_texturado,
            cenario,
            cube,
            hud_quad,
            hud_shader,
            modelos,
            instanced_shader,
            textura_reserva,
            hud_lotes: Vec::new(),
            hud_chamadas_gl: 0,
            driver,
            requested_gles,
            frames: 0,
        })
    }

    /// Nome do driver de vídeo em uso, como a plataforma reportou na abertura.
    pub fn driver(&self) -> &'static str {
        self.driver
    }

    /// Versão de GLES pedida na inicialização.
    pub fn requested_gles(&self) -> GlesVersion {
        self.requested_gles
    }

    /// Versão de GLES realmente obtida.
    pub fn gles_major(&self) -> u8 {
        self.render.gles_version().major()
    }

    /// Nome do adaptador gráfico.
    pub fn adapter(&self) -> &str {
        self.render.adapter()
    }

    /// `true` quando o driver entregou 3.0 (não pedido, apenas observado).
    pub fn supports_gles3(&self) -> bool {
        self.render.supports_gles3()
    }

    /// Tamanho real do drawable. Sempre este, nunca o pedido: no KMSDRM o painel
    /// sobrescreve o `WindowConfig`.
    pub fn drawable_size(&self) -> (u32, u32) {
        self.window.drawable_size()
    }

    /// Quantos quadros já foram apresentados.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Malha do plano do chão (1×1 m escalado no desenho, ver [`Self::draw_ground`]).
    pub fn plane(&self) -> MeshHandle {
        self.plane
    }

    /// Inicia o quadro (limpa cor e profundidade, aplica viewport).
    pub fn begin(&mut self) {
        self.render.begin_frame(&self.window);
    }

    /// Apresenta o quadro e drena a fila de erros do GL.
    ///
    /// Um erro de GL aqui vira `GameError`: é o que transforma "tela preta" em
    /// falha nomeada, como em `cube_demo.rs:227-229`.
    pub fn end(&mut self) -> Result<()> {
        self.render.end_frame(&self.window);
        self.frames += 1;
        let error = self.render.take_gl_error();
        if error != 0 {
            return Err(GameError::message(format!("GL 0x{error:04x} no quadro {}", self.frames)));
        }
        Ok(())
    }

    /// Desenha o chão do mapa inteiro.
    ///
    /// O caminho bom é a malha de 600 m com a textura de regolito repetida pelo
    /// mundo. Sem a textura (pacote sem `assets/`), cai no plano antigo com cor
    /// plana — mesmo tamanho do campo, mesma luminância. É degradação, não erro:
    /// um asset ausente num pacote mal montado não pode impedir de jogar.
    pub fn draw_ground(&mut self, camera: &Camera3D) -> Result<()> {
        let model = Transform3D::IDENTITY;
        if let Some(textura) = self.textura_chao {
            let desenhou = self.render.draw_mesh_textured(
                self.chao,
                self.shader_texturado,
                textura,
                &model,
                camera,
                // Branco: a cor vem do BMP. Tingir aqui escureceria duas vezes.
                Color3D::WHITE,
            )?;
            if desenhou {
                return Ok(());
            }
        }
        self.draw(self.plane, &model, camera, GROUND_COLOR)
    }

    /// Desenha o cenário do planeta (pedras, crateras, mesas e destroços).
    ///
    /// Uma chamada por modelo — cinco no total. O cenário é estático, então as
    /// matrizes de instância foram aplicadas aos vértices na abertura: nada é
    /// reenviado por quadro e não há dependência de `GL_ARB_instanced_arrays`.
    pub fn draw_cenario(&mut self, camera: &Camera3D) -> Result<()> {
        for i in 0..self.cenario.len() {
            let (malha, cor) = self.cenario[i];
            self.draw(malha, &Transform3D::IDENTITY, camera, cor)?;
        }
        Ok(())
    }

    /// Quantas malhas o cenário tem (uma por modelo com peças).
    pub fn cenario_malhas(&self) -> usize {
        self.cenario.len()
    }

    /// Desenha um cubo de aresta `size` centrado em `position`.
    ///
    /// A malha é um cubo unitário centrado na origem; o tamanho vem da escala do
    /// `Transform3D`, que a engine compõe como T·R·S (`transform.rs:72-76`).
    pub fn draw_cube(
        &mut self,
        camera: &Camera3D,
        position: GlamVec3,
        size: f32,
        color: Color3D,
    ) -> Result<()> {
        let edge = size.max(0.01);
        let model = Transform3D::new(
            EngineVec3::from_glam(position),
            Quat::IDENTITY,
            EngineVec3::new(edge, edge, edge),
        );
        self.draw(self.cube, &model, camera, color)
    }

    /// Desenha um cubo com rotação e escala arbitrárias (para os testes de cena).
    pub fn draw_cube_transformed(
        &mut self,
        camera: &Camera3D,
        model: &Transform3D,
        color: Color3D,
    ) -> Result<()> {
        self.draw(self.cube, model, camera, color)
    }

    fn draw(
        &mut self,
        mesh: MeshHandle,
        model: &Transform3D,
        camera: &Camera3D,
        color: Color3D,
    ) -> Result<()> {
        // `Ok(false)` significa handle obsoleto: a engine não trata isso como
        // erro, e um jogo que ignora o booleano desenha nada e acha que desenhou.
        let drew = self
            .render
            .draw_mesh_colored(mesh, self.shader, model, camera, color)?;
        if !drew {
            log::warn!("draw ignorado: malha {mesh} ou shader não existe mais");
        }
        Ok(())
    }

    /// `true` quando há modelos de verdade (e não o cubo de reserva).
    pub fn tem_modelos(&self) -> bool {
        self.modelos.as_ref().is_some_and(|c| c.do_disco() > 0)
    }

    /// Quantos modelos vieram do disco.
    pub fn modelos_carregados(&self) -> usize {
        self.modelos.as_ref().map(Catalogo::do_disco).unwrap_or(0)
    }

    /// Desenha um personagem, boss ou a torre pelo nome do modelo.
    ///
    /// `altura_no_mundo` é a altura desejada em metros — a escala é derivada da
    /// altura medida no arquivo, então mexer no OBJ não muda o tamanho que a
    /// jogabilidade usa. Sem catálogo, cai no cubo do tamanho equivalente para o
    /// quadro continuar legível.
    pub fn draw_modelo(
        &mut self,
        camera: &Camera3D,
        nome: &str,
        pos: GlamVec3,
        altura_no_mundo: f32,
        giro: f32,
        cor: Color3D,
    ) -> Result<()> {
        let Some(catalogo) = self.modelos.as_ref() else {
            return self.draw_cube(camera, GlamVec3::new(pos.x, altura_no_mundo * 0.5, pos.z), altura_no_mundo * 0.62, cor);
        };
        // `pos` é a **base**: o modelo é medido a partir do pé, então o cubo de
        // reserva sobe meia altura. Passar `pos.y` direto afundaria o personagem.
        let pedido = modelos::Pedido::novo(nome, EngineVec3::from_glam(pos), altura_no_mundo)
            .com_giro(giro)
            .com_tint(cor);
        match catalogo.desenhar(&mut self.render, camera, pedido) {
            Ok(()) => Ok(()),
            Err(erro) => {
                log::warn!("modelo '{nome}': {erro}");
                Ok(())
            }
        }
    }

    /// Desenha `matrizes` cópias de um modelo numa chamada (a horda).
    pub fn draw_modelo_lote(
        &mut self,
        camera: &Camera3D,
        nome: &str,
        matrizes: &[giochi_engine_3d::math3d::Mat4],
        cor: Color3D,
    ) -> Result<u32> {
        let Some(catalogo) = self.modelos.as_ref() else {
            return Ok(0);
        };
        catalogo
            .desenhar_lote(&mut self.render, camera, nome, matrizes, cor, self.instanced_shader)
            .map_err(|erro| GameError::message(format!("lote de '{nome}': {erro}")))
    }

    /// Matriz de instância para um modelo: posição, giro em Y e escala uniforme.
    ///
    /// Pública porque quem monta a lista da horda é `main.rs`, que conhece a
    /// altura de cada tipo — e a matriz é a única coisa que os dois lados precisam
    /// combinar.
    pub fn matriz_de_modelo(pos: GlamVec3, giro: f32, escala: f32) -> giochi_engine_3d::math3d::Mat4 {
        Transform3D::new(
            EngineVec3::from_glam(pos),
            Quat::from_axis_angle(EngineVec3::UP, giro),
            EngineVec3::new(escala, escala, escala),
        )
        .matrix()
    }

    /// Desenha o HUD sobre a cena 3D.
    ///
    /// Tem de ser chamada **depois** do mundo e **antes** de [`Self::end`]: o HUD
    /// divide o mesmo depth buffer, e o plano próximo em 9,0 da câmera de 90° é o
    /// que garante que ele fica na frente.
    ///
    /// A luz é trocada para a emissiva aqui e restaurada para o sol no fim. Sem a
    /// restauração, o quadro seguinte desenharia o mundo com `ambient = 1.0` e
    /// cor de luz preta — tudo chapado, e o defeito apareceria como "a iluminação
    /// quebrou", longe da causa.
    pub fn draw_ui(&mut self, plano: &layout::Plano) -> Result<()> {
        let (largura, altura) = self.window.drawable_size();
        if largura == 0 || altura == 0 {
            return Ok(());
        }
        let tela = layout::Tela::new(largura, altura);
        let lotes = ui::lotes(plano, tela);

        let camera = ui::camera((largura, altura));
        self.render.set_light(luz_do_hud());
        let mut chamadas = 0;
        for lote in &lotes {
            let cor = Color3D::new(lote.cor[0], lote.cor[1], lote.cor[2], lote.cor[3]);
            chamadas += self.render.draw_instanced(
                self.hud_quad,
                self.hud_shader,
                &lote.matrizes,
                &camera,
                cor,
            )?;
        }
        self.hud_chamadas_gl = chamadas;
        self.render.set_light(luz_do_planeta());
        self.hud_lotes = lotes;
        Ok(())
    }

    /// Quantas chamadas de desenho o HUD custou no último quadro.
    ///
    /// Uma por cor. É o número que responde "eu agrupei?" — e que um
    /// `draw_rect` por letra levaria a centenas.
    pub fn hud_draw_calls(&self) -> usize {
        self.hud_lotes.len()
    }

    /// Quantas instâncias (retângulos) o HUD desenhou no último quadro.
    pub fn hud_instances(&self) -> usize {
        self.hud_lotes.iter().map(|l| l.matrizes.len()).sum()
    }

    /// Quantas chamadas o driver emitiu para o HUD no último quadro.
    ///
    /// Igual a [`Self::hud_draw_calls`] quando há instancing; igual ao número de
    /// instâncias quando não há. É o que separa "eu agrupei" de "o driver
    /// agrupou".
    pub fn hud_gl_draws(&self) -> u32 {
        self.hud_chamadas_gl
    }

    /// Captura o quadro atual em BMP de 24 bits.
    ///
    /// Tem de ser chamada **antes** de [`Self::end`]: depois do swap, o back buffer
    /// já pertence ao quadro seguinte (`cube_demo.rs:211-212`).
    pub fn capture_bmp(&self, path: &Path) -> Result<()> {
        let (width, height) = self.render.drawable_size();
        let pixels = self.render.read_pixels(width, height)?;
        write_bmp(path, width, height, &pixels)?;
        log::info!("quadro: {} ({width}x{height})", path.display());
        Ok(())
    }

    /// Libera malhas, shader e o contexto.
    pub fn destroy_all(&mut self) {
        // Os retornos são `bool` (handle existia?). Aqui não há decisão a tomar com
        // eles, e ignorá-los explicitamente evita o aviso de `must_use`.
        let reserva = self.textura_reserva.unwrap_or(0);
        if let Some(mut catalogo) = self.modelos.take() {
            catalogo.destruir(&mut self.render, self.cube, reserva);
        }
        if reserva != 0 {
            let _ = self.render.destroy_texture(reserva);
            self.textura_reserva = None;
        }
        let _ = self.render.destroy_mesh(self.plane);
        let _ = self.render.destroy_mesh(self.cube);
        let _ = self.render.destroy_mesh(self.hud_quad);
        let _ = self.render.destroy_shader(self.shader);
        let _ = self.render.destroy_shader(self.hud_shader);
        let _ = self.render.destroy_shader(self.instanced_shader);
        let _ = self.render.release_all();
    }
}

/// Escreve um BMP 24 bits a partir de pixels RGBA de `glReadPixels`.
///
/// O GL devolve as linhas de baixo para cima e o BMP com altura positiva guarda
/// exatamente nessa ordem, então as linhas são copiadas em sequência — trocá-las
/// produziria um quadro de cabeça para baixo que "parece" um bug de câmera.
fn write_bmp(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err(GameError::message(format!(
            "captura curta: {} bytes para {width}x{height} (esperava {expected})",
            rgba.len()
        )));
    }

    let row_bytes = width as usize * 3;
    let padding = (4 - (row_bytes % 4)) % 4;
    let row_size = row_bytes + padding;
    let pixel_bytes = row_size * height as usize;
    let file_size = 54 + pixel_bytes;

    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in 0..height as usize {
        let row = &rgba[y * width as usize * 4..(y + 1) * width as usize * 4];
        for x in 0..width as usize {
            let p = &row[x * 4..x * 4 + 4];
            out.push(p[2]); // B
            out.push(p[1]); // G
            out.push(p[0]); // R
        }
        out.extend(std::iter::repeat_n(0u8, padding));
    }

    std::fs::write(path, &out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bmp_header_is_a_24_bit_windows_bitmap() {
        let dir = std::env::temp_dir().join("giochi-tower-bmp");
        std::fs::create_dir_all(&dir).expect("tmp");
        let path = dir.join("header.bmp");
        let pixels = vec![255u8; 4 * 2 * 2];
        write_bmp(&path, 2, 2, &pixels).expect("escrever bmp");

        let bytes = std::fs::read(&path).expect("ler bmp");
        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize, bytes.len());
        assert_eq!(u16::from_le_bytes(bytes[28..30].try_into().unwrap()), 24);
        // 2 px de largura = 6 bytes por linha, mais 2 de padding.
        let row_size = 8;
        assert_eq!(bytes.len(), 54 + row_size * 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_short_capture_is_refused_instead_of_panicking() {
        let err = write_bmp(Path::new("nao-usado.bmp"), 4, 4, &[0u8; 8]);
        assert!(err.is_err());
    }
}
