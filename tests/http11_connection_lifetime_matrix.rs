//! HTTP/1.1 connection-lifetime and response-framing regression matrix.
//!
//! Exercises the sync client path (and the async path under `--features async`)
//! against deterministic socket2 scripted servers. Valid framing / lifetime
//! cases are separate from raw malformed-wire failures so helper normalization
//! assertions never paper over ambiguous framing.

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::{Config, HttpClient};
use rttp_test_support::{
  spawn_socket2_scripted_http_server, ScriptedHttpConnection, ScriptedHttpServerReport,
  ScriptedHttpStage,
};

#[derive(Clone, Copy)]
enum ClientPath {
  Sync,
  #[cfg(feature = "async")]
  Async,
}

impl ClientPath {
  fn name(self) -> &'static str {
    match self {
      Self::Sync => "sync",
      #[cfg(feature = "async")]
      Self::Async => "async",
    }
  }

  fn emit(
    self,
    client: &mut HttpClient,
  ) -> Result<rttp_client::response::Response, rttp_client::error::Error> {
    match self {
      Self::Sync => client.emit(),
      #[cfg(feature = "async")]
      Self::Async => block_on(client.rasync()),
    }
  }
}

fn client_paths() -> Vec<ClientPath> {
  vec![
    ClientPath::Sync,
    #[cfg(feature = "async")]
    ClientPath::Async,
  ]
}

fn client() -> HttpClient {
  HttpClient::new()
}

fn request_target(request: &[u8]) -> String {
  String::from_utf8_lossy(request)
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(1))
    .unwrap_or_default()
    .to_string()
}

fn request_method(request: &[u8]) -> String {
  String::from_utf8_lossy(request)
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().next())
    .unwrap_or_default()
    .to_string()
}

fn join_report(
  handle: std::thread::JoinHandle<ScriptedHttpServerReport>,
) -> ScriptedHttpServerReport {
  handle
    .join()
    .expect("scripted http server thread should finish")
}

struct FramingCase {
  name: &'static str,
  response: &'static [u8],
  method: MethodKind,
  expected_status: u32,
  expected_body: &'static str,
  client_reusable: bool,
}

#[derive(Clone, Copy)]
enum MethodKind {
  Get,
  Head,
}

impl MethodKind {
  fn apply(self, client: &mut HttpClient) -> &mut HttpClient {
    match self {
      Self::Get => client.get(),
      Self::Head => client.head(),
    }
  }
}

const FRAMING_CASES: &[FramingCase] = &[
  FramingCase {
    name: "content-length keep-alive",
    response: b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nfixed",
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "fixed",
    client_reusable: true,
  },
  FramingCase {
    name: "chunked keep-alive",
    response: concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Connection: keep-alive\r\n",
      "\r\n",
      "5\r\nchunk\r\n",
      "0\r\n",
      "\r\n"
    )
    .as_bytes(),
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "chunk",
    client_reusable: true,
  },
  FramingCase {
    name: "close-delimited until EOF",
    response: b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\neof-body",
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "eof-body",
    client_reusable: false,
  },
  FramingCase {
    name: "HEAD content-length without body bytes",
    response: b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: keep-alive\r\n\r\n",
    method: MethodKind::Head,
    expected_status: 200,
    expected_body: "",
    client_reusable: true,
  },
  FramingCase {
    name: "204 without body",
    response: b"HTTP/1.1 204 No Content\r\nConnection: keep-alive\r\n\r\n",
    method: MethodKind::Get,
    expected_status: 204,
    expected_body: "",
    client_reusable: true,
  },
  FramingCase {
    name: "304 without body",
    response: b"HTTP/1.1 304 Not Modified\r\nConnection: keep-alive\r\n\r\n",
    method: MethodKind::Get,
    expected_status: 304,
    expected_body: "",
    client_reusable: true,
  },
  FramingCase {
    name: "HTTP/1.0 default close",
    response: b"HTTP/1.0 200 OK\r\nContent-Length: 5\r\n\r\nhttp0",
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "http0",
    client_reusable: false,
  },
  FramingCase {
    name: "HTTP/1.0 explicit keep-alive",
    response: b"HTTP/1.0 200 OK\r\nContent-Length: 5\r\nConnection: keep-alive\r\n\r\nhttp0",
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "http0",
    client_reusable: true,
  },
  FramingCase {
    name: "HTTP/1.1 explicit Connection close",
    response: b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nclose",
    method: MethodKind::Get,
    expected_status: 200,
    expected_body: "close",
    client_reusable: false,
  },
];

#[test]
fn framing_and_lifetime_matrix_matches_exact_bodies_and_client_closed_state() {
  for path in client_paths() {
    for case in FRAMING_CASES {
      let (addr, handle) = spawn_socket2_scripted_http_server(vec![
        ScriptedHttpConnection::stages([ScriptedHttpStage::respond_and_close(case.response)]),
        // Probe accept for a follow-up emit when the client remains reusable.
        ScriptedHttpConnection::stages([ScriptedHttpStage::respond_and_close(
          b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nprobe".as_slice(),
        )]),
      ]);

      let mut http = client();
      case.method.apply(&mut http);
      let response = path
        .emit(
          http
            .url(format!(
              "http://{addr}/matrix/{}",
              case.name.replace(' ', "-")
            ))
            .config(Config::builder().read_timeout(2_000)),
        )
        .unwrap_or_else(|err| panic!("{} {} should succeed: {err}", path.name(), case.name));

      assert_eq!(
        case.expected_status,
        response.code(),
        "{} {}",
        path.name(),
        case.name
      );
      assert_eq!(
        case.expected_body,
        response.body().string().unwrap(),
        "{} {}",
        path.name(),
        case.name
      );

      let probe = path.emit(
        http
          .get()
          .url(format!("http://{addr}/matrix/probe"))
          .config(Config::builder().read_timeout(500)),
      );

      if case.client_reusable {
        let probe = probe.unwrap_or_else(|err| {
          panic!(
            "{} {} should leave the client reusable: {err}",
            path.name(),
            case.name
          )
        });
        assert_eq!("probe", probe.body().string().unwrap());
        let report = join_report(handle);
        assert_eq!(2, report.accept_count, "{} {}", path.name(), case.name);
        assert_eq!(
          2,
          report.requests_per_connection.len(),
          "{} {}",
          path.name(),
          case.name
        );
        assert_eq!(
          1,
          report.requests_per_connection[0].len(),
          "{} {}",
          path.name(),
          case.name
        );
        assert_eq!(
          1,
          report.requests_per_connection[1].len(),
          "{} {}",
          path.name(),
          case.name
        );
      } else {
        let err = probe.expect_err(&format!(
          "{} {} should mark the client closed",
          path.name(),
          case.name
        ));
        assert!(
          err.to_string().to_ascii_lowercase().contains("closed"),
          "{} {} unexpected closed error: {err}",
          path.name(),
          case.name
        );
        // Drop the unused second accept by ending the client; the scripted thread
        // may still be blocked on accept, so do not join with a hard hang: open
        // and close a throwaway connection to unblock the listener.
        let _ = std::net::TcpStream::connect(addr);
        let report = join_report(handle);
        assert_eq!(2, report.accept_count, "{} {}", path.name(), case.name);
        assert_eq!(
          1,
          report.requests_per_connection[0].len(),
          "{} {}",
          path.name(),
          case.name
        );
        assert!(
          report.requests_per_connection[1].is_empty(),
          "{} {} probe accept should see no HTTP request",
          path.name(),
          case.name
        );
      }
    }
  }
}

#[test]
fn redirect_reuse_keeps_request_boundaries_and_rejects_body_leak_into_next_response() {
  for path in client_paths() {
    let first = concat!(
      "HTTP/1.1 302 Found\r\n",
      "Location: /final\r\n",
      "Content-Length: 5\r\n",
      "Connection: keep-alive\r\n",
      "\r\n",
      "redir"
    );
    let second = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 5\r\n",
      "Connection: close\r\n",
      "\r\n",
      "final"
    );
    let (addr, handle) =
      spawn_socket2_scripted_http_server(vec![ScriptedHttpConnection::stages([
        ScriptedHttpStage::respond(first),
        ScriptedHttpStage::respond_and_close(second),
      ])]);

    let response = path
      .emit(
        client()
          .get()
          .config(Config::builder().auto_redirect(true).read_timeout(2_000))
          .url(format!("http://{addr}/start")),
      )
      .unwrap_or_else(|err| panic!("{} redirect reuse should succeed: {err}", path.name()));

    assert_eq!(200, response.code(), "{}", path.name());
    assert_eq!(
      "final",
      response.body().string().unwrap(),
      "{} must not leak redirect body bytes into the final response",
      path.name()
    );

    let report = join_report(handle);
    assert_eq!(1, report.accept_count, "{}", path.name());
    assert_eq!(
      vec![2usize],
      report
        .requests_per_connection
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>(),
      "{}",
      path.name()
    );
    assert_eq!(
      "/start",
      request_target(&report.requests_per_connection[0][0]),
      "{}",
      path.name()
    );
    assert_eq!(
      "/final",
      request_target(&report.requests_per_connection[0][1]),
      "{}",
      path.name()
    );
  }
}

#[test]
fn response_connection_close_forces_fresh_accept_on_redirect_follow() {
  for path in client_paths() {
    let (addr, handle) = spawn_socket2_scripted_http_server(vec![
      ScriptedHttpConnection::stages([ScriptedHttpStage::respond_and_close(concat!(
        "HTTP/1.1 302 Found\r\n",
        "Location: /final\r\n",
        "Content-Length: 0\r\n",
        "Connection: close\r\n",
        "\r\n"
      ))]),
      ScriptedHttpConnection::stages([ScriptedHttpStage::respond_and_close(concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 5\r\n",
        "Connection: close\r\n",
        "\r\n",
        "final"
      ))]),
    ]);

    let response = path
      .emit(
        client()
          .get()
          .config(Config::builder().auto_redirect(true).read_timeout(2_000))
          .url(format!("http://{addr}/start")),
      )
      .unwrap_or_else(|err| panic!("{} close redirect should succeed: {err}", path.name()));

    assert_eq!(200, response.code(), "{}", path.name());
    assert_eq!(
      "final",
      response.body().string().unwrap(),
      "{}",
      path.name()
    );

    let report = join_report(handle);
    assert_eq!(2, report.accept_count, "{}", path.name());
    assert_eq!(
      vec![1usize, 1usize],
      report
        .requests_per_connection
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>(),
      "{}",
      path.name()
    );
  }
}

#[test]
fn head_then_get_on_reused_redirect_socket_does_not_consume_phantom_body() {
  for path in client_paths() {
    // HEAD response advertises Content-Length but correctly omits body bytes.
    // A subsequent GET on the same socket must parse cleanly.
    let (addr, handle) =
      spawn_socket2_scripted_http_server(vec![ScriptedHttpConnection::stages([
        ScriptedHttpStage::respond(concat!(
          "HTTP/1.1 302 Found\r\n",
          "Location: /final\r\n",
          "Content-Length: 11\r\n",
          "Connection: keep-alive\r\n",
          "\r\n"
        )),
        ScriptedHttpStage::respond_and_close(concat!(
          "HTTP/1.1 200 OK\r\n",
          "Content-Length: 5\r\n",
          "Connection: close\r\n",
          "\r\n",
          "final"
        )),
      ])]);

    let response = path
      .emit(
        client()
          .head()
          .config(Config::builder().auto_redirect(true).read_timeout(2_000))
          .url(format!("http://{addr}/head-start")),
      )
      .unwrap_or_else(|err| panic!("{} HEAD redirect reuse should succeed: {err}", path.name()));

    assert_eq!(200, response.code(), "{}", path.name());
    assert_eq!("", response.body().string().unwrap(), "{}", path.name());

    let report = join_report(handle);
    assert_eq!(1, report.accept_count, "{}", path.name());
    assert_eq!(
      2,
      report.requests_per_connection[0].len(),
      "{}",
      path.name()
    );
    assert_eq!(
      "HEAD",
      request_method(&report.requests_per_connection[0][0]),
      "{}",
      path.name()
    );
    // Redirect follow for HEAD may preserve HEAD or switch method depending on
    // status handling; either way the second request must be a distinct boundary.
    assert!(
      !report.requests_per_connection[0][1].is_empty(),
      "{} second request boundary must be captured",
      path.name()
    );
  }
}

#[test]
fn request_connection_close_is_observed_on_the_wire() {
  for path in client_paths() {
    let (addr, handle) =
      spawn_socket2_scripted_http_server(vec![ScriptedHttpConnection::stages([
        ScriptedHttpStage::respond_and_close(
          b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK".as_slice(),
        ),
      ])]);

    let response = path
      .emit(
        client()
          .get()
          .header(("Connection", "close"))
          .url(format!("http://{addr}/explicit-close")),
      )
      .unwrap_or_else(|err| panic!("{} request close should succeed: {err}", path.name()));
    assert_eq!(200, response.code(), "{}", path.name());

    let report = join_report(handle);
    let request = String::from_utf8_lossy(&report.requests_per_connection[0][0]);
    assert!(
      request
        .lines()
        .any(|line| line.to_ascii_lowercase().starts_with("connection:")
          && line.to_ascii_lowercase().contains("close")),
      "{} missing Connection: close on wire: {request}",
      path.name()
    );
  }
}

struct MalformedCase {
  name: &'static str,
  response: &'static [u8],
  error_needle: &'static str,
}

const MALFORMED_CASES: &[MalformedCase] = &[
  MalformedCase {
    name: "transfer-encoding conflicts with content-length",
    response: concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "0\r\n",
      "\r\n"
    )
    .as_bytes(),
    error_needle: "Content-Length",
  },
  MalformedCase {
    name: "conflicting content-length values",
    response: concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 6\r\n",
      "Connection: close\r\n",
      "\r\n",
      "hello!"
    )
    .as_bytes(),
    error_needle: "Content-Length",
  },
  MalformedCase {
    name: "incomplete fixed-length body",
    response: concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 10\r\n",
      "Connection: close\r\n",
      "\r\n",
      "short"
    )
    .as_bytes(),
    error_needle: "failed to fill whole buffer",
  },
  MalformedCase {
    name: "incomplete chunked body",
    response: concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Connection: close\r\n",
      "\r\n",
      "5\r\nab"
    )
    .as_bytes(),
    error_needle: "chunked",
  },
];

#[test]
fn malformed_wire_framing_fails_deterministically_and_separately_from_helpers() {
  for path in client_paths() {
    for case in MALFORMED_CASES {
      let (addr, handle) = spawn_socket2_scripted_http_server(vec![
        ScriptedHttpConnection::stages([ScriptedHttpStage::respond_and_close(case.response)]),
      ]);

      let error = path
        .emit(
          client()
            .get()
            .config(Config::builder().read_timeout(2_000))
            .url(format!(
              "http://{addr}/malformed/{}",
              case.name.replace(' ', "-")
            )),
        )
        .expect_err(&format!(
          "{} {} must fail closed on ambiguous/incomplete framing",
          path.name(),
          case.name
        ));

      let message = error.to_string();
      assert!(
        message.contains(case.error_needle)
          || message.to_ascii_lowercase().contains("unexpected")
          || message.contains("EOF")
          || message.contains("eof"),
        "{} {} unexpected error (wanted {:?}): {message}",
        path.name(),
        case.name,
        case.error_needle
      );

      let report = join_report(handle);
      assert_eq!(1, report.accept_count, "{} {}", path.name(), case.name);
      assert_eq!(
        1,
        report.requests_per_connection[0].len(),
        "{} {}",
        path.name(),
        case.name
      );
    }
  }
}

#[test]
fn truncated_stage_prefix_rejects_incomplete_fixed_length_body() {
  for path in client_paths() {
    let full = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789";
    let (addr, handle) =
      spawn_socket2_scripted_http_server(vec![ScriptedHttpConnection::stages([
        ScriptedHttpStage::truncated(full.as_slice(), full.len() - 4),
      ])]);

    let error = path
      .emit(
        client()
          .get()
          .config(Config::builder().read_timeout(2_000))
          .url(format!("http://{addr}/truncated")),
      )
      .expect_err(&format!("{} truncated body must fail", path.name()));

    assert!(
      error.to_string().contains("failed to fill whole buffer")
        || error
          .to_string()
          .to_ascii_lowercase()
          .contains("unexpected"),
      "{} unexpected truncation error: {error}",
      path.name()
    );
    let report = join_report(handle);
    assert_eq!(1, report.accept_count, "{}", path.name());
  }
}

#[test]
fn ambiguous_eof_without_framing_still_completes_as_until_eof_close() {
  // Until-EOF is valid when Connection: close is explicit; this is not a
  // malformed-wire case. Kept beside the malformed matrix only to document the
  // boundary between rejected ambiguity and legitimate close-delimited bodies.
  for path in client_paths() {
    let (addr, handle) =
      spawn_socket2_scripted_http_server(vec![ScriptedHttpConnection::stages([
        ScriptedHttpStage::respond_and_close(
          b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nuntil-eof".as_slice(),
        ),
      ])]);

    let response = path
      .emit(
        client()
          .get()
          .config(Config::builder().read_timeout(2_000))
          .url(format!("http://{addr}/until-eof")),
      )
      .unwrap_or_else(|err| panic!("{} until-EOF should succeed: {err}", path.name()));

    assert_eq!(
      "until-eof",
      response.body().string().unwrap(),
      "{}",
      path.name()
    );
    let _ = join_report(handle);
  }
}
