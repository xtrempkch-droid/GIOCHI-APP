//! Ferramentas puras: matemática pequena e geração de malha em código.
//!
//! Nada aqui toca GL, SDL ou estado global. É o que permite testar a geometria do
//! asteroide e os limites da câmera sem abrir janela — a regra da casa é que uma
//! afirmação só vale depois de medida, e medir sem tela é o que faz o CI servir
//! para alguma coisa.

pub mod math;
pub mod procedural_mesh;
