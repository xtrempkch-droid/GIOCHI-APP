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
    AXIS_RIGHT_X, AXIS_RIGHT_Y, BUTTON_EAST, BUTTON_RIGHT_SHOULDER, BUTTON_SOUTH,
};
use giochi_engine::input::keyboard::{
    KEY_A, KEY_D, KEY_DOWN, KEY_ESCAPE, KEY_LEFT, KEY_RIGHT, KEY_S, KEY_SPACE, KEY_UP, KEY_W,
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

use giochi_defender::audio::volume::{Alsa, Volume};
use giochi_defender::audio::{Som, Sons};
use giochi_defender::bench::Benchmark;
use giochi_defender::diag::{self, Diag};
use giochi_defender::gameplay::arena::{self, Arena};
use giochi_defender::gameplay::camera::OrbitalCamera;
use giochi_defender::gameplay::partida::{Inimigos, Partida};
use giochi_defender::gameplay::tremor::Tremor;
use giochi_defender::gameplay::weapon::{Weapon, PONTOS_POR_ABATE};
use giochi_defender::input_evdev::{ButtonsMap, EvdevInput};
use giochi_defender::screens::{self, Acao, Menu, Resultado, Tela};
use giochi_defender::state::{cota_da_onda, GameState, WAVES_TOTAIS};
use giochi_defender::ui::hud::{self, Hud};
use giochi_defender::ui::reticle::{self, Mira};
use giochi_defender::utils::math::{graus_para_radianos, radianos_para_graus};

/// Tamanho pedido da janela. No aparelho o painel manda (ADR-035).
const LARGURA: u32 = 1280;
/// Altura pedida da janela.
const ALTURA: u32 = 720;
/// Título da janela.
const TITULO: &str = "GIOCHI DEFENDER";

/// `SDLK_VOLUMEUP` do SDL2 (`0x4000_0080`).
///
/// Declarado aqui porque a engine exporta 15 teclas e não estas duas: o teclado
/// dela é um conjunto mínimo por app, não o enum inteiro do SDL. O valor vem do
/// `SDL_keycode.h` (`SDLK_SCANCODE_MASK | 0x80`), e o teste
/// `as_teclas_de_volume_sao_as_do_sdl` confere contra o `sdl2` que está linkado —
/// escrito à mão, ele poderia apodrecer em silêncio.
const KEY_VOLUMEUP: giochi_engine::input::keyboard::KeyCode = 0x4000_0080;
/// `SDLK_VOLUMEDOWN` do SDL2 (`0x4000_0081`).
const KEY_VOLUMEDOWN: giochi_engine::input::keyboard::KeyCode = 0x4000_0081;
/// `SDLK_RETURN`. A engine exporta 15 teclas e não esta; o valor é o do SDL.
const KEY_ENTER: giochi_engine::input::keyboard::KeyCode = 13;
/// `SDLK_BACKSPACE`.
const KEY_BACKSPACE: giochi_engine::input::keyboard::KeyCode = 8;
/// `SDLK_r`.
const KEY_R: giochi_engine::input::keyboard::KeyCode = 114;

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
    // O balanceamento não abre janela nem toca em GL: é a simulação sozinha com
    // um piloto automático. Sai antes de qualquer recurso ser criado.
    if diag.balanceamento {
        return relatorio_de_balanceamento(diag.semente);
    }
    if let Some(segundos) = diag.espiar_input {
        return espiar_input(segundos);
    }

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
    // Os sons são sintetizados aqui e carregados por `from_raw_buffer`: o mixer
    // já está aberto pela engine, e o `AudioSystem::load_sound` só aceita
    // caminho de arquivo. Ver `src/audio.rs`.
    let mut sons = if diag.som {
        Sons::carregar(audio.is_available())
    } else {
        log::info!("som desligado por --sem-som");
        Sons::mudo()
    };
    if diag.testar_som {
        tocar_todos(&mut sons);
    }
    // O controle de volume do sistema: as teclas do aparelho durante o jogo
    // (o launcher fica em `fork`+`wait` e não as lê) e o `--volume`.
    // O controle é lido de `/dev/input`, **não** pela API de gamepad do SDL: os
    // handhelds usam driver próprio e não têm entrada no `SDL_GameControllerDB`,
    // então no PC qualquer controle funciona e no aparelho nenhum funciona. Sem
    // isto o jogo abria, desenhava o menu e parava ali — o launcher esperava em
    // `fork`+`wait` e o aparelho parecia travado.
    // `GIOCHI_INPUT_DIR` existe porque no PC o usuário não está no grupo
    // `input` e `/dev/input/event*` é 660 root:input — sem o override não há
    // como exercitar este caminho fora do aparelho. Mesma ideia do
    // `GIOCHI_CATALOGO`: um caminho, não uma segunda implementação.
    let diretorio_entrada = std::env::var("GIOCHI_INPUT_DIR")
        .unwrap_or_else(|_| giochi_defender::input_evdev::DIRETORIO_DE_ENTRADA.to_string());
    let mut controle = if diag.evdev {
        EvdevInput::abrir(
            std::path::Path::new(&diretorio_entrada),
            ButtonsMap::carregar(std::path::Path::new(
                giochi_defender::input_evdev::MAPA_PADRAO,
            )),
        )
    } else {
        log::info!("controle: leitura de /dev/input desligada por --sem-evdev");
        EvdevInput::abrir(std::path::Path::new("/nao/existe"), ButtonsMap::padrao())
    };

    let mut volume = Volume::novo(Alsa);
    if !volume.disponivel() {
        log::warn!(
            "volume: sem controle (amixer ausente ou sem Master); \
             o som segue o volume do sistema, mas as teclas não o alteram"
        );
    }
    if let Some(alvo) = diag.volume {
        let aplicado = volume.definir(alvo);
        log::info!(
            "volume: pedido {alvo}%, aplicado {aplicado}% ({} sistema antes)",
            if volume.disponivel() { "com" } else { "sem" }
        );
    }

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
    let mut tremor = Tremor::novo();
    if diag.tremor {
        tremor.sacudir_dano();
    }
    aplicar_preset(&mut camera, &diag);
    let mut estado = GameState::new();
    let mut partida = Partida::new(diag.semente);
    let mut hud_estado = Hud::new();
    let mut menu = Menu::novo();
    let mut tela = match diag.tela_inicial() {
        diag::TelaInicial::Menu => Tela::Menu,
        diag::TelaInicial::Jogo => Tela::Jogando,
        diag::TelaInicial::Vitoria => {
            estado.wave = WAVES_TOTAIS + 1;
            Tela::Fim { venceu: true }
        }
        diag::TelaInicial::Derrota => {
            estado.lives = 0;
            Tela::Fim { venceu: false }
        }
    };
    log::info!("tela inicial: {tela:?}");

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
        // As consequências do dano passam pela **mesma** função do laço: sem
        // isso a captura depois de `--simular` mostraria uma HUD que nunca levou
        // dano, e o `--testar-som` seria o único jeito de ouvir o som de dano.
        let derrotas_antes = partida.derrotas();
        partida.atualizar(segundos, &mut estado);
        for _ in derrotas_antes..partida.derrotas() {
            reagir_ao_dano(&mut hud_estado, &mut sons, &mut tremor);
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
        &mut sons,
        &mut volume,
        &mut controle,
        &mut tremor,
        &mut camera,
        &mut partida,
        &mut estado,
        &mut hud_estado,
        &mut menu,
        &mut tela,
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
    log::info!(
        "som: {} efeito(s) tocado(s) na sessão{}",
        sons.tocados(),
        if sons.esta_mudo() { " (mudo)" } else { "" }
    );
    drop(sons);
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

/// Empurra uma tecla na fila do SDL, como se viesse do teclado.
///
/// Usa o `EventSender` do próprio SDL, então o evento percorre o caminho de
/// produção inteiro: fila do SDL → `poll_events` da engine → `Input` →
/// `key_pressed`. Um atalho que marcasse o estado direto testaria a minha
/// suposição, não o jogo — e o `--teclas` existe justamente para não ter
/// suposição.
fn injetar_tecla(plataforma: &Platform, nome: &str) -> Result<()> {
    let Some(codigo) = diag::codigo_da_tecla(nome) else {
        bail!("--teclas: '{nome}' não é uma tecla conhecida");
    };
    // `from_i32` devolve `Option`: um código que o SDL não reconhece viraria
    // uma tecla "válida" e sem efeito, e o diagnóstico mentiria.
    let tecla = sdl2::keyboard::Keycode::from_i32(codigo as i32)
        .ok_or_else(|| anyhow::anyhow!("--teclas: código {codigo} não é tecla do SDL"))?;
    for eh_descida in [true, false] {
        let evento = if eh_descida {
            sdl2::event::Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: Some(tecla),
                scancode: None,
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            }
        } else {
            sdl2::event::Event::KeyUp {
                timestamp: 0,
                window_id: 0,
                keycode: Some(tecla),
                scancode: None,
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            }
        };
        // `Platform::push_event` da engine já embrulha o `SDL_PushEvent` — a
        // fachada existe, e é ela que mantém a fila coerente com o
        // `poll_events` do mesmo objeto.
        plataforma
            .push_event(evento)
            .map_err(|erro| anyhow::anyhow!("não consegui injetar '{nome}': {erro}"))?;
    }
    Ok(())
}

/// Lê `/dev/input` cru por `segundos` e imprime cada código recebido.
///
/// É o que separa **"o botão não chega"** de **"chega e o app ignora"** — a
/// distinção que o launcher já aprendeu a fazer com `aparelho.sh input`. Sem
/// ela, um botão que não responde manda procurar defeito no código quando o
/// problema está no driver ou no mapa.
///
/// Não abre janela nem GL: roda por SSH, que é onde a pergunta aparece.
fn espiar_input(segundos: f32) -> Result<()> {
    use giochi_defender::input_evdev::{ButtonsMap, EvdevInput};

    let mapa = ButtonsMap::carregar(std::path::Path::new(
        giochi_defender::input_evdev::MAPA_PADRAO,
    ));
    let mut leitor = EvdevInput::abrir(
        std::path::Path::new(giochi_defender::input_evdev::DIRETORIO_DE_ENTRADA),
        mapa.clone(),
    );

    println!("GIOCHI DEFENDER — espiar /dev/input por {segundos:.0}s");
    println!("  dispositivos: {}", leitor.dispositivos());
    for nome in leitor.nomes() {
        println!("    {nome}");
    }
    println!("  mapa: {} símbolo(s)", leitor.mapa().len());
    if !leitor.mapa().desconhecidos().is_empty() {
        println!(
            "  ATENÇÃO: {} símbolo(s) do mapa não reconhecido(s): {}",
            leitor.mapa().desconhecidos().len(),
            leitor.mapa().desconhecidos().join(", ")
        );
    }
    println!();
    println!("pressione os botões agora (o código de cada um sai abaixo):");

    // Nome de cada código que o mapa conhece, para a leitura ser em português.
    let mut nomes: Vec<(u16, String)> = Vec::new();
    for (nome, codigo) in mapa.pares() {
        nomes.push((codigo, nome));
    }
    nomes.sort();

    let inicio = Instant::now();
    let mut vistos: Vec<u16> = Vec::new();
    let mut anterior = giochi_defender::input_evdev::Controles::default();
    while inicio.elapsed().as_secs_f32() < segundos {
        leitor.poll();
        let agora = leitor.controles();
        let mut mudou = Vec::new();
        if agora.cima != anterior.cima {
            mudou.push(("cima", agora.cima));
        }
        if agora.baixo != anterior.baixo {
            mudou.push(("baixo", agora.baixo));
        }
        if agora.esquerda != anterior.esquerda {
            mudou.push(("esquerda", agora.esquerda));
        }
        if agora.direita != anterior.direita {
            mudou.push(("direita", agora.direita));
        }
        if agora.confirmar != anterior.confirmar {
            mudou.push(("confirmar (a)", agora.confirmar));
        }
        if agora.voltar != anterior.voltar {
            mudou.push(("voltar (b)", agora.voltar));
        }
        if agora.atirar != anterior.atirar {
            mudou.push(("atirar (r)", agora.atirar));
        }
        if agora.menu != anterior.menu {
            mudou.push(("menu (start)", agora.menu));
        }
        if agora.volume_mais != anterior.volume_mais {
            mudou.push(("volume +", agora.volume_mais));
        }
        if agora.volume_menos != anterior.volume_menos {
            mudou.push(("volume -", agora.volume_menos));
        }
        for (nome, apertou) in mudou {
            println!(
                "  [{:5.2}s] {nome} {}",
                inicio.elapsed().as_secs_f32(),
                if apertou { "APERTOU" } else { "soltou" }
            );
            if apertou {
                vistos.push(0);
            }
        }
        anterior = agora;
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    println!();
    if vistos.is_empty() {
        println!("nenhum botão foi visto — o problema está entre o driver e o mapa");
    } else {
        println!("{} apertão(ões) visto(s)", vistos.len());
    }
    Ok(())
}

/// Roda o balanceamento em várias sementes e imprime a tabela.
///
/// Uma semente só responde "é ganhável nesta partida"; cinco respondem "é
/// ganhável". A diferença importa porque o spawn é sorteado: uma partida fácil
/// por acaso esconderia uma dificuldade injusta.
fn relatorio_de_balanceamento(semente: u64) -> Result<()> {
    const PARTIDAS: u64 = 5;
    const LIMITE: f32 = 240.0;

    println!("GIOCHI DEFENDER — balanceamento com piloto automático");
    println!(
        "limites do piloto: giro {:.1} rad/s, reacao {:.0} ms, tolerancia {:.0} mrad",
        giochi_defender::gameplay::piloto::GIRO_POR_SEGUNDO,
        giochi_defender::gameplay::piloto::REACAO * 1000.0,
        giochi_defender::gameplay::piloto::TOLERANCIA * 1000.0
    );
    println!();
    println!("  #  semente   tempo   wave  vidas  abates  erros  precisao  resultado");

    let mut vitorias = 0;
    let mut soma_tempo = 0.0;
    let mut soma_vidas = 0.0;
    let mut soma_precisao = 0.0;
    for i in 0..PARTIDAS {
        let s = semente.wrapping_add(i);
        let r = balancear(s, LIMITE);
        if r.venceu {
            vitorias += 1;
        }
        soma_tempo += r.segundos;
        soma_vidas += r.vidas as f32;
        soma_precisao += r.precisao();
        println!(
            "{:3}  {:#08x}  {:6.1}s  {:4}  {:5}  {:6}  {:5}  {:7.1}%  {}",
            i + 1,
            s,
            r.segundos,
            r.wave,
            r.vidas,
            r.abates,
            r.erros,
            r.precisao() * 100.0,
            if r.venceu { "VITORIA" } else { "derrota" }
        );
    }
    println!();
    println!(
        "resumo: {vitorias}/{PARTIDAS} vitorias · {:.1}s de media · {:.1} vidas · {:.1}% de precisao",
        soma_tempo / PARTIDAS as f32,
        soma_vidas / PARTIDAS as f32,
        soma_precisao / PARTIDAS as f32 * 100.0
    );
    Ok(())
}

/// Relatório de uma partida jogada pelo piloto automático.
#[derive(Debug, Default, Clone, Copy)]
struct Relatorio {
    segundos: f32,
    abates: u32,
    erros: u32,
    vidas: i32,
    wave: u32,
    venceu: bool,
}

impl Relatorio {
    fn precisao(&self) -> f32 {
        let tiros = self.abates + self.erros;
        if tiros == 0 {
            return 0.0;
        }
        self.abates as f32 / tiros as f32
    }
}

/// Joga a partida inteira com o [`piloto`] e devolve o que aconteceu.
///
/// Sem GL e sem janela: a simulação não precisa de nenhum dos dois, e é o que
/// permite rodar o balanceamento dentro do `cargo test` e numa sessão SSH.
///
/// O piloto gira `GIRO_POR_SEGUNDO` por segundo, então ele **não** é um `atan2`
/// instantâneo: mirar num cubo do outro lado do asteroide custa tempo, que é o
/// custo real do jogador.
fn balancear(semente: u64, limite_de_segundos: f32) -> Relatorio {
    use giochi_defender::gameplay::piloto::{self, Ameaca};

    let mut estado = GameState::new();
    let mut partida = Partida::new(semente);
    let mut arma = Weapon::new();
    let mut camera = OrbitalCamera::new();
    let mut relatorio = Relatorio::default();
    let dt = 1.0 / 60.0;
    let mut tempo: f32 = 0.0;
    let mut tempo_no_alvo: f32 = piloto::REACAO;
    // O alvo é rastreado pela **posição**, não pelo índice na lista: o índice
    // muda quando qualquer cubo morre, e a primeira versão zerava o tempo de
    // reação a cada morte — o piloto nunca chegava a atirar (medido: 0 abates,
    // 0 erros em cinco partidas).
    let mut alvo_anterior: Option<giochi_engine_3d::math3d::Vec3> = None;

    while tempo < limite_de_segundos {
        let panorama = piloto::panorama_da_camera(&camera, tempo_no_alvo);
        let ameacas: Vec<Ameaca> = partida
            .pool()
            .iter_alive()
            .map(|(_, inimigo)| {
                let p = inimigo.position;
                Ameaca {
                    posicao: p,
                    distancia_ao_centro: (p.x * p.x + p.y * p.y + p.z * p.z).sqrt(),
                }
            })
            .collect();

        // O relógio do alvo zera quando o alvo **muda**: é o tempo de reação.
        let escolhido = piloto::escolher_alvo(&panorama, &ameacas);
        let posicao_escolhida = escolhido.map(|i| ameacas[i].posicao);
        let mudou = match (posicao_escolhida, alvo_anterior) {
            (Some(a), Some(b)) => {
                let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
                (dx * dx + dy * dy + dz * dz).sqrt() > 0.5
            }
            (None, None) => false,
            _ => true,
        };
        if mudou {
            tempo_no_alvo = 0.0;
        }
        alvo_anterior = posicao_escolhida;
        tempo_no_alvo += dt;

        let decisao = piloto::decidir(&panorama, &ameacas, dt);
        camera.yaw += decisao.yaw;
        camera.pitch = (camera.pitch + decisao.pitch).clamp(
            giochi_defender::gameplay::camera::PITCH_MINIMO,
            giochi_defender::gameplay::camera::PITCH_MAXIMO,
        );

        if decisao.atirar {
            let camera3d = camera.camera(640, 480);
            let origem = camera3d.position;
            let direcao = direcao_da_camera(&camera3d);
            let pronta = arma.pronta();
            let resultado = partida.atirar(&mut arma, origem, direcao);
            if pronta {
                match resultado {
                    Some(_) => {
                        relatorio.abates += 1;
                        estado.pontuar(PONTOS_POR_ABATE);
                        estado.registrou_abate();
                    }
                    None => relatorio.erros += 1,
                }
            }
        }

        arma.atualizar(dt);
        partida.atualizar(dt, &mut estado);
        tempo += dt;

        if estado.acabou() || estado.venceu() {
            break;
        }
    }

    relatorio.segundos = tempo;
    relatorio.vidas = estado.lives;
    relatorio.wave = estado.wave;
    relatorio.venceu = estado.venceu();
    relatorio
}

/// Recomeça a partida do zero.
///
/// A lista é o que **volta ao início**, e ela é maior do que parece: além do
/// placar e das vidas, o pool de cubos, o temporizador de spawn, o sorteio, a
/// cadência da arma, os efeitos da HUD e o tremor. Zerar só o placar deixaria a
/// segunda partida começando com os cubos da primeira no ar.
fn reiniciar_partida(
    partida: &mut Partida,
    estado: &mut GameState,
    hud_estado: &mut Hud,
    tremor: &mut Tremor,
    arma: &mut Weapon,
    mira_estado: &mut Mira,
) {
    estado.reiniciar();
    partida.limpar();
    hud_estado.limpar();
    *tremor = Tremor::novo();
    *arma = Weapon::new();
    *mira_estado = Mira::new();
}

/// Tudo o que um dano no asteroide provoca, num lugar só.
///
/// Existe porque o dano acontece em **dois** caminhos — o laço e o `--simular` —
/// e a primeira versão pôs HUD, som e tremor em cada um. O `--simular` ficou sem
/// som e sem tremor, e o sintoma foi "o diagnóstico mostra o dano mas não toca
/// nada". Uma função para as três consequências torna a divergência impossível.
fn reagir_ao_dano(hud_estado: &mut Hud, sons: &mut Sons, tremor: &mut Tremor) {
    hud_estado.levou_dano();
    sons.tocar(Som::Dano);
    tremor.sacudir_dano();
}

/// Onde o abate aconteceu e com que câmera o desenho do quadro vai ser feito.
///
/// Agrupado porque são cinco valores que só fazem sentido juntos, e porque uma
/// função de oito parâmetros posicionais é fácil de chamar na ordem errada — a
/// troca de `tela` com `posicao` compilaria.
struct AlvoDoAbate<'a> {
    mira: &'a mut Mira,
    camera: &'a Camera3D,
    tela: (u32, u32),
    posicao: Vec3,
    pontos: u32,
}

/// Tudo o que um abate provoca. Ver [`reagir_ao_dano`].
fn reagir_ao_abate(
    hud_estado: &mut Hud,
    sons: &mut Sons,
    tremor: &mut Tremor,
    alvo: AlvoDoAbate<'_>,
) {
    alvo.mira.acertou();
    sons.tocar(Som::Abate);
    tremor.sacudir_abate();
    let (px, py) = projetar(alvo.camera, alvo.posicao, alvo.tela);
    hud_estado.abateu(px, py, alvo.pontos);
}

/// Toca os cinco efeitos em sequência, com uma pausa entre eles.
///
/// Existe porque **ouvir** o som do aparelho não é possível por SSH, e sem um
/// comando que toque cada efeito o diagnóstico no console vira suposição. Cada
/// linha impressa é o que o mixer respondeu, e a pausa é o que permite distinguir
/// um som do outro de fora.
fn tocar_todos(sons: &mut Sons) {
    if sons.esta_mudo() {
        log::warn!("--testar-som: sem dispositivo de áudio; nada a tocar");
        return;
    }
    for som in Som::TODOS {
        let inicio = Instant::now();
        sons.tocar(som);
        log::info!(
            "som '{}': {:.0} ms de amostra, {:.1} ms de chamada",
            som.nome(),
            som.duracao() * 1000.0,
            inicio.elapsed().as_secs_f32() * 1000.0
        );
        std::thread::sleep(std::time::Duration::from_millis(650));
    }
    log::info!("--testar-som: {} efeito(s) tocado(s)", sons.tocados());
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
fn atualizar_camera(
    camera: &mut OrbitalCamera,
    entrada: &Input,
    pad: giochi_defender::input_evdev::Controles,
    dt: f32,
) {
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
    // O direcional do controle entra como **nível**: segurar gira, que é o que
    // o jogador espera de uma câmera. O `pitch` é a elevação do olho, então
    // "para cima" no direcional sobe o olho — e olhar para baixo é o que se
    // ganha com isso, igual ao `W`.
    if entrada.key_down(KEY_UP) || entrada.key_down(KEY_W) || pad.cima {
        pitch += 1.0;
    }
    if entrada.key_down(KEY_DOWN) || entrada.key_down(KEY_S) || pad.baixo {
        pitch -= 1.0;
    }
    if pad.esquerda {
        yaw -= 1.0;
    }
    if pad.direita {
        yaw += 1.0;
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
    ///
    /// A `cena` vem pronta porque **qual** cena depende da tela do jogo: menu,
    /// partida e fim desenham coisas diferentes, e a escolha pertence ao laço
    /// (que sabe onde está), não ao desenhista.
    fn desenhar(
        &mut self,
        render: &mut RenderContext3D,
        cena: hud::Cena,
        tela: (u32, u32),
    ) -> Result<()> {
        let tela = hud::Tela::new(tela.0, tela.1);
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
    sons: &mut Sons,
    volume: &mut Volume<Alsa>,
    controle: &mut EvdevInput,
    tremor: &mut Tremor,
    camera: &mut OrbitalCamera,
    partida: &mut Partida,
    estado: &mut GameState,
    hud_estado: &mut Hud,
    menu: &mut Menu,
    tela: &mut Tela,
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
        // `--teclas`: uma tecla por quadro, injetada na fila do SDL **antes** do
        // `poll_events` deste quadro. Um comando por quadro garante que
        // `key_pressed` (borda) veja cada um separadamente.
        if let Some(nome) = diag.teclas.get(quadros as usize) {
            injetar_tecla(plataforma, nome)?;
            log::info!("--teclas: quadro {quadros} -> '{nome}'");
        }
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

        // O controle é lido aqui, uma vez por quadro, e mesclado com o SDL: no
        // PC vale o teclado/mouse/SDL-gamepad, no aparelho vale o evdev. As duas
        // fontes somam em vez de competir.
        controle.poll();
        let pad = controle.controles();

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
        let camera3d: Camera3D = camera.camera_com_tremor(largura, altura, tremor.deslocamento());

        // ── Entrada de tela ─────────────────────────────────────
        // A ação é resolvida pela tela atual e a transição vem de
        // `screens::processar`, que é pura e testada. O laço não decide nada.
        // `Enter`/`Espaço` confirmam em qualquer tela. O `A` **só** fora da
        // partida: em jogo ele orbita a câmera para a esquerda, e reaproveitá-lo
        // como "confirmar" faria o jogador recomeçar a partida ao girar a câmera
        // quando as vidas acabam.
        let confirmar = entrada.key_pressed(KEY_SPACE)
            || entrada.key_pressed(KEY_ENTER)
            || pad.confirmar_apertou
            || (*tela != Tela::Jogando && entrada.key_pressed(KEY_A));
        // `B` do controle (botão leste) e `Backspace`/`Esc` no teclado: o
        // aparelho não tem teclado, então o leste é o caminho real do jogador.
        let voltar = entrada.key_pressed(KEY_BACKSPACE) || entrada.gamepad().is_down(BUTTON_EAST);
        let reiniciar = entrada.key_pressed(KEY_R);
        let mut acao = None;
        if *tela == Tela::Menu {
            if entrada.key_pressed(KEY_UP) || entrada.key_pressed(KEY_W) || pad.cima_apertou {
                menu.mover(false);
            }
            if entrada.key_pressed(KEY_DOWN) || entrada.key_pressed(KEY_S) || pad.baixo_apertou {
                menu.mover(true);
            }
        }
        if reiniciar {
            acao = Some(Acao::Reiniciar);
        } else if *tela == Tela::Jogando && pad.menu_apertou {
            // `start` no meio da partida volta ao menu: é a saída que o jogador
            // tem no aparelho, onde não há tecla.
            acao = Some(Acao::Voltar);
        } else if confirmar {
            acao = Some(Acao::Confirmar);
        } else if voltar {
            acao = Some(Acao::Voltar);
        }
        // O mouse também confirma no menu, e o gatilho do controle também: no
        // aparelho não há teclado, e um menu que só responde a `Enter` seria
        // inoperável no X35H.
        if acao.is_none() && *tela != Tela::Jogando && entrada.mouse_down(MOUSE_LEFT) {
            acao = Some(Acao::Confirmar);
        }
        // Sem tecla nenhuma a tela pode mudar mesmo assim (a partida acaba
        // sozinha), então o `processar` roda **todo** quadro.
        let acao = acao.unwrap_or(Acao::Nenhuma);

        // No menu, o `A` respeita o item em foco (INICIAR ou SAIR); no resto, a
        // regra é a de `screens::processar`.
        let resultado = if *tela == Tela::Menu && acao == Acao::Confirmar {
            menu.confirmar()
        } else {
            screens::processar(*tela, acao, estado)
        };
        match resultado {
            Resultado::Sair => break,
            Resultado::Fica => {}
            Resultado::Vai(nova) => {
                if nova == Tela::Jogando && *tela != Tela::Jogando {
                    reiniciar_partida(
                        partida,
                        estado,
                        hud_estado,
                        tremor,
                        &mut arma,
                        &mut mira_estado,
                    );
                    abates = 0;
                    log::info!("partida iniciada");
                }
                log::info!("tela: {:?} -> {:?}", *tela, nova);
                *tela = nova;
            }
        }

        // O tiro é resolvido **antes** de a simulação andar este quadro: assim o
        // raio usa a câmera que o jogador está vendo, e não a de 16 ms atrás.
        let atirar = *tela == Tela::Jogando
            && (entrada.mouse_down(MOUSE_LEFT)
                || entrada.gamepad().is_down(BUTTON_RIGHT_SHOULDER)
                || entrada.gamepad().is_down(BUTTON_SOUTH)
                || pad.atirar
                || pad.confirmar
                // `--atirar` mantém o gatilho apertado pelo mesmo caminho da
                // tecla: não há um segundo laço de tiro só para o diagnóstico.
                || diag.atirar);
        if atirar && !estado.acabou() {
            let origem = camera3d.position;
            let direcao = direcao_da_camera(&camera3d);
            let pronta_antes = arma.pronta();
            let resultado = partida.atirar(&mut arma, origem, direcao);
            // O som é do **disparo**, não do acerto: sai mesmo quando o tiro
            // passa longe. Mas só quando a cadência deixou atirar — o gatilho
            // apertado não pode virar metralhadora de som.
            if pronta_antes {
                sons.tocar(Som::Tiro);
            }
            if let Some(atingido) = resultado {
                abates += 1;
                estado.pontuar(PONTOS_POR_ABATE);
                // A cota da onda é quem decide a vitória; sem isto `wave` nunca
                // saía de 1 e a tela de vitória era inalcançável.
                if let Some(onda) = estado.registrou_abate() {
                    log::info!("onda {onda} começou ({} abates)", cota_da_onda(onda));
                }
                // O `+100` nasce onde o tiro bateu, projetado com a **mesma**
                // câmera do desenho deste quadro.
                reagir_ao_abate(
                    hud_estado,
                    sons,
                    tremor,
                    AlvoDoAbate {
                        mira: &mut mira_estado,
                        camera: &camera3d,
                        tela: (largura, altura),
                        posicao: atingido.posicao,
                        pontos: PONTOS_POR_ABATE,
                    },
                );
                log::info!(
                    "abate {}: +{PONTOS_POR_ABATE} (score {}), {:.1} m",
                    abates,
                    estado.score,
                    atingido.distancia
                );
            }
        }

        // Teclas de volume: quem as lia era o launcher, que está bloqueado em
        // `fork`+`wait` enquanto o jogo roda. Sem isto, o jogador não consegue
        // abaixar o som justamente quando está jogando.
        if entrada.key_pressed(KEY_VOLUMEUP) || pad.volume_mais {
            let nivel = volume.aumentar();
            log::info!("volume: {nivel}% (tecla)");
        }
        if entrada.key_pressed(KEY_VOLUMEDOWN) || pad.volume_menos {
            let nivel = volume.diminuir();
            log::info!("volume: {nivel}% (tecla)");
        }

        // Pular o `advance` já congela o acumulador: ele só cresce dentro dele,
        // então o tempo do menu não é despejado quando a partida começa.
        let passos = if tela.simula() {
            laco.advance(delta)
        } else {
            0
        };
        for _ in 0..passos {
            atualizar_camera(camera, entrada, pad, laco.fixed_timestep());
            arma.atualizar(laco.fixed_timestep());
            mira_estado.atualizar(laco.fixed_timestep());
            hud_estado.atualizar(laco.fixed_timestep());
            tremor.atualizar(laco.fixed_timestep());
            let derrotas_antes = partida.derrotas();
            partida.atualizar(laco.fixed_timestep(), estado);
            if partida.derrotas() > derrotas_antes {
                reagir_ao_dano(hud_estado, sons, tremor);
            }
        }

        // A onda e o fim são anunciados pelo **estado**, uma vez por mudança.
        // O `Hud` é quem decide se é novidade (o aviso é idempotente), então o
        // som segue a mesma decisão em vez de repetir a condição — duas cópias
        // da regra envelheceriam diferente.
        if hud_estado.anunciar_onda(estado.wave) {
            sons.tocar(Som::Onda);
        }
        if (estado.acabou() || estado.venceu()) && hud_estado.anunciar_fim(estado.venceu()) {
            sons.tocar(Som::Fim);
        }

        render.begin_frame(janela);
        arena.desenhar(render, &camera3d)?;
        inimigos.desenhar(render, partida, &camera3d)?;
        // A mira é do jogo: no menu e no fim ela apontaria para lugar nenhum.
        if diag.mira && *tela == Tela::Jogando {
            mira.ajustar(render, (largura, altura));
            mira.desenhar(render, mira_estado, (largura, altura))?;
        }
        if diag.hud {
            // Uma cena por tela: o menu não mostra vida e o fim não mostra onda
            // corrente — cada uma desenha o que faz sentido ali.
            let cena = match *tela {
                Tela::Menu => hud::menu(menu, hud::Tela::new(largura, altura)),
                Tela::Jogando => hud::cena(estado, hud_estado, hud::Tela::new(largura, altura)),
                Tela::Fim { venceu } => hud::fim(venceu, estado, hud::Tela::new(largura, altura)),
            };
            hud_gl.desenhar(render, cena, (largura, altura))?;
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_teclas_de_volume_sao_as_do_sdl() {
        // As duas constantes foram escritas à mão porque não são `const fn` no
        // `sdl2`. Este teste é o que impede o literal de apodrecer: se o SDL
        // mudar o valor, ele reprova aqui em vez de a tecla parar de funcionar
        // no aparelho sem nenhum sinal.
        assert_eq!(
            KEY_VOLUMEUP,
            sdl2::keyboard::Keycode::VOLUMEUP.into_i32() as u32
        );
        assert_eq!(
            KEY_VOLUMEDOWN,
            sdl2::keyboard::Keycode::VOLUMEDOWN.into_i32() as u32
        );
    }
}
