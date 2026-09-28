use rttp_protocol::service_worker_allowed::{
  ServiceWorkerAllowed, MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES,
};

#[test]
fn service_worker_allowed_accepts_origin_relative_and_absolute_paths() {
  for (value, expected) in [
    ("/", "/"),
    ("/app/", "/app/"),
    ("/static/sw/", "/static/sw/"),
    ("/scope?feature=1", "/scope?feature=1"),
    ("/scope#section", "/scope#section"),
    ("./", "./"),
    ("../scope", "../scope"),
    ("scope/nested", "scope/nested"),
    ("\t/\t", "/"),
    (" /static/sw/ ", "/static/sw/"),
  ] {
    let allowed = ServiceWorkerAllowed::parse(value).expect("Service-Worker-Allowed should parse");

    assert_eq!(expected, allowed.as_str());
    assert_eq!(expected, allowed.header_value());
    assert_eq!(expected, allowed.as_ref());
    assert_eq!(expected, allowed.to_string());
  }
}

#[test]
fn service_worker_allowed_accepts_uri_reference_percent_encoding_and_empty_components() {
  for value in [
    "/scope%20with%20spaces/",
    "/scope%2Fchild?feature=%26%3F#section%23two",
    "/scope?",
    "/scope#",
    "./nested:scope",
    "../scope;v=1",
  ] {
    let allowed = ServiceWorkerAllowed::parse(value).expect("valid URI reference should parse");
    assert_eq!(value, allowed.as_str());
    assert_eq!(value, allowed.header_value());
    assert_eq!(
      value,
      ServiceWorkerAllowed::parse(allowed.header_value())
        .unwrap()
        .as_str()
    );
  }
}

#[test]
fn service_worker_allowed_parse_values_enforces_singleton_fields() {
  let allowed = ServiceWorkerAllowed::parse_values([" / "]).expect("single field should parse");

  assert_eq!("/", allowed.as_str());
  assert!(
    ServiceWorkerAllowed::parse_values(["/", "/app/"]).is_err(),
    "duplicate fields must be rejected"
  );
  assert!(
    ServiceWorkerAllowed::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn service_worker_allowed_rejects_invalid_percent_encoding_fragments_and_controls() {
  for value in [
    "/bad%",
    "/bad%0",
    "/bad%gg",
    "/bad#one#two",
    "/bad\r\nInjected: yes",
    "/bad\tpath",
    "/bad\u{000b}path",
    "/bad\u{007f}path",
  ] {
    assert!(
      ServiceWorkerAllowed::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn service_worker_allowed_rejects_malformed_injection_and_authority_values() {
  for value in [
    "",
    " ",
    "/bad path",
    "/bad%zz",
    "/bad\\path",
    "/bad<path>",
    "/bad\r\nInjected: yes",
    "/bad\tpath",
    "/bad#one#two",
    "http://example.test/scope",
    "https://example.test/scope",
    "//example.test/scope",
    "/safe\u{7f}",
    "/safe\u{1f}",
    "/bad\"path",
  ] {
    assert!(
      ServiceWorkerAllowed::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn service_worker_allowed_enforces_value_bounds() {
  let at_limit = format!(
    "/{}",
    "a".repeat(MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES - 1)
  );
  let parsed = ServiceWorkerAllowed::parse(&at_limit).expect("value at limit should parse");
  assert_eq!(at_limit, parsed.as_str());

  assert!(
    ServiceWorkerAllowed::parse(format!(
      "/{}",
      "a".repeat(MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES)
    ))
    .is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = format!("/{}", "a".repeat(MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES));
  assert!(
    ServiceWorkerAllowed::parse_values(["/valid", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
