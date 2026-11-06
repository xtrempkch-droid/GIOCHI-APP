//! Núcleo da loja Giochi: catálogo, pedido, licença e verificação de pacote.
//!
//! # A ideia central: três coisas diferentes, três chaves diferentes
//!
//! A pergunta que originou este crate foi "e se alguém usar a chave pública para
//! roubar os jogos?". Ela parte de uma confusão comum, e desfazê-la é o desenho
//! inteiro:
//!
//! | Conceito | Pergunta que responde | Quem assina |
//! | --- | --- | --- |
//! | **Autenticidade** | "este pacote veio da Giochi?" | chave de **catálogo** |
//! | **Licença** | "este aparelho comprou?" | chave de **licença** |
//! | **Identidade** | "é este aparelho mesmo?" | par gerado **no aparelho** |
//!
//! Uma chave **pública** não permite nada além de conferir assinaturas: não
//! decifra, não abre catálogo, não gera licença. O risco real nunca foi ela —
//! é (a) a chave **privada** vazar do servidor, e (b) **confundir "autêntico"
//! com "comprado"**, que é o que faria um único pacote assinado servir para
//! todo mundo.
//!
//! Este crate implementa (b) separado desde o tipo: `PacoteVerificado` prova
//! origem e nada mais; `LicencaVerificada` prova compra e exige o **aparelho**.
//!
//! # O que este crate não faz
//!
//! Não guarda chave privada de catálogo — ela fica no servidor, e é por isso que
//! existe o `--emitir-licenca` de teste: para exercitar o fluxo sem ela.
//! Também não fala HTTP nem desenha: isso é do binário.

pub mod baixador;
pub mod catalogo;
pub mod chave;
pub mod licenca;
pub mod pacote;
pub mod pagamento;
pub mod preco;

pub use chave::{ChavePublica, ErroDeChave};
pub use preco::{Moeda, Preco};
