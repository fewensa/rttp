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
minimal bounded handler, including bodyless DELETE requests. The h2c path
handles `HEAD` without writing response DATA frames. Incoming padded HEADERS,
DATA, and trailer frames are accepted without exposing padding bytes to
handlers, HPACK static Huffman strings, request dynamic table entries, and large
header blocks are carried with CONTINUATION frames, and multiple prior-knowledge
h2c request streams may be open on one connection up to the caller's bounded
`serve_requests` loop.
Responses are still written synchronously as requests complete. The minimal
h2c path supports conservative DATA flow-control for prior-knowledge use.

The server is intentionally not a full RFC-covering web server and still does
not implement server TLS, TLS ALPN, proxy h2, full HTTP/2 features such as
unbounded multiplexing and priority scheduling, or async accept loops.

## Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Trailer names that affect framing or routing are rejected |
| Prior-knowledge h2c | The same `socket2` listener detects the HTTP/2 preface, validates SETTINGS, serves bounded prior-knowledge streams including bodyless DELETE, handles HEAD without response DATA, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman/dynamic fields and CONTINUATION blocks, and applies conservative DATA flow control | No TLS ALPN, proxy h2, unbounded multiplexing, priority scheduling, or full HTTP/2 server feature set |

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`http2`, `tls-native`, `tls-rustls`, or `all` for the corresponding
`rttp_client` capabilities. The `http2` feature exposes the minimal
prior-knowledge h2c client path for GET, HEAD, bodyless DELETE, and buffered
POST, PUT, or PATCH requests, including rejection of request bodies for GET,
HEAD, and DELETE, HEAD response body suppression, HPACK static Huffman
strings, request dynamic entries within the peer's advertised table size,
response dynamic table decoding, large header blocks via CONTINUATION frames,
padded incoming response frames, and conservative DATA flow-control for
single-stream prior-knowledge use; TLS ALPN, proxy h2, and full HTTP/2 features
such as multiplexing and priority scheduling remain outside that path.

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
