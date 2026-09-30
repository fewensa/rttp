//! Shared Content-Encoding decoder-stack helpers for buffered and streaming paths.

use std::io::{self, Read, Write};

use flate2::write::MultiGzDecoder;
use rttp_protocol::content_encoding::ContentEncoding;

use crate::error;
use crate::types::Header;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContentDecoder {
  Gzip,
  Deflate,
}

/// Returns a fully supported gzip/deflate decoder stack in header order, or
/// `None` when the coding list is absent, empty, unsupported, or unparsable.
pub(crate) fn content_decoders(headers: &[Header]) -> Option<Vec<ContentDecoder>> {
  let values: Vec<&str> = headers
    .iter()
    .filter(|header| header.name().eq_ignore_ascii_case("Content-Encoding"))
    .map(|header| header.value().as_str())
    .collect();
  if values.is_empty() {
    return None;
  }
  let parsed = ContentEncoding::parse_values(values).ok()?;
  let mut decoders = Vec::with_capacity(parsed.len());
  for coding in parsed.codings() {
    if coding.eq_ignore_ascii_case("gzip") {
      decoders.push(ContentDecoder::Gzip);
    } else if coding.eq_ignore_ascii_case("deflate") {
      decoders.push(ContentDecoder::Deflate);
    } else {
      return None;
    }
  }
  if decoders.is_empty() {
    return None;
  }
  Some(decoders)
}

pub(crate) fn strip_stale_representation_headers(headers: &mut Vec<Header>) {
  headers.retain(|header| {
    !header.name().eq_ignore_ascii_case("Content-Encoding")
      && !header.name().eq_ignore_ascii_case("Content-Length")
  });
}

pub(crate) fn read_decoded_body_to_end<R: Read>(
  reader: &mut R,
  body: &mut Vec<u8>,
  max_body_bytes: usize,
) -> error::Result<()> {
  let mut buffer = [0u8; 8 * 1024];
  loop {
    let remaining = max_body_bytes - body.len();
    let read_limit = buffer.len().min(remaining.saturating_add(1));
    let read = reader
      .read(&mut buffer[..read_limit])
      .map_err(error::decode)?;
    if read == 0 {
      return Ok(());
    }
    if read > remaining {
      return Err(error::body_too_large(max_body_bytes));
    }
    body.extend_from_slice(&buffer[..read]);
  }
}

pub(crate) fn decode_deflate_buffer(
  compressed: &[u8],
  max_body_bytes: usize,
) -> error::Result<Vec<u8>> {
  let mut decoded = Vec::new();
  match read_decoded_body_to_end(
    &mut flate2::read::ZlibDecoder::new(compressed),
    &mut decoded,
    max_body_bytes,
  ) {
    Ok(()) => Ok(decoded),
    Err(error) if error.is_body_too_large() => Err(error),
    Err(_) => {
      decoded.clear();
      read_decoded_body_to_end(
        &mut flate2::read::DeflateDecoder::new(compressed),
        &mut decoded,
        max_body_bytes,
      )?;
      Ok(decoded)
    }
  }
}

/// Pull-based stacked content decoder for streaming body readers.
///
/// Layers are stored outermost-first (reverse of the Content-Encoding header
/// order). Gzip layers stream through `MultiGzDecoder` (including concatenated
/// members). Deflate layers buffer compressed input until EOF, then apply the
/// same zlib-then-raw fallback as buffered decoding.
pub(crate) struct StreamingDecodeStack {
  layers: Vec<DecoderLayer>,
  pending: Vec<u8>,
  pending_pos: usize,
  decoded_total: usize,
  max_decoded: usize,
  input_finished: bool,
  layers_finished: bool,
  output_finished: bool,
  wire_capture: Vec<u8>,
}

enum DecoderLayer {
  Gzip { decoder: MultiGzDecoder<Vec<u8>> },
  Deflate { compressed: Vec<u8> },
}

impl StreamingDecodeStack {
  pub(crate) fn new(decoders: Vec<ContentDecoder>, max_decoded: usize) -> Self {
    let layers = decoders
      .into_iter()
      .rev()
      .map(|decoder| match decoder {
        ContentDecoder::Gzip => DecoderLayer::Gzip {
          decoder: MultiGzDecoder::new(Vec::new()),
        },
        ContentDecoder::Deflate => DecoderLayer::Deflate {
          compressed: Vec::new(),
        },
      })
      .collect();
    Self {
      layers,
      pending: Vec::new(),
      pending_pos: 0,
      decoded_total: 0,
      max_decoded,
      input_finished: false,
      layers_finished: false,
      output_finished: false,
      wire_capture: Vec::new(),
    }
  }

  pub(crate) fn take_wire_capture(&mut self) -> Vec<u8> {
    std::mem::take(&mut self.wire_capture)
  }

  pub(crate) fn fill_pending(&mut self, buf: &mut [u8]) -> Option<usize> {
    if self.pending_pos >= self.pending.len() {
      return None;
    }
    let available = &self.pending[self.pending_pos..];
    let copy = available.len().min(buf.len());
    buf[..copy].copy_from_slice(&available[..copy]);
    self.pending_pos += copy;
    if self.pending_pos == self.pending.len() {
      self.pending.clear();
      self.pending_pos = 0;
    }
    Some(copy)
  }

  pub(crate) fn output_finished(&self) -> bool {
    self.output_finished
  }

  pub(crate) fn feed_wire(&mut self, chunk: &[u8]) -> error::Result<()> {
    self.wire_capture.extend_from_slice(chunk);
    self.feed_layers(chunk)
  }

  pub(crate) fn finish_input(&mut self) -> error::Result<()> {
    if self.output_finished {
      return Ok(());
    }
    self.input_finished = true;
    if !self.layers_finished {
      if self.wire_capture.is_empty() {
        // Empty framed body: match buffered "empty bodies are not decoded".
        self.layers_finished = true;
        self.output_finished = true;
        return Ok(());
      }
      self.finish_layers()?;
      self.layers_finished = true;
    }
    if self.pending.is_empty() {
      self.output_finished = true;
    }
    Ok(())
  }

  pub(crate) fn read_from_with<F>(
    &mut self,
    buf: &mut [u8],
    mut read_framed: F,
  ) -> io::Result<usize>
  where
    F: FnMut(&mut [u8]) -> io::Result<usize>,
  {
    if buf.is_empty() {
      return Ok(0);
    }
    loop {
      if let Some(copy) = self.fill_pending(buf) {
        return Ok(copy);
      }

      if self.output_finished {
        return Ok(0);
      }

      if self.input_finished {
        self.finish_input().map_err(streaming_decode_io_error)?;
        if self.pending.is_empty() {
          return Ok(0);
        }
        continue;
      }

      let mut chunk = [0u8; 8 * 1024];
      let read = read_framed(&mut chunk)?;
      if read == 0 {
        self.finish_input().map_err(streaming_decode_io_error)?;
        continue;
      }
      self
        .feed_wire(&chunk[..read])
        .map_err(streaming_decode_io_error)?;
    }
  }

  fn feed_layers(&mut self, data: &[u8]) -> error::Result<()> {
    let mut current = data.to_vec();
    let last = self.layers.len() - 1;
    for index in 0..=last {
      if current.is_empty() {
        return Ok(());
      }
      current = self.feed_layer(index, &current)?;
      if index == last {
        self.push_decoded(&current)?;
      } else {
        self.ensure_intermediate_within_limit(current.len())?;
      }
    }
    Ok(())
  }

  fn feed_layer(&mut self, index: usize, input: &[u8]) -> error::Result<Vec<u8>> {
    match &mut self.layers[index] {
      DecoderLayer::Gzip { decoder } => {
        decoder.write_all(input).map_err(error::decode)?;
        Ok(std::mem::take(decoder.get_mut()))
      }
      DecoderLayer::Deflate { compressed } => {
        if index > 0
          && compressed
            .len()
            .checked_add(input.len())
            .is_none_or(|len| len > self.max_decoded)
        {
          return Err(error::body_too_large(self.max_decoded));
        }
        compressed.extend_from_slice(input);
        Ok(Vec::new())
      }
    }
  }

  fn finish_layers(&mut self) -> error::Result<()> {
    let mut current = Vec::new();
    let last = self.layers.len() - 1;
    for index in 0..=last {
      current = self.finish_layer(index, &current)?;
      if index == last {
        self.push_decoded(&current)?;
      } else {
        self.ensure_intermediate_within_limit(current.len())?;
      }
    }
    Ok(())
  }

  fn finish_layer(&mut self, index: usize, carried: &[u8]) -> error::Result<Vec<u8>> {
    match &mut self.layers[index] {
      DecoderLayer::Gzip { decoder } => {
        if !carried.is_empty() {
          decoder.write_all(carried).map_err(error::decode)?;
        }
        decoder.try_finish().map_err(error::decode)?;
        Ok(std::mem::take(decoder.get_mut()))
      }
      DecoderLayer::Deflate { compressed } => {
        if !carried.is_empty() {
          if index > 0
            && compressed
              .len()
              .checked_add(carried.len())
              .is_none_or(|len| len > self.max_decoded)
          {
            return Err(error::body_too_large(self.max_decoded));
          }
          compressed.extend_from_slice(carried);
        }
        let compressed = std::mem::take(compressed);
        decode_deflate_buffer(&compressed, self.max_decoded)
      }
    }
  }

  fn ensure_intermediate_within_limit(&self, len: usize) -> error::Result<()> {
    if len > self.max_decoded {
      return Err(error::body_too_large(self.max_decoded));
    }
    Ok(())
  }

  fn push_decoded(&mut self, data: &[u8]) -> error::Result<()> {
    if data.is_empty() {
      return Ok(());
    }
    let remaining = self.max_decoded.saturating_sub(self.decoded_total);
    if data.len() > remaining {
      return Err(error::body_too_large(self.max_decoded));
    }
    self.decoded_total += data.len();
    if self.pending_pos > 0 {
      self.pending.drain(..self.pending_pos);
      self.pending_pos = 0;
    }
    self.pending.extend_from_slice(data);
    Ok(())
  }
}

pub(crate) fn streaming_decode_io_error(err: error::Error) -> io::Error {
  if err.is_body_too_large() {
    io::Error::other(err)
  } else {
    io::Error::new(io::ErrorKind::InvalidData, err)
  }
}

pub(crate) fn map_streaming_body_io_error(err: io::Error) -> error::Error {
  if let Some(limit) = err
    .get_ref()
    .and_then(|source| source.downcast_ref::<error::Error>())
    .and_then(error::Error::body_limit)
  {
    return error::body_too_large(limit);
  }
  if let Some(inner) = err
    .get_ref()
    .and_then(|source| source.downcast_ref::<error::Error>())
  {
    let message = inner.to_string();
    if message.starts_with("error decoding response body") {
      return error::decode(message);
    }
    if inner.is_body_too_large() {
      return error::body_too_large(inner.body_limit().unwrap_or(0));
    }
  }
  let message = err.to_string();
  if message.starts_with("error decoding response body") {
    return error::decode(message);
  }
  match err.kind() {
    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
      error::bad_response(err.to_string())
    }
    _ => error::request(err),
  }
}
