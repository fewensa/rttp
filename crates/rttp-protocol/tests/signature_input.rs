use rttp_protocol::signature_input::{
  SignatureInput, SignatureParameterValue, MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS,
  MAX_SIGNATURE_INPUT_COVERED_COMPONENTS, MAX_SIGNATURE_INPUT_MEMBERS,
  MAX_SIGNATURE_INPUT_PARAMETERS, MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES,
  MAX_SIGNATURE_INPUT_VALUE_BYTES,
};

#[test]
fn signature_input_parses_rfc_shaped_metadata_without_signature_policy() {
  let input = SignatureInput::parse(
    r#"sig1=("@method" "@path" "content-digest";sf);created=1700000000;keyid="test-key";alg="ed25519""#,
  )
  .expect("Signature-Input should parse");

  assert_eq!(input.len(), 1);
  let member = input.member("sig1").expect("sig1 should be retained");
  assert_eq!(member.label(), "sig1");
  assert_eq!(member.covered_components().len(), 3);
  assert_eq!(member.covered_components()[0].identifier(), "@method");
  assert_eq!(member.covered_components()[1].identifier(), "@path");
  assert_eq!(
    member.covered_components()[2].identifier(),
    "content-digest"
  );
  assert!(member.covered_components()[2]
    .parameter("sf")
    .expect("sf parameter")
    .is_valueless());
  assert_eq!(
    member
      .parameter("created")
      .and_then(|parameter| parameter.value()),
    Some(&SignatureParameterValue::Integer(1_700_000_000))
  );
  assert_eq!(
    member
      .parameter("keyid")
      .and_then(|parameter| parameter.value()),
    Some(&SignatureParameterValue::String("test-key".to_string()))
  );
  assert_eq!(
    member
      .parameter("alg")
      .and_then(|parameter| parameter.value()),
    Some(&SignatureParameterValue::String("ed25519".to_string()))
  );
  assert_eq!(
    input.header_value(),
    r#"sig1=("@method" "@path" "content-digest";sf);created=1700000000;keyid="test-key";alg="ed25519""#
  );
}

#[test]
fn signature_input_combines_fields_and_preserves_ordered_parameter_values() {
  let input = SignatureInput::parse_values([
    r#"sig1=("content-digest";sf;req);created=001;flag=?0"#,
    r#"sig2=("@status");nonce=:YWJj:;ratio=1.230;tok=*abc/def"#,
  ])
  .expect("combined Signature-Input should parse");

  assert_eq!(input.members()[0].label(), "sig1");
  assert_eq!(input.members()[1].label(), "sig2");
  assert_eq!(
    input.members()[1]
      .parameter("nonce")
      .and_then(|parameter| parameter.value()),
    Some(&SignatureParameterValue::ByteSequence(b"abc".to_vec()))
  );
  assert_eq!(
    input.header_value(),
    r#"sig1=("content-digest";sf;req);created=1;flag=?0, sig2=("@status");nonce=:YWJj:;ratio=1.23;tok=*abc/def"#
  );
  assert_eq!(
    SignatureInput::parse(input.header_value())
      .expect("canonical value should parse")
      .members(),
    input.members()
  );
}

#[test]
fn signature_input_rejects_malformed_or_wrong_shape_values() {
  for value in [
    "",
    "sig1",
    "sig1=abc",
    "sig1=()",
    "sig1=(content-digest)",
    "sig1=(\"@method\", \"@path\")",
    "sig1=(\"@method\" \"@path\"",
    "sig1=(\"@method\");bad=@1",
    "sig1=(\"@method\");bad=%\"display\"",
    "sig1=(\"@method\");p=1, sig1=(\"@path\")",
    "sig1=(\"@method\";p;p)",
    "sig1=(\"@method\");p;p",
  ] {
    assert!(
      SignatureInput::parse(value).is_err(),
      "should reject {value:?}"
    );
  }
}

#[test]
fn signature_input_enforces_field_and_member_bounds() {
  assert!(SignatureInput::parse("x".repeat(MAX_SIGNATURE_INPUT_VALUE_BYTES + 1)).is_err());

  let too_many_members = (0..=MAX_SIGNATURE_INPUT_MEMBERS)
    .map(|index| format!("m{index}=(\"@method\")"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(SignatureInput::parse(too_many_members).is_err());

  let too_many_components = format!(
    "sig1=({})",
    std::iter::repeat_n("\"@method\"", MAX_SIGNATURE_INPUT_COVERED_COMPONENTS + 1)
      .collect::<Vec<_>>()
      .join(" ")
  );
  assert!(SignatureInput::parse(too_many_components).is_err());

  let too_many_member_params = format!(
    "sig1=(\"@method\"){}",
    (0..=MAX_SIGNATURE_INPUT_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(SignatureInput::parse(too_many_member_params).is_err());

  let too_many_component_params = format!(
    "sig1=(\"@method\"{})",
    (0..=MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  );
  assert!(SignatureInput::parse(too_many_component_params).is_err());

  let oversized_string = format!(
    "sig1=(\"@method\");keyid=\"{}\"",
    "x".repeat(MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(SignatureInput::parse(oversized_string).is_err());
}
