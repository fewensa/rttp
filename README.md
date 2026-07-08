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
| Trailers | Chunked response trailers are exposed for blocking and async APIs; streaming chunked uploads can send declared request trailers | Forbidden framing/routing trailer fields are rejected |
| Prior-knowledge h2c | With `http2`, direct `socket2` h2c sends GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, and buffered POST, PUT, or PATCH requests, suppresses HEAD response bodies, treats `RST_STREAM` on the active stream as a bounded reset/cancellation signal, acknowledges valid PING frames with matching opaque data, DATA bodies, trailers, HPACK static Huffman strings, dynamic entries within peer settings, large header blocks, padded incoming frames, `GOAWAY` shutdown boundaries, PRIORITY metadata validation without scheduling, HTTP/2-allowed unknown/extension frame ignoring inside this bounded path, reserved stream-id high-bit normalization, and conservative DATA flow control | `CONNECT` is rejected deterministically before opening a client socket, and `PUSH_PROMISE`/server push is rejected instead of managed; bounded prior-knowledge h2c only, with no public cancellation callback API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, proxy tunneling to h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplex scheduling, general multiplexing, priority scheduling, or request bodies for GET, HEAD, DELETE, OPTIONS, or TRACE |

With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a bounded
prior-knowledge h2c request over a direct socket2 TCP connection. It supports
GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, and buffered POST, PUT, or PATCH requests,
including non-empty buffered request bodies as DATA frames for the write
methods. GET, HEAD, DELETE, OPTIONS, and TRACE requests with bodies are rejected; HEAD,
bodyless DELETE, OPTIONS, and TRACE requests do not send request DATA frames,
and any HEAD response DATA frames are consumed without being exposed as a
response body. The client
supports HPACK static Huffman
strings and large header blocks via CONTINUATION frames. It uses HPACK dynamic
entries for repeated request header and trailer fields within the peer's
advertised table size, and it decodes response dynamic table entries and
table-size updates from incoming padded HEADERS, DATA, and trailer frames
without exposing padding bytes. Valid response PRIORITY frames and HEADERS
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
boundary rejects the response deterministically. `RST_STREAM` is likewise
bounded to this prior-knowledge h2c client path: a reset for the active stream
is reported as response cancellation, while malformed reset frames are rejected
deterministically. RTTP does not expose a public cancellation callback API or
retry the request automatically. `CONNECT` and proxy tunneling are rejected
before a client socket is opened. TLS ALPN, extension callback APIs, full
extension negotiation, external h2 integration, proxy h2, tunnel handoff,
connection pooling, persistent HTTP/2 session management, automatic retry,
server push, full stream state machines, and full HTTP/2 features such as
unbounded multiplex scheduling, general multiplexing, and priority scheduling
are not part of that bounded prior-knowledge client path.

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
and TRACE requests. It preserves the same HEAD body suppression for
prior-knowledge h2c responses.
Incoming padded HEADERS, DATA, and trailer frames are accepted without exposing
padding bytes to handlers, and HPACK static Huffman strings, request dynamic
table entries, and large header blocks are carried with CONTINUATION frames.
Prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch; `TE` is accepted only as `te: trailers`.
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
boundary for already processed streams.
Within that same prior-knowledge h2c server path, inbound `RST_STREAM` is a
bounded reset/cancellation signal for the affected stream: reset request
streams are not dispatched to handlers, and reset response streams stop within
the bounded write path. RTTP does not expose a public cancellation callback API,
retry work automatically, keep persistent HTTP/2 sessions, or model a full
HTTP/2 stream state machine around those resets.
The prior-knowledge h2c path does not share the HTTP/1.1 `CONNECT` handoff
path: h2c `CONNECT` is rejected before handler dispatch. TLS ALPN, extension
callback APIs, full extension negotiation, external h2 integration, proxy h2,
tunnel handoff, connection pooling, persistent HTTP/2 session management, and
full HTTP/2 features such as unbounded multiplexing, unbounded multiplex
scheduling, general multiplexing, server push, and priority scheduling remain
outside this bounded prior-knowledge server path.

It is not a full RFC-covering web server and still does not implement server
TLS or async accept loops.

### Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Trailer names that affect framing or routing are rejected |
| Prior-knowledge h2c | The same `socket2` listener detects the HTTP/2 preface, validates SETTINGS, serves bounded prior-knowledge streams including bodyless DELETE, OPTIONS, and TRACE, handles HEAD without response DATA, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges valid PING frames with matching opaque data, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman/dynamic fields and CONTINUATION blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | `CONNECT` and `PUSH_PROMISE` are rejected deterministically before handler dispatch; bounded prior-knowledge h2c only, with no public cancellation callback API, no extension callback API, full extension negotiation, TLS ALPN, external h2 integration, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, priority scheduling, or full HTTP/2 server feature set |
