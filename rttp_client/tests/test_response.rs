use rttp_client::response::{ContentDisposition, ContentLocation, ContentType, Response};
use rttp_client::types::{Cookie, RoUrl};
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn test_parse_cookie_name_can_match_attribute_name() {
  let same_site = Cookie::parse("SameSite=choice; Path=/").unwrap();
  assert_eq!("SameSite", same_site.name());
  assert_eq!("choice", same_site.value());
  assert_eq!(Some(&"/".to_string()), same_site.path().as_ref());

  let path = Cookie::parse("Path=value; HttpOnly").unwrap();
  assert_eq!("Path", path.name());
  assert_eq!("value", path.value());
  assert!(path.http_only());
}

#[test]
fn test_parse_response() {
  let s = "HTTP/1.1 200 OK\r\n\
        Content-Length: 18\r\n\
        Server: GWS/2.0\r\n\
        Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
        Content-Type: text/html\r\n\
        Cache-control: private\r\n\
        Set-Cookie: 1P_JAR=2019-11-21-07; expires=Sat, 21-Dec-2019 07:23:44 GMT; path=/; domain=.example.test; SameSite=none\r\n\
        Connection: keep-alive\r\n\
        \r\n\
        <html>hello</html>";
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec());
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
  let cookies = response.cookies();
  println!("{:#?}", cookies);
}

#[test]
fn test_parse_response_preserves_duplicate_headers_with_case_insensitive_lookup() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Set-Cookie: session=abc; Path=/; HttpOnly\r\n",
    "cache-control: no-cache\r\n",
    "SET-COOKIE: theme=dark; Path=/; SameSite=Lax\r\n",
    "Cache-Control: max-age=60\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse raw duplicate header response");

  let header_lines = response
    .headers()
    .iter()
    .map(|header| (header.name().as_str(), header.value().as_str()))
    .collect::<Vec<_>>();
  assert_eq!(
    vec![
      ("Set-Cookie", "session=abc; Path=/; HttpOnly"),
      ("cache-control", "no-cache"),
      ("SET-COOKIE", "theme=dark; Path=/; SameSite=Lax"),
      ("Cache-Control", "max-age=60"),
      ("Content-Length", "2")
    ],
    header_lines
  );

  assert_eq!(
    vec![
      "session=abc; Path=/; HttpOnly",
      "theme=dark; Path=/; SameSite=Lax"
    ],
    response
      .headers_of_name("set-cookie")
      .iter()
      .map(|header| header.value().as_str())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec!["no-cache", "max-age=60"],
    response
      .headers_of_name("CACHE-CONTROL")
      .iter()
      .map(|header| header.value().as_str())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec![
      &"session=abc; Path=/; HttpOnly".to_string(),
      &"theme=dark; Path=/; SameSite=Lax".to_string()
    ],
    response.header_values("SET-cookie")
  );
  assert_eq!(2, response.cookies().len());
  assert_eq!(
    Some("abc"),
    response
      .cookie("session")
      .map(|cookie| cookie.value().as_str())
  );
  assert_eq!(
    Some("dark"),
    response
      .cookie("theme")
      .map(|cookie| cookie.value().as_str())
  );
}

#[test]
fn test_parse_partial_content_range_metadata() {
  let s = concat!(
    "HTTP/1.1 206 Partial Content\r\n",
    "Content-Range: bytes 10-19/200\r\n",
    "Content-Length: 10\r\n",
    "\r\n",
    "0123456789"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse partial content response");
  let content_range = response
    .content_range()
    .expect("partial content response should expose content range");

  assert!(response.is_partial_content());
  assert!(!response.is_range_not_satisfiable());
  assert_eq!("bytes", content_range.unit());
  assert_eq!(Some(10), content_range.start());
  assert_eq!(Some(19), content_range.end());
  assert_eq!(Some(200), content_range.complete_length());
  assert!(!content_range.is_unsatisfied());
  assert_eq!("0123456789", response.body().string().unwrap());
}

#[test]
fn test_parse_range_not_satisfiable_metadata_preserves_body_and_headers() {
  let s = concat!(
    "HTTP/1.1 416 Range Not Satisfiable\r\n",
    "Content-Range: bytes */200\r\n",
    "Content-Type: text/plain\r\n",
    "Content-Length: 17\r\n",
    "\r\n",
    "range unavailable"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse range not satisfiable response");
  let content_range = response
    .content_range()
    .expect("416 response should expose unsatisfied content range");

  assert!(!response.is_partial_content());
  assert!(response.is_range_not_satisfiable());
  assert_eq!("bytes", content_range.unit());
  assert_eq!(None, content_range.start());
  assert_eq!(None, content_range.end());
  assert_eq!(Some(200), content_range.complete_length());
  assert!(content_range.is_unsatisfied());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("Content-Type")
  );
  assert_eq!("range unavailable", response.body().string().unwrap());
}

#[test]
fn test_parse_content_type_response_helper_normalizes_media_type_and_preserves_parameters() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: Text/Plain; charset=utf-8; boundary=\"AaB03x\"; format=flowed\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with content-type");

  let content_type = response
    .content_type()
    .expect("valid content-type should parse")
    .expect("content-type header should be present");

  assert_eq!("text", content_type.type_());
  assert_eq!("plain", content_type.subtype());
  assert_eq!("text/plain", content_type.essence());
  assert!(content_type.is("TEXT", "PLAIN"));
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));
  assert_eq!(Some("AaB03x"), content_type.parameter("BOUNDARY"));
  assert_eq!(
    vec![
      ("charset", "utf-8"),
      ("boundary", "AaB03x"),
      ("format", "flowed")
    ],
    content_type
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some(&"Text/Plain; charset=utf-8; boundary=\"AaB03x\"; format=flowed".to_string()),
    response.header_value("Content-Type")
  );
}

#[test]
fn test_parse_content_type_response_helper_accepts_common_application_json() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: application/json\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "{}"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with application/json content-type");
  let content_type = response
    .content_type()
    .expect("valid content-type should parse")
    .expect("content-type header should be present");

  assert_eq!("application", content_type.type_());
  assert_eq!("json", content_type.subtype());
  assert!(content_type.parameters().is_empty());
}

#[test]
fn test_parse_content_type_response_helper_returns_none_when_absent() {
  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response without content-type");

  assert_eq!(
    None,
    response
      .content_type()
      .expect("absent content-type should parse")
  );
}

#[test]
fn test_parse_content_type_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "text",
    "text/",
    "/plain",
    "te xt/plain",
    "text/pla in",
    "text/plain;",
    "text/plain; charset",
    "text/plain; char set=utf-8",
    "text/plain; charset=utf 8",
    "text/plain; charset=\"unterminated",
    "text/plain; charset=\"bad\\\r\"",
    "text/plain; charset=\"bad\rvalue\"",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nContent-Type: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_type().is_err(),
      "content-type helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn test_parse_content_type_rejects_duplicate_singleton_duplicate_parameter_and_bounds() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: text/plain\r\n",
    "content-type: application/json\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-type remains usable");

  assert!(
    response.content_type().is_err(),
    "content-type helper should reject duplicate singleton fields"
  );
  assert_eq!(
    vec![&"text/plain".to_string(), &"application/json".to_string()],
    response.header_values("Content-Type")
  );

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Type: text/plain; charset=utf-8; CHARSET=iso-8859-1\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response with duplicate content-type parameter remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject duplicate parameters"
  );

  let oversized = format!("text/plain; charset={}", "a".repeat(64 * 1024));
  let raw = format!("HTTP/1.1 200 OK\r\nContent-Type: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-type remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject oversized values"
  );

  let too_many = (0..257)
    .map(|ix| format!("p{ix}=v"))
    .collect::<Vec<_>>()
    .join("; ");
  let raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; {too_many}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-type parameters remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject too many parameters"
  );
}

#[test]
fn test_content_type_parse_rejects_crlf_injection() {
  let error = ContentType::parse("text/plain; charset=\"bad\r\nX-Evil: yes\"")
    .expect_err("content-type helper should reject CR/LF injection");

  assert!(
    error.to_string().contains("Content-Type"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_conditional_response_metadata() {
  let s = concat!(
    "HTTP/1.1 304 Not Modified\r\n",
    "ETag: \"abc123\"\r\n",
    "Last-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse not modified response");

  assert!(response.is_not_modified());
  assert!(!response.is_precondition_failed());
  assert_eq!(Some(&"\"abc123\"".to_string()), response.etag());
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.last_modified()
  );

  let s = concat!(
    "HTTP/1.1 412 Precondition Failed\r\n",
    "ETag: W/\"stale\"\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "stale"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse precondition failed response");

  assert!(!response.is_not_modified());
  assert!(response.is_precondition_failed());
  assert_eq!(Some(&"W/\"stale\"".to_string()), response.etag());
  assert_eq!(None, response.last_modified());
}

#[test]
fn test_parse_content_location_response_helper_accepts_uri_references() {
  let cases = [
    (
      "https://cdn.example.test/images/logo.png?size=small#v1",
      "absolute URI",
    ),
    (
      "http://[::1]/images/logo.png",
      "absolute URI with IPv6 authority",
    ),
    ("/images/logo.png?size=small#v1", "absolute path"),
    ("images/logo.png?size=small#v1", "relative path reference"),
    ("../images/logo.png", "relative dot segment reference"),
    ("?variant=small", "query-only relative reference"),
  ];

  for (value, name) in cases {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Location: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .unwrap_or_else(|err| panic!("{name} response should parse: {err}"));
    let content_location = response
      .content_location()
      .unwrap_or_else(|err| panic!("{name} content-location should parse: {err}"))
      .unwrap_or_else(|| panic!("{name} content-location should be present"));

    assert_eq!(value, content_location.as_str());
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Location")
    );
  }
}

#[test]
fn test_parse_content_disposition_response_helper_preserves_ordered_parameters() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=\"report \\\"final\\\".txt\"; filename*=UTF-8''report-final.txt; preview=yes\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/download"),
    s.as_bytes().to_vec(),
  )
  .expect("parse response with content-disposition");

  let content_disposition = response
    .content_disposition()
    .expect("valid content-disposition should parse")
    .expect("content-disposition header should be present");

  assert_eq!("attachment", content_disposition.disposition_type());
  assert_eq!(Some("report \"final\".txt"), content_disposition.filename());
  assert_eq!(
    Some("UTF-8''report-final.txt"),
    content_disposition.filename_ext()
  );
  assert_eq!(
    vec![
      ("filename", "report \"final\".txt"),
      ("filename*", "UTF-8''report-final.txt"),
      ("preview", "yes")
    ],
    content_disposition
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some(
      &"attachment; filename=\"report \\\"final\\\".txt\"; filename*=UTF-8''report-final.txt; preview=yes"
        .to_string()
    ),
    response.header_value("Content-Disposition")
  );
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_parse_content_disposition_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without content-disposition");

  assert_eq!(
    None,
    response
      .content_disposition()
      .expect("absent content-disposition should parse")
  );
}

#[test]
fn test_parse_content_disposition_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "attach ment",
    "attachment;",
    "attachment; filename",
    "attachment; file name=report.txt",
    "attachment; filename=report txt",
    "attachment; filename=\"unterminated",
    "attachment; filename=\"bad\\\r\"",
    "attachment; filename=\"bad\rname\"",
    "attachment; filename*=UTF-8''bad%ZZname",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Disposition: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_disposition().is_err(),
      "content-disposition helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Disposition")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn test_parse_content_disposition_rejects_duplicate_singleton_duplicate_parameter_and_bounds() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=one.txt\r\n",
    "content-disposition: inline; filename=two.txt\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-disposition remains usable");

  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject duplicate singleton fields"
  );
  assert_eq!(
    vec![
      &"attachment; filename=one.txt".to_string(),
      &"inline; filename=two.txt".to_string()
    ],
    response.header_values("Content-Disposition")
  );

  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=one.txt; FILENAME=two.txt\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate parameter remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject duplicate parameters"
  );

  let oversized = "a".repeat(64 * 1024 + 1);
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Disposition: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-disposition remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject oversized values"
  );

  let too_many = (0..257)
    .map(|ix| format!("p{ix}=v"))
    .collect::<Vec<_>>()
    .join("; ");
  let raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Disposition: attachment; {too_many}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-disposition parameters remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject too many parameters"
  );
}

#[test]
fn test_content_disposition_parse_rejects_crlf_injection() {
  let error = ContentDisposition::parse("attachment; filename=\"bad\r\nX-Evil: yes\"")
    .expect_err("content-disposition helper should reject CR/LF injection");

  assert!(
    error.to_string().contains("Content-Disposition"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_content_location_response_helper_trims_outer_whitespace_and_allows_absent() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Location:   /representations/current.json   \r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with content-location");
  let content_location = response
    .content_location()
    .expect("valid content-location should parse")
    .expect("content-location header should be present");

  assert_eq!("/representations/current.json", content_location.as_str());
  assert_eq!(
    Some(&"/representations/current.json".to_string()),
    response.header_value("Content-Location")
  );

  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without content-location");
  assert_eq!(
    None,
    response
      .content_location()
      .expect("absent content-location should parse")
  );
}

#[test]
fn test_parse_content_location_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["", "http://[::1", "not valid", "/bad path", "ok\u{7f}"];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Location: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.content_location().is_err(),
      "content-location helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.trim().to_string()),
      response.header_value("Content-Location")
    );
  }
}

#[test]
fn test_parse_content_location_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Location: /one\r\n",
    "content-location: /two\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate content-location remains usable");

  assert!(
    response.content_location().is_err(),
    "content-location helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"/one".to_string(), &"/two".to_string()],
    response.header_values("Content-Location")
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Location: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized content-location remains usable");

  assert!(
    response.content_location().is_err(),
    "content-location helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Content-Location"));
}

#[test]
fn test_parse_content_location_rejects_control_characters_and_crlf_injection() {
  let invalid_values = ["\r\nLocation: /evil", "/ok\r", "/ok\n", "/ok\tinner"];

  for value in invalid_values {
    assert!(
      ContentLocation::parse(value).is_err(),
      "content-location parser should reject {value:?}"
    );
  }
}

#[test]
fn test_parse_cache_control_response_directives() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cache-Control: no-cache=\"Set-Cookie, Authorization\", no-store, max-age=60\r\n",
    "Cache-Control: s-maxage=120, private=\"X-User\", public, must-revalidate\r\n",
    "Cache-Control: proxy-revalidate, immutable, stale-while-revalidate=30, stale-if-error=90\r\n",
    "Cache-Control: community=\"u=1, tier=gold\", ext-token\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse cache-control response");

  let cache_control = response
    .cache_control()
    .expect("valid cache-control should parse")
    .expect("cache-control header should be present");

  assert!(cache_control.no_cache());
  assert_eq!(
    vec!["Set-Cookie", "Authorization"],
    cache_control.no_cache_fields()
  );
  assert!(cache_control.no_store());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(Some(120), cache_control.s_maxage());
  assert!(cache_control.private());
  assert_eq!(vec!["X-User"], cache_control.private_fields());
  assert!(cache_control.public());
  assert!(cache_control.must_revalidate());
  assert!(cache_control.proxy_revalidate());
  assert!(cache_control.immutable());
  assert_eq!(Some(30), cache_control.stale_while_revalidate());
  assert_eq!(Some(90), cache_control.stale_if_error());
  assert_eq!(2, cache_control.extensions().len());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(
    Some("u=1, tier=gold"),
    cache_control.extensions()[0].value()
  );
  assert_eq!("ext-token", cache_control.extensions()[1].name());
  assert_eq!(None, cache_control.extensions()[1].value());
}

#[test]
fn test_parse_cache_control_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "max-age=-1",
    "s-maxage=abc",
    "stale-while-revalidate=1.5",
    "stale-if-error=\"60\"",
    "private=\"unterminated",
    "extension=\"bad\\\"",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nCache-Control: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.cache_control().is_err(),
      "cache-control helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cache-Control")
    );
  }
}

#[test]
fn test_parse_age_and_expires_response_metadata() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Age: 2147483648\r\n",
    "Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with age and expires metadata");

  assert_eq!(
    Some(2_147_483_648),
    response.age().expect("valid age should parse")
  );
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    response.expires().expect("valid expires should parse")
  );
  assert_eq!(
    Some(&"2147483648".to_string()),
    response.header_value("Age")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Expires")
  );

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without age or expires");
  assert_eq!(None, response.age().expect("absent age should parse"));
  assert_eq!(
    None,
    response.expires().expect("absent expires should parse")
  );
}

#[test]
fn test_parse_retry_after_response_metadata() {
  let s = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: 120\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with retry-after delta metadata");
  let retry_after = response
    .retry_after()
    .expect("valid retry-after should parse")
    .expect("retry-after should be present");

  assert_eq!(Some(120), retry_after.delta_seconds());
  assert_eq!(None, retry_after.http_date());
  assert_eq!(
    Some(&"120".to_string()),
    response.header_value("Retry-After")
  );

  let s = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with retry-after date metadata");
  let retry_after = response
    .retry_after()
    .expect("valid retry-after date should parse")
    .expect("retry-after should be present");

  assert_eq!(None, retry_after.delta_seconds());
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    retry_after.http_date()
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Retry-After")
  );

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without retry-after");
  assert_eq!(
    None,
    response
      .retry_after()
      .expect("absent retry-after should parse")
  );
}

#[test]
fn test_parse_retry_after_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "-1",
    "+1",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    "18446744073709551616",
    "Sun, 06 Nov 1994 08:49:37 PST",
  ];

  for value in invalid_values {
    let raw = format!(
      "HTTP/1.1 503 Service Unavailable\r\nRetry-After: {value}\r\nContent-Length: 4\r\n\r\nbusy"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.retry_after().is_err(),
      "retry-after helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Retry-After")
    );
  }
}

#[test]
fn test_parse_retry_after_rejects_duplicate_and_oversized_helper_values() {
  let raw = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: 60\r\n",
    "retry-after: 120\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate retry-after remains usable");

  assert!(
    response.retry_after().is_err(),
    "retry-after helper should reject duplicates"
  );
  assert_eq!(
    vec![&"60".to_string(), &"120".to_string()],
    response.header_values("Retry-After")
  );

  let oversized = "1".repeat(64 * 1024 + 1);
  let raw = format!(
    "HTTP/1.1 503 Service Unavailable\r\nRetry-After: {oversized}\r\nContent-Length: 4\r\n\r\nbusy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized retry-after remains usable");

  assert!(
    response.retry_after().is_err(),
    "retry-after helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Retry-After"));
}

#[test]
fn test_parse_allow_response_helper_preserves_method_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 405 Method Not Allowed\r\n",
    "Allow: GET, HEAD\r\n",
    "allow: POST, OPTIONS\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with allow headers");
  let allow = response
    .allow()
    .expect("valid allow should parse")
    .expect("allow header should be present");

  assert_eq!(vec!["GET", "HEAD", "POST", "OPTIONS"], allow.methods());
  assert!(allow.contains_method("POST"));
  assert!(!allow.contains_method("PATCH"));
  assert_eq!(
    vec![&"GET, HEAD".to_string(), &"POST, OPTIONS".to_string()],
    response.header_values("Allow")
  );
}

#[test]
fn test_parse_allow_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without allow");

  assert_eq!(None, response.allow().expect("absent allow should parse"));
}

#[test]
fn test_parse_allow_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "GET,",
    ",GET",
    "GET,,POST",
    "GET, ,POST",
    "GET POST",
    "GET@POST",
    "GE\tT",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.allow().is_err(),
      "allow helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Allow"));
  }
}

#[test]
fn test_parse_allow_rejects_duplicate_oversized_and_too_many_methods() {
  let raw = concat!(
    "HTTP/1.1 405 Method Not Allowed\r\n",
    "Allow: GET, HEAD\r\n",
    "allow: POST, GET\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate allow remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject duplicate method names"
  );
  assert_eq!(
    vec![&"GET, HEAD".to_string(), &"POST, GET".to_string()],
    response.header_values("Allow")
  );

  let oversized = "GET".repeat(64 * 1024);
  let raw =
    format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized allow remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Allow"));

  let too_many = (0..257)
    .map(|ix| format!("M{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw =
    format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {too_many}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many allow methods remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject too many methods"
  );
  assert_eq!(Some(&too_many), response.header_value("Allow"));
}

#[test]
fn test_parse_accept_ranges_response_helper_preserves_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: bytes, pages\r\n",
    "accept-ranges: records\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with accept-ranges headers");
  let accept_ranges = response
    .accept_ranges()
    .expect("valid accept-ranges should parse")
    .expect("accept-ranges header should be present");

  assert!(!accept_ranges.is_none());
  assert!(accept_ranges.accepts_bytes());
  assert_eq!(vec!["bytes", "pages", "records"], accept_ranges.units());
  assert_eq!(
    vec![&"bytes, pages".to_string(), &"records".to_string()],
    response.header_values("Accept-Ranges")
  );
}

#[test]
fn test_parse_accept_ranges_response_helper_supports_none_and_absent_header() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: none\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with none accept-ranges");
  let accept_ranges = response
    .accept_ranges()
    .expect("valid none accept-ranges should parse")
    .expect("accept-ranges header should be present");

  assert!(accept_ranges.is_none());
  assert!(!accept_ranges.accepts_bytes());
  assert_eq!(vec!["none"], accept_ranges.units());

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without accept-ranges");
  assert_eq!(
    None,
    response
      .accept_ranges()
      .expect("absent accept-ranges should parse")
  );
}

#[test]
fn test_parse_accept_ranges_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "bytes,",
    ",bytes",
    "bytes,,pages",
    "bytes, ,pages",
    "byte ranges",
    "bytes@pages",
    "bytes, none",
    "none, bytes",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.accept_ranges().is_err(),
      "accept-ranges helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Accept-Ranges")
    );
  }
}

#[test]
fn test_parse_accept_ranges_rejects_duplicate_oversized_and_too_many_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: bytes, pages\r\n",
    "accept-ranges: BYTES\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate accept-ranges remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject normalized duplicate units"
  );
  assert_eq!(
    vec![&"bytes, pages".to_string(), &"BYTES".to_string()],
    response.header_values("Accept-Ranges")
  );

  let oversized = "bytes".repeat(16 * 1024);
  let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized accept-ranges remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Accept-Ranges"));

  let too_many = (0..257)
    .map(|ix| format!("unit{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {too_many}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many accept-ranges values remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject too many values"
  );
  assert_eq!(Some(&too_many), response.header_value("Accept-Ranges"));
}

#[test]
fn test_parse_age_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "-1",
    "+1",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    "18446744073709551616",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nAge: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.age().is_err(),
      "age helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Age"));
  }
}

#[test]
fn test_parse_expires_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["", "not a date", "Sun, 06 Nov 1994 08:49:37 PST"];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nExpires: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.expires().is_err(),
      "expires helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Expires"));
  }
}

#[test]
fn test_parse_vary_response_helper_normalizes_and_deduplicates_field_names() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Vary: Accept-Encoding, User-Agent\r\n",
    "VARY: accept-encoding, X-Feature\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with vary headers");

  let vary = response
    .vary()
    .expect("valid vary should parse")
    .expect("vary header should be present");

  assert!(!vary.is_any());
  assert_eq!(
    vec!["accept-encoding", "user-agent", "x-feature"],
    vary.field_names()
  );
  assert!(vary.contains_field_name("ACCEPT-ENCODING"));
  assert!(vary.contains_field_name("user-agent"));
  assert!(!vary.contains_field_name("authorization"));
  assert_eq!(
    vec![
      &"Accept-Encoding, User-Agent".to_string(),
      &"accept-encoding, X-Feature".to_string()
    ],
    response.header_values("vary")
  );
}

#[test]
fn test_parse_vary_response_helper_supports_wildcard_and_absent_header() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Vary: *\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with wildcard vary");
  let vary = response
    .vary()
    .expect("valid wildcard vary should parse")
    .expect("vary header should be present");

  assert!(vary.is_any());
  assert!(vary.field_names().is_empty());
  assert!(!vary.contains_field_name("accept"));

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without vary");
  assert_eq!(None, response.vary().expect("absent vary should parse"));
}

#[test]
fn test_parse_vary_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "Accept,",
    ",Accept",
    "Accept,,User-Agent",
    "Accept, ,User-Agent",
    "Accept Encoding",
    "Accept@Encoding",
    "*, Accept",
    "Accept, *",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nVary: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.vary().is_err(),
      "vary helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Vary"));
  }
}

#[test]
fn test_parse_content_language_response_helper_preserves_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Language: en-US, fr\r\n",
    "content-language: zh-Hant-TW, *\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with content-language headers");

  let content_language = response
    .content_language()
    .expect("valid content-language should parse")
    .expect("content-language header should be present");

  assert_eq!(
    vec!["en-US", "fr", "zh-Hant-TW", "*"],
    content_language.tags()
  );
  assert_eq!(
    vec![&"en-US, fr".to_string(), &"zh-Hant-TW, *".to_string()],
    response.header_values("Content-Language")
  );
}

#[test]
fn test_parse_content_language_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without content-language");

  assert_eq!(
    None,
    response
      .content_language()
      .expect("absent content-language should parse")
  );
}

#[test]
fn test_parse_content_language_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "en-US,",
    ",en-US",
    "en-US,,fr",
    "en-US, ,fr",
    "en_US",
    "en US",
    "en-",
    "-en",
    "englishlong",
    "en-toolongsubtag",
    "en-@",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Language: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_language().is_err(),
      "content-language helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Language")
    );
  }
}

#[test]
fn test_parse_content_language_rejects_duplicate_oversized_and_too_many_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Language: en-US, fr\r\n",
    "content-language: EN-us\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-language remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject normalized duplicate tags"
  );
  assert_eq!(
    vec![&"en-US, fr".to_string(), &"EN-us".to_string()],
    response.header_values("Content-Language")
  );

  let oversized = "en".repeat(32 * 1024 + 1);
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Language: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-language remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Content-Language"));

  let too_many = (0..257)
    .map(|ix| format!("x-{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Language: {too_many}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-language values remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject too many values"
  );
  assert_eq!(Some(&too_many), response.header_value("Content-Language"));
}

#[test]
fn test_parse_response_rejects_header_without_colon() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "BrokenHeader\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );

  let error = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect_err("malformed response header should be rejected");

  assert!(
    error.to_string().contains("Invalid response header"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_response_1() {
  let s = "HTTP/1.1 200 OK\r\n\
  Access-Control-Allow-Credentials: true\r\n\
  Access-Control-Allow-Origin: *\r\n\
  Content-Type: application/json\r\n\
  Date: Thu, 21 Nov 2019 02:23:24 GMT\r\n\
  Referrer-Policy: no-referrer-when-downgrade\r\n\
  Server: nginx\r\n\
  X-Content-Type-Options: nosniff\r\n\
  X-Frame-Options: DENY\r\n\
  X-XSS-Protection: 1; mode=block\r\n\
  Content-Length: 711\r\n\
  Connection: Close\r\n\
  \r\n\
  {
    \"args\": {
      \"id\": \"1\",
      \"name\": [
        \"jack\",
        \"Julia\"
      ]
    },
    \"data\": \"\",
    \"files\": {
      \"file\": \"[workspace]\\\\nmembers = [\\\\n  \\\"rttp_client\\\",\\\\n]\\\\n\"
    },
    \"form\": {
      \"debug\": \"true\",
      \"id\": \"1\",
      \"name\": [
        \"Chico\",
        \"\\u6587\",
        \"Form\"
      ],
      \"relation\": \"eq\"
    },
    \"headers\": {
      \"Content-Length\": \"863\",
      \"Content-Type\": \"multipart/form-data; boundary=---------------------------5jl1RuC429HeXVP2GOoO\",
      \"Cookie\": \"token=123234;uid=abcdef\",
      \"Host\": \"example.test\",
      \"User-Agent\": \"Mozilla/5.0\"
    },
    \"json\": null,
    \"origin\": \"222.69.134.133, 222.69.134.133\",
    \"url\": \"https://example.test/post?id=1&name=jack&name=Julia\"
  }";
  let response = Response::new(
    RoUrl::with("https://example.test/post"),
    s.as_bytes().to_vec(),
  );
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
fn test_non_chunked_response_exposes_empty_trailers() {
  let s = "HTTP/1.1 200 OK\r\n\
        Content-Length: 2\r\n\
        \r\n\
        OK";
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec());

  assert!(response.is_ok());
  let response = response.unwrap();
  assert!(response.trailers().is_empty());
  assert!(response.trailer("x-trace").is_none());
}

#[test]
fn test_no_body_status_responses_expose_empty_body_with_illegal_framing_bytes() {
  for raw in [
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    ),
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    ),
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    ),
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    ),
  ] {
    let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
      .expect("no-body status response should parse");

    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!("", response.body().string().unwrap());
  }
}
