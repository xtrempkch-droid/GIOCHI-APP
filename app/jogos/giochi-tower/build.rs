//! Build do GIOCHI Tower Defense.
//!
//! # Por que não há passo de link
//!
//! O prompt mestre descreve a engine como ABI C (`libgiochi.so` + `giochi.h`) e
//! pede um `build.rs` "para linkar libgiochi". **Esse caminho não existe neste
//! repositório**: o snapshot em `vendor/` traz `giochi-engine` e
//! `giochi-engine-3d` como crates Rust, sem `extern "C"`, sem `libgiochi` e sem
//! `docs/C_ABI_CONTRACT.md`. O crate `giochi-engine-capi` mora no repositório da
//! engine, não aqui, e `giochi-defender`, `neon-drift` e `cobrinha` — os três
//! jogos que compilam hoje — consomem os mesmos crates Rust por caminho.
//!
//! Portanto: não há `println!("cargo:rustc-link-lib=giochi")`, porque inventar um
//! link que não resolve transformaria o sintoma ("não acha -lgiochi") em
//! diagnóstico. A decisão e o caminho de troca estão em `DECISIONS.md` (D-002).
//!
//! O que este script faz é real e útil: carimba origem da compilação nos
//! binários (aparece no banner de abertura) e falha cedo se o snapshot da engine
//! não estiver presente — em vez de deixar o erro aparecer como um `cargo`
//! reclamando de um caminho que falta.

use std::env;
use std::path::Path;

/// Caminho do snapshot da engine, relativo à raiz do jogo.
const VENDOR_SNAPSHOT: &str = "vendor/giochi-engine-3d/Cargo.toml";

fn main() {
    let target = env::var("TARGET").unwrap_or_else(|_| "desconhecido".to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "desconhecido".to_string());

    println!("cargo:rustc-env=GIOCHI_TOWER_TARGET={target}");
    println!("cargo:rustc-env=GIOCHI_TOWER_PROFILE={profile}");

    // Sem o snapshot o resto do build produziria uma cascata de erros de módulo.
    if !Path::new(VENDOR_SNAPSHOT).exists() {
        panic!(
            "snapshot da engine ausente: {VENDOR_SNAPSHOT} não existe. \
             Copie de outro jogo do repositório: \
             `Copy-Item ../giochi-defender/vendor . -Recurse` (ou `cp -r` no Linux). \
             Ver vendor/Cargo.toml para o procedimento de atualização."
        );
    }

    // O manifesto do snapshot é contrato: mudar versão ou features dele muda o
    // que o jogo compila, então o build tem de ser refeito.
    println!("cargo:rerun-if-changed={VENDOR_SNAPSHOT}");
    println!("cargo:rerun-if-changed=vendor/Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
}
