use std::net::TcpStream as StdTcpStream;

#[test]
fn request_cache_control_combines_case_insensitive_header_fields() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCache-Control: no-cache, max-age=60\r\ncache-control: min-fresh=30, only-if-cached\r\n\r\n",
  )
  .expect("request should parse");

  let cache_control = request
    .cache_control()
    .expect("Cache-Control should parse")
    .expect("Cache-Control should be present");

  assert!(cache_control.no_cache());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(Some(30), cache_control.min_fresh());
  assert!(cache_control.only_if_cached());
}

#[test]
fn request_raw_parser_rejects_folded_and_bare_lf_headers() {
  for raw in [
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\n second\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\n\tsecond\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\nsecond\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\rsecond\r\n\r\n".as_slice(),
  ] {
    let error = Request::from_raw_frame(raw)
      .expect_err("folded and bare-LF request headers must be rejected");
    assert_eq!(std::io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid request header", error.to_string());
  }
}

#[test]
fn request_raw_parser_preserves_duplicate_ordinary_headers_in_wire_order() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\nx-test: second\r\n\r\n",
  )
  .expect("duplicate ordinary request headers should parse");

  assert_eq!(
    vec!["first", "second"],
    request.headers_named("X-Test").collect::<Vec<_>>()
  );
}

#[test]
fn request_raw_parser_preserves_obs_text_header_values_as_latin1_code_points() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Obs: \x80\xc3\xa9\xff\r\n\r\n",
  )
  .expect("obs-text request header should parse");

  // Request headers use `String`, so raw obs-text bytes cross the API boundary
  // as their corresponding Latin-1 code points.
  assert_eq!(Some("\u{0080}\u{00c3}\u{00a9}\u{00ff}"), request.header("X-Obs"));
}

#[test]
fn request_raw_parser_preserves_non_ows_obs_text_header_value_edges() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Obs: \xa0value\xa0\r\n\r\n",
  )
  .expect("obs-text request header should parse");

  assert_eq!(Some("\u{00a0}value\u{00a0}"), request.header("X-Obs"));
}

#[test]
fn request_cache_control_preserves_malformed_headers_for_handler_policy() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCache-Control: max-age=invalid\r\n\r\n",
  )
  .expect("request should retain malformed metadata");

  assert!(request.cache_control().is_err());
  assert_eq!(Some("max-age=invalid"), request.header("Cache-Control"));
}

#[test]
fn request_cache_control_rejects_oversized_values_without_panicking() {
  let request = Request {
    method: "GET".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![(
      "Cache-Control".to_string(),
      format!("x={}", "a".repeat(MAX_CACHE_CONTROL_VALUE_BYTES)),
    )],
    trailers: Vec::new(),
    body: Vec::new(),
    extended_connect_protocol: None,
  };

  let error = request
    .cache_control()
    .expect_err("oversized Cache-Control should be rejected");

  assert_eq!("Cache-Control header value is too large", error.to_string());
}

#[test]
fn request_cache_control_rejects_directive_counts_across_header_fields() {
  let directive_field = std::iter::repeat_n("extension", MAX_CACHE_CONTROL_DIRECTIVES)
    .collect::<Vec<_>>()
    .join(", ");
  let request = Request {
    method: "GET".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Cache-Control".to_string(), directive_field),
      ("cache-control".to_string(), "another-extension".to_string()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    extended_connect_protocol: None,
  };

  let error = request
    .cache_control()
    .expect_err("too many Cache-Control directives should be rejected");

  assert_eq!("too many Cache-Control directives", error.to_string());
}

#[test]
fn access_control_allow_methods_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Methods", "DELETE")
    .header("access-control-allow-methods", "PATCH")
    .with_access_control_allow_methods("get, POST")
    .expect("Access-Control-Allow-Methods should be accepted");

  let allow_methods = response
    .access_control_allow_methods()
    .expect("Access-Control-Allow-Methods should parse")
    .expect("Access-Control-Allow-Methods should be present");
  assert_eq!(["GET", "POST"], allow_methods.methods());
  assert_eq!(
    vec![("Access-Control-Allow-Methods", "GET, POST")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn access_control_allow_origin_helpers_validate_replace_and_preserve_raw_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Origin", "https://legacy.test")
    .header("access-control-allow-origin", "https://deprecated.test")
    .with_access_control_allow_origin("https://example.test:8443")
    .expect("Access-Control-Allow-Origin should be accepted");

  assert_eq!(
    "https://example.test:8443",
    response
      .access_control_allow_origin()
      .expect("Access-Control-Allow-Origin should parse")
      .expect("Access-Control-Allow-Origin should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Access-Control-Allow-Origin", "https://example.test:8443")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Origin", "https://example.test/path");
  assert!(malformed.access_control_allow_origin().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_origin("https://example.test/path")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .access_control_allow_origin()
      .expect("absent Access-Control-Allow-Origin should parse")
  );
  for value in ["*", "null"] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_access_control_allow_origin(value)
        .expect("valid Access-Control-Allow-Origin should be accepted")
        .access_control_allow_origin()
        .expect("Access-Control-Allow-Origin should parse")
        .expect("Access-Control-Allow-Origin should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Access-Control-Allow-Origin", "https://example.test")
    .header("access-control-allow-origin", "https://other.test");
  assert!(duplicate.access_control_allow_origin().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_origin("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn access_control_allow_headers_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Headers", "X-Legacy")
    .header("access-control-allow-headers", "X-Deprecated")
    .with_access_control_allow_headers("X-Request-Id, ETag")
    .expect("Access-Control-Allow-Headers should be accepted");

  let allow_headers = response
    .access_control_allow_headers()
    .expect("Access-Control-Allow-Headers should parse")
    .expect("Access-Control-Allow-Headers should be present");
  assert_eq!(["x-request-id", "etag"], allow_headers.field_names());
  assert_eq!(
    vec![("Access-Control-Allow-Headers", "x-request-id, etag")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn access_control_allow_headers_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Headers", "X-Request Id");
  assert!(malformed.access_control_allow_headers().is_err());
  assert_eq!(
    Some("X-Request Id"),
    malformed
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Access-Control-Allow-Headers"))
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_access_control_allow_headers("X-Request Id")
    .is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_headers("*")
    .expect("wildcard Access-Control-Allow-Headers should be accepted")
    .access_control_allow_headers()
    .expect("wildcard Access-Control-Allow-Headers should parse")
    .expect("wildcard Access-Control-Allow-Headers should be present")
    .is_wildcard());
}

#[test]
fn access_control_allow_methods_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Methods", "GET POST");
  assert!(malformed.access_control_allow_methods().is_err());
  assert_eq!(
    Some("GET POST"),
    malformed
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Access-Control-Allow-Methods"))
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_access_control_allow_methods("GET POST")
    .is_err());
}

#[test]
fn access_control_allow_methods_helpers_do_not_apply_cors_policy() {
  assert_eq!(
    None,
    HttpResponse::ok([])
      .access_control_allow_methods()
      .expect("absent Access-Control-Allow-Methods should parse")
  );
}

#[test]
fn request_max_forwards_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.max_forwards().expect("missing value should be valid"));

  for value in ["255", "256", "999999999999999999999"] {
    let valid = Request::from_raw_frame(
      format!(
        "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    assert_eq!(
      Some(value.to_owned()),
      valid.max_forwards().expect("value should parse")
    );
  }

  for value in ["", "-1", "+1", "1.0"] {
    let request = Request::from_raw_frame(
      format!(
        "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(request.max_forwards().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Max-Forwards"));
  }

  let duplicate = Request::from_raw_frame(
    b"OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: 1\r\nmax-forwards: 2\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.max_forwards().is_err());
  assert_eq!(Some("1"), duplicate.header("Max-Forwards"));
}

#[test]
fn request_cookies_are_bounded_and_preserve_pairs() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCookie: session=abc; theme=dark\r\nCookie: flag=\r\n\r\n",
  )
  .expect("request should parse");
  let cookies = request
    .cookies()
    .expect("cookie metadata should parse")
    .expect("Cookie header should be present");
  assert_eq!(
    vec![("session", "abc"), ("theme", "dark"), ("flag", "")],
    cookies
      .pairs()
      .iter()
      .map(|pair| (pair.name(), pair.value()))
      .collect::<Vec<_>>()
  );

  assert!(HttpCookies::parse("session=abc\x01").is_err());
}

#[test]
fn request_exposes_bounded_range_and_conditional_metadata() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=-4\r\n",
    "If-Range: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "If-None-Match: *\r\n",
    "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "\r\n"
  )
  .as_bytes())
  .expect("request should retain metadata");

  assert_eq!(Some(HttpByteRange::new(6, 9)), request.range(10).expect("Range should parse"));
  assert!(matches!(request.if_range(), Ok(Some(HttpIfRange::Date(_)))));
  assert_eq!(Ok(Some(HttpIfNoneMatch::Any)), request.if_none_match());
  assert!(matches!(request.if_modified_since(), Ok(Some(_))));
}

  #[test]
  fn http2_huffman_decode_table_resolves_symbols_without_linear_scan() {
    let table = http2_huffman_decode_table();

    assert_eq!(Some(b'0' as u16), table.decode_symbol(0x00, 5));
    assert_eq!(Some(b'.' as u16), table.decode_symbol(0x17, 6));
    assert_eq!(Some(b'/' as u16), table.decode_symbol(0x18, 6));
    assert_eq!(
      Some(HTTP2_HUFFMAN_EOS),
      table.decode_symbol(0x3fff_ffff, 30)
    );
    assert_eq!(None, table.decode_symbol(0x00, 1));
  }

  #[test]
  fn http2_window_update_rejects_zero_increment() {
    let error = http2_window_update_increment(&[0, 0, 0, 0])
      .expect_err("zero WINDOW_UPDATE increment must be rejected");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid HTTP/2 WINDOW_UPDATE frame", error.to_string());
  }

  #[test]
  fn http2_send_window_rejects_increment_above_maximum() {
    let mut window = Http2SendWindow::new(1);

    let error = window
      .increase(0x7fff_ffff)
      .expect_err("WINDOW_UPDATE must not exceed the HTTP/2 maximum window");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("HTTP/2 flow-control window overflow", error.to_string());
    assert_eq!(1, window.size);
  }

  #[test]
  fn response_flow_control_reads_update_accepted_stream_tracking() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
    let mut client =
      StdTcpStream::connect(listener.local_addr().expect("listener addr")).expect("client connect");
    let (mut server, _) = listener.accept().expect("server accept");
    let header_block = [0x82, 0x84, 0x86];

    write_http2_frame(
      &mut client,
      HTTP2_FRAME_HEADERS,
      HTTP2_FLAG_END_HEADERS | HTTP2_FLAG_END_STREAM,
      3,
      &header_block,
    )
    .expect("client HEADERS frame should write");
    write_http2_window_update(&mut client, 1, 1).expect("client WINDOW_UPDATE should write");
    client.flush().expect("client frames should flush");

    let mut max_frame_size = HTTP2_DEFAULT_MAX_FRAME_SIZE;
    let mut peer_header_table_size = HTTP2_DEFAULT_HEADER_TABLE_SIZE;
    let mut peer_initial_stream_send_window = HTTP2_DEFAULT_INITIAL_WINDOW_SIZE;
    let mut connection_send_window = Http2SendWindow::new(0);
    let mut connection_receive_window = HTTP2_DEFAULT_INITIAL_WINDOW_SIZE;
    let mut stream_send_window = Http2SendWindow::new(0);
    let mut streams = Vec::new();
    let mut reset_streams = Vec::new();
    let mut stream_ids = Http2ClientStreamIds::new();
    let mut request_header_decoder = Http2HeaderDecoder::new(HTTP2_DEFAULT_HEADER_TABLE_SIZE);
    let mut accepted_stream_count = 1;
    let mut last_accepted_stream_id = 1;
    let mut peer_enable_connect_protocol = false;

    let read = {
      let mut flow_control = Http2ResponseFlowControl {
        max_inbound_frame_size: HTTP2_DEFAULT_MAX_FRAME_SIZE,
        max_header_list_size: HTTP2_MAX_HEADER_LIST_SIZE,
        max_frame_size: &mut max_frame_size,
        peer_header_table_size: &mut peer_header_table_size,
        peer_initial_stream_send_window: &mut peer_initial_stream_send_window,
        peer_enable_connect_protocol: &mut peer_enable_connect_protocol,
        connection_send_window: &mut connection_send_window,
        connection_receive_window: &mut connection_receive_window,
        stream_send_window: &mut stream_send_window,
        streams: &mut streams,
        reset_streams: &mut reset_streams,
        stream_ids: &mut stream_ids,
        request_header_decoder: &mut request_header_decoder,
        accepted_stream_count: &mut accepted_stream_count,
        last_accepted_stream_id: &mut last_accepted_stream_id,
      };
      read_http2_response_flow_control_frame(&mut server, 1, &mut flow_control)
        .expect("flow-control read should accept request frames")
    };

    assert!(matches!(
      read,
      Http2ResponseFlowControlRead::WindowAvailable
    ));
    assert_eq!(2, accepted_stream_count);
    assert_eq!(3, last_accepted_stream_id);
    assert_eq!(1, streams.len());
    assert_eq!(3, streams[0].stream_id);
    assert!(streams[0].is_complete());
  }

  #[test]
  fn read_next_from_consumes_one_fully_framed_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello",
      "POST /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "world"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let second = Request::read_next_from(&mut reader)
      .expect("second frame should parse")
      .expect("second request should be present");

    assert_eq!("POST", first.method());
    assert_eq!("/first", first.target());
    assert_eq!(b"hello", first.body());
    assert_eq!("POST", second.method());
    assert_eq!("/second", second.target());
    assert_eq!(b"world", second.body());
    assert!(reader.fill_buf().expect("remaining bytes").is_empty());
  }

  #[test]
  fn read_next_from_rejects_conflicting_duplicate_content_length() {
    let raw = concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 6\r\n",
      "\r\n",
      "hello!"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("conflicting Content-Length should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("conflicting Content-Length headers", error.to_string());
  }

  #[test]
  fn read_next_from_accepts_duplicate_matching_content_length() {
    let raw = concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("matching duplicate Content-Length should parse")
      .expect("request should be present");

    assert_eq!("POST", request.method());
    assert_eq!("/upload", request.target());
    assert_eq!(b"hello", request.body());
  }

  #[test]
  fn read_next_from_enforces_request_body_framing_conflict_matrix() {
    let cases = [
      (
        "matching duplicate Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Content-Length: 5\r\n",
          "Content-Length: 5\r\n",
          "\r\n",
          "hello"
        )
        .as_bytes(),
        Ok(b"hello".as_slice()),
      ),
      (
        "conflicting duplicate Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Content-Length: 5\r\n",
          "Content-Length: 6\r\n",
          "\r\n",
          "hello!"
        )
        .as_bytes(),
        Err("conflicting Content-Length headers"),
      ),
      (
        "Transfer-Encoding with Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "Content-Length: 0\r\n",
          "\r\n",
          "0\r\n\r\n"
        )
        .as_bytes(),
        Err("Transfer-Encoding conflicts with Content-Length"),
      ),
      (
        "empty chunked body",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "\r\n",
          "0\r\n\r\n"
        )
        .as_bytes(),
        Ok(b"".as_slice()),
      ),
      (
        "malformed chunk terminator",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "\r\n",
          "5\r\nhelloXX0\r\n\r\n"
        )
        .as_bytes(),
        Err("invalid chunk terminator"),
      ),
    ];

    for (name, raw, expected) in cases {
      let mut reader = BufReader::new(Cursor::new(raw));
      match expected {
        Ok(body) => {
          let request = Request::read_next_from(&mut reader)
            .unwrap_or_else(|error| panic!("{name} should parse: {error}"))
            .unwrap_or_else(|| panic!("{name} should include a request"));
          assert_eq!(body, request.body(), "{name}");
        }
        Err(message) => {
          let error = Request::read_next_from(&mut reader)
            .expect_err("{name} should be rejected");
          assert_eq!(io::ErrorKind::InvalidData, error.kind(), "{name}");
          assert_eq!(message, error.to_string(), "{name}");
        }
      }
    }
  }

  #[test]
  fn read_next_from_consumes_one_chunked_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "\r\n",
      "GET /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let second = Request::read_next_from(&mut reader)
      .expect("second frame should parse")
      .expect("second request should be present");

    assert_eq!("POST", first.method());
    assert_eq!("/first", first.target());
    assert_eq!(b"hello", first.body());
    assert_eq!("GET", second.method());
    assert_eq!("/second", second.target());
    assert!(reader.fill_buf().expect("remaining bytes").is_empty());
  }

  #[test]
  fn read_next_from_accepts_obs_text_in_quoted_chunk_extensions() {
    let raw = b"POST /chunked HTTP/1.1\r\n\
Host: example.test\r\n\
Transfer-Encoding: chunked\r\n\
\r\n\
5;meta=\"\xff\"\r\n\
hello\r\n\
0\r\n\
\r\n";
    let mut reader = BufReader::new(Cursor::new(raw));

    let request = Request::read_next_from(&mut reader)
      .expect("chunk extension with obs-text should parse")
      .expect("request should be present");

    assert_eq!("/chunked", request.target());
    assert_eq!(b"hello", request.body());
  }

  #[test]
  fn read_next_from_rejects_invalid_chunk_size_characters() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5G\r\nhello\r\n",
      "0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("invalid chunk size should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid chunk size", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_oversized_chunk_size_line() {
    let chunk_size = "1".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "{}\r\n",
        "x\r\n",
        "0\r\n",
        "\r\n"
      ),
      chunk_size
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("oversized chunk size line should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_missing_crlf_after_chunk_data() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello",
      "0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("missing chunk data terminator should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid chunk terminator", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_malformed_trailer_termination() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("missing trailer terminator should fail");

    assert_eq!(io::ErrorKind::UnexpectedEof, error.kind());
    assert_eq!("incomplete chunked request body", error.to_string());
  }

  #[test]
  fn connection_close_request_marks_keep_alive_loop_terminal() {
    let raw = concat!(
      "POST /final HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: close\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "done",
      "GET /ignored HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("request frame should parse")
      .expect("request should be present");

    assert_eq!("/final", request.target());
    assert_eq!(b"done", request.body());
    assert!(request.closes_connection());
    assert!(reader
      .fill_buf()
      .expect("remaining bytes")
      .starts_with(b"GET /ignored"));
  }

  #[test]
  fn partial_second_request_returns_unexpected_eof_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "POST /partial HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "he"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::UnexpectedEof, error.kind());
    assert_eq!("incomplete HTTP request body", error.to_string());
  }

  #[test]
  fn chunk_extension_bytes_count_toward_request_body_limit() {
    let chunk_extension = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0;{}\r\n",
        "\r\n"
      ),
      chunk_extension
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk extension should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn chunk_trailer_bytes_count_toward_request_body_limit() {
    let trailer_value = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0\r\n",
        "X-Trace: {}\r\n",
        "\r\n"
      ),
      trailer_value
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk trailer should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn malformed_second_request_returns_invalid_data_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "GET /broken HTTP/1.1\r\n",
      "Host example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid request header", error.to_string());
  }

  #[test]
  fn priority_helpers_parse_requests_and_build_responses() {
    let raw = concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Priority: u=1, i, x=token\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");
    let priority = request
      .priority()
      .expect("Priority should parse")
      .expect("Priority should be present");

    assert_eq!(Some(1), priority.urgency());
    assert!(priority.incremental());
    assert_eq!(Some("token"), priority.extensions()[0].value());

    let response = HttpResponse::ok([])
      .with_priority("u=1, i, x=token")
      .expect("Priority should be accepted");
    assert_eq!(
      "u=1, i, x=token",
      response
        .priority()
        .expect("response Priority should parse")
        .expect("response Priority should be present")
        .header_value()
    );
  }

  #[test]
  fn authentication_helpers_parse_bounded_metadata_without_authentication_policy() {
    let raw = concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Authorization: Bearer origin-token\r\n",
      "Proxy-Authorization: Basic cHJveHk6c2VjcmV0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");

    assert_eq!(
      "Bearer",
      request
        .authorization()
        .expect("Authorization should parse")
        .expect("Authorization should be present")
        .scheme()
    );
    let proxy_authorization = request
      .proxy_authorization()
      .expect("Proxy-Authorization should parse")
      .expect("Proxy-Authorization should be present");
    assert_eq!("Basic", proxy_authorization.scheme());
    assert_eq!("cHJveHk6c2VjcmV0", proxy_authorization.credentials());

    let response = HttpResponse::new(401, "Unauthorized")
      .header("WWW-Authenticate", "Broken")
      .with_www_authenticate("Basic realm=\"private\", Bearer")
      .expect("WWW-Authenticate should be accepted");
    let challenges = response
      .www_authenticate()
      .expect("WWW-Authenticate should parse")
      .expect("WWW-Authenticate should be present");
    assert_eq!(2, challenges.challenges().len());
    assert_eq!("Basic", challenges.challenges()[0].scheme());
    assert_eq!("Bearer", challenges.challenges()[1].scheme());
    assert!(HttpResponse::ok([])
      .with_www_authenticate("Basic @")
      .is_err());
    let malformed = HttpRequest::parse(
      concat!(
        "GET / HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Proxy-Authorization: invalid\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    assert!(malformed.proxy_authorization().is_err());
    assert_eq!(
      Some("invalid"),
      malformed.header("Proxy-Authorization")
    );
  }

  #[test]
  fn alt_svc_helpers_validate_build_and_parse_response_metadata() {
    let response = HttpResponse::ok([])
      .with_alt_svc("h3=\":443\"; ma=3600; persist=1; region=\"us-east\"")
      .expect("Alt-Svc should be accepted");
    let alt_svc = response
      .alt_svc()
      .expect("response Alt-Svc should parse")
      .expect("response Alt-Svc should be present");

    assert_eq!("h3", alt_svc.alternatives()[0].protocol_id());
    assert_eq!(":443", alt_svc.alternatives()[0].authority());
    assert_eq!(Some(3600), alt_svc.alternatives()[0].max_age());
    assert_eq!(Some(true), alt_svc.alternatives()[0].persist());
    assert_eq!(
      "h3=\":443\"; ma=3600; persist=1; region=us-east",
      alt_svc.header_value()
    );

    let clear = HttpResponse::ok([])
      .with_alt_svc("clear")
      .expect("clear should be accepted");
    assert!(clear
      .alt_svc()
      .expect("clear should parse")
      .expect("clear should be present")
      .is_clear());
  }
