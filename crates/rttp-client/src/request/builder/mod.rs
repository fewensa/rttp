//! Request builder boundaries.
//!
//! The builder turns a `Request` into a raw wire request. Ownership of the
//! three construction concerns is split across focused modules:
//!
//! - `build_para_and_url` owns URL parameters and query/path rebuilding
//!   (`rebuild_paras`, `rebuild_url`).
//! - `build_header` owns header construction: the request line plus the
//!   auto-added `Host`, `Connection`, `User-Agent`, `Accept`, `Content-Type`,
//!   and `Content-Length` headers.
//! - `build_body_block` owns body construction for raw, binary, url-encoded,
//!   and multipart form-data bodies on the blocking path, including
//!   `build_body_common` shared with the async path.
//! - `build_body_async` owns the async body path (`build_body_async`).
//! - `form_data` owns the multipart form-data wire helpers used by body
//!   construction.
//!
//! `common` holds the shared `RawBuilder` state and the entry points
//! (`raw_request_block` / `raw_request_async`) that call the owned steps in
//! order.

pub use self::common::*;

mod build_body_async;
mod build_body_block;
mod build_header;
mod build_para_and_url;
mod common;
mod form_data;
