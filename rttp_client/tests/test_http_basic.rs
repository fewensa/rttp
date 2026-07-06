mod support;

use std::collections::HashMap;

use rttp_client::types::{Auth, Para, Proxy, RoUrl};
use rttp_client::{Config, HttpClient};

fn client() -> HttpClient {
  HttpClient::new()
}

#[test]
fn test_http() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client().url(format!("http://{}/get", addr)).emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
  println!("{}", response);
}

#[test]
fn test_multi() {
  let (addr, _handle) = support::spawn_http_server();
  let mut para_map = HashMap::new();
  para_map.insert("id", "1");
  para_map.insert("relation", "eq");
  let response = client()
    .method("post")
    .url(RoUrl::with(format!("http://{}/?id=1&name=jack#none", addr)).para("name=Julia"))
    .path("post")
    .header("User-Agent: Mozilla/5.0")
    .header("Host: localhost")
    .para("name=Chico")
    .para(&"name=文".to_string())
    .para(para_map)
    .form(("debug", "true", "name=Form"))
    .cookie("token=123234")
    .cookie("uid=abcdef")
    .content_type("application/x-www-form-urlencoded")
    .encode(true)
    .traditional(true)
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_gzip() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .header(("Accept-Encoding", "gzip, deflate"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_invalid_gzip_returns_error_instead_of_panicking() {
  let (addr, _handle) = support::spawn_invalid_gzip_server();
  let result =
    std::panic::catch_unwind(|| client().get().url(format!("http://{}/gzip", addr)).emit());

  assert!(result.is_ok());
  assert!(result.unwrap().is_err());
}

#[test]
fn test_chunked() {
  let (addr, _handle) = support::spawn_chunked_server();
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(
    Some(&"chunked".to_string()),
    response.header_value("Transfer-Encoding")
  );
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some("abc"),
    response.trailer("x-trace").map(|h| h.value().as_str())
  );
  assert_eq!(
    Some("signed"),
    response.trailer("X-SIGNATURE").map(|h| h.value().as_str())
  );
}

#[test]
fn test_chunked_without_trailers_exposes_empty_trailers() {
  let (addr, _handle) = support::spawn_chunked_server_without_trailers();
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
  assert!(response.trailers().is_empty());
  assert!(response.trailer("x-trace").is_none());
}

#[test]
fn test_chunked_with_trailers_decodes_body() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=bar\r\nchunked\r\n",
    "6\r\n body!\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ));

  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("chunked body!", response.body().string().unwrap());
}

#[test]
fn test_chunked_oversized_extension_is_rejected() {
  let extension = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7;foo={extension}\r\n\
     chunked\r\n\
     0\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("oversized chunk extension should be rejected");

  assert!(
    error
      .to_string()
      .contains("chunked response line is too large"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_oversized_trailer_is_rejected() {
  let trailer = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7\r\n\
     chunked\r\n\
     0\r\n\
     X-Trace: {trailer}\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("oversized chunk trailer should be rejected");

  assert!(
    error
      .to_string()
      .contains("chunked response line is too large"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_duplicate_set_cookie_headers_are_preserved() {
  let (addr, _handle) = support::spawn_duplicate_set_cookie_server();
  let response = client()
    .get()
    .url(format!("http://{}/cookies", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!(
    vec![
      &"session=abc; Path=/; HttpOnly".to_string(),
      &"theme=dark; Path=/; SameSite=Lax".to_string()
    ],
    response.header_values("set-cookie")
  );
  assert_eq!(
    Some(&"session=abc; Path=/; HttpOnly".to_string()),
    response.header_value("set-cookie")
  );
  assert_eq!(2, response.cookies().len());
  assert!(response.cookie("session").is_some());
  assert!(response.cookie("theme").is_some());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
}

#[test]
fn test_content_length_response_does_not_wait_for_eof() {
  let (addr, _handle) = support::spawn_keep_alive_server();
  let response = client()
    .get()
    .config(Config::builder().read_timeout(100))
    .url(format!("http://{}/keep-alive", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_upload() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client()
    .method("post")
    .url(format!("http://{}/post", addr))
    .form(("debug", "true", "name=Form"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_raw_json() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("post")
    .url(format!("http://{}/post?raw=json", addr))
    .para("name=Chico")
    .content_type("application/json")
    .raw(r#"  {"from": "rttp"} "#)
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_raw_form_urlencoded() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("post")
    .url(format!("http://{}/post", addr))
    .para(Para::with_form("name", "Chico"))
    .raw("name=Nick&name=Wendy")
    .content_type("application/x-www-form-urlencoded")
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(feature = "tls-rustls")]
fn test_https() {
  let (addr, _handle) = support::spawn_tls_server();
  let response = client()
    .get()
    .url(format!("https://{}/", addr))
    .config(
      Config::builder()
        .verify_ssl_cert(false)
        .verify_ssl_hostname(false),
    )
    .para(Para::with_form("q", "News"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_http_with_url() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("get")
    .url(
      RoUrl::with(format!("http://{}", addr))
        .path("/get")
        .para(("name", "Chico")),
    )
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(any(feature = "tls-rustls", feature = "tls-native"))]
#[ignore]
fn test_with_proxy_http() {
  client()
    .get()
    .url("https://example.test")
    .proxy(Proxy::http("127.0.0.1", 1081))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_with_proxy_socks5() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) = support::spawn_socks5_proxy_server();
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .proxy(Proxy::socks5("127.0.0.1", proxy_addr.port().into()))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_with_proxy_socks5_auth() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) =
    support::spawn_socks5_proxy_server_with_credentials("username", "password");
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .proxy(Proxy::socks5_with_authorization(
      "127.0.0.1",
      proxy_addr.port().into(),
      "username",
      "password",
    ))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_auto_redirect() {
  let (addr, _handle) = support::spawn_redirect_server();
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/", addr))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert!(response.ok());
}

#[test]
fn test_http_proxy_uses_absolute_form_for_http_requests() {
  let (addr, _handle) = support::spawn_http_proxy_server();
  let response = client()
    .get()
    .url("http://example.test/proxy?q=1")
    .proxy(Proxy::http("127.0.0.1", u32::from(addr.port())))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!(
    "GET http://example.test/proxy?q=1 HTTP/1.1",
    response.body().string().unwrap()
  );
}

#[test]
fn test_http_proxy_with_auth_uses_proxy_authorization_header() {
  let (addr, _handle) = support::spawn_http_proxy_auth_echo_server();
  let response = client()
    .get()
    .url("http://example.test/proxy?q=1")
    .proxy(Proxy::http_with_authorization(
      "127.0.0.1",
      u32::from(addr.port()),
      "user",
      "secret",
    ))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
}

#[test]
#[cfg(feature = "tls-rustls")]
fn test_https_proxy_with_auth_uses_connect_tunnel() {
  let (proxy_addr, target_addr, _proxy_handle) =
    support::spawn_https_proxy_server_with_credentials("user", "secret");
  let response = client()
    .get()
    .url(format!("https://localhost:{}/", target_addr.port()))
    .proxy(Proxy::http_with_authorization(
      "127.0.0.1",
      u32::from(proxy_addr.port()),
      "user",
      "secret",
    ))
    .config(
      Config::builder()
        .verify_ssl_cert(false)
        .verify_ssl_hostname(false),
    )
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_connection_closed() {
  let (addr, _handle) = support::spawn_http_server_count(5);
  let mut client = client();
  let resp0 = client.url(format!("http://{}/get", addr)).emit();
  assert!(resp0.is_ok());
  let resp1 = client.post().url(format!("http://{}/post", addr)).emit();
  assert!(resp1.is_err());
  let resp2 = self::client().url(format!("http://{}/get", addr)).emit();
  assert!(resp2.is_ok());
  let resp3 = self::client()
    .post()
    .url(format!("http://{}/post", addr))
    .emit();
  assert!(resp3.is_ok());
  let resp4 = client
    .reset()
    .post()
    .url(format!("http://{}/post", addr))
    .emit();
  assert!(resp4.is_ok());
}

#[test]
fn test_basic_auth() {
  let (addr, _handle) = support::spawn_auth_echo_server();
  let response = client()
    .get()
    .url(format!("http://{}/", addr))
    .auth(Auth::basic("user", "secret"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  // base64("user:secret") = "dXNlcjpzZWNyZXQ="
  assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
}

#[test]
fn test_bearer_auth() {
  let (addr, _handle) = support::spawn_auth_echo_server();
  let response = client()
    .get()
    .url(format!("http://{}/", addr))
    .auth(Auth::bearer("my-token-abc"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("Bearer my-token-abc", response.body().string().unwrap());
}
