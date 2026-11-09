//! Custo de simulação por quadro, sem GL.
//!
//! ## O que este banco mede, e o que ele **não** mede
//!
//! Mede o **custo de CPU da simulação** com o pior caso do envelope (15 cubos):
//! `Partida::atualizar` passo a passo, mais a geração de malha que só acontece
//! uma vez. É o número que responde "a simulação é um risco de orçamento?".
//!
//! Não mede o quadro de tela. Intervalo entre quadros inclui o desenho, o
//! `drmModePageFlip` e a espera pelo vblank — nada disso existe sem janela, e é
//! por isso que essa medição mora no próprio jogo, atrás de `--benchmark N`.
//! Os dois números são diferentes e ambos úteis; confundi-los é o que faz um
//! relatório afirmar "60 fps" para um jogo que engasga.
//!
//! ## Por que sem `criterion`
//!
//! Duas razões. A primeira é dependência: `criterion` puxa dezenas de crates
//! para medir três laços. A segunda é o alvo: o número que importa é o do
//! aparelho, e o criterion não roda lá. Um binário que imprime microssegundos
//! serve aos dois.
//!
//! ```text
//! cargo bench --bench frame_time
//! ```

use std::hint::black_box;
use std::time::Instant;

use giochi_defender::gameplay::partida::{Partida, PASSO_MAX};
use giochi_defender::state::GameState;
use giochi_defender::utils::procedural_mesh::{generate_icosphere, generate_starfield};

/// Quadros simulados por amostra.
const QUADROS: u32 = 20_000;
/// Quantas vezes cada medição se repete; fica o melhor tempo.
const REPETICOES: u32 = 5;
/// De quantos em quantos quadros o cenário é reposto.
///
/// Sem repor, a partida **acaba** em poucos segundos (cinco cubos chegam e o
/// jogador perde as cinco vidas) e `Partida::atualizar` passa a devolver na
/// primeira linha — o que a medição mediria é o custo de um `return`, e foi
/// exatamente o que aconteceu na primeira versão: **0,010 µs por quadro** com 15
/// cubos, um número bom demais para ser verdade.
const REPOR_A_CADA: u32 = 60;

fn main() {
    println!("GIOCHI DEFENDER — custo de simulação por quadro (sem GL)\n");

    medir("simulacao, 1 cubo", || {
        let mut partida = Partida::new(1);
        partida.povoar(1);
        (partida, GameState::new())
    });
    medir("simulacao, 15 cubos (envelope do R5)", || {
        let mut partida = Partida::new(1);
        partida.povoar(15);
        (partida, GameState::new())
    });

    println!("\nGeração de malha (uma vez, na abertura):");
    let inicio = Instant::now();
    let asteroide = generate_icosphere(1, 2.0, 0.22);
    println!(
        "  asteroide           {:>8.1} us  ({} triangulos)",
        inicio.elapsed().as_secs_f64() * 1e6,
        asteroide.triangulos()
    );
    let inicio = Instant::now();
    let ceu = generate_starfield(500, 120.0, 0.45, 0x5EED_0000);
    println!(
        "  ceu de 500 estrelas {:>8.1} us  ({} triangulos)",
        inicio.elapsed().as_secs_f64() * 1e6,
        ceu.triangulos()
    );

    println!(
        "\npasso fixo: {:.4} ms por quadro ({} Hz)",
        PASSO_MAX * 1000.0,
        (1.0 / PASSO_MAX).round() as u32
    );
}

/// Mede uma simulação que começa cheia e roda `QUADROS` passos.
fn medir(rotulo: &str, montar: impl Fn() -> (Partida, GameState)) {
    let mut melhor = f64::INFINITY;
    for _ in 0..REPETICOES {
        let (mut partida, mut estado) = montar();
        // Um passo de aquecimento: a primeira chamada paga o que o preditor de
        // desvio e o cache têm de aprender, e ela não é o regime permanente.
        partida.atualizar(PASSO_MAX, &mut estado);

        let inicio = Instant::now();
        for quadro in 0..QUADROS {
            partida.atualizar(PASSO_MAX, &mut estado);
            if quadro % REPOR_A_CADA == REPOR_A_CADA - 1 {
                // Devolve o cenário ao pior caso: jogador vivo e pool cheio.
                estado.lives = giochi_defender::state::VIDAS_INICIAIS;
                partida.povoar(partida.capacidade() as u32);
            }
            // `black_box` sobre um valor que sai da simulação: sem isto o
            // compilador vê que nada observa o resultado e remove o laço inteiro.
            black_box(partida.inimigos());
        }
        let por_quadro = inicio.elapsed().as_secs_f64() * 1e6 / QUADROS as f64;
        melhor = melhor.min(por_quadro);
    }

    // O orçamento é de 16,67 ms por quadro a 60 Hz. Um por cento disso é o teto
    // que a simulação pode ocupar sem competir com o desenho.
    let fracao = melhor / 16_667.0 * 100.0;
    println!("{rotulo:<42} {melhor:>7.3} us/quadro  ({fracao:.2} % do orcamento de 60 Hz)");
}
