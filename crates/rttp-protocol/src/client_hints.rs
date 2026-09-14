use std::error::Error;
use std::fmt;

pub const MAX_CLIENT_HINT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CLIENT_HINT_NAMES: usize = 256;
pub const MAX_DPR_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_DOWNLINK_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ECT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_WIDTH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_VIEWPORT_WIDTH_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `DPR` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Dpr {
  value: String,
}

/// Parsed, bounded `Downlink` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Downlink {
  value: String,
}

/// The effective connection type declared by the `ECT` request Client Hint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Ect {
  Slow2g,
  TwoG,
  ThreeG,
  FourG,
}

/// Parsed, bounded `Width` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Width(u64);

/// Parsed, bounded `Viewport-Width` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ViewportWidth(u64);

/// Parsed, bounded `Accept-CH` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptCh {
  client_hints: Vec<String>,
}

/// Parsed, bounded `Critical-CH` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalCh {
  client_hints: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientHintsParseError {
  message: String,
}

pub type AcceptChParseError = ClientHintsParseError;
pub type CriticalChParseError = ClientHintsParseError;
pub type DprParseError = ClientHintsParseError;
pub type DownlinkParseError = ClientHintsParseError;
pub type EctParseError = ClientHintsParseError;
pub type WidthParseError = ClientHintsParseError;
pub type ViewportWidthParseError = ClientHintsParseError;

impl ClientHintsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ClientHintsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ClientHintsParseError {}

impl Dpr {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DprParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DprParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_dpr_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    parse_dpr_ratio(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn ratio(&self) -> f64 {
    parse_dpr_ratio(&self.value).expect("DPR values are validated at construction")
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl Downlink {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DownlinkParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DownlinkParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_downlink_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    parse_downlink_mbps(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn mbps(&self) -> f64 {
    parse_downlink_mbps(&self.value).expect("Downlink values are validated at construction")
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl Ect {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, EctParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, EctParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_ect_singleton(values)?.trim_matches([' ', '\t']) {
      "slow-2g" => Ok(Self::Slow2g),
      "2g" => Ok(Self::TwoG),
      "3g" => Ok(Self::ThreeG),
      "4g" => Ok(Self::FourG),
      _ => Err(invalid_ect_value()),
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Slow2g => "slow-2g",
      Self::TwoG => "2g",
      Self::ThreeG => "3g",
      Self::FourG => "4g",
    }
  }
}

impl Width {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, WidthParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, WidthParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_width_singleton(values).map(Self)
  }

  pub const fn value(self) -> u64 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

impl ViewportWidth {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, ViewportWidthParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ViewportWidthParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_viewport_width_singleton(values).map(Self)
  }

  pub const fn value(self) -> u64 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

impl AcceptCh {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptChParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptChParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      client_hints: parse_client_hints(values, "Accept-CH")?,
    })
  }

  pub fn client_hints(&self) -> &[String] {
    &self.client_hints
  }

  pub fn len(&self) -> usize {
    self.client_hints.len()
  }

  pub fn is_empty(&self) -> bool {
    self.client_hints.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.client_hints.join(", ")
  }
}

impl CriticalCh {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CriticalChParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CriticalChParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      client_hints: parse_client_hints(values, "Critical-CH")?,
    })
  }

  pub fn client_hints(&self) -> &[String] {
    &self.client_hints
  }

  pub fn len(&self) -> usize {
    self.client_hints.len()
  }

  pub fn is_empty(&self) -> bool {
    self.client_hints.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.client_hints.join(", ")
  }
}

fn parse_client_hints<'a, I>(
  values: I,
  header_name: &str,
) -> Result<Vec<String>, ClientHintsParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut client_hints = Vec::new();
  for value in values {
    if value.len() > MAX_CLIENT_HINT_VALUE_BYTES {
      return Err(ClientHintsParseError::new(format!(
        "{header_name} header value is too large"
      )));
    }
    for member in value.split(',') {
      let client_hint = member.trim();
      if !is_structured_token(client_hint) {
        return Err(ClientHintsParseError::new(format!(
          "invalid {header_name} client hint"
        )));
      }
      if client_hints.len() >= MAX_CLIENT_HINT_NAMES {
        return Err(ClientHintsParseError::new(format!(
          "too many {header_name} client hints"
        )));
      }
      client_hints.push(client_hint.to_string());
    }
  }
  if client_hints.is_empty() {
    return Err(ClientHintsParseError::new(format!(
      "invalid {header_name} client hint"
    )));
  }
  Ok(client_hints)
}

fn parse_dpr_singleton<'a, I>(values: I) -> Result<&'a str, DprParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_dpr_value)?;
  validate_bounded_dpr_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_dpr_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new("duplicate DPR header fields"));
  }
  Ok(value)
}

fn validate_bounded_dpr_value(value: &str) -> Result<(), DprParseError> {
  if value.len() > MAX_DPR_VALUE_BYTES {
    return Err(ClientHintsParseError::new("DPR header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid DPR control byte"));
  }
  Ok(())
}

fn parse_dpr_ratio(value: &str) -> Result<f64, DprParseError> {
  if !matches_decimal_grammar(value) {
    return Err(invalid_dpr_value());
  }
  let ratio: f64 = value.parse().map_err(|_| invalid_dpr_value())?;
  if !ratio.is_finite() || ratio <= 0.0 {
    return Err(invalid_dpr_value());
  }
  Ok(ratio)
}

fn parse_downlink_singleton<'a, I>(values: I) -> Result<&'a str, DownlinkParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_downlink_value)?;
  validate_bounded_downlink_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_downlink_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Downlink header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_downlink_value(value: &str) -> Result<(), DownlinkParseError> {
  if value.len() > MAX_DOWNLINK_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Downlink header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid Downlink control byte"));
  }
  Ok(())
}

fn parse_downlink_mbps(value: &str) -> Result<f64, DownlinkParseError> {
  if !matches_decimal_grammar(value) {
    return Err(invalid_downlink_value());
  }
  let mbps: f64 = value.parse().map_err(|_| invalid_downlink_value())?;
  if !mbps.is_finite() || mbps < 0.0 {
    return Err(invalid_downlink_value());
  }
  Ok(mbps)
}

fn parse_ect_singleton<'a, I>(values: I) -> Result<&'a str, EctParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_ect_value)?;
  validate_bounded_ect_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_ect_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new("duplicate ECT header fields"));
  }
  Ok(value)
}

fn validate_bounded_ect_value(value: &str) -> Result<(), EctParseError> {
  if value.len() > MAX_ECT_VALUE_BYTES {
    return Err(ClientHintsParseError::new("ECT header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid ECT control byte"));
  }
  Ok(())
}

fn parse_width_singleton<'a, I>(values: I) -> Result<u64, WidthParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_width_value)?;
  validate_bounded_width_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_width_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new("duplicate Width header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_width_value());
  }
  value.parse().map_err(|_| invalid_width_value())
}

fn validate_bounded_width_value(value: &str) -> Result<(), WidthParseError> {
  if value.len() > MAX_WIDTH_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Width header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid Width control byte"));
  }
  Ok(())
}

fn parse_viewport_width_singleton<'a, I>(values: I) -> Result<u64, ViewportWidthParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_viewport_width_value)?;
  validate_bounded_viewport_width_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_viewport_width_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Viewport-Width header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_viewport_width_value());
  }
  value.parse().map_err(|_| invalid_viewport_width_value())
}

fn validate_bounded_viewport_width_value(value: &str) -> Result<(), ViewportWidthParseError> {
  if value.len() > MAX_VIEWPORT_WIDTH_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Viewport-Width header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Viewport-Width control byte",
    ));
  }
  Ok(())
}

fn matches_decimal_grammar(value: &str) -> bool {
  let bytes = value.as_bytes();
  if bytes.is_empty() || !bytes[0].is_ascii_digit() {
    return false;
  }

  let mut index = 0;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  if index == bytes.len() {
    return true;
  }
  if bytes[index] != b'.' {
    return false;
  }
  index += 1;
  let fraction_start = index;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  index > fraction_start && index == bytes.len()
}

fn invalid_dpr_value() -> DprParseError {
  ClientHintsParseError::new("invalid DPR header value")
}

fn invalid_downlink_value() -> DownlinkParseError {
  ClientHintsParseError::new("invalid Downlink header value")
}

fn invalid_ect_value() -> EctParseError {
  ClientHintsParseError::new("invalid ECT header value")
}

fn invalid_width_value() -> WidthParseError {
  ClientHintsParseError::new("invalid Width header value")
}

fn invalid_viewport_width_value() -> ViewportWidthParseError {
  ClientHintsParseError::new("invalid Viewport-Width header value")
}

fn is_structured_token(value: &str) -> bool {
  let mut bytes = value.bytes();
  matches!(bytes.next(), Some(b'*' | b'a'..=b'z' | b'A'..=b'Z'))
    && bytes.all(|byte| {
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
            | b':'
            | b'/'
        )
    })
}
