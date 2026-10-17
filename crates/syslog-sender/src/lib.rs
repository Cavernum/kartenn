// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Syslog messages for the network: [RFC 5424] formatting (and [RFC 3164] for local
//! sockets), validated header fields, structured data, and the timestamp of the event
//! rather than the time of sending.
//!
//! The crate has no dependency on any logging framework: callers build a [`Message`] from
//! their own events and write the bytes it formats.
//!
//! ```
//! use std::time::{Duration, UNIX_EPOCH};
//! use syslog_sender::{Facility, Message, SdElement, Severity};
//!
//! let at = UNIX_EPOCH + Duration::from_millis(1_065_910_455_003);
//! let message = Message::new(Facility::Auth, Severity::Critical)
//!     .timestamp(at)
//!     .hostname("mymachine.example.com")?
//!     .app_name("su")?
//!     .msgid("ID47")?
//!     .structured(SdElement::meta_sequence(1)?)?
//!     .text("'su root' failed for lonvick on /dev/pts/8");
//! let bytes = message.to_rfc5424(2048)?;
//! assert!(bytes.starts_with(b"<34>1 2003-10-11T22:14:15.003000Z mymachine.example.com su - ID47 [meta sequenceId=\"1\"] \xEF\xBB\xBF"));
//! # Ok::<(), syslog_sender::Error>(())
//! ```
//!
//! [RFC 5424]: https://www.rfc-editor.org/rfc/rfc5424
//! [RFC 3164]: https://www.rfc-editor.org/rfc/rfc3164

mod error;
mod field;
mod message;
mod priority;
mod structured;
mod time;
#[cfg(feature = "tokio")]
mod transport;

pub use error::Error;
pub use message::{Body, Message};
pub use priority::{Facility, Severity};
pub use structured::SdElement;
/// The rustls types a [`Target::Tls`] needs (`ClientConfig`, `ServerName`…).
#[cfg(feature = "tls")]
pub use tokio_rustls::rustls;
#[cfg(feature = "tokio")]
pub use transport::{Backoff, Framing, Sender, Target};
