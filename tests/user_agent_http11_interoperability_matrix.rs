#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rttp::server::{HttpResponse, HttpUserAgent, Request};

const BODY: &str = "user-agent-interoperability";
const EXPLICIT_INPUT: &str = "  Mozilla/5.0\tAcme/2.1 (compatible; facade) ";
const EXPLICIT_WIRE: &str = "Mozilla/5.0 Acme/2.1 (compatible; facade)";
const AUTOMATIC_PREFIX: &str = "Mozilla/5.0 rttp/";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy)]
enum Transport {
  Http11Sync,
  #[cfg(feature = "async")]
  Http11Async,
  H2cPriorKnowledge,
}

impl Transport {
  fn name(self) -> &'static str {
    match self {
      Self::Http11Sync => "http11-sync",
      #[cfg(feature = "async")]
      Self::Http11Async => "http11-async",
      Self::H2cPriorKnowledge => "h2c-prior-knowledge",
    }
  }

  fn version(self) -> &'static str {
    match self {
      Self::Http11Sync => "HTTP/1.1",
      #[cfg(feature = "async")]
      Self::Http11Async => "HTTP/1.1",
      Self::H2cPriorKnowledge => "HTTP/2",
    }
  }
}

fn transports() -> Vec<Transport> {
  vec![
    Transport::Http11Sync,
    #[cfg(feature = "async")]
    Transport::Http11Async,
    Transport::H2cPriorKnowledge,
  ]
}

#[derive(Debug, PartialEq)]
struct ObservedUserAgent {
  version: String,
  raw: Option<String>,
  typed: Result<Option<HttpUserAgent>, String>,
}

fn observe_user_agent(request: &Request) -> ObservedUserAgent {
  ObservedUserAgent {
    version: request.version().to_string(),
    raw: request.header("User-Agent").map(str::to_string),
    typed: request.user_agent().map_err(|error| error.to_string()),
  }
}

fn spawn_facade_server() -> (SocketAddr, Receiver<ObservedUserAgent>, JoinHandle<()>) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind User-Agent facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT));
  let addr = server
    .local_addr()
    .expect("User-Agent facade server address");
  let (observed_tx, observed_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        observed_tx
          .send(observe_user_agent(&request))
          .expect("send observed User-Agent metadata");
        HttpResponse::ok(BODY)
      })
      .expect("serve User-Agent facade request");
  });
  (addr, observed_rx, handle)
}

fn run_transport_case(transport: Transport, explicit: bool) {
  let (addr, observed_rx, handle) = spawn_facade_server();
  let mut client = rttp::Http::client();
  client.get().url(format!("http://{addr}/user-agent"));
  if explicit {
    client
      .user_agent(EXPLICIT_INPUT)
      .expect("explicit User-Agent metadata should be accepted");
  }

  let response = match transport {
    Transport::Http11Sync => client.emit(),
    #[cfg(feature = "async")]
    Transport::Http11Async => block_on(client.rasync()),
    Transport::H2cPriorKnowledge => client.emit_http2_prior_knowledge(),
  }
  .unwrap_or_else(|error| {
    panic!(
      "{} User-Agent request should succeed: {error}",
      transport.name()
    )
  });
  assert_eq!(transport.version(), response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    BODY,
    response
      .body()
      .string()
      .expect("response body should parse")
  );

  let observed = observed_rx.recv_timeout(TIMEOUT).unwrap_or_else(|error| {
    panic!(
      "{} server should observe User-Agent: {error}",
      transport.name()
    )
  });
  assert_eq!(transport.version(), observed.version);
  if explicit {
    assert_explicit_user_agent(observed);
  } else {
    assert_automatic_user_agent(observed);
  }
  handle
    .join()
    .unwrap_or_else(|_| panic!("{} User-Agent facade server thread", transport.name()));
}

fn assert_explicit_user_agent(observed: ObservedUserAgent) {
  let expected = rttp::UserAgent::parse(EXPLICIT_WIRE).expect("explicit User-Agent should parse");
  assert_eq!(Some(EXPLICIT_WIRE.to_string()), observed.raw);
  assert_eq!(Ok(Some(expected.clone())), observed.typed);
  assert_eq!(Some("Mozilla"), expected.members()[0].product());
  assert_eq!(Some("5.0"), expected.members()[0].version());
  assert_eq!(Some("compatible; facade"), expected.members()[2].comment());
}

fn assert_automatic_user_agent(observed: ObservedUserAgent) {
  let raw = observed
    .raw
    .expect("automatic User-Agent should remain visible as a raw header");
  assert!(
    raw.starts_with(AUTOMATIC_PREFIX),
    "automatic User-Agent should retain the existing default: {raw}"
  );
  let expected =
    rttp::UserAgent::parse(&raw).expect("automatic default should be valid typed metadata");
  assert_eq!(Ok(Some(expected.clone())), observed.typed);
  assert_eq!(raw, expected.header_value());
  assert_eq!(Some("Mozilla"), expected.members()[0].product());
  assert_eq!(Some("5.0"), expected.members()[0].version());
  assert_eq!(Some("rttp"), expected.members()[1].product());
}

#[test]
fn facade_user_agent_transport_matrix_covers_explicit_and_automatic_values() {
  for transport in transports() {
    run_transport_case(transport, true);
    run_transport_case(transport, false);
  }
}

fn run_raw_peer_case(
  request_bytes: &[u8],
  expected_raw: Option<&str>,
  expected_typed: Result<Option<HttpUserAgent>, ()>,
) {
  let (addr, observed_rx, handle) = spawn_facade_server();
  let mut stream = TcpStream::connect(addr).expect("connect raw User-Agent peer");
  stream
    .set_read_timeout(Some(TIMEOUT))
    .expect("set raw User-Agent read timeout");
  stream
    .set_write_timeout(Some(TIMEOUT))
    .expect("set raw User-Agent write timeout");
  stream
    .write_all(request_bytes)
    .expect("write raw User-Agent request");
  let mut response = Vec::new();
  stream
    .read_to_end(&mut response)
    .expect("read raw User-Agent response");
  assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));

  let observed = observed_rx
    .recv_timeout(TIMEOUT)
    .expect("receive raw User-Agent metadata");
  assert_eq!(expected_raw.map(str::to_string), observed.raw);
  match expected_typed {
    Ok(None) => assert_eq!(Ok(None), observed.typed.map(|value| value.map(|_| ()))),
    Err(()) => assert!(observed.typed.is_err(), "typed User-Agent should fail"),
    Ok(Some(_)) => unreachable!("raw peer cases only cover absence and invalid values"),
  }
  handle.join().expect("raw User-Agent facade server thread");
}

#[test]
fn facade_server_raw_user_agent_cases_preserve_absence_and_invalid_headers() {
  run_raw_peer_case(
    b"GET /absent HTTP/1.1\r\nHost: example.test\r\n\r\n",
    None,
    Ok(None),
  );
  run_raw_peer_case(
    b"GET /malformed HTTP/1.1\r\nHost: example.test\r\nUser-Agent: product/\r\n\r\n",
    Some("product/"),
    Err(()),
  );
  run_raw_peer_case(
    b"GET /duplicate HTTP/1.1\r\nHost: example.test\r\nUser-Agent: client/1\r\nuser-agent: client/2\r\n\r\n",
    Some("client/1"),
    Err(()),
  );
}

fn assert_rejected_before_connect(value: String, label: &str) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused User-Agent listener");
  listener
    .set_nonblocking(true)
    .expect("unused User-Agent listener should be nonblocking");
  let addr = listener
    .local_addr()
    .expect("unused User-Agent listener address");
  let mut client = rttp::Http::client();
  client.get().url(format!("http://{addr}/rejected"));
  let error = client.user_agent(value).expect_err(label);
  assert!(error.is_builder(), "{label} must be a builder error");
  match listener.accept() {
    Err(error) => assert_eq!(
      std::io::ErrorKind::WouldBlock,
      error.kind(),
      "{label} must not open a socket"
    ),
    Ok(_) => panic!("{label} unexpectedly opened a socket"),
  }
}

#[test]
fn facade_client_rejects_malformed_and_oversized_user_agent_before_connecting() {
  for (label, value) in [
    ("empty User-Agent", String::new()),
    ("malformed User-Agent", "product/".to_string()),
    ("control-byte User-Agent", "Mozilla/5.0\0".to_string()),
    ("oversized User-Agent", "p".repeat(64 * 1024 + 1)),
  ] {
    assert_rejected_before_connect(value, label);
  }
}
