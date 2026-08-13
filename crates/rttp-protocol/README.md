# rttp-protocol

Shared HTTP wire syntax and framing primitives for the rttp client and server crates.

This crate supports rttp's implementation; its public API is not a standalone
application-level HTTP interface.

This crate owns shared bounded HTTP wire syntax and framing primitives: header
parsing and validation, structured-field and token syntax, media types, ranges,
and similar on-the-wire helpers shared by the client and server crates.
Application-level HTTP policy is owned by those crates, not by this one.

Protocol helpers define and bound wire metadata for the client and server
crates. They do not add higher-level runtime policy such as caching,
authentication, retries, representation selection, or body transformation.
