//! Adapters of kartenn's observability (spec 012): the outputs of the audit journal
//! ([`sinks`]), and later the outputs of the technical logs and OpenTelemetry. The core and
//! the service only know the ports (`AuditSink`, the `tracing` facade); the server
//! assembles these adapters from its configuration (REQ-012-2).

pub mod capture;
pub mod sinks;
