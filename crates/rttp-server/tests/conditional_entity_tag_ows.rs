use std::io::Write;
use std::net::TcpStream;

use rttp_server::server::{
  HttpConditionalMetadata, HttpConditionalRequestOutcome, HttpEntityTag, HttpResponse, HttpServer,
  Request,
};

const NBSP: char = '\u{00a0}';
const EM_SPACE: char = '\u{2003}';
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

fn evaluate_once(raw: &str, metadata: HttpConditionalMetadata) -> HttpConditionalRequestOutcome {
  let server = HttpServer::bind(("127.0.0.1", 0)).expect("server should bind");
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  client
    .write_all(raw.as_bytes())
    .expect("client should send request");

  let mut outcome = None;
  server
    .accept_one(|request: Request| {
      outcome = Some(request.evaluate_conditional(&metadata));
      HttpResponse::ok("")
    })
    .expect("server should accept request");
  outcome.expect("conditional outcome should be recorded")
}

fn get_request(header_name: &str, value: &str) -> String {
  format!("GET /asset HTTP/1.1\r\nHost: example.test\r\n{header_name}: {value}\r\n\r\n")
}

#[test]
fn if_match_accepts_http_ows_around_wildcard_and_tags() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  for padding in [" ", "\t", " \t", "\t "] {
    let wildcard = get_request("If-Match", &format!("{padding}*{padding}"));
    assert_eq!(
      HttpConditionalRequestOutcome::Proceed,
      evaluate_once(&wildcard, metadata.clone()),
      "HTTP OWS should be accepted around If-Match wildcard"
    );

    let matching = get_request("If-Match", &format!("{padding}\"revision-42\"{padding}"));
    assert_eq!(
      HttpConditionalRequestOutcome::Proceed,
      evaluate_once(&matching, metadata.clone()),
      "HTTP OWS should be accepted around If-Match entity tags"
    );

    let list = get_request(
      "If-Match",
      &format!("{padding}\"other\"{padding},{padding}\"revision-42\"{padding}"),
    );
    assert_eq!(
      HttpConditionalRequestOutcome::Proceed,
      evaluate_once(&list, metadata.clone()),
      "HTTP OWS should be accepted around If-Match list members"
    );
  }
}

#[test]
fn if_none_match_accepts_http_ows_around_wildcard_and_tags() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  for padding in [" ", "\t", " \t", "\t "] {
    let wildcard = get_request("If-None-Match", &format!("{padding}*{padding}"));
    assert_eq!(
      HttpConditionalRequestOutcome::NotModified,
      evaluate_once(&wildcard, metadata.clone()),
      "HTTP OWS should be accepted around If-None-Match wildcard"
    );

    let matching = get_request(
      "If-None-Match",
      &format!("{padding}\"revision-42\"{padding}"),
    );
    assert_eq!(
      HttpConditionalRequestOutcome::NotModified,
      evaluate_once(&matching, metadata.clone()),
      "HTTP OWS should be accepted around If-None-Match entity tags"
    );

    let list = get_request(
      "If-None-Match",
      &format!("{padding}\"other\"{padding},{padding}\"revision-42\"{padding}"),
    );
    assert_eq!(
      HttpConditionalRequestOutcome::NotModified,
      evaluate_once(&list, metadata.clone()),
      "HTTP OWS should be accepted around If-None-Match list members"
    );
  }
}

#[test]
fn if_match_rejects_non_ows_padding_and_ignores_the_header() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  for whitespace in [NBSP, EM_SPACE, IDEOGRAPHIC_SPACE] {
    for value in [
      format!("{whitespace}*"),
      format!("*{whitespace}"),
      format!("{whitespace}\"revision-42\""),
      format!("\"revision-42\"{whitespace}"),
      format!("\"other\",{whitespace}\"revision-42\""),
      format!("\"revision-42\"{whitespace},\"other\""),
    ] {
      let raw = get_request("If-Match", &value);
      assert_eq!(
        HttpConditionalRequestOutcome::Proceed,
        evaluate_once(&raw, metadata.clone()),
        "non-OWS If-Match padding should be ignored: {value:?}"
      );
    }
  }
}

#[test]
fn if_none_match_rejects_non_ows_padding_and_ignores_the_header() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  for whitespace in [NBSP, EM_SPACE, IDEOGRAPHIC_SPACE] {
    for value in [
      format!("{whitespace}*"),
      format!("*{whitespace}"),
      format!("{whitespace}\"revision-42\""),
      format!("\"revision-42\"{whitespace}"),
      format!("\"other\",{whitespace}\"revision-42\""),
      format!("\"revision-42\"{whitespace},\"other\""),
    ] {
      let raw = get_request("If-None-Match", &value);
      assert_eq!(
        HttpConditionalRequestOutcome::Proceed,
        evaluate_once(&raw, metadata.clone()),
        "non-OWS If-None-Match padding should be ignored: {value:?}"
      );
    }
  }
}

#[test]
fn preserves_strong_if_match_and_weak_if_none_match_comparison() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  let weak_if_match = get_request("If-Match", " \tW/\"revision-42\"\t ");
  assert_eq!(
    HttpConditionalRequestOutcome::PreconditionFailed,
    evaluate_once(&weak_if_match, metadata.clone()),
    "If-Match should keep strong comparison with OWS-padded weak tags"
  );

  let weak_if_none_match = get_request("If-None-Match", " \tW/\"revision-42\"\t ");
  assert_eq!(
    HttpConditionalRequestOutcome::NotModified,
    evaluate_once(&weak_if_none_match, metadata),
    "If-None-Match should keep weak comparison with OWS-padded weak tags"
  );
}
