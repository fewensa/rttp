use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const HTTP_OK_RESPONSE: &[u8] =
  b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";

pub fn bind_local_http_listener(name: &str) -> (TcpListener, SocketAddr) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| panic!("bind {}", name));
  let addr = listener
    .local_addr()
    .unwrap_or_else(|_| panic!("{} addr", name));
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

pub fn spawn_ok_http_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_ok_http_server_count(1)
}

pub fn spawn_ok_http_server_count(count: usize) -> (SocketAddr, JoinHandle<()>) {
  let (listener, addr) = bind_local_http_listener("http server");
  let handle = thread::spawn(move || {
    for _ in 0..count {
      if let Ok((mut stream, _)) = listener.accept() {
        let _ = read_http_request(&mut stream);
        let _ = stream.write_all(HTTP_OK_RESPONSE);
      }
    }
  });
  (addr, handle)
}

pub fn capture_raw_http_request() -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_local_http_listener("raw request capture server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    let request = read_http_request(&mut stream);
    let _ = stream.write_all(HTTP_OK_RESPONSE);
    request
  });
  (addr, handle)
}

pub fn capture_optional_raw_http_request(timeout: Duration) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_local_http_listener("optional raw request capture server");
  listener
    .set_nonblocking(true)
    .expect("set optional raw request capture server nonblocking");
  let handle = thread::spawn(move || {
    let deadline = Instant::now() + timeout;
    loop {
      match listener.accept() {
        Ok((mut stream, _)) => {
          let request = read_http_request(&mut stream);
          let _ = stream.write_all(HTTP_OK_RESPONSE);
          return request;
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
          if Instant::now() >= deadline {
            return Vec::new();
          }
          thread::sleep(Duration::from_millis(10));
        }
        Err(_) => return Vec::new(),
      }
    }
  });
  (addr, handle)
}
