#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_client::response::Response;
use rttp_client::HttpClient;
use rttp_server::server::{
  HttpRateLimitLimit, HttpRateLimitLimitItem, HttpRateLimitRemaining, HttpRateLimitReset,
  HttpResponse, Request,
};

const FROM_CANONICAL: &str = "Ops Team <ops@example.test>";
const REFERER_CANONICAL: &str = "https://shop.example/checkout?step=pay";
const ACCEPT_PATCH_WIRE: &str = r#"Text/Plain; title="a,b\"c", application/json"#;
const ACCEPT_POST_WIRE: &str = "application/json, text/plain; charset=utf-8";
const RATE_LIMIT_LIMIT_WIRE: &str = "100, 50;w=3600";
const RATE_LIMIT_REMAINING_WIRE: &str = "0";
const RATE_LIMIT_RESET_WIRE: &str = "30";
const WWW_AUTHENTICATE_INPUT: &str = r#"Digest realm="apps, \"primary\"", nonce="a\\b", algorithm=SHA-256, Bearer mF_9.B5f-4.1JqM=, Basic"#;
const WWW_AUTHENTICATE_WIRE: &str = r#"Digest realm="apps, \"primary\"", nonce="a\\b", algorithm=SHA-256, Bearer mF_9.B5f-4.1JqM=, Basic"#;
const AUTHENTICATION_INFO_INPUT: &str =
  r#"nextnonce="n-2", qop=auth, rspauth="origin, \"rsp\"", cnonce="c-1", nc=00000001"#;
const AUTHENTICATION_INFO_WIRE: &str =
  r#"nextnonce=n-2, qop=auth, rspauth="origin, \"rsp\"", cnonce=c-1, nc=00000001"#;
const PROXY_AUTHENTICATE_INPUT: &str =
  r#"Basic realm="proxy", Bearer mF_9.B5f-4.1JqM, Digest realm="edge", nonce="p\\b""#;
const PROXY_AUTHENTICATE_WIRE: &str =
  r#"Basic realm="proxy", Bearer mF_9.B5f-4.1JqM, Digest realm="edge", nonce="p\\b""#;
const PROXY_AUTHENTICATION_INFO_INPUT: &str =
  r#"nextnonce="p-2", qop=auth, rspauth="proxy, \"rsp\"", cnonce="pc-1", nc=00000001"#;
const PROXY_AUTHENTICATION_INFO_WIRE: &str =
  r#"nextnonce=p-2, qop=auth, rspauth="proxy, \"rsp\"", cnonce=pc-1, nc=00000001"#;
const BODY: &str = "metadata-http11-interoperability";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
struct ObservedRequestMetadata {
  version: String,
  target: String,
  from: Result<Option<String>, String>,
  from_address: Option<String>,
  raw_from: Option<String>,
  referer: Result<Option<String>, String>,
  raw_referer: Option<String>,
}

fn client() -> HttpClient {
  rttp::Http::client()
}

fn bind_facade_server() -> rttp_server::server::HttpServer {
  rttp::Http::server("127.0.0.1:0")
    .expect("bind metadata facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT))
}

fn observe_request(request: &Request) -> ObservedRequestMetadata {
  ObservedRequestMetadata {
    version: request.version().to_string(),
    target: request.target().to_string(),
    from: request
      .from()
      .map(|from| from.map(|from| from.header_value()))
      .map_err(|error| error.to_string()),
    from_address: request
      .from()
      .ok()
      .flatten()
      .map(|from| from.address().to_string()),
    raw_from: request.header("From").map(str::to_string),
    referer: request
      .referer()
      .map(|referer| referer.map(|referer| referer.header_value()))
      .map_err(|error| error.to_string()),
    raw_referer: request.header("Referer").map(str::to_string),
  }
}

fn metadata_response(body: &'static str) -> HttpResponse {
  HttpResponse::ok(body)
    .with_accept_patch([r#"Text/Plain; title="a,b\"c""#, "application/json"])
    .expect("response Accept-Patch should be accepted")
    .with_accept_post(["application/json", "text/plain; charset=utf-8"])
    .expect("response Accept-Post should be accepted")
    .with_rate_limit_limit(HttpRateLimitLimit::new([
      HttpRateLimitLimitItem::new(100),
      HttpRateLimitLimitItem::new(50).with_window(3_600),
    ]))
    .expect("response RateLimit-Limit should be accepted")
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(0))
    .expect("response RateLimit-Remaining should be accepted")
    .with_rate_limit_reset(HttpRateLimitReset::new(30))
    .expect("response RateLimit-Reset should be accepted")
    .with_www_authenticate(WWW_AUTHENTICATE_INPUT)
    .expect("response WWW-Authenticate should be accepted")
    .with_authentication_info(AUTHENTICATION_INFO_INPUT)
    .expect("response Authentication-Info should be accepted")
    .with_proxy_authenticate(PROXY_AUTHENTICATE_INPUT)
    .expect("response Proxy-Authenticate should be accepted")
    .with_proxy_authentication_info(PROXY_AUTHENTICATION_INFO_INPUT)
    .expect("response Proxy-Authentication-Info should be accepted")
}

fn spawn_observed_facade_server(
  response: impl Fn(Request) -> HttpResponse + Send + 'static,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedRequestMetadata>,
  thread::JoinHandle<()>,
) {
  let server = bind_facade_server();
  let addr = server.local_addr().expect("facade server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_request(&request))
          .expect("send observed metadata");
        response(request)
      })
      .expect("serve metadata request");
  });
  (addr, observed_rx, handle)
}

fn attach_valid_client_metadata(client: &mut HttpClient) -> &mut HttpClient {
  client
    .from(" Ops\t Team  <ops@example.test> ")
    .expect("From should be accepted")
    .referer("\thttps://shop.example/checkout?step=pay\t")
    .expect("Referer should be accepted")
}

fn assert_valid_request_metadata(observed: &ObservedRequestMetadata) {
  assert_eq!("HTTP/1.1", observed.version);
  assert_eq!("/asset", observed.target);
  assert_eq!(Ok(Some(FROM_CANONICAL.to_string())), observed.from);
  assert_eq!(Some("ops@example.test".to_string()), observed.from_address);
  assert_eq!(Some(FROM_CANONICAL.to_string()), observed.raw_from);
  assert_eq!(Ok(Some(REFERER_CANONICAL.to_string())), observed.referer);
  assert_eq!(Some(REFERER_CANONICAL.to_string()), observed.raw_referer);
}

fn assert_valid_response_metadata(response: &Response) {
  assert_eq!(200, response.code());
  assert_eq!(
    BODY,
    response
      .body()
      .string()
      .expect("response body should remain ordinary HTTP bytes")
  );

  let www_authenticate = response
    .www_authenticate()
    .expect("WWW-Authenticate should parse")
    .expect("WWW-Authenticate should be present");
  assert_eq!(WWW_AUTHENTICATE_WIRE, www_authenticate.header_value());
  assert_eq!(3, www_authenticate.len());
  assert_eq!("Digest", www_authenticate.challenges()[0].scheme());
  assert_eq!(
    vec!["realm", "nonce", "algorithm"],
    www_authenticate.challenges()[0]
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("apps, \"primary\""),
    www_authenticate.challenges()[0].parameter("realm")
  );
  assert_eq!(
    Some("a\\b"),
    www_authenticate.challenges()[0].parameter("nonce")
  );
  assert_eq!("Bearer", www_authenticate.challenges()[1].scheme());
  assert_eq!(
    Some("mF_9.B5f-4.1JqM="),
    www_authenticate.challenges()[1].token68()
  );
  assert_eq!("Basic", www_authenticate.challenges()[2].scheme());
  assert_eq!(
    Some(WWW_AUTHENTICATE_WIRE),
    response
      .header_value("WWW-Authenticate")
      .map(String::as_str)
  );

  let authentication_info = response
    .authentication_info()
    .expect("Authentication-Info should parse")
    .expect("Authentication-Info should be present");
  assert_eq!(AUTHENTICATION_INFO_WIRE, authentication_info.header_value());
  assert_eq!(
    vec!["nextnonce", "qop", "rspauth", "cnonce", "nc"],
    authentication_info
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(Some("n-2"), authentication_info.parameter("nextnonce"));
  assert_eq!(Some("auth"), authentication_info.parameter("qop"));
  assert_eq!(
    Some("origin, \"rsp\""),
    authentication_info.parameter("rspauth")
  );
  assert_eq!(
    Some(AUTHENTICATION_INFO_WIRE),
    response
      .header_value("Authentication-Info")
      .map(String::as_str)
  );

  let proxy_authenticate = response
    .proxy_authenticate()
    .expect("Proxy-Authenticate should parse")
    .expect("Proxy-Authenticate should be present");
  assert_eq!(PROXY_AUTHENTICATE_WIRE, proxy_authenticate.header_value());
  assert_eq!(3, proxy_authenticate.len());
  assert_eq!("Basic", proxy_authenticate.challenges()[0].scheme());
  assert_eq!(
    Some("proxy"),
    proxy_authenticate.challenges()[0].parameter("realm")
  );
  assert_eq!("Bearer", proxy_authenticate.challenges()[1].scheme());
  assert_eq!(
    Some("mF_9.B5f-4.1JqM"),
    proxy_authenticate.challenges()[1].token68()
  );
  assert_eq!("Digest", proxy_authenticate.challenges()[2].scheme());
  assert_eq!(
    vec!["realm", "nonce"],
    proxy_authenticate.challenges()[2]
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("edge"),
    proxy_authenticate.challenges()[2].parameter("realm")
  );
  assert_eq!(
    Some("p\\b"),
    proxy_authenticate.challenges()[2].parameter("nonce")
  );
  assert_eq!(
    Some(PROXY_AUTHENTICATE_WIRE),
    response
      .header_value("Proxy-Authenticate")
      .map(String::as_str)
  );

  let proxy_authentication_info = response
    .proxy_authentication_info()
    .expect("Proxy-Authentication-Info should parse")
    .expect("Proxy-Authentication-Info should be present");
  assert_eq!(
    PROXY_AUTHENTICATION_INFO_WIRE,
    proxy_authentication_info.header_value()
  );
  assert_eq!(
    vec!["nextnonce", "qop", "rspauth", "cnonce", "nc"],
    proxy_authentication_info
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("p-2"),
    proxy_authentication_info.parameter("nextnonce")
  );
  assert_eq!(Some("auth"), proxy_authentication_info.parameter("qop"));
  assert_eq!(
    Some("proxy, \"rsp\""),
    proxy_authentication_info.parameter("rspauth")
  );
  assert_eq!(
    Some(PROXY_AUTHENTICATION_INFO_WIRE),
    response
      .header_value("Proxy-Authentication-Info")
      .map(String::as_str)
  );

  let accept_patch = response
    .accept_patch()
    .expect("Accept-Patch should parse")
    .expect("Accept-Patch should be present");
  assert_eq!(ACCEPT_PATCH_WIRE, accept_patch.header_value());
  assert_eq!(2, accept_patch.media_types().len());
  assert_eq!("Text", accept_patch.media_types()[0].type_());
  assert_eq!("Plain", accept_patch.media_types()[0].subtype());
  assert_eq!(
    "a,b\"c",
    accept_patch.media_types()[0].parameters()[0].value()
  );
  assert_eq!("application", accept_patch.media_types()[1].type_());
  assert_eq!("json", accept_patch.media_types()[1].subtype());
  assert_eq!(
    Some(ACCEPT_PATCH_WIRE),
    response.header_value("Accept-Patch").map(String::as_str)
  );

  let accept_post = response
    .accept_post()
    .expect("Accept-Post should parse")
    .expect("Accept-Post should be present");
  assert_eq!(ACCEPT_POST_WIRE, accept_post.header_value());
  assert_eq!(2, accept_post.media_types().len());
  assert_eq!("application", accept_post.media_types()[0].type_());
  assert_eq!("json", accept_post.media_types()[0].subtype());
  assert_eq!("text", accept_post.media_types()[1].type_());
  assert_eq!("plain", accept_post.media_types()[1].subtype());
  assert_eq!(
    "charset",
    accept_post.media_types()[1].parameters()[0].name()
  );
  assert_eq!(
    "utf-8",
    accept_post.media_types()[1].parameters()[0].value()
  );
  assert_eq!(
    Some(ACCEPT_POST_WIRE),
    response.header_value("Accept-Post").map(String::as_str)
  );

  let limit = response
    .rate_limit_limit()
    .expect("RateLimit-Limit should parse")
    .expect("RateLimit-Limit should be present");
  assert_eq!(RATE_LIMIT_LIMIT_WIRE, limit.header_value());
  assert_eq!(
    &[(100, None), (50, Some(3_600))],
    limit
      .items()
      .iter()
      .map(|item| (item.value(), item.window()))
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    Some(RATE_LIMIT_LIMIT_WIRE),
    response.header_value("RateLimit-Limit").map(String::as_str)
  );

  let remaining = response
    .rate_limit_remaining()
    .expect("RateLimit-Remaining should parse")
    .expect("RateLimit-Remaining should be present");
  assert_eq!(RATE_LIMIT_REMAINING_WIRE, remaining.header_value());
  assert_eq!(0, remaining.value());
  assert_eq!(
    Some(RATE_LIMIT_REMAINING_WIRE),
    response
      .header_value("RateLimit-Remaining")
      .map(String::as_str)
  );

  let reset = response
    .rate_limit_reset()
    .expect("RateLimit-Reset should parse")
    .expect("RateLimit-Reset should be present");
  assert_eq!(RATE_LIMIT_RESET_WIRE, reset.header_value());
  assert_eq!(30, reset.value());
  assert_eq!(
    Some(RATE_LIMIT_RESET_WIRE),
    response.header_value("RateLimit-Reset").map(String::as_str)
  );
}

fn reject_before_connect(
  label: &str,
  apply: impl FnOnce(&mut HttpClient) -> rttp_client::error::Result<&mut HttpClient>,
) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused metadata listener");
  listener
    .set_nonblocking(true)
    .expect("unused metadata listener should be nonblocking");
  let addr = listener
    .local_addr()
    .expect("unused metadata listener addr");
  let mut client = client();
  client.get().url(format!("http://{addr}/asset"));
  let error = apply(&mut client).expect_err(label);
  assert!(
    error.is_builder(),
    "{label} must fail as a builder error before connect"
  );
  assert!(listener.accept().is_err(), "{label} must not open a socket");
}

#[test]
fn http11_facade_roundtrip_exchanges_all_metadata_families() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| metadata_response(BODY));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/asset")))
    .emit()
    .expect("HTTP/1.1 metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe HTTP/1.1 metadata");
  assert_valid_request_metadata(&observed);
  assert_valid_response_metadata(&response);
  handle.join().expect("HTTP/1.1 metadata server thread");
}

#[cfg(feature = "async")]
#[test]
fn async_http11_facade_roundtrip_exchanges_all_metadata_families() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| metadata_response(BODY));

  let response = block_on(async {
    let mut client = client();
    attach_valid_client_metadata(client.get().url(format!("http://{addr}/asset")))
      .rasync()
      .await
      .expect("async HTTP/1.1 metadata response should parse")
  });

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe async HTTP/1.1 metadata");
  assert_valid_request_metadata(&observed);
  assert_valid_response_metadata(&response);
  handle
    .join()
    .expect("async HTTP/1.1 metadata server thread");
}

#[test]
fn http11_absent_metadata_returns_ok_none() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("absent-metadata"));

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("absent metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe absent metadata request");
  assert_eq!(Ok(None), observed.from);
  assert_eq!(None, observed.raw_from);
  assert_eq!(Ok(None), observed.referer);
  assert_eq!(None, observed.raw_referer);

  assert!(response
    .accept_patch()
    .expect("absent Accept-Patch should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Accept-Patch"));
  assert!(response
    .accept_post()
    .expect("absent Accept-Post should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Accept-Post"));
  assert!(response
    .rate_limit_limit()
    .expect("absent RateLimit-Limit should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("RateLimit-Limit"));
  assert!(response
    .rate_limit_remaining()
    .expect("absent RateLimit-Remaining should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("RateLimit-Remaining"));
  assert!(response
    .rate_limit_reset()
    .expect("absent RateLimit-Reset should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("RateLimit-Reset"));
  assert!(response
    .www_authenticate()
    .expect("absent WWW-Authenticate should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("WWW-Authenticate"));
  assert!(response
    .authentication_info()
    .expect("absent Authentication-Info should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Authentication-Info"));
  assert!(response
    .proxy_authenticate()
    .expect("absent Proxy-Authenticate should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Proxy-Authenticate"));
  assert!(response
    .proxy_authentication_info()
    .expect("absent Proxy-Authentication-Info should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Proxy-Authentication-Info"));
  assert_eq!(
    "absent-metadata",
    response.body().string().expect("absent metadata body")
  );
  handle.join().expect("absent metadata server thread");
}

#[test]
fn typed_request_helpers_reject_malformed_values_before_connect() {
  reject_before_connect("malformed From mailbox", |client| client.from("ops"));
  reject_before_connect("From with control byte", |client| {
    client.from("ops@example.test\0")
  });
  reject_before_connect("oversized From", |client| {
    client.from(format!("{}@example.test", "a".repeat(64 * 1024)))
  });
  reject_before_connect("Referer with fragment", |client| {
    client.referer("https://example.test/path#frag")
  });
  reject_before_connect("Referer with control byte", |client| {
    client.referer("https://example.test/path\0")
  });
  reject_before_connect("oversized Referer", |client| {
    client.referer("a".repeat(64 * 1024 + 1))
  });
}

#[test]
fn response_builders_reject_malformed_values_without_replacing_fields() {
  let original = HttpResponse::ok("body")
    .header("Accept-Patch", "application/json")
    .header("Accept-Post", "text/plain")
    .header("RateLimit-Limit", "1")
    .header("RateLimit-Remaining", "2")
    .header("RateLimit-Reset", "3");

  assert!(original
    .clone()
    .with_accept_patch(["not-a-media-type"])
    .is_err());
  assert!(original
    .clone()
    .with_accept_post(["not-a-media-type"])
    .is_err());
  assert!(original
    .clone()
    .with_rate_limit_limit(HttpRateLimitLimit::new([]))
    .is_err());
  assert!(original
    .clone()
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(1_000_000_000_000_000))
    .is_err());
  assert!(original
    .clone()
    .with_rate_limit_reset(HttpRateLimitReset::new(1_000_000_000_000_000))
    .is_err());

  let serialized = String::from_utf8(original.to_bytes()).expect("response should serialize");
  assert!(serialized.contains("Accept-Patch: application/json"));
  assert!(serialized.contains("Accept-Post: text/plain"));
  assert!(serialized.contains("RateLimit-Limit: 1"));
  assert!(serialized.contains("RateLimit-Remaining: 2"));
  assert!(serialized.contains("RateLimit-Reset: 3"));
}

#[test]
fn facade_server_preserves_raw_headers_when_typed_request_helpers_reject_malformed_values() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("malformed-request"));

  let mut stream = TcpStream::connect(addr).expect("connect malformed metadata request");
  stream
    .set_read_timeout(Some(TIMEOUT))
    .expect("set malformed request read timeout");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set malformed request write timeout");
  stream
    .write_all(
      b"GET /asset HTTP/1.1\r\n\
Host: example.test\r\n\
From: ops\r\n\
Referer: https://example.test/path#frag\r\n\
Connection: close\r\n\
\r\n",
    )
    .expect("write malformed metadata request");
  let mut response = Vec::new();
  stream
    .read_to_end(&mut response)
    .expect("read malformed metadata response");
  let response = String::from_utf8(response).expect("response should be utf-8");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe malformed request metadata");
  assert!(observed.from.is_err());
  assert_eq!(Some("ops".to_string()), observed.raw_from);
  assert!(observed.referer.is_err());
  assert_eq!(
    Some("https://example.test/path#frag".to_string()),
    observed.raw_referer
  );
  assert!(
    response.starts_with("HTTP/1.1 200 "),
    "malformed typed metadata must not fail the HTTP exchange: {response}"
  );
  assert!(response.contains("malformed-request"));
  handle.join().expect("malformed request server thread");
}

#[test]
fn facade_server_preserves_raw_headers_when_duplicate_request_fields_are_rejected() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("duplicate-request"));

  let mut stream = TcpStream::connect(addr).expect("connect duplicate metadata request");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set duplicate request write timeout");
  stream
    .write_all(
      b"GET /asset HTTP/1.1\r\n\
Host: example.test\r\n\
From: ops@example.test\r\n\
from: other@example.test\r\n\
Referer: https://shop.example/a\r\n\
referer: https://shop.example/b\r\n\
Connection: close\r\n\
\r\n",
    )
    .expect("write duplicate metadata request");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe duplicate metadata");
  assert!(observed.from.is_err());
  assert_eq!(Some("ops@example.test".to_string()), observed.raw_from);
  assert!(observed.referer.is_err());
  assert_eq!(
    Some("https://shop.example/a".to_string()),
    observed.raw_referer
  );
  handle.join().expect("duplicate request server thread");
}

#[test]
fn live_http11_response_helpers_reject_malformed_metadata_while_preserving_raw_headers() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("invalid-response")
      .header("Accept-Patch", "not-a-media-type")
      .header("Accept-Post", "application/json,")
      .header("RateLimit-Limit", "100, (50)")
      .header("RateLimit-Remaining", "1, 2")
      .header("RateLimit-Reset", "1\0")
      .header("WWW-Authenticate", "Basic realm=\"unterminated")
      .header("Authentication-Info", "nextnonce")
      .header("Proxy-Authenticate", "Basic @")
      .header("Proxy-Authentication-Info", "rspauth")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("invalid response metadata headers should remain observable");

  let _ = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the invalid-response request");

  assert!(response.accept_patch().is_err());
  assert_eq!(
    Some("not-a-media-type"),
    response.header_value("Accept-Patch").map(String::as_str)
  );
  assert!(response.accept_post().is_err());
  assert_eq!(
    Some("application/json,"),
    response.header_value("Accept-Post").map(String::as_str)
  );
  assert!(response.rate_limit_limit().is_err());
  assert_eq!(
    Some("100, (50)"),
    response.header_value("RateLimit-Limit").map(String::as_str)
  );
  assert!(response.rate_limit_remaining().is_err());
  assert_eq!(
    Some("1, 2"),
    response
      .header_value("RateLimit-Remaining")
      .map(String::as_str)
  );
  assert!(response.rate_limit_reset().is_err());
  assert_eq!(
    Some("1\0"),
    response.header_value("RateLimit-Reset").map(String::as_str)
  );
  assert!(response.www_authenticate().is_err());
  assert_eq!(
    Some("Basic realm=\"unterminated"),
    response
      .header_value("WWW-Authenticate")
      .map(String::as_str)
  );
  assert!(response.authentication_info().is_err());
  assert_eq!(
    Some("nextnonce"),
    response
      .header_value("Authentication-Info")
      .map(String::as_str)
  );
  assert!(response.proxy_authenticate().is_err());
  assert_eq!(
    Some("Basic @"),
    response
      .header_value("Proxy-Authenticate")
      .map(String::as_str)
  );
  assert!(response.proxy_authentication_info().is_err());
  assert_eq!(
    Some("rspauth"),
    response
      .header_value("Proxy-Authentication-Info")
      .map(String::as_str)
  );
  assert_eq!(200, response.code());
  assert_eq!(
    "invalid-response",
    response.body().string().expect("invalid response body")
  );
  handle
    .join()
    .expect("invalid HTTP/1.1 response server thread");
}

#[test]
fn live_http11_authentication_metadata_combines_repeated_fields_in_wire_order() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("repeated-auth")
      .header("WWW-Authenticate", "Digest realm=apps")
      .header("WWW-Authenticate", "nonce=abc")
      .header("WWW-Authenticate", "Basic")
      .header("Authentication-Info", "nextnonce=origin")
      .header("Authentication-Info", "qop=auth")
      .header("Proxy-Authenticate", "Digest realm=proxy")
      .header("Proxy-Authenticate", "nonce=xyz")
      .header("Proxy-Authenticate", "Basic")
      .header("Proxy-Authentication-Info", "nextnonce=proxy")
      .header("Proxy-Authentication-Info", "qop=auth")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("repeated authentication metadata response should parse");
  let _ = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe repeated authentication metadata request");

  let www_authenticate = response
    .www_authenticate()
    .expect("repeated WWW-Authenticate fields should parse")
    .expect("repeated WWW-Authenticate fields should be present");
  assert_eq!(
    vec!["Digest", "Basic"],
    www_authenticate
      .challenges()
      .iter()
      .map(|challenge| challenge.scheme())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec!["realm", "nonce"],
    www_authenticate.challenges()[0]
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("apps"),
    www_authenticate.challenges()[0].parameter("realm")
  );
  assert_eq!(
    Some("abc"),
    www_authenticate.challenges()[0].parameter("nonce")
  );
  assert_eq!(
    vec![
      "Digest realm=apps".to_string(),
      "nonce=abc".to_string(),
      "Basic".to_string()
    ],
    response
      .header_values("WWW-Authenticate")
      .iter()
      .map(|value| value.as_str())
      .map(str::to_string)
      .collect::<Vec<_>>()
  );

  let authentication_info = response
    .authentication_info()
    .expect("repeated Authentication-Info fields should parse")
    .expect("repeated Authentication-Info fields should be present");
  assert_eq!(
    vec!["nextnonce", "qop"],
    authentication_info
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
  );
  assert_eq!(Some("origin"), authentication_info.parameter("nextnonce"));
  assert_eq!(Some("auth"), authentication_info.parameter("qop"));
  assert_eq!(
    "nextnonce=origin, qop=auth",
    authentication_info.header_value()
  );

  let proxy_authenticate = response
    .proxy_authenticate()
    .expect("repeated Proxy-Authenticate fields should parse")
    .expect("repeated Proxy-Authenticate fields should be present");
  assert_eq!(
    vec!["Digest", "Basic"],
    proxy_authenticate
      .challenges()
      .iter()
      .map(|challenge| challenge.scheme())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("proxy"),
    proxy_authenticate.challenges()[0].parameter("realm")
  );
  assert_eq!(
    Some("xyz"),
    proxy_authenticate.challenges()[0].parameter("nonce")
  );
  assert_eq!(
    vec![
      "Digest realm=proxy".to_string(),
      "nonce=xyz".to_string(),
      "Basic".to_string()
    ],
    response
      .header_values("Proxy-Authenticate")
      .iter()
      .map(|value| value.as_str())
      .map(str::to_string)
      .collect::<Vec<_>>()
  );

  let proxy_authentication_info = response
    .proxy_authentication_info()
    .expect("repeated Proxy-Authentication-Info fields should parse")
    .expect("repeated Proxy-Authentication-Info fields should be present");
  assert_eq!(
    Some("proxy"),
    proxy_authentication_info.parameter("nextnonce")
  );
  assert_eq!(Some("auth"), proxy_authentication_info.parameter("qop"));
  assert_eq!(
    "nextnonce=proxy, qop=auth",
    proxy_authentication_info.header_value()
  );

  assert_eq!(
    "repeated-auth",
    response.body().string().expect("response body")
  );
  handle
    .join()
    .expect("repeated authentication metadata server thread");
}

#[test]
fn response_authentication_builders_replace_and_reject_without_losing_raw_fields() {
  let original = HttpResponse::ok("builder-auth")
    .header("WWW-Authenticate", "legacy-www")
    .header("www-authenticate", "legacy-www-case")
    .header("Authentication-Info", "legacy-info")
    .header("authentication-info", "legacy-info-case")
    .header("Proxy-Authenticate", "legacy-proxy-www")
    .header("proxy-authenticate", "legacy-proxy-www-case")
    .header("Proxy-Authentication-Info", "legacy-proxy-info")
    .header("proxy-authentication-info", "legacy-proxy-info-case");

  let response = original
    .clone()
    .with_www_authenticate(WWW_AUTHENTICATE_INPUT)
    .expect("WWW-Authenticate builder should accept valid metadata")
    .with_authentication_info(AUTHENTICATION_INFO_INPUT)
    .expect("Authentication-Info builder should accept valid metadata")
    .with_proxy_authenticate(PROXY_AUTHENTICATE_INPUT)
    .expect("Proxy-Authenticate builder should accept valid metadata")
    .with_proxy_authentication_info(PROXY_AUTHENTICATION_INFO_INPUT)
    .expect("Proxy-Authentication-Info builder should accept valid metadata");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert!(!serialized.contains("legacy-"));
  assert!(serialized.contains(&format!("WWW-Authenticate: {WWW_AUTHENTICATE_WIRE}")));
  assert!(serialized.contains(&format!("Authentication-Info: {AUTHENTICATION_INFO_WIRE}")));
  assert!(serialized.contains(&format!("Proxy-Authenticate: {PROXY_AUTHENTICATE_WIRE}")));
  assert!(serialized.contains(&format!(
    "Proxy-Authentication-Info: {PROXY_AUTHENTICATION_INFO_WIRE}"
  )));
  assert_eq!(
    WWW_AUTHENTICATE_WIRE,
    response
      .www_authenticate()
      .expect("WWW-Authenticate should parse")
      .expect("WWW-Authenticate should be present")
      .header_value()
  );
  assert_eq!(
    AUTHENTICATION_INFO_WIRE,
    response
      .authentication_info()
      .expect("Authentication-Info should parse")
      .expect("Authentication-Info should be present")
      .header_value()
  );
  assert_eq!(
    PROXY_AUTHENTICATE_WIRE,
    response
      .proxy_authenticate()
      .expect("Proxy-Authenticate should parse")
      .expect("Proxy-Authenticate should be present")
      .header_value()
  );
  assert_eq!(
    PROXY_AUTHENTICATION_INFO_WIRE,
    response
      .proxy_authentication_info()
      .expect("Proxy-Authentication-Info should parse")
      .expect("Proxy-Authentication-Info should be present")
      .header_value()
  );

  assert!(original.clone().with_www_authenticate("Basic @").is_err());
  assert!(original
    .clone()
    .with_authentication_info("nextnonce")
    .is_err());
  assert!(original.clone().with_proxy_authenticate("Basic @").is_err());
  assert!(original
    .clone()
    .with_proxy_authentication_info("rspauth")
    .is_err());
  let original_serialized =
    String::from_utf8(original.to_bytes()).expect("original response should serialize");
  for value in [
    "legacy-www",
    "legacy-www-case",
    "legacy-info",
    "legacy-info-case",
    "legacy-proxy-www",
    "legacy-proxy-www-case",
    "legacy-proxy-info",
    "legacy-proxy-info-case",
  ] {
    assert!(
      original_serialized.contains(value),
      "original response should retain {value:?}"
    );
  }
}

#[test]
fn live_http11_authentication_metadata_enforces_documented_bounds() {
  let oversized_www = format!("Basic realm={}", "a".repeat(64 * 1024));
  let oversized_info = format!("value={}", "a".repeat(64 * 1024));
  let too_many_challenges = (0..257)
    .map(|index| format!("Scheme{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_parameters = (0..257)
    .map(|index| format!("p{index}=v"))
    .collect::<Vec<_>>()
    .join(", ");

  assert!(HttpResponse::ok("")
    .with_www_authenticate(oversized_www.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_authentication_info(oversized_info.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_proxy_authenticate(oversized_www.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_proxy_authentication_info(oversized_info.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_www_authenticate(too_many_challenges.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_authentication_info(too_many_parameters.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_proxy_authenticate(too_many_challenges.clone())
    .is_err());
  assert!(HttpResponse::ok("")
    .with_proxy_authentication_info(too_many_parameters.clone())
    .is_err());

  let expected_oversized_www = oversized_www.clone();
  let expected_oversized_info = oversized_info.clone();
  let expected_too_many_challenges = too_many_challenges.clone();
  let expected_too_many_parameters = too_many_parameters.clone();
  let (addr, observed_rx, handle) = spawn_observed_facade_server(move |_| {
    HttpResponse::ok("bounds-auth")
      .header("WWW-Authenticate", oversized_www.clone())
      .header("Authentication-Info", oversized_info.clone())
      .header("Proxy-Authenticate", too_many_challenges.clone())
      .header("Proxy-Authentication-Info", too_many_parameters.clone())
  });
  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("bounded metadata response should remain parseable");
  let _ = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe bounds metadata request");

  assert!(response.www_authenticate().is_err());
  assert_eq!(
    Some(expected_oversized_www.as_str()),
    response
      .header_value("WWW-Authenticate")
      .map(String::as_str)
  );
  assert!(response.authentication_info().is_err());
  assert_eq!(
    Some(expected_oversized_info.as_str()),
    response
      .header_value("Authentication-Info")
      .map(String::as_str)
  );
  assert!(response.proxy_authenticate().is_err());
  assert_eq!(
    Some(expected_too_many_challenges.as_str()),
    response
      .header_value("Proxy-Authenticate")
      .map(String::as_str)
  );
  assert!(response.proxy_authentication_info().is_err());
  assert_eq!(
    Some(expected_too_many_parameters.as_str()),
    response
      .header_value("Proxy-Authentication-Info")
      .map(String::as_str)
  );
  assert_eq!(
    "bounds-auth",
    response.body().string().expect("response body")
  );
  handle.join().expect("bounds metadata server thread");
}
