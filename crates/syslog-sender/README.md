# syslog-sender

Syslog messages for the network, as the RFCs write them:

- [RFC 5424](https://www.rfc-editor.org/rfc/rfc5424) formatting, with validated header
  fields, structured data escaped as §6.3.3 requires, the UTF-8 BOM, and truncation at the
  end of the message on a character boundary (§6.1);
- the timestamp of the **event**, given by the caller, in UTC to the microsecond, rather
  than the time of sending;
- the [RFC 3164](https://www.rfc-editor.org/rfc/rfc3164) form that local sockets
  (`/dev/log`) expect;
- octet-counting framing ([RFC 6587 §3.4.1](https://www.rfc-editor.org/rfc/rfc6587#section-3.4.1),
  required over TLS by [RFC 5425 §4.3](https://www.rfc-editor.org/rfc/rfc5425#section-4.3))
  or line-feed framing, over local sockets, UDP, TCP and TLS (rustls), asynchronous on
  tokio, with reconnection after an exponential back-off and a clean TLS close.

Features: `tokio` (transports) and `tls` (rustls, ring provider), both on by default;
without them the crate only formats.

It depends on no logging framework. It was written for kartenn because no published crate
frames messages by octet counting or takes the time of the event; it is meant to be
published on its own.

Tests check the output against the examples of RFC 5424 §6.5 and, by property tests,
against an independent parser (`syslog_rfc5424`); transports are tested against local
TCP, TLS, UDP and Unix servers. `examples/send.rs` sends one message to a receiver.
