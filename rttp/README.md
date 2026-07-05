rttp
====

`rttp` wraps `rttp_client` behind optional client features and provides a small
blocking HTTP server for local tests and simple embedded use.

## Server

Create a listener with `rttp::Http::server` or call `HttpServer::bind` directly.
The server listener is built with `socket2`.

```rust,no_run
use rttp::server::HttpResponse;

fn main() -> std::io::Result<()> {
  let server = rttp::Http::server("127.0.0.1:0")?;
  println!("listening on {}", server.local_addr()?);

  server.accept_one(|request| {
    println!("{} {}", request.method(), request.target());
    HttpResponse::ok("hello")
  })
}
```

`HttpServer::local_addr` returns the bound address, which is useful when binding
to port `0` in tests. `HttpServer::accept_one` serves one connection.
`HttpServer::serve_requests` serves a fixed number of sequential connections on
the same listener.

The server currently parses blocking HTTP/1.x requests, supports fixed
`Content-Length` request bodies, writes one response, and closes the connection.
It does not implement chunked request bodies, keep-alive serving, TLS, or async
accept loops.

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`tls-native`, `tls-rustls`, or `all` for the corresponding `rttp_client`
capabilities.

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
