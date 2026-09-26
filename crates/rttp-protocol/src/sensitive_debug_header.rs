/// Returns whether a header value should be redacted in debug output.
///
/// Header names are compared case-insensitively after removing only HTTP
/// optional whitespace (SP and HTAB) from either end.
pub fn is_sensitive_debug_header(name: &str) -> bool {
  let name = name.trim_matches([' ', '\t']);
  name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("cookie")
    || name.eq_ignore_ascii_case("idempotency-key")
    || name.eq_ignore_ascii_case("if")
    || name.eq_ignore_ascii_case("lock-token")
    || name.eq_ignore_ascii_case("origin-trial")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("sec-websocket-accept")
    || name.eq_ignore_ascii_case("sec-websocket-key")
    || name.eq_ignore_ascii_case("set-cookie")
    || name.eq_ignore_ascii_case("speculation-rules")
    || name.eq_ignore_ascii_case("traceparent")
    || name.eq_ignore_ascii_case("tracestate")
    || name.eq_ignore_ascii_case("baggage")
}

#[cfg(test)]
mod tests {
  use super::is_sensitive_debug_header;

  #[test]
  fn matches_sensitive_names_case_insensitively_with_http_ows_only() {
    for name in [
      " Authorization",
      "AUTHORIZATION ",
      "\tOrigin-Trial\t",
      "Speculation-Rules",
    ] {
      assert!(is_sensitive_debug_header(name));
    }
    assert!(!is_sensitive_debug_header(" Accept "));
    assert!(!is_sensitive_debug_header("\u{2003}Authorization"));
  }
}
