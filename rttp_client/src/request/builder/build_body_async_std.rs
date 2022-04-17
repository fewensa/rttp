#[cfg(feature = "async-std")]
use crate::error;
#[cfg(feature = "async-std")]
use crate::request::builder::common::RawBuilder;
#[cfg(feature = "async-std")]
use crate::request::RequestBody;
#[cfg(feature = "async-std")]
use crate::types::{FormDataType, RoUrl};

#[cfg(feature = "async-std")]
impl<'a> RawBuilder<'a> {
  pub async fn build_body_async_std(
    &mut self,
    rourl: &mut RoUrl,
  ) -> error::Result<Option<RequestBody>> {
    if let Some(body) = self.build_body_common(rourl)? {
      return Ok(Some(body));
    }

    let formdatas = self.request().formdatas();

    // form-data
    if !formdatas.is_empty() {
      return self.build_body_with_form_data_async_std().await;
    }

    return Ok(None);
  }

  async fn build_body_with_form_data_async_std(&mut self) -> error::Result<Option<RequestBody>> {
    let fdw = self.build_body_with_form_data_sync_common()?;
    let mut disposition = fdw.disposition;
    let mut buffer = fdw.buffer;

    let traditional = self.request().traditional();
    let formdatas = self.request().formdatas();
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
        let file_content = async_std::fs::read(&file).await.map_err(error::builder)?;
        buffer.extend(file_content);
      }
    }
    let end = disposition.end();
    buffer.extend_from_slice(end.as_bytes());

    let body = RequestBody::with_vec(buffer);
    Ok(Some(body))
  }
}
