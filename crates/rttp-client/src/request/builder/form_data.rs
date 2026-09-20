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

  pub fn create_with_name(&self, name: &str) -> error::Result<String> {
    Ok(format!(
      "{}{}{}{}Content-Disposition: form-data; name=\"{}\"{}{}",
      DISPOSITION_PREFIX,
      HYPHENS,
      self.boundary,
      DISPOSITION_END,
      quote_parameter(name)?,
      DISPOSITION_END,
      DISPOSITION_END
    ))
  }

  pub fn create_with_filename_and_content_type(
    &self,
    name: &str,
    filename: &str,
    mime: Mime,
  ) -> error::Result<String> {
    let mut disposition = format!(
      "{}{}{}{}Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}",
      DISPOSITION_PREFIX,
      HYPHENS,
      self.boundary,
      DISPOSITION_END,
      quote_parameter(name)?,
      quote_parameter(filename)?,
      DISPOSITION_END
    );

    disposition.push_str(&format!("Content-Type: {}{}", mime, DISPOSITION_END));
    disposition.push_str(DISPOSITION_END);
    Ok(disposition)
  }

  pub fn end(&self) -> String {
    format!("{}--{}--{}", HYPHENS, self.boundary, DISPOSITION_END)
  }
}

fn quote_parameter(value: &str) -> error::Result<String> {
  let mut escaped = Vec::with_capacity(value.len());
  for byte in value.as_bytes() {
    match *byte {
      b'"' | b'\\' => {
        escaped.push(b'\\');
        escaped.push(*byte);
      }
      byte if is_qdtext(byte) => escaped.push(byte),
      _ => {
        return Err(error::builder_with_message(
          "Invalid multipart Content-Disposition parameter",
        ));
      }
    }
  }
  String::from_utf8(escaped).map_err(error::builder)
}
