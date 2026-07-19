#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

#[path = "local_http.rs"]
mod local_http;

pub use local_http::read_http_request;
use local_http::{bind_local_http_listener, HTTP_OK_RESPONSE};

fn read_exact_bytes<R: Read>(stream: &mut R, len: usize) -> io::Result<Vec<u8>> {
  let mut bytes = vec![0u8; len];
  stream.read_exact(&mut bytes)?;
  Ok(bytes)
}

fn socks5_target_addr(stream: &mut TcpStream, auth: Option<(&str, &str)>) -> io::Result<String> {
  let header = read_exact_bytes(stream, 2)?;
  let methods = read_exact_bytes(stream, header[1] as usize)?;

  match auth {
    Some((username, password)) => {
      if !methods.contains(&0x02) {
        return Err(io::Error::other(
          "client does not support username/password auth",
        ));
      }
      stream.write_all(&[0x05, 0x02])?;

      let auth_header = read_exact_bytes(stream, 2)?;
      let user = read_exact_bytes(stream, auth_header[1] as usize)?;
      let password_len = read_exact_bytes(stream, 1)?[0] as usize;
      let password_bytes = read_exact_bytes(stream, password_len)?;
      let auth_ok = user == username.as_bytes() && password_bytes == password.as_bytes();
      stream.write_all(&[0x01, if auth_ok { 0x00 } else { 0x01 }])?;
      if !auth_ok {
        return Err(io::Error::other("invalid socks5 credentials"));
      }
    }
    None => {
      if !methods.contains(&0x00) {
        return Err(io::Error::other("client does not support no-auth socks5"));
      }
      stream.write_all(&[0x05, 0x00])?;
    }
  }

  let request = read_exact_bytes(stream, 4)?;
  if request[0] != 0x05 || request[1] != 0x01 {
    return Err(io::Error::other("unsupported socks5 command"));
  }

  let host = match request[3] {
    0x01 => {
      let ip = read_exact_bytes(stream, 4)?;
      Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]).to_string()
    }
    0x03 => {
      let len = read_exact_bytes(stream, 1)?[0] as usize;
      String::from_utf8(read_exact_bytes(stream, len)?)
        .map_err(|_| io::Error::other("invalid domain name"))?
    }
    0x04 => {
      let ip = read_exact_bytes(stream, 16)?;
      let mut segments = [0u16; 8];
      for (idx, chunk) in ip.chunks_exact(2).enumerate() {
        segments[idx] = u16::from_be_bytes([chunk[0], chunk[1]]);
      }
      Ipv6Addr::from(segments).to_string()
    }
    _ => return Err(io::Error::other("unsupported socks5 address type")),
  };

  let port = read_exact_bytes(stream, 2)?;
  let port = u16::from_be_bytes([port[0], port[1]]);

  stream.write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0, 0])?;

  Ok(format!("{}:{}", host, port))
}

fn proxy_http_request(mut stream: TcpStream, auth: Option<(&str, &str)>) -> io::Result<()> {
  let target_addr = socks5_target_addr(&mut stream, auth)?;
  let mut target = TcpStream::connect(target_addr)?;
  let request = read_http_request(&mut stream);
  target.write_all(&request)?;
  target.flush()?;

  let mut response = Vec::new();
  target.read_to_end(&mut response)?;
  stream.write_all(&response)?;
  stream.flush()?;
  Ok(())
}

fn header_value(request: &[u8], name: &str) -> Option<String> {
  String::from_utf8_lossy(request).lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    if header_name.eq_ignore_ascii_case(name) {
      Some(value.trim().to_string())
    } else {
      None
    }
  })
}

pub fn spawn_http_server() -> (SocketAddr, JoinHandle<()>) {
  local_http::spawn_ok_http_server()
}

pub fn spawn_http_server_count(count: usize) -> (SocketAddr, JoinHandle<()>) {
  local_http::spawn_ok_http_server_count(count)
}

pub fn capture_raw_http_request() -> (SocketAddr, JoinHandle<Vec<u8>>) {
  local_http::capture_raw_http_request()
}

pub fn capture_optional_raw_http_request(timeout: Duration) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  local_http::capture_optional_raw_http_request(timeout)
}

pub fn spawn_chunked_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=bar\r\nchunked\r\n",
    "6\r\n body!\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ))
}

pub fn spawn_socket2_chunked_trailer_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("chunked trailer server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let _ = stream.write_all(
        concat!(
          "HTTP/1.1 200 OK\r\n",
          "Transfer-Encoding: chunked\r\n",
          "Connection: close\r\n",
          "\r\n",
          "14\r\nsocket2 chunked body\r\n",
          "0\r\n",
          "X-Trace: abc\r\n",
          "X-Signature: signed\r\n",
          "\r\n"
        )
        .as_bytes(),
      );
    }
  });
  (addr, handle)
}

pub fn spawn_chunked_response_server(response: impl Into<Vec<u8>>) -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("chunked server");
  let response = response.into();
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let _ = stream.write_all(&response);
    }
  });
  (addr, handle)
}

enum GatedResponseBodyControl {
  Release,
  Cancel,
}

pub struct GatedResponseBody {
  addr: SocketAddr,
  partial_body_sent: Receiver<()>,
  control: Sender<GatedResponseBodyControl>,
  handle: JoinHandle<()>,
}

impl GatedResponseBody {
  pub fn addr(&self) -> SocketAddr {
    self.addr
  }

  pub fn wait_for_partial_body(&self, timeout: Duration) -> Result<(), RecvTimeoutError> {
    self.partial_body_sent.recv_timeout(timeout)
  }

  pub fn release_body(&self) -> Result<(), ()> {
    self
      .control
      .send(GatedResponseBodyControl::Release)
      .map_err(|_| ())
  }

  pub fn cancel_body(&self) -> Result<(), ()> {
    self
      .control
      .send(GatedResponseBodyControl::Cancel)
      .map_err(|_| ())
  }

  pub fn join(self) -> thread::Result<()> {
    self.handle.join()
  }
}

pub fn spawn_gated_response_body(
  partial_body: impl Into<Vec<u8>>,
  remaining_body: impl Into<Vec<u8>>,
) -> GatedResponseBody {
  let (listener, addr) = bind_local_http_listener("gated response body server");
  let (partial_body_sent_tx, partial_body_sent) = mpsc::channel();
  let (control, control_rx) = mpsc::channel();
  let partial_body = partial_body.into();
  let remaining_body = remaining_body.into();
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return;
    };
    let _ = read_http_request(&mut stream);
    let response_head = format!(
      "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
      partial_body.len() + remaining_body.len()
    );
    if stream.write_all(response_head.as_bytes()).is_err()
      || stream.write_all(&partial_body).is_err()
      || stream.flush().is_err()
    {
      return;
    }
    if partial_body_sent_tx.send(()).is_err() {
      return;
    }
    if matches!(control_rx.recv(), Ok(GatedResponseBodyControl::Release)) {
      let _ = stream.write_all(&remaining_body);
      let _ = stream.flush();
    }
  });

  GatedResponseBody {
    addr,
    partial_body_sent,
    control,
    handle,
  }
}

pub fn spawn_chunked_server_without_trailers() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("chunked server without trailers");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Transfer-Encoding: chunked\r\n",
        "Connection: close\r\n",
        "\r\n",
        "2\r\nOK\r\n",
        "0\r\n",
        "\r\n"
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_duplicate_set_cookie_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("duplicate set-cookie server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/plain\r\n",
        "Set-Cookie: session=abc; Path=/; HttpOnly\r\n",
        "cache-control: no-cache\r\n",
        "SET-COOKIE: theme=dark; Path=/; SameSite=Lax\r\n",
        "Cache-Control: max-age=60\r\n",
        "Content-Length: 2\r\n",
        "Connection: close\r\n",
        "\r\n",
        "OK"
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_redirect_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("redirect server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let _ = stream.write_all(HTTP_OK_RESPONSE);
    }
  });
  (addr, handle)
}

pub fn spawn_redirect_target_echo_server<F>(location: F) -> (SocketAddr, JoinHandle<()>)
where
  F: FnOnce(SocketAddr) -> String + Send + 'static,
{
  let (listener, addr) = bind_local_http_listener("redirect target echo server");
  let handle = thread::spawn(move || {
    let location = location(addr);
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        location
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let target = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("")
        .to_string();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        target.len(),
        target
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_redirect_chain_server(
  redirects: Vec<(&'static str, &'static str)>,
  max_requests: usize,
) -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("redirect chain server");
  let handle = thread::spawn(move || {
    let redirects: HashMap<&'static str, &'static str> = redirects.into_iter().collect();

    for _ in 0..max_requests {
      if let Ok((mut stream, _)) = listener.accept() {
        let request = read_http_request(&mut stream);
        let target = String::from_utf8_lossy(&request)
          .lines()
          .next()
          .and_then(|line| line.split_whitespace().nth(1))
          .unwrap_or("")
          .to_string();

        if let Some(location) = redirects.get(target.as_str()) {
          let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            location
          );
          let _ = stream.write_all(response.as_bytes());
        } else {
          let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            target.len(),
            target
          );
          let _ = stream.write_all(response.as_bytes());
        }
      }
    }
  });
  (addr, handle)
}

pub fn spawn_status_redirect_request_capture_server(
  status_code: u16,
  reason: &'static str,
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_local_http_listener("status redirect request capture server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 {} {}\r\nLocation: http://{}/final?via=redirect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        status_code, reason, addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let _ = stream.write_all(HTTP_OK_RESPONSE);
      return request;
    }

    Vec::new()
  });
  (addr, handle)
}

pub fn spawn_same_url_303_redirect_method_echo_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("same url 303 redirect method echo server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response =
        "HTTP/1.1 303 See Other\r\nLocation: /submit\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let request_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        request_line.len(),
        request_line
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_cross_authority_redirect_host_echo_server() -> (SocketAddr, SocketAddr, JoinHandle<()>)
{
  let (origin_listener, origin_addr) = bind_local_http_listener("cross authority redirect origin");
  let (target_listener, target_addr) = bind_local_http_listener("cross authority redirect target");

  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = origin_listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        target_addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = target_listener.accept() {
      let request = read_http_request(&mut stream);
      let host = header_value(&request, "Host").unwrap_or_default();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        host.len(),
        host
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });

  (origin_addr, target_addr, handle)
}

pub fn spawn_cross_authority_redirect_header_echo_server(
) -> (SocketAddr, SocketAddr, JoinHandle<()>) {
  let (origin_listener, origin_addr) = bind_local_http_listener("cross authority redirect origin");
  let (target_listener, target_addr) = bind_local_http_listener("cross authority redirect target");

  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = origin_listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        target_addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = target_listener.accept() {
      let request = read_http_request(&mut stream);
      let body = echoed_redirect_headers(&request);
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });

  (origin_addr, target_addr, handle)
}

pub fn spawn_cross_authority_redirect_userinfo_echo_server(
) -> (SocketAddr, SocketAddr, JoinHandle<()>) {
  let (origin_listener, origin_addr) = bind_local_http_listener("cross authority redirect origin");
  let (target_listener, target_addr) = bind_local_http_listener("cross authority redirect target");

  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = origin_listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://user:secret@{}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        target_addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = target_listener.accept() {
      let request = read_http_request(&mut stream);
      let body = echoed_redirect_request_target_and_headers(&request);
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });

  (origin_addr, target_addr, handle)
}

pub fn spawn_same_authority_redirect_header_echo_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("same authority redirect");

  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response =
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let body = echoed_redirect_headers(&request);
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });

  (addr, handle)
}

fn echoed_redirect_headers(request: &[u8]) -> String {
  format!(
    "authorization={}\ncookie={}\nproxy-authorization={}\nx-trace={}",
    header_value(request, "Authorization").unwrap_or_default(),
    header_value(request, "Cookie").unwrap_or_default(),
    header_value(request, "Proxy-Authorization").unwrap_or_default(),
    header_value(request, "X-Trace").unwrap_or_default()
  )
}

fn echoed_redirect_request_target_and_headers(request: &[u8]) -> String {
  let request_text = String::from_utf8_lossy(request);
  let request_target = request_text
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(1))
    .unwrap_or_default();
  format!(
    "request-target={}\n{}",
    request_target,
    echoed_redirect_headers(request)
  )
}

pub fn spawn_keep_alive_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_keep_alive_server_count(1)
}

pub fn spawn_keep_alive_server_count(count: usize) -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("keep-alive server");
  let handle = thread::spawn(move || {
    for _ in 0..count {
      let Ok((mut stream, _)) = listener.accept() else {
        break;
      };
      let _ = read_http_request(&mut stream);
      let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nOK";
      let _ = stream.write_all(response);
      if count == 1 {
        thread::sleep(Duration::from_millis(300));
      }
    }
  });
  (addr, handle)
}

pub fn spawn_redirect_connection_lifecycle_server(
  first_response_connection_close: bool,
) -> (SocketAddr, JoinHandle<Vec<usize>>) {
  let (listener, addr) = bind_local_http_listener("redirect connection lifecycle server");
  let handle = thread::spawn(move || {
    let Ok((mut first_stream, _)) = listener.accept() else {
      return Vec::new();
    };

    let mut requests_per_connection = vec![1];
    let _ = read_http_request(&mut first_stream);
    let connection_header = if first_response_connection_close {
      "Connection: close\r\n"
    } else {
      ""
    };
    let redirect = format!(
      "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n{}\r\n",
      connection_header
    );
    let _ = first_stream.write_all(redirect.as_bytes());

    if first_response_connection_close {
      drop(first_stream);
    } else {
      first_stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .expect("set redirect lifecycle read timeout");
      match read_http_request(&mut first_stream) {
        request if !request.is_empty() => {
          requests_per_connection[0] += 1;
          let _ = first_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfinal");
          return requests_per_connection;
        }
        _ => {}
      }
    }

    let Ok((mut second_stream, _)) = listener.accept() else {
      return requests_per_connection;
    };
    requests_per_connection.push(1);
    let _ = read_http_request(&mut second_stream);
    let _ = second_stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfinal");
    requests_per_connection
  });
  (addr, handle)
}

pub fn spawn_eof_delimited_response_server(body: &'static str) -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("eof delimited response server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = concat!("HTTP/1.1 200 OK\r\n", "Connection: close\r\n", "\r\n");
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(body.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_truncated_content_length_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("truncated content length server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 10\r\n",
        "Connection: close\r\n",
        "\r\n",
        "short"
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  (addr, handle)
}

pub fn spawn_continue_then_ok_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_informational_then_ok_server("100 Continue")
}

pub fn spawn_expect_continue_gate_server() -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_local_http_listener("expect continue gate server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };

    stream
      .set_read_timeout(Some(Duration::from_millis(100)))
      .expect("set gate read timeout");

    let mut request_head = Vec::new();
    let mut byte = [0u8; 1];
    while !request_head.ends_with(b"\r\n\r\n") {
      stream
        .read_exact(&mut byte)
        .expect("read expect request head");
      request_head.push(byte[0]);
    }

    let mut request = request_head;
    let mut body = [0u8; 12];
    stream.read_exact(&mut body).expect("read expect body");
    request.extend_from_slice(&body);

    stream
      .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
      .expect("write continue");

    stream
      .write_all(
        concat!(
          "HTTP/1.1 200 OK\r\n",
          "Content-Length: 8\r\n",
          "Connection: close\r\n",
          "\r\n",
          "accepted"
        )
        .as_bytes(),
      )
      .expect("write final response");

    request
  });
  (addr, handle)
}

pub fn spawn_expect_continue_reject_gate_server() -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_local_http_listener("expect continue reject gate server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };

    stream
      .set_read_timeout(Some(Duration::from_millis(100)))
      .expect("set reject gate read timeout");

    let mut request_head = Vec::new();
    let mut byte = [0u8; 1];
    while !request_head.ends_with(b"\r\n\r\n") {
      stream
        .read_exact(&mut byte)
        .expect("read expect request head");
      request_head.push(byte[0]);
    }

    let mut body = [0u8; 12];
    stream.read_exact(&mut body).expect("read expect body");
    request_head.extend_from_slice(&body);

    stream
      .write_all(
        concat!(
          "HTTP/1.1 417 Expectation Failed\r\n",
          "Content-Length: 18\r\n",
          "Connection: close\r\n",
          "\r\n",
          "Expectation Failed"
        )
        .as_bytes(),
      )
      .expect("write expectation failed");

    request_head
  });
  (addr, handle)
}

pub fn spawn_informational_then_ok_server(status: &'static str) -> (SocketAddr, JoinHandle<()>) {
  let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
    .expect("create continue server socket");
  socket
    .bind(&SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into())
    .expect("bind continue server");
  socket.listen(1).expect("listen continue server");
  let listener = TcpListener::from(socket);
  let addr = listener.local_addr().expect("continue server addr");

  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let mut request_head = Vec::new();
      let mut byte = [0u8; 1];
      while !request_head.ends_with(b"\r\n\r\n") {
        if stream.read_exact(&mut byte).is_err() {
          return;
        }
        request_head.push(byte[0]);
      }

      let response = format!(
        concat!(
          "HTTP/1.1 {}\r\n",
          "X-Interim: ignored\r\n",
          "\r\n",
          "HTTP/1.1 200 OK\r\n",
          "Content-Type: text/plain\r\n",
          "X-Final: yes\r\n",
          "Content-Length: 10\r\n",
          "Connection: close\r\n",
          "\r\n",
          "final body"
        ),
        status
      );
      let Some((interim, final_response)) = response.split_once("HTTP/1.1 200 OK\r\n") else {
        return;
      };
      let _ = stream.write_all(interim.as_bytes());

      let content_length = String::from_utf8_lossy(&request_head)
        .lines()
        .find_map(|line| {
          let (name, value) = line.split_once(':')?;
          if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
          } else {
            None
          }
        })
        .unwrap_or(0);
      let mut body = vec![0u8; content_length];
      if stream.read_exact(&mut body).is_err() {
        return;
      }

      let final_response = format!("HTTP/1.1 200 OK\r\n{final_response}");
      let _ = stream.write_all(final_response.as_bytes());
    }
  });

  (addr, handle)
}

pub fn spawn_switching_protocols_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("switching protocols server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = concat!(
        "HTTP/1.1 101 Switching Protocols\r\n",
        "Connection: Upgrade\r\n",
        "Upgrade: websocket\r\n",
        "Sec-WebSocket-Accept: test-accept\r\n",
        "\r\n",
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 18\r\n",
        "\r\n",
        "must not be read\r\n"
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });

  (addr, handle)
}

pub fn spawn_http_proxy_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("http proxy server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let request_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
      let body = request_line.as_bytes();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      );
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(body);
    }
  });
  (addr, handle)
}

pub fn spawn_http_proxy_auth_echo_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("http proxy auth server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let request = read_http_request(&mut stream);
      let auth = header_value(&request, "Proxy-Authorization").unwrap_or_default();
      let body = auth.as_bytes();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      );
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(body);
    }
  });
  (addr, handle)
}

pub fn spawn_invalid_gzip_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("invalid gzip server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let body = b"not-gzip";
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      );
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(body);
    }
  });
  (addr, handle)
}

pub fn spawn_auth_echo_server() -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("auth echo server");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let mut request = Vec::new();
      let mut buf = [0u8; 1024];
      loop {
        let Ok(read) = stream.read(&mut buf) else {
          break;
        };
        if read == 0 {
          break;
        }
        request.extend_from_slice(&buf[..read]);
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
          break;
        }
      }

      let req_str = String::from_utf8_lossy(&request);
      let auth_value = req_str
        .lines()
        .find_map(|line| {
          let (name, value) = line.split_once(':')?;
          if name.eq_ignore_ascii_case("authorization") {
            Some(value.trim().to_string())
          } else {
            None
          }
        })
        .unwrap_or_default();

      let body = auth_value.as_bytes();
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      );
      let _ = stream.write_all(response.as_bytes());
      let _ = stream.write_all(body);
    }
  });
  (addr, handle)
}

pub fn spawn_socks5_proxy_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_socks5_proxy_server_with_auth(None)
}

pub fn spawn_socks5_proxy_server_with_credentials(
  username: &'static str,
  password: &'static str,
) -> (SocketAddr, JoinHandle<()>) {
  spawn_socks5_proxy_server_with_auth(Some((username, password)))
}

fn spawn_socks5_proxy_server_with_auth(
  auth: Option<(&'static str, &'static str)>,
) -> (SocketAddr, JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind socks5 proxy");
  let addr = listener.local_addr().expect("socks5 proxy addr");
  let handle = thread::spawn(move || {
    if let Ok((stream, _)) = listener.accept() {
      proxy_http_request(stream, auth).expect("proxy socks5 request");
    }
  });
  (addr, handle)
}

#[cfg(feature = "tls-rustls")]
pub fn spawn_https_proxy_server_with_credentials(
  username: &'static str,
  password: &'static str,
) -> (SocketAddr, SocketAddr, JoinHandle<()>) {
  use base64::Engine;
  use std::io::copy;

  let (target_addr, _target_handle) = spawn_tls_server();
  let (listener, proxy_addr) = bind_local_http_listener("https proxy server");
  let handle = thread::spawn(move || {
    if let Ok((mut client, _)) = listener.accept() {
      let request = read_http_request(&mut client);
      let request_str = String::from_utf8_lossy(&request);
      let request_line = request_str.lines().next().unwrap_or_default().to_string();
      let proxy_auth = header_value(&request, "Proxy-Authorization").unwrap_or_default();
      let expected_auth = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, password))
      );

      if proxy_auth != expected_auth {
        let _ = client
          .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n");
        return;
      }

      let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
      let mut server = TcpStream::connect(&target).expect("connect tls target");

      let _ = client.write_all(b"HTTP/1.1 200 Conne");
      let _ = client.flush();
      thread::sleep(Duration::from_millis(20));
      let _ = client.write_all(b"ction Established\r\nProxy-Agent: test\r\n\r\n");
      let _ = client.flush();

      let mut client_reader = client.try_clone().expect("clone client");
      let mut server_writer = server.try_clone().expect("clone target");
      let relay = thread::spawn(move || {
        let _ = copy(&mut client_reader, &mut server_writer);
      });

      let _ = copy(&mut server, &mut client);
      let _ = relay.join();
    }
  });

  (proxy_addr, target_addr, handle)
}

#[cfg(feature = "tls-rustls")]
pub fn spawn_tls_server() -> (SocketAddr, JoinHandle<()>) {
  use rcgen::generate_simple_self_signed;
  use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
  use rustls::{ServerConfig, ServerConnection, StreamOwned};
  use std::sync::Arc;

  let cert = generate_simple_self_signed(vec!["localhost".to_string()]).expect("generate cert");
  let cert_der = cert.serialize_der().expect("cert der");
  let key_der = cert.serialize_private_key_der();

  let config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(
      vec![CertificateDer::from(cert_der)],
      PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_der)),
    )
    .expect("set cert");
  let config = Arc::new(config);

  let (listener, addr) = bind_local_http_listener("tls server");
  let handle = thread::spawn(move || {
    if let Ok((stream, _)) = listener.accept() {
      let session = ServerConnection::new(config.clone()).expect("server connection");
      let mut tls = StreamOwned::new(session, stream);
      let _ = read_http_request(&mut tls);
      let _ = tls.write_all(HTTP_OK_RESPONSE);
      let _ = tls.flush();
      tls.conn.send_close_notify();
      let _ = tls.flush();
    }
  });
  (addr, handle)
}
