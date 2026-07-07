//! `rttp_client` is a small HTTP client crate. Plain HTTP is available by
//! default; optional features add async request APIs and TLS implementations.
//!
//! | name | comment |
//! |------|---------|
//! | async | Async request APIs |
//! | http2 | Prior-knowledge h2c over direct `socket2` TCP connections |
//! | tls-native | HTTPS with `native-tls` |
//! | tls-rustls | HTTPS with `rustls` |
//!
//! Direct TCP connections use `socket2`. SOCKS proxy handshakes remain delegated
//! to the `socks` crate.
//! With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a
//! minimal prior-knowledge h2c request over a direct socket2 TCP connection.
//! TLS ALPN and full HTTP/2 multiplexing are not part of that path.
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
