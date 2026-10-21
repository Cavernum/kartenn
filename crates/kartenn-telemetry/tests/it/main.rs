//! The integration tests of kartenn-telemetry, one binary for the crate (ADR 0027): a
//! module per subject.

mod architecture;
mod audit_export;
mod audit_otlp;
mod audit_retention;
mod clock;
mod framing;
mod otel;
mod outputs;
mod panic;
