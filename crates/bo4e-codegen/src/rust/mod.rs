//! Rust output generators.
//!
//! Two flavours under their own Cargo features:
//! - `rust-plain` → loose Rust files for embedding into a consumer crate
//! - `rust-crate` → full self-contained Cargo crate

#[cfg(feature = "rust-plain")]
pub mod plain;

#[cfg(feature = "rust-crate")]
pub mod crate_;
