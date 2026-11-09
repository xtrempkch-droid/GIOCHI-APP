//! Medição de tempo de quadro.
//!
//! ## Por que no jogo e não num `benches/` com criterion
//!
//! O número que interessa é o do **aparelho**, com o painel de 640×480 e o
//! KMSDRM esperando o vblank. O criterion roda no host, mede a CPU e não tem
//! como representar a espera de campo. Então a medição vive no laço do jogo, é
//! ligada por `--benchmark N`, e o mesmo comando roda no PC e no console.
//!
//! Mede duas coisas, e a diferença entre elas é o diagnóstico:
//!
//! * **intervalo entre inícios de quadro** — o que o jogador sente como
//!   fluidez, já com a espera pelo vblank;
//! * **trabalho de submissão** — do início do desenho até a troca de buffers.
//!
//! No aparelho as duas podem ser muito diferentes: o `drmModePageFlip` espera o
//! campo, então um quadro pode custar 3 ms de trabalho e 20 ms de intervalo. Se
//! as duas coincidirem, o gargalo é o jogo; se o trabalho for uma fração
//! pequena, o gargalo é o painel. Sem separar, "49 fps" e "49 fps porque o
//! painel é assim" viram a mesma frase — e a segunda é a verdade no X35H.
//!
//! O trabalho medido é o de **submissão**: os comandos de GL são enfileirados e
//! a GPU termina durante a espera do vblank. Ele não é "tempo de GPU"; é o que
//! a CPU precisa fazer antes de poder trocar os buffers.
//!
//! ## Por que percentil e não média
//!
//! Uma média de 40 fps pode ser 40 fps constantes ou 60 com engasgos de 200 ms.
//! O segundo caso é injogável e a média esconde. `p95` e o pior quadro são os
//! números que aparecem na mão do jogador.

use std::time::Duration;

/// Estatísticas de uma medição de quadros.
#[derive(Debug, Clone, PartialEq)]
pub struct Benchmark {
    /// Quantos quadros foram medidos.
    pub quadros: u64,
    /// Durações coletadas, em microssegundos.
    duracoes_us: Vec<u64>,
    /// Trabalho de submissão de cada quadro, em microssegundos.
    trabalho_us: Vec<u64>,
}

impl Benchmark {
    /// Medição vazia.
    pub fn new() -> Self {
        Self {
            quadros: 0,
            duracoes_us: Vec::new(),
            trabalho_us: Vec::new(),
        }
    }

    /// Registra o intervalo até o quadro anterior.
    pub fn registrar(&mut self, duracao: Duration) {
        self.quadros += 1;
        self.duracoes_us.push(duracao.as_micros() as u64);
    }

    /// Registra o trabalho de submissão do quadro que acabou.
    pub fn registrar_trabalho(&mut self, duracao: Duration) {
        self.trabalho_us.push(duracao.as_micros() as u64);
    }

    /// Trabalho de submissão mediano, em microssegundos.
    pub fn trabalho_mediano_us(&self) -> u64 {
        if self.trabalho_us.is_empty() {
            return 0;
        }
        let mut ordenadas = self.trabalho_us.clone();
        ordenadas.sort_unstable();
        ordenadas[ordenadas.len() / 2]
    }

    /// Quanto do intervalo é trabalho do jogo, de 0 a 1.
    ///
    /// Perto de 1, o jogo é o gargalo; perto de 0, é o painel. É a razão pela
    /// qual esta medição existe: sem ela, "49 fps" não diz de quem é o limite.
    pub fn fracao_de_trabalho(&self) -> f32 {
        let intervalo = self.mediana_us();
        if intervalo == 0 {
            return 0.0;
        }
        (self.trabalho_mediano_us() as f32 / intervalo as f32).clamp(0.0, 1.0)
    }

    /// Mediana do intervalo entre quadros, em microssegundos.
    pub fn mediana_us(&self) -> u64 {
        self.percentil(0.50)
    }

    /// Intervalo que 95 % dos quadros respeitam, em microssegundos.
    pub fn p95_us(&self) -> u64 {
        self.percentil(0.95)
    }

    /// Intervalo que 99 % dos quadros respeitam, em microssegundos.
    pub fn p99_us(&self) -> u64 {
        self.percentil(0.99)
    }

    /// O pior quadro, em microssegundos.
    pub fn pior_us(&self) -> u64 {
        self.duracoes_us.iter().copied().max().unwrap_or(0)
    }

    /// Quadros por segundo a partir da mediana.
    ///
    /// A mediana e não a média: é o que o jogador sente na maior parte do tempo,
    /// e uma pausa de carregamento não a puxa para baixo como puxaria a média.
    pub fn fps_mediano(&self) -> f32 {
        let mediana = self.mediana_us();
        if mediana == 0 {
            return 0.0;
        }
        1_000_000.0 / mediana as f32
    }

    /// Percentil `p` (0,0 a 1,0) por posto, sem interpolar.
    ///
    /// Ordena na leitura em vez de a cada quadro: a medição é de algumas
    /// centenas de quadros e o custo aparece uma vez, no fim.
    pub fn percentil(&self, p: f32) -> u64 {
        if self.duracoes_us.is_empty() {
            return 0;
        }
        let mut ordenadas = self.duracoes_us.clone();
        ordenadas.sort_unstable();
        let indice = ((ordenadas.len() - 1) as f32 * p.clamp(0.0, 1.0)).round() as usize;
        ordenadas[indice.min(ordenadas.len() - 1)]
    }

    /// Resumo em uma linha, para o log.
    pub fn resumo(&self) -> String {
        if self.quadros == 0 {
            return "benchmark: nenhum quadro medido".to_string();
        }
        format!(
            "benchmark: {} quadros · {:.1} fps (mediana) · p50 {:.2} ms · p95 {:.2} ms · \
             p99 {:.2} ms · pior {:.2} ms · trabalho {:.2} ms ({:.0} % do intervalo)",
            self.quadros,
            self.fps_mediano(),
            self.mediana_us() as f32 / 1000.0,
            self.p95_us() as f32 / 1000.0,
            self.p99_us() as f32 / 1000.0,
            self.pior_us() as f32 / 1000.0,
            self.trabalho_mediano_us() as f32 / 1000.0,
            self.fracao_de_trabalho() * 100.0
        )
    }
}

impl Default for Benchmark {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn com_duracoes(ms: &[u64]) -> Benchmark {
        let mut benchmark = Benchmark::new();
        for valor in ms {
            benchmark.registrar(Duration::from_millis(*valor));
        }
        benchmark
    }

    #[test]
    fn a_mediana_de_um_conjunto_impar_e_o_do_meio() {
        let benchmark = com_duracoes(&[10, 30, 20]);
        assert_eq!(benchmark.mediana_us(), 20_000);
    }

    #[test]
    fn o_fps_vem_da_mediana_e_nao_da_media() {
        // O caso que a média esconde: 9 quadros de 16 ms e um de 500 ms. A média
        // dá ~30 fps, a mediana dá ~62 — e 62 é o que o jogador sente.
        let mut benchmark = com_duracoes(&[16, 16, 16, 16, 16, 16, 16, 16, 16]);
        benchmark.registrar(Duration::from_millis(500));
        assert!(
            benchmark.fps_mediano() > 60.0,
            "fps mediano {:.1}",
            benchmark.fps_mediano()
        );
        assert_eq!(benchmark.pior_us(), 500_000, "o engasgo tem de aparecer");
    }

    #[test]
    fn os_percentis_ordenam() {
        let benchmark = com_duracoes(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert!(benchmark.mediana_us() <= benchmark.p95_us());
        assert!(benchmark.p95_us() <= benchmark.p99_us());
        assert!(benchmark.p99_us() <= benchmark.pior_us());
    }

    #[test]
    fn vazio_nao_divide_por_zero() {
        let benchmark = Benchmark::new();
        assert_eq!(benchmark.mediana_us(), 0);
        assert_eq!(benchmark.fps_mediano(), 0.0);
        assert!(benchmark.resumo().contains("nenhum quadro"));
    }

    #[test]
    fn a_fracao_de_trabalho_diz_quem_e_o_gargalo() {
        // O caso medido no X35H: 20,28 ms de intervalo com poucos milissegundos
        // de trabalho é o painel; trabalho igual ao intervalo é o jogo.
        let mut painel = Benchmark::new();
        for _ in 0..10 {
            painel.registrar(Duration::from_micros(20_280));
            painel.registrar_trabalho(Duration::from_micros(3_000));
        }
        assert!(
            painel.fracao_de_trabalho() < 0.25,
            "{:.2}",
            painel.fracao_de_trabalho()
        );

        let mut jogo = Benchmark::new();
        for _ in 0..10 {
            jogo.registrar(Duration::from_micros(20_000));
            jogo.registrar_trabalho(Duration::from_micros(19_000));
        }
        assert!(jogo.fracao_de_trabalho() > 0.9);
    }

    #[test]
    fn sem_trabalho_registrado_a_fracao_e_zero() {
        let mut benchmark = com_duracoes(&[16, 16]);
        assert_eq!(benchmark.trabalho_mediano_us(), 0);
        assert_eq!(benchmark.fracao_de_trabalho(), 0.0);
        benchmark.registrar_trabalho(Duration::from_millis(4));
        assert_eq!(benchmark.trabalho_mediano_us(), 4_000);
    }

    #[test]
    fn o_resumo_traz_os_quatro_numeros() {
        let benchmark = com_duracoes(&[16, 16, 16, 33]);
        let resumo = benchmark.resumo();
        for esperado in ["fps", "p50", "p95", "p99", "pior"] {
            assert!(
                resumo.contains(esperado),
                "faltou '{esperado}' em '{resumo}'"
            );
        }
    }
}
