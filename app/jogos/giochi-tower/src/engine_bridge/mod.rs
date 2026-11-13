//! Ponte entre o jogo e a engine.
//!
//! É a **única** camada autorizada a falar com `giochi-engine`/`giochi-engine-3d`.
//! O resto do jogo (`game/`, `ui/`, `net/`) trabalha com os tipos deste módulo —
//! `Axis`, `InputFrame`, `Buttons`, `IsoCamera` — e não com `sdl2` nem com
//! `RenderContext3D`.
//!
//! # Por que uma fachada e não chamadas diretas
//!
//! Três motivos concretos, não estéticos:
//!
//! 1. **A engine não tem borda de botão no gamepad.** O jogo precisa de "apertou",
//!    e a derivação tem de morar em um lugar só, senão cada tela inventa a sua.
//! 2. **O `Vec3` da engine não tem aritmética.** A conta é feita em `glam` e
//!    convertida numa fronteira única.
//! 3. **A ABI C pode chegar.** O prompt descreve um `libgiochi.so`; o snapshot
//!    deste repositório é Rust puro. Como o jogo só enxerga esta fachada, trocar a
//!    implementação de baixo (Rust hoje, FFI quando o `giochi-engine-capi` estiver
//!    disponível) não toca no jogo. Ver `DECISIONS.md` (D-002).
//!
//! Note o que a fachada **não** esconde: nada de `unsafe`, nada de `transmute`.
//! Um wrapper "seguro" que precisa de `unsafe` para existir esconde a dívida em
//! vez de pagá-la.

pub mod audio;
pub mod cena;
pub mod input;
pub mod modelos;
pub mod render;
pub mod touch;

pub use audio::AudioBridge;
pub use cena::{Aparencia, Copia};
pub use modelos::{Catalogo, Modelo};
pub use input::{Axis, Buttons, Edges, InputBridge, InputFrame, InputSource};
pub use render::RenderBridge;
pub use touch::{TouchButton, TouchEmulation};
