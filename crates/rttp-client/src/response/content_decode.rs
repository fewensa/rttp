use std::io::{self, Write};

use rttp_protocol::content_encoding::ContentEncoding;

use crate::error;
use crate::types::Header;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContentDecoder {
  Gzip,
  Deflate,
}

/// Select a fully supported gzip / zlib-deflate stack for decoding.
///
/// Returns `None` for missing, empty, identity, unknown, mixed-unsupported, or
/// parse-invalid `Content-Encoding` values so callers preserve raw headers and
/// body bytes.
pub(crate) fn content_decoders(headers: &[Header]) -> Option<Vec<ContentDecoder>> {
  let parsed = ContentEncoding::parse_values(
    headers
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case("Content-Encoding"))
      .map(|header| header.value().as_str()),
  )
  .ok()?;
  if parsed.codings().is_empty() {
    return None;
  }
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
  Some(decoders)
}

pub(crate) fn strip_content_encoding_and_length(headers: &mut Vec<Header>) {
  headers.retain(|header| {
    !header.name().eq_ignore_ascii_case("Content-Encoding")
      && !header.name().eq_ignore_ascii_case("Content-Length")
  });
}

enum LayerWriter {
  Gzip(flate2::write::GzDecoder<Vec<u8>>),
  Deflate(flate2::write::ZlibDecoder<Vec<u8>>),
}

impl LayerWriter {
  fn new(decoder: ContentDecoder) -> Self {
    match decoder {
      ContentDecoder::Gzip => Self::Gzip(flate2::write::GzDecoder::new(Vec::new())),
      ContentDecoder::Deflate => Self::Deflate(flate2::write::ZlibDecoder::new(Vec::new())),
    }
  }

  fn write_all(&mut self, input: &[u8]) -> io::Result<()> {
    match self {
      Self::Gzip(writer) => writer.write_all(input),
      Self::Deflate(writer) => writer.write_all(input),
    }
  }

  fn take_output(&mut self) -> io::Result<Vec<u8>> {
    match self {
      Self::Gzip(writer) => {
        writer.flush()?;
        Ok(std::mem::take(writer.get_mut()))
      }
      Self::Deflate(writer) => {
        writer.flush()?;
        Ok(std::mem::take(writer.get_mut()))
      }
    }
  }

  fn finish(self) -> io::Result<Vec<u8>> {
    match self {
      Self::Gzip(writer) => writer.finish(),
      Self::Deflate(writer) => writer.finish(),
    }
  }
}

/// Incremental reverse-order gzip / zlib-deflate decoder for streaming bodies.
pub(crate) struct StreamingContentDecoder {
  layers: Vec<LayerWriter>,
  layer_decoded_bytes: Vec<usize>,
  pending: Vec<u8>,
  pending_pos: usize,
  max_decoded_bytes: Option<usize>,
  saw_input: bool,
  finished: bool,
  succeeded: bool,
}

impl StreamingContentDecoder {
  pub(crate) fn new(decoders: &[ContentDecoder], max_decoded_bytes: Option<usize>) -> Self {
    // Header order is applied outer-last on the wire; decode in reverse.
    let layers: Vec<LayerWriter> = decoders
      .iter()
      .rev()
      .copied()
      .map(LayerWriter::new)
      .collect();
    let layer_count = layers.len();
    Self {
      layers,
      layer_decoded_bytes: vec![0; layer_count],
      pending: Vec::new(),
      pending_pos: 0,
      max_decoded_bytes,
      saw_input: false,
      finished: false,
      succeeded: false,
    }
  }

  pub(crate) fn succeeded(&self) -> bool {
    self.succeeded
  }

  pub(crate) fn finished(&self) -> bool {
    self.finished
  }

  pub(crate) fn set_max_decoded_bytes(&mut self, max_decoded_bytes: Option<usize>) {
    self.max_decoded_bytes = max_decoded_bytes;
  }

  fn record_layer_bytes(&mut self, layer_index: usize, produced: usize) -> error::Result<()> {
    if produced == 0 {
      return Ok(());
    }
    let total = self.layer_decoded_bytes[layer_index]
      .checked_add(produced)
      .ok_or_else(|| {
        self
          .max_decoded_bytes
          .map(error::body_too_large)
          .unwrap_or_else(|| error::decode("decoded response body is too large"))
      })?;
    if let Some(max) = self.max_decoded_bytes {
      if total > max {
        return Err(error::body_too_large(max));
      }
    }
    self.layer_decoded_bytes[layer_index] = total;
    Ok(())
  }

  fn push_pending(&mut self, bytes: Vec<u8>) {
    if bytes.is_empty() {
      return;
    }
    if self.pending_pos > 0 {
      self.pending.drain(..self.pending_pos);
      self.pending_pos = 0;
    }
    self.pending.extend_from_slice(&bytes);
  }

  fn pass_through_layers(&mut self, mut current: Vec<u8>) -> error::Result<()> {
    for index in 0..self.layers.len() {
      if current.is_empty() {
        break;
      }
      self.layers[index]
        .write_all(&current)
        .map_err(error::decode)?;
      current = self.layers[index].take_output().map_err(error::decode)?;
      self.record_layer_bytes(index, current.len())?;
    }
    self.push_pending(current);
    Ok(())
  }

  pub(crate) fn feed(&mut self, compressed: &[u8]) -> error::Result<()> {
    if compressed.is_empty() {
      return Ok(());
    }
    if self.finished {
      return Err(error::decode("content decoder already finished"));
    }
    self.saw_input = true;
    self.pass_through_layers(compressed.to_vec())
  }

  pub(crate) fn finish(&mut self) -> error::Result<()> {
    if self.finished {
      return Ok(());
    }
    self.finished = true;
    if !self.saw_input {
      // Empty wire bodies are not decoded; leave headers untouched.
      self.layers.clear();
      return Ok(());
    }

    let layers = std::mem::take(&mut self.layers);
    let mut carry = Vec::new();
    for (index, layer) in layers.into_iter().enumerate() {
      let mut layer = layer;
      if !carry.is_empty() {
        layer.write_all(&carry).map_err(error::decode)?;
      }
      let finished = layer.finish().map_err(error::decode)?;
      self.record_layer_bytes(index, finished.len())?;
      carry = finished;
    }
    self.push_pending(carry);
    self.succeeded = true;
    Ok(())
  }

  pub(crate) fn fill(&mut self, buf: &mut [u8]) -> usize {
    if self.pending_pos >= self.pending.len() || buf.is_empty() {
      return 0;
    }
    let available = &self.pending[self.pending_pos..];
    let n = available.len().min(buf.len());
    buf[..n].copy_from_slice(&available[..n]);
    self.pending_pos += n;
    if self.pending_pos >= self.pending.len() {
      self.pending.clear();
      self.pending_pos = 0;
    }
    n
  }

  pub(crate) fn has_pending(&self) -> bool {
    self.pending_pos < self.pending.len()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use flate2::write::{GzEncoder, ZlibEncoder};
  use flate2::Compression;
  use std::io::Write;

  fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
  }

  fn zlib(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
  }

  #[test]
  fn streams_single_gzip() {
    let compressed = gzip(b"hello");
    let mut decoder = StreamingContentDecoder::new(&[ContentDecoder::Gzip], None);
    decoder.feed(&compressed[..3]).unwrap();
    decoder.feed(&compressed[3..]).unwrap();
    decoder.finish().unwrap();
    let mut out = vec![0; 16];
    let n = decoder.fill(&mut out);
    assert_eq!(b"hello", &out[..n]);
    assert!(decoder.succeeded());
  }

  #[test]
  fn streams_gzip_then_deflate_stack() {
    // Header order gzip, deflate => wire is zlib(gzip(data))
    let compressed = zlib(&gzip(b"OK"));
    let mut decoder =
      StreamingContentDecoder::new(&[ContentDecoder::Gzip, ContentDecoder::Deflate], None);
    for chunk in compressed.chunks(2) {
      decoder.feed(chunk).unwrap();
    }
    decoder.finish().unwrap();
    let mut out = Vec::new();
    let mut buf = [0u8; 8];
    loop {
      let n = decoder.fill(&mut buf);
      if n == 0 {
        break;
      }
      out.extend_from_slice(&buf[..n]);
    }
    assert_eq!(b"OK", out.as_slice());
    assert!(decoder.succeeded());
  }

  #[test]
  fn empty_input_skips_success() {
    let mut decoder = StreamingContentDecoder::new(&[ContentDecoder::Gzip], None);
    decoder.finish().unwrap();
    assert!(!decoder.succeeded());
  }

  #[test]
  fn rejects_raw_deflate() {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(b"OK").unwrap();
    let raw = encoder.finish().unwrap();
    let mut decoder = StreamingContentDecoder::new(&[ContentDecoder::Deflate], None);
    let err = decoder.feed(&raw).err().or_else(|| decoder.finish().err());
    assert!(err.is_some());
  }
}
