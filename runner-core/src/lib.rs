//! Platform-independent engine and game logic for Rust Runner OS.
//!
//! Everything here is `no_std` so the kernel can link it directly, while
//! `cargo test -p runner-core` runs the unit tests on the host with `std`.
#![cfg_attr(not(test), no_std)]

pub mod audio;
pub mod engine;
pub mod game;
