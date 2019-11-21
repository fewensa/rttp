
use std::str;
use std::time::{Duration, UNIX_EPOCH};
use httpdate::{HttpDate, parse_http_date, fmt_http_date};

#[test]
fn test_rfc_example() {
  let d = UNIX_EPOCH + Duration::from_secs(784111777);
  assert_eq!(d,
             parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").expect("#1"));
  assert_eq!(d,
             parse_http_date("Sunday, 06-Nov-94 08:49:37 GMT").expect("#2"));
  assert_eq!(d, parse_http_date("Sun Nov  6 08:49:37 1994").expect("#3"));
}

#[test]
fn test_parse() {
  let f = parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").expect("#1");
  println!("{:?}", f);
  let z = parse_http_date("Sat, 21 Dec 2019 07:23:44 GMT").expect("#1");
  println!("{:?}", z);
  let z = parse_http_date("Sat, 21 Dec 2019 07:23:44 GMT").expect("#1");
  println!("{:?}", z);
}
