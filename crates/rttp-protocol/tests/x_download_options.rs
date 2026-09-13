use rttp_protocol::x_download_options::{XDownloadOptions, MAX_X_DOWNLOAD_OPTIONS_VALUE_BYTES};

#[test]
fn x_download_options_parses_noopen_case_insensitively() {
  assert_eq!(
    XDownloadOptions::Noopen,
    XDownloadOptions::parse("noopen").expect("noopen should parse")
  );
  assert_eq!(
    XDownloadOptions::Noopen,
    XDownloadOptions::parse("NoOpen").expect("NoOpen should parse")
  );
  assert_eq!(
    XDownloadOptions::Noopen,
    XDownloadOptions::parse("NOOPEN").expect("NOOPEN should parse")
  );
  assert_eq!("noopen", XDownloadOptions::Noopen.header_value());
}

#[test]
fn x_download_options_accepts_http_optional_whitespace_padding() {
  for value in ["\tnoopen\t", " \tnoopen\t ", "noopen\t", "\tnoopen"] {
    assert_eq!(
      XDownloadOptions::Noopen,
      XDownloadOptions::parse(value).expect("OWS-padded noopen should parse")
    );
  }
}

#[test]
fn x_download_options_rejects_empty_duplicate_malformed_and_ambiguous_values() {
  for value in [
    "",
    "   ",
    "unknown",
    "noopen, noopen",
    "noopen; foo",
    "\"noopen\"",
    "noopen\r\nX: y",
    "noopen\u{7f}",
  ] {
    assert!(
      XDownloadOptions::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    XDownloadOptions::parse_values(["noopen", "noopen"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    XDownloadOptions::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    XDownloadOptions::parse("a".repeat(MAX_X_DOWNLOAD_OPTIONS_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn x_download_options_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_X_DOWNLOAD_OPTIONS_VALUE_BYTES + 1);

  assert!(
    XDownloadOptions::parse_values(["noopen", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
