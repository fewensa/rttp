use crate::request::builder::common::{DISPOSITION_END, DISPOSITION_PREFIX, HYPHENS};
use mime::Mime;
use rand::Rng;

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
      .map(|()| rng.sample(rand::distributions::Alphanumeric))
      .take(20)
      .collect();
    Self { boundary }
  }

  pub fn boundary(&self) -> &String {
    &self.boundary
  }

  pub fn content_type(&self) -> String {
    format!("multipart/form-data; boundary={}{}", HYPHENS, self.boundary)
  }

  pub fn create_with_name(&self, name: &String) -> String {
    format!(
      "{}{}{}{}Content-Disposition: form-data; name=\"{}\"{}{}",
      DISPOSITION_PREFIX,
      HYPHENS,
      self.boundary,
      DISPOSITION_END,
      name,
      DISPOSITION_END,
      DISPOSITION_END
    )
  }

  pub fn create_with_filename_and_content_type(
    &self,
    name: &String,
    filename: &String,
    mime: Mime,
  ) -> String {
    let mut disposition = format!(
      "{}{}{}{}Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}",
      DISPOSITION_PREFIX, HYPHENS, self.boundary, DISPOSITION_END, name, filename, DISPOSITION_END
    );

    disposition.push_str(&format!(
      "Content-Type: {}{}",
      mime.to_string(),
      DISPOSITION_END
    ));
    disposition.push_str(DISPOSITION_END);
    disposition
  }

  pub fn end(&self) -> String {
    format!("{}--{}--{}", HYPHENS, self.boundary, DISPOSITION_END)
  }
}
