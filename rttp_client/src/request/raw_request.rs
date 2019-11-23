use std::str::FromStr;

use mime::Mime;
use rand::Rng;
use url::Url;

use crate::error;
use crate::request::{Request, RequestBody};
use crate::types::{FormDataType, Header, RoUrl, ToUrl};

const HYPHENS: &'static str = "---------------------------";
const DISPOSITION_PREFIX: &'static str = "--";
const DISPOSITION_END: &'static str = "\r\n";


#[derive(Debug)]
pub struct RawRequest<'a> {
  origin: &'a mut Request,

  url: RoUrl,
  header: String,
  body: Option<RequestBody>,
}

impl<'a> RawRequest<'a> {
  pub fn new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    Standardization::new(request).standard()
  }

  pub fn origin(&self) -> &Request { &self.origin }
  pub fn url(&self) -> &RoUrl { &self.url }
  pub fn header(&self) -> &String { &self.header }
  pub fn body(&self) -> &Option<RequestBody> { &self.body }

  pub(crate) fn origin_mut(&mut self) -> &mut Request { &mut self.origin }
}


#[derive(Debug)]
pub struct Standardization<'a> {
  content_type: Option<Mime>,
  request: &'a mut Request,
}

// create
impl<'a> Standardization<'a> {
  pub fn new(request: &'a mut Request) -> Self {
    Self {
      content_type: None,
      request,
    }
  }

  pub fn standard(mut self) -> error::Result<RawRequest<'a>> {
    let mut rourl = self.request.url()
      .clone()
      .ok_or(error::none_url())?;

    self.rebuild_paras(&mut rourl);
    self.rebuild_url(&mut rourl);
    let body = self.build_body(&mut rourl)?;
    let header = self.build_header(&rourl, &body)?;
    Ok(RawRequest {
      origin: self.request,
      url: rourl,
      header,
      body,
    })
  }
}

// build body && rebuild para/url
impl<'a> Standardization<'a> {
  fn build_body(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let method = self.request.method();
    let raw = self.request.raw();
    let paras = self.request.paras();
    let binary = self.request.binary();
    let formdatas = self.request.formdatas();

    let has_multi_body_type = vec![raw.is_some(), !binary.is_empty(), !formdatas.is_empty()]
      .iter()
      .filter(|v| **v)
      .collect::<Vec<&bool>>()
      .len() > 1;
    if has_multi_body_type {
      return Err(error::builder_with_message("Bad request body, raw binary and form-data only support choose one"));
    }

    self.content_type = Some(mime::APPLICATION_WWW_FORM_URLENCODED);

    // if get request
    let is_get = method.eq_ignore_ascii_case("get");
    if is_get {
      for para in paras { rourl.para(para); }
    }

    // paras
    if !paras.is_empty() && raw.is_none() && binary.is_empty() && formdatas.is_empty() && !is_get {
      return self.build_body_with_form_urlencoded(rourl);
    }

    // form-data
    if !formdatas.is_empty() {
      return self.build_body_with_form_data(rourl);
    }

    // raw
    if raw.is_some() {
      self.content_type = Some(Mime::from_str(&self.request.header("content-type").map_or(mime::TEXT_PLAIN.to_string(), |v| v)[..])
        .map_err(error::builder)?);

      let body = raw.clone().map(|raw| RequestBody::with_text(raw));
      if !is_get && !paras.is_empty() {
        for para in paras { rourl.para(para); }
      }
      return Ok(body);
    }

    // binary
    if !binary.is_empty() {
      self.content_type = Some(Mime::from_str(&self.request.header("content-type").map_or(mime::APPLICATION_OCTET_STREAM.to_string(), |v| v)[..])
        .map_err(error::builder)?);

      let body = Some(RequestBody::with_vec(binary.clone()));
      if !is_get && !paras.is_empty() {
        for para in paras { rourl.para(para); }
      }
      return Ok(body);
    }

    // no body
    Ok(None)
  }

  fn rebuild_paras(&mut self, rourl: &mut RoUrl) {
    let traditional = self.request.traditional();
    rourl.traditional(traditional);


    let mut formdata_req = self.request.formdatas().clone();
    let mut paras_req = self.request.paras().clone();
    let mut paras_url = rourl.paras_get().clone();

    let mut names: Vec<(String, bool)> = Vec::with_capacity(paras_req.len() + paras_url.len());

    formdata_req.iter().for_each(|p| {
      if let Some(v) = names.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        names.push((p.name().clone(), false));
      }
    });
    paras_req.iter().for_each(|p| {
      if let Some(v) = names.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        names.push((p.name().clone(), false));
      }
    });
    paras_url.iter().for_each(|p| {
      if let Some(v) = names.iter_mut().find(|(key, _)| key == p.name()) {
        v.1 = true;
      } else {
        names.push((p.name().clone(), false));
      }
    });

    formdata_req.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = names.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    paras_req.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = names.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    paras_url.iter_mut().for_each(|para| {
      if let Some((_, is_array)) = names.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    });

    self.request.formdatas_set(formdata_req);
    self.request.paras_set(paras_req);
    rourl.paras_set(paras_url);
  }

  fn rebuild_url(&mut self, rourl: &mut RoUrl) {
    self.request.paths().iter().for_each(|path| {
      rourl.path(path);
    });
  }

  fn build_body_with_form_urlencoded(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let encode = self.request.encode();
    let traditional = self.request.traditional();
    let paras = self.request.paras();
    let len = paras.len();
    let mut body = String::new();
    for (i, para) in paras.iter().enumerate() {
      let name = if encode {
        percent_encoding::percent_encode(para.name().as_bytes(), percent_encoding::NON_ALPHANUMERIC).to_string()
      } else {
        para.name().to_string()
      };
      let value = if let Some(text) = para.value() {
        if encode {
          percent_encoding::percent_encode(text.as_bytes(), percent_encoding::NON_ALPHANUMERIC).to_string()
        } else {
          text.clone()
        }
      } else {
        Default::default()
      };
      body.push_str(&format!("{}{}={}", name,
                             if para.array() && !traditional { "[]" } else { "" },
                             value));
      if i + 1 < len {
        body.push_str("&");
      }
    }
    let req_body = RequestBody::with_text(body);
    Ok(Some(req_body))
  }

  fn build_body_with_form_data(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let traditional = self.request.traditional();
    let paras = self.request.paras();
    let formdatas = self.request.formdatas();
    let is_get = self.request.method().eq_ignore_ascii_case("get");

    let disposition = Disposition::new();
    let mut buffer = vec![];

    let content_type = disposition.content_type();
    self.content_type = Some(Mime::from_str(&content_type[..]).map_err(error::builder)?);

    if !is_get {
      for para in paras {
        let field_name = if para.array() && !traditional { format!("{}[]", para.name()) } else { para.name().to_string() };
        let value = if let Some(v) = para.value() { v.to_string() } else { "".to_string() };
        let item = format!("{}{}", disposition.create_with_name(&field_name), value);
        buffer.extend_from_slice(item.as_bytes());
        buffer.extend_from_slice(DISPOSITION_END.as_bytes());
      }
    }
    for formdata in formdatas {
      let field_name = if formdata.array() && !traditional { format!("{}[]", formdata.name()) } else { formdata.name().to_string() };
      match formdata.type_() {
        FormDataType::TEXT => {
          let value = if let Some(v) = formdata.text() { v.to_string() } else { "".to_string() };
          let item = format!("{}{}", disposition.create_with_name(&field_name), value);
          buffer.extend_from_slice(item.as_bytes());
        }
        FormDataType::FILE => {
          let file = formdata.file().clone().ok_or(error::builder_with_message(&format!("{} not have file", formdata.name())))?;
          let guess = mime_guess::from_path(&file);
          let filename = if let Some(fname) = formdata.filename() { fname.to_string() } else { "".to_string() };
          let item = disposition.create_with_filename_and_content_type(&field_name, &filename, guess.first_or_octet_stream());
          buffer.extend_from_slice(item.as_bytes());
          let file_content = std::fs::read(&file).map_err(error::builder)?;
          buffer.extend(file_content);
        }
        FormDataType::BINARY => {
          let filename = if let Some(fname) = formdata.filename() { fname.to_string() } else { "".to_string() };
          let octe_stream = Mime::from_str(&mime::APPLICATION_OCTET_STREAM.to_string()[..]).map_err(error::builder)?;
          let item = disposition.create_with_filename_and_content_type(&field_name, &filename, octe_stream);
          buffer.extend_from_slice(item.as_bytes());
          buffer.extend(formdata.binary());
        }
      }
      buffer.extend_from_slice(DISPOSITION_END.as_bytes());
    }
    let end = disposition.end();
    buffer.extend_from_slice(end.as_bytes());

//    println!("{}", String::from_utf8_lossy(buffer.clone().as_slice()));
    let body = RequestBody::with_vec(buffer);
    Ok(Some(body))
  }
}

// build header
impl<'a> Standardization<'a> {
  fn request_url(&self, url: &Url, full: bool) -> String {
    if full {
      return url.as_str().to_owned();
    }

    let mut result = format!("{}", url.path());
    if let Some(query) = url.query() {
      result.push_str(&format!("?{}", query));
    }
    if let Some(fragment) = url.fragment() {
      result.push_str(&format!("#{}", fragment));
    }
    result
  }

  fn build_header(&mut self, rourl: &RoUrl, body: &Option<RequestBody>) -> error::Result<String> {
    let url = rourl.to_url()?;

    let mut builder = String::new();
    let is_http = url.scheme() == "http";
    let request_url = self.request_url(&url, is_http);

    builder.push_str(&format!("{} {} HTTP/1.1{}", self.request.method().to_uppercase(), request_url, DISPOSITION_END));

    let mut found_host = false;
    let mut found_connection = false;
    let mut found_ua = false;
    let mut found_content_type = false;
    let mut found_content_length = false;

    for header in self.request.headers() {
      let name = header.name();
      let value = header.value().replace(DISPOSITION_END, "");

      if name.eq_ignore_ascii_case("host") { found_host = true; }
      if name.eq_ignore_ascii_case("connection") { found_connection = true; }
      if name.eq_ignore_ascii_case("user-agent") { found_ua = true; }

      if name.eq_ignore_ascii_case("content-type") {
        found_content_type = true;
        if !self.request.formdatas().is_empty() {
          continue;
        }
      }
      if name.eq_ignore_ascii_case("content-length") {
        found_content_length = true;
        continue;
      }

      builder.push_str(&format!("{}: {}{}", name, value, DISPOSITION_END));
    }

    // auto add host header
    if !found_host {
      let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
      let port: u16 = url.port().map_or_else(|| {
        match url.scheme() {
          "https" => Ok(443),
          "http" => Ok(80),
          _ => Err(error::url_bad_scheme(url.clone()))
        }
      }, |v| Ok(v))?;
      builder.push_str(&format!("Host: {}:{}{}", host, port, DISPOSITION_END));
      self.request.headers_mut().push(Header::new("Host", format!("{}:{}", host, port)));
    }

    // auto add connection header
    if !found_connection {
      let conn = format!("Connection: Close{}", DISPOSITION_END);
      builder.push_str(&conn);
      self.request.headers_mut().push(Header::new("Connection", "Close"));
    }

    // auto add user agent header
    if !found_ua {
      let ua = format!("Mozilla/5.0 rttp/{}", env!("CARGO_PKG_VERSION"));
      builder.push_str(&format!("User-Agent: {}{}", ua, DISPOSITION_END));
      self.request.headers_mut().push(Header::new("User-Agent", ua));
    }

    // auto add content type header
    // if it's form data request, replace header use this class generate header
    if self.request.formdatas().is_empty() {
      if !found_content_type {
        match &self.content_type {
          Some(ct) => {
            let ctstr = ct.to_string();
            builder.push_str(&format!("Content-Type: {}{}", ctstr, DISPOSITION_END));
            self.request.headers_mut().push(Header::new("Content-Type", ctstr));
          }
          None => {
            builder.push_str(&format!("Content-Type: {}{}", mime::APPLICATION_OCTET_STREAM.to_string(), DISPOSITION_END));
            self.request.headers_mut().push(Header::new("Content-Type", mime::APPLICATION_OCTET_STREAM.to_string()));
          }
        }
      }
    } else {
      let mut headers = self.request.headers().iter()
        .filter(|h| !h.name().eq_ignore_ascii_case("content-type"))
        .map(|v| v.clone())
        .collect::<Vec<Header>>();
      if let Some(ct) = &self.content_type {
        let cts = ct.to_string();
        builder.push_str(&format!("Content-Type: {}{}", cts, DISPOSITION_END));
        headers.push(Header::new("Content-Type", cts));
        self.request.headers_set(headers);
      }
    }

    // auto add content length header
    let len = if let Some(body) = body {
      body.len()
    } else {
      0
    };
    builder.push_str(&format!("Content-Length: {}{}", len, DISPOSITION_END));
    if found_content_length {
      let mut headers = self.request.headers().iter()
        .filter(|h| h.name().eq_ignore_ascii_case("content-length"))
        .map(|v| v.clone())
        .collect::<Vec<Header>>();
      headers.push(Header::new("Content-Length", len.to_string()));
      self.request.headers_set(headers);
    }

    builder.push_str(DISPOSITION_END);
    Ok(builder)
  }
}


struct Disposition {
  boundary: String
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

  pub fn boundary(&self) -> &String { &self.boundary }

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

  pub fn create_with_filename_and_content_type(&self, name: &String, filename: &String, mime: Mime) -> String {
    let mut disposition = format!(
      "{}{}{}{}Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"{}",
      DISPOSITION_PREFIX,
      HYPHENS,
      self.boundary,
      DISPOSITION_END,
      name,
      filename,
      DISPOSITION_END
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
    format!(
      "{}--{}--{}",
      HYPHENS,
      self.boundary,
      DISPOSITION_END
    )
  }
}

