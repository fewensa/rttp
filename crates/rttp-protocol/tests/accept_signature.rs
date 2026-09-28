use rttp_protocol::accept_signature::{
  AcceptSignature, AcceptSignatureBareItem, MAX_ACCEPT_SIGNATURE_COMPONENT_PARAMETERS,
  MAX_ACCEPT_SIGNATURE_ENTRIES, MAX_ACCEPT_SIGNATURE_ENTRY_COMPONENTS,
  MAX_ACCEPT_SIGNATURE_ENTRY_PARAMETERS, MAX_ACCEPT_SIGNATURE_TOTAL_BYTES,
  MAX_ACCEPT_SIGNATURE_VALUE_BYTES,
};

#[test]
fn parses_rfc_request_metadata_and_canonicalizes_it() {
  let value = r#" sig1=("@method";req "content-digest";sf);expires;created;nonce="n1";alg="rsa-pss-sha512";keyid="test-key";tag="app" "#;
  let parsed = AcceptSignature::parse(value).expect("Accept-Signature should parse");
  let entry = parsed.entry("sig1").expect("sig1 should exist");

  assert_eq!(entry.components()[0].identifier(), "@method");
  assert_eq!(entry.components()[1].identifier(), "content-digest");
  assert_eq!(entry.components()[0].parameters()[0].name(), "req");
  assert!(entry
    .parameter("created")
    .is_some_and(|parameter| parameter.is_valueless()));
  assert!(entry.created());
  assert!(entry.expires());
  assert_eq!(entry.nonce(), Some("n1"));
  assert_eq!(entry.alg(), Some("rsa-pss-sha512"));
  assert_eq!(entry.keyid(), Some("test-key"));
  assert_eq!(entry.tag(), Some("app"));
  assert_eq!(
    parsed.header_value(),
    r#"sig1=("@method";req "content-digest";sf);expires;created;nonce="n1";alg="rsa-pss-sha512";keyid="test-key";tag="app""#
  );
}

#[test]
fn combines_repeated_fields_in_wire_order() {
  let parsed = AcceptSignature::parse_values([
    r#"sig1=("@method"), sig2=("@path")"#,
    r#"sig3=("@authority")"#,
  ])
  .expect("repeated Accept-Signature fields should parse");

  assert_eq!(
    parsed
      .entries()
      .iter()
      .map(|entry| entry.label())
      .collect::<Vec<_>>(),
    ["sig1", "sig2", "sig3"]
  );
}

#[test]
fn preserves_component_and_parameter_order() {
  let parsed =
    AcceptSignature::parse(r#"sig1=("x";b=2;a=1 "x";a=1;b=3 "x";a=1;b=4);z="last";a="first""#)
      .expect("distinct parameterized components should parse");
  let entry = parsed.entry("sig1").unwrap();

  assert_eq!(
    entry
      .components()
      .iter()
      .map(|component| component.identifier())
      .collect::<Vec<_>>(),
    ["x", "x", "x"]
  );
  assert_eq!(
    entry.components()[0]
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>(),
    ["b", "a"]
  );
  assert_eq!(
    parsed.header_value(),
    r#"sig1=("x";b=2;a=1 "x";a=1;b=3 "x";a=1;b=4);z="last";a="first""#
  );
}

#[test]
fn rejects_duplicate_labels_and_parameters() {
  for value in [
    r#"sig1=("@method"), sig1=("@path")"#,
    r#"sig1=("@method";sf;sf)"#,
    r#"sig1=("@method");created;created"#,
    r#"sig1=("@method" "@method")"#,
    r#"sig1=("x";a=1;b=2 "x";b=2;a=1)"#,
  ] {
    assert!(
      AcceptSignature::parse(value).is_err(),
      "{value} should fail"
    );
  }
  assert!(AcceptSignature::parse_values([r#"sig1=("@method")"#, r#"sig1=("@path")"#]).is_err());
}

#[test]
fn validates_registered_parameter_forms() {
  for value in [
    r#"sig1=("@method");created=1"#,
    r#"sig1=("@method");expires=?1"#,
    r#"sig1=("@method");nonce=?1"#,
    r#"sig1=("@method");alg=sha256"#,
    r#"sig1=("@method");keyid"#,
    r#"sig1=("@method");tag=:YWJj:"#,
  ] {
    assert!(
      AcceptSignature::parse(value).is_err(),
      "{value} should fail"
    );
  }
  let unknown = AcceptSignature::parse(r#"sig1=("@method");extension=42"#)
    .expect("unknown well-formed parameters remain metadata");
  assert_eq!(
    unknown
      .entry("sig1")
      .unwrap()
      .parameter("extension")
      .unwrap()
      .value(),
    Some(&AcceptSignatureBareItem::Integer(42))
  );
}

#[test]
fn accepts_wildcard_keys_tokens_and_byte_sequences() {
  let parsed =
    AcceptSignature::parse(r#"*=("*";mode=token);quoted="value";token=abc;bytes=:AQI=:"#)
      .expect("valid structured-field wildcard and bare-item forms should parse");
  let entry = parsed
    .entry("*")
    .expect("wildcard label should be retained");

  assert_eq!(entry.components()[0].identifier(), "*");
  assert_eq!(
    entry.components()[0].parameter("mode").unwrap().value(),
    &AcceptSignatureBareItem::Token("token".to_string())
  );
  assert_eq!(
    parsed.header_value(),
    r#"*=("*";mode=token);quoted="value";token=abc;bytes=:AQI=:"#,
  );
}

#[test]
fn rejects_malformed_byte_sequences_and_controls() {
  for value in [
    r#"sig1=("@method");bytes=:not-base64:"#,
    r#"sig1=("@method");bytes=:AQI"#,
    "sig1=(\"@method\");bytes=:AQI:\u{000b}",
    "sig1=(\"@method\");bytes=:AQI:\u{0080}",
  ] {
    assert!(
      AcceptSignature::parse(value).is_err(),
      "{value:?} should fail"
    );
  }
}

#[test]
fn accepts_empty_component_lists() {
  let parsed = AcceptSignature::parse("sig1=();created")
    .expect("empty Accept-Signature component lists should parse");
  let entry = parsed.entry("sig1").expect("sig1 should exist");

  assert!(entry.components().is_empty());
  assert!(entry.created());
  assert_eq!(parsed.header_value(), "sig1=();created");

  let bare = AcceptSignature::parse("sig1=()").expect("bare empty inner list should parse");
  assert!(bare.entry("sig1").unwrap().components().is_empty());
  assert_eq!(bare.header_value(), "sig1=()");
}

#[test]
fn rejects_empty_non_ascii_control_and_non_inner_list_members() {
  for value in [
    "",
    "   ",
    "sig1=\"@method\"",
    "sig1=(\"@method\u{00e9}\")",
    "sig1=(\"@method\u{0000}\")",
    "sig1=(\"@method\")\r\n",
  ] {
    assert!(
      AcceptSignature::parse(value).is_err(),
      "{value:?} should fail"
    );
  }
}

#[test]
fn enforces_field_total_and_count_bounds() {
  let prefix = "sig1=(\"";
  let suffix = "\")";
  let exact = format!(
    "{prefix}{}{suffix}",
    "a".repeat(MAX_ACCEPT_SIGNATURE_VALUE_BYTES - prefix.len() - suffix.len())
  );
  assert_eq!(exact.len(), MAX_ACCEPT_SIGNATURE_VALUE_BYTES);
  assert!(AcceptSignature::parse(exact).is_ok());

  assert!(AcceptSignature::parse("x".repeat(MAX_ACCEPT_SIGNATURE_VALUE_BYTES + 1)).is_err());
  let large_component = "a".repeat(MAX_ACCEPT_SIGNATURE_TOTAL_BYTES / 2);
  let total_values = [
    format!("sig1=(\"{large_component}\")"),
    format!("sig2=(\"{large_component}\")"),
  ];
  assert!(AcceptSignature::parse_values(total_values.iter().map(String::as_str)).is_err());

  let too_many_entries = (0..=MAX_ACCEPT_SIGNATURE_ENTRIES)
    .map(|index| format!("sig{index}=(\"@method\")"))
    .collect::<Vec<_>>();
  assert!(AcceptSignature::parse(too_many_entries.join(", ")).is_err());

  let too_many_components = (0..=MAX_ACCEPT_SIGNATURE_ENTRY_COMPONENTS)
    .map(|_| "\"@method\"")
    .collect::<Vec<_>>()
    .join(" ");
  assert!(AcceptSignature::parse(format!("sig1=({too_many_components})")).is_err());

  let too_many_entry_parameters = (0..=MAX_ACCEPT_SIGNATURE_ENTRY_PARAMETERS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(";");
  assert!(
    AcceptSignature::parse(format!("sig1=(\"@method\");{too_many_entry_parameters}")).is_err()
  );

  let too_many_component_parameters = (0..=MAX_ACCEPT_SIGNATURE_COMPONENT_PARAMETERS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(";");
  assert!(AcceptSignature::parse(format!(
    "sig1=(\"@method\";{too_many_component_parameters})"
  ))
  .is_err());
}
