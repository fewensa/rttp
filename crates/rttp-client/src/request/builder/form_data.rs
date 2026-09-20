use crate::error;
use crate::request::builder::common::{DISPOSITION_END, DISPOSITION_PREFIX, HYPHENS};
use mime::Mime;
use rand::Rng;
use rttp_protocol::http1::is_qdtext;

pub struct FormDataWrap {
  pub disposition: Disposition,
  pub buffer: Vec<u8>,
}

pub struct Disposition {
  boundary: String,
}

impl Disposition {
  pub fn new() -> Self {
    let mut rng = rand::thread_rng();
    let boundary: String = std::iter::repeat(())
      .map(|()| rng.sample(rand::distributions::Alphanumeric) as char)
      .take(20)
      .collect();
    Self { boundary }
  }

  #[allow(dead_code)]
  pub fn boundary(&self) -> &String {
    &self.boundary
  }

  pub fn content_type(&self) -> String {
    format!("multipart/form-data; boundary={}{}", HYPHENS, self.boundary)
  }

  pub fn create_with_name(&self, name: &String) -> error::Result<String> {
    let name = quoted_string(name)?;
    Ok(format!(
      "{}{}{}{}Content-Disposition: form-data; name={}{}{}",
      DISPOSITION_PREFIX,
      HYPHENS,
      self.boundary,
      DISPOSITION_END,
      name,
      DISPOSITION_END,
      DISPOSITION_END
    ))
  }

  pub fn create_with_filename_and_content_type(
    &self,
    name: &String,
    filename: &String,
    mime: Mime,
  ) -> error::Result<String> {
    let name = quoted_string(name)?;
    let filename = quoted_string(filename)?;
    let mut disposition = format!(
      "{}{}{}{}Content-Disposition: form-data; name={}; filename={}{}",
      DISPOSITION_PREFIX, HYPHENS, self.boundary, DISPOSITION_END, name, filename, DISPOSITION_END
    );

    disposition.push_str(&format!("Content-Type: {}{}", mime, DISPOSITION_END));
    disposition.push_str(DISPOSITION_END);
    Ok(disposition)
  }

  pub fn end(&self) -> String {
    format!("{}--{}--{}", HYPHENS, self.boundary, DISPOSITION_END)
  }
}

/// Serialize a multipart `Content-Disposition` parameter as an RFC 7230 quoted-string.
///
/// CR, LF, NUL, DEL, and other controls other than HTAB are rejected. `"` and `\` are
/// escaped as quoted-pairs; remaining qdtext, including HTAB, SP, and obs-text, is kept.
fn quoted_string(value: &str) -> error::Result<String> {
  let mut encoded = Vec::with_capacity(value.len() + 2);
  encoded.push(b'"');
  for &byte in value.as_bytes() {
    if byte == b'"' || byte == b'\\' {
      encoded.push(b'\\');
      encoded.push(byte);
    } else if is_qdtext(byte) {
      encoded.push(byte);
    } else {
      return Err(error::builder_with_message(
        "Invalid multipart Content-Disposition quoted-string",
      ));
    }
  }
  encoded.push(b'"');
  String::from_utf8(encoded).map_err(error::builder)
}
