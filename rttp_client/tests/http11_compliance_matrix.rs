#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::HttpClient;
use rttp_http11_test_fixtures as fixtures;

fn client() -> HttpClient {
  HttpClient::new()
}

const NO_BODY_STATUS_WITH_FRAMING_CASES: &[(&str, &[u8], u32, &str, &str)] = &[
  (
    "204 content-length",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    204,
    "Content-Length",
    "7",
  ),
  (
    "204 chunked",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    204,
    "Transfer-Encoding",
    "chunked",
  ),
  (
    "304 content-length",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    304,
    "Content-Length",
    "7",
  ),
  (
    "304 chunked",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    304,
    "Transfer-Encoding",
    "chunked",
  ),
];

const ORDINARY_200_FRAMING_CASES: &[(&str, &[u8], &str, &str, &str)] = &[
  (
    "200 content-length",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes(),
    "Content-Length",
    "2",
    "OK",
  ),
  (
    "200 chunked",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nchunked\r\n",
      "6\r\n body!\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
    "Transfer-Encoding",
    "chunked",
    "chunked body!",
  ),
];

#[test]
fn sync_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  let response = client()
    .get()
    .url(format!("http://{}/matrix/chunked", addr))
    .emit()
    .expect("sync response should parse");

  assert_eq!(200, response.code());
  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
  assert_eq!(
    Some(&"signed".to_string()),
    response.trailer_value("X-SIGNATURE")
  );

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
fn sync_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/no-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(*status, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!("", response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/ordinary-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(200, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!(*body, response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  let error = client()
    .get()
    .url(format!("http://{}/matrix/ambiguous", addr))
    .emit()
    .expect_err("ambiguous response should be rejected");

  assert!(
    error.to_string().contains("Content-Length"),
    "unexpected error: {error}"
  );
  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_waits_for_shared_expect_continue_fixture() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  let response = client()
    .post()
    .url(format!("http://{}{}", addr, fixture.target))
    .header(("Expect", "100-continue"))
    .raw(String::from_utf8_lossy(fixture.body).as_ref())
    .emit()
    .expect("expect-continue response should parse");

  assert_eq!(200, response.code());
  assert_eq!("accepted", response.body().string().unwrap());

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}

#[test]
#[cfg(feature = "async")]
fn async_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/matrix/chunked", addr))
      .rasync()
      .await
      .expect("async response should parse");

    assert_eq!(200, response.code());
    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
    assert_eq!(
      Some(&"signed".to_string()),
      response.trailer_value("X-SIGNATURE")
    );
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
#[cfg(feature = "async")]
fn async_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/no-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(*status, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!("", response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/ordinary-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(200, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!(*body, response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/matrix/ambiguous", addr))
      .rasync()
      .await
      .expect_err("ambiguous response should be rejected");

    assert!(
      error.to_string().contains("Content-Length"),
      "unexpected error: {error}"
    );
  });

  handle.join().expect("raw response server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_client_waits_for_shared_expect_continue_fixture() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}{}", addr, fixture.target))
      .header(("Expect", "100-continue"))
      .raw(String::from_utf8_lossy(fixture.body).as_ref())
      .rasync()
      .await
      .expect("expect-continue response should parse");

    assert_eq!(200, response.code());
    assert_eq!("accepted", response.body().string().unwrap());
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}
