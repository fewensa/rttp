#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_client::response::Response;
use rttp_client::HttpClient;
use rttp_server::server::{HttpResponse, Request};
use rttp_test_support as fixtures;

const BODY: &str = "content-negotiation-metadata";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
struct ObservedAcceptCharset {
  ranges: Result<Option<Vec<(String, u16)>>, String>,
  raw: Option<String>,
}

#[derive(Debug, PartialEq)]
struct ObservedAcceptEncoding {
  codings: Result<Option<Vec<(String, u16)>>, String>,
  raw: Option<String>,
}

#[derive(Debug, PartialEq)]
struct ObservedAcceptLanguage {
  ranges: Result<Option<Vec<String>>, String>,
  qualities: Result<Option<Vec<Option<String>>>, String>,
  raw: Option<String>,
}

fn client() -> HttpClient {
  rttp::Http::client()
}

fn bind_facade_server() -> rttp_server::server::HttpServer {
  rttp::Http::server("127.0.0.1:0")
    .expect("bind content-negotiation facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT))
}

fn spawn_observed_facade_server<T, F>(
  observe: F,
  response: impl Fn(Request) -> HttpResponse + Send + 'static,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<T>,
  thread::JoinHandle<()>,
)
where
  T: Send + 'static,
  F: Fn(&Request) -> T + Send + 'static,
{
  let server = bind_facade_server();
  let addr = server.local_addr().expect("facade server addr");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe(&request))
          .expect("send observed content-negotiation metadata");
        response(request)
      })
      .expect("serve content-negotiation metadata request");
  });
  (addr, observed_rx, handle)
}

fn observe_accept_charset(request: &Request) -> ObservedAcceptCharset {
  ObservedAcceptCharset {
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

fn observe_accept_encoding(request: &Request) -> ObservedAcceptEncoding {
  ObservedAcceptEncoding {
    codings: request
      .accept_encoding()
      .map(|encodings| {
        encodings.map(|encodings| {
          encodings
            .codings()
            .iter()
            .map(|coding| (coding.coding().to_owned(), coding.quality()))
            .collect()
        })
      })
      .map_err(|error| error.to_string()),
    raw: request.header("Accept-Encoding").map(str::to_owned),
  }
}

fn observe_accept_language(request: &Request) -> ObservedAcceptLanguage {
  ObservedAcceptLanguage {
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
    raw: request.header("Accept-Language").map(str::to_owned),
  }
}

fn negotiation_response(body: &'static str) -> HttpResponse {
  HttpResponse::ok(body)
    .with_vary("Accept-Encoding, Accept-Language, Accept-Charset")
    .expect("response Vary should be accepted")
    .with_content_language(["en-US", "fr-CA"])
    .expect("response Content-Language should be accepted")
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

  let vary = response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present");
  assert!(!vary.is_any());
  assert_eq!(
    ["accept-encoding", "accept-language", "accept-charset"],
    vary.field_names().as_slice()
  );
  assert_eq!(
    Some("accept-encoding, accept-language, accept-charset"),
    response.header_value("Vary").map(String::as_str)
  );

  let content_language = response
    .content_language()
    .expect("Content-Language should parse")
    .expect("Content-Language should be present");
  assert_eq!(["en-US", "fr-CA"], content_language.tags().as_slice());
  assert_eq!(
    Some("en-US, fr-CA"),
    response
      .header_value("Content-Language")
      .map(String::as_str)
  );
}

fn write_raw_request(addr: std::net::SocketAddr, request: &[u8]) {
  let mut stream = TcpStream::connect(addr).expect("connect raw content-negotiation request");
  stream
    .set_read_timeout(Some(TIMEOUT))
    .expect("set raw request read timeout");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set raw request write timeout");
  stream
    .write_all(request)
    .expect("write raw content-negotiation request");
  let mut response = Vec::new();
  let _ = stream.read_to_end(&mut response);
}

fn reject_before_connect<F>(name: &str, mutate: F)
where
  F: FnOnce(&mut HttpClient) -> Result<&mut HttpClient, rttp_client::error::Error>,
{
  let mut client = client();
  let error = mutate(client.get().url("http://127.0.0.1:9/unreachable")).expect_err(name);
  assert!(
    error.is_builder(),
    "{name} should fail as a builder error before connect: {error}"
  );
}

#[test]
fn public_facade_exports_content_negotiation_metadata_types() {
  let accept_charset: rttp::AcceptCharset =
    rttp::AcceptCharset::parse("utf-8, iso-8859-1;q=0.5, *;q=0")
      .expect("Accept-Charset facade type should parse");
  assert_eq!(
    "utf-8, iso-8859-1;q=0.5, *;q=0",
    accept_charset.header_value()
  );

  let accept_encoding: rttp::AcceptEncoding =
    rttp::AcceptEncoding::parse("gzip, br;q=0.8, identity;q=0")
      .expect("Accept-Encoding facade type should parse");
  assert_eq!(
    "gzip, br;q=0.8, identity;q=0",
    accept_encoding.header_value()
  );

  let accept_language = rttp_server::server::HttpAcceptLanguages::parse("en-US, fr-CA; q=0.8, *")
    .expect("Accept-Language server facade type should parse");
  assert_eq!(["en-US", "fr-CA", "*"], accept_language.ranges().as_slice());

  let content_language: rttp::ContentLanguage = rttp::ContentLanguage::parse("en-US, fr-CA")
    .expect("Content-Language facade type should parse");
  assert_eq!(["en-US", "fr-CA"], content_language.tags().as_slice());

  let vary = rttp_client::response::Vary::parse("Accept-Encoding, User-Agent")
    .expect("Vary client facade type should parse");
  assert_eq!(
    ["accept-encoding", "user-agent"],
    vary.field_names().as_slice()
  );

  let server_vary =
    rttp_server::server::HttpVary::parse("*").expect("Vary server type should parse");
  assert!(server_vary.is_wildcard());
}

#[test]
fn request_builders_exchange_accept_charset_quality_wildcards_and_order() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(observe_accept_charset, |_| HttpResponse::ok("OK"));

  let response = client()
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
    ObservedAcceptCharset {
      ranges: Ok(Some(vec![
        ("utf-8".to_owned(), 1000),
        ("iso-8859-1".to_owned(), 500),
        ("*".to_owned(), 0),
      ])),
      raw: Some("utf-8, iso-8859-1;q=0.5, *;q=0".to_owned()),
    },
    observed_rx
      .recv_timeout(TIMEOUT)
      .expect("server should observe Accept-Charset metadata")
  );
  handle.join().expect("Accept-Charset server thread");
}

#[test]
fn request_builders_exchange_accept_encoding_quality_and_order() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(observe_accept_encoding, |_| HttpResponse::ok("OK"));

  let response = client()
    .get()
    .url(format!("http://{addr}/compressed"))
    .accept_gzip()
    .expect("gzip should be accepted")
    .accept_br_with_q("0.8")
    .expect("br quality should be accepted")
    .accept_identity_with_q("0")
    .expect("identity quality should be accepted")
    .emit()
    .expect("Accept-Encoding request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptEncoding {
      codings: Ok(Some(vec![
        ("gzip".to_owned(), 1000),
        ("br".to_owned(), 800),
        ("identity".to_owned(), 0),
      ])),
      raw: Some("gzip, br;q=0.8, identity;q=0".to_owned()),
    },
    observed_rx
      .recv_timeout(TIMEOUT)
      .expect("server should observe Accept-Encoding metadata")
  );
  handle.join().expect("Accept-Encoding server thread");
}

#[test]
fn request_builders_exchange_accept_language_quality_wildcards_and_order() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(observe_accept_language, |_| HttpResponse::ok("OK"));

  let response = client()
    .get()
    .url(format!("http://{addr}/localized"))
    .accept_language(["en-US", "fr-CA; q=0.8", "*"])
    .expect("language ranges should be accepted")
    .emit()
    .expect("Accept-Language request should succeed");

  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptLanguage {
      ranges: Ok(Some(vec![
        "en-US".to_owned(),
        "fr-CA".to_owned(),
        "*".to_owned(),
      ])),
      qualities: Ok(Some(vec![None, Some("0.8".to_owned()), None])),
      raw: Some("en-US, fr-CA; q=0.8, *".to_owned()),
    },
    observed_rx
      .recv_timeout(TIMEOUT)
      .expect("server should observe Accept-Language metadata")
  );
  handle.join().expect("Accept-Language server thread");
}

#[test]
fn request_accessors_combine_ordered_repeated_accept_fields() {
  let (addr, charset_rx, charset_handle) =
    spawn_observed_facade_server(observe_accept_charset, |_| HttpResponse::ok("charset"));
  write_raw_request(
    addr,
    b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Charset: utf-8, iso-8859-1;q=0.5\r\naccept-charset: *; q=0\r\nConnection: close\r\n\r\n",
  );
  assert_eq!(
    ObservedAcceptCharset {
      ranges: Ok(Some(vec![
        ("utf-8".to_owned(), 1000),
        ("iso-8859-1".to_owned(), 500),
        ("*".to_owned(), 0),
      ])),
      raw: Some("utf-8, iso-8859-1;q=0.5".to_owned()),
    },
    charset_rx
      .recv_timeout(TIMEOUT)
      .expect("observe repeated Accept-Charset")
  );
  charset_handle
    .join()
    .expect("repeated Accept-Charset thread");

  let (addr, encoding_rx, encoding_handle) =
    spawn_observed_facade_server(observe_accept_encoding, |_| HttpResponse::ok("encoding"));
  write_raw_request(
    addr,
    b"GET /compressed HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Encoding: gzip, br;q=0.8\r\naccept-encoding: identity; q=0\r\nConnection: close\r\n\r\n",
  );
  assert_eq!(
    ObservedAcceptEncoding {
      codings: Ok(Some(vec![
        ("gzip".to_owned(), 1000),
        ("br".to_owned(), 800),
        ("identity".to_owned(), 0),
      ])),
      raw: Some("gzip, br;q=0.8".to_owned()),
    },
    encoding_rx
      .recv_timeout(TIMEOUT)
      .expect("observe repeated Accept-Encoding")
  );
  encoding_handle
    .join()
    .expect("repeated Accept-Encoding thread");

  let (addr, language_rx, language_handle) =
    spawn_observed_facade_server(observe_accept_language, |_| HttpResponse::ok("language"));
  write_raw_request(
    addr,
    b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Language: en-US, fr-CA; q=0.8\r\naccept-language: *\r\nConnection: close\r\n\r\n",
  );
  assert_eq!(
    ObservedAcceptLanguage {
      ranges: Ok(Some(vec![
        "en-US".to_owned(),
        "fr-CA".to_owned(),
        "*".to_owned(),
      ])),
      qualities: Ok(Some(vec![None, Some("0.8".to_owned()), None])),
      raw: Some("en-US, fr-CA; q=0.8".to_owned()),
    },
    language_rx
      .recv_timeout(TIMEOUT)
      .expect("observe repeated Accept-Language")
  );
  language_handle
    .join()
    .expect("repeated Accept-Language thread");
}

#[test]
fn request_accessors_report_absent_accept_metadata() {
  let (addr, charset_rx, charset_handle) =
    spawn_observed_facade_server(observe_accept_charset, |_| HttpResponse::ok("OK"));
  let response = client()
    .get()
    .url(format!("http://{addr}/plain"))
    .emit()
    .expect("absent Accept-Charset request should succeed");
  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptCharset {
      ranges: Ok(None),
      raw: None,
    },
    charset_rx
      .recv_timeout(TIMEOUT)
      .expect("observe absent Accept-Charset")
  );
  charset_handle.join().expect("absent Accept-Charset thread");

  let (addr, encoding_rx, encoding_handle) =
    spawn_observed_facade_server(observe_accept_encoding, |_| HttpResponse::ok("OK"));
  let response = client()
    .get()
    .url(format!("http://{addr}/plain"))
    .emit()
    .expect("absent Accept-Encoding request should succeed");
  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptEncoding {
      codings: Ok(None),
      raw: None,
    },
    encoding_rx
      .recv_timeout(TIMEOUT)
      .expect("observe absent Accept-Encoding")
  );
  encoding_handle
    .join()
    .expect("absent Accept-Encoding thread");

  let (addr, language_rx, language_handle) =
    spawn_observed_facade_server(observe_accept_language, |_| HttpResponse::ok("OK"));
  let response = client()
    .get()
    .url(format!("http://{addr}/plain"))
    .emit()
    .expect("absent Accept-Language request should succeed");
  assert_eq!("OK", response.body().string().expect("response body"));
  assert_eq!(
    ObservedAcceptLanguage {
      ranges: Ok(None),
      qualities: Ok(None),
      raw: None,
    },
    language_rx
      .recv_timeout(TIMEOUT)
      .expect("observe absent Accept-Language")
  );
  language_handle
    .join()
    .expect("absent Accept-Language thread");
}

#[test]
fn request_accessors_reject_malformed_accept_metadata_without_losing_raw_headers() {
  let (addr, charset_rx, charset_handle) =
    spawn_observed_facade_server(observe_accept_charset, |_| HttpResponse::ok("malformed"));
  write_raw_request(
    addr,
    b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Charset: utf-8, UTF-8\r\nConnection: close\r\n\r\n",
  );
  let observed = charset_rx
    .recv_timeout(TIMEOUT)
    .expect("observe malformed Accept-Charset");
  assert!(observed.ranges.is_err());
  assert_eq!(observed.raw.as_deref(), Some("utf-8, UTF-8"));
  charset_handle
    .join()
    .expect("malformed Accept-Charset thread");

  let (addr, encoding_rx, encoding_handle) =
    spawn_observed_facade_server(observe_accept_encoding, |_| HttpResponse::ok("malformed"));
  write_raw_request(
    addr,
    b"GET /compressed HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Encoding: gzip, GZIP\r\nConnection: close\r\n\r\n",
  );
  let observed = encoding_rx
    .recv_timeout(TIMEOUT)
    .expect("observe malformed Accept-Encoding");
  assert!(observed.codings.is_err());
  assert_eq!(observed.raw.as_deref(), Some("gzip, GZIP"));
  encoding_handle
    .join()
    .expect("malformed Accept-Encoding thread");

  let (addr, language_rx, language_handle) =
    spawn_observed_facade_server(observe_accept_language, |_| HttpResponse::ok("malformed"));
  write_raw_request(
    addr,
    b"GET /localized HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept-Language: en; q=1.001, EN\r\nConnection: close\r\n\r\n",
  );
  let observed = language_rx
    .recv_timeout(TIMEOUT)
    .expect("observe malformed Accept-Language");
  assert!(observed.ranges.is_err());
  assert!(observed.qualities.is_err());
  assert_eq!(observed.raw.as_deref(), Some("en; q=1.001, EN"));
  language_handle
    .join()
    .expect("malformed Accept-Language thread");
}

#[test]
fn request_builders_reject_invalid_and_oversized_accept_metadata_before_connect() {
  reject_before_connect("invalid Accept-Charset member", |client| {
    client.accept_charset("utf 8")
  });
  reject_before_connect("invalid Accept-Charset q-value", |client| {
    client.accept_charset_with_q("utf-8", "1.1")
  });
  reject_before_connect("duplicate Accept-Charset", |client| {
    client.accept_charset("utf-8")?.accept_charset("UTF-8")
  });
  reject_before_connect("invalid Accept-Encoding member", |client| {
    client.accept_encoding("bad coding")
  });
  reject_before_connect("invalid Accept-Encoding q-value", |client| {
    client.accept_encoding_with_q("gzip", "1.1")
  });
  reject_before_connect("duplicate Accept-Encoding", |client| {
    client.accept_gzip()?.accept_encoding("GZIP")
  });
  reject_before_connect("invalid Accept-Language range", |client| {
    client.accept_language(["en_US"])
  });
  reject_before_connect("invalid Accept-Language q-value", |client| {
    client.accept_language(["en; q=1.001"])
  });

  let too_many_charsets = (0..33)
    .map(|index| format!("charset{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::AcceptCharset::parse(&too_many_charsets).is_err(),
    "configured Accept-Charset member bound must reject overflow"
  );
  assert!(
    rttp::AcceptCharset::parse("utf-8".repeat(64 * 1024 + 1)).is_err(),
    "configured Accept-Charset size bound must reject oversized values"
  );

  let too_many_codings = (0..33)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::AcceptEncoding::parse(&too_many_codings).is_err(),
    "configured Accept-Encoding member bound must reject overflow"
  );
  assert!(
    rttp::AcceptEncoding::parse("gzip".repeat(64 * 1024 + 1)).is_err(),
    "configured Accept-Encoding size bound must reject oversized values"
  );

  let too_many_languages = (0..33)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp_server::server::HttpAcceptLanguages::parse(&too_many_languages).is_err(),
    "configured Accept-Language member bound must reject overflow"
  );
}

#[test]
fn sync_response_builders_exchange_vary_and_content_language_metadata() {
  let (addr, _observed_rx, handle) =
    spawn_observed_facade_server(|_| (), |_| negotiation_response(BODY));

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("sync content-negotiation response should parse");

  assert_valid_response_metadata(&response);
  handle.join().expect("sync response metadata server thread");
}

#[cfg(feature = "async")]
#[test]
fn async_response_builders_exchange_vary_and_content_language_metadata() {
  let (addr, _observed_rx, handle) =
    spawn_observed_facade_server(|_| (), |_| negotiation_response(BODY));

  let response = block_on(async {
    client()
      .get()
      .url(format!("http://{addr}/asset"))
      .rasync()
      .await
      .expect("async content-negotiation response should parse")
  });

  assert_valid_response_metadata(&response);
  handle
    .join()
    .expect("async response metadata server thread");
}

#[test]
fn sync_and_async_clients_parse_shared_vary_fixture_matrix() {
  for case in fixtures::vary::response_cases() {
    let mut response = HttpResponse::ok("OK");
    for value in case.values {
      response = response.header("Vary", *value);
    }
    let (addr, _observed_rx, handle) =
      spawn_observed_facade_server(|_| (), move |_| response.clone());

    let sync_response = client()
      .get()
      .url(format!("http://{addr}/matrix/vary"))
      .emit()
      .unwrap_or_else(|err| panic!("{} sync Vary should parse: {err}", case.name));
    assert_response_vary(case.name, &sync_response, case);
    handle.join().expect("sync Vary fixture server thread");

    #[cfg(feature = "async")]
    {
      let mut response = HttpResponse::ok("OK");
      for value in case.values {
        response = response.header("Vary", *value);
      }
      let (addr, _observed_rx, handle) =
        spawn_observed_facade_server(|_| (), move |_| response.clone());
      let async_response = block_on(async {
        client()
          .get()
          .url(format!("http://{addr}/matrix/vary"))
          .rasync()
          .await
          .unwrap_or_else(|err| panic!("{} async Vary should parse: {err}", case.name))
      });
      assert_response_vary(case.name, &async_response, case);
      handle.join().expect("async Vary fixture server thread");
    }
  }
}

#[test]
fn sync_and_async_clients_parse_shared_content_language_fixture_matrix() {
  for case in fixtures::content_language::response_cases() {
    let response = HttpResponse::ok("OK")
      .with_content_language(case.languages)
      .unwrap_or_else(|err| {
        panic!(
          "{} Content-Language declaration should be accepted: {err}",
          case.name
        )
      });
    let (addr, _observed_rx, handle) =
      spawn_observed_facade_server(|_| (), move |_| response.clone());

    let sync_response = client()
      .get()
      .url(format!("http://{addr}/matrix/content-language"))
      .emit()
      .unwrap_or_else(|err| panic!("{} sync Content-Language should parse: {err}", case.name));
    assert_response_content_language(case.name, &sync_response, case);
    handle
      .join()
      .expect("sync Content-Language fixture server thread");

    #[cfg(feature = "async")]
    {
      let response = HttpResponse::ok("OK")
        .with_content_language(case.languages)
        .unwrap_or_else(|err| {
          panic!(
            "{} Content-Language declaration should be accepted: {err}",
            case.name
          )
        });
      let (addr, _observed_rx, handle) =
        spawn_observed_facade_server(|_| (), move |_| response.clone());
      let async_response = block_on(async {
        client()
          .get()
          .url(format!("http://{addr}/matrix/content-language"))
          .rasync()
          .await
          .unwrap_or_else(|err| panic!("{} async Content-Language should parse: {err}", case.name))
      });
      assert_response_content_language(case.name, &async_response, case);
      handle
        .join()
        .expect("async Content-Language fixture server thread");
    }
  }
}

#[test]
fn response_accessors_report_absent_vary_and_content_language() {
  let (addr, _observed_rx, handle) =
    spawn_observed_facade_server(|_| (), |_| HttpResponse::ok("absent"));

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("absent response metadata should parse");

  assert!(response
    .vary()
    .expect("absent Vary should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Vary"));
  assert!(response
    .content_language()
    .expect("absent Content-Language should parse as None")
    .is_none());
  assert_eq!(None, response.header_value("Content-Language"));
  handle
    .join()
    .expect("absent response metadata server thread");
}

#[test]
fn response_accessors_reject_malformed_vary_and_content_language_while_preserving_raw() {
  let (addr, _observed_rx, handle) = spawn_observed_facade_server(
    |_| (),
    |_| {
      HttpResponse::ok("invalid-response")
        .header("Vary", "Accept Encoding")
        .header("Content-Language", "en_US")
    },
  );

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("invalid response metadata headers should remain observable");

  assert!(response.vary().is_err());
  assert_eq!(
    Some("Accept Encoding"),
    response.header_value("Vary").map(String::as_str)
  );
  assert!(response.content_language().is_err());
  assert_eq!(
    Some("en_US"),
    response
      .header_value("Content-Language")
      .map(String::as_str)
  );
  assert_eq!(
    "invalid-response",
    response.body().string().expect("invalid response body")
  );
  handle
    .join()
    .expect("invalid response metadata server thread");
}

#[test]
fn response_builders_reject_malformed_and_oversized_values_without_replacing_fields() {
  let original = HttpResponse::ok("body")
    .header("Vary", "Accept-Encoding")
    .header("Content-Language", "en");

  assert!(original.clone().with_vary("Accept Encoding").is_err());
  assert!(original.clone().with_vary("*, Accept-Encoding").is_err());
  assert!(original.clone().with_content_language(["en_US"]).is_err());
  assert!(original.clone().with_content_language([""]).is_err());

  let serialized = String::from_utf8(original.to_bytes()).expect("response should serialize");
  assert!(serialized.contains("Vary: Accept-Encoding"));
  assert!(serialized.contains("Content-Language: en"));

  assert!(
    HttpResponse::ok("")
      .with_vary(fixtures::vary::oversized_value())
      .is_err(),
    "configured Vary size bound must reject oversized values"
  );
  assert!(
    HttpResponse::ok("")
      .with_vary(fixtures::vary::too_many_field_names_value())
      .is_err(),
    "configured Vary member bound must reject overflow"
  );
  assert!(
    HttpResponse::ok("")
      .with_content_language(
        fixtures::content_language::too_many_server_languages_value()
          .split(", ")
          .collect::<Vec<_>>()
      )
      .is_err(),
    "configured Content-Language member bound must reject overflow"
  );
  assert!(
    rttp::ContentLanguage::parse(fixtures::content_language::oversized_value()).is_err(),
    "configured Content-Language size bound must reject oversized values"
  );
}

#[test]
fn response_accessors_preserve_ordered_repeated_vary_and_content_language_fields() {
  let (addr, _observed_rx, handle) = spawn_observed_facade_server(
    |_| (),
    |_| {
      HttpResponse::ok("repeated")
        .header("Vary", "Accept-Encoding, User-Agent")
        .header("Vary", "accept-language, X-Feature")
        .header("Content-Language", "en-US, fr")
        .header("Content-Language", "zh-Hant-TW, es-419")
    },
  );

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("repeated response metadata should parse");

  assert_eq!(
    ["Accept-Encoding, User-Agent", "accept-language, X-Feature"],
    response
      .header_values("vary")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
      .as_slice()
  );
  let vary = response
    .vary()
    .expect("repeated Vary should parse")
    .expect("repeated Vary should be present");
  assert_eq!(
    [
      "accept-encoding",
      "user-agent",
      "accept-language",
      "x-feature"
    ],
    vary.field_names().as_slice()
  );

  assert_eq!(
    ["en-US, fr", "zh-Hant-TW, es-419"],
    response
      .header_values("content-language")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
      .as_slice()
  );
  let content_language = response
    .content_language()
    .expect("repeated Content-Language should parse")
    .expect("repeated Content-Language should be present");
  assert_eq!(
    ["en-US", "fr", "zh-Hant-TW", "es-419"],
    content_language.tags().as_slice()
  );
  handle
    .join()
    .expect("repeated response metadata server thread");
}

fn assert_response_vary(name: &str, response: &Response, expected: &fixtures::vary::ResponseCase) {
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
}

fn assert_response_content_language(
  name: &str,
  response: &Response,
  expected: &fixtures::content_language::ResponseCase,
) {
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
