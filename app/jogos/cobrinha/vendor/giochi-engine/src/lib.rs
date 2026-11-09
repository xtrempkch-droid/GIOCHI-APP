//! SDL2 platform and frame-counter support used by the vendored 3D renderer.
//!
//! The game uses the separate GLES renderer; only these shared platform types are
//! exposed here to avoid linking the unused 2D, image, and audio subsystems.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod platform;

#[path = "perf/frame_stats.rs"]
mod frame_stats;

pub mod perf {
    //! Frame counter types shared with the 3D renderer.

    pub use crate::frame_stats::{FrameCounters, FrameStats, FrameTimeSummary};

    /// Number of frame samples retained by the profiler.
    pub const FRAME_WINDOW: usize = 60;
}

/// Semantic version of the vendored engine snapshot.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
