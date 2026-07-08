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
requests. The h2c path handles `HEAD` without writing response DATA frames.
Incoming padded HEADERS, DATA, and trailer frames are accepted without exposing
padding bytes to handlers, HPACK static Huffman strings, request dynamic table
entries, and large header blocks are carried with CONTINUATION frames. Valid
prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch; `TE` is accepted only as `te: trailers`. Valid
standalone PRIORITY frames and HEADERS priority fields are validated and ignored
as metadata; malformed priority metadata is rejected, and request or response
ordering does not use priority scheduling. Multiple prior-knowledge h2c request
streams may be open on one connection up to the caller's bounded
`serve_requests` loop.
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
push state management.
Responses are still written synchronously as requests complete. The bounded
h2c path supports conservative DATA flow-control for prior-knowledge use. It
uses `GOAWAY` as a bounded shutdown signal when the loop ends, reporting the
last completed stream id so clients can apply a deterministic stream boundary.
Within that same prior-knowledge h2c server path, inbound `RST_STREAM` is a
bounded reset/cancellation signal for the affected stream: reset request
streams are not dispatched to handlers, and reset response streams stop within
the bounded write path. RTTP does not expose a public cancellation callback API,
retry work automatically, keep persistent HTTP/2 sessions, or model a full
HTTP/2 stream state machine around those resets.
It does not share the HTTP/1.1 `CONNECT` handoff path: prior-knowledge h2c
`CONNECT` is rejected before handler dispatch.

The server is intentionally not a full RFC-covering web server and still does
not implement server TLS, TLS ALPN, extension callback APIs, full extension
negotiation, external h2 integration, proxy h2, h2c tunnel handoff, connection
pooling, persistent HTTP/2 session management, automatic retry, public
cancellation callbacks, full stream state machines, full HTTP/2 features such
as unbounded multiplexing, unbounded multiplex scheduling, general
multiplexing, server push, and priority scheduling, or async accept loops.

## Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Trailer names that affect framing or routing are rejected |
| Prior-knowledge h2c | The same `socket2` listener detects the HTTP/2 preface, validates SETTINGS, serves bounded prior-knowledge streams including bodyless DELETE, OPTIONS, and TRACE, handles HEAD without response DATA, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges valid PING frames with matching opaque data, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman/dynamic fields and CONTINUATION blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | `CONNECT` and `PUSH_PROMISE` are rejected deterministically before handler dispatch; bounded prior-knowledge h2c only, with no public cancellation callback API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, priority scheduling, or full HTTP/2 server feature set |

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`http2`, `tls-native`, `tls-rustls`, or `all` for the corresponding
`rttp_client` capabilities. The `http2` feature exposes the bounded
prior-knowledge h2c client path for GET, HEAD, bodyless DELETE, OPTIONS, or
TRACE, and buffered POST, PUT, or PATCH requests, including rejection of
request bodies for GET, HEAD, DELETE, OPTIONS, and TRACE, HEAD response body
suppression, HPACK static Huffman strings, request dynamic entries within the
peer's advertised table size, response dynamic table decoding, large header
blocks via CONTINUATION frames, padded incoming response frames, and
conservative DATA flow-control for single-stream prior-knowledge use. Valid
response PRIORITY frames and HEADERS priority fields are validated and ignored
as metadata; malformed priority metadata is rejected, and no priority
scheduling is performed. Valid PING frames are acknowledged with PING ACK
frames that carry the same opaque 8-byte data. Server push is outside this
bounded client path. Unknown frame types, including extension frames, are
ignored only after the prior-knowledge h2c handshake in this bounded
direct-client path where HTTP/2 permits that behavior; RTTP does not expose
extension callbacks or perform full extension negotiation. Reserved stream
identifier high bits are masked when frames are parsed or written, which
normalizes wire framing but does not add broader multiplex scheduling or
persistent session management. Incoming `PUSH_PROMISE` frames are rejected
deterministically instead of creating or tracking push state. HTTP/1.1
`CONNECT` tunnel handoff remains a separate path; prior-knowledge h2c `GOAWAY`
is treated as a bounded shutdown signal: completed responses remain usable,
active responses continue only when the peer's `last-stream-id` includes the
stream, and lower boundaries reject the response deterministically.
`RST_STREAM` is likewise bounded to this prior-knowledge h2c client path: a
reset for the active stream is reported as response cancellation, while
malformed reset frames are rejected deterministically. RTTP does not expose a
public cancellation callback API or retry the request automatically. `CONNECT`
and proxy tunneling are rejected before a client socket is opened. TLS ALPN,
extension callback APIs, full extension negotiation, external h2 integration,
proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session
management, automatic retry, server push, full stream state machines, and full
HTTP/2 features such as unbounded multiplex scheduling, general multiplexing,
and priority scheduling remain outside that bounded prior-knowledge path.

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
