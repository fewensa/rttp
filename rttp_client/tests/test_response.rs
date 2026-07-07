use rttp_client::response::Response;
use rttp_client::types::{Cookie, RoUrl};

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
