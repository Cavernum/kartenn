//! Kubernetes connector of kartenn (spec 019, design « Connecteur »): a separate process,
//! client of the REST API of kartenn, that synchronises the clusters it is given
//! (REQ-019-18). This crate holds its configuration, its credentials read from files
//! (REQ-019-29) and the client of an API server (Q-019-12).

pub mod client;
pub mod config;
pub mod credentials;

pub use client::{ConnectError, connect, connect_with};
pub use config::{Cluster, Config, ConfigError, Credentials, Kartenn};
pub use credentials::{CredentialError, CredentialFile, Secret};
