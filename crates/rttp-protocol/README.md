# rttp-protocol

Shared HTTP wire syntax and framing primitives for the rttp client and server crates.

This crate owns shared, bounded HTTP wire syntax and framing primitives. It
does not own application-level HTTP policy; that policy belongs to the client
and server crates.

This crate supports rttp's implementation; its public API is not a standalone
application-level HTTP interface.

## Accept-Ranges

`accept_ranges` parses one or more `Accept-Ranges` field values into an ordered
list of range-unit tokens, preserving each unit's spelling and wire order. Each
field value is bounded to 64 KiB, and the cumulative unit count across all
supplied fields is bounded to 256 units. Units are split on commas with SP and
HTAB accepted only as optional whitespace around each unit; empty members,
members containing forbidden ASCII control bytes, and units that are not RFC
9110 tokens are rejected. Duplicate units are rejected case-insensitively while
the first-seen spelling is retained. The `none` sentinel is accepted only alone
and is represented as an empty unit list; `none` combined with any unit fails
as invalid. A present header set that yields no unit still fails as invalid.
The server facade aliases this type as `HttpAcceptRanges` and reuses
`from_units`/`none` for its declaration helpers.

## Accept-Language

`accept_language` parses one or more RFC 9110 `Accept-Language` field values
into an ordered list of language ranges with optional q-values. Each field
value is bounded to 64 KiB, and the cumulative range count across all supplied
fields is bounded to 32 ranges. Items are split on commas with surrounding
whitespace trimmed from each item; empty members and malformed ranges are
errors. A range is `*` or a primary subtag of 1-8 ASCII letters followed by
any number of 1-8 character ASCII alphanumeric subtags separated by hyphens.
Each range may carry one `q` parameter whose value is `0` or `1`, optionally
with up to three fractional digits and with a `1` integer part requiring an
all-zero fraction. Case-insensitive duplicate ranges are rejected while the
first-seen spelling is retained. `from_ranges` validates supplied ranges for
client construction and `header_value()` re-emits them normalized as
`range; q=quality`. This parser reports declared metadata only; it does not
perform locale matching, fallback selection, translation lookup, routing, or
automatic response choice.

## Age

`age` parses a singleton HTTP `Age` field as non-negative `1*DIGIT`
delta-seconds that fit in `u64`. Each field value is bounded to 64 KiB. A
second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. Empty values,
signed or plus-prefixed numbers, fractions, comma-lists, non-digits, overflow
beyond `u64::MAX`, and forbidden ASCII control bytes are errors. This parser
reports declared metadata only; it does not calculate freshness, adjust age
over elapsed time, store cache entries, or apply cache policy.

## Max-Forwards

`max_forwards` parses a singleton HTTP `Max-Forwards` request field as
non-negative `1*DIGIT` hop counts that fit in `u32`. Each field value is
bounded to 64 KiB. A second field is rejected after every supplied field is
bound-checked. Surrounding SP and HTAB are trimmed as optional whitespace.
Empty values, signed or plus-prefixed numbers, fractions, comma-lists,
non-digits, overflow beyond `u32::MAX`, oversized values, and forbidden ASCII
control bytes are errors. `header_value()` emits the accepted count in
canonical decimal form. This parser reports declared metadata only; it does
not decrement the hop count, route through proxies, select TRACE or OPTIONS,
or apply forwarding policy.

## If-Modified-Since

`if_modified_since` parses a singleton HTTP `If-Modified-Since` request field
as one HTTP-date instant through `httpdate`. Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is
bound-checked. Surrounding SP and HTAB are trimmed as optional whitespace.
Empty values, malformed dates, forbidden ASCII control bytes, and oversized
values are errors. `header_value()` formats the accepted instant as
IMF-fixdate. This parser reports declared request metadata only; it does not
compare `Last-Modified`, evaluate conditional precedence, serve a
representation, or apply cache policy.

## If-Unmodified-Since

`if_unmodified_since` parses a singleton HTTP `If-Unmodified-Since` request
field as one HTTP-date instant through `httpdate`. Each field value is bounded
to 64 KiB. A second field is rejected after every supplied field is
bound-checked. Surrounding SP and HTAB are trimmed as optional whitespace.
Empty values, malformed dates, forbidden ASCII control bytes, and oversized
values are errors. `header_value()` formats the accepted instant as
IMF-fixdate. This parser reports declared request metadata only; it does not
compare `Last-Modified`, evaluate conditional precedence, reject a
representation, or apply cache policy.

## Content-DPR

`content_dpr` parses a singleton HTTP `Content-DPR` field as a finite positive
decimal ratio matching `1*DIGIT["." 1*DIGIT]`. Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace, and the trimmed
decimal text is preserved through `header_value()`. Empty values, zero,
non-finite numbers, trailing or leading decimal points, signs, exponent
notation, leftover characters, and forbidden ASCII control bytes are errors.
This parser reports declared metadata only; it does not rescale images, send
request DPR, apply Client Hints policy, retry, or change transport.

## Memento-Datetime

`memento_datetime` parses a singleton `Memento-Datetime` field as one
IMF-fixdate through `httpdate`. Each field value is bounded to 64 KiB. A
second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. Empty values,
malformed dates, forbidden ASCII control bytes, and oversized values are
errors. `header_value()` formats the accepted instant as IMF-fixdate. This
parser reports declared metadata only; it does not select an archival
representation, negotiate `Accept-Datetime`, or implement TimeGate behavior.

## Deprecation

`deprecation` parses a singleton HTTP `Deprecation` field as one Structured
Fields item that is either a boolean (`?0` / `?1`) or a date (`@` followed by
a signed integer number of UNIX seconds). Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. Empty values,
item parameters, inner lists, comma-joined items, integers without `@`,
decimals, strings, tokens (including historical `true`), byte sequences,
display strings, IMF-fixdate values, forbidden ASCII control bytes, and dates
that cannot be represented as `SystemTime` are errors. This parser reports
declared metadata only; it does not compare `Sunset`, follow `Link`
`rel=deprecation`, decide whether a resource is already deprecated, retry
requests, or select another endpoint.

## Content-Disposition

`content_disposition` parses a singleton response `Content-Disposition` field
as one disposition type plus an ordered list of parameters. Each field value
is bounded to 64 KiB, the parameter count is bounded to 256, and each
parameter value is bounded to 64 KiB. A second field is rejected after every
supplied field is bound-checked. Surrounding SP and HTAB are treated as
optional whitespace around separators. Quoted-strings are unescaped, including
obs-text, and the stored parameter value is the logical value rather than the
wire quoting. Parameter names are compared case-insensitively for duplicates,
and both the disposition type and parameter names are stored in lowercase.
`filename` and `filename*` remain independent parameters; `filename*` must be
an unquoted RFC 5987 ext-value and is preserved without decoding. Empty
values, empty parameter values, malformed quoted-strings, ASCII controls other
than HTAB, duplicate parameters, invalid tokens, and unparsable input are
errors. This parser never fails open to `inline` or an empty parameter list.
It reports declared metadata only: callers own download handling, filesystem
paths, filename precedence, RFC 5987 decoding, MIME sniffing, cache behavior,
redirects, retries, negotiation, and status policy.

## Content-Location

`content_location` parses a singleton response `Content-Location` field as one
bounded URI reference. Each field value is bounded to 64 KiB. A second field is
rejected after every supplied field is bound-checked. Surrounding SP and HTAB
are trimmed as optional whitespace, and the trimmed reference text is preserved
through `as_str()` and `header_value()` without resolution against any request
or response URL. Empty values, ASCII controls, interior whitespace, unsafe
field-value characters, malformed absolute URIs, malformed relative
references, and broken percent-encoding are errors. This is syntax validation
only: callers own redirect handling, cache variant selection, representation
replacement, route generation, retries, and status policy.

## Connection

`connection` parses one or more RFC 9110 `Connection` field values into an
ordered list of connection-option tokens. This is header-field metadata, not a
transport socket type. Each field value is bounded to 64 KiB, and the
cumulative token count across all supplied fields is bounded to 256 tokens.
Tokens are split on commas with SP and HTAB accepted only as optional
whitespace around each token; empty members and members containing forbidden
ASCII control bytes are rejected. Each token must be an RFC 9110 token, and
repeated tokens are retained in wire order with their original spelling. A
present header set that yields no token still fails as invalid. This parser
never fails open and does not apply keep-alive, hop-by-hop stripping, upgrade,
or HTTP/2 rejection policy.

## Sec-Purpose

`fetch_metadata::SecPurpose` parses a singleton `Sec-Purpose` request field as
a comma-separated list of HTTP tokens. Each field value is bounded to 64 KiB.
Tokens are split on commas with optional surrounding SP trimmed; empty members,
forbidden ASCII control bytes, malformed tokens, duplicate fields, and
case-insensitive duplicate tokens are rejected. Unknown extension tokens are
preserved with their original spelling, `contains_prefetch()` detects the
common `prefetch` token, and `header_value()` serializes the normalized
comma-space list. This is request metadata only; callers own browser policy,
prefetch execution, cache behavior, navigation handling, and request blocking.

## Upgrade

`upgrade` parses one or more HTTP/1 `Upgrade` field values into an ordered
list of protocol names. This is header-field metadata, not a socket handoff
type. Each field value is bounded to 64 KiB, and the cumulative protocol count
across all supplied fields is bounded to 32 protocols.

Protocols are split on commas with SP and HTAB accepted only as optional
whitespace around each protocol. A protocol is an RFC 9110 token, optionally
followed by `/` and a token protocol version. Empty members, forbidden ASCII
control bytes, malformed tokens, empty versions, nested `/` versions, and
over-limit protocol lists are rejected. This parser validates declared
metadata only; callers own `Connection: Upgrade`, h2c negotiation, socket
handoff, and any upgraded protocol bytes.

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

## Content-Security-Policy

`content_security_policy` parses one or more `Content-Security-Policy` field
values as opaque response metadata. Each field value is bounded to 64 KiB.
Absent fields, empty values, ASCII control bytes other than HTAB, and oversized
values are errors. Valid values are preserved exactly in wire order for
`header_values()`, with `as_str()` and `header_value()` returning the first
policy value. This parser does not evaluate directives, enforce browser
security policy, deliver violation reports, or change raw header availability.

## Content-Language

`content_language` parses one or more `Content-Language` field values into an
ordered list of concrete language tags, preserving each tag's spelling and
wire order. Each field value is bounded to 64 KiB, and the cumulative tag
count across all supplied fields is bounded to 256 tags. Tags are split on
commas with SP and HTAB accepted only as optional whitespace around each tag;
empty members, members containing forbidden ASCII control bytes, `*`, and
non-ASCII bytes are rejected. Each tag must match the supported BCP 47-shaped
grammar: language, optional extlang, script, region, variant, extension, and
private-use subtags, plus registered grandfathered tags. Duplicate tags are
rejected case-insensitively while valid spelling and order are preserved. A
present header set that yields no tag still fails as invalid. This parser
reports declared representation metadata only; it does not negotiate, infer, or
select languages.

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

## Host

`host` parses a singleton HTTP `Host` request field as one inbound authority
(`uri-host` plus optional port). Each field value is bounded to 64 KiB. A
second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. The parser
preserves the trimmed host and port spelling and does not canonicalize names,
IPv6 text, or default ports. Empty values, userinfo, path, query, fragment,
unbracketed IPv6, empty ports, ASCII controls, and other values outside the
inbound Host grammar are errors. This is syntax validation only: callers own
virtual-host routing and scheme defaults.

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

## Signature-Input

`signature_input` parses one or more RFC 9421 `Signature-Input` field values
into an ordered dictionary of labels, covered-component identifiers, and
opaque parameters. Each field value is bounded to 64 KiB, the combined entry
count is bounded to 256, each entry may carry at most 256 covered components
and 256 member parameters, and each component may carry at most 256
parameters. Members must be dictionary keys mapped to inner lists of strings.
Well-formed member parameters (`created`, `keyid`, `alg`, `nonce`, `tag`, and
unknown names) and well-formed component parameters are retained as opaque
data and are not interpreted. Duplicate labels keep the later value.
Non-inner-list members, non-string components, empty covered-component lists,
empty present fields, and other unparsable input are errors. This parser does
not sign, verify, look up keys, canonicalize covered components, or apply
cryptographic policy.

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

## Cache-Status

`cache_status` parses one or more `Cache-Status` response field values as a
bounded RFC 9211 / RFC 8941 `sf-list` of cache identifiers and parameters. Each
identifier is an `sf-token` or `sf-string`. Known parameters (`hit`, `fwd`,
`fwd-status`, `ttl`, `stored`, `collapsed`, `key`, and `detail`) are typed;
unknown well-formed parameters are retained as extension metadata. Each field
value is bounded to 64 KiB, the combined member count is bounded to 256, each
member is bounded to 256 parameters, and each parameter value is bounded to
64 KiB.

Repeated `Cache-Status` fields are concatenated in wire order into one list.
Empty fields, empty list members, inner lists, trailing commas, control bytes
other than HTAB, invalid Structured Fields grammar, and duplicate parameter
keys on one member are rejected. A member with neither `hit` nor `fwd`, or
with both, remains valid metadata. `ttl` is a signed integer and may be
negative.

The parser only reports bounded wire metadata. It does not store cache
entries, compute freshness, revalidate, select endpoints, retry, or change
response acceptance.

## CDN-Cache-Control

`cdn_cache_control` parses one or more `CDN-Cache-Control` response field
values into ordered directive metadata. It preserves CDN-specific extension
directives with each directive token name and optional parsed value. Each field
value is bounded to 64 KiB, the combined directive count is bounded to 256, and
directive names and unquoted values must be valid HTTP tokens. Quoted strings
must be well formed and are exposed as parsed values.

The parser only reports bounded wire metadata. It does not create a CDN cache,
compute freshness, evaluate surrogate keys, revalidate automatically, enforce
shared-cache policy, retry, replay, redirect, or choose response-acceptance
behavior.

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

## Proxy-Authenticate

`proxy_authenticate` parses one or more `Proxy-Authenticate` field values into
bounded proxy authentication challenge metadata. Each field value is bounded to
64 KiB, the combined challenge count is bounded to 256, each challenge keeps
its scheme, optional token68 value, and ordered auth-parameters. Each
challenge's parameter count is bounded to 256. Parameter values are bounded to
64 KiB, quoted-string values are unescaped, and duplicate parameter names
within a challenge are rejected case-insensitively.

`ProxyAuthenticate::parse()` validates a single field value, and
`ProxyAuthenticate::parse_values()` preserves challenges across multiple field
values. Empty input, empty members, malformed syntax, invalid tokens,
oversized values, excessive challenges or parameters, and duplicate parameter
names are rejected. This parser exposes proxy authentication challenges as
metadata only; it does not select credentials, retry requests, generate
`Proxy-Authorization`, or implement proxy authentication policy.

## Proxy-Status

`proxy_status` parses one or more RFC 9209 `Proxy-Status` field values as a
Structured Fields list of Token or String proxy identifiers with opaque
parameters. Each field value is bounded to 64 KiB, the combined member count
is bounded to 256, each member holds at most 256 parameters, and each
parameter value is bounded to 64 KiB. Combined fields are parsed in wire
order. Empty input, empty lists, inner-lists, non-Token/non-String
identifiers, malformed syntax, control bytes other than HTAB, oversized
values, excessive members or parameters, and duplicate parameter names are
rejected. This parser reports declared metadata only; it does not interpret
proxy health, promote trailers, retry requests, or generate origin
`Proxy-Status` values.

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

## Link

`link` parses one or more RFC 8288 `Link` field values into ordered `LinkValues`
and `LinkValue` metadata. Each value retains its target URI-reference and its
ordered parameters, including unknown extension parameters alongside `rel`.
Targets are validated structurally as RFC 3986 URI-references and stored as raw
text, never resolved, normalized, fetched, or preloaded; fragments are allowed.
Each field value is bounded to 64 KiB, the cumulative value count is bounded to
256, each value holds at most 256 parameters, and each parameter value is
bounded to 64 KiB. Parameter names are matched case-insensitively, stored
lowercase, and must be unique within a value. Quoted parameter values are
unescaped and valueless parameters are preserved with an empty value. Empty
input, empty members, malformed syntax, and duplicate parameter names are
rejected. This parser does not preload, schedule fetches, redirect, apply cache
policy, or generate routes.

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

## TE

`te` parses one or more RFC 9110 `TE` field values into an ordered list of
transfer codings with optional q-values. Each field value is bounded to
64 KiB, and the cumulative coding count across all supplied fields is bounded
to 32 codings.

Codings are split on commas with SP and HTAB accepted only as optional
whitespace around each coding. Each coding must be an RFC 9110 token;
`chunked` is rejected because request framing remains owned by the HTTP/1
implementation. `trailers` is accepted only without a parameter and carries no
q-value. Other codings accept one optional `q` parameter whose value is a
weight from `0` through `1` with at most three fractional digits, stored as
thousandths. Empty members, forbidden ASCII control bytes, malformed tokens,
multiple parameters, non-`q` parameter names, invalid q-values,
case-insensitive duplicate codings across all supplied fields, over-limit
coding lists, and empty present field sets are errors. This parser never fails
open and does not enable a transfer-coding engine, negotiate trailers, or
apply compression or proxy behavior.

## X-Frame-Options

`x_frame_options` parses a singleton `X-Frame-Options` response field. Each
field value is bounded to 64 KiB. A second field is rejected after every
supplied field is bound-checked. Surrounding SP and HTAB are trimmed as
optional whitespace. The value must be exactly `DENY` or `SAMEORIGIN`, matched
case-insensitively and formatted canonically in uppercase. Empty values,
comma-joined values, semicolon parameters, quoted values, `ALLOW-FROM`,
unsupported tokens, ASCII controls, and other ambiguous input are errors. This
parser reports declared metadata only; it does not decide whether a response
may be framed.

## Warning

`warning` parses `#warning-value` lists from RFC 7234 `Warning` fields. Each
field value is bounded to 64 KiB, the combined item count is bounded to 256,
and each unescaped warn-agent and warn-text is bounded to 64 KiB. Warn-codes
are any 3 ASCII digits; warn-agent is opaque non-space text; warn-text is a
quoted-string; an optional quoted HTTP-date is parsed with the same
`httpdate` helper as Sunset. Empty input, empty members, malformed quoting,
invalid codes, and bound violations are rejected. This parser does not
implement cache, freshness, stale-response, or response-acceptance policy.

## Access-Control-Allow-Credentials

`access_control_allow_credentials` parses a singleton
`Access-Control-Allow-Credentials` field. Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
The field value must be exactly the standards-defined `true` token, matched
case-sensitively per the Fetch `%s"true"` grammar and returned in canonical
lowercase wire form. Surrounding SP and HTAB are trimmed as optional
whitespace. Unknown tokens, lists, quoted values, empty values, control
bytes, and other unparsable input are errors.
This parser does not evaluate CORS requests or grant credentials automatically.

## Access-Control-Request-Method

`access_control_request_method` parses a singleton
`Access-Control-Request-Method` request field. Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. The value must be
exactly one HTTP method token and is returned in canonical ASCII-uppercase
form. The `*` token, comma-separated lists, empty values, control bytes,
oversized values, and other unparsable input are errors. This parser reports
declared preflight request metadata only; it does not decide whether a
preflight is needed or apply CORS policy.

## Save-Data

`save_data` parses a singleton `Save-Data` request field. Each field value is
bounded to 64 KiB. A second field is rejected after every supplied field is
bound-checked. The field value must be exactly the standards-defined `on`
token, matched case-sensitively and returned in canonical lowercase wire form.
Surrounding SP and HTAB are trimmed as optional whitespace. Unknown tokens,
lists, parameterized values, empty values, control bytes, and other
unparsable input are errors.
This parser does not apply reduced-data serving, content adaptation, or
browser data-saver policy.

## NEL

`nel` parses one `NEL` response field as a bounded JSON object exposing the W3C
Network Error Logging policy members `report_to`, `max_age`,
`include_subdomains`, `success_fraction`, and `failure_fraction` with checked
types. Each field value is bounded to 64 KiB, member counts are bounded to 256
per object, nesting depth is bounded to 64, and each decoded string is bounded
to 64 KiB. A second field is rejected after every supplied field is
bound-checked. `max_age` is required and must be a non-negative JSON integer
literal that fits in `u64`; fraction and exponent forms are rejected for this
member. Fractions must parse as finite doubles in the inclusive range `0.0` to
`1.0`. Malformed JSON, invalid member types, duplicate singleton members,
non-finite or out-of-range fractions, missing `max_age`, and bound violations
are errors. Unknown JSON members are preserved verbatim as raw metadata
without policy semantics. Absent optional members keep their W3C defaults
(`include_subdomains` `false`, `success_fraction` `0.0`, `failure_fraction`
`1.0`) but are not re-emitted by `header_value()`. This parser does not send
reports, persist policy, or configure Reporting endpoint groups.

## Keep-Alive

`keep_alive` parses RFC 2068 `Keep-Alive` fields as a comma-separated list of
`timeout=delta-seconds` and `max=1*DIGIT` parameters (both optional) with
case-insensitive parameter names and optional whitespace around separators.
Each field value is bounded to 64 KiB and the combined parameter count is
bounded to 256. Values are parsed as checked unsigned 64-bit integers;
unrecognized `name=token` parameters are preserved as bounded extension
metadata. Empty input, missing `=`, duplicate recognized parameters, malformed
tokens, overflow, and bound violations are rejected. This parser does not
change connection lifetime, connection pooling, or HTTP/2 behavior.

## No-Vary-Search

`no_vary_search` parses bounded Structured Fields dictionary metadata for the
`No-Vary-Search` response field. It exposes recognized `key-order`, `params`,
and `except` members, keeps extension dictionary members as metadata, and
formats a normalized header value. Each field value is limited to 64 KiB,
parameter lists are limited to 256 strings, and extension members are limited
to 64. The parser does not implement cache storage, cache-key matching, URL
normalization, navigation behavior, request replay, or shared-cache policy.
