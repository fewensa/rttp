use rttp_protocol::www_authenticate::WwwAuthenticate;

#[test]
fn www_authenticate_combines_repeated_fields_before_parsing() {
  let challenges = WwwAuthenticate::parse_values(["Digest realm=corp", "nonce=abc"])
    .expect("repeated WWW-Authenticate fields should combine before parsing");

  assert_eq!(challenges.len(), 1);
  let digest = &challenges.challenges()[0];
  assert_eq!(digest.scheme(), "Digest");
  assert_eq!(digest.parameter("realm"), Some("corp"));
  assert_eq!(digest.parameter("nonce"), Some("abc"));
}
