//! `rttp` wraps `rttp_client` behind optional client features and provides a
//! small blocking HTTP server for local tests and simple embedded use.
//!
//! The server accepts HTTP/1.x on a `socket2` listener and also detects the
//! HTTP/2 client preface for bounded prior-knowledge h2c or a valid
//! `Upgrade: h2c` request for the same bounded h2c handler. That h2c server
//! path advertises the default 16,384-byte
//! `SETTINGS_MAX_FRAME_SIZE`, accepts only legal peer
//! `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes,
//! rejects inbound frame payloads above the active local limit, and splits
//! outbound response HEADERS, DATA, and trailing HEADERS to the active peer
//! frame-size limit. Peer `SETTINGS_HEADER_TABLE_SIZE` values bound response
//! dynamic indexing, and a peer value of zero keeps response HEADERS and
//! trailers literal encoded. Request and request-trailer HPACK decoding stays
//! bounded to the server's fixed 4,096-byte dynamic table limit; incoming
//! dynamic table size updates may shrink that table, including to zero, but
//! updates above 4,096 bytes are rejected. Those table-size boundaries affect
//! HPACK compression state only, not trailer validation or request dispatch.
//! Large request HEADERS and trailing HEADERS may arrive as HEADERS plus
//! CONTINUATION fragments; the server reassembles the HPACK block before
//! decoding and header-list validation. Outbound response HEADERS and trailing
//! HEADERS are fragmented to the active peer frame-size limit. CONTINUATION
//! ordering is strict: orphan CONTINUATION frames, stream-0 CONTINUATION,
//! wrong-stream fragments, interleaved frames before `END_HEADERS`, and EOF
//! before `END_HEADERS` are rejected before handler dispatch.
//! Peer `SETTINGS_ENABLE_PUSH` values are validated as only `0` or `1`; any
//! other value rejects the bounded h2c handshake. `PUSH_PROMISE` frames are
//! rejected before handler dispatch, and this path does not implement
//! server-side push state.
//! It remains a bounded prior-knowledge server path: it can accept multiple
//! open streams only up to the advertised active-stream allowance, uses
//! synchronous response writes, and does not provide full multiplex scheduling,
//! persistent HTTP/2 session management, dynamic policy APIs, extension
//! callbacks, full extension negotiation, TLS ALPN, server push, proxy h2,
//! tunnel handoff, or a full
//! HTTP/2 server feature set.
//!
//! With the `client` or `http2` feature enabled, the wrapper exposes the
//! `rttp_client` bounded prior-knowledge h2c client behavior. The client opens
//! at most one stream, advertises `SETTINGS_ENABLE_PUSH = 0`, validates
//! received `SETTINGS_ENABLE_PUSH` values as only `0` or `1`, validates the
//! same legal `SETTINGS_MAX_FRAME_SIZE` range, splits outbound request HEADERS,
//! DATA, and trailing HEADERS to the peer frame-size limit, and rejects inbound
//! oversized frames when a local `http2_max_frame_size` is configured.
//! Response HEADERS and trailing HEADERS may span CONTINUATION frames, which
//! are reassembled before HPACK decoding and metadata validation. The client
//! rejects orphan, wrong-stream, interrupted, or incomplete CONTINUATION
//! sequences before returning a response.
//! The peer's
//! `SETTINGS_HEADER_TABLE_SIZE` bounds request dynamic indexing, including
//! disabling request dynamic indexing at zero. Response decoding uses the
//! locally advertised HPACK dynamic table limit, defaulting to 4,096 bytes
//! unless client configuration advertises another `u32`-sized value.
//! Incoming `PUSH_PROMISE` frames are rejected instead of creating or tracking
//! push state, and full extension negotiation remains outside this bounded
//! client path.

pub struct Http {}

pub mod server;

impl Http {
  #[cfg(feature = "client")]
  pub fn client() -> rttp_client::HttpClient {
    rttp_client::HttpClient::new()
  }

  pub fn server<A>(addr: A) -> std::io::Result<server::HttpServer>
  where
    A: std::net::ToSocketAddrs,
  {
    server::HttpServer::bind(addr)
  }
}
