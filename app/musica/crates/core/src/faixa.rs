//! Faixa: o que é uma música para este app.
//!
//! Tudo aqui é função pura sobre texto e bytes, sem tocar no disco: a leitura
//! de diretório fica em [`crate::biblioteca`], que recebe as entradas já lidas.
//! A divisão existe porque o disco do aparelho não está disponível no CI — e a
//! ordem em que as faixas aparecem é decisão de produto, não detalhe de
//! `readdir`.

use std::cmp::Ordering;

/// Formato reconhecido pela **extensão** do arquivo.
///
/// Extensão não prova que a biblioteca da imagem decodifica o arquivo: quem
/// responde isso é o `SDL_mixer` em tempo de execução. Por isso a lista é de
/// *aceitos* — o app oferece o arquivo ao mixer e, quando ele recusa, a razão
/// que ele deu aparece na tela. Declarar "toca MP3" sem medir seria a mesma
/// promessa vazia que já custou caro neste projeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    Wav,
    Ogg,
    Flac,
    Mp3,
    Mod,
    Xm,
    It,
    S3m,
}

impl Formato {
    pub const TODOS: [Formato; 8] = [
        Formato::Wav,
        Formato::Ogg,
        Formato::Flac,
        Formato::Mp3,
        Formato::Mod,
        Formato::Xm,
        Formato::It,
        Formato::S3m,
    ];

    /// Formato a partir da extensão, sem diferenciar maiúsculas de minúsculas.
    ///
    /// `MP3`, `Mp3` e `mp3` são o mesmo formato: num cartão SD copiado do
    /// Windows, mudar a caixa é a regra, não a exceção.
    pub fn de_extensao(ext: &str) -> Option<Formato> {
        match ext.trim().to_ascii_lowercase().as_str() {
            "wav" | "wave" => Some(Formato::Wav),
            "ogg" | "oga" => Some(Formato::Ogg),
            "flac" => Some(Formato::Flac),
            "mp3" => Some(Formato::Mp3),
            "mod" => Some(Formato::Mod),
            "xm" => Some(Formato::Xm),
            "it" => Some(Formato::It),
            "s3m" => Some(Formato::S3m),
            _ => None,
        }
    }

    /// A extensão canônica que este app reconhece.
    pub fn extensao(self) -> &'static str {
        match self {
            Formato::Wav => "wav",
            Formato::Ogg => "ogg",
            Formato::Flac => "flac",
            Formato::Mp3 => "mp3",
            Formato::Mod => "mod",
            Formato::Xm => "xm",
            Formato::It => "it",
            Formato::S3m => "s3m",
        }
    }

    /// Rótulo curto para a ficha da faixa.
    pub fn rotulo(self) -> &'static str {
        match self {
            Formato::Wav => "WAV",
            Formato::Ogg => "OGG",
            Formato::Flac => "FLAC",
            Formato::Mp3 => "MP3",
            Formato::Mod => "MOD",
            Formato::Xm => "XM",
            Formato::It => "IT",
            Formato::S3m => "S3M",
        }
    }
}

/// Lista de extensões aceitas, para a tela de pasta vazia.
///
/// Sai de [`Formato::TODOS`] em vez de um texto escrito no desenho: assim a tela
/// não pode prometer um formato que a descoberta não procura.
pub fn extensoes_aceitas() -> String {
    Formato::TODOS
        .iter()
        .map(|f| f.extensao())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Uma música da biblioteca.
#[derive(Clone, Debug, PartialEq)]
pub struct Faixa {
    /// Nome do arquivo, com extensão. É a identidade: renomear muda a ordem,
    /// como em qualquer gerenciador de arquivos.
    pub arquivo: String,
    /// Caminho completo, entregue ao mixer.
    pub caminho: String,
    /// Título legível, derivado do nome do arquivo.
    pub titulo: String,
    pub formato: Formato,
    /// Duração **medida**, quando o formato permite medir pelo cabeçalho.
    ///
    /// `None` não é "zero": é "não sei". A UI mostra `—`, nunca um `0:00` que
    /// parece informação. Hoje só o WAV declara o tamanho do fluxo no
    /// cabeçalho; MP3/OGG/FLAC exigiriam decodificar o arquivo.
    pub duracao: Option<f32>,
}

impl Faixa {
    /// Monta a faixa a partir do nome do arquivo e da pasta.
    pub fn nova(pasta: &str, arquivo: &str) -> Option<Faixa> {
        let formato = Formato::de_extensao(extensao_de(arquivo)?)?;
        Some(Faixa {
            arquivo: arquivo.to_string(),
            caminho: juntar(pasta, arquivo),
            titulo: titulo_de_arquivo(arquivo),
            formato,
            duracao: None,
        })
    }

    /// Duração para exibir: `3:42`, ou `—` quando não foi medida.
    pub fn duracao_legivel(&self) -> String {
        match self.duracao.and_then(crate::texto::formatar_tempo) {
            Some(texto) => texto,
            None => "—".to_string(),
        }
    }
}

/// Junta pasta e arquivo com uma barra só.
///
/// `String` e não `PathBuf` porque o caminho é dado de tela e de log antes de
/// ser dado de sistema — e `PathBuf` num tipo que viaja para a UI só acrescenta
/// uma conversão em cada ponta.
pub fn juntar(pasta: &str, arquivo: &str) -> String {
    let pasta = pasta.trim_end_matches('/');
    if pasta.is_empty() {
        format!("/{arquivo}")
    } else {
        format!("{pasta}/{arquivo}")
    }
}

/// Extensão do nome do arquivo, sem o ponto.
fn extensao_de(arquivo: &str) -> Option<&str> {
    let nome = arquivo.rsplit('/').next().unwrap_or(arquivo);
    let (base, ext) = nome.rsplit_once('.')?;
    // `.mp3` é nome escondido sem extensão, não um arquivo de áudio:
    // `rsplit_once` devolveria base vazia e o app listaria um arquivo "".
    if base.is_empty() || ext.is_empty() {
        return None;
    }
    Some(ext)
}

/// Título legível a partir do nome do arquivo.
///
/// A regra que importa é a do número de faixa: `01 - Song.mp3` é "Song", mas
/// `1979.mp3` é "1979". Um `trim_start_matches` de dígitos transformaria o
/// segundo em string vazia — e ano no nome do arquivo é comum em discografia.
pub fn titulo_de_arquivo(arquivo: &str) -> String {
    let nome = arquivo.rsplit('/').next().unwrap_or(arquivo);
    let sem_ext = match nome.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => base,
        _ => nome,
    };
    let limpo = sem_ext.replace('_', " ");
    let sem_numero = remover_numero_de_faixa(&limpo);
    let compacto = colapsar_espacos(sem_numero.trim());
    if compacto.is_empty() {
        // Vazio só acontece com nome degenerado; devolver o original é melhor
        // que devolver nada para a tela desenhar.
        colapsar_espacos(limpo.trim())
    } else {
        compacto
    }
}

/// Remove um número de faixa do começo, exigindo um separador.
///
/// Um a três dígitos seguidos de `-`, `_`, `)`, `]` ou de `.`/`:` **mais um
/// espaço**. A distinção do ponto não é preciosismo: `10.000 Maniacs - Candy`
/// começaria com "000" se qualquer ponto valesse.
fn remover_numero_de_faixa(nome: &str) -> &str {
    let digitos = nome.bytes().take_while(u8::is_ascii_digit).count();
    if digitos == 0 || digitos > 3 {
        return nome;
    }
    let resto = &nome[digitos..];
    let resto_sem_espaco = resto.trim_start_matches(' ');

    // `-` e `_` dispensam espaço depois: `01-Song` é tão comum quanto
    // `01 - Song`, e nos dois o resto já é o título.
    for separador in ['-', '_'] {
        if let Some(depois) = resto_sem_espaco.strip_prefix(separador) {
            let depois = depois.trim_start_matches(' ');
            if !depois.is_empty() && !depois.starts_with(|c: char| c.is_ascii_digit()) {
                return depois;
            }
        }
    }

    // `.`, `)` e `]` exigem espaço: sem ele, o que vem depois é continuação do
    // número (`10.000`), não título.
    for separador in ['.', ')', ']'] {
        if let Some(depois) = resto_sem_espaco.strip_prefix(separador) {
            if depois.starts_with(' ') {
                let depois = depois.trim_start_matches(' ');
                if !depois.is_empty() {
                    return depois;
                }
            }
        }
    }

    // `02 Song`: dígitos, um espaço, e o resto **não** começa com dígito.
    // `10 000 Maniacs` fica de fora justamente por isso.
    if resto.starts_with(' ')
        && !resto_sem_espaco.is_empty()
        && !resto_sem_espaco.starts_with(|c: char| c.is_ascii_digit())
        && digitos <= 2
    {
        return resto_sem_espaco;
    }

    nome
}

/// Junta espaços repetidos.
fn colapsar_espacos(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut espaco = false;
    for c in texto.chars() {
        if c.is_whitespace() {
            if !espaco {
                saida.push(' ');
            }
            espaco = true;
        } else {
            saida.push(c);
            espaco = false;
        }
    }
    saida
}

/// Ordem natural: `Faixa 2` antes de `Faixa 10`.
///
/// A ordem de bytes põe "10" antes de "2" e o usuário vê a playlist fora de
/// ordem num diretório de álbum — o defeito clássico de listagem de arquivos.
/// Números são comparados como números; o resto, ignorando a caixa.
pub fn ordem_natural(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();

    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let na = tirar_digitos(&mut ai);
                    let nb = tirar_digitos(&mut bi);
                    let ord = comparar_numeros(&na, &nb);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    ai.next();
                    bi.next();
                    let ord = ca.to_ascii_lowercase().cmp(&cb.to_ascii_lowercase());
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    // Desempate por bytes: sem ele `A` e `a` seriam `Equal`, e
                    // um comparador que devolve `Equal` para textos diferentes
                    // não é ordem total — a playlist mudaria de ordem entre
                    // duas leituras do mesmo diretório.
                    let ord = ca.cmp(&cb);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
        }
    }
}

fn tirar_digitos(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut saida = String::new();
    while let Some(c) = iter.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        saida.push(c);
        iter.next();
    }
    saida
}

fn comparar_numeros(a: &str, b: &str) -> Ordering {
    let sa = a.trim_start_matches('0');
    let sb = b.trim_start_matches('0');
    let va = if sa.is_empty() { "0" } else { sa };
    let vb = if sb.is_empty() { "0" } else { sb };

    // Mais dígitos significativos = maior. Comparar como texto só entre
    // números do mesmo tamanho evita "9" > "10".
    va.len()
        .cmp(&vb.len())
        .then_with(|| va.cmp(vb))
        // `01` e `1` valem o mesmo; quem escreveu menos zero vem antes, para a
        // ordem não depender de qual arquivo o `readdir` devolveu primeiro.
        .then_with(|| a.len().cmp(&b.len()))
}

/// Duração de um WAV a partir dos primeiros bytes do arquivo.
///
/// Só o WAV declara o tamanho do fluxo no cabeçalho: `data` dividido por
/// `byte_rate`. Os outros formatos exigem decodificar o arquivo, e inventar o
/// número seria pior que mostrar `—`.
///
/// Devolve `None` para: arquivo que não é RIFF/WAVE, cabeçalho truncado,
/// `byte_rate` zero, e `data` de tamanho zero ou `0xFFFFFFFF` (gravação em
/// streaming, em que o tamanho nunca foi corrigido).
pub fn duracao_wav(cabecalho: &[u8]) -> Option<f32> {
    if cabecalho.len() < 12 || &cabecalho[0..4] != b"RIFF" || &cabecalho[8..12] != b"WAVE" {
        return None;
    }

    let mut pos = 12usize;
    let mut byte_rate: u32 = 0;
    let mut data: Option<u32> = None;

    while pos + 8 <= cabecalho.len() {
        let id = &cabecalho[pos..pos + 4];
        let tamanho = u32::from_le_bytes([
            cabecalho[pos + 4],
            cabecalho[pos + 5],
            cabecalho[pos + 6],
            cabecalho[pos + 7],
        ]);
        let corpo = pos + 8;

        if id == b"fmt " {
            if tamanho < 16 || corpo + 16 > cabecalho.len() {
                return None;
            }
            let canais = u16::from_le_bytes([cabecalho[corpo + 2], cabecalho[corpo + 3]]) as u32;
            let taxa = u32::from_le_bytes([
                cabecalho[corpo + 4],
                cabecalho[corpo + 5],
                cabecalho[corpo + 6],
                cabecalho[corpo + 7],
            ]);
            let declarado = u32::from_le_bytes([
                cabecalho[corpo + 8],
                cabecalho[corpo + 9],
                cabecalho[corpo + 10],
                cabecalho[corpo + 11],
            ]);
            let bits = u16::from_le_bytes([cabecalho[corpo + 14], cabecalho[corpo + 15]]) as u32;
            byte_rate = if declarado > 0 {
                declarado
            } else {
                canais.saturating_mul(bits / 8).saturating_mul(taxa)
            };
        } else if id == b"data" {
            if tamanho > 0 && tamanho != u32::MAX {
                data = Some(tamanho);
            }
            break;
        }

        // Todo chunk é alinhado em 2 bytes; sem o padding o próximo id sai
        // deslocado de um e nenhum chunk seguinte é encontrado.
        let avanca = (tamanho as usize).saturating_add(tamanho as usize & 1);
        match corpo.checked_add(avanca) {
            Some(proximo) if proximo > pos => pos = proximo,
            _ => break,
        }
    }

    let data = data?;
    if byte_rate == 0 {
        return None;
    }
    let segundos = data as f32 / byte_rate as f32;
    if !segundos.is_finite() || segundos <= 0.0 {
        return None;
    }
    Some(segundos)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── formato ─────────────────────────────────────────────

    #[test]
    fn extensao_reconhece_maiuscula_e_minuscula() {
        assert_eq!(Formato::de_extensao("MP3"), Some(Formato::Mp3));
        assert_eq!(Formato::de_extensao("mp3"), Some(Formato::Mp3));
        assert_eq!(Formato::de_extensao(" Mp3 "), Some(Formato::Mp3));
    }

    #[test]
    fn extensao_desconhecida_nao_e_formato() {
        assert_eq!(Formato::de_extensao("txt"), None);
        assert_eq!(Formato::de_extensao(""), None);
        // `.opus` não está na lista porque o `SDL_mixer` 2.x não decodifica
        // Opus; aceitar e falhar só na hora de tocar seria promessa vazia.
        assert_eq!(Formato::de_extensao("opus"), None);
    }

    #[test]
    fn todas_as_extensoes_da_lista_sao_reconhecidas() {
        for formato in Formato::TODOS {
            assert_eq!(Formato::de_extensao(formato.extensao()), Some(formato));
            assert_eq!(formato.rotulo(), formato.extensao().to_ascii_uppercase());
        }
        // A tela de pasta vazia mostra esta lista; ela tem de citar as oito.
        let texto = extensoes_aceitas();
        for formato in Formato::TODOS {
            assert!(texto.contains(formato.extensao()), "falta {}", formato.extensao());
        }
    }

    #[test]
    fn arquivo_sem_extensao_nao_vira_faixa() {
        assert_eq!(Faixa::nova("/m", "sem_extensao"), None);
        // `.mp3` é nome escondido: base vazia não é extensão.
        assert_eq!(Faixa::nova("/m", ".mp3"), None);
        assert_eq!(
            Faixa::nova("/m", "musica.MP3").map(|f| f.formato),
            Some(Formato::Mp3)
        );
    }

    #[test]
    fn caminho_junta_com_uma_barra_so() {
        assert_eq!(juntar("/user/musica/", "a.mp3"), "/user/musica/a.mp3");
        assert_eq!(juntar("/user/musica", "a.mp3"), "/user/musica/a.mp3");
        assert_eq!(juntar("", "a.mp3"), "/a.mp3");
    }

    // ── título ──────────────────────────────────────────────

    #[test]
    fn titulo_tira_numero_de_faixa_e_extensao() {
        assert_eq!(titulo_de_arquivo("01 - Song.mp3"), "Song");
        assert_eq!(titulo_de_arquivo("01-Song.mp3"), "Song");
        assert_eq!(titulo_de_arquivo("02_Song.mp3"), "Song");
        assert_eq!(titulo_de_arquivo("03. Song.mp3"), "Song");
        assert_eq!(titulo_de_arquivo("02 Song.mp3"), "Song");
    }

    #[test]
    fn titulo_nao_come_ano_nem_decimal() {
        // A regra de "qualquer ponto separa" transformava este em "000".
        assert_eq!(
            titulo_de_arquivo("10.000 Maniacs - Candy.mp3"),
            "10.000 Maniacs - Candy"
        );
        assert_eq!(titulo_de_arquivo("1979.mp3"), "1979");
        assert_eq!(titulo_de_arquivo("1984 - Album.mp3"), "1984 - Album");
        // Quatro dígitos nunca são número de faixa.
        assert_eq!(titulo_de_arquivo("1000 - Song.mp3"), "1000 - Song");
    }

    #[test]
    fn titulo_troca_underline_por_espaco_e_colapsa() {
        assert_eq!(
            titulo_de_arquivo("Minha_Musica_Favorita.ogg"),
            "Minha Musica Favorita"
        );
        assert_eq!(titulo_de_arquivo("  espacos   demais  .wav"), "espacos demais");
        assert_eq!(titulo_de_arquivo("Música Ação.flac"), "Música Ação");
    }

    #[test]
    fn titulo_de_nome_degenerado_devolve_algo_desenhavel() {
        // Sem esta guarda a tela receberia "" e desenharia um cartão vazio.
        assert_eq!(titulo_de_arquivo("-----.mp3"), "-----");
        // "01 - .mp3" perde o número e sobra o separador; o que não pode é "".
        assert_eq!(titulo_de_arquivo("01 - .mp3"), "-");
    }

    #[test]
    fn titulo_ignora_pasta_do_caminho() {
        assert_eq!(titulo_de_arquivo("/tmp/pasta/07 - Faixa.mp3"), "Faixa");
    }

    // ── ordem natural ───────────────────────────────────────

    #[test]
    fn ordem_natural_poe_2_antes_de_10() {
        assert_eq!(ordem_natural("faixa 2", "faixa 10"), Ordering::Less);
        assert_eq!(ordem_natural("faixa 10", "faixa 2"), Ordering::Greater);
        assert_eq!(ordem_natural("2", "10"), Ordering::Less);
        assert_eq!(ordem_natural("a2", "a10"), Ordering::Less);
    }

    #[test]
    fn ordem_natural_trata_prefixo_e_sufixo() {
        assert_eq!(ordem_natural("a", "a1"), Ordering::Less);
        assert_eq!(ordem_natural("a1", "a"), Ordering::Greater);
        assert_eq!(ordem_natural("", "a"), Ordering::Less);
        assert_eq!(ordem_natural("a", "a"), Ordering::Equal);
    }

    #[test]
    fn ordem_natural_ignora_caixa_mas_desempata() {
        assert_eq!(ordem_natural("Abc", "abd"), Ordering::Less);
        // Textos diferentes nunca são `Equal`, senão a ordenação é instável.
        assert_ne!(ordem_natural("Abc", "abc"), Ordering::Equal);
        assert_eq!(ordem_natural("Abc", "abc"), Ordering::Less);
    }

    #[test]
    fn ordem_natural_aceita_acento_e_zero_a_esquerda() {
        assert_eq!(ordem_natural("Música 2", "Música 10"), Ordering::Less);
        assert_eq!(ordem_natural("01 - a", "2 - b"), Ordering::Less);
        // Mesmo valor numérico: quem tem menos zero vem antes, e o desempate
        // existe para a ordem não depender de qual arquivo o `readdir` leu
        // primeiro.
        assert_eq!(ordem_natural("1 - a", "01 - a"), Ordering::Less);
        assert_eq!(ordem_natural("01 - a", "1 - a"), Ordering::Greater);
    }

    #[test]
    fn ordem_natural_ordena_uma_lista_de_verdade() {
        let mut nomes = vec![
            "Faixa 10.mp3",
            "Faixa 2.mp3",
            "Faixa 1.mp3",
            "Faixa 20.mp3",
        ];
        nomes.sort_by(|a, b| ordem_natural(a, b));
        assert_eq!(
            nomes,
            vec!["Faixa 1.mp3", "Faixa 2.mp3", "Faixa 10.mp3", "Faixa 20.mp3"]
        );
    }

    // ── duração ─────────────────────────────────────────────

    /// Cabeçalho WAV de 44 bytes, 16 bits, com `data` de `bytes` bytes.
    fn wav(canais: u16, taxa: u32, bits: u16, bytes: u32) -> Vec<u8> {
        let block_align = canais * bits / 8;
        let byte_rate = taxa * block_align as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32.saturating_add(bytes)).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&canais.to_le_bytes());
        v.extend_from_slice(&taxa.to_le_bytes());
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&block_align.to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&bytes.to_le_bytes());
        v
    }

    #[test]
    fn duracao_de_wav_estereo_de_um_segundo() {
        // 44100 Hz * 2 canais * 2 bytes = 176400 bytes/s.
        let cabecalho = wav(2, 44_100, 16, 176_400);
        let d = duracao_wav(&cabecalho).expect("1 s");
        assert!((d - 1.0).abs() < 1e-6, "duração {d}");
    }

    #[test]
    fn duracao_de_wav_curto_e_fracionaria() {
        let cabecalho = wav(1, 8000, 8, 6000);
        let d = duracao_wav(&cabecalho).expect("0,75 s");
        assert!((d - 0.75).abs() < 1e-6, "duração {d}");
    }

    #[test]
    fn duracao_de_wav_pula_chunk_desconhecido() {
        // `LIST` de 3 bytes: ímpar, então o próximo chunk começa depois de um
        // byte de padding. Sem o padding o `fmt ` não é achado.
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"LIST");
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(b"abc");
        v.push(0);
        let mut resto = wav(2, 44_100, 16, 88_200);
        resto.drain(0..12);
        v.extend_from_slice(&resto);
        let d = duracao_wav(&v).expect("meio segundo");
        assert!((d - 0.5).abs() < 1e-6, "duração {d}");
    }

    #[test]
    fn duracao_de_wav_recusa_o_que_nao_mede() {
        assert_eq!(duracao_wav(b""), None);
        assert_eq!(duracao_wav(b"RIFF"), None);
        // Cabeçalho de MP3 disfarçado: não é RIFF.
        assert_eq!(
            duracao_wav(&[0xFF, 0xFB, 0x90, 0x64, 0, 0, 0, 0, 0, 0, 0, 0]),
            None
        );
        // `data` de tamanho zero: arquivo em gravação, tamanho não corrigido.
        assert_eq!(duracao_wav(&wav(2, 44_100, 16, 0)), None);
        // `data` 0xFFFFFFFF: o mesmo, escrito como "desconhecido".
        assert_eq!(duracao_wav(&wav(2, 44_100, 16, u32::MAX)), None);
        // Canais/bits zerados deixam `byte_rate` zero sem declaração — o
        // fallback não pode inventar um divisor.
        let mut ruim = wav(2, 44_100, 16, 1000);
        for b in &mut ruim[20..32] {
            *b = 0;
        }
        assert_eq!(duracao_wav(&ruim), None);
    }

    #[test]
    fn duracao_de_wav_usa_fallback_quando_byte_rate_e_zero() {
        // Alguns encoders gravam `byte_rate` 0 e só o resto certo; o fallback
        // por canais*bits*taxa é o que salva a medição.
        let mut cabecalho = wav(2, 44_100, 16, 176_400);
        for b in &mut cabecalho[28..32] {
            *b = 0;
        }
        let d = duracao_wav(&cabecalho).expect("1 s pelo fallback");
        assert!((d - 1.0).abs() < 1e-6, "duração {d}");
    }

    #[test]
    fn duracao_sem_medida_aparece_como_travessao() {
        let mut faixa = Faixa::nova("/m", "a.mp3").unwrap();
        assert_eq!(faixa.duracao_legivel(), "—");
        faixa.duracao = Some(222.4);
        assert_eq!(faixa.duracao_legivel(), "3:42");
    }
}
