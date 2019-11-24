rttp
===

# rttp
A simple to use http lib for rust.

# rttp_client

## Additional features
rttp_client is a minimal http client, the default features only support
http request, but you can add features to support https request, and async support

| name | comment |
|------|---------|
| async | Async request features |
| tls-native | support https request use `native-tls` crate |
| tls-rustls | support https request use `rustls` crate |

The default use

```toml
[dependencies]
rttp_client = "0.1"
```

With tls-native

```toml
[dependencies]
rttp_client = { version = "0.1", features = ["tls-native"] }
```

With tls-rustls

```toml
[dependencies]
rttp_client = { version = "0.1", features = ["tls-rustls"] }
```

Async support


```toml
[dependencies]
rttp_client = { version = "0.1", features = ["async"] }
```

Full support

```toml
[dependencies]
rttp_client = { version = "0.1", features = ["async", "tls-native"] }
```

*Important*
`tls-native` and `tls-rustls` only support choose on features, do not same to use.

## Examples

### GET

```rust
# use rttp_client::HttpClient;
HttpClient::new().get()
  .url("http://httpbin.org/get")
  .emit();
```

### POST

```rust
# use rttp_client::HttpClient;
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .emit();
```

### Header

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::Header;
# use std::collections::HashMap;
let mut multi_headers = HashMap::new();
multi_headers.insert("name", "value");
HttpClient::new().get()
 .url("http://httpbin.org/get")
 .header("name: value\nname: value")
 .header(("name", "value", "name: value\nname: value"))
 .header(Header::new("name", "value"))
 .header(multi_headers)
 .emit();
```

### Para

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::Para;
# use std::collections::HashMap;
let mut multi_para = HashMap::new();
multi_para.insert("name", "value");
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .para("name=value&name=value")
  .para(("name", "value", "name=value&name=value"))
  .para(Para::new("name", "value"))
  .para(multi_para)
  .emit();
```

### Url

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::RoUrl;
HttpClient::new().get()
  .url(RoUrl::with("http://httpbin.org").path("get").para("name=value").para(("from", "rttp")))
  .emit();
```

### POST JSON

```rust
# use rttp_client::HttpClient;
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .content_type("application/json")
  .raw(r#" {"id": 1, "from": "rttp"} "#)
  .emit();
```

### Form && Upload file

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::FormData;
# use std::collections::HashMap;
let mut multi_form = HashMap::new();
multi_form.insert("name", "value");
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .para("name=value")
  .form("name=value")
  .form("name=value&name=value")
  .form(("name", "value", "name=value&name=value"))
  .form("file=@filename#/path/to/file")
  .form("file=@/path/to/file")
  .form(multi_form)
  .form(FormData::with_text("name", "value"))
  .form(FormData::with_file("name", "/path/to/file"))
  .form(FormData::with_file_and_name("name", "/path/to/file", "filename"))
  .form(FormData::with_binary("name", vec![]))  // Vec<u8>
  .emit();
```
Para and form can be mixed, para does not support file parsing

### Proxy

*BASIC*

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::Proxy;
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .content_type("application/json")
  .raw(r#" {"id": 1, "from": "rttp"} "#)
  .proxy(Proxy::http("127.0.0.1", 1081))
  .emit();
```

*BASIC WITH AUTHORIZATION*

```rust
# use rttp_client::HttpClient;
# use rttp_client::types::Proxy;
HttpClient::new().post()
  .url("http://httpbin.org/post")
  .content_type("application/json")
  .raw(r#" {"id": 1, "from": "rttp"} "#)
  .proxy(Proxy::socks5_with_authorization("127.0.0.1", 1081, "username", "password"))
  .emit();
```

### Auto redirect

```rust
# use rttp_client::HttpClient;
# use rttp_client::Config;
let response = HttpClient::new().post()
  .config(Config::builder().auto_redirect(true))
  .get()
  .url("http://bing.com")
  .emit();
assert!(response.is_ok());
let response = response.unwrap();
assert_ne!("bing.com", response.host());
```

### Async

```rust
# use rttp_client::HttpClient;
# #[cfg(feature = "async")]
let response = HttpClient::new().post()
  .url("http://httpbin.org/post")
  .rasync()
  .await;
```









