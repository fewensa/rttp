use rttp_protocol::range::{
  ByteRangeSpec, ContentRange, Range, MAX_CONTENT_RANGE_VALUE_BYTES, MAX_RANGE_COUNT,
  MAX_RANGE_VALUE_BYTES,
};

#[test]
fn range_parses_byte_and_suffix_specs_from_one_field_value() {
  let range = Range::parse("bytes=0-499, 500-, -200").expect("valid range");

  assert_eq!(
    &[
      ByteRangeSpec::FromTo {
        start: 0,
        end: Some(499)
      },
      ByteRangeSpec::FromTo {
        start: 500,
        end: None,
      },
      ByteRangeSpec::Suffix { length: 200 },
    ],
    range.ranges()
  );
  assert_eq!("bytes=0-499, 500-, -200", range.header_value());
}

#[test]
fn range_table_accepts_closed_open_suffix_and_duplicate_members() {
  let cases = [
    (
      "bytes=0-0",
      vec![ByteRangeSpec::FromTo {
        start: 0,
        end: Some(0),
      }],
      "bytes=0-0",
    ),
    (
      "bytes=0-",
      vec![ByteRangeSpec::FromTo {
        start: 0,
        end: None,
      }],
      "bytes=0-",
    ),
    (
      "bytes=-1",
      vec![ByteRangeSpec::Suffix { length: 1 }],
      "bytes=-1",
    ),
    (
      "bytes=-0",
      vec![ByteRangeSpec::Suffix { length: 0 }],
      "bytes=-0",
    ),
    (
      "bytes=0-1,0-1",
      vec![
        ByteRangeSpec::FromTo {
          start: 0,
          end: Some(1),
        },
        ByteRangeSpec::FromTo {
          start: 0,
          end: Some(1),
        },
      ],
      "bytes=0-1, 0-1",
    ),
    (
      "bytes=0007-0009, 0010-, -0003",
      vec![
        ByteRangeSpec::FromTo {
          start: 7,
          end: Some(9),
        },
        ByteRangeSpec::FromTo {
          start: 10,
          end: None,
        },
        ByteRangeSpec::Suffix { length: 3 },
      ],
      "bytes=7-9, 10-, -3",
    ),
    (
      "BYTES=1-2",
      vec![ByteRangeSpec::FromTo {
        start: 1,
        end: Some(2),
      }],
      "bytes=1-2",
    ),
  ];

  for (value, expected, canonical) in cases {
    let range = Range::parse(value).unwrap_or_else(|error| panic!("{value:?}: {error}"));
    assert_eq!(expected.as_slice(), range.ranges(), "{value:?}");
    assert_eq!(canonical, range.header_value(), "{value:?}");
  }
}

#[test]
fn range_table_rejects_malformed_and_reversed_members() {
  for value in [
    "",
    "bytes",
    "bytes=",
    "bytes=,",
    "bytes=0-1,",
    "bytes=,0-1",
    "bytes=0",
    "bytes=-",
    "bytes=0-1-2",
    "bytes=1-0",
    "bytes=5-2",
    "bytes=0--1",
    "bytes=+0-1",
    "bytes=0-+1",
    "bytes=0.5-1",
    "bytes=0-1a",
    "bytes=a-1",
    "bytes=0 -1",
    "items=0-1",
    "bytes=0-1;q=1",
    "bytes=0-1\r\nInjected: yes",
    "bytes=0-1\n",
    "bytes=0-1\0",
  ] {
    assert!(Range::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn range_accepts_http_ows_and_rejects_non_ascii_whitespace() {
  let expected = [
    ByteRangeSpec::FromTo {
      start: 0,
      end: Some(1),
    },
    ByteRangeSpec::Suffix { length: 2 },
  ];

  for value in [
    " bytes=0-1,-2 ",
    "\tbytes=0-1,-2\t",
    " \t BYTES \t = \t 0-1 \t , \t -2 \t ",
    "bytes =0-1,-2",
    "bytes= 0-1,-2",
    "bytes\t=\t0-1,-2",
    "bytes=0-1 , -2",
    "bytes=0-1\t,\t-2",
  ] {
    let range = Range::parse(value)
      .unwrap_or_else(|error| panic!("{value:?} must accept HTTP OWS padding: {error}"));
    assert_eq!(expected.as_slice(), range.ranges(), "{value:?}");
    assert_eq!("bytes=0-1, -2", range.header_value());
  }

  for value in [
    "\u{00A0}bytes=0-1,-2",
    "bytes\u{00A0}=0-1,-2",
    "bytes=\u{00A0}0-1,-2",
    "bytes=0-1\u{00A0},-2",
    "bytes=0-1,\u{00A0}-2",
    "bytes=0-1,-2\u{00A0}",
    "\u{2003}bytes=0-1",
    "bytes\u{3000}=0-1,-2",
    "bytes=0-1\u{0085},-2",
    "bytes=0-\u{00A0}1",
  ] {
    assert!(
      Range::parse(value).is_err(),
      "{value:?} must reject non-ASCII whitespace"
    );
  }
}

#[test]
fn range_rejects_repeated_field_values() {
  assert!(Range::parse_values(["bytes=0-1", "bytes=2-3"]).is_err());
  assert!(Range::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn range_accepts_exact_member_limit_and_rejects_one_over() {
  let allowed = (0..MAX_RANGE_COUNT)
    .map(|index| format!("{index}-{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let range = Range::parse(format!("bytes={allowed}")).expect("32 members should parse");
  assert_eq!(MAX_RANGE_COUNT, range.ranges().len());
  assert_eq!(
    &ByteRangeSpec::FromTo {
      start: 0,
      end: Some(0),
    },
    &range.ranges()[0]
  );
  assert_eq!(
    &ByteRangeSpec::FromTo {
      start: 31,
      end: Some(31),
    },
    &range.ranges()[31]
  );

  let rejected = (0..=MAX_RANGE_COUNT)
    .map(|index| format!("{index}-{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(Range::parse(format!("bytes={rejected}")).is_err());
}

#[test]
fn range_enforces_exact_and_over_value_byte_limits() {
  let core = "bytes=0-0";
  let at_bound = format!("{}{}", " ".repeat(MAX_RANGE_VALUE_BYTES - core.len()), core);
  assert_eq!(MAX_RANGE_VALUE_BYTES, at_bound.len());
  let range = Range::parse(&at_bound).expect("exact size bound should parse");
  assert_eq!(
    &[ByteRangeSpec::FromTo {
      start: 0,
      end: Some(0),
    }],
    range.ranges()
  );

  let over = format!(
    "{}{}",
    " ".repeat(MAX_RANGE_VALUE_BYTES - core.len() + 1),
    core
  );
  assert_eq!(MAX_RANGE_VALUE_BYTES + 1, over.len());
  assert!(Range::parse(&over).is_err());
}

#[test]
fn range_accepts_u64_max_and_rejects_decimal_overflow() {
  let max = u64::MAX.to_string();
  let closed = Range::parse(format!("bytes={max}-{max}")).expect("u64::MAX closed range");
  assert_eq!(
    &[ByteRangeSpec::FromTo {
      start: u64::MAX,
      end: Some(u64::MAX),
    }],
    closed.ranges()
  );
  assert_eq!(format!("bytes={max}-{max}"), closed.header_value());

  let open = Range::parse(format!("bytes={max}-")).expect("u64::MAX open range");
  assert_eq!(
    &[ByteRangeSpec::FromTo {
      start: u64::MAX,
      end: None,
    }],
    open.ranges()
  );

  let suffix = Range::parse(format!("bytes=-{max}")).expect("u64::MAX suffix");
  assert_eq!(
    &[ByteRangeSpec::Suffix { length: u64::MAX }],
    suffix.ranges()
  );

  let overflow = "18446744073709551616";
  for value in [
    format!("bytes={overflow}-1"),
    format!("bytes=0-{overflow}"),
    format!("bytes=-{overflow}"),
    format!("bytes={overflow}-"),
  ] {
    assert!(
      Range::parse(&value).is_err(),
      "{value} must reject decimal overflow"
    );
  }
}

#[test]
fn content_range_parses_satisfied_unknown_and_unsatisfied_forms() {
  let satisfied = ContentRange::parse("bytes 0-499/1234").expect("satisfied content range");
  assert_eq!(
    ContentRange::Bytes {
      start: 0,
      end: 499,
      complete_length: Some(1_234),
    },
    satisfied
  );
  assert_eq!(
    ContentRange::Bytes {
      start: 500,
      end: 999,
      complete_length: None,
    },
    ContentRange::parse("bytes 500-999/*").expect("unknown complete length")
  );
  assert_eq!(
    ContentRange::Unsatisfied {
      complete_length: 1_234,
    },
    ContentRange::parse("bytes */1234").expect("unsatisfied content range")
  );
  assert_eq!("bytes", satisfied.unit());
  assert_eq!(Some(0), satisfied.start());
  assert_eq!(Some(499), satisfied.end());
  assert_eq!(Some(1_234), satisfied.complete_length());
  assert!(!satisfied.is_unsatisfied());
  assert_eq!("bytes 0-499/1234", satisfied.header_value());
}

#[test]
fn content_range_satisfiability_boundaries_wildcards_and_separators() {
  let accepted: [(&str, ContentRange, &str); 6] = [
    (
      "bytes 0-0/1",
      ContentRange::Bytes {
        start: 0,
        end: 0,
        complete_length: Some(1),
      },
      "bytes 0-0/1",
    ),
    (
      "bytes 0-9/10",
      ContentRange::Bytes {
        start: 0,
        end: 9,
        complete_length: Some(10),
      },
      "bytes 0-9/10",
    ),
    (
      "bytes 9-9/10",
      ContentRange::Bytes {
        start: 9,
        end: 9,
        complete_length: Some(10),
      },
      "bytes 9-9/10",
    ),
    (
      "bytes 0-0/*",
      ContentRange::Bytes {
        start: 0,
        end: 0,
        complete_length: None,
      },
      "bytes 0-0/*",
    ),
    (
      "bytes */0",
      ContentRange::Unsatisfied { complete_length: 0 },
      "bytes */0",
    ),
    (
      "bytes */1",
      ContentRange::Unsatisfied { complete_length: 1 },
      "bytes */1",
    ),
  ];

  for (value, expected, canonical) in accepted {
    let parsed = ContentRange::parse(value).unwrap_or_else(|error| panic!("{value:?}: {error}"));
    assert_eq!(expected, parsed, "{value:?}");
    assert_eq!(canonical, parsed.header_value(), "{value:?}");
  }

  let max = u64::MAX.to_string();
  let max_complete = format!("bytes 0-0/{max}");
  let max_unsatisfied = format!("bytes */{max}");
  assert_eq!(
    ContentRange::Bytes {
      start: 0,
      end: 0,
      complete_length: Some(u64::MAX),
    },
    ContentRange::parse(&max_complete).expect("u64::MAX complete length")
  );
  assert_eq!(
    max_complete,
    ContentRange::parse(&max_complete).unwrap().header_value()
  );
  assert_eq!(
    ContentRange::Unsatisfied {
      complete_length: u64::MAX,
    },
    ContentRange::parse(&max_unsatisfied).expect("u64::MAX unsatisfied length")
  );
  assert_eq!(
    max_unsatisfied,
    ContentRange::parse(&max_unsatisfied)
      .unwrap()
      .header_value()
  );

  for value in [
    "bytes 0-0/0",
    "bytes 0-9/9",
    "bytes 0-10/10",
    "bytes 2-1/2",
    "bytes */*",
    "bytes 0-1*",
    "bytes * /1",
    "bytes 0-1 /2",
    "bytes 0-1/\t2",
    "bytes\t0-1/2",
    "bytes 0-1/2/",
    "bytes 0-1",
    "bytes 0-1/",
    "bytes 0-1/2, bytes 2-3/4",
    "items 0-1/2",
    "bytes */18446744073709551616",
    "bytes 0-18446744073709551616/1",
    "bytes 18446744073709551616-18446744073709551616/*",
    "bytes 0-1/2\n",
  ] {
    assert!(
      ContentRange::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_range_trims_http_ows_and_formats_canonical_values() {
  let content_range =
    ContentRange::parse("\tBYTES 0003-0006/0010 ").expect("OWS-padded content range");
  let unsatisfied = ContentRange::parse(" bytes */0010\t").expect("OWS-padded unsatisfied range");

  assert_eq!(
    ContentRange::Bytes {
      start: 3,
      end: 6,
      complete_length: Some(10),
    },
    content_range
  );
  assert_eq!("bytes 3-6/10", content_range.header_value());
  assert_eq!(
    ContentRange::Unsatisfied {
      complete_length: 10,
    },
    unsatisfied
  );
  assert_eq!("bytes */10", unsatisfied.header_value());
}

#[test]
fn content_range_rejects_repeated_field_values() {
  assert!(ContentRange::parse_values(["bytes 0-1/4", "bytes 2-3/4"]).is_err());
  assert!(ContentRange::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn content_range_enforces_exact_and_over_value_byte_limits() {
  let core = "bytes 0-0/1";
  let at_bound = format!(
    "{}{}",
    " ".repeat(MAX_CONTENT_RANGE_VALUE_BYTES - core.len()),
    core
  );
  assert_eq!(MAX_CONTENT_RANGE_VALUE_BYTES, at_bound.len());
  assert_eq!(
    ContentRange::Bytes {
      start: 0,
      end: 0,
      complete_length: Some(1),
    },
    ContentRange::parse(&at_bound).expect("exact size bound should parse")
  );

  let over = format!(
    "{}{}",
    " ".repeat(MAX_CONTENT_RANGE_VALUE_BYTES - core.len() + 1),
    core
  );
  assert_eq!(MAX_CONTENT_RANGE_VALUE_BYTES + 1, over.len());
  assert!(ContentRange::parse(&over).is_err());
}

#[test]
fn range_and_content_range_reject_invalid_syntax_controls_and_overflow() {
  for value in [
    "items=0-1",
    "bytes=1-0",
    "bytes=0--1",
    "bytes=18446744073709551616-1",
    "bytes=0-1\r\nInjected: yes",
  ] {
    assert!(Range::parse(value).is_err(), "{value:?} must be rejected");
  }

  for value in [
    "items 0-1/2",
    "bytes 2-1/2",
    "bytes 0-2/2",
    "bytes */*",
    "bytes */18446744073709551616",
    "bytes\t0-1/2",
    "bytes 0-1 /\t2",
    "bytes 0-1/2, bytes 2-3/4",
    "bytes 0-1/2/",
    "bytes 0-1",
    "bytes 0-1/",
    "bytes 0-1/2\n",
  ] {
    assert!(
      ContentRange::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn range_enforces_the_member_limit() {
  let values = (0..=MAX_RANGE_COUNT)
    .map(|index| format!("{index}-{index}"))
    .collect::<Vec<_>>()
    .join(",");

  assert!(Range::parse(format!("bytes={values}")).is_err());
}

#[test]
fn public_range_variants_round_trip_through_canonical_formatting() {
  let max = u64::MAX.to_string();
  let range_inputs = [
    "bytes=0-499".to_string(),
    "bytes=500-".to_string(),
    "bytes=-200".to_string(),
    "bytes=-0".to_string(),
    "bytes=0-0, 1-, -2".to_string(),
    format!("bytes=0-{max}"),
    format!("bytes=-{max}"),
  ];
  for value in range_inputs {
    let parsed = Range::parse(&value).expect("range should parse");
    let canonical = parsed.header_value();
    let round_trip = Range::parse(&canonical).expect("canonical Range should reparse");
    assert_eq!(parsed, round_trip, "{value}");
    assert_eq!(canonical, round_trip.header_value(), "{value}");
  }

  let content_inputs = [
    "bytes 0-499/1234".to_string(),
    "bytes 500-999/*".to_string(),
    "bytes */1234".to_string(),
    "bytes */0".to_string(),
    format!("bytes 0-{}/{max}", u64::MAX - 1),
    format!("bytes */{max}"),
  ];
  for value in content_inputs {
    let parsed = ContentRange::parse(&value).expect("content range should parse");
    let canonical = parsed.header_value();
    let round_trip =
      ContentRange::parse(&canonical).expect("canonical Content-Range should reparse");
    assert_eq!(parsed, round_trip, "{value}");
    assert_eq!(canonical, round_trip.header_value(), "{value}");
    assert_eq!("bytes", round_trip.unit());
  }
}
