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
  assert_eq!(content_disposition.filename(), Some("report-Q1.txt"));
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
    "attachment; filename=\"bad\\\r\"",
    "attachment; filename=\"bad\\\n\"",
    "attachment; filename=\"bad\\\u{7f}\"",
    "attachment; filename=one; FILENAME=two",
    "attachment; filename*=UTF-8''bad%ZZname",
    "attachment; filename*=UTF-8''bad%A",
    "attachment; filename*=UTF-8''%",
    "attachment; filename*=UTF-8''",
    "attachment; filename*=\"UTF-8''report.txt\"",
    "attachment; filename*=not-an-ext-value",
    "attachment; filename*=UTF.8''report.txt",
    "attachment; filename*=UTF*8''report.txt",
    "attachment; filename*=UTF|8''report.txt",
    "attachment; filename*=UTF'8''report.txt",
    "attachment; filename*=x{y}''report.txt",
    "attachment; filename*=UTF-8'en.US'report.txt",
    "attachment; filename*=UTF-8'e'report.txt",
    "attachment; filename*=UTF-8'en_US'report.txt",
    "attachment; filename*=UTF-8' en'report.txt",
    "attachment; filename*=UTF-8'en,'report.txt",
    "attachment; filename*=UTF-8'*'report.txt",
    "attachment; filename*=UTF-8''%80",
    "attachment; filename*=UTF-8''%FF",
    "attachment; filename*=UTF-8''%C0%80",
    "attachment; filename*=UTF-8''%E2%82",
    "attachment; filename*=UTF-8''%00name",
    "attachment; filename*=UTF-8''%0Aname",
    "attachment; filename*=UTF-8''%1Fname",
    "attachment; filename*=UTF-8''name%7F",
    "attachment; filename*=UTF-8''%ED%A0%80",
  ] {
    assert!(
      ContentDisposition::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_disposition_accepts_rfc5987_ext_value_charsets_and_languages() {
  for value in [
    "attachment; filename*=UTF-8''report.txt",
    "attachment; filename*=ISO-8859-1''report.txt",
    "attachment; filename*=utf-8''report.txt",
    "attachment; filename*=UTF-8'en'report.txt",
    "attachment; filename*=UTF-8'en-US'report.txt",
    "attachment; filename*=UTF-8'x-private'report.txt",
    "attachment; filename*=UTF-8'i-klingon'report.txt",
    "attachment; filename*=UTF-8''%20report%2Etxt",
    "attachment; filename*=UTF-8''a!#$&+.^_`|~-",
  ] {
    let parsed =
      ContentDisposition::parse(value).unwrap_or_else(|_| panic!("{value:?} must parse"));
    let round_trip = ContentDisposition::parse(parsed.header_value())
      .expect("canonical Content-Disposition should parse");
    assert_eq!(round_trip, parsed);
    assert_eq!(round_trip.header_value(), parsed.header_value());
  }
}

#[test]
fn content_disposition_prefers_decoded_utf8_filename_star() {
  let preferred = ContentDisposition::parse(
    "attachment; filename=\"plain.txt\"; filename*=UTF-8''%E2%82%AC%20rates.txt",
  )
  .expect("UTF-8 filename* should parse");
  assert_eq!(preferred.filename(), Some("€ rates.txt"));
  assert_eq!(
    preferred
      .parameter("filename")
      .map(|parameter| parameter.value()),
    Some("plain.txt")
  );
  assert_eq!(
    preferred.filename_ext(),
    Some("UTF-8''%E2%82%AC%20rates.txt")
  );
  assert_eq!(
    preferred.header_value(),
    "attachment; filename=plain.txt; filename*=UTF-8''%E2%82%AC%20rates.txt"
  );

  let only_star = ContentDisposition::parse("attachment; filename*=utf-8''%C3%A9.txt")
    .expect("lowercase UTF-8 filename* should parse");
  assert_eq!(only_star.filename(), Some("é.txt"));
  assert_eq!(only_star.filename_ext(), Some("utf-8''%C3%A9.txt"));

  let fallback = ContentDisposition::parse(
    "attachment; filename=\"plain.txt\"; filename*=ISO-8859-1''na%EFve.txt",
  )
  .expect("non-UTF-8 filename* should parse");
  assert_eq!(fallback.filename(), Some("plain.txt"));
  assert_eq!(fallback.filename_ext(), Some("ISO-8859-1''na%EFve.txt"));

  let encoded_only = ContentDisposition::parse("attachment; filename*=ISO-8859-1''na%EFve.txt")
    .expect("non-UTF-8 filename* without filename should parse");
  assert_eq!(encoded_only.filename(), None);
  assert_eq!(encoded_only.filename_ext(), Some("ISO-8859-1''na%EFve.txt"));

  let star_first = ContentDisposition::parse(
    "attachment; filename*=UTF-8'en-US'%C3%A9.txt; filename=\"plain.txt\"",
  )
  .expect("filename* before filename should parse");
  assert_eq!(star_first.filename(), Some("é.txt"));
  assert_eq!(star_first.parameters()[0].name(), "filename*");
  assert_eq!(star_first.parameters()[1].name(), "filename");
  assert_eq!(
    star_first.header_value(),
    "attachment; filename*=UTF-8'en-US'%C3%A9.txt; filename=plain.txt"
  );
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

  let quoted_content_len = MAX_CONTENT_DISPOSITION_VALUE_BYTES - "attachment; filename=\"\"".len();
  let quoted_at_header_limit = format!(
    "attachment; filename=\"{}\"",
    "a".repeat(quoted_content_len)
  );
  assert_eq!(
    quoted_at_header_limit.len(),
    MAX_CONTENT_DISPOSITION_VALUE_BYTES
  );
  let parsed_quoted = ContentDisposition::parse(&quoted_at_header_limit)
    .expect("quoted parameter at the header bound should parse");
  assert_eq!(
    parsed_quoted.filename().map(str::len),
    Some(quoted_content_len)
  );

  let escaped_filename = format!("attachment; filename=\"{}\"", "\\a".repeat(32));
  let parsed_escaped = ContentDisposition::parse(&escaped_filename)
    .expect("escaped quoted-pairs should parse against the unescaped value bound");
  let unescaped_filename = "a".repeat(32);
  assert_eq!(parsed_escaped.filename(), Some(unescaped_filename.as_str()));
  assert_eq!(
    parsed_escaped.header_value(),
    format!("attachment; filename={unescaped_filename}")
  );

  let compact_prefix = "attachment;filename=";
  let compact_over_canonical = format!(
    "{compact_prefix}{}",
    "a".repeat(MAX_CONTENT_DISPOSITION_VALUE_BYTES - compact_prefix.len())
  );
  assert_eq!(
    compact_over_canonical.len(),
    MAX_CONTENT_DISPOSITION_VALUE_BYTES
  );
  assert!(
    ContentDisposition::parse(&compact_over_canonical).is_err(),
    "wire values whose canonical form exceeds the header bound must be rejected"
  );

  let canonical_at_limit = format!(
    "attachment; filename={}",
    "a".repeat(MAX_CONTENT_DISPOSITION_VALUE_BYTES - "attachment; filename=".len())
  );
  assert_eq!(
    canonical_at_limit.len(),
    MAX_CONTENT_DISPOSITION_VALUE_BYTES
  );
  let parsed_at_limit = ContentDisposition::parse(&canonical_at_limit)
    .expect("canonical values at the header bound should parse");
  assert_eq!(
    parsed_at_limit.header_value().len(),
    MAX_CONTENT_DISPOSITION_VALUE_BYTES
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
fn content_disposition_round_trips_canonical_serialization() {
  for value in [
    "inline",
    "attachment; filename=report.txt",
    "attachment; filename=\"read me.txt\"; preview=yes",
    "attachment; filename=\"report \\\"Q1\\\".txt\"; filename*=UTF-8''report-Q1.txt",
    "attachment; filename=\"é\"",
    r#"attachment; filename="a\"b\\c""#,
    "attachment; filename=plain.txt; filename*=UTF-8''%E2%82%AC%20rates.txt",
    "attachment; filename=plain.txt; filename*=ISO-8859-1''na%EFve.txt",
    "attachment; filename*=UTF-8'en-US'%C3%A9.txt; filename=plain.txt",
  ] {
    let parsed =
      ContentDisposition::parse(value).unwrap_or_else(|_| panic!("{value:?} must parse"));
    let serialized = parsed.header_value();
    let round_trip =
      ContentDisposition::parse(&serialized).expect("canonical serialization should parse");
    assert_eq!(round_trip, parsed);
    assert_eq!(round_trip.header_value(), serialized);
  }
}

#[test]
fn content_disposition_builds_common_dispositions_and_parameters() {
  let content_disposition = ContentDisposition::attachment()
    .with_parameter("Filename", "financial report.txt")
    .expect("filename should build")
    .with_parameter("filename*", "UTF-8''financial-report.txt")
    .expect("filename* should build");

  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert_eq!(content_disposition.filename(), Some("financial-report.txt"));
  assert_eq!(
    content_disposition
      .parameter("filename")
      .map(|parameter| parameter.value()),
    Some("financial report.txt")
  );
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
fn content_disposition_builder_accepts_http_ows_around_type_and_parameter_names() {
  for (disposition_type, parameter_name) in [
    (" attachment ", " filename "),
    ("\tattachment\t", "\tfilename\t"),
    (" \tAttachment\t ", " \tFilename\t "),
  ] {
    let content_disposition = ContentDisposition::new(disposition_type)
      .expect("OWS-padded disposition type should build")
      .with_parameter(parameter_name, "financial report.txt")
      .expect("OWS-padded parameter name should build");

    assert_eq!(content_disposition.disposition_type(), "attachment");
    assert_eq!(content_disposition.filename(), Some("financial report.txt"));
    assert_eq!(
      content_disposition.header_value(),
      "attachment; filename=\"financial report.txt\""
    );
  }
}

#[test]
fn content_disposition_builder_rejects_non_ows_padding_around_type_and_parameter_names() {
  for whitespace in ["\u{000b}", "\u{000c}", "\r", "\n", "\u{00a0}", "\u{2003}"] {
    let disposition_type = format!("{whitespace}attachment{whitespace}");
    assert!(
      ContentDisposition::new(&disposition_type).is_err(),
      "non-OWS disposition type padding must be rejected: {disposition_type:?}"
    );

    let parameter_name = format!("{whitespace}filename{whitespace}");
    assert!(
      ContentDisposition::attachment()
        .with_parameter(&parameter_name, "report.txt")
        .is_err(),
      "non-OWS parameter name padding must be rejected: {parameter_name:?}"
    );
  }
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
      .with_parameter("filename*", "UTF-8''%80")
      .is_err(),
    "invalid UTF-8 filename* octets must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename*", "UTF-8''%00name")
      .is_err(),
    "decoded control bytes in filename* must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename*", "UTF.8''report.txt")
      .is_err(),
    "filename* charsets outside mime-charset must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename*", "UTF-8'en.US'report.txt")
      .is_err(),
    "filename* languages outside Language-Tag must be rejected"
  );
  assert!(
    content_disposition
      .clone()
      .with_parameter("filename*", "x{y}''report.txt")
      .is_err(),
    "filename* charsets that cannot stay unquoted tokens must be rejected"
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
