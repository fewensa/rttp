use rttp_server::server::{HttpResponse, HttpVary};

#[test]
fn parse_accepts_http_ows_around_field_names_and_wildcard() {
  for padding in [" ", "\t", " \t", "\t "] {
    let vary = HttpVary::parse(format!(
      "{padding}Accept-Encoding{padding},{padding}User-Agent{padding}"
    ))
    .expect("HTTP OWS should be accepted around field names");
    assert_eq!(vec!["accept-encoding", "user-agent"], vary.field_names());

    let wildcard = HttpVary::parse(format!("{padding}*{padding}"))
      .expect("HTTP OWS should be accepted around wildcard");
    assert!(wildcard.is_wildcard());
    assert!(wildcard.field_names().is_empty());
  }
}

#[test]
fn parse_rejects_non_ows_padding_around_field_names_and_wildcard() {
  for whitespace in [
    "\r", "\n", "\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}", "\u{3000}",
  ] {
    for value in [
      format!("{whitespace}Accept-Encoding"),
      format!("Accept-Encoding{whitespace}"),
      format!("{whitespace}Accept-Encoding{whitespace}"),
      format!("Accept-Encoding,{whitespace}User-Agent"),
      format!("Accept-Encoding{whitespace},User-Agent"),
      format!("{whitespace}*"),
      format!("*{whitespace}"),
      format!("{whitespace}*{whitespace}"),
    ] {
      assert!(
        HttpVary::parse(&value).is_err(),
        "non-OWS padding should be rejected: {value:?}"
      );
    }
  }
}

#[test]
fn with_vary_and_response_vary_use_ows_only_rules() {
  for padding in [" ", "\t", " \t"] {
    let response = HttpResponse::ok("body")
      .with_vary(format!("{padding}Accept-Encoding{padding}"))
      .expect("with_vary should accept HTTP OWS");
    let vary = response
      .vary()
      .expect("response.vary should parse OWS-padded Vary")
      .expect("Vary should be present");
    assert_eq!(vec!["accept-encoding"], vary.field_names());

    let attached = HttpResponse::ok("body").header(
      "Vary",
      format!("{padding}User-Agent{padding},{padding}Accept-Language{padding}"),
    );
    let attached_vary = attached
      .vary()
      .expect("response.vary should accept OWS on attached Vary")
      .expect("Vary should be present");
    assert_eq!(
      vec!["user-agent", "accept-language"],
      attached_vary.field_names()
    );
  }

  let ows_wildcard = HttpResponse::ok("body")
    .with_vary(" \t* \t")
    .expect("with_vary should accept OWS-padded wildcard");
  assert!(ows_wildcard
    .vary()
    .expect("response.vary should parse")
    .expect("Vary should be present")
    .is_wildcard());

  for whitespace in ["\u{00a0}", "\u{2003}", "\u{3000}"] {
    assert!(
      HttpResponse::ok("body")
        .with_vary(format!("{whitespace}Accept-Encoding"))
        .is_err(),
      "with_vary should reject non-OWS padding: {whitespace:?}"
    );
    assert!(
      HttpResponse::ok("body")
        .with_vary(format!("*{whitespace}"))
        .is_err(),
      "with_vary should reject non-OWS around wildcard: {whitespace:?}"
    );

    let response =
      HttpResponse::ok("body").header("Vary", format!("{whitespace}Accept-Encoding{whitespace}"));
    assert!(
      response.vary().is_err(),
      "response.vary should reject non-OWS padding: {whitespace:?}"
    );

    let wildcard = HttpResponse::ok("body").header("Vary", format!("{whitespace}*"));
    assert!(
      wildcard.vary().is_err(),
      "response.vary should reject non-OWS around wildcard: {whitespace:?}"
    );
  }
}
