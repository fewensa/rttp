//! Internal, transport-independent HTTP wire primitives shared by rttp crates.
//!
//! This crate is intentionally unpublished. It owns protocol syntax and framing
//! validation only; client and server application policy remains in its callers.

pub mod accept_patch;
pub mod accept_post;
pub mod access_control_allow_headers;
pub mod access_control_allow_methods;
pub mod access_control_allow_origin;
pub mod access_control_expose_headers;
pub mod access_control_max_age;
pub mod alt_svc;
pub mod cache_control;
pub mod clear_site_data;
pub mod client_hints;
pub mod cookie;
pub mod cross_origin_resource_policy;
pub mod digest;
pub mod entity_tag;
pub mod fetch_metadata;
pub mod forwarded;
pub mod http1;
mod media_type;
pub mod origin;
pub mod prefer;
pub mod priority;
pub mod range;
pub mod rate_limit;
pub mod referrer_policy;
pub mod server_timing;
pub mod sunset;
pub mod timing_allow_origin;
pub mod trailer;
pub mod vary;
pub mod www_authenticate;
