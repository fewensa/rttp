rttp
====

A small Rust HTTP workspace with a client crate (`rttp_client`) and a wrapper
crate (`rttp`) that also provides a minimal blocking HTTP server.

## Client

`rttp_client` supports plain HTTP by default. Optional features add async
request APIs and TLS implementations:

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

Direct TCP client connections are opened with `socket2`. SOCKS proxy handshakes
are still delegated to the `socks` crate.
HTTP/1.x chunked responses are decoded by the client, and response trailers are
available through `Response::trailers`, `Response::trailer`, and
`Response::trailer_value`.

### Tested client protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 response parsing | `Content-Length`, chunked transfer coding, chunk extensions, informational responses, bodyless `204`/`304`, duplicate `Set-Cookie`, and framing ambiguity rejection | Not a complete RFC conformance suite |
| HTTP/1.1 request emission | Origin-form requests, absolute-form proxy requests, `CONNECT`, `HEAD`, fixed bodies, streaming chunked uploads, and `Expect: 100-continue` | SOCKS handshakes are delegated to the `socks` crate |
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Trailers | Chunked response trailers are exposed for blocking and async APIs; streaming chunked uploads can send declared request trailers | Application metadata trailers such as `X-Trace` are allowed; pseudo-header, connection-specific, routing, authentication/cookie, and framing trailer fields are rejected |
| Prior-knowledge h2c | With `http2`, direct `socket2` h2c sends GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, and buffered POST, PUT, or PATCH requests, opens at most one request stream, honors initial peer `SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the peer allows zero streams, honors peer-advertised `SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits, accepts only legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, rejects oversized inbound frames when a configured local frame-size limit is exceeded, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, strips HTTP/1.x connection-specific request fields before emission, rejects connection-specific peer response fields, suppresses HEAD response bodies, treats `RST_STREAM` on the active stream as a bounded reset/cancellation signal, acknowledges valid PING frames with matching opaque data, DATA bodies, trailers, HPACK static Huffman strings, bounded large header blocks, padded incoming frames, `GOAWAY` shutdown boundaries, PRIORITY metadata validation without scheduling, HTTP/2-allowed unknown/extension frame ignoring inside this bounded path, reserved stream-id high-bit normalization, and conservative DATA flow control | `CONNECT`, RFC 8441 `:protocol` extended CONNECT metadata, and HTTP/1.1 `Upgrade` handoff requests are rejected deterministically before opening a client socket, and `PUSH_PROMISE`/server push is rejected instead of managed; bounded prior-knowledge h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, proxy tunneling to h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplex scheduling, general multiplexing, priority scheduling, or request bodies for GET, HEAD, DELETE, OPTIONS, or TRACE |

With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a bounded
prior-knowledge h2c request over a direct socket2 TCP connection. It opens at
most one request stream and honors the peer's initial
`SETTINGS_MAX_CONCURRENT_STREAMS`: a value of zero rejects the request before
HEADERS are sent. It also honors the peer's advertised
`SETTINGS_MAX_HEADER_LIST_SIZE` for request metadata: encoded request HEADERS
and trailing HEADERS must stay within that peer boundary before emission, while
peers that do not advertise the setting keep the bounded direct-client default.
It supports GET, HEAD, bodyless DELETE, OPTIONS, or TRACE,
and buffered POST, PUT, or PATCH requests, including non-empty buffered request
bodies as DATA frames for the write methods. GET, HEAD, DELETE, OPTIONS, and
TRACE requests with bodies are rejected; HEAD, bodyless DELETE, OPTIONS, and
TRACE requests do not send request DATA frames, and any HEAD response DATA
frames are consumed without being exposed as a response body. The client
validates `SETTINGS_MAX_FRAME_SIZE` boundaries on both sides of the bounded
h2c handshake. A configured local
`http2_max_frame_size` is advertised only when set, must be in the legal
HTTP/2 range of 16,384 through 16,777,215 bytes, and is used to reject inbound
frame payloads larger than that active local limit. Peer-advertised
`SETTINGS_MAX_FRAME_SIZE` values outside that same legal range reject the
handshake. Legal peer values become the outbound frame boundary, so request
HEADERS, DATA, and trailing HEADERS are split into frames no larger than the
active peer limit while the client remains a single-stream prior-knowledge
path. Before encoding request HEADERS, this bounded h2c client path
strips HTTP/1.x connection-specific fields: `Connection`, `Keep-Alive`,
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
`Upgrade`, and `WWW-Authenticate`. The client
supports HPACK static Huffman
strings and bounded large header blocks via CONTINUATION frames. It uses HPACK
dynamic entries for repeated request header and trailer fields within the
peer's advertised `SETTINGS_HEADER_TABLE_SIZE`. The default peer limit is
4,096 bytes when the peer does not advertise the setting; a peer-advertised
zero disables request dynamic indexing, so request HEADERS and trailers remain
literal encoded. Peer values above 4,096 bytes are valid, but RTTP caps request
dynamic indexing at its 4,096-byte bounded encoder size. Response decoding is
bounded by the local advertised
`SETTINGS_HEADER_TABLE_SIZE`: the client uses the default 4,096-byte decoder
limit unless `ConfigBuilder::http2_header_table_size` configures and advertises
another `u32`-sized value. Incoming HPACK dynamic table size updates from
response HEADERS or trailers may shrink that decoder table, including to zero;
updates above the local advertised limit are rejected. Dynamic table size
updates are HPACK compression state only and do not change
`SETTINGS_MAX_HEADER_LIST_SIZE`, trailer validation, body framing, or the
single-stream h2c policy. Valid response PRIORITY frames and HEADERS
priority fields are validated and ignored as metadata; malformed priority
metadata is rejected, and no priority scheduling is performed. Valid PING
frames are acknowledged with PING ACK frames that carry the same opaque 8-byte
data. Unknown frame types, including extension frames, are ignored only after
the prior-knowledge h2c handshake in this bounded direct-client path where
HTTP/2 permits that behavior; RTTP does not expose extension callbacks or
perform full extension negotiation. Reserved stream identifier high bits are
masked when frames are parsed or written, which normalizes wire framing but
does not add broader multiplex scheduling or persistent session management.
Server push is outside this bounded client path: incoming
`PUSH_PROMISE` frames are rejected deterministically instead of creating or
tracking push state. HTTP/1.1 `CONNECT` tunnel handoff remains a separate
client path;
prior-knowledge h2c `GOAWAY` is treated as a bounded shutdown signal: a
response already completed before `GOAWAY` remains usable, an active stream
continues only when the peer's `last-stream-id` includes it, and a lower
boundary rejects the response deterministically. A `GOAWAY` received before
stream 1 is opened is treated as request refusal and no request HEADERS are
sent. RTTP returns that refusal to the caller instead of retrying on a new
connection; callers that know a request is safe or idempotent must choose any
retry policy themselves. This protocol shutdown boundary is distinct from a
transport-level disconnect, read timeout, write timeout, or TCP reset, which
is reported through the normal socket/error path without an HTTP/2
`last-stream-id` boundary. `RST_STREAM` is likewise bounded to this
prior-knowledge h2c client path: a reset for the active stream is reported as
response cancellation, while malformed reset frames are rejected
deterministically. RTTP does not expose a public cancellation callback API or
retry the request automatically. `CONNECT`, RFC 8441 `:protocol` extended
CONNECT metadata, HTTP/1.1 `Upgrade` handoff requests, and proxy tunneling are
rejected before a client socket is opened. HTTP/1.1 `CONNECT` tunnel handoff
and `Upgrade` remain separate client handoff paths; this h2c path does not
provide WebSocket over h2, proxy h2, tunnel handoff, persistent HTTP/2
sessions, or full RFC 8441 support. TLS ALPN, extension callback APIs, full
extension negotiation, external h2 integration, connection pooling, automatic
retry, server push, full stream state machines, and full HTTP/2 features such
as unbounded multiplex scheduling, general multiplexing, and priority
scheduling are not part of that bounded prior-knowledge client path; RTTP also
does not provide a dynamic policy API for changing h2c frame-size or metadata
limits at runtime.

```rust,no_run
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Server

The `rttp` crate exposes `rttp::Http::server`, which creates a blocking
`HttpServer` listener.

```toml
[dependencies]
rttp = "0.2"
```

```rust,no_run
use std::time::Duration;

use rttp::server::HttpResponse;

fn main() -> std::io::Result<()> {
  let server = rttp::Http::server("127.0.0.1:0")?
    .with_read_timeout(Some(Duration::from_secs(5)))
    .with_write_timeout(Some(Duration::from_secs(5)));
  println!("listening on {}", server.local_addr()?);

  server.accept_one(|request| {
    println!("{} {}", request.method(), request.target());
    HttpResponse::ok("hello")
      .header("Transfer-Encoding", "chunked")
      .header("Trailer", "X-Trace")
      .trailer("X-Trace", "abc")
  })
}
```

Use `HttpServer::bind` directly when you already want the server type,
`HttpServer::local_addr` to read the bound address, `accept_one` for one
connection, and `serve_requests` for a fixed number of sequential connections.
Use `with_read_timeout` and `with_write_timeout` to apply socket-level
timeouts to each accepted connection; pass `None` to leave the corresponding
socket timeout unset. Add `Transfer-Encoding: chunked` to an `HttpResponse` to
write the complete response body with HTTP/1.x chunked transfer framing instead
of an automatic `Content-Length`; response trailers added with
`HttpResponse::trailer` are written after the terminating zero-size chunk. Add a
`Trailer` response header when advertising which trailer fields will follow.
The listener path uses `socket2`.

The server is intentionally small: it handles blocking HTTP/1.x request parsing
for local tests and simple embedded use. It accepts fixed `Content-Length` and
chunked request bodies, exposes chunked request trailers, applies bounded
request head/body validation, handles `HEAD` without writing a response body,
honors `Connection` close/keep-alive semantics across a bounded
`serve_requests` loop, writes response body framing and response trailers
consistently, and accepts `Expect: 100-continue`. On the same socket2 listener,
the accept path detects the HTTP/2 client preface and dispatches prior-knowledge
h2c requests to a minimal bounded handler, including bodyless DELETE, OPTIONS,
and TRACE requests. The server advertises `SETTINGS_MAX_CONCURRENT_STREAMS`
from the active request allowance for that bounded accept path and rejects new
h2c streams once the open-stream count plus completed requests reaches that
allowance. It also advertises and enforces a conservative
`SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata; request HEADERS
and trailing HEADERS can span CONTINUATION frames, but the decoded metadata
remains bounded before handler dispatch. The server advertises the default
16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects peer SETTINGS values outside the
legal HTTP/2 range of 16,384 through 16,777,215 bytes, rejects inbound frames
larger than the active local limit, and splits outbound response HEADERS, DATA,
and trailing HEADERS to the active peer frame-size limit. It preserves the same
HEAD body suppression for prior-knowledge h2c responses.
Incoming padded HEADERS, DATA, and trailer frames are accepted without exposing
padding bytes to handlers, and application trailers such as `X-Trace`,
`X-Upload-Status`, and `X-Upload-Checksum` are preserved on `Request`.
Trailing HEADERS that contain HTTP/2 pseudo-headers are rejected before handler
dispatch. Trailer field names that affect connection state, routing,
authentication/cookies, framing, or payload processing are also rejected,
including `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`,
`Transfer-Encoding`, `Upgrade`, `Host`, `Content-Length`, `Cache-Control`,
`Content-Encoding`, `Content-Range`, `Content-Type`, `Max-Forwards`,
`Authorization`, `Proxy-Authenticate`, `Proxy-Authorization`, `Cookie`,
`Set-Cookie`, and `WWW-Authenticate`. HPACK static Huffman strings, request
dynamic table entries, and bounded large header blocks are carried with
CONTINUATION frames. The server accepts peer `SETTINGS_HEADER_TABLE_SIZE`
values as the outbound response compression allowance and applies later peer
updates before encoding response trailers. If the peer advertises zero, the
server evicts response dynamic entries and writes response HEADERS and trailers
without dynamic indexing. Inbound request and request-trailer decoding stays
bounded to the server's fixed 4,096-byte HPACK dynamic table limit; incoming
dynamic table size updates may shrink that decoder table, including to zero,
but updates above 4,096 bytes are rejected. These table-size boundaries affect
only HPACK compression state, not decoded metadata limits, trailer validation,
handler dispatch, DATA flow control, or multiplex scheduling.
Prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, and `Upgrade`; `TE` is accepted only as `te: trailers`
and other `TE` values are rejected. When serializing h2c responses, the server
strips HTTP/1.x connection-specific response fields and generated HTTP/2
framing fields from HEADERS: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`TE`, `Trailer`, `Transfer-Encoding`, `Upgrade`, and `Content-Length`. H2c
response trailers skip the existing forbidden trailer set, including invalid
pseudo-header-like names, connection-specific, transfer/framing, routing,
authentication, and cookie fields.
Valid standalone PRIORITY frames and HEADERS priority fields are validated and
ignored as metadata; malformed priority metadata is rejected, and request or
response ordering does not use priority scheduling. Valid PING frames on
stream 0 are acknowledged with PING ACK frames that carry the same opaque
8-byte data.
Unknown frame types, including extension frames, are ignored only after the
HTTP/2 preface is accepted in this bounded prior-knowledge h2c server path
where HTTP/2 permits that behavior; RTTP does not expose an extension callback
API or negotiate extensions. Reserved stream identifier high bits are masked
when frames are parsed or written, which normalizes frame identifiers without
adding unbounded multiplexing, session management, or external h2-stack
support.
Server push is outside this bounded server path: inbound `PUSH_PROMISE` frames
are rejected deterministically before handler dispatch instead of attempting
push state management.
When the bounded prior-knowledge h2c server loop finishes, it sends `GOAWAY`
with the last completed stream id so clients have a deterministic shutdown
boundary for already processed streams. If the bounded request allowance is
exhausted while additional streams are already open, the server first sends a
graceful `GOAWAY` boundary and lets streams within that boundary finish; new
streams outside the boundary are refused with `REFUSED_STREAM` and are not
dispatched to the handler. If the peer closes the TCP connection, a read/write
timeout fires, or the socket is reset before `GOAWAY` can be written, that is
transport termination rather than an HTTP/2 graceful shutdown signal and no
additional stream boundary is implied.
Within that same prior-knowledge h2c server path, inbound `RST_STREAM` is a
bounded reset/cancellation signal for the affected stream: reset request
streams are not dispatched to handlers, and reset response streams stop within
the bounded write path. RTTP does not expose a public cancellation callback API,
retry work automatically, keep persistent HTTP/2 sessions, or model a full
HTTP/2 stream state machine around those resets.
The prior-knowledge h2c path does not share the HTTP/1.1 `CONNECT` or
`Upgrade` handoff paths: ordinary h2c `CONNECT` and RFC 8441 extended
`CONNECT` metadata such as `:protocol` are rejected before handler dispatch.
HTTP/1.1 `CONNECT` authority-form requests and `HttpResponse::upgrade` remain
separate handoff paths for caller-owned protocols. TLS ALPN, extension callback
APIs, full extension negotiation, external h2 integration, WebSocket over h2,
proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session
management, full RFC 8441 support, and full HTTP/2 features such as unbounded
multiplexing, unbounded multiplex scheduling, general multiplexing, server
push, and priority scheduling remain outside this bounded prior-knowledge
server path. RTTP does not expose a dynamic policy API for changing the h2c
frame-size or metadata limit at runtime.

It is not a full RFC-covering web server and still does not implement server
TLS or async accept loops.

### Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Application metadata trailers are allowed; trailer names that affect connection state, routing, authentication/cookies, framing, or payload processing are rejected |
| Prior-knowledge h2c | The same `socket2` listener detects the HTTP/2 preface, validates SETTINGS including legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, advertises the default 16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects inbound frames above the active local limit, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the bounded active stream allowance, enforces that allowance before dispatching new streams, advertises and enforces a conservative `SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, serves bounded prior-knowledge streams including bodyless DELETE, OPTIONS, and TRACE, handles HEAD without response DATA, rejects connection-specific request fields before handler dispatch, strips connection-specific response fields during h2c serialization, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges valid PING frames with matching opaque data, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman fields and bounded CONTINUATION header blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | Ordinary `CONNECT`, RFC 8441 extended `CONNECT`/`:protocol`, and `PUSH_PROMISE` are rejected deterministically before handler dispatch; HTTP/1.1 `CONNECT` and `Upgrade` remain separate handoff paths; bounded prior-knowledge h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, WebSocket over h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full RFC 8441 support, full stream state machine, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, priority scheduling, or full HTTP/2 server feature set |
