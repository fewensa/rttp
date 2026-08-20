use rttp_protocol::via::{Via, MAX_VIA_MEMBERS, MAX_VIA_VALUE_BYTES};

#[test]
fn via_parses_rfc_examples_and_combined_fields() {
  let via = Via::parse_values(["1.1 edge-a (TLS terminator)", "HTTP/2 upstream"])
    .expect("illustrative Via hops should parse");

  assert_eq!(2, via.len());
  assert!(!via.is_empty());
  assert_eq!(None, via.members()[0].protocol_name());
  assert_eq!("1.1", via.members()[0].protocol_version());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("TLS terminator"), via.members()[0].comment());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!("2", via.members()[1].protocol_version());
  assert_eq!("upstream", via.members()[1].received_by());
  assert_eq!(None, via.members()[1].comment());
  assert_eq!(
    "1.1 edge-a (TLS terminator), HTTP/2 upstream",
    via.header_value()
  );
}

#[test]
fn via_parses_protocol_name_version_port_ipv6_and_comments() {
  let via = Via::parse(
    "1.0 fred, 1.1 p.example.net:8080 (Apache/2.4), HTTP/1.1 GWA, 1.1 [2001:db8::1]:8443",
  )
  .expect("received-protocol, received-by, and comment forms should parse");

  assert_eq!(4, via.len());
  assert_eq!(None, via.members()[0].protocol_name());
  assert_eq!("1.0", via.members()[0].protocol_version());
  assert_eq!("fred", via.members()[0].received_by());
  assert_eq!("p.example.net:8080", via.members()[1].received_by());
  assert_eq!(Some("Apache/2.4"), via.members()[1].comment());
  assert_eq!(Some("HTTP"), via.members()[2].protocol_name());
  assert_eq!("1.1", via.members()[2].protocol_version());
  assert_eq!("GWA", via.members()[2].received_by());
  assert_eq!("[2001:db8::1]:8443", via.members()[3].received_by());
  assert_eq!(
    "1.0 fred, 1.1 p.example.net:8080 (Apache/2.4), HTTP/1.1 GWA, 1.1 [2001:db8::1]:8443",
    via.header_value()
  );
}

#[test]
fn via_preserves_token_spelling_comment_content_duplicates_and_order() {
  let via = Via::parse("Http/2 Edge.A (TLS terminator), 1.1 edge-a, Http/2 Edge.A")
    .expect("duplicate hops and mixed-case protocol tokens should parse");

  assert_eq!(3, via.len());
  assert_eq!(Some("Http"), via.members()[0].protocol_name());
  assert_eq!("2", via.members()[0].protocol_version());
  assert_eq!("Edge.A", via.members()[0].received_by());
  assert_eq!(Some("TLS terminator"), via.members()[0].comment());
  assert_eq!("edge-a", via.members()[1].received_by());
  assert_eq!("Edge.A", via.members()[2].received_by());
  assert_eq!(
    "Http/2 Edge.A (TLS terminator), 1.1 edge-a, Http/2 Edge.A",
    via.header_value()
  );
}

#[test]
fn via_normalizes_optional_whitespace_and_nested_comments() {
  let via = Via::parse("  1.1\tedge-a\t (outer (inner) more) , HTTP/2\tupstream  ")
    .expect("optional whitespace and nested comments should parse");

  assert_eq!(2, via.len());
  assert_eq!(Some("outer (inner) more"), via.members()[0].comment());
  assert_eq!(
    "1.1 edge-a (outer (inner) more), HTTP/2 upstream",
    via.header_value()
  );
}

#[test]
fn via_rejects_malformed_protocol_received_by_comments_and_members() {
  for value in [
    "",
    " ",
    "1.1",
    "1.1 ",
    "HTTP/",
    "/1.1 hop",
    "1.1hop",
    "1.1 hop(comment)",
    "1.1 hop extra",
    "1.1 hop (",
    "1.1 hop (unterminated",
    "1.1 hop (\\",
    "1.1 :8080",
    "1.1 hop:",
    "1.1 hop:abc",
    "1.1 [2001:db8::1",
    "1.1 not valid",
    "1.1 hop,",
    ",1.1 hop",
    "1.1 hop,,1.0 other",
    "1.1 hop\r\nX-Injected: 1",
    "1.1 hop\u{0}",
    "1.1 hop\u{7f}",
  ] {
    assert!(Via::parse(value).is_err(), "Via should reject {value:?}");
  }

  for (index, values) in [vec!["", "1.1 hop"], vec!["1.1 hop", ""], vec!["", ""]]
    .into_iter()
    .enumerate()
  {
    assert!(
      Via::parse_values(values).is_err(),
      "Via should reject empty combined fields at index {index}"
    );
  }
}

#[test]
fn via_enforces_member_and_size_bounds() {
  let too_many = (0..=MAX_VIA_MEMBERS)
    .map(|index| format!("1.1 hop{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Via::parse(&too_many).is_err());

  let oversized_field = format!("1.1 {}", "a".repeat(MAX_VIA_VALUE_BYTES));
  assert!(Via::parse(&oversized_field).is_err());

  let first = format!("1.1 {}", "a".repeat(MAX_VIA_VALUE_BYTES - 8));
  assert!(
    Via::parse_values([first.as_str(), "1.1 second"]).is_err(),
    "combined Via fields over the value bound should be rejected"
  );

  let padded = format!("1.1 hop{}", " ".repeat(MAX_VIA_VALUE_BYTES - 7));
  assert_eq!(padded.len(), MAX_VIA_VALUE_BYTES);
  assert!(
    Via::parse(padded.as_str()).is_ok(),
    "one OWS-padded field at the value bound should parse"
  );
  assert!(
    Via::parse_values([padded.as_str(), padded.as_str()]).is_err(),
    "repeated OWS-padded Via fields over the raw aggregate bound should be rejected"
  );
}
