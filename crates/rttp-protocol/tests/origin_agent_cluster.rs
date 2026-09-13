use rttp_protocol::origin_agent_cluster::{
  OriginAgentCluster, MAX_ORIGIN_AGENT_CLUSTER_VALUE_BYTES,
};

#[test]
fn origin_agent_cluster_parses_and_formats_structured_booleans() {
  let enabled = OriginAgentCluster::parse("?1").expect("enabled value should parse");
  assert_eq!(OriginAgentCluster::Boolean(true), enabled);
  assert!(enabled.boolean());
  assert!(enabled.value());
  assert!(enabled.is_enabled());
  assert_eq!("?1", enabled.header_value());

  let disabled = OriginAgentCluster::parse("?0").expect("disabled value should parse");
  assert_eq!(OriginAgentCluster::Boolean(false), disabled);
  assert!(!disabled.boolean());
  assert!(!disabled.value());
  assert!(!disabled.is_enabled());
  assert_eq!("?0", disabled.header_value());
}

#[test]
fn origin_agent_cluster_accepts_ows_and_rejects_invalid_values() {
  for value in ["\t?1\t", " ?1 ", " \t?0\t ", "?0\t", "\t?0"] {
    assert!(
      OriginAgentCluster::parse(value).is_ok(),
      "{value:?} should parse"
    );
  }

  assert!(OriginAgentCluster::parse_values(["?1", "?0"]).is_err());
  assert!(OriginAgentCluster::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "true",
    "false",
    "?2",
    "?1, ?1",
    "?1;foo=?1",
    "(?1)",
    "\"?1\"",
    "1",
    "?1\r\nX: y",
    "?1\u{7f}",
    "?1\0",
  ] {
    assert!(
      OriginAgentCluster::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn origin_agent_cluster_enforces_bounds_on_all_values() {
  assert!(OriginAgentCluster::parse("a".repeat(MAX_ORIGIN_AGENT_CLUSTER_VALUE_BYTES + 1)).is_err());

  let oversized = "a".repeat(MAX_ORIGIN_AGENT_CLUSTER_VALUE_BYTES + 1);
  assert!(OriginAgentCluster::parse_values(["?1", oversized.as_str()]).is_err());
}
