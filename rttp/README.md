rttp
====

`rttp` wraps `rttp_client` behind optional client features and provides a small
blocking HTTP server for local tests and simple embedded use.

## Server

Create a listener with `rttp::Http::server` or call `HttpServer::bind` directly.
The server listener is built with `socket2`.

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
      .header("Trailer", "X-Trace, X-Signature")
      .trailer("X-Trace", "abc")
      .trailer("X-Signature", "signed")
  })
}
```

`HttpServer::local_addr` returns the bound address, which is useful when binding
to port `0` in tests. `HttpServer::accept_one` serves one connection.
`HttpServer::serve_requests` serves a fixed number of sequential connections on
the same listener. `HttpServer::with_read_timeout` and
`HttpServer::with_write_timeout` apply socket-level timeouts to each accepted
connection; pass `None` to leave the corresponding socket timeout unset.

Add `Transfer-Encoding: chunked` to an `HttpResponse` to write the complete
response body with HTTP/1.x chunked transfer framing instead of an automatic
`Content-Length` when the response status permits a message body. Response
trailers added with `HttpResponse::trailer` are written after the terminating
zero-size chunk, and can be inspected before serialization with
`HttpResponse::trailers` or `HttpResponse::trailer_value`. Add a `Trailer`
response header when advertising which trailer fields will follow.

The server currently parses blocking HTTP/1.x requests for local tests and
simple embedded use. It supports fixed `Content-Length` and chunked request
bodies, preserves chunked request trailers on `Request`, bounds request
head/body parsing, handles `HEAD` without writing a response body, honors
`Connection` close/keep-alive semantics across a bounded `serve_requests` loop,
writes response body framing and response trailers consistently, and accepts
`Expect: 100-continue`. On the same socket2 listener, the accept path detects
the HTTP/2 client preface and dispatches prior-knowledge h2c requests to a
minimal bounded handler, including bodyless DELETE, OPTIONS, and TRACE
requests. The server advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the
active request allowance for that bounded accept path and rejects new h2c
streams once the open-stream count plus completed requests reaches that
allowance. The h2c path handles `HEAD` without writing response DATA frames.
The server validates peer `SETTINGS_ENABLE_PUSH` values as only `0` or `1`;
any other value rejects the bounded h2c handshake. The server also advertises
and enforces a conservative
`SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata; request HEADERS
and trailing HEADERS can span CONTINUATION frames, but decoded metadata remains
bounded before handler dispatch. It advertises the default 16,384-byte
`SETTINGS_MAX_FRAME_SIZE`, rejects peer SETTINGS values outside the legal
HTTP/2 range of 16,384 through 16,777,215 bytes, rejects inbound frames larger
than the active local limit, and splits outbound response HEADERS, DATA, and
trailing HEADERS to the active peer frame-size limit.
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
CONTINUATION frames. Peer `SETTINGS_HEADER_TABLE_SIZE` values bound response
dynamic indexing: the server uses the peer's latest advertised table size when
encoding response HEADERS and applies later updates before response trailers.
A peer value of zero evicts response dynamic entries and keeps response HEADERS
and trailers literal encoded. Inbound request and request-trailer decoding is
bounded to the server's fixed 4,096-byte HPACK dynamic table limit; incoming
dynamic table size updates may shrink that table, including to zero, but
updates above 4,096 bytes are rejected. These HPACK limits affect compression
state only and do not change `SETTINGS_MAX_HEADER_LIST_SIZE`, trailer
validation, DATA flow control, handler dispatch, or multiplex scheduling. Valid
prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, and `Upgrade`; `TE` is accepted only as `te: trailers`
and other `TE` values are rejected. When serializing h2c responses, the server
strips HTTP/1.x connection-specific response fields and generated HTTP/2
framing fields from HEADERS: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`TE`, `Trailer`, `Transfer-Encoding`, `Upgrade`, and `Content-Length`. H2c
response trailers skip the existing forbidden trailer set, including invalid
pseudo-header-like names, connection-specific, transfer/framing, routing,
authentication, and cookie fields. Valid
standalone PRIORITY frames and HEADERS priority fields are validated and ignored
as metadata; malformed priority metadata is rejected, and request or response
ordering does not use priority scheduling. Multiple prior-knowledge h2c request
streams may be open on one connection only up to the advertised bounded
active-stream allowance from `SETTINGS_MAX_CONCURRENT_STREAMS`; this is not
general multiplex scheduling or full persistent HTTP/2 session management.
Valid PING frames on stream 0 are acknowledged with PING ACK frames that carry
the same opaque 8-byte data.
Unknown frame types, including extension frames, are ignored only after the
HTTP/2 preface is accepted in this bounded prior-knowledge h2c server path
where HTTP/2 permits that behavior; RTTP does not expose an extension callback
API or negotiate extensions. Reserved stream identifier high bits are masked
when frames are parsed or written, which normalizes frame identifiers without
adding unbounded multiplexing, session management, or external h2-stack
support.
Server push is outside this bounded server path: inbound `PUSH_PROMISE` frames
are rejected deterministically before handler dispatch instead of attempting
push state management, and RTTP does not implement server-side push state even
when a peer sends `SETTINGS_ENABLE_PUSH = 1`.
Responses are still written synchronously as requests complete. The bounded
h2c path supports conservative DATA flow-control for prior-knowledge use. It
uses `GOAWAY` as a bounded shutdown signal when the loop ends, reporting the
last completed stream id so clients can apply a deterministic stream boundary.
If the bounded request allowance is exhausted while additional streams are
already open, the server first sends a graceful `GOAWAY` boundary and lets
streams within that boundary finish; new streams outside the boundary are
refused with `REFUSED_STREAM` and are not dispatched to the handler. If the
peer closes the TCP connection, a read/write timeout fires, or the socket is
reset before `GOAWAY` can be written, that is transport termination rather
than an HTTP/2 graceful shutdown signal and no additional stream boundary is
implied.
Within that same prior-knowledge h2c server path, inbound `RST_STREAM` is a
bounded reset/cancellation signal for the affected stream: reset request
streams are not dispatched to handlers, and reset response streams stop within
the bounded write path. RTTP does not expose a public cancellation callback API,
retry work automatically, keep persistent HTTP/2 sessions, or model a full
HTTP/2 stream state machine around those resets.
It does not share the HTTP/1.1 `CONNECT` or `Upgrade` handoff paths:
ordinary prior-knowledge h2c `CONNECT` and RFC 8441 extended `CONNECT`
metadata such as `:protocol` are rejected before handler dispatch.
HTTP/1.1 `CONNECT` authority-form requests and `HttpResponse::upgrade` remain
separate handoff paths for caller-owned protocols.

The server is intentionally not a full RFC-covering web server and still does
not implement server TLS, TLS ALPN, extension callback APIs, full extension
negotiation, external h2 integration, WebSocket over h2, proxy h2, h2c tunnel
handoff, connection pooling, persistent HTTP/2 session management, automatic
retry, public cancellation callbacks, dynamic policy APIs, full RFC 8441
support, full stream state machines, full HTTP/2 features such as unbounded
multiplexing, unbounded multiplex scheduling, general multiplexing, server
push, and priority scheduling, or async accept loops.

## Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Application metadata trailers are allowed; trailer names that affect connection state, routing, authentication/cookies, framing, or payload processing are rejected |
| Prior-knowledge h2c | The same `socket2` listener detects the HTTP/2 preface, validates SETTINGS including legal `SETTINGS_ENABLE_PUSH` values of only `0` or `1` and legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, advertises the default 16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects inbound frames above the active local limit, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the bounded active stream allowance, enforces that allowance before dispatching new streams, advertises and enforces a conservative `SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, serves bounded prior-knowledge streams including bodyless DELETE, OPTIONS, and TRACE, handles HEAD without response DATA, rejects connection-specific request fields before handler dispatch, strips connection-specific response fields during h2c serialization, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges valid PING frames with matching opaque data, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman fields and bounded CONTINUATION header blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | Ordinary `CONNECT`, RFC 8441 extended `CONNECT`/`:protocol`, and `PUSH_PROMISE` are rejected deterministically before handler dispatch; HTTP/1.1 `CONNECT` and `Upgrade` remain separate handoff paths; bounded prior-knowledge h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, WebSocket over h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full RFC 8441 support, full stream state machine, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, priority scheduling, or full HTTP/2 server feature set |

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`http2`, `tls-native`, `tls-rustls`, or `all` for the corresponding
`rttp_client` capabilities. The `http2` feature exposes the bounded
prior-knowledge h2c client path for GET, HEAD, bodyless DELETE, OPTIONS, or
TRACE, and buffered POST, PUT, or PATCH requests. It opens at most one request
stream, advertises `SETTINGS_ENABLE_PUSH = 0` so peers see server push
disabled on the client side, validates received `SETTINGS_ENABLE_PUSH` values
as only `0` or `1`, and honors the peer's initial
`SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the
peer allows zero streams. The bounded client path also honors peer-advertised
`SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits before sending request
HEADERS or trailing HEADERS. It
validates `SETTINGS_MAX_FRAME_SIZE` on both sides of the bounded h2c handshake:
a configured local `http2_max_frame_size` is advertised only when set, must be
in the legal HTTP/2 range of 16,384 through 16,777,215 bytes, and rejects
inbound frame payloads above that active local limit; peer-advertised values
outside the same range reject the handshake, while legal peer values are used
to split outbound request HEADERS, DATA, and trailing HEADERS. It also
includes rejection of request bodies for GET, HEAD, DELETE,
OPTIONS, and TRACE, HEAD response body suppression, stripping of HTTP/1.x
connection-specific request fields before h2c emission, rejection of
connection-specific peer response fields, HPACK static Huffman strings, request
dynamic entries within the peer's advertised `SETTINGS_HEADER_TABLE_SIZE`,
bounded local response dynamic table decoding, bounded large header blocks via
CONTINUATION frames, padded incoming response frames, and conservative DATA
flow-control for single-stream prior-knowledge use. Any received
`SETTINGS_ENABLE_PUSH` value other than `0` or `1` rejects the bounded h2c
handshake. Valid response PRIORITY
frames and HEADERS priority fields are validated and ignored as metadata;
malformed priority metadata is rejected, and no priority scheduling is
performed. Valid PING frames are acknowledged with PING ACK frames that carry
the same opaque 8-byte data. Server push is outside this bounded client path.
For client HPACK, the peer's `SETTINGS_HEADER_TABLE_SIZE` bounds outbound
request dynamic indexing for request HEADERS and trailers, and a peer value of
zero disables that request dynamic table. The local response decoder uses the
default 4,096-byte table unless `ConfigBuilder::http2_header_table_size`
configures and advertises another `u32`-sized limit; incoming response
table-size updates may shrink the decoder table, including to zero, but
updates above the advertised local limit are rejected. The wrapper does not add
a public dynamic policy API for changing those limits after the h2c handshake.
Unknown frame types, including extension frames, are ignored only after the
prior-knowledge h2c handshake in this bounded direct-client path where HTTP/2
permits that behavior; RTTP does not expose extension callbacks or perform
full extension negotiation. Reserved stream identifier high bits are masked
when frames are parsed or written, which normalizes wire framing but does not
add broader multiplex scheduling or persistent session management. Incoming
`PUSH_PROMISE` frames are rejected
deterministically instead of creating or tracking push state even when the
peer advertises `SETTINGS_ENABLE_PUSH = 1`. HTTP/1.1
`CONNECT` tunnel handoff remains a separate path; prior-knowledge h2c `GOAWAY`
is treated as a bounded shutdown signal: completed responses remain usable,
active responses continue only when the peer's `last-stream-id` includes the
stream, and lower boundaries reject the response deterministically. A
`GOAWAY` received before stream 1 is opened is treated as request refusal and
no request HEADERS are sent. RTTP returns that refusal to the caller instead
of retrying on a new connection; callers that know a request is safe or
idempotent must choose any retry policy themselves. This protocol shutdown
boundary is distinct from a transport-level disconnect, read timeout, write
timeout, or TCP reset, which is reported through the normal socket/error path
without an HTTP/2 `last-stream-id` boundary.
`RST_STREAM` is likewise bounded to this prior-knowledge h2c client path: a
reset for the active stream is reported as response cancellation, while
malformed reset frames are rejected deterministically. RTTP does not expose a
public cancellation callback API or retry the request automatically. The h2c
client strips `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, `Upgrade`, `TE`, `Trailer`, `Host`, and any field named
by a `Connection` token from emitted request HEADERS. Peer response HEADERS
containing `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`,
`Transfer-Encoding`, or `Upgrade` are rejected. Application request trailers
such as `X-Trace`, `X-Upload-Status`, or `X-Upload-Checksum` are valid in this
bounded h2c path and are encoded as trailing HEADERS after request DATA.
Configured request trailers are rejected before emission when their field name
is invalid or reserved for connection/framing/routing behavior: `Connection`,
`Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, `Content-Length`, `Host`, `Proxy-Authenticate`, or
`Proxy-Authorization`. Peer response trailers use the existing
forbidden-trailer validation for invalid pseudo-header-like names,
connection-specific, routing, authentication/cookie, and framing fields such
as `Authorization`, `Connection`, `Content-Length`, `Cookie`, `Host`,
`Keep-Alive`, `Proxy-Authenticate`, `Proxy-Authorization`,
`Proxy-Connection`, `Set-Cookie`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, and `WWW-Authenticate`. `CONNECT`, RFC 8441 `:protocol` extended
CONNECT metadata, HTTP/1.1 `Upgrade` handoff requests, and proxy tunneling are
rejected before a client socket is opened. HTTP/1.1 `CONNECT` tunnel handoff
and `Upgrade` remain separate client handoff paths; this h2c path does not
provide WebSocket over h2, proxy h2, tunnel handoff, persistent HTTP/2
sessions, or full RFC 8441 support. TLS ALPN, extension callback APIs, full
extension negotiation, external h2 integration, connection pooling, automatic
retry, server push, full stream state machines, and full HTTP/2 features such
as unbounded multiplex scheduling, general multiplexing, and priority
scheduling remain outside that bounded prior-knowledge path. RTTP does not
expose a dynamic policy API for changing h2c frame-size or metadata limits at
runtime.

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
