use rttp_test_support as support;

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::types::{FormData, Para};
use rttp_client::HttpClient;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    .expect("request header terminator")
    + 4;
  &request[body_start..]
}

#[cfg(feature = "async")]
fn multipart_boundary<'a>(request: &'a str) -> &'a str {
  header_value(request, "Content-Type")
    .and_then(|value| value.strip_prefix("multipart/form-data; boundary="))
    .expect("multipart boundary")
}

#[cfg(feature = "async")]
fn replace_bytes(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
  if needle.is_empty() {
    return haystack.to_vec();
  }
  let mut replaced = Vec::with_capacity(haystack.len());
  let mut remaining = haystack;
  while let Some(index) = remaining
    .windows(needle.len())
    .position(|window| window == needle)
  {
    replaced.extend_from_slice(&remaining[..index]);
    replaced.extend_from_slice(replacement);
    remaining = &remaining[index + needle.len()..];
  }
  replaced.extend_from_slice(remaining);
  replaced
}

#[cfg(feature = "async")]
fn normalize_multipart_boundary(request: &[u8]) -> Vec<u8> {
  let text = request_text(request);
  let boundary = multipart_boundary(&text);
  replace_bytes(
    request_body(request),
    boundary.as_bytes(),
    b"NORMALIZED-BOUNDARY",
  )
}

fn write_temp_file(contents: &[u8]) -> PathBuf {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("system time")
    .as_nanos();
  let path = std::env::temp_dir().join(format!(
    "rttp-multipart-safety-{}-{nanos}.txt",
    std::process::id()
  ));
  std::fs::write(&path, contents).expect("write multipart fixture");
  path
}

fn unsafe_control_values() -> &'static [(&'static str, &'static str)] {
  &[
    ("carriage return", "na\rme"),
    ("line feed", "na\nme"),
    (
      "crlf",
      "na\r\nContent-Disposition: form-data; name=\"evil\"",
    ),
    ("nul", "na\0me"),
    ("other control", "na\u{0001}me"),
    ("delete", "na\u{007f}me"),
  ]
}

fn assert_quoted_string_builder_error(error: rttp_client::error::Error) {
  assert!(error.is_builder(), "unexpected error: {error}");
  assert!(
    error
      .to_string()
      .contains("Invalid multipart Content-Disposition quoted-string"),
    "unexpected error: {error}"
  );
}

fn assert_rejects_without_connect(configure: impl FnOnce(&mut HttpClient, String)) {
  let captured = capture_optional_request(|base_url| {
    let mut request = client();
    configure(&mut request, format!("{base_url}/form"));
    let error = request
      .emit()
      .expect_err("unsafe multipart field must be rejected");
    assert_quoted_string_builder_error(error);
  });
  assert!(
    captured.is_empty(),
    "unsafe multipart field must not open a connection"
  );
}

#[cfg(feature = "async")]
fn assert_async_rejects_without_connect(configure: impl FnOnce(&mut HttpClient, String)) {
  let captured = capture_optional_request(|base_url| {
    let mut request = client();
    configure(&mut request, format!("{base_url}/form"));
    let error = block_on(request.rasync()).expect_err("unsafe multipart field must be rejected");
    assert_quoted_string_builder_error(error);
  });
  assert!(
    captured.is_empty(),
    "unsafe multipart field must not open a connection"
  );
}

fn configure_escaped_parts(client: &mut HttpClient, url: String, file: &Path) {
  client
    .post()
    .url(url)
    .para(Para::with_form("para\"name\\", "pval"))
    .form(FormData::with_text("text\"name\\", "hello"))
    .form(FormData::with_binary(
      "bin\"name\\",
      b"\x00\x01\x02".to_vec(),
    ))
    .form(FormData::with_file_and_name(
      "file\"name\\",
      file,
      "my\"file\\name.txt",
    ));
}

#[test]
fn multipart_escapes_quote_and_backslash_for_text_binary_file_and_parameter_parts() {
  let file = write_temp_file(b"file-bytes");
  let request = capture_request(|base_url| {
    let mut request = client();
    configure_escaped_parts(&mut request, format!("{base_url}/form"), &file);
    request.emit().expect("request should succeed");
  });
  let _ = std::fs::remove_file(&file);

  let text = request_text(&request);
  let body = String::from_utf8(request_body(&request).to_vec()).expect("multipart body utf-8");
  let dispositions: Vec<&str> = body
    .lines()
    .filter(|line| line.starts_with("Content-Disposition:"))
    .collect();

  assert!(text.starts_with("POST /form HTTP/1.1\r\n"));
  assert!(header_value(&text, "Content-Type")
    .expect("content type")
    .starts_with("multipart/form-data; boundary="));
  assert_eq!(
    dispositions,
    [
      r#"Content-Disposition: form-data; name="para\"name\\""#,
      r#"Content-Disposition: form-data; name="text\"name\\""#,
      r#"Content-Disposition: form-data; name="bin\"name\\"; filename="""#,
      r#"Content-Disposition: form-data; name="file\"name\\"; filename="my\"file\\name.txt""#,
    ]
  );
  assert!(!body.contains("\r\nContent-Disposition: form-data; name=\"evil\""));
  assert!(body.contains("hello"));
  assert!(body.contains("pval"));
  assert!(body
    .as_bytes()
    .windows(3)
    .any(|window| window == b"\x00\x01\x02"));
  assert!(body.contains("file-bytes"));
}

#[test]
fn multipart_accepts_htab_and_obs_text_in_quoted_strings() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{base_url}/form"))
      .form(FormData::with_text("na\tme", "ok"))
      .form(FormData::with_text("na\u{00e9}me", "ok"))
      .emit()
      .expect("request should succeed");
  });

  let body = request_text(request_body(&request));
  assert!(body.contains("name=\"na\tme\""));
  assert!(body.contains("name=\"naéme\""));
}

#[test]
fn multipart_rejects_unsafe_controls_in_text_names_before_connect() {
  for (_label, name) in unsafe_control_values() {
    assert_rejects_without_connect(|request, url| {
      request
        .post()
        .url(url)
        .form(FormData::with_text(*name, "ok"));
    });
  }
}

#[test]
fn multipart_rejects_unsafe_controls_in_binary_names_before_connect() {
  for (_label, name) in unsafe_control_values() {
    assert_rejects_without_connect(|request, url| {
      request
        .post()
        .url(url)
        .form(FormData::with_binary(*name, b"data".to_vec()));
    });
  }
}

#[test]
fn multipart_rejects_unsafe_controls_in_file_names_and_filenames_before_connect() {
  let file = write_temp_file(b"unused");
  for (_label, value) in unsafe_control_values() {
    assert_rejects_without_connect(|request, url| {
      request
        .post()
        .url(url)
        .form(FormData::with_file_and_name(*value, &file, "safe.txt"));
    });
    assert_rejects_without_connect(|request, url| {
      request
        .post()
        .url(url)
        .form(FormData::with_file_and_name("safe", &file, *value));
    });
  }
  let _ = std::fs::remove_file(&file);
}

#[test]
fn multipart_rejects_unsafe_controls_in_parameter_names_before_connect() {
  for (_label, name) in unsafe_control_values() {
    assert_rejects_without_connect(|request, url| {
      request
        .post()
        .url(url)
        .para(Para::with_form(*name, "v"))
        .form("ok=1");
    });
  }
}

#[cfg(feature = "async")]
#[test]
fn multipart_async_rejects_unsafe_controls_before_connect() {
  assert_async_rejects_without_connect(|request, url| {
    request
      .post()
      .url(url)
      .form(FormData::with_text("na\r\nme", "ok"));
  });
  assert_async_rejects_without_connect(|request, url| {
    request
      .post()
      .url(url)
      .form(FormData::with_binary("na\0me", b"data".to_vec()));
  });
  let file = write_temp_file(b"unused");
  assert_async_rejects_without_connect(|request, url| {
    request
      .post()
      .url(url)
      .form(FormData::with_file_and_name("safe", &file, "bad\nname.txt"));
  });
  let _ = std::fs::remove_file(&file);
}

#[cfg(feature = "async")]
#[test]
fn multipart_sync_and_async_bodies_match_after_normalizing_boundary() {
  let file = write_temp_file(b"file-bytes");
  let sync_request = capture_request(|base_url| {
    let mut request = client();
    configure_escaped_parts(&mut request, format!("{base_url}/form"), &file);
    request.emit().expect("sync request should succeed");
  });
  let async_request = capture_request(|base_url| {
    let mut request = client();
    configure_escaped_parts(&mut request, format!("{base_url}/form"), &file);
    block_on(request.rasync()).expect("async request should succeed");
  });
  let _ = std::fs::remove_file(&file);

  let sync_text = request_text(&sync_request);
  let async_text = request_text(&async_request);
  assert_eq!(
    replace_bytes(
      header_value(&sync_text, "Content-Type")
        .expect("sync content type")
        .as_bytes(),
      multipart_boundary(&sync_text).as_bytes(),
      b"NORMALIZED-BOUNDARY",
    ),
    replace_bytes(
      header_value(&async_text, "Content-Type")
        .expect("async content type")
        .as_bytes(),
      multipart_boundary(&async_text).as_bytes(),
      b"NORMALIZED-BOUNDARY",
    )
  );
  assert_eq!(
    header_value(&sync_text, "Content-Length"),
    header_value(&async_text, "Content-Length")
  );
  assert_eq!(
    normalize_multipart_boundary(&sync_request),
    normalize_multipart_boundary(&async_request)
  );
}
