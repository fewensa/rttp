//! Internal, transport-independent HTTP wire primitives shared by rttp crates.
//!
//! This crate is intentionally unpublished. It owns protocol syntax and framing
//! validation only; client and server application policy remains in its callers.

pub mod accept_patch;
pub mod accept_post;
pub mod alt_svc;
pub mod cache_control;
pub mod clear_site_data;
pub mod cookie;
pub mod digest;
pub mod forwarded;
pub mod http1;
mod media_type;
pub mod priority;
pub mod range;
pub mod server_timing;
pub mod trailer;
pub mod www_authenticate;
