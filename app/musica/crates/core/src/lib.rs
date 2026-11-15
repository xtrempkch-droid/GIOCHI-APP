//! Núcleo do app de música do Giochi.
//!
//! # Por que um crate separado do binário
//!
//! O binário depende da `giochi-engine`, que depende de SDL2 — e o SDL2 não
//! existe no CI. Este crate **não tem nenhuma dependência**, então tudo o que
//! decide comportamento roda em `cargo test` em qualquer runner: ordem das
//! faixas, o que acontece ao apertar `A`, geometria do carrossel, capa derivada
//! do nome, layout em cada tamanho de painel e a rotação que o sistema pede.
//!
//! A regra é a mesma do `giochi-launcher-core`: **este crate não conhece pixel,
//! janela, GPU nem placa de som.** Ele recebe valores e devolve decisões.
//!
//! # O que o app de exemplo fazia e este núcleo corrige
//!
//! | Defeito do exemplo | Onde é resolvido |
//! | --- | --- |
//! | lia botão por **nível** dentro do passo fixo, então segurar o direcional corria a lista inteira | [`Acao`] vem de borda (quem lê o controle é o app, com o `Mapper` do launcher) |
//! | não havia pausa, parada nem volume: só `play` | [`Efeito::Pausar`], [`Efeito::Parar`], [`Efeito::Volume`] |
//! | o carrossel não dava a volta | [`carrossel::Carrossel::mover`] |
//! | pasta vazia derrubava o app no primeiro asset ausente | [`biblioteca::Biblioteca::Vazia`] com motivo |
//! | ordem das faixas vinha do `readdir` | [`faixa::ordem_natural`] |
//! | nome de arquivo cru na tela (`01_-_musica.mp3`) | [`faixa::titulo_de_arquivo`] |
//! | aspect fixo 16:9, sem painel nem rotação | [`layout::Layout`] e [`rotacao`] |
//! | nenhum teste | 70+ testes sem SDL |

pub mod app;
pub mod biblioteca;
pub mod capa;
pub mod carrossel;
pub mod faixa;
pub mod layout;
pub mod rotacao;
pub mod texto;
pub mod transporte;

pub use app::{Acao, Efeito, Estado};
pub use biblioteca::{montar, pasta_padrao, Biblioteca, Entrada, Vazio};
pub use capa::Capa;
pub use carrossel::{Carrossel, Geometria};
pub use faixa::{Faixa, Formato};
pub use layout::Layout;
pub use rotacao::{Fonte, Projecao, Rotacao};
pub use texto::Dica;
pub use transporte::{EstadoPlay, Transporte};

/// Versão do app, para a tela de diagnóstico.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");
