//! Request builder modules that turn a `Request` into an outbound `RawRequest`.
//!
//! Ownership of the build steps is split across modules:
//!
//! - `common` defines `RawBuilder`, the shared builder type, and the entry
//!   points that sequence the build: URL parameters, URL, body, then header.
//! - `build_para_and_url` owns URL parameters and URL reconstruction, merging
//!   request and form-data parameters into the URL and applying path segments.
//! - `build_header` owns header construction, serializing the request line and
//!   headers and auto-adding Host, Connection, User-Agent, Accept,
//!   Content-Type, and Content-Length.
//! - `build_body_block` owns body construction for the blocking path (raw,
//!   binary, and form-urlencoded bodies) plus the shared body helpers.
//! - `build_body_async` owns body construction for the async path, including
//!   multipart form-data bodies.
//! - `form_data` owns the multipart/form-data serialization helpers used by
//!   both body builders.

pub use self::common::*;

mod build_body_async;
mod build_body_block;
mod build_header;
mod build_para_and_url;
mod common;
mod form_data;
