use rttp_protocol::tcn::{Tcn, TcnDirective, MAX_TCN_TOTAL_BYTES, MAX_TCN_VALUE_BYTES};

#[test]
fn parses_valid_tcn_result_tokens_in_wire_order() {
  let tcn = Tcn::parse("List, Choice, adhoc, re-choose, keep").expect("TCN should parse");

  assert_eq!(
    &[
      TcnDirective::List,
      TcnDirective::Choice,
      TcnDirective::Adhoc,
      TcnDirective::ReChoose,
      TcnDirective::Keep,
    ],
    tcn.members()
  );
  assert_eq!("list, choice, adhoc, re-choose, keep", tcn.header_value());
  assert_eq!(5, tcn.len());
  assert!(!tcn.is_empty());
}

#[test]
fn parses_singleton_field_with_optional_whitespace() {
  let tcn = Tcn::parse("\tchoice , keep ").expect("OWS-padded TCN should parse");

  assert_eq!(&[TcnDirective::Choice, TcnDirective::Keep], tcn.members());
  assert_eq!("choice, keep", tcn.header_value());
}

#[test]
fn rejects_malformed_and_unknown_tcn_values() {
  for value in [
    "",
    " ",
    ",",
    "list,",
    ",list",
    "list,,choice",
    "transparent",
    "list=value",
    "rechoose",
    "re-choose=yes",
    "choice;param",
  ] {
    assert!(Tcn::parse(value).is_err(), "TCN should reject {value:?}");
  }
}

#[test]
fn rejects_duplicate_tcn_members_case_insensitively() {
  assert!(Tcn::parse("list, LIST").is_err());
  assert!(Tcn::parse("choice, Choice").is_err());
  assert!(Tcn::parse("adhoc, ADHOC").is_err());
  assert!(Tcn::parse("re-choose, RE-CHOOSE").is_err());
  assert!(Tcn::parse("keep, Keep").is_err());
}

#[test]
fn rejects_duplicate_tcn_header_fields() {
  assert!(Tcn::parse_values(["list", "choice"]).is_err());
}

#[test]
fn rejects_oversized_tcn_values() {
  let oversized = format!("list{}", " ".repeat(MAX_TCN_VALUE_BYTES));
  assert!(Tcn::parse(oversized).is_err());
}

#[test]
fn rejects_oversized_tcn_aggregate_values() {
  let oversized = format!("list{}", " ".repeat(MAX_TCN_TOTAL_BYTES));
  assert!(Tcn::parse(oversized).is_err());
}

#[test]
fn rejects_tcn_control_bytes_except_horizontal_tab() {
  assert!(Tcn::parse("list\r").is_err());
  assert!(Tcn::parse("list\n").is_err());
  assert!(Tcn::parse("list\0").is_err());
  assert_eq!(
    &[TcnDirective::List],
    Tcn::parse("\tlist\t").expect("tab OWS is valid").members()
  );
}
