rttp
===

# rttp

Wrapper of rttp_client

[rttp_client](https://github.com/fewensa/rttp)

## Minimal server

```rust
use rttp::server::HttpResponse;

fn main() -> std::io::Result<()> {
  let server = rttp::Http::server("127.0.0.1:8080")?;
  server.accept_one(|request| {
    println!("{} {}", request.method(), request.target());
    HttpResponse::ok("hello")
  })
}
```






