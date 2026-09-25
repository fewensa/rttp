use rttp_server::server::{HttpIfNoneMatch, HttpRequest};

fn request_with_headers(headers: &str) -> HttpRequest {
  let raw = format!("GET /asset HTTP/1.1\r\nHost: example.test\r\n{headers}\r\n");
  HttpRequest::parse(raw.as_bytes()).expect("request should parse")
}

#[test]
fn parses_http_ows_around_wildcards_and_entity_tags() {
  for padding in [" ", "\t", " \t", "\t "] {
    assert_eq!(
      Ok(HttpIfNoneMatch::Any),
      HttpIfNoneMatch::parse(format!("{padding}*{padding}")),
      "HTTP OWS should be accepted around a wildcard"
    );

    let parsed = HttpIfNoneMatch::parse(format!(r#"{padding}"revision-42"{padding}"#))
      .expect("HTTP OWS should be accepted around an entity tag");
    assert!(matches!(parsed, HttpIfNoneMatch::Tags(tags) if tags.len() == 1));
  }
}

#[test]
fn rejects_non_ows_padding_in_public_parser() {
  for whitespace in ["\r", "\n", "\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}"] {
    for value in [
      format!("{whitespace}*"),
      format!("*{whitespace}"),
      format!(r#"{whitespace}"revision-42""#),
      format!(r#""revision-42"{whitespace}"#),
    ] {
      assert!(
        HttpIfNoneMatch::parse(&value).is_err(),
        "non-OWS padding should be rejected: {value:?}"
      );
    }
  }
}

#[test]
fn request_if_none_match_accessor_uses_ows_only_parser() {
  let valid = request_with_headers("If-None-Match: \t\"revision-42\" \t\r\n");
  assert!(valid
    .if_none_match()
    .expect("valid If-None-Match should parse")
    .is_some());

  for whitespace in ["\u{00a0}", "\u{2003}"] {
    let request = request_with_headers(&format!("If-None-Match: {whitespace}*\r\n"));
    assert!(
      request.if_none_match().is_err(),
      "request accessor should reject non-OWS padding: {whitespace:?}"
    );
  }
}
