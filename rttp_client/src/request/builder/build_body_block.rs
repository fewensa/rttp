use std::str::FromStr;

use mime::Mime;

use crate::error;
use crate::request::builder::common::{RawBuilder, DISPOSITION_END};
use crate::request::builder::form_data::{Disposition, FormDataWrap};
use crate::request::RequestBody;
use crate::types::{FormDataType, RoUrl};

impl<'a> RawBuilder<'a> {
  pub fn build_body_common(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let method = self.request.method();
    let raw = self.request.raw();
    let paras = self.request.paras();
    let binary = self.request.binary();
    let formdatas = self.request.formdatas();

    let have_multi_body_type = raw.is_some() && !binary.is_empty() && !formdatas.is_empty();
    if have_multi_body_type {
      return Err(error::builder_with_message(
        "Bad request body, `raw`, `binary` and `form-data` only support choose one",
      ));
    }

    // if get request
    let is_get = method.eq_ignore_ascii_case("get");
    if is_get {
      for para in paras {
        rourl.para(para);
      }
    }
    self.content_type = Some(mime::APPLICATION_WWW_FORM_URLENCODED);

    // paras
    if !paras.is_empty() && raw.is_none() && binary.is_empty() && formdatas.is_empty() && !is_get {
      return self.build_body_with_form_urlencoded();
    }

    // raw
    if raw.is_some() {
      self.content_type = Some(
        Mime::from_str(
          &self
            .request
            .header("content-type")
            .map_or(mime::TEXT_PLAIN.to_string(), |v| v)[..],
        )
        .map_err(error::builder)?,
      );

      let body = raw.clone().map(|raw| RequestBody::with_text(raw));
      if !paras.is_empty() {
        for para in paras {
          rourl.para(para);
        }
      }
      return Ok(body);
    }

    // binary
    if !binary.is_empty() {
      self.content_type = Some(
        Mime::from_str(
          &self
            .request
            .header("content-type")
            .map_or(mime::APPLICATION_OCTET_STREAM.to_string(), |v| v)[..],
        )
        .map_err(error::builder)?,
      );

      let body = Some(RequestBody::with_vec(binary.clone()));
      if !paras.is_empty() {
        for para in paras {
          rourl.para(para);
        }
      }
      return Ok(body);
    }

    // no body (Not include form data)
    Ok(None)
  }
}

// build sync body
impl<'a> RawBuilder<'a> {
  pub fn build_body_block(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    if let Some(body) = self.build_body_common(rourl)? {
      return Ok(Some(body));
    }

    let formdatas = self.request.formdatas();

    // form-data
    if !formdatas.is_empty() {
      return self.build_body_with_form_data_block();
    }

    return Ok(None);
  }

  fn build_body_with_form_data_block(&mut self) -> error::Result<Option<RequestBody>> {
    let fdw = self.build_body_with_form_data_sync_common()?;
    let mut disposition = fdw.disposition;
    let mut buffer = fdw.buffer;

    let traditional = self.request.traditional();
    let formdatas = self.request.formdatas();
    for formdata in formdatas {
      let field_name = if formdata.array() && !traditional {
        format!("{}[]", formdata.name())
      } else {
        formdata.name().to_string()
      };
      if formdata.type_() == &FormDataType::FILE {
        let file = formdata
          .file()
          .clone()
          .ok_or(error::builder_with_message(&format!(
            "Missing file path for field: {}",
            formdata.name()
          )))?;
        let guess = mime_guess::from_path(&file);
        let file_name = formdata.filename().clone().unwrap_or_default();
        let item = disposition.create_with_filename_and_content_type(
          &field_name,
          &file_name,
          guess.first_or_octet_stream(),
        );
        buffer.extend_from_slice(item.as_bytes());
        let file_content = std::fs::read(&file).map_err(error::builder)?;
        buffer.extend(file_content);
      }
    }
    let end = disposition.end();
    buffer.extend_from_slice(end.as_bytes());

    let body = RequestBody::with_vec(buffer);
    Ok(Some(body))
  }

  pub fn build_body_with_form_data_sync_common(&mut self) -> error::Result<FormDataWrap> {
    let traditional = self.request.traditional();
    let is_get = self.request.method().eq_ignore_ascii_case("get");

    let disposition = Disposition::new();
    let mut buffer = vec![];

    let content_type = disposition.content_type();
    self.content_type = Some(Mime::from_str(&content_type[..]).map_err(error::builder)?);

    if !is_get {
      let paras = self.request.paras();
      for para in paras {
        let field_name = if para.array() && !traditional {
          format!("{}[]", para.name())
        } else {
          para.name().to_string()
        };
        let value = if let Some(v) = para.value() {
          v.clone()
        } else {
          Default::default()
        };
        let item = format!("{}{}", disposition.create_with_name(&field_name), value);
        buffer.extend_from_slice(item.as_bytes());
        buffer.extend_from_slice(DISPOSITION_END.as_bytes());
      }
    }

    let formdatas = self.request.formdatas();
    for formdata in formdatas {
      let field_name = if formdata.array() && !traditional {
        format!("{}[]", formdata.name())
      } else {
        formdata.name().clone()
      };
      match formdata.type_() {
        FormDataType::TEXT => {
          let value = if let Some(v) = formdata.text() {
            v.to_string()
          } else {
            Default::default()
          };
          let item = format!("{}{}", disposition.create_with_name(&field_name), value);
          buffer.extend_from_slice(item.as_bytes());
        }
        FormDataType::BINARY => {
          let file_name = formdata.filename().clone().unwrap_or_default();
          let octe_stream = Mime::from_str(&mime::APPLICATION_OCTET_STREAM.to_string()[..])
            .map_err(error::builder)?;
          let item =
            disposition.create_with_filename_and_content_type(&field_name, &file_name, octe_stream);
          buffer.extend_from_slice(item.as_bytes());
          buffer.extend(formdata.binary());
        }
        FormDataType::FILE => continue,
      }
      buffer.extend_from_slice(DISPOSITION_END.as_bytes());
    }

    Ok(FormDataWrap {
      disposition,
      buffer,
    })
  }
}

// build body common
impl<'a> RawBuilder<'a> {
  fn build_body_with_form_urlencoded(&mut self) -> error::Result<Option<RequestBody>> {
    let encode = self.request.encode();
    let traditional = self.request.traditional();
    let paras = self.request.paras();
    let len = paras.len();
    let mut body = String::new();
    for (i, para) in paras.iter().enumerate() {
      let name = if encode {
        percent_encoding::percent_encode(para.name().as_bytes(), percent_encoding::NON_ALPHANUMERIC)
          .to_string()
      } else {
        para.name().to_string()
      };
      let value = if let Some(text) = para.value() {
        if encode {
          percent_encoding::percent_encode(text.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
            .to_string()
        } else {
          text.clone()
        }
      } else {
        Default::default()
      };
      body.push_str(&format!(
        "{}{}={}",
        name,
        if para.array() && !traditional {
          "[]"
        } else {
          ""
        },
        value
      ));
      if i + 1 < len {
        body.push_str("&");
      }
    }
    let req_body = RequestBody::with_text(body);
    Ok(Some(req_body))
  }
}
