use rttp_server::server::{HttpByteRange, HttpByteRangeError};

#[test]
fn accepts_http_ows_around_single_range_components() {
  for header in [
    " bytes = 2 - 5 ",
    "\tbytes\t=\t2\t-\t5\t",
    " \t BYTES \t = \t 2 \t - \t 5 \t ",
  ] {
    assert_eq!(
      HttpByteRange::new(2, 5),
      HttpByteRange::parse(header, 10).expect("HTTP OWS should be accepted")
    );
  }

  assert_eq!(
    HttpByteRange::new(6, 9),
    HttpByteRange::parse("\tbytes \t= \t- \t4 \t", 10)
      .expect("OWS-padded suffix range should be accepted")
  );
  assert_eq!(
    HttpByteRange::new(7, 9),
    HttpByteRange::parse(" bytes\t=\t7\t-\t ", 10)
      .expect("OWS-padded open-ended range should be accepted")
  );
}

#[test]
fn preserves_single_range_outcomes() {
  assert_eq!(
    Err(HttpByteRangeError::UnsatisfiedRange),
    HttpByteRange::parse("bytes=10-", 10)
  );
  assert_eq!(
    Err(HttpByteRangeError::UnsupportedUnit),
    HttpByteRange::parse("items=0-1", 10)
  );
  assert_eq!(
    Err(HttpByteRangeError::MultipleRanges),
    HttpByteRange::parse("bytes=0-1,4-5", 10)
  );
}

#[test]
fn rejects_non_ows_padding_in_every_single_range_position() {
  for whitespace in ["\u{00a0}", "\u{2003}", "\u{000b}", "\u{000c}", "\r", "\n"] {
    for header in [
      format!("{whitespace}bytes=0-1"),
      format!("bytes=0-1{whitespace}"),
      format!("bytes{whitespace}=0-1"),
      format!("bytes={whitespace}0-1"),
      format!("bytes=0{whitespace}-1"),
      format!("bytes=0-{whitespace}1"),
    ] {
      assert!(
        HttpByteRange::parse(&header, 10).is_err(),
        "non-OWS padding should be rejected: {header:?}"
      );
    }
  }
}
