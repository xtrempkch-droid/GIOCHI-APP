//! Diagnóstico: argumentos de linha de comando e captura de quadro em BMP.
//!
//! ## Por que isto existe num jogo
//!
//! "Abriu a janela" e "desenhou o asteroide" são afirmações diferentes, e num
//! aparelho de mão por SSH não há como olhar a tela. As duas opções `--frames` e
//! `--quadro` transformam a segunda afirmação em algo que se mede: o quadro sai
//! do buffer antes da troca e vira um arquivo que o PC analisa.
//!
//! É ferramenta, não funcionalidade de jogo: nada aqui aparece para o jogador.
//! Mesmo motivo e mesmo formato do `--quadro` dos exemplos da engine.
//!
//! ## BMP e não PNG
//!
//! O `SDL2_image` da imagem do aparelho é compilado sem `libpng` (medido: "PNG
//! images are not supported", §24 do AGENTS.md). Carregar um codificador de PNG
//! para uma captura que o PC converte em um segundo seria peso sem retorno. Um
//! BMP de 32 bits sem compressão são 54 bytes de cabeçalho e pronto.

use anyhow::{bail, Context, Result};
use std::path::Path;

/// Texto de `--ajuda`.
pub const AJUDA: &str = "\
GIOCHI DEFENDER — tower defense espacial 3D low-poly sobre a GIOCHI-ENGINE

USO:
    giochi-defender [opções]

OPÇÕES:
    --frames N            Roda N quadros e sai (não espera o ESC).
                          `--quadros N` é aceito como sinônimo.
    --quadro ARQUIVO.bmp  Grava o último quadro em BMP de 32 bits.
    --yaw GRAUS           Yaw inicial da câmera (0 = de frente para a arena).
    --pitch GRAUS         Inclinação inicial (-60 a 80; fora disso é preso).
    --distancia D         Distância inicial (5 a 15; fora disso é preso).
    --simular SEGUNDOS    Roda a simulação antes de desenhar (para capturar
                          cubos no ar sem esperar na tela).
    --semente N           Semente do sorteio de posições (padrão: fixa).
    --inimigos N          Enche o pool com N cubos antes do primeiro quadro.
    --benchmark N         Roda N quadros sem limite de 60 Hz e mede o tempo de
                          cada um (imprime fps, p50/p95/p99 e o pior quadro).
    --sem-mouse-relativo  Não prende o cursor.
    --sem-mira            Não desenha a mira (para isolar o que ela muda).
    --sem-hud             Não desenha a HUD (vida, onda, pontos, efeitos).
    --demo-hud            Liga os efeitos da HUD na abertura (+100, clarão de
                          dano e aviso de onda), para capturar sem jogar.
    --tremor              Arma o tremor de dano na abertura, para capturar a
                          câmera sacudida sem esperar um cubo acertar.
    --sem-som             Não carrega nem toca som (para isolar o custo dele).
    --volume N            Aplica N% no Master do ALSA pelo mesmo caminho das
                          teclas de volume, imprime e sai (0 a 100).
    --testar-som          Toca os cinco efeitos em sequência e sai. Roda no
                          aparelho pelo SSH, onde não há como ouvir o PC.
    --atirar              Mantém o gatilho apertado (o cooldown faz a cadência).
    --alvo N              Planta N cubos parados no eixo da câmera, para o tiro
                          do centro da tela ter alvo certo.
    --tela LxA            Tamanho pedido da janela (padrão 1280x720). No aparelho
                          o painel manda, então pedir outro tamanho é como se
                          testa a reconstrução da mira.
    -h, --ajuda           Esta mensagem.
";

/// O que a linha de comando pediu.
#[derive(Debug, Clone, PartialEq)]
pub struct Diag {
    /// Rodar só estes quadros e sair.
    pub frames: Option<u64>,
    /// Gravar o último quadro neste caminho.
    pub quadro: Option<String>,
    /// Prender o cursor no centro (câmera relativa). Padrão: ligado.
    pub mouse_relativo: bool,
    /// Yaw inicial em graus.
    pub yaw_graus: Option<f32>,
    /// Pitch inicial em graus.
    pub pitch_graus: Option<f32>,
    /// Distância inicial.
    pub distancia: Option<f32>,
    /// Segundos de simulação antes do primeiro quadro.
    pub simular: Option<f32>,
    /// Semente do sorteio de posições.
    pub semente: u64,
    /// Quantos cubos plantar antes do primeiro quadro.
    pub inimigos: Option<u32>,
    /// Quadros a medir sem o limite de 60 Hz.
    pub benchmark: Option<u64>,
    /// Desenhar a mira.
    pub mira: bool,
    /// Desenhar a HUD.
    pub hud: bool,
    /// Ligar os efeitos da HUD na abertura, sem jogar.
    pub demo_hud: bool,
    /// Manter o gatilho apertado.
    pub atirar: bool,
    /// Armar o tremor de dano na abertura.
    pub tremor: bool,
    /// Carregar e tocar som.
    pub som: bool,
    /// Tocar os cinco efeitos e sair.
    pub testar_som: bool,
    /// Volume a aplicar no sistema e sair.
    pub volume: Option<u8>,
    /// Quantos cubos plantar no eixo da câmera.
    pub alvo: Option<u32>,
    /// Tamanho pedido da janela.
    pub tela: Option<(u32, u32)>,
    /// Pediu a ajuda.
    pub ajuda: bool,
}

/// Semente padrão do sorteio.
///
/// Fixa de propósito: duas execuções do mesmo comando têm de desenhar a mesma
/// cena, senão comparar capturas mediria o sorteio em vez do código.
pub const SEMENTE_PADRAO: u64 = 0x0DEF_E17E_u64;

impl Default for Diag {
    /// Padrão com o mouse relativo **ligado** — `derive` deixaria desligado, e o
    /// padrão do jogo é prender o cursor.
    fn default() -> Self {
        Self {
            frames: None,
            quadro: None,
            mouse_relativo: true,
            yaw_graus: None,
            pitch_graus: None,
            distancia: None,
            simular: None,
            semente: SEMENTE_PADRAO,
            inimigos: None,
            benchmark: None,
            mira: true,
            hud: true,
            tremor: false,
            som: true,
            testar_som: false,
            volume: None,
            demo_hud: false,
            atirar: false,
            alvo: None,
            tela: None,
            ajuda: false,
        }
    }
}

impl Diag {
    /// Interpreta os argumentos.
    ///
    /// Argumento desconhecido é **erro**, não aviso: um `--frames=3oo` engolido
    /// em silêncio vira execução sem limite, e num CI isso é um job que expira em
    /// vez de uma mensagem.
    pub fn parse(argumentos: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut diag = Self::default();
        let mut argumentos = argumentos.into_iter();

        while let Some(argumento) = argumentos.next() {
            let (nome, embutido) = match argumento.split_once('=') {
                Some((nome, valor)) => (nome.to_string(), Some(valor.to_string())),
                None => (argumento.clone(), None),
            };

            match nome.as_str() {
                "-h" | "--ajuda" | "--help" => diag.ajuda = true,
                "--sem-mouse-relativo" => diag.mouse_relativo = false,
                "--sem-mira" => diag.mira = false,
                "--sem-hud" => diag.hud = false,
                "--demo-hud" => diag.demo_hud = true,
                "--tremor" => diag.tremor = true,
                "--sem-som" => diag.som = false,
                "--volume" => {
                    let valor = inteiro(&mut argumentos, embutido, &nome)?;
                    if !(0..=100).contains(&valor) {
                        anyhow::bail!("--volume espera 0 a 100, veio {valor}");
                    }
                    diag.volume = Some(valor as u8);
                    diag.frames.get_or_insert(1);
                }
                "--testar-som" => {
                    diag.testar_som = true;
                    // Quem pede para testar o som não quer uma partida junto.
                    diag.frames.get_or_insert(1);
                }
                "--atirar" => diag.atirar = true,
                "--alvo" => diag.alvo = Some(inteiro(&mut argumentos, embutido, &nome)? as u32),
                "--tela" => {
                    let valor = texto(&mut argumentos, embutido, &nome)?;
                    diag.tela = Some(parse_tela(&valor).ok_or_else(|| {
                        anyhow::anyhow!("--tela espera LARGURAxALTURA, veio '{valor}'")
                    })?);
                }
                // `--quadros` é o nome que o `aparelho.sh verificar-jogo` usa para
                // todo jogo (`cobrinha`, `volley`); `--frames` é o dos exemplos da
                // engine. Aceitar os dois custa uma linha e evita um "argumento
                // desconhecido" que manda procurar defeito no jogo.
                "--frames" | "--quadros" => {
                    diag.frames = Some(inteiro(&mut argumentos, embutido, &nome)?);
                }
                "--quadro" => diag.quadro = Some(texto(&mut argumentos, embutido, &nome)?),
                "--yaw" => diag.yaw_graus = Some(real(&mut argumentos, embutido, &nome)?),
                "--pitch" => diag.pitch_graus = Some(real(&mut argumentos, embutido, &nome)?),
                "--distancia" => diag.distancia = Some(real(&mut argumentos, embutido, &nome)?),
                "--simular" => diag.simular = Some(real(&mut argumentos, embutido, &nome)?),
                "--semente" => diag.semente = inteiro(&mut argumentos, embutido, &nome)?,
                "--inimigos" => {
                    diag.inimigos = Some(inteiro(&mut argumentos, embutido, &nome)? as u32);
                }
                "--benchmark" => {
                    diag.benchmark = Some(inteiro(&mut argumentos, embutido, &nome)?);
                }
                outro => bail!("argumento desconhecido: '{outro}' (use --ajuda)"),
            }
        }

        Ok(diag)
    }
}

/// Interpreta `1280x720` (ou `1280X720`).
///
/// Devolve `None` para qualquer coisa que não seja dois inteiros positivos: uma
/// janela de 0 px não abre, e o erro tem de nomear o formato.
pub fn parse_tela(valor: &str) -> Option<(u32, u32)> {
    let (largura, altura) = valor.split_once(['x', 'X'])?;
    let largura: u32 = largura.trim().parse().ok()?;
    let altura: u32 = altura.trim().parse().ok()?;
    if largura == 0 || altura == 0 || largura > 8192 || altura > 8192 {
        return None;
    }
    Some((largura, altura))
}

fn texto(
    argumentos: &mut impl Iterator<Item = String>,
    embutido: Option<String>,
    nome: &str,
) -> Result<String> {
    embutido
        .or_else(|| argumentos.next())
        .with_context(|| format!("{nome} precisa de um valor (use --ajuda)"))
}

fn inteiro(
    argumentos: &mut impl Iterator<Item = String>,
    embutido: Option<String>,
    nome: &str,
) -> Result<u64> {
    let valor = texto(argumentos, embutido, nome)?;
    valor
        .parse()
        .with_context(|| format!("{nome} espera um inteiro, veio '{valor}'"))
}

fn real(
    argumentos: &mut impl Iterator<Item = String>,
    embutido: Option<String>,
    nome: &str,
) -> Result<f32> {
    let valor = texto(argumentos, embutido, nome)?;
    let numero: f32 = valor
        .parse()
        .with_context(|| format!("{nome} espera um número, veio '{valor}'"))?;
    if !numero.is_finite() {
        bail!("{nome} espera um número finito, veio '{valor}'");
    }
    Ok(numero)
}

/// Distâncias, a partir da câmera, onde o `--alvo` planta os cubos.
///
/// ## A regra que a primeira versão não tinha
///
/// Plantar em 7, 10 e 13 unidades da câmera parecia razoável e estava errado: a
/// câmera fica a ~9 do asteroide, então o cubo de 10 caía a **1 unidade do
/// centro** — dentro da esfera de colisão de 2,5. Ele era despawnado como dano no
/// primeiro passo e o terceiro tiro ia para o alvo seguinte. O log mostrou dois
/// abates em vez de três, e o culpado parecia a arma.
///
/// Agora os alvos começam **depois** do asteroide, com folga explícita:
/// `distância da câmera + raio de colisão + meia aresta do cubo + 1`.
pub fn distancias_de_alvo(distancia_camera: f32, quantos: u32) -> Vec<f32> {
    let inicio = distancia_camera
        + crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE
        + crate::gameplay::enemy::LADO_PADRAO * 0.5
        + 1.0;
    (0..quantos)
        .map(|indice| inicio + indice as f32 * ESPACO_ENTRE_ALVOS)
        .collect()
}

/// Espaço entre dois alvos, em unidades de mundo.
///
/// 2,5 é folgado o bastante para os cubos não se sobreporem na tela (aresta 0,8)
/// e próximo o bastante para os três caberem no campo de visão de 60°.
pub const ESPACO_ENTRE_ALVOS: f32 = 2.5;

/// Escreve pixels RGBA8 como BMP de 32 bits, de baixo para cima.
///
/// O GL entrega as linhas de baixo para cima, que é a ordem do BMP — não há
/// inversão. Canais, sim: RGBA vira BGRA.
pub fn escrever_bmp(caminho: &Path, largura: u32, altura: u32, rgba: &[u8]) -> Result<()> {
    let esperado = (largura as usize) * (altura as usize) * 4;
    if rgba.len() != esperado {
        bail!(
            "captura com {} bytes; esperado {esperado} ({largura}x{altura} RGBA)",
            rgba.len()
        );
    }

    let mut pixels = Vec::with_capacity(esperado);
    for pixel in rgba.chunks_exact(4) {
        pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }

    const CABECALHO: usize = 54;
    let mut bmp = Vec::with_capacity(CABECALHO + pixels.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((CABECALHO + pixels.len()) as u32).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(CABECALHO as u32).to_le_bytes());
    // BITMAPINFOHEADER: 40 bytes, 32 bpp, BI_RGB, altura positiva = de baixo para cima.
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(largura as i32).to_le_bytes());
    bmp.extend_from_slice(&(altura as i32).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    bmp.extend_from_slice(&2835i32.to_le_bytes());
    bmp.extend_from_slice(&2835i32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&pixels);

    std::fs::write(caminho, &bmp)
        .with_context(|| format!("não consegui escrever '{}'", caminho.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(argumentos: &[&str]) -> Result<Diag> {
        Diag::parse(argumentos.iter().map(|s| s.to_string()))
    }

    #[test]
    fn sem_argumento_nada_e_pedido_e_o_mouse_e_relativo() {
        let diag = parse(&[]).expect("vazio é válido");
        assert_eq!(diag.frames, None);
        assert_eq!(diag.quadro, None);
        assert!(diag.mouse_relativo);
        assert!(!diag.ajuda);
    }

    #[test]
    fn as_duas_formas_de_passar_valor_funcionam() {
        assert_eq!(parse(&["--frames", "3"]).unwrap().frames, Some(3));
        assert_eq!(parse(&["--frames=3"]).unwrap().frames, Some(3));
        assert_eq!(
            parse(&["--quadro=/tmp/a.bmp"]).unwrap().quadro.as_deref(),
            Some("/tmp/a.bmp")
        );
    }

    #[test]
    fn argumento_desconhecido_reprova() {
        // O silêncio aqui já custou caro na engine: `--frames=3oo` virava
        // "sem limite" e o processo não terminava.
        let erro = parse(&["--frame", "3"]).expect_err("devia reprovar");
        assert!(format!("{erro:#}").contains("--frame"));
    }

    #[test]
    fn valor_que_nao_e_numero_reprova() {
        assert!(parse(&["--frames", "3oo"]).is_err());
        assert!(parse(&["--yaw", "leste"]).is_err());
    }

    #[test]
    fn flag_que_precisa_de_valor_sem_valor_reprova() {
        assert!(parse(&["--quadro"]).is_err());
        assert!(parse(&["--frames"]).is_err());
    }

    #[test]
    fn o_preset_de_camera_e_lido_em_graus() {
        let diag = parse(&["--yaw", "90", "--pitch=-10", "--distancia", "12.5"]).unwrap();
        assert_eq!(diag.yaw_graus, Some(90.0));
        assert_eq!(diag.pitch_graus, Some(-10.0));
        assert_eq!(diag.distancia, Some(12.5));
    }

    #[test]
    fn numero_nao_finito_reprova() {
        assert!(parse(&["--yaw", "inf"]).is_err());
        assert!(parse(&["--distancia", "NaN"]).is_err());
    }

    #[test]
    fn a_simulacao_e_a_semente_sao_lidas() {
        let diag = parse(&["--simular", "6.5", "--semente", "99"]).unwrap();
        assert_eq!(diag.simular, Some(6.5));
        assert_eq!(diag.semente, 99);
    }

    #[test]
    fn a_semente_padrao_e_fixa() {
        // Fixa para a captura ser reproduzível; ver a doc de `SEMENTE_PADRAO`.
        assert_eq!(parse(&[]).unwrap().semente, SEMENTE_PADRAO);
        assert_eq!(parse(&[]).unwrap().semente, parse(&[]).unwrap().semente);
    }

    #[test]
    fn simulacao_negativa_nao_reprova_mas_e_lida() {
        // O sinal é tratado na simulação (dt <= 0 não avança); o parser só
        // garante que é um número finito.
        assert_eq!(parse(&["--simular=-1"]).unwrap().simular, Some(-1.0));
        assert!(parse(&["--simular", "muito"]).is_err());
    }

    #[test]
    fn quadros_e_frames_sao_o_mesmo_flag() {
        // O `aparelho.sh verificar-jogo` passa `--quadros`; os exemplos da engine
        // usam `--frames`. Os dois têm de valer, senão o jogo roda no PC e falha
        // no aparelho com "argumento desconhecido".
        assert_eq!(parse(&["--quadros", "3"]).unwrap().frames, Some(3));
        assert_eq!(parse(&["--frames=3"]).unwrap().frames, Some(3));
        assert_eq!(parse(&["--quadros=3"]).unwrap().frames, Some(3));
    }

    #[test]
    fn os_dois_flags_de_performance_sao_lidos() {
        let diag = parse(&["--inimigos", "15", "--benchmark=300"]).unwrap();
        assert_eq!(diag.inimigos, Some(15));
        assert_eq!(diag.benchmark, Some(300));
    }

    #[test]
    fn benchmark_zero_e_aceito_e_nao_roda_nada() {
        // Zero quadros é resposta legítima (mede o custo de abrir o dispositivo) e
        // não pode ser confundido com "sem limite".
        assert_eq!(parse(&["--benchmark", "0"]).unwrap().benchmark, Some(0));
        assert_eq!(parse(&[]).unwrap().benchmark, None);
    }

    #[test]
    fn a_tela_e_lida_nas_duas_formas() {
        assert_eq!(parse_tela("800x600"), Some((800, 600)));
        assert_eq!(parse_tela("1280X720"), Some((1280, 720)));
        assert_eq!(
            parse(&["--tela", "640x480"]).unwrap().tela,
            Some((640, 480))
        );
    }

    #[test]
    fn tela_invalida_reprova() {
        // Uma janela de 0 px não abre, e 9000 px não cabe em painel nenhum:
        // aceitar os dois faria o erro aparecer na criação da janela, longe do
        // argumento que o causou.
        for ruim in [
            "800", "800x", "x600", "0x600", "800x0", "9000x600", "axb", "",
        ] {
            assert!(parse_tela(ruim).is_none(), "'{ruim}' devia ser recusado");
        }
        assert!(parse(&["--tela", "800"]).is_err());
    }

    #[test]
    fn os_flags_de_tiro_sao_lidos() {
        let diag = parse(&["--atirar", "--alvo", "3"]).unwrap();
        assert!(diag.atirar);
        assert_eq!(diag.alvo, Some(3));
        assert!(!parse(&[]).unwrap().atirar, "o padrão não atira sozinho");
        assert_eq!(parse(&[]).unwrap().alvo, None);
    }

    #[test]
    fn desligar_a_mira_e_uma_flag_so() {
        assert!(parse(&[]).unwrap().mira);
        assert!(!parse(&["--sem-mira"]).unwrap().mira);
    }

    #[test]
    fn a_hud_liga_por_padrao_e_desliga_com_uma_flag() {
        assert!(parse(&[]).unwrap().hud);
        assert!(!parse(&["--sem-hud"]).unwrap().hud);
        // São camadas diferentes: tirar a mira não tira a HUD, e vice-versa.
        assert!(parse(&["--sem-mira"]).unwrap().hud);
        assert!(parse(&["--sem-hud"]).unwrap().mira);
    }

    #[test]
    fn o_volume_e_lido_e_limitado() {
        assert_eq!(parse(&[]).unwrap().volume, None);
        assert_eq!(parse(&["--volume", "40"]).unwrap().volume, Some(40));
        assert_eq!(parse(&["--volume", "0"]).unwrap().volume, Some(0));
        assert_eq!(parse(&["--volume", "100"]).unwrap().volume, Some(100));
        assert_eq!(parse(&["--volume=55"]).unwrap().volume, Some(55));
        // Fora da faixa é erro, não prisão silenciosa: quem digita 150 quer
        // outra coisa, e um aviso é mais útil que um 100 calado.
        assert!(parse(&["--volume", "101"]).is_err());
        assert!(parse(&["--volume", "-1"]).is_err());
    }

    #[test]
    fn o_volume_sai_sozinho_quando_pedido() {
        assert_eq!(parse(&["--volume", "40"]).unwrap().frames, Some(1));
    }

    #[test]
    fn o_tremor_e_opt_in() {
        assert!(!parse(&[]).unwrap().tremor);
        assert!(parse(&["--tremor"]).unwrap().tremor);
    }

    #[test]
    fn o_som_liga_por_padrao_e_desliga_com_uma_flag() {
        assert!(parse(&[]).unwrap().som);
        assert!(!parse(&["--sem-som"]).unwrap().som);
    }

    #[test]
    fn testar_o_som_nao_abre_uma_partida() {
        let diag = parse(&["--testar-som"]).unwrap();
        assert!(diag.testar_som);
        assert_eq!(diag.frames, Some(1), "o teste de som tem de sair sozinho");
        // Pedir quadros explicitamente não é atropelado pela flag.
        assert_eq!(
            parse(&["--testar-som", "--frames", "5"]).unwrap().frames,
            Some(5)
        );
    }

    #[test]
    fn a_demo_da_hud_e_opt_in() {
        assert!(!parse(&[]).unwrap().demo_hud);
        assert!(parse(&["--demo-hud"]).unwrap().demo_hud);
    }

    #[test]
    fn desligar_o_mouse_relativo_e_uma_flag_so() {
        assert!(!parse(&["--sem-mouse-relativo"]).unwrap().mouse_relativo);
    }

    #[test]
    fn os_alvos_ficam_fora_da_esfera_do_asteroide() {
        // A regra que a primeira versão violou: um alvo dentro da esfera de
        // colisão é despawnado como dano antes de poder ser atingido.
        for distancia_camera in [5.0, 9.0, 15.0] {
            let distancias = distancias_de_alvo(distancia_camera, 5);
            assert_eq!(distancias.len(), 5);
            for d in &distancias {
                let do_centro = (distancia_camera - d).abs();
                assert!(
                    do_centro
                        > crate::gameplay::partida::RAIO_COLISAO_ASTEROIDE
                            + crate::gameplay::enemy::LADO_PADRAO * 0.5,
                    "alvo a {d} da câmera fica a {do_centro} do centro — dentro da \
                     esfera de colisão"
                );
                assert!(*d > distancia_camera, "o alvo tem de estar à frente");
            }
        }
    }

    #[test]
    fn os_alvos_ficam_em_ordem_e_espacados() {
        let distancias = distancias_de_alvo(9.0, 3);
        for par in distancias.windows(2) {
            let passo = par[1] - par[0];
            assert!(
                (passo - ESPACO_ENTRE_ALVOS).abs() < 1e-4,
                "espaçamento {passo}"
            );
            assert!(
                passo > crate::gameplay::enemy::LADO_PADRAO,
                "cubos sobrepostos na tela"
            );
        }
    }

    #[test]
    fn o_bmp_tem_o_cabecalho_e_o_tamanho_certos() {
        let destino = std::env::temp_dir().join("giochi-defender-bmp-teste.bmp");
        let rgba = vec![
            255, 0, 0, 255, // vermelho
            0, 255, 0, 255, // verde
            0, 0, 255, 255, // azul
            255, 255, 255, 255,
        ];
        escrever_bmp(&destino, 2, 2, &rgba).expect("escrever");
        let bytes = std::fs::read(&destino).expect("ler");
        let _ = std::fs::remove_file(&destino);

        assert_eq!(&bytes[0..2], b"BM");
        assert_eq!(bytes.len(), 54 + rgba.len());
        assert_eq!(
            u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]),
            2,
            "largura no cabeçalho"
        );
        assert_eq!(
            u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]),
            2,
            "altura no cabeçalho"
        );
        // Primeiro pixel do arquivo é o canto inferior esquerdo, em BGRA.
        assert_eq!(&bytes[54..58], &[0, 0, 255, 255]);
    }

    #[test]
    fn captura_com_tamanho_errado_nao_vira_arquivo() {
        let destino = std::env::temp_dir().join("giochi-defender-bmp-errado.bmp");
        let erro = escrever_bmp(&destino, 4, 4, &[0u8; 8]).expect_err("devia reprovar");
        assert!(format!("{erro:#}").contains("esperado"));
        assert!(!destino.exists(), "arquivo parcial não pode ficar no disco");
    }
}
