use rttp_protocol::want_content_digest::{
  WantContentDigest, MAX_WANT_CONTENT_DIGEST_ALGORITHMS, MAX_WANT_CONTENT_DIGEST_VALUE_BYTES,
};

#[test]
fn want_content_digest_parses_rfc_example_and_reformats() {
  let digest = WantContentDigest::parse("sha-512=3, sha-256=10, unixsum=0")
    .expect("RFC Want-Content-Digest example should parse");

  assert_eq!(digest.len(), 3);
  assert!(!digest.is_empty());
  assert_eq!(digest.entries()[0].algorithm(), "sha-512");
  assert_eq!(digest.entries()[0].preference(), 3);
  assert_eq!(digest.entries()[1].algorithm(), "sha-256");
  assert_eq!(digest.entries()[1].preference(), 10);
  assert_eq!(digest.entries()[2].algorithm(), "unixsum");
  assert_eq!(digest.entries()[2].preference(), 0);
  assert_eq!(
    digest.entry("sha-256").map(|entry| entry.preference()),
    Some(10)
  );
  assert_eq!(digest.entry("SHA-256"), None);
  assert_eq!(digest.header_value(), "sha-512=3, sha-256=10, unixsum=0");
}

#[test]
fn want_content_digest_accepts_multiple_fields_in_wire_order() {
  let digest = WantContentDigest::parse_values(["sha-256=10", "sha-512=3, unixsum=0"])
    .expect("combined Want-Content-Digest fields should parse");

  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].preference(), 3);
  assert_eq!(digest.entries()[2].algorithm(), "unixsum");
  assert_eq!(digest.header_value(), "sha-256=10, sha-512=3, unixsum=0");
}

#[test]
fn want_content_digest_accepts_preference_bounds() {
  let digest = WantContentDigest::parse("sha-256=0, sha-512=10")
    .expect("preference values 0 and 10 should parse");
  assert_eq!(digest.entry("sha-256").unwrap().preference(), 0);
  assert_eq!(digest.entry("sha-512").unwrap().preference(), 10);
}

#[test]
fn want_content_digest_retains_unknown_algorithms() {
  let digest =
    WantContentDigest::parse("example-alg=5").expect("unknown algorithms should be retained");
  assert_eq!(digest.entries()[0].algorithm(), "example-alg");
  assert_eq!(digest.entries()[0].preference(), 5);
}

#[test]
fn want_content_digest_ignores_unrecognized_parameters() {
  let digest = WantContentDigest::parse("sha-256=10;foo=bar")
    .expect("unrecognized Structured Fields parameters should be ignored");
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.header_value(), "sha-256=10");
}

#[test]
fn want_content_digest_ignores_quoted_commas_in_unrecognized_parameters() {
  let digest = WantContentDigest::parse(r#"sha-256=10;note="a,b""#)
    .expect("quoted commas in ignored parameters should not count as members");
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.header_value(), "sha-256=10");
}

#[test]
fn want_content_digest_accepts_leading_zero_integers() {
  let digest = WantContentDigest::parse("sha-256=01")
    .expect("leading-zero integers should parse as sf-integer");
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 1);
  assert_eq!(digest.header_value(), "sha-256=1");
}

#[test]
fn want_content_digest_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    "sha-256",
    "sha-256=?1",
    "sha-256=11",
    "sha-256=-1",
    "sha-256=1.0",
    "sha-256=+10",
    "sha-256=(10)",
    "SHA-256=10",
    "sha-256=10, sha-256=3",
  ] {
    assert!(
      WantContentDigest::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn want_content_digest_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    WantContentDigest::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    WantContentDigest::parse_values(["sha-256=10", "sha-256=3"]).is_err(),
    "duplicate keys across fields must be rejected"
  );
}

#[test]
fn want_content_digest_enforces_value_and_algorithm_bounds() {
  assert!(
    WantContentDigest::parse("x".repeat(MAX_WANT_CONTENT_DIGEST_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_WANT_CONTENT_DIGEST_VALUE_BYTES + 1);
  assert!(
    WantContentDigest::parse_values(["sha-256=10", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_WANT_CONTENT_DIGEST_ALGORITHMS)
    .map(|index| format!("alg{index}={}", index % 11))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = WantContentDigest::parse(&at_limit).expect("32 algorithms should parse");
  assert_eq!(parsed.len(), MAX_WANT_CONTENT_DIGEST_ALGORITHMS);

  let too_many = (0..=MAX_WANT_CONTENT_DIGEST_ALGORITHMS)
    .map(|index| format!("alg{index}=10"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    WantContentDigest::parse(&too_many).is_err(),
    "more than 32 algorithms must be rejected"
  );
}
