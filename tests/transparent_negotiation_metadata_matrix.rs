use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::TcnDirective;
use rttp_client::response::Response;
use rttp_client::types::RoUrl;
use rttp_client::HttpClient;
use rttp_server::server::{
  Http2ServerPolicy, HttpDeltaBase, HttpEntityTag, HttpResponse, HttpServer, Request,
};

const A_IM_REQUEST: &str = "diffe, gzip;q=0.3, identity;q=0;profile=compact";
const NEGOTIATE_REQUEST: &str = "trans, 1.0, feature-x=preview, *";
const ALTERNATES_RESPONSE: &str = r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }, { "/resource.fr.html" 0.8 {type "text/html; charset=utf-8"} {language fr} }"#;
const HTTP11_BODY: &str = "http11-negotiation-metadata";
const H2C_BODY: &str = "h2c-negotiation-metadata";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
struct ObservedNegotiationRequest {
  version: String,
  target: String,
  a_im: Result<Option<String>, String>,
  a_im_quality_order: Option<Vec<(String, u16)>>,
  raw_a_im: Option<String>,
  negotiate: Result<Option<String>, String>,
  raw_negotiate: Option<String>,
}

fn client() -> rttp_client::HttpClient {
  rttp::Http::client()
}

fn bind_facade_server() -> rttp_server::server::HttpServer {
  rttp::Http::server("127.0.0.1:0")
    .expect("bind transparent negotiation facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT))
}

fn observe_request(request: &Request) -> ObservedNegotiationRequest {
  ObservedNegotiationRequest {
    version: request.version().to_string(),
    target: request.target().to_string(),
    a_im: request
      .a_im()
      .map(|a_im| a_im.map(|a_im| a_im.header_value()))
      .map_err(|error| error.to_string()),
    a_im_quality_order: request.a_im().ok().flatten().map(|a_im| {
      a_im
        .members()
        .iter()
        .map(|member| (member.token().to_string(), member.quality()))
        .collect()
    }),
    raw_a_im: request.header("A-IM").map(str::to_string),
    negotiate: request
      .negotiate()
      .map(|negotiate| negotiate.map(|negotiate| negotiate.header_value()))
      .map_err(|error| error.to_string()),
    raw_negotiate: request.header("Negotiate").map(str::to_string),
  }
}

fn negotiation_response(body: &'static str) -> HttpResponse {
  HttpResponse::ok(body)
    .with_im(["diffe", "gzip;profile=compact"])
    .expect("response IM should be accepted")
    .with_delta_base(HttpDeltaBase::new(HttpEntityTag::weak("asset-v7")))
    .with_alternates(ALTERNATES_RESPONSE)
    .expect("response Alternates should be accepted")
    .with_tcn("choice, keep")
    .expect("response TCN should be accepted")
    .with_variant_vary("Accept-Language, Sec-CH-DPR")
    .expect("response Variant-Vary should be accepted")
}

fn spawn_observed_facade_server(
  response: impl Fn(Request) -> HttpResponse + Send + 'static,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedNegotiationRequest>,
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
          .expect("send observed transparent negotiation metadata");
        response(request)
      })
      .expect("serve transparent negotiation metadata request");
  });
  (addr, observed_rx, handle)
}

fn attach_valid_client_metadata(
  client: &mut rttp_client::HttpClient,
) -> &mut rttp_client::HttpClient {
  client
    .a_im("diffe")
    .expect("A-IM diffe should be accepted")
    .a_im_with_q("gzip", "0.3")
    .expect("A-IM gzip q-value should be accepted")
    .a_im_value("identity;q=0;profile=compact")
    .expect("A-IM identity member should be accepted")
    .negotiate("Trans, 1.0, feature-x=preview, *")
    .expect("Negotiate should be accepted")
}

fn assert_no_negotiation_policy(response: &Response) {
  assert_eq!(
    200,
    response.code(),
    "metadata helpers must not change the caller-controlled status"
  );
  assert_ne!(
    226,
    response.code(),
    "metadata helpers must not synthesize 226 IM Used"
  );
  for name in [
    "IM",
    "Delta-Base",
    "Alternates",
    "TCN",
    "Variant-Vary",
    "Vary",
    "Cache-Control",
  ] {
    assert_eq!(
      None,
      response.header_value(name),
      "metadata helpers must not synthesize {name}"
    );
  }
}

fn assert_valid_response_metadata(response: &Response, body: &str) {
  assert_eq!(200, response.code());
  assert_ne!(
    226,
    response.code(),
    "metadata helpers must not synthesize 226 IM Used"
  );
  assert_eq!(
    body,
    response
      .body()
      .string()
      .expect("response body should remain ordinary HTTP bytes")
  );
  assert_eq!(
    None,
    response.header_value("Vary"),
    "metadata helpers must not synthesize Vary"
  );
  assert_eq!(
    None,
    response.header_value("Cache-Control"),
    "metadata helpers must not synthesize Cache-Control"
  );

  let im = response
    .im()
    .expect("IM should parse")
    .expect("IM should be present");
  assert_eq!("diffe, gzip;profile=compact", im.header_value());
  assert_eq!("diffe", im.members()[0].token());
  assert_eq!("gzip", im.members()[1].token());
  assert_eq!("profile", im.members()[1].parameters()[0].name());
  assert_eq!(Some("compact"), im.members()[1].parameters()[0].value());
  assert_eq!(
    Some("diffe, gzip;profile=compact"),
    response.header_value("IM").map(String::as_str)
  );

  let delta_base = response
    .delta_base()
    .expect("Delta-Base should parse")
    .expect("Delta-Base should be present");
  assert_eq!("W/\"asset-v7\"", delta_base.header_value());
  assert!(delta_base.entity_tag().is_weak());
  assert_eq!("asset-v7", delta_base.entity_tag().opaque_tag());
  assert_eq!(
    Some("W/\"asset-v7\""),
    response.header_value("Delta-Base").map(String::as_str)
  );

  let alternates = response
    .alternates()
    .expect("Alternates should parse")
    .expect("Alternates should be present");
  assert_eq!(2, alternates.len());
  assert_eq!("/resource.en.html", alternates.variants()[0].uri());
  assert_eq!("1.0", alternates.variants()[0].quality());
  assert_eq!(Some("1234"), alternates.variants()[0].attribute("length"));
  assert_eq!("/resource.fr.html", alternates.variants()[1].uri());
  assert_eq!(
    "0.8",
    alternates.variants()[1].quality(),
    "q-value text must be recorded, not used to reorder variants"
  );
  assert_eq!(
    Some("text/html; charset=utf-8"),
    alternates.variants()[1].attribute("type")
  );
  assert_eq!(Some("fr"), alternates.variants()[1].attribute("language"));

  let tcn = response
    .tcn()
    .expect("TCN should parse")
    .expect("TCN should be present");
  assert_eq!(&[TcnDirective::Choice, TcnDirective::Keep], tcn.members());
  assert_eq!("choice, keep", tcn.header_value());

  let variant_vary = response
    .variant_vary()
    .expect("Variant-Vary should parse")
    .expect("Variant-Vary should be present");
  assert!(!variant_vary.is_any());
  assert_eq!(
    vec!["accept-language", "sec-ch-dpr"],
    variant_vary.field_names()
  );
  assert_eq!("accept-language, sec-ch-dpr", variant_vary.header_value());
  assert_eq!(
    Some("accept-language, sec-ch-dpr"),
    response.header_value("Variant-Vary").map(String::as_str)
  );
}

fn reject_before_connect(
  label: &str,
  apply: impl FnOnce(
    &mut rttp_client::HttpClient,
  ) -> rttp_client::error::Result<&mut rttp_client::HttpClient>,
) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused negotiation listener");
  listener
    .set_nonblocking(true)
    .expect("unused negotiation listener should be nonblocking");
  let addr = listener
    .local_addr()
    .expect("unused negotiation listener addr");
  let mut client = client();
  client.get().url(format!("http://{addr}/asset"));
  let error = apply(&mut client).expect_err(label);
  assert!(
    error.is_builder(),
    "{label} must fail as a builder error before connect"
  );
  assert!(listener.accept().is_err(), "{label} must not open a socket");
}

fn negotiation_field_parse_failed(request: &Request, name: &str) -> bool {
  match name {
    "A-IM" => request.a_im().is_err(),
    _ => request.negotiate().is_err(),
  }
}

#[test]
fn http11_facade_roundtrip_exchanges_all_negotiation_metadata_without_negotiation() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| negotiation_response(HTTP11_BODY));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/asset")))
    .emit()
    .expect("HTTP/1.1 negotiation metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe HTTP/1.1 negotiation metadata");
  assert_eq!("HTTP/1.1", observed.version);
  assert_eq!("/asset", observed.target);
  assert_eq!(Ok(Some(A_IM_REQUEST.to_string())), observed.a_im);
  assert_eq!(Some(A_IM_REQUEST.to_string()), observed.raw_a_im);
  assert_eq!(
    Some(vec![
      ("diffe".to_string(), 1000),
      ("gzip".to_string(), 300),
      ("identity".to_string(), 0),
    ]),
    observed.a_im_quality_order,
    "A-IM q-values must be recorded in declared order, not quality-sorted"
  );
  assert_eq!(Ok(Some(NEGOTIATE_REQUEST.to_string())), observed.negotiate);
  assert_eq!(Some(NEGOTIATE_REQUEST.to_string()), observed.raw_negotiate);
  assert_eq!("HTTP/1.1", response.version());
  assert_valid_response_metadata(&response, HTTP11_BODY);
  handle
    .join()
    .expect("HTTP/1.1 negotiation metadata server thread");
}

#[test]
fn h2c_facade_roundtrip_exchanges_ordinary_negotiation_headers_without_negotiation() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| negotiation_response(H2C_BODY));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/asset")))
    .emit_http2_prior_knowledge()
    .expect("h2c negotiation metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe h2c negotiation metadata");
  assert_eq!("HTTP/2", observed.version);
  assert_eq!("/asset", observed.target);
  assert_eq!(Ok(Some(A_IM_REQUEST.to_string())), observed.a_im);
  // HTTP/2 stores lower-case names; typed helpers and raw lookup stay
  // case-insensitive.
  assert_eq!(Some(A_IM_REQUEST.to_string()), observed.raw_a_im);
  assert_eq!(
    Some(vec![
      ("diffe".to_string(), 1000),
      ("gzip".to_string(), 300),
      ("identity".to_string(), 0),
    ]),
    observed.a_im_quality_order,
    "A-IM q-values must be recorded in declared order, not quality-sorted"
  );
  assert_eq!(Ok(Some(NEGOTIATE_REQUEST.to_string())), observed.negotiate);
  assert_eq!(Some(NEGOTIATE_REQUEST.to_string()), observed.raw_negotiate);
  assert_eq!("HTTP/2", response.version());
  assert_valid_response_metadata(&response, H2C_BODY);
  handle
    .join()
    .expect("h2c negotiation metadata server thread");
}

#[test]
fn typed_request_helpers_reject_malformed_duplicate_and_oversized_values_before_connect() {
  reject_before_connect("malformed A-IM token", |client| client.a_im("not a token"));
  reject_before_connect("duplicate A-IM members", |client| {
    client.a_im("diffe").and_then(|client| client.a_im("DIFFE"))
  });
  reject_before_connect("invalid A-IM q-value", |client| {
    client.a_im_with_q("gzip", "1.001")
  });
  reject_before_connect("too many A-IM members", |client| {
    client.a_im_value(
      (0..33)
        .map(|index| format!("im{index}"))
        .collect::<Vec<_>>()
        .join(", "),
    )
  });
  reject_before_connect("oversized A-IM value", |client| {
    client.a_im_value("a".repeat(64 * 1024 + 1))
  });
  reject_before_connect("malformed Negotiate directive", |client| {
    client.negotiate("not a directive")
  });
  reject_before_connect("duplicate Negotiate directives", |client| {
    client.negotiate("trans, TRANS")
  });
  reject_before_connect("too many Negotiate members", |client| {
    client.negotiate(
      (0..33)
        .map(|index| format!("ext{index}"))
        .collect::<Vec<_>>()
        .join(", "),
    )
  });
  reject_before_connect("oversized Negotiate value", |client| {
    client.negotiate("a".repeat(64 * 1024 + 1))
  });
}

#[test]
fn facade_server_preserves_raw_headers_when_typed_request_helpers_reject_malformed_values() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("malformed-request"));

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("A-IM", "diffe, DIFFE"))
    .header(("Negotiate", "trans, TRANS"))
    .emit()
    .expect("raw malformed negotiation headers should still complete as HTTP");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe malformed negotiation metadata");
  assert!(observed.a_im.is_err());
  assert_eq!(Some("diffe, DIFFE".to_string()), observed.raw_a_im);
  assert_eq!(None, observed.a_im_quality_order);
  assert!(observed.negotiate.is_err());
  assert_eq!(Some("trans, TRANS".to_string()), observed.raw_negotiate);
  assert_eq!(200, response.code());
  assert_eq!(
    "malformed-request",
    response.body().string().expect("malformed request body")
  );
  handle.join().expect("malformed request server thread");
}

#[test]
fn facade_server_preserves_raw_headers_when_duplicate_request_fields_are_rejected() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("duplicate-request"));

  let mut stream = TcpStream::connect(addr).expect("connect duplicate negotiation request");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set duplicate request write timeout");
  stream
    .write_all(
      b"GET /asset HTTP/1.1\r\n\
Host: example.test\r\n\
A-IM: diffe\r\n\
a-im: DIFFE\r\n\
Negotiate: trans\r\n\
negotiate: TRANS\r\n\
Connection: close\r\n\
\r\n",
    )
    .expect("write duplicate negotiation request");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe duplicate negotiation metadata");
  assert!(observed.a_im.is_err());
  assert_eq!(Some("diffe".to_string()), observed.raw_a_im);
  assert!(observed.negotiate.is_err());
  assert_eq!(Some("trans".to_string()), observed.raw_negotiate);
  handle.join().expect("duplicate request server thread");
}

#[test]
fn h2c_duplicate_request_fields_fail_closed_while_raw_headers_remain_visible() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("duplicate-h2c-request"));

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", &authority),
      ("a-im", "diffe"),
      ("a-im", "DIFFE"),
      ("negotiate", "trans"),
      ("negotiate", "TRANS"),
    ],
  );

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe duplicate h2c negotiation metadata");
  assert_eq!("HTTP/2", observed.version);
  assert!(observed.a_im.is_err());
  assert_eq!(Some("diffe".to_string()), observed.raw_a_im);
  assert!(observed.negotiate.is_err());
  assert_eq!(Some("trans".to_string()), observed.raw_negotiate);
  handle.join().expect("duplicate h2c request server thread");
}

#[test]
fn live_http11_response_helpers_reject_malformed_and_duplicate_metadata_while_preserving_raw_headers(
) {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("invalid-response")
      .header("IM", "diffe, DIFFE")
      .header("Delta-Base", "\"one\", \"two\"")
      .header("Alternates", r#"{ "/broken" 1.001 }"#)
      .header("TCN", "variant")
      .header("Variant-Vary", "Accept-Language, *")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("invalid negotiation response headers should remain observable");

  let _ = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the invalid-response request");

  assert!(response.im().is_err());
  assert_eq!(
    Some("diffe, DIFFE"),
    response.header_value("IM").map(String::as_str)
  );
  assert!(response.delta_base().is_err());
  assert_eq!(
    Some("\"one\", \"two\""),
    response.header_value("Delta-Base").map(String::as_str)
  );
  assert!(response.alternates().is_err());
  assert_eq!(
    Some(r#"{ "/broken" 1.001 }"#),
    response.header_value("Alternates").map(String::as_str)
  );
  assert!(response.tcn().is_err());
  assert_eq!(
    Some("variant"),
    response.header_value("TCN").map(String::as_str)
  );
  assert!(response.variant_vary().is_err());
  assert_eq!(
    Some("Accept-Language, *"),
    response.header_value("Variant-Vary").map(String::as_str)
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
fn live_h2c_response_helpers_reject_malformed_metadata_while_preserving_raw_headers() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("invalid-h2c-response")
      .header("IM", "diffe, DIFFE")
      .header("Delta-Base", "\"one\", \"two\"")
      .header("Alternates", r#"{ "/broken" 1.001 }"#)
      .header("TCN", "variant")
      .header("Variant-Vary", "Accept-Language, *")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit_http2_prior_knowledge()
    .expect("invalid h2c negotiation response headers should remain observable");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the invalid h2c response request");
  assert_eq!("HTTP/2", observed.version);

  assert!(response.im().is_err());
  assert_eq!(
    Some("diffe, DIFFE"),
    response.header_value("IM").map(String::as_str)
  );
  assert!(response.delta_base().is_err());
  assert_eq!(
    Some("\"one\", \"two\""),
    response.header_value("Delta-Base").map(String::as_str)
  );
  assert!(response.alternates().is_err());
  assert_eq!(
    Some(r#"{ "/broken" 1.001 }"#),
    response.header_value("Alternates").map(String::as_str)
  );
  assert!(response.tcn().is_err());
  assert_eq!(
    Some("variant"),
    response.header_value("TCN").map(String::as_str)
  );
  assert!(response.variant_vary().is_err());
  assert_eq!(
    Some("Accept-Language, *"),
    response.header_value("Variant-Vary").map(String::as_str)
  );
  assert_eq!(200, response.code());
  assert_eq!(
    "invalid-h2c-response",
    response.body().string().expect("invalid h2c response body")
  );
  handle.join().expect("invalid h2c response server thread");
}

#[test]
fn facade_server_rejects_oversized_negotiation_request_heads_as_400_before_dispatch() {
  for name in ["A-IM", "Negotiate"] {
    // A 64 KiB + 1 field value plus the request line exceeds the shared
    // HTTP/1.1 request-head bound, so the request is rejected as 400 before
    // handler dispatch. Oversized accessor parsing without losing raw access
    // is covered by protocol and server unit tests plus the raised-limit h2c
    // facade test.
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind oversized negotiation server")
      .with_read_timeout(Some(TIMEOUT))
      .with_write_timeout(Some(TIMEOUT));
    let addr = server
      .local_addr()
      .expect("oversized negotiation server addr");
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
      "GET /asset HTTP/1.1\r\nHost: example.test\r\n{name}: {oversized}\r\nConnection: close\r\n\r\n"
    );
    let mut stream = TcpStream::connect(addr).expect("connect oversized negotiation request");
    stream
      .write_all(request.as_bytes())
      .expect("write oversized negotiation request");
    let mut response = Vec::new();
    stream
      .read_to_end(&mut response)
      .expect("read oversized request-head response");
    let response = String::from_utf8(response).expect("response should be utf-8");

    assert!(
      response.starts_with("HTTP/1.1 400 "),
      "oversized {name} request head should be rejected before handler dispatch: {response}"
    );
    assert!(
      observed_rx.try_recv().is_err(),
      "oversized {name} must not reach the handler"
    );
    handle.join().expect("oversized negotiation server thread");
  }
}

#[test]
fn h2c_oversized_negotiation_metadata_reaches_accessor_fails_closed_and_keeps_raw_headers() {
  for name in ["A-IM", "Negotiate"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind oversized h2c negotiation server")
      .with_read_timeout(Some(TIMEOUT))
      .with_write_timeout(Some(TIMEOUT))
      .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
    let addr = server
      .local_addr()
      .expect("oversized h2c negotiation server addr");
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let raw = request.header(name).map(str::to_string);
          tx.send((
            request.version().to_string(),
            raw.as_ref().map(String::len),
            negotiation_field_parse_failed(&request, name),
            raw.is_some(),
          ))
          .expect("record oversized h2c negotiation field");
          HttpResponse::ok("ok")
        })
        .expect("serve oversized h2c negotiation request");
    });

    let oversized = "1".repeat(64 * 1024 + 1);
    let response = HttpClient::new()
      .get()
      .url(format!("http://{addr}/asset"))
      .header((name, oversized.as_str()))
      .emit_http2_prior_knowledge()
      .expect("receive oversized h2c response");

    assert_eq!("ok", response.body().string().expect("h2c response body"));
    assert_eq!(
      ("HTTP/2".to_string(), Some(64 * 1024 + 1), true, true),
      rx.recv_timeout(TIMEOUT)
        .unwrap_or_else(|_| panic!("recorded oversized h2c {name}"))
    );
    handle
      .join()
      .unwrap_or_else(|_| panic!("oversized h2c {name} server thread"));
  }
}

#[test]
fn response_helpers_reject_member_count_and_size_bounds_while_preserving_raw_headers() {
  // Server typed builders reject malformed or over-limit negotiation metadata
  // before sending, so live responses cannot carry these shapes.
  assert!(HttpResponse::ok([]).with_im(["diffe", "DIFFE"]).is_err());
  assert!(HttpResponse::ok([])
    .with_alternates(r#"{ "/broken" 1.001 }"#)
    .is_err());
  assert!(HttpResponse::ok([]).with_tcn("variant").is_err());
  assert!(HttpResponse::ok([])
    .with_variant_vary("Accept-Language, *")
    .is_err());

  // A 257-name Variant-Vary value and a 257-variant Alternates value fit in
  // a raw response head, so the client parser edges stay observable.
  let too_many_names = (0..=256)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpResponse::ok([])
      .with_variant_vary(too_many_names.clone())
      .is_err(),
    "Variant-Vary builder must reject more than 256 field names"
  );
  let too_many_names_response = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nVariant-Vary: {too_many_names}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("over-limit Variant-Vary should remain in the raw response");
  assert!(too_many_names_response.variant_vary().is_err());
  assert_eq!(
    Some(too_many_names.as_str()),
    too_many_names_response
      .header_value("Variant-Vary")
      .map(String::as_str)
  );

  let too_many_variants = (0..=256)
    .map(|index| format!(r#"{{ "/v{index}" 1 }}"#))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_variants_response = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nAlternates: {too_many_variants}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("over-limit Alternates should remain in the raw response");
  assert!(too_many_variants_response.alternates().is_err());
  assert_eq!(
    Some(too_many_variants.as_str()),
    too_many_variants_response
      .header_value("Alternates")
      .map(String::as_str)
  );

  // A 64 KiB + 1 response field exceeds the shared HTTP/1.1 and h2c header
  // bounds before the client typed helpers run, so the oversized parser
  // edges stay on Response::new with raw headers preserved.
  let oversized = "A".repeat(64 * 1024 + 1);
  let oversized_im = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nIM: {oversized}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
  )
  .expect("oversized IM should remain in the raw response");
  assert!(oversized_im.im().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_im.header_value("IM").map(String::as_str)
  );

  let oversized_delta_base = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nDelta-Base: {oversized}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
  )
  .expect("oversized Delta-Base should remain in the raw response");
  assert!(oversized_delta_base.delta_base().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_delta_base
      .header_value("Delta-Base")
      .map(String::as_str)
  );

  let oversized_alternates = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nAlternates: {oversized}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
  )
  .expect("oversized Alternates should remain in the raw response");
  assert!(oversized_alternates.alternates().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_alternates
      .header_value("Alternates")
      .map(String::as_str)
  );

  let oversized_tcn = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    format!("HTTP/1.1 200 OK\r\nTCN: {oversized}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
  )
  .expect("oversized TCN should remain in the raw response");
  assert!(oversized_tcn.tcn().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_tcn.header_value("TCN").map(String::as_str)
  );

  // TCN rejects duplicate fields; the 32-member count is unreachable because
  // only five distinct directives exist, so the count bound stays covered by
  // protocol unit tests.
  let duplicate_tcn = Response::new(
    RoUrl::with("http://127.0.0.1/asset"),
    b"HTTP/1.1 200 OK\r\nTCN: choice\r\nTCN: keep\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("duplicate TCN fields should remain observable");
  assert!(duplicate_tcn.tcn().is_err());
  assert_eq!(
    vec!["choice", "keep"],
    duplicate_tcn
      .header_values("TCN")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
  );
}

#[test]
fn metadata_helpers_never_synthesize_negotiation_delta_or_cache_policy() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("negotiation-metadata"));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/asset")))
    .emit()
    .expect("no-policy response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the no-policy request");
  assert_eq!("HTTP/1.1", observed.version);
  assert_eq!(Ok(Some(A_IM_REQUEST.to_string())), observed.a_im);
  assert_eq!(Ok(Some(NEGOTIATE_REQUEST.to_string())), observed.negotiate);

  // The handler ignored the request negotiation metadata and returned an
  // ordinary 200: helpers must not pick an Alternates URI, apply a delta,
  // synthesize 226, or build cache policy from the request fields.
  assert_eq!(
    "negotiation-metadata",
    response.body().string().expect("no-policy body")
  );
  assert_no_negotiation_policy(&response);
  assert!(response
    .im()
    .expect("absent IM should parse as None")
    .is_none());
  assert!(response
    .delta_base()
    .expect("absent Delta-Base should parse as None")
    .is_none());
  assert!(response
    .alternates()
    .expect("absent Alternates should parse as None")
    .is_none());
  assert!(response
    .tcn()
    .expect("absent TCN should parse as None")
    .is_none());
  assert!(response
    .variant_vary()
    .expect("absent Variant-Vary should parse as None")
    .is_none());
  handle.join().expect("no-policy server thread");
}

fn send_h2c_prior_knowledge_headers(
  addr: std::net::SocketAddr,
  fields: &[(&str, &str)],
) -> TcpStream {
  let mut stream = TcpStream::connect(addr).expect("connect raw h2c client");
  stream
    .set_read_timeout(Some(TIMEOUT))
    .expect("set raw h2c read timeout");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set raw h2c write timeout");
  stream
    .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
    .expect("write HTTP/2 preface");
  write_http2_frame(&mut stream, 0x4, 0, 0, &[]);

  let mut saw_settings = false;
  let mut saw_settings_ack = false;
  while !saw_settings || !saw_settings_ack {
    let frame = read_http2_frame(&mut stream);
    if frame.0 == 0x4 && frame.1 & 0x1 == 0 {
      saw_settings = true;
    }
    if frame.0 == 0x4 && frame.1 & 0x1 == 0x1 {
      saw_settings_ack = true;
    }
  }
  write_http2_frame(&mut stream, 0x4, 0x1, 0, &[]);

  let mut block = Vec::new();
  for (name, value) in fields {
    block.push(0);
    encode_hpack_string(&mut block, name.as_bytes());
    encode_hpack_string(&mut block, value.as_bytes());
  }
  write_http2_frame(&mut stream, 0x1, 0x1 | 0x4, 1, &block);
  stream
}

fn encode_hpack_string(block: &mut Vec<u8>, value: &[u8]) {
  assert!(
    value.len() < 127,
    "raw h2c test helper only encodes short HPACK strings"
  );
  block.push(value.len() as u8);
  block.extend_from_slice(value);
}

fn write_http2_frame(
  stream: &mut impl Write,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream
    .write_all(&header)
    .expect("write HTTP/2 frame header");
  stream
    .write_all(payload)
    .expect("write HTTP/2 frame payload");
  stream.flush().expect("flush HTTP/2 frame");
}

fn read_http2_frame(stream: &mut impl Read) -> (u8, u8, u32, Vec<u8>) {
  let mut header = [0; 9];
  stream
    .read_exact(&mut header)
    .expect("read HTTP/2 frame header");
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream
    .read_exact(&mut payload)
    .expect("read HTTP/2 frame payload");
  let stream_id = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) & 0x7fff_ffff;
  (header[3], header[4], stream_id, payload)
}
