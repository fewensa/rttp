use url::Url;

use crate::error;
use crate::request::{Request, RequestBody};
use crate::types::{Header, RoUrl, ToUrl, Para};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Connection {
  request: Request
}

impl Connection {
  pub fn new(request: Request) -> Self {
    Self { request }
  }

  pub fn call(mut self) -> error::Result<()> {
    let mut rourl = self.request.url()
      .clone()
      .ok_or(error::none_url())?;

//    match url.scheme() {
//      "http" => self.call_http(&url),
//      "https" => self.call_https(&url),
//      _ => Err(error::url_bad_scheme(url.to_url()?))
//    }

    self.rebuild_paras(&mut rourl);
    let body = self.build_body(&mut rourl)?;
    let header = self.build_header(&rourl)?;
    println!("{}", header);
    println!("{:?}", body);
    Ok(())
  }
}


impl Connection {
  fn build_body(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let method = self.request.method();
    let raw = self.request.raw();
    let paras = self.request.paras();
    let binary = self.request.binary();

    if raw.is_some() && !binary.is_empty() {
      return Err(error::builder_with_message("Bad request body, raw and binary only support choose one"));
    }

    // if get request
    let is_get = method.eq_ignore_ascii_case("get");
    if is_get {
      for para in paras { rourl.para(para); }
    }

    // paras
    if !paras.is_empty() && raw.is_none() && binary.is_empty() && !is_get {
      // todo use form data
//      let has_file = self.request.paras().iter().find(|&para| para.is_file()).is_some();
//      return if has_file {
//        self.build_body_with_form_data(rourl)
//      } else {
//        self.build_body_with_form_urlencoded(rourl)
//      };
    }

    // raw
    if raw.is_some() {
      let body = raw.clone().map(|raw| RequestBody::with_text(raw));
      if !is_get && !paras.is_empty() {
        for para in paras { rourl.para(para); }
      }
      return Ok(body);
    }

    // binary
    if !binary.is_empty() {
      let body = Some(RequestBody::with_vec(binary.clone()));
      if !is_get && !paras.is_empty() {
        for para in paras { rourl.para(para); }
      }
      return Ok(body);
    }

    // no body
    Ok(None)
  }

  fn build_body_with_form_urlencoded(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
    let traditional = self.request.traditional();
    let paras = self.request.paras();
    let len = paras.len();
    let mut body = String::new();
    for (i, para) in paras.iter().enumerate() {
      let name = percent_encoding::percent_encode(para.name().as_bytes(), percent_encoding::NON_ALPHANUMERIC);
      let value = if let Some(text) = para.value() {
        let value = percent_encoding::percent_encode(text.as_bytes(), percent_encoding::NON_ALPHANUMERIC);
        Some(value)
      } else {
        None
      };
      body.push_str(&format!("{}{}={}", name,
                             if para.array() && !traditional { "[]" } else { "" },
                             value.map_or("".to_string(), |v| v.to_string())));
      if i + 1 < len {
        body.push_str("&");
      }
    }
    let req_body = RequestBody::with_text(body);
    Ok(Some(req_body))
  }

  fn build_body_with_form_data(&mut self, rourl: &mut RoUrl) -> error::Result<Option<RequestBody>> {
//    let buffer = vec![];
    Ok(None)
  }


  fn rebuild_paras(&mut self, rourl: &mut RoUrl) {
    let traditional = self.request.traditional();
    rourl.traditional(traditional);


    let mut paras_req = self.request.paras_mut();
    let mut paras_url = rourl.paras_mut();

    let mut names: Vec<(String, bool)> = Vec::with_capacity(paras_req.len() + paras_url.len());

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

    for para in paras_req {
      if let Some((_, is_array)) = names.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    }

    for para in paras_url {
      if let Some((_, is_array)) = names.iter().find(|(key, _)| key == para.name()) {
        *para.array_mut() = *is_array;
      }
    }

  }
}

impl Connection {
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

  fn build_header(&mut self, rourl: &RoUrl) -> error::Result<String> {
    let url = rourl.to_url()?;

    let mut builder = String::new();
    let request_url = self.request_url(&url, true);

    builder.push_str(&format!("{} {} HTTP/1.1\r\n", self.request.method().to_uppercase(), request_url));

    let mut found_host = false;
    let mut found_connection = false;
    let mut found_ua = false;

    for header in self.request.headers() {
      let name = header.name();
      let value = header.value().replace("\r\n", "");

      if name.eq_ignore_ascii_case("host") { found_host = true; }
      if name.eq_ignore_ascii_case("connection") { found_connection = true; }
      if name.eq_ignore_ascii_case("user-agent") { found_ua = true; }

      builder.push_str(&format!("{}: {}\r\n", name, value));
    }

    if !found_host {
      let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
      let port: u16 = url.port().map_or_else(|| {
        match url.scheme() {
          "https" => Ok(443),
          "http" => Ok(80),
          _ => Err(error::url_bad_scheme(url.clone()))
        }
      }, |v| Ok(v))?;
      builder.push_str(&format!("Host: {}:{}\r\n", host, port));
      self.request.headers_mut().push(Header::new("Host", format!("{}:{}", host, port)));
    }
    if !found_connection {
      let conn = format!("Connection: Close\r\n");
      builder.push_str(&conn);
      self.request.headers_mut().push(Header::new("Connection", "Close"));
    }
    if !found_ua {
      let ua = format!("Mozilla/5.0 rttp/{}", env!("CARGO_PKG_VERSION"));
      builder.push_str(&format!("User-Agent: {}", ua));
      self.request.headers_mut().push(Header::new("User-Agent", ua));
    }

    builder.push_str("\r\n");
    Ok(builder)
  }
}

