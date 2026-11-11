//! Erros do jogo.
//!
//! Uma única enum para atravessar `main`, o bridge e o estado: o jogo é pequeno e
//! o custo de uma taxonomia de erro por módulo seria maior que o benefício. As
//! variantes que distinguem causa (`Config` × `Engine` × `Render3D`) existem para
//! que a mensagem de falha diga *de quem* é o problema — a mesma exigência do
//! `docs/checklist-imagem.md` do repositório.

use thiserror::Error;

/// Resultado do jogo.
pub type Result<T> = std::result::Result<T, GameError>;

/// Falha em qualquer camada do jogo.
#[derive(Debug, Error)]
pub enum GameError {
    /// Erro devolvido por `giochi-engine` (plataforma, janela, áudio, input).
    #[error("engine: {0}")]
    Engine(#[from] giochi_engine::GiochiError),

    /// Erro devolvido pelo renderer 3D (contexto GLES, malha, shader, draw).
    #[error("render 3D: {0}")]
    Render3D(#[from] giochi_engine_3d::Error3D),

    /// Configuração ausente, ilegível ou fora das regras do GDD.
    #[error("configuração: {0}")]
    Config(#[from] crate::config::ConfigError),

    /// Falha de I/O do próprio jogo (captura de quadro, bench).
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    /// Falha com mensagem já pronta para o jogador/operador.
    #[error("{0}")]
    Message(String),
}

impl GameError {
    /// Cria um [`GameError::Message`] a partir de qualquer coisa exibível.
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}
