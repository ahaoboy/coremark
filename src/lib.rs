//! CoreMark 1.0 — a Rust port of the EEMBC CoreMark benchmark.
//!
//! The algorithms mirror the reference C implementation (`core_list_join.c`,
//! `core_matrix.c`, `core_state.c`, `core_util.c` and the `core_main.c`
//! framework) so the reported CRCs match bit-for-bit.
//!
//! The crate is usable in two ways:
//!
//! * **As a library** — build a [`Config`], call [`run`] and render the
//!   returned [`Results`] (it implements [`Display`](fmt::Display) with the
//!   reference report).
//! * **As a command-line program** — enabled by the default `cli` feature,
//!   which pulls in `clap`. Building with `--no-default-features` yields a
//!   dependency-free library only.

pub mod list;
pub mod matrix;
pub mod state;
pub mod types;
pub mod util;

mod bench;

pub use bench::{Config, Results, run};

/// Rust compiler version, captured at build time.
pub const COMPILER_VERSION: &str = env!("COREMARK_RUSTC_VERSION");
/// Build profile, captured at build time.
pub const COMPILER_FLAGS: &str = concat!("profile=", env!("COREMARK_PROFILE"));
