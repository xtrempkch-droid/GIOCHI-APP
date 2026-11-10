//! Sons gerados em código — nenhum `.wav` no pacote.
//!
//! ## Por que sintetizar em vez de empacotar arquivos
//!
//! É a mesma razão das malhas procedurais: um `.wav` é um binário que precisa
//! viajar no pacote, combinar com licença e existir na imagem do aparelho. Uma
//! função de 20 linhas gera o som, é determinística e cabe em teste.
//!
//! ## Por que `from_raw_buffer` e não `load_sound`
//!
//! `AudioSystem::load_sound` só aceita **caminho de arquivo**: ele chama
//! `Chunk::from_file`. Para tocar bytes em memória, as saídas eram gravar um
//! temporário em disco ou falar com o `sdl2::mixer` direto. A segunda não abre
//! segundo contexto — a engine **leaka** o `Sdl2MixerContext` de propósito
//! ("keeps SDL_mixer initialised for the process"), então o dispositivo já está
//! aberto e configurado quando este módulo carrega os sons.
//!
//! **É uma lacuna da engine, e vale reportar:** `AudioSystem` não tem
//! `load_sound_from_bytes`. O jogo já depende do `sdl2` direto para o mouse
//! relativo pelo mesmo motivo (ver `Cargo.toml`).
//!
//! ## O formato não é escolha deste módulo
//!
//! `Mix_QuickLoad_RAW` exige os dados **no formato que o mixer abriu**: a engine
//! pede `AUDIO_S16LSB`, 2 canais, a [`SAMPLE_RATE`] Hz. Não há conversão depois.
//! Por isso o gerador produz estéreo entrelaçado e o teste
//! `o_formato_bate_com_o_que_o_mixer_abriu` amarra as três constantes.

pub mod volume;

use giochi_engine::audio::sound::SAMPLE_RATE;
use sdl2::mixer::Chunk;

use crate::utils::rng::Sorteio;

/// Canais que o mixer da engine abre (`open_audio(..., 2, ...)`).
pub const CANAIS: usize = 2;
/// Taxa de amostragem do mixer, em Hz.
pub const AMOSTRAGEM: u32 = SAMPLE_RATE as u32;
/// Pico de cada som, em fração da escala cheia.
///
/// 0,35 e não 1,0: tiro e abate podem soar no mesmo quadro, e dois sons em escala
/// cheia somados estouram. Com esta folga, dez simultâneos ainda cabem.
pub const PICO: f32 = 0.35;

/// Identidade de um som do jogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Som {
    /// Disparo da arma.
    Tiro,
    /// Cubo destruído.
    Abate,
    /// Asteroide atingido.
    Dano,
    /// Onda nova.
    Onda,
    /// Fim de partida.
    Fim,
}

impl Som {
    /// Todos os sons, na ordem em que o `--testar-som` os toca.
    pub const TODOS: [Som; 5] = [Som::Tiro, Som::Abate, Som::Dano, Som::Onda, Som::Fim];

    /// Nome curto, para log e diagnóstico.
    pub fn nome(self) -> &'static str {
        match self {
            Som::Tiro => "tiro",
            Som::Abate => "abate",
            Som::Dano => "dano",
            Som::Onda => "onda",
            Som::Fim => "fim",
        }
    }

    /// Duração em segundos, que é o que a síntese usa.
    pub fn duracao(self) -> f32 {
        match self {
            Som::Tiro => 0.09,
            Som::Abate => 0.16,
            Som::Dano => 0.28,
            Som::Onda => 0.42,
            Som::Fim => 0.75,
        }
    }
}

/// Forma de onda de um trecho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Forma {
    /// Senoide: doce, boa para tom musical.
    Seno,
    /// Onda quadrada: áspera, boa para impacto e para "chiado" de laser.
    Quadrada,
    /// Ruído branco determinístico.
    Ruido,
}

/// Uma nota: forma, frequência inicial e final, e o envelope.
struct Nota {
    forma: Forma,
    inicio: f32,
    fim: f32,
    atraso: f32,
    duracao: f32,
    ataque: f32,
    peso: f32,
}

impl Nota {
    fn nova(forma: Forma, inicio: f32, fim: f32, atraso: f32, duracao: f32) -> Self {
        Self {
            forma,
            inicio,
            fim,
            atraso,
            duracao,
            ataque: 0.004,
            peso: 1.0,
        }
    }

    fn com_peso(mut self, peso: f32) -> Self {
        self.peso = peso;
        self
    }

    fn com_ataque(mut self, ataque: f32) -> Self {
        self.ataque = ataque;
        self
    }

    /// Soma esta nota em `saida` (mono, 1 amostra por quadro).
    fn somar(&self, saida: &mut [f32], semente: &mut Sorteio) {
        let total = saida.len();
        let primeiro = (self.atraso * AMOSTRAGEM as f32).round() as usize;
        let quantas = ((self.duracao * AMOSTRAGEM as f32).round() as usize)
            .min(total.saturating_sub(primeiro));
        let mut fase = 0.0f32;
        for i in 0..quantas {
            let t = i as f32 / AMOSTRAGEM as f32;
            let progresso = if self.duracao > 0.0 {
                t / self.duracao
            } else {
                1.0
            };
            // Varredura linear de frequência: é o que dá o "desce" do laser.
            let freq = self.inicio + (self.fim - self.inicio) * progresso;
            let amostra = match self.forma {
                Forma::Seno => (fase * std::f32::consts::TAU).sin(),
                Forma::Quadrada => {
                    if fase.fract() < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Forma::Ruido => semente.faixa(-1.0, 1.0),
            };
            fase += freq / AMOSTRAGEM as f32;
            if fase > 1.0 {
                fase -= fase.floor();
            }
            // Ataque linear curto (sem clique no início) e queda quadrática: a
            // cauda reta soa como corte, e o corte é o clique no fim.
            let ataque = if self.ataque > 0.0 {
                (t / self.ataque).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let queda = (1.0 - progresso).max(0.0).powi(2);
            saida[primeiro + i] += amostra * ataque * queda * self.peso;
        }
    }
}

/// Sintetiza `duracao` segundos com `notas` somadas.
fn sintetizar(duracao: f32, notas: &[Nota], semente: u64) -> Vec<f32> {
    let total = (duracao * AMOSTRAGEM as f32).round().max(1.0) as usize;
    let mut saida = vec![0.0f32; total];
    let mut sorteio = Sorteio::new(semente);
    for nota in notas {
        nota.somar(&mut saida, &mut sorteio);
    }
    normalizar(&mut saida);
    // Rampa final de 4 ms: sem ela o último valor é um degrau até zero, e o
    // alto-falante reproduz esse degrau como um estalo depois do som.
    let rampa = (0.004 * AMOSTRAGEM as f32) as usize;
    let n = saida.len();
    for i in 0..rampa.min(n) {
        saida[n - 1 - i] *= i as f32 / rampa as f32;
    }
    saida
}

/// Escala a faixa para [`PICO`], com um teto de proteção.
fn normalizar(amostras: &mut [f32]) {
    let maior = amostras
        .iter()
        .fold(0.0f32, |acc, valor| acc.max(valor.abs()));
    if !maior.is_finite() || maior <= f32::EPSILON {
        return;
    }
    let ganho = PICO / maior;
    for amostra in amostras.iter_mut() {
        *amostra *= ganho;
    }
}

/// Converte mono em estéreo entrelaçado, na escala de 16 bits.
///
/// O mesmo sinal nos dois canais: sintetizar panorâmica não vale o código, e um
/// som deslocado para um lado num aparelho de mão soa como alto-falante quebrado.
pub fn para_estereo_i16(mono: &[f32]) -> Vec<i16> {
    let mut saida = Vec::with_capacity(mono.len() * CANAIS);
    for valor in mono {
        // `i16::MAX` e não `32768`: o valor positivo da escala cheia de i16 é
        // 32767, e arredondar para 32768 estoura para o negativo.
        let amostra = (valor.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        for _ in 0..CANAIS {
            saida.push(amostra);
        }
    }
    saida
}

/// As amostras mono de um som, antes da conversão para estéreo.
pub fn amostras(som: Som) -> Vec<f32> {
    let (notas, semente): (Vec<Nota>, u64) = match som {
        // Laser curto: quadrada descendo rápido com um sopro de ruído em cima.
        Som::Tiro => (
            vec![
                Nota::nova(Forma::Quadrada, 900.0, 260.0, 0.0, 0.085).com_peso(0.85),
                Nota::nova(Forma::Ruido, 0.0, 0.0, 0.0, 0.035).com_peso(0.35),
            ],
            0x7110,
        ),
        // Dois blips subindo: o "acertou" curto e limpo.
        Som::Abate => (
            vec![
                Nota::nova(Forma::Seno, 660.0, 660.0, 0.0, 0.07),
                Nota::nova(Forma::Seno, 990.0, 990.0, 0.065, 0.09),
            ],
            0xABA7,
        ),
        // Impacto grave com ruído: quadrada baixa + estalo.
        Som::Dano => (
            vec![
                Nota::nova(Forma::Quadrada, 150.0, 70.0, 0.0, 0.26).com_peso(0.9),
                Nota::nova(Forma::Ruido, 0.0, 0.0, 0.0, 0.12)
                    .com_peso(0.55)
                    .com_ataque(0.001),
            ],
            0xDA10,
        ),
        // Três notas ascendentes: fanfarra de onda nova.
        Som::Onda => (
            vec![
                Nota::nova(Forma::Seno, 440.0, 440.0, 0.0, 0.12),
                Nota::nova(Forma::Seno, 554.0, 554.0, 0.12, 0.12),
                Nota::nova(Forma::Seno, 660.0, 660.0, 0.24, 0.17),
            ],
            0x0ADA,
        ),
        // Três notas descendentes, mais longas: o fim.
        Som::Fim => (
            vec![
                Nota::nova(Forma::Seno, 440.0, 440.0, 0.0, 0.22),
                Nota::nova(Forma::Seno, 349.0, 349.0, 0.22, 0.22),
                Nota::nova(Forma::Seno, 262.0, 262.0, 0.44, 0.30).com_peso(0.9),
                // Um sopro de ruído grave embaixo, para não soar só "três bipes".
                Nota::nova(Forma::Ruido, 0.0, 0.0, 0.44, 0.28)
                    .com_peso(0.12)
                    .com_ataque(0.02),
            ],
            0xF1A0,
        ),
    };
    sintetizar(som.duracao(), &notas, semente)
}

/// Amostras estéreo prontas para o mixer.
pub fn amostras_estereo(som: Som) -> Vec<i16> {
    para_estereo_i16(&amostras(som))
}

/// Toca os sons do jogo.
///
/// ## Por que os `Chunk` vivem aqui e não na `AudioSystem`
///
/// Porque a engine não aceita bytes. Os quatro `Chunk` são carregados uma vez na
/// abertura e ficam vivos até o processo terminar; `Mix_QuickLoad_RAW` exige
/// exatamente isso (o buffer tem de sobreviver ao chunk), e por isso o
/// `Box<[i16]>` é vazado de propósito dentro do `Chunk`.
pub struct Sons {
    tiros: Option<Chunk>,
    abates: Option<Chunk>,
    danos: Option<Chunk>,
    ondas: Option<Chunk>,
    fins: Option<Chunk>,
    mudo: bool,
    /// Quantos sons foram pedidos — medido no `--testar-som` e nos benchmarks.
    tocados: u64,
}

impl Sons {
    /// Sistema mudo: nenhum `Chunk`, nenhuma chamada ao mixer.
    ///
    /// É o que os testes usam e o que `--sem-som` monta: um caminho de código só,
    /// sem `if mudo` espalhado por seis lugares.
    pub fn mudo() -> Self {
        Self {
            tiros: None,
            abates: None,
            danos: None,
            ondas: None,
            fins: None,
            mudo: true,
            tocados: 0,
        }
    }

    /// Carrega os cinco sons. Sem dispositivo, devolve um sistema mudo.
    ///
    /// Nunca falha: som é acessório, e um `Result` aqui obrigaria o chamador a
    /// decidir o que fazer com um erro que não muda o jogo.
    pub fn carregar(disponivel: bool) -> Self {
        if !disponivel {
            log::info!("som: sem dispositivo de áudio, seguindo em silêncio");
            return Self::mudo();
        }
        let mut sistema = Self::mudo();
        sistema.mudo = false;
        sistema.tiros = carregar_um(Som::Tiro);
        sistema.abates = carregar_um(Som::Abate);
        sistema.danos = carregar_um(Som::Dano);
        sistema.ondas = carregar_um(Som::Onda);
        sistema.fins = carregar_um(Som::Fim);

        let carregados = [
            &sistema.tiros,
            &sistema.abates,
            &sistema.danos,
            &sistema.ondas,
            &sistema.fins,
        ]
        .iter()
        .filter(|chunk| chunk.is_some())
        .count();
        if carregados == 0 {
            log::warn!("som: nenhum efeito pôde ser carregado; seguindo em silêncio");
            sistema.mudo = true;
        } else {
            log::info!("som: {carregados} efeito(s) carregado(s) de código");
        }
        sistema
    }

    /// Toca um som. Ignorado quando mudo.
    pub fn tocar(&mut self, som: Som) {
        if self.mudo {
            return;
        }
        let chunk = match som {
            Som::Tiro => &self.tiros,
            Som::Abate => &self.abates,
            Som::Dano => &self.danos,
            Som::Onda => &self.ondas,
            Som::Fim => &self.fins,
        };
        let Some(chunk) = chunk else {
            return;
        };
        match sdl2::mixer::Channel::all().play(chunk, 0) {
            Ok(_) => self.tocados += 1,
            Err(erro) => {
                // Um aviso só: se o mixer recusou uma vez, recusa sempre, e uma
                // linha por quadro afogaria o log.
                log::warn!("som: não consegui tocar '{}' ({erro})", som.nome());
                self.mudo = true;
            }
        }
    }

    /// Som mudou?
    pub fn esta_mudo(&self) -> bool {
        self.mudo
    }

    /// Quantos sons foram efetivamente tocados.
    pub fn tocados(&self) -> u64 {
        self.tocados
    }
}

fn carregar_um(som: Som) -> Option<Chunk> {
    let dados = amostras_estereo(som);
    match Chunk::from_raw_buffer(dados.into_boxed_slice()) {
        Ok(chunk) => Some(chunk),
        Err(erro) => {
            log::warn!("som: falha ao carregar '{}': {erro}", som.nome());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODOS: [Som; 5] = Som::TODOS;

    fn pico(amostras: &[f32]) -> f32 {
        amostras
            .iter()
            .fold(0.0f32, |acc, valor| acc.max(valor.abs()))
    }

    #[test]
    fn o_formato_bate_com_o_que_o_mixer_abriu() {
        // `Mix_QuickLoad_RAW` não converte: os dados têm de estar no formato que
        // a engine pediu em `open_audio(SAMPLE_RATE, AUDIO_S16LSB, 2, ...)`.
        assert_eq!(CANAIS, 2, "o mixer da engine abre estéreo");
        assert_eq!(AMOSTRAGEM, 44_100);
        for som in TODOS {
            let estereo = amostras_estereo(som);
            assert_eq!(
                estereo.len() % CANAIS,
                0,
                "{}: {} amostras não é múltiplo de {CANAIS}",
                som.nome(),
                estereo.len()
            );
        }
    }

    #[test]
    fn a_duracao_de_cada_som_bate_com_a_declarada() {
        for som in TODOS {
            let amostras = amostras(som);
            let segundos = amostras.len() as f32 / AMOSTRAGEM as f32;
            assert!(
                (segundos - som.duracao()).abs() < 0.002,
                "{}: {segundos:.3}s medidos, {:.3}s declarados",
                som.nome(),
                som.duracao()
            );
        }
    }

    #[test]
    fn a_sintese_e_deterministica() {
        // Sem isso a captura de áudio num teste A/B mediria o sorteio.
        for som in TODOS {
            assert_eq!(
                amostras(som),
                amostras(som),
                "{}: duas sínteses deram amostras diferentes",
                som.nome()
            );
        }
    }

    #[test]
    fn todo_som_soa_e_nao_estala() {
        for som in TODOS {
            let faixa = amostras(som);
            let maior = pico(&faixa);
            assert!(
                (maior - PICO).abs() < 0.01,
                "{}: pico {maior:.3}, esperado {PICO:.3}",
                som.nome()
            );
            // Primeira e última amostra perto de zero: um degrau até zero é o
            // estalo que se ouve depois do som.
            assert!(
                faixa[0].abs() < 0.01,
                "{}: começa em {:.3}, o que estala",
                som.nome(),
                faixa[0]
            );
            assert!(
                faixa[faixa.len() - 1].abs() < 0.01,
                "{}: termina em {:.3}, o que estala",
                som.nome(),
                faixa[faixa.len() - 1]
            );
        }
    }

    #[test]
    fn nenhum_som_e_silencio() {
        // O defeito que este teste existe para pegar: uma nota com `peso` errado
        // ou um envelope que zera tudo produz um vetor de zeros que toca "nada"
        // sem erro nenhum.
        for som in TODOS {
            let faixa = amostras(som);
            let ativos = faixa.iter().filter(|v| v.abs() > 0.01).count();
            let fracao = ativos as f32 / faixa.len() as f32;
            assert!(
                fracao > 0.30,
                "{}: só {:.1}% das amostras têm sinal",
                som.nome(),
                fracao * 100.0
            );
        }
    }

    #[test]
    fn a_conversao_para_16_bits_nao_estoura() {
        // `i16::MAX` é 32767; multiplicar por 32768 faz o valor positivo da
        // escala cheia virar o negativo, e o som sai invertido no pico.
        let mono = vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0];
        let estereo = para_estereo_i16(&mono);
        // Par 2i/2i+1 é a amostra i: 0→-2.0, 1→-1.0, 2→-0.5, 3→0.0, 4→0.5,
        // 5→1.0, 6→2.0.
        assert_eq!(estereo[0], -i16::MAX, "abaixo de -1 satura no mínimo");
        assert_eq!(estereo[2], -i16::MAX);
        assert_eq!(estereo[6], 0, "o zero tem de continuar zero");
        assert_eq!(estereo[8], 16384, "0.5 de escala cheia");
        assert_eq!(estereo[10], i16::MAX);
        assert_eq!(estereo[12], i16::MAX, "acima de 1 satura no máximo");
        assert!(estereo.iter().all(|v| *v > i16::MIN), "nenhum -32768");
    }

    #[test]
    fn o_estereo_repete_o_canal() {
        let mono: Vec<f32> = (0..10).map(|i| i as f32 / 10.0).collect();
        let estereo = para_estereo_i16(&mono);
        assert_eq!(estereo.len(), mono.len() * 2);
        for (i, _) in mono.iter().enumerate() {
            assert_eq!(estereo[i * 2], estereo[i * 2 + 1], "amostra {i} difere");
        }
    }

    #[test]
    fn silencio_nao_vira_nan_na_normalizacao() {
        // `normalizar` divide pelo pico: com pico zero o ganho seria infinito e
        // cada amostra viraria NaN, que em `i16` vira 0 — silêncio silencioso.
        let mut zeros = vec![0.0f32; 100];
        normalizar(&mut zeros);
        assert!(zeros.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn o_sistema_mudo_nao_conta_nem_toca() {
        let mut sons = Sons::mudo();
        for som in TODOS {
            sons.tocar(som);
        }
        assert!(sons.esta_mudo());
        assert_eq!(sons.tocados(), 0, "mudo não pode contar como tocado");
    }

    #[test]
    fn carregar_sem_dispositivo_devolve_mudo() {
        // Sem placa de som o jogo não pode morrer nem ficar com handles inválidos.
        let sons = Sons::carregar(false);
        assert!(sons.esta_mudo());
        assert_eq!(sons.tocados(), 0);
    }

    #[test]
    fn cada_som_tem_nome_e_duracao_proprios() {
        let mut nomes = Vec::new();
        for som in TODOS {
            assert!(
                !nomes.contains(&som.nome()),
                "nome repetido: {}",
                som.nome()
            );
            nomes.push(som.nome());
            assert!(som.duracao() > 0.0);
        }
        // Os cinco sons têm de soar diferentes: dois com a mesma duração e o
        // mesmo número de amostras seria o começo de uma cópia silenciosa.
        let assinaturas: Vec<usize> = TODOS.iter().map(|som| amostras(*som).len()).collect();
        for (i, a) in assinaturas.iter().enumerate() {
            for b in assinaturas.iter().skip(i + 1) {
                assert_ne!(a, b, "dois sons com o mesmo comprimento");
            }
        }
    }
}
