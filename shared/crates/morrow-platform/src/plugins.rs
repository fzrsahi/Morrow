//! The TypeScript extensions, compiled into the binary (#234 batch 4b).
//!
//! Copied from `macos/Sources/Morrow/Resources/` at build time rather than read from
//! it: the Linux and Windows packages do not ship the macOS tree, and a plugin read
//! from a path that only exists in a source checkout is a plugin that works for
//! developers and nobody else.
//!
//! The copies are **not** byte-identical to the macOS originals — the install rewrites
//! the bridge line for the platform it is running on (see
//! `morrow_core::installer::extension`). Keeping the originals here and rewriting at
//! install time means one artefact per extension, not one per platform.

pub const PI: &str = include_str!("plugins/morrow-pi.ts");
pub const OMP: &str = include_str!("plugins/morrow-omp.ts");
pub const OPENCLAW: &str = include_str!("plugins/morrow-openclaw.ts");
