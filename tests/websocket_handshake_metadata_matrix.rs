use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::SecWebSocketAccept;
use rttp_client::response::Response;
use rttp_client::types::RoUrl;
use rttp_client::HttpClient;
use rttp_server::server::{HttpResponse, HttpSecWebSocketKey, Request};

const RFC_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const RFC_ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
const PROTOCOL_OFFERS: &str = "graphql-transport-ws, chat";
const SELECTED_PROTOCOL: &str = "graphql-transport-ws";
const EXTENSION_OFFERS: &str = "permessage-deflate; client_max_window_bits";
const SELECTED_EXTENSION: &str = "permessage-deflate";
const HTTP11_BODY: &str = "http11-ws-metadata";
const H2C_BODY: &str = "h2c-ws-metadata";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
struct ObservedHandshakeRequest {
  version: String,
  target: String,
  key: Result<Option<String>, String>,
  raw_key: Option<String>,
  key_debug: Option<String>,
  versions: Result<Option<String>, String>,
  raw_version: Option<String>,
  protocols: Result<Option<String>, String>,
  raw_protocol: Option<String>,
  extensions: Result<Option<String>, String>,
  raw_extensions: Option<String>,
  upgrade: Option<String>,
  connection: Option<String>,
  extended_connect_protocol: Option<String>,
}

fn client() -> HttpClient {
  HttpClient::new()
}

fn bind_facade_server() -> rttp_server::server::HttpServer {
  rttp::Http::server("127.0.0.1:0")
    .expect("bind websocket handshake facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT))
}

fn observe_request(request: &Request) -> ObservedHandshakeRequest {
  let key = request
    .sec_websocket_key()
    .map(|key| key.map(|key| key.as_str().to_string()))
    .map_err(|error| error.to_string());
  let key_debug = request
    .sec_websocket_key()
    .ok()
    .flatten()
    .map(|key| format!("{key:?}"));
  ObservedHandshakeRequest {
    version: request.version().to_string(),
    target: request.target().to_string(),
    key,
    raw_key: request.header("Sec-WebSocket-Key").map(str::to_string),
    key_debug,
    versions: request
      .sec_websocket_version()
      .map(|versions| versions.map(|versions| versions.header_value()))
      .map_err(|error| error.to_string()),
    raw_version: request.header("Sec-WebSocket-Version").map(str::to_string),
    protocols: request
      .sec_websocket_protocol()
      .map(|offers| offers.map(|offers| offers.header_value()))
      .map_err(|error| error.to_string()),
    raw_protocol: request.header("Sec-WebSocket-Protocol").map(str::to_string),
    extensions: request
      .sec_websocket_extensions()
      .map(|offers| offers.map(|offers| offers.header_value()))
      .map_err(|error| error.to_string()),
    raw_extensions: request
      .header("Sec-WebSocket-Extensions")
      .map(str::to_string),
    upgrade: request.header("Upgrade").map(str::to_string),
    connection: request.header("Connection").map(str::to_string),
    extended_connect_protocol: request.extended_connect_protocol().map(str::to_string),
  }
}

fn negotiated_response(body: &'static str) -> HttpResponse {
  let key = HttpSecWebSocketKey::parse(RFC_KEY).expect("RFC example key should parse");
  HttpResponse::ok(body)
    .with_sec_websocket_accept_for_key(&key)
    .with_sec_websocket_version(["13"])
    .expect("response Sec-WebSocket-Version should be accepted")
    .with_sec_websocket_protocol(SELECTED_PROTOCOL)
    .expect("response Sec-WebSocket-Protocol should be accepted")
    .with_sec_websocket_extensions(SELECTED_EXTENSION)
    .expect("response Sec-WebSocket-Extensions should be accepted")
}

fn spawn_observed_facade_server(
  response: impl Fn(Request) -> HttpResponse + Send + 'static,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedHandshakeRequest>,
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
          .expect("send observed websocket handshake metadata");
        response(request)
      })
      .expect("serve websocket handshake metadata request");
  });
  (addr, observed_rx, handle)
}

fn assert_no_upgrade_headers(observed: &ObservedHandshakeRequest) {
  assert_eq!(
    None, observed.upgrade,
    "typed WebSocket handshake metadata must not set Upgrade"
  );
  assert_ne!(
    Some("Upgrade".to_string()),
    observed.connection,
    "typed WebSocket handshake metadata must not set Connection: Upgrade"
  );
  assert_eq!(
    None, observed.extended_connect_protocol,
    "typed WebSocket handshake metadata must not create :protocol"
  );
}

fn assert_no_upgrade_response(response: &Response) {
  assert_eq!(response.header_value("Upgrade"), None);
  assert_ne!(
    response.header_value("Connection").map(String::as_str),
    Some("Upgrade"),
    "typed WebSocket handshake metadata must not emit Connection: Upgrade"
  );
  assert!(
    response
      .headers()
      .iter()
      .all(|header| header.name() != ":protocol"),
    "typed WebSocket handshake metadata must not emit :protocol"
  );
}

fn assert_valid_response_metadata(response: &Response, body: &str) {
  assert_eq!(200, response.code());
  assert_eq!(
    body,
    response
      .body()
      .string()
      .expect("response body should remain ordinary HTTP bytes")
  );
  assert_no_upgrade_response(response);

  let key = HttpSecWebSocketKey::parse(RFC_KEY).expect("RFC example key should parse");
  let accept: SecWebSocketAccept = response
    .sec_websocket_accept()
    .expect("Sec-WebSocket-Accept should parse")
    .expect("Sec-WebSocket-Accept should be present");
  assert_eq!(RFC_ACCEPT, accept.as_str());
  assert_eq!(
    Some(RFC_ACCEPT),
    response
      .header_value("Sec-WebSocket-Accept")
      .map(String::as_str)
  );
  assert!(response
    .verify_sec_websocket_accept(&key)
    .expect("Sec-WebSocket-Accept should verify"));
  let accept_debug = format!("{accept:?}");
  assert!(!accept_debug.contains(RFC_KEY));
  assert!(!accept_debug.contains(RFC_ACCEPT));

  let versions = response
    .sec_websocket_version()
    .expect("Sec-WebSocket-Version should parse")
    .expect("Sec-WebSocket-Version should be present");
  assert_eq!(versions.versions(), ["13"]);
  assert_eq!(versions.header_value(), "13");
  assert_eq!(
    Some("13"),
    response
      .header_value("Sec-WebSocket-Version")
      .map(String::as_str)
  );

  let protocol = response
    .sec_websocket_protocol()
    .expect("selected Sec-WebSocket-Protocol should parse")
    .expect("selected Sec-WebSocket-Protocol should be present");
  assert_eq!(protocol.selected(), Some(SELECTED_PROTOCOL));
  assert_eq!(
    Some(SELECTED_PROTOCOL),
    response
      .header_value("Sec-WebSocket-Protocol")
      .map(String::as_str)
  );

  let extensions = response
    .sec_websocket_extensions()
    .expect("selected Sec-WebSocket-Extensions should parse")
    .expect("selected Sec-WebSocket-Extensions should be present");
  assert_eq!(
    extensions.selected().map(|extension| extension.token()),
    Some(SELECTED_EXTENSION)
  );
  assert_eq!(
    Some(SELECTED_EXTENSION),
    response
      .header_value("Sec-WebSocket-Extensions")
      .map(String::as_str)
  );
}

fn attach_valid_client_metadata(client: &mut HttpClient) -> &mut HttpClient {
  client
    .sec_websocket_key(RFC_KEY)
    .expect("Sec-WebSocket-Key should be accepted")
    .sec_websocket_version("13")
    .expect("Sec-WebSocket-Version should be accepted")
    .sec_websocket_protocol(PROTOCOL_OFFERS)
    .expect("Sec-WebSocket-Protocol should be accepted")
    .sec_websocket_extensions(EXTENSION_OFFERS)
    .expect("Sec-WebSocket-Extensions should be accepted")
}

fn reject_before_connect(
  label: &str,
  apply: impl FnOnce(&mut HttpClient) -> rttp_client::error::Result<&mut HttpClient>,
) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused websocket listener");
  listener
    .set_nonblocking(true)
    .expect("unused websocket listener should be nonblocking");
  let addr = listener
    .local_addr()
    .expect("unused websocket listener addr");
  let mut client = client();
  client.get().url(format!("http://{addr}/chat"));
  let error = apply(&mut client).expect_err(label);
  assert!(
    error.is_builder(),
    "{label} must fail as a builder error before connect"
  );
  assert!(listener.accept().is_err(), "{label} must not open a socket");
}

#[test]
fn http11_facade_roundtrip_exchanges_all_handshake_metadata_without_upgrade() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| negotiated_response(HTTP11_BODY));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/chat")))
    .emit()
    .expect("HTTP/1.1 websocket metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe HTTP/1.1 websocket metadata");
  assert_eq!("HTTP/1.1", observed.version);
  assert_eq!("/chat", observed.target);
  assert_eq!(Ok(Some(RFC_KEY.to_string())), observed.key);
  assert_eq!(Some(RFC_KEY.to_string()), observed.raw_key);
  assert_eq!(Ok(Some("13".to_string())), observed.versions);
  assert_eq!(Some("13".to_string()), observed.raw_version);
  assert_eq!(Ok(Some(PROTOCOL_OFFERS.to_string())), observed.protocols);
  assert_eq!(Some(PROTOCOL_OFFERS.to_string()), observed.raw_protocol);
  assert_eq!(Ok(Some(EXTENSION_OFFERS.to_string())), observed.extensions);
  assert_eq!(Some(EXTENSION_OFFERS.to_string()), observed.raw_extensions);
  let key_debug = observed
    .key_debug
    .as_deref()
    .expect("typed Sec-WebSocket-Key Debug should be present");
  assert!(!key_debug.contains(RFC_KEY));
  assert_no_upgrade_headers(&observed);
  assert_eq!("HTTP/1.1", response.version());
  assert_valid_response_metadata(&response, HTTP11_BODY);
  handle
    .join()
    .expect("HTTP/1.1 websocket metadata server thread");
}

#[test]
fn h2c_facade_roundtrip_exchanges_ordinary_handshake_headers_without_extended_connect() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| negotiated_response(H2C_BODY));

  let mut client = client();
  let response = attach_valid_client_metadata(client.get().url(format!("http://{addr}/chat")))
    .emit_http2_prior_knowledge()
    .expect("h2c websocket metadata response should parse");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe h2c websocket metadata");
  assert_eq!("HTTP/2", observed.version);
  assert_eq!("/chat", observed.target);
  assert_eq!(Ok(Some(RFC_KEY.to_string())), observed.key);
  // HTTP/2 stores lower-case names; typed helpers and raw lookup stay case-insensitive.
  assert_eq!(Some(RFC_KEY.to_string()), observed.raw_key);
  assert_eq!(Ok(Some("13".to_string())), observed.versions);
  assert_eq!(Some("13".to_string()), observed.raw_version);
  assert_eq!(Ok(Some(PROTOCOL_OFFERS.to_string())), observed.protocols);
  assert_eq!(Some(PROTOCOL_OFFERS.to_string()), observed.raw_protocol);
  assert_eq!(Ok(Some(EXTENSION_OFFERS.to_string())), observed.extensions);
  assert_eq!(Some(EXTENSION_OFFERS.to_string()), observed.raw_extensions);
  let key_debug = observed
    .key_debug
    .as_deref()
    .expect("typed Sec-WebSocket-Key Debug should be present");
  assert!(!key_debug.contains(RFC_KEY));
  assert_no_upgrade_headers(&observed);
  assert_eq!("HTTP/2", response.version());
  assert_valid_response_metadata(&response, H2C_BODY);
  let accept_name = response
    .headers_of_name("Sec-WebSocket-Accept")
    .first()
    .expect("h2c response should expose raw Sec-WebSocket-Accept")
    .name()
    .to_ascii_lowercase();
  assert_eq!("sec-websocket-accept", accept_name);
  handle.join().expect("h2c websocket metadata server thread");
}

#[test]
fn typed_request_helpers_reject_malformed_and_oversized_values_before_connect() {
  reject_before_connect("malformed Sec-WebSocket-Key", |client| {
    client.sec_websocket_key("not-a-nonce")
  });
  reject_before_connect("oversized Sec-WebSocket-Key", |client| {
    client.sec_websocket_key("A".repeat(64 * 1024 + 1))
  });
  reject_before_connect("malformed Sec-WebSocket-Version", |client| {
    client.sec_websocket_version("8, 13")
  });
  reject_before_connect("oversized Sec-WebSocket-Version", |client| {
    client.sec_websocket_version("1".repeat(64 * 1024 + 1))
  });
  reject_before_connect("duplicate Sec-WebSocket-Protocol tokens", |client| {
    client.sec_websocket_protocol("chat, chat")
  });
  reject_before_connect("oversized Sec-WebSocket-Protocol", |client| {
    client.sec_websocket_protocol("a".repeat(64 * 1024 + 1))
  });
  reject_before_connect("duplicate Sec-WebSocket-Extensions tokens", |client| {
    client.sec_websocket_extensions("permessage-deflate, permessage-deflate")
  });
  reject_before_connect("oversized Sec-WebSocket-Extensions", |client| {
    client.sec_websocket_extensions("a".repeat(64 * 1024 + 1))
  });
}

#[test]
fn facade_server_preserves_raw_headers_when_typed_request_helpers_reject_malformed_values() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("malformed-request"));

  let response = client()
    .get()
    .url(format!("http://{addr}/chat"))
    .header(("Sec-WebSocket-Key", "not-a-nonce"))
    .header(("Sec-WebSocket-Version", "013"))
    .header(("Sec-WebSocket-Protocol", "not a token"))
    .header(("Sec-WebSocket-Extensions", "permessage deflate"))
    .emit()
    .expect("raw malformed websocket headers should still complete as HTTP");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe malformed websocket metadata");
  assert!(observed.key.is_err());
  assert_eq!(Some("not-a-nonce".to_string()), observed.raw_key);
  assert!(observed.versions.is_err());
  assert_eq!(Some("013".to_string()), observed.raw_version);
  assert!(observed.protocols.is_err());
  assert_eq!(Some("not a token".to_string()), observed.raw_protocol);
  assert!(observed.extensions.is_err());
  assert_eq!(
    Some("permessage deflate".to_string()),
    observed.raw_extensions
  );
  assert_no_upgrade_headers(&observed);
  assert_eq!(200, response.code());
  assert_eq!(
    "malformed-request",
    response.body().string().expect("malformed request body")
  );
  handle.join().expect("malformed request server thread");
}

#[test]
fn facade_server_preserves_raw_headers_when_duplicate_singleton_fields_are_rejected() {
  let (addr, observed_rx, handle) =
    spawn_observed_facade_server(|_| HttpResponse::ok("duplicate-request"));

  let mut stream = TcpStream::connect(addr).expect("connect duplicate websocket request");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set duplicate request write timeout");
  stream
    .write_all(
      b"GET /chat HTTP/1.1\r\n\
Host: example.test\r\n\
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
sec-websocket-key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\
Sec-WebSocket-Version: 13\r\n\
Sec-WebSocket-Version: 13\r\n\
Sec-WebSocket-Protocol: chat\r\n\
Sec-WebSocket-Protocol: chat\r\n\
Sec-WebSocket-Extensions: permessage-deflate\r\n\
Sec-WebSocket-Extensions: permessage-deflate\r\n\
Connection: close\r\n\
\r\n",
    )
    .expect("write duplicate websocket request");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe duplicate websocket metadata");
  assert!(observed.key.is_err());
  assert_eq!(Some(RFC_KEY.to_string()), observed.raw_key);
  assert!(observed.versions.is_err());
  assert_eq!(Some("13".to_string()), observed.raw_version);
  assert!(observed.protocols.is_err());
  assert_eq!(Some("chat".to_string()), observed.raw_protocol);
  assert!(observed.extensions.is_err());
  assert_eq!(
    Some("permessage-deflate".to_string()),
    observed.raw_extensions
  );
  handle.join().expect("duplicate request server thread");
}

#[test]
fn live_http11_response_helpers_reject_duplicate_and_multi_selection_while_preserving_raw_headers()
{
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("invalid-response")
      .header("Sec-WebSocket-Accept", RFC_ACCEPT)
      .header("sec-websocket-accept", "AAAAAAAAAAAAAAAAAAAAAAAAAAA=")
      .header("Sec-WebSocket-Protocol", "chat, superchat")
      .header("Sec-WebSocket-Extensions", "permessage-deflate, x-test")
      .header("Sec-WebSocket-Version", "8, 13")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/chat"))
    .emit()
    .expect("invalid websocket response headers should remain observable");

  let _ = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the invalid-response request");

  assert!(response.sec_websocket_accept().is_err());
  assert_eq!(
    vec![RFC_ACCEPT, "AAAAAAAAAAAAAAAAAAAAAAAAAAA="],
    response
      .header_values("Sec-WebSocket-Accept")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
  );
  assert!(response.sec_websocket_protocol().is_err());
  assert_eq!(
    Some("chat, superchat"),
    response
      .header_value("Sec-WebSocket-Protocol")
      .map(String::as_str)
  );
  assert!(response.sec_websocket_extensions().is_err());
  assert_eq!(
    Some("permessage-deflate, x-test"),
    response
      .header_value("Sec-WebSocket-Extensions")
      .map(String::as_str)
  );
  assert!(response.sec_websocket_version().is_err());
  assert_eq!(
    Some("8, 13"),
    response
      .header_value("Sec-WebSocket-Version")
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
fn live_h2c_response_helpers_reject_malformed_values_while_preserving_raw_headers() {
  let (addr, observed_rx, handle) = spawn_observed_facade_server(|_| {
    HttpResponse::ok("invalid-h2c-response")
      .header("Sec-WebSocket-Accept", "not-an-accept")
      .header("Sec-WebSocket-Protocol", "chat, superchat")
      .header("Sec-WebSocket-Extensions", "permessage-deflate, x-test")
      .header("Sec-WebSocket-Version", "013")
  });

  let response = client()
    .get()
    .url(format!("http://{addr}/chat"))
    .emit_http2_prior_knowledge()
    .expect("invalid h2c websocket response headers should remain observable");

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("server should observe the invalid h2c response request");
  assert_eq!("HTTP/2", observed.version);
  assert_eq!(None, observed.extended_connect_protocol);

  assert!(response.sec_websocket_accept().is_err());
  assert_eq!(
    Some("not-an-accept"),
    response
      .header_value("Sec-WebSocket-Accept")
      .map(String::as_str)
  );
  assert!(response.sec_websocket_protocol().is_err());
  assert_eq!(
    Some("chat, superchat"),
    response
      .header_value("Sec-WebSocket-Protocol")
      .map(String::as_str)
  );
  assert!(response.sec_websocket_extensions().is_err());
  assert_eq!(
    Some("permessage-deflate, x-test"),
    response
      .header_value("Sec-WebSocket-Extensions")
      .map(String::as_str)
  );
  assert!(response.sec_websocket_version().is_err());
  assert_eq!(
    Some("013"),
    response
      .header_value("Sec-WebSocket-Version")
      .map(String::as_str)
  );
  // Live h2c coverage here stays on single malformed or multi-selection
  // values. Duplicate raw-field shape is asserted on the HTTP/1.1 live path
  // and on Response::new parser edges below.
  assert_eq!(
    1,
    response.headers_of_name("Sec-WebSocket-Accept").len(),
    "this h2c case exposes one raw Sec-WebSocket-Accept field"
  );
  handle.join().expect("invalid h2c response server thread");
}

#[test]
fn response_new_covers_oversized_and_duplicate_parser_edges_without_a_live_server() {
  // Live HttpResponse helpers reject invalid typed values before sending, and
  // a 64 KiB + 1 response field exceeds the shared HTTP/1.1 and h2c header
  // bounds before the client typed helpers run. These parser-only edges stay
  // on Response::new so raw headers remain observable after helper errors.
  let oversized = "A".repeat(64 * 1024 + 1);
  let oversized_response = Response::new(
    RoUrl::with("http://127.0.0.1/chat"),
    format!("HTTP/1.1 200 OK\r\nSec-WebSocket-Accept: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("oversized Sec-WebSocket-Accept should remain in the raw response");
  assert!(oversized_response.sec_websocket_accept().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_response
      .header_value("Sec-WebSocket-Accept")
      .map(String::as_str)
  );

  let duplicate = Response::new(
    RoUrl::with("http://127.0.0.1/chat"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n",
      "sec-websocket-accept: AAAAAAAAAAAAAAAAAAAAAAAAAAA=\r\n",
      "Sec-WebSocket-Protocol: chat\r\n",
      "Sec-WebSocket-Protocol: superchat\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("duplicate response fields should remain observable");
  assert!(duplicate.sec_websocket_accept().is_err());
  assert_eq!(
    vec![RFC_ACCEPT, "AAAAAAAAAAAAAAAAAAAAAAAAAAA="],
    duplicate
      .header_values("Sec-WebSocket-Accept")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
  );
  assert!(duplicate.sec_websocket_protocol().is_err());
  assert_eq!(
    vec!["chat", "superchat"],
    duplicate
      .header_values("Sec-WebSocket-Protocol")
      .into_iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
  );
}
