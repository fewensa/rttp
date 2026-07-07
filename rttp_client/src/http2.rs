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
const FRAME_GOAWAY: u8 = 0x7;
const FRAME_WINDOW_UPDATE: u8 = 0x8;
const FRAME_CONTINUATION: u8 = 0x9;

const FLAG_END_STREAM: u8 = 0x1;
const FLAG_ACK: u8 = 0x1;
const FLAG_END_HEADERS: u8 = 0x4;

const STREAM_ID: u32 = 1;
const SETTING_MAX_FRAME_SIZE: u16 = 0x5;
const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;
const MAX_FRAME_SIZE_LIMIT: usize = 16_777_215;
const WINDOW_UPDATE_THRESHOLD: usize = 32 * 1024;

pub struct PriorKnowledgeClient<'a> {
  request: RawRequest<'a>,
}

impl<'a> PriorKnowledgeClient<'a> {
  pub(crate) fn new(request: RawRequest<'a>) -> Self {
    Self { request }
  }

  pub fn get(mut self) -> error::Result<Response> {
    let method = self.request.origin().method();
    if method.eq_ignore_ascii_case("GET") && self.request.body().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge GET cannot send a request body",
      ));
    }
    if !is_supported_request_method(method) {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge client supports GET and buffered POST, PUT, or PATCH",
      ));
    }

    let url = self.request.url().to_url().map_err(error::builder)?;
    if url.scheme() != "http" {
      return Err(error::url_bad_scheme(url));
    }

    let mut stream = connect_tcp_stream(addr(&url)?, self.request.origin().config())?;
    write_connection_preface(&mut stream)?;
    let peer_max_frame_size = read_settings_and_ack(&mut stream)?;
    write_request(&mut stream, &self.request, &url, peer_max_frame_size)?;
    let response = read_single_stream_response(&mut stream, self.request.url().clone())?;
    self.request.origin_mut().closed_set(true);
    Ok(response)
  }
}

fn is_supported_request_method(method: &str) -> bool {
  method.eq_ignore_ascii_case("GET")
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

fn read_settings_and_ack(stream: &mut TcpStream) -> error::Result<usize> {
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
    if frame.stream_id != 0 || frame.payload.len() % 6 != 0 {
      return Err(error::bad_response("invalid HTTP/2 SETTINGS frame"));
    }
    let peer_max_frame_size = peer_max_frame_size(&frame.payload)?;
    write_frame(stream, FRAME_SETTINGS, FLAG_ACK, 0, &[])?;
    stream.flush().map_err(error::request)?;
    return Ok(peer_max_frame_size);
  }
}

fn peer_max_frame_size(payload: &[u8]) -> error::Result<usize> {
  let mut max_frame_size = DEFAULT_MAX_FRAME_SIZE;
  for setting in payload.chunks_exact(6) {
    let identifier = u16::from_be_bytes([setting[0], setting[1]]);
    let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]) as usize;
    if identifier == SETTING_MAX_FRAME_SIZE {
      if !(DEFAULT_MAX_FRAME_SIZE..=MAX_FRAME_SIZE_LIMIT).contains(&value) {
        return Err(error::bad_response(
          "invalid HTTP/2 SETTINGS_MAX_FRAME_SIZE value",
        ));
      }
      max_frame_size = value;
    }
  }
  Ok(max_frame_size)
}

fn write_request(
  stream: &mut TcpStream,
  request: &RawRequest<'_>,
  url: &Url,
  peer_max_frame_size: usize,
) -> error::Result<()> {
  let header_block = encode_request_headers(request, url)?;
  let body = request
    .body()
    .as_ref()
    .map(|body| body.bytes())
    .unwrap_or(&[]);
  let header_flags = if body.is_empty() {
    FLAG_END_STREAM | FLAG_END_HEADERS
  } else {
    FLAG_END_HEADERS
  };
  write_frame(
    stream,
    FRAME_HEADERS,
    header_flags,
    STREAM_ID,
    &header_block,
  )?;
  if !body.is_empty() {
    write_data_frames(stream, body, peer_max_frame_size)?;
  }
  stream.flush().map_err(error::request)
}

fn write_data_frames(
  stream: &mut TcpStream,
  body: &[u8],
  peer_max_frame_size: usize,
) -> error::Result<()> {
  for (index, chunk) in body.chunks(peer_max_frame_size).enumerate() {
    let sent = index
      .checked_mul(peer_max_frame_size)
      .ok_or_else(|| error::request(io::Error::other("HTTP/2 request body is too large")))?;
    let flags = if sent + chunk.len() == body.len() {
      FLAG_END_STREAM
    } else {
      0
    };
    write_frame(stream, FRAME_DATA, flags, STREAM_ID, chunk)?;
  }
  Ok(())
}

fn encode_request_headers(request: &RawRequest<'_>, url: &Url) -> error::Result<Vec<u8>> {
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

  for (name, value) in regular_headers(request.header()) {
    encode_literal_new_name_without_indexing(&mut block, name.as_bytes(), value.as_bytes())?;
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

fn read_single_stream_response(stream: &mut TcpStream, url: RoUrl) -> error::Result<Response> {
  let mut header_block = Vec::new();
  let mut headers = Vec::new();
  let mut trailers = Vec::new();
  let mut body = Vec::new();
  let mut status = None;
  let mut pending_header_block = None;
  let mut final_response_started = false;
  let mut response_body_started = false;
  let mut pending_window_update = 0usize;

  loop {
    let frame = read_frame(stream)?;
    if pending_header_block.is_some()
      && (frame.frame_type != FRAME_CONTINUATION || frame.stream_id != STREAM_ID)
    {
      return Err(error::bad_response(
        "expected HTTP/2 CONTINUATION frame for incomplete header block",
      ));
    }
    match (frame.frame_type, frame.stream_id) {
      (FRAME_SETTINGS, 0) => {
        if frame.flags & FLAG_ACK == 0 {
          write_frame(stream, FRAME_SETTINGS, FLAG_ACK, 0, &[])?;
          stream.flush().map_err(error::request)?;
        } else if !frame.payload.is_empty() {
          return Err(error::bad_response(
            "HTTP/2 SETTINGS ACK frame must not contain payload",
          ));
        }
      }
      (FRAME_HEADERS, STREAM_ID) => {
        let kind = if final_response_started || response_body_started {
          HeaderBlockKind::Trailers
        } else {
          HeaderBlockKind::ResponseHeaders
        };
        let end_stream = frame.flags & FLAG_END_STREAM == FLAG_END_STREAM;
        header_block.extend_from_slice(&frame.payload);
        if frame.flags & FLAG_END_HEADERS == FLAG_END_HEADERS {
          if apply_header_block(
            kind,
            &header_block,
            &mut status,
            &mut headers,
            &mut trailers,
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
        response_body_started = true;
        body.extend_from_slice(&frame.payload);
        pending_window_update = pending_window_update
          .checked_add(frame.payload.len())
          .ok_or_else(|| error::bad_response("HTTP/2 response body is too large"))?;
        if !end_stream && pending_window_update >= WINDOW_UPDATE_THRESHOLD {
          write_window_update_best_effort(stream, STREAM_ID, pending_window_update)?;
          write_window_update_best_effort(stream, 0, pending_window_update)?;
          flush_best_effort(stream)?;
          pending_window_update = 0;
        }
        if end_stream {
          break;
        }
      }
      (FRAME_RST_STREAM, STREAM_ID) => {
        return Err(error::bad_response("HTTP/2 stream received RST_STREAM"));
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

fn apply_header_block(
  kind: HeaderBlockKind,
  block: &[u8],
  status: &mut Option<u32>,
  headers: &mut Vec<(String, String)>,
  trailers: &mut Vec<Header>,
) -> error::Result<bool> {
  match kind {
    HeaderBlockKind::ResponseHeaders => {
      let decoded = decode_header_block(block)?;
      if decoded.status.is_some_and(is_informational_status) {
        return Ok(false);
      }
      *status = decoded.status;
      *headers = decoded.headers;
      Ok(status.is_some())
    }
    HeaderBlockKind::Trailers => {
      trailers.extend(decode_trailer_block(block)?);
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

fn encode_string(block: &mut Vec<u8>, value: &[u8]) -> error::Result<()> {
  encode_integer(block, value.len(), 7, 0)?;
  block.extend_from_slice(value);
  Ok(())
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

fn decode_header_block(block: &[u8]) -> error::Result<DecodedHeaders> {
  let entries = decode_header_entries(block)?;
  let mut status = None;
  let mut headers = Vec::new();

  for (name, value) in entries {
    push_decoded_header(&name, &value, &mut status, &mut headers)?;
  }

  Ok(DecodedHeaders { status, headers })
}

fn decode_trailer_block(block: &[u8]) -> error::Result<Vec<Header>> {
  let entries = decode_header_entries(block)?;
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

fn decode_header_entries(block: &[u8]) -> error::Result<Vec<(String, String)>> {
  let mut cursor = 0;
  let mut entries = Vec::new();

  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      let index = decode_integer(block, &mut cursor, 7)?;
      let (name, value) = static_header(index)?;
      entries.push((name.to_string(), value.to_string()));
      continue;
    }

    let (name, value) = if byte & 0x40 == 0x40 {
      decode_literal(block, &mut cursor, 6)?
    } else if byte & 0x20 == 0x20 {
      return Err(error::bad_response(
        "HTTP/2 dynamic table size updates are not supported",
      ));
    } else {
      decode_literal(block, &mut cursor, 4)?
    };
    entries.push((name, value));
  }

  Ok(entries)
}

fn decode_literal(
  block: &[u8],
  cursor: &mut usize,
  prefix_bits: u8,
) -> error::Result<(String, String)> {
  let name_index = decode_integer(block, cursor, prefix_bits)?;
  let name = if name_index == 0 {
    decode_string(block, cursor)?
  } else {
    static_header(name_index)?.0.to_string()
  };
  let value = decode_string(block, cursor)?;
  Ok((name, value))
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
  if huffman {
    return Err(error::bad_response(
      "HPACK Huffman strings are not supported by the minimal decoder",
    ));
  }
  let end = cursor
    .checked_add(len)
    .ok_or_else(|| error::bad_response("HPACK string length overflow"))?;
  if end > block.len() {
    return Err(error::bad_response("truncated HPACK string"));
  }
  let value = String::from_utf8(block[*cursor..end].to_vec()).map_err(error::response)?;
  *cursor = end;
  Ok(value)
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
