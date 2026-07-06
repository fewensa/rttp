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
| tls-native | HTTPS with `native-tls` |
| tls-rustls | HTTPS with `rustls` |

```toml
[dependencies]
rttp_client = "0.2"
```

Direct TCP client connections are opened with `socket2`. SOCKS proxy handshakes
are still delegated to the `socks` crate.
Chunked responses are decoded by the client, and response trailers are available
through the response trailer accessors.

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
    HttpResponse::ok("hello").header("Transfer-Encoding", "chunked")
  })
}
```

Use `HttpServer::bind` directly when you already want the server type,
`HttpServer::local_addr` to read the bound address, `accept_one` for one
connection, and `serve_requests` for a fixed number of sequential connections.
Use `with_read_timeout` and `with_write_timeout` to apply socket-level
timeouts to each accepted connection; pass `None` to leave the corresponding
socket timeout unset. Add `Transfer-Encoding: chunked` to an `HttpResponse` to
write the complete response body with chunked transfer framing instead of an
automatic `Content-Length`. The listener path uses `socket2`.

The server is intentionally small: it handles blocking HTTP/1.x request parsing
for local tests and simple embedded use. It accepts fixed `Content-Length` and
chunked request bodies, exposes chunked request trailers, applies bounded
request head/body validation, handles `HEAD` without writing a response body,
honors `Connection` close/keep-alive semantics across a bounded
`serve_requests` loop, writes response body framing consistently, and accepts
`Expect: 100-continue`.

It is not a full RFC-covering web server and still does not implement server
TLS or async accept loops.
