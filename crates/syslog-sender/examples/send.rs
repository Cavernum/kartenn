// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Sends one RFC 5424 message over TCP with octet counting:
//! `cargo run -p syslog-sender --example send -- 127.0.0.1:5514 "hello"`.

use std::time::SystemTime;

use syslog_sender::{Facility, Message, SdElement, Sender, Severity, Target};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let address = args.next().unwrap_or_else(|| "127.0.0.1:5514".into());
    let text = args
        .next()
        .unwrap_or_else(|| "hello from syslog-sender".into());
    let message = Message::new(Facility::User, Severity::Notice)
        .timestamp(SystemTime::now())
        .hostname("localhost")?
        .app_name("syslog-sender")?
        .procid(&std::process::id().to_string())?
        .msgid("example")?
        .structured(SdElement::meta_sequence(1)?)?
        .text(text)
        .to_rfc5424(2048)?;
    let mut sender = Sender::new(Target::Tcp(address));
    sender.send(&[message]).await?;
    sender.close().await?;
    Ok(())
}
