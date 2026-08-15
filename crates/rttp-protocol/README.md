# rttp-protocol

Shared HTTP wire syntax and framing primitives for the rttp client and server crates.

This crate owns shared, bounded HTTP wire syntax and framing primitives. It
does not own application-level HTTP policy; that policy belongs to the client
and server crates.

This crate supports rttp's implementation; its public API is not a standalone
application-level HTTP interface.

Protocol helpers define and bound wire metadata for the client and server
crates. They do not add higher-level runtime policy such as caching,
authentication, retries, representation selection, or body transformation.

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
