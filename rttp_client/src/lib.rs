//! `rttp_client` is a small HTTP client crate. Plain HTTP is available by
//! default; optional features add async request APIs and TLS implementations.
//!
//! | name | comment |
//! |------|---------|
//! | async | Async request APIs |
//! | http2 | Bounded prior-knowledge h2c over direct `socket2` TCP connections |
//! | tls-native | HTTPS with `native-tls` |
//! | tls-rustls | HTTPS with `rustls` |
//!
//! Direct TCP connections use `socket2`. SOCKS proxy handshakes remain delegated
//! to the `socks` crate.
//! With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a
//! bounded prior-knowledge h2c request over a direct socket2 TCP connection.
//! It opens at most one stream and validates `SETTINGS_MAX_FRAME_SIZE` on both
//! sides of the handshake. A configured local `http2_max_frame_size` is
//! advertised only when set, must be in the legal HTTP/2 range of 16,384
//! through 16,777,215 bytes, and rejects inbound frame payloads larger than
//! that active local limit. Peer-advertised values outside the same range
//! reject the handshake, while legal peer values are used to split outbound
//! request HEADERS, DATA, and trailing HEADERS.
//! It decodes incoming padded HEADERS, DATA, and trailer frames without
//! exposing padding bytes, including response HPACK dynamic table entries.
//! `GOAWAY` is treated as a protocol shutdown boundary: completed responses
//! remain usable, an active stream may finish only when the peer's
//! `last-stream-id` includes stream 1, and pre-stream `GOAWAY` refuses the
//! request before request HEADERS are sent. RTTP reports those conditions to
//! the caller and does not retry automatically; transport disconnects remain
//! ordinary socket errors without an HTTP/2 stream boundary. TLS ALPN, proxy
//! h2, full HTTP/2 multiplexing, dynamic policy APIs, and priority scheduling
//! are not part of that single-stream path.
//!
//! ```rust,no_run
//! use rttp_client::HttpClient;
//!
//! let response = HttpClient::new()
//!   .get()
//!   .url("http://127.0.0.1:8080/health")
//!   .emit()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```rust,no_run
//! use rttp_client::HttpClient;
//! use rttp_client::types::Proxy;
//!
//! let response = HttpClient::new()
//!   .post()
//!   .url("http://127.0.0.1:8080/messages")
//!   .content_type("application/json")
//!   .raw(r#"{"from":"rttp"}"#)
//!   .proxy(Proxy::http("127.0.0.1", 1081))
//!   .emit()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ```rust,no_run
//! # #[cfg(feature = "async")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use rttp_client::HttpClient;
//!
//! let response = HttpClient::new()
//!   .get()
//!   .url("http://127.0.0.1:8080/health")
//!   .rasync()
//!   .await?;
//! # Ok(())
//! # }
//! ```

pub use self::client::*;
pub use self::config::*;
#[cfg(feature = "async")]
pub use self::connection::{
  async_streaming_response_after_header, AsyncResponseBodyReader, AsyncStreamingResponse,
};
pub use self::connection::{ConnectionReader, ResponseBodyReader, StreamingResponse};

mod client;
mod config;
mod connection;
#[cfg(feature = "http2")]
pub mod http2;
mod request;

pub mod error;
pub mod response;
pub mod types;
