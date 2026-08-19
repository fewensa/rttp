use rttp_protocol::signature_input::{
  SignatureInput, SignatureInputBareItem, MAX_SIGNATURE_INPUT_ENTRIES,
  MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS, MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS,
  MAX_SIGNATURE_INPUT_VALUE_BYTES,
};

#[test]
fn signature_input_parses_rfc_labeled_inner_list_and_reformats() {
  let signature_input = SignatureInput::parse(
    r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#,
  )
  .expect("RFC labeled inner list should parse");

  assert_eq!(signature_input.len(), 1);
  assert!(!signature_input.is_empty());
  assert_eq!(signature_input.entries()[0].label(), "sig1");
  assert_eq!(
    signature_input.entries()[0]
      .components()
      .iter()
      .map(|component| component.identifier())
      .collect::<Vec<_>>(),
    ["@method", "@authority", "@path"]
  );
  assert_eq!(
    signature_input
      .entry("sig1")
      .and_then(|entry| entry.parameter("created"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Integer(1_618_884_473))
  );
  assert_eq!(
    signature_input
      .entry("sig1")
      .and_then(|entry| entry.parameter("keyid"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::String("test-key".to_string()))
  );
  assert_eq!(signature_input.entry("SIG1"), None);
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#
  );
}

#[test]
fn signature_input_accepts_multiple_fields_in_wire_order() {
  let signature_input = SignatureInput::parse_values([
    r#"sig1=("@method")"#,
    r#"sig-b24=("@status");created=1618884473"#,
  ])
  .expect("combined Signature-Input fields should parse");

  assert_eq!(signature_input.entries()[0].label(), "sig1");
  assert_eq!(
    signature_input.entries()[0].components()[0].identifier(),
    "@method"
  );
  assert_eq!(signature_input.entries()[1].label(), "sig-b24");
  assert_eq!(
    signature_input.entries()[1].components()[0].identifier(),
    "@status"
  );
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method"), sig-b24=("@status");created=1618884473"#
  );
}

#[test]
fn signature_input_retains_well_formed_member_and_component_parameters() {
  let signature_input = SignatureInput::parse(
    r#"sig1=("@method";req "@query-param";name="Pet" "@status");created=1618884473;keyid="test-key";alg="hmac-sha256";nonce="n1";tag="app";unknown;sf;key="hdr";bs;tr=?0"#,
  )
  .expect("Signature-Input should retain well-formed parameters");

  let entry = signature_input.entry("sig1").expect("sig1 should exist");
  assert_eq!(entry.components()[0].identifier(), "@method");
  assert_eq!(
    entry.components()[0]
      .parameter("req")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Boolean(true))
  );
  assert_eq!(entry.components()[1].identifier(), "@query-param");
  assert_eq!(
    entry.components()[1]
      .parameter("name")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::String("Pet".to_string()))
  );
  assert_eq!(
    entry
      .parameter("unknown")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Boolean(true))
  );
  assert_eq!(
    entry.parameter("tr").map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Boolean(false))
  );
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method";req "@query-param";name="Pet" "@status");created=1618884473;keyid="test-key";alg="hmac-sha256";nonce="n1";tag="app";unknown;sf;key="hdr";bs;tr=?0"#
  );
}

#[test]
fn signature_input_keeps_later_duplicate_labels_in_one_field() {
  let signature_input = SignatureInput::parse(
    r#"sig1=("@method");created=1, sig2=("@path"), sig1=("@status");created=2"#,
  )
  .expect("duplicate labels in one field should keep the later value");

  assert_eq!(signature_input.len(), 2);
  assert_eq!(signature_input.entries()[0].label(), "sig1");
  assert_eq!(
    signature_input.entries()[0].components()[0].identifier(),
    "@status"
  );
  assert_eq!(
    signature_input
      .entry("sig1")
      .and_then(|entry| entry.parameter("created"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Integer(2))
  );
  assert_eq!(signature_input.entries()[1].label(), "sig2");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@status");created=2, sig2=("@path")"#
  );
}

#[test]
fn signature_input_keeps_later_duplicate_labels_across_fields() {
  let signature_input = SignatureInput::parse_values([
    r#"sig1=("@method");created=1, sig2=("@path")"#,
    r#"sig1=("@status");created=2"#,
  ])
  .expect("duplicate labels should keep the later value");

  assert_eq!(signature_input.len(), 2);
  assert_eq!(signature_input.entries()[0].label(), "sig1");
  assert_eq!(
    signature_input.entries()[0].components()[0].identifier(),
    "@status"
  );
  assert_eq!(
    signature_input
      .entry("sig1")
      .and_then(|entry| entry.parameter("created"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Integer(2))
  );
  assert_eq!(signature_input.entries()[1].label(), "sig2");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@status");created=2, sig2=("@path")"#
  );
}

#[test]
fn signature_input_keeps_later_duplicate_parameters() {
  let signature_input =
    SignatureInput::parse(r#"sig1=("@method";sf=?0;sf;name="old";name="new");created=1;created=2"#)
      .expect("duplicate parameters should keep the later value");
  let entry = signature_input.entry("sig1").expect("sig1 should exist");

  assert_eq!(
    entry.components()[0]
      .parameter("sf")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Boolean(true))
  );
  assert_eq!(
    entry.components()[0]
      .parameter("name")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::String("new".to_string()))
  );
  assert_eq!(
    entry
      .parameter("created")
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Integer(2))
  );
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method";sf;name="new");created=2"#
  );
}

#[test]
fn signature_input_treats_bare_and_explicit_true_parameters_equally() {
  let bare = SignatureInput::parse(r#"sig1=("@method";sf);test"#)
    .expect("bare true parameters should parse");
  let explicit = SignatureInput::parse(r#"sig1=("@method";sf=?1);test=?1"#)
    .expect("explicit true parameters should parse");

  assert_eq!(bare, explicit);
  assert_eq!(
    bare
      .entry("sig1")
      .and_then(|entry| entry.components()[0].parameter("sf"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Boolean(true))
  );
  assert_eq!(bare.header_value(), r#"sig1=("@method";sf);test"#);
}

#[test]
fn signature_input_accepts_multi_entry_field_with_colon_bearing_token_parameter() {
  let value = r#"sig1=("@method");alg=rsa:pss, sig2=("@path")"#;
  let parsed = SignatureInput::parse(value)
    .expect("colon-bearing token parameters must not be treated as byte sequences");
  let from_values = SignatureInput::parse_values([value])
    .expect("parse_values should accept the same colon-bearing token field");

  assert_eq!(parsed.len(), 2);
  assert_eq!(
    parsed
      .entry("sig1")
      .and_then(|entry| entry.parameter("alg"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::Token("rsa:pss".to_string()))
  );
  assert_eq!(parsed.entries()[1].label(), "sig2");
  assert_eq!(parsed.header_value(), value);
  assert_eq!(from_values.header_value(), parsed.header_value());
}

#[test]
fn signature_input_roundtrips_display_string_parameter_with_non_ascii() {
  let value = r#"sig1=("@method");note=%"caf%c3%a9""#;
  let parsed =
    SignatureInput::parse(value).expect("DisplayString parameters with non-ASCII should parse");

  assert_eq!(
    parsed
      .entry("sig1")
      .and_then(|entry| entry.parameter("note"))
      .map(|parameter| parameter.value()),
    Some(&SignatureInputBareItem::DisplayString("café".to_string()))
  );
  assert_eq!(parsed.header_value(), value);
}

#[test]
fn signature_input_rejects_empty_malformed_and_non_inner_list_values() {
  for value in [
    "",
    "   ",
    "sig1",
    "sig1=",
    "sig1=()",
    "sig1=:YWJj:",
    r#"sig1="@method""#,
    r#"SIG1=("@method")"#,
    r#"sig1=("@method" 1)"#,
    r#"sig1=("@method";Foo=bar)"#,
  ] {
    assert!(
      SignatureInput::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn signature_input_rejects_empty_field_sets() {
  assert!(
    SignatureInput::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn signature_input_enforces_value_entry_component_and_parameter_bounds() {
  assert!(
    SignatureInput::parse("x".repeat(MAX_SIGNATURE_INPUT_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_SIGNATURE_INPUT_VALUE_BYTES + 1);
  assert!(
    SignatureInput::parse_values([r#"sig1=("@method")"#, oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_SIGNATURE_INPUT_ENTRIES)
    .map(|index| format!(r#"sig{index}=("@method")"#))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = SignatureInput::parse(&at_limit).expect("256 signature-input entries should parse");
  assert_eq!(parsed.len(), MAX_SIGNATURE_INPUT_ENTRIES);

  let too_many = (0..=MAX_SIGNATURE_INPUT_ENTRIES)
    .map(|index| format!(r#"sig{index}=("@method")"#))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    SignatureInput::parse(&too_many).is_err(),
    "more than 256 signature-input entries must be rejected"
  );

  let at_component_limit = format!(
    "sig1=({})",
    (0..MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS)
      .map(|index| format!(r#""c{index}""#))
      .collect::<Vec<_>>()
      .join(" ")
  );
  let parsed_components =
    SignatureInput::parse(&at_component_limit).expect("256 entry components should parse");
  assert_eq!(
    parsed_components.entries()[0].components().len(),
    MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS
  );

  let too_many_components = format!(
    "sig1=({})",
    (0..=MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS)
      .map(|index| format!(r#""c{index}""#))
      .collect::<Vec<_>>()
      .join(" ")
  );
  assert!(
    SignatureInput::parse(&too_many_components).is_err(),
    "more than 256 entry components must be rejected"
  );

  let at_parameter_limit = format!(
    r#"sig1=("@method"){}"#,
    (0..MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  let parsed_parameters =
    SignatureInput::parse(&at_parameter_limit).expect("256 entry parameters should parse");
  assert_eq!(
    parsed_parameters.entries()[0].parameters().len(),
    MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS
  );

  let too_many_parameters = format!(
    r#"sig1=("@method"){}"#,
    (0..=MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(
    SignatureInput::parse(&too_many_parameters).is_err(),
    "more than 256 entry parameters must be rejected"
  );
}
