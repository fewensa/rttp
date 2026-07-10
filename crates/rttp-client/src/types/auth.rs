use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// HTTP authentication type for use with [`rttp_client::HttpClient::auth`].
///
/// # Examples
///
/// ```rust
/// use rttp_client::types::Auth;
///
/// let basic = Auth::basic("user", "password");
/// let bearer = Auth::bearer("my-token");
/// ```
#[derive(Clone, Debug)]
pub enum Auth {
  /// HTTP Basic authentication (`Authorization: Basic <base64(user:pass)>`).
  Basic { username: String, password: String },
  /// Bearer token authentication (`Authorization: Bearer <token>`).
  Bearer { token: String },
}

impl Auth {
  /// Create a Basic auth credential.
  pub fn basic<U: AsRef<str>, P: AsRef<str>>(username: U, password: P) -> Self {
    Auth::Basic {
      username: username.as_ref().to_string(),
      password: password.as_ref().to_string(),
    }
  }

  /// Create a Bearer token credential.
  pub fn bearer<T: AsRef<str>>(token: T) -> Self {
    Auth::Bearer {
      token: token.as_ref().to_string(),
    }
  }

  /// Return the `Authorization` header value for this credential.
  pub fn header_value(&self) -> String {
    match self {
      Auth::Basic { username, password } => {
        let encoded = STANDARD.encode(format!("{}:{}", username, password));
        format!("Basic {}", encoded)
      }
      Auth::Bearer { token } => format!("Bearer {}", token),
    }
  }
}

impl AsRef<Auth> for Auth {
  fn as_ref(&self) -> &Auth {
    self
  }
}

#[cfg(test)]
mod tests {
  use super::Auth;

  #[test]
  fn test_basic_auth_header_value() {
    let auth = Auth::basic("user", "secret");
    // base64("user:secret") = "dXNlcjpzZWNyZXQ="
    assert_eq!("Basic dXNlcjpzZWNyZXQ=", auth.header_value());
  }

  #[test]
  fn test_basic_auth_empty_password() {
    let auth = Auth::basic("admin", "");
    // base64("admin:") = "YWRtaW46"
    assert_eq!("Basic YWRtaW46", auth.header_value());
  }

  #[test]
  fn test_bearer_auth_header_value() {
    let auth = Auth::bearer("my-token-123");
    assert_eq!("Bearer my-token-123", auth.header_value());
  }

  #[test]
  fn test_bearer_auth_complex_token() {
    let auth = Auth::bearer("eyJhbGciOiJIUzI1NiJ9.payload.signature");
    assert_eq!(
      "Bearer eyJhbGciOiJIUzI1NiJ9.payload.signature",
      auth.header_value()
    );
  }
}
