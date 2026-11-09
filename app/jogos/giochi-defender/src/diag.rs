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
    --quadro ARQUIVO.bmp  Grava o último quadro em BMP de 32 bits.
    --yaw GRAUS           Yaw inicial da câmera (0 = de frente para a arena).
    --pitch GRAUS         Inclinação inicial (-60 a 80; fora disso é preso).
    --distancia D         Distância inicial (5 a 15; fora disso é preso).
    --simular SEGUNDOS    Roda a simulação antes de desenhar (para capturar
                          cubos no ar sem esperar na tela).
    --semente N           Semente do sorteio de posições (padrão: fixa).
    --sem-mouse-relativo  Não prende o cursor.
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
                "--frames" => diag.frames = Some(inteiro(&mut argumentos, embutido, &nome)?),
                "--quadro" => diag.quadro = Some(texto(&mut argumentos, embutido, &nome)?),
                "--yaw" => diag.yaw_graus = Some(real(&mut argumentos, embutido, &nome)?),
                "--pitch" => diag.pitch_graus = Some(real(&mut argumentos, embutido, &nome)?),
                "--distancia" => diag.distancia = Some(real(&mut argumentos, embutido, &nome)?),
                "--simular" => diag.simular = Some(real(&mut argumentos, embutido, &nome)?),
                "--semente" => diag.semente = inteiro(&mut argumentos, embutido, &nome)?,
                outro => bail!("argumento desconhecido: '{outro}' (use --ajuda)"),
            }
        }

        Ok(diag)
    }
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
    fn desligar_o_mouse_relativo_e_uma_flag_so() {
        assert!(!parse(&["--sem-mouse-relativo"]).unwrap().mouse_relativo);
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
