#[cfg(feature = "async")]
use crate::connection::{AsyncConnection, AsyncStreamingRequestBody};
use crate::connection::{BlockConnection, HandoffConnection, StreamingRequestBody};
use crate::request::{RawRequest, Request};
use crate::response::Response;
use crate::types::{Auth, Header, IntoHeader, IntoPara, Proxy, ToFormData, ToRoUrl};
use crate::{error, Config, H2cClientPolicy};
#[cfg(feature = "async")]
use futures::io::AsyncRead;
use rttp_protocol::accept_charset::AcceptCharset;
use rttp_protocol::accept_encoding::AcceptEncoding;
use rttp_protocol::accept_language::{AcceptLanguage, MAX_ACCEPT_LANGUAGE_VALUE_BYTES};
use rttp_protocol::access_control_request_headers::AccessControlRequestHeaders;
use rttp_protocol::access_control_request_method::AccessControlRequestMethod;
use rttp_protocol::access_control_request_private_network::AccessControlRequestPrivateNetwork;
use rttp_protocol::authorization::Authorization;
use rttp_protocol::baggage::Baggage;
use rttp_protocol::cdn_loop::{CdnLoop, MAX_CDN_LOOP_VALUE_BYTES};
use rttp_protocol::depth::Depth;
use rttp_protocol::expect::Expect;
use rttp_protocol::fetch_metadata::{
  SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};
use rttp_protocol::forwarded::{Forwarded, MAX_FORWARDED_VALUE_BYTES};
use rttp_protocol::idempotency_key::IdempotencyKey;
use rttp_protocol::if_modified_since::IfModifiedSince;
use rttp_protocol::if_unmodified_since::IfUnmodifiedSince;
use rttp_protocol::max_forwards::MaxForwards;
use rttp_protocol::origin::Origin;
use rttp_protocol::pragma::Pragma;
use rttp_protocol::priority::Priority;
use rttp_protocol::save_data::SaveData;
use rttp_protocol::sec_gpc::SecGpc;
use rttp_protocol::sec_websocket_key::SecWebSocketKey;
use rttp_protocol::signature::Signature;
use rttp_protocol::signature_input::SignatureInput;
use rttp_protocol::te::{Te, MAX_TE_CODINGS, MAX_TE_VALUE_BYTES};
use rttp_protocol::timeout::Timeout;
use rttp_protocol::trace_context::{TraceParent, TraceState};
use rttp_protocol::trailer::Trailer;
use rttp_protocol::upgrade::Upgrade;
use rttp_protocol::upgrade_insecure_requests::UpgradeInsecureRequests;
use std::io;

#[derive(Debug)]
pub struct HttpClient {
  request: Request,
}

impl Default for HttpClient {
  fn default() -> Self {
    Self {
      request: Request::new(),
    }
  }
}

impl HttpClient {
  /// Create a `HttpClient` object.
  /// # Examples
  /// ```rust
  /// use rttp_client::HttpClient;
  /// let client = HttpClient::new();
  /// ```
  pub fn new() -> Self {
    Default::default()
  }

  /// Reset this request, The request only use once, This function can reset request.
  pub fn reset(&mut self) -> &mut Self {
    self.request = Request::new();
    self
  }

  /// Set get request
  pub fn get(&mut self) -> &mut Self {
    self.method("GET")
  }

  /// Set post request
  pub fn post(&mut self) -> &mut Self {
    self.method("POST")
  }

  /// Set put request
  pub fn put(&mut self) -> &mut Self {
    self.method("PUT")
  }

  /// Set delete request
  pub fn delete(&mut self) -> &mut Self {
    self.method("DELETE")
  }

  /// Set options request
  pub fn options(&mut self) -> &mut Self {
    self.method("OPTIONS")
  }

  /// Set head request
  pub fn head(&mut self) -> &mut Self {
    self.method("HEAD")
  }

  /// Set trace request
  pub fn trace(&mut self) -> &mut Self {
    self.method("TRACE")
  }

  /// Set request by method
  pub fn method<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.request.method_set(method);
    self
  }

  /// Set request url.
  pub fn url<U: ToRoUrl>(&mut self, url: U) -> &mut Self {
    self.request.url_set(url.to_rourl());
    self
  }

  /// Set request config
  pub fn config<C: AsRef<Config>>(&mut self, config: C) -> &mut Self {
    self.request.config_set(config);
    self
  }

  /// Configure local settings for the bounded prior-knowledge h2c client path.
  ///
  /// This is honored only by `emit_http2_prior_knowledge` and does not enable
  /// pooling, retries, server push, or multiplexing. Invalid HTTP/2 settings
  /// are rejected before the client opens its TCP socket.
  pub fn h2c_policy(&mut self, policy: H2cClientPolicy) -> &mut Self {
    self.request.config_mut().h2c_policy_set(policy);
    self
  }

  /// Whether traditional request, if false, the same para name will be add []
  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.request.traditional_set(traditional);
    self
  }

  /// Add url path
  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.request.paths_mut().push(path.as_ref().into());
    self
  }

  /// Whether encode para
  pub fn encode(&mut self, encode: bool) -> &mut Self {
    self.request.encode_set(encode);
    self
  }

  /// Set proxy request
  pub fn proxy<P: AsRef<Proxy>>(&mut self, proxy: P) -> &mut Self {
    self.request.proxy_set(proxy.as_ref().clone());
    self
  }

  /// Use RFC 8441 extended CONNECT on the bounded prior-knowledge h2c path.
  ///
  /// This is only honored by `emit_http2_prior_knowledge` with the `http2`
  /// feature enabled. The client opens a direct `socket2` h2c TCP connection,
  /// advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1`, emits `:method
  /// CONNECT`, and includes the configured `:protocol` pseudo-header. It
  /// returns the peer's HTTP/2 response through the normal `Response` API; it
  /// does not hand an upgraded socket to the caller.
  #[cfg(feature = "http2")]
  pub fn http2_extended_connect<S: AsRef<str>>(&mut self, protocol: S) -> &mut Self {
    self
      .request
      .http2_extended_connect_protocol_set(protocol.as_ref());
    self
  }

  /// Set HTTP authentication. Supports Basic Auth and Bearer Token.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rttp_client::HttpClient;
  /// use rttp_client::types::Auth;
  ///
  /// let mut client = HttpClient::new();
  /// client.auth(Auth::basic("user", "secret"));
  /// client.auth(Auth::bearer("my-token"));
  /// ```
  pub fn auth<A: AsRef<Auth>>(&mut self, auth: A) -> &mut Self {
    self.header(Header::new("Authorization", auth.as_ref().header_value()))
  }

  /// Set bounded `Authorization` request metadata from an authentication
  /// scheme and opaque credentials.
  ///
  /// The scheme must be an HTTP token and credentials must be a non-empty,
  /// bounded header value. RTTP does not interpret credentials or implement
  /// scheme-specific authentication behavior. Use [`Self::header`] when an
  /// application needs to send a custom Authorization syntax.
  pub fn authorization<S: AsRef<str>, C: AsRef<str>>(
    &mut self,
    scheme: S,
    credentials: C,
  ) -> error::Result<&mut Self> {
    let authorization = Authorization::new(scheme, credentials)
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Authorization", authorization.header_value())))
  }

  ///  Add request header
  pub fn header<P: IntoHeader>(&mut self, header: P) -> &mut Self {
    let headers = self.request.headers_mut();
    for h in header.into_headers() {
      let exit = headers
        .iter_mut()
        .find(|d| d.name().eq_ignore_ascii_case(h.name()));

      if let Some(eh) = exit {
        if h.name().eq_ignore_ascii_case("cookie") {
          let new_cookie_value = format!("{};{}", eh.value(), h.value());
          eh.replace(Header::new("Cookie", new_cookie_value));
          continue;
        }

        eh.replace(h);
        continue;
      }
      headers.push(h);
    }
    self
  }

  /// Add a request trailer field for chunked streaming uploads.
  pub fn trailer<P: IntoHeader>(&mut self, trailer: P) -> error::Result<&mut Self> {
    let trailers = trailer.into_headers();
    for h in &trailers {
      validate_request_trailer_header(h.name(), h.value())?;
    }
    for h in trailers {
      let trailers = self.request.trailers_mut();
      if let Some(existing) = trailers
        .iter_mut()
        .find(|d| d.name().eq_ignore_ascii_case(h.name()))
      {
        existing.replace(h);
      } else {
        trailers.push(h);
      }
    }
    Ok(self)
  }

  /// Declare bounded `Trailer` field-name metadata without enabling streaming
  /// request trailers or adding `TE` capability metadata.
  pub fn trailer_header<I, S>(&mut self, field_names: I) -> error::Result<&mut Self>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let fields: Vec<String> = field_names
      .into_iter()
      .map(|field_name| field_name.as_ref().to_string())
      .collect();
    let trailer = Trailer::parse_values(fields.iter().map(String::as_str))
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Trailer", trailer.header_value())))
  }

  /// Set bounded `Upgrade` protocol metadata without changing connection
  /// handoff behavior or adding `Connection: Upgrade`.
  pub fn upgrade_protocols<I, S>(&mut self, protocols: I) -> error::Result<&mut Self>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let protocols: Vec<String> = protocols
      .into_iter()
      .map(|protocol| protocol.as_ref().to_string())
      .collect();
    let upgrade = Upgrade::parse_values(protocols.iter().map(String::as_str))
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Upgrade", upgrade.header_value())))
  }

  /// Add request cookie
  pub fn cookie<S: AsRef<str>>(&mut self, cookie: S) -> &mut Self {
    self.header(("Cookie", cookie.as_ref()))
  }

  /// Set bounded `Accept-Language` request metadata.
  ///
  /// Each supplied item is a language range, optionally followed by a `q`
  /// weight such as `fr-CA; q=0.8`. Validation is delegated to the shared
  /// protocol-owned `AcceptLanguage` type. This validates metadata only; it
  /// does not perform locale matching or choose a response language.
  pub fn accept_language<I, L>(&mut self, ranges: I) -> error::Result<&mut Self>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    let value = build_accept_language_value(ranges)?;
    Ok(self.header(Header::new("Accept-Language", value)))
  }

  /// Emit the standardized `Expect: 100-continue` request metadata.
  ///
  /// This is metadata only: the client still writes the request body normally
  /// and does not wait for an interim response before sending it.
  pub fn expect_continue(&mut self) -> &mut Self {
    self.header(Header::new(
      "Expect",
      Expect::expect_continue().header_value(),
    ))
  }

  /// Set bounded `Sec-Fetch-Site` request metadata without applying browser policy.
  pub fn sec_fetch_site(&mut self, site: SecFetchSite) -> &mut Self {
    self.header(("Sec-Fetch-Site", site.header_value()))
  }

  /// Set bounded `Sec-Fetch-Mode` request metadata without applying browser policy.
  pub fn sec_fetch_mode(&mut self, mode: SecFetchMode) -> &mut Self {
    self.header(("Sec-Fetch-Mode", mode.header_value()))
  }

  /// Set bounded `Sec-Fetch-Dest` request metadata without applying browser policy.
  pub fn sec_fetch_dest(&mut self, dest: SecFetchDest) -> &mut Self {
    self.header(("Sec-Fetch-Dest", dest.header_value()))
  }

  /// Set the `Sec-Fetch-User: ?1` request metadata without applying browser policy.
  pub fn sec_fetch_user(&mut self) -> &mut Self {
    self.header(("Sec-Fetch-User", SecFetchUser.header_value()))
  }

  /// Set bounded `Sec-Purpose` request metadata without applying browser policy,
  /// starting prefetches, or changing cache behavior.
  pub fn sec_purpose(&mut self, purpose: &SecPurpose) -> &mut Self {
    self.header(Header::new("Sec-Purpose", purpose.header_value()))
  }

  /// Set bounded `Origin` request metadata for preflight composition.
  ///
  /// The value must be `null` or an `http`/`https` tuple origin without a
  /// path, query, fragment, or userinfo. This declares request metadata only;
  /// it does not decide whether a preflight is needed or apply CORS policy.
  pub fn origin<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let origin = Origin::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new("Origin", origin.header_value())))
  }

  /// Set bounded `Access-Control-Request-Method` request metadata.
  ///
  /// The value must be a single HTTP method token; `*` and comma-separated
  /// lists are rejected. This declares request metadata only; it does not
  /// decide whether a preflight is needed or apply CORS policy.
  pub fn access_control_request_method<S: AsRef<str>>(
    &mut self,
    value: S,
  ) -> error::Result<&mut Self> {
    let method = AccessControlRequestMethod::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new(
      "Access-Control-Request-Method",
      method.header_value(),
    )))
  }

  /// Set bounded RFC 9421 `Signature` request metadata.
  ///
  /// This validates and replaces one `Signature` field. It does not sign,
  /// verify, or look up keys.
  pub fn signature<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let signature = Signature::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new("Signature", signature.header_value())))
  }

  /// Set bounded RFC 9421 `Signature-Input` request metadata.
  ///
  /// This validates and replaces one `Signature-Input` field. It does not
  /// sign, verify, look up keys, or apply cryptographic policy.
  pub fn signature_input<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let signature_input = SignatureInput::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new(
      "Signature-Input",
      signature_input.header_value(),
    )))
  }

  /// Set bounded `Access-Control-Request-Headers` request metadata.
  ///
  /// Field names are normalized to lowercase and duplicates are rejected
  /// before the header is emitted. This declares request metadata only; it
  /// does not decide whether a preflight is needed or apply CORS policy.
  pub fn access_control_request_headers<I, S>(&mut self, field_names: I) -> error::Result<&mut Self>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let field_names: Vec<String> = field_names
      .into_iter()
      .map(|field_name| field_name.as_ref().to_string())
      .collect();
    let request_headers =
      AccessControlRequestHeaders::parse_values(field_names.iter().map(String::as_str))
        .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "Access-Control-Request-Headers",
      request_headers.header_value(),
    )))
  }

  /// Set `Access-Control-Request-Private-Network: true` request metadata.
  ///
  /// This declares the valid private-network preflight request form only; it
  /// does not decide whether a preflight is needed or apply browser policy.
  pub fn access_control_request_private_network(&mut self) -> error::Result<&mut Self> {
    let request_private_network = AccessControlRequestPrivateNetwork::parse("true")
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "Access-Control-Request-Private-Network",
      request_private_network.header_value(),
    )))
  }

  /// Set `Save-Data: on` request metadata.
  ///
  /// This declares the valid reduced-data request form only; it does not
  /// select a representation or apply browser data-saver policy.
  pub fn save_data(&mut self) -> error::Result<&mut Self> {
    let save_data =
      SaveData::parse("on").map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Save-Data", save_data.header_value())))
  }

  /// Set `Sec-GPC: 1` request metadata.
  ///
  /// This declares the valid Global Privacy Control request signal only; it
  /// does not infer consent, tracking, legal, or serving policy.
  pub fn sec_gpc(&mut self) -> error::Result<&mut Self> {
    let sec_gpc =
      SecGpc::parse("1").map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Sec-GPC", sec_gpc.header_value())))
  }

  /// Set `Upgrade-Insecure-Requests: 1` request metadata.
  ///
  /// This declares the valid upgrade-insecure-requests form only; it does not
  /// rewrite URLs, redirect requests, or enforce Content-Security-Policy.
  pub fn upgrade_insecure_requests(&mut self) -> error::Result<&mut Self> {
    let metadata = UpgradeInsecureRequests::parse("1")
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "Upgrade-Insecure-Requests",
      metadata.header_value(),
    )))
  }

  /// Set bounded `Pragma` request metadata from an RFC 9111 directive list.
  ///
  /// The value is validated through the shared protocol `Pragma` type: each
  /// directive name must be an HTTP token, optional values must be tokens or
  /// quoted-strings, `no-cache` must appear without a value, duplicate
  /// directive names are rejected case-insensitively, and combined fields are
  /// bounded to 256 directives with 64 KiB per field and per value. Any
  /// already-attached `Pragma` fields are combined in wire order and replaced
  /// by one normalized field. This declares request metadata only; it does
  /// not translate `Pragma` into `Cache-Control`, store cache entries, or
  /// apply cache or intermediary policy. Use `header` directly for unusual
  /// values.
  pub fn pragma<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let mut values: Vec<String> = self
      .request
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case("Pragma"))
      .map(|header| header.value().clone())
      .collect();
    values.push(value.as_ref().to_string());
    let pragma = Pragma::parse_values(values.iter().map(String::as_str))
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Pragma", pragma.header_value())))
  }

  /// Set `Pragma: no-cache` request metadata.
  ///
  /// This is a convenience for [`Self::pragma`] with the defined valueless
  /// `no-cache` directive. It declares request metadata only; it does not
  /// translate `Pragma` into `Cache-Control` or apply cache policy.
  pub fn pragma_no_cache(&mut self) -> error::Result<&mut Self> {
    self.pragma("no-cache")
  }

  /// Append a validated `Accept` media range with its supplied quality value.
  ///
  /// This declares request metadata only; it does not select a response
  /// representation.
  pub fn accept<S: AsRef<str>>(&mut self, media_range: S) -> error::Result<&mut Self> {
    self.accept_member(media_range.as_ref(), None)
  }

  /// Append a validated `Accept` media range with an HTTP q-value.
  ///
  /// The q-value must be between `0` and `1` with at most three fractional
  /// digits. This declares request metadata only; it does not select a
  /// response representation.
  pub fn accept_with_q<M: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    media_range: M,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.accept_member(media_range.as_ref(), Some(qvalue.as_ref()))
  }

  /// Append `*/*` to `Accept`.
  pub fn accept_any(&mut self) -> error::Result<&mut Self> {
    self.accept("*/*")
  }

  /// Append `*/*` to `Accept` with an HTTP q-value.
  pub fn accept_any_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_with_q("*/*", qvalue)
  }

  /// Append `application/json` to `Accept`.
  pub fn accept_json(&mut self) -> error::Result<&mut Self> {
    self.accept("application/json")
  }

  /// Append `application/json` to `Accept` with an HTTP q-value.
  pub fn accept_json_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_with_q("application/json", qvalue)
  }

  /// Append `text/html` to `Accept`.
  pub fn accept_html(&mut self) -> error::Result<&mut Self> {
    self.accept("text/html")
  }

  /// Append `text/html` to `Accept` with an HTTP q-value.
  pub fn accept_html_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_with_q("text/html", qvalue)
  }

  /// Append `application/xml` to `Accept`.
  pub fn accept_xml(&mut self) -> error::Result<&mut Self> {
    self.accept("application/xml")
  }

  /// Append `application/xml` to `Accept` with an HTTP q-value.
  pub fn accept_xml_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_with_q("application/xml", qvalue)
  }

  /// Append `text/plain` to `Accept`.
  pub fn accept_plain_text(&mut self) -> error::Result<&mut Self> {
    self.accept("text/plain")
  }

  /// Append `text/plain` to `Accept` with an HTTP q-value.
  pub fn accept_plain_text_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_with_q("text/plain", qvalue)
  }

  /// Append `no-cache` to bounded `Cache-Control` request metadata.
  ///
  /// This declares request metadata only; it does not enable cache storage or
  /// automatic revalidation.
  pub fn cache_control_no_cache(&mut self) -> error::Result<&mut Self> {
    self.cache_control_member("no-cache", None)
  }

  /// Append `no-store` to bounded `Cache-Control` request metadata.
  ///
  /// This declares request metadata only; it does not enable cache storage or
  /// automatic revalidation.
  pub fn cache_control_no_store(&mut self) -> error::Result<&mut Self> {
    self.cache_control_member("no-store", None)
  }

  /// Append `max-age=<seconds>` to bounded `Cache-Control` request metadata.
  pub fn cache_control_max_age(&mut self, seconds: u64) -> error::Result<&mut Self> {
    let value = seconds.to_string();
    self.cache_control_member("max-age", Some(&value))
  }

  /// Append a valueless Cache-Control extension directive.
  pub fn cache_control_extension<N: AsRef<str>>(&mut self, name: N) -> error::Result<&mut Self> {
    self.cache_control_extension_member(name.as_ref(), None)
  }

  /// Append a token-valued Cache-Control extension directive.
  ///
  /// Both the extension name and value must be HTTP tokens. Use [`Self::header`]
  /// directly for extension values that require quoted-string syntax.
  pub fn cache_control_extension_with_value<N: AsRef<str>, V: AsRef<str>>(
    &mut self,
    name: N,
    value: V,
  ) -> error::Result<&mut Self> {
    self.cache_control_extension_member(name.as_ref(), Some(value.as_ref()))
  }

  /// Set bounded HTTP `Priority` request metadata.
  ///
  /// This validates RFC 9218 urgency, incremental, and extension parameters
  /// before connecting. It only declares request metadata; it does not change
  /// transport scheduling.
  pub fn priority<V: AsRef<str>>(&mut self, value: V) -> error::Result<&mut Self> {
    let priority = Priority::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new("Priority", priority.header_value())))
  }

  /// Append bounded RFC 7239 `Forwarded` request metadata.
  ///
  /// This validates and preserves forwarding elements such as `for`, `by`,
  /// `host`, and `proto`; it does not select a proxy, establish trust, or
  /// rewrite any address.
  pub fn forwarded<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let forwarded = Forwarded::parse(value.as_ref())
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Forwarded"))
    {
      let combined = Forwarded::parse_values([header.value().as_str(), value.as_ref()])
        .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
      let value = bounded_forwarded_header_value(combined)?;
      header.replace(Header::new("Forwarded", value));
    } else {
      headers.push(Header::new(
        "Forwarded",
        bounded_forwarded_header_value(forwarded)?,
      ));
    }
    Ok(self)
  }

  /// Append bounded RFC 8586 `CDN-Loop` request metadata.
  ///
  /// This validates and preserves CDN identifiers and optional parameters,
  /// combining with any existing validated `CDN-Loop` field before a socket
  /// is opened. It only emits caller-supplied metadata: it does not invent a
  /// local CDN identifier, append on every request, or treat a repeated
  /// identifier as a transport failure.
  pub fn cdn_loop<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let cdn_loop = CdnLoop::parse(value.as_ref())
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("CDN-Loop"))
    {
      let combined = CdnLoop::parse_values([header.value().as_str(), value.as_ref()])
        .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
      let value = bounded_cdn_loop_header_value(combined)?;
      header.replace(Header::new("CDN-Loop", value));
    } else {
      headers.push(Header::new(
        "CDN-Loop",
        bounded_cdn_loop_header_value(cdn_loop)?,
      ));
    }
    Ok(self)
  }

  /// Set a single bounded byte range request header, `Range: bytes=start-end`.
  pub fn range(&mut self, start: u64, end: u64) -> error::Result<&mut Self> {
    if start > end {
      return Err(error::builder_with_message(
        "byte range start cannot be greater than end",
      ));
    }
    Ok(self.header(("Range", format!("bytes={}-{}", start, end).as_str())))
  }

  /// Set a single open-ended byte range request header, `Range: bytes=start-`.
  pub fn range_from(&mut self, start: u64) -> &mut Self {
    self.header(("Range", format!("bytes={}-", start).as_str()))
  }

  /// Set a single suffix byte range request header, `Range: bytes=-suffix`.
  pub fn range_suffix(&mut self, suffix: u64) -> error::Result<&mut Self> {
    if suffix == 0 {
      return Err(error::builder_with_message(
        "byte range suffix length must be greater than zero",
      ));
    }
    Ok(self.header(("Range", format!("bytes=-{}", suffix).as_str())))
  }

  /// Set a bounded `Max-Forwards` request header for TRACE or OPTIONS diagnostics.
  ///
  /// The value must be a singleton `1*DIGIT` hop count that fits in the `u32`
  /// range (`0` through `4294967295`). This only emits the header; it does not
  /// route through proxies, decrement the value, retry requests, or select a
  /// TRACE or OPTIONS policy. Use `header` directly for unusual values.
  pub fn max_forwards<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let max_forwards = MaxForwards::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Max-Forwards", max_forwards.header_value())))
  }

  /// Set bounded WebDAV `Depth` request metadata.
  ///
  /// The value must be the singleton value `0`, `1`, or `infinity`, with
  /// optional whitespace trimmed and `infinity` normalized to lowercase. This
  /// only validates and emits the header; it does not traverse resources,
  /// choose methods, or enforce WebDAV policy. Use `header` directly for
  /// unusual values.
  pub fn depth<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let depth = Depth::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Depth", depth.header_value())))
  }

  /// Set bounded WebDAV `Timeout` request metadata.
  ///
  /// The value must be an ordered list of `Second-n` and `Infinite`
  /// alternatives. Members are normalized to lowercase, duplicate alternatives
  /// are rejected, and size and count bounds are enforced before connecting.
  /// This only validates and emits the header; it does not create locks,
  /// refresh locks, or select an application timeout. Use `header` directly for
  /// unusual values.
  pub fn timeout<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let timeout = Timeout::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("Timeout", timeout.header_value())))
  }

  /// Append a validated `Accept-Charset` range with the default quality of
  /// `1`. This declares request metadata only; it does not negotiate,
  /// transcode, or select a response charset.
  pub fn accept_charset<S: AsRef<str>>(&mut self, charset: S) -> error::Result<&mut Self> {
    self.accept_charset_member(charset.as_ref(), None)
  }

  /// Append a validated `Accept-Charset` range with an HTTP q-value.
  ///
  /// The q-value must be between `0` and `1` with at most three fractional
  /// digits. This declares request metadata only; it does not negotiate,
  /// transcode, or select a response charset.
  pub fn accept_charset_with_q<C: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    charset: C,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.accept_charset_member(charset.as_ref(), Some(qvalue.as_ref()))
  }

  /// Set a bounded `Idempotency-Key` request header as opaque metadata.
  ///
  /// The key must be one or more visible ASCII bytes after HTTP optional
  /// whitespace is trimmed, and is limited to 64 KiB. CR, LF, NUL, other
  /// control bytes, and obs-text are rejected before a socket is opened. This
  /// only validates and emits the header; it does not retry requests, store
  /// keys, compare keys across requests, or apply application idempotency
  /// policy. Use `header` directly for unusual values.
  pub fn idempotency_key<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let idempotency_key = IdempotencyKey::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "Idempotency-Key",
      idempotency_key.header_value(),
    )))
  }

  /// Set a bounded `Sec-WebSocket-Key` request header as typed nonce metadata.
  ///
  /// The value must be one RFC 4648 section 4 base64 encoding of exactly 16
  /// nonce bytes, and is limited to 64 KiB. CR, LF, NUL, other control bytes,
  /// and obs-text are rejected before a socket is opened. This only validates
  /// and emits the header; it does not perform an HTTP upgrade, compute
  /// `Sec-WebSocket-Accept`, generate a random nonce, or implement WebSocket
  /// frames. Use `header` directly for unusual values.
  pub fn sec_websocket_key<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let sec_websocket_key = SecWebSocketKey::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "Sec-WebSocket-Key",
      sec_websocket_key.header_value(),
    )))
  }

  /// Set bounded W3C `traceparent` request metadata.
  ///
  /// This validates the version 00 wire value before connecting and replaces
  /// any existing `traceparent` field. It does not create trace identifiers,
  /// decide sampling, install a tracing backend, or automatically propagate
  /// metadata between requests.
  pub fn traceparent<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let traceparent = TraceParent::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("traceparent", traceparent.header_value())))
  }

  /// Set bounded W3C `tracestate` request metadata.
  ///
  /// This validates member grammar, duplicate keys, ordering, count, and size
  /// bounds before connecting and replaces any existing `tracestate` field. It
  /// does not configure or invoke a tracing backend.
  pub fn tracestate<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let tracestate = TraceState::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("tracestate", tracestate.header_value())))
  }

  /// Set bounded W3C `baggage` request metadata.
  ///
  /// This validates member keys, values, properties, duplicate keys, ordering,
  /// count, and size bounds before connecting and replaces any existing
  /// `baggage` field. It does not interpret application data, store request
  /// context, or automatically propagate metadata between requests.
  pub fn baggage<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let baggage = Baggage::parse(value.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new("baggage", baggage.header_value())))
  }

  /// Append a validated `Accept-Encoding` coding with the default quality of
  /// `1`. This declares request metadata only; it does not enable compression
  /// or decompression.
  pub fn accept_encoding<S: AsRef<str>>(&mut self, coding: S) -> error::Result<&mut Self> {
    self.accept_encoding_member(coding.as_ref(), None)
  }

  /// Append a validated `Accept-Encoding` coding with an HTTP q-value.
  ///
  /// The q-value must be between `0` and `1` with at most three fractional
  /// digits. This declares request metadata only; it does not enable
  /// compression or decompression.
  pub fn accept_encoding_with_q<C: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    coding: C,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.accept_encoding_member(coding.as_ref(), Some(qvalue.as_ref()))
  }

  /// Append `gzip` to `Accept-Encoding`.
  pub fn accept_gzip(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("gzip")
  }

  /// Append `gzip` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_gzip_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("gzip", qvalue)
  }

  /// Append `deflate` to `Accept-Encoding`.
  pub fn accept_deflate(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("deflate")
  }

  /// Append `deflate` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_deflate_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("deflate", qvalue)
  }

  /// Append `br` to `Accept-Encoding`.
  pub fn accept_br(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("br")
  }

  /// Append `br` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_br_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("br", qvalue)
  }

  /// Append `identity` to `Accept-Encoding`.
  pub fn accept_identity(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("identity")
  }

  /// Append `identity` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_identity_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("identity", qvalue)
  }

  /// Append a validated digest algorithm to `Want-Content-Digest` with the
  /// highest preference.
  ///
  /// This declares request metadata only; it does not compute or validate
  /// content digests.
  pub fn want_content_digest<S: AsRef<str>>(&mut self, algorithm: S) -> error::Result<&mut Self> {
    self.want_digest_member("Want-Content-Digest", algorithm.as_ref(), None)
  }

  /// Append a validated digest algorithm with a relative preference from `0`
  /// through `10` to `Want-Content-Digest`.
  pub fn want_content_digest_with_q<A: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    algorithm: A,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.want_digest_member(
      "Want-Content-Digest",
      algorithm.as_ref(),
      Some(qvalue.as_ref()),
    )
  }

  /// Append a validated digest algorithm to `Want-Repr-Digest` with the
  /// highest preference.
  ///
  /// This declares request metadata only; it does not compute or validate
  /// representation digests.
  pub fn want_repr_digest<S: AsRef<str>>(&mut self, algorithm: S) -> error::Result<&mut Self> {
    self.want_digest_member("Want-Repr-Digest", algorithm.as_ref(), None)
  }

  /// Append a validated digest algorithm with a relative preference from `0`
  /// through `10` to `Want-Repr-Digest`.
  pub fn want_repr_digest_with_q<A: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    algorithm: A,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.want_digest_member(
      "Want-Repr-Digest",
      algorithm.as_ref(),
      Some(qvalue.as_ref()),
    )
  }

  /// Append validated `TE` transfer codings. A single call may carry one
  /// coding or several comma-separated codings, and a coding may include an
  /// inline `;q=` value. This declares request metadata only; it does not
  /// enable a transfer-coding engine.
  pub fn te<S: AsRef<str>>(&mut self, coding: S) -> error::Result<&mut Self> {
    self.te_member(coding.as_ref(), None)
  }

  /// Append a validated `TE` transfer coding with an HTTP q-value.
  ///
  /// The q-value must be between `0` and `1` with at most three fractional
  /// digits. `trailers` cannot carry a q-value.
  pub fn te_with_q<C: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    coding: C,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.te_member(coding.as_ref(), Some(qvalue.as_ref()))
  }

  /// Append `trailers` to `TE`.
  ///
  /// On bounded HTTP/2 paths, this is the only `TE` value emitted; other `TE`
  /// values are stripped with connection-specific request metadata.
  pub fn te_trailers(&mut self) -> error::Result<&mut Self> {
    self.te_member("trailers", None)
  }

  /// Append a token-only `Prefer` request metadata item.
  ///
  /// This records preference metadata only; it does not schedule asynchronous
  /// work or alter response handling.
  pub fn prefer<S: AsRef<str>>(&mut self, name: S) -> error::Result<&mut Self> {
    self.prefer_member(name.as_ref(), None)
  }

  /// Append a token-valued `Prefer` request metadata item.
  ///
  /// `wait` values must be unsigned decimal integers. This records preference
  /// metadata only; it does not apply response preference policy.
  pub fn prefer_with_value<N: AsRef<str>, V: AsRef<str>>(
    &mut self,
    name: N,
    value: V,
  ) -> error::Result<&mut Self> {
    self.prefer_member(name.as_ref(), Some(value.as_ref()))
  }

  /// Set an `If-Range` validator with a single strong entity tag.
  ///
  /// `If-Range` only permits strong entity-tag validators. Use `header`
  /// directly for manual values that intentionally bypass this helper.
  pub fn if_range_etag<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_strong_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-Range", etag)))
  }

  /// Set an `If-Range` validator with an HTTP-date.
  pub fn if_range_date<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let http_date = validate_http_date(http_date.as_ref())?;
    Ok(self.header(Header::new("If-Range", http_date)))
  }

  /// Set a single entity-tag validator, `If-None-Match: <etag>`.
  ///
  /// Accepts `*`, a strong entity tag such as `"abc"`, or a weak entity tag
  /// such as `W/"abc"`. Use `header` directly for multiple validators.
  pub fn if_none_match<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-None-Match", etag)))
  }

  /// Set a single entity-tag validator, `If-Match: <etag>`.
  ///
  /// Accepts `*`, a strong entity tag such as `"abc"`, or a weak entity tag
  /// such as `W/"abc"`. Use `header` directly for multiple validators.
  pub fn if_match<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-Match", etag)))
  }

  /// Set an HTTP-date modification validator, `If-Modified-Since: <http-date>`.
  pub fn if_modified_since<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let if_modified_since = IfModifiedSince::parse(http_date.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "If-Modified-Since",
      if_modified_since.header_value(),
    )))
  }

  /// Set an HTTP-date modification validator, `If-Unmodified-Since: <http-date>`.
  pub fn if_unmodified_since<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let if_unmodified_since = IfUnmodifiedSince::parse(http_date.as_ref())
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    Ok(self.header(Header::new(
      "If-Unmodified-Since",
      if_unmodified_since.header_value(),
    )))
  }

  /// Set request content type
  pub fn content_type<S: AsRef<str>>(&mut self, content_type: S) -> &mut Self {
    self.header(("Content-Type", content_type.as_ref()))
  }

  fn accept_charset_member(
    &mut self,
    charset: &str,
    qvalue: Option<&str>,
  ) -> error::Result<&mut Self> {
    let charset = charset.trim();
    if !is_http_token(charset) {
      return Err(error::builder_with_message("invalid Accept-Charset range"));
    }
    let member = qvalue.map_or_else(
      || charset.to_string(),
      |qvalue| format!("{charset};q={qvalue}"),
    );
    let parsed_member = AcceptCharset::parse(&member)
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    if parsed_member.len() != 1 {
      return Err(error::builder_with_message(if qvalue.is_some() {
        "invalid Accept-Charset q-value"
      } else {
        "invalid Accept-Charset range"
      }));
    }
    let headers = self.request.headers_mut();
    let candidate = match headers
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case("Accept-Charset"))
    {
      Some(header) => format!("{}, {member}", header.value()),
      None => member,
    };
    let charsets = AcceptCharset::parse(&candidate)
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    let value = charsets.header_value();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Accept-Charset"))
    {
      header.replace(Header::new("Accept-Charset", value));
    } else {
      headers.push(Header::new("Accept-Charset", value));
    }
    Ok(self)
  }

  fn accept_encoding_member(
    &mut self,
    coding: &str,
    qvalue: Option<&str>,
  ) -> error::Result<&mut Self> {
    let coding = coding.trim();
    if !is_http_token(coding) {
      return Err(error::builder_with_message(
        "invalid Accept-Encoding coding",
      ));
    }
    let member = qvalue.map_or_else(
      || coding.to_string(),
      |qvalue| format!("{coding};q={qvalue}"),
    );
    let parsed_member = AcceptEncoding::parse(&member)
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    if parsed_member.len() != 1 {
      return Err(error::builder_with_message(if qvalue.is_some() {
        "invalid Accept-Encoding q-value"
      } else {
        "invalid Accept-Encoding coding"
      }));
    }
    let headers = self.request.headers_mut();
    let candidate = match headers
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case("Accept-Encoding"))
    {
      Some(header) => format!("{}, {member}", header.value()),
      None => member,
    };
    let encodings = AcceptEncoding::parse(&candidate)
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    let value = encodings.header_value();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Accept-Encoding"))
    {
      header.replace(Header::new("Accept-Encoding", value));
    } else {
      headers.push(Header::new("Accept-Encoding", value));
    }
    Ok(self)
  }

  fn te_member(&mut self, coding: &str, qvalue: Option<&str>) -> error::Result<&mut Self> {
    let coding = coding.trim();
    let qvalue = qvalue.map(str::trim);
    let member = qvalue.map_or_else(
      || coding.to_string(),
      |qvalue| format!("{coding};q={qvalue}"),
    );
    let incoming = Te::parse_values([member.as_str()])
      .map_err(|error| error::builder_with_message(error.to_string()))?;
    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("TE"))
    {
      let known = Te::parse_values([header.value().as_str()])
        .map_err(|_| error::builder_with_message("invalid TE coding"))?;
      if incoming.codings().iter().any(|incoming| {
        known
          .codings()
          .iter()
          .any(|known| known.coding().eq_ignore_ascii_case(incoming.coding()))
      }) {
        return Err(error::builder_with_message("duplicate TE coding"));
      }
      if known.len() + incoming.len() > MAX_TE_CODINGS {
        return Err(error::builder_with_message("too many TE codings"));
      }
      let value = format!("{}, {member}", header.value());
      if value.len() > MAX_TE_VALUE_BYTES {
        return Err(error::builder_with_message("TE header value is too large"));
      }
      header.replace(Header::new("TE", value));
    } else {
      headers.push(Header::new("TE", member));
    }
    self.ensure_connection_te_token();
    Ok(self)
  }

  fn want_digest_member(
    &mut self,
    header_name: &str,
    algorithm: &str,
    qvalue: Option<&str>,
  ) -> error::Result<&mut Self> {
    let algorithm = algorithm.trim();
    if !is_http_token(algorithm) {
      return Err(error::builder_with_message("invalid digest algorithm"));
    }
    let preference = qvalue.map(validate_digest_qvalue).transpose()?;
    let member = preference.map_or_else(
      || format!("{algorithm}=10"),
      |preference| format!("{algorithm}={preference}"),
    );
    append_unique_metadata_member(
      self.request.headers_mut(),
      header_name,
      algorithm,
      member,
      "invalid digest algorithm",
      "duplicate digest algorithm",
      "too many digest algorithms",
      "digest header value is too large",
      parse_digest_algorithms,
    )?;
    Ok(self)
  }

  fn ensure_connection_te_token(&mut self) {
    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Connection"))
    {
      if !header
        .value()
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case("TE"))
      {
        header.replace(Header::new("Connection", format!("{}, TE", header.value())));
      }
    } else {
      headers.push(Header::new("Connection", "Close, TE"));
    }
  }

  fn prefer_member(&mut self, name: &str, value: Option<&str>) -> error::Result<&mut Self> {
    let name = name.trim();
    let value = value.map(str::trim);
    if !is_http_token(name)
      || (name.eq_ignore_ascii_case("wait")
        && !value.is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_digit())))
      || value.is_some_and(|value| value.len() > MAX_PREFER_VALUE_BYTES || !is_http_token(value))
    {
      return Err(error::builder_with_message("invalid Prefer preference"));
    }
    let member = value.map_or_else(|| name.to_string(), |value| format!("{name}={value}"));
    if member.len() > MAX_PREFER_FIELD_BYTES {
      return Err(error::builder_with_message(
        "Prefer header value is too large",
      ));
    }

    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Prefer"))
    {
      let names = parse_prefer_names(header.value())?;
      if names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
        return Err(error::builder_with_message("duplicate Prefer preference"));
      }
      if names.len() >= MAX_PREFERENCES {
        return Err(error::builder_with_message("too many Prefer preferences"));
      }
      let value = format!("{}, {member}", header.value());
      if value.len() > MAX_PREFER_FIELD_BYTES {
        return Err(error::builder_with_message(
          "Prefer header value is too large",
        ));
      }
      header.replace(Header::new("Prefer", value));
    } else {
      headers.push(Header::new("Prefer", member));
    }
    Ok(self)
  }

  fn accept_member(&mut self, media_range: &str, qvalue: Option<&str>) -> error::Result<&mut Self> {
    if media_range.bytes().any(|byte| byte.is_ascii_control()) {
      return Err(error::builder_with_message("invalid Accept media range"));
    }
    let media_range = media_range.trim();
    let has_quality = validate_accept_media_range(media_range)?;
    let qvalue = qvalue.map(validate_accept_qvalue).transpose()?;
    if has_quality && qvalue.is_some() {
      return Err(error::builder_with_message(
        "duplicate Accept quality value",
      ));
    }
    let member = qvalue.map_or_else(
      || media_range.to_string(),
      |qvalue| format!("{media_range};q={qvalue}"),
    );
    if member.len() > MAX_ACCEPT_VALUE_BYTES {
      return Err(error::builder_with_message(
        "Accept header value is too large",
      ));
    }

    let headers = self.request.headers_mut();
    let existing = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Accept"));
    if let Some(header) = existing {
      let existing_ranges = parse_accept_media_ranges(header.value())?;
      if existing_ranges >= MAX_ACCEPT_MEDIA_RANGES {
        return Err(error::builder_with_message("too many Accept media ranges"));
      }
      let value = format!("{}, {member}", header.value());
      if value.len() > MAX_ACCEPT_VALUE_BYTES {
        return Err(error::builder_with_message(
          "Accept header value is too large",
        ));
      }
      header.replace(Header::new("Accept", value));
    } else {
      headers.push(Header::new("Accept", member));
    }
    Ok(self)
  }

  fn cache_control_member(&mut self, name: &str, value: Option<&str>) -> error::Result<&mut Self> {
    let name = name.trim();
    if !is_http_token(name) || value.is_some_and(|value| !is_http_token(value)) {
      return Err(error::builder_with_message(
        "invalid Cache-Control directive",
      ));
    }
    let member = value.map_or_else(|| name.to_string(), |value| format!("{name}={value}"));
    if member.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
      return Err(error::builder_with_message(
        "Cache-Control header value is too large",
      ));
    }

    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Cache-Control"))
    {
      let directives = parse_cache_control_directive_names(header.value())?;
      if directives
        .iter()
        .any(|known| known.eq_ignore_ascii_case(name))
      {
        return Err(error::builder_with_message(
          "duplicate Cache-Control directive",
        ));
      }
      if directives.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
        return Err(error::builder_with_message(
          "too many Cache-Control directives",
        ));
      }
      let value = format!("{}, {member}", header.value());
      if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
        return Err(error::builder_with_message(
          "Cache-Control header value is too large",
        ));
      }
      header.replace(Header::new("Cache-Control", value));
    } else {
      headers.push(Header::new("Cache-Control", member));
    }
    Ok(self)
  }

  fn cache_control_extension_member(
    &mut self,
    name: &str,
    value: Option<&str>,
  ) -> error::Result<&mut Self> {
    if ["no-cache", "no-store", "max-age"]
      .iter()
      .any(|directive| directive.eq_ignore_ascii_case(name.trim()))
    {
      return Err(error::builder_with_message(
        "Cache-Control directive must use a dedicated helper",
      ));
    }
    self.cache_control_member(name, value)
  }

  /// Add request para
  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
    let paras = para.into_paras();
    self.request.paras_mut().extend(paras);
    self
  }

  /// Add request form data. include file
  pub fn form<S: ToFormData>(&mut self, formdata: S) -> &mut Self {
    let formdatas = formdata.to_formdatas();
    self.request.formdatas_mut().extend(formdatas);
    self
  }

  /// Set request raw data
  pub fn raw<S: AsRef<str>>(&mut self, raw: S) -> &mut Self {
    self.request.raw_set(raw);
    self
  }

  /// Set binary data
  pub fn binary(&mut self, binary: Vec<u8>) -> &mut Self {
    self.request.binary_set(binary);
    self
  }

  /// emit a request
  ///
  /// # Examples
  /// ```rust
  /// # use rttp_client::HttpClient;
  /// HttpClient::new()
  ///   .url("http://httpbin.org.get")
  ///   .emit();
  /// ```
  pub fn emit(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call()
  }

  #[cfg(feature = "http2")]
  pub fn emit_http2_prior_knowledge(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.proxy().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge client does not support proxies",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    crate::http2::PriorKnowledgeClient::new(request).get()
  }

  #[cfg(feature = "http2")]
  pub fn emit_http2_upgrade(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.http2_extended_connect_protocol().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 extended CONNECT is only supported by the prior-knowledge h2c client",
      ));
    }
    if self.request.proxy().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 h2c upgrade client does not support proxies",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    crate::http2::UpgradeClient::new(request).get()
  }

  pub fn emit_streaming_fixed<R>(
    &mut self,
    mut reader: R,
    content_length: u64,
  ) -> error::Result<Response>
  where
    R: io::Read,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_fixed_body(content_length);
    let result = (|| {
      let request = RawRequest::block_new(&mut self.request)?;
      BlockConnection::new(request).call_streaming_body(StreamingRequestBody::Fixed {
        reader: &mut reader,
        content_length,
      })
    })();
    self.request.clear_streaming_body_headers();
    result
  }

  pub fn emit_streaming_chunked<R>(&mut self, mut reader: R) -> error::Result<Response>
  where
    R: io::Read,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_chunked_body();
    let trailers = self.request.trailers().clone();
    let result = (|| {
      let request = RawRequest::block_new(&mut self.request)?;
      BlockConnection::new(request).call_streaming_body(StreamingRequestBody::Chunked {
        reader: &mut reader,
        trailers: &trailers,
      })
    })();
    self.request.clear_streaming_chunked_body_headers();
    result
  }

  pub fn connect(&mut self) -> error::Result<HandoffConnection> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "CONNECT socket handoff cannot be combined with a request body",
      ));
    }
    self.request.method_set("CONNECT");
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call_connect_handoff()
  }

  pub fn upgrade(&mut self) -> error::Result<HandoffConnection> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "Upgrade socket handoff cannot be combined with a request body",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call_upgrade_handoff()
  }

  /// Async request emit
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use rttp_client::HttpClient;
  /// # #[cfg(feature = "async")]
  /// # async fn test_async() {
  /// HttpClient::new()
  ///   .url("http://httpbin.org.get")
  ///   .rasync()
  ///   .await;
  /// # }
  /// ```
  #[cfg(feature = "async")]
  pub async fn rasync(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    let request = RawRequest::async_new(&mut self.request).await?;
    AsyncConnection::new(request).async_call().await
  }

  #[cfg(feature = "async")]
  pub async fn rasync_streaming_fixed<R>(
    &mut self,
    mut reader: R,
    content_length: u64,
  ) -> error::Result<Response>
  where
    R: AsyncRead + Unpin,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_fixed_body(content_length);
    let result = async {
      let request = RawRequest::async_new(&mut self.request).await?;
      AsyncConnection::new(request)
        .async_call_streaming_body(AsyncStreamingRequestBody::Fixed {
          reader: &mut reader,
          content_length,
        })
        .await
    }
    .await;
    self.request.clear_streaming_body_headers();
    result
  }

  #[cfg(feature = "async")]
  pub async fn rasync_streaming_chunked<R>(&mut self, mut reader: R) -> error::Result<Response>
  where
    R: AsyncRead + Unpin,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_chunked_body();
    let trailers = self.request.trailers().clone();
    let result = async {
      let request = RawRequest::async_new(&mut self.request).await?;
      AsyncConnection::new(request)
        .async_call_streaming_body(AsyncStreamingRequestBody::Chunked {
          reader: &mut reader,
          trailers: &trailers,
        })
        .await
    }
    .await;
    self.request.clear_streaming_chunked_body_headers();
    result
  }
}

fn bounded_forwarded_header_value(forwarded: Forwarded) -> error::Result<String> {
  let value = forwarded.header_value();
  if value.len() > MAX_FORWARDED_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Forwarded header value is too large",
    ));
  }
  Ok(value)
}

fn bounded_cdn_loop_header_value(cdn_loop: CdnLoop) -> error::Result<String> {
  let value = cdn_loop.header_value();
  if value.len() > MAX_CDN_LOOP_VALUE_BYTES {
    return Err(error::builder_with_message(
      "CDN-Loop header value is too large",
    ));
  }
  Ok(value)
}

const MAX_REQUEST_METADATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_METADATA_MEMBERS: usize = 32;
const MAX_PREFER_FIELD_BYTES: usize = 64 * 1024;
const MAX_PREFER_VALUE_BYTES: usize = 8 * 1024;
const MAX_PREFERENCES: usize = 32;

#[allow(clippy::too_many_arguments)]
fn append_unique_metadata_member(
  headers: &mut Vec<Header>,
  header_name: &str,
  key: &str,
  member: String,
  invalid_error: &str,
  duplicate_error: &str,
  count_error: &str,
  size_error: &str,
  parse_keys: fn(&str) -> error::Result<Vec<&str>>,
) -> error::Result<()> {
  if member.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
    return Err(error::builder_with_message(size_error));
  }
  if let Some(header) = headers
    .iter_mut()
    .find(|header| header.name().eq_ignore_ascii_case(header_name))
  {
    let known =
      parse_keys(header.value()).map_err(|_| error::builder_with_message(invalid_error))?;
    if known.iter().any(|known| known.eq_ignore_ascii_case(key)) {
      return Err(error::builder_with_message(duplicate_error));
    }
    if known.len() >= MAX_REQUEST_METADATA_MEMBERS {
      return Err(error::builder_with_message(count_error));
    }
    let value = format!("{}, {member}", header.value());
    if value.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
      return Err(error::builder_with_message(size_error));
    }
    header.replace(Header::new(header_name, value));
  } else {
    headers.push(Header::new(header_name, member));
  }
  Ok(())
}

fn parse_digest_algorithms(value: &str) -> error::Result<Vec<&str>> {
  parse_metadata_members(value, "invalid digest algorithm", |member| {
    let (algorithm, preference) = member.trim().split_once('=')?;
    let algorithm = algorithm.trim();
    (is_http_token(algorithm) && validate_digest_qvalue(preference.trim()).is_ok())
      .then_some(algorithm)
  })
}

fn validate_digest_qvalue(qvalue: &str) -> error::Result<&str> {
  matches!(
    qvalue,
    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10"
  )
  .then_some(qvalue)
  .ok_or_else(|| error::builder_with_message("invalid digest preference"))
}

fn parse_prefer_names(value: &str) -> error::Result<Vec<&str>> {
  if value.len() > MAX_PREFER_FIELD_BYTES {
    return Err(error::builder_with_message(
      "Prefer header value is too large",
    ));
  }
  let mut names = Vec::new();
  for member in value.split(',') {
    let (name, preference_value) = split_prefer_member(member)?;
    if names
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(name))
    {
      return Err(error::builder_with_message("duplicate Prefer preference"));
    }
    if names.len() >= MAX_PREFERENCES {
      return Err(error::builder_with_message("too many Prefer preferences"));
    }
    let _ = preference_value;
    names.push(name);
  }
  Ok(names)
}

fn split_prefer_member(member: &str) -> error::Result<(&str, Option<&str>)> {
  let (name, value) = member
    .trim()
    .split_once('=')
    .map_or((member.trim(), None), |(name, value)| {
      (name.trim(), Some(value.trim()))
    });
  if !is_http_token(name)
    || (name.eq_ignore_ascii_case("wait")
      && !value.is_some_and(|value| value.bytes().all(|byte| byte.is_ascii_digit())))
    || value.is_some_and(|value| value.len() > MAX_PREFER_VALUE_BYTES || !is_http_token(value))
  {
    return Err(error::builder_with_message("invalid Prefer preference"));
  }
  Ok((name, value))
}

fn parse_metadata_members<'a>(
  value: &'a str,
  error_message: &str,
  parse: impl Fn(&'a str) -> Option<&'a str>,
) -> error::Result<Vec<&'a str>> {
  if value.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
    return Err(error::builder_with_message(error_message));
  }
  let mut members = Vec::new();
  for member in value.split(',') {
    let Some(key) = parse(member.trim()) else {
      return Err(error::builder_with_message(error_message));
    };
    if members
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(key))
      || members.len() >= MAX_REQUEST_METADATA_MEMBERS
    {
      return Err(error::builder_with_message(error_message));
    }
    members.push(key);
  }
  Ok(members)
}

const MAX_ACCEPT_VALUE_BYTES: usize = 64 * 1024;
const MAX_ACCEPT_MEDIA_RANGES: usize = 32;
const MAX_CACHE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
const MAX_CACHE_CONTROL_DIRECTIVES: usize = 256;

fn parse_cache_control_directive_names(value: &str) -> error::Result<Vec<&str>> {
  if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Cache-Control header value is too large",
    ));
  }
  let mut names = Vec::new();
  for directive in value.split(',') {
    let (name, directive_value) = directive
      .trim()
      .split_once('=')
      .map_or((directive.trim(), None), |(name, value)| {
        (name.trim(), Some(value.trim()))
      });
    if !is_http_token(name) || directive_value.is_some_and(|value| !is_http_token(value)) {
      return Err(error::builder_with_message(
        "invalid Cache-Control directive",
      ));
    }
    if names
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(name))
    {
      return Err(error::builder_with_message(
        "duplicate Cache-Control directive",
      ));
    }
    if names.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
      return Err(error::builder_with_message(
        "too many Cache-Control directives",
      ));
    }
    names.push(name);
  }
  Ok(names)
}

fn parse_accept_media_ranges(value: &str) -> error::Result<usize> {
  if value.len() > MAX_ACCEPT_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Accept header value is too large",
    ));
  }
  let members = split_accept_delimited(value, b',')?;
  if members.len() > MAX_ACCEPT_MEDIA_RANGES {
    return Err(error::builder_with_message("too many Accept media ranges"));
  }
  for member in &members {
    validate_accept_media_range(member)?;
  }
  Ok(members.len())
}

fn validate_accept_media_range(value: &str) -> error::Result<bool> {
  let parts = split_accept_delimited(value, b';')?;
  let Some(media_type) = parts.first() else {
    return Err(error::builder_with_message("invalid Accept media range"));
  };
  validate_accept_media_type(media_type.trim())?;

  let mut names = Vec::new();
  let mut has_quality = false;
  for parameter in parts.iter().skip(1) {
    let Some((name, value)) = parameter.trim().split_once('=') else {
      return Err(error::builder_with_message("invalid Accept parameter"));
    };
    let name = name.trim();
    let value = value.trim();
    if !is_http_token(name) || !is_accept_parameter_value(value) {
      return Err(error::builder_with_message("invalid Accept parameter"));
    }
    if names
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(name))
    {
      return Err(error::builder_with_message("duplicate Accept parameter"));
    }
    if name.eq_ignore_ascii_case("q") {
      validate_accept_qvalue(value)?;
      has_quality = true;
    }
    names.push(name);
  }
  Ok(has_quality)
}

fn validate_accept_media_type(value: &str) -> error::Result<()> {
  let Some((type_name, subtype)) = value.split_once('/') else {
    return Err(error::builder_with_message("invalid Accept media range"));
  };
  if subtype.contains('/') {
    return Err(error::builder_with_message("invalid Accept media range"));
  }
  let type_name = type_name.trim();
  let subtype = subtype.trim();
  if (type_name == "*" && subtype != "*")
    || !(type_name == "*" || is_http_token(type_name))
    || !(subtype == "*" || is_http_token(subtype))
  {
    return Err(error::builder_with_message("invalid Accept media range"));
  }
  Ok(())
}

fn validate_accept_qvalue(qvalue: &str) -> error::Result<&str> {
  if is_qvalue(qvalue) {
    Ok(qvalue)
  } else {
    Err(error::builder_with_message("invalid Accept quality value"))
  }
}

fn split_accept_delimited(value: &str, delimiter: u8) -> error::Result<Vec<&str>> {
  let mut members = Vec::new();
  let mut quoted = false;
  let mut escaped = false;
  let mut start = 0usize;
  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      if byte.is_ascii_control() {
        return Err(error::builder_with_message("invalid Accept parameter"));
      }
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' => quoted = !quoted,
      byte if byte == delimiter && !quoted => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err(error::builder_with_message("invalid Accept media range"));
        }
        members.push(member);
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped {
    return Err(error::builder_with_message("invalid Accept parameter"));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(error::builder_with_message("invalid Accept media range"));
  }
  members.push(member);
  Ok(members)
}

fn is_accept_parameter_value(value: &str) -> bool {
  is_http_token(value) || is_accept_quoted_string(value)
}

fn is_accept_quoted_string(value: &str) -> bool {
  let Some(value) = value
    .strip_prefix('"')
    .and_then(|value| value.strip_suffix('"'))
  else {
    return false;
  };
  let mut escaped = false;
  for byte in value.bytes() {
    if escaped {
      if byte.is_ascii_control() {
        return false;
      }
      escaped = false;
    } else if byte == b'\\' {
      escaped = true;
    } else if byte == b'"' || byte.is_ascii_control() && byte != b'\t' {
      return false;
    }
  }
  !escaped
}

fn validate_request_trailer_header(name: &str, value: &str) -> error::Result<()> {
  if !is_http_token(name) || !value.bytes().all(is_header_value_byte) {
    return Err(error::builder_with_message(
      "Invalid request trailer header",
    ));
  }
  if is_forbidden_request_trailer_name(name) {
    return Err(error::builder_with_message(
      "Forbidden request trailer header",
    ));
  }
  Ok(())
}

const MAX_CONDITIONAL_VALIDATOR_VALUE_BYTES: usize = 64 * 1024;

fn validate_single_etag(etag: &str) -> error::Result<&str> {
  let etag = etag.trim();
  if etag.len() > MAX_CONDITIONAL_VALIDATOR_VALUE_BYTES {
    return Err(error::builder_with_message(
      "conditional entity-tag validator is too large",
    ));
  }
  if etag == "*" {
    return Ok(etag);
  }
  if etag.contains(',') {
    return Err(error::builder_with_message(
      "conditional entity-tag helper accepts one validator; use header() for lists",
    ));
  }

  let opaque_tag = etag.strip_prefix("W/").unwrap_or(etag);
  let Some(inner) = opaque_tag
    .strip_prefix('"')
    .and_then(|value| value.strip_suffix('"'))
  else {
    return Err(error::builder_with_message(
      "conditional entity-tag must be *, \"tag\", or W/\"tag\"",
    ));
  };

  if inner
    .as_bytes()
    .iter()
    .any(|byte| matches!(*byte, b'"' | b'\r' | b'\n') || *byte < 0x21 || *byte == 0x7f)
  {
    return Err(error::builder_with_message(
      "conditional entity-tag contains invalid characters",
    ));
  }

  Ok(etag)
}

fn validate_single_strong_etag(etag: &str) -> error::Result<&str> {
  let etag = validate_single_etag(etag)?;
  if etag == "*" || etag.starts_with("W/") {
    return Err(error::builder_with_message(
      "If-Range entity-tag helper accepts only a single strong entity tag",
    ));
  }
  Ok(etag)
}

fn validate_http_date(http_date: &str) -> error::Result<&str> {
  let http_date = http_date.trim();
  httpdate::parse_http_date(http_date).map_err(|_| {
    error::builder_with_message("conditional modification time must be a valid HTTP-date")
  })?;
  Ok(http_date)
}

fn build_accept_language_value<I, L>(ranges: I) -> error::Result<String>
where
  I: IntoIterator<Item = L>,
  L: AsRef<str>,
{
  let accept_language = AcceptLanguage::from_ranges(ranges)
    .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
  let value = accept_language.header_value();
  if value.len() > MAX_ACCEPT_LANGUAGE_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Accept-Language header value is too large",
    ));
  }
  Ok(value)
}

fn is_qvalue(value: &str) -> bool {
  match value.split_once('.') {
    Some((whole, fraction)) => {
      (whole == "0" || whole == "1")
        && fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => value == "0" || value == "1",
  }
}

fn is_forbidden_request_trailer_name(name: &str) -> bool {
  matches!(
    name.trim().to_ascii_lowercase().as_str(),
    "connection"
      | "content-length"
      | "host"
      | "keep-alive"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "proxy-connection"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
  )
}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_http_token_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!'
        | b'#'
        | b'$'
        | b'%'
        | b'&'
        | b'\''
        | b'*'
        | b'+'
        | b'-'
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~'
    )
}

fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}
