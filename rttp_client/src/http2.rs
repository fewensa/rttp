use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use url::Url;

use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Header, RoUrl, ToUrl};
use crate::{error, Config};

const CLIENT_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

const FRAME_DATA: u8 = 0x0;
const FRAME_HEADERS: u8 = 0x1;
const FRAME_RST_STREAM: u8 = 0x3;
const FRAME_SETTINGS: u8 = 0x4;
const FRAME_PING: u8 = 0x6;
const FRAME_GOAWAY: u8 = 0x7;
const FRAME_WINDOW_UPDATE: u8 = 0x8;
const FRAME_CONTINUATION: u8 = 0x9;

const FLAG_END_STREAM: u8 = 0x1;
const FLAG_ACK: u8 = 0x1;
const FLAG_END_HEADERS: u8 = 0x4;
const FLAG_PADDED: u8 = 0x8;
const FLAG_PRIORITY: u8 = 0x20;

const STREAM_ID: u32 = 1;
const SETTING_HEADER_TABLE_SIZE: u16 = 0x1;
const SETTING_ENABLE_PUSH: u16 = 0x2;
const SETTING_INITIAL_WINDOW_SIZE: u16 = 0x4;
const SETTING_MAX_FRAME_SIZE: u16 = 0x5;
const DEFAULT_INITIAL_WINDOW_SIZE: i64 = 65_535;
const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;
const MAX_FRAME_SIZE_LIMIT: usize = 16_777_215;
const MAX_INITIAL_WINDOW_SIZE: u32 = 2_147_483_647;
const WINDOW_UPDATE_THRESHOLD: usize = 32 * 1024;
const DEFAULT_HPACK_DYNAMIC_TABLE_SIZE: usize = 4096;
const HPACK_STATIC_TABLE_LENGTH: usize = 61;
const HPACK_HUFFMAN_EOS_SYMBOL: usize = 256;
const HPACK_HUFFMAN_EOS_BITS: u8 = 30;

const HPACK_HUFFMAN_CODES: [u32; 257] = [
  0x1ff8, 0x7fffd8, 0xfffffe2, 0xfffffe3, 0xfffffe4, 0xfffffe5, 0xfffffe6, 0xfffffe7, 0xfffffe8,
  0xffffea, 0x3ffffffc, 0xfffffe9, 0xfffffea, 0x3ffffffd, 0xfffffeb, 0xfffffec, 0xfffffed,
  0xfffffee, 0xfffffef, 0xffffff0, 0xffffff1, 0xffffff2, 0x3ffffffe, 0xffffff3, 0xffffff4,
  0xffffff5, 0xffffff6, 0xffffff7, 0xffffff8, 0xffffff9, 0xffffffa, 0xffffffb, 0x14, 0x3f8, 0x3f9,
  0xffa, 0x1ff9, 0x15, 0xf8, 0x7fa, 0x3fa, 0x3fb, 0xf9, 0x7fb, 0xfa, 0x16, 0x17, 0x18, 0x0, 0x1,
  0x2, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x5c, 0xfb, 0x7ffc, 0x20, 0xffb, 0x3fc, 0x1ffa,
  0x21, 0x5d, 0x5e, 0x5f, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b,
  0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0xfc, 0x73, 0xfd, 0x1ffb, 0x7fff0, 0x1ffc, 0x3ffc,
  0x22, 0x7ffd, 0x3, 0x23, 0x4, 0x24, 0x5, 0x25, 0x26, 0x27, 0x6, 0x74, 0x75, 0x28, 0x29, 0x2a,
  0x7, 0x2b, 0x76, 0x2c, 0x8, 0x9, 0x2d, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7ffe, 0x7fc, 0x3ffd,
  0x1ffd, 0xffffffc, 0xfffe6, 0x3fffd2, 0xfffe7, 0xfffe8, 0x3fffd3, 0x3fffd4, 0x3fffd5, 0x7fffd9,
  0x3fffd6, 0x7fffda, 0x7fffdb, 0x7fffdc, 0x7fffdd, 0x7fffde, 0xffffeb, 0x7fffdf, 0xffffec,
  0xffffed, 0x3fffd7, 0x7fffe0, 0xffffee, 0x7fffe1, 0x7fffe2, 0x7fffe3, 0x7fffe4, 0x1fffdc,
  0x3fffd8, 0x7fffe5, 0x3fffd9, 0x7fffe6, 0x7fffe7, 0xffffef, 0x3fffda, 0x1fffdd, 0xfffe9,
  0x3fffdb, 0x3fffdc, 0x7fffe8, 0x7fffe9, 0x1fffde, 0x7fffea, 0x3fffdd, 0x3fffde, 0xfffff0,
  0x1fffdf, 0x3fffdf, 0x7fffeb, 0x7fffec, 0x1fffe0, 0x1fffe1, 0x3fffe0, 0x1fffe2, 0x7fffed,
  0x3fffe1, 0x7fffee, 0x7fffef, 0xfffea, 0x3fffe2, 0x3fffe3, 0x3fffe4, 0x7ffff0, 0x3fffe5,
  0x3fffe6, 0x7ffff1, 0x3ffffe0, 0x3ffffe1, 0xfffeb, 0x7fff1, 0x3fffe7, 0x7ffff2, 0x3fffe8,
  0x1ffffec, 0x3ffffe2, 0x3ffffe3, 0x3ffffe4, 0x7ffffde, 0x7ffffdf, 0x3ffffe5, 0xfffff1, 0x1ffffed,
  0x7fff2, 0x1fffe3, 0x3ffffe6, 0x7ffffe0, 0x7ffffe1, 0x3ffffe7, 0x7ffffe2, 0xfffff2, 0x1fffe4,
  0x1fffe5, 0x3ffffe8, 0x3ffffe9, 0xffffffd, 0x7ffffe3, 0x7ffffe4, 0x7ffffe5, 0xfffec, 0xfffff3,
  0xfffed, 0x1fffe6, 0x3fffe9, 0x1fffe7, 0x1fffe8, 0x7ffff3, 0x3fffea, 0x3fffeb, 0x1ffffee,
  0x1ffffef, 0xfffff4, 0xfffff5, 0x3ffffea, 0x7ffff4, 0x3ffffeb, 0x7ffffe6, 0x3ffffec, 0x3ffffed,
  0x7ffffe7, 0x7ffffe8, 0x7ffffe9, 0x7ffffea, 0x7ffffeb, 0xffffffe, 0x7ffffec, 0x7ffffed,
  0x7ffffee, 0x7ffffef, 0x7fffff0, 0x3ffffee, 0x3fffffff,
];

const HPACK_HUFFMAN_CODE_LENGTHS: [u8; 257] = [
  13, 23, 28, 28, 28, 28, 28, 28, 28, 24, 30, 28, 28, 30, 28, 28, 28, 28, 28, 28, 28, 28, 30, 28,
  28, 28, 28, 28, 28, 28, 28, 28, 6, 10, 10, 12, 13, 6, 8, 11, 10, 10, 8, 11, 8, 6, 6, 6, 5, 5, 5,
  6, 6, 6, 6, 6, 6, 6, 7, 8, 15, 6, 12, 10, 13, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
  7, 7, 7, 7, 7, 7, 8, 7, 8, 13, 19, 13, 14, 6, 15, 5, 6, 5, 6, 5, 6, 6, 6, 5, 7, 7, 6, 6, 6, 5, 6,
  7, 6, 5, 5, 6, 7, 7, 7, 7, 7, 15, 11, 14, 13, 28, 20, 22, 20, 20, 22, 22, 22, 23, 22, 23, 23, 23,
  23, 23, 24, 23, 24, 24, 22, 23, 24, 23, 23, 23, 23, 21, 22, 23, 22, 23, 23, 24, 22, 21, 20, 22,
  22, 23, 23, 21, 23, 22, 22, 24, 21, 22, 23, 23, 21, 21, 22, 21, 23, 22, 23, 23, 20, 22, 22, 22,
  23, 22, 22, 23, 26, 26, 20, 19, 22, 23, 22, 25, 26, 26, 26, 27, 27, 26, 24, 25, 19, 21, 26, 27,
  27, 26, 27, 24, 21, 21, 26, 26, 28, 27, 27, 27, 20, 24, 20, 21, 22, 21, 21, 23, 22, 22, 25, 25,
  24, 24, 26, 23, 26, 27, 26, 26, 27, 27, 27, 27, 27, 28, 27, 27, 27, 27, 27, 26, 30,
];

pub struct PriorKnowledgeClient<'a> {
  request: RawRequest<'a>,
}

impl<'a> PriorKnowledgeClient<'a> {
  pub(crate) fn new(request: RawRequest<'a>) -> Self {
    Self { request }
  }

  pub fn get(mut self) -> error::Result<Response> {
    let method = self.request.origin().method();
    let is_head = method.eq_ignore_ascii_case("HEAD");
    let is_delete = method.eq_ignore_ascii_case("DELETE");
    let is_options = method.eq_ignore_ascii_case("OPTIONS");
    if (method.eq_ignore_ascii_case("GET") || is_head) && self.request.body().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge GET or HEAD cannot send a request body",
      ));
    }
    if is_delete && self.request.body().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge DELETE cannot send a request body",
      ));
    }
    if is_options && self.request.body().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge OPTIONS cannot send a request body",
      ));
    }
    if !is_supported_request_method(method) {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge client supports GET, HEAD, bodyless DELETE or OPTIONS, and buffered POST, PUT, or PATCH",
      ));
    }

    let url = self.request.url().to_url().map_err(error::builder)?;
    if url.scheme() != "http" {
      return Err(error::url_bad_scheme(url));
    }

    let mut stream = connect_tcp_stream(addr(&url)?, self.request.origin().config())?;
    write_connection_preface(&mut stream)?;
    let peer_settings = read_settings_and_ack(&mut stream)?;
    let response = match write_request(
      &mut stream,
      &self.request,
      &url,
      self.request.url().clone(),
      peer_settings,
    )? {
      Some(response) => response,
      None => read_single_stream_response(&mut stream, self.request.url().clone(), !is_head)?,
    };
    self.request.origin_mut().closed_set(true);
    Ok(response)
  }
}

fn is_supported_request_method(method: &str) -> bool {
  method.eq_ignore_ascii_case("GET")
    || method.eq_ignore_ascii_case("HEAD")
    || method.eq_ignore_ascii_case("DELETE")
    || method.eq_ignore_ascii_case("OPTIONS")
    || method.eq_ignore_ascii_case("POST")
    || method.eq_ignore_ascii_case("PUT")
    || method.eq_ignore_ascii_case("PATCH")
}

fn addr(url: &Url) -> error::Result<String> {
  let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
  let port = url
    .port_or_known_default()
    .ok_or(error::url_bad_host(url.clone()))?;
  Ok(format!("{}:{}", host, port))
}

fn connect_tcp_stream<A>(addr: A, config: &Config) -> error::Result<TcpStream>
where
  A: ToSocketAddrs,
{
  let timeout_read = timeout_duration("read", config.read_timeout())?;
  let timeout_write = timeout_duration("write", config.write_timeout())?;
  let mut last_err = None;

  for addr in addr.to_socket_addrs().map_err(error::request)? {
    let socket = match Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP)) {
      Ok(socket) => socket,
      Err(err) => {
        last_err = Some(err);
        continue;
      }
    };
    if let Err(err) = socket.set_read_timeout(Some(timeout_read)) {
      last_err = Some(err);
      continue;
    }
    if let Err(err) = socket.set_write_timeout(Some(timeout_write)) {
      last_err = Some(err);
      continue;
    }
    if let Err(err) = socket.connect(&addr.into()) {
      last_err = Some(err);
      continue;
    }
    return Ok(socket.into());
  }

  Err(error::request(last_err.unwrap_or_else(|| {
    io::Error::new(io::ErrorKind::NotFound, "no socket address resolved")
  })))
}

fn timeout_duration(name: &'static str, millis: u64) -> error::Result<Duration> {
  if millis == 0 {
    return Err(error::request(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("{} timeout must be greater than 0", name),
    )));
  }
  Ok(Duration::from_millis(millis))
}

fn write_connection_preface(stream: &mut TcpStream) -> error::Result<()> {
  stream.write_all(CLIENT_PREFACE).map_err(error::request)?;
  write_frame(stream, FRAME_SETTINGS, 0, 0, &[])?;
  stream.flush().map_err(error::request)
}

fn read_settings_and_ack(stream: &mut TcpStream) -> error::Result<PeerSettings> {
  loop {
    let frame = read_frame(stream)?;
    if frame.frame_type != FRAME_SETTINGS {
      return Err(error::bad_response(
        "HTTP/2 peer did not start with a SETTINGS frame",
      ));
    }
    if frame.flags & FLAG_ACK == FLAG_ACK {
      if frame.payload.is_empty() {
        continue;
      }
      return Err(error::bad_response(
        "HTTP/2 SETTINGS ACK frame must not contain payload",
      ));
    }
    if frame.stream_id != 0 {
      return Err(error::bad_response("invalid HTTP/2 SETTINGS frame"));
    }
    let peer_settings = validate_settings_payload(&frame.payload)?;
    write_frame(stream, FRAME_SETTINGS, FLAG_ACK, 0, &[])?;
    stream.flush().map_err(error::request)?;
    return Ok(peer_settings);
  }
}

#[derive(Clone, Copy)]
struct PeerSettings {
  header_table_size: usize,
  initial_window_size: u32,
  initial_window_size_changed: bool,
  max_frame_size: usize,
  max_frame_size_changed: bool,
}

fn validate_settings_payload(payload: &[u8]) -> error::Result<PeerSettings> {
  if payload.len() % 6 != 0 {
    return Err(error::bad_response("invalid HTTP/2 SETTINGS frame"));
  }

  let mut settings = PeerSettings {
    header_table_size: DEFAULT_HPACK_DYNAMIC_TABLE_SIZE,
    initial_window_size: DEFAULT_INITIAL_WINDOW_SIZE as u32,
    initial_window_size_changed: false,
    max_frame_size: DEFAULT_MAX_FRAME_SIZE,
    max_frame_size_changed: false,
  };
  for setting in payload.chunks_exact(6) {
    let identifier = u16::from_be_bytes([setting[0], setting[1]]);
    let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
    match identifier {
      SETTING_HEADER_TABLE_SIZE => settings.header_table_size = value as usize,
      SETTING_ENABLE_PUSH => {
        if value > 1 {
          return Err(error::bad_response(
            "invalid HTTP/2 SETTINGS_ENABLE_PUSH value",
          ));
        }
      }
      SETTING_INITIAL_WINDOW_SIZE => {
        if value > MAX_INITIAL_WINDOW_SIZE {
          return Err(error::bad_response(
            "invalid HTTP/2 SETTINGS_INITIAL_WINDOW_SIZE value",
          ));
        }
        settings.initial_window_size = value;
        settings.initial_window_size_changed = true;
      }
      SETTING_MAX_FRAME_SIZE => {
        let value = value as usize;
        if !(DEFAULT_MAX_FRAME_SIZE..=MAX_FRAME_SIZE_LIMIT).contains(&value) {
          return Err(error::bad_response(
            "invalid HTTP/2 SETTINGS_MAX_FRAME_SIZE value",
          ));
        }
        settings.max_frame_size = value;
        settings.max_frame_size_changed = true;
      }
      _ => {}
    }
  }
  Ok(settings)
}

fn validate_settings_frame(frame: &Frame) -> error::Result<()> {
  if frame.stream_id != 0 {
    return Err(error::bad_response("invalid HTTP/2 SETTINGS frame"));
  }
  if frame.flags & FLAG_ACK == FLAG_ACK {
    if frame.payload.is_empty() {
      return Ok(());
    }
    return Err(error::bad_response(
      "HTTP/2 SETTINGS ACK frame must not contain payload",
    ));
  }
  validate_settings_payload(&frame.payload)?;
  Ok(())
}

fn write_request(
  stream: &mut TcpStream,
  request: &RawRequest<'_>,
  url: &Url,
  response_url: RoUrl,
  peer_settings: PeerSettings,
) -> error::Result<Option<Response>> {
  let mut hpack = RequestHpackEncoder::new(
    peer_settings
      .header_table_size
      .min(DEFAULT_HPACK_DYNAMIC_TABLE_SIZE),
  );
  let regular_header_fields = regular_headers(request.header());
  let trailer_announcement = (!request.origin().trailers().is_empty())
    .then(|| ("trailer".to_string(), request_trailer_field_value(request)));
  let trailer_fields = request_trailer_fields(request);
  let mut dynamic_field_plan = Vec::new();
  dynamic_field_plan.extend(regular_header_fields.iter().cloned());
  dynamic_field_plan.extend(trailer_announcement.iter().cloned());
  dynamic_field_plan.extend(trailer_fields.iter().cloned());
  let mut dynamic_field_position = 0;

  let header_block = encode_request_headers(
    request,
    url,
    &regular_header_fields,
    trailer_announcement.as_ref(),
    &dynamic_field_plan,
    &mut dynamic_field_position,
    &mut hpack,
  )?;
  let body = request
    .body()
    .as_ref()
    .map(|body| body.bytes())
    .unwrap_or(&[]);
  let has_trailers = !request.origin().trailers().is_empty();
  let header_flags = if body.is_empty() && !has_trailers {
    FLAG_END_STREAM
  } else {
    0
  };
  write_header_block_frames(
    stream,
    header_flags,
    STREAM_ID,
    &header_block,
    peer_settings.max_frame_size,
  )?;
  if !body.is_empty() {
    if let Some(response) =
      write_data_frames(stream, body, peer_settings, has_trailers, response_url)?
    {
      return Ok(Some(response));
    }
  }
  if has_trailers {
    let trailer_block = encode_request_trailers(
      &trailer_fields,
      &dynamic_field_plan,
      &mut dynamic_field_position,
      &mut hpack,
    )?;
    write_header_block_frames(
      stream,
      FLAG_END_STREAM,
      STREAM_ID,
      &trailer_block,
      peer_settings.max_frame_size,
    )?;
  }
  stream.flush().map_err(error::request).map(|()| None)
}

fn write_header_block_frames(
  stream: &mut TcpStream,
  initial_flags: u8,
  stream_id: u32,
  header_block: &[u8],
  peer_max_frame_size: usize,
) -> error::Result<()> {
  let mut chunks = header_block.chunks(peer_max_frame_size).peekable();
  let first_chunk = chunks.next().unwrap_or(&[]);
  if chunks.peek().is_none() {
    return write_frame(
      stream,
      FRAME_HEADERS,
      initial_flags | FLAG_END_HEADERS,
      stream_id,
      first_chunk,
    );
  }

  write_frame(stream, FRAME_HEADERS, initial_flags, stream_id, first_chunk)?;
  while let Some(chunk) = chunks.next() {
    let flags = if chunks.peek().is_none() {
      FLAG_END_HEADERS
    } else {
      0
    };
    write_frame(stream, FRAME_CONTINUATION, flags, stream_id, chunk)?;
  }
  Ok(())
}

fn write_data_frames(
  stream: &mut TcpStream,
  body: &[u8],
  peer_settings: PeerSettings,
  has_trailers: bool,
  url: RoUrl,
) -> error::Result<Option<Response>> {
  let mut connection_send_window = SendWindow::new();
  let mut stream_send_window =
    SendWindow::with_available(i64::from(peer_settings.initial_window_size));
  let mut current_initial_window_size = peer_settings.initial_window_size;
  let mut current_max_frame_size = peer_settings.max_frame_size;
  let mut sent = 0;

  while sent < body.len() {
    let available = connection_send_window
      .available()
      .min(stream_send_window.available());
    if available <= 0 {
      if let Some(response) = read_until_send_window_available(
        stream,
        &mut connection_send_window,
        &mut stream_send_window,
        &mut current_initial_window_size,
        &mut current_max_frame_size,
        url.clone(),
      )? {
        return Ok(Some(response));
      }
      continue;
    }

    let window_chunk_size = usize::try_from(available)
      .map_err(|_| error::request(io::Error::other("HTTP/2 send window is too large")))?;
    let chunk_size = current_max_frame_size
      .min(window_chunk_size)
      .min(body.len() - sent);
    let chunk = &body[sent..sent + chunk_size];
    sent += chunk_size;
    connection_send_window.consume(chunk_size)?;
    stream_send_window.consume(chunk_size)?;

    let flags = if sent == body.len() && !has_trailers {
      FLAG_END_STREAM
    } else {
      0
    };
    write_frame(stream, FRAME_DATA, flags, STREAM_ID, chunk)?;
  }
  Ok(None)
}

fn read_until_send_window_available(
  stream: &mut TcpStream,
  connection_send_window: &mut SendWindow,
  stream_send_window: &mut SendWindow,
  current_initial_window_size: &mut u32,
  current_max_frame_size: &mut usize,
  url: RoUrl,
) -> error::Result<Option<Response>> {
  loop {
    let frame = read_frame(stream)?;
    match (frame.frame_type, frame.stream_id) {
      (FRAME_WINDOW_UPDATE, 0) => {
        connection_send_window.increase(window_update_increment(&frame)?)?;
      }
      (FRAME_WINDOW_UPDATE, STREAM_ID) => {
        stream_send_window.increase(window_update_increment(&frame)?)?;
      }
      (FRAME_WINDOW_UPDATE, _) => {
        window_update_increment(&frame)?;
      }
      (FRAME_SETTINGS, _) => {
        handle_settings_while_sending(
          stream,
          &frame,
          stream_send_window,
          current_initial_window_size,
          current_max_frame_size,
        )?;
      }
      (FRAME_RST_STREAM, STREAM_ID) => {
        return Err(error::bad_response("HTTP/2 stream received RST_STREAM"));
      }
      (FRAME_PING, 0) => {
        if frame.payload.len() != 8 {
          return Err(error::bad_response("invalid HTTP/2 PING frame"));
        }
        if frame.flags & FLAG_ACK == 0 {
          write_frame(stream, FRAME_PING, FLAG_ACK, 0, &frame.payload)?;
          stream.flush().map_err(error::request)?;
        }
      }
      (FRAME_PING, _) => {
        return Err(error::bad_response("invalid HTTP/2 PING frame"));
      }
      (FRAME_GOAWAY, 0) => {
        if goaway_last_stream_id(&frame.payload)? < STREAM_ID {
          return Err(error::bad_response("HTTP/2 connection received GOAWAY"));
        }
      }
      (FRAME_HEADERS, STREAM_ID) | (FRAME_DATA, STREAM_ID) | (FRAME_CONTINUATION, STREAM_ID) => {
        return read_single_stream_response_from_frame(stream, url, frame).map(Some);
      }
      _ => {}
    }

    if connection_send_window
      .available()
      .min(stream_send_window.available())
      > 0
    {
      return Ok(None);
    }
  }
}

fn handle_settings_while_sending(
  stream: &mut TcpStream,
  frame: &Frame,
  stream_send_window: &mut SendWindow,
  current_initial_window_size: &mut u32,
  current_max_frame_size: &mut usize,
) -> error::Result<()> {
  validate_settings_frame(frame)?;
  if frame.flags & FLAG_ACK == FLAG_ACK {
    return Ok(());
  }
  let settings = validate_settings_payload(&frame.payload)?;
  if settings.initial_window_size_changed {
    let delta = i64::from(settings.initial_window_size) - i64::from(*current_initial_window_size);
    if delta != 0 {
      stream_send_window.adjust(delta)?;
      *current_initial_window_size = settings.initial_window_size;
    }
  }
  if settings.max_frame_size_changed {
    *current_max_frame_size = settings.max_frame_size;
  }
  write_frame(stream, FRAME_SETTINGS, FLAG_ACK, 0, &[])?;
  stream.flush().map_err(error::request)
}

fn encode_request_headers(
  request: &RawRequest<'_>,
  url: &Url,
  regular_header_fields: &[(String, String)],
  trailer_announcement: Option<&(String, String)>,
  dynamic_field_plan: &[(String, String)],
  dynamic_field_position: &mut usize,
  hpack: &mut RequestHpackEncoder,
) -> error::Result<Vec<u8>> {
  let mut block = Vec::new();
  encode_method(&mut block, request.origin().method())?;
  if url.scheme() == "http" {
    block.push(0x86);
  } else {
    return Err(error::url_bad_scheme(url.clone()));
  }

  let path = request_target(url);
  if path == "/" {
    block.push(0x84);
  } else {
    encode_literal_indexed_name_without_indexing(&mut block, 4, path.as_bytes())?;
  }
  encode_literal_indexed_name_without_indexing(&mut block, 1, authority(url)?.as_bytes())?;

  for (name, value) in regular_header_fields {
    let remaining = dynamic_field_plan
      .get(*dynamic_field_position + 1..)
      .unwrap_or(&[]);
    hpack.encode_field(&mut block, name, value, remaining)?;
    *dynamic_field_position += 1;
  }

  if let Some((name, value)) = trailer_announcement {
    let remaining = dynamic_field_plan
      .get(*dynamic_field_position + 1..)
      .unwrap_or(&[]);
    hpack.encode_field(&mut block, name, value, remaining)?;
    *dynamic_field_position += 1;
  }

  Ok(block)
}

fn request_trailer_field_value(request: &RawRequest<'_>) -> String {
  request
    .origin()
    .trailers()
    .iter()
    .map(|header| header.name().as_str())
    .collect::<Vec<_>>()
    .join(", ")
}

fn request_trailer_fields(request: &RawRequest<'_>) -> Vec<(String, String)> {
  request
    .origin()
    .trailers()
    .iter()
    .map(|header| {
      (
        header.name().to_ascii_lowercase(),
        header.value().to_string(),
      )
    })
    .collect()
}

fn encode_request_trailers(
  trailer_fields: &[(String, String)],
  dynamic_field_plan: &[(String, String)],
  dynamic_field_position: &mut usize,
  hpack: &mut RequestHpackEncoder,
) -> error::Result<Vec<u8>> {
  let mut block = Vec::new();
  for (name, value) in trailer_fields {
    let remaining = dynamic_field_plan
      .get(*dynamic_field_position + 1..)
      .unwrap_or(&[]);
    hpack.encode_field(&mut block, name, value, remaining)?;
    *dynamic_field_position += 1;
  }
  Ok(block)
}

fn encode_method(block: &mut Vec<u8>, method: &str) -> error::Result<()> {
  if method.eq_ignore_ascii_case("GET") {
    block.push(0x82);
  } else if method.eq_ignore_ascii_case("POST") {
    block.push(0x83);
  } else {
    encode_literal_indexed_name_without_indexing(block, 2, method.to_uppercase().as_bytes())?;
  }
  Ok(())
}

fn request_target(url: &Url) -> String {
  let mut target = url.path().to_string();
  if target.is_empty() {
    target.push('/');
  }
  if let Some(query) = url.query() {
    target.push('?');
    target.push_str(query);
  }
  target
}

fn authority(url: &Url) -> error::Result<String> {
  let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
  let host = if host.contains(':') && !host.starts_with('[') {
    format!("[{}]", host)
  } else {
    host.to_string()
  };
  Ok(match url.port() {
    Some(port) => format!("{}:{}", host, port),
    None => host,
  })
}

fn regular_headers(header: &str) -> Vec<(String, String)> {
  header
    .lines()
    .skip(1)
    .filter_map(|line| line.split_once(':'))
    .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
    .filter(|(name, _)| {
      !matches!(
        name.as_str(),
        "connection"
          | "host"
          | "keep-alive"
          | "proxy-connection"
          | "te"
          | "trailer"
          | "transfer-encoding"
          | "upgrade"
      )
    })
    .collect()
}

#[derive(Clone, Copy)]
enum HeaderBlockKind {
  ResponseHeaders,
  Trailers,
}

#[derive(Clone, Copy)]
struct PendingHeaderBlock {
  kind: HeaderBlockKind,
  end_stream: bool,
}

fn read_single_stream_response(
  stream: &mut TcpStream,
  url: RoUrl,
  include_data_payload: bool,
) -> error::Result<Response> {
  read_single_stream_response_with_first_frame(stream, url, None, include_data_payload)
}

fn read_single_stream_response_from_frame(
  stream: &mut TcpStream,
  url: RoUrl,
  first_frame: Frame,
) -> error::Result<Response> {
  read_single_stream_response_with_first_frame(stream, url, Some(first_frame), true)
}

fn read_single_stream_response_with_first_frame(
  stream: &mut TcpStream,
  url: RoUrl,
  mut first_frame: Option<Frame>,
  include_data_payload: bool,
) -> error::Result<Response> {
  let mut header_block = Vec::new();
  let mut headers = Vec::new();
  let mut trailers = Vec::new();
  let mut body = Vec::new();
  let mut status = None;
  let mut pending_header_block = None;
  let mut final_response_started = false;
  let mut response_body_started = false;
  let mut hpack = HpackDecoder::new(DEFAULT_HPACK_DYNAMIC_TABLE_SIZE);
  let mut connection_receive_window = ReceiveWindow::new();
  let mut stream_receive_window = ReceiveWindow::new();
  let mut connection_send_window = SendWindow::new();
  let mut stream_send_window = SendWindow::new();

  loop {
    let frame = match first_frame.take() {
      Some(frame) => frame,
      None => match read_frame(stream) {
        Ok(frame) => frame,
        Err(err) if pending_header_block.is_some() && is_unexpected_eof(&err) => {
          return Err(error::bad_response("incomplete HTTP/2 header block"));
        }
        Err(err) => return Err(err),
      },
    };
    if pending_header_block.is_some()
      && (frame.frame_type != FRAME_CONTINUATION || frame.stream_id != STREAM_ID)
    {
      return Err(error::bad_response(
        "expected HTTP/2 CONTINUATION frame for incomplete header block",
      ));
    }
    match (frame.frame_type, frame.stream_id) {
      (FRAME_SETTINGS, _) => {
        validate_settings_frame(&frame)?;
        if frame.flags & FLAG_ACK == 0 {
          write_frame(stream, FRAME_SETTINGS, FLAG_ACK, 0, &[])?;
          stream.flush().map_err(error::request)?;
        }
      }
      (FRAME_HEADERS, STREAM_ID) => {
        let kind = if final_response_started || response_body_started {
          HeaderBlockKind::Trailers
        } else {
          HeaderBlockKind::ResponseHeaders
        };
        let end_stream = frame.flags & FLAG_END_STREAM == FLAG_END_STREAM;
        header_block.extend_from_slice(header_block_fragment(&frame)?);
        if frame.flags & FLAG_END_HEADERS == FLAG_END_HEADERS {
          if apply_header_block(
            kind,
            &header_block,
            &mut status,
            &mut headers,
            &mut trailers,
            &mut hpack,
          )? {
            final_response_started = true;
          }
          header_block.clear();
          pending_header_block = None;
        } else {
          pending_header_block = Some(PendingHeaderBlock { kind, end_stream });
        }
        if frame.flags & FLAG_END_HEADERS == FLAG_END_HEADERS && end_stream {
          break;
        }
      }
      (FRAME_CONTINUATION, STREAM_ID) => {
        let pending = pending_header_block.ok_or_else(|| {
          error::bad_response("unexpected HTTP/2 CONTINUATION frame without header block")
        })?;
        header_block.extend_from_slice(&frame.payload);
        if frame.flags & FLAG_END_HEADERS == FLAG_END_HEADERS {
          if apply_header_block(
            pending.kind,
            &header_block,
            &mut status,
            &mut headers,
            &mut trailers,
            &mut hpack,
          )? {
            final_response_started = true;
          }
          header_block.clear();
          pending_header_block = None;
          if pending.end_stream {
            break;
          }
        }
      }
      (FRAME_DATA, STREAM_ID) => {
        let end_stream = frame.flags & FLAG_END_STREAM == FLAG_END_STREAM;
        let data = data_payload(&frame)?;
        response_body_started = true;
        if include_data_payload {
          body.extend_from_slice(data);
        }
        let stream_update = stream_receive_window.consume(frame.payload.len())?;
        let connection_update = connection_receive_window.consume(frame.payload.len())?;
        if !end_stream && (stream_update > 0 || connection_update > 0) {
          if stream_update > 0 {
            write_window_update_best_effort(stream, STREAM_ID, stream_update)?;
            stream_receive_window.release(stream_update)?;
          }
          if connection_update > 0 {
            write_window_update_best_effort(stream, 0, connection_update)?;
            connection_receive_window.release(connection_update)?;
          }
          flush_best_effort(stream)?;
        }
        if end_stream {
          break;
        }
      }
      (FRAME_WINDOW_UPDATE, 0) => {
        connection_send_window.increase(window_update_increment(&frame)?)?;
      }
      (FRAME_WINDOW_UPDATE, STREAM_ID) => {
        stream_send_window.increase(window_update_increment(&frame)?)?;
      }
      (FRAME_WINDOW_UPDATE, _) => {
        window_update_increment(&frame)?;
      }
      (FRAME_RST_STREAM, STREAM_ID) => {
        return Err(error::bad_response("HTTP/2 stream received RST_STREAM"));
      }
      (FRAME_PING, 0) => {
        if frame.payload.len() != 8 {
          return Err(error::bad_response("invalid HTTP/2 PING frame"));
        }
        if frame.flags & FLAG_ACK == 0 {
          write_frame(stream, FRAME_PING, FLAG_ACK, 0, &frame.payload)?;
          stream.flush().map_err(error::request)?;
        }
      }
      (FRAME_PING, _) => {
        return Err(error::bad_response("invalid HTTP/2 PING frame"));
      }
      (FRAME_GOAWAY, 0) => {
        if goaway_last_stream_id(&frame.payload)? < STREAM_ID {
          return Err(error::bad_response("HTTP/2 connection received GOAWAY"));
        }
      }
      (_, STREAM_ID) => {}
      (FRAME_CONTINUATION, _) => {
        return Err(error::bad_response(
          "unexpected HTTP/2 CONTINUATION frame without header block",
        ));
      }
      (_, 0) => {}
      _ => {}
    }
  }

  if pending_header_block.is_some() {
    return Err(error::bad_response("incomplete HTTP/2 header block"));
  }

  let status = status.ok_or_else(|| error::bad_response("missing HTTP/2 :status header"))?;
  build_response(url, status, &headers, body, trailers)
}

struct ReceiveWindow {
  available: i64,
  pending_update: usize,
}

impl ReceiveWindow {
  fn new() -> Self {
    Self {
      available: DEFAULT_INITIAL_WINDOW_SIZE,
      pending_update: 0,
    }
  }

  fn consume(&mut self, amount: usize) -> error::Result<usize> {
    let amount = i64::try_from(amount)
      .map_err(|_| error::bad_response("HTTP/2 DATA frame exceeds flow-control window"))?;
    self.available = self
      .available
      .checked_sub(amount)
      .ok_or_else(|| error::bad_response("HTTP/2 DATA frame exceeds flow-control window"))?;
    if self.available < 0 {
      return Err(error::bad_response(
        "HTTP/2 DATA frame exceeds flow-control window",
      ));
    }
    self.pending_update = self
      .pending_update
      .checked_add(amount as usize)
      .ok_or_else(|| error::bad_response("HTTP/2 response body is too large"))?;
    if self.pending_update >= WINDOW_UPDATE_THRESHOLD {
      Ok(std::mem::take(&mut self.pending_update))
    } else {
      Ok(0)
    }
  }

  fn release(&mut self, amount: usize) -> error::Result<()> {
    let amount = i64::try_from(amount)
      .map_err(|_| error::bad_response("HTTP/2 window update increment is too large"))?;
    self.available = self
      .available
      .checked_add(amount)
      .ok_or_else(|| error::bad_response("HTTP/2 flow-control window overflow"))?;
    if self.available > MAX_INITIAL_WINDOW_SIZE as i64 {
      return Err(error::bad_response("HTTP/2 flow-control window overflow"));
    }
    Ok(())
  }
}

struct SendWindow {
  available: i64,
}

impl SendWindow {
  fn new() -> Self {
    Self {
      available: DEFAULT_INITIAL_WINDOW_SIZE,
    }
  }

  fn with_available(available: i64) -> Self {
    Self { available }
  }

  fn available(&self) -> i64 {
    self.available
  }

  fn consume(&mut self, amount: usize) -> error::Result<()> {
    let amount = i64::try_from(amount)
      .map_err(|_| error::request(io::Error::other("HTTP/2 DATA frame is too large")))?;
    self.available = self
      .available
      .checked_sub(amount)
      .ok_or_else(|| error::request(io::Error::other("HTTP/2 send window underflow")))?;
    Ok(())
  }

  fn increase(&mut self, amount: u32) -> error::Result<()> {
    self.available = self
      .available
      .checked_add(i64::from(amount))
      .ok_or_else(|| error::bad_response("HTTP/2 WINDOW_UPDATE overflow"))?;
    if self.available > MAX_INITIAL_WINDOW_SIZE as i64 {
      return Err(error::bad_response("HTTP/2 WINDOW_UPDATE overflow"));
    }
    Ok(())
  }

  fn adjust(&mut self, delta: i64) -> error::Result<()> {
    self.available = self
      .available
      .checked_add(delta)
      .ok_or_else(|| error::bad_response("HTTP/2 flow-control window overflow"))?;
    if self.available > MAX_INITIAL_WINDOW_SIZE as i64 {
      return Err(error::bad_response("HTTP/2 flow-control window overflow"));
    }
    Ok(())
  }
}

fn window_update_increment(frame: &Frame) -> error::Result<u32> {
  if frame.payload.len() != 4 {
    return Err(error::bad_response("invalid HTTP/2 WINDOW_UPDATE frame"));
  }
  let increment = u32::from_be_bytes([
    frame.payload[0] & 0x7f,
    frame.payload[1],
    frame.payload[2],
    frame.payload[3],
  ]);
  if increment == 0 {
    return Err(error::bad_response(
      "invalid HTTP/2 WINDOW_UPDATE increment",
    ));
  }
  Ok(increment)
}

fn data_payload(frame: &Frame) -> error::Result<&[u8]> {
  if frame.flags & FLAG_PADDED == 0 {
    return Ok(&frame.payload);
  }
  let (&pad_length, payload) = frame
    .payload
    .split_first()
    .ok_or_else(|| error::bad_response("invalid HTTP/2 DATA padding"))?;
  strip_padding(payload, pad_length as usize, "DATA")
}

fn header_block_fragment(frame: &Frame) -> error::Result<&[u8]> {
  let mut payload = frame.payload.as_slice();
  let pad_length = if frame.flags & FLAG_PADDED == FLAG_PADDED {
    let (&pad_length, rest) = payload
      .split_first()
      .ok_or_else(|| error::bad_response("invalid HTTP/2 HEADERS padding"))?;
    payload = rest;
    pad_length as usize
  } else {
    0
  };

  if frame.flags & FLAG_PRIORITY == FLAG_PRIORITY {
    payload = payload
      .get(5..)
      .ok_or_else(|| error::bad_response("invalid HTTP/2 HEADERS priority"))?;
  }

  strip_padding(payload, pad_length, "HEADERS")
}

fn strip_padding<'a>(
  payload: &'a [u8],
  pad_length: usize,
  frame_name: &str,
) -> error::Result<&'a [u8]> {
  let data_length = payload
    .len()
    .checked_sub(pad_length)
    .ok_or_else(|| error::bad_response(format!("invalid HTTP/2 {} padding length", frame_name)))?;
  Ok(&payload[..data_length])
}

fn is_unexpected_eof(err: &error::Error) -> bool {
  std::error::Error::source(err)
    .and_then(|source| source.downcast_ref::<io::Error>())
    .is_some_and(|source| source.kind() == io::ErrorKind::UnexpectedEof)
}

fn apply_header_block(
  kind: HeaderBlockKind,
  block: &[u8],
  status: &mut Option<u32>,
  headers: &mut Vec<(String, String)>,
  trailers: &mut Vec<Header>,
  hpack: &mut HpackDecoder,
) -> error::Result<bool> {
  match kind {
    HeaderBlockKind::ResponseHeaders => {
      let decoded = decode_header_block(block, hpack)?;
      if decoded.status.is_some_and(is_informational_status) {
        return Ok(false);
      }
      *status = decoded.status;
      *headers = decoded.headers;
      Ok(status.is_some())
    }
    HeaderBlockKind::Trailers => {
      trailers.extend(decode_trailer_block(block, hpack)?);
      Ok(false)
    }
  }
}

fn is_informational_status(status: u32) -> bool {
  (100..200).contains(&status)
}

fn goaway_last_stream_id(payload: &[u8]) -> error::Result<u32> {
  if payload.len() < 8 {
    return Err(error::bad_response("invalid HTTP/2 GOAWAY frame"));
  }
  Ok(u32::from_be_bytes([
    payload[0] & 0x7f,
    payload[1],
    payload[2],
    payload[3],
  ]))
}

fn build_response(
  url: RoUrl,
  status: u32,
  headers: &[(String, String)],
  body: Vec<u8>,
  trailers: Vec<Header>,
) -> error::Result<Response> {
  let mut binary = format!("HTTP/2 {}\r\n", status).into_bytes();
  for (name, value) in headers {
    binary.extend_from_slice(name.as_bytes());
    binary.extend_from_slice(b": ");
    binary.extend_from_slice(value.as_bytes());
    binary.extend_from_slice(b"\r\n");
  }
  binary.extend_from_slice(b"\r\n");
  binary.extend_from_slice(&body);
  Response::with_trailers(url, binary, trailers)
}

struct Frame {
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: Vec<u8>,
}

fn read_frame(stream: &mut TcpStream) -> error::Result<Frame> {
  let mut header = [0; 9];
  stream.read_exact(&mut header).map_err(error::response)?;
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload).map_err(error::response)?;
  Ok(Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  })
}

fn write_frame(
  stream: &mut TcpStream,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> error::Result<()> {
  write_frame_io(stream, frame_type, flags, stream_id, payload).map_err(error::request)
}

fn write_frame_io(
  stream: &mut TcpStream,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> io::Result<()> {
  if payload.len() > MAX_FRAME_SIZE_LIMIT {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "HTTP/2 frame payload is too large",
    ));
  }

  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header)?;
  stream.write_all(payload)
}

fn write_window_update_best_effort(
  stream: &mut TcpStream,
  stream_id: u32,
  increment: usize,
) -> error::Result<()> {
  match write_window_update_io(stream, stream_id, increment) {
    Ok(()) => Ok(()),
    Err(err) if is_connection_closed(&err) => Ok(()),
    Err(err) => Err(error::request(err)),
  }
}

fn write_window_update_io(
  stream: &mut TcpStream,
  stream_id: u32,
  increment: usize,
) -> io::Result<()> {
  let increment = u32::try_from(increment).map_err(|_| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      "HTTP/2 window update increment is too large",
    )
  })?;
  if increment == 0 || increment > 0x7fff_ffff {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "invalid HTTP/2 window update increment",
    ));
  }
  write_frame_io(
    stream,
    FRAME_WINDOW_UPDATE,
    0,
    stream_id,
    &increment.to_be_bytes(),
  )
}

fn flush_best_effort(stream: &mut TcpStream) -> error::Result<()> {
  match stream.flush() {
    Ok(()) => Ok(()),
    Err(err) if is_connection_closed(&err) => Ok(()),
    Err(err) => Err(error::request(err)),
  }
}

fn is_connection_closed(err: &io::Error) -> bool {
  matches!(
    err.kind(),
    io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::NotConnected
  )
}

fn encode_literal_indexed_name_without_indexing(
  block: &mut Vec<u8>,
  name_index: u8,
  value: &[u8],
) -> error::Result<()> {
  if name_index > 15 {
    return Err(error::request(io::Error::new(
      io::ErrorKind::InvalidInput,
      "HPACK literal indexed name is too large for the minimal encoder",
    )));
  }
  block.push(name_index);
  encode_string(block, value)
}

fn encode_literal_new_name_without_indexing(
  block: &mut Vec<u8>,
  name: &[u8],
  value: &[u8],
) -> error::Result<()> {
  block.push(0);
  encode_string(block, name)?;
  encode_string(block, value)
}

fn encode_literal_new_name_with_indexing(
  block: &mut Vec<u8>,
  name: &[u8],
  value: &[u8],
) -> error::Result<()> {
  block.push(0x40);
  encode_string(block, name)?;
  encode_string(block, value)
}

struct RequestHpackEncoder {
  dynamic_entries: Vec<(String, String)>,
  max_size: usize,
  current_size: usize,
}

impl RequestHpackEncoder {
  fn new(max_size: usize) -> Self {
    Self {
      dynamic_entries: Vec::new(),
      max_size,
      current_size: 0,
    }
  }

  fn encode_field(
    &mut self,
    block: &mut Vec<u8>,
    name: &str,
    value: &str,
    remaining_fields: &[(String, String)],
  ) -> error::Result<()> {
    let literal_len = literal_new_name_without_indexing_len(name.as_bytes(), value.as_bytes())?;
    if let Some(index) = self.index(name, value) {
      if hpack_integer_len(index, 7) < literal_len {
        return encode_integer(block, index, 7, 0x80);
      }
    }

    if self.should_index(name, value, literal_len, remaining_fields) {
      encode_literal_new_name_with_indexing(block, name.as_bytes(), value.as_bytes())?;
      self.insert(name.to_string(), value.to_string());
      return Ok(());
    }

    encode_literal_new_name_without_indexing(block, name.as_bytes(), value.as_bytes())
  }

  fn should_index(
    &self,
    name: &str,
    value: &str,
    literal_len: usize,
    remaining_fields: &[(String, String)],
  ) -> bool {
    self.max_size > 0
      && dynamic_entry_size(name, value) <= self.max_size
      && hpack_integer_len(HPACK_STATIC_TABLE_LENGTH + 1, 7) < literal_len
      && remaining_fields
        .iter()
        .any(|(remaining_name, remaining_value)| remaining_name == name && remaining_value == value)
  }

  fn index(&self, name: &str, value: &str) -> Option<usize> {
    self
      .dynamic_entries
      .iter()
      .position(|(entry_name, entry_value)| entry_name == name && entry_value == value)
      .map(|position| HPACK_STATIC_TABLE_LENGTH + 1 + position)
  }

  fn insert(&mut self, name: String, value: String) {
    let entry_size = dynamic_entry_size(&name, &value);
    if entry_size > self.max_size {
      self.dynamic_entries.clear();
      self.current_size = 0;
      return;
    }

    self.dynamic_entries.insert(0, (name, value));
    self.current_size += entry_size;
    self.evict_to_capacity();
  }

  fn evict_to_capacity(&mut self) {
    while self.current_size > self.max_size {
      let Some((name, value)) = self.dynamic_entries.pop() else {
        self.current_size = 0;
        return;
      };
      self.current_size -= dynamic_entry_size(&name, &value);
    }
  }
}

fn literal_new_name_without_indexing_len(name: &[u8], value: &[u8]) -> error::Result<usize> {
  let mut block = Vec::new();
  encode_literal_new_name_without_indexing(&mut block, name, value)?;
  Ok(block.len())
}

fn hpack_integer_len(mut value: usize, prefix_bits: u8) -> usize {
  let max_prefix = (1usize << prefix_bits) - 1;
  if value < max_prefix {
    return 1;
  }

  let mut len = 1;
  value -= max_prefix;
  while value >= 128 {
    len += 1;
    value /= 128;
  }
  len + 1
}

fn encode_string(block: &mut Vec<u8>, value: &[u8]) -> error::Result<()> {
  if let Some(encoded) = encode_huffman_string_if_smaller(value)? {
    encode_integer(block, encoded.len(), 7, 0x80)?;
    block.extend_from_slice(&encoded);
    return Ok(());
  }

  encode_integer(block, value.len(), 7, 0)?;
  block.extend_from_slice(value);
  Ok(())
}

fn encode_huffman_string_if_smaller(value: &[u8]) -> error::Result<Option<Vec<u8>>> {
  let mut bit_len = 0usize;
  for byte in value {
    bit_len = bit_len
      .checked_add(HPACK_HUFFMAN_CODE_LENGTHS[*byte as usize] as usize)
      .ok_or_else(|| error::request(io::Error::other("HPACK Huffman string is too large")))?;
  }

  let encoded_len = bit_len
    .checked_add(7)
    .ok_or_else(|| error::request(io::Error::other("HPACK Huffman string is too large")))?
    / 8;
  if encoded_len >= value.len() {
    return Ok(None);
  }

  let mut encoded = Vec::with_capacity(encoded_len);
  let mut current = 0u8;
  let mut current_len = 0u8;

  for byte in value {
    let code = HPACK_HUFFMAN_CODES[*byte as usize];
    let code_len = HPACK_HUFFMAN_CODE_LENGTHS[*byte as usize];
    for bit_offset in (0..code_len).rev() {
      current = (current << 1) | (((code >> bit_offset) & 1) as u8);
      current_len += 1;
      if current_len == 8 {
        encoded.push(current);
        current = 0;
        current_len = 0;
      }
    }
  }

  if current_len > 0 {
    let padding_len = 8 - current_len;
    current = (current << padding_len) | ((1u8 << padding_len) - 1);
    encoded.push(current);
  }

  Ok(Some(encoded))
}

fn encode_integer(
  block: &mut Vec<u8>,
  mut value: usize,
  prefix_bits: u8,
  first_byte_prefix: u8,
) -> error::Result<()> {
  let max_prefix = (1usize << prefix_bits) - 1;
  if value < max_prefix {
    block.push(first_byte_prefix | value as u8);
    return Ok(());
  }

  block.push(first_byte_prefix | max_prefix as u8);
  value -= max_prefix;
  while value >= 128 {
    block.push((value % 128) as u8 + 128);
    value /= 128;
  }
  block.push(value as u8);
  Ok(())
}

struct DecodedHeaders {
  status: Option<u32>,
  headers: Vec<(String, String)>,
}

struct HpackDecoder {
  dynamic_entries: Vec<(String, String)>,
  dynamic_size: usize,
  max_dynamic_size: usize,
  peer_max_dynamic_size: usize,
}

impl HpackDecoder {
  fn new(peer_max_dynamic_size: usize) -> Self {
    Self {
      dynamic_entries: Vec::new(),
      dynamic_size: 0,
      max_dynamic_size: peer_max_dynamic_size,
      peer_max_dynamic_size,
    }
  }

  fn decode_entries(&mut self, block: &[u8]) -> error::Result<Vec<(String, String)>> {
    let mut cursor = 0;
    let mut entries = Vec::new();

    while cursor < block.len() {
      let byte = block[cursor];
      if byte & 0x80 == 0x80 {
        let (name, value) = self.decode_indexed(block, &mut cursor)?;
        entries.push((name, value));
        continue;
      }

      if byte & 0x40 == 0x40 {
        let (name, value) = self.decode_literal(block, &mut cursor, 6)?;
        self.insert(name.clone(), value.clone())?;
        entries.push((name, value));
      } else if byte & 0x20 == 0x20 {
        self.update_dynamic_size(decode_integer(block, &mut cursor, 5)?)?;
      } else {
        entries.push(self.decode_literal(block, &mut cursor, 4)?);
      }
    }

    Ok(entries)
  }

  fn decode_indexed(&self, block: &[u8], cursor: &mut usize) -> error::Result<(String, String)> {
    self.header(decode_integer(block, cursor, 7)?)
  }

  fn decode_literal(
    &self,
    block: &[u8],
    cursor: &mut usize,
    prefix_bits: u8,
  ) -> error::Result<(String, String)> {
    let name_index = decode_integer(block, cursor, prefix_bits)?;
    let name = if name_index == 0 {
      decode_string(block, cursor)?
    } else {
      self.header(name_index)?.0
    };
    let value = decode_string(block, cursor)?;
    Ok((name, value))
  }

  fn header(&self, index: usize) -> error::Result<(String, String)> {
    if index == 0 {
      return Err(error::bad_response("invalid HPACK header index"));
    }
    if index <= HPACK_STATIC_TABLE_LENGTH {
      let (name, value) = static_header(index)?;
      return Ok((name.to_string(), value.to_string()));
    }

    let dynamic_index = index - HPACK_STATIC_TABLE_LENGTH - 1;
    self
      .dynamic_entries
      .get(dynamic_index)
      .cloned()
      .ok_or_else(|| error::bad_response("invalid HPACK dynamic table index"))
  }

  fn update_dynamic_size(&mut self, size: usize) -> error::Result<()> {
    if size > self.peer_max_dynamic_size {
      return Err(error::bad_response(
        "HPACK dynamic table size update exceeds limit",
      ));
    }
    self.max_dynamic_size = size;
    self.evict_to_capacity();
    Ok(())
  }

  fn insert(&mut self, name: String, value: String) -> error::Result<()> {
    let entry_size = name
      .len()
      .checked_add(value.len())
      .and_then(|size| size.checked_add(32))
      .ok_or_else(|| error::bad_response("HPACK dynamic table entry is too large"))?;

    if entry_size > self.max_dynamic_size {
      self.dynamic_entries.clear();
      self.dynamic_size = 0;
      return Ok(());
    }

    self.dynamic_entries.insert(0, (name, value));
    self.dynamic_size = self
      .dynamic_size
      .checked_add(entry_size)
      .ok_or_else(|| error::bad_response("HPACK dynamic table size overflow"))?;
    self.evict_to_capacity();
    Ok(())
  }

  fn evict_to_capacity(&mut self) {
    while self.dynamic_size > self.max_dynamic_size {
      if let Some((name, value)) = self.dynamic_entries.pop() {
        self.dynamic_size -= dynamic_entry_size(&name, &value);
      } else {
        self.dynamic_size = 0;
      }
    }
  }
}

fn dynamic_entry_size(name: &str, value: &str) -> usize {
  32 + name.len() + value.len()
}

fn decode_header_block(block: &[u8], hpack: &mut HpackDecoder) -> error::Result<DecodedHeaders> {
  let entries = hpack.decode_entries(block)?;
  let mut status = None;
  let mut headers = Vec::new();

  for (name, value) in entries {
    push_decoded_header(&name, &value, &mut status, &mut headers)?;
  }

  Ok(DecodedHeaders { status, headers })
}

fn decode_trailer_block(block: &[u8], hpack: &mut HpackDecoder) -> error::Result<Vec<Header>> {
  let entries = hpack.decode_entries(block)?;
  let mut trailers = Vec::new();

  for (name, value) in entries {
    if name.starts_with(':') {
      return Err(error::bad_response("Invalid trailer header"));
    }
    validate_response_trailer_header(&name, &value)?;
    trailers.push(Header::new(name, value));
  }

  Ok(trailers)
}

fn decode_integer(block: &[u8], cursor: &mut usize, prefix_bits: u8) -> error::Result<usize> {
  if *cursor >= block.len() {
    return Err(error::bad_response("truncated HPACK integer"));
  }

  let max_prefix = (1usize << prefix_bits) - 1;
  let mut value = (block[*cursor] as usize) & max_prefix;
  *cursor += 1;
  if value < max_prefix {
    return Ok(value);
  }

  let mut shift = 0;
  loop {
    if *cursor >= block.len() {
      return Err(error::bad_response("truncated HPACK integer"));
    }
    let byte = block[*cursor];
    *cursor += 1;
    value += ((byte & 0x7f) as usize) << shift;
    if byte & 0x80 == 0 {
      return Ok(value);
    }
    shift += 7;
  }
}

fn decode_string(block: &[u8], cursor: &mut usize) -> error::Result<String> {
  if *cursor >= block.len() {
    return Err(error::bad_response("truncated HPACK string"));
  }
  let huffman = block[*cursor] & 0x80 == 0x80;
  let len = decode_integer(block, cursor, 7)?;
  let end = cursor
    .checked_add(len)
    .ok_or_else(|| error::bad_response("HPACK string length overflow"))?;
  if end > block.len() {
    return Err(error::bad_response("truncated HPACK string"));
  }
  let value = if huffman {
    decode_huffman_string(&block[*cursor..end])?
  } else {
    String::from_utf8(block[*cursor..end].to_vec()).map_err(error::response)?
  };
  *cursor = end;
  Ok(value)
}

fn decode_huffman_string(encoded: &[u8]) -> error::Result<String> {
  let mut decoded = Vec::new();
  let mut code = 0u32;
  let mut code_len = 0u8;

  for byte in encoded {
    for bit_offset in (0..8).rev() {
      code = (code << 1) | (((byte >> bit_offset) & 1) as u32);
      code_len += 1;

      if code_len > HPACK_HUFFMAN_EOS_BITS {
        return Err(error::bad_response("invalid HPACK Huffman code"));
      }

      if let Some(symbol) = hpack_huffman_symbol(code, code_len) {
        if symbol == HPACK_HUFFMAN_EOS_SYMBOL {
          return Err(error::bad_response("HPACK Huffman EOS symbol used as data"));
        }
        decoded.push(symbol as u8);
        code = 0;
        code_len = 0;
      }
    }
  }

  if code_len > 0 {
    let eos_padding = code == ((1u32 << code_len) - 1);
    if eos_padding {
      if code_len > 7 {
        return Err(error::bad_response("overlong HPACK Huffman padding"));
      }
    } else if code_len > 7 && hpack_huffman_has_prefix(code, code_len) {
      return Err(error::bad_response("truncated HPACK Huffman code"));
    } else {
      return Err(error::bad_response("invalid HPACK Huffman padding"));
    }
  }

  String::from_utf8(decoded).map_err(error::response)
}

fn hpack_huffman_symbol(code: u32, code_len: u8) -> Option<usize> {
  HPACK_HUFFMAN_CODES
    .iter()
    .zip(HPACK_HUFFMAN_CODE_LENGTHS.iter())
    .position(|(&candidate, &candidate_len)| candidate_len == code_len && candidate == code)
}

fn hpack_huffman_has_prefix(code: u32, code_len: u8) -> bool {
  HPACK_HUFFMAN_CODES
    .iter()
    .zip(HPACK_HUFFMAN_CODE_LENGTHS.iter())
    .any(|(&candidate, &candidate_len)| {
      candidate_len > code_len && (candidate >> (candidate_len - code_len)) == code
    })
}

fn push_decoded_header(
  name: &str,
  value: &str,
  status: &mut Option<u32>,
  headers: &mut Vec<(String, String)>,
) -> error::Result<()> {
  if name == ":status" {
    *status = Some(
      value
        .parse::<u32>()
        .map_err(|_| error::bad_response("invalid HTTP/2 :status header"))?,
    );
  } else if !name.starts_with(':') {
    headers.push((name.to_string(), value.to_string()));
  }
  Ok(())
}

fn validate_response_trailer_header(name: &str, value: &str) -> error::Result<()> {
  if name.is_empty()
    || name.contains(|ch: char| ch.is_ascii_control() || ch == ':' || ch == ' ')
    || value.contains('\r')
    || value.contains('\n')
  {
    return Err(error::bad_response("Invalid trailer header"));
  }
  if is_forbidden_response_trailer_name(name) {
    return Err(error::bad_response("Forbidden trailer header"));
  }
  Ok(())
}

fn is_forbidden_response_trailer_name(name: &str) -> bool {
  matches!(
    name.to_ascii_lowercase().as_str(),
    "authorization"
      | "connection"
      | "content-length"
      | "cookie"
      | "host"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "www-authenticate"
      | "set-cookie"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
  )
}

fn static_header(index: usize) -> error::Result<(&'static str, &'static str)> {
  match index {
    1 => Ok((":authority", "")),
    2 => Ok((":method", "GET")),
    3 => Ok((":method", "POST")),
    4 => Ok((":path", "/")),
    5 => Ok((":path", "/index.html")),
    6 => Ok((":scheme", "http")),
    7 => Ok((":scheme", "https")),
    8 => Ok((":status", "200")),
    9 => Ok((":status", "204")),
    10 => Ok((":status", "206")),
    11 => Ok((":status", "304")),
    12 => Ok((":status", "400")),
    13 => Ok((":status", "404")),
    14 => Ok((":status", "500")),
    15 => Ok(("accept-charset", "")),
    16 => Ok(("accept-encoding", "gzip, deflate")),
    17 => Ok(("accept-language", "")),
    18 => Ok(("accept-ranges", "")),
    19 => Ok(("accept", "")),
    20 => Ok(("access-control-allow-origin", "")),
    21 => Ok(("age", "")),
    22 => Ok(("allow", "")),
    23 => Ok(("authorization", "")),
    24 => Ok(("cache-control", "")),
    25 => Ok(("content-disposition", "")),
    26 => Ok(("content-encoding", "")),
    27 => Ok(("content-language", "")),
    28 => Ok(("content-length", "")),
    29 => Ok(("content-location", "")),
    30 => Ok(("content-range", "")),
    31 => Ok(("content-type", "")),
    32 => Ok(("cookie", "")),
    33 => Ok(("date", "")),
    34 => Ok(("etag", "")),
    35 => Ok(("expect", "")),
    36 => Ok(("expires", "")),
    37 => Ok(("from", "")),
    38 => Ok(("host", "")),
    39 => Ok(("if-match", "")),
    40 => Ok(("if-modified-since", "")),
    41 => Ok(("if-none-match", "")),
    42 => Ok(("if-range", "")),
    43 => Ok(("if-unmodified-since", "")),
    44 => Ok(("last-modified", "")),
    45 => Ok(("link", "")),
    46 => Ok(("location", "")),
    47 => Ok(("max-forwards", "")),
    48 => Ok(("proxy-authenticate", "")),
    49 => Ok(("proxy-authorization", "")),
    50 => Ok(("range", "")),
    51 => Ok(("referer", "")),
    52 => Ok(("refresh", "")),
    53 => Ok(("retry-after", "")),
    54 => Ok(("server", "")),
    55 => Ok(("set-cookie", "")),
    56 => Ok(("strict-transport-security", "")),
    57 => Ok(("transfer-encoding", "")),
    58 => Ok(("user-agent", "")),
    59 => Ok(("vary", "")),
    60 => Ok(("via", "")),
    61 => Ok(("www-authenticate", "")),
    _ => Err(error::bad_response("unsupported HPACK static table index")),
  }
}
