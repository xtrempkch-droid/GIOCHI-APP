//! GIOCHI DEFENDER — host do jogo.
//!
//! ## Por que não existe um `Engine` aqui
//!
//! `giochi_engine::Engine` é a fachada **2D**: ela cria um `RenderContext` sobre
//! `sdl2::render`. O ADR-037 da engine diz que um jogo escolhe um renderer e não
//! troca em tempo de execução — e este jogo é 3D (GLES sobre EGL). As duas coisas
//! não compartilham janela.
//!
//! O que se reusa é o resto, que é a maior parte: `Platform` (SDL2, KMSDRM),
//! `Window` (com a flag OpenGL), `Input`, `AudioSystem`, `GameLoop` e `FrameClock`
//! — todos da engine 2D, que os cede sem exigir o renderer dela.
//!
//! O laço abaixo é o mesmo desenho do `Engine::run`: passo fixo de 1/60 s,
//! acumulador com teto, `render` uma vez por quadro de tela.

use std::process::ExitCode;
use std::time::Instant;

use anyhow::{bail, Result};
use giochi_engine::audio::AudioSystem;
use giochi_engine::core::game_loop::GameLoop;
use giochi_engine::core::time::FrameClock;
use giochi_engine::input::gamepad::{
    AXIS_RIGHT_X, AXIS_RIGHT_Y, BUTTON_RIGHT_SHOULDER, BUTTON_SOUTH,
};
use giochi_engine::input::keyboard::{
    KEY_A, KEY_D, KEY_DOWN, KEY_ESCAPE, KEY_LEFT, KEY_RIGHT, KEY_S, KEY_UP, KEY_W,
};
use giochi_engine::input::mouse::MOUSE_LEFT;
use giochi_engine::input::Input;
use giochi_engine::platform::window::{Window, WindowConfig};
use giochi_engine::platform::{quit_signal, Platform, QuitSignal};
use giochi_engine_3d::context::{self, GlState, GlesVersion};
use giochi_engine_3d::math3d::{Camera3D, Color3D, Transform3D, Vec3};
use giochi_engine_3d::render3d::{MeshHandle, ShaderHandle};
use giochi_engine_3d::shader::BuiltinShader;
use giochi_engine_3d::RenderContext3D;

use giochi_defender::bench::Benchmark;
use giochi_defender::diag::{self, Diag};
use giochi_defender::gameplay::arena::{self, Arena};
use giochi_defender::gameplay::camera::OrbitalCamera;
use giochi_defender::gameplay::partida::{Inimigos, Partida};
use giochi_defender::gameplay::weapon::{Weapon, PONTOS_POR_ABATE};
use giochi_defender::state::{GameState, WAVES_TOTAIS};
use giochi_defender::ui::hud::{self, Hud};
use giochi_defender::ui::reticle::{self, Mira};
use giochi_defender::utils::math::{graus_para_radianos, radianos_para_graus};

/// Tamanho pedido da janela. No aparelho o painel manda (ADR-035).
const LARGURA: u32 = 1280;
/// Altura pedida da janela.
const ALTURA: u32 = 720;
/// Título da janela.
const TITULO: &str = "GIOCHI DEFENDER";

/// Giro do teclado, em radianos por segundo.
const TAXA_TECLADO: f32 = 1.8;
/// Giro do analógico direito na deflexão máxima, em radianos por segundo.
const TAXA_ANALOGICO: f32 = 2.6;
/// Zona morta do analógico: abaixo disto o valor é ruído do potenciômetro.
const ZONA_MORTA: f32 = 0.25;
/// Orçamento de um quadro a 60 Hz, em milissegundos.
const QUADRO_MS: u32 = 1000 / 60;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let diag = match Diag::parse(std::env::args().skip(1)) {
        Ok(diag) => diag,
        Err(erro) => {
            eprintln!("erro: {erro:#}");
            return ExitCode::FAILURE;
        }
    };

    if diag.ajuda {
        println!("{}", diag::AJUDA);
        return ExitCode::SUCCESS;
    }

    match executar(diag) {
        Ok(()) => ExitCode::SUCCESS,
        Err(erro) => {
            log::error!("{erro:#}");
            ExitCode::FAILURE
        }
    }
}

/// Abre dispositivo, janela e contexto, e roda até o ESC ou até o limite de
/// quadros.
fn executar(diag: Diag) -> Result<()> {
    let plataforma = Platform::new()?;
    log::info!(
        "GIOCHI DEFENDER boot — engine {} · vídeo '{}'",
        giochi_engine_3d::VERSION,
        plataforma.video_driver_name()
    );

    let requisitada = GlesVersion::from_env();
    // Os atributos `SDL_GL_*` são globais no SDL2 e lidos na criação da JANELA:
    // configurá-los depois de abrir a janela deixa uma janela sem a flag OpenGL.
    context::configure_attributes(&plataforma, requisitada)?;

    let janela = Window::new_opengl(
        &plataforma,
        configuracao_janela(&plataforma, diag.tela),
        TITULO,
    )?;
    let mut render = RenderContext3D::new(context::create(&janela)?);
    render.set_clear_color(arena::COR_ESPACO);

    // O drawable só é conhecido depois do primeiro quadro: no KMSDRM ele é o
    // painel, não o tamanho pedido (ADR-035). Aqui o que se sabe é o que foi
    // pedido.
    log::info!(
        "GLES {} em '{}' · janela pedida {}x{}",
        render.gles_version().major(),
        render.adapter(),
        janela.size().0,
        janela.size().1
    );
    if requisitada == GlesVersion::Gles3 && !render.supports_gles3() {
        log::warn!("GLES 3.0 foi pedido e o driver entregou 2.0; seguindo em 2.0");
    }

    let mut entrada = Input::new(&plataforma);
    // R6: áudio quebrado é silêncio com aviso, nunca `panic`.
    let audio = iniciar_audio(&plataforma);

    if diag.mouse_relativo {
        prender_cursor(&plataforma);
    }

    // Blend ligado no quadro inteiro: a mira usa alfa para o antialiasing e para
    // a cor de repouso. Todo o resto da cena desenha com alfa 1,0, então ligar o
    // blend não muda um pixel do mundo — e `set_state` só é aplicado dentro de
    // `begin_frame`, por isso ele vem antes do laço e não dentro dele.
    render.set_state(GlState {
        blend: true,
        ..GlState::default()
    });

    let arena = Arena::new(&mut render)?;
    let inimigos = Inimigos::new(&mut render)?;
    // A mira nasce com o tamanho **pedido**, e o primeiro quadro a reconstrói com
    // o drawable real. Não é desperdício: no console o painel tem 640×480 e o
    // pedido é 1280×720, então é exatamente esta diferença que a reconstrução
    // existe para tratar — construí-la já com o tamanho certo deixaria o caminho
    // de redimensionamento sem nunca rodar em produção.
    let (largura_inicial, altura_inicial) = diag.tela.unwrap_or((LARGURA, ALTURA));
    let mut mira = MiraGl::new(&mut render, (largura_inicial, altura_inicial))?;
    let mut hud_gl = HudGl::new(&mut render)?;
    let mut camera = OrbitalCamera::new();
    aplicar_preset(&mut camera, &diag);
    let mut estado = GameState::new();
    let mut partida = Partida::new(diag.semente);
    let mut hud_estado = Hud::new();

    // `--alvo N`: cubos parados no eixo da câmera. É a bancada de tiro — sem
    // ela, provar que o tiro mata dependeria de o sorteio cair na linha de mira.
    if let Some(quantos) = diag.alvo {
        let (largura, altura) = janela.size();
        let camera3d = camera.camera(largura, altura);
        let frente = direcao_da_camera(&camera3d);
        let mut plantados = 0;
        let distancia_camera = (camera3d.position.x * camera3d.position.x
            + camera3d.position.y * camera3d.position.y
            + camera3d.position.z * camera3d.position.z)
            .sqrt();
        for (indice, distancia) in diag::distancias_de_alvo(distancia_camera, quantos)
            .into_iter()
            .enumerate()
        {
            let posicao = Vec3::new(
                camera3d.position.x + frente.x * distancia,
                camera3d.position.y + frente.y * distancia,
                camera3d.position.z + frente.z * distancia,
            );
            if partida
                .plantar(posicao, indice as f32 / quantos.max(1) as f32)
                .is_some()
            {
                plantados += 1;
            }
        }
        log::info!("alvos plantados no eixo da câmera: {plantados} de {quantos}");
    }

    // `--inimigos N`: mede "15 cubos" sem esperar 30 s de spawn a 2 s. Vem antes
    // do `--simular` para que a simulação, se pedida, mova esses mesmos cubos.
    if let Some(quantos) = diag.inimigos {
        let criados = partida.povoar(quantos);
        log::info!(
            "povoado: {criados} de {quantos} cubo(s) pedido(s), capacidade {}",
            partida.capacidade()
        );
        if criados < quantos as usize {
            log::warn!("o pool encheu: pedido de {quantos} acima da capacidade");
        }
    }

    // `--simular`: adianta a partida antes do primeiro quadro. Sem isto uma
    // captura headless só mostraria a arena vazia, e a simulação de inimigos
    // ficaria sem nenhuma prova que não dependesse de alguém olhar a tela.
    if let Some(segundos) = diag.simular.filter(|valor| *valor > 0.0) {
        // O clarão de dano é acendido pela **mesma** regra do laço (mudança em
        // `derrotas`), e não por um caminho só do diagnóstico: sem isso a captura
        // depois de `--simular` mostraria uma HUD que nunca levou dano.
        let derrotas_antes = partida.derrotas();
        partida.atualizar(segundos, &mut estado);
        if partida.derrotas() > derrotas_antes {
            hud_estado.levou_dano();
        }
        log::info!(
            "simulados {segundos:.1}s: {} inimigo(s) no ar, {} vida(s), {} derrota(s)",
            partida.inimigos(),
            estado.lives,
            partida.derrotas()
        );
    }

    log::info!(
        "arena: asteroide raio {:.2} (externo {:.2}), {} estrelas, {} waves",
        arena.raio(),
        arena.raio_externo(),
        arena.estrelas(),
        WAVES_TOTAIS
    );
    log::info!(
        "câmera: yaw {:.1}°, pitch {:.1}°, distância {:.1}",
        radianos_para_graus(camera.yaw),
        radianos_para_graus(camera.pitch_seguro()),
        camera.distancia_segura()
    );
    log::info!(
        "partida nova: {} vidas, wave 1 de {}",
        estado.lives,
        WAVES_TOTAIS
    );

    let resultado = laco(
        &plataforma,
        &janela,
        &mut render,
        &mut entrada,
        &arena,
        &inimigos,
        &mut mira,
        &mut hud_gl,
        &mut camera,
        &mut partida,
        &mut estado,
        &mut hud_estado,
        &diag,
    );

    hud_gl.destruir(&mut render);
    mira.destruir(&mut render);
    inimigos.destruir(&mut render);
    arena.destruir(&mut render);
    log::info!(
        "encerrando: {} malhas e {} shaders ainda na engine (devem ser 0)",
        render.mesh_count(),
        render.shader_count()
    );

    // O áudio é solto depois do laço para o mixer não segurar o dispositivo
    // enquanto o processo termina.
    drop(audio);

    resultado
}

/// Janela em modo janela no PC e em *fullscreen* no console.
///
/// O KMSDRM não tem gerenciador de janelas: pedir uma janela lá é pedir para
/// desenhar num canto da tela. Detectar pelo driver evita um `--fullscreen` que
/// alguém esquece de passar no aparelho.
fn configuracao_janela(plataforma: &Platform, tela: Option<(u32, u32)>) -> WindowConfig {
    let (largura, altura) = tela.unwrap_or((LARGURA, ALTURA));
    let config = WindowConfig::new(largura.max(1), altura.max(1));
    if plataforma
        .video_driver_name()
        .eq_ignore_ascii_case("kmsdrm")
    {
        config.with_fullscreen()
    } else {
        config
    }
}

/// Cria o sistema de áudio, caindo para o modo silencioso quando não há device.
fn iniciar_audio(plataforma: &Platform) -> AudioSystem {
    match AudioSystem::new(plataforma) {
        Ok(audio) => audio,
        Err(erro) => {
            log::warn!("áudio indisponível ({erro}); seguindo em silêncio");
            AudioSystem::silent()
        }
    }
}

/// Prende o cursor no centro da janela, para a câmera relativa.
fn prender_cursor(plataforma: &Platform) {
    match plataforma.sdl_mouse() {
        Some(mouse) => mouse.set_relative_mouse_mode(true),
        None => log::warn!("sem subsistema de mouse; a câmera fica no teclado/analógico"),
    }
}

/// Aplica `--yaw`, `--pitch` e `--distancia` sobre a câmera inicial.
fn aplicar_preset(camera: &mut OrbitalCamera, diag: &Diag) {
    if let Some(graus) = diag.yaw_graus {
        camera.yaw = graus_para_radianos(graus);
    }
    if let Some(graus) = diag.pitch_graus {
        camera.pitch = graus_para_radianos(graus);
    }
    if let Some(distancia) = diag.distancia {
        camera.distance = distancia;
    }
}

/// Gira a câmera com mouse, teclado e analógico direito.
///
/// Os três somam em vez de competir: no aparelho não há mouse, no PC não há
/// controle, e um único caminho de câmera evita que o teste no PC meça outro
/// comportamento.
fn atualizar_camera(camera: &mut OrbitalCamera, entrada: &Input, dt: f32) {
    let delta = entrada.mouse().delta();
    if delta.x != 0.0 || delta.y != 0.0 {
        camera.orbitar(delta.x, delta.y);
    }

    let roda = entrada.mouse().wheel_delta();
    if roda != 0.0 {
        camera.zoom(roda);
    }

    let mut yaw = 0.0;
    let mut pitch = 0.0;
    if entrada.key_down(KEY_LEFT) || entrada.key_down(KEY_A) {
        yaw -= 1.0;
    }
    if entrada.key_down(KEY_RIGHT) || entrada.key_down(KEY_D) {
        yaw += 1.0;
    }
    if entrada.key_down(KEY_UP) || entrada.key_down(KEY_W) {
        pitch += 1.0;
    }
    if entrada.key_down(KEY_DOWN) || entrada.key_down(KEY_S) {
        pitch -= 1.0;
    }

    let (analogico_x, analogico_y) = com_zona_morta(
        entrada.gamepad().axis(AXIS_RIGHT_X),
        entrada.gamepad().axis(AXIS_RIGHT_Y),
    );
    yaw += analogico_x * (TAXA_ANALOGICO / TAXA_TECLADO);
    pitch += analogico_y * (TAXA_ANALOGICO / TAXA_TECLADO);

    if yaw != 0.0 || pitch != 0.0 {
        camera.girar(yaw * TAXA_TECLADO * dt, pitch * TAXA_TECLADO * dt);
    }
}

/// Aplica a zona morta e reescala o resto para `0..=1`.
///
/// Sem a reescala, o analógico "começa" a 0.25 e o movimento tem um degrau; com
/// ela, sair da zona morta dá velocidade zero e a rampa é contínua.
fn com_zona_morta(x: f32, y: f32) -> (f32, f32) {
    let aplicar = |valor: f32| {
        let modulo = valor.abs();
        if modulo <= ZONA_MORTA {
            0.0
        } else {
            valor.signum() * ((modulo - ZONA_MORTA) / (1.0 - ZONA_MORTA)).min(1.0)
        }
    };
    (aplicar(x), aplicar(y))
}

/// A mira na GPU: uma malha e um shader, um desenho por quadro.
///
/// A malha é reconstruída quando o tamanho do drawable muda — o que acontece uma
/// vez no PC e uma vez no console, quando o painel real substitui o tamanho
/// pedido (ADR-035).
struct MiraGl {
    malha: MeshHandle,
    shader: ShaderHandle,
    tela: (u32, u32),
}

impl MiraGl {
    fn new(render: &mut RenderContext3D, tela: (u32, u32)) -> Result<Self> {
        let dados = reticle::construir(tela);
        let malha = render.create_mesh(&dados.vertices, &dados.indices)?;
        // `phong` e **não** `flat`. O `flat` tem um termo de Lambert fixo dentro
        // do fragment shader (`0.35 + 0.65 * lambert`) e não lê `u_light_*`; com
        // ele a mira saía a **54 %** do brilho declarado. Medido na captura: os
        // pixels da mira chegavam a (119,126,137) contra os (219,232,255)
        // pedidos, e sobre a rocha a diferença era de 1 nível — invisível.
        // O `phong` com luz emissiva entrega a cor exatamente como está na
        // constante, que é o que uma camada de HUD precisa.
        let shader = render.create_builtin_shader(BuiltinShader::Phong)?;
        Ok(Self {
            malha,
            shader,
            tela,
        })
    }

    /// Reconstrói a malha se a tela mudou de tamanho.
    fn ajustar(&mut self, render: &mut RenderContext3D, tela: (u32, u32)) {
        if tela == self.tela || tela.0 == 0 || tela.1 == 0 {
            return;
        }
        let dados = reticle::construir(tela);
        render.destroy_mesh(self.malha);
        match render.create_mesh(&dados.vertices, &dados.indices) {
            Ok(malha) => {
                self.malha = malha;
                self.tela = tela;
                log::info!("mira reconstruída para {tela:?}");
            }
            Err(erro) => log::warn!("não consegui reconstruir a mira: {erro}"),
        }
    }

    /// Desenha a mira no centro, com a cor e a escala do estado.
    fn desenhar(&self, render: &mut RenderContext3D, estado: Mira, tela: (u32, u32)) -> Result<()> {
        let camera = giochi_defender::ui::camera(tela);
        let cor = estado.cor();
        let escala = estado.escala();
        // Uma luz emissiva para a mira: ela não é um objeto do mundo e não pode
        // escurecer conforme a orientação da câmera.
        render.set_light(arena::luz_do_hud());
        let modelo = Transform3D::from_scale(escala);
        render.draw_mesh_colored(
            self.malha,
            self.shader,
            &modelo,
            &camera,
            Color3D::new(cor[0], cor[1], cor[2], cor[3]),
        )?;
        Ok(())
    }

    fn destruir(self, render: &mut RenderContext3D) {
        render.destroy_mesh(self.malha);
        render.destroy_shader(self.shader);
    }
}

/// Desenha a HUD: uma malha (o quad unitário) e uma chamada por cor.
///
/// ## Por que instanciado e não uma malha por quadro
///
/// A HUD de um quadro tem centenas de quadriláteros — cada célula acesa de cada
/// glifo é um. Reconstruir a malha a cada quadro custaria subir centenas de KB de
/// vértice por quadro para desenhar a mesma coisa; instanciar manda **uma**
/// matriz por quadrilátero e reusa a geometria. O `neon-drift` mediu a diferença
/// na mesma engine: o halo desenhado por contorno custava +6,9 ms e por faixas
/// preenchidas custou +0,4 ms.
///
/// A cor é uniforme por desenho, então a HUD inteira sai em um punhado de
/// chamadas — uma por cor distinta (com o alfa quantizado em [`hud_lotes`]).
struct HudGl {
    quad: MeshHandle,
    shader: ShaderHandle,
    /// Matrizes reaproveitadas entre quadros: `draw_instanced` recebe uma fatia,
    /// então não há razão para alocar um vetor por lote por quadro.
    lotes: Vec<giochi_defender::ui::Lote>,
}

impl HudGl {
    fn new(render: &mut RenderContext3D) -> Result<Self> {
        let malha = giochi_defender::ui::quad_da_hud();
        let quad = render.create_mesh(&malha.vertices, &malha.indices)?;
        let shader = render.create_builtin_shader(BuiltinShader::Instanced)?;
        Ok(Self {
            quad,
            shader,
            lotes: Vec::new(),
        })
    }

    /// Monta os lotes do quadro e desenha.
    fn desenhar(
        &mut self,
        render: &mut RenderContext3D,
        estado: &GameState,
        hud_estado: &Hud,
        tela: (u32, u32),
    ) -> Result<()> {
        let tela = hud::Tela::new(tela.0, tela.1);
        let cena = hud::cena(estado, hud_estado, tela);
        let lotes = giochi_defender::ui::lotes(&cena, tela);

        let camera = giochi_defender::ui::camera((tela.largura as u32, tela.altura as u32));
        // Luz emissiva: a HUD não é um objeto do mundo e não pode escurecer
        // conforme a orientação da câmera.
        render.set_light(arena::luz_do_hud());
        for lote in &lotes {
            let cor = Color3D::new(lote.cor[0], lote.cor[1], lote.cor[2], lote.cor[3]);
            render.draw_instanced(self.quad, self.shader, &lote.matrizes, &camera, cor)?;
        }
        self.lotes = lotes;
        Ok(())
    }

    /// Quantas chamadas de desenho a HUD custou no último quadro.
    fn chamadas(&self) -> usize {
        self.lotes.len()
    }

    fn destruir(self, render: &mut RenderContext3D) {
        render.destroy_mesh(self.quad);
        render.destroy_shader(self.shader);
    }
}

/// Projeta um ponto do mundo em pixels da tela.
///
/// Devolve o **centro** do quadro quando a projeção não existe (ponto atrás da
/// câmera, matriz degenerada): um `+100` no meio da tela é errado, mas um `+100`
/// em `NaN` some para sempre e ninguém descobre por quê.
fn projetar(camera: &Camera3D, ponto: Vec3, tela: (u32, u32)) -> (f32, f32) {
    let (largura, altura) = (tela.0 as f32, tela.1 as f32);
    let matriz = match camera.view_projection() {
        Ok(matriz) => matriz,
        Err(_) => return (largura * 0.5, altura * 0.5),
    };
    match matriz.project_point(ponto) {
        Some(ndc) if ndc.x.is_finite() && ndc.y.is_finite() => {
            ((ndc.x + 1.0) * 0.5 * largura, (1.0 - ndc.y) * 0.5 * altura)
        }
        _ => (largura * 0.5, altura * 0.5),
    }
}

/// Direção "para frente" da câmera, a partir da posição e do alvo.
///
/// É a linha do raio: o centro da tela corresponde exatamente a esta direção,
/// então não há matriz para inverter por tiro.
fn direcao_da_camera(camera: &Camera3D) -> Vec3 {
    let frente = Vec3::new(
        camera.target.x - camera.position.x,
        camera.target.y - camera.position.y,
        camera.target.z - camera.position.z,
    );
    let tamanho = frente.length();
    if !tamanho.is_finite() || tamanho <= 1e-6 {
        return Vec3::new(0.0, 0.0, -1.0);
    }
    Vec3::new(frente.x / tamanho, frente.y / tamanho, frente.z / tamanho)
}

/// O laço principal.
#[allow(clippy::too_many_arguments)]
fn laco(
    plataforma: &Platform,
    janela: &Window,
    render: &mut RenderContext3D,
    entrada: &mut Input,
    arena: &Arena,
    inimigos: &Inimigos,
    mira: &mut MiraGl,
    hud_gl: &mut HudGl,
    camera: &mut OrbitalCamera,
    partida: &mut Partida,
    estado: &mut GameState,
    hud_estado: &mut Hud,
    diag: &Diag,
) -> Result<()> {
    let mut relogio = FrameClock::new();
    let mut laco = GameLoop::new();
    let mut quadros: u64 = 0;
    let mut arma = Weapon::new();
    let mut mira_estado = Mira::new();
    // `--demo-hud` acende os três efeitos na abertura. Sem uma bancada, provar
    // que o `+100` sobe exigiria acertar um cubo com o dedo no aparelho.
    if diag.demo_hud {
        // Um pouco acima e à esquerda do centro, que é onde um cubo em frente
        // cairia com a câmera na posição inicial.
        let (w, h) = janela.drawable_size();
        hud_estado.abateu(w as f32 * 0.42, h as f32 * 0.46, PONTOS_POR_ABATE);
        hud_estado.levou_dano();
        hud_estado.anunciar_onda(2);
    }
    let mut abates: u32 = 0;
    let mut benchmark = Benchmark::new();
    // `--benchmark N` implica rodar N quadros, sem o teto de 60 Hz: medir o
    // intervalo com uma espera artificial dentro dele mediria a espera.
    let limite = diag.benchmark.or(diag.frames);

    // Início do quadro anterior: o que se mede é o intervalo **entre inícios**,
    // que é o que o jogador sente. Medir o fim de um quadro até o fim do outro
    // deixaria o trabalho de preparação de fora justamente no primeiro quadro de
    // cada rajada.
    let mut inicio_anterior: Option<Instant> = None;

    loop {
        let inicio = Instant::now();
        if diag.benchmark.is_some() {
            if let Some(anterior) = inicio_anterior {
                benchmark.registrar(inicio.duration_since(anterior));
            }
            inicio_anterior = Some(inicio);
        }

        let eventos = plataforma.poll_events();
        let mut pediu_sair = false;
        for evento in &eventos {
            if quit_signal(evento) != QuitSignal::None {
                pediu_sair = true;
            }
        }
        entrada.update(&eventos);
        if pediu_sair || entrada.key_pressed(KEY_ESCAPE) {
            break;
        }

        // Em modo de captura o passo é fixo: dois processos têm de simular o
        // mesmo número de segundos, senão comparar quadros mede o relógio.
        let delta = if limite.is_some() {
            laco.fixed_timestep()
        } else {
            relogio.tick()
        };

        let (largura, altura) = janela.drawable_size();
        let camera3d: Camera3D = camera.camera(largura, altura);

        // O tiro é resolvido **antes** de a simulação andar este quadro: assim o
        // raio usa a câmera que o jogador está vendo, e não a de 16 ms atrás.
        let atirar = entrada.mouse_down(MOUSE_LEFT)
            || entrada.gamepad().is_down(BUTTON_RIGHT_SHOULDER)
            || entrada.gamepad().is_down(BUTTON_SOUTH)
            // `--atirar` mantém o gatilho apertado pelo mesmo caminho da tecla:
            // não há um segundo laço de tiro só para o diagnóstico.
            || diag.atirar;
        if atirar && !estado.acabou() {
            let origem = camera3d.position;
            let direcao = direcao_da_camera(&camera3d);
            if let Some(atingido) = partida.atirar(&mut arma, origem, direcao) {
                abates += 1;
                estado.pontuar(PONTOS_POR_ABATE);
                mira_estado.acertou();
                // O `+100` nasce onde o tiro bateu, projetado com a **mesma**
                // câmera do desenho deste quadro.
                let (px, py) = projetar(&camera3d, atingido.posicao, (largura, altura));
                hud_estado.abateu(px, py, PONTOS_POR_ABATE);
                log::info!(
                    "abate {}: +{PONTOS_POR_ABATE} (score {}), {:.1} m",
                    abates,
                    estado.score,
                    atingido.distancia
                );
            }
        }

        let passos = laco.advance(delta);
        for _ in 0..passos {
            atualizar_camera(camera, entrada, laco.fixed_timestep());
            arma.atualizar(laco.fixed_timestep());
            mira_estado.atualizar(laco.fixed_timestep());
            hud_estado.atualizar(laco.fixed_timestep());
            let derrotas_antes = partida.derrotas();
            partida.atualizar(laco.fixed_timestep(), estado);
            if partida.derrotas() > derrotas_antes {
                hud_estado.levou_dano();
            }
        }

        // A onda e o fim são anunciados pelo **estado**, uma vez por mudança.
        hud_estado.anunciar_onda(estado.wave);
        if estado.acabou() || estado.venceu() {
            hud_estado.anunciar_fim(estado.venceu());
        }

        render.begin_frame(janela);
        arena.desenhar(render, &camera3d)?;
        inimigos.desenhar(render, partida, &camera3d)?;
        if diag.mira {
            mira.ajustar(render, (largura, altura));
            mira.desenhar(render, mira_estado, (largura, altura))?;
        }
        if diag.hud {
            hud_gl.desenhar(render, estado, hud_estado, (largura, altura))?;
        }

        // A captura tem de acontecer ANTES da troca de buffers: depois do
        // `end_frame` o buffer de trás já tem o quadro seguinte (ou lixo).
        let capturar = diag
            .quadro
            .as_deref()
            .filter(|_| limite.is_some_and(|total| quadros + 1 >= total));
        if let Some(caminho) = capturar {
            let (largura, altura) = render.drawable_size();
            let pixels = render.read_pixels(largura, altura)?;
            diag::escrever_bmp(std::path::Path::new(caminho), largura, altura, &pixels)?;
            log::info!("quadro gravado em '{caminho}' ({largura}x{altura})");
            if diag.hud {
                log::info!(
                    "hud: {} chamada(s) de desenho, {} efeito(s) vivo(s)",
                    hud_gl.chamadas(),
                    hud_estado.flutuantes()
                );
            }
        }

        // Trabalho de submissão: do começo do desenho até antes da troca de
        // buffers. A troca é onde o KMSDRM espera o vblank, e incluir a espera
        // aqui faria o número ser sempre igual ao intervalo.
        if diag.benchmark.is_some() {
            benchmark.registrar_trabalho(inicio.elapsed());
        }

        render.end_frame(janela);

        // Drenar a fila de erro a cada quadro é o que transforma "tela preta"
        // numa falha com nome.
        let erro = render.take_gl_error();
        if erro != 0 {
            bail!("erro de GL 0x{erro:04x} no quadro {quadros}");
        }

        quadros += 1;

        if let Some(total) = limite {
            if quadros >= total {
                break;
            }
            // No modo de medição não se dorme: o intervalo medido é o do quadro,
            // não o do quadro mais uma espera escolhida por nós.
            continue;
        }

        // Teto de 60 quadros por segundo. O vsync já limita no aparelho; no PC,
        // sem esta espera, um quadro trivial roda a milhares de Hz e aquece a
        // máquina à toa.
        let gasto = inicio.elapsed().as_millis() as u32;
        if gasto < QUADRO_MS {
            plataforma.delay_ms(QUADRO_MS - gasto);
        }
    }

    if diag.benchmark.is_some() {
        log::info!("{}", benchmark.resumo());
    }

    let (largura, altura) = render.drawable_size();
    log::info!(
        "{quadros} quadros renderizados · drawable {largura}x{altura} · {:.1} s simulados",
        quadros as f32 * laco.fixed_timestep()
    );
    Ok(())
}
