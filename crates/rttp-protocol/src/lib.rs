//! Internal, transport-independent HTTP wire primitives shared by rttp crates.
//!
//! This crate is intentionally unpublished. It owns protocol syntax and framing
//! validation only; client and server application policy remains in its callers.
//!
//! Each individual header module owns the parsing and formatting rules for its
//! specific header type, including bounded value parsing, wire formatting, and
//! the limits and error types that apply to that header.

pub mod accept_patch;
pub mod accept_post;
pub mod accept_ranges;
pub mod access_control_allow_headers;
pub mod access_control_allow_methods;
pub mod access_control_allow_origin;
pub mod access_control_expose_headers;
pub mod access_control_max_age;
pub mod access_control_request_headers;
pub mod access_control_request_method;
pub mod age;
pub mod allow;
pub mod alt_svc;
pub mod authentication_info;
pub mod cache_control;
pub mod clear_site_data;
pub mod client_hints;
pub mod connection;
pub mod content_encoding;
pub mod content_length;
pub mod content_location;
pub mod content_type;
pub mod cookie;
pub mod cross_origin_embedder_policy;
pub mod cross_origin_embedder_policy_report_only;
pub mod cross_origin_opener_policy;
pub mod cross_origin_resource_policy;
pub mod digest;
pub mod entity_tag;
pub mod fetch_metadata;
pub mod forwarded;
pub mod from;
pub mod host;
pub mod http1;
pub mod location;
mod media_type;
pub mod no_vary_search;
pub mod origin;
pub mod prefer;
pub mod priority;
pub mod proxy_authentication_info;
pub mod range;
pub mod rate_limit;
pub mod referer;
pub mod referrer_policy;
pub mod server_timing;
pub mod signature;
pub mod signature_input;
pub mod strict_transport_security;
pub mod sunset;
pub mod timing_allow_origin;
pub mod trailer;
pub mod transfer_encoding;
pub mod upgrade;
pub mod vary;
pub mod want_content_digest;
pub mod want_repr_digest;
pub mod warning;
pub mod www_authenticate;
pub mod x_content_type_options;
pub mod x_frame_options;
