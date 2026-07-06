mod support;

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::HttpClient;
use std::time::Duration;

fn client() -> HttpClient {
  HttpClient::new()
}

fn capture_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_raw_http_request();
  request(format!("http://{}", addr));
  handle.join().expect("raw request capture server")
}

fn capture_optional_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
  request(format!("http://{}", addr));
  handle.join().expect("optional raw request capture server")
}

fn request_text(request: &[u8]) -> String {
  String::from_utf8(request.to_vec()).expect("request should be utf-8")
}

fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
  request.lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    if header_name.eq_ignore_ascii_case(name) {
      Some(value.trim())
    } else {
      None
    }
  })
}

fn request_body(request: &[u8]) -> &[u8] {
  let body_start = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| position + 4)
    .expect("request should contain header terminator");
  &request[body_start..]
}

#[test]
fn get_with_query_parameters_sends_request_target_without_body() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/search", base_url))
      .para("name=Julia")
      .para(("debug", "true"))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("GET /search?name=Julia&debug=true HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn head_without_body_omits_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .head()
      .url(format!("{}/metadata", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("HEAD /metadata HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn delete_without_body_omits_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .delete()
      .url(format!("{}/resource", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("DELETE /resource HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn bodyless_request_preserves_explicit_content_type_without_content_length() {
  let request = capture_request(|base_url| {
    client()
      .head()
      .url(format!("{}/metadata", base_url))
      .content_type("application/json")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("HEAD /metadata HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/json"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn post_para_sends_form_urlencoded_body_and_matching_content_length() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/submit", base_url))
      .para("name=Julia")
      .para(("debug", "true"))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /submit HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/x-www-form-urlencoded"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(b"name=Julia&debug=true", body);
}

#[test]
fn raw_body_without_explicit_content_type_sends_text_plain() {
  let raw_body = "plain body";
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/raw", base_url))
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /raw HTTP/1.1\r\n"));
  assert_eq!(Some("text/plain"), header_value(&text, "Content-Type"));
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn raw_json_preserves_explicit_content_type_and_content_length() {
  let raw_body = r#"{"from":"rttp"}"#;
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/json", base_url))
      .content_type("application/json")
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /json HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/json"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn raw_body_preserves_existing_query_parameters_in_request_target() {
  let raw_body = "plain body";
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/raw?trace=abc&debug=true", base_url))
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /raw?trace=abc&debug=true HTTP/1.1\r\n"));
  assert_eq!(Some("text/plain"), header_value(&text, "Content-Type"));
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn binary_body_without_explicit_content_type_sends_octet_stream() {
  let binary_body = vec![0, 1, 2, 3];
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/binary", base_url))
      .binary(binary_body.clone())
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /binary HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/octet-stream"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(binary_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(binary_body.as_slice(), body);
}

#[test]
fn binary_body_preserves_existing_query_parameters_in_request_target() {
  let binary_body = vec![0, 1, 2, 3];
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/binary?trace=abc&debug=true", base_url))
      .binary(binary_body.clone())
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /binary?trace=abc&debug=true HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/octet-stream"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(binary_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(binary_body.as_slice(), body);
}

#[test]
fn multipart_form_body_sends_generated_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/form", base_url))
      .form("name=Julia")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);
  let content_type = header_value(&text, "Content-Type").expect("content type header");

  assert!(text.starts_with("POST /form HTTP/1.1\r\n"));
  assert!(content_type.starts_with("multipart/form-data; boundary="));
  assert_eq!(
    Some(body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert!(body.starts_with(b"-----------------------------"));
  assert!(body.ends_with(b"--\r\n"));
}

#[test]
fn custom_common_headers_are_not_overwritten_by_auto_headers() {
  let request = capture_request(|base_url| {
    let authority = base_url
      .strip_prefix("http://")
      .expect("test URL should be http");
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", authority))
      .header("User-Agent: custom-agent/1.0")
      .header("Accept: application/json")
      .header("Connection: keep-alive")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert_eq!(Some("custom-agent/1.0"), header_value(&text, "User-Agent"));
  assert_eq!(Some("application/json"), header_value(&text, "Accept"));
  assert_eq!(Some("keep-alive"), header_value(&text, "Connection"));
}

#[test]
fn matching_explicit_host_header_is_preserved() {
  let request = capture_request(|base_url| {
    let authority = base_url
      .strip_prefix("http://")
      .expect("test URL should be http");
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", authority))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
fn conflicting_explicit_host_header_is_rejected_before_sending_request() {
  let request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", "example.test"))
      .emit()
      .expect_err("conflicting host should be rejected");

    assert!(error.is_builder());
    assert!(error.to_string().contains("Host header"));
  });

  assert_eq!(b"", request.as_slice());
}

#[test]
fn missing_host_header_is_generated_from_url_authority() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = text
    .lines()
    .find_map(|line| line.strip_prefix("Host: "))
    .expect("generated host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn async_matching_explicit_host_header_is_preserved() {
  let request = {
    let (addr, handle) = support::capture_raw_http_request();
    block_on(async {
      let base_url = format!("http://{}", addr);
      let authority = base_url
        .strip_prefix("http://")
        .expect("test URL should be http");
      client()
        .get()
        .url(format!("{}/headers", base_url))
        .header(("Host", authority))
        .rasync()
        .await
        .expect("request should succeed");
    });
    handle.join().expect("raw request capture server")
  };

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn async_conflicting_explicit_host_header_is_rejected_before_sending_request() {
  let request = {
    let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
    block_on(async {
      let error = client()
        .get()
        .url(format!("http://{}/headers", addr))
        .header(("Host", "example.test"))
        .rasync()
        .await
        .expect_err("conflicting host should be rejected");

      assert!(error.is_builder());
      assert!(error.to_string().contains("Host header"));
    });
    handle.join().expect("optional raw request capture server")
  };

  assert_eq!(b"", request.as_slice());
}

#[test]
#[cfg(feature = "async")]
fn async_missing_host_header_is_generated_from_url_authority() {
  let request = {
    let (addr, handle) = support::capture_raw_http_request();
    block_on(async {
      client()
        .get()
        .url(format!("http://{}/headers", addr))
        .rasync()
        .await
        .expect("request should succeed");
    });
    handle.join().expect("raw request capture server")
  };

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("generated host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
fn connect_method_uses_authority_form_request_target() {
  let request = capture_request(|base_url| {
    client()
      .method("CONNECT")
      .url(base_url)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let request_line = text.lines().next().expect("request line");
  let host = header_value(&text, "Host").expect("host header");

  assert_eq!(format!("CONNECT {} HTTP/1.1", host), request_line);
}
