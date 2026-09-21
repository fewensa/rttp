use rttp_protocol::server_timing::{
  ServerTiming, ServerTimingMetric, MAX_SERVER_TIMING_METRICS, MAX_SERVER_TIMING_PARAMETERS,
  MAX_SERVER_TIMING_PARAMETER_VALUE_BYTES, MAX_SERVER_TIMING_VALUE_BYTES,
};

fn extension_pairs(metric: &ServerTimingMetric) -> Vec<(&str, Option<&str>)> {
  metric
    .parameters()
    .iter()
    .map(|parameter| (parameter.name(), parameter.value()))
    .collect()
}

#[test]
fn server_timing_parses_combined_fields_preserving_order_and_duplicate_names() {
  let timing = ServerTiming::parse_values([
    r#"db;dur=53.2;desc="primary database";region=us-east;cached, db;dur=4"#,
    r#"app;desc="render \"home\"";build=2026"#,
  ])
  .expect("combined Server-Timing fields should parse");

  assert_eq!(3, timing.len());
  assert!(!timing.is_empty());

  assert_eq!("db", timing.metrics()[0].name());
  assert_eq!(Some(53.2), timing.metrics()[0].duration());
  assert_eq!(Some("primary database"), timing.metrics()[0].description());
  assert_eq!(
    vec![("region", Some("us-east")), ("cached", None)],
    extension_pairs(&timing.metrics()[0])
  );

  assert_eq!("db", timing.metrics()[1].name());
  assert_eq!(Some(4.0), timing.metrics()[1].duration());
  assert_eq!(None, timing.metrics()[1].description());
  assert!(timing.metrics()[1].parameters().is_empty());

  assert_eq!("app", timing.metrics()[2].name());
  assert_eq!(None, timing.metrics()[2].duration());
  assert_eq!(Some("render \"home\""), timing.metrics()[2].description());
  assert_eq!(
    vec![("build", Some("2026"))],
    extension_pairs(&timing.metrics()[2])
  );

  assert_eq!(
    r#"db; dur=53.2; desc="primary database"; region=us-east; cached, db; dur=4, app; desc="render \"home\""; build=2026"#,
    timing.header_value()
  );
}

#[test]
fn server_timing_round_trips_escaped_descriptions_and_extension_forms() {
  let timing = ServerTiming::parse(
    r#"miss;desc="say \"hi\" and \\path";note="two words";flag;token=abc;quoted="not-a-token, yet""#,
  )
  .expect("escaped descriptions and mixed extension forms should parse");

  assert_eq!(1, timing.len());
  assert_eq!("miss", timing.metrics()[0].name());
  assert_eq!(None, timing.metrics()[0].duration());
  assert_eq!(
    Some(r#"say "hi" and \path"#),
    timing.metrics()[0].description()
  );
  assert_eq!(
    vec![
      ("note", Some("two words")),
      ("flag", None),
      ("token", Some("abc")),
      ("quoted", Some("not-a-token, yet")),
    ],
    extension_pairs(&timing.metrics()[0])
  );
  assert_eq!(
    r#"miss; desc="say \"hi\" and \\path"; note="two words"; flag; token=abc; quoted="not-a-token, yet""#,
    timing.header_value()
  );

  let reparsed = ServerTiming::parse(timing.header_value())
    .expect("canonical Server-Timing output should reparse");
  assert_eq!(timing, reparsed);
  assert_eq!(timing.header_value(), reparsed.header_value());
}

#[test]
fn server_timing_rejects_duplicates_malformed_durations_and_empty_members() {
  for value in [
    "db;dur=1;dur=2",
    "db;DUR=1;dur=2",
    r#"db;desc="one";desc="two""#,
    r#"db;desc="one";DESC="two""#,
    "db;region=a;region=b",
    "db;region=a;REGION=b",
    "db;cached;cached",
    "db;dur=not-a-number",
    "db;dur=-1",
    "db;dur=-0.1",
    "db;dur=inf",
    "db;dur=+inf",
    "db;dur=-inf",
    "db;dur=NaN",
    "db;dur=1e9999",
    "db;dur=",
    "db;dur",
    "db;desc",
    "db;desc=",
    "db;=value",
    "db;",
    "db; desc=\"unterminated",
    "db;desc=unterminated value",
    "db extra",
    "db\r\nX-Injected: 1",
    "",
    "   ",
    "\t",
    ",",
    ",db",
    "db,",
    "db,,app",
    "db, ",
    "db, ,app",
  ] {
    assert!(
      ServerTiming::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    ServerTiming::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  for (index, values) in [vec!["", "db"], vec!["db", ""], vec!["", ""]]
    .into_iter()
    .enumerate()
  {
    assert!(
      ServerTiming::parse_values(values).is_err(),
      "empty combined fields must be rejected at index {index}"
    );
  }
}

#[test]
fn server_timing_enforces_value_metric_and_parameter_bounds() {
  let exact_description_len = MAX_SERVER_TIMING_VALUE_BYTES - "db;desc=\"\"".len();
  let exact_value = format!("db;desc=\"{}\"", "a".repeat(exact_description_len));
  assert_eq!(exact_value.len(), MAX_SERVER_TIMING_VALUE_BYTES);
  let exact = ServerTiming::parse(&exact_value).expect("a 64 KiB Server-Timing field should parse");
  assert_eq!(
    Some(exact_description_len),
    exact.metrics()[0].description().map(str::len)
  );

  assert!(ServerTiming::parse("x".repeat(MAX_SERVER_TIMING_VALUE_BYTES + 1)).is_err());
  assert!(
    ServerTiming::parse_values([
      "db;dur=1",
      "x".repeat(MAX_SERVER_TIMING_VALUE_BYTES + 1).as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );

  let oversized_parameter = format!(
    "db;note={}",
    "a".repeat(MAX_SERVER_TIMING_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    ServerTiming::parse(&oversized_parameter).is_err(),
    "parameter values larger than 64 KiB must be rejected"
  );
  let oversized_quoted_parameter = format!(
    "db;desc=\"{}\"",
    "a".repeat(MAX_SERVER_TIMING_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    ServerTiming::parse(&oversized_quoted_parameter).is_err(),
    "quoted parameter values larger than 64 KiB must be rejected"
  );

  let at_metric_limit = (0..MAX_SERVER_TIMING_METRICS)
    .map(|index| format!("metric{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed_metrics =
    ServerTiming::parse(&at_metric_limit).expect("256 Server-Timing metrics should parse");
  assert_eq!(parsed_metrics.len(), MAX_SERVER_TIMING_METRICS);

  let too_many_metrics = (0..=MAX_SERVER_TIMING_METRICS)
    .map(|index| format!("metric{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    ServerTiming::parse(&too_many_metrics).is_err(),
    "more than 256 Server-Timing metrics must be rejected"
  );

  let combined_at_limit = (0..MAX_SERVER_TIMING_METRICS)
    .map(|index| format!("metric{index}"))
    .collect::<Vec<_>>();
  let (first, rest) = combined_at_limit.split_at(128);
  let parsed_combined =
    ServerTiming::parse_values([first.join(", ").as_str(), rest.join(", ").as_str()])
      .expect("256 metrics across Server-Timing fields should parse");
  assert_eq!(parsed_combined.len(), MAX_SERVER_TIMING_METRICS);

  let combined_over_limit = (0..=MAX_SERVER_TIMING_METRICS)
    .map(|index| format!("metric{index}"))
    .collect::<Vec<_>>();
  assert!(
    ServerTiming::parse_values(
      combined_over_limit
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
    )
    .is_err(),
    "more than 256 metrics across fields must be rejected"
  );

  let at_parameter_limit = format!(
    "db;dur=1;desc=\"x\"{}",
    (0..MAX_SERVER_TIMING_PARAMETERS)
      .map(|index| format!(";ext{index}=value"))
      .collect::<String>()
  );
  let parsed_parameters =
    ServerTiming::parse(&at_parameter_limit).expect("256 extension parameters should parse");
  assert_eq!(
    parsed_parameters.metrics()[0].parameters().len(),
    MAX_SERVER_TIMING_PARAMETERS
  );

  let too_many_parameters = format!(
    "db;dur=1;desc=\"x\"{}",
    (0..=MAX_SERVER_TIMING_PARAMETERS)
      .map(|index| format!(";ext{index}=value"))
      .collect::<String>()
  );
  assert!(
    ServerTiming::parse(&too_many_parameters).is_err(),
    "more than 256 extension parameters must be rejected"
  );
}
