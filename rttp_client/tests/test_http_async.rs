mod support;

#[cfg(feature = "async")]
use futures::executor::block_on;
#[cfg(feature = "async")]
use rttp_client::types::Proxy;
#[cfg(feature = "async")]
use rttp_client::{Config, HttpClient};

#[cfg(feature = "async")]
fn client() -> HttpClient {
  HttpClient::new()
}

#[test]
#[cfg(feature = "async")]
fn test_async_http() {
  let (addr, _handle) = support::spawn_http_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/post", addr))
      .form("debug=true")
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked() {
  let (addr, _handle) = support::spawn_chunked_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(2, response.trailers().len());
    assert_eq!(
      Some("abc"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("signed"),
      response.trailer("x-signature").map(|h| h.value().as_str())
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_oversized_extension_is_rejected() {
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

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("oversized chunk extension should be rejected");

    assert!(
      error
        .to_string()
        .contains("chunked response line is too large"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_oversized_trailer_is_rejected() {
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

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("oversized chunk trailer should be rejected");

    assert!(
      error
        .to_string()
        .contains("chunked response line is too large"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_duplicate_set_cookie_headers_are_preserved() {
  let (addr, _handle) = support::spawn_duplicate_set_cookie_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/cookies", addr))
      .rasync()
      .await;
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
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_content_length_response_does_not_wait_for_eof() {
  let (addr, _handle) = support::spawn_keep_alive_server();
  block_on(async {
    let response = client()
      .get()
      .config(Config::builder().read_timeout(100))
      .url(format!("http://{}/keep-alive", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("OK", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_skips_100_continue_before_final_response() {
  let (addr, _handle) = support::spawn_continue_then_ok_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue", addr))
      .header(("Expect", "100-continue"))
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("OK", response.reason());
    assert_eq!(
      Some(&"text/plain".to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
    assert!(response.header_value("X-Interim").is_none());
    assert_eq!("final body", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_skips_103_early_hints_before_final_response() {
  let (addr, _handle) = support::spawn_informational_then_ok_server("103 Early Hints");
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/early-hints", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("OK", response.reason());
    assert_eq!(
      Some(&"text/plain".to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
    assert!(response.header_value("X-Interim").is_none());
    assert_eq!("final body", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect() {
  let (addr, _handle) = support::spawn_redirect_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert!(response.ok());
  });
}

#[test]
#[cfg(all(feature = "async", feature = "tls-rustls"))]
fn test_async_https() {
  let (addr, _handle) = support::spawn_tls_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("https://{}/get", addr))
      .config(
        rttp_client::Config::builder()
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_http_proxy_uses_absolute_form_for_http_requests() {
  let (addr, _handle) = support::spawn_http_proxy_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://example.test/proxy?q=1")
      .proxy(Proxy::http("127.0.0.1", u32::from(addr.port())))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(
      "GET http://example.test/proxy?q=1 HTTP/1.1",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_http_proxy_with_auth_uses_proxy_authorization_header() {
  let (addr, _handle) = support::spawn_http_proxy_auth_echo_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://example.test/proxy?q=1")
      .proxy(Proxy::http_with_authorization(
        "127.0.0.1",
        u32::from(addr.port()),
        "user",
        "secret",
      ))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_proxy_socks5() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) = support::spawn_socks5_proxy_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/get", addr))
      .proxy(Proxy::socks5("127.0.0.1", proxy_addr.port().into()))
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(all(feature = "async", feature = "tls-rustls"))]
fn test_async_https_proxy_with_auth_uses_connect_tunnel() {
  let (proxy_addr, target_addr, _proxy_handle) =
    support::spawn_https_proxy_server_with_credentials("user", "secret");
  block_on(async {
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
        rttp_client::Config::builder()
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("OK", response.body().string().unwrap());
  });
}
