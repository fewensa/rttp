# rttp-protocol

Shared HTTP wire syntax and framing primitives for the rttp client and server crates.

This crate owns shared, bounded HTTP wire syntax and framing primitives. It
does not own application-level HTTP policy; that policy belongs to the client
and server crates.

This crate supports rttp's implementation; its public API is not a standalone
application-level HTTP interface.
## Content-Encoding

`content_encoding` parses one or more `Content-Encoding` field values into an
ordered list of content-coding tokens. Each field value is bounded to 64 KiB,
and the cumulative coding count across all supplied fields is bounded to 256
codings. Codings are split on commas with SP and HTAB accepted only as optional
whitespace around each coding; empty members and members containing forbidden
ASCII control bytes are rejected. Each coding must be an RFC 9110 token, and
repeated codings are retained in wire order so callers can inspect the full
encoding stack. A present header set that yields no coding still fails as
invalid.

## Transfer-Encoding

`transfer_encoding` parses one or more `Transfer-Encoding` field values into
an ordered list of transfer-coding tokens. Each field value is bounded to
64 KiB, and the cumulative coding count across all supplied fields is bounded
to 256 codings. Codings are split on commas with SP and HTAB accepted only as
optional whitespace around each coding; empty members and members containing
forbidden ASCII control bytes are rejected. Each coding must be an RFC 9110
token. Combined fields are validated in wire order and must yield a sole
`chunked` coding, matched case-insensitively, as the last and only token so
the type matches existing HTTP/1 framing. Duplicate fields, stacked codings,
`chunked` that is not last, and other unparsable input are errors. This
parser never fails open and does not decode a chunked body, negotiate `TE`,
or change Content-Length or HTTP/2 decode.

## Want-Content-Digest

`want_content_digest` parses one or more RFC 9530 `Want-Content-Digest` field
values into an ordered dictionary of algorithm keys and integer preferences.
Each field value is bounded to 64 KiB, and the combined algorithm count across
all supplied fields is bounded to 32. Members must be parameter-free Structured
Fields items whose values are Integers in `0` through `10` inclusive. Unknown
well-formed algorithm keys are retained as opaque data. Duplicate keys, bare
Boolean members, parameterized members, inner lists, decimals, negatives,
out-of-range integers, empty present fields, and other unparsable input are
errors. This parser never fails open to an empty preference set and does not
select an algorithm, compute a digest, or attach `Content-Digest`.

## Want-Repr-Digest

`want_repr_digest` parses one or more RFC 9530 `Want-Repr-Digest` field values
into an ordered dictionary of algorithm keys and integer preferences. Each
field value is bounded to 64 KiB, and the combined algorithm count across all
supplied fields is bounded to 32. Members must be parameter-free Structured
Fields items whose values are Integers in `0` through `10` inclusive. Unknown
well-formed algorithm keys are retained as opaque data. Duplicate keys, bare
Boolean members, parameterized members, inner lists, decimals, negatives,
out-of-range integers, empty present fields, and other unparsable input are
errors. This parser never fails open to an empty preference set and does not
select an algorithm, compute a digest, or attach `Repr-Digest`.

## Signature

`signature` parses one or more RFC 9421 `Signature` field values into an
ordered dictionary of labels and byte sequences. Each field value is bounded
to 64 KiB, the combined entry count is bounded to 256, each entry value is
bounded to 64 KiB, and each entry may carry at most 256 Structured Fields
parameters. Members must be dictionary keys mapped to byte sequences.
Well-formed item parameters are accepted as syntax and discarded. Duplicate
labels, non-byte-sequence values, empty present fields, and other unparsable
input are errors. This parser does not sign, verify, look up keys, or parse
`Signature-Input`.

## Cross-Origin-Opener-Policy

`cross_origin_opener_policy` parses a singleton `Cross-Origin-Opener-Policy`
structured-field item. Each field value is bounded to 64 KiB. A second field is
rejected after every supplied field is bound-checked. The bare item must be
exactly one of the tokens `unsafe-none`, `same-origin-allow-popups`,
`same-origin`, or `noopener-allow-popups`. Well-formed parameters, including
`report-to`, are accepted as syntax and discarded; this parser does not retain
reporting metadata or enforce opener policy. Case variants, lists, quoted
values, unknown tokens, empty values, and other unparsable input are errors.
The parser never fails open to `unsafe-none`.

## Cross-Origin-Opener-Policy

`cross_origin_opener_policy` parses a singleton `Cross-Origin-Opener-Policy`
structured-field item. Each field value is bounded to 64 KiB. A second field is
rejected after every supplied field is bound-checked. The bare item must be
exactly one of the tokens `unsafe-none`, `same-origin-allow-popups`,
`same-origin`, or `noopener-allow-popups`. Well-formed parameters, including
`report-to`, are accepted as syntax and discarded; this parser does not retain
reporting metadata or enforce opener policy. Case variants, lists, quoted
values, unknown tokens, empty values, and other unparsable input are errors.
The parser never fails open to `unsafe-none`.

Protocol helpers define and bound wire metadata for the client and server
crates. They do not add higher-level runtime policy such as caching,
authentication, retries, representation selection, or body transformation.

## Authentication-Info

`authentication_info` parses `#auth-param` lists from `Authentication-Info`
fields. Each field value is bounded to 64 KiB, the combined parameter count is
bounded to 256, and each parameter value is bounded to 64 KiB. Parameter names
are matched case-insensitively and must be unique across the combined field
set. Empty input, empty members, malformed syntax, and duplicate names are
rejected. This parser does not implement authentication policy.

## Proxy-Authentication-Info

`proxy_authentication_info` parses `#auth-param` lists from
`Proxy-Authentication-Info` fields. Each field value is bounded to 64 KiB, the
combined parameter count is bounded to 256, and each parameter value is
bounded to 64 KiB. Parameter names are matched case-insensitively and must be
unique across the combined field set. Empty input, empty members, malformed
syntax, and duplicate names are rejected. This parser does not implement
authentication policy.

## Cross-Origin-Embedder-Policy

`cross_origin_embedder_policy` parses a singleton `Cross-Origin-Embedder-Policy`
structured-field item. Each field value is bounded to 64 KiB. A second field is
rejected after every supplied field is bound-checked. The bare item must be
exactly one of the tokens `unsafe-none`, `require-corp`, or `credentialless`.
Well-formed parameters, including `report-to`, are accepted as syntax and
discarded; this parser does not retain reporting metadata or enforce embedder
policy. Case variants, lists, quoted values, unknown tokens, empty values, and
other unparsable input are errors. The parser never fails open to `unsafe-none`.

## Cross-Origin-Embedder-Policy-Report-Only

`cross_origin_embedder_policy_report_only` parses a singleton
`Cross-Origin-Embedder-Policy-Report-Only` structured-field item with the same
directive grammar as `Cross-Origin-Embedder-Policy`. Each field value is
bounded to 64 KiB. A second field is rejected after every supplied field is
bound-checked. The bare item must be exactly one of the tokens `unsafe-none`,
`require-corp`, or `credentialless`. Well-formed parameters, including
`report-to`, are accepted as syntax and discarded; this parser does not retain
reporting metadata, enforce embedder policy, deliver reports, or schedule
report delivery. Case variants, lists, quoted values, unknown tokens, empty
values, and other unparsable input are errors. The parser never fails open to
`unsafe-none`.

## Referer

`referer` parses a singleton HTTP `Referer` request field as one RFC 9110 URI
reference (`absolute-URI` / `partial-URI`). Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. The parser
preserves the trimmed reference text and does not canonicalize scheme, host,
port, path, query, or userinfo. Fragments, ASCII controls, interior
whitespace, non-URI bytes, broken percent-encoding, empty values, and values
the structural URL parser cannot accept are errors. This is syntax validation
only: callers own trust, logging, CSRF, and `Referrer-Policy` decisions. This
module is distinct from `referrer_policy`, which parses response policy tokens
rather than URI references.

## Referrer-Policy

`referrer_policy` parses one or more `Referrer-Policy` field values into
recognized policy tokens. Each field value is bounded to 64 KiB, and the
cumulative member count across all supplied fields is bounded to 256 members,
counting recognized and unknown members alike. Members are split on commas with
SP and HTAB accepted only as optional whitespace around each member; empty
members and members containing forbidden ASCII control bytes are rejected.
Recognized tokens are parsed case-insensitively and retained in wire order,
including repeated tokens, while valid unknown tokens are ignored so future
policy names remain forward-compatible within the same validation and count
bounds. A present header set that yields no recognized token still fails as
invalid.

## Strict-Transport-Security

`strict_transport_security` parses a singleton `Strict-Transport-Security`
field. Each field value is bounded to 64 KiB. A second field is rejected after
every supplied field is bound-checked. The field is a semicolon-separated
directive list bounded to 256 slots, including empty `;` slots from the RFC
6797 ABNF. `max-age` is required and, after optional quoted-string unescape,
must be unsigned `1*DIGIT` delta-seconds that fit in `u64`. `includeSubDomains`
and `preload` are optional valueless flags. Directive names are
case-insensitive and must appear at most once. Unknown well-formed directives
are ignored and not retained. Duplicate fields, duplicate directive names,
valued flags, malformed tokens or quoted-strings, missing `max-age`, and other
unparsable input are errors. The parser reports declared metadata only; it does
not pin TLS, store hosts, consult a preload list, or apply HTTPS-only policy.
`max-age=0` is returned as data and does not delete stored HSTS hosts.

## X-Frame-Options

`x_frame_options` parses a singleton `X-Frame-Options` field. Each field value
is bounded to 64 KiB. A second field is rejected after every supplied field is
bound-checked. The field value must be exactly one of the tokens `DENY` or
`SAMEORIGIN`, matched case-insensitively and returned in canonical uppercase
wire form. The deprecated `ALLOW-FROM` directive, unknown tokens, lists,
quoted values, empty values, and other unparsable input are errors. This
parser does not enforce clickjacking protection or frame-embedding policy.

## Warning

`warning` parses `#warning-value` lists from RFC 7234 `Warning` fields. Each
field value is bounded to 64 KiB, the combined item count is bounded to 256,
and each unescaped warn-agent and warn-text is bounded to 64 KiB. Warn-codes
are any 3 ASCII digits; warn-agent is opaque non-space text; warn-text is a
quoted-string; an optional quoted HTTP-date is parsed with the same
`httpdate` helper as Sunset. Empty input, empty members, malformed quoting,
invalid codes, and bound violations are rejected. This parser does not
implement cache, freshness, stale-response, or response-acceptance policy.
