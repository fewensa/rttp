use rttp_protocol::content_disposition::{
  ContentDisposition, MAX_CONTENT_DISPOSITION_PARAMETERS,
  MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES, MAX_CONTENT_DISPOSITION_VALUE_BYTES,
};

#[test]
fn content_disposition_parses_type_ordered_parameters_and_filenames() {
  let content_disposition = ContentDisposition::parse(
    "Attachment; Filename=\"report \\\"Q1\\\".txt\"; filename*=UTF-8''report-Q1.txt",
  )
  .expect("Content-Disposition should parse");

  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert_eq!(content_disposition.filename(), Some("report \"Q1\".txt"));
  assert_eq!(
    content_disposition.filename_ext(),
    Some("UTF-8''report-Q1.txt")
  );
  assert_eq!(content_disposition.parameters()[0].name(), "filename");
  assert_eq!(
    content_disposition.parameters()[0].value(),
    "report \"Q1\".txt"
  );
  assert_eq!(
    content_disposition
      .parameter("FILENAME*")
      .map(|parameter| parameter.value()),
    Some("UTF-8''report-Q1.txt")
  );
  assert_eq!(
    content_disposition.header_value(),
    "attachment; filename=\"report \\\"Q1\\\".txt\"; filename*=UTF-8''report-Q1.txt"
  );
}

#[test]
fn content_disposition_preserves_quoted_strings_and_optional_whitespace() {
  let content_disposition =
    ContentDisposition::parse("\tinline\t;\tfilename\t=\t\"read me.txt\"\t; preview=yes\t")
      .expect("quoted-string and OWS should parse");

  assert_eq!(content_disposition.disposition_type(), "inline");
  assert_eq!(content_disposition.filename(), Some("read me.txt"));
  assert_eq!(
    content_disposition
      .parameter("preview")
      .map(|parameter| parameter.value()),
    Some("yes")
  );
  assert_eq!(
    content_disposition.header_value(),
    "inline; filename=\"read me.txt\"; preview=yes"
  );
}

#[test]
fn content_disposition_parses_obs_text_and_escaped_quoted_strings() {
  let obs_text = ContentDisposition::parse("attachment; filename=\"é\"")
    .expect("obs-text quoted filename should parse");
  assert_eq!(obs_text.filename(), Some("é"));
  assert_eq!(obs_text.header_value(), "attachment; filename=\"é\"");

  let escaped = ContentDisposition::parse(r#"attachment; filename="a\"b\\c""#)
    .expect("escaped quoted filename should parse");
  assert_eq!(escaped.filename(), Some(r#"a"b\c"#));
  assert_eq!(escaped.header_value(), r#"attachment; filename="a\"b\\c""#);
}

#[test]
fn content_disposition_parse_values_enforces_singleton_fields() {
  let content_disposition =
    ContentDisposition::parse_values([" attachment "]).expect("single field should parse");
  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert!(
    ContentDisposition::parse_values(["attachment", "inline"]).is_err(),
    "duplicate fields must be rejected"
  );
  assert!(
    ContentDisposition::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn content_disposition_rejects_invalid_syntax() {
  for value in [
    "",
    " ",
    "attach ment",
    "attachment;",
    "attachment; filename",
    "attachment; filename=",
    "attachment; file name=report.txt",
    "attachment; filename=report txt",
    "attachment; filename=\"unterminated",
    "attachment; filename=\"\"",
    "attachment; filename=\"bad\\\"",
    "attachment; filename=\"bad\r\nX-Evil: yes\"",
    "attachment; filename=\"bad\u{7f}\"",
    "attachment; filename=one; FILENAME=two",
    "attachment; filename*=UTF-8''bad%ZZname",
    "attachment; filename*=\"UTF-8''report.txt\"",
    "attachment; filename*=not-an-ext-value",
  ] {
    assert!(
      ContentDisposition::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_disposition_enforces_value_and_parameter_bounds() {
  assert!(
    ContentDisposition::parse("x".repeat(MAX_CONTENT_DISPOSITION_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_CONTENT_DISPOSITION_VALUE_BYTES + 1);
  assert!(
    ContentDisposition::parse_values(["attachment", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let oversized_parameter = format!(
    "attachment; filename={}",
    "a".repeat(MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    ContentDisposition::parse(&oversized_parameter).is_err(),
    "oversized parameter values must be rejected"
  );

  let at_limit = format!(
    "attachment{}",
    (0..MAX_CONTENT_DISPOSITION_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let parsed = ContentDisposition::parse(&at_limit).expect("256 parameters should parse");
  assert_eq!(
    parsed.parameters().len(),
    MAX_CONTENT_DISPOSITION_PARAMETERS
  );

  let too_many = format!(
    "attachment{}",
    (0..=MAX_CONTENT_DISPOSITION_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(
    ContentDisposition::parse(&too_many).is_err(),
    "more than 256 parameters must be rejected"
  );
}

#[test]
fn content_disposition_builds_common_dispositions_and_parameters() {
  let content_disposition = ContentDisposition::attachment()
    .with_parameter("Filename", "financial report.txt")
    .expect("filename should build")
    .with_parameter("filename*", "UTF-8''financial-report.txt")
    .expect("filename* should build");

  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert_eq!(content_disposition.filename(), Some("financial report.txt"));
  assert_eq!(
    content_disposition.filename_ext(),
    Some("UTF-8''financial-report.txt")
  );
  assert_eq!(
    content_disposition.header_value(),
    "attachment; filename=\"financial report.txt\"; filename*=UTF-8''financial-report.txt"
  );
  assert_eq!(ContentDisposition::inline().header_value(), "inline");
  assert_eq!(
    ContentDisposition::new("Attachment")
      .expect("disposition type should build")
      .header_value(),
    "attachment"
  );
}

#[test]
fn content_disposition_builder_rejects_invalid_types_and_parameters() {
  assert!(
    ContentDisposition::new("bad type").is_err(),
    "invalid disposition types must be rejected"
  );
  assert!(
    ContentDisposition::new("").is_err(),
    "empty disposition types must be rejected"
  );

  let content_disposition = ContentDisposition::attachment();
  assert!(
    content_disposition
      .clone()
      .with_parameter("bad name", "value")
      .is_err(),
    "invalid parameter names must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename", "")
      .is_err(),
    "empty parameter values must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename", "caf\u{e9}.txt")
      .is_err(),
    "non-ASCII parameter values must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename", "bad\r\nX-Evil: yes")
      .is_err(),
    "control bytes in parameter values must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename*", "UTF-8''bad%ZZname")
      .is_err(),
    "invalid filename* ext-values must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename", "one")
      .expect("parameter should build")
      .with_parameter("FILENAME", "two")
      .is_err(),
    "case-insensitive duplicate parameters must be rejected"
  );

  let at_limit = (0..MAX_CONTENT_DISPOSITION_PARAMETERS).fold(
    content_disposition,
    |content_disposition, index| {
      content_disposition
        .with_parameter(format!("p{index}"), "v")
        .expect("parameter should build")
    },
  );
  assert!(
    at_limit.with_parameter("overflow", "v").is_err(),
    "more than 256 parameters must be rejected"
  );
}
