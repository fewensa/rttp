#![cfg(feature = "http2")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread;

use rttp_client::HttpClient;

const FRAME_DATA: u8 = 0x0;
const FRAME_HEADERS: u8 = 0x1;
const FRAME_SETTINGS: u8 = 0x4;

const FLAG_END_STREAM: u8 = 0x1;
const FLAG_END_HEADERS: u8 = 0x4;
const FLAG_ACK: u8 = 0x1;

fn spawn_h2_prior_knowledge_peer() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    let mut preface = [0; 24];
    stream
      .read_exact(&mut preface)
      .expect("read client preface");
    assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", &preface);

    let client_settings = read_frame(&mut stream);
    assert_eq!(FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.stream_id);
    assert_eq!(0, client_settings.payload.len());

    write_frame(&mut stream, FRAME_SETTINGS, 0, 0, &[]);

    let client_settings_ack = read_frame(&mut stream);
    assert_eq!(FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);
    assert_eq!(0, client_settings_ack.payload.len());

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_STREAM | FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      &[
        0x88, 0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
    write_frame(
      &mut stream,
      FRAME_DATA,
      FLAG_END_STREAM,
      1,
      b"hello over h2",
    );

    request_headers.payload
  });

  (addr, handle)
}

#[test]
fn prior_knowledge_get_sends_h2_handshake_and_reads_single_response_stream() {
  let (addr, handle) = spawn_h2_prior_knowledge_peer();

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/hello?via=h2", addr))
    .emit_http2_prior_knowledge()
    .expect("single h2 GET response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
  assert_eq!("hello over h2", response.body().string().unwrap());

  let request_header_block = handle.join().expect("h2 peer thread");
  assert!(request_header_block.contains(&0x82));
  assert!(request_header_block.contains(&0x86));
  assert!(request_header_block
    .windows(b"/hello?via=h2".len())
    .any(|window| window == b"/hello?via=h2"));
}

struct Frame {
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: Vec<u8>,
}

fn read_frame(stream: &mut impl Read) -> Frame {
  let mut header = [0; 9];
  stream.read_exact(&mut header).expect("read frame header");
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload).expect("read frame payload");
  Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  }
}

fn write_frame(stream: &mut impl Write, frame_type: u8, flags: u8, stream_id: u32, payload: &[u8]) {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header).expect("write frame header");
  stream.write_all(payload).expect("write frame payload");
  stream.flush().expect("flush frame");
}
