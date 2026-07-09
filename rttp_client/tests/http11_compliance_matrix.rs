#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{HttpByteRange, HttpByteRangeError, HttpResponse, Request};
use rttp_client::HttpClient;
use rttp_http11_test_fixtures as fixtures;

fn client() -> HttpClient {
  HttpClient::new()
}

const RANGE_BODY: &[u8] = b"0123456789abcdef";

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

fn range_response(request: Request) -> HttpResponse {
  match request.header("range") {
    Some(range_header) => match HttpByteRange::parse(range_header, RANGE_BODY.len()) {
      Ok(range) => HttpResponse::partial_content(RANGE_BODY, range),
      Err(HttpByteRangeError::UnsatisfiedRange) => {
        HttpResponse::range_not_satisfiable(RANGE_BODY.len())
      }
      Err(error) => HttpResponse::new(400, "Bad Request").body(error.to_string()),
    },
    None => HttpResponse::ok(RANGE_BODY),
  }
}

fn spawn_range_server() -> (std::net::SocketAddr, thread::JoinHandle<Option<String>>) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send observed range");
        range_response(request)
      })
      .expect("serve range request");
    rx.recv().expect("observed range")
  });

  (addr, handle)
}

fn assert_partial_response(
  name: &str,
  response: rttp_client::response::Response,
  expected_content_range: &str,
  expected_body: &str,
) {
  assert_eq!(206, response.code(), "{name}");
  assert!(response.is_partial_content(), "{name}");
  assert_eq!(
    Some(&expected_content_range.to_string()),
    response.header_value("Content-Range"),
    "{name}"
  );
  assert_eq!(expected_body, response.body().string().unwrap(), "{name}");
}

fn assert_observed_range(
  handle: thread::JoinHandle<Option<String>>,
  expected_range: &str,
  name: &str,
) {
  assert_eq!(
    Some(expected_range.to_string()),
    handle.join().expect("range server thread"),
    "{name}"
  );
}

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
fn sync_client_range_helpers_interoperate_with_server_partial_content_helper() {
  for (name, expected_range, expected_content_range, expected_body, request) in [
    (
      "bounded range",
      "bytes=3-7",
      "bytes 3-7/16",
      "34567",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted");
      }) as Box<dyn Fn(&mut HttpClient)>,
    ),
    (
      "open-ended range",
      "bytes=12-",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client.range_from(12);
      }),
    ),
    (
      "suffix range",
      "bytes=-4",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_suffix(4)
          .expect("suffix range should be accepted");
      }),
    ),
  ] {
    let (addr, handle) = spawn_range_server();
    let mut client = client();
    client.get().url(format!("http://{}/matrix/range", addr));
    request(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_partial_response(name, response, expected_content_range, expected_body);
    assert_observed_range(handle, expected_range, name);
  }
}

#[test]
fn sync_client_unsatisfied_range_maps_to_server_416_response() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_from(RANGE_BODY.len() as u64)
    .emit()
    .expect("unsatisfied range response should parse");

  assert_eq!(416, response.code());
  assert!(response.is_range_not_satisfiable());
  assert_eq!(
    Some(&format!("bytes */{}", RANGE_BODY.len())),
    response.header_value("Content-Range")
  );
  assert_eq!("", response.body().string().unwrap());
  assert_observed_range(
    handle,
    &format!("bytes={}-", RANGE_BODY.len()),
    "unsatisfied range",
  );
}

#[test]
fn sync_client_malformed_range_helpers_reject_before_reaching_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send unexpected range");
        HttpResponse::ok("unexpected")
      })
      .expect("serve optional range request");
  });

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range(7, 3)
    .expect_err("inverted range should be rejected");
  assert!(error.is_builder());

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_suffix(0)
    .expect_err("empty suffix should be rejected");
  assert!(error.is_builder());

  assert!(
    rx.recv_timeout(Duration::from_millis(100)).is_err(),
    "malformed helper input should not reach the range server"
  );

  let mut stream = TcpStream::connect(addr).expect("release range server");
  stream
    .write_all(b"GET /matrix/release HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    .expect("write release request");
  assert_eq!(None, rx.recv().expect("release request observed range"));
  handle.join().expect("range server thread");
}

#[test]
fn sync_client_manual_range_header_interoperates_with_server_partial_content_helper() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .header(("Range", "bytes=5-9"))
    .emit()
    .expect("manual range response should parse");

  assert_partial_response("manual range", response, "bytes 5-9/16", "56789");
  assert_observed_range(handle, "bytes=5-9", "manual range");
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
