//! Chaves públicas Ed25519, nos três formatos que este projeto já produz.
//!
//! # Por que três formatos
//!
//! Ferramentas diferentes já geraram chaves diferentes, e recusar as antigas
//! quebraria o que existe:
//!
//! | Formato | Quem produz | Onde |
//! | --- | --- | --- |
//! | 32 bytes crus | `giochi_forge.generate_keypair` | `pubkey.ed25519` |
//! | PEM SPKI | `scripts/empacotar.mjs`, `openssl` | `*.pem` |
//! | base64 dos 32 bytes | catálogo/API | JSON |
//!
//! O PEM do Ed25519 é sempre o mesmo DER de 44 bytes: 12 de cabeçalho e os 32 da
//! chave. Não há ASN.1 variável para interpretar — é por isso que "os últimos 32
//! bytes" é correto aqui e seria errado para RSA.

use std::fmt;
use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Prefixo DER de uma chave pública Ed25519 em SPKI (RFC 8410).
const PREFIXO_SPKI: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Tamanho de uma assinatura Ed25519, em bytes.
pub const TAMANHO_ASSINATURA: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDeChave {
    Vazia,
    TamanhoErrado(usize),
    Base64Invalido(String),
    NaoEd25519,
    Arquivo(String),
}

impl fmt::Display for ErroDeChave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vazia => write!(f, "chave vazia"),
            Self::TamanhoErrado(n) => {
                write!(f, "chave com {n} bytes; Ed25519 tem 32")
            }
            Self::Base64Invalido(e) => write!(f, "base64 inválido: {e}"),
            Self::NaoEd25519 => write!(f, "a chave não é Ed25519"),
            Self::Arquivo(e) => write!(f, "não consegui ler a chave: {e}"),
        }
    }
}

impl std::error::Error for ErroDeChave {}

/// Uma chave pública Ed25519 já validada.
#[derive(Clone, PartialEq, Eq)]
pub struct ChavePublica(VerifyingKey);

impl ChavePublica {
    /// Direto dos 32 bytes.
    pub fn de_bytes(bytes: &[u8]) -> Result<Self, ErroDeChave> {
        if bytes.is_empty() {
            return Err(ErroDeChave::Vazia);
        }
        if bytes.len() != 32 {
            return Err(ErroDeChave::TamanhoErrado(bytes.len()));
        }
        let mut a = [0u8; 32];
        a.copy_from_slice(bytes);
        // `from_bytes` recusa ponto não canônico. É a diferença entre "32 bytes
        // de tamanho certo" e "uma chave válida" — e a primeira não verifica
        // nada.
        VerifyingKey::from_bytes(&a)
            .map(Self)
            .map_err(|_| ErroDeChave::NaoEd25519)
    }

    /// base64 dos 32 bytes crus (o formato que viaja em JSON).
    pub fn de_base64(texto: &str) -> Result<Self, ErroDeChave> {
        let limpo: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
        if limpo.is_empty() {
            return Err(ErroDeChave::Vazia);
        }
        let bytes = B64
            .decode(limpo.as_bytes())
            .map_err(|e| ErroDeChave::Base64Invalido(e.to_string()))?;
        Self::de_bytes(&bytes)
    }

    /// PEM SPKI (`-----BEGIN PUBLIC KEY-----`).
    ///
    /// Aceita o DER "como está" porque o PEM é só base64 dele entre linhas de
    /// cabeçalho — não há motivo para um decodificador PEM completo aqui.
    pub fn de_pem(texto: &str) -> Result<Self, ErroDeChave> {
        let corpo: String = texto
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");
        let der = B64
            .decode(corpo.trim().as_bytes())
            .map_err(|e| ErroDeChave::Base64Invalido(e.to_string()))?;
        Self::de_der(&der)
    }

    /// DER SPKI. Espera o prefixo do Ed25519 e ignora o resto além dos 32 bytes.
    pub fn de_der(der: &[u8]) -> Result<Self, ErroDeChave> {
        if der.len() < PREFIXO_SPKI.len() + 32 {
            return Err(ErroDeChave::TamanhoErrado(der.len()));
        }
        if der[..PREFIXO_SPKI.len()] != PREFIXO_SPKI {
            // Sem esta checagem, uma chave RSA de 44 bytes passaria e falharia
            // na primeira verificação, com uma mensagem que culpa o pacote.
            return Err(ErroDeChave::NaoEd25519);
        }
        Self::de_bytes(&der[PREFIXO_SPKI.len()..PREFIXO_SPKI.len() + 32])
    }

    /// Tenta os três formatos, na ordem em que são reconhecíveis.
    ///
    /// A ordem importa: PEM tem cabeçalho, base64 é longo, e 32 bytes crus só
    /// chegam por binário. Tentar na ordem errada daria "base64 inválido" para
    /// um PEM.
    pub fn de_texto(texto: &str) -> Result<Self, ErroDeChave> {
        if texto.contains("-----BEGIN") {
            return Self::de_pem(texto);
        }
        Self::de_base64(texto)
    }

    pub fn de_arquivo(caminho: impl AsRef<Path>) -> Result<Self, ErroDeChave> {
        let caminho = caminho.as_ref();
        let bytes =
            std::fs::read(caminho).map_err(|e| ErroDeChave::Arquivo(format!("{}: {e}", caminho.display())))?;
        // Regra: **32 bytes é chave crua; qualquer outro tamanho é texto.**
        //
        // Tentar texto primeiro seria errado (uma chave crua pode ser UTF-8
        // válido por acaso), e tentar binário primeiro quebra `.b64`, que é o
        // formato mais comum de vir de um servidor. O tamanho desempata sem
        // ambiguidade, porque 32 é exatamente o tamanho de uma chave.
        if bytes.len() == 32 {
            return Self::de_bytes(&bytes);
        }
        let texto = std::str::from_utf8(&bytes)
            .map_err(|_| ErroDeChave::TamanhoErrado(bytes.len()))?;
        Self::de_texto(texto)
    }

    /// A pública correspondente a uma semente de 32 bytes.
    ///
    /// É a operação que o **aparelho** faz no primeiro boot: gera 32 bytes
    /// aleatórios (a privada, que nunca sai) e guarda só o resultado aqui.
    ///
    /// ## Por que não é `de_bytes(semente)`
    ///
    /// Porque a semente **não é** uma chave pública. Um Ed25519 exige um ponto
    /// válido da curva, e 32 bytes quaisquer não são: passar `[7u8; 32]`
    /// devolve `NaoEd25519`. Descobri isso ao tentar usar bytes fixos como
    /// "aparelho de teste" — o erro estava certo e a expectativa errada.
    pub fn de_semente(semente: &[u8; 32]) -> Self {
        let chave = ed25519_dalek::SigningKey::from_bytes(semente);
        // `verifying_key().to_bytes()` é sempre um ponto válido: a derivação
        // garante isso, então não há `Result` aqui.
        let a = chave.verifying_key().to_bytes();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&a);
        Self(VerifyingKey::from_bytes(&arr).expect("chave derivada é sempre válida"))
    }

    /// Os 32 bytes, para mandar ao servidor.
    pub fn bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn base64(&self) -> String {
        B64.encode(self.0.to_bytes())
    }

    /// Confere uma assinatura sobre `mensagem`.
    ///
    /// Devolve `false` — não erro — para assinatura de tamanho errado. A
    /// diferença entre "não confere" e "não sei conferir" fica para quem chama,
    /// porque só ele sabe o que fazer com cada caso.
    pub fn verifica(&self, mensagem: &[u8], assinatura: &[u8]) -> bool {
        if assinatura.len() != TAMANHO_ASSINATURA {
            return false;
        }
        let mut a = [0u8; TAMANHO_ASSINATURA];
        a.copy_from_slice(assinatura);
        self.0.verify(mensagem, &Signature::from_bytes(&a)).is_ok()
    }
}

impl fmt::Debug for ChavePublica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // O Debug padrão imprimiria 32 bytes em decimal, ilegível. O prefixo
        // base64 é o que aparece nos logs de servidor.
        write!(f, "ChavePublica({}…)", &self.base64()[..8])
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn par() -> (SigningKey, ChavePublica) {
        // Semente fixa: teste não pode depender de aleatoriedade.
        let s = SigningKey::from_bytes(&[7u8; 32]);
        let p = ChavePublica::de_bytes(&s.verifying_key().to_bytes()).unwrap();
        (s, p)
    }

    #[test]
    fn verifica_assinatura_do_proprio_par() {
        let (s, p) = par();
        let msg = b"pacote de teste";
        let sig = s.sign(msg).to_bytes();
        assert!(p.verifica(msg, &sig));
    }

    #[test]
    fn recusa_mensagem_alterada() {
        let (s, p) = par();
        let sig = s.sign(b"original").to_bytes();
        assert!(!p.verifica(b"alterada", &sig));
    }

    #[test]
    fn recusa_assinatura_de_outro_par() {
        let (_, p) = par();
        let outro = SigningKey::from_bytes(&[9u8; 32]);
        let sig = outro.sign(b"x").to_bytes();
        assert!(!p.verifica(b"x", &sig));
    }

    #[test]
    fn assinatura_de_tamanho_errado_e_falso_nao_erro() {
        // 89 bytes é assinatura base64 gravada como texto — o erro que já
        // aconteceu neste projeto. Tem de ser `false`, não pânico.
        let (_, p) = par();
        assert!(!p.verifica(b"x", &[0u8; 89]));
        assert!(!p.verifica(b"x", &[]));
    }

    #[test]
    fn os_tres_formatos_dao_a_mesma_chave() {
        let (_, p) = par();
        let cru = p.bytes();
        assert_eq!(ChavePublica::de_bytes(&cru).unwrap(), p);
        assert_eq!(ChavePublica::de_base64(&p.base64()).unwrap(), p);

        let mut der = PREFIXO_SPKI.to_vec();
        der.extend_from_slice(&cru);
        assert_eq!(ChavePublica::de_der(&der).unwrap(), p);

        let pem = format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
            B64.encode(&der)
        );
        assert_eq!(ChavePublica::de_pem(&pem).unwrap(), p);
        assert_eq!(ChavePublica::de_texto(&pem).unwrap(), p);
    }

    #[test]
    fn der_que_nao_e_ed25519_e_recusado_com_motivo() {
        // Um DER de RSA começa com 3082..., não com o prefixo Ed25519. Se
        // passasse, a falha apareceria na verificação e culparia o pacote.
        let der_rsa = [0x30u8, 0x82, 0x01, 0x22, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86,
                       0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00, 0x03, 0x82, 0x01, 0x0f, 0x00];
        let mut grande = der_rsa.to_vec();
        grande.resize(600, 0);
        assert_eq!(ChavePublica::de_der(&grande), Err(ErroDeChave::NaoEd25519));
    }

    #[test]
    fn arquivo_com_base64_em_texto_e_lido() {
        // Um `.b64` de 45 bytes: se `de_arquivo` só tentasse bytes crus, o erro
        // seria "chave com 45 bytes; Ed25519 tem 32" — que não sugere o que
        // fazer. Aconteceu na primeira execução da ferramenta de licença.
        let (_, p) = par();
        let dir = std::env::temp_dir().join(format!("giochi-chave-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let b64 = dir.join("pub.b64");
        std::fs::write(&b64, format!("{}\n", p.base64())).unwrap();
        assert_eq!(ChavePublica::de_arquivo(&b64).unwrap(), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arquivo_binario_de_32_bytes_e_lido_como_cru() {
        let (_, p) = par();
        let dir = std::env::temp_dir().join(format!("giochi-chave-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cru = dir.join("pub.ed25519");
        std::fs::write(&cru, p.bytes()).unwrap();
        assert_eq!(ChavePublica::de_arquivo(&cru).unwrap(), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base64_com_espacos_e_quebras_e_aceito() {
        let (_, p) = par();
        let b = p.base64();
        let com_espacos = format!("{} \n {}", &b[..10], &b[10..]);
        assert_eq!(ChavePublica::de_base64(&com_espacos).unwrap(), p);
    }

    #[test]
    fn semente_deriva_uma_chave_valida_e_estavel() {
        // O caso que motivou: usar `[7u8; 32]` como "aparelho de teste" dava
        // `NaoEd25519`, porque a semente não é uma chave pública. Derivar é o
        // que o aparelho faz no primeiro boot.
        let a = ChavePublica::de_semente(&[7u8; 32]);
        let b = ChavePublica::de_semente(&[7u8; 32]);
        assert_eq!(a, b, "a mesma semente tem de dar a mesma chave");
        assert_ne!(a, ChavePublica::de_semente(&[8u8; 32]));
        // E a derivada tem de sobreviver à ida e volta pelo base64, que é como
        // ela viaja até o servidor.
        assert_eq!(ChavePublica::de_base64(&a.base64()).unwrap(), a);
    }

    #[test]
    fn bytes_quaisquer_nao_sao_chave_publica() {
        // A distinção que importa: 32 bytes de tamanho certo não são uma chave.
        assert_eq!(ChavePublica::de_bytes(&[7u8; 32]), Err(ErroDeChave::NaoEd25519));
        assert!(ChavePublica::de_semente(&[7u8; 32]).base64().len() == 44);
    }

    #[test]
    fn vazio_e_tamanho_errado_tem_mensagens_distintas() {
        assert_eq!(ChavePublica::de_bytes(&[]), Err(ErroDeChave::Vazia));
        assert_eq!(
            ChavePublica::de_bytes(&[0u8; 31]),
            Err(ErroDeChave::TamanhoErrado(31))
        );
    }
}
