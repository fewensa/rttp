mod support;

#[cfg(feature = "async")]
use std::collections::HashMap;

#[cfg(feature = "async")]
use futures::executor::block_on;
#[cfg(feature = "async")]
use futures::io::AllowStdIo;
#[cfg(feature = "async")]
use rttp_client::types::Proxy;
#[cfg(feature = "async")]
use rttp_client::{async_streaming_response_after_header, Config, HttpClient};
#[cfg(feature = "async")]
use std::io::Cursor;

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
fn test_async_streaming_response_constructor_is_exported() {
  block_on(async {
    let head = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 5\r\n", "\r\n")
      .as_bytes()
      .to_vec();
    let mut stream = AllowStdIo::new(Cursor::new(b"hello"));
    let mut response = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();

    response.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(b"hello", body.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_socket2_server_chunked_trailers_match_sync_accessors() {
  let (addr, _handle) = support::spawn_socket2_chunked_trailer_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("socket2 chunked body", response.body().string().unwrap());
    assert_eq!(2, response.trailers().len());
    assert_eq!(
      Some("abc"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("abc"),
      response.trailer_value("x-trace").map(String::as_str)
    );
    assert_eq!(
      Some("signed"),
      response.trailer("x-signature").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("signed"),
      response.trailer_value("X-SIGNATURE").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_quoted_extensions_are_accepted() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=\"bar;baz\";answer=42\r\nchunked\r\n",
    "6;empty;quoted=\"\\\\\\\"\"\r\n body!\r\n",
    "0;done=\"yes\"\r\n",
    "X-Trace: abc\r\n",
    "\r\n"
  ));

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(
      Some("abc"),
      response.trailer("x-trace").map(|h| h.value().as_str())
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_quoted_extensions_accept_obs_text() {
  let mut response = b"HTTP/1.1 200 OK\r\n\
Transfer-Encoding: chunked\r\n\
Connection: close\r\n\
\r\n\
7;meta=\""
    .to_vec();
  response.push(0xff);
  response.extend_from_slice(b"\"\r\nchunked\r\n0\r\n\r\n");

  let (addr, _handle) = support::spawn_chunked_response_server(response);
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("chunked", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_transfer_encoding_chunked_with_content_length_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 2\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("ambiguous response framing should be rejected");

    assert!(
      error
        .to_string()
        .contains("Transfer-Encoding conflicts with Content-Length"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_non_chunked_transfer_coding_before_chunked_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: gzip, chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("unsupported transfer coding should be rejected");

    assert!(
      error
        .to_string()
        .contains("Unsupported Transfer-Encoding response body"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_malformed_extension_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;bad name=value\r\nchunked\r\n",
    "0\r\n",
    "\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("malformed chunk extension should be rejected");

    assert!(
      error.to_string().contains("Invalid chunk extension"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_missing_crlf_after_chunk_data_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK",
    "0\r\n",
    "\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("missing chunk data terminator should be rejected");

    assert!(
      error.to_string().contains("Invalid chunk terminator"),
      "unexpected error: {error}"
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
fn test_async_forbidden_chunked_response_trailer_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "WWW-Authenticate: unsafe\r\n",
    "\r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("forbidden chunk trailer should be rejected");

    assert!(
      error.to_string().contains("Forbidden trailer header"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_malformed_chunked_response_trailer_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "Bad Name: unsafe\r\n",
    "\r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("malformed chunk trailer should be rejected");

    assert!(
      error.to_string().contains("Invalid trailer header"),
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
fn test_async_client_waits_for_100_continue_before_sending_body() {
  let (addr, handle) = support::spawn_expect_continue_gate_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue-gate", addr))
      .header(("Expect", "100-continue"))
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("accepted", response.body().string().unwrap());
  });

  let request = handle.join().expect("expect continue gate thread");
  assert!(!request.is_empty(), "body was sent before 100 Continue");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(b"request body"));
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_does_not_send_body_when_expect_continue_gets_final_response() {
  let (addr, handle) = support::spawn_expect_continue_reject_gate_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue-reject", addr))
      .header(("Expect", "100-continue"))
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(417, response.code());
    assert_eq!("Expectation Failed", response.body().string().unwrap());
  });

  let request = handle.join().expect("expect continue reject gate thread");
  assert!(
    !request.is_empty(),
    "body was sent before final expectation response"
  );
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(!request.ends_with(b"request body"));
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
fn test_async_client_returns_101_switching_protocols_as_terminal_response() {
  let (addr, _handle) = support::spawn_switching_protocols_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/upgrade", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(101, response.code());
    assert_eq!("Switching Protocols", response.reason());
    assert_eq!(
      Some(&"Upgrade".to_string()),
      response.header_value("Connection")
    );
    assert_eq!(
      Some(&"websocket".to_string()),
      response.header_value("Upgrade")
    );
    assert_eq!(
      Some(&"test-accept".to_string()),
      response.header_value("Sec-WebSocket-Accept")
    );
    assert_eq!("", response.body().string().unwrap());
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

#[cfg(feature = "async")]
async fn assert_async_redirect_resolves_to_target<F>(location: F, expected_target: &str)
where
  F: FnOnce(std::net::SocketAddr) -> String + Send + 'static,
{
  let (addr, _handle) = support::spawn_redirect_target_echo_server(location);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect/from?old=1", addr))
    .rasync()
    .await;

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(expected_target, response.body().string().unwrap());
}

#[cfg(feature = "async")]
struct CapturedRequest {
  method: String,
  target: String,
  headers: HashMap<String, String>,
  body: Vec<u8>,
}

#[cfg(feature = "async")]
fn captured_request(request: Vec<u8>) -> CapturedRequest {
  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .expect("captured request headers");
  let header = String::from_utf8_lossy(&request[..header_end]);
  let mut lines = header.lines();
  let request_line = lines.next().expect("captured request line");
  let mut request_line_parts = request_line.split_whitespace();
  let method = request_line_parts
    .next()
    .expect("captured request method")
    .to_string();
  let target = request_line_parts
    .next()
    .expect("captured request target")
    .to_string();
  let headers = lines
    .filter_map(|line| {
      let (name, value) = line.split_once(':')?;
      Some((name.to_ascii_lowercase(), value.trim().to_string()))
    })
    .collect();
  let body = request[header_end + 4..].to_vec();

  CapturedRequest {
    method,
    target,
    headers,
    body,
  }
}

#[cfg(feature = "async")]
async fn captured_async_redirected_post(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .post()
    .url(format!("http://{}/redirect", addr))
    .raw("redirect-body")
    .rasync()
    .await;

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[cfg(feature = "async")]
async fn captured_async_redirected_head(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .head()
    .url(format!("http://{}/redirect", addr))
    .rasync()
    .await;

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_303_post_becomes_get_without_body_or_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(303, "See Other").await;

    assert_eq!("GET", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"", request.body.as_slice());
    assert!(!request.headers.contains_key("content-length"));
    assert!(!request.headers.contains_key("content-type"));
    assert!(!request.headers.contains_key("transfer-encoding"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_303_head_preserves_method() {
  block_on(async {
    let request = captured_async_redirected_head(303, "See Other").await;

    assert_eq!("HEAD", request.method);
    assert_eq!("/final?via=redirect", request.target);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_307_post_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(307, "Temporary Redirect").await;

    assert_eq!("POST", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_308_post_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(308, "Permanent Redirect").await;

    assert_eq!("POST", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_absolute_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|addr| format!("http://{}/final", addr), "/final")
      .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_absolute_path_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|_| "/final?x=1".to_string(), "/final?x=1").await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_relative_path_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|_| "../final".to_string(), "/final").await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_query_only_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|_| "?page=2".to_string(), "/redirect/from?page=2")
      .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_percent_encoded_path_and_query_octets() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |_| "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b".to_string(),
      "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_uses_preserved_percent_encoded_path_as_relative_base() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/files/%2e%2e/a/"),
      ("/files/%2e%2e/a/", "next%2fhop?token=%2e%2e"),
    ],
    3,
  );
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "/files/%2e%2e/a/next%2fhop?token=%2e%2e",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_chain_that_finishes_within_max_redirect() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![("/start", "/hop-one"), ("/hop-one", "/final?done=1")],
    3,
  );
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("/final?done=1", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_enforces_max_redirect_bound() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/hop-one"),
      ("/hop-one", "/hop-two"),
      ("/hop-two", "/final"),
    ],
    3,
  );
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .expect_err("redirect chain should exceed max_redirect");

    assert!(error.is_redirect());
    assert!(error.to_string().contains("too many redirects"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_loop_after_relative_location_is_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/redirect/from?old=1", "?old=1")], 8);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect/from?old=1", addr))
      .rasync()
      .await
      .expect_err("redirect should resolve back to current URL");

    assert!(error.is_redirect());
    assert!(error
      .to_string()
      .contains("infinite redirect loop detected"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_loop_after_dot_segments_are_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/a/current", "../a/current")], 8);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/a/current", addr))
      .rasync()
      .await
      .expect_err("redirect should normalize back to current URL");

    assert!(error.is_redirect());
    assert!(error
      .to_string()
      .contains("infinite redirect loop detected"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_rebuilds_host_for_cross_authority_location() {
  let (origin_addr, target_addr, _handle) =
    support::spawn_cross_authority_redirect_host_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", origin_addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(target_addr.to_string(), response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_strips_sensitive_headers_for_cross_authority_location() {
  let (origin_addr, _target_addr, _handle) =
    support::spawn_cross_authority_redirect_header_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", origin_addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .header(("X-Trace", "trace-123"))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "authorization=\ncookie=\nproxy-authorization=\nx-trace=trace-123",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_sensitive_headers_for_same_authority_location() {
  let (addr, _handle) = support::spawn_same_authority_redirect_header_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .header(("X-Trace", "trace-123"))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "authorization=Bearer secret\ncookie=session=secret\nproxy-authorization=Basic proxy-secret\nx-trace=trace-123",
      response.body().string().unwrap()
    );
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
fn test_async_eof_delimited_response_body_is_read_to_connection_close() {
  let (addr, _handle) = support::spawn_eof_delimited_response_server("async eof body");
  block_on(async {
    let response = client().url(format!("http://{}/eof", addr)).rasync().await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("async eof body", response.body().string().unwrap());
    assert_eq!(
      Some(&"close".to_string()),
      response.header_value("Connection")
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_truncated_content_length_response_is_rejected() {
  let (addr, _handle) = support::spawn_truncated_content_length_server();
  block_on(async {
    let error = client()
      .url(format!("http://{}/truncated", addr))
      .rasync()
      .await
      .expect_err("truncated fixed-length body should be rejected");

    assert!(
      error.to_string().contains("failed to fill whole buffer")
        || error.to_string().contains("unexpected end of file"),
      "unexpected error: {error}"
    );
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
