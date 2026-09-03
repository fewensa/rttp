use std::io::Write;
use std::thread;
use std::time::Duration;

use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::{Config, HttpClient};
use rttp_server::server::{HttpResponse, HttpServer};

#[derive(Clone, Copy)]
enum Transport {
  Http11Sync,
  #[cfg(feature = "async")]
  Http11Async,
  H2cPriorKnowledge,
  H2cUpgrade,
}

impl Transport {
  fn name(self) -> &'static str {
    match self {
      Self::Http11Sync => "http11-sync",
      #[cfg(feature = "async")]
      Self::Http11Async => "http11-async",
      Self::H2cPriorKnowledge => "h2c-prior-knowledge",
      Self::H2cUpgrade => "h2c-upgrade",
    }
  }

  fn emit(
    self,
    client: &mut HttpClient,
  ) -> Result<rttp_client::response::Response, rttp_client::error::Error> {
    match self {
      Self::Http11Sync => client.emit(),
      #[cfg(feature = "async")]
      Self::Http11Async => block_on(client.rasync()),
      Self::H2cPriorKnowledge => client.emit_http2_prior_knowledge(),
      Self::H2cUpgrade => client.emit_http2_upgrade(),
    }
  }
}

fn transports() -> Vec<Transport> {
  vec![
    Transport::Http11Sync,
    #[cfg(feature = "async")]
    Transport::Http11Async,
    Transport::H2cPriorKnowledge,
    Transport::H2cUpgrade,
  ]
}

enum Outcome {
  Decoded,
  Preserved { encoding_ok: bool },
  DecodeError,
  BodyTooLarge { limit: usize },
}

struct Fixture {
  name: &'static str,
  encoding: &'static str,
  body: Vec<u8>,
  expected_body: Vec<u8>,
  decoded_limit: Option<usize>,
  outcome: Outcome,
}

fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write gzip fixture");
  encoder.finish().expect("finish gzip fixture")
}

fn zlib_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write zlib fixture");
  encoder.finish().expect("finish zlib fixture")
}

fn raw_deflate_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write raw deflate fixture");
  encoder.finish().expect("finish raw deflate fixture")
}

fn truncated_zlib() -> Vec<u8> {
  let mut body = zlib_bytes(b"OK");
  body.pop();
  body
}

fn fixtures() -> Vec<Fixture> {
  let gzip_ok = gzip_bytes(b"OK");
  let zlib_ok = zlib_bytes(b"OK");
  let exact_limit = vec![b'a'; 64];
  vec![
    Fixture {
      name: "gzip",
      encoding: "gzip",
      body: gzip_ok.clone(),
      expected_body: b"OK".to_vec(),
      decoded_limit: None,
      outcome: Outcome::Decoded,
    },
    Fixture {
      name: "zlib-deflate",
      encoding: "deflate",
      body: zlib_ok.clone(),
      expected_body: b"OK".to_vec(),
      decoded_limit: None,
      outcome: Outcome::Decoded,
    },
    Fixture {
      name: "gzip-then-deflate",
      encoding: "gzip, deflate",
      body: zlib_bytes(&gzip_ok),
      expected_body: b"OK".to_vec(),
      decoded_limit: None,
      outcome: Outcome::Decoded,
    },
    Fixture {
      name: "deflate-then-gzip",
      encoding: "deflate, gzip",
      body: gzip_bytes(&zlib_ok),
      expected_body: b"OK".to_vec(),
      decoded_limit: None,
      outcome: Outcome::Decoded,
    },
    Fixture {
      name: "unknown-mixed",
      encoding: "gzip, br",
      body: gzip_ok.clone(),
      expected_body: gzip_ok.clone(),
      decoded_limit: None,
      outcome: Outcome::Preserved { encoding_ok: true },
    },
    Fixture {
      name: "parse-invalid",
      encoding: "gzip,",
      body: gzip_ok.clone(),
      expected_body: gzip_ok,
      decoded_limit: None,
      outcome: Outcome::Preserved { encoding_ok: false },
    },
    Fixture {
      name: "identity",
      encoding: "identity",
      body: b"OK".to_vec(),
      expected_body: b"OK".to_vec(),
      decoded_limit: None,
      outcome: Outcome::Preserved { encoding_ok: true },
    },
    Fixture {
      name: "empty-gzip",
      encoding: "gzip",
      body: Vec::new(),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::Preserved { encoding_ok: true },
    },
    Fixture {
      name: "exact-limit",
      encoding: "gzip",
      body: gzip_bytes(&exact_limit),
      expected_body: exact_limit,
      decoded_limit: Some(64),
      outcome: Outcome::Decoded,
    },
    Fixture {
      name: "over-limit",
      encoding: "gzip",
      body: gzip_bytes(&[b'a'; 65]),
      expected_body: Vec::new(),
      decoded_limit: Some(64),
      outcome: Outcome::BodyTooLarge { limit: 64 },
    },
    Fixture {
      name: "malformed-gzip",
      encoding: "gzip",
      body: b"not-gzip".to_vec(),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::DecodeError,
    },
    Fixture {
      name: "malformed-deflate",
      encoding: "deflate",
      body: b"not-zlib".to_vec(),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::DecodeError,
    },
    Fixture {
      name: "truncated-deflate",
      encoding: "deflate",
      body: truncated_zlib(),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::DecodeError,
    },
    Fixture {
      name: "raw-deflate",
      encoding: "deflate",
      body: raw_deflate_bytes(b"OK"),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::DecodeError,
    },
    Fixture {
      name: "malformed-inner-stack",
      encoding: "gzip, deflate",
      body: zlib_bytes(b"not-gzip"),
      expected_body: Vec::new(),
      decoded_limit: None,
      outcome: Outcome::DecodeError,
    },
  ]
}

fn serve_encoded(encoding: &str, body: Vec<u8>) -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind decoding matrix server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("decoding matrix addr");
  let encoding = encoding.to_string();
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| HttpResponse::ok(body).header("Content-Encoding", encoding))
      .expect("serve decoding matrix request");
  });
  (addr, handle)
}

fn client_for(limit: Option<usize>) -> HttpClient {
  let mut client = rttp::Http::client();
  if let Some(limit) = limit {
    client.config(
      Config::builder()
        .max_buffered_response_body_bytes(limit)
        .build(),
    );
  }
  client
}

fn binary_contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
  haystack.windows(needle.len()).any(|window| {
    window.len() == needle.len()
      && window
        .iter()
        .zip(needle)
        .all(|(left, right)| left.eq_ignore_ascii_case(right))
  })
}

fn assert_decode_error(error: &rttp_client::error::Error, transport: Transport, name: &str) {
  assert!(
    error
      .to_string()
      .starts_with("error decoding response body"),
    "{}/{} unexpected error: {error}",
    transport.name(),
    name
  );
  assert!(
    !error.is_body_too_large(),
    "{}/{} classified as body-too-large: {error}",
    transport.name(),
    name
  );
}

fn assert_decoded(
  response: &rttp_client::response::Response,
  fixture: &Fixture,
  transport: Transport,
) {
  assert_eq!(
    fixture.expected_body,
    response.body().binary(),
    "{}/{} decoded body",
    transport.name(),
    fixture.name
  );
  assert!(
    response.header("Content-Encoding").is_none(),
    "{}/{} Content-Encoding",
    transport.name(),
    fixture.name
  );
  assert!(
    response.header("Content-Length").is_none(),
    "{}/{} Content-Length header",
    transport.name(),
    fixture.name
  );
  assert!(
    response
      .content_encoding()
      .unwrap_or_else(|_| panic!(
        "{}/{} content_encoding parse",
        transport.name(),
        fixture.name
      ))
      .is_none(),
    "{}/{} typed Content-Encoding",
    transport.name(),
    fixture.name
  );
  assert!(
    response.content_length().is_none(),
    "{}/{} typed Content-Length",
    transport.name(),
    fixture.name
  );
  let capture = format!("Content-Encoding: {}", fixture.encoding);
  assert!(
    binary_contains_ignore_ascii_case(response.binary(), capture.as_bytes()),
    "{}/{} missing capture {capture}",
    transport.name(),
    fixture.name
  );
}

fn assert_preserved(
  response: &rttp_client::response::Response,
  fixture: &Fixture,
  transport: Transport,
  encoding_ok: bool,
) {
  assert_eq!(
    fixture.expected_body,
    response.body().binary(),
    "{}/{} preserved body",
    transport.name(),
    fixture.name
  );
  assert_eq!(
    Some(fixture.encoding),
    response
      .header_value("Content-Encoding")
      .map(String::as_str),
    "{}/{} Content-Encoding",
    transport.name(),
    fixture.name
  );
  assert_eq!(
    Some(fixture.body.len().to_string()),
    response.header_value("Content-Length").cloned(),
    "{}/{} Content-Length header",
    transport.name(),
    fixture.name
  );
  if encoding_ok {
    let parsed = response
      .content_encoding()
      .unwrap_or_else(|_| panic!("{}/{} content_encoding", transport.name(), fixture.name))
      .unwrap_or_else(|| {
        panic!(
          "{}/{} missing typed Content-Encoding",
          transport.name(),
          fixture.name
        )
      });
    let expected: Vec<&str> = fixture
      .encoding
      .split(',')
      .map(str::trim)
      .filter(|coding| !coding.is_empty())
      .collect();
    assert_eq!(
      expected,
      parsed.codings(),
      "{}/{} typed Content-Encoding",
      transport.name(),
      fixture.name
    );
  } else {
    assert!(
      response.content_encoding().is_err(),
      "{}/{} parse-invalid Content-Encoding",
      transport.name(),
      fixture.name
    );
  }
  assert_eq!(
    Some(fixture.body.len()),
    response.content_length().map(|length| length.len()),
    "{}/{} typed Content-Length",
    transport.name(),
    fixture.name
  );
  let capture = format!("Content-Encoding: {}", fixture.encoding);
  assert!(
    binary_contains_ignore_ascii_case(response.binary(), capture.as_bytes()),
    "{}/{} missing capture {capture}",
    transport.name(),
    fixture.name
  );
}

#[test]
fn response_decoding_transport_matrix() {
  for fixture in fixtures() {
    let mut bodies = Vec::new();
    let mut error_kinds = Vec::new();
    for transport in transports() {
      let (addr, handle) = serve_encoded(fixture.encoding, fixture.body.clone());
      let mut client = client_for(fixture.decoded_limit);
      client.get().url(format!("http://{addr}/{}", fixture.name));
      let result = transport.emit(&mut client);
      match &fixture.outcome {
        Outcome::Decoded => {
          let response = result.unwrap_or_else(|error| {
            panic!(
              "{}/{} should succeed: {error}",
              transport.name(),
              fixture.name
            )
          });
          assert_decoded(&response, &fixture, transport);
          bodies.push((transport.name(), response.body().binary().to_vec()));
        }
        Outcome::Preserved { encoding_ok } => {
          let response = result.unwrap_or_else(|error| {
            panic!(
              "{}/{} should succeed: {error}",
              transport.name(),
              fixture.name
            )
          });
          assert_preserved(&response, &fixture, transport, *encoding_ok);
          bodies.push((transport.name(), response.body().binary().to_vec()));
        }
        Outcome::DecodeError => {
          let error = result.expect_err(&format!(
            "{}/{} should fail decode",
            transport.name(),
            fixture.name
          ));
          assert_decode_error(&error, transport, fixture.name);
          error_kinds.push((
            transport.name(),
            error.to_string(),
            error.is_body_too_large(),
            error.body_limit(),
          ));
        }
        Outcome::BodyTooLarge { limit } => {
          let error = result.expect_err(&format!(
            "{}/{} should fail body limit",
            transport.name(),
            fixture.name
          ));
          assert!(
            error.is_body_too_large(),
            "{}/{} unexpected error: {error}",
            transport.name(),
            fixture.name
          );
          assert_eq!(
            Some(*limit),
            error.body_limit(),
            "{}/{} body limit",
            transport.name(),
            fixture.name
          );
          error_kinds.push((
            transport.name(),
            error.to_string(),
            error.is_body_too_large(),
            error.body_limit(),
          ));
        }
      }
      handle.join().expect("decoding matrix server thread");
    }

    if let Some((_, expected)) = bodies.first() {
      for (name, body) in &bodies {
        assert_eq!(
          expected, body,
          "{name} bytes diverge from {} on {}",
          bodies[0].0, fixture.name
        );
      }
    }
    if let Some((_, expected_message, expected_too_large, expected_limit)) = error_kinds.first() {
      let expected_kind = expected_message.split(':').next();
      for (name, message, too_large, limit) in &error_kinds {
        assert_eq!(
          expected_too_large, too_large,
          "{name} error classification diverges from {} on {}",
          error_kinds[0].0, fixture.name
        );
        assert_eq!(
          expected_limit, limit,
          "{name} body limit diverges from {} on {}",
          error_kinds[0].0, fixture.name
        );
        assert_eq!(
          expected_kind,
          message.split(':').next(),
          "{name} error kind diverges from {} on {}",
          error_kinds[0].0,
          fixture.name
        );
      }
    }
  }
}
