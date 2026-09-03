//! Blocking HTTP server implementation used by the `rttp` compatibility facade.
//!
//! Connection handoff responsibilities are split across modules:
//!
//! - `connection` accepts sockets, detects the wire protocol, and dispatches
//!   each connection. It drives the HTTP/1 request loop and owns the handoffs
//!   that move a connection off that loop: h2c upgrades transfer the buffered
//!   stream to `http2` handling, while CONNECT and Upgrade responses transfer
//!   the socket to handler code through `handoff`.
//! - `http1` provides HTTP/1 parsing and body helpers used by the connection
//!   loop; it does not accept sockets or perform handoffs.
//! - `http2` owns HTTP/2 framing, flow control, and request serving for
//!   prior-knowledge and h2c-upgraded connections, including any bytes buffered
//!   during the HTTP/1 handoff.
//!
//! The public server API is re-exported here while protocol and model concerns live in
//! focused internal modules.

use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use socket2::{Domain, Protocol, Socket, Type};
use url::Url;

mod byte_range;
mod connection;
mod handoff;
mod http1;
mod http2;
mod request;
mod response;

pub use byte_range::*;
pub use connection::HttpServer;
pub use handoff::*;
pub use http2::Http2ServerPolicy;
pub use request::*;
pub use response::*;

// Protocol modules share implementation details without exposing them from the crate.
pub(crate) use http1::*;
pub(crate) use http2::*;

#[cfg(test)]
include!("server/server_tests.rs");
