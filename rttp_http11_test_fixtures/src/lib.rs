use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

pub mod request {
  pub struct FixedLengthRequest {
    pub raw: &'static [u8],
    pub method: &'static str,
    pub path: &'static str,
    pub query: Option<&'static str>,
    pub version: &'static str,
    pub host: &'static str,
    pub body: &'static [u8],
  }

  pub struct InvalidRequest {
    pub name: &'static str,
    pub raw: &'static [u8],
    pub error: &'static str,
  }

  pub struct TargetFormRequest {
    pub name: &'static str,
    pub raw: &'static [u8],
    pub method: &'static str,
    pub target: &'static str,
  }

  pub struct ChunkedRequest {
    pub raw: &'static [u8],
    pub method: &'static str,
    pub target: &'static str,
    pub body: &'static [u8],
    pub trailers: &'static [(&'static str, &'static str)],
  }

  pub struct ExpectContinueRequest {
    pub head: &'static [u8],
    pub body: &'static [u8],
    pub target: &'static str,
  }

  pub fn fixed_length_post() -> FixedLengthRequest {
    FixedLengthRequest {
      raw: b"POST /matrix/fixed?case=shared HTTP/1.1\r\nHost: example.test\r\nContent-Length: 11\r\n\r\nhello=world",
      method: "POST",
      path: "/matrix/fixed",
      query: Some("case=shared"),
      version: "HTTP/1.1",
      host: "example.test",
      body: b"hello=world",
    }
  }

  pub fn invalid_host_and_target_cases() -> &'static [InvalidRequest] {
    &[
      InvalidRequest {
        name: "missing host",
        raw: b"GET /matrix HTTP/1.1\r\n\r\n",
        error: "HTTP/1.1 request requires exactly one Host header",
      },
      InvalidRequest {
        name: "duplicate host",
        raw: b"GET /matrix HTTP/1.1\r\nHost: example.test\r\nHost: other.test\r\n\r\n",
        error: "HTTP/1.1 request requires exactly one Host header",
      },
      InvalidRequest {
        name: "invalid origin target",
        raw: b"GET matrix HTTP/1.1\r\nHost: example.test\r\n\r\n",
        error: "invalid request target",
      },
      InvalidRequest {
        name: "non-CONNECT authority-form target",
        raw: b"GET example.test:443 HTTP/1.1\r\nHost: example.test\r\n\r\n",
        error: "invalid request target",
      },
      InvalidRequest {
        name: "connect authority host mismatch",
        raw: b"CONNECT example.test:443 HTTP/1.1\r\nHost: other.test:443\r\n\r\n",
        error: "invalid Host header",
      },
    ]
  }

  pub fn valid_origin_and_absolute_form_cases() -> &'static [TargetFormRequest] {
    &[
      TargetFormRequest {
        name: "origin-form target",
        raw: b"GET /matrix/origin?case=shared HTTP/1.1\r\nHost: example.test\r\n\r\n",
        method: "GET",
        target: "/matrix/origin?case=shared",
      },
      TargetFormRequest {
        name: "absolute-form target",
        raw: b"GET http://example.test/matrix/absolute?case=shared HTTP/1.1\r\nHost: proxy.local\r\n\r\n",
        method: "GET",
        target: "/matrix/absolute?case=shared",
      },
    ]
  }

  pub fn framing_ambiguity_cases() -> &'static [InvalidRequest] {
    &[
      InvalidRequest {
        name: "conflicting content length",
        raw: b"POST /matrix HTTP/1.1\r\nHost: example.test\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello",
        error: "conflicting Content-Length headers",
      },
      InvalidRequest {
        name: "transfer encoding with content length",
        raw: b"POST /matrix HTTP/1.1\r\nHost: example.test\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\nhello",
        error: "Transfer-Encoding conflicts with Content-Length",
      },
    ]
  }

  pub fn obsolete_line_folding_cases() -> &'static [InvalidRequest] {
    &[
      InvalidRequest {
        name: "space-prefixed folded request header",
        raw: b"GET /matrix HTTP/1.1\r\nHost: example.test\r\nX-Test: one\r\n two\r\n\r\n",
        error: "invalid request header",
      },
      InvalidRequest {
        name: "tab-prefixed folded request header",
        raw: b"GET /matrix HTTP/1.1\r\nHost: example.test\r\nX-Test: one\r\n\ttwo\r\n\r\n",
        error: "invalid request header",
      },
    ]
  }

  pub fn chunked_with_extensions_and_trailers() -> ChunkedRequest {
    ChunkedRequest {
      raw: concat!(
        "POST /matrix/chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "5;foo=\"bar;baz\";answer=42\r\n",
        "hello\r\n",
        "6;token=value\r\n",
        " world\r\n",
        "0\r\n",
        "X-Trace: abc\r\n",
        "X-Signature: signed\r\n",
        "\r\n"
      )
      .as_bytes(),
      method: "POST",
      target: "/matrix/chunked",
      body: b"hello world",
      trailers: &[("x-trace", "abc"), ("X-SIGNATURE", "signed")],
    }
  }

  pub fn keep_alive_pipeline() -> &'static [u8] {
    concat!(
      "POST /matrix/first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: keep-alive\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "alpha",
      "POST /matrix/second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: close\r\n",
      "Content-Length: 6\r\n",
      "\r\n",
      "bravo!"
    )
    .as_bytes()
  }

  pub fn expect_continue_fixed_length() -> ExpectContinueRequest {
    ExpectContinueRequest {
      head: concat!(
        "POST /matrix/continue HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Expect: 100-continue\r\n",
        "Content-Length: 12\r\n",
        "\r\n"
      )
      .as_bytes(),
      body: b"request body",
      target: "/matrix/continue",
    }
  }
}

pub mod response {
  pub const CONTINUE: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n";

  pub const CHUNKED_WITH_EXTENSIONS_AND_TRAILERS: &[u8] = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "\r\n",
    "7;foo=\"bar;baz\";answer=42\r\n",
    "chunked\r\n",
    "6;token=value\r\n",
    " body!\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  )
  .as_bytes();

  pub const TRANSFER_ENCODING_WITH_CONTENT_LENGTH: &[u8] = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 13\r\n",
    "\r\n",
    "0\r\n",
    "\r\n"
  )
  .as_bytes();
}

pub mod cache_control {
  pub const MAX_VALUE_BYTES: usize = 64 * 1024;
  pub const MAX_DIRECTIVES: usize = 256;

  pub struct RequestCase {
    pub name: &'static str,
    pub values: &'static [&'static str],
    pub no_cache: bool,
    pub no_store: bool,
    pub max_age: Option<u64>,
    pub max_stale: Option<Option<u64>>,
    pub min_fresh: Option<u64>,
    pub no_transform: bool,
    pub only_if_cached: bool,
    pub extensions: &'static [(&'static str, Option<&'static str>)],
  }

  pub struct ResponseCase {
    pub name: &'static str,
    pub values: &'static [&'static str],
    pub no_cache: bool,
    pub no_cache_fields: &'static [&'static str],
    pub no_store: bool,
    pub max_age: Option<u64>,
    pub s_maxage: Option<u64>,
    pub private: bool,
    pub private_fields: &'static [&'static str],
    pub public: bool,
    pub must_revalidate: bool,
    pub proxy_revalidate: bool,
    pub immutable: bool,
    pub stale_while_revalidate: Option<u64>,
    pub stale_if_error: Option<u64>,
    pub extensions: &'static [(&'static str, Option<&'static str>)],
  }

  pub struct InvalidCase {
    pub name: &'static str,
    pub value: &'static str,
  }

  pub fn request_cases() -> &'static [RequestCase] {
    &[
      RequestCase {
        name: "request known directives across header fields",
        values: &[
          "no-cache, no-store, max-age=60, max-stale=120",
          "min-fresh=30, no-transform, only-if-cached",
        ],
        no_cache: true,
        no_store: true,
        max_age: Some(60),
        max_stale: Some(Some(120)),
        min_fresh: Some(30),
        no_transform: true,
        only_if_cached: true,
        extensions: &[],
      },
      RequestCase {
        name: "request extension and quoted-string values",
        values: &[
          "ext=\"a,b\", token-ext, escaped=\"quoted\\\\value\"",
          "obs-ext=\"cache policy\"",
        ],
        no_cache: false,
        no_store: false,
        max_age: None,
        max_stale: None,
        min_fresh: None,
        no_transform: false,
        only_if_cached: false,
        extensions: &[
          ("ext", Some("a,b")),
          ("token-ext", None),
          ("escaped", Some("quoted\\value")),
          ("obs-ext", Some("cache policy")),
        ],
      },
      RequestCase {
        name: "request duplicate directives keep helper parity",
        values: &["max-age=10, max-age=20, no-cache, no-cache"],
        no_cache: true,
        no_store: false,
        max_age: Some(20),
        max_stale: None,
        min_fresh: None,
        no_transform: false,
        only_if_cached: false,
        extensions: &[],
      },
      RequestCase {
        name: "request max-stale without value remains a bounded helper",
        values: &["max-stale"],
        no_cache: false,
        no_store: false,
        max_age: None,
        max_stale: Some(None),
        min_fresh: None,
        no_transform: false,
        only_if_cached: false,
        extensions: &[],
      },
    ]
  }

  pub fn response_cases() -> &'static [ResponseCase] {
    &[
      ResponseCase {
        name: "response known directives across header fields",
        values: &[
          "no-cache=\"Set-Cookie, Authorization\", no-store, max-age=60",
          "s-maxage=120, private=\"X-User\", public, must-revalidate",
          "proxy-revalidate, immutable, stale-while-revalidate=30, stale-if-error=90",
        ],
        no_cache: true,
        no_cache_fields: &["Set-Cookie", "Authorization"],
        no_store: true,
        max_age: Some(60),
        s_maxage: Some(120),
        private: true,
        private_fields: &["X-User"],
        public: true,
        must_revalidate: true,
        proxy_revalidate: true,
        immutable: true,
        stale_while_revalidate: Some(30),
        stale_if_error: Some(90),
        extensions: &[],
      },
      ResponseCase {
        name: "response extension and quoted-string values",
        values: &[
          "community=\"u=1, tier=gold\", ext-token",
          "escaped=\"quoted\\\\value\"",
        ],
        no_cache: false,
        no_cache_fields: &[],
        no_store: false,
        max_age: None,
        s_maxage: None,
        private: false,
        private_fields: &[],
        public: false,
        must_revalidate: false,
        proxy_revalidate: false,
        immutable: false,
        stale_while_revalidate: None,
        stale_if_error: None,
        extensions: &[
          ("community", Some("u=1, tier=gold")),
          ("ext-token", None),
          ("escaped", Some("quoted\\value")),
        ],
      },
      ResponseCase {
        name: "response duplicate directives keep helper parity",
        values: &["max-age=10, max-age=20, private=\"A\", private=\"B\""],
        no_cache: false,
        no_cache_fields: &[],
        no_store: false,
        max_age: Some(20),
        s_maxage: None,
        private: true,
        private_fields: &["B"],
        public: false,
        must_revalidate: false,
        proxy_revalidate: false,
        immutable: false,
        stale_while_revalidate: None,
        stale_if_error: None,
        extensions: &[],
      },
    ]
  }

  pub fn invalid_request_cases() -> &'static [InvalidCase] {
    &[
      InvalidCase {
        name: "request invalid max-age delta-seconds",
        value: "max-age=-1",
      },
      InvalidCase {
        name: "request invalid max-stale delta-seconds",
        value: "max-stale=1.5",
      },
      InvalidCase {
        name: "request quoted min-fresh delta-seconds",
        value: "min-fresh=\"60\"",
      },
      InvalidCase {
        name: "request malformed quoted-string",
        value: "extension=\"unterminated",
      },
    ]
  }

  pub fn invalid_response_cases() -> &'static [InvalidCase] {
    &[
      InvalidCase {
        name: "response invalid max-age delta-seconds",
        value: "max-age=-1",
      },
      InvalidCase {
        name: "response invalid s-maxage delta-seconds",
        value: "s-maxage=abc",
      },
      InvalidCase {
        name: "response quoted stale-if-error delta-seconds",
        value: "stale-if-error=\"60\"",
      },
      InvalidCase {
        name: "response malformed quoted-string",
        value: "private=\"unterminated",
      },
    ]
  }

  pub fn too_many_directives_value() -> String {
    (0..=MAX_DIRECTIVES)
      .map(|index| format!("ext{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  }

  pub fn oversized_value() -> String {
    format!("ext=\"{}\"", "a".repeat(MAX_VALUE_BYTES))
  }
}

pub mod age_expires {
  pub const EXPIRES_UNIX_SECONDS: u64 = 784_111_777;
  pub const EXPIRES_IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";

  pub struct AgeCase {
    pub name: &'static str,
    pub value: &'static str,
    pub delta_seconds: u64,
  }

  pub struct ExpiresCase {
    pub name: &'static str,
    pub value: &'static str,
    pub unix_seconds: u64,
  }

  pub struct DeclarationCase {
    pub name: &'static str,
    pub age: u64,
    pub age_value: &'static str,
    pub expires_unix_seconds: u64,
    pub expires_value: &'static str,
  }

  pub struct InvalidCase {
    pub name: &'static str,
    pub value: &'static str,
  }

  const AGE_CASES: &[AgeCase] = &[
    AgeCase {
      name: "Age zero delta-seconds",
      value: "0",
      delta_seconds: 0,
    },
    AgeCase {
      name: "Age positive delta-seconds",
      value: "60",
      delta_seconds: 60,
    },
  ];

  const EXPIRES_CASES: &[ExpiresCase] = &[
    ExpiresCase {
      name: "Expires IMF-fixdate",
      value: EXPIRES_IMF_FIXDATE,
      unix_seconds: EXPIRES_UNIX_SECONDS,
    },
    ExpiresCase {
      name: "Expires obsolete RFC 850 date",
      value: "Sunday, 06-Nov-94 08:49:37 GMT",
      unix_seconds: EXPIRES_UNIX_SECONDS,
    },
  ];

  const DECLARATION_CASES: &[DeclarationCase] = &[
    DeclarationCase {
      name: "declared zero Age with Expires",
      age: 0,
      age_value: "0",
      expires_unix_seconds: EXPIRES_UNIX_SECONDS,
      expires_value: EXPIRES_IMF_FIXDATE,
    },
    DeclarationCase {
      name: "declared positive Age with Expires",
      age: 60,
      age_value: "60",
      expires_unix_seconds: EXPIRES_UNIX_SECONDS,
      expires_value: EXPIRES_IMF_FIXDATE,
    },
  ];

  const INVALID_AGE_CASES: &[InvalidCase] = &[
    InvalidCase {
      name: "Age empty value",
      value: "",
    },
    InvalidCase {
      name: "Age signed delta-seconds",
      value: "-1",
    },
    InvalidCase {
      name: "Age fractional delta-seconds",
      value: "1.5",
    },
    InvalidCase {
      name: "Age non-numeric delta-seconds",
      value: "abc",
    },
    InvalidCase {
      name: "Age comma-list delta-seconds",
      value: "0, 60",
    },
    InvalidCase {
      name: "Age overflowing delta-seconds",
      value: "18446744073709551616",
    },
  ];

  const INVALID_EXPIRES_CASES: &[InvalidCase] = &[
    InvalidCase {
      name: "Expires empty value",
      value: "",
    },
    InvalidCase {
      name: "Expires non-date value",
      value: "not a date",
    },
    InvalidCase {
      name: "Expires unsupported timezone",
      value: "Sun, 06 Nov 1994 08:49:37 PST",
    },
  ];

  pub fn age_cases() -> &'static [AgeCase] {
    AGE_CASES
  }

  pub fn expires_cases() -> &'static [ExpiresCase] {
    EXPIRES_CASES
  }

  pub fn declaration_cases() -> &'static [DeclarationCase] {
    DECLARATION_CASES
  }

  pub fn invalid_age_cases() -> &'static [InvalidCase] {
    INVALID_AGE_CASES
  }

  pub fn invalid_expires_cases() -> &'static [InvalidCase] {
    INVALID_EXPIRES_CASES
  }
}

pub mod retry_after {
  pub const RETRY_AFTER_UNIX_SECONDS: u64 = 784_111_777;
  pub const RETRY_AFTER_IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";

  pub struct RetryAfterCase {
    pub name: &'static str,
    pub value: &'static str,
    pub kind: RetryAfterKind,
  }

  pub enum RetryAfterKind {
    DeltaSeconds(u64),
    HttpDate(u64),
  }

  pub struct DeclarationCase {
    pub name: &'static str,
    pub delta_seconds: u64,
    pub delta_value: &'static str,
    pub date_unix_seconds: u64,
    pub date_value: &'static str,
  }

  pub struct InvalidCase {
    pub name: &'static str,
    pub value: &'static str,
  }

  const RETRY_AFTER_CASES: &[RetryAfterCase] = &[
    RetryAfterCase {
      name: "Retry-After zero delta-seconds",
      value: "0",
      kind: RetryAfterKind::DeltaSeconds(0),
    },
    RetryAfterCase {
      name: "Retry-After positive delta-seconds",
      value: "120",
      kind: RetryAfterKind::DeltaSeconds(120),
    },
    RetryAfterCase {
      name: "Retry-After IMF-fixdate",
      value: RETRY_AFTER_IMF_FIXDATE,
      kind: RetryAfterKind::HttpDate(RETRY_AFTER_UNIX_SECONDS),
    },
    RetryAfterCase {
      name: "Retry-After obsolete RFC 850 date",
      value: "Sunday, 06-Nov-94 08:49:37 GMT",
      kind: RetryAfterKind::HttpDate(RETRY_AFTER_UNIX_SECONDS),
    },
  ];

  const DECLARATION_CASES: &[DeclarationCase] = &[
    DeclarationCase {
      name: "declared zero Retry-After delta with HTTP-date",
      delta_seconds: 0,
      delta_value: "0",
      date_unix_seconds: RETRY_AFTER_UNIX_SECONDS,
      date_value: RETRY_AFTER_IMF_FIXDATE,
    },
    DeclarationCase {
      name: "declared positive Retry-After delta with HTTP-date",
      delta_seconds: 120,
      delta_value: "120",
      date_unix_seconds: RETRY_AFTER_UNIX_SECONDS,
      date_value: RETRY_AFTER_IMF_FIXDATE,
    },
  ];

  const INVALID_CASES: &[InvalidCase] = &[
    InvalidCase {
      name: "Retry-After empty value",
      value: "",
    },
    InvalidCase {
      name: "Retry-After signed delta-seconds",
      value: "-1",
    },
    InvalidCase {
      name: "Retry-After fractional delta-seconds",
      value: "1.5",
    },
    InvalidCase {
      name: "Retry-After non-numeric non-date value",
      value: "abc",
    },
    InvalidCase {
      name: "Retry-After comma-list delta-seconds",
      value: "0, 60",
    },
    InvalidCase {
      name: "Retry-After overflowing delta-seconds",
      value: "18446744073709551616",
    },
    InvalidCase {
      name: "Retry-After unsupported timezone",
      value: "Sun, 06 Nov 1994 08:49:37 PST",
    },
  ];

  pub fn retry_after_cases() -> &'static [RetryAfterCase] {
    RETRY_AFTER_CASES
  }

  pub fn declaration_cases() -> &'static [DeclarationCase] {
    DECLARATION_CASES
  }

  pub fn invalid_cases() -> &'static [InvalidCase] {
    INVALID_CASES
  }

  pub fn oversized_value() -> String {
    "1".repeat(64 * 1024 + 1)
  }
}

pub mod allow {
  pub const MAX_VALUE_BYTES: usize = 64 * 1024;
  pub const MAX_METHODS: usize = 256;

  pub struct ResponseCase {
    pub name: &'static str,
    pub values: &'static [&'static str],
    pub methods: &'static [&'static str],
  }

  pub struct InvalidCase {
    pub name: &'static str,
    pub value: &'static str,
  }

  const RESPONSE_CASES: &[ResponseCase] = &[
    ResponseCase {
      name: "single method",
      values: &["GET"],
      methods: &["GET"],
    },
    ResponseCase {
      name: "methods across header fields",
      values: &["GET, HEAD", "POST, OPTIONS"],
      methods: &["GET", "HEAD", "POST", "OPTIONS"],
    },
    ResponseCase {
      name: "optional whitespace around commas",
      values: &["GET,\tHEAD , POST ,OPTIONS"],
      methods: &["GET", "HEAD", "POST", "OPTIONS"],
    },
    ResponseCase {
      name: "extension methods preserve token order",
      values: &["PATCH, MKCOL, REPORT"],
      methods: &["PATCH", "MKCOL", "REPORT"],
    },
  ];

  const INVALID_CASES: &[InvalidCase] = &[
    InvalidCase {
      name: "empty value",
      value: "",
    },
    InvalidCase {
      name: "trailing comma",
      value: "GET,",
    },
    InvalidCase {
      name: "leading comma",
      value: ", GET",
    },
    InvalidCase {
      name: "empty member",
      value: "GET,,POST",
    },
    InvalidCase {
      name: "method with whitespace",
      value: "GET POST",
    },
    InvalidCase {
      name: "method with separator",
      value: "GET@POST",
    },
  ];

  pub fn response_cases() -> &'static [ResponseCase] {
    RESPONSE_CASES
  }

  pub fn invalid_cases() -> &'static [InvalidCase] {
    INVALID_CASES
  }

  pub fn too_many_methods_value() -> String {
    (0..=MAX_METHODS)
      .map(|index| format!("M{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  }

  pub fn oversized_value() -> String {
    format!("M{}", "A".repeat(MAX_VALUE_BYTES))
  }
}

pub mod vary {
  pub const MAX_VALUE_BYTES: usize = 64 * 1024;
  pub const MAX_FIELD_NAMES: usize = 256;

  pub struct ResponseCase {
    pub name: &'static str,
    pub values: &'static [&'static str],
    pub wildcard: bool,
    pub field_names: &'static [&'static str],
  }

  pub struct SelectionCase {
    pub name: &'static str,
    pub request: &'static [u8],
    pub value: &'static str,
    pub wildcard: bool,
    pub field_names: &'static [&'static str],
    pub selected_values: &'static [(&'static str, &'static [&'static str])],
  }

  pub struct InvalidCase {
    pub name: &'static str,
    pub value: &'static str,
  }

  const RESPONSE_CASES: &[ResponseCase] = &[
    ResponseCase {
      name: "field names across header fields",
      values: &["Accept-Encoding, User-Agent", "accept-language, X-Feature"],
      wildcard: false,
      field_names: &[
        "accept-encoding",
        "user-agent",
        "accept-language",
        "x-feature",
      ],
    },
    ResponseCase {
      name: "case-insensitive duplicate field names are deduplicated",
      values: &["Accept-Encoding, accept-encoding, ACCEPT-LANGUAGE"],
      wildcard: false,
      field_names: &["accept-encoding", "accept-language"],
    },
    ResponseCase {
      name: "wildcard response",
      values: &["*"],
      wildcard: true,
      field_names: &[],
    },
  ];

  const ACCEPT_ENCODING_VALUES: &[&str] = &["gzip", "br"];
  const X_USER_VALUES: &[&str] = &["123"];
  const EMPTY_VALUES: &[&str] = &[];
  const SELECTION_VALUES: &[(&str, &[&str])] = &[
    ("accept-encoding", ACCEPT_ENCODING_VALUES),
    ("x-user", X_USER_VALUES),
    ("accept-language", EMPTY_VALUES),
  ];
  const EMPTY_SELECTION_VALUES: &[(&str, &[&str])] = &[];

  const SELECTION_CASES: &[SelectionCase] = &[
    SelectionCase {
      name: "case-insensitive field selection preserves duplicate request values",
      request: concat!(
        "GET /matrix/vary HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Accept-Encoding: gzip\r\n",
        "accept-encoding: br\r\n",
        "X-User: 123\r\n",
        "\r\n"
      )
      .as_bytes(),
      value: "ACCEPT-ENCODING, x-user, accept-language",
      wildcard: false,
      field_names: &["accept-encoding", "x-user", "accept-language"],
      selected_values: SELECTION_VALUES,
    },
    SelectionCase {
      name: "wildcard selection does not bind request fields",
      request: concat!(
        "GET /matrix/vary HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Accept-Encoding: gzip\r\n",
        "\r\n"
      )
      .as_bytes(),
      value: "*",
      wildcard: true,
      field_names: &[],
      selected_values: EMPTY_SELECTION_VALUES,
    },
  ];

  const INVALID_CASES: &[InvalidCase] = &[
    InvalidCase {
      name: "empty value",
      value: "",
    },
    InvalidCase {
      name: "trailing comma",
      value: "Accept-Encoding,",
    },
    InvalidCase {
      name: "leading comma",
      value: ", Accept-Encoding",
    },
    InvalidCase {
      name: "empty member",
      value: "Accept-Encoding,,User-Agent",
    },
    InvalidCase {
      name: "field name with whitespace",
      value: "Accept Encoding",
    },
    InvalidCase {
      name: "field name with separator",
      value: "Accept@Encoding",
    },
    InvalidCase {
      name: "wildcard followed by field name",
      value: "*, Accept-Encoding",
    },
    InvalidCase {
      name: "field name followed by wildcard",
      value: "Accept-Encoding, *",
    },
  ];

  pub fn response_cases() -> &'static [ResponseCase] {
    RESPONSE_CASES
  }

  pub fn selection_cases() -> &'static [SelectionCase] {
    SELECTION_CASES
  }

  pub fn invalid_cases() -> &'static [InvalidCase] {
    INVALID_CASES
  }

  pub fn too_many_field_names_value() -> String {
    (0..=MAX_FIELD_NAMES)
      .map(|index| format!("x-vary-{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  }

  pub fn oversized_value() -> String {
    format!("x-{}", "a".repeat(MAX_VALUE_BYTES))
  }
}

pub fn bind_socket2_tcp_listener(name: &str) -> (TcpListener, SocketAddr) {
  let addr: SocketAddr = "127.0.0.1:0".parse().expect("parse local addr");
  let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
    .unwrap_or_else(|err| panic!("create {name} socket: {err}"));
  socket
    .set_reuse_address(true)
    .unwrap_or_else(|err| panic!("set {name} reuse addr: {err}"));
  socket
    .bind(&addr.into())
    .unwrap_or_else(|err| panic!("bind {name}: {err}"));
  socket
    .listen(16)
    .unwrap_or_else(|err| panic!("listen {name}: {err}"));
  let listener = TcpListener::from(socket);
  let addr = listener
    .local_addr()
    .unwrap_or_else(|err| panic!("read {name} local addr: {err}"));
  (listener, addr)
}

pub fn read_http_request<R: Read>(stream: &mut R) -> Vec<u8> {
  let mut request = Vec::new();
  let mut buf = [0u8; 1024];
  let mut content_length = None;

  while let Ok(read) = stream.read(&mut buf) {
    if read == 0 {
      break;
    }

    request.extend_from_slice(&buf[..read]);

    let header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
    if content_length.is_none() {
      if let Some(header_end) = header_end {
        let headers = String::from_utf8_lossy(&request[..header_end + 4]);
        content_length = headers
          .lines()
          .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
              value.trim().parse::<usize>().ok()
            } else {
              None
            }
          })
          .or(Some(0));
      }
    }

    if let (Some(header_end), Some(content_length)) = (header_end, content_length) {
      let expected_len = header_end + 4 + content_length;
      if request.len() >= expected_len {
        break;
      }
    }
  }

  request
}

pub fn spawn_socket2_raw_response_server(
  response: &'static [u8],
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  spawn_socket2_owned_raw_response_server(response.to_vec())
}

pub fn spawn_socket2_owned_raw_response_server(
  response: Vec<u8>,
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_socket2_tcp_listener("raw response server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set read timeout");
    let request = read_http_request(&mut stream);
    let _ = stream.write_all(&response);
    request
  });
  (addr, handle)
}

pub fn spawn_socket2_expect_continue_server(
  final_response: &'static [u8],
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let (listener, addr) = bind_socket2_tcp_listener("expect continue server");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set read timeout");

    serve_expect_continue_stream(&mut stream, final_response)
  });
  (addr, handle)
}

fn serve_expect_continue_stream<S: Read + Write>(stream: &mut S, final_response: &[u8]) -> Vec<u8> {
  let mut request = read_until_header_end(stream);
  let content_length = request_content_length(&request).unwrap_or(0);

  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| position + 4)
    .unwrap_or(request.len());
  let body_bytes_read = request.len().saturating_sub(header_end);
  if body_bytes_read != 0 {
    return Vec::new();
  }

  let _ = stream.write_all(response::CONTINUE);

  if body_bytes_read < content_length {
    let mut body = vec![0; content_length - body_bytes_read];
    if stream.read_exact(&mut body).is_ok() {
      request.extend_from_slice(&body);
    }
  }

  let _ = stream.write_all(final_response);
  request
}

fn read_until_header_end<R: Read>(stream: &mut R) -> Vec<u8> {
  let mut request = Vec::new();
  let mut buf = [0u8; 256];

  while let Ok(read) = stream.read(&mut buf) {
    if read == 0 {
      break;
    }
    request.extend_from_slice(&buf[..read]);
    if request.windows(4).any(|window| window == b"\r\n\r\n") {
      break;
    }
  }

  request
}

fn request_content_length(request: &[u8]) -> Option<usize> {
  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")?;
  let headers = String::from_utf8_lossy(&request[..header_end + 4]);
  headers.lines().find_map(|line| {
    let (name, value) = line.split_once(':')?;
    if name.eq_ignore_ascii_case("content-length") {
      value.trim().parse::<usize>().ok()
    } else {
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::{request, serve_expect_continue_stream};
  use std::io::{self, Read, Write};

  struct InMemoryStream {
    read: Vec<u8>,
    written: Vec<u8>,
  }

  impl InMemoryStream {
    fn new(read: Vec<u8>) -> Self {
      Self {
        read,
        written: Vec::new(),
      }
    }
  }

  impl Read for InMemoryStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
      let read = buf.len().min(self.read.len());
      buf[..read].copy_from_slice(&self.read[..read]);
      self.read.drain(..read);
      Ok(read)
    }
  }

  impl Write for InMemoryStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      self.written.extend_from_slice(buf);
      Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn expect_continue_server_rejects_premature_body_bytes() {
    let fixture = request::expect_continue_fixed_length();
    let mut request = Vec::new();
    request.extend_from_slice(fixture.head);
    request.extend_from_slice(fixture.body);
    let mut stream = InMemoryStream::new(request);

    assert_eq!(
      Vec::<u8>::new(),
      serve_expect_continue_stream(&mut stream, b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
    );
    assert!(
      !stream
        .written
        .windows(super::response::CONTINUE.len())
        .any(|window| window == super::response::CONTINUE),
      "server must not send 100 Continue after premature body"
    );
  }
}
