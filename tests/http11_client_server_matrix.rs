use flate2::write::GzEncoder;
use flate2::Compression;
#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use rttp_client::HttpClient;
use rttp_server::server::{
  HttpByteRange, HttpByteRangeError, HttpConditionalMetadata, HttpConditionalRequestOutcome,
  HttpContentDisposition, HttpContentType, HttpEntityTag, HttpIfRangeRequestOutcome, HttpResponse,
  Request, SecFetchDest, SecFetchMode, SecFetchSite, SecPurpose,
};
use rttp_test_support as fixtures;

type ObservedIfRangeHeaders = (Option<String>, Option<String>);
type ObservedIfRangeHandle = thread::JoinHandle<ObservedIfRangeHeaders>;

fn client() -> HttpClient {
  HttpClient::new()
}

#[derive(Debug, PartialEq)]
struct ObservedCorsPreflight {
  method: String,
  origin: Option<String>,
  raw_request_method: Option<String>,
  raw_request_headers: Option<String>,
  raw_request_private_network: Option<String>,
  request_method: Result<Option<String>, String>,
  request_headers: Result<Option<Vec<String>>, String>,
  request_private_network: Result<Option<String>, String>,
}

fn observe_cors_preflight(request: Request) -> ObservedCorsPreflight {
  ObservedCorsPreflight {
    method: request.method().to_string(),
    origin: request.header("Origin").map(str::to_string),
    raw_request_method: request
      .header("Access-Control-Request-Method")
      .map(str::to_string),
    raw_request_headers: request
      .header("Access-Control-Request-Headers")
      .map(str::to_string),
    raw_request_private_network: request
      .header("Access-Control-Request-Private-Network")
      .map(str::to_string),
    request_method: request
      .access_control_request_method()
      .map(|method| method.map(|method| method.method().to_string()))
      .map_err(|error| error.to_string()),
    request_headers: request
      .access_control_request_headers()
      .map(|headers| headers.map(|headers| headers.field_names().to_vec()))
      .map_err(|error| error.to_string()),
    request_private_network: request
      .access_control_request_private_network()
      .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
      .map_err(|error| error.to_string()),
  }
}

#[derive(Debug, PartialEq)]
struct ObservedSignatureMetadata {
  raw_signature: Option<String>,
  raw_signature_input: Option<String>,
  signature: Result<Option<String>, String>,
  signature_input: Result<Option<String>, String>,
}

fn observe_signature_metadata(request: &Request) -> ObservedSignatureMetadata {
  ObservedSignatureMetadata {
    raw_signature: request.header("Signature").map(str::to_string),
    raw_signature_input: request.header("Signature-Input").map(str::to_string),
    signature: request
      .signature()
      .map(|signature| signature.map(|signature| signature.header_value()))
      .map_err(|error| error.to_string()),
    signature_input: request
      .signature_input()
      .map(|signature_input| signature_input.map(|signature_input| signature_input.header_value()))
      .map_err(|error| error.to_string()),
  }
}

#[derive(Debug, PartialEq)]
struct ObservedAcceptLanguageMetadata {
  ranges: Result<Option<Vec<String>>, String>,
  qualities: Result<Option<Vec<Option<String>>>, String>,
}

fn observe_accept_language_metadata(request: &Request) -> ObservedAcceptLanguageMetadata {
  ObservedAcceptLanguageMetadata {
    ranges: request
      .accept_language()
      .map(|languages| {
        languages.map(|languages| languages.ranges().into_iter().map(str::to_owned).collect())
      })
      .map_err(|error| error.to_string()),
    qualities: request
      .accept_language()
      .map(|languages| {
        languages.map(|languages| {
          languages
            .qualities()
            .into_iter()
            .map(|quality| quality.map(str::to_owned))
            .collect()
        })
      })
      .map_err(|error| error.to_string()),
  }
}

#[derive(Debug, PartialEq)]
struct ObservedAcceptCharsetMetadata {
  ranges: Result<Option<Vec<(String, u16)>>, String>,
  raw: Option<String>,
}

fn observe_accept_charset_metadata(request: &Request) -> ObservedAcceptCharsetMetadata {
  ObservedAcceptCharsetMetadata {
    ranges: request
      .accept_charset()
      .map(|charsets| {
        charsets.map(|charsets| {
          charsets
            .charsets()
            .iter()
            .map(|range| (range.charset().to_owned(), range.quality()))
            .collect()
        })
      })
      .map_err(|error| error.to_string()),
    raw: request.header("Accept-Charset").map(str::to_owned),
  }
}

fn spawn_facade_accept_charset_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedAcceptCharsetMetadata>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind Accept-Charset facade server");
  let addr = server.local_addr().expect("Accept-Charset facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_accept_charset_metadata(&request))
          .expect("send observed Accept-Charset metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve Accept-Charset facade request");
  });

  (addr, observed_rx, handle)
}

fn spawn_facade_accept_language_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedAcceptLanguageMetadata>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind Accept-Language facade server");
  let addr = server.local_addr().expect("Accept-Language facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_accept_language_metadata(&request))
          .expect("send observed Accept-Language metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve Accept-Language facade request");
  });

  (addr, observed_rx, handle)
}

fn spawn_facade_signature_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedSignatureMetadata>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind signature facade server");
  let addr = server.local_addr().expect("signature facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = observe_signature_metadata(&request);
        let mut response = HttpResponse::ok("OK");
        if let (Ok(Some(signature_input)), Ok(Some(signature))) =
          (request.signature_input(), request.signature())
        {
          response = response
            .with_signature_input(signature_input.header_value())
            .expect("echo Signature-Input")
            .with_signature(signature.header_value())
            .expect("echo Signature");
        }
        observed_tx
          .send(observed)
          .expect("send observed signature metadata");
        response
      })
      .expect("serve signature facade request");
  });

  (addr, observed_rx, handle)
}

#[derive(Debug, PartialEq)]
struct ObservedDigestMetadata {
  raw_want_content_digest: Option<String>,
  raw_want_repr_digest: Option<String>,
  want_content_digest: Result<Option<String>, String>,
  want_repr_digest: Result<Option<String>, String>,
}

fn observe_digest_metadata(request: &Request) -> ObservedDigestMetadata {
  ObservedDigestMetadata {
    raw_want_content_digest: request.header("Want-Content-Digest").map(str::to_string),
    raw_want_repr_digest: request.header("Want-Repr-Digest").map(str::to_string),
    want_content_digest: request
      .want_content_digest()
      .map(|digest| digest.map(|digest| digest.header_value()))
      .map_err(|error| error.to_string()),
    want_repr_digest: request
      .want_repr_digest()
      .map(|digest| digest.map(|digest| digest.header_value()))
      .map_err(|error| error.to_string()),
  }
}

fn spawn_facade_digest_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedDigestMetadata>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind digest facade server");
  let addr = server.local_addr().expect("digest facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = observe_digest_metadata(&request);
        let mut response = HttpResponse::ok("OK");
        if let (Ok(Some(_)), Ok(Some(_))) =
          (request.want_content_digest(), request.want_repr_digest())
        {
          response = response
            .with_digest("sha-256=:YWJj:, sha-512=:ZGVm:")
            .expect("attach Content-Digest")
            .with_repr_digest("sha-256=:Z2hp:")
            .expect("attach Repr-Digest");
        }
        observed_tx
          .send(observed)
          .expect("send observed digest metadata");
        response
      })
      .expect("serve digest facade request");
  });

  (addr, observed_rx, handle)
}

fn spawn_facade_cors_preflight_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedCorsPreflight>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind CORS preflight facade server");
  let addr = server.local_addr().expect("CORS preflight facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_cors_preflight(request))
          .expect("send observed CORS preflight metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve CORS preflight facade request");
  });

  (addr, observed_rx, handle)
}

fn cache_control_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Cache-Control: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn cache_status_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Cache-Status: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn cdn_cache_control_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("CDN-Cache-Control: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn vary_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Vary: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn no_vary_search_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("No-Vary-Search: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn allow_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 405 Method Not Allowed\r\n");
  for value in values {
    response.push_str("Allow: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 0\r\n\r\n");
  response.into_bytes()
}

fn authentication_info_response(
  authentication_info: Option<&str>,
  proxy_authentication_info: Option<&str>,
) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  if let Some(value) = authentication_info {
    response.push_str("Authentication-Info: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if let Some(value) = proxy_authentication_info {
    response.push_str("Proxy-Authentication-Info: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn accept_ranges_response(values: &[&str], include_adjacent_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Accept-Ranges: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_adjacent_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Age: 5\r\n");
    response.push_str("Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
    response.push_str("Retry-After: 30\r\n");
    response.push_str("Allow: GET, HEAD\r\n");
    response.push_str("Content-Language: fr-CA, es-419\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn content_language_response(values: &[&str], include_adjacent_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Content-Language: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_adjacent_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Age: 5\r\n");
    response.push_str("Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
    response.push_str("Retry-After: 30\r\n");
    response.push_str("Allow: GET, HEAD\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn service_worker_allowed_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Service-Worker-Allowed: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn content_location_response(values: &[&str], include_adjacent_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Content-Location: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_adjacent_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Age: 5\r\n");
    response.push_str("Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
    response.push_str("Retry-After: 30\r\n");
    response.push_str("Allow: GET, HEAD\r\n");
    response.push_str("Content-Language: fr-CA, es-419\r\n");
    response.push_str("Accept-Ranges: bytes, pages\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn content_disposition_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Content-Disposition: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn content_type_response(values: &[&str], include_adjacent_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Content-Type: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_adjacent_metadata {
    response.push_str("Content-Encoding: gzip, br\r\n");
    response.push_str("Content-Language: fr-CA, es-419\r\n");
    response.push_str("Content-Location: /representations/current\r\n");
    response.push_str("Accept-Ranges: bytes, pages\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn content_encoding_response(values: &[&str], include_adjacent_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Content-Encoding: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_adjacent_metadata {
    response.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    response.push_str("Content-Language: fr-CA, es-419\r\n");
    response.push_str("Content-Location: /representations/current\r\n");
    response.push_str("Accept-Ranges: bytes, pages\r\n");
  }
  let body = content_encoding_body(values);
  response.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
  let mut response = response.into_bytes();
  response.extend_from_slice(&body);
  response
}

fn content_encoding_body(values: &[&str]) -> Vec<u8> {
  if values == ["gzip"] {
    return gzip_body();
  }
  b"OK".to_vec()
}

fn gzip_body() -> Vec<u8> {
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(b"OK").expect("write gzip fixture body");
  encoder.finish().expect("finish gzip fixture body")
}

fn allow_response_with_cache_metadata(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 405 Method Not Allowed\r\n");
  for value in values {
    response.push_str("Allow: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Cache-Control: public, max-age=60\r\n");
  response.push_str("Age: 5\r\n");
  response.push_str("Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n");
  response.push_str("Vary: Accept-Encoding\r\n");
  response.push_str("Retry-After: 30\r\n");
  response.push_str("Content-Length: 0\r\n\r\n");
  response.into_bytes()
}

fn age_expires_response(age: &str, expires: &str, include_cache_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  response.push_str("Age: ");
  response.push_str(age);
  response.push_str("\r\nExpires: ");
  response.push_str(expires);
  response.push_str("\r\n");
  if include_cache_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn retry_after_response(values: &[&str], include_cache_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 503 Service Unavailable\r\n");
  for value in values {
    response.push_str("Retry-After: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  if include_cache_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Age: 5\r\n");
    response.push_str("Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
  }
  response.push_str("Content-Length: 4\r\n\r\nbusy");
  response.into_bytes()
}

const RANGE_BODY: &[u8] = b"0123456789abcdef";
const CONDITIONAL_LAST_MODIFIED: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const CONDITIONAL_STALE_DATE: &str = "Sun, 06 Nov 1994 08:49:36 GMT";
const CONDITIONAL_FRESH_DATE: &str = "Sun, 06 Nov 1994 08:49:38 GMT";
const CONDITIONAL_BODY: &str = "cache representation";

const NO_BODY_STATUS_WITH_FRAMING_CASES: &[(&str, &[u8], u32, &str, &str)] = &[
  (
    "204 content-length",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    204,
    "Content-Length",
    "7",
  ),
  (
    "204 chunked",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    204,
    "Transfer-Encoding",
    "chunked",
  ),
  (
    "304 content-length",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    304,
    "Content-Length",
    "7",
  ),
  (
    "304 chunked",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    304,
    "Transfer-Encoding",
    "chunked",
  ),
];

fn range_response(request: Request) -> HttpResponse {
  match request.header("range") {
    Some(range_header) => match HttpByteRange::parse(range_header, RANGE_BODY.len()) {
      Ok(range) => HttpResponse::partial_content(RANGE_BODY, range),
      Err(HttpByteRangeError::UnsatisfiedRange) => {
        HttpResponse::range_not_satisfiable(RANGE_BODY.len())
      }
      Err(error) => HttpResponse::new(400, "Bad Request").body(error.to_string()),
    },
    None => HttpResponse::ok(RANGE_BODY),
  }
}

fn if_range_response(request: Request, metadata: HttpConditionalMetadata) -> HttpResponse {
  match request.evaluate_if_range(&metadata, RANGE_BODY.len()) {
    Ok(HttpIfRangeRequestOutcome::PartialContent(range)) => {
      HttpResponse::partial_content(RANGE_BODY, range)
        .header("ETag", r#""abc""#)
        .header("Last-Modified", CONDITIONAL_LAST_MODIFIED)
    }
    Ok(HttpIfRangeRequestOutcome::RangeNotSatisfiable) => {
      HttpResponse::range_not_satisfiable(RANGE_BODY.len())
    }
    Ok(HttpIfRangeRequestOutcome::FullResponse) => HttpResponse::ok(RANGE_BODY)
      .header("ETag", r#""abc""#)
      .header("Last-Modified", CONDITIONAL_LAST_MODIFIED),
    Err(error) => HttpResponse::new(400, "Bad Request").body(error.to_string()),
  }
}

fn spawn_range_server() -> (std::net::SocketAddr, thread::JoinHandle<Option<String>>) {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send observed range");
        range_response(request)
      })
      .expect("serve range request");
    rx.recv().expect("observed range")
  });

  (addr, handle)
}

fn spawn_if_range_server(
  metadata: HttpConditionalMetadata,
) -> (std::net::SocketAddr, ObservedIfRangeHandle) {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind if-range server");
  let addr = server.local_addr().expect("if-range server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = (
          request.header("range").map(str::to_string),
          request.header("if-range").map(str::to_string),
        );
        tx.send(observed).expect("send observed if-range headers");
        if_range_response(request, metadata.clone())
      })
      .expect("serve if-range request");
    rx.recv().expect("observed if-range headers")
  });

  (addr, handle)
}

fn conditional_metadata() -> HttpConditionalMetadata {
  HttpConditionalMetadata::new()
    .entity_tag(HttpEntityTag::strong("abc"))
    .last_modified(httpdate::parse_http_date(CONDITIONAL_LAST_MODIFIED).expect("metadata date"))
}

fn conditional_response(request: Request) -> HttpResponse {
  let metadata = conditional_metadata();
  match request.evaluate_conditional(&metadata) {
    HttpConditionalRequestOutcome::Proceed => HttpResponse::ok(CONDITIONAL_BODY)
      .header("ETag", r#""abc""#)
      .header("Last-Modified", CONDITIONAL_LAST_MODIFIED),
    HttpConditionalRequestOutcome::NotModified => HttpResponse::not_modified(&metadata),
    HttpConditionalRequestOutcome::PreconditionFailed => HttpResponse::precondition_failed(),
  }
}

fn spawn_conditional_server() -> (std::net::SocketAddr, thread::JoinHandle<Option<String>>) {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind conditional server");
  let addr = server.local_addr().expect("conditional server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed_validator = [
          "If-None-Match",
          "If-Match",
          "If-Modified-Since",
          "If-Unmodified-Since",
        ]
        .iter()
        .find_map(|name| request.header(name).map(|value| format!("{name}: {value}")));
        tx.send(observed_validator)
          .expect("send observed validator");
        conditional_response(request)
      })
      .expect("serve conditional request");
    rx.recv().expect("observed validator")
  });

  (addr, handle)
}

fn spawn_content_disposition_server(
  disposition: HttpContentDisposition,
) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind content-disposition server");
  let addr = server
    .local_addr()
    .expect("content-disposition server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .header("Content-Disposition", "inline")
          .with_content_disposition(disposition.clone())
          .expect("Content-Disposition declaration should parse")
      })
      .expect("serve content-disposition request");
  });

  (addr, handle)
}

fn spawn_content_type_server(
  content_type: HttpContentType,
) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind content-type server");
  let addr = server.local_addr().expect("content-type server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .header("Content-Type", "application/octet-stream")
          .with_content_type(content_type.clone())
          .expect("Content-Type declaration should parse")
      })
      .expect("serve content-type request");
  });

  (addr, handle)
}

fn spawn_content_encoding_server(
  codings: &'static [&'static str],
) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind content-encoding server");
  let addr = server.local_addr().expect("content-encoding server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok(content_encoding_body(codings))
          .header("Content-Encoding", "identity")
          .with_content_encoding(codings.iter().copied())
          .expect("Content-Encoding declaration should parse")
      })
      .expect("serve content-encoding request");
  });

  (addr, handle)
}

fn spawn_metadata_response_server(
  headers: &'static [(&'static str, &'static str)],
) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind metadata response server");
  let addr = server.local_addr().expect("metadata response server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        headers
          .iter()
          .fold(HttpResponse::ok("OK"), |response, (name, value)| {
            response.header(*name, *value)
          })
      })
      .expect("serve metadata response request");
  });

  (addr, handle)
}

#[test]
fn sync_client_and_server_exchange_access_control_allow_origin_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Access-Control-Allow-Origin server");
  let addr = server
    .local_addr()
    .expect("Access-Control-Allow-Origin server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_access_control_allow_origin("https://example.test:8443")
          .expect("Access-Control-Allow-Origin should be accepted")
      })
      .expect("serve Access-Control-Allow-Origin response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/access-control-allow-origin"))
    .emit()
    .expect("Access-Control-Allow-Origin response should parse");
  assert_eq!(
    "https://example.test:8443",
    response
      .access_control_allow_origin()
      .expect("Access-Control-Allow-Origin should parse")
      .expect("Access-Control-Allow-Origin should be present")
      .header_value()
  );
  assert_eq!(
    Some(&"https://example.test:8443".to_string()),
    response.header_value("Access-Control-Allow-Origin")
  );
  handle
    .join()
    .expect("Access-Control-Allow-Origin server thread");
}

#[test]
fn sync_client_and_server_exchange_access_control_allow_credentials_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Access-Control-Allow-Credentials server");
  let addr = server
    .local_addr()
    .expect("Access-Control-Allow-Credentials server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_access_control_allow_credentials("true")
          .expect("Access-Control-Allow-Credentials should be accepted")
      })
      .expect("serve Access-Control-Allow-Credentials response");
  });

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/access-control-allow-credentials"
    ))
    .emit()
    .expect("Access-Control-Allow-Credentials response should parse");
  assert_eq!(
    "true",
    response
      .access_control_allow_credentials()
      .expect("Access-Control-Allow-Credentials should parse")
      .expect("Access-Control-Allow-Credentials should be present")
      .header_value()
  );
  assert_eq!(
    Some(&"true".to_string()),
    response.header_value("Access-Control-Allow-Credentials")
  );
  handle
    .join()
    .expect("Access-Control-Allow-Credentials server thread");
}

#[test]
fn sync_client_and_server_exchange_cross_origin_resource_policy_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Cross-Origin-Resource-Policy server");
  let addr = server
    .local_addr()
    .expect("Cross-Origin-Resource-Policy server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_cross_origin_resource_policy("SAME-ORIGIN")
          .expect("Cross-Origin-Resource-Policy should be accepted")
      })
      .expect("serve Cross-Origin-Resource-Policy response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/cross-origin-resource-policy"))
    .emit()
    .expect("Cross-Origin-Resource-Policy response should parse");
  assert_eq!(
    "same-origin",
    response
      .cross_origin_resource_policy()
      .expect("Cross-Origin-Resource-Policy should parse")
      .expect("Cross-Origin-Resource-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some(&"same-origin".to_string()),
    response.header_value("Cross-Origin-Resource-Policy")
  );
  handle
    .join()
    .expect("Cross-Origin-Resource-Policy server thread");
}

#[test]
fn sync_client_and_server_exchange_content_security_policy_report_only_metadata_without_policy() {
  let (addr, handle) = spawn_metadata_response_server(&[
    ("Content-Security-Policy-Report-Only", "default-src 'self'"),
    (
      "content-security-policy-report-only",
      "object-src 'none'; report-to csp-endpoint",
    ),
  ]);

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/content-security-policy-report-only"
    ))
    .emit()
    .expect("Content-Security-Policy-Report-Only response should parse");

  let metadata = response
    .content_security_policy_report_only()
    .expect("Content-Security-Policy-Report-Only should parse")
    .expect("Content-Security-Policy-Report-Only should be present");
  assert_eq!(metadata.as_str(), "default-src 'self'");
  assert_eq!(
    metadata.header_values(),
    [
      "default-src 'self'",
      "object-src 'none'; report-to csp-endpoint"
    ]
  );
  assert_eq!(
    response
      .header_values("Content-Security-Policy-Report-Only")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>(),
    [
      "default-src 'self'",
      "object-src 'none'; report-to csp-endpoint"
    ]
  );

  handle
    .join()
    .expect("Content-Security-Policy-Report-Only server thread");

  let raw_response = b"HTTP/1.1 200 OK\r\nContent-Security-Policy-Report-Only: default-src 'self'\x7f\r\nContent-Length: 2\r\n\r\nOK";
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);
  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/content-security-policy-report-only-invalid"
    ))
    .emit()
    .expect("malformed Content-Security-Policy-Report-Only response should remain parseable");
  assert!(response.content_security_policy_report_only().is_err());
  assert_eq!(
    Some(&"default-src 'self'\u{7f}".to_string()),
    response.header_value("Content-Security-Policy-Report-Only")
  );
  handle
    .join()
    .expect("raw Content-Security-Policy-Report-Only server thread");
}

#[test]
fn sync_client_and_server_exchange_cross_origin_embedder_policy_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Cross-Origin-Embedder-Policy server");
  let addr = server
    .local_addr()
    .expect("Cross-Origin-Embedder-Policy server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_cross_origin_embedder_policy("require-corp; report-to=\"coep\"")
          .expect("Cross-Origin-Embedder-Policy should be accepted")
      })
      .expect("serve Cross-Origin-Embedder-Policy response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/cross-origin-embedder-policy"))
    .emit()
    .expect("Cross-Origin-Embedder-Policy response should parse");
  assert_eq!(
    "require-corp",
    response
      .cross_origin_embedder_policy()
      .expect("Cross-Origin-Embedder-Policy should parse")
      .expect("Cross-Origin-Embedder-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some(&"require-corp".to_string()),
    response.header_value("Cross-Origin-Embedder-Policy")
  );
  handle
    .join()
    .expect("Cross-Origin-Embedder-Policy server thread");
}

#[test]
fn sync_client_and_server_exchange_cross_origin_opener_policy_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Cross-Origin-Opener-Policy server");
  let addr = server
    .local_addr()
    .expect("Cross-Origin-Opener-Policy server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_cross_origin_opener_policy("noopener-allow-popups; report-to=\"coop\"")
          .expect("Cross-Origin-Opener-Policy should be accepted")
      })
      .expect("serve Cross-Origin-Opener-Policy response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/cross-origin-opener-policy"))
    .emit()
    .expect("Cross-Origin-Opener-Policy response should parse");
  assert_eq!(
    "noopener-allow-popups",
    response
      .cross_origin_opener_policy()
      .expect("Cross-Origin-Opener-Policy should parse")
      .expect("Cross-Origin-Opener-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some(&"noopener-allow-popups".to_string()),
    response.header_value("Cross-Origin-Opener-Policy")
  );
  handle
    .join()
    .expect("Cross-Origin-Opener-Policy server thread");
}

#[test]
fn sync_client_and_server_exchange_cross_origin_opener_policy_report_only_metadata_without_policy()
{
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Cross-Origin-Opener-Policy-Report-Only server");
  let addr = server
    .local_addr()
    .expect("Cross-Origin-Opener-Policy-Report-Only server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_cross_origin_opener_policy_report_only("same-origin; report-to=\"coop-reporting\"")
          .expect("Cross-Origin-Opener-Policy-Report-Only should be accepted")
      })
      .expect("serve Cross-Origin-Opener-Policy-Report-Only response");
  });

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/cross-origin-opener-policy-report-only"
    ))
    .emit()
    .expect("Cross-Origin-Opener-Policy-Report-Only response should parse");
  let policy = response
    .cross_origin_opener_policy_report_only()
    .expect("Cross-Origin-Opener-Policy-Report-Only should parse")
    .expect("Cross-Origin-Opener-Policy-Report-Only should be present");
  assert_eq!(
    rttp_client::response::CrossOriginOpenerPolicy::SameOrigin,
    policy.policy()
  );
  assert_eq!(Some("coop-reporting"), policy.report_to());
  assert_eq!(
    r#"same-origin; report-to="coop-reporting""#,
    policy.header_value()
  );
  assert_eq!(
    Some(&r#"same-origin; report-to="coop-reporting""#.to_string()),
    response.header_value("Cross-Origin-Opener-Policy-Report-Only")
  );
  handle
    .join()
    .expect("Cross-Origin-Opener-Policy-Report-Only server thread");
}

#[test]
fn sync_client_and_server_exchange_alt_svc_metadata_without_connection_policy() {
  const HEADERS: &[(&str, &str)] = &[(
    "Alt-Svc",
    "h3=\":443\"; ma=3600; persist=1; region=\"us-east\"",
  )];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/alt-svc"))
    .emit()
    .expect("Alt-Svc response should parse without connection policy");
  let alt_svc = response
    .alt_svc()
    .expect("Alt-Svc should parse")
    .expect("Alt-Svc should be present");
  let alternative = &alt_svc.alternatives()[0];

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(
    Some(&"h3=\":443\"; ma=3600; persist=1; region=\"us-east\"".to_string()),
    response.header_value("Alt-Svc")
  );
  assert!(!alt_svc.is_clear());
  assert_eq!(1, alt_svc.len());
  assert_eq!("h3", alternative.protocol_id());
  assert_eq!(":443", alternative.authority());
  assert_eq!(Some(3600), alternative.max_age());
  assert_eq!(Some(true), alternative.persist());
  assert_eq!(
    vec![("region", Some("us-east"))],
    alternative
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );

  handle.join().expect("Alt-Svc server thread");
}

#[test]
fn sync_client_and_server_exchange_reporting_endpoints_metadata_without_scheduling_reports() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind Reporting-Endpoints server");
  let addr = server
    .local_addr()
    .expect("Reporting-Endpoints server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_reporting_endpoints([
            ("default", r#"https://reports.example/a"b\c"#),
            ("csp", "https://reports.example/csp"),
          ])
          .expect("Reporting-Endpoints should be accepted")
      })
      .expect("serve Reporting-Endpoints response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/reporting-endpoints"))
    .emit()
    .expect("Reporting-Endpoints response should parse without scheduling reports");
  let endpoints = response
    .reporting_endpoints()
    .expect("Reporting-Endpoints should parse")
    .expect("Reporting-Endpoints should be present");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(
    vec![
      ("default", r#"https://reports.example/a"b\c"#),
      ("csp", "https://reports.example/csp"),
    ],
    endpoints.endpoints()
  );
  assert_eq!(
    Some(
      &r#"default="https://reports.example/a\"b\\c", csp="https://reports.example/csp""#
        .to_string()
    ),
    response.header_value("Reporting-Endpoints")
  );
  handle.join().expect("Reporting-Endpoints server thread");
}

#[test]
fn sync_client_preserves_malformed_and_duplicate_reporting_endpoints_without_scheduling_reports() {
  const HEADERS: &[(&str, &str)] = &[
    (
      "Reporting-Endpoints",
      r#"default="https://reports.example/default""#,
    ),
    (
      "Reporting-Endpoints",
      r#"default="https://reports.example/other""#,
    ),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/reporting-endpoints-duplicate"
    ))
    .emit()
    .expect("duplicate Reporting-Endpoints should not prevent response parsing");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(
    vec![
      r#"default="https://reports.example/default""#,
      r#"default="https://reports.example/other""#,
    ],
    response
      .header_values("Reporting-Endpoints")
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
  );
  assert!(
    response.reporting_endpoints().is_err(),
    "duplicate endpoint names must produce the typed parse error"
  );

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_preserves_malformed_reporting_endpoints_without_scheduling_reports() {
  const HEADERS: &[(&str, &str)] = &[(
    "Reporting-Endpoints",
    "default=https://reports.example/default",
  )];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/reporting-endpoints-malformed"
    ))
    .emit()
    .expect("malformed Reporting-Endpoints should not prevent response parsing");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(
    Some(&"default=https://reports.example/default".to_string()),
    response.header_value("Reporting-Endpoints")
  );
  assert!(
    response.reporting_endpoints().is_err(),
    "unquoted Reporting-Endpoints URLs must produce the typed parse error"
  );

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_and_server_exchange_nel_metadata_without_report_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind NEL server");
  let addr = server.local_addr().expect("NEL server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_nel(
            r#"{"report_to":"network-errors","max_age":2592000,"include_subdomains":true,"success_fraction":0.1,"failure_fraction":1.0}"#,
          )
          .expect("NEL policy should be accepted")
      })
      .expect("serve NEL response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/nel"))
    .emit()
    .expect("NEL response should parse without report policy");
  let nel = response
    .nel()
    .expect("NEL should parse")
    .expect("NEL should be present");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(2592000, nel.max_age());
  assert_eq!(Some("network-errors"), nel.report_to());
  assert_eq!(Some(true), nel.include_subdomains());
  assert_eq!(Some(0.1), nel.success_fraction());
  assert_eq!(Some(1.0), nel.failure_fraction());
  assert_eq!(
    Some(
      &"{\"max_age\":2592000,\"report_to\":\"network-errors\",\"include_subdomains\":true,\"success_fraction\":0.1,\"failure_fraction\":1}".to_string()
    ),
    response.header_value("NEL")
  );
  handle.join().expect("NEL server thread");
}

#[test]
fn facade_client_and_server_exchange_strict_transport_security_metadata_without_policy() {
  let server =
    rttp::Http::server("127.0.0.1:0").expect("bind Strict-Transport-Security facade server");
  let addr = server
    .local_addr()
    .expect("Strict-Transport-Security facade addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        rttp::server::HttpResponse::ok("OK")
          .with_strict_transport_security("max-age=31536000; includeSubDomains; preload")
          .expect("Strict-Transport-Security should be accepted")
      })
      .expect("serve Strict-Transport-Security facade response");
  });

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/matrix/strict-transport-security"))
    .emit()
    .expect("Strict-Transport-Security facade response should parse");
  let strict_transport_security = response
    .strict_transport_security()
    .expect("Strict-Transport-Security should parse")
    .expect("Strict-Transport-Security should be present");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    Some(&"max-age=31536000; includeSubDomains; preload".to_string()),
    response.header_value("Strict-Transport-Security")
  );
  assert_eq!(31_536_000, strict_transport_security.max_age());
  assert!(strict_transport_security.include_sub_domains());
  assert!(strict_transport_security.preload());
  assert_eq!(
    "max-age=31536000; includeSubDomains; preload",
    strict_transport_security.header_value()
  );

  handle
    .join()
    .expect("Strict-Transport-Security facade server thread");
}

#[test]
fn sync_client_preserves_duplicate_cross_origin_resource_policy_fields_without_policy() {
  const HEADERS: &[(&str, &str)] = &[
    ("Cross-Origin-Resource-Policy", "same-origin"),
    ("cross-origin-resource-policy", "same-site"),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!(
      "http://{}/matrix/cross-origin-resource-policy-duplicate",
      addr
    ))
    .emit()
    .expect("duplicate Cross-Origin-Resource-Policy fields should not prevent response parsing");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  assert_eq!(
    Some(&"same-origin".to_string()),
    response.header_value("Cross-Origin-Resource-Policy")
  );
  assert_eq!(
    vec!["same-origin", "same-site"],
    response
      .header_values("Cross-Origin-Resource-Policy")
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
  );
  assert!(
    response.cross_origin_resource_policy().is_err(),
    "duplicate singleton fields must produce the typed parse error"
  );

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_sec_fetch_metadata_is_observed_by_server_helpers() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind Sec-Fetch metadata server");
  let addr = server.local_addr().expect("Sec-Fetch metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send((
            request.sec_fetch_site(),
            request.sec_fetch_mode(),
            request.sec_fetch_dest(),
            request.sec_fetch_user(),
            request.sec_purpose(),
          ))
          .expect("send observed Sec-Fetch metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve Sec-Fetch metadata request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/sec-fetch-metadata"))
    .sec_fetch_site(SecFetchSite::SameOrigin)
    .sec_fetch_mode(SecFetchMode::Navigate)
    .sec_fetch_dest(SecFetchDest::Document)
    .sec_fetch_user()
    .sec_purpose(
      &SecPurpose::from_tokens(["prefetch", "vendor-ext"]).expect("valid Sec-Purpose should parse"),
    )
    .emit()
    .expect("Sec-Fetch metadata request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  let (site, mode, dest, user, purpose) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe Sec-Fetch metadata");
  assert_eq!(
    Some(SecFetchSite::SameOrigin),
    site.expect("Sec-Fetch-Site should parse")
  );
  assert_eq!(
    Some(SecFetchMode::Navigate),
    mode.expect("Sec-Fetch-Mode should parse")
  );
  assert_eq!(
    Some(SecFetchDest::Document),
    dest.expect("Sec-Fetch-Dest should parse")
  );
  assert!(user.expect("Sec-Fetch-User should parse").is_some());
  let purpose = purpose
    .expect("Sec-Purpose should parse")
    .expect("Sec-Purpose should be present");
  assert_eq!(purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(purpose.contains_prefetch());

  handle.join().expect("Sec-Fetch metadata server thread");
}

#[derive(Debug, PartialEq)]
struct ObservedUpgradeInsecureRequests {
  target: String,
  raw: Option<String>,
  parsed: Result<Option<String>, String>,
}

fn observe_upgrade_insecure_requests(request: &Request) -> ObservedUpgradeInsecureRequests {
  ObservedUpgradeInsecureRequests {
    target: request.target().to_string(),
    raw: request
      .header("Upgrade-Insecure-Requests")
      .map(str::to_string),
    parsed: request
      .upgrade_insecure_requests()
      .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
      .map_err(|error| error.to_string()),
  }
}

fn spawn_upgrade_insecure_requests_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedUpgradeInsecureRequests>,
  thread::JoinHandle<()>,
) {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Upgrade-Insecure-Requests metadata server");
  let addr = server
    .local_addr()
    .expect("Upgrade-Insecure-Requests metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_upgrade_insecure_requests(&request))
          .expect("send observed Upgrade-Insecure-Requests metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve Upgrade-Insecure-Requests request");
  });

  (addr, observed_rx, handle)
}

#[test]
fn sync_client_upgrade_insecure_requests_is_observed_by_server_helpers() {
  let (addr, observed_rx, handle) = spawn_upgrade_insecure_requests_observer();

  let response = client()
    .get()
    .url(format!("http://{addr}/page"))
    .upgrade_insecure_requests()
    .expect("Upgrade-Insecure-Requests should be accepted")
    .emit()
    .expect("Upgrade-Insecure-Requests request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedUpgradeInsecureRequests {
      target: "/page".to_string(),
      raw: Some("1".to_string()),
      parsed: Ok(Some("1".to_string())),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe Upgrade-Insecure-Requests metadata")
  );
  handle
    .join()
    .expect("Upgrade-Insecure-Requests server thread");
}

#[test]
fn facade_server_reports_absent_upgrade_insecure_requests_without_policy() {
  let (addr, observed_rx, handle) = spawn_upgrade_insecure_requests_observer();

  let response = client()
    .get()
    .url(format!("http://{addr}/page"))
    .emit()
    .expect("request without Upgrade-Insecure-Requests should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedUpgradeInsecureRequests {
      target: "/page".to_string(),
      raw: None,
      parsed: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent Upgrade-Insecure-Requests")
  );
  handle
    .join()
    .expect("absent Upgrade-Insecure-Requests server thread");
}

#[test]
fn facade_server_rejects_malformed_upgrade_insecure_requests_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_upgrade_insecure_requests_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed Upgrade-Insecure-Requests");
  stream
    .write_all(
      b"GET /page HTTP/1.1\r\nHost: example.test\r\nUpgrade-Insecure-Requests: 0\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Upgrade-Insecure-Requests request");

  assert_eq!(
    ObservedUpgradeInsecureRequests {
      target: "/page".to_string(),
      raw: Some("0".to_string()),
      parsed: Err("invalid Upgrade-Insecure-Requests header value".to_string()),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe malformed Upgrade-Insecure-Requests")
  );
  handle
    .join()
    .expect("malformed Upgrade-Insecure-Requests server thread");
}

#[test]
fn facade_server_rejects_duplicate_upgrade_insecure_requests_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_upgrade_insecure_requests_observer();

  let mut stream = TcpStream::connect(addr).expect("connect duplicate Upgrade-Insecure-Requests");
  stream
    .write_all(
      b"GET /page HTTP/1.1\r\nHost: example.test\r\nUpgrade-Insecure-Requests: 1\r\nupgrade-insecure-requests: 1\r\nConnection: close\r\n\r\n",
    )
    .expect("write duplicate Upgrade-Insecure-Requests request");

  assert_eq!(
    ObservedUpgradeInsecureRequests {
      target: "/page".to_string(),
      raw: Some("1".to_string()),
      parsed: Err("duplicate Upgrade-Insecure-Requests header fields".to_string()),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe duplicate Upgrade-Insecure-Requests")
  );
  handle
    .join()
    .expect("duplicate Upgrade-Insecure-Requests server thread");
}

#[test]
fn facade_server_rejects_oversized_upgrade_insecure_requests_request_head() {
  // A 64 KiB + 1 field value plus the request line exceeds the shared HTTP/1.1
  // request-head bound, so the request is rejected as 400 before handler
  // dispatch. Oversized accessor parsing without losing raw access is covered
  // by protocol and server unit tests plus the raised-limit h2c facade test.
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind oversized Upgrade-Insecure-Requests server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server
    .local_addr()
    .expect("oversized Upgrade-Insecure-Requests server addr");
  let (observed_tx, observed_rx) = mpsc::channel::<()>();
  let handle = thread::spawn(move || {
    server
      .accept_one(|_request| {
        observed_tx
          .send(())
          .expect("handler must not observe oversized request head");
        HttpResponse::ok("unreachable")
      })
      .expect("oversized request head should be answered as 400");
  });

  let oversized = "1".repeat(64 * 1024 + 1);
  let request = format!(
    "GET /page HTTP/1.1\r\nHost: example.test\r\nUpgrade-Insecure-Requests: {oversized}\r\nConnection: close\r\n\r\n"
  );
  let mut stream = TcpStream::connect(addr).expect("connect oversized Upgrade-Insecure-Requests");
  stream
    .write_all(request.as_bytes())
    .expect("write oversized Upgrade-Insecure-Requests request");
  let mut response = Vec::new();
  stream
    .read_to_end(&mut response)
    .expect("read oversized request-head response");
  let response = String::from_utf8(response).expect("response should be utf-8");

  assert!(
    response.starts_with("HTTP/1.1 400 "),
    "oversized request head should be rejected before handler dispatch: {response}"
  );
  assert!(
    observed_rx.try_recv().is_err(),
    "oversized Upgrade-Insecure-Requests must not reach the handler"
  );
  handle
    .join()
    .expect("oversized Upgrade-Insecure-Requests server thread");
}

#[test]
fn facade_client_and_server_exchange_valid_cors_preflight_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_cors_preflight_observer();

  let response = rttp::Http::client()
    .options()
    .url(format!("http://{addr}/matrix/cors-preflight"))
    .origin("https://spa.example.test")
    .expect("Origin should be accepted")
    .access_control_request_method("patch")
    .expect("Access-Control-Request-Method should be accepted")
    .access_control_request_headers(["X-Request-Id", "Content-Type"])
    .expect("Access-Control-Request-Headers should be accepted")
    .access_control_request_private_network()
    .expect("Access-Control-Request-Private-Network should be accepted")
    .emit()
    .expect("CORS preflight request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(None, response.header_value("Access-Control-Allow-Origin"));
  assert_eq!(
    ObservedCorsPreflight {
      method: "OPTIONS".to_string(),
      origin: Some("https://spa.example.test".to_string()),
      raw_request_method: Some("PATCH".to_string()),
      raw_request_headers: Some("x-request-id, content-type".to_string()),
      raw_request_private_network: Some("true".to_string()),
      request_method: Ok(Some("PATCH".to_string())),
      request_headers: Ok(Some(vec![
        "x-request-id".to_string(),
        "content-type".to_string(),
      ])),
      request_private_network: Ok(Some("true".to_string())),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid CORS preflight metadata")
  );
  handle
    .join()
    .expect("valid CORS preflight facade server thread");
}

#[test]
fn facade_server_reports_absent_cors_preflight_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_cors_preflight_observer();

  let response = rttp::Http::client()
    .options()
    .url(format!("http://{addr}/matrix/cors-preflight-absent"))
    .emit()
    .expect("OPTIONS request without preflight metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedCorsPreflight {
      method: "OPTIONS".to_string(),
      origin: None,
      raw_request_method: None,
      raw_request_headers: None,
      raw_request_private_network: None,
      request_method: Ok(None),
      request_headers: Ok(None),
      request_private_network: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent CORS preflight metadata")
  );
  handle
    .join()
    .expect("absent CORS preflight facade server thread");
}

#[test]
fn facade_server_rejects_malformed_cors_preflight_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_cors_preflight_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed CORS preflight request");
  stream
    .write_all(
      b"OPTIONS /matrix/cors-preflight-malformed HTTP/1.1\r\nHost: example.test\r\nOrigin: https://spa.example.test\r\nAccess-Control-Request-Method: GET, POST\r\nAccess-Control-Request-Headers: X Bad\r\nAccess-Control-Request-Private-Network: false\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed CORS preflight request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed CORS preflight metadata");
  assert_eq!("OPTIONS", observed.method);
  assert_eq!(
    Some("https://spa.example.test".to_string()),
    observed.origin
  );
  assert_eq!(Some("GET, POST".to_string()), observed.raw_request_method);
  assert_eq!(Some("X Bad".to_string()), observed.raw_request_headers);
  assert_eq!(
    Some("false".to_string()),
    observed.raw_request_private_network
  );
  assert!(observed.request_method.is_err());
  assert!(observed.request_headers.is_err());
  assert!(observed.request_private_network.is_err());

  handle
    .join()
    .expect("malformed CORS preflight facade server thread");
}

#[test]
fn facade_server_combines_multi_header_cors_preflight_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_cors_preflight_observer();

  let mut stream = TcpStream::connect(addr).expect("connect multi-header CORS preflight request");
  stream
    .write_all(
      b"OPTIONS /matrix/cors-preflight-multi-header HTTP/1.1\r\nHost: example.test\r\nOrigin: https://spa.example.test\r\nAccess-Control-Request-Method: patch\r\nAccess-Control-Request-Headers: X-Request-Id\r\naccess-control-request-headers: Content-Type\r\nAccess-Control-Request-Private-Network: true\r\naccess-control-request-private-network: true\r\nConnection: close\r\n\r\n",
    )
    .expect("write multi-header CORS preflight request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe multi-header CORS preflight metadata");
  assert_eq!("OPTIONS", observed.method);
  assert_eq!(
    Some("https://spa.example.test".to_string()),
    observed.origin
  );
  assert_eq!(Some("patch".to_string()), observed.raw_request_method);
  assert_eq!(
    Some("X-Request-Id".to_string()),
    observed.raw_request_headers
  );
  assert_eq!(
    Some("true".to_string()),
    observed.raw_request_private_network
  );
  assert_eq!(Ok(Some("PATCH".to_string())), observed.request_method);
  assert_eq!(
    Ok(Some(vec![
      "x-request-id".to_string(),
      "content-type".to_string(),
    ])),
    observed.request_headers
  );
  assert!(observed.request_private_network.is_err());

  handle
    .join()
    .expect("multi-header CORS preflight facade server thread");
}

#[test]
fn facade_client_and_server_exchange_valid_signature_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{addr}/signed"))
    .signature_input(r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#)
    .expect("Signature-Input should be accepted")
    .signature("sig1=:YWJj:")
    .expect("Signature should be accepted")
    .emit()
    .expect("signed request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    Some("sig1=:YWJj:"),
    response
      .signature()
      .expect("client Signature should parse")
      .map(|signature| signature.header_value())
      .as_deref()
  );
  assert_eq!(
    Some(r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#),
    response
      .signature_input()
      .expect("client Signature-Input should parse")
      .map(|signature_input| signature_input.header_value())
      .as_deref()
  );
  assert_eq!(
    ObservedSignatureMetadata {
      raw_signature: Some("sig1=:YWJj:".to_string()),
      raw_signature_input: Some(
        r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#.to_string()
      ),
      signature: Ok(Some("sig1=:YWJj:".to_string())),
      signature_input: Ok(Some(
        r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#.to_string()
      )),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid signature metadata")
  );
  handle.join().expect("valid signature facade server thread");
}

#[test]
fn facade_server_reports_absent_signature_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{addr}/signed-absent"))
    .emit()
    .expect("request without signature metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    None,
    response
      .signature()
      .expect("absent client Signature should parse")
  );
  assert_eq!(
    None,
    response
      .signature_input()
      .expect("absent client Signature-Input should parse")
  );
  assert_eq!(
    ObservedSignatureMetadata {
      raw_signature: None,
      raw_signature_input: None,
      signature: Ok(None),
      signature_input: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent signature metadata")
  );
  handle
    .join()
    .expect("absent signature facade server thread");
}

#[test]
fn facade_client_and_server_exchange_accept_charset_request_metadata() {
  let (addr, observed_rx, handle) = spawn_facade_accept_charset_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/localized"))
    .accept_charset("utf-8")
    .expect("utf-8 should be accepted")
    .accept_charset_with_q("iso-8859-1", "0.5")
    .expect("iso-8859-1 quality should be accepted")
    .accept_charset_with_q("*", "0")
    .expect("wildcard quality should be accepted")
    .emit()
    .expect("Accept-Charset request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptCharsetMetadata {
      ranges: Ok(Some(vec![
        ("utf-8".to_owned(), 1000),
        ("iso-8859-1".to_owned(), 500),
        ("*".to_owned(), 0),
      ])),
      raw: Some("utf-8, iso-8859-1;q=0.5, *;q=0".to_owned()),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid Accept-Charset metadata")
  );
  handle
    .join()
    .expect("valid Accept-Charset facade server thread");
}

#[test]
fn facade_server_rejects_malformed_accept_charset_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_accept_charset_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed Accept-Charset request");
  stream
    .write_all(
      b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Charset: utf-8, UTF-8\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Accept-Charset request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed Accept-Charset metadata");
  assert!(observed.ranges.is_err());
  assert_eq!(observed.raw.as_deref(), Some("utf-8, UTF-8"));

  handle
    .join()
    .expect("malformed Accept-Charset facade server thread");
}

#[test]
fn facade_server_reports_absent_accept_charset_metadata() {
  let (addr, observed_rx, handle) = spawn_facade_accept_charset_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/plain"))
    .emit()
    .expect("request without Accept-Charset metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptCharsetMetadata {
      ranges: Ok(None),
      raw: None,
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent Accept-Charset metadata")
  );
  handle
    .join()
    .expect("absent Accept-Charset facade server thread");
}

#[test]
fn facade_client_and_server_exchange_accept_language_request_metadata() {
  let (addr, observed_rx, handle) = spawn_facade_accept_language_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/localized"))
    .accept_language(["en-US", "fr-CA; q=0.8", "*"])
    .expect("language ranges should be accepted")
    .emit()
    .expect("Accept-Language request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptLanguageMetadata {
      ranges: Ok(Some(vec![
        "en-US".to_owned(),
        "fr-CA".to_owned(),
        "*".to_owned()
      ])),
      qualities: Ok(Some(vec![None, Some("0.8".to_owned()), None])),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid Accept-Language metadata")
  );
  handle
    .join()
    .expect("valid Accept-Language facade server thread");
}

#[test]
fn facade_server_rejects_malformed_accept_language_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_accept_language_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed Accept-Language request");
  stream
    .write_all(
      b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Language: en; q=1.001, EN\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Accept-Language request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed Accept-Language metadata");
  assert!(observed.ranges.is_err());
  assert!(observed.qualities.is_err());

  handle
    .join()
    .expect("malformed Accept-Language facade server thread");
}

#[test]
fn facade_server_reports_absent_accept_language_metadata() {
  let (addr, observed_rx, handle) = spawn_facade_accept_language_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/plain"))
    .emit()
    .expect("request without Accept-Language metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptLanguageMetadata {
      ranges: Ok(None),
      qualities: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent Accept-Language metadata")
  );
  handle
    .join()
    .expect("absent Accept-Language facade server thread");
}

#[test]
fn facade_server_rejects_malformed_signature_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed signature request");
  stream
    .write_all(
      b"POST /signed-malformed HTTP/1.1\r\nHost: 127.0.0.1\r\nSignature: not-a-signature\r\nSignature-Input: not-an-input\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed signature request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed signature metadata");
  assert_eq!(Some("not-a-signature".to_string()), observed.raw_signature);
  assert_eq!(
    Some("not-an-input".to_string()),
    observed.raw_signature_input
  );
  assert!(observed.signature.is_err());
  assert!(observed.signature_input.is_err());

  handle
    .join()
    .expect("malformed signature facade server thread");
}

#[test]
fn facade_server_combines_multi_header_signature_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let mut stream = TcpStream::connect(addr).expect("connect multi-header signature request");
  stream
    .write_all(
      b"POST /signed-multi-header HTTP/1.1\r\nHost: 127.0.0.1\r\nSignature: sig1=:YWJj:\r\nsignature: sig-b24=:ZGVm:\r\nSignature-Input: sig1=(\"@method\")\r\nsignature-input: sig-b24=(\"@status\")\r\nConnection: close\r\n\r\n",
    )
    .expect("write multi-header signature request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe multi-header signature metadata");
  assert_eq!(Some("sig1=:YWJj:".to_string()), observed.raw_signature);
  assert_eq!(
    Some(r#"sig1=("@method")"#.to_string()),
    observed.raw_signature_input
  );
  assert_eq!(
    Ok(Some("sig1=:YWJj:, sig-b24=:ZGVm:".to_string())),
    observed.signature
  );
  assert_eq!(
    Ok(Some(r#"sig1=("@method"), sig-b24=("@status")"#.to_string())),
    observed.signature_input
  );

  handle
    .join()
    .expect("multi-header signature facade server thread");
}

#[test]
fn facade_server_parses_signature_fields_independently_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let mut stream = TcpStream::connect(addr).expect("connect independent signature request");
  stream
    .write_all(
      b"POST /signed-independent HTTP/1.1\r\nHost: 127.0.0.1\r\nSignature-Input: sig1=(\"@method\" \"@authority\" \"@path\");created=1618884473;keyid=\"test-key\"\r\nSignature: not-a-signature\r\nConnection: close\r\n\r\n",
    )
    .expect("write independent signature request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe independent signature metadata");
  assert_eq!(Some("not-a-signature".to_string()), observed.raw_signature);
  assert_eq!(
    Some(
      r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#.to_string()
    ),
    observed.raw_signature_input
  );
  assert!(observed.signature.is_err());
  assert_eq!(
    Ok(Some(
      r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#.to_string()
    )),
    observed.signature_input
  );

  handle
    .join()
    .expect("independent signature facade server thread");
}

#[test]
fn facade_server_parses_signature_input_independently_when_signature_is_valid() {
  let (addr, observed_rx, handle) = spawn_facade_signature_observer();

  let mut stream = TcpStream::connect(addr).expect("connect reverse independent signature request");
  stream
    .write_all(
      b"POST /signed-independent-reverse HTTP/1.1\r\nHost: 127.0.0.1\r\nSignature: sig1=:YWJj:\r\nSignature-Input: not-an-input\r\nConnection: close\r\n\r\n",
    )
    .expect("write reverse independent signature request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe reverse independent signature metadata");
  assert_eq!(Some("sig1=:YWJj:".to_string()), observed.raw_signature);
  assert_eq!(
    Some("not-an-input".to_string()),
    observed.raw_signature_input
  );
  assert_eq!(Ok(Some("sig1=:YWJj:".to_string())), observed.signature);
  assert!(observed.signature_input.is_err());

  handle
    .join()
    .expect("reverse independent signature facade server thread");
}

type ObservedTeCodings = Result<Option<Vec<(String, Option<u16>)>>, String>;

#[derive(Debug, PartialEq)]
struct ObservedTeMetadata {
  raw_te: Option<String>,
  raw_connection: Option<String>,
  connection_tokens: Result<Option<Vec<String>>, String>,
  te: ObservedTeCodings,
}

fn observe_te_metadata(request: &Request) -> ObservedTeMetadata {
  ObservedTeMetadata {
    raw_te: request.header("TE").map(str::to_string),
    raw_connection: request.header("Connection").map(str::to_string),
    connection_tokens: request
      .connection()
      .map(|connection| {
        connection.map(|connection| {
          connection
            .tokens()
            .into_iter()
            .map(str::to_string)
            .collect()
        })
      })
      .map_err(|error| error.to_string()),
    te: request
      .te()
      .map(|te| {
        te.map(|te| {
          te.codings()
            .iter()
            .map(|coding| (coding.coding().to_string(), coding.quality()))
            .collect()
        })
      })
      .map_err(|error| error.to_string()),
  }
}

fn spawn_facade_te_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedTeMetadata>,
  thread::JoinHandle<()>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind TE facade server");
  let addr = server.local_addr().expect("TE facade addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_te_metadata(&request))
          .expect("send observed TE metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve TE facade request");
  });

  (addr, observed_rx, handle)
}

#[test]
fn sync_client_and_server_exchange_te_metadata_with_connection_te_preserved() {
  let (addr, observed_rx, handle) = spawn_facade_te_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset"))
    .te("gzip")
    .expect("transfer coding should be accepted")
    .te_with_q("deflate", "0.5")
    .expect("transfer coding quality should be accepted")
    .te_trailers()
    .expect("trailers should be accepted")
    .emit()
    .expect("TE request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedTeMetadata {
      raw_te: Some("gzip, deflate;q=0.5, trailers".to_string()),
      raw_connection: Some("Close, TE".to_string()),
      connection_tokens: Ok(Some(vec!["Close".to_string(), "TE".to_string()])),
      te: Ok(Some(vec![
        ("gzip".to_string(), Some(1000)),
        ("deflate".to_string(), Some(500)),
        ("trailers".to_string(), None),
      ])),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid TE metadata")
  );
  handle.join().expect("valid TE facade server thread");
}

#[test]
fn facade_server_reports_absent_te_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_te_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset-absent"))
    .emit()
    .expect("request without TE metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedTeMetadata {
      raw_te: None,
      raw_connection: Some("Close".to_string()),
      connection_tokens: Ok(Some(vec!["Close".to_string()])),
      te: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent TE metadata")
  );
  handle.join().expect("absent TE facade server thread");
}

#[test]
fn facade_server_rejects_malformed_te_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_te_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed TE request");
  stream
    .write_all(
      b"GET /asset-malformed HTTP/1.1\r\nHost: 127.0.0.1\r\nTE: gzip;q=1.1\r\nConnection: close, TE\r\n\r\n",
    )
    .expect("write malformed TE request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed TE metadata");
  assert_eq!(Some("gzip;q=1.1".to_string()), observed.raw_te);
  assert_eq!(Some("close, TE".to_string()), observed.raw_connection);
  assert_eq!(
    Ok(Some(vec!["close".to_string(), "TE".to_string()])),
    observed.connection_tokens
  );
  assert!(observed.te.is_err());

  handle.join().expect("malformed TE facade server thread");
}

#[test]
fn facade_client_and_server_exchange_valid_digest_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/digest"))
    .want_content_digest("sha-256")
    .expect("Want-Content-Digest algorithm should be accepted")
    .want_content_digest_with_q("sha-512", "8")
    .expect("Want-Content-Digest preference should be accepted")
    .want_repr_digest("sha-256")
    .expect("Want-Repr-Digest algorithm should be accepted")
    .want_repr_digest_with_q("sha-512", "0")
    .expect("Want-Repr-Digest preference should be accepted")
    .emit()
    .expect("digest preference request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  let content_digest = response
    .content_digest()
    .expect("client Content-Digest should parse")
    .expect("client Content-Digest should be present");
  assert_eq!(
    Some(&b"abc"[..]),
    content_digest.entry("sha-256").map(|entry| entry.value())
  );
  assert_eq!(
    Some(&b"def"[..]),
    content_digest.entry("sha-512").map(|entry| entry.value())
  );
  assert_eq!(
    "sha-256=:YWJj:, sha-512=:ZGVm:",
    content_digest.header_value()
  );
  let repr_digest = response
    .repr_digest()
    .expect("client Repr-Digest should parse")
    .expect("client Repr-Digest should be present");
  assert_eq!(
    Some(&b"ghi"[..]),
    repr_digest.entry("sha-256").map(|entry| entry.value())
  );
  assert_eq!("sha-256=:Z2hp:", repr_digest.header_value());
  assert_eq!(
    ObservedDigestMetadata {
      raw_want_content_digest: Some("sha-256=10, sha-512=8".to_string()),
      raw_want_repr_digest: Some("sha-256=10, sha-512=0".to_string()),
      want_content_digest: Ok(Some("sha-256=10, sha-512=8".to_string())),
      want_repr_digest: Ok(Some("sha-256=10, sha-512=0".to_string())),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe valid digest metadata")
  );
  handle.join().expect("valid digest facade server thread");
}

#[test]
fn facade_server_reports_absent_digest_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/digest-absent"))
    .emit()
    .expect("request without digest metadata should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    None,
    response
      .content_digest()
      .expect("absent client Content-Digest should parse")
  );
  assert_eq!(
    None,
    response
      .repr_digest()
      .expect("absent client Repr-Digest should parse")
  );
  assert_eq!(
    ObservedDigestMetadata {
      raw_want_content_digest: None,
      raw_want_repr_digest: None,
      want_content_digest: Ok(None),
      want_repr_digest: Ok(None),
    },
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe absent digest metadata")
  );
  handle.join().expect("absent digest facade server thread");
}

#[test]
fn facade_server_rejects_malformed_want_digest_metadata_without_losing_raw_headers() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let mut stream = TcpStream::connect(addr).expect("connect malformed digest request");
  stream
    .write_all(
      b"GET /digest-malformed HTTP/1.1\r\nHost: 127.0.0.1\r\nWant-Content-Digest: sha-256\r\nWant-Repr-Digest: sha-256=11\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed digest request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe malformed digest metadata");
  assert_eq!(
    Some("sha-256".to_string()),
    observed.raw_want_content_digest
  );
  assert_eq!(
    Some("sha-256=11".to_string()),
    observed.raw_want_repr_digest
  );
  assert!(observed.want_content_digest.is_err());
  assert!(observed.want_repr_digest.is_err());

  handle
    .join()
    .expect("malformed digest facade server thread");
}

#[test]
fn facade_server_combines_multi_header_want_digest_metadata_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let mut stream = TcpStream::connect(addr).expect("connect multi-header digest request");
  stream
    .write_all(
      b"GET /digest-multi-header HTTP/1.1\r\nHost: 127.0.0.1\r\nWant-Content-Digest: sha-256=10\r\nwant-content-digest: sha-512=8\r\nWant-Repr-Digest: sha-256=10\r\nwant-repr-digest: sha-512=0\r\nConnection: close\r\n\r\n",
    )
    .expect("write multi-header digest request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe multi-header digest metadata");
  assert_eq!(
    Some("sha-256=10".to_string()),
    observed.raw_want_content_digest
  );
  assert_eq!(
    Some("sha-256=10".to_string()),
    observed.raw_want_repr_digest
  );
  assert_eq!(
    Ok(Some("sha-256=10, sha-512=8".to_string())),
    observed.want_content_digest
  );
  assert_eq!(
    Ok(Some("sha-256=10, sha-512=0".to_string())),
    observed.want_repr_digest
  );

  handle
    .join()
    .expect("multi-header digest facade server thread");
}

#[test]
fn facade_server_parses_want_digest_fields_independently_without_policy() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let mut stream = TcpStream::connect(addr).expect("connect independent digest request");
  stream
    .write_all(
      b"GET /digest-independent HTTP/1.1\r\nHost: 127.0.0.1\r\nWant-Content-Digest: sha-256=10\r\nWant-Repr-Digest: not-a-digest-preference\r\nConnection: close\r\n\r\n",
    )
    .expect("write independent digest request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe independent digest metadata");
  assert_eq!(
    Some("sha-256=10".to_string()),
    observed.raw_want_content_digest
  );
  assert_eq!(
    Some("not-a-digest-preference".to_string()),
    observed.raw_want_repr_digest
  );
  assert_eq!(
    Ok(Some("sha-256=10".to_string())),
    observed.want_content_digest
  );
  assert!(observed.want_repr_digest.is_err());

  handle
    .join()
    .expect("independent digest facade server thread");
}

#[test]
fn facade_server_parses_want_repr_digest_independently_when_want_content_digest_is_malformed() {
  let (addr, observed_rx, handle) = spawn_facade_digest_observer();

  let mut stream = TcpStream::connect(addr).expect("connect reverse independent digest request");
  stream
    .write_all(
      b"GET /digest-independent-reverse HTTP/1.1\r\nHost: 127.0.0.1\r\nWant-Content-Digest: not-a-digest-preference\r\nWant-Repr-Digest: sha-256=10\r\nConnection: close\r\n\r\n",
    )
    .expect("write reverse independent digest request");

  let observed = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe reverse independent digest metadata");
  assert_eq!(
    Some("not-a-digest-preference".to_string()),
    observed.raw_want_content_digest
  );
  assert_eq!(
    Some("sha-256=10".to_string()),
    observed.raw_want_repr_digest
  );
  assert!(observed.want_content_digest.is_err());
  assert_eq!(
    Ok(Some("sha-256=10".to_string())),
    observed.want_repr_digest
  );

  handle
    .join()
    .expect("reverse independent digest facade server thread");
}

#[test]
fn sync_client_and_server_exchange_forwarded_metadata_without_proxy_policy() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind Forwarded metadata server");
  let addr = server.local_addr().expect("Forwarded metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let forwarded = request
          .forwarded()
          .expect("Forwarded should parse")
          .expect("Forwarded should be present");
        observed_tx
          .send((
            request.header("Forwarded").map(str::to_string),
            forwarded
              .elements()
              .iter()
              .map(|element| {
                (
                  element.for_value().map(str::to_string),
                  element.by().map(str::to_string),
                  element.host().map(str::to_string),
                  element.proto().map(str::to_string),
                )
              })
              .collect::<Vec<_>>(),
          ))
          .expect("send observed Forwarded metadata");
        HttpResponse::ok("OK")
      })
      .expect("serve Forwarded metadata request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/forwarded-metadata"))
    .forwarded(r#"for=192.0.2.60;by=203.0.113.43;host=example.test;proto="https""#)
    .expect("first Forwarded value should be accepted")
    .forwarded(r#"for="[2001:db8:cafe::17]""#)
    .expect("second Forwarded value should be accepted")
    .emit()
    .expect("Forwarded metadata request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  let (raw_header, forwarded) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe Forwarded metadata");
  assert!(raw_header.is_some());
  assert_eq!(
    vec![
      (
        Some("192.0.2.60".to_string()),
        Some("203.0.113.43".to_string()),
        Some("example.test".to_string()),
        Some("https".to_string()),
      ),
      (Some("[2001:db8:cafe::17]".to_string()), None, None, None),
    ],
    forwarded
  );
  handle.join().expect("Forwarded metadata server thread");
}

#[test]
fn server_forwarded_helper_rejects_malformed_values_without_losing_raw_headers() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind malformed Forwarded metadata server");
  let addr = server
    .local_addr()
    .expect("malformed Forwarded metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send((
            request.header("Forwarded").map(str::to_string),
            request.header("Host").map(str::to_string),
            request.forwarded().is_err(),
          ))
          .expect("send malformed Forwarded observation");
        HttpResponse::ok("OK")
      })
      .expect("serve malformed Forwarded request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect malformed Forwarded request");
  stream
    .write_all(
      b"GET /matrix/malformed-forwarded HTTP/1.1\r\nHost: example.test\r\nForwarded: for=192.0.2.60;host\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Forwarded request");

  assert_eq!(
    (
      Some("for=192.0.2.60;host".to_string()),
      Some("example.test".to_string()),
      true,
    ),
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe malformed Forwarded metadata"),
  );
  handle
    .join()
    .expect("malformed Forwarded metadata server thread");
}

#[test]
fn sync_client_and_server_exchange_bounded_authentication_metadata() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind authentication server");
  let addr = server.local_addr().expect("authentication server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let authorization = request
          .authorization()
          .expect("Authorization should parse")
          .map(|value| (value.scheme().to_string(), value.credentials().to_string()));
        let proxy_authorization = request
          .proxy_authorization()
          .expect("Proxy-Authorization should parse")
          .map(|value| (value.scheme().to_string(), value.credentials().to_string()));
        observed_tx
          .send((authorization, proxy_authorization))
          .expect("send observed authentication metadata");
        HttpResponse::new(401, "Unauthorized")
          .header("WWW-Authenticate", "Broken")
          .with_www_authenticate("Basic realm=\"private\", Bearer")
          .expect("WWW-Authenticate declaration should parse")
      })
      .expect("serve authentication request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/authentication"))
    .header(("Authorization", "Bearer origin-token"))
    .header(("Proxy-Authorization", "Basic cHJveHk6c2VjcmV0"))
    .emit()
    .expect("authentication response should parse");

  let (authorization, proxy_authorization) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe authentication metadata");
  assert_eq!(
    Some(("Bearer".to_string(), "origin-token".to_string())),
    authorization
  );
  assert_eq!(
    Some(("Basic".to_string(), "cHJveHk6c2VjcmV0".to_string())),
    proxy_authorization
  );

  let challenges = response
    .www_authenticate()
    .expect("WWW-Authenticate should parse")
    .expect("WWW-Authenticate should be present");
  assert_eq!(2, challenges.challenges().len());
  assert_eq!("Basic", challenges.challenges()[0].scheme());
  assert_eq!("Bearer", challenges.challenges()[1].scheme());
  assert_eq!(
    Some(&"Basic realm=\"private\", Bearer".to_string()),
    response.header_value("WWW-Authenticate")
  );
  handle.join().expect("authentication server thread");
}

#[test]
fn sync_client_and_server_exchange_bounded_idempotency_key_metadata_without_policy() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind idempotency server");
  let addr = server.local_addr().expect("idempotency server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = (
          request
            .idempotency_key()
            .expect("Idempotency-Key should parse")
            .map(|key| key.as_str().to_string()),
          request.header("Idempotency-Key").map(str::to_string),
        );
        observed_tx
          .send(observed)
          .expect("send observed idempotency metadata");
        HttpResponse::new(201, "Created")
      })
      .expect("serve idempotency request");
  });

  let response = client()
    .post()
    .url(format!("http://{addr}/matrix/charges"))
    .idempotency_key("charge-2026-08-19-9f3c")
    .expect("Idempotency-Key should be accepted")
    .emit()
    .expect("idempotency response should parse");

  let (typed, raw) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe idempotency metadata");
  assert_eq!(Some("charge-2026-08-19-9f3c".to_string()), typed);
  assert_eq!(Some("charge-2026-08-19-9f3c".to_string()), raw);
  assert_eq!(201, response.code());
  handle.join().expect("idempotency server thread");
}

#[test]
fn sync_client_and_server_exchange_bounded_pragma_metadata_without_policy() {
  const PRAGMA_REQUEST: &str = "no-cache, community=private";
  const PRAGMA_RESPONSE: &str = "no-cache, vendor=private";

  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind pragma server");
  let addr = server.local_addr().expect("pragma server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = (
          request
            .pragma()
            .expect("Pragma should parse")
            .map(|pragma| pragma.header_value()),
          request.header("Pragma").map(str::to_string),
          request.header("Cache-Control").map(str::to_string),
        );
        observed_tx
          .send(observed)
          .expect("send observed pragma metadata");
        HttpResponse::new(200, "OK")
          .with_pragma(PRAGMA_RESPONSE)
          .expect("Pragma should be accepted")
      })
      .expect("serve pragma request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/asset"))
    .pragma(PRAGMA_REQUEST)
    .expect("Pragma should be accepted")
    .emit()
    .expect("pragma response should parse");

  let (typed, raw, cache_control) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe pragma metadata");
  assert_eq!(Some(PRAGMA_REQUEST.to_string()), typed);
  assert_eq!(Some(PRAGMA_REQUEST.to_string()), raw);
  assert_eq!(None, cache_control, "Pragma must not invent Cache-Control");

  let pragma = response
    .pragma()
    .expect("response Pragma should parse")
    .expect("response Pragma should be present");
  assert!(pragma.no_cache());
  assert_eq!(1, pragma.extensions().len());
  assert_eq!("vendor", pragma.extensions()[0].name());
  assert_eq!(Some("private"), pragma.extensions()[0].value());
  assert_eq!(
    PRAGMA_RESPONSE,
    response
      .header_value("Pragma")
      .map(String::as_str)
      .unwrap_or_default()
  );
  assert_eq!(200, response.code());
  handle.join().expect("pragma server thread");
}

#[test]
fn sync_client_and_server_observe_pragma_and_cache_control_independently() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind pragma/cache server");
  let addr = server.local_addr().expect("pragma/cache server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = (
          request
            .pragma()
            .expect("Pragma should parse")
            .map(|pragma| pragma.header_value()),
          request
            .cache_control()
            .expect("Cache-Control should parse")
            .map(|cache_control| cache_control.max_age()),
        );
        observed_tx
          .send(observed)
          .expect("send observed pragma/cache metadata");
        HttpResponse::new(200, "OK")
          .with_pragma("no-cache")
          .expect("Pragma should be accepted")
          .header("Cache-Control", "max-age=60")
      })
      .expect("serve pragma/cache request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/asset"))
    .pragma("no-cache")
    .expect("Pragma should be accepted")
    .header(("Cache-Control", "max-age=60"))
    .emit()
    .expect("pragma/cache response should parse");

  let (pragma, cache_control) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe pragma/cache metadata");
  assert_eq!(Some("no-cache".to_string()), pragma);
  assert_eq!(Some(Some(60)), cache_control);

  let response_pragma = response
    .pragma()
    .expect("response Pragma should parse")
    .expect("response Pragma should be present");
  assert!(response_pragma.no_cache());
  assert_eq!(
    Some("max-age=60"),
    response.header_value("Cache-Control").map(String::as_str),
    "response Pragma helpers must leave Cache-Control untouched"
  );
  assert_eq!(
    Some("no-cache"),
    response.header_value("Pragma").map(String::as_str)
  );
  handle.join().expect("pragma/cache server thread");
}

#[test]
fn sync_client_and_server_exchange_w3c_trace_context_metadata_without_policy() {
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind trace context server");
  let addr = server.local_addr().expect("trace context server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let traceparent = request
          .traceparent()
          .expect("traceparent should parse")
          .expect("traceparent should be present");
        let tracestate = request
          .tracestate()
          .expect("tracestate should parse")
          .expect("tracestate should be present");
        let observed = (
          traceparent.version().to_string(),
          traceparent.trace_id().to_string(),
          traceparent.parent_id().to_string(),
          traceparent.sampled(),
          tracestate
            .members()
            .iter()
            .map(|member| (member.key().to_string(), member.value().to_string()))
            .collect::<Vec<_>>(),
          request.header("traceparent").map(str::to_string),
          request.header("tracestate").map(str::to_string),
        );
        observed_tx
          .send(observed)
          .expect("send observed trace context metadata");
        HttpResponse::new(204, "No Content")
      })
      .expect("serve trace context request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/trace"))
    .traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    .expect("traceparent should be accepted")
    .tracestate("rojo=00f067aa0ba902b7,congo=t61rcWkgMzE")
    .expect("tracestate should be accepted")
    .emit()
    .expect("trace context response should parse");

  let (version, trace_id, parent_id, sampled, members, raw_traceparent, raw_tracestate) =
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe trace context metadata");
  assert_eq!("00", version);
  assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", trace_id);
  assert_eq!("00f067aa0ba902b7", parent_id);
  assert!(sampled);
  assert_eq!(
    vec![
      ("rojo".to_string(), "00f067aa0ba902b7".to_string()),
      ("congo".to_string(), "t61rcWkgMzE".to_string())
    ],
    members
  );
  assert_eq!(
    Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string()),
    raw_traceparent
  );
  assert_eq!(
    Some("rojo=00f067aa0ba902b7,congo=t61rcWkgMzE".to_string()),
    raw_tracestate
  );
  assert_eq!(204, response.code());
  handle.join().expect("trace context server thread");
}

#[test]
fn sync_client_and_server_exchange_w3c_baggage_metadata_without_policy() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind baggage server");
  let addr = server.local_addr().expect("baggage server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let baggage = request
          .baggage()
          .expect("baggage should parse")
          .expect("baggage should be present");
        let observed = (
          baggage
            .members()
            .iter()
            .map(|member| {
              (
                member.key().to_string(),
                member.value().to_string(),
                member
                  .properties()
                  .iter()
                  .map(|property| {
                    (
                      property.key().to_string(),
                      property.value().map(str::to_string),
                    )
                  })
                  .collect::<Vec<_>>(),
              )
            })
            .collect::<Vec<_>>(),
          request.header("baggage").map(str::to_string),
        );
        observed_tx
          .send(observed)
          .expect("send observed baggage metadata");
        HttpResponse::new(204, "No Content")
      })
      .expect("serve baggage request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/baggage"))
    .baggage("tenant=acme;source=gateway,release=2026-08-19")
    .expect("baggage should be accepted")
    .emit()
    .expect("baggage response should parse");

  let (members, raw_baggage) = observed_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server should observe baggage metadata");
  assert_eq!(
    vec![
      (
        "tenant".to_string(),
        "acme".to_string(),
        vec![("source".to_string(), Some("gateway".to_string()))]
      ),
      ("release".to_string(), "2026-08-19".to_string(), vec![])
    ],
    members
  );
  assert_eq!(
    Some("tenant=acme;source=gateway,release=2026-08-19".to_string()),
    raw_baggage
  );
  assert_eq!(204, response.code());
  handle.join().expect("baggage server thread");
}

#[test]
fn sync_client_and_server_exchange_authentication_info_response_metadata_without_policy() {
  const AUTHENTICATION_INFO: &str =
    r#"nextnonce="n-2", qop=auth, rspauth="origin-rsp", cnonce="c-1", nc=00000001"#;
  const PROXY_AUTHENTICATION_INFO: &str =
    r#"nextnonce="p-2", qop=auth, rspauth="proxy-rsp", cnonce="pc-1", nc=00000001"#;
  const AUTHENTICATION_INFO_CANONICAL: &str =
    "nextnonce=n-2, qop=auth, rspauth=origin-rsp, cnonce=c-1, nc=00000001";
  const PROXY_AUTHENTICATION_INFO_CANONICAL: &str =
    "nextnonce=p-2, qop=auth, rspauth=proxy-rsp, cnonce=pc-1, nc=00000001";

  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind Authentication-Info response server");
  let addr = server
    .local_addr()
    .expect("Authentication-Info response server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("OK")
          .with_authentication_info(AUTHENTICATION_INFO)
          .expect("Authentication-Info should be accepted")
          .with_proxy_authentication_info(PROXY_AUTHENTICATION_INFO)
          .expect("Proxy-Authentication-Info should be accepted")
      })
      .expect("serve Authentication-Info response");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/authentication-info"))
    .emit()
    .expect("Authentication-Info response should parse");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    Some(&AUTHENTICATION_INFO_CANONICAL.to_string()),
    response.header_value("Authentication-Info")
  );
  assert_eq!(
    Some(&PROXY_AUTHENTICATION_INFO_CANONICAL.to_string()),
    response.header_value("Proxy-Authentication-Info")
  );

  let authentication_info = response
    .authentication_info()
    .expect("Authentication-Info should parse")
    .expect("Authentication-Info should be present");
  assert_eq!(Some("n-2"), authentication_info.parameter("nextnonce"));
  assert_eq!(Some("auth"), authentication_info.parameter("qop"));
  assert_eq!(Some("origin-rsp"), authentication_info.parameter("rspauth"));
  assert_eq!(
    AUTHENTICATION_INFO_CANONICAL,
    authentication_info.header_value()
  );

  let proxy_authentication_info = response
    .proxy_authentication_info()
    .expect("Proxy-Authentication-Info should parse")
    .expect("Proxy-Authentication-Info should be present");
  assert_eq!(
    Some("p-2"),
    proxy_authentication_info.parameter("nextnonce")
  );
  assert_eq!(Some("auth"), proxy_authentication_info.parameter("qop"));
  assert_eq!(
    Some("proxy-rsp"),
    proxy_authentication_info.parameter("rspauth")
  );
  assert_eq!(
    PROXY_AUTHENTICATION_INFO_CANONICAL,
    proxy_authentication_info.header_value()
  );

  handle
    .join()
    .expect("Authentication-Info response server thread");
}

#[test]
fn sync_client_reports_absent_authentication_info_response_metadata() {
  let (addr, handle) = spawn_metadata_response_server(&[]);

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/authentication-info-absent"))
    .emit()
    .expect("response without Authentication-Info should parse");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert!(response
    .authentication_info()
    .expect("absent Authentication-Info should parse")
    .is_none());
  assert!(response
    .proxy_authentication_info()
    .expect("absent Proxy-Authentication-Info should parse")
    .is_none());

  handle
    .join()
    .expect("absent Authentication-Info response server thread");
}

#[test]
fn sync_client_authentication_info_helpers_reject_malformed_metadata_without_losing_response() {
  const AUTHENTICATION_INFO: &str = "nextnonce";
  const PROXY_AUTHENTICATION_INFO: &str = "rspauth";
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(
    authentication_info_response(Some(AUTHENTICATION_INFO), Some(PROXY_AUTHENTICATION_INFO)),
  );

  let response = client()
    .get()
    .url(format!(
      "http://{addr}/matrix/authentication-info-malformed"
    ))
    .emit()
    .expect("malformed Authentication-Info response should remain parseable");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    Some(&AUTHENTICATION_INFO.to_string()),
    response.header_value("Authentication-Info")
  );
  assert_eq!(
    Some(&PROXY_AUTHENTICATION_INFO.to_string()),
    response.header_value("Proxy-Authentication-Info")
  );
  assert!(
    response.authentication_info().is_err(),
    "Authentication-Info helper should reject malformed metadata"
  );
  assert!(
    response.proxy_authentication_info().is_err(),
    "Proxy-Authentication-Info helper should reject malformed metadata"
  );

  handle
    .join()
    .expect("malformed Authentication-Info response server thread");
}

#[test]
fn server_sec_fetch_helpers_reject_malformed_values_without_losing_raw_headers() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind malformed Sec-Fetch metadata server");
  let addr = server
    .local_addr()
    .expect("malformed Sec-Fetch metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send((
            request.header("Sec-Fetch-Site").map(str::to_string),
            request.sec_fetch_site().is_err(),
          ))
          .expect("send malformed Sec-Fetch observation");
        HttpResponse::ok("OK")
      })
      .expect("serve malformed Sec-Fetch request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect malformed Sec-Fetch request");
  stream
    .write_all(
      b"GET /matrix/malformed-sec-fetch HTTP/1.1\r\nHost: example.test\r\nSec-Fetch-Site: invalid\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Sec-Fetch request");

  assert_eq!(
    (Some("invalid".to_string()), true),
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe malformed Sec-Fetch metadata"),
  );
  handle
    .join()
    .expect("malformed Sec-Fetch metadata server thread");
}

#[test]
fn server_sec_purpose_helper_rejects_malformed_value_without_losing_raw_header() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0")
    .expect("bind malformed Sec-Purpose metadata server");
  let addr = server
    .local_addr()
    .expect("malformed Sec-Purpose metadata server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send((
            request.header("Sec-Purpose").map(str::to_string),
            request.sec_purpose().is_err(),
          ))
          .expect("send malformed Sec-Purpose observation");
        HttpResponse::ok("OK")
      })
      .expect("serve malformed Sec-Purpose request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect malformed Sec-Purpose request");
  stream
    .write_all(
      b"GET /matrix/malformed-sec-purpose HTTP/1.1\r\nHost: example.test\r\nSec-Purpose: prefetch,\r\nConnection: close\r\n\r\n",
    )
    .expect("write malformed Sec-Purpose request");

  assert_eq!(
    (Some("prefetch,".to_string()), true),
    observed_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("server should observe malformed Sec-Purpose metadata"),
  );
  handle
    .join()
    .expect("malformed Sec-Purpose metadata server thread");
}

fn assert_partial_response(
  name: &str,
  response: rttp_client::response::Response,
  expected_content_range: &str,
  expected_body: &str,
) {
  assert_eq!(206, response.code(), "{name}");
  assert!(response.is_partial_content(), "{name}");
  assert_eq!(
    Some(&expected_content_range.to_string()),
    response.header_value("Content-Range"),
    "{name}"
  );
  assert_eq!(expected_body, response.body().string().unwrap(), "{name}");
}

fn assert_observed_range(
  handle: thread::JoinHandle<Option<String>>,
  expected_range: &str,
  name: &str,
) {
  assert_eq!(
    Some(expected_range.to_string()),
    handle.join().expect("range server thread"),
    "{name}"
  );
}

fn assert_observed_if_range(
  handle: ObservedIfRangeHandle,
  expected_range: &str,
  expected_if_range: &str,
  name: &str,
) {
  assert_eq!(
    (
      Some(expected_range.to_string()),
      Some(expected_if_range.to_string())
    ),
    handle.join().expect("if-range server thread"),
    "{name}"
  );
}

fn assert_response_cache_control(
  name: &str,
  response: rttp_client::response::Response,
  expected: &fixtures::cache_control::ResponseCase,
) {
  let cache_control = response
    .cache_control()
    .unwrap_or_else(|err| panic!("{name} cache-control should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Cache-Control"));

  assert_eq!(expected.no_cache, cache_control.no_cache(), "{name}");
  assert_eq!(
    expected.no_cache_fields,
    cache_control.no_cache_fields().as_slice(),
    "{name}"
  );
  assert_eq!(expected.no_store, cache_control.no_store(), "{name}");
  assert_eq!(expected.max_age, cache_control.max_age(), "{name}");
  assert_eq!(expected.s_maxage, cache_control.s_maxage(), "{name}");
  assert_eq!(expected.private, cache_control.private(), "{name}");
  assert_eq!(
    expected.private_fields,
    cache_control.private_fields().as_slice(),
    "{name}"
  );
  assert_eq!(expected.public, cache_control.public(), "{name}");
  assert_eq!(
    expected.must_revalidate,
    cache_control.must_revalidate(),
    "{name}"
  );
  assert_eq!(
    expected.proxy_revalidate,
    cache_control.proxy_revalidate(),
    "{name}"
  );
  assert_eq!(expected.immutable, cache_control.immutable(), "{name}");
  assert_eq!(
    expected.stale_while_revalidate,
    cache_control.stale_while_revalidate(),
    "{name}"
  );
  assert_eq!(
    expected.stale_if_error,
    cache_control.stale_if_error(),
    "{name}"
  );
  assert_eq!(
    expected.extensions.len(),
    cache_control.extensions().len(),
    "{name}"
  );
  for ((expected_name, expected_value), observed) in
    expected.extensions.iter().zip(cache_control.extensions())
  {
    assert_eq!(*expected_name, observed.name(), "{name}");
    assert_eq!(*expected_value, observed.value(), "{name}");
  }
}

fn assert_response_vary(
  name: &str,
  response: rttp_client::response::Response,
  expected: &fixtures::vary::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("vary")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let vary = response
    .vary()
    .unwrap_or_else(|err| panic!("{name} Vary should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Vary"));

  assert_eq!(expected.wildcard, vary.is_any(), "{name}");
  assert_eq!(
    expected.field_names,
    vary.field_names().as_slice(),
    "{name}"
  );
  for field_name in expected.field_names {
    assert!(vary.contains_field_name(field_name), "{name} {field_name}");
    assert!(
      vary.contains_field_name(field_name.to_ascii_uppercase()),
      "{name} uppercase {field_name}"
    );
  }
}

fn assert_response_allow(
  name: &str,
  response: rttp_client::response::Response,
  expected: &fixtures::allow::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("allow")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let allow = response
    .allow()
    .unwrap_or_else(|err| panic!("{name} Allow should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Allow"));

  assert_eq!(expected.methods, allow.methods().as_slice(), "{name}");
  for method in expected.methods {
    assert!(allow.contains_method(method), "{name} {method}");
  }
}

fn assert_response_accept_ranges(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::accept_ranges::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("accept-ranges")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let accept_ranges = response
    .accept_ranges()
    .unwrap_or_else(|err| panic!("{name} Accept-Ranges should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Accept-Ranges"));

  assert_eq!(expected.none, accept_ranges.is_none(), "{name}");
  assert_eq!(
    expected.accepts_bytes,
    accept_ranges.accepts_bytes(),
    "{name}"
  );
  assert_eq!(expected.units, accept_ranges.units().as_slice(), "{name}");
}

fn assert_response_content_language(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_language::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("content-language")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let content_language = response
    .content_language()
    .unwrap_or_else(|err| panic!("{name} Content-Language should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Language"));

  assert_eq!(
    expected.languages,
    content_language.tags().as_slice(),
    "{name}"
  );
}

fn assert_response_content_location(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_location::ResponseCase,
) {
  let content_location = response
    .content_location()
    .unwrap_or_else(|err| panic!("{name} Content-Location should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Location"));

  assert_eq!(
    expected.normalized_value,
    content_location.as_str(),
    "{name}"
  );
  assert_eq!(
    Some(&expected.raw_value.to_string()),
    response.header_value("Content-Location"),
    "{name}"
  );
}

fn assert_response_content_disposition(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_disposition::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("content-disposition")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  assert_content_disposition_metadata(name, response, expected);
}

fn assert_content_disposition_metadata(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_disposition::ResponseCase,
) {
  let content_disposition = response
    .content_disposition()
    .unwrap_or_else(|err| panic!("{name} Content-Disposition should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Disposition"));

  assert_eq!(
    expected.disposition_type,
    content_disposition.disposition_type(),
    "{name}"
  );
  assert_eq!(expected.filename, content_disposition.filename(), "{name}");
  assert_eq!(
    expected.filename_ext,
    content_disposition.filename_ext(),
    "{name}"
  );
  assert_eq!(
    expected.parameters.len(),
    content_disposition.parameters().len(),
    "{name}"
  );
  for ((expected_name, expected_value), observed) in expected
    .parameters
    .iter()
    .zip(content_disposition.parameters())
  {
    assert_eq!(*expected_name, observed.name(), "{name}");
    assert_eq!(*expected_value, observed.value(), "{name}");
  }
}

fn assert_response_content_type(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_type::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("content-type")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");
  assert_content_type_metadata(name, response, expected);
}

fn assert_content_type_metadata(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_type::ResponseCase,
) {
  let content_type = response
    .content_type()
    .unwrap_or_else(|err| panic!("{name} Content-Type should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Type"));

  assert_eq!(expected.type_name, content_type.type_(), "{name}");
  assert_eq!(expected.subtype, content_type.subtype(), "{name}");
  assert_eq!(
    format!("{}/{}", expected.type_name, expected.subtype),
    content_type.essence(),
    "{name}"
  );
  assert_eq!(
    expected.parameters.len(),
    content_type.parameters().len(),
    "{name}"
  );
  for ((expected_name, expected_value), observed) in
    expected.parameters.iter().zip(content_type.parameters())
  {
    assert_eq!(*expected_name, observed.name(), "{name}");
    assert_eq!(*expected_value, observed.value(), "{name}");
    assert_eq!(
      Some(*expected_value),
      content_type.parameter(*expected_name),
      "{name}"
    );
  }
}

fn assert_response_content_encoding(
  name: &str,
  response: &rttp_client::response::Response,
  expected: &fixtures::content_encoding::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("content-encoding")
    .into_iter()
    .map(String::as_str)
    .collect();
  if expected.values == ["gzip"] {
    assert!(raw_values.is_empty(), "{name}");
    assert!(response.header("content-length").is_none(), "{name}");
    assert!(response.content_encoding().unwrap().is_none(), "{name}");
    return;
  }
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let content_encoding = response
    .content_encoding()
    .unwrap_or_else(|err| panic!("{name} Content-Encoding should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Encoding"));

  assert_eq!(
    expected.codings,
    content_encoding.codings().as_slice(),
    "{name}"
  );
}

fn assert_informational_response(
  name: &str,
  observed: &rttp_client::response::InformationalResponse,
  expected: &fixtures::response::InformationalExpectation,
) {
  assert_eq!(expected.code, observed.code(), "{name}");
  assert_eq!(expected.reason, observed.reason(), "{name}");
  assert_eq!(expected.headers.len(), observed.headers().len(), "{name}");
  for ((header_name, header_value), observed_header) in
    expected.headers.iter().zip(observed.headers())
  {
    assert_eq!(*header_name, observed_header.name(), "{name}");
    assert_eq!(
      *header_value,
      observed_header.value(),
      "{name} {header_name}"
    );
  }
}

fn assert_cache_control_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-control-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.cache_control().is_err(),
    "{name} helper should reject invalid Cache-Control"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_cache_status_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-status-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.cache_status().is_err(),
    "{name} helper should reject invalid Cache-Status"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_cdn_cache_control_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cdn-cache-control-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.cdn_cache_control().is_err(),
    "{name} helper should reject invalid CDN-Cache-Control"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_vary_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/vary-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.vary().is_err(),
    "{name} helper should reject invalid Vary"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_age_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/age-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.age().is_err(),
    "{name} helper should reject invalid Age"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_expires_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/expires-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.expires().is_err(),
    "{name} helper should reject invalid Expires"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_retry_after_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/retry-after-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.retry_after().is_err(),
    "{name} helper should reject invalid Retry-After"
  );
  assert_eq!("busy", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_allow_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/allow-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.allow().is_err(),
    "{name} helper should reject invalid Allow"
  );
  assert_eq!("", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_accept_ranges_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/accept-ranges-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.accept_ranges().is_err(),
    "{name} helper should reject invalid Accept-Ranges"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_content_language_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_body: &str,
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/content-language-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.content_language().is_err(),
    "{name} helper should reject invalid Content-Language"
  );
  assert_eq!(expected_body, response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_service_worker_allowed_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_values: &[&str],
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!(
      "http://{}/matrix/service-worker-allowed-invalid",
      addr
    ))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.service_worker_allowed().is_err(),
    "{name} helper should reject invalid Service-Worker-Allowed"
  );
  let raw_values: Vec<&str> = response
    .header_values("Service-Worker-Allowed")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected_values, raw_values.as_slice(), "{name}");
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_content_location_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_value: &str,
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/content-location-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.content_location().is_err(),
    "{name} helper should reject invalid Content-Location"
  );
  assert_eq!(
    Some(&expected_value.to_string()),
    response.header_value("Content-Location"),
    "{name}"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_content_disposition_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_values: &[&str],
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!(
      "http://{}/matrix/content-disposition-invalid",
      addr
    ))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.content_disposition().is_err(),
    "{name} helper should reject invalid Content-Disposition"
  );
  let raw_values: Vec<&str> = response
    .header_values("Content-Disposition")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected_values, raw_values.as_slice(), "{name}");
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_content_type_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_values: &[&str],
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/content-type-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.content_type().is_err(),
    "{name} helper should reject invalid Content-Type"
  );
  let raw_values: Vec<&str> = response
    .header_values("Content-Type")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected_values, raw_values.as_slice(), "{name}");
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_content_encoding_helper_rejects_but_preserves_response(
  name: &str,
  raw_response: Vec<u8>,
  expected_values: &[&str],
) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/content-encoding-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.content_encoding().is_err(),
    "{name} helper should reject invalid Content-Encoding"
  );
  let raw_values: Vec<&str> = response
    .header_values("Content-Encoding")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected_values, raw_values.as_slice(), "{name}");
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

enum ConditionalHeader {
  IfNoneMatch(&'static str),
  IfMatch(&'static str),
  IfModifiedSince(&'static str),
  IfUnmodifiedSince(&'static str),
  Manual(&'static str, &'static str),
}

#[test]
fn sync_client_preserves_shared_informational_response_matrix() {
  for case in fixtures::response::informational_response_cases() {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(case.raw);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/informational", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(case.final_status, response.code(), "{}", case.name);
    assert_eq!(case.final_reason, response.reason(), "{}", case.name);
    assert_eq!(
      Some(&case.final_marker.to_string()),
      response.header_value("X-Final"),
      "{}",
      case.name
    );
    assert_eq!(
      case.final_body,
      response.body().string().unwrap(),
      "{}",
      case.name
    );
    assert_eq!(
      case.informational.len(),
      response.informational_responses().len(),
      "{}",
      case.name
    );
    for (observed, expected) in response
      .informational_responses()
      .iter()
      .zip(case.informational)
    {
      assert_informational_response(case.name, observed, expected);
    }

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_filters_early_hints_from_mixed_informational_responses() {
  let raw = concat!(
    "HTTP/1.1 100 Continue\r\n",
    "X-Continue: first\r\n",
    "\r\n",
    "HTTP/1.1 103 Early Hints\r\n",
    "Link: </first.css>; rel=preload\r\n",
    "\r\n",
    "HTTP/1.1 102 Processing\r\n",
    "X-Progress: accepted\r\n",
    "\r\n",
    "HTTP/1.1 103 Early Hints\r\n",
    "Link: </second.css>; rel=preload\r\n",
    "\r\n",
    "HTTP/1.1 200 OK\r\n",
    "X-Final: mixed-early-hints\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  )
  .as_bytes();
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/early-hints", addr))
    .emit()
    .expect("mixed informational response should parse");

  assert_eq!(200, response.code());
  let informational = response.informational_responses();
  assert_eq!(4, informational.len());
  assert_eq!(
    vec![100, 103, 102, 103],
    informational
      .iter()
      .map(|response| response.code())
      .collect::<Vec<_>>()
  );

  let early_hints = response.early_hints();
  assert_eq!(2, early_hints.len());
  assert_eq!(103, early_hints[0].code());
  assert_eq!(
    Some("</first.css>; rel=preload"),
    early_hints[0].header_value("Link").map(String::as_str)
  );
  assert_eq!(103, early_hints[1].code());
  assert_eq!(
    Some("</second.css>; rel=preload"),
    early_hints[1].header_value("Link").map(String::as_str)
  );

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_returns_no_early_hints_without_103_informational_response() {
  let raw = concat!(
    "HTTP/1.1 100 Continue\r\n",
    "\r\n",
    "HTTP/1.1 102 Processing\r\n",
    "X-Progress: accepted\r\n",
    "\r\n",
    "HTTP/1.1 200 OK\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  )
  .as_bytes();
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/no-early-hints", addr))
    .emit()
    .expect("informational response should parse");

  assert_eq!(200, response.code());
  assert_eq!(2, response.informational_responses().len());
  assert!(response.early_hints().is_empty());

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_rejects_shared_malformed_informational_heads() {
  for case in fixtures::response::malformed_informational_response_cases() {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(case.raw);

    let error = client()
      .get()
      .url(format!("http://{}/matrix/informational-invalid", addr))
      .emit()
      .expect_err(case.name);

    assert!(
      error.to_string().contains(case.error_contains),
      "{} unexpected error: {error}",
      case.name
    );

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_rejects_shared_oversized_informational_head() {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(
    fixtures::response::oversized_informational_response(),
  );

  let error = client()
    .get()
    .url(format!("http://{}/matrix/informational-oversized", addr))
    .emit()
    .expect_err("oversized informational response should be rejected");

  assert!(
    error
      .to_string()
      .contains("HTTP informational response head is too large"),
    "unexpected error: {error}"
  );

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_keeps_shared_101_handoff_separate_from_informational_history() {
  let (addr, handle) =
    fixtures::spawn_socket2_raw_response_server(fixtures::response::SWITCHING_PROTOCOLS_HANDOFF);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/upgrade", addr))
    .emit()
    .expect("101 response should parse as the final response");

  assert_eq!(101, response.code());
  assert_eq!("Switching Protocols", response.reason());
  assert!(response.informational_responses().is_empty());
  assert_eq!(
    Some(&"websocket".to_string()),
    response.header_value("Upgrade")
  );
  assert_eq!("", response.body().string().unwrap());

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_parses_shared_allow_response_matrix() {
  for case in fixtures::allow::response_cases() {
    let raw_response = allow_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/allow", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_allow(case.name, response, case);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_accept_ranges_response_matrix() {
  for case in fixtures::accept_ranges::response_cases() {
    let raw_response = accept_ranges_response(case.values, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/accept-ranges", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_accept_ranges(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_content_language_response_matrix() {
  for case in fixtures::content_language::response_cases() {
    let raw_response = content_language_response(case.values, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-language", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_language(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_service_worker_allowed_from_server_declaration() {
  let server_response = HttpResponse::ok("OK")
    .with_service_worker_allowed("/")
    .expect("Service-Worker-Allowed declaration should parse");
  let raw_response = server_response.to_bytes();
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/service-worker-allowed", addr))
    .emit()
    .expect("Service-Worker-Allowed response should parse");

  assert_eq!(
    "/",
    response
      .service_worker_allowed()
      .expect("Service-Worker-Allowed should parse")
      .expect("Service-Worker-Allowed should be present")
      .as_str()
  );
  assert_eq!(
    Some(&"/".to_string()),
    response.header_value("Service-Worker-Allowed")
  );
  assert_eq!("OK", response.body().string().unwrap());
  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_service_worker_allowed_helper_rejects_malformed_duplicate_and_oversized_values() {
  for value in [
    "",
    "/bad path",
    "/bad%zz",
    "http://example.test/scope",
    "//example.test/scope",
  ] {
    assert_service_worker_allowed_helper_rejects_but_preserves_response(
      "malformed Service-Worker-Allowed value",
      service_worker_allowed_response(&[value]),
      &[value.trim()],
    );
  }

  assert_service_worker_allowed_helper_rejects_but_preserves_response(
    "duplicate Service-Worker-Allowed header fields",
    service_worker_allowed_response(&["/", "/app/"]),
    &["/", "/app/"],
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  assert_service_worker_allowed_helper_rejects_but_preserves_response(
    "oversized Service-Worker-Allowed value",
    service_worker_allowed_response(&[&oversized]),
    &[&oversized],
  );
}

#[test]
fn sync_client_parses_shared_content_location_response_matrix() {
  for case in fixtures::content_location::response_cases() {
    let raw_response = content_location_response(case.values, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-location", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_location(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_content_disposition_response_matrix() {
  for case in fixtures::content_disposition::response_cases() {
    let raw_response = content_disposition_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-disposition", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_disposition(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_content_type_response_matrix() {
  for case in fixtures::content_type::response_cases() {
    let raw_response = content_type_response(case.values, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-type", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_type(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_content_encoding_response_matrix() {
  for case in fixtures::content_encoding::response_cases() {
    let raw_response = content_encoding_response(case.values, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-encoding", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_encoding(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_cache_control_response_matrix() {
  for case in fixtures::cache_control::response_cases() {
    let raw_response = cache_control_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/cache-control", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_cache_control(case.name, response, case);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_vary_response_matrix() {
  for case in fixtures::vary::response_cases() {
    let raw_response = vary_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/vary", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_vary(case.name, response, case);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_age_response_matrix() {
  for case in fixtures::age_expires::age_cases() {
    let raw_response = age_expires_response(
      case.value,
      fixtures::age_expires::EXPIRES_IMF_FIXDATE,
      false,
    );
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/age", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(case.delta_seconds),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(&case.value.to_string()),
      response.header_value("Age"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_expires_response_matrix() {
  for case in fixtures::age_expires::expires_cases() {
    let raw_response = age_expires_response("0", case.value, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/expires", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(std::time::UNIX_EPOCH + Duration::from_secs(case.unix_seconds)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(&case.value.to_string()),
      response.header_value("Expires"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_reads_sunset_emitted_by_server() {
  let sunset = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let server =
    rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind Sunset response server");
  let addr = server.local_addr().expect("Sunset response server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| HttpResponse::ok("OK").with_sunset(sunset))
      .expect("serve Sunset response request");
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/sunset"))
    .emit()
    .expect("Sunset response should parse");

  assert_eq!(
    Some(sunset),
    response.sunset().expect("Sunset should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.sunset_value()
  );
  handle.join().expect("Sunset response server thread");
}

#[test]
fn sync_client_parses_shared_retry_after_response_matrix() {
  for case in fixtures::retry_after::retry_after_cases() {
    let raw_response = retry_after_response(&[case.value], false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/retry-after", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    let retry_after = response
      .retry_after()
      .unwrap_or_else(|err| panic!("{} Retry-After should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Retry-After", case.name));
    match &case.kind {
      fixtures::retry_after::RetryAfterKind::DeltaSeconds(delta_seconds) => {
        assert_eq!(
          Some(*delta_seconds),
          retry_after.delta_seconds(),
          "{}",
          case.name
        );
        assert_eq!(None, retry_after.http_date(), "{}", case.name);
      }
      fixtures::retry_after::RetryAfterKind::HttpDate(unix_seconds) => {
        assert_eq!(None, retry_after.delta_seconds(), "{}", case.name);
        assert_eq!(
          Some(UNIX_EPOCH + Duration::from_secs(*unix_seconds)),
          retry_after.http_date(),
          "{}",
          case.name
        );
      }
    }
    assert_eq!(
      Some(&case.value.to_string()),
      response.header_value("Retry-After"),
      "{}",
      case.name
    );
    assert_eq!("busy", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_allow_with_existing_cache_and_retry_metadata_helpers() {
  let raw_response = allow_response_with_cache_metadata(&["GET, HEAD", "POST"]);
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/allow-cache-metadata", addr))
    .emit()
    .expect("Allow response with cache metadata should parse");

  assert_eq!(
    &["GET", "HEAD", "POST"],
    response
      .allow()
      .expect("Allow should parse")
      .expect("Allow should be present")
      .methods()
      .as_slice()
  );
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .contains_field_name("accept-encoding"));
  assert_eq!(
    Some(30),
    response
      .retry_after()
      .expect("Retry-After should parse")
      .expect("Retry-After should be present")
      .delta_seconds()
  );
  assert_eq!("", response.body().string().unwrap());

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_parses_no_vary_search_metadata_without_cache_policy() {
  let raw_response = no_vary_search_response(&["key-order=?0, params", r#"except=("session")"#]);
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/no-vary-search-metadata", addr))
    .emit()
    .expect("No-Vary-Search response should parse");

  let no_vary_search = response
    .no_vary_search()
    .expect("No-Vary-Search should parse")
    .expect("No-Vary-Search should be present");

  assert_eq!(Some(false), no_vary_search.key_order());
  assert!(no_vary_search.ignores_all_query_params());
  assert_eq!(no_vary_search.except(), ["session"]);
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("No-Vary-Search server thread");
}

#[test]
fn sync_client_parses_accept_ranges_with_existing_metadata_helpers() {
  let raw_response = accept_ranges_response(&["bytes, pages"], true);
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/accept-ranges-metadata", addr))
    .emit()
    .expect("Accept-Ranges response with adjacent metadata should parse");

  assert_eq!(
    &["bytes", "pages"],
    response
      .accept_ranges()
      .expect("Accept-Ranges should parse")
      .expect("Accept-Ranges should be present")
      .units()
      .as_slice()
  );
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .contains_field_name("accept-encoding"));
  assert_eq!(
    Some(30),
    response
      .retry_after()
      .expect("Retry-After should parse")
      .expect("Retry-After should be present")
      .delta_seconds()
  );
  assert!(response
    .allow()
    .expect("Allow should parse")
    .expect("Allow should be present")
    .contains_method("GET"));
  assert_eq!(
    &["fr-CA", "es-419"],
    response
      .content_language()
      .expect("Content-Language should parse")
      .expect("Content-Language should be present")
      .tags()
      .as_slice()
  );
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_parses_age_expires_with_existing_cache_metadata_helpers() {
  for case in fixtures::age_expires::declaration_cases() {
    let raw_response = age_expires_response(case.age_value, case.expires_value, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/cache-metadata", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(case.age),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(std::time::UNIX_EPOCH + Duration::from_secs(case.expires_unix_seconds)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(60),
      response
        .cache_control()
        .expect("Cache-Control should parse")
        .expect("Cache-Control should be present")
        .max_age(),
      "{}",
      case.name
    );
    assert!(
      response
        .vary()
        .expect("Vary should parse")
        .expect("Vary should be present")
        .contains_field_name("accept-encoding"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_retry_after_with_existing_cache_metadata_helpers() {
  for case in fixtures::retry_after::retry_after_cases() {
    let raw_response = retry_after_response(&[case.value], true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/retry-after-cache-metadata", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert!(
      response
        .retry_after()
        .unwrap_or_else(|err| panic!("{} Retry-After should parse: {err}", case.name))
        .is_some(),
      "{}",
      case.name
    );
    assert_eq!(
      Some(60),
      response
        .cache_control()
        .expect("Cache-Control should parse")
        .expect("Cache-Control should be present")
        .max_age(),
      "{}",
      case.name
    );
    assert_eq!(
      Some(5),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert!(
      response
        .vary()
        .expect("Vary should parse")
        .expect("Vary should be present")
        .contains_field_name("accept-encoding"),
      "{}",
      case.name
    );
    assert_eq!("busy", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_content_language_with_existing_metadata_helpers() {
  for case in fixtures::content_language::response_cases() {
    let raw_response = content_language_response(case.values, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-language-adjacent", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_language(case.name, &response, case);
    assert_eq!(
      Some(60),
      response
        .cache_control()
        .expect("Cache-Control should parse")
        .expect("Cache-Control should be present")
        .max_age(),
      "{}",
      case.name
    );
    assert_eq!(Some(5), response.age().expect("Age should parse"));
    assert_eq!(
      Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
      response.expires().expect("Expires should parse"),
      "{}",
      case.name
    );
    assert!(
      response
        .vary()
        .expect("Vary should parse")
        .expect("Vary should be present")
        .contains_field_name("accept-encoding"),
      "{}",
      case.name
    );
    assert_eq!(
      Some(30),
      response
        .retry_after()
        .expect("Retry-After should parse")
        .expect("Retry-After should be present")
        .delta_seconds(),
      "{}",
      case.name
    );
    assert!(
      response
        .allow()
        .expect("Allow should parse")
        .expect("Allow should be present")
        .contains_method("GET"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_content_location_with_existing_metadata_helpers() {
  for case in fixtures::content_location::response_cases() {
    let raw_response = content_location_response(case.values, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-location-adjacent", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_location(case.name, &response, case);
    assert_eq!(
      Some(60),
      response
        .cache_control()
        .expect("Cache-Control should parse")
        .expect("Cache-Control should be present")
        .max_age(),
      "{}",
      case.name
    );
    assert_eq!(Some(5), response.age().expect("Age should parse"));
    assert_eq!(
      Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
      response.expires().expect("Expires should parse"),
      "{}",
      case.name
    );
    assert!(
      response
        .vary()
        .expect("Vary should parse")
        .expect("Vary should be present")
        .contains_field_name("accept-encoding"),
      "{}",
      case.name
    );
    assert_eq!(
      Some(30),
      response
        .retry_after()
        .expect("Retry-After should parse")
        .expect("Retry-After should be present")
        .delta_seconds(),
      "{}",
      case.name
    );
    assert!(
      response
        .allow()
        .expect("Allow should parse")
        .expect("Allow should be present")
        .contains_method("GET"),
      "{}",
      case.name
    );
    assert_eq!(
      &["fr-CA", "es-419"],
      response
        .content_language()
        .expect("Content-Language should parse")
        .expect("Content-Language should be present")
        .tags()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!(
      &["bytes", "pages"],
      response
        .accept_ranges()
        .expect("Accept-Ranges should parse")
        .expect("Accept-Ranges should be present")
        .units()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_content_type_with_existing_metadata_helpers() {
  for case in fixtures::content_type::response_cases() {
    let raw_response = content_type_response(case.values, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-type-adjacent", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_type(case.name, &response, case);
    assert_eq!(
      &["gzip", "br"],
      response
        .content_encoding()
        .expect("Content-Encoding should parse")
        .expect("Content-Encoding should be present")
        .codings()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!(
      &["fr-CA", "es-419"],
      response
        .content_language()
        .expect("Content-Language should parse")
        .expect("Content-Language should be present")
        .tags()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!(
      Some("/representations/current"),
      response
        .content_location()
        .expect("Content-Location should parse")
        .as_ref()
        .map(|location| location.as_str()),
      "{}",
      case.name
    );
    assert_eq!(
      &["bytes", "pages"],
      response
        .accept_ranges()
        .expect("Accept-Ranges should parse")
        .expect("Accept-Ranges should be present")
        .units()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_content_encoding_with_existing_metadata_helpers() {
  for case in fixtures::content_encoding::response_cases() {
    let raw_response = content_encoding_response(case.values, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-encoding-adjacent", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_content_encoding(case.name, &response, case);
    let content_type = response
      .content_type()
      .expect("Content-Type should parse")
      .expect("Content-Type should be present");
    assert!(content_type.is("text", "plain"), "{}", case.name);
    assert_eq!(
      Some("utf-8"),
      content_type.parameter("charset"),
      "{}",
      case.name
    );
    assert_eq!(
      &["fr-CA", "es-419"],
      response
        .content_language()
        .expect("Content-Language should parse")
        .expect("Content-Language should be present")
        .tags()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!(
      Some("/representations/current"),
      response
        .content_location()
        .expect("Content-Location should parse")
        .as_ref()
        .map(|location| location.as_str()),
      "{}",
      case.name
    );
    assert_eq!(
      &["bytes", "pages"],
      response
        .accept_ranges()
        .expect("Accept-Ranges should parse")
        .expect("Accept-Ranges should be present")
        .units()
        .as_slice(),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_observes_rttp_server_content_disposition_helpers() {
  for case in fixtures::content_disposition::response_cases() {
    let disposition = HttpContentDisposition::new(case.disposition_type)
      .unwrap_or_else(|err| panic!("{} disposition type should parse: {err}", case.name));
    let disposition = case
      .parameters
      .iter()
      .fold(disposition, |disposition, (name, value)| {
        disposition
          .with_parameter(name, value)
          .unwrap_or_else(|err| panic!("{} parameter should parse: {err}", case.name))
      });
    let (addr, handle) = spawn_content_disposition_server(disposition);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-disposition-server", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      vec![case.normalized_value],
      response
        .header_values("Content-Disposition")
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
      "{}",
      case.name
    );
    assert_content_disposition_metadata(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);

    handle.join().expect("content-disposition server thread");
  }
}

#[test]
fn sync_client_observes_rttp_server_content_type_helpers() {
  for case in fixtures::content_type::response_cases() {
    let content_type = HttpContentType::new(case.type_name, case.subtype)
      .unwrap_or_else(|err| panic!("{} media type should parse: {err}", case.name));
    let content_type = case
      .parameters
      .iter()
      .fold(content_type, |content_type, (name, value)| {
        content_type
          .with_parameter(name, value)
          .unwrap_or_else(|err| panic!("{} parameter should parse: {err}", case.name))
      });
    let (addr, handle) = spawn_content_type_server(content_type);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-type-server", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      vec![case.normalized_value],
      response
        .header_values("Content-Type")
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
      "{}",
      case.name
    );
    assert_content_type_metadata(case.name, &response, case);
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);

    handle.join().expect("content-type server thread");
  }
}

#[test]
fn sync_client_observes_rttp_server_content_encoding_helpers() {
  for case in fixtures::content_encoding::response_cases() {
    let (addr, handle) = spawn_content_encoding_server(case.codings);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/content-encoding-server", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    if case.codings == ["gzip"] {
      assert!(
        response.header("Content-Encoding").is_none(),
        "{}",
        case.name
      );
      assert!(response.header("Content-Length").is_none(), "{}", case.name);
      assert!(
        response.content_encoding().unwrap().is_none(),
        "{}",
        case.name
      );
    } else {
      let expected_header_value = case.codings.join(", ");
      assert_eq!(
        vec![expected_header_value.as_str()],
        response
          .header_values("Content-Encoding")
          .into_iter()
          .map(String::as_str)
          .collect::<Vec<_>>(),
        "{}",
        case.name
      );
      let content_encoding = response
        .content_encoding()
        .unwrap_or_else(|err| panic!("{} Content-Encoding should parse: {err}", case.name))
        .unwrap_or_else(|| panic!("{} should include Content-Encoding", case.name));
      assert_eq!(
        case.codings,
        content_encoding.codings().as_slice(),
        "{}",
        case.name
      );
    }
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);

    handle.join().expect("content-encoding server thread");
  }
}

#[test]
fn sync_client_cache_control_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::cache_control::invalid_response_cases() {
    assert_cache_control_helper_rejects_but_preserves_response(
      case.name,
      cache_control_response(&[case.value]),
    );
  }
}

#[test]
fn sync_client_allow_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::allow::invalid_cases() {
    assert_allow_helper_rejects_but_preserves_response(case.name, allow_response(&[case.value]));
  }
}

#[test]
fn sync_client_accept_ranges_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::accept_ranges::invalid_cases() {
    assert_accept_ranges_helper_rejects_but_preserves_response(
      case.name,
      accept_ranges_response(&[case.value], false),
    );
  }
}

#[test]
fn sync_client_vary_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::vary::invalid_cases() {
    assert_vary_helper_rejects_but_preserves_response(case.name, vary_response(&[case.value]));
  }
}

#[test]
fn sync_client_age_and_expires_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::age_expires::invalid_age_cases() {
    assert_age_helper_rejects_but_preserves_response(
      case.name,
      age_expires_response(
        case.value,
        fixtures::age_expires::EXPIRES_IMF_FIXDATE,
        false,
      ),
    );
  }

  for case in fixtures::age_expires::invalid_expires_cases() {
    assert_expires_helper_rejects_but_preserves_response(
      case.name,
      age_expires_response("0", case.value, false),
    );
  }
}

#[test]
fn sync_client_content_language_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::content_language::invalid_cases() {
    assert_content_language_helper_rejects_but_preserves_response(
      case.name,
      content_language_response(&[case.value], false),
      "OK",
    );
  }
}

#[test]
fn sync_client_content_location_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::content_location::invalid_cases() {
    assert_content_location_helper_rejects_but_preserves_response(
      case.name,
      content_location_response(&[case.value], false),
      case.value.trim(),
    );
  }
}

#[test]
fn sync_client_content_disposition_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::content_disposition::invalid_cases() {
    assert_content_disposition_helper_rejects_but_preserves_response(
      case.name,
      content_disposition_response(&[case.value]),
      &[case.value],
    );
  }
}

#[test]
fn sync_client_content_type_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::content_type::invalid_cases() {
    assert_content_type_helper_rejects_but_preserves_response(
      case.name,
      content_type_response(&[case.value], false),
      &[case.value],
    );
  }
}

#[test]
fn sync_client_content_encoding_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::content_encoding::invalid_cases() {
    assert_content_encoding_helper_rejects_but_preserves_response(
      case.name,
      content_encoding_response(&[case.value], false),
      &[case.value.trim()],
    );
  }
}

#[test]
fn sync_client_retry_after_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::retry_after::invalid_cases() {
    assert_retry_after_helper_rejects_but_preserves_response(
      case.name,
      retry_after_response(&[case.value], false),
    );
  }
}

#[test]
fn sync_client_cache_control_helper_enforces_shared_bounds() {
  assert_cache_control_helper_rejects_but_preserves_response(
    "too many response Cache-Control directives",
    cache_control_response(&[&fixtures::cache_control::too_many_directives_value()]),
  );
  assert_cache_control_helper_rejects_but_preserves_response(
    "oversized response Cache-Control value",
    cache_control_response(&[&fixtures::cache_control::oversized_value()]),
  );
}

#[test]
fn sync_client_retry_after_helper_rejects_duplicate_singleton_and_oversized_values() {
  assert_retry_after_helper_rejects_but_preserves_response(
    "duplicate Retry-After header fields",
    retry_after_response(&["60", "120"], false),
  );
  assert_retry_after_helper_rejects_but_preserves_response(
    "oversized Retry-After value",
    retry_after_response(&[&fixtures::retry_after::oversized_value()], false),
  );
}

#[test]
fn sync_client_content_location_helper_rejects_duplicate_singleton_and_oversized_values() {
  assert_content_location_helper_rejects_but_preserves_response(
    "duplicate Content-Location header fields",
    content_location_response(&["/one", "/two"], false),
    "/one",
  );
  let oversized = fixtures::content_location::oversized_value();
  assert_content_location_helper_rejects_but_preserves_response(
    "oversized Content-Location value",
    content_location_response(&[&oversized], false),
    &oversized,
  );
}

#[test]
fn sync_client_content_disposition_helper_rejects_duplicates_and_enforces_shared_bounds() {
  assert_content_disposition_helper_rejects_but_preserves_response(
    "duplicate Content-Disposition header fields",
    content_disposition_response(&["attachment; filename=one.txt", "inline; filename=two.txt"]),
    &["attachment; filename=one.txt", "inline; filename=two.txt"],
  );
  assert_content_disposition_helper_rejects_but_preserves_response(
    "duplicate Content-Disposition parameters",
    content_disposition_response(&[fixtures::content_disposition::duplicate_parameter_value()]),
    &[fixtures::content_disposition::duplicate_parameter_value()],
  );

  let oversized = fixtures::content_disposition::oversized_value();
  assert_content_disposition_helper_rejects_but_preserves_response(
    "oversized Content-Disposition value",
    content_disposition_response(&[&oversized]),
    &[&oversized],
  );

  let too_many = fixtures::content_disposition::too_many_parameters_value();
  assert_content_disposition_helper_rejects_but_preserves_response(
    "too many Content-Disposition parameters",
    content_disposition_response(&[&too_many]),
    &[&too_many],
  );
}

#[test]
fn sync_client_content_type_helper_rejects_duplicates_and_enforces_shared_bounds() {
  assert_content_type_helper_rejects_but_preserves_response(
    "duplicate Content-Type header fields",
    content_type_response(&["text/plain", "application/json"], false),
    &["text/plain", "application/json"],
  );
  assert_content_type_helper_rejects_but_preserves_response(
    "duplicate Content-Type parameters",
    content_type_response(
      &[fixtures::content_type::duplicate_parameter_value()],
      false,
    ),
    &[fixtures::content_type::duplicate_parameter_value()],
  );

  let oversized = fixtures::content_type::oversized_value();
  assert_content_type_helper_rejects_but_preserves_response(
    "oversized Content-Type value",
    content_type_response(&[&oversized], false),
    &[&oversized],
  );

  let too_many = fixtures::content_type::too_many_client_parameters_value();
  assert_content_type_helper_rejects_but_preserves_response(
    "too many Content-Type parameters",
    content_type_response(&[&too_many], false),
    &[&too_many],
  );
}

#[test]
fn sync_client_content_encoding_helper_rejects_duplicates_and_enforces_shared_bounds() {
  assert_content_encoding_helper_rejects_but_preserves_response(
    "duplicate Content-Encoding codings across header fields",
    content_encoding_response(&["gzip, br", "GZIP"], false),
    &["gzip, br", "GZIP"],
  );

  let too_many = fixtures::content_encoding::too_many_client_codings_value();
  assert_content_encoding_helper_rejects_but_preserves_response(
    "too many Content-Encoding codings",
    content_encoding_response(&[&too_many], false),
    &[&too_many],
  );

  let oversized = fixtures::content_encoding::oversized_value();
  assert_content_encoding_helper_rejects_but_preserves_response(
    "oversized Content-Encoding value",
    content_encoding_response(&[&oversized], false),
    &[&oversized],
  );
}

#[test]
fn sync_client_allow_helper_rejects_duplicate_methods_and_enforces_shared_bounds() {
  assert_allow_helper_rejects_but_preserves_response(
    "duplicate Allow methods across header fields",
    allow_response(&["GET, HEAD", "POST, GET"]),
  );
  assert_allow_helper_rejects_but_preserves_response(
    "too many Allow methods",
    allow_response(&[&fixtures::allow::too_many_methods_value()]),
  );
  assert_allow_helper_rejects_but_preserves_response(
    "oversized Allow value",
    allow_response(&[&fixtures::allow::oversized_value()]),
  );
}

#[test]
fn sync_client_accept_ranges_helper_rejects_duplicates_and_enforces_shared_bounds() {
  assert_accept_ranges_helper_rejects_but_preserves_response(
    "duplicate Accept-Ranges units across header fields",
    accept_ranges_response(&["bytes, pages", "BYTES"], false),
  );
  assert_accept_ranges_helper_rejects_but_preserves_response(
    "too many client Accept-Ranges units",
    accept_ranges_response(
      &[&fixtures::accept_ranges::too_many_client_units_value()],
      false,
    ),
  );
  assert_accept_ranges_helper_rejects_but_preserves_response(
    "oversized Accept-Ranges value",
    accept_ranges_response(&[&fixtures::accept_ranges::oversized_value()], false),
  );
}

#[test]
fn sync_client_content_language_helper_rejects_duplicates_and_enforces_client_bounds() {
  assert_content_language_helper_rejects_but_preserves_response(
    "duplicate Content-Language tags across header fields",
    content_language_response(&["en-US, fr", "EN-us"], false),
    "OK",
  );
  assert_content_language_helper_rejects_but_preserves_response(
    "too many client Content-Language tags",
    content_language_response(
      &[&fixtures::content_language::too_many_client_languages_value()],
      false,
    ),
    "OK",
  );
  assert_content_language_helper_rejects_but_preserves_response(
    "oversized Content-Language value",
    content_language_response(&[&fixtures::content_language::oversized_value()], false),
    "OK",
  );
}

#[test]
fn sync_client_vary_helper_enforces_shared_bounds() {
  assert_vary_helper_rejects_but_preserves_response(
    "too many Vary field names",
    vary_response(&[&fixtures::vary::too_many_field_names_value()]),
  );
  assert_vary_helper_rejects_but_preserves_response(
    "oversized Vary value",
    vary_response(&[&fixtures::vary::oversized_value()]),
  );
}

#[test]
fn sync_client_cache_control_matrix_keeps_cache_engine_non_goals_explicit() {
  let raw_response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "ETag: \"representation\"\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  )
  .as_bytes();
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-control-non-goals", addr))
    .emit()
    .expect("response without Cache-Control should parse");

  assert!(response
    .cache_control()
    .expect("missing header is valid")
    .is_none());
  assert_eq!(
    Some(&"\"representation\"".to_string()),
    response.header_value("ETag")
  );
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_parses_cache_status_response_metadata_without_policy() {
  const HEADERS: &[(&str, &str)] = &[
    ("Cache-Status", "OriginCache; hit; ttl=1100"),
    ("cache-status", r#""CDN Company Here"; hit; ttl=545"#),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-status", addr))
    .emit()
    .expect("Cache-Status response should parse");

  let metadata = response
    .cache_status()
    .expect("Cache-Status metadata should parse")
    .expect("Cache-Status should be present");

  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.members()[0].identifier().as_str(), "OriginCache");
  assert_eq!(metadata.members()[0].hit(), Some(true));
  assert_eq!(metadata.members()[0].ttl(), Some(1100));
  assert_eq!(
    metadata.members()[1].identifier().as_str(),
    "CDN Company Here"
  );
  assert!(metadata.members()[1].identifier().is_string());
  assert_eq!(metadata.members()[1].ttl(), Some(545));
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_cache_status_helper_rejects_invalid_and_bounded_metadata() {
  let oversized = "x".repeat(64 * 1024 + 1);
  assert_cache_status_helper_rejects_but_preserves_response(
    "invalid Cache-Status boolean",
    cache_status_response(&["OriginCache; hit=yes"]),
  );
  assert_cache_status_helper_rejects_but_preserves_response(
    "trailing Cache-Status member",
    cache_status_response(&["OriginCache,"]),
  );
  assert_cache_status_helper_rejects_but_preserves_response(
    "oversized Cache-Status value",
    cache_status_response(&[oversized.as_str()]),
  );
}

#[test]
fn sync_client_parses_cdn_cache_control_response_metadata_without_policy() {
  const HEADERS: &[(&str, &str)] = &[
    (
      "CDN-Cache-Control",
      "max-age=600, stale-while-revalidate=30, cdn-example=\"a, b\"",
    ),
    ("cdn-cache-control", "immutable"),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cdn-cache-control", addr))
    .emit()
    .expect("CDN-Cache-Control response should parse");

  let metadata = response
    .cdn_cache_control()
    .expect("CDN-Cache-Control metadata should parse")
    .expect("CDN-Cache-Control should be present");

  assert_eq!(metadata.len(), 4);
  assert_eq!(metadata.directives()[0].name(), "max-age");
  assert_eq!(metadata.directives()[0].value(), Some("600"));
  assert_eq!(metadata.directives()[2].name(), "cdn-example");
  assert_eq!(metadata.directives()[2].value(), Some("a, b"));
  assert_eq!(metadata.directives()[3].name(), "immutable");
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_cdn_cache_control_helper_rejects_invalid_and_bounded_metadata() {
  assert_cdn_cache_control_helper_rejects_but_preserves_response(
    "invalid CDN-Cache-Control directive",
    cdn_cache_control_response(&["max-age="]),
  );
  assert_cdn_cache_control_helper_rejects_but_preserves_response(
    "too many CDN-Cache-Control directives",
    cdn_cache_control_response(&[&fixtures::cache_control::too_many_directives_value()]),
  );
  assert_cdn_cache_control_helper_rejects_but_preserves_response(
    "oversized CDN-Cache-Control value",
    cdn_cache_control_response(&[&fixtures::cache_control::oversized_value()]),
  );
}

#[test]
fn sync_client_preserves_new_metadata_headers_without_applying_policy() {
  const HEADERS: &[(&str, &str)] = &[
    (
      "Accept-Patch",
      "application/json, application/merge-patch+json",
    ),
    ("Accept-Post", "application/json"),
    ("Accept-CH", "Sec-CH-UA, Sec-CH-UA-Platform"),
    ("Critical-CH", "Sec-CH-UA-Platform"),
    ("Clear-Site-Data", "\"cache\", \"cookies\""),
    (
      "Reporting-Endpoints",
      "default=\"https://reports.example/default\"",
    ),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/new-metadata", addr))
    .emit()
    .expect("metadata response should parse without client policy handling");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  for (name, value) in HEADERS {
    assert_eq!(
      Some(&value.to_string()),
      response.header_value(name),
      "{name}"
    );
  }

  handle.join().expect("metadata response server thread");
}

#[test]
fn sync_client_preserves_semantically_malformed_new_metadata_headers() {
  const HEADERS: &[(&str, &str)] = &[
    ("Accept-Patch", "application/json; q=bogus"),
    ("Accept-Post", "not a media type"),
    ("Accept-CH", "Sec-CH-UA,, Sec-CH-UA-Platform"),
    ("Critical-CH", "Sec-CH-UA,,"),
    ("Clear-Site-Data", "cache"),
    (
      "Reporting-Endpoints",
      "default=https://reports.example/default",
    ),
  ];
  let (addr, handle) = spawn_metadata_response_server(HEADERS);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/new-metadata-malformed", addr))
    .emit()
    .expect("malformed metadata should not prevent response parsing");

  assert_eq!(200, response.code());
  assert_eq!("OK", response.body().string().unwrap());
  for (name, value) in HEADERS {
    assert_eq!(
      Some(&value.to_string()),
      response.header_value(name),
      "{name}"
    );
  }

  handle.join().expect("metadata response server thread");
}

impl ConditionalHeader {
  fn apply(&self, client: &mut HttpClient) {
    match self {
      Self::IfNoneMatch(value) => {
        client
          .if_none_match(value)
          .expect("If-None-Match helper should accept test validator");
      }
      Self::IfMatch(value) => {
        client
          .if_match(value)
          .expect("If-Match helper should accept test validator");
      }
      Self::IfModifiedSince(value) => {
        client
          .if_modified_since(value)
          .expect("If-Modified-Since helper should accept test date");
      }
      Self::IfUnmodifiedSince(value) => {
        client
          .if_unmodified_since(value)
          .expect("If-Unmodified-Since helper should accept test date");
      }
      Self::Manual(name, value) => {
        client.header((*name, *value));
      }
    }
  }
}

struct ConditionalCase {
  name: &'static str,
  method: &'static str,
  header: ConditionalHeader,
  expected_validator: &'static str,
  expected_code: u32,
  expected_body: &'static str,
}

const ORDINARY_200_FRAMING_CASES: &[(&str, &[u8], &str, &str, &str)] = &[
  (
    "200 content-length",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes(),
    "Content-Length",
    "2",
    "OK",
  ),
  (
    "200 chunked",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nchunked\r\n",
      "6\r\n body!\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
    "Transfer-Encoding",
    "chunked",
    "chunked body!",
  ),
];

#[test]
fn sync_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  let response = client()
    .get()
    .url(format!("http://{}/matrix/chunked", addr))
    .emit()
    .expect("sync response should parse");

  assert_eq!(200, response.code());
  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
  assert_eq!(
    Some(&"signed".to_string()),
    response.trailer_value("X-SIGNATURE")
  );

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
fn sync_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/no-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(*status, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!("", response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/ordinary-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(200, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!(*body, response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  let error = client()
    .get()
    .url(format!("http://{}/matrix/ambiguous", addr))
    .emit()
    .expect_err("ambiguous response should be rejected");

  assert!(
    error.to_string().contains("Content-Length"),
    "unexpected error: {error}"
  );
  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_parses_shared_expect_continue_after_sending_the_body() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  let response = client()
    .post()
    .url(format!("http://{}{}", addr, fixture.target))
    .expect_continue()
    .raw(String::from_utf8_lossy(fixture.body).as_ref())
    .emit()
    .expect("expect-continue response should parse");

  assert_eq!(200, response.code());
  assert_eq!("accepted", response.body().string().unwrap());

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}

#[test]
fn sync_client_range_helpers_interoperate_with_server_partial_content_helper() {
  for (name, expected_range, expected_content_range, expected_body, request) in [
    (
      "bounded range",
      "bytes=3-7",
      "bytes 3-7/16",
      "34567",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted");
      }) as Box<dyn Fn(&mut HttpClient)>,
    ),
    (
      "open-ended range",
      "bytes=12-",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client.range_from(12);
      }),
    ),
    (
      "suffix range",
      "bytes=-4",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_suffix(4)
          .expect("suffix range should be accepted");
      }),
    ),
  ] {
    let (addr, handle) = spawn_range_server();
    let mut client = client();
    client.get().url(format!("http://{}/matrix/range", addr));
    request(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_partial_response(name, response, expected_content_range, expected_body);
    assert_observed_range(handle, expected_range, name);
  }
}

#[test]
fn sync_client_if_range_helpers_interoperate_with_server_range_validator_evaluation() {
  for (name, metadata, expected_range, expected_if_range, expected_code, expected_body, request) in [
    (
      "matching strong ETag returns partial content",
      conditional_metadata(),
      "bytes=3-7",
      r#""abc""#,
      206,
      "34567",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""abc""#)
          .expect("matching strong etag should be accepted");
      }) as Box<dyn Fn(&mut HttpClient)>,
    ),
    (
      "non-matching strong ETag falls back to full response",
      conditional_metadata(),
      "bytes=3-7",
      r#""other""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""other""#)
          .expect("non-matching strong etag should be accepted");
      }),
    ),
    (
      "matching HTTP-date returns partial content",
      conditional_metadata(),
      "bytes=12-",
      CONDITIONAL_LAST_MODIFIED,
      206,
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(12)
          .if_range_date(CONDITIONAL_LAST_MODIFIED)
          .expect("matching date should be accepted");
      }),
    ),
    (
      "stale HTTP-date falls back to full response",
      conditional_metadata(),
      "bytes=12-",
      CONDITIONAL_STALE_DATE,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(12)
          .if_range_date(CONDITIONAL_STALE_DATE)
          .expect("stale date should be accepted");
      }),
    ),
    (
      "missing metadata falls back to full response",
      HttpConditionalMetadata::new(),
      "bytes=3-7",
      r#""abc""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""abc""#)
          .expect("strong etag should be accepted");
      }),
    ),
    (
      "matching validator preserves unsatisfied range response",
      conditional_metadata(),
      "bytes=16-",
      r#""abc""#,
      416,
      "",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(RANGE_BODY.len() as u64)
          .if_range_etag(r#""abc""#)
          .expect("matching strong etag should be accepted");
      }),
    ),
    (
      "manual If-Range header remains available",
      conditional_metadata(),
      "bytes=3-7",
      r#"W/"abc""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .header(("If-Range", r#"W/"abc""#));
      }),
    ),
  ] {
    let (addr, handle) = spawn_if_range_server(metadata);
    let mut client = client();
    client.get().url(format!("http://{}/matrix/if-range", addr));
    request(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(expected_code, response.code(), "{name}");
    assert_eq!(expected_body, response.body().string().unwrap(), "{name}");
    if expected_code == 206 {
      assert!(response.is_partial_content(), "{name}");
    }
    if expected_code == 416 {
      assert!(response.is_range_not_satisfiable(), "{name}");
      assert_eq!(
        Some(&format!("bytes */{}", RANGE_BODY.len())),
        response.header_value("Content-Range"),
        "{name}"
      );
    }
    assert_observed_if_range(handle, expected_range, expected_if_range, name);
  }
}

#[test]
fn sync_client_conditional_helpers_interoperate_with_server_validator_evaluation() {
  let cases = [
    ConditionalCase {
      name: "GET If-None-Match strong match returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#""abc""#),
      expected_validator: r#"If-None-Match: "abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-None-Match weak match returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#"W/"abc""#),
      expected_validator: r#"If-None-Match: W/"abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-None-Match miss proceeds",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#""different""#),
      expected_validator: r#"If-None-Match: "different""#,
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-None-Match wildcard returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch("*"),
      expected_validator: "If-None-Match: *",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "PUT If-None-Match wildcard returns 412",
      method: "PUT",
      header: ConditionalHeader::IfNoneMatch("*"),
      expected_validator: "If-None-Match: *",
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match strong match proceeds",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#""abc""#),
      expected_validator: r#"If-Match: "abc""#,
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Match weak comparison miss returns 412",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#"W/"abc""#),
      expected_validator: r#"If-Match: W/"abc""#,
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match non-match returns 412",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#""different""#),
      expected_validator: r#"If-Match: "different""#,
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match wildcard proceeds",
      method: "GET",
      header: ConditionalHeader::IfMatch("*"),
      expected_validator: "If-Match: *",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Modified-Since fresh returns 304",
      method: "GET",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Modified-Since stale proceeds",
      method: "GET",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_STALE_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:36 GMT",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Unmodified-Since stale returns 412",
      method: "GET",
      header: ConditionalHeader::IfUnmodifiedSince(CONDITIONAL_STALE_DATE),
      expected_validator: "If-Unmodified-Since: Sun, 06 Nov 1994 08:49:36 GMT",
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Unmodified-Since fresh proceeds",
      method: "GET",
      header: ConditionalHeader::IfUnmodifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Unmodified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "HEAD If-None-Match match returns bodyless 304",
      method: "HEAD",
      header: ConditionalHeader::IfNoneMatch(r#""abc""#),
      expected_validator: r#"If-None-Match: "abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "HEAD If-Modified-Since fresh returns bodyless 304",
      method: "HEAD",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "manual If-None-Match list remains available",
      method: "GET",
      header: ConditionalHeader::Manual("If-None-Match", r#""different", "abc""#),
      expected_validator: r#"If-None-Match: "different", "abc""#,
      expected_code: 304,
      expected_body: "",
    },
  ];

  for case in cases {
    let (addr, handle) = spawn_conditional_server();
    let mut client = client();
    client
      .method(case.method)
      .url(format!("http://{}/matrix/conditional", addr));
    case.header.apply(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{} should parse: {err}", case.name));

    assert_eq!(case.expected_code, response.code(), "{}", case.name);
    assert_eq!(
      case.expected_body,
      response.body().string().unwrap(),
      "{}",
      case.name
    );
    assert_eq!(
      Some(case.expected_validator.to_string()),
      handle.join().expect("conditional server thread"),
      "{}",
      case.name
    );
  }
}

#[test]
fn sync_client_unsatisfied_range_maps_to_server_416_response() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_from(RANGE_BODY.len() as u64)
    .emit()
    .expect("unsatisfied range response should parse");

  assert_eq!(416, response.code());
  assert!(response.is_range_not_satisfiable());
  assert_eq!(
    Some(&format!("bytes */{}", RANGE_BODY.len())),
    response.header_value("Content-Range")
  );
  assert_eq!("", response.body().string().unwrap());
  assert_observed_range(
    handle,
    &format!("bytes={}-", RANGE_BODY.len()),
    "unsatisfied range",
  );
}

#[test]
fn sync_client_malformed_range_helpers_reject_before_reaching_server() {
  let server = rttp_server::server::HttpServer::bind("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send unexpected range");
        HttpResponse::ok("unexpected")
      })
      .expect("serve optional range request");
  });

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range(7, 3)
    .expect_err("inverted range should be rejected");
  assert!(error.is_builder());

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_suffix(0)
    .expect_err("empty suffix should be rejected");
  assert!(error.is_builder());

  assert!(
    rx.recv_timeout(Duration::from_millis(100)).is_err(),
    "malformed helper input should not reach the range server"
  );

  let mut stream = TcpStream::connect(addr).expect("release range server");
  stream
    .write_all(b"GET /matrix/release HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    .expect("write release request");
  assert_eq!(None, rx.recv().expect("release request observed range"));
  handle.join().expect("range server thread");
}

#[test]
fn sync_client_manual_range_header_interoperates_with_server_partial_content_helper() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .header(("Range", "bytes=5-9"))
    .emit()
    .expect("manual range response should parse");

  assert_partial_response("manual range", response, "bytes 5-9/16", "56789");
  assert_observed_range(handle, "bytes=5-9", "manual range");
}

#[test]
#[cfg(feature = "async")]
fn async_client_preserves_shared_informational_response_matrix() {
  for case in fixtures::response::informational_response_cases() {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(case.raw);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/informational", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

      assert_eq!(case.final_status, response.code(), "{}", case.name);
      assert_eq!(case.final_reason, response.reason(), "{}", case.name);
      assert_eq!(
        Some(&case.final_marker.to_string()),
        response.header_value("X-Final"),
        "{}",
        case.name
      );
      assert_eq!(
        case.final_body,
        response.body().string().unwrap(),
        "{}",
        case.name
      );
      assert_eq!(
        case.informational.len(),
        response.informational_responses().len(),
        "{}",
        case.name
      );
      for (observed, expected) in response
        .informational_responses()
        .iter()
        .zip(case.informational)
      {
        assert_informational_response(case.name, observed, expected);
      }
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_rejects_shared_malformed_informational_heads() {
  for case in fixtures::response::malformed_informational_response_cases() {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(case.raw);

    block_on(async {
      let error = client()
        .get()
        .url(format!("http://{}/matrix/informational-invalid", addr))
        .rasync()
        .await
        .expect_err(case.name);

      assert!(
        error.to_string().contains(case.error_contains),
        "{} unexpected error: {error}",
        case.name
      );
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_rejects_shared_oversized_informational_head() {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(
    fixtures::response::oversized_informational_response(),
  );

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/matrix/informational-oversized", addr))
      .rasync()
      .await
      .expect_err("oversized informational response should be rejected");

    assert!(
      error
        .to_string()
        .contains("HTTP informational response head is too large"),
      "unexpected error: {error}"
    );
  });

  handle.join().expect("raw response server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_client_keeps_shared_101_handoff_separate_from_informational_history() {
  let (addr, handle) =
    fixtures::spawn_socket2_raw_response_server(fixtures::response::SWITCHING_PROTOCOLS_HANDOFF);

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/matrix/upgrade", addr))
      .rasync()
      .await
      .expect("101 response should parse as the final response");

    assert_eq!(101, response.code());
    assert_eq!("Switching Protocols", response.reason());
    assert!(response.informational_responses().is_empty());
    assert_eq!(
      Some(&"websocket".to_string()),
      response.header_value("Upgrade")
    );
    assert_eq!("", response.body().string().unwrap());
  });

  handle.join().expect("raw response server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/matrix/chunked", addr))
      .rasync()
      .await
      .expect("async response should parse");

    assert_eq!(200, response.code());
    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
    assert_eq!(
      Some(&"signed".to_string()),
      response.trailer_value("X-SIGNATURE")
    );
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
#[cfg(feature = "async")]
fn async_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/no-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(*status, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!("", response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/ordinary-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(200, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!(*body, response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/matrix/ambiguous", addr))
      .rasync()
      .await
      .expect_err("ambiguous response should be rejected");

    assert!(
      error.to_string().contains("Content-Length"),
      "unexpected error: {error}"
    );
  });

  handle.join().expect("raw response server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_client_parses_shared_expect_continue_after_sending_the_body() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}{}", addr, fixture.target))
      .expect_continue()
      .raw(String::from_utf8_lossy(fixture.body).as_ref())
      .rasync()
      .await
      .expect("expect-continue response should parse");

    assert_eq!(200, response.code());
    assert_eq!("accepted", response.body().string().unwrap());
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}
