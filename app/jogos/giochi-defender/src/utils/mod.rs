//! Ferramentas puras: matemática pequena, sorteio e geração de malha em código.
//!
//! Nada aqui toca GL, SDL ou estado global. É o que permite testar a geometria do
//! asteroide, os limites da câmera e a colisão sem abrir janela — a regra da casa
//! é que uma afirmação só vale depois de medida, e medir sem tela é o que faz o
//! CI servir para alguma coisa.

pub mod math;
pub mod procedural_mesh;
pub mod rng;
