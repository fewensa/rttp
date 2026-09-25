use rttp_server::server::{HttpResponse, HttpVary};

#[test]
fn accepts_http_ows_around_field_names_and_wildcard() {
  for padding in [" ", "\t", " \t", "\t "] {
    let vary = HttpVary::parse(format!("{padding}Accept-Encoding{padding}"))
      .expect("HTTP OWS should be accepted around a field name");
    assert_eq!(vec!["accept-encoding"], vary.field_names());

    let list = HttpVary::parse(format!(
      "{padding}Accept-Encoding{padding},{padding}User-Agent{padding}"
    ))
    .expect("HTTP OWS should be accepted around list members");
    assert_eq!(vec!["accept-encoding", "user-agent"], list.field_names());

    let wildcard = HttpVary::parse(format!("{padding}*{padding}"))
      .expect("HTTP OWS should be accepted around a wildcard");
    assert!(wildcard.is_wildcard());
  }
}

#[test]
fn rejects_non_ows_padding_around_field_names_and_wildcard() {
  for whitespace in ["\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}"] {
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
fn with_vary_rejects_non_ows_padding() {
  for whitespace in ["\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}"] {
    assert!(
      HttpResponse::ok("body")
        .with_vary(format!("{whitespace}Accept-Encoding"))
        .is_err(),
      "with_vary should reject non-OWS padding: {whitespace:?}"
    );
  }

  let response = HttpResponse::ok("body")
    .with_vary(" \tAccept-Encoding\t , \tUser-Agent \t")
    .expect("with_vary should accept HTTP OWS");
  let vary = response
    .vary()
    .expect("OWS-padded with_vary should round-trip")
    .expect("Vary should be present");
  assert_eq!(vec!["accept-encoding", "user-agent"], vary.field_names());
}

#[test]
fn response_vary_accessor_rejects_non_ows_raw_headers() {
  let valid = HttpResponse::ok("body").header("Vary", " \tAccept-Encoding\t ");
  let vary = valid
    .vary()
    .expect("OWS-padded Vary should parse")
    .expect("Vary should be present");
  assert_eq!(vec!["accept-encoding"], vary.field_names());

  for whitespace in ["\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}"] {
    let response = HttpResponse::ok("body").header("Vary", format!("{whitespace}Accept-Encoding"));
    assert!(
      response.vary().is_err(),
      "vary() should reject non-OWS raw header padding: {whitespace:?}"
    );
  }
}

#[test]
fn preserves_normalization_dedupe_wildcard_and_bounds() {
  let normalized = HttpVary::parse("Accept-Encoding, accept-encoding, USER-AGENT")
    .expect("case-insensitive dedupe should remain valid");
  assert_eq!(
    vec!["accept-encoding", "user-agent"],
    normalized.field_names()
  );

  let wildcard = HttpVary::parse(" \t*\t ").expect("OWS-padded wildcard should parse");
  assert!(wildcard.is_wildcard());
  assert!(wildcard.field_names().is_empty());

  assert!(
    HttpVary::parse("*, Accept-Encoding").is_err(),
    "wildcard exclusivity should remain enforced"
  );

  let too_many = std::iter::repeat_n("Accept-Encoding", 257)
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpVary::parse(too_many).is_err(),
    "field-name bound should remain enforced"
  );

  let multi = HttpVary::parse_values(["Accept-Encoding", " \tUser-Agent\t ", "accept-encoding"])
    .expect("repeated values should parse with OWS");
  assert_eq!(vec!["accept-encoding", "user-agent"], multi.field_names());
}
