use rttp_protocol::digest::{
  ReprDigest, MAX_DIGEST_ENTRIES, MAX_DIGEST_ENTRY_PARAMETERS, MAX_DIGEST_VALUE_BYTES,
};

#[test]
fn repr_digest_parses_byte_sequence_and_reformats() {
  let digest = ReprDigest::parse("sha-256=:YWJj:").expect("Repr-Digest should parse");

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
fn repr_digest_accepts_rfc_example_and_unknown_algorithms() {
  let digest = ReprDigest::parse(
    "sha-256=:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=:, sha-512=:z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==:",
  )
  .expect("RFC Repr-Digest example should parse");

  assert_eq!(digest.len(), 2);
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(
    digest.header_value(),
    "sha-256=:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=:, sha-512=:z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==:"
  );

  let unknown =
    ReprDigest::parse("example-alg=:YWJj:").expect("unknown algorithms should be retained");
  assert_eq!(unknown.entries()[0].algorithm(), "example-alg");
  assert_eq!(unknown.entries()[0].value(), b"abc");
}

#[test]
fn repr_digest_accepts_multiple_fields_in_wire_order() {
  let digest = ReprDigest::parse_values(["sha-256=:YWJj:", "sha-512=:ZGVm:"])
    .expect("combined Repr-Digest fields should parse");

  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].value(), b"abc");
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].value(), b"def");
  assert_eq!(digest.header_value(), "sha-256=:YWJj:, sha-512=:ZGVm:");
}

#[test]
fn repr_digest_looks_up_first_matching_algorithm_key() {
  let digest =
    ReprDigest::parse("sha-256=:YWJj:, sha-512=:ZGVm:").expect("Repr-Digest lookup should parse");

  assert_eq!(
    digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    digest.entry("sha-512").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(digest.entry("sha-384"), None);
  assert_eq!(digest.entry("SHA-256"), None);
  assert_eq!(digest.entry("sha256"), None);
}

#[test]
fn repr_digest_discards_well_formed_item_parameters() {
  let digest = ReprDigest::parse(
    "sha-256=:YWJj:;foo=bar;enabled;count=2;flag=?1;bin=:YWJj:;note=\"ok\", sha-512=:ZGVm:;dec=1.5",
  )
  .expect("Repr-Digest should parse item parameters");

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
fn repr_digest_accepts_ows_around_dictionary_separators() {
  let digest = ReprDigest::parse(" \tsha-256=:YWJj:\t , \tsha-512=:ZGVm: \t")
    .expect("OWS around dictionary members should parse");
  assert_eq!(digest.header_value(), "sha-256=:YWJj:, sha-512=:ZGVm:");
}

#[test]
fn repr_digest_accepts_structured_field_key_grammar() {
  let digest = ReprDigest::parse("*alg=:YWJj:, sha_256=:ZGVm:, a.b-1=:YQ==:")
    .expect("Structured Fields key grammar should parse");
  assert_eq!(
    digest.entry("*alg").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    digest.entry("sha_256").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(
    digest.entry("a.b-1").map(|entry| entry.value()),
    Some(&b"a"[..])
  );
  assert_eq!(
    digest.header_value(),
    "*alg=:YWJj:, sha_256=:ZGVm:, a.b-1=:YQ==:"
  );
}

#[test]
fn repr_digest_decodes_unpadded_and_empty_byte_sequences() {
  let digest = ReprDigest::parse("sha-256=:YQ:, unixsum=::")
    .expect("unpadded and empty Structured Fields byte sequences should parse");

  assert_eq!(
    digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"a"[..])
  );
  assert_eq!(
    digest.entry("unixsum").map(|entry| entry.value()),
    Some(&b""[..])
  );
  assert_eq!(digest.header_value(), "sha-256=:YQ==:, unixsum=::");
}

#[test]
fn repr_digest_rejects_empty_malformed_duplicate_and_non_byte_values() {
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
    "sha-256=:@@@:",
    "sha-256=:A:",
    "sha-256=:Y=Q:",
    "sha-256=:YWJj",
    "sha-256=YWJj:",
    "sha-256=:YWJj:;foo=",
    "sha-256=:YWJj:;foo=1.",
    "sha-256=:YWJj:;Foo=bar",
    "sha-256=:YWJj:;\tfoo=bar",
    "sha-256=:YWJj:,",
    ",sha-256=:YWJj:",
    "sha-256=:YWJj:,,sha-512=:ZGVm:",
    "sha-256 =:YWJj:",
    "sha-256= :YWJj:",
    "1sha=:YWJj:",
    "-sha=:YWJj:",
  ] {
    assert!(
      ReprDigest::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn repr_digest_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    ReprDigest::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    ReprDigest::parse_values(["sha-256=:YWJj:", "sha-256=:ZGVm:"]).is_err(),
    "duplicate keys across fields must be rejected"
  );
}

#[test]
fn repr_digest_enforces_value_entry_and_parameter_bounds() {
  assert!(
    ReprDigest::parse("x".repeat(MAX_DIGEST_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_DIGEST_VALUE_BYTES + 1);
  assert!(
    ReprDigest::parse_values(["sha-256=:YWJj:", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_DIGEST_ENTRIES)
    .map(|index| format!("alg{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = ReprDigest::parse(&at_limit).expect("256 digest entries should parse");
  assert_eq!(parsed.len(), MAX_DIGEST_ENTRIES);
  assert_eq!(
    parsed.entry("alg0").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    parsed.entry("alg255").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );

  let too_many = (0..=MAX_DIGEST_ENTRIES)
    .map(|index| format!("alg{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    ReprDigest::parse(&too_many).is_err(),
    "more than 256 digest entries must be rejected"
  );

  let at_parameter_limit = format!(
    "sha-256=:YWJj:{}",
    (0..MAX_DIGEST_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  let parsed_parameters =
    ReprDigest::parse(&at_parameter_limit).expect("256 entry parameters should parse");
  assert_eq!(parsed_parameters.header_value(), "sha-256=:YWJj:");

  let too_many_parameters = format!(
    "sha-256=:YWJj:{}",
    (0..=MAX_DIGEST_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(
    ReprDigest::parse(&too_many_parameters).is_err(),
    "more than 256 entry parameters must be rejected"
  );
}
