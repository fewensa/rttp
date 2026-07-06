mod support;

use rttp_client::HttpClient;

fn client() -> HttpClient {
  HttpClient::new()
}

fn capture_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_raw_http_request();
  request(format!("http://{}", addr));
  handle.join().expect("raw request capture server")
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
fn custom_common_headers_are_not_overwritten_by_auto_headers() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .header("Host: example.test")
      .header("User-Agent: custom-agent/1.0")
      .header("Accept: application/json")
      .header("Connection: keep-alive")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert_eq!(Some("example.test"), header_value(&text, "Host"));
  assert_eq!(Some("custom-agent/1.0"), header_value(&text, "User-Agent"));
  assert_eq!(Some("application/json"), header_value(&text, "Accept"));
  assert_eq!(Some("keep-alive"), header_value(&text, "Connection"));
}
