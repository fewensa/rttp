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
//! sides of the handshake. A configured local `H2cClientPolicy::max_frame_size`
//! is advertised only when set, must be in the legal HTTP/2 range of 16,384
//! through 16,777,215 bytes, and rejects inbound frame payloads larger than
//! that active local limit. Peer-advertised values outside the same range
//! reject the handshake, while legal peer values are used to split outbound
//! request HEADERS, DATA, and trailing HEADERS.
//! Large outbound request HEADERS and trailing HEADERS are fragmented as
//! HEADERS plus CONTINUATION frames when the encoded HPACK block exceeds the
//! active peer frame-size limit. Inbound response HEADERS and trailing HEADERS
//! may span CONTINUATION frames, which are reassembled before HPACK decoding
//! and metadata validation. The client rejects orphan CONTINUATION frames,
//! wrong-stream fragments, interleaved frames before `END_HEADERS`, and EOF
//! before a pending header block closes.
//! The client advertises `SETTINGS_ENABLE_PUSH = 0` so peers see server push
//! disabled, validates received `SETTINGS_ENABLE_PUSH` values as only `0` or
//! `1`, and rejects any incoming `PUSH_PROMISE` frame instead of creating or
//! tracking push state.
//! Inbound PING without ACK on stream 0 and exactly 8 octets is acknowledged
//! with matching opaque data. Inbound PING ACK is ignored for this bounded
//! path. PING with a non-zero stream id or payload length other than 8 is
//! malformed and rejected. This acknowledgement path does not add keepalive
//! timers, automatic client- or server-initiated PING policy, retry/replay, a
//! full session manager, or a full multiplex scheduler.
//! `HttpClient::http2_extended_connect(protocol)` is the only public RFC 8441
//! entry point on this bounded path. It is prior-knowledge h2c only over the
//! direct `socket2` transport boundary: the client advertises
//! `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1`, emits `:method CONNECT` with
//! `:protocol`, `:scheme`, `:authority`, and `:path`, and returns the peer's
//! response through the normal `Response` API. Ordinary `CONNECT`,
//! header-configured `:protocol` metadata, HTTP/1.1 `Upgrade` handoff requests,
//! proxies, request bodies, and request trailers are rejected before the h2c
//! request is sent.
//! It decodes incoming padded HEADERS, DATA, and trailer frames without
//! exposing padding bytes, including response HPACK dynamic table entries.
//! Peer `SETTINGS_HEADER_TABLE_SIZE` bounds outbound request dynamic indexing;
//! a peer value of zero disables request dynamic indexing for HEADERS and
//! trailers. Response decoding uses the locally advertised HPACK dynamic table
//! limit, defaulting to 4,096 bytes unless
//! `H2cClientPolicy::header_table_size` configures another `u32`-sized value
//! through `HttpClient::h2c_policy`. Incoming dynamic table size updates may
//! shrink that decoder table,
//! including to zero, but updates above the local advertised limit are
//! rejected. These boundaries affect HPACK compression state only and do not
//! change trailer validation, body framing, DATA flow control, or the
//! single-stream h2c policy.
//! `GOAWAY` is treated as a protocol shutdown boundary: completed responses
//! remain usable, an active stream may finish only when the peer's
//! `last-stream-id` includes stream 1, and pre-stream `GOAWAY` refuses the
//! request before request HEADERS are sent. RTTP reports those conditions to
//! the caller and does not retry automatically; transport disconnects remain
//! ordinary socket errors without an HTTP/2 stream boundary. This is not a full
//! WebSocket-over-h2 implementation, arbitrary tunnel scheduler, or general
//! multiplexing guarantee beyond the bounded single-stream path, and it does
//! not change HTTP/1.1 `CONNECT` or `Upgrade` semantics. TLS ALPN, proxy h2,
//! full extension negotiation, full HTTP/2 multiplexing, dynamic policy APIs,
//! server push, and priority scheduling are not part of that path.
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
//! # #[cfg(feature = "http2")]
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use rttp_client::HttpClient;
//!
//! let response = HttpClient::new()
//!   .get()
//!   .url("http://127.0.0.1:8080/chat")
//!   .http2_extended_connect("websocket")
//!   .emit_http2_prior_knowledge()?;
//! # Ok(())
//! # }
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
pub use rttp_protocol::fetch_metadata::{
  SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};
pub use rttp_protocol::trace_context::{
  TraceParent, TraceParentParseError, TraceState, TraceStateMember, TraceStateParseError,
};
pub use rttp_protocol::upgrade_insecure_requests::{
  UpgradeInsecureRequests, UpgradeInsecureRequestsParseError,
};

mod client;
mod config;
mod connection;
#[cfg(feature = "http2")]
pub mod http2;
mod request;

pub mod error;
pub mod response;
pub mod types;
