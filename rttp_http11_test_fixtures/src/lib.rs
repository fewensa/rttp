use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

pub mod request {
  pub struct FixedLengthRequest {
    pub raw: &'static [u8],
    pub method: &'static str,
    pub path: &'static str,
    pub query: Option<&'static str>,
    pub version: &'static str,
    pub host: &'static str,
    pub body: &'static [u8],
  }

  pub struct InvalidRequest {
    pub name: &'static str,
    pub raw: &'static [u8],
    pub error: &'static str,
  }

  pub struct ChunkedRequest {
    pub raw: &'static [u8],
    pub method: &'static str,
    pub target: &'static str,
    pub body: &'static [u8],
    pub trailers: &'static [(&'static str, &'static str)],
  }

  pub struct ExpectContinueRequest {
    pub head: &'static [u8],
    pub body: &'static [u8],
    pub target: &'static str,
  }

  pub fn fixed_length_post() -> FixedLengthRequest {
    FixedLengthRequest {
      raw: b"POST /matrix/fixed?case=shared HTTP/1.1\r\nHost: example.test\r\nContent-Length: 11\r\n\r\nhello=world",
      method: "POST",
      path: "/matrix/fixed",
      query: Some("case=shared"),
      version: "HTTP/1.1",
      host: "example.test",
      body: b"hello=world",
    }
  }

  pub fn invalid_host_and_target_cases() -> &'static [InvalidRequest] {
    &[
      InvalidRequest {
        name: "missing host",
        raw: b"GET /matrix HTTP/1.1\r\n\r\n",
        error: "HTTP/1.1 request requires exactly one Host header",
      },
      InvalidRequest {
        name: "duplicate host",
        raw: b"GET /matrix HTTP/1.1\r\nHost: example.test\r\nHost: other.test\r\n\r\n",
        error: "HTTP/1.1 request requires exactly one Host header",
      },
      InvalidRequest {
        name: "invalid origin target",
        raw: b"GET matrix HTTP/1.1\r\nHost: example.test\r\n\r\n",
        error: "invalid request target",
      },
      InvalidRequest {
        name: "connect authority host mismatch",
        raw: b"CONNECT example.test:443 HTTP/1.1\r\nHost: other.test:443\r\n\r\n",
        error: "invalid Host header",
      },
    ]
  }

  pub fn framing_ambiguity_cases() -> &'static [InvalidRequest] {
    &[
      InvalidRequest {
        name: "conflicting content length",
        raw: b"POST /matrix HTTP/1.1\r\nHost: example.test\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello",
        error: "conflicting Content-Length headers",
      },
      InvalidRequest {
        name: "transfer encoding with content length",
        raw: b"POST /matrix HTTP/1.1\r\nHost: example.test\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\nhello",
        error: "Transfer-Encoding conflicts with Content-Length",
      },
    ]
  }

  pub fn chunked_with_extensions_and_trailers() -> ChunkedRequest {
    ChunkedRequest {
      raw: concat!(
        "POST /matrix/chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "5;foo=\"bar;baz\";answer=42\r\n",
        "hello\r\n",
        "6;token=value\r\n",
        " world\r\n",
        "0\r\n",
        "X-Trace: abc\r\n",
        "X-Signature: signed\r\n",
        "\r\n"
      )
      .as_bytes(),
      method: "POST",
      target: "/matrix/chunked",
      body: b"hello world",
      trailers: &[("x-trace", "abc"), ("X-SIGNATURE", "signed")],
    }
  }

  pub fn keep_alive_pipeline() -> &'static [u8] {
    concat!(
      "POST /matrix/first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: keep-alive\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "alpha",
      "POST /matrix/second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: close\r\n",
      "Content-Length: 6\r\n",
      "\r\n",
      "bravo!"
    )
    .as_bytes()
  }

  pub fn expect_continue_fixed_length() -> ExpectContinueRequest {
    ExpectContinueRequest {
      head: concat!(
        "POST /matrix/continue HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Expect: 100-continue\r\n",
        "Content-Length: 12\r\n",
        "\r\n"
      )
      .as_bytes(),
      body: b"request body",
      target: "/matrix/continue",
    }
  }
}

pub mod response {
  pub const CONTINUE: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n";

  pub const CHUNKED_WITH_EXTENSIONS_AND_TRAILERS: &[u8] = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "\r\n",
    "7;foo=\"bar;baz\";answer=42\r\n",
    "chunked\r\n",
    "6;token=value\r\n",
    " body!\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  )
  .as_bytes();

  pub const TRANSFER_ENCODING_WITH_CONTENT_LENGTH: &[u8] = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 13\r\n",
    "\r\n",
    "0\r\n",
    "\r\n"
  )
  .as_bytes();
}

pub fn bind_socket2_tcp_listener(name: &str) -> (TcpListener, SocketAddr) {
  let addr: SocketAddr = "127.0.0.1:0".parse().expect("parse local addr");
  let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
    .unwrap_or_else(|err| panic!("create {name} socket: {err}"));
  socket
    .set_reuse_address(true)
    .unwrap_or_else(|err| panic!("set {name} reuse addr: {err}"));
  socket
    .bind(&addr.into())
    .unwrap_or_else(|err| panic!("bind {name}: {err}"));
  socket
    .listen(16)
    .unwrap_or_else(|err| panic!("listen {name}: {err}"));
  let listener = TcpListener::from(socket);
  let addr = listener
    .local_addr()
    .unwrap_or_else(|err| panic!("read {name} local addr: {err}"));
  (listener, addr)
}

pub fn read_http_request<R: Read>(stream: &mut R) -> Vec<u8> {
  let mut request = Vec::new();
  let mut buf = [0u8; 1024];
  let mut content_length = None;

  loop {
    let Ok(read) = stream.read(&mut buf) else {
      break;
    };
    if read == 0 {
      break;
    }

    request.extend_from_slice(&buf[..read]);

    let header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
    if content_length.is_none() {
      if let Some(header_end) = header_end {
        let headers = String::from_utf8_lossy(&request[..header_end + 4]);
        content_length = headers
          .lines()
          .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
              value.trim().parse::<usize>().ok()
            } else {
              None
            }
          })
          .or(Some(0));
      }
    }

    if let (Some(header_end), Some(content_length)) = (header_end, content_length) {
      let expected_len = header_end + 4 + content_length;
      if request.len() >= expected_len {
        break;
      }
    }
  }

  request
}

pub fn spawn_socket2_raw_response_server(
  response: &'static [u8],
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_socket2_tcp_listener("raw response server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set read timeout");
    let request = read_http_request(&mut stream);
    let _ = stream.write_all(response);
    request
  });
  (addr, handle)
}

pub fn spawn_socket2_expect_continue_server(
  final_response: &'static [u8],
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_socket2_tcp_listener("expect continue server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set read timeout");

    serve_expect_continue_stream(&mut stream, final_response)
  });
  (addr, handle)
}

fn serve_expect_continue_stream<S: Read + Write>(stream: &mut S, final_response: &[u8]) -> Vec<u8> {
  let mut request = read_until_header_end(stream);
  let content_length = request_content_length(&request).unwrap_or(0);

  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| position + 4)
    .unwrap_or(request.len());
  let body_bytes_read = request.len().saturating_sub(header_end);
  if body_bytes_read != 0 {
    return Vec::new();
  }

  let _ = stream.write_all(response::CONTINUE);

  if body_bytes_read < content_length {
    let mut body = vec![0; content_length - body_bytes_read];
    if stream.read_exact(&mut body).is_ok() {
      request.extend_from_slice(&body);
    }
  }

  let _ = stream.write_all(final_response);
  request
}

fn read_until_header_end<R: Read>(stream: &mut R) -> Vec<u8> {
  let mut request = Vec::new();
  let mut buf = [0u8; 256];

  loop {
    let Ok(read) = stream.read(&mut buf) else {
      break;
    };
    if read == 0 {
      break;
    }
    request.extend_from_slice(&buf[..read]);
    if request.windows(4).any(|window| window == b"\r\n\r\n") {
      break;
    }
  }

  request
}

fn request_content_length(request: &[u8]) -> Option<usize> {
  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")?;
  let headers = String::from_utf8_lossy(&request[..header_end + 4]);
  headers.lines().find_map(|line| {
    let (name, value) = line.split_once(':')?;
    if name.eq_ignore_ascii_case("content-length") {
      value.trim().parse::<usize>().ok()
    } else {
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::{request, serve_expect_continue_stream};
  use std::io::{self, Read, Write};

  struct InMemoryStream {
    read: Vec<u8>,
    written: Vec<u8>,
  }

  impl InMemoryStream {
    fn new(read: Vec<u8>) -> Self {
      Self {
        read,
        written: Vec::new(),
      }
    }
  }

  impl Read for InMemoryStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      let read = buf.len().min(self.read.len());
      buf[..read].copy_from_slice(&self.read[..read]);
      self.read.drain(..read);
      Ok(read)
    }
  }

  impl Write for InMemoryStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      self.written.extend_from_slice(buf);
      Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn expect_continue_server_rejects_premature_body_bytes() {
    let fixture = request::expect_continue_fixed_length();
    let mut request = Vec::new();
    request.extend_from_slice(fixture.head);
    request.extend_from_slice(fixture.body);
    let mut stream = InMemoryStream::new(request);

    assert_eq!(
      Vec::<u8>::new(),
      serve_expect_continue_stream(&mut stream, b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
    );
    assert!(
      !stream
        .written
        .windows(super::response::CONTINUE.len())
        .any(|window| window == super::response::CONTINUE),
      "server must not send 100 Continue after premature body"
    );
  }
}
