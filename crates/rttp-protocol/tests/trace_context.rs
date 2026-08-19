use rttp_protocol::trace_context::{
  TraceParent, TraceState, MAX_TRACESTATE_MEMBERS, MAX_TRACESTATE_VALUE_BYTES,
};

const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

#[test]
fn traceparent_parses_version_ids_flags_and_sampling() {
  let traceparent = TraceParent::parse(TRACEPARENT).expect("traceparent should parse");

  assert_eq!("00", traceparent.version());
  assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", traceparent.trace_id());
  assert_eq!("00f067aa0ba902b7", traceparent.parent_id());
  assert_eq!("01", traceparent.flags());
  assert!(traceparent.sampled());
  assert_eq!(TRACEPARENT, traceparent.header_value());
}

#[test]
fn traceparent_rejects_invalid_versions_identifiers_flags_and_duplicates() {
  for value in [
    "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
    "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-0G",
    "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",
  ] {
    assert!(
      TraceParent::parse(value).is_err(),
      "traceparent should reject {value:?}"
    );
  }

  assert!(TraceParent::parse_values([TRACEPARENT, TRACEPARENT]).is_err());
}

#[test]
fn traceparent_debug_and_errors_do_not_echo_propagation_values() {
  let traceparent = TraceParent::parse(TRACEPARENT).expect("traceparent should parse");
  let debug = format!("{traceparent:?}");

  assert!(!debug.contains(traceparent.trace_id()));
  assert!(!debug.contains(traceparent.parent_id()));
  assert!(!TraceParent::parse("not-a-traceparent")
    .expect_err("invalid traceparent should fail")
    .to_string()
    .contains("not-a-traceparent"));
}

#[test]
fn tracestate_parses_ordered_members_and_combines_header_fields() {
  let tracestate = TraceState::parse_values(["rojo=00f067aa0ba902b7", " , congo=t61rcWkgMzE"])
    .expect("tracestate should parse");

  assert_eq!(2, tracestate.members().len());
  assert_eq!("rojo", tracestate.members()[0].key());
  assert_eq!("00f067aa0ba902b7", tracestate.members()[0].value());
  assert_eq!("congo", tracestate.members()[1].key());
  assert_eq!(
    "rojo=00f067aa0ba902b7,congo=t61rcWkgMzE",
    tracestate.header_value()
  );

  let empty = TraceState::parse("").expect("empty tracestate should parse");
  assert!(empty.members().is_empty());
  let space = TraceState::parse("rojo= value").expect("space inside value should parse");
  assert_eq!(" value", space.members()[0].value());
}

#[test]
fn tracestate_rejects_duplicates_invalid_members_count_and_size_bounds() {
  for value in [
    "rojo=1,rojo=2",
    "1=1",
    "Rojo=1",
    "rojo =1",
    "rojo=value=with-equals",
    "rojo=value\u{7f}",
    "tenant@1system=value",
  ] {
    assert!(
      TraceState::parse(value).is_err(),
      "tracestate should reject {value:?}"
    );
  }

  let too_many = (0..=MAX_TRACESTATE_MEMBERS)
    .map(|index| format!("k{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(TraceState::parse(too_many).is_err());

  let oversized = format!("k={}", "v".repeat(MAX_TRACESTATE_VALUE_BYTES + 1));
  assert!(TraceState::parse(oversized).is_err());
}

#[test]
fn tracestate_debug_and_errors_do_not_echo_member_values() {
  let tracestate = TraceState::parse("rojo=00f067aa0ba902b7").expect("tracestate should parse");
  let debug = format!("{tracestate:?} {:?}", tracestate.members()[0]);

  assert!(!debug.contains("00f067aa0ba902b7"));
  assert!(!TraceState::parse("rojo=secret=value")
    .expect_err("invalid tracestate should fail")
    .to_string()
    .contains("secret"));
}
