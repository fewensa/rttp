use rttp_client::response::Response;
use rttp_client::types::RoUrl;

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
