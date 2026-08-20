pub(crate) const MAX_CSP_POLICY_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_CSP_POLICY_FIELDS: usize = 256;

pub(crate) fn parse_csp_policy_values<'a, I, E, F>(
  values: I,
  header_name: &str,
  mut error: F,
) -> Result<Vec<String>, E>
where
  I: IntoIterator<Item = &'a str>,
  F: FnMut(String) -> E,
{
  let mut policies = Vec::new();
  for value in values {
    if policies.len() == MAX_CSP_POLICY_FIELDS {
      return Err(error(format!("too many {header_name} header values")));
    }
    validate_csp_policy_value(value, header_name, &mut error)?;
    policies.push(value.to_owned());
  }
  if policies.is_empty() {
    return Err(error(format!("invalid {header_name} header value")));
  }
  Ok(policies)
}

fn validate_csp_policy_value<E, F>(value: &str, header_name: &str, error: &mut F) -> Result<(), E>
where
  F: FnMut(String) -> E,
{
  if value.is_empty() {
    return Err(error(format!("invalid {header_name} header value")));
  }
  if value.len() > MAX_CSP_POLICY_VALUE_BYTES {
    return Err(error(format!("{header_name} header value is too large")));
  }
  if value
    .bytes()
    .any(|byte| byte != b'\t' && (byte <= 0x1f || byte == 0x7f))
  {
    return Err(error(format!("invalid {header_name} control byte")));
  }
  Ok(())
}
