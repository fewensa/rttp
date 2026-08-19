use rttp_protocol::signature::{
  Signature, MAX_SIGNATURE_ENTRIES, MAX_SIGNATURE_ENTRY_PARAMETERS, MAX_SIGNATURE_VALUE_BYTES,
};

#[test]
fn signature_parses_rfc_labeled_byte_sequence_and_reformats() {
  let signature = Signature::parse("sig1=:YWJj:").expect("RFC labeled byte sequence should parse");

  assert_eq!(signature.len(), 1);
  assert!(!signature.is_empty());
  assert_eq!(signature.entries()[0].label(), "sig1");
  assert_eq!(signature.entries()[0].value(), b"abc");
  assert_eq!(
    signature.entry("sig1").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(signature.entry("SIG1"), None);
  assert_eq!(signature.header_value(), "sig1=:YWJj:");
}

#[test]
fn signature_accepts_multiple_fields_in_wire_order() {
  let signature = Signature::parse_values(["sig1=:YWJj:", "sig-b24=:ZGVm:"])
    .expect("combined Signature fields should parse");

  assert_eq!(signature.entries()[0].label(), "sig1");
  assert_eq!(signature.entries()[0].value(), b"abc");
  assert_eq!(signature.entries()[1].label(), "sig-b24");
  assert_eq!(signature.entries()[1].value(), b"def");
  assert_eq!(signature.header_value(), "sig1=:YWJj:, sig-b24=:ZGVm:");
}

#[test]
fn signature_discards_well_formed_item_parameters() {
  let signature =
    Signature::parse("sig1=:YWJj:;foo=bar;enabled;count=2, sig-b24=:ZGVm:;note=\"ok\"")
      .expect("Signature should parse item parameters");

  assert_eq!(signature.len(), 2);
  assert_eq!(
    signature.entry("sig1").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    signature.entry("sig-b24").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(signature.header_value(), "sig1=:YWJj:, sig-b24=:ZGVm:");
}

#[test]
fn signature_rejects_empty_malformed_duplicate_and_non_byte_values() {
  for value in [
    "",
    "   ",
    "sig1",
    "sig1=",
    "sig1=abc",
    "sig1=\"abc\"",
    "sig1=123",
    "sig1=?1",
    "sig1=(:YWJj:)",
    "SIG1=:YWJj:",
    "sig1=:YWJj:, sig1=:ZGVm:",
    "sig1=:YWJj:;foo=",
    "sig1=:YWJj:;foo=1.",
    "sig1=:YWJj:;Foo=bar",
    "sig1=:YWJj:;\tfoo=bar",
  ] {
    assert!(
      Signature::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn signature_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    Signature::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    Signature::parse_values(["sig1=:YWJj:", "sig1=:ZGVm:"]).is_err(),
    "duplicate labels across fields must be rejected"
  );
}

#[test]
fn signature_enforces_value_entry_and_parameter_bounds() {
  assert!(
    Signature::parse("x".repeat(MAX_SIGNATURE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_SIGNATURE_VALUE_BYTES + 1);
  assert!(
    Signature::parse_values(["sig1=:YWJj:", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_SIGNATURE_ENTRIES)
    .map(|index| format!("sig{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = Signature::parse(&at_limit).expect("256 signature entries should parse");
  assert_eq!(parsed.len(), MAX_SIGNATURE_ENTRIES);

  let too_many = (0..=MAX_SIGNATURE_ENTRIES)
    .map(|index| format!("sig{index}=:YWJj:"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    Signature::parse(&too_many).is_err(),
    "more than 256 signature entries must be rejected"
  );

  let at_parameter_limit = format!(
    "sig1=:YWJj:{}",
    (0..MAX_SIGNATURE_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  let parsed_parameters =
    Signature::parse(&at_parameter_limit).expect("256 entry parameters should parse");
  assert_eq!(parsed_parameters.header_value(), "sig1=:YWJj:");

  let too_many_parameters = format!(
    "sig1=:YWJj:{}",
    (0..=MAX_SIGNATURE_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(
    Signature::parse(&too_many_parameters).is_err(),
    "more than 256 entry parameters must be rejected"
  );
}
