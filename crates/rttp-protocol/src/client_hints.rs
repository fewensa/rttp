use std::error::Error;
use std::fmt;

use sfv::{BareItem, List, ListEntry, Parser, Version};

pub const MAX_CLIENT_HINT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CLIENT_HINT_NAMES: usize = 256;
pub const MAX_DPR_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_DPR_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_DOWNLINK_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_DEVICE_MEMORY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PREFERS_COLOR_SCHEME_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_MOBILE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_WOW64_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_PLATFORM_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_MODEL_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_ARCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_BITNESS_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_PLATFORM_VERSION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_FULL_VERSION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_ENTRIES: usize = 256;
pub const MAX_SEC_CH_UA_FULL_VERSION_LIST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_FULL_VERSION_LIST_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_FULL_VERSION_LIST_ENTRIES: usize = 256;
pub const MAX_SEC_CH_UA_FORM_FACTORS_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_FORM_FACTORS_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_UA_FORM_FACTORS_ITEMS: usize = 256;
pub const MAX_PREFERS_REDUCED_MOTION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PREFERS_REDUCED_TRANSPARENCY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PREFERS_REDUCED_DATA_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PREFERS_CONTRAST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ECT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_WIDTH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_VIEWPORT_WIDTH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_VIEWPORT_HEIGHT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_CH_VIEWPORT_WIDTH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_RTT_VALUE_BYTES: usize = 64 * 1024;

const MAX_SEC_CH_VIEWPORT_HEIGHT_INTEGER: u64 = 999_999_999_999_999;
const MAX_SEC_CH_VIEWPORT_HEIGHT_INTEGER_DIGITS: usize = 15;
const MAX_SEC_CH_VIEWPORT_WIDTH_INTEGER: u64 = 999_999_999_999_999;
const MAX_SEC_CH_VIEWPORT_WIDTH_INTEGER_DIGITS: usize = 15;

/// Parsed, bounded `DPR` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Dpr {
  value: String,
}

/// Parsed, bounded `Sec-CH-DPR` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChDpr {
  value: String,
}

/// Parsed, bounded `Downlink` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Downlink {
  value: String,
}

/// Parsed, bounded `Device-Memory` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeviceMemory {
  value: String,
}

/// Parsed, bounded `Sec-CH-Prefers-Color-Scheme` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrefersColorScheme {
  Light,
  Dark,
}

/// Parsed, bounded `Sec-CH-UA-Mobile` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecChUaMobile {
  NotMobile,
  Mobile,
}

/// Parsed, bounded `Sec-CH-UA-WoW64` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecChUaWow64 {
  NotWow64,
  Wow64,
}

/// Parsed, bounded `Sec-CH-UA-Platform` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaPlatform {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA-Model` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaModel {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA-Arch` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaArch {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA-Bitness` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaBitness {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA-Platform-Version` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaPlatformVersion {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA-Full-Version` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SecChUaFullVersion {
  value: String,
}

/// Parsed, bounded `Sec-CH-UA` request Client Hint metadata.
///
/// Entries are retained in wire order. This type only represents the syntax of
/// the declared brand/version list; it does not identify a browser or apply
/// user-agent policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecChUa {
  entries: Vec<SecChUaEntry>,
}

/// One ordered brand/version entry in a `Sec-CH-UA` value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecChUaEntry {
  brand: String,
  version: String,
}

/// Parsed, bounded `Sec-CH-UA-Full-Version-List` request Client Hint metadata.
///
/// Entries are retained in wire order. This type only represents the syntax of
/// the declared brand/version list; it does not identify a browser or apply
/// user-agent policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecChUaFullVersionList {
  entries: Vec<SecChUaFullVersionListEntry>,
}

/// One ordered brand/version entry in a `Sec-CH-UA-Full-Version-List` value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecChUaFullVersionListEntry {
  brand: String,
  version: String,
}

/// Parsed, bounded `Sec-CH-UA-Form-Factors` request Client Hint metadata.
///
/// Items are retained in wire order. Repeated field lines are combined in that
/// order, as Structured Fields requires for lists. This type only represents
/// the syntax of the declared RFC 8941 string list; it does not identify a
/// device class or apply browser policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecChUaFormFactors {
  items: Vec<String>,
}

/// Parsed, bounded `Sec-CH-Prefers-Reduced-Motion` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrefersReducedMotion {
  NoPreference,
  Reduce,
}

/// Parsed, bounded `Sec-CH-Prefers-Reduced-Transparency` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrefersReducedTransparency {
  NoPreference,
  Reduce,
}

/// Parsed, bounded `Sec-CH-Prefers-Reduced-Data` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrefersReducedData {
  NoPreference,
  Reduce,
}

/// Parsed, bounded `Sec-CH-Prefers-Contrast` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrefersContrast {
  NoPreference,
  More,
  Less,
  Custom,
}

/// Parsed, bounded `ECT` request Client Hint metadata.
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

/// Parsed, bounded `Sec-CH-Viewport-Height` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SecChViewportHeight(u64);

/// Parsed, bounded `Sec-CH-Viewport-Width` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SecChViewportWidth(u64);

/// Parsed, bounded `RTT` request Client Hint metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rtt(u64);

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
pub type SecChDprParseError = ClientHintsParseError;
pub type DownlinkParseError = ClientHintsParseError;
pub type DeviceMemoryParseError = ClientHintsParseError;
pub type PrefersColorSchemeParseError = ClientHintsParseError;
pub type SecChUaMobileParseError = ClientHintsParseError;
pub type SecChUaWow64ParseError = ClientHintsParseError;
pub type SecChUaPlatformParseError = ClientHintsParseError;
pub type SecChUaModelParseError = ClientHintsParseError;
pub type SecChUaArchParseError = ClientHintsParseError;
pub type SecChUaBitnessParseError = ClientHintsParseError;
pub type SecChUaPlatformVersionParseError = ClientHintsParseError;
pub type SecChUaFullVersionParseError = ClientHintsParseError;
pub type SecChUaParseError = ClientHintsParseError;
pub type SecChUaFullVersionListParseError = ClientHintsParseError;
pub type SecChUaFormFactorsParseError = ClientHintsParseError;
pub type PrefersReducedMotionParseError = ClientHintsParseError;
pub type PrefersReducedTransparencyParseError = ClientHintsParseError;
pub type PrefersReducedDataParseError = ClientHintsParseError;
pub type PrefersContrastParseError = ClientHintsParseError;
pub type EctParseError = ClientHintsParseError;
pub type WidthParseError = ClientHintsParseError;
pub type ViewportWidthParseError = ClientHintsParseError;
pub type SecChViewportHeightParseError = ClientHintsParseError;
pub type SecChViewportWidthParseError = ClientHintsParseError;
pub type RttParseError = ClientHintsParseError;

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

impl SecChDpr {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChDprParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChDprParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_dpr_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_dpr_decimal(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn ratio(&self) -> f64 {
    parse_sec_ch_dpr_ratio(&self.value).expect("Sec-CH-DPR values are validated at construction")
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

impl DeviceMemory {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DeviceMemoryParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DeviceMemoryParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_device_memory_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    parse_device_memory_gib(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn gib(&self) -> f64 {
    parse_device_memory_gib(&self.value)
      .expect("Device-Memory values are validated at construction")
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl PrefersColorScheme {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PrefersColorSchemeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PrefersColorSchemeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_prefers_color_scheme_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("light") {
      Ok(Self::Light)
    } else if value.eq_ignore_ascii_case("dark") {
      Ok(Self::Dark)
    } else {
      Err(invalid_prefers_color_scheme_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Light => "light",
      Self::Dark => "dark",
    }
  }
}

impl SecChUaMobile {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaMobileParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaMobileParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_mobile_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value == "?0" {
      Ok(Self::NotMobile)
    } else if value == "?1" {
      Ok(Self::Mobile)
    } else {
      Err(invalid_sec_ch_ua_mobile_value())
    }
  }

  pub const fn is_mobile(self) -> bool {
    matches!(self, Self::Mobile)
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NotMobile => "?0",
      Self::Mobile => "?1",
    }
  }
}

impl SecChUaWow64 {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaWow64ParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaWow64ParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_wow64_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value == "?0" {
      Ok(Self::NotWow64)
    } else if value == "?1" {
      Ok(Self::Wow64)
    } else {
      Err(invalid_sec_ch_ua_wow64_value())
    }
  }

  pub const fn is_wow64(self) -> bool {
    matches!(self, Self::Wow64)
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NotWow64 => "?0",
      Self::Wow64 => "?1",
    }
  }
}

impl SecChUaPlatform {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaPlatformParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaPlatformParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_platform_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_platform_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUaModel {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaModelParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaModelParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_model_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_model_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUaArch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaArchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaArchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_arch_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_arch_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUaBitness {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaBitnessParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaBitnessParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_bitness_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_bitness_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUaPlatformVersion {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaPlatformVersionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaPlatformVersionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_platform_version_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_platform_version_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUaFullVersion {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaFullVersionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaFullVersionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_sec_ch_ua_full_version_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    let value = parse_sec_ch_ua_full_version_string(value)?;
    Ok(Self { value })
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    let mut header_value = String::with_capacity(self.value.len() + 2);
    header_value.push('"');
    for character in self.value.chars() {
      if matches!(character, '"' | '\\') {
        header_value.push('\\');
      }
      header_value.push(character);
    }
    header_value.push('"');
    header_value
  }
}

impl SecChUa {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let entries = parse_sec_ch_ua_brand_list_values(
      values,
      "Sec-CH-UA",
      MAX_SEC_CH_UA_VALUE_BYTES,
      MAX_SEC_CH_UA_TOTAL_BYTES,
      MAX_SEC_CH_UA_ENTRIES,
    )?;
    Ok(Self {
      entries: entries
        .into_iter()
        .map(|(brand, version)| SecChUaEntry { brand, version })
        .collect(),
    })
  }

  pub fn entries(&self) -> &[SecChUaEntry] {
    &self.entries
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .entries
      .iter()
      .map(SecChUaEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SecChUaEntry {
  pub fn brand(&self) -> &str {
    &self.brand
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn v(&self) -> &str {
    &self.version
  }

  pub fn header_value(&self) -> String {
    format!(
      "{};v={}",
      canonical_sec_ch_ua_structured_string(&self.brand),
      canonical_sec_ch_ua_structured_string(&self.version),
    )
  }
}

impl SecChUaFullVersionList {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaFullVersionListParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaFullVersionListParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let entries = parse_sec_ch_ua_brand_list_values(
      values,
      "Sec-CH-UA-Full-Version-List",
      MAX_SEC_CH_UA_FULL_VERSION_LIST_VALUE_BYTES,
      MAX_SEC_CH_UA_FULL_VERSION_LIST_TOTAL_BYTES,
      MAX_SEC_CH_UA_FULL_VERSION_LIST_ENTRIES,
    )?;
    Ok(Self {
      entries: entries
        .into_iter()
        .map(|(brand, version)| SecChUaFullVersionListEntry { brand, version })
        .collect(),
    })
  }

  pub fn entries(&self) -> &[SecChUaFullVersionListEntry] {
    &self.entries
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .entries
      .iter()
      .map(SecChUaFullVersionListEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SecChUaFullVersionListEntry {
  pub fn brand(&self) -> &str {
    &self.brand
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn v(&self) -> &str {
    &self.version
  }

  pub fn header_value(&self) -> String {
    format!(
      "{};v={}",
      canonical_sec_ch_ua_structured_string(&self.brand),
      canonical_sec_ch_ua_structured_string(&self.version),
    )
  }
}

impl SecChUaFormFactors {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChUaFormFactorsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChUaFormFactorsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let value = values
      .next()
      .ok_or_else(invalid_sec_ch_ua_form_factors_value)?;
    let mut total_bytes = 0;
    validate_bounded_sec_ch_ua_form_factors_value(value, &mut total_bytes)?;

    let mut items = Vec::new();
    parse_sec_ch_ua_form_factors_field(value, &mut items)?;

    for value in values {
      validate_bounded_sec_ch_ua_form_factors_value(value, &mut total_bytes)?;
      parse_sec_ch_ua_form_factors_field(value, &mut items)?;
    }

    Ok(Self { items })
  }

  pub fn items(&self) -> &[String] {
    &self.items
  }

  pub fn len(&self) -> usize {
    self.items.len()
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .items
      .iter()
      .map(|item| canonical_sec_ch_ua_structured_string(item.as_str()))
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl PrefersReducedMotion {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PrefersReducedMotionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PrefersReducedMotionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_prefers_reduced_motion_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("no-preference") {
      Ok(Self::NoPreference)
    } else if value.eq_ignore_ascii_case("reduce") {
      Ok(Self::Reduce)
    } else {
      Err(invalid_prefers_reduced_motion_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NoPreference => "no-preference",
      Self::Reduce => "reduce",
    }
  }
}

impl PrefersReducedTransparency {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PrefersReducedTransparencyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PrefersReducedTransparencyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_prefers_reduced_transparency_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("no-preference") {
      Ok(Self::NoPreference)
    } else if value.eq_ignore_ascii_case("reduce") {
      Ok(Self::Reduce)
    } else {
      Err(invalid_prefers_reduced_transparency_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NoPreference => "no-preference",
      Self::Reduce => "reduce",
    }
  }
}

impl PrefersReducedData {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PrefersReducedDataParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PrefersReducedDataParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_prefers_reduced_data_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("no-preference") {
      Ok(Self::NoPreference)
    } else if value.eq_ignore_ascii_case("reduce") {
      Ok(Self::Reduce)
    } else {
      Err(invalid_prefers_reduced_data_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NoPreference => "no-preference",
      Self::Reduce => "reduce",
    }
  }
}

impl PrefersContrast {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PrefersContrastParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PrefersContrastParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_prefers_contrast_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("no-preference") {
      Ok(Self::NoPreference)
    } else if value.eq_ignore_ascii_case("more") {
      Ok(Self::More)
    } else if value.eq_ignore_ascii_case("less") {
      Ok(Self::Less)
    } else if value.eq_ignore_ascii_case("custom") {
      Ok(Self::Custom)
    } else {
      Err(invalid_prefers_contrast_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::NoPreference => "no-preference",
      Self::More => "more",
      Self::Less => "less",
      Self::Custom => "custom",
    }
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
    let value = parse_ect_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    if value.eq_ignore_ascii_case("slow-2g") {
      Ok(Self::Slow2g)
    } else if value.eq_ignore_ascii_case("2g") {
      Ok(Self::TwoG)
    } else if value.eq_ignore_ascii_case("3g") {
      Ok(Self::ThreeG)
    } else if value.eq_ignore_ascii_case("4g") {
      Ok(Self::FourG)
    } else {
      Err(invalid_ect_value())
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

impl SecChViewportHeight {
  pub const fn new(value: u64) -> Self {
    assert!(value <= MAX_SEC_CH_VIEWPORT_HEIGHT_INTEGER);
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChViewportHeightParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChViewportHeightParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_sec_ch_viewport_height_singleton(values).map(Self)
  }

  pub const fn value(self) -> u64 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

impl SecChViewportWidth {
  pub const fn new(value: u64) -> Self {
    assert!(value <= MAX_SEC_CH_VIEWPORT_WIDTH_INTEGER);
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecChViewportWidthParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecChViewportWidthParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_sec_ch_viewport_width_singleton(values).map(Self)
  }

  pub const fn value(self) -> u64 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

impl Rtt {
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, RttParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, RttParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_rtt_singleton(values).map(Self)
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
      let client_hint = member.trim_matches([' ', '\t']);
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

fn parse_sec_ch_dpr_singleton<'a, I>(values: I) -> Result<&'a str, SecChDprParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_dpr_value)?;
  validate_bounded_sec_ch_dpr_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_dpr_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-DPR header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_dpr_value(value: &str) -> Result<(), SecChDprParseError> {
  if value.len() > MAX_SEC_CH_DPR_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-DPR header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-DPR control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_dpr_decimal(value: &str) -> Result<sfv::Decimal, SecChDprParseError> {
  let item = Parser::new(value)
    .with_version(Version::Rfc8941)
    .parse::<sfv::Item>()
    .map_err(|_| invalid_sec_ch_dpr_value())?;
  if !item.params.is_empty() {
    return Err(invalid_sec_ch_dpr_value());
  }
  let BareItem::Decimal(decimal) = item.bare_item else {
    return Err(invalid_sec_ch_dpr_value());
  };
  let ratio = f64::from(decimal);
  if !ratio.is_finite() || ratio <= 0.0 {
    return Err(invalid_sec_ch_dpr_value());
  }
  Ok(decimal)
}

fn parse_sec_ch_dpr_ratio(value: &str) -> Result<f64, SecChDprParseError> {
  Ok(f64::from(parse_sec_ch_dpr_decimal(value)?))
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

fn parse_device_memory_singleton<'a, I>(values: I) -> Result<&'a str, DeviceMemoryParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_device_memory_value)?;
  validate_bounded_device_memory_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_device_memory_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Device-Memory header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_device_memory_value(value: &str) -> Result<(), DeviceMemoryParseError> {
  if value.len() > MAX_DEVICE_MEMORY_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Device-Memory header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Device-Memory control byte",
    ));
  }
  Ok(())
}

fn parse_device_memory_gib(value: &str) -> Result<f64, DeviceMemoryParseError> {
  if !matches_decimal_grammar(value) {
    return Err(invalid_device_memory_value());
  }
  let gib: f64 = value.parse().map_err(|_| invalid_device_memory_value())?;
  if !gib.is_finite() || gib < 0.0 {
    return Err(invalid_device_memory_value());
  }
  Ok(gib)
}

fn parse_prefers_color_scheme_singleton<'a, I>(
  values: I,
) -> Result<&'a str, PrefersColorSchemeParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_prefers_color_scheme_value)?;
  validate_bounded_prefers_color_scheme_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_prefers_color_scheme_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Prefers-Color-Scheme header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_prefers_color_scheme_value(
  value: &str,
) -> Result<(), PrefersColorSchemeParseError> {
  if value.len() > MAX_PREFERS_COLOR_SCHEME_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Prefers-Color-Scheme header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Color-Scheme control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_mobile_singleton<'a, I>(values: I) -> Result<&'a str, SecChUaMobileParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_mobile_value)?;
  validate_bounded_sec_ch_ua_mobile_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_mobile_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Mobile header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_mobile_value(value: &str) -> Result<(), SecChUaMobileParseError> {
  if value.len() > MAX_SEC_CH_UA_MOBILE_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Mobile header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Mobile control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_wow64_singleton<'a, I>(values: I) -> Result<&'a str, SecChUaWow64ParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_wow64_value)?;
  validate_bounded_sec_ch_ua_wow64_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_wow64_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-WoW64 header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_wow64_value(value: &str) -> Result<(), SecChUaWow64ParseError> {
  if value.len() > MAX_SEC_CH_UA_WOW64_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-WoW64 header value is too large",
    ));
  }
  if !value.is_ascii() {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-WoW64 non-ASCII byte",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-WoW64 control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_platform_singleton<'a, I>(
  values: I,
) -> Result<&'a str, SecChUaPlatformParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_platform_value)?;
  validate_bounded_sec_ch_ua_platform_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_platform_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Platform header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_platform_value(value: &str) -> Result<(), SecChUaPlatformParseError> {
  if value.len() > MAX_SEC_CH_UA_PLATFORM_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Platform header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Platform control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_platform_string(value: &str) -> Result<String, SecChUaPlatformParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_platform_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_platform_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_platform_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_platform_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_model_singleton<'a, I>(values: I) -> Result<&'a str, SecChUaModelParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_model_value)?;
  validate_bounded_sec_ch_ua_model_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_model_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Model header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_model_value(value: &str) -> Result<(), SecChUaModelParseError> {
  if value.len() > MAX_SEC_CH_UA_MODEL_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Model header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Model control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_model_string(value: &str) -> Result<String, SecChUaModelParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_model_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_model_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_model_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_model_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_arch_singleton<'a, I>(values: I) -> Result<&'a str, SecChUaArchParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_arch_value)?;
  validate_bounded_sec_ch_ua_arch_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_arch_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Arch header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_arch_value(value: &str) -> Result<(), SecChUaArchParseError> {
  if value.len() > MAX_SEC_CH_UA_ARCH_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Arch header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Arch control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_arch_string(value: &str) -> Result<String, SecChUaArchParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_arch_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_arch_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_arch_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_arch_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_bitness_singleton<'a, I>(values: I) -> Result<&'a str, SecChUaBitnessParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_sec_ch_ua_bitness_value)?;
  validate_bounded_sec_ch_ua_bitness_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_bitness_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Bitness header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_bitness_value(value: &str) -> Result<(), SecChUaBitnessParseError> {
  if value.len() > MAX_SEC_CH_UA_BITNESS_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Bitness header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Bitness control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_bitness_string(value: &str) -> Result<String, SecChUaBitnessParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_bitness_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_bitness_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_bitness_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_bitness_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_platform_version_singleton<'a, I>(
  values: I,
) -> Result<&'a str, SecChUaPlatformVersionParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_sec_ch_ua_platform_version_value)?;
  validate_bounded_sec_ch_ua_platform_version_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_platform_version_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Platform-Version header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_platform_version_value(
  value: &str,
) -> Result<(), SecChUaPlatformVersionParseError> {
  if value.len() > MAX_SEC_CH_UA_PLATFORM_VERSION_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Platform-Version header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Platform-Version control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_platform_version_string(
  value: &str,
) -> Result<String, SecChUaPlatformVersionParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_platform_version_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_platform_version_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_platform_version_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_platform_version_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  if parsed.is_empty() {
    return Err(invalid_sec_ch_ua_platform_version_value());
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_full_version_singleton<'a, I>(
  values: I,
) -> Result<&'a str, SecChUaFullVersionParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_sec_ch_ua_full_version_value)?;
  validate_bounded_sec_ch_ua_full_version_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_full_version_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-UA-Full-Version header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_sec_ch_ua_full_version_value(
  value: &str,
) -> Result<(), SecChUaFullVersionParseError> {
  if value.len() > MAX_SEC_CH_UA_FULL_VERSION_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Full-Version header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Full-Version control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_full_version_string(
  value: &str,
) -> Result<String, SecChUaFullVersionParseError> {
  let characters: Vec<char> = value.chars().collect();
  if characters.len() < 2 || characters[0] != '"' || characters[characters.len() - 1] != '"' {
    return Err(invalid_sec_ch_ua_full_version_value());
  }

  let mut parsed = String::with_capacity(value.len() - 2);
  let mut index = 1;
  while index < characters.len() - 1 {
    match characters[index] {
      '\\' => {
        index += 1;
        if index >= characters.len() - 1 || !matches!(characters[index], '"' | '\\') {
          return Err(invalid_sec_ch_ua_full_version_value());
        }
        parsed.push(characters[index]);
      }
      '"' => return Err(invalid_sec_ch_ua_full_version_value()),
      character if !character.is_ascii() || character.is_ascii_control() => {
        return Err(invalid_sec_ch_ua_full_version_value());
      }
      character => parsed.push(character),
    }
    index += 1;
  }
  Ok(parsed)
}

fn parse_sec_ch_ua_brand_list_values<'a, I>(
  values: I,
  header_name: &str,
  value_limit: usize,
  total_limit: usize,
  entry_limit: usize,
) -> Result<Vec<(String, String)>, ClientHintsParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(|| invalid_sec_ch_ua_brand_list_value(header_name))?;
  let mut total_bytes = 0;
  validate_bounded_sec_ch_ua_brand_list_value(
    value,
    header_name,
    value_limit,
    total_limit,
    &mut total_bytes,
  )?;

  let mut entries = Vec::new();
  parse_sec_ch_ua_brand_list_field(value, header_name, entry_limit, &mut entries)?;

  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_ua_brand_list_value(
      value,
      header_name,
      value_limit,
      total_limit,
      &mut total_bytes,
    )?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(format!(
      "duplicate {header_name} header fields"
    )));
  }

  Ok(entries)
}

fn validate_bounded_sec_ch_ua_brand_list_value(
  value: &str,
  header_name: &str,
  value_limit: usize,
  total_limit: usize,
  total_bytes: &mut usize,
) -> Result<(), ClientHintsParseError> {
  if value.len() > value_limit {
    return Err(ClientHintsParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  *total_bytes = total_bytes.saturating_add(value.len());
  if *total_bytes > total_limit {
    return Err(ClientHintsParseError::new(format!(
      "{header_name} header list is too large"
    )));
  }
  if value
    .bytes()
    .any(|byte| byte > 0x7f || (byte.is_ascii_control() && !matches!(byte, b' ' | b'\t')))
  {
    return Err(ClientHintsParseError::new(format!(
      "invalid {header_name} control or non-ASCII byte"
    )));
  }
  Ok(())
}

fn parse_sec_ch_ua_brand_list_field(
  value: &str,
  header_name: &str,
  entry_limit: usize,
  entries: &mut Vec<(String, String)>,
) -> Result<(), ClientHintsParseError> {
  let value = value.trim_matches([' ', '\t']);
  let list = Parser::new(value)
    .with_version(Version::Rfc8941)
    .parse::<List>()
    .map_err(|_| invalid_sec_ch_ua_brand_list_value(header_name))?;
  if list.is_empty() {
    return Err(invalid_sec_ch_ua_brand_list_value(header_name));
  }
  reject_duplicate_sec_ch_ua_brand_list_parameters(value, header_name)?;

  for entry in list {
    if entries.len() >= entry_limit {
      return Err(ClientHintsParseError::new(format!(
        "too many {header_name} entries"
      )));
    }
    let ListEntry::Item(item) = entry else {
      return Err(invalid_sec_ch_ua_brand_list_value(header_name));
    };
    let BareItem::String(brand) = item.bare_item else {
      return Err(invalid_sec_ch_ua_brand_list_value(header_name));
    };
    if item.params.len() != 1 {
      return Err(invalid_sec_ch_ua_brand_list_value(header_name));
    }
    let Some(BareItem::String(version)) = item.params.get("v") else {
      return Err(invalid_sec_ch_ua_brand_list_value(header_name));
    };
    entries.push((brand.as_str().to_owned(), version.as_str().to_owned()));
  }
  Ok(())
}

fn reject_duplicate_sec_ch_ua_brand_list_parameters(
  value: &str,
  header_name: &str,
) -> Result<(), ClientHintsParseError> {
  let bytes = value.as_bytes();
  let mut in_string = false;
  let mut escaped = false;
  let mut version_seen = false;
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    if in_string {
      if escaped {
        escaped = false;
      } else if byte == b'\\' {
        escaped = true;
      } else if byte == b'"' {
        in_string = false;
      }
      index += 1;
      continue;
    }
    match byte {
      b'"' => in_string = true,
      b',' => version_seen = false,
      b';' => {
        index += 1;
        while matches!(bytes.get(index), Some(b' ')) {
          index += 1;
        }
        let start = index;
        while matches!(
          bytes.get(index),
          Some(b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
        ) {
          index += 1;
        }
        if &bytes[start..index] == b"v" {
          if version_seen {
            return Err(ClientHintsParseError::new(format!(
              "duplicate {header_name} v parameter"
            )));
          }
          version_seen = true;
        }
        continue;
      }
      _ => {}
    }
    index += 1;
  }
  Ok(())
}

fn canonical_sec_ch_ua_structured_string(value: &str) -> String {
  let mut result = String::with_capacity(value.len() + 2);
  result.push('"');
  for character in value.chars() {
    if matches!(character, '"' | '\\') {
      result.push('\\');
    }
    result.push(character);
  }
  result.push('"');
  result
}

fn validate_bounded_sec_ch_ua_form_factors_value(
  value: &str,
  total_bytes: &mut usize,
) -> Result<(), SecChUaFormFactorsParseError> {
  if value.len() > MAX_SEC_CH_UA_FORM_FACTORS_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Form-Factors header value is too large",
    ));
  }
  *total_bytes = total_bytes.saturating_add(value.len());
  if *total_bytes > MAX_SEC_CH_UA_FORM_FACTORS_TOTAL_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-UA-Form-Factors header list is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte > 0x7f || (byte.is_ascii_control() && !matches!(byte, b' ' | b'\t')))
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-UA-Form-Factors control or non-ASCII byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_ua_form_factors_field(
  value: &str,
  items: &mut Vec<String>,
) -> Result<(), SecChUaFormFactorsParseError> {
  let value = value.trim_matches([' ', '\t']);
  let list = Parser::new(value)
    .with_version(Version::Rfc8941)
    .parse::<List>()
    .map_err(|_| invalid_sec_ch_ua_form_factors_value())?;
  if list.is_empty() {
    return Err(invalid_sec_ch_ua_form_factors_value());
  }

  for entry in list {
    if items.len() >= MAX_SEC_CH_UA_FORM_FACTORS_ITEMS {
      return Err(ClientHintsParseError::new(
        "too many Sec-CH-UA-Form-Factors items",
      ));
    }
    let ListEntry::Item(item) = entry else {
      return Err(invalid_sec_ch_ua_form_factors_value());
    };
    let BareItem::String(form_factor) = item.bare_item else {
      return Err(invalid_sec_ch_ua_form_factors_value());
    };
    if !item.params.is_empty() {
      return Err(invalid_sec_ch_ua_form_factors_value());
    }
    items.push(form_factor.as_str().to_owned());
  }
  Ok(())
}

fn parse_prefers_reduced_motion_singleton<'a, I>(
  values: I,
) -> Result<&'a str, PrefersReducedMotionParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_prefers_reduced_motion_value)?;
  validate_bounded_prefers_reduced_motion_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_prefers_reduced_motion_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Prefers-Reduced-Motion header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_prefers_reduced_motion_value(
  value: &str,
) -> Result<(), PrefersReducedMotionParseError> {
  if value.len() > MAX_PREFERS_REDUCED_MOTION_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Prefers-Reduced-Motion header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Reduced-Motion control byte",
    ));
  }
  Ok(())
}

fn parse_prefers_reduced_transparency_singleton<'a, I>(
  values: I,
) -> Result<&'a str, PrefersReducedTransparencyParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_prefers_reduced_transparency_value)?;
  validate_bounded_prefers_reduced_transparency_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_prefers_reduced_transparency_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Prefers-Reduced-Transparency header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_prefers_reduced_transparency_value(
  value: &str,
) -> Result<(), PrefersReducedTransparencyParseError> {
  if value.len() > MAX_PREFERS_REDUCED_TRANSPARENCY_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Prefers-Reduced-Transparency header value is too large",
    ));
  }
  if !value.is_ascii() {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Reduced-Transparency non-ASCII byte",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Reduced-Transparency control byte",
    ));
  }
  Ok(())
}

fn parse_prefers_reduced_data_singleton<'a, I>(
  values: I,
) -> Result<&'a str, PrefersReducedDataParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_prefers_reduced_data_value)?;
  validate_bounded_prefers_reduced_data_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_prefers_reduced_data_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Prefers-Reduced-Data header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_prefers_reduced_data_value(
  value: &str,
) -> Result<(), PrefersReducedDataParseError> {
  if value.len() > MAX_PREFERS_REDUCED_DATA_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Prefers-Reduced-Data header value is too large",
    ));
  }
  if !value.is_ascii() {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Reduced-Data non-ASCII byte",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Reduced-Data control byte",
    ));
  }
  Ok(())
}

fn parse_prefers_contrast_singleton<'a, I>(values: I) -> Result<&'a str, PrefersContrastParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_prefers_contrast_value)?;
  validate_bounded_prefers_contrast_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_prefers_contrast_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Prefers-Contrast header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_prefers_contrast_value(value: &str) -> Result<(), PrefersContrastParseError> {
  if value.len() > MAX_PREFERS_CONTRAST_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Prefers-Contrast header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Prefers-Contrast control byte",
    ));
  }
  Ok(())
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

fn parse_sec_ch_viewport_height_singleton<'a, I>(
  values: I,
) -> Result<u64, SecChViewportHeightParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_sec_ch_viewport_height_value)?;
  validate_bounded_sec_ch_viewport_height_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_viewport_height_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Viewport-Height header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty()
    || value.len() > MAX_SEC_CH_VIEWPORT_HEIGHT_INTEGER_DIGITS
    || !value.bytes().all(|byte| byte.is_ascii_digit())
  {
    return Err(invalid_sec_ch_viewport_height_value());
  }
  let value = value
    .parse::<u64>()
    .map_err(|_| invalid_sec_ch_viewport_height_value())?;
  if value > MAX_SEC_CH_VIEWPORT_HEIGHT_INTEGER {
    return Err(invalid_sec_ch_viewport_height_value());
  }
  Ok(value)
}

fn validate_bounded_sec_ch_viewport_height_value(
  value: &str,
) -> Result<(), SecChViewportHeightParseError> {
  if value.len() > MAX_SEC_CH_VIEWPORT_HEIGHT_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Viewport-Height header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Viewport-Height control byte",
    ));
  }
  Ok(())
}

fn parse_sec_ch_viewport_width_singleton<'a, I>(
  values: I,
) -> Result<u64, SecChViewportWidthParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values
    .next()
    .ok_or_else(invalid_sec_ch_viewport_width_value)?;
  validate_bounded_sec_ch_viewport_width_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_sec_ch_viewport_width_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new(
      "duplicate Sec-CH-Viewport-Width header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty()
    || value.len() > MAX_SEC_CH_VIEWPORT_WIDTH_INTEGER_DIGITS
    || !value.bytes().all(|byte| byte.is_ascii_digit())
  {
    return Err(invalid_sec_ch_viewport_width_value());
  }
  let value = value
    .parse::<u64>()
    .map_err(|_| invalid_sec_ch_viewport_width_value())?;
  if value > MAX_SEC_CH_VIEWPORT_WIDTH_INTEGER {
    return Err(invalid_sec_ch_viewport_width_value());
  }
  Ok(value)
}

fn validate_bounded_sec_ch_viewport_width_value(
  value: &str,
) -> Result<(), SecChViewportWidthParseError> {
  if value.len() > MAX_SEC_CH_VIEWPORT_WIDTH_VALUE_BYTES {
    return Err(ClientHintsParseError::new(
      "Sec-CH-Viewport-Width header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new(
      "invalid Sec-CH-Viewport-Width control byte",
    ));
  }
  Ok(())
}

fn parse_rtt_singleton<'a, I>(values: I) -> Result<u64, RttParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_rtt_value)?;
  validate_bounded_rtt_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_rtt_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new("duplicate RTT header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_rtt_value());
  }
  value.parse().map_err(|_| invalid_rtt_value())
}

fn validate_bounded_rtt_value(value: &str) -> Result<(), RttParseError> {
  if value.len() > MAX_RTT_VALUE_BYTES {
    return Err(ClientHintsParseError::new("RTT header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid RTT control byte"));
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

fn invalid_sec_ch_dpr_value() -> SecChDprParseError {
  ClientHintsParseError::new("invalid Sec-CH-DPR header value")
}

fn invalid_downlink_value() -> DownlinkParseError {
  ClientHintsParseError::new("invalid Downlink header value")
}

fn invalid_device_memory_value() -> DeviceMemoryParseError {
  ClientHintsParseError::new("invalid Device-Memory header value")
}

fn invalid_prefers_color_scheme_value() -> PrefersColorSchemeParseError {
  ClientHintsParseError::new("invalid Sec-CH-Prefers-Color-Scheme header value")
}

fn invalid_sec_ch_ua_mobile_value() -> SecChUaMobileParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Mobile header value")
}

fn invalid_sec_ch_ua_wow64_value() -> SecChUaWow64ParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-WoW64 header value")
}

fn invalid_sec_ch_ua_platform_value() -> SecChUaPlatformParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Platform header value")
}

fn invalid_sec_ch_ua_model_value() -> SecChUaModelParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Model header value")
}

fn invalid_sec_ch_ua_arch_value() -> SecChUaArchParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Arch header value")
}

fn invalid_sec_ch_ua_bitness_value() -> SecChUaBitnessParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Bitness header value")
}

fn invalid_sec_ch_ua_platform_version_value() -> SecChUaPlatformVersionParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Platform-Version header value")
}

fn invalid_sec_ch_ua_full_version_value() -> SecChUaFullVersionParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Full-Version header value")
}

fn invalid_sec_ch_ua_brand_list_value(header_name: &str) -> ClientHintsParseError {
  ClientHintsParseError::new(format!("invalid {header_name} header value"))
}

fn invalid_sec_ch_ua_form_factors_value() -> SecChUaFormFactorsParseError {
  ClientHintsParseError::new("invalid Sec-CH-UA-Form-Factors header value")
}

fn invalid_prefers_reduced_motion_value() -> PrefersReducedMotionParseError {
  ClientHintsParseError::new("invalid Sec-CH-Prefers-Reduced-Motion header value")
}

fn invalid_prefers_reduced_transparency_value() -> PrefersReducedTransparencyParseError {
  ClientHintsParseError::new("invalid Sec-CH-Prefers-Reduced-Transparency header value")
}

fn invalid_prefers_reduced_data_value() -> PrefersReducedDataParseError {
  ClientHintsParseError::new("invalid Sec-CH-Prefers-Reduced-Data header value")
}

fn invalid_prefers_contrast_value() -> PrefersContrastParseError {
  ClientHintsParseError::new("invalid Sec-CH-Prefers-Contrast header value")
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

fn invalid_sec_ch_viewport_height_value() -> SecChViewportHeightParseError {
  ClientHintsParseError::new("invalid Sec-CH-Viewport-Height header value")
}

fn invalid_sec_ch_viewport_width_value() -> SecChViewportWidthParseError {
  ClientHintsParseError::new("invalid Sec-CH-Viewport-Width header value")
}

fn invalid_rtt_value() -> RttParseError {
  ClientHintsParseError::new("invalid RTT header value")
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
