//! Internal, transport-independent HTTP wire primitives shared by rttp crates.
//!
//! This crate is intentionally unpublished. It owns protocol syntax and framing
//! validation only; client and server application policy remains in its callers.

pub mod http1;
pub mod server_timing;
pub mod www_authenticate;
