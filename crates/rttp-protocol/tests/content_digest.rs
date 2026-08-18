use rttp_protocol::digest::{
  ContentDigest, MAX_DIGEST_ENTRIES, MAX_DIGEST_ENTRY_PARAMETERS, MAX_DIGEST_VALUE_BYTES,
};

#[test]
fn content_digest_parses_byte_sequence_and_reformats() {
  let digest = ContentDigest::parse("sha-256=:YWJj:").expect("Content-Digest should parse");

  assert_eq!(digest.len(), 1);
  assert!(!digest.is_empty());
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].value(), b"abc");
  assert_eq!(
    digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(digest.entry("SHA-256"), None);
  assert_eq!(digest.header_value(), "sha-256=:YWJj:");
}

#[test]
fn content_digest_accepts_multiple_fields_in_wire_order() {
  let digest = ContentDigest::parse_values(["sha-256=:YWJj:", "sha-512=:ZGVm:"])
    .expect("combined Content-Digest fields should parse");

  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].value(), b"abc");
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].value(), b"def");
  assert_eq!(digest.header_value(), "sha-256=:YWJj:, sha-512=:ZGVm:");
}

#[test]
fn content_digest_retains_unknown_algorithms() {
  let digest =
    ContentDigest::parse("example-alg=:YWJj:").expect("unknown algorithms should be retained");
  assert_eq!(digest.entries()[0].algorithm(), "example-alg");
  assert_eq!(digest.entries()[0].value(), b"abc");
}

#[test]
fn content_digest_discards_well_formed_item_parameters() {
  let digest =
    ContentDigest::parse("sha-256=:YWJj:;foo=bar;enabled;count=2, sha-512=:ZGVm:;note=\"ok\"")
      .expect("Content-Digest should parse item parameters");

  assert_eq!(digest.len(), 2);
  assert_eq!(
    digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    digest.entry("sha-512").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(digest.header_value(), "sha-256=:YWJj:, sha-512=:ZGVm:");
}

#[test]
fn content_digest_rejects_empty_malformed_duplicate_and_non_byte_values() {
  for value in [
    "",
    "   ",
    "sha-256",
    "sha-256=",
    "sha-256=abc",
    "sha-256=\"abc\"",
    "sha-256=123",
    "sha-256=?1",
    "sha-256=(:YWJj:)",
    "SHA-256=:YWJj:",
    "sha-256=:YWJj:, sha-256=:ZGVm:",
    "sha-256=:not-base64!:",
    "sha-256=:YWJj:;foo=",
    "sha-256=:YWJj:;foo=1.",
    "sha-256=:YWJj:;Foo=bar",
    "sha-256=:YWJj:;\tfoo=bar",
  ] {
    assert!(
      ContentDigest::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_digest_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    ContentDigest::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    ContentDigest::parse_values(["sha-256=:YWJj:", "sha-256=:ZGVm:"]).is_err(),
    "duplicate keys across fields must be rejected"
  );
}

#[test]
fn content_digest_enforces_value_entry_and_parameter_bounds() {
  assert!(
    ContentDigest::parse("x".repeat(MAX_DIGEST_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_DIGEST_VALUE_BYTES + 1);
  assert!(
    ContentDigest::parse_values(["sha-256=:YWJj:", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_DIGEST_ENTRIES)
    .map(|index| format!("alg{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = ContentDigest::parse(&at_limit).expect("256 digest entries should parse");
  assert_eq!(parsed.len(), MAX_DIGEST_ENTRIES);

  let too_many = (0..=MAX_DIGEST_ENTRIES)
    .map(|index| format!("alg{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    ContentDigest::parse(&too_many).is_err(),
    "more than 256 digest entries must be rejected"
  );

  let at_parameter_limit = format!(
    "sha-256=:YWJj:{}",
    (0..MAX_DIGEST_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  let parsed_parameters =
    ContentDigest::parse(&at_parameter_limit).expect("256 entry parameters should parse");
  assert_eq!(parsed_parameters.header_value(), "sha-256=:YWJj:");

  let too_many_parameters = format!(
    "sha-256=:YWJj:{}",
    (0..=MAX_DIGEST_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(
    ContentDigest::parse(&too_many_parameters).is_err(),
    "more than 256 entry parameters must be rejected"
  );
}
