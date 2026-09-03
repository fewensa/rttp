use std::ops::Deref;
use std::slice;

use rttp_protocol::range::{ByteRangeSpec, Range, RangeParseError, MAX_RANGE_COUNT};

use super::response::{HttpByteRange, HttpByteRangeError};

/// Satisfiable `bytes` ranges resolved from one `Range` field.
///
/// Invariant: the set contains at least one member and at most
/// [`Self::MAX_RANGES`] (32) members, stored in wire order. Unsatisfiable
/// members are omitted during resolution; this type is never empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpByteRangeSet {
  ranges: Vec<HttpByteRange>,
}

impl HttpByteRangeSet {
  /// Maximum number of members retained from one `Range` field.
  ///
  /// This cap matches the protocol `Range` parser bound (`MAX_RANGE_COUNT`).
  pub const MAX_RANGES: usize = MAX_RANGE_COUNT;

  /// Creates a resolved range set.
  ///
  /// # Panics
  ///
  /// Panics if `ranges` is empty or contains more than [`Self::MAX_RANGES`]
  /// members.
  pub fn new(ranges: impl Into<Vec<HttpByteRange>>) -> Self {
    let ranges = ranges.into();
    assert!(
      !ranges.is_empty(),
      "resolved byte range set must not be empty"
    );
    assert!(
      ranges.len() <= Self::MAX_RANGES,
      "resolved byte range set exceeds {} members",
      Self::MAX_RANGES
    );
    Self { ranges }
  }

  /// Parses and resolves one `Range` field against a representation length.
  ///
  /// Satisfiable closed, open-ended, and suffix members are retained in wire
  /// order. Unsatisfiable members are omitted. If every member is
  /// unsatisfiable, [`HttpByteRangeError::UnsatisfiedRange`] is returned.
  pub fn parse<S: AsRef<str>>(
    range_header: S,
    entity_length: usize,
  ) -> Result<Self, HttpByteRangeError> {
    match Self::parse_values([range_header.as_ref()], entity_length)? {
      Some(ranges) => Ok(ranges),
      None => Err(HttpByteRangeError::InvalidRange),
    }
  }

  /// Parses `Range` field values and resolves satisfiable members.
  ///
  /// Absence yields `Ok(None)`. Repeated `Range` fields yield
  /// [`HttpByteRangeError::MultipleRanges`].
  pub fn parse_values<'a, I>(
    values: I,
    entity_length: usize,
  ) -> Result<Option<Self>, HttpByteRangeError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_range_values(values, entity_length)
  }

  pub fn ranges(&self) -> &[HttpByteRange] {
    &self.ranges
  }

  pub fn len(&self) -> usize {
    self.ranges.len()
  }

  pub fn is_empty(&self) -> bool {
    self.ranges.is_empty()
  }

  pub fn iter(&self) -> slice::Iter<'_, HttpByteRange> {
    self.ranges.iter()
  }

  fn from_resolved(ranges: Vec<HttpByteRange>) -> Result<Self, HttpByteRangeError> {
    if ranges.is_empty() {
      return Err(HttpByteRangeError::UnsatisfiedRange);
    }
    debug_assert!(ranges.len() <= Self::MAX_RANGES);
    Ok(Self { ranges })
  }
}

impl Deref for HttpByteRangeSet {
  type Target = [HttpByteRange];

  fn deref(&self) -> &[HttpByteRange] {
    &self.ranges
  }
}

impl<'a> IntoIterator for &'a HttpByteRangeSet {
  type Item = &'a HttpByteRange;
  type IntoIter = slice::Iter<'a, HttpByteRange>;

  fn into_iter(self) -> Self::IntoIter {
    self.ranges.iter()
  }
}

impl IntoIterator for HttpByteRangeSet {
  type Item = HttpByteRange;
  type IntoIter = std::vec::IntoIter<HttpByteRange>;

  fn into_iter(self) -> Self::IntoIter {
    self.ranges.into_iter()
  }
}

fn parse_range_values<'a, I>(
  values: I,
  entity_length: usize,
) -> Result<Option<HttpByteRangeSet>, HttpByteRangeError>
where
  I: IntoIterator<Item = &'a str>,
{
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  if values.len() > 1 {
    return Err(HttpByteRangeError::MultipleRanges);
  }

  let value = values[0].trim();
  if let Some((unit, _)) = value.split_once('=') {
    if !unit.trim().eq_ignore_ascii_case("bytes") {
      return Err(HttpByteRangeError::UnsupportedUnit);
    }
  }

  let parsed = Range::parse_values(values).map_err(map_range_parse_error)?;
  let mut resolved = Vec::new();
  for spec in parsed.ranges() {
    match spec {
      ByteRangeSpec::FromTo { start, end } => {
        if let Some(range) = resolve_from_to(*start, *end, entity_length) {
          resolved.push(range);
        }
      }
      ByteRangeSpec::Suffix { length } => {
        if let Some(range) = resolve_suffix(*length, entity_length) {
          resolved.push(range);
        }
      }
    }
  }
  HttpByteRangeSet::from_resolved(resolved).map(Some)
}

fn map_range_parse_error(error: RangeParseError) -> HttpByteRangeError {
  if error.to_string() == "too many Range members" {
    HttpByteRangeError::MultipleRanges
  } else {
    HttpByteRangeError::InvalidRange
  }
}

fn resolve_from_to(start: u64, end: Option<u64>, entity_length: usize) -> Option<HttpByteRange> {
  let start = usize::try_from(start).ok()?;
  if start >= entity_length {
    return None;
  }
  let last = entity_length.checked_sub(1)?;
  let end = match end {
    None => last,
    Some(end) => usize::try_from(end).unwrap_or(usize::MAX).min(last),
  };
  Some(HttpByteRange::new(start, end))
}

fn resolve_suffix(length: u64, entity_length: usize) -> Option<HttpByteRange> {
  if length == 0 {
    return None;
  }
  let last = entity_length.checked_sub(1)?;
  let start = match usize::try_from(length) {
    Ok(suffix) => entity_length.saturating_sub(suffix),
    Err(_) => 0,
  };
  Some(HttpByteRange::new(start, last))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(header: &str, entity_length: usize) -> Result<HttpByteRangeSet, HttpByteRangeError> {
    HttpByteRangeSet::parse(header, entity_length)
  }

  fn ranges(header: &str, entity_length: usize) -> Vec<HttpByteRange> {
    parse(header, entity_length)
      .expect("range set should resolve")
      .ranges()
      .to_vec()
  }

  #[test]
  fn resolves_mixed_closed_open_and_suffix_members_in_wire_order() {
    assert_eq!(
      vec![
        HttpByteRange::new(0, 1),
        HttpByteRange::new(5, 9),
        HttpByteRange::new(7, 9),
      ],
      ranges("bytes=0-1,5-,-3", 10)
    );
  }

  #[test]
  fn keeps_satisfiable_members_when_others_are_unsatisfiable() {
    assert_eq!(
      vec![HttpByteRange::new(0, 1), HttpByteRange::new(4, 5)],
      ranges("bytes=0-1,99-100,4-5", 10)
    );
  }

  #[test]
  fn returns_unsatisfied_when_every_member_is_outside_the_representation() {
    assert_eq!(
      Err(HttpByteRangeError::UnsatisfiedRange),
      parse("bytes=99-100,200-", 10)
    );
  }

  #[test]
  fn zero_length_representation_is_unsatisfiable_for_closed_open_and_suffix() {
    for header in ["bytes=0-0", "bytes=0-", "bytes=-1", "bytes=0-1,5-,-3"] {
      assert_eq!(
        Err(HttpByteRangeError::UnsatisfiedRange),
        parse(header, 0),
        "{header}"
      );
    }
  }

  #[test]
  fn accepts_thirty_two_members_and_rejects_thirty_three() {
    let allowed = (0..HttpByteRangeSet::MAX_RANGES)
      .map(|index| format!("{index}-{index}"))
      .collect::<Vec<_>>()
      .join(",");
    let set = parse(&format!("bytes={allowed}"), HttpByteRangeSet::MAX_RANGES)
      .expect("32 members should resolve");
    assert_eq!(HttpByteRangeSet::MAX_RANGES, set.len());
    assert_eq!(HttpByteRange::new(0, 0), set.ranges()[0]);
    assert_eq!(HttpByteRange::new(31, 31), set.ranges()[31]);

    let rejected = (0..=HttpByteRangeSet::MAX_RANGES)
      .map(|index| format!("{index}-{index}"))
      .collect::<Vec<_>>()
      .join(",");
    assert_eq!(
      Err(HttpByteRangeError::MultipleRanges),
      parse(
        &format!("bytes={rejected}"),
        HttpByteRangeSet::MAX_RANGES + 1
      )
    );
  }

  #[test]
  fn clips_u64_end_overflow_and_skips_start_beyond_usize() {
    assert_eq!(
      vec![HttpByteRange::new(0, 9)],
      ranges("bytes=0-18446744073709551615", 10)
    );
    assert_eq!(
      Err(HttpByteRangeError::UnsatisfiedRange),
      parse("bytes=18446744073709551615-", 10)
    );
    assert_eq!(
      vec![HttpByteRange::new(0, 1)],
      ranges("bytes=0-1,18446744073709551615-", 10)
    );
    assert_eq!(
      vec![HttpByteRange::new(0, 9)],
      ranges("bytes=-18446744073709551615", 10)
    );
  }

  #[test]
  fn duplicate_range_fields_are_rejected() {
    assert_eq!(
      Err(HttpByteRangeError::MultipleRanges),
      parse_range_values(["bytes=0-1", "bytes=2-3"], 10)
    );
  }

  #[test]
  fn legacy_single_closed_open_and_suffix_forms_still_resolve() {
    assert_eq!(vec![HttpByteRange::new(2, 5)], ranges("bytes=2-5", 10));
    assert_eq!(vec![HttpByteRange::new(7, 9)], ranges("bytes=7-", 10));
    assert_eq!(vec![HttpByteRange::new(6, 9)], ranges("bytes=-4", 10));
  }

  #[test]
  fn preserves_unsupported_and_malformed_errors() {
    assert_eq!(
      Err(HttpByteRangeError::UnsupportedUnit),
      parse("items=0-1", 10)
    );
    assert_eq!(
      Err(HttpByteRangeError::InvalidRange),
      parse("bytes=5-2", 10)
    );
  }

  #[test]
  fn zero_suffix_is_unsatisfiable_and_does_not_drop_other_members() {
    assert_eq!(
      Err(HttpByteRangeError::UnsatisfiedRange),
      parse("bytes=-0", 10)
    );
    assert_eq!(vec![HttpByteRange::new(0, 1)], ranges("bytes=0-1,-0", 10));
  }
}
