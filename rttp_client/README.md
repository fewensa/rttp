rttp_client
===========

`rttp_client` is a small HTTP client crate. Plain HTTP is available by default;
optional features add async request APIs and TLS implementations.

| name | comment |
|------|---------|
| async | Async request APIs |
| http2 | Bounded prior-knowledge h2c over direct `socket2` TCP connections |
| tls-native | HTTPS with `native-tls` |
| tls-rustls | HTTPS with `rustls` |

```toml
[dependencies]
rttp_client = "0.2"
```

```toml
[dependencies]
rttp_client = { version = "0.2", features = ["async", "tls-rustls"] }
```

Direct TCP connections use `socket2`. SOCKS proxy handshakes remain delegated to
the `socks` crate.
HTTP/1.x chunked responses are decoded, and response trailers are exposed
through `Response::trailers`, `Response::trailer`, and
`Response::trailer_value` for both blocking and async request APIs.

## Tested protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 response parsing | `Content-Length`, chunked transfer coding, chunk extensions, informational responses, bodyless `204`/`304`, duplicate `Set-Cookie`, and framing ambiguity rejection | Not a complete RFC conformance suite |
| HTTP/1.1 request emission | Origin-form requests, absolute-form proxy requests, `CONNECT`, `HEAD`, fixed bodies, streaming chunked uploads, and `Expect: 100-continue` | SOCKS handshakes are delegated to the `socks` crate |
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Trailers | Chunked response trailers are exposed for blocking and async APIs; streaming chunked uploads can send declared request trailers | Application metadata trailers such as `X-Trace` are allowed; pseudo-header, connection-specific, routing, authentication/cookie, and framing trailer fields are rejected |
| Prior-knowledge h2c | With `http2`, direct `socket2` h2c sends GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, and buffered POST, PUT, or PATCH requests, opens at most one request stream, honors initial peer `SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the peer allows zero streams, honors peer-advertised `SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits, strips HTTP/1.x connection-specific request fields before emission, rejects connection-specific peer response fields, suppresses HEAD response bodies, treats `RST_STREAM` on the active stream as a bounded reset/cancellation signal, acknowledges valid PING frames with matching opaque data, DATA bodies, trailers, HPACK static Huffman strings, dynamic entries within peer settings, bounded large header blocks, padded incoming frames, `GOAWAY` shutdown boundaries, PRIORITY metadata validation without scheduling, HTTP/2-allowed unknown/extension frame ignoring inside this bounded path, reserved stream-id high-bit normalization, and conservative DATA flow control | `CONNECT` is rejected deterministically before opening a client socket, and `PUSH_PROMISE`/server push is rejected instead of managed; bounded prior-knowledge h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, proxy tunneling to h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplex scheduling, general multiplexing, priority scheduling, or request bodies for GET, HEAD, DELETE, OPTIONS, or TRACE |

With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a bounded
prior-knowledge h2c request over a direct socket2 TCP connection. It opens at
most one request stream and honors the peer's initial
`SETTINGS_MAX_CONCURRENT_STREAMS`: a value of zero rejects the request before
HEADERS are sent. It also honors the peer's advertised
`SETTINGS_MAX_HEADER_LIST_SIZE` for request metadata: encoded request HEADERS
and trailing HEADERS must stay within that peer boundary before emission, while
peers that do not advertise the setting keep the bounded direct-client default.
It supports GET, HEAD, bodyless DELETE, OPTIONS, or TRACE,
and buffered POST, PUT, or PATCH requests. Non-empty buffered request bodies
are sent as DATA frames for the write methods; GET, HEAD, DELETE, OPTIONS, and
TRACE requests with bodies are rejected. HEAD, bodyless DELETE, OPTIONS, and
TRACE requests do not send request DATA frames, and any HEAD response DATA
frames are consumed without being exposed as a response body. Before encoding
request HEADERS, this bounded h2c path strips
HTTP/1.x connection-specific fields: `Connection`, `Keep-Alive`,
`Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, `TE`, `Trailer`, `Host`,
and any field named by a `Connection` token. Peer response HEADERS are rejected
when they contain `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`,
`Transfer-Encoding`, or `Upgrade`. Application request trailers such as
`X-Trace`, `X-Upload-Status`, or `X-Upload-Checksum` are valid in this bounded
h2c path and are encoded as trailing HEADERS after request DATA. Configured
request trailers are rejected before emission when their field name is invalid
or reserved for connection/framing/routing behavior: `Connection`,
`Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, `Content-Length`, `Host`, `Proxy-Authenticate`, or
`Proxy-Authorization`. Peer response trailers use the existing
forbidden-trailer validation for invalid pseudo-header-like names,
connection-specific, routing, authentication/cookie, and framing fields such
as `Authorization`, `Connection`, `Content-Length`, `Cookie`, `Host`,
`Keep-Alive`, `Proxy-Authenticate`, `Proxy-Authorization`,
`Proxy-Connection`, `Set-Cookie`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, and `WWW-Authenticate`.
HPACK static Huffman strings and bounded large header
blocks are supported, repeated request header and trailer fields can use HPACK
dynamic entries within the peer's advertised table size, and incoming dynamic
table entries, table-size updates, padded HEADERS, DATA, and trailer frames are
decoded without exposing padding bytes. Valid response PRIORITY frames and
HEADERS priority fields are validated and ignored as metadata; malformed
priority metadata is rejected, and no priority scheduling is performed. Valid
PING frames are acknowledged with PING ACK frames that carry the same opaque
8-byte data.
Unknown frame types, including extension frames, are ignored only after the
prior-knowledge h2c handshake in this bounded direct-client path where HTTP/2
permits that behavior; RTTP does not expose extension callbacks or perform
full extension negotiation. Reserved stream identifier high bits are masked
when frames are parsed or written, which normalizes wire framing but does not
add broader multiplex scheduling or persistent session management.
Server push is outside this bounded client path: incoming `PUSH_PROMISE` frames
are rejected deterministically instead of creating or tracking push state.
HTTP/1.1 `CONNECT` tunnel handoff remains a separate client path;
prior-knowledge h2c `GOAWAY` is treated as a bounded shutdown signal:
completed responses remain usable, active responses continue only when the
peer's `last-stream-id` includes the stream, and lower boundaries reject the
response deterministically. `RST_STREAM` is likewise bounded to this
prior-knowledge h2c client path: a reset for the active stream is reported as
response cancellation, while malformed reset frames are rejected
deterministically. RTTP does not expose a public cancellation callback API or
retry the request automatically. `CONNECT` and proxy tunneling are rejected
before a client socket is opened. TLS ALPN, extension callback APIs, full
extension negotiation, external h2 integration, proxy h2, tunnel handoff,
connection pooling, persistent HTTP/2 session management, automatic retry,
server push, full stream state machines, and full HTTP/2 features such as
unbounded multiplex scheduling, general multiplexing, and priority scheduling
are not part of that bounded prior-knowledge client path. RTTP does not expose
a dynamic policy API for changing h2c metadata limits at runtime.

## Examples

```rust,no_run
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

```rust,no_run
use rttp_client::HttpClient;
use rttp_client::types::Proxy;

let response = HttpClient::new()
  .post()
  .url("http://127.0.0.1:8080/messages")
  .content_type("application/json")
  .raw(r#"{"from":"rttp"}"#)
  .proxy(Proxy::http("127.0.0.1", 1081))
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

```rust,no_run
# #[cfg(feature = "async")]
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .rasync()
  .await?;
# Ok(())
# }
```
